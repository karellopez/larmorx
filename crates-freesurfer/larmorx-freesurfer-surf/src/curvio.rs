// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisurf_io.cpp`'s per-vertex value files (`fs.surfio`, queue item 3.1-48 part a): the
//! curvature as text (`mrisReadAsciiCurvatureFile` 4779, `mrisWriteAsciiCurvatureFile` 4820), the
//! old binary curvature (`__mrisreadcurvoldformat` 4945), the curvature vectors and arrays
//! (`MRISreadCurvatureVector` 5015, `MRISreadCurvatureIntoArray` 5057,
//! `MRISreadNewCurvatureVector` 5867, `MRISreadNewCurvatureIntoArray` 5908), the float and area
//! files (`MRISreadFloatFile` 5113, `MRISreadBinaryAreas` 5184), the names of files to read
//! (`MRISbuildFileName_read` 5660), the decimation files (`MRISwriteDecimation` 5711,
//! `MRISreadDecimation` 5731) and the GIFTI alternatives of `FS_GII` (`__MRISapplyFSGIIread`
//! 6226, `__MRISapplyFSGIIwrite` 6287).
//!
//! Files are C streams ([`CFile`]) opened in `dir` (the program's working directory, for a
//! relative name); a failed `fopen` sets `errno`, which the error that follows prints first.
//!
//! The defects: FS-126 (names of 1,024 bytes or more), FS-134 (a surface name of 1,024 bytes or
//! more in `FileNamePath`), FS-136 (the text curvature reader's message), FS-137 (a text
//! curvature file of too few lines), FS-138 (the new curvature reader's message: a crash), FS-139
//! and FS-140 (decimation files that end early, or whose count cannot be read), FS-148 (the bytes
//! before a name), FS-149 (names written past the caller's buffer); fs.io's FS-49 and FS-167 (a
//! number the file ends before).
//!
//! Not here (they wait for other items): `MRISreadBinaryCurvature`, `MRISreadCurvatureFile`,
//! `__mrisreadcurvmri`, `MRISreadNewCurvatureFile`, `MRISreadMarked` (the format identification
//! `mri_identify`, fs.io 3.1-27, and `MRIread`, 3.1-28).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_01, FS_126, FS_136, FS_137, FS_138, FS_139, FS_140, FS_148, FS_149};
use larmorx_freesurfer_core::cscan::Bytes;
use larmorx_freesurfer_core::cstd::{fmt_f, is_space};
use larmorx_freesurfer_core::diag::{self, DIAG_SHOW};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOFILE, ERROR_NOMEMORY, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::utils::file_name_path;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::fio::{
    fio_file_exists_readable, fread_float, fread_int, fread2, fread3, fwrite_int,
};
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;

use crate::aaa::NEW_VERSION_MAGIC_NUMBER;
use crate::coords::Io;
use crate::mris::Mris;
use crate::scanf::Scanf;
use crate::sfile::{
    LEFT_HEMISPHERE, STRLEN, c_str, file_name_path_buf, nonneg, open, open_failed, snprintf_name,
    strcpy_checked,
};

/// `RIGHT_HEMISPHERE`, `BOTH_HEMISPHERES` (`mrisurf.h`).
const RIGHT_HEMISPHERE: i32 = 1;
const BOTH_HEMISPHERES: i32 = 3;

/// `GIFTI_FILE` (`mri.h`): the type `FS_GII` sets.
pub const GIFTI_FILE: i32 = 27;

/// `ENOMEM` (Linux): what a failed `calloc` leaves in `errno`.
const ENOMEM: i32 = 12;

/// The first bytes of the official build's code of `mrisReadAsciiCurvatureFile` before a NUL,
/// what its open-failure message prints for its own name (FS-136; observed in the unit oracle's
/// `mrisurf_io.cpp.o` and the probe).
const ASCII_CURV_CODE: [u8; 25] = [
    0xf3, 0x0f, 0x1e, 0xfa, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x55, 0x53, 0x48,
    0x89, 0xfb, 0x48, 0x89, 0xf7, 0x48, 0x81, 0xec, 0x38, 0x04,
];

/// `ErrorPrintf(code, ...)` of a message that holds bytes that are not text (a file's own
/// characters): as fs.core's `error_printf`.
pub(crate) fn error_printf_bytes(g: &mut Globals, err: &mut dyn Write, code: i32, message: &[u8]) {
    g.gerror = code;
    if g.errno != 0 {
        log::error(g, err, &log::strerror(g.errno));
    }
    let mut m = message.to_vec();
    if g.notices.reproduce() {
        m.push(0);
    }
    g.notices.add(
        &FS_01,
        "an error message without FreeSurfer's NUL byte before its newline",
    );
    let _ = err.write_all(b"error: ");
    let _ = err.write_all(&m);
    let _ = err.write_all(b"\n");
    log::write_to_error_log(g, &m);
}

/// `fgetl(line, n, fp)`: the next line (`fgets` of at most `n - 1` bytes) that is not empty,
/// white space or a comment after its leading white space; every `#` ends it, its newline
/// removed. The line as the buffer holds it (its leading white space too) and where `fgetl`'s
/// result starts in it; `None` at the end of the file.
pub(crate) fn fgetl_line(fp: &mut CFile, n: usize) -> Option<(Vec<u8>, usize)> {
    loop {
        let raw = fp.fgets(n)?;
        let s = &raw[..raw.iter().position(|&b| b == 0).unwrap_or(raw.len())];
        let start = s.iter().position(|&b| !is_space(b)).unwrap_or(s.len());
        if start == s.len() || s[start] == b'#' {
            continue;
        }
        let end = s[start..]
            .iter()
            .position(|&b| b == b'#')
            .map_or(s.len(), |i| start + i);
        let mut line = s[..end].to_vec();
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        return Some((line, start));
    }
}

fn verbose_show(g: &Globals) -> bool {
    g.gdiag & DIAG_SHOW != 0 && diag::verbose_on(g)
}

/// `MRIsetVoxVal(curvmri, vno, 0, 0, 0, v)` where a volume of the values was given.
fn set_value(
    g: &mut Globals,
    io: &mut Io,
    curvmri: &mut Option<&mut Mri>,
    vno: usize,
    v: f32,
) -> Result<(), Fatal> {
    if let Some(m) = curvmri.as_deref_mut() {
        mri_set_vox_val(g, io.err, m, vno as i32, 0, 0, 0, v, 0)?;
    }
    Ok(())
}

// ---- the curvature as text --------------------------------------------------------------------

/// `mrisReadAsciiCurvatureFile(mris, fname, curvmri)`: one line (`fgetl`, 99 bytes) per unripped
/// vertex, its fifth number its curvature (`%*d %*f %*f %*f %f`), into the vertex and, given, the
/// volume's voxel `vno`. Returns `NO_ERROR`, or `ERROR_BADFILE`: a file that cannot be opened
/// (`mrisReadAsciiCurvatureFile could not open file <file>.` with its newline; FS-136), a line
/// without a fifth number (`mrisReadAsciiCurvatureFile(<file>): could not scan curvature from line
/// '<line>'`, the line with its leading white space), a file of fewer lines than unripped
/// vertices (FS-137, rule B: `mrisReadAsciiCurvatureFile(<file>): the file ends before vertex
/// <vno>`; FreeSurfer stops there and returns `NO_ERROR`, as `reproduce` does).
pub fn read_ascii_curvature_file(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
    mut curvmri: Option<&mut Mri>,
) -> Result<i32, Fatal> {
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            g.notices.add(
                &FS_136,
                "mrisReadAsciiCurvatureFile's open-failure message names the function (FreeSurfer prints 25 bytes of binary data)",
            );
            let mut m = Vec::new();
            if g.notices.reproduce() {
                m.extend_from_slice(&ASCII_CURV_CODE);
            } else {
                m.extend_from_slice(b"mrisReadAsciiCurvatureFile");
            }
            m.extend_from_slice(format!(" could not open file {fname}.\n").as_bytes());
            error_printf_bytes(g, io.err, ERROR_BADFILE, &m);
            return Ok(ERROR_BADFILE);
        }
    };
    for vno in 0..nonneg(mris.nvertices) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let Some((line, _)) = fgetl_line(&mut fp, 100) else {
            g.notices.add(
                &FS_137,
                format!("{fname}: a text curvature file of too few lines is refused (FreeSurfer leaves the rest as they were)"),
            );
            if g.notices.reproduce() {
                break;
            }
            log::error_printf(
                g,
                io.err,
                ERROR_BADFILE,
                &format!("mrisReadAsciiCurvatureFile({fname}): the file ends before vertex {vno}"),
            );
            return Ok(ERROR_BADFILE);
        };
        let mut b = Bytes::new(&line);
        let mut sc = Scanf::new(&mut b);
        let e = &mut g.errno;
        sc.d(e, false);
        for _ in 0..3 {
            sc.text(e, b" ");
            sc.f(e, false);
        }
        sc.text(e, b" ");
        let curv = sc.f(e, true);
        sc.text(e, b"\n");
        if sc.finish(e) != 1 {
            let mut m = format!("mrisReadAsciiCurvatureFile({fname}): could not scan curvature from line '")
                .into_bytes();
            m.extend_from_slice(&line);
            m.push(b'\'');
            error_printf_bytes(g, io.err, ERROR_BADFILE, &m);
            return Ok(ERROR_BADFILE);
        }
        let c = curv.expect("one item assigned");
        mris.vertex_mut(vno).curv = c;
        set_value(g, io, &mut curvmri, vno, c)?;
    }
    fp.close();
    Ok(NO_ERROR)
}

/// `mrisWriteAsciiCurvatureFile(mris, fname)`: per unripped vertex `%3.3d %2.5f %2.5f %2.5f
/// %2.5f` of its number, coordinates and curvature. A file that cannot be created ends the
/// program (`mrisWriteAsciiCurvatureFile() could not open output file <file>.` with its newline).
/// Returns `NO_ERROR`.
pub fn write_ascii_curvature_file(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> Result<i32, Fatal> {
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_exit(
                g,
                io.err,
                ERROR_BADFILE,
                &format!("mrisWriteAsciiCurvatureFile() could not open output file {fname}.\n"),
            )?;
            unreachable!("ErrorExit returns only through its Err");
        }
    };
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let f = |x: f32| fmt_f(f64::from(x), 2, 5);
        let s = format!("{vno:03} {} {} {} {}\n", f(v.x), f(v.y), f(v.z), f(v.curv));
        fp.put_bytes(s.as_bytes());
    }
    fp.close();
    Ok(NO_ERROR)
}

// ---- the old binary curvature -----------------------------------------------------------------

/// `__mrisreadcurvoldformat(mris, fname, curvmri)`: the vertex count (3 bytes), the face count (3
/// bytes), then per vertex a 2-byte number in hundredths (`i / 100.0`), into the vertices'
/// curvature (and the volume, given); the surface's `max_curv` and `min_curv` from them (10,000
/// and -10,000 for none). Returns `NO_ERROR`, or `ERROR_NOFILE` (`__mrisreadoldcurvaturefile():
/// could not open <file>`, `... incompatible vertex number in file <file>`). With `DIAG_SHOW` and
/// verbose: `reading curvature file (old format) ...` and `done. min=%2.3f max=%2.3f` (with its
/// newline). A value the file does not hold: fs.io's `fread2` (FS-167).
pub fn read_curv_old_format(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
    mut curvmri: Option<&mut Mri>,
) -> Result<i32, Fatal> {
    let fname = c_str(fname);
    let verbose = verbose_show(g);
    if verbose {
        let _ = write!(io.out, "reading curvature file (old format) ...");
    }
    let mut fp = match open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("__mrisreadoldcurvaturefile(): could not open {fname}");
            return Ok(open_failed(g, io, e, ERROR_NOFILE, &m));
        }
    };
    let (_, vnum) = fread3(&mut fp);
    let (_, _fnum) = fread3(&mut fp);
    if vnum != mris.nvertices {
        fp.close();
        let m = format!("__mrisreadoldcurvaturefile(): incompatible vertex number in file {fname}");
        log::error_printf(g, io.err, ERROR_NOFILE, &m);
        return Ok(ERROR_NOFILE);
    }
    let (mut curvmin, mut curvmax) = (10000.0f32, -10000.0f32);
    for k in 0..nonneg(vnum) {
        let (_, i) = fread2(g, &mut fp);
        let curv = (f64::from(i) / 100.0) as f32;
        if k == 0 {
            curvmin = curv;
            curvmax = curv;
        }
        if curv > curvmax {
            curvmax = curv;
        }
        if curv < curvmin {
            curvmin = curv;
        }
        mris.vertex_mut(k).curv = curv;
        set_value(g, io, &mut curvmri, k, curv)?;
    }
    mris.max_curv = curvmax;
    mris.min_curv = curvmin;
    if verbose {
        let _ = writeln!(
            io.out,
            "done. min={} max={}",
            fmt_f(f64::from(curvmin), 2, 3),
            fmt_f(f64::from(curvmax), 2, 3)
        );
    }
    fp.close();
    Ok(NO_ERROR)
}

// ---- the curvature vectors and arrays ---------------------------------------------------------

/// The file a value reader opens for `sname` (`MRISreadCurvatureVector`, `MRISreadFloatFile`,
/// `MRISreadNewCurvatureVector`): a name with a `/` as it is (copied into a 1,024-byte buffer:
/// FS-126), else in the folder of the surface's name, with the hemisphere's `lh.` or `rh.` before
/// a name without a dot (cut at 1,023 bytes with `<func>: Truncation on line <n>`: FS-126).
fn value_file_name(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    sname: &str,
    func: &str,
    lines: (u32, u32),
) -> Result<String, Fatal> {
    if sname.contains('/') {
        return strcpy_checked(g, io, sname, func);
    }
    let path = file_name_path(c_str(&mris.fname));
    Ok(if sname.contains('.') {
        snprintf_name(g, io, format!("{path}/{sname}"), func, lines.0)
    } else {
        let hemi = if mris.hemisphere == LEFT_HEMISPHERE {
            "lh"
        } else {
            "rh"
        };
        snprintf_name(g, io, format!("{path}/{hemi}.{sname}"), func, lines.1)
    })
}

/// `MRISreadCurvatureVector(mris, sname)`: [`read_curvature_into_array`] of the file
/// [`value_file_name`] names, `nvertices` values; `None` on an error.
pub fn read_curvature_vector(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    sname: &str,
) -> Result<Option<Vec<f32>>, Fatal> {
    let sname = c_str(sname);
    let fname = value_file_name(g, io, mris, sname, "MRISreadCurvatureVector", (5030, 5038))?;
    let (code, cvec) = read_curvature_into_array(g, io, dir, &fname, mris.nvertices)?;
    Ok(if code == NO_ERROR { cvec } else { None })
}

/// `MRISreadCurvatureIntoArray(sname, in_array_size, &out)`: an old curvature file (the vertex
/// count in 3 bytes, the face count, then 2-byte hundredths) of exactly `in_array_size` vertices;
/// a file that starts with the new format's mark is read by [`read_new_curvature_into_array`].
/// Returns `(NO_ERROR, Some(values))`, or `ERROR_BADFILE` without a message for another vertex
/// count, `ERROR_BADFILE` with `MRISreadCurvatureVectorIntoArray(<file>): fopen failed` for a file
/// that cannot be opened. With `DIAG_SHOW` and verbose: `reading curvature file...`.
pub fn read_curvature_into_array(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    sname: &str,
    in_array_size: i32,
) -> Result<(i32, Option<Vec<f32>>), Fatal> {
    let sname = c_str(sname);
    if verbose_show(g) {
        let _ = write!(io.out, "reading curvature file...");
    }
    let mut fp = match open(dir, sname, "r") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("MRISreadCurvatureVectorIntoArray({sname}): fopen failed");
            return Ok((open_failed(g, io, e, ERROR_BADFILE, &m), None));
        }
    };
    let (_, vnum) = fread3(&mut fp);
    if vnum == NEW_VERSION_MAGIC_NUMBER {
        fp.close();
        return read_new_curvature_into_array(g, io, dir, sname, in_array_size);
    }
    if vnum != in_array_size {
        fp.close();
        return Ok((ERROR_BADFILE, None));
    }
    let (_, _fnum) = fread3(&mut fp);
    let mut cvec = vec![0.0f32; nonneg(in_array_size)];
    for k in 0..nonneg(vnum) {
        let (_, i) = fread2(g, &mut fp);
        cvec[k] = (f64::from(i) / 100.0) as f32;
    }
    fp.close();
    Ok((NO_ERROR, Some(cvec)))
}

/// `MRISreadNewCurvatureIntoArray(sname, in_array_size, &out)`: a new curvature file (the mark
/// 16,777,215 in 3 bytes, then the vertex count, the face count and the values per vertex as
/// `int`s, then the values as `float`s) of exactly `in_array_size` vertices and one value each; a
/// file without the mark is read by [`read_curvature_into_array`]. Returns `(NO_ERROR,
/// Some(values))` or `ERROR_BADFILE`: a file that cannot be opened
/// (`MRISreadNewCurvatureIntoArray(<file>): fopen failed`; FS-138: FreeSurfer's message reads a
/// name it is never given, a crash with signal 11 in the official build after the `errno` line,
/// as `reproduce` ends), another vertex count (`... number of vertices (<n>) doesn't match what
/// was expected (<m>)`), more than one value per vertex (`... vals_per_vertex was not 1`). A
/// negative size that the file matches fails the allocation (`MRISreadNewCurvatureVector(<file>):
/// calloc failed`, the program ends). With `DIAG_SHOW` and verbose: `reading curvature file...`.
pub fn read_new_curvature_into_array(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    sname: &str,
    in_array_size: i32,
) -> Result<(i32, Option<Vec<f32>>), Fatal> {
    const F: &str = "MRISreadNewCurvatureIntoArray";
    let sname = c_str(sname);
    if verbose_show(g) {
        let _ = write!(io.out, "reading curvature file...");
    }
    let mut fp = match open(dir, sname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            g.notices.add(
                &FS_138,
                format!("{F}'s open-failure message names the file (FreeSurfer's reads an argument it is never given: a crash)"),
            );
            if g.notices.reproduce() {
                if g.errno != 0 {
                    log::error(g, io.err, &log::strerror(g.errno));
                }
                return Err(Fatal::Exit(139));
            }
            log::error_printf(g, io.err, ERROR_BADFILE, &format!("{F}({sname}): fopen failed"));
            return Ok((ERROR_BADFILE, None));
        }
    };
    let (_, vnum) = fread3(&mut fp);
    if vnum != NEW_VERSION_MAGIC_NUMBER {
        fp.close();
        return read_curvature_into_array(g, io, dir, sname, in_array_size);
    }
    let vnum = fread_int(g, &mut fp);
    let _fnum = fread_int(g, &mut fp);
    if vnum != in_array_size {
        fp.close();
        let m = format!(
            "{F}({sname}): number of vertices ({vnum}) doesn't match what was expected ({in_array_size})"
        );
        log::error_printf(g, io.err, ERROR_BADFILE, &m);
        return Ok((ERROR_BADFILE, None));
    }
    let vals_per_vertex = fread_int(g, &mut fp);
    if vals_per_vertex != 1 {
        fp.close();
        let m = format!("{F}({sname}): vals_per_vertex was not 1");
        log::error_printf(g, io.err, ERROR_BADFILE, &m);
        return Ok((ERROR_BADFILE, None));
    }
    if in_array_size < 0 {
        g.errno = ENOMEM;
        log::error_exit(
            g,
            io.err,
            ERROR_NOMEMORY,
            &format!("MRISreadNewCurvatureVector({sname}): calloc failed"),
        )?;
        unreachable!("ErrorExit returns only through its Err");
    }
    let mut cvec = vec![0.0f32; nonneg(in_array_size)];
    for k in 0..nonneg(vnum) {
        cvec[k] = fread_float(g, io.err, &mut fp);
    }
    fp.close();
    Ok((NO_ERROR, Some(cvec)))
}

/// `MRISreadNewCurvatureVector(mris, sname)`: [`read_new_curvature_into_array`] of the file
/// [`value_file_name`] names, `nvertices` values; `None` on an error.
pub fn read_new_curvature_vector(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    sname: &str,
) -> Result<Option<Vec<f32>>, Fatal> {
    let sname = c_str(sname);
    let fname = value_file_name(g, io, mris, sname, "MRISreadNewCurvatureVector", (5882, 5889))?;
    let (code, cvec) = read_new_curvature_into_array(g, io, dir, &fname, mris.nvertices)?;
    Ok(if code == NO_ERROR { cvec } else { None })
}

// ---- the float and area files -----------------------------------------------------------------

/// `MRISreadFloatFile(mris, sname)`: the file [`value_file_name`] names: the vertex and face
/// counts (`int`s), then one `float` per vertex into its `val`. Returns `NO_ERROR`, or
/// `ERROR_NOFILE`: `MRISreadFloatFile: could not open <file>`, `MRISreadFloatFile: incompatible #
/// of vertices in file <file>`, `MRISreadFloatFile: incompatible # of faces file <file>`. With
/// `DIAG_SHOW` and verbose: `reading float file...`.
pub fn read_float_file(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    sname: &str,
) -> Result<i32, Fatal> {
    const F: &str = "MRISreadFloatFile";
    let sname = c_str(sname);
    let fname = value_file_name(g, io, mris, sname, F, (5129, 5136))?;
    if verbose_show(g) {
        let _ = write!(io.out, "reading float file...");
    }
    let mut fp = match open(dir, &fname, "r") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("{F}: could not open {fname}");
            return Ok(open_failed(g, io, e, ERROR_NOFILE, &m));
        }
    };
    let vnum = fread_int(g, &mut fp);
    let fnum = fread_int(g, &mut fp);
    if vnum != mris.nvertices {
        fp.close();
        let m = format!("{F}: incompatible # of vertices in file {fname}");
        log::error_printf(g, io.err, ERROR_NOFILE, &m);
        return Ok(ERROR_NOFILE);
    }
    if fnum != mris.nfaces {
        fp.close();
        let m = format!("{F}: incompatible # of faces file {fname}");
        log::error_printf(g, io.err, ERROR_NOFILE, &m);
        return Ok(ERROR_NOFILE);
    }
    for k in 0..nonneg(vnum) {
        let f = fread_float(g, io.err, &mut fp);
        mris.vertex_mut(k).val = f;
    }
    fp.close();
    Ok(NO_ERROR)
}

/// `MRISreadBinaryAreas(mris, mris_fname)`: `<folder of mris_fname>/<lh|rh>.area` (FS-134, FS-126):
/// the vertex and face counts (3 bytes each), then one `float` per vertex into its `origarea`.
/// Returns `NO_ERROR`, `ERROR_BADPARM` (`MRISreadBinaryAreas: no area file <file>` with its
/// newline) or `ERROR_NOFILE` (`MRISreadBinaryAreas: incompatible vertex number in file <file>`).
/// With `DIAG_SHOW` and verbose: `reading area file...`.
pub fn read_binary_areas(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    mris_fname: &str,
) -> Result<i32, Fatal> {
    const F: &str = "MRISreadBinaryAreas";
    let mris_fname = c_str(mris_fname);
    if verbose_show(g) {
        let _ = write!(io.out, "reading area file...");
    }
    let fpref = file_name_path_buf(g, mris_fname, F);
    let hemi = if mris.hemisphere == LEFT_HEMISPHERE {
        "lh"
    } else {
        "rh"
    };
    let fname = snprintf_name(g, io, format!("{fpref}/{hemi}.area"), F, 5199);
    let mut fp = match open(dir, &fname, "r") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("{F}: no area file {fname}\n");
            return Ok(open_failed(g, io, e, ERROR_BADPARM, &m));
        }
    };
    let (_, vnum) = fread3(&mut fp);
    let (_, _fnum) = fread3(&mut fp);
    if vnum != mris.nvertices {
        fp.close();
        let m = format!("{F}: incompatible vertex number in file {fname}");
        log::error_printf(g, io.err, ERROR_NOFILE, &m);
        return Ok(ERROR_NOFILE);
    }
    for k in 0..nonneg(vnum) {
        let f = fread_float(g, io.err, &mut fp);
        mris.vertex_mut(k).origarea = f;
    }
    fp.close();
    Ok(NO_ERROR)
}

// ---- the names of files to read ---------------------------------------------------------------

/// FS-149: a name written into a caller's 1,024-byte buffer without a bound.
fn caller_buffer(g: &mut Globals, name: String, func: &str) -> String {
    if name.len() >= STRLEN {
        g.notices.add(
            &FS_149,
            format!("{func}: a name of {} bytes, 1,024 or more, is returned whole (FreeSurfer writes past its caller's buffer)", name.len()),
        );
    }
    name
}

/// `MRISbuildFileName_read(mris, sname, fname)`: the file to read for `sname`: a name with a `/` as
/// it is; else, in the folder of the surface's name (`./` with `FS_POSIX` set), the name as it is
/// when it starts with `lh.` or `rh.` before its first dot (the two letters before the first dot
/// `lh` or `rh`), else after the surface's hemisphere (`lh.`, `rh.`, `both.` or nothing).
/// FS-148: a first dot at the name's start (or second, after an `h`) makes FreeSurfer read the
/// byte(s) before the name; larmorx takes them as no hemisphere letters in both modes. FS-149:
/// the legacy names without a hemisphere and the names with a `/` are not bounded by the caller's
/// buffer (larmorx returns them whole); the others are cut at 1,023 bytes (FS-126). Returns
/// `NO_ERROR` and the name.
pub fn build_file_name_read(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    sname: &str,
) -> (i32, String) {
    const F: &str = "MRISbuildFileName_read";
    let sname = c_str(sname);
    if sname.contains('/') {
        return (NO_ERROR, caller_buffer(g, sname.to_owned(), F));
    }
    let b = sname.as_bytes();
    let path = file_name_path(c_str(&mris.fname));
    let posix = io.env.get("FS_POSIX").is_some();
    let hemi_named = match b.iter().position(|&c| c == b'.') {
        None => false,
        Some(dot) => {
            let before = |k: usize| if dot >= k { b[dot - k] } else { 0 };
            if dot == 0 || (dot == 1 && b[0] == b'h') {
                g.notices.add(
                    &FS_148,
                    format!("{F}: {sname}: the bytes before the name are taken as no hemisphere letters"),
                );
            }
            before(1) == b'h' && (before(2) == b'l' || before(2) == b'r')
        }
    };
    let name = if hemi_named {
        if posix {
            snprintf_name(g, io, format!("./{sname}"), F, 5675)
        } else {
            snprintf_name(g, io, format!("{path}/{sname}"), F, 5682)
        }
    } else {
        let hemistr = match mris.hemisphere {
            LEFT_HEMISPHERE => "lh.",
            RIGHT_HEMISPHERE => "rh.",
            BOTH_HEMISPHERES => "both.",
            _ => "",
        };
        if posix {
            snprintf_name(g, io, format!("./{hemistr}{sname}"), F, 5696)
        } else {
            caller_buffer(g, format!("{path}/{hemistr}{sname}"), F)
        }
    };
    (NO_ERROR, name)
}

// ---- the decimation files ---------------------------------------------------------------------

/// `MRISwriteDecimation(mris, fname)`: a NUL byte, the vertex count (`int`), then per vertex a
/// byte, 1 where its `d` is 0. Returns `NO_ERROR`, or `ERROR_BADFILE` (`MRISwriteDecimation: could
/// not create <file>`).
pub fn write_decimation(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> i32 {
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("MRISwriteDecimation: could not create {fname}");
            return open_failed(g, io, e, ERROR_BADFILE, &m);
        }
    };
    fp.put_bytes(&[0]);
    fwrite_int(&mut fp, mris.nvertices);
    for k in 0..nonneg(mris.nvertices) {
        fp.put_bytes(&[u8::from(mris.vertex(k).d == 0.0)]);
    }
    fp.close();
    NO_ERROR
}

/// `MRISreadDecimation(mris, fname)`: every vertex first undefined and not fixed; then a file of
/// [`write_decimation`]'s form, or text (a first byte `#`: a word, the vertex count, one number per
/// vertex): each vertex whose byte or number is not 0 gets `d` 0, fixed and defined. Returns the
/// number of such vertices; 0 with `<program>: decimation file <file> has wrong # of vertices` for
/// another count, 0 with `ERROR_BADFILE` (`MRISreadDecimation: could not create <file>`) for a file
/// that cannot be opened. FS-139 (rule B): a file that ends before every vertex: larmorx's error
/// `MRISreadDecimation: decimation file <file> ends before vertex <k>` (0 returned); FreeSurfer,
/// as `reproduce`, takes the missing bytes as not 0 (decimated) and the missing numbers as the
/// last one read. FS-140: a text file whose count cannot be read: FreeSurfer compares memory
/// contents with the vertex count; larmorx (both modes, `reproduce` with 0 for them) reports the
/// wrong count.
pub fn read_decimation(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
) -> i32 {
    const F: &str = "MRISreadDecimation";
    let fname = c_str(fname);
    for k in 0..nonneg(mris.nvertices) {
        let v = mris.vertex_mut(k);
        v.undefval = 1;
        v.fixedval = 0;
    }
    let mut fp = match open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("{F}: could not create {fname}");
            return open_failed(g, io, e, ERROR_BADFILE, &m);
        }
    };
    let wrong = |g: &mut Globals, io: &mut Io| {
        let m = format!(
            "{}: decimation file {fname} has wrong # of vertices\n",
            g.progname
        );
        log::error_printf(g, io.err, ERROR_BADFILE, &m);
        0
    };
    let ends = |g: &mut Globals, io: &mut Io, k: usize| -> bool {
        g.notices.add(
            &FS_139,
            format!("{fname}: a decimation file that ends early is refused (FreeSurfer goes on with made-up values)"),
        );
        if g.notices.reproduce() {
            return false;
        }
        let m = format!("{F}: decimation file {fname} ends before vertex {k}");
        log::error_printf(g, io.err, ERROR_BADFILE, &m);
        true
    };
    let mut ndec = 0;
    let first = fp.getc();
    // `char c = fgetc(fp)`: the end of the file is -1, not `#`
    let mut decimate = |mris: &mut Mris, k: usize| {
        let v = mris.vertex_mut(k);
        v.d = 0.0;
        v.fixedval = 1;
        v.undefval = 0;
        ndec += 1;
    };
    if first == i32::from(b'#') {
        {
            let mut sc = Scanf::new(&mut fp);
            sc.s(&mut g.errno, usize::MAX, false);
            sc.finish(&mut g.errno);
        }
        let mut sc = Scanf::new(&mut fp);
        let count = sc.d(&mut g.errno, true);
        sc.finish(&mut g.errno);
        let d = match count {
            Some(d) => d,
            None => {
                g.notices.add(
                    &FS_140,
                    format!("{fname}: a decimation count that cannot be read is taken as a wrong count (FreeSurfer compares memory contents)"),
                );
                if !g.notices.reproduce() {
                    return wrong(g, io);
                }
                0
            }
        };
        if d != mris.nvertices {
            return wrong(g, io);
        }
        let mut d = d;
        for k in 0..nonneg(mris.nvertices) {
            let mut sc = Scanf::new(&mut fp);
            let r = sc.d(&mut g.errno, true);
            sc.finish(&mut g.errno);
            match r {
                Some(x) => d = x,
                None => {
                    if ends(g, io, k) {
                        return 0;
                    }
                }
            }
            if d != 0 {
                decimate(mris, k);
            }
        }
    } else {
        let d = fread_int(g, &mut fp);
        if d != mris.nvertices {
            return wrong(g, io);
        }
        for k in 0..nonneg(mris.nvertices) {
            let c = fp.getc();
            if c < 0 && ends(g, io, k) {
                return 0;
            }
            if c != 0 {
                decimate(mris, k);
            }
        }
    }
    fp.close();
    ndec
}

// ---- FS_GII -----------------------------------------------------------------------------------

/// `sprintf(gii, "%s.gii", fname)` into a checked 1,024-byte buffer: a name of 1,020 bytes or
/// more aborts the official build (FS-126); larmorx takes it whole, `reproduce` aborts.
fn gii_name(g: &mut Globals, io: &mut Io, fname: &str, func: &str) -> Result<String, Fatal> {
    let gii = format!("{fname}.gii");
    if gii.len() >= STRLEN {
        g.notices.add(
            &FS_126,
            format!("{func}: a name of {} bytes with .gii is taken whole (FreeSurfer aborts)", gii.len()),
        );
        if g.notices.reproduce() {
            let _ = io
                .err
                .write_all(b"*** buffer overflow detected ***: terminated\n");
            return Err(Fatal::Exit(134));
        }
    }
    Ok(gii)
}

fn exists(g: &mut Globals, dir: Option<&Path>, name: &str) -> bool {
    fio_file_exists_readable(g, dir, name) != 0
}

/// `__MRISapplyFSGIIread(file_to_read, fname, filetype)`: both `fname` and `fname.gii` readable
/// ends the program (`[ERROR] both <f> and <f>.gii exist. Check FS_GII setting, remove unused
/// files`, exit status 1); with `FS_GII` `.gii` and `fname.gii` readable, `file_to_read` becomes
/// it and `filetype` `GIFTI_FILE` (`[INFO] read, FS_GII set, read as <f>.gii (<f>)`; without it
/// `[INFO] read, FS_GII set, cannot find <f>.gii, trying <f> ...`); without `FS_GII` and with
/// `fname` not readable, likewise to `fname.gii` (`[INFO] read, cannot find <f>, trying <f>.gii
/// ...`). The messages on standard output.
pub fn apply_fsgii_read(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    file_to_read: &mut String,
    fname: &str,
    filetype: &mut i32,
) -> Result<(), Fatal> {
    let fname = c_str(fname);
    let gii = gii_name(g, io, fname, "__MRISapplyFSGIIread")?;
    if exists(g, dir, fname) && exists(g, dir, &gii) {
        let _ = writeln!(
            io.out,
            "[ERROR] both {fname} and {gii} exist. Check FS_GII setting, remove unused files"
        );
        return Err(Fatal::Exit(1));
    }
    let fs_gii = io.env.get("FS_GII").map(|v| v.as_encoded_bytes().to_vec());
    if fs_gii.as_deref() == Some(b".gii") {
        if exists(g, dir, &gii) {
            *file_to_read = format!("{fname}.gii");
            let _ = writeln!(
                io.out,
                "[INFO] read, FS_GII set, read as {file_to_read} ({fname})"
            );
            *filetype = GIFTI_FILE;
        } else {
            let _ = writeln!(
                io.out,
                "[INFO] read, FS_GII set, cannot find {gii}, trying {fname} ..."
            );
        }
    } else if !exists(g, dir, fname) {
        *file_to_read = format!("{fname}.gii");
        let _ = writeln!(
            io.out,
            "[INFO] read, cannot find {fname}, trying {file_to_read} ..."
        );
        *filetype = GIFTI_FILE;
    }
    Ok(())
}

/// `__MRISapplyFSGIIwrite(file_to_write, fname, filetype)`: with `FS_GII` `.gii`, `file_to_write`
/// becomes `fname.gii` and `filetype` `GIFTI_FILE` (`[WARN] write, FS_GII set, write as <f>.gii
/// (<f>)`; FS-149: not bounded by the caller's buffer), and a readable `fname` ends the program
/// (`[ERROR] attemp to write GIFTI <f>.gii, its regular form <f> exists`, exit status 1); without
/// it a readable `fname.gii` ends the program (`[ERROR] attemp to write <f>, its GIFTI form <f>.gii
/// exists`). The messages on standard output.
pub fn apply_fsgii_write(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    file_to_write: &mut String,
    fname: &str,
    filetype: &mut i32,
) -> Result<(), Fatal> {
    const F: &str = "__MRISapplyFSGIIwrite";
    let fname = c_str(fname);
    let fs_gii = io.env.get("FS_GII").map(|v| v.as_encoded_bytes().to_vec());
    if fs_gii.as_deref() == Some(b".gii") {
        *file_to_write = caller_buffer(g, format!("{fname}.gii"), F);
        let _ = writeln!(
            io.out,
            "[WARN] write, FS_GII set, write as {file_to_write} ({fname})"
        );
        *filetype = GIFTI_FILE;
        if exists(g, dir, fname) {
            let _ = writeln!(
                io.out,
                "[ERROR] attemp to write GIFTI {file_to_write}, its regular form {fname} exists"
            );
            return Err(Fatal::Exit(1));
        }
    } else {
        let gii = gii_name(g, io, fname, F)?;
        if exists(g, dir, &gii) {
            let _ = writeln!(
                io.out,
                "[ERROR] attemp to write {fname}, its GIFTI form {gii} exists"
            );
            return Err(Fatal::Exit(1));
        }
    }
    Ok(())
}
