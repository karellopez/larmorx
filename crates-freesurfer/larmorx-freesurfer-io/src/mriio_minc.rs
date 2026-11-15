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
use larmorx_core::defects::{FS_742, FS_743, FS_744, FS_745};
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
        Some(d) if !b.is_empty() && b[0] != b'/' && b[0] != b'~' && !Path::new(fname).is_absolute() => {
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
            let line: String = row.iter().map(|&x| format!("{} ", fmt_f(x, 7, 4))).collect();
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
