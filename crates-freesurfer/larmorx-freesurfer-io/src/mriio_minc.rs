// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! MINC 1 volumes (`utils/mriio.cpp` 2381-2592, item 3.1-27): `mincRead` ([`minc_read`]) over
//! volume_io's volume input (`larmorx_io::minc`, track C's port of MINC 1.5.1 with netCDF
//! 3.6.0-p1's reader) in the dialect of FreeSurfer's copy ([`crate::minc::FsMinc`]). Wired into
//! [`crate::mriio::mri_read`].
//!
//! The file is opened with the dimension names `xspace`, `yspace`, `zspace` and `time` in that
//! order, in its own type and sign; three or four dimensions are taken (the fourth the frames). The
//! types: bytes as `MRI_UCHAR`, 16-bit integers as `MRI_SHORT`, 32-bit integers as `MRI_LONG`,
//! floats (and, corrected, doubles) as `MRI_FLOAT`; each voxel is volume_io's voxel value (in the
//! file's valid range, not scaled to its real range) converted as C converts a `double`. The
//! voxel sizes are the steps' magnitudes, the axes' direction cosines negated for a negative step,
//! the centre the world position of voxel (width/2, height/2, depth/2).
//!
//! Defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-io.md`, Part 27):
//! FS-742 (signed bytes stored as unsigned bytes: -1 read as 255), FS-743 (unsigned 16-bit values
//! above 32,767 stored as negative numbers), FS-744 (32-bit integers stored as 32-bit numbers
//! packed into the first half of each row of 64-bit voxels, the rest of each row 0), FS-745 (a
//! file of five or more dimensions: volume_io reads the dimension names past mincRead's four).
//! The library's own defects (FS-774 to FS-796, FS-775 and FS-788 in FreeSurfer's dialect) follow
//! the same mode inside `larmorx_io::minc`.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{FS_58, FS_742, FS_743, FS_744, FS_745, FS_746, FS_747, FS_748};
use larmorx_freesurfer_core::cstd::{double_to_int_x86, fmt_f};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::file_exists;
use larmorx_freesurfer_mri::mri::{self, MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, Mri};
use larmorx_io::minc::{self, Ctx, Exit, Session, Status, Volume, netcdf};

use crate::minc::FsMinc;
use crate::mriio_nifti::narrow;
use crate::mriio_nrrd::replace_all;

/// The program's streams with the folder a relative name was opened in taken out of every
/// message, so that they name the file as the program does (it opens the name in its working
/// directory).
struct InDir<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    prefix: Option<Vec<u8>>,
}

impl InDir<'_> {
    fn strip(&self, b: &[u8]) -> Vec<u8> {
        match &self.prefix {
            Some(p) => replace_all(b, p, b""),
            None => b.to_vec(),
        }
    }
}

impl Streams for InDir<'_> {
    fn out(&mut self, text: &str) {
        self.out_bytes(text.as_bytes());
    }
    fn err(&mut self, text: &str) {
        self.err_bytes(text.as_bytes());
    }
    fn out_bytes(&mut self, bytes: &[u8]) {
        let b = self.strip(bytes);
        let _ = self.out.write_all(&b);
    }
    fn err_bytes(&mut self, bytes: &[u8]) {
        let b = self.strip(bytes);
        let _ = self.err.write_all(&b);
    }
}

/// The name volume_io opens (`fname` in `dir` for a relative name) and the prefix to take out of
/// its messages. On Windows the folder's backslashes are written as `/` (a separator there too):
/// volume_io's file names take a backslash as an escape (FS-79's `reproduce`).
fn in_dir(dir: Option<&Path>, fname: &str) -> (Vec<u8>, Option<Vec<u8>>) {
    let b = fname.as_bytes();
    match dir {
        Some(d)
            if !b.is_empty() && b[0] != b'/' && b[0] != b'~' && !Path::new(fname).is_absolute() =>
        {
            let mut p = d.to_string_lossy().into_owned().into_bytes();
            if cfg!(windows) && !p.starts_with(b"\\\\?\\") {
                for c in p.iter_mut().filter(|c| **c == b'\\') {
                    *c = b'/';
                }
            }
            p.push(b'/');
            let prefix = p.clone();
            p.extend_from_slice(b);
            (p, Some(prefix))
        }
        _ => (b.to_vec(), None),
    }
}

/// `(long) x` on x86-64 (`cvttsd2si` to 64 bits): truncated; a NaN or a value outside the range
/// gives the lowest `long`.
fn double_to_long_x86(x: f64) -> i64 {
    if x.is_nan() || x >= 9_223_372_036_854_775_808.0 || x < -9_223_372_036_854_775_808.0 {
        i64::MIN
    } else {
        x.trunc() as i64
    }
}

/// The geometry mincRead sets, kept apart so that a volume allocated again (a type widened,
/// FS-742/FS-743) gets it too.
struct Geometry {
    sizes: [f32; 3],
    dircos: [[f32; 3]; 3],
    centre: [f32; 3],
}

fn apply(m: &mut Mri, geo: &Geometry) {
    (m.vg.xsize, m.vg.ysize, m.vg.zsize) = (geo.sizes[0], geo.sizes[1], geo.sizes[2]);
    m.ps = m.vg.xsize;
    m.thick = m.vg.zsize;
    let d = &geo.dircos;
    (m.vg.x_r, m.vg.x_a, m.vg.x_s) = (d[0][0], d[0][1], d[0][2]);
    (m.vg.y_r, m.vg.y_a, m.vg.y_s) = (d[1][0], d[1][1], d[1][2]);
    (m.vg.z_r, m.vg.z_a, m.vg.z_s) = (d[2][0], d[2][1], d[2][2]);
    (m.vg.c_r, m.vg.c_a, m.vg.c_s) = (geo.centre[0], geo.centre[1], geo.centre[2]);
    m.vg.ras_good_flag = 1;
    // xend = xsize * width / 2.0: the product in float, the division in double
    let end = |size: f32, n: i32| narrow(f64::from(sse::mul(size, n as f32)) / 2.0);
    m.xend = end(m.vg.xsize, m.vg.width);
    m.xstart = -m.xend;
    m.yend = end(m.vg.ysize, m.vg.height);
    m.ystart = -m.yend;
    m.zend = end(m.vg.zsize, m.vg.depth);
    m.zstart = -m.zend;
    let xfov = sse::sub(m.xend, m.xstart);
    let yfov = sse::sub(m.yend, m.ystart);
    let zfov = sse::sub(m.zend, m.zstart);
    m.fov = if xfov > yfov {
        if xfov > zfov { xfov } else { zfov }
    } else if yfov > zfov {
        yfov
    } else {
        zfov
    };
}

/// The first byte of row `j` of slice `k` of frame `t`.
fn row_start(m: &Mri, j: usize, k: usize, t: usize) -> usize {
    let slice = t * m.vg.depth as usize + k;
    (slice * m.vg.height as usize + j) * m.vox_per_row as usize * m.bytes_per_vox
}

/// `mincRead(fname, read_volume)`: the volume, or `None` after FreeSurfer's message
/// (`mincRead(): can't find file <f>`, `mincRead(): error reading volume from file <f>` after
/// volume_io's own messages, `mincRead(): <n> dimensions in file; expecting 3 or 4`, `mincRead():
/// bad data type (<t>) in input file <f>`); `Err(Fatal::Exit)` where the library ends the program.
pub fn minc_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    let rep = g.notices.reproduce();
    if !file_exists(dir, fname) {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!("mincRead(): can't find file {fname}"),
        );
        return Ok(None);
    }
    let (path, prefix) = in_dir(dir, fname);
    let names: Vec<Vec<u8>> = [&b"xspace"[..], b"yspace", b"zspace", b"time"]
        .iter()
        .map(|n| n.to_vec())
        .collect();
    let mut s = Session::new();
    let mut vol: Option<Volume> = None;
    let exit = |e: Exit| Fatal::Exit(e.status);
    let (n, started) = {
        let mut streams = InDir {
            out: &mut *out,
            err: &mut *err,
            prefix: prefix.clone(),
        };
        let mut c = Ctx {
            streams: &mut streams,
            notices: &mut g.notices,
            dialect: &FsMinc,
        };
        // the dimension count first: start_volume_input counts them itself when given 0, as
        // FreeSurfer gives it, and fails after the count's messages when there are none (the same
        // reads and messages); given the count it does not count again
        let n = minc::get_minc_file_n_dimensions(&mut s, &mut c, &path).map_err(exit)?;
        if n >= 5 {
            // FS-745: volume_io reads n names from mincRead's array of four
            c.notices.add(
                &FS_745,
                format!(
                    "mincRead: a file of {n} dimensions refused before its dimension names are read (FreeSurfer: names read past its array of four, as memory holds)"
                ),
            );
        }
        if n < 1 {
            (n, Some((Status::Error, None)))
        } else if n >= 5 && !rep {
            (n, None)
        } else {
            let started = minc::start_volume_input(
                &mut s,
                &mut c,
                &path,
                n,
                Some(&names),
                0, // NC_UNSPECIFIED: the file's own type and sign
                false,
                0.0,
                0.0,
                true,
                &mut vol,
                None,
            )
            .map_err(exit)?;
            (n, Some(started))
        }
    };
    let Some((status, input)) = started else {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("mincRead(): {n} dimensions in file; expecting 3 or 4"),
        );
        return Ok(None);
    };
    if status != Status::Ok {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("mincRead(): error reading volume from file {fname}"),
        );
        return Ok(None);
    }
    let mut v = vol.expect("a volume when the input started");
    let mut input = input.expect("an input when the input started");
    let ndims = v.n_dimensions();
    if ndims != 3 && ndims != 4 {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("mincRead(): {ndims} dimensions in file; expecting 3 or 4"),
        );
        return Ok(None);
    }
    let mut dim_sizes = [0i32; 4];
    for (i, d) in dim_sizes.iter_mut().enumerate().take(ndims) {
        *d = v.sizes[i];
    }
    if ndims == 3 {
        dim_sizes[3] = 1;
    }
    let verbose = g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0;
    if verbose {
        let _ = writeln!(
            out,
            "DimSizes: {ndims}, {}, {}, {}, {}",
            dim_sizes[0], dim_sizes[1], dim_sizes[2], dim_sizes[3]
        );
        let _ = writeln!(out, "DataType: {}", v.nc_data_type);
    }
    let nc = v.nc_data_type;
    let mut dtype = match nc {
        netcdf::NC_BYTE | netcdf::NC_CHAR => MRI_UCHAR,
        netcdf::NC_SHORT => MRI_SHORT,
        netcdf::NC_INT => MRI_LONG,
        netcdf::NC_FLOAT | netcdf::NC_DOUBLE => MRI_FLOAT,
        _ => {
            g.errno = 0;
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!("mincRead(): bad data type ({nc}) in input file {fname}"),
            );
            return Ok(None);
        }
    };
    let (w, h, d, nf) = (dim_sizes[0], dim_sizes[1], dim_sizes[2], dim_sizes[3]);
    let mut m = if read_volume {
        mri::mri_alloc_sequence(g, out, err, w, h, d, dtype, nf)?
    } else {
        let mut m = mri::mri_alloc_header(g, out, err, w, h, d, dtype, nf)?;
        m.nframes = nf;
        m
    };
    // the geometry
    let sep = &v.separations;
    let mut geo = Geometry {
        sizes: [
            narrow(sep[0].abs()),
            narrow(sep[1].abs()),
            narrow(sep[2].abs()),
        ],
        dircos: [[0.0; 3]; 3],
        centre: [0.0; 3],
    };
    for (a, row) in geo.dircos.iter_mut().enumerate() {
        for (c, x) in row.iter_mut().enumerate() {
            *x = narrow(v.direction_cosines[a][c]);
        }
        if sep[a] < 0.0 {
            for x in row.iter_mut() {
                *x = -*x;
            }
        }
    }
    let world = {
        let mut streams = InDir {
            out: &mut *out,
            err: &mut *err,
            prefix: prefix.clone(),
        };
        let voxel = [
            f64::from(w) / 2.0,
            f64::from(h) / 2.0,
            f64::from(d) / 2.0,
            0.0,
            0.0,
        ];
        v.convert_voxel_to_world(&voxel, &FsMinc, &mut streams)
    };
    geo.centre = [narrow(world[0]), narrow(world[1]), narrow(world[2])];
    apply(&mut m, &geo);
    if read_volume {
        {
            let mut streams = InDir {
                out: &mut *out,
                err: &mut *err,
                prefix: prefix.clone(),
            };
            let mut c = Ctx {
                streams: &mut streams,
                notices: &mut g.notices,
                dialect: &FsMinc,
            };
            while minc::input_more_of_volume(&mut s, &mut c, &mut v, &mut input)
                .map_err(exit)?
                .0
            {}
        }
        let (wu, hu, du, nfu) = (w as usize, h as usize, d as usize, nf.max(0) as usize);
        let value = |i: usize, j: usize, k: usize, t: usize| v.voxel_value(&[i, j, k, t, 0]);
        let each = |f: &mut dyn FnMut(f64)| {
            for t in 0..nfu {
                for k in 0..du {
                    for j in 0..hu {
                        for i in 0..wu {
                            f(value(i, j, k, t));
                        }
                    }
                }
            }
        };
        // FS-742 / FS-743: values FreeSurfer's type cannot hold
        let signed_bytes = matches!(nc, netcdf::NC_BYTE | netcdf::NC_CHAR) && v.signed_flag;
        let unsigned_shorts = nc == netcdf::NC_SHORT && !v.signed_flag;
        if signed_bytes || unsigned_shorts {
            let (mut least, mut most) = (f64::INFINITY, f64::NEG_INFINITY);
            each(&mut |x| {
                least = least.min(x);
                most = most.max(x);
            });
            if signed_bytes && least < 0.0 {
                g.notices.add(
                    &FS_742,
                    format!(
                        "mincRead: signed 8-bit values read as 16-bit integers (FreeSurfer: unsigned bytes, {least} read as {})",
                        double_to_int_x86(least) as u8
                    ),
                );
                if !rep {
                    dtype = MRI_SHORT;
                }
            }
            if unsigned_shorts && most > 32767.0 {
                g.notices.add(
                    &FS_743,
                    format!(
                        "mincRead: unsigned 16-bit values read as 32-bit integers (FreeSurfer: signed 16-bit, {most} read as {})",
                        double_to_int_x86(most) as i16
                    ),
                );
                if !rep {
                    dtype = MRI_INT;
                }
            }
            if dtype != m.mtype {
                drop(m);
                m = mri::mri_alloc_sequence(g, out, err, w, h, d, dtype, nf)?;
                apply(&mut m, &geo);
            }
        }
        let long_packed = dtype == MRI_LONG && rep;
        let mut long_differs = false;
        let bpv = m.bytes_per_vox;
        for t in 0..nfu {
            for k in 0..du {
                for j in 0..hu {
                    let at = row_start(&m, j, k, t);
                    let Some(chunk) = m.chunk.as_mut() else {
                        continue;
                    };
                    let row = &mut chunk[at..at + wu * bpv];
                    if dtype == MRI_LONG {
                        // FreeSurfer's stores: 32-bit numbers at 4 i of the row of 64-bit voxels
                        let mut packed = vec![0u8; wu * 8];
                        let mut whole = vec![0u8; wu * 8];
                        for i in 0..wu {
                            let l = double_to_long_x86(value(i, j, k, t));
                            packed[4 * i..4 * i + 4].copy_from_slice(&(l as i32).to_le_bytes());
                            whole[8 * i..8 * i + 8].copy_from_slice(&l.to_le_bytes());
                        }
                        long_differs |= packed != whole;
                        row.copy_from_slice(if long_packed { &packed } else { &whole });
                        continue;
                    }
                    for i in 0..wu {
                        let x = value(i, j, k, t);
                        let p = &mut row[i * bpv..(i + 1) * bpv];
                        match dtype {
                            MRI_UCHAR => p[0] = double_to_int_x86(x) as u8,
                            MRI_SHORT => {
                                p.copy_from_slice(&(double_to_int_x86(x) as i16).to_le_bytes())
                            }
                            MRI_INT => p.copy_from_slice(&double_to_int_x86(x).to_le_bytes()),
                            _ => p.copy_from_slice(&narrow(x).to_bits().to_le_bytes()),
                        }
                    }
                }
            }
        }
        if long_differs {
            g.notices.add(
                &FS_744,
                "mincRead: 32-bit integers stored one to a 64-bit voxel (FreeSurfer: as 32-bit numbers in the first half of each row, the rest 0)",
            );
        }
    }
    if verbose {
        let mut streams = InDir {
            out: &mut *out,
            err: &mut *err,
            prefix: prefix.clone(),
        };
        let t = v.voxel_to_world_matrix(&FsMinc, &mut streams);
        let _ = writeln!(out, "MINC Linear Transform");
        for row in &t {
            let line: String = row
                .iter()
                .map(|&x| format!("{} ", fmt_f(x, 7, 4)))
                .collect();
            let _ = writeln!(out, "{line}");
        }
    }
    {
        let mut streams = InDir {
            out: &mut *out,
            err: &mut *err,
            prefix,
        };
        let mut c = Ctx {
            streams: &mut streams,
            notices: &mut g.notices,
            dialect: &FsMinc,
        };
        minc::delete_volume_input(&mut s, &mut c, input).map_err(exit)?;
    }
    Ok(Some(m))
}

/// `mincWrite(mri, fname)` (`utils/mriio.cpp` 2596-2760): the volume as a MINC 1 file through
/// volume_io's volume output (`larmorx_io::minc`, MINC 1.5.1's writer) in the dialect of FreeSurfer's
/// copy ([`FsMinc`]). `NO_ERROR`; `ERROR_BADPARM` after `mincWrite(): bad data type (<t>) in mri
/// structure` (an error; types other than bytes, 16- and 32-bit integers, longs and floats); 1 after
/// `ERROR: mincWrite: output_volume exited with 1` (standard output) when the output fails (its
/// messages first); `Err(Fatal::Exit)` where the library ends the program (a direction cosine of
/// length 0, a volume of more than one frame with `reproduce`: FS-797).
///
/// A volume whose RAS is not good gets the coronal direction cosines and centre 0 first, in the
/// caller's volume (`setDirectionCosine`). The file's type: bytes unsigned, 16-bit integers signed,
/// 32-bit integers and longs 32-bit signed, floats float; its voxel range the volume's least and
/// greatest values as floats (`MRIlimits`); dimensions `xspace`, `yspace`, `zspace` (and `time` with
/// more than one frame) of the volume's width, height, depth (and frames); steps the voxel sizes;
/// the axes' direction cosines the volume's columns' (made unit vectors); starts that put voxel
/// (width/2, height/2, depth/2) at the centre; every voxel's value as a C `double`; history "".
///
/// Defects: FS-746 (a long volume's values read as the 32-bit numbers of the first half of each row:
/// by default each voxel's value, clamped to the 32-bit range); FS-58 (a volume made without voxels:
/// FreeSurfer reads them through rows it does not have and crashes; refused, the program ends).
pub fn minc_write(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    m: &mut Mri,
    fname: &str,
) -> Result<i32, Fatal> {
    let rep = g.notices.reproduce();
    if m.vg.ras_good_flag == 0 {
        crate::mriio::set_direction_cosine(g, err, &mut m.vg, crate::mriio::MRI_CORONAL);
    }
    let (nc, signed) = match m.mtype {
        MRI_UCHAR => (netcdf::NC_BYTE, false),
        MRI_SHORT => (netcdf::NC_SHORT, true),
        MRI_INT | MRI_LONG => (netcdf::NC_INT, true),
        MRI_FLOAT => (netcdf::NC_FLOAT, true),
        t => {
            g.errno = 0;
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!("mincWrite(): bad data type ({t}) in mri structure"),
            );
            return Ok(ERROR_BADPARM);
        }
    };
    let (w, h, d, nf) = (m.vg.width, m.vg.height, m.vg.depth, m.nframes);
    let (wu, hu, du, nfu) = (
        w.max(0) as usize,
        h.max(0) as usize,
        d.max(0) as usize,
        nf.max(0) as usize,
    );
    let n_voxels = wu * hu * du * nfu;
    // MRIlimits: a volume without voxels leaves the range unset (memory never written)
    let (mut fmin, mut fmax) = (0.0f32, 0.0f32);
    larmorx_freesurfer_mri::info::mri_limits(g, err, m, &mut fmin, &mut fmax)?;
    // FS-746: the values a long volume gives (its 64-bit voxels, or FreeSurfer's 32-bit numbers
    // at 4 x in each row)
    let long_value = |m: &Mri, x: usize, y: usize, z: usize, t: usize, packed: bool| -> i64 {
        let chunk = m.chunk.as_ref().expect("voxels");
        let at = row_start(m, y, z, t);
        if packed {
            let b = &chunk[at + 4 * x..at + 4 * x + 4];
            i64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        } else {
            let b = &chunk[at + 8 * x..at + 8 * x + 8];
            i64::from_le_bytes(b.try_into().expect("8 bytes"))
        }
    };
    let mut min = larmorx_io::minc::libsrc::f32_to_f64(fmin);
    let mut max = larmorx_io::minc::libsrc::f32_to_f64(fmax);
    let ints = matches!(m.mtype, MRI_INT | MRI_LONG);
    if ints
        && n_voxels > 0
        && let Some(c) = m.chunk.as_ref()
    {
        // FS-746: a long volume's values (each voxel's, clamped to 32 bits, or FreeSurfer's 32-bit
        // numbers at 4 x in each row); FS-747: the exact range of the 32-bit values written
        let (mut differs, mut clamped) = (false, None);
        let (mut lo, mut hi) = (i64::MAX, i64::MIN);
        for t in 0..nfu {
            for z in 0..du {
                for y in 0..hu {
                    for x in 0..wu {
                        let v = if m.mtype == MRI_LONG {
                            let whole = long_value(m, x, y, z, t, false);
                            differs |= whole != long_value(m, x, y, z, t, true);
                            if clamped.is_none() && i32::try_from(whole).is_err() {
                                clamped = Some(whole);
                            }
                            whole.clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                        } else {
                            let at = row_start(m, y, z, t) + 4 * x;
                            i64::from(i32::from_le_bytes(
                                c[at..at + 4].try_into().expect("4 bytes"),
                            ))
                        };
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
            }
        }
        if differs || clamped.is_some() {
            g.notices.add(
                &FS_746,
                match clamped {
                    _ if rep => "mincWrite of a long volume: the 32-bit numbers of the first half of each row written, as in FreeSurfer".to_string(),
                    Some(l) => format!(
                        "mincWrite of a long volume: each voxel's value written, clamped to the file's 32-bit integers ({l} first; FreeSurfer: the 32-bit numbers in the first half of each row)"
                    ),
                    None => "mincWrite of a long volume: each voxel's value written (FreeSurfer: the 32-bit numbers in the first half of each row)".to_string(),
                },
            );
        }
        // MRIlimits' range as floats: past 2^24 it misses the values, and near the 32-bit limits
        // it passes them (the file's every voxel then -2147483648)
        let (elo, ehi) = (lo as f64, hi as f64);
        if !rep && (elo, ehi) != (min, max) {
            g.notices.add(
                &FS_747,
                format!(
                    "mincWrite: the valid range of 32-bit integers taken exactly, {elo} to {ehi} (FreeSurfer: its values as floats, {min} to {max})"
                ),
            );
            (min, max) = (elo, ehi);
        } else if rep && m.mtype == MRI_INT && (elo, ehi) != (min, max) {
            g.notices.add(
                &FS_747,
                format!(
                    "mincWrite: the valid range of 32-bit integers taken from their values as floats, {min} to {max}, as in FreeSurfer"
                ),
            );
        }
    }
    if m.mtype == MRI_FLOAT
        && min.is_nan()
        && n_voxels > 0
        && let Some(c) = m.chunk.as_ref()
    {
        // FS-748: MRIlimits starts from the first voxel; a NaN there stays the range whatever the
        // other voxels hold (the file's valid range NaN, volume_io's message)
        let (mut lo, mut hi) = (f32::NAN, f32::NAN);
        for t in 0..nfu {
            for z in 0..du {
                for y in 0..hu {
                    for x in 0..wu {
                        let at = row_start(m, y, z, t) + 4 * x;
                        let v = f32::from_le_bytes(c[at..at + 4].try_into().expect("4 bytes"));
                        if !v.is_nan() {
                            lo = if lo.is_nan() || v < lo { v } else { lo };
                            hi = if hi.is_nan() || v > hi { v } else { hi };
                        }
                    }
                }
            }
        }
        if !lo.is_nan() {
            let (lo, hi) = (f64::from(lo), f64::from(hi));
            if rep {
                g.notices.add(
                    &FS_748,
                    format!(
                        "mincWrite: a float volume whose first voxel is NaN written with the valid range NaN, as in FreeSurfer (its other values {lo} to {hi})"
                    ),
                );
            } else {
                g.notices.add(
                    &FS_748,
                    format!(
                        "mincWrite: a float volume whose first voxel is NaN written with the range of its other values, {lo} to {hi} (FreeSurfer: NaN)"
                    ),
                );
                (min, max) = (lo, hi);
            }
        }
    }
    let names: Vec<Vec<u8>> = [&b"xspace"[..], b"yspace", b"zspace", b"time"]
        .iter()
        .map(|n| n.to_vec())
        .collect();
    let n_dims = if nf == 1 { 3 } else { 4 };
    let (path, prefix) = in_dir(dir, fname);
    let exit = |e: Exit| Fatal::Exit(e.status);
    let mut s = Session::new();
    let f = larmorx_io::minc::libsrc::f32_to_f64;
    let vg = &m.vg;
    let seps = [f(vg.xsize), f(vg.ysize), f(vg.zsize), 1.0];
    let cosines = [
        [vg.x_r, vg.x_a, vg.x_s],
        [vg.y_r, vg.y_a, vg.y_s],
        [vg.z_r, vg.z_a, vg.z_s],
    ];
    let centre = [f(vg.c_r), f(vg.c_a), f(vg.c_s)];
    let mut vol = {
        let mut streams = InDir {
            out: &mut *out,
            err: &mut *err,
            prefix: prefix.clone(),
        };
        let mut c = Ctx {
            streams: &mut streams,
            notices: &mut g.notices,
            dialect: &FsMinc,
        };
        let mut vol = Volume::create_volume(
            &mut c,
            n_dims as i32,
            Some(&names[..n_dims]),
            nc,
            signed,
            min,
            max,
        )
        .map_err(exit)?
        .expect("3 or 4 dimensions");
        vol.set_sizes(&[w, h, d, nf]);
        vol.alloc_volume_data(&mut c);
        vol.set_separations(&seps);
        for (axis, cos) in cosines.iter().enumerate() {
            vol.set_direction_cosine(
                axis as i32,
                &[f(cos[0]), f(cos[1]), f(cos[2])],
                c.dialect,
                c.streams,
            )
            .map_err(exit)?;
        }
        vol.set_translation(
            &[
                f64::from(w) / 2.0,
                f64::from(h) / 2.0,
                f64::from(d) / 2.0,
                0.0,
                0.0,
            ],
            &centre,
            c.dialect,
            c.streams,
        );
        vol
    };
    if m.chunk.is_none() && n_voxels > 0 {
        // FreeSurfer reads the first voxel through rows the volume does not have (a crash)
        g.notices.add(
            &FS_58,
            "mincWrite of a volume made without voxels".to_string(),
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "mincWrite(): the volume has no voxels (FreeSurfer crashes here)\n",
        )?;
    } else if m.chunk.is_none() {
        g.notices.add(
            &FS_58,
            "mincWrite of a volume made without voxels: its range taken as 0 (FreeSurfer: memory never written)".to_string(),
        );
    }
    if let Some(chunk) = m.chunk.as_ref() {
        let bpv = m.bytes_per_vox;
        for t in 0..nfu {
            for x in 0..wu {
                for y in 0..hu {
                    for z in 0..du {
                        let at = row_start(m, y, z, t) + x * bpv;
                        let v = match m.mtype {
                            MRI_UCHAR => f64::from(chunk[at]),
                            MRI_SHORT => f64::from(i16::from_le_bytes([chunk[at], chunk[at + 1]])),
                            MRI_INT => f64::from(i32::from_le_bytes(
                                chunk[at..at + 4].try_into().expect("4 bytes"),
                            )),
                            MRI_LONG => {
                                let l = long_value(m, x, y, z, t, rep);
                                l.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as f64
                            }
                            _ => f(f32::from_le_bytes(
                                chunk[at..at + 4].try_into().expect("4 bytes"),
                            )),
                        };
                        vol.set_voxel_value(&[x, y, z, t, 0], v);
                    }
                }
            }
        }
    }
    let status = {
        let mut streams = InDir {
            out: &mut *out,
            err: &mut *err,
            prefix,
        };
        let mut c = Ctx {
            streams: &mut streams,
            notices: &mut g.notices,
            dialect: &FsMinc,
        };
        minc::output_volume(
            &mut s,
            &mut c,
            &path,
            nc,
            signed,
            min,
            max,
            &mut vol,
            Some(b""),
            None,
        )
        .map_err(exit)?
    };
    if status != Status::Ok {
        let _ = writeln!(out, "ERROR: mincWrite: output_volume exited with 1");
        return Ok(1);
    }
    Ok(log::NO_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    /// A netCDF classic file of fixed-size variables: `dims` (name, size), `vars` (name,
    /// dimension ids, text attributes, double attributes, type, the values' big-endian bytes).
    #[allow(clippy::type_complexity)]
    fn cdf(
        dims: &[(&str, u32)],
        vars: &[(
            &str,
            &[u32],
            &[(&str, &str)],
            &[(&str, &[f64])],
            i32,
            Vec<u8>,
        )],
    ) -> Vec<u8> {
        fn name(o: &mut Vec<u8>, n: &[u8]) {
            o.extend_from_slice(&(n.len() as u32).to_be_bytes());
            o.extend_from_slice(n);
            while !o.len().is_multiple_of(4) {
                o.push(0);
            }
        }
        let header = |begins: &[u32]| {
            let mut o = b"CDF\x01".to_vec();
            o.extend_from_slice(&0u32.to_be_bytes());
            o.extend_from_slice(&0x0Au32.to_be_bytes());
            o.extend_from_slice(&(dims.len() as u32).to_be_bytes());
            for (n, s) in dims {
                name(&mut o, n.as_bytes());
                o.extend_from_slice(&s.to_be_bytes());
            }
            o.extend_from_slice(&[0; 8]);
            o.extend_from_slice(&0x0Bu32.to_be_bytes());
            o.extend_from_slice(&(vars.len() as u32).to_be_bytes());
            for ((n, ids, texts, nums, ty, data), b) in vars.iter().zip(begins) {
                name(&mut o, n.as_bytes());
                o.extend_from_slice(&(ids.len() as u32).to_be_bytes());
                for d in *ids {
                    o.extend_from_slice(&d.to_be_bytes());
                }
                let natt = texts.len() + nums.len();
                if natt == 0 {
                    o.extend_from_slice(&[0; 8]);
                } else {
                    o.extend_from_slice(&0x0Cu32.to_be_bytes());
                    o.extend_from_slice(&(natt as u32).to_be_bytes());
                    for (an, t) in *texts {
                        name(&mut o, an.as_bytes());
                        o.extend_from_slice(&2u32.to_be_bytes());
                        name(&mut o, t.as_bytes());
                    }
                    for (an, v) in *nums {
                        name(&mut o, an.as_bytes());
                        o.extend_from_slice(&6u32.to_be_bytes());
                        o.extend_from_slice(&(v.len() as u32).to_be_bytes());
                        for x in *v {
                            o.extend_from_slice(&x.to_bits().to_be_bytes());
                        }
                    }
                }
                o.extend_from_slice(&(*ty as u32).to_be_bytes());
                o.extend_from_slice(&(data.len().div_ceil(4) as u32 * 4).to_be_bytes());
                o.extend_from_slice(&b.to_be_bytes());
            }
            o
        };
        let mut begins = vec![0u32; vars.len()];
        let mut off = header(&begins).len() as u32;
        for (b, v) in begins.iter_mut().zip(vars) {
            *b = off;
            off += v.5.len().div_ceil(4) as u32 * 4;
        }
        let mut o = header(&begins);
        for v in vars {
            o.extend_from_slice(&v.5);
            while !o.len().is_multiple_of(4) {
                o.push(0);
            }
        }
        o
    }

    const DIMS: [(&str, u32); 3] = [("zspace", 1), ("yspace", 1), ("xspace", 2)];

    fn minc(ty: i32, sign: &str, data: Vec<u8>) -> Vec<u8> {
        cdf(
            &DIMS,
            &[
                (
                    "xspace",
                    &[],
                    &[],
                    &[("step", &[-2.0]), ("start", &[10.0])],
                    4,
                    vec![0; 4],
                ),
                ("image", &[0, 1, 2], &[("signtype", sign)], &[], ty, data),
            ],
        )
    }

    fn read(dir: &Path, b: &[u8], mode: UpstreamDefects) -> (Option<Mri>, String, String) {
        std::fs::write(dir.join("v.mnc"), b).unwrap();
        let mut g = Globals::new(mode);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let m = minc_read(&mut g, &mut out, &mut err, Some(dir), "v.mnc", true).unwrap();
        let s = |b: Vec<u8>| String::from_utf8_lossy(&b).into_owned();
        (m, s(out), s(err))
    }

    /// Signed bytes: `fix` 16-bit voxels, `reproduce` FreeSurfer's unsigned bytes; a negative step
    /// gives a positive voxel size and the axis's direction negated.
    #[test]
    fn signed_bytes_and_a_negative_step() {
        let dir = tempfile::tempdir().unwrap();
        let b = minc(1, "signed__", vec![0xff, 5]);
        let (m, _, err) = read(dir.path(), &b, UpstreamDefects::Fix);
        let m = m.unwrap_or_else(|| panic!("{err}"));
        assert_eq!(m.mtype, MRI_SHORT);
        assert_eq!(m.chunk.as_deref(), Some(&[0xff, 0xff, 5, 0][..]));
        assert_eq!(
            (m.vg.xsize, m.vg.x_r, m.vg.x_a, m.vg.x_s),
            (2.0, -1.0, -0.0, -0.0)
        );
        // voxel 1 (of 2) at x = 10 - 2 = 8
        assert_eq!(m.vg.c_r, 8.0);
        let (m, _, _) = read(dir.path(), &b, UpstreamDefects::Reproduce);
        let m = m.unwrap();
        assert_eq!(
            (m.mtype, m.chunk.as_deref()),
            (MRI_UCHAR, Some(&[0xff, 5][..]))
        );
    }

    /// 32-bit integers: `reproduce` packs them as 32-bit numbers into the row's first half.
    #[test]
    fn long_voxels_both_ways() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = 7i32.to_be_bytes().to_vec();
        d.extend_from_slice(&(-2i32).to_be_bytes());
        let b = minc(4, "signed__", d);
        let (m, _, _) = read(dir.path(), &b, UpstreamDefects::Fix);
        let m = m.unwrap();
        assert_eq!(m.mtype, MRI_LONG);
        let mut want = 7i64.to_le_bytes().to_vec();
        want.extend_from_slice(&(-2i64).to_le_bytes());
        assert_eq!(m.chunk.as_deref(), Some(&want[..]));
        let (m, _, _) = read(dir.path(), &b, UpstreamDefects::Reproduce);
        let c = m.unwrap().chunk.unwrap();
        assert_eq!(&c[..8], &[7, 0, 0, 0, 0xfe, 0xff, 0xff, 0xff]);
        assert_eq!(&c[8..], &[0u8; 8]);
    }

    /// The refusals: no file, not a netCDF file, five dimensions (`fix`).
    #[test]
    fn refusals() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let m = minc_read(&mut g, &mut out, &mut err, Some(dir.path()), "no.mnc", true).unwrap();
        assert!(m.is_none());
        assert!(String::from_utf8_lossy(&err).contains("mincRead(): can't find file no.mnc"));
        let (m, _, err) = read(dir.path(), b"not netCDF", UpstreamDefects::Fix);
        assert!(m.is_none());
        assert!(
            err.contains("mincRead(): error reading volume from file v.mnc"),
            "{err}"
        );
        assert!(!err.contains(&*dir.path().to_string_lossy()), "{err}");
        let five = cdf(
            &[
                ("foo", 1),
                ("zspace", 1),
                ("yspace", 1),
                ("xspace", 1),
                ("time", 1),
            ],
            &[("image", &[0, 1, 2, 3, 4], &[], &[], 3, vec![0, 1])],
        );
        let (m, _, err) = read(dir.path(), &five, UpstreamDefects::Fix);
        assert!(m.is_none());
        assert!(
            err.contains("mincRead(): 5 dimensions in file; expecting 3 or 4"),
            "{err}"
        );
    }

    /// A volume of `ty` of `w` x 1 x 1 voxels (one frame) holding `vals` (little-endian bytes of
    /// each), coronal, RAS good.
    fn vol(ty: i32, vals: &[Vec<u8>]) -> Mri {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut m = mri::mri_alloc_sequence(&mut g, &mut o, &mut e, vals.len() as i32, 1, 1, ty, 1)
            .unwrap();
        m.vg.ras_good_flag = 1;
        (m.vg.x_r, m.vg.x_a, m.vg.x_s) = (-1.0, 0.0, 0.0);
        (m.vg.y_r, m.vg.y_a, m.vg.y_s) = (0.0, 0.0, -1.0);
        (m.vg.z_r, m.vg.z_a, m.vg.z_s) = (0.0, 1.0, 0.0);
        let c = m.chunk.as_mut().unwrap();
        let mut at = 0;
        for v in vals {
            c[at..at + v.len()].copy_from_slice(v);
            at += m.bytes_per_vox;
        }
        m
    }

    /// Written in `mode`, read back by the default reader: the voxels' bytes, the type, the notices
    /// of the write and its standard error.
    fn round_trip(
        m: &mut Mri,
        mode: UpstreamDefects,
    ) -> (i32, Option<Vec<u8>>, Vec<String>, String) {
        let dir = tempfile::tempdir().unwrap();
        let mut g = Globals::new(mode);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let r = minc_write(&mut g, &mut out, &mut err, Some(dir.path()), m, "o.mnc");
        let ids: Vec<String> = g
            .notices
            .as_slice()
            .iter()
            .map(|n| n.defect.id.to_string())
            .collect();
        let e = String::from_utf8_lossy(&err).into_owned();
        if r != Ok(0) {
            return (-1, None, ids, e);
        }
        let mut g = Globals::new(UpstreamDefects::Fix);
        let b = minc_read(&mut g, &mut out, &mut err, Some(dir.path()), "o.mnc", true)
            .unwrap()
            .unwrap();
        (b.mtype, b.chunk, ids, e)
    }

    fn as_i64(c: Option<Vec<u8>>) -> Vec<i64> {
        c.unwrap()
            .chunks(8)
            .map(|x| i64::from_le_bytes(x.try_into().unwrap()))
            .collect()
    }

    fn as_bits(c: Option<Vec<u8>>) -> Vec<u32> {
        c.unwrap()
            .chunks(4)
            .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
            .collect()
    }

    fn ints(xs: &[i32]) -> Vec<Vec<u8>> {
        xs.iter().map(|x| x.to_le_bytes().to_vec()).collect()
    }

    fn floats(xs: &[u32]) -> Vec<Vec<u8>> {
        xs.iter().map(|x| x.to_le_bytes().to_vec()).collect()
    }

    /// FS-747: 32-bit integers past 2^24 and near the limits (read back as 32-bit integers into
    /// `MRI_LONG`); FreeSurfer's float range sinks every voxel to -2147483648 near the limits.
    #[test]
    fn integer_ranges() {
        for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
            let mut m = vol(MRI_INT, &ints(&[16_777_217, 0, -16_777_217, 5]));
            let (ty, b, ids, _) = round_trip(&mut m, mode);
            assert_eq!(ty, MRI_LONG);
            assert_eq!(as_i64(b), [16_777_217, 0, -16_777_217, 5]);
            assert!(ids.contains(&"FS-747".to_string()), "{ids:?}");
        }
        let edges = [2_147_483_647, -2_147_483_648, 1, 123_456_789];
        let mut m = vol(MRI_INT, &ints(&edges));
        let (_, b, ids, _) = round_trip(&mut m, UpstreamDefects::Reproduce);
        assert_eq!(as_i64(b), [-2_147_483_648; 4]);
        assert!(ids.contains(&"FS-747".to_string()), "{ids:?}");
        let mut m = vol(MRI_INT, &ints(&edges));
        let (_, b, ids, _) = round_trip(&mut m, UpstreamDefects::Fix);
        assert_eq!(as_i64(b), edges.map(i64::from));
        assert_eq!(ids, ["FS-747"]);
        // small values: the same file both ways, no notice
        let mut m = vol(MRI_INT, &ints(&[3, -4, 7]));
        let (_, b, ids, _) = round_trip(&mut m, UpstreamDefects::Fix);
        assert_eq!(as_i64(b), [3, -4, 7]);
        assert!(ids.is_empty(), "{ids:?}");
    }

    /// FS-746: a long volume. FreeSurfer writes the 32-bit numbers of each row's first half; by
    /// default each voxel's value, clamped to 32 bits.
    #[test]
    fn long_volumes() {
        let longs = |xs: &[i64]| {
            xs.iter()
                .map(|x| x.to_le_bytes().to_vec())
                .collect::<Vec<_>>()
        };
        let vals = [7i64, -2, 3_000_000_000, 5];
        let mut m = vol(MRI_LONG, &longs(&vals));
        let (_, b, ids, _) = round_trip(&mut m, UpstreamDefects::Fix);
        assert_eq!(as_i64(b), [7, -2, 2_147_483_647, 5]);
        assert!(ids.contains(&"FS-746".to_string()), "{ids:?}");
        // FreeSurfer: 7 and -2 as 32-bit halves: 7, 0, -2, -1
        let mut m = vol(MRI_LONG, &longs(&[7, -2, 9, 5]));
        let (_, b, ids, _) = round_trip(&mut m, UpstreamDefects::Reproduce);
        assert_eq!(as_i64(b), [7, 0, -2, -1]);
        assert!(ids.contains(&"FS-746".to_string()), "{ids:?}");
    }

    /// Floats: NaN and infinities are written; MINC's reader takes infinities to the largest
    /// floats. FS-748: a NaN first voxel makes the file's valid range NaN (volume_io's message).
    #[test]
    fn float_values() {
        for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
            let mut m = vol(MRI_FLOAT, &floats(&[0x3f80_0000, 0x7f80_0000, 0xff80_0000]));
            let (ty, b, _, _) = round_trip(&mut m, mode);
            assert_eq!(ty, MRI_FLOAT);
            assert_eq!(as_bits(b), [0x3f80_0000, 0x7f7f_ffff, 0xff7f_ffff]);
            // a signalling NaN made quiet
            let mut m = vol(MRI_FLOAT, &floats(&[0x3f80_0000, 0x7fa0_0001, 0x4000_0000]));
            let (_, b, _, _) = round_trip(&mut m, mode);
            assert_eq!(as_bits(b), [0x3f80_0000, 0x7fe0_0001, 0x4000_0000]);
        }
        let first_nan = floats(&[0x7fc0_0000, 0x3f80_0000, 0x4000_0000]);
        let mut m = vol(MRI_FLOAT, &first_nan);
        let (_, b, ids, e) = round_trip(&mut m, UpstreamDefects::Reproduce);
        assert_eq!(as_bits(b), [0x7fc0_0000, 0x3f80_0000, 0x4000_0000]);
        assert!(
            e.contains("Volume has invalid min and max voxel value"),
            "{e}"
        );
        assert!(ids.contains(&"FS-748".to_string()));
        let mut m = vol(MRI_FLOAT, &first_nan);
        let (_, b, ids, e) = round_trip(&mut m, UpstreamDefects::Fix);
        assert_eq!(as_bits(b), [0x7fc0_0000, 0x3f80_0000, 0x4000_0000]);
        assert!(e.is_empty(), "{e}");
        assert_eq!(ids, ["FS-748"]);
    }

    /// The refusals and the program's ends: a type the writer does not take, a volume made without
    /// voxels (FS-58), a zero direction cosine (the copy's exit), more than one frame (FS-797:
    /// FreeSurfer's copy ends the program; by default written).
    #[test]
    fn refusals_and_exits() {
        let dir = tempfile::tempdir().unwrap();
        let run = |m: &mut Mri, mode| {
            let mut g = Globals::new(mode);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let r = minc_write(&mut g, &mut out, &mut err, Some(dir.path()), m, "o.mnc");
            let s = |b: Vec<u8>| String::from_utf8_lossy(&b).into_owned();
            (r, s(out), s(err))
        };
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut m =
            mri::mri_alloc_sequence(&mut g, &mut o, &mut e, 2, 1, 1, mri::MRI_USHRT, 1).unwrap();
        let (r, _, err) = run(&mut m, UpstreamDefects::Fix);
        assert_eq!(r, Ok(ERROR_BADPARM));
        assert!(
            err.contains("mincWrite(): bad data type (10) in mri structure"),
            "{err}"
        );
        let mut m = mri::mri_alloc_header(&mut g, &mut o, &mut e, 2, 1, 1, MRI_UCHAR, 1).unwrap();
        m.nframes = 1;
        let (r, _, err) = run(&mut m, UpstreamDefects::Reproduce);
        assert!(matches!(r, Err(Fatal::Exit(_))), "{r:?}");
        assert!(
            err.contains("mincWrite(): the volume has no voxels"),
            "{err}"
        );
        // RAS not good: the coronal cosines set in the caller's volume, then written
        let mut m = vol(MRI_UCHAR, &[vec![1], vec![2]]);
        m.vg.ras_good_flag = 0;
        (m.vg.x_r, m.vg.x_a, m.vg.x_s) = (0.0, 0.0, 0.0);
        let (r, _, _) = run(&mut m, UpstreamDefects::Fix);
        assert_eq!(r, Ok(0));
        assert_eq!((m.vg.x_r, m.vg.z_a), (-1.0, 1.0));
        let mut m = vol(MRI_UCHAR, &[vec![1], vec![2]]);
        (m.vg.y_r, m.vg.y_a, m.vg.y_s) = (0.0, 0.0, 0.0);
        let (r, _, err) = run(&mut m, UpstreamDefects::Fix);
        assert_eq!(r, Err(Fatal::Exit(1)));
        assert!(
            err.contains("Warning: zero length direction cosine"),
            "{err}"
        );
        let mut m = mri::mri_alloc_sequence(&mut g, &mut o, &mut e, 2, 1, 1, MRI_UCHAR, 2).unwrap();
        m.vg.ras_good_flag = 1;
        (m.vg.x_r, m.vg.y_s, m.vg.z_a) = (-1.0, -1.0, 1.0);
        let (r, _, err) = run(&mut m, UpstreamDefects::Reproduce);
        assert_eq!(r, Err(Fatal::Exit(1)));
        assert!(
            err.contains("convert_spatial_axis_to_dim_name, bad axis"),
            "{err}"
        );
        let mut m = mri::mri_alloc_sequence(&mut g, &mut o, &mut e, 2, 1, 1, MRI_UCHAR, 2).unwrap();
        m.vg.ras_good_flag = 1;
        (m.vg.x_r, m.vg.y_s, m.vg.z_a) = (-1.0, -1.0, 1.0);
        let (r, _, err) = run(&mut m, UpstreamDefects::Fix);
        assert_eq!(r, Ok(0), "{err}");
        let mut g = Globals::new(UpstreamDefects::Fix);
        let b = minc_read(&mut g, &mut o, &mut e, Some(dir.path()), "o.mnc", true)
            .unwrap()
            .unwrap();
        assert_eq!(b.nframes, 2);
        // a missing folder: the output's message
        let mut m = vol(MRI_UCHAR, &[vec![1]]);
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let r = minc_write(
            &mut g,
            &mut out,
            &mut err,
            Some(dir.path()),
            &mut m,
            "no/o.mnc",
        );
        assert_eq!(r, Ok(1));
        assert!(
            String::from_utf8_lossy(&out).contains("ERROR: mincWrite: output_volume exited with 1"),
            "{}",
            String::from_utf8_lossy(&out)
        );
    }
}
