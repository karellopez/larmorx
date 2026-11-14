// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The old Siemens Numaris `.ima` series as FreeSurfer 8.2.0 reads it: `siemensRead`
//! (`utils/mriio.cpp`) with the `IMAFILEINFO` header reader it calls (`utils/imautils.cpp`:
//! `imaLoadVal`, `imaLoadValFromKey`, the dictionary, `imaParseName`, `imaCountFilesInSeries`,
//! `imaLoadFileInfo`), wired into [`crate::mriio::mri_read`] (`SIEMENS_FILE`, relay 29 of track A).
//!
//! `siemensRead` first asks [`crate::dicom_probe::is_dicom`] whether the file is really a DICOM
//! (a newer `.ima`): if so it prints the three `INFO:` lines and loads it as a Siemens DICOM
//! (`sdcmLoadVolume`, queue item 3.7-14, not ported). For the old format it builds the
//! `IMAFILEINFO` (which decides whether the series is a mosaic), checks the name ends in `.ima` or
//! `.IMA`, counts the files around this one by name (`<study>-<series>-<n>.ima`, or `_` for the
//! separator), reads the geometry straight from fixed byte offsets of the lowest-numbered file
//! (every value big-endian, as the files were written on a Sun), and, with the voxels, reads each
//! file's pixels at offset 6,144 and lays the slices out of their mosaics.
//!
//! The header reader `imaLoadFileInfo` reads some of its fields into stack variables through
//! `imaLoadValFromKey`, which frees its buffer when a read fails — the buffer is then a stack
//! address and `free` aborts the program (FS-688, a (memory corruption): a crash). It strikes
//! every `.ima` whose header is shorter than 4,998 bytes. By default larmorx does not crash: it
//! reads what the file holds (0 past its end) and goes on; `reproduce` cannot abort deterministically,
//! so the fs-io probe's piece 21 does not compare those lines. FS-689: fields `siemensRead` reads
//! past the end of a header between 4,998 and 5,016 bytes are memory contents (0 by default).
//! FS-690: `imaLoadFileInfo`'s row/column/slice direction cosines store the Sagittal and Coronal
//! parts and then overwrite the Coronal with the Transverse, never filling the third element (a
//! copy-and-paste slip); no program reads these fields (`siemensRead` reads the cosines itself), so
//! larmorx keeps the behaviour and documents it (no defect constant, as nothing produces it).
//! FS-691: `siemensRead` reads the field-of-view
//! width into the height's value and then byte-swaps the height a second time, so the width is
//! ignored and the field of view is the height (the second value is a byte-swap artefact that only
//! wins for pathological bytes); kept and documented.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_688, FS_689, FS_691, FS_692};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::machine;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::{cstd, utils};
use larmorx_freesurfer_mri::mri::{self, MRI_SHORT, Mri};

use crate::cfile::CFile;
use crate::fio;

/// `RADIANS(x)` (`macros.h`): `x * M_PI / 180`.
fn radians(x: f64) -> f64 {
    x * std::f64::consts::PI / 180.0
}

/// A big-endian value of `n` bytes at `at` in `fp` (from the start), 0-filled past the file's end;
/// `false` when the file ends first. `siemensRead` reads most fields this way
/// (`fseek; fread(&d, k, 1, fp)`), prints `siemensRead(): could not read file` without stopping
/// on a short read, and uses whatever was in the stack variable (FS-689: memory contents).
fn read_be(fp: &mut CFile, at: i64, n: usize) -> ([u8; 8], bool) {
    let mut errno = 0;
    fp.fseek(&mut errno, at, 0);
    let mut b = [0u8; 8];
    let got = fp.read_bytes(&mut b[..n]);
    (b, got == n)
}

fn be_i16(b: [u8; 8]) -> i16 {
    i16::from_be_bytes([b[0], b[1]])
}
fn be_i32(b: [u8; 8]) -> i32 {
    i32::from_be_bytes([b[0], b[1], b[2], b[3]])
}
fn be_f64(b: [u8; 8]) -> f64 {
    f64::from_be_bytes(b)
}

/// `imaParseName(imafile, ...)`: whether the base name splits into three fields
/// `<study><sep><series><sep><image>` (the extension dropped), the separator `-` tried first and
/// then `_`; `false` after `ERROR: could not parse <imafile>` on standard output when neither
/// separator gives three parts. The numbers themselves are not needed by `siemensRead`, which
/// finds the series by the name's own last number.
fn ima_parse_name(out: &mut dyn Write, imafile: &str) -> bool {
    let base = fio::fio_basename(Some(imafile), None).unwrap_or_default();
    let b = base.as_bytes();
    // the separator is decided by the first field: `-` when the name holds one, else `_`; once
    // chosen FreeSurfer does not fall back (the first field is read until the first `-`, and only
    // a name with no `-` at all retries with `_`).
    let sep = if b.contains(&b'-') {
        b'-'
    } else if b.contains(&b'_') {
        b'_'
    } else {
        let _ = writeln!(out, "ERROR: could not parse {imafile}");
        return false;
    };
    let p1 = b.iter().position(|&c| c == sep).unwrap();
    let rest = &b[p1 + 1..];
    let fail = |out: &mut dyn Write| {
        let _ = writeln!(out, "ERROR: could not parse {imafile}");
        false
    };
    let Some(p2) = rest.iter().position(|&c| c == sep) else {
        return fail(out);
    };
    let third = &rest[p2 + 1..];
    if !third.contains(&b'.') {
        return fail(out);
    }
    true
}

/// What `siemensRead` takes from `imaLoadFileInfo`: whether it parsed and, if so, whether the
/// series is a mosaic. `None` reproduces `imaLoadFileInfo` returning `NULL` (a failed open or an
/// unparsable name), after which `siemensRead` prints `siemensRead(): <fname>`.
struct ImaInfo {
    is_mosaic: bool,
}

/// `imaLoadFileInfo(imafile)` as far as `siemensRead` needs it: the open, the name parse (its
/// messages and `NULL`), the file count, and `IsMosaic = BaseRawMatrixSize != Columns`. FS-688:
/// the loads into stack variables abort the program when the header is shorter than 4,998 bytes;
/// larmorx reads 0 past the end and does not crash (the probe does not compare such lines).
fn ima_load_file_info(
    g: &mut Globals,
    out: &mut dyn Write,
    dir: Option<&Path>,
    imafile: &str,
) -> Option<ImaInfo> {
    let mut fp = match CFile::open(dir, imafile, "r") {
        Ok(f) => f,
        Err(_) => {
            let _ = writeln!(out, "ERROR: could not open {imafile}");
            return None;
        }
    };
    if !ima_parse_name(out, imafile) {
        return None;
    }
    // imaCountFilesInSeries runs next; it reopens the file (which succeeds) and its neighbours and
    // prints nothing, and siemensRead ignores its count, so it has no observable effect here.
    // the length of the header: imaLoadFileInfo crashes (FS-688) when a stack-targeted load runs
    // past it; the last such load ends at byte 4,998 (G28_Pre_Columns)
    let (b, ok1) = read_be(&mut fp, 2864, 4); // G19_Acq3_Mr_BaseRawMatrixSize (int)
    let base_raw = be_i32(b);
    let (b, ok2) = read_be(&mut fp, 4996, 2); // G28_Pre_Columns (short)
    let columns = i32::from(be_i16(b));
    if !(ok1 && ok2) {
        g.notices.add(
            &FS_688,
            format!(
                "imaLoadFileInfo({imafile}): the header is shorter than 4,998 bytes; FreeSurfer frees a stack address when a field read fails and aborts (signal 6): larmorx reads 0 past the end and goes on"
            ),
        );
    }
    Some(ImaInfo {
        is_mosaic: base_raw != columns,
    })
}

/// `siemensRead(fname, read_volume_flag)`: the Siemens `.ima` series `fname` belongs to.
#[allow(clippy::too_many_arguments)]
pub fn siemens_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    let _ = out.write_all(b"Starting siemensRead()\n");

    // a newer .ima is a DICOM: siemensRead unpacks it as a Siemens DICOM (not ported: 3.7-14)
    if crate::dicom_probe::is_dicom(g, dir, env, out, err, fname) != 0 {
        let _ = out.write_all(
            b"INFO: this looks like a dicom, not an old Siemens .ima file, so I'm going to unpack it as a dicom.\n",
        );
        let _ = out.write_all(b"If this fails, run mri_convert with -it dicom\n");
        let _ = writeln!(
            err,
            "larmorx-freesurfer: reading Siemens DICOM volumes is not ported yet ({fname})"
        );
        return Ok(None);
    }

    // imaLoadFileInfo: NULL (a bad name) gives siemensRead(): <fname>
    let Some(ifi) = ima_load_file_info(g, out, dir, fname) else {
        let _ = writeln!(err, "ERROR: siemensRead(): {fname}");
        return Ok(None);
    };

    // the name must end in .ima or .IMA (the last four bytes)
    let fb = fname.as_bytes();
    let ima = if fb.len() >= 4 && &fb[fb.len() - 4..] == b".ima" {
        "ima"
    } else if fb.len() >= 4 && &fb[fb.len() - 4..] == b".IMA" {
        "IMA"
    } else {
        return Ok(fail(
            g,
            err,
            ERROR_BADPARM,
            &format!("siemensRead(): bad file name {fname} (must end in '.ima' or '.IMA')"),
        ));
    };

    // c points to the first digit of the last number; the text up to '.' is the image number
    let stem = &fname[..fname.len() - 4];
    let digits = stem
        .bytes()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .count();
    let prefix = &fname[..fname.len() - 4 - digits];

    if !utils::file_exists(dir, fname) {
        return Ok(fail(
            g,
            err,
            ERROR_BADFILE,
            &format!("siemensRead(): file {fname} doesn't exist"),
        ));
    }

    let file_n = cstd::atoi(stem[stem.len() - digits..].as_bytes());
    let name_of = |n: i32| format!("{prefix}{n}.{ima}");

    // the lowest and highest image numbers whose files exist around this one
    let mut n_low = file_n - 1;
    while utils::file_exists(dir, &name_of(n_low)) {
        n_low -= 1;
    }
    n_low += 1;
    let mut n_high = file_n + 1;
    while utils::file_exists(dir, &name_of(n_high)) {
        n_high += 1;
    }
    n_high -= 1;
    let n_files = n_high - n_low + 1;
    // each counting loop ends on `fopen` of a file that does not exist, which leaves errno at
    // ENOENT (2); FreeSurfer prints it at the end (nothing later clears it on a success)
    g.errno = 2;

    let first = name_of(n_low);
    let mut fp = match CFile::open(dir, &first, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            return Ok(fail(
                g,
                err,
                ERROR_BADFILE,
                &format!(
                    "siemensRead(): can't open file {first} (low = {n_low}, this = {file_n}, high = {n_high})"
                ),
            ));
        }
    };

    let (b, _) = read_be(&mut fp, 4994, 2);
    let rows = i32::from(be_i16(b));
    let _ = writeln!(out, "rows = {rows}");
    let (b, _) = read_be(&mut fp, 4996, 2);
    let cols = i32::from(be_i16(b));
    let _ = writeln!(out, "cols = {cols}");
    let (b, _) = read_be(&mut fp, 4004, 4);
    let mut n_slices = be_i32(b);
    if n_slices == 0 {
        return Ok(fail(
            g,
            err,
            ERROR_BADFILE,
            "\n\nPlease try with the option '-it siemens_dicom'.\n The handling failed assuming the old siemens format.\n",
        ));
    }
    let (b, _) = read_be(&mut fp, 2864, 4);
    let base_raw_matrix_size = be_i32(b);
    let (b, _) = read_be(&mut fp, 1584, 4);
    let _number_of_averages = be_i32(b);
    let (seq, _) = read_bytes_at(&mut fp, 3009, 65);

    // scout: the sequence file name holds "scout"
    let ps2 = utils::str_lower(&String::from_utf8_lossy(&c_text(&seq)));
    if ps2.contains("scout") {
        let name = String::from_utf8_lossy(&c_text(&seq)).into_owned();
        return Ok(fail(
            g,
            err,
            ERROR_BADPARM,
            &format!("siemensRead(): series appears to be a scout (sequence file name is {name})"),
        ));
    }

    let (mos_r, mos_c, mosaic_size, n_t);
    if n_slices == 1 && !ifi.is_mosaic {
        n_slices = n_files;
        n_t = 1;
        if base_raw_matrix_size != rows || base_raw_matrix_size != cols {
            return Ok(fail(
                g,
                err,
                ERROR_BADPARM,
                "siemensRead(): bad file/base matrix sizes",
            ));
        }
        mos_r = 1;
        mos_c = 1;
        mosaic_size = 1;
    } else {
        if base_raw_matrix_size == 0 || rows % base_raw_matrix_size != 0 {
            // C's % by 0 would trap; FreeSurfer's base matrix is non-zero on this path
            return Ok(fail(
                g,
                err,
                ERROR_BADPARM,
                &format!("siemensRead(): file rows ({rows}) not divisible by image rows ({base_raw_matrix_size})"),
            ));
        }
        if cols % base_raw_matrix_size != 0 {
            return Ok(fail(
                g,
                err,
                ERROR_BADPARM,
                &format!("siemensRead(): file cols ({cols}) not divisible by image cols ({base_raw_matrix_size})"),
            ));
        }
        mos_r = rows / base_raw_matrix_size;
        mos_c = cols / base_raw_matrix_size;
        mosaic_size = mos_r * mos_c;
        if mosaic_size == 0 {
            return Ok(fail(
                g,
                err,
                ERROR_BADPARM,
                "siemensRead(): mosaic_size==0, Try '-it siemens_dicom' flag with mri_convert",
            ));
        }
        let n_dangling = n_slices.rem_euclid(mosaic_size);
        let n_full = (n_slices - n_dangling) / mosaic_size;
        let mosaics_per_volume = n_full + i32::from(n_dangling != 0);
        if mosaics_per_volume == 0 || n_files % mosaics_per_volume != 0 {
            return Ok(fail(
                g,
                err,
                ERROR_BADPARM,
                &format!("siemensRead(): files in volume ({n_files}) not divisible by mosaics per volume ({mosaics_per_volume})"),
            ));
        }
        n_t = n_files / mosaics_per_volume;
    }

    let mut m = if read_volume {
        mri::mri_alloc_sequence(
            g,
            out,
            err,
            base_raw_matrix_size,
            base_raw_matrix_size,
            n_slices,
            MRI_SHORT,
            n_t,
        )?
    } else {
        let mut m = mri::mri_alloc_header(
            g,
            out,
            err,
            base_raw_matrix_size,
            base_raw_matrix_size,
            n_slices,
            MRI_SHORT,
            n_t,
        )?;
        m.nframes = n_t;
        m
    };

    let depth = m.vg.depth;

    // pixel sizes (mos_r, mos_c factors as in FreeSurfer)
    let (b, ok) = read_be(&mut fp, 5000, 8);
    note_short(g, ok);
    m.vg.xsize = (f64::from(mos_r) * be_f64(b)) as f32;
    let (b, ok) = read_be(&mut fp, 5008, 8);
    note_short(g, ok);
    m.vg.ysize = (f64::from(mos_c) * be_f64(b)) as f32;

    // slice distance factor
    let (b, _) = read_be(&mut fp, 4136, 8);
    let mut d = be_f64(b);
    if d == -19222.0 {
        d = 0.0;
    }
    let (b, _) = read_be(&mut fp, 1544, 8);
    let d2 = be_f64(b);
    m.vg.zsize = ((1.0 + d) * d2) as f32;

    // field of view: FreeSurfer reads the width into the same variable and then byte-swaps the
    // height's value a second time, so the width is ignored (FS-691)
    let (b, _) = read_be(&mut fp, 3744, 8);
    let d = be_f64(b);
    let (_b, _) = read_be(&mut fp, 3752, 8);
    let d2 = machine::swap_double(d);
    m.fov = (if d > d2 { d } else { d2 }) as f32;
    if d2 > d {
        g.notices.add(
            &FS_691,
            "siemensRead: the field of view is the byte-swap artefact of the height, not the width",
        );
    }

    m.thick = m.vg.zsize;
    m.ps = m.vg.xsize;
    m.location = 0.0;

    let (b, _) = read_be(&mut fp, 2112, 8);
    m.flip_angle = radians(be_f64(b));

    let (b, _) = read_be(&mut fp, 1560, 8);
    m.tr = be_f64(b) as f32;
    let (b, _) = read_be(&mut fp, 1568, 8);
    m.te = be_f64(b) as f32;
    let (b, _) = read_be(&mut fp, 1576, 8);
    m.ti = be_f64(b) as f32;

    // direction cosines (three consecutive doubles each)
    let read3 = |fp: &mut CFile, at: i64| -> [f64; 3] {
        let (b0, _) = read_be(fp, at, 8);
        let (b1, _) = read_be(fp, at + 8, 8);
        let (b2, _) = read_be(fp, at + 16, 8);
        [be_f64(b0), be_f64(b1), be_f64(b2)]
    };
    let z = read3(&mut fp, 3792);
    m.vg.z_r = (-z[0]) as f32;
    m.vg.z_a = z[1] as f32;
    m.vg.z_s = (-z[2]) as f32;
    let x = read3(&mut fp, 3832);
    m.vg.x_r = (-x[0]) as f32;
    m.vg.x_a = x[1] as f32;
    m.vg.x_s = (-x[2]) as f32;
    let y = read3(&mut fp, 3856);
    m.vg.y_r = (-y[0]) as f32;
    m.vg.y_a = y[1] as f32;
    m.vg.y_s = (-y[2]) as f32;
    let c = read3(&mut fp, 3768);
    let im_c_r = -c[0];
    let im_c_a = c[1];
    let im_c_s = -c[2];

    // c = im_c - (mosaic_size-1)*z*zsize + (depth/2)*z*zsize  (float z and zsize promote to double)
    let zr = f64::from(m.vg.z_r);
    let za = f64::from(m.vg.z_a);
    let zs = f64::from(m.vg.z_s);
    let zsz = f64::from(m.vg.zsize);
    let msm1 = f64::from(mosaic_size - 1);
    let half = f64::from(depth) / 2.0;
    m.vg.c_r = (im_c_r - msm1 * zr * zsz + half * zr * zsz) as f32;
    m.vg.c_a = (im_c_a - msm1 * za * zsz + half * za * zsz) as f32;
    m.vg.c_s = (im_c_s - msm1 * zs * zsz + half * zs * zsz) as f32;
    m.vg.ras_good_flag = 1;

    let (b, _) = read_be(&mut fp, 3760, 4);
    let slice_dir = be_i32(b);
    if !(1..=6).contains(&slice_dir) {
        return Ok(fail(
            g,
            err,
            ERROR_BADFILE,
            &format!("siemensRead(): bad slice direction ({slice_dir}) in file {first}"),
        ));
    }

    m.xstart = (-m.vg.width as f32 * m.vg.xsize) / 2.0;
    m.xend = -m.xstart;
    m.ystart = (-m.vg.height as f32 * m.vg.ysize) / 2.0;
    m.yend = -m.ystart;
    m.zstart = (-m.vg.depth as f32 * m.vg.zsize) / 2.0;
    m.zend = -m.zstart;

    fp.close();

    m.imnr0 = 1;
    m.imnr1 = m.vg.depth;

    if read_volume
        && !read_voxels(
            g, err, dir, prefix, ima, n_low, n_high, rows, cols, mos_r, mosaic_size, &mut m,
        )?
    {
        return Ok(None);
    }

    Ok(Some(m))
}

/// The pixels of every file read at offset 6,144 and laid out of their mosaics (`MRIalloc` of a
/// raw volume, then `MRISvox`/`MRISseq_vox`). `true` on success; `false` (the caller returns
/// `None`) when a file cannot be opened, after `siemensRead(): can't open file ...`.
#[allow(clippy::too_many_arguments)]
fn read_voxels(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    prefix: &str,
    ima: &str,
    n_low: i32,
    n_high: i32,
    rows: i32,
    cols: i32,
    mos_r: i32,
    mosaic_size: i32,
    m: &mut Mri,
) -> Result<bool, Fatal> {
    let rows_u = rows.max(0) as usize;
    let cols_u = cols.max(0) as usize;
    let n_files = (n_high - n_low + 1).max(0) as usize;
    // FS-692: `MRIalloc(rows, cols, n_files)` gives the raw volume `rows` columns and `cols` rows,
    // but the read fills `rows` rows of `cols` columns, so when the file is not square (more rows
    // than columns) it writes past the voxels (memory corruption, a crash): larmorx does not
    // crash; the probe does not compare such a series.
    if rows > cols {
        g.notices.add(
            &FS_692,
            format!(
                "siemensRead: a {rows} x {cols} file is read into a raw volume of {cols} rows: FreeSurfer writes {rows} rows and runs past the voxels (memory corruption or a crash): no out-of-bounds write here"
            ),
        );
    }
    // MRIalloc(rows, cols, n_files, MRI_SHORT): voxel (x,y,z) at (z*rows*cols + y*rows + x)
    let raw_slice = rows_u * cols_u;
    let mut raw = vec![0i16; raw_slice * n_files];
    for file_n in n_low..=n_high {
        let name = format!("{prefix}{file_n}.{ima}");
        let mut fp = match CFile::open(dir, &name, "r") {
            Ok(f) => f,
            Err(_) => {
                // MRIfree then errno = 0 then ErrorReturn
                g.errno = 0;
                log::error_printf(
                    g,
                    err,
                    ERROR_BADFILE,
                    &format!(
                        "siemensRead(): can't open file {name} (low = {n_low}, this = {file_n}, high = {n_high})"
                    ),
                );
                return Ok(false);
            }
        };
        let mut errno = 0;
        fp.fseek(&mut errno, 6144, 0);
        let z = (file_n - n_low) as usize;
        for i in 0..rows_u {
            // &MRISvox(mri_raw, 0, i, z): base x=0, y=i, z
            let base = z * raw_slice + i * rows_u;
            let mut row = vec![0u8; cols_u * 2];
            let got = fp.read_bytes(&mut row);
            if got != cols_u * 2 {
                // one message per row the file cannot fill (errno still ENOENT from the count)
                log::error_printf(g, err, ERROR_BADFILE, "siemensRead(): could not read file");
            }
            // the file holds big-endian shorts (`swab` turns them into the native order)
            for (k, v) in row.chunks_exact(2).enumerate() {
                let idx = base + k;
                if idx < raw.len() && 2 * (k + 1) <= got {
                    raw[idx] = i16::from_be_bytes([v[0], v[1]]);
                }
            }
        }
        fp.close();
    }

    // de-mosaic into m (base x base x depth x nframes)
    let width = m.vg.width.max(0) as usize;
    let height = m.vg.height.max(0) as usize;
    let depth = m.vg.depth.max(0) as usize;
    let nframes = m.nframes.max(0) as usize;
    let vox_per_row = width as u64;
    let vox_per_slice = vox_per_row * height as u64;
    let vox_per_vol = vox_per_slice * depth as u64;
    let msz = mosaic_size.max(1) as usize;
    let mos_r_u = mos_r.max(1) as usize;
    for t in 0..nframes {
        for s in 0..depth {
            let slice_in_mosaic = s % msz;
            let mosaic = (s - slice_in_mosaic) / msz;
            let file = nframes * mosaic + t;
            let bc = slice_in_mosaic % mos_r_u;
            let br = (slice_in_mosaic - bc) / mos_r_u;
            for i in 0..width {
                for j in 0..height {
                    let src_x = width * bc + i;
                    let src_y = height * br + j;
                    let src = file * raw_slice + src_y * rows_u + src_x;
                    let val = raw.get(src).copied().unwrap_or(0);
                    let off = (t as u64 * vox_per_vol
                        + s as u64 * vox_per_slice
                        + j as u64 * vox_per_row
                        + i as u64) as usize
                        * 2;
                    if let Some(c) = m.chunk.as_mut() {
                        if off + 2 <= c.len() {
                            c[off..off + 2].copy_from_slice(&val.to_le_bytes());
                        }
                    }
                }
            }
        }
    }
    Ok(true)
}

/// A header's bytes at `at` (not byte-swapped: a C string or raw block); `false` on a short read.
fn read_bytes_at(fp: &mut CFile, at: i64, n: usize) -> (Vec<u8>, bool) {
    let mut errno = 0;
    fp.fseek(&mut errno, at, 0);
    let mut b = vec![0u8; n];
    let got = fp.read_bytes(&mut b);
    (b, got == n)
}

/// The bytes of a C string: up to the first NUL.
fn c_text(b: &[u8]) -> Vec<u8> {
    b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec()
}

/// FS-689: a header field read past the end of the file is memory contents (0 by default).
fn note_short(g: &mut Globals, ok: bool) {
    if !ok {
        g.notices.add(
            &FS_689,
            "siemensRead: a geometry field was read past the end of the header: 0 here (FreeSurfer's is the stack's earlier value)",
        );
    }
}

/// `ErrorReturn(NULL, ...)`: `errno` 0, the message, `None`.
fn fail(g: &mut Globals, err: &mut dyn Write, code: i32, message: &str) -> Option<Mri> {
    g.errno = 0;
    log::error_printf(g, err, code, message);
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn put(b: &mut Vec<u8>, at: usize, v: &[u8]) {
        if b.len() < at + v.len() {
            b.resize(at + v.len(), 0);
        }
        b[at..at + v.len()].copy_from_slice(v);
    }

    fn p64(b: &mut Vec<u8>, at: usize, v: f64) {
        put(b, at, &v.to_be_bytes());
    }
    fn p32(b: &mut Vec<u8>, at: usize, v: i32) {
        put(b, at, &v.to_be_bytes());
    }
    fn p16(b: &mut Vec<u8>, at: usize, v: i16) {
        put(b, at, &v.to_be_bytes());
    }

    /// A complete structural `.ima` file of `n` x `n` voxels `base + k`, one slice, an axial
    /// sequence 1.5 mm thick with 1 x 1.25 mm pixels around (1, -2, -3) in file coordinates.
    fn ima(n: i16, base: i16) -> Vec<u8> {
        let mut b = vec![0u8; 6144];
        p64(&mut b, 1544, 1.5); // slice thickness
        p64(&mut b, 1560, 2000.0); // tr
        p64(&mut b, 1568, 30.0); // te
        p64(&mut b, 1576, 900.0); // ti
        p32(&mut b, 1584, 1); // averages
        p64(&mut b, 2112, 90.0); // flip angle (degrees)
        p32(&mut b, 2864, n as i32); // base raw matrix size
        put(&mut b, 3009, b"fl3d"); // sequence name
        p64(&mut b, 3744, 256.0); // fov height
        p64(&mut b, 3752, 240.0); // fov width (ignored)
        p32(&mut b, 3760, 1); // slice direction
        p64(&mut b, 3768, 1.0); // image centre sag
        p64(&mut b, 3776, 2.0); // cor
        p64(&mut b, 3784, 3.0); // tra
        p64(&mut b, 3792, 0.0); // normal sag
        p64(&mut b, 3800, 0.0); // normal cor
        p64(&mut b, 3808, 1.0); // normal tra
        p64(&mut b, 3832, 1.0); // column sag
        p64(&mut b, 3856, 0.0); // row sag
        p64(&mut b, 3864, 1.0); // row cor
        p32(&mut b, 4004, 1); // nominal slices
        p64(&mut b, 4136, 0.0); // distance factor
        p16(&mut b, 4994, n); // rows
        p16(&mut b, 4996, n); // cols
        p64(&mut b, 5000, 1.0); // pixel size row
        p64(&mut b, 5008, 1.25); // pixel size col
        for k in 0..(n as i32 * n as i32) {
            b.extend_from_slice(&(base + k as i16).to_be_bytes());
        }
        b
    }

    fn shorts(m: &Mri) -> Vec<i16> {
        m.chunk
            .as_ref()
            .unwrap()
            .chunks(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect()
    }

    /// A three-file structural series reads its geometry and voxels.
    #[test]
    fn a_structural_series_reads() {
        let dir = tempfile::tempdir().unwrap();
        for n in 3..=5 {
            std::fs::write(
                dir.path().join(format!("1-2-{n}.ima")),
                ima(2, 10 * n as i16),
            )
            .unwrap();
        }
        let p = Some(dir.path());
        let env = Environment::default();
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let m = siemens_read(&mut g, &mut o, &mut e, &env, p, "1-2-4.ima", true)
            .unwrap()
            .unwrap();
        // one slice per file: width, height = base matrix (2); depth = file count (3)
        assert_eq!(
            (m.vg.width, m.vg.height, m.vg.depth, m.nframes),
            (2, 2, 3, 1)
        );
        assert_eq!(m.mtype, MRI_SHORT);
        assert_eq!(
            shorts(&m),
            vec![30, 31, 32, 33, 40, 41, 42, 43, 50, 51, 52, 53]
        );
        assert_eq!((m.vg.xsize, m.vg.ysize, m.vg.zsize), (1.0, 1.25, 1.5));
        assert_eq!((m.tr, m.te, m.ti), (2000.0, 30.0, 900.0));
        assert!((m.flip_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-15);
        assert_eq!(m.fov, 256.0); // the height, not the width (FS-691)
        assert_eq!(m.vg.ras_good_flag, 1);
        // the direction cosines are read and negated as siemensRead does
        assert_eq!((m.vg.z_r, m.vg.z_a, m.vg.z_s), (0.0, 0.0, -1.0));
        assert_eq!((m.vg.x_r, m.vg.x_a, m.vg.x_s), (-1.0, 0.0, 0.0));
        assert_eq!((m.vg.y_r, m.vg.y_a, m.vg.y_s), (0.0, 1.0, 0.0));
    }

    /// A name that does not parse gives imaLoadFileInfo's `NULL` and `siemensRead(): <fname>`.
    #[test]
    fn an_unparsable_name_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x.ima"), ima(2, 0)).unwrap();
        let env = Environment::default();
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = siemens_read(&mut g, &mut o, &mut e, &env, Some(dir.path()), "x.ima", false).unwrap();
        assert!(r.is_none());
        assert!(String::from_utf8(o).unwrap().contains("could not parse x.ima"));
        assert!(String::from_utf8(e).unwrap().contains("siemensRead(): x.ima"));
    }

    /// A file name that does not end in `.ima` or `.IMA` is refused.
    #[test]
    fn a_bad_extension_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("1-2-4.Ima"), ima(2, 0)).unwrap();
        let env = Environment::default();
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r =
            siemens_read(&mut g, &mut o, &mut e, &env, Some(dir.path()), "1-2-4.Ima", false).unwrap();
        assert!(r.is_none());
        assert!(
            String::from_utf8(e)
                .unwrap()
                .contains("must end in '.ima' or '.IMA'")
        );
    }

    /// FS-688: a header too short for imaLoadFileInfo aborts FreeSurfer; larmorx does not crash and
    /// notes it.
    #[test]
    fn a_short_header_is_noted_not_crashed() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = ima(2, 0);
        f.truncate(3000);
        std::fs::write(dir.path().join("1-2-4.ima"), f).unwrap();
        let env = Environment::default();
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let _ = siemens_read(&mut g, &mut o, &mut e, &env, Some(dir.path()), "1-2-4.ima", false);
        assert!(g.notices.as_slice().iter().any(|n| n.defect.id == "FS-688"));
    }

    /// imaParseName tries `-` first and only falls back to `_` when there is no `-` at all.
    #[test]
    fn name_parsing_commits_to_the_first_separator() {
        let mut out = Vec::new();
        assert!(ima_parse_name(&mut out, "w/1-2-3.ima"));
        assert!(ima_parse_name(&mut out, "w/1_2_3.ima"));
        // a single `-` has no second field: it fails and does not retry `_`
        assert!(!ima_parse_name(&mut out, "w/1-2_3.ima"));
        assert!(!ima_parse_name(&mut out, "w/x.ima"));
    }
}
