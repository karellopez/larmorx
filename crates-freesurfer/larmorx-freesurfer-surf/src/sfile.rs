// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisurf_io.cpp`'s small file readers and writers (`fs.surfio`, queue item 3.1-47 part
//! b): the frame `MRISreadValues` reads (`MRISsetReadFrame`, `MRISgetReadFrame`, lines 64-89),
//! the triangle-property files (`MRISreadTriangleProperties` 99, `MRISwriteTriangleProperties`
//! 213), the distance, area-error and angle-error files (`MRISwriteDists` 343,
//! `MRISwriteAreaError` 535, `MRISwriteAngleError` 625), the curvature as a w file
//! (`MRISwriteCurvatureToWFile` 668), the backup and imaginary value readers
//! (`MRISreadValuesBak` 2042, `MRISreadImagValues` 2079), the marks (`MRIScopyMarksToAnnotation`
//! 2113, `MRISmaxMarked` 2128) and the text formats (`MRISwriteAscii` 2625,
//! `MRISwriteNormalsAscii` 2658, `MRISwriteVTK` 2794, `MRISwriteCurvVTK` 2835).
//!
//! Every file is a C stream (`larmorx-freesurfer-io`'s [`CFile`]) opened in `dir` (the program's
//! working directory, for a relative name); a failed `fopen` sets `errno`, which the error that
//! follows prints first (`ErrorPrintf`), as in C. The numbers are written with fs.io's big-endian
//! helpers (`fwrite2` clamps to ±32,767, `fwrite3` keeps 24 bits, `fwrite4` writes an `int`).
//!
//! The defects: FS-126 (names of 1,024 bytes or more: an abort, a cut with `Truncation on line
//! <n>`), FS-127 (the triangle-property reader takes the counts as floats), FS-128 (the
//! triangle-property writer's angles of the place after the last face), FS-129 (a hemisphere read
//! from before the name), FS-130 (the angle error divided at every face), FS-131 (the angle
//! error's conversion to an integer), FS-132 (the largest mark from a ripped or missing vertex 0),
//! FS-133 (more faces in a triangle-property file than the surface has), FS-134 (a surface or path
//! name of 1,024 bytes or more overruns `FileNamePath`'s buffer).
//!
//! Not here (they wait for fs.io's format identification `mri_identify`, track A):
//! `MRISwriteValues` and `MRISwriteAreaErrorToValFile` (its only caller writes through it).

use std::path::Path;

use larmorx_core::defects::{
    FS_126, FS_127, FS_128, FS_129, FS_130, FS_131, FS_132, FS_133, FS_134,
};
use larmorx_freesurfer_core::cscan;
use larmorx_freesurfer_core::cstd::{double_to_int_x86, fmt_f};
use larmorx_freesurfer_core::diag::{self, DIAG_SHOW};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADPARM, ERROR_NO_FILE, ERROR_NOFILE, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sub};
use larmorx_freesurfer_core::utils::{delta_angle, file_name_path, nint};
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::fio::fwrite_float;
use larmorx_freesurfer_io::fio::{fread_float, fread2, fread3, fread4, fwrite2, fwrite3, fwrite4};

use crate::aaa::{ANGLES_PER_TRIANGLE, SurfGlobals, VERTICES_PER_FACE};
use crate::coords::Io;
use crate::facenorm;
use crate::mp_slow;
use crate::mris::Mris;

/// `STRLEN`: the size of the name buffers.
pub(crate) const STRLEN: usize = 1024;

/// `EDOM` (Linux).
const EDOM: i32 = 33;

/// `LEFT_HEMISPHERE` (`mrisurf.h`).
pub(crate) const LEFT_HEMISPHERE: i32 = 0;

pub(crate) fn nonneg(n: i32) -> usize {
    usize::try_from(n).unwrap_or(0)
}

/// A name as C's string functions see it: the bytes before the first NUL.
pub(crate) fn c_str(name: &str) -> &str {
    match name.find('\0') {
        Some(i) => &name[..i],
        None => name,
    }
}

/// The bytes of a name cut to `n`, as a name again (names are ASCII in practice; a cut inside a
/// character keeps the bytes as text with replacements).
pub(crate) fn cut(name: &str, n: usize) -> String {
    let b = name.as_bytes();
    String::from_utf8_lossy(&b[..n.min(b.len())]).into_owned()
}

/// `snprintf(buf, STRLEN, ...)` of a name, then `if (req >= STRLEN) std::cerr << func <<
/// ": Truncation on line " << line`: FS-126 (rule A): larmorx keeps the whole name; `reproduce`
/// prints the line and cuts the name to 1,023 bytes, as FreeSurfer does.
pub(crate) fn snprintf_name(g: &mut Globals, io: &mut Io, name: String, func: &str, line: u32) -> String {
    if name.len() < STRLEN {
        return name;
    }
    g.notices.add(
        &FS_126,
        format!(
            "{func}: a file name of {} bytes, 1,024 or more, is kept whole (FreeSurfer cuts it)",
            name.len()
        ),
    );
    if g.notices.reproduce() {
        let _ = writeln!(io.err, "{func}: Truncation on line {line}");
        return cut(&name, STRLEN - 1);
    }
    name
}

/// `strcpy(fname, name)` into a `char[STRLEN]` the official build checks (`_FORTIFY_SOURCE`):
/// FS-126 (rule A): a name of 1,024 bytes or more is taken whole; `reproduce` ends the program as
/// FreeSurfer's does (`*** buffer overflow detected ***: terminated`, signal 6: exit status 134).
pub(crate) fn strcpy_checked(g: &mut Globals, io: &mut Io, name: &str, func: &str) -> Result<String, Fatal> {
    if name.len() >= STRLEN {
        g.notices.add(
            &FS_126,
            format!("{func}: a file name of {} bytes, 1,024 or more, is taken whole (FreeSurfer aborts)", name.len()),
        );
        if g.notices.reproduce() {
            let _ = io
                .err
                .write_all(b"*** buffer overflow detected ***: terminated\n");
            return Err(Fatal::Exit(134));
        }
    }
    Ok(name.to_owned())
}

/// `FileNamePath(fname, path)` into a caller's `char[STRLEN]` (an unchecked `strcpy`): FS-134: a
/// name of 1,024 bytes or more overruns the caller's buffer in FreeSurfer (the stack after it:
/// what follows depends on the build); larmorx takes the whole name in both modes.
pub(crate) fn file_name_path_buf(g: &mut Globals, fname: &str, func: &str) -> String {
    if fname.len() >= STRLEN {
        g.notices.add(
            &FS_134,
            format!("{func}: a name of {} bytes, 1,024 or more, is read whole (FreeSurfer writes past its buffer)", fname.len()),
        );
    }
    file_name_path(fname)
}

/// `ErrorReturn(code, (code, message))` after a failed `fopen` (its `errno` printed first).
pub(crate) fn open_failed(g: &mut Globals, io: &mut Io, errno: i32, code: i32, message: &str) -> i32 {
    g.errno = errno;
    log::error_printf(g, io.err, code, message);
    code
}

pub(crate) fn open(dir: Option<&Path>, name: &str, mode: &str) -> Result<CFile, i32> {
    CFile::open(dir, name, mode)
}

// ---- the frame of MRISreadValues --------------------------------------------------------------

/// `MRISsetReadFrame(frame)`: the frame `MRISreadValues` reads from a volume.
pub fn set_read_frame(sg: &mut SurfGlobals, frame: i32) {
    sg.readval_frame = frame;
}

/// `MRISgetReadFrame()`: the frame set (when not negative), else `MRIS_READVAL_FRAME` read as
/// `sscanf("%d")` (0 when it is unset or holds no number; `errno` as glibc's `sscanf` leaves it).
pub fn get_read_frame(g: &mut Globals, sg: &SurfGlobals, io: &Io) -> i32 {
    if sg.readval_frame >= 0 {
        return sg.readval_frame;
    }
    let Some(v) = io.env.get("MRIS_READVAL_FRAME") else {
        return 0;
    };
    let bytes = v.as_encoded_bytes().to_vec();
    let mut st = cscan::Bytes::new(&bytes);
    cscan::Call::new().i32(&mut st, &mut g.errno).unwrap_or(0)
}

// ---- the triangle-property files ----------------------------------------------------------------

/// `strrchr(name, '.')` then `strncpy(hemi, cp - 2, 2)`: the two bytes before the last dot, or
/// `None` without a dot. FS-129 (rule B): a dot at index 0 or 1 makes FreeSurfer read the bytes
/// before the name (memory contents); larmorx refuses such a name as one without a dot; with
/// `reproduce` those bytes read as 0, so the hemisphere is empty.
fn hemisphere(g: &mut Globals, name: &str, func: &str) -> Option<String> {
    let dot = name.rfind('.')?;
    if dot < 2 {
        g.notices.add(
            &FS_129,
            format!("{func}: the hemisphere of {name} would be read from before the name: refused"),
        );
        if g.notices.reproduce() {
            return Some(String::new());
        }
        return None;
    }
    Some(name[dot - 2..dot].to_owned())
}

/// The surface's face `fno` (its place may be a spare one, up to `max_faces`).
fn face_place_ok(mris: &Mris, fno: usize) -> bool {
    mris.faces.as_ref().is_some_and(|f| fno < f.len())
}

/// `(int) f` of a count read as a `float` (x86-64's conversion).
fn float_count(f: f32) -> i32 {
    double_to_int_x86(f64::from(f))
}

/// The two counts at the start of a triangle-property file: FS-127 (rule A): the writer writes
/// `int`s, FreeSurfer's reader reads `float`s and converts them, so it refuses every file the
/// writer wrote (but one of 0 vertices); larmorx reads the `int`s; `reproduce` reads `float`s.
fn read_counts(g: &mut Globals, fp: &mut CFile, nvertices: i32) -> (i32, i32) {
    let (_, a) = fread4(g, fp);
    let (_, b) = fread4(g, fp);
    let (fv, ff) = (float_count(a), float_count(b));
    let (iv, ifc) = (a.to_bits() as i32, b.to_bits() as i32);
    if (fv, ff) != (iv, ifc) && (iv == nvertices || fv == nvertices) {
        g.notices.add(
            &FS_127,
            "a triangle-property file's counts are read as the integers its writer writes (FreeSurfer reads them as floats)",
        );
    }
    if g.notices.reproduce() {
        (fv, ff)
    } else {
        (iv, ifc)
    }
}

/// FS-133 (rule B): a triangle-property file whose face count is not the surface's: FreeSurfer
/// reads as many faces as the file says, past the surface's faces (into its spare places, or past
/// its arrays); larmorx refuses it (`ERROR_NOFILE`, as for the vertex count); with `reproduce`
/// counts beyond the surface's faces fill its spare places and only a count beyond its places is
/// refused.
fn faces_ok(g: &mut Globals, io: &mut Io, mris: &Mris, fnum: i32, fname: &str) -> Option<i32> {
    let places = mris.faces.as_ref().map_or(0, Vec::len);
    let fits = if g.notices.reproduce() {
        nonneg(fnum) <= places
    } else {
        fnum == mris.nfaces
    };
    if fnum != mris.nfaces && fnum > 0 {
        g.notices.add(
            &FS_133,
            format!("{fname}: {fnum} faces for a surface of {}: refused (FreeSurfer reads past the faces)", mris.nfaces),
        );
    }
    if fits || (fnum <= 0 && g.notices.reproduce()) {
        return None;
    }
    log::error_printf(
        g,
        io.err,
        ERROR_NOFILE,
        &format!("MRISreadTriangleProperties: incompatible face number in file {fname}"),
    );
    Some(ERROR_NOFILE)
}

/// `MRISreadTriangleProperties(mris, mris_fname)`: the faces' original areas and angles from
/// `<path>/<hemi>.triangle_area` and `<path>/<hemi>.triangle_angle` (the path and the two letters
/// before the last dot of `mris_fname`). The area file: the vertex and face counts, then each
/// face's area (big-endian floats); the surface's `orig_area` their `float` sum, each vertex's
/// `origarea` the `float` sum of its faces' (`getFaceOrigArea`: FS-116 for a deferred one). The
/// angle file: the counts, then three angles per face into `orig_angle`. Returns `NO_ERROR`; 1
/// when a file is missing (`no precomputed triangle areas and angles - computing...` on standard
/// output for the area file); `ERROR_BADPARM` without a dot (`... could not scan hemisphere from
/// fname`), `ERROR_NOFILE` for another vertex count (`... incompatible vertex number in file
/// <file>`) or face count (FS-133). With `Gdiag` `DIAG_SHOW` and verbose: `reading triangle
/// files...` and `total area = %2.0f.`. FreeSurfer never closes the two files.
pub fn read_triangle_properties(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    mris_fname: &str,
) -> Result<i32, Fatal> {
    const F: &str = "MRISreadTriangleProperties";
    let mris_fname = c_str(mris_fname);
    let verbose = g.gdiag & DIAG_SHOW != 0 && diag::verbose_on(g);
    if verbose {
        let _ = write!(io.out, "reading triangle files...");
    }
    let Some(hemi) = hemisphere(g, mris_fname, F) else {
        log::error_printf(
            g,
            io.err,
            ERROR_BADPARM,
            &format!("{F}({mris_fname}): could not scan hemisphere from fname"),
        );
        return Ok(ERROR_BADPARM);
    };
    let fpref = file_name_path_buf(g, mris_fname, F);
    let fname = snprintf_name(g, io, format!("{fpref}/{hemi}.triangle_area"), F, 126);
    let mut fp = match open(dir, &fname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let _ = write!(
                io.out,
                "\nno precomputed triangle areas and angles - computing...\n"
            );
            return Ok(1);
        }
    };
    let (vnum, fnum) = read_counts(g, &mut fp, mris.nvertices);
    if vnum != mris.nvertices {
        fp.close();
        log::error_printf(
            g,
            io.err,
            ERROR_NOFILE,
            &format!("{F}: incompatible vertex number in file {fname}"),
        );
        return Ok(ERROR_NOFILE);
    }
    if let Some(code) = faces_ok(g, io, mris, fnum, &fname) {
        return Ok(code);
    }
    mris.orig_area = 0.0;
    for fno in 0..nonneg(fnum) {
        let f = fread_float(g, io.err, &mut fp);
        facenorm::set_face_orig_area(mris, fno, f);
        mris.orig_area = add(mris.orig_area, f);
    }
    for vno in 0..nonneg(vnum) {
        mris.vertex_mut(vno).origarea = 0.0;
        for n in 0..usize::from(mris.topology(vno).num) {
            let fno = crate::mp::vface(mris, vno, n);
            let a = facenorm::get_face_orig_area(g, io, mris, fno)?;
            // the official build adds the face's area to the running sum (the face's NaN first)
            let v = mris.vertex_mut(vno);
            v.origarea = add(a, v.origarea);
        }
    }
    if verbose {
        let _ = writeln!(
            io.out,
            "total area = {}.",
            fmt_f(f64::from(mris.orig_area), 2, 0)
        );
    }
    let fname = snprintf_name(g, io, format!("{fpref}/{hemi}.triangle_angle"), F, 174);
    let mut fp = match open(dir, &fname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            return Ok(1);
        }
    };
    let (vnum, fnum) = read_counts(g, &mut fp, mris.nvertices);
    if vnum != mris.nvertices {
        fp.close();
        log::error_printf(
            g,
            io.err,
            ERROR_NOFILE,
            &format!("{F}: incompatible vertex number in file {fname}"),
        );
        return Ok(ERROR_NOFILE);
    }
    if let Some(code) = faces_ok(g, io, mris, fnum, &fname) {
        return Ok(code);
    }
    for fno in 0..nonneg(fnum) {
        for ano in 0..ANGLES_PER_TRIANGLE {
            let f = fread_float(g, io.err, &mut fp);
            mris.face_mut(fno).orig_angle[ano] = f;
        }
    }
    Ok(NO_ERROR)
}

/// `MRISwriteTriangleProperties(mris, mris_fname)`: computes the triangle properties
/// (`MRIScomputeTriangleProperties`), then writes `<path>/<hemi>.triangle_area` (the vertex and
/// face counts as big-endian `int`s, each face's area) and `<path>/<hemi>.triangle_angle` (the
/// counts, three angles per face). FS-128 (rule A): FreeSurfer writes, for every face, the angles
/// of the place after the last face (`faces[nfaces]`: a spare place's, or memory past the faces);
/// larmorx writes each face's own; `reproduce` the spare place's (0 past the surface's places,
/// with the notice). Returns `NO_ERROR`, `ERROR_BADPARM` without a dot (`... could not
/// scanhemisphere from fname`, FreeSurfer's missing space), `ERROR_NO_FILE` when a file cannot be
/// created.
pub fn write_triangle_properties(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    mris_fname: &str,
) -> Result<i32, Fatal> {
    const F: &str = "MRISwriteTriangleProperties";
    mp_slow::compute_triangle_properties(g, sg, io, mris);
    let mris_fname = c_str(mris_fname);
    let Some(hemi) = hemisphere(g, mris_fname, F) else {
        log::error_printf(
            g,
            io.err,
            ERROR_BADPARM,
            &format!("{F}({mris_fname}): could not scanhemisphere from fname"),
        );
        return Ok(ERROR_BADPARM);
    };
    let fpref = file_name_path_buf(g, mris_fname, F);
    let fname = snprintf_name(g, io, format!("{fpref}/{hemi}.triangle_area"), F, 236);
    let mut fp = match open(dir, &fname, "wb") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("{F}: could not open {fname}");
            return Ok(open_failed(g, io, e, ERROR_NO_FILE, &m));
        }
    };
    let nfaces = nonneg(mris.nfaces);
    fwrite4(&mut fp, mris.nvertices);
    fwrite4(&mut fp, mris.nfaces);
    for fno in 0..nfaces {
        fwrite_float(&mut fp, mris.face(fno).area);
    }
    fp.close();
    let fname = snprintf_name(g, io, format!("{fpref}/{hemi}.triangle_angle"), F, 254);
    let mut fp = match open(dir, &fname, "wb") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("{F}: could not open {fname}");
            return Ok(open_failed(g, io, e, ERROR_NO_FILE, &m));
        }
    };
    fwrite4(&mut fp, mris.nvertices);
    fwrite4(&mut fp, mris.nfaces);
    // the place the first loop's face counter ends at
    let after = if nfaces > 0 { nfaces } else { 0 };
    let stale = if face_place_ok(mris, after) {
        mris.face(after).angle
    } else {
        [0.0; ANGLES_PER_TRIANGLE]
    };
    for fno in 0..nfaces {
        let own = mris.face(fno).angle;
        if own.map(f32::to_bits) != stale.map(f32::to_bits) {
            g.notices.add(
                &FS_128,
                if face_place_ok(mris, after) {
                    "each face's own angles are written (FreeSurfer writes the angles of the place after the last face)"
                } else {
                    "each face's own angles are written (FreeSurfer writes memory past the faces: 0 with reproduce)"
                },
            );
        }
        let ang = if g.notices.reproduce() { stale } else { own };
        for a in ang {
            fwrite_float(&mut fp, a);
        }
    }
    fp.close();
    Ok(NO_ERROR)
}

// ---- the distance, area-error and angle-error files --------------------------------------------

/// `MRISwriteDists(mris, sname)`: each vertex's `d` (`nint(d * 100)`, clamped to ±32,767) after
/// the vertex and face counts (3 bytes each), into `sname` when it holds a `/`, else beside the
/// surface's file (`<path>/<sname>`, or `<path>/<lh|rh>.<sname>` when `sname` has no dot), or in
/// the working directory with `FS_POSIX` set (`./<sname>`, `./<lh|rh>.<sname>`). Returns
/// `NO_ERROR`, or `ERROR_NOFILE` (`MRISwriteDists: could not open <file>`). FS-126 for names of
/// 1,024 bytes or more.
pub fn write_dists(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    sname: &str,
) -> Result<i32, Fatal> {
    const F: &str = "MRISwriteDists";
    let sname = c_str(sname);
    let fname = if !sname.contains('/') {
        // the surface's own name is at most 1,023 bytes (MRIS::fname's buffer)
        let path = file_name_path(&mris.fname);
        let hemi = if mris.hemisphere == LEFT_HEMISPHERE {
            "lh"
        } else {
            "rh"
        };
        let posix = io.env.get("FS_POSIX").is_some();
        match (sname.contains('.'), posix) {
            (false, true) => snprintf_name(g, io, format!("./{hemi}.{sname}"), F, 361),
            (false, false) => snprintf_name(g, io, format!("{path}/{hemi}.{sname}"), F, 369),
            (true, true) => snprintf_name(g, io, format!("./{sname}"), F, 377),
            (true, false) => snprintf_name(g, io, format!("{path}/{sname}"), F, 384),
        }
    } else {
        strcpy_checked(g, io, sname, F)?
    };
    let mut fp = match open(dir, &fname, "wb") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("{F}: could not open {fname}");
            return Ok(open_failed(g, io, e, ERROR_NOFILE, &m));
        }
    };
    fwrite3(&mut fp, mris.nvertices);
    fwrite3(&mut fp, mris.nfaces);
    for k in 0..nonneg(mris.nvertices) {
        let dist = mris.vertex(k).d;
        let i = nint(g, mul64(f64::from(dist), 100.0));
        fwrite2(&mut fp, i);
    }
    fp.close();
    Ok(NO_ERROR)
}

/// `MRISwriteAreaError(mris, name)`: per vertex, `nint((area - orig_area) * 100 / num)` in
/// `float` (the sums of its faces' areas and cached original areas, `getFaceNorm`), clamped to
/// ±32,767, after the two counts (3 bytes each). A vertex without faces divides 0 by 0: `nint` of
/// NaN (FS-12). Returns `NO_ERROR`, or `ERROR_NOFILE` (`MRISwriteAreaError: could not open
/// <file>`). With `DIAG_SHOW` and verbose: `writing area error file <file>...` and `done.`.
pub fn write_area_error(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    name: &str,
) -> Result<i32, Fatal> {
    const F: &str = "MRISwriteAreaError";
    let fname = strcpy_checked(g, io, c_str(name), F)?;
    let verbose = g.gdiag & DIAG_SHOW != 0 && diag::verbose_on(g);
    if verbose {
        let _ = write!(io.out, "writing area error file {fname}...");
    }
    let mut fp = match open(dir, &fname, "wb") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("{F}: could not open {fname}");
            return Ok(open_failed(g, io, e, ERROR_NOFILE, &m));
        }
    };
    fwrite3(&mut fp, mris.nvertices);
    fwrite3(&mut fp, mris.nfaces);
    for vno in 0..nonneg(mris.nvertices) {
        let (mut area, mut orig_area) = (0.0f32, 0.0f32);
        let num = mris.topology(vno).num;
        for fi in 0..usize::from(num) {
            let fno = crate::mp::vface(mris, vno, fi);
            let fa = mris.face(fno).area;
            let e = facenorm::get_face_norm(mris, fno);
            area = add(area, fa);
            orig_area = add(orig_area, e.orig_area);
        }
        let x = div(mul(sub(area, orig_area), 100.0), f32::from(num));
        let i = nint(g, f64::from(x));
        fwrite2(&mut fp, i);
    }
    fp.close();
    if verbose {
        let _ = writeln!(io.out, "done.");
    }
    Ok(NO_ERROR)
}

/// `(int) x` of the angle error (FS-131, D13's conversions: clamped by default, NaN 0; x86-64's
/// `INT_MIN` with `reproduce`).
fn angle_error_int(g: &mut Globals, x: f64) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        g.notices.add(
            &FS_131,
            format!("MRISwriteAngleError: an angle error of {x} hundredths of a degree, outside the int range, clamped (FreeSurfer's conversion gives -2147483648)"),
        );
        if g.notices.reproduce() {
            return double_to_int_x86(x);
        }
        if x.is_nan() {
            return 0;
        }
        return if x > 0.0 { i32::MAX } else { i32::MIN };
    }
    x.trunc() as i32
}

/// `MRISwriteAngleError(mris, fname)`: per vertex, the mean of `|deltaAngle(angle,
/// orig_angle)|` over its faces' three angles (in `float`), in hundredths of a degree
/// (`DEGREES(error) * 100`, truncated: FS-131), clamped to ±32,767, after the two counts. FS-130
/// (rule A): FreeSurfer divides the running sum by `3 * num` after every face, so a vertex of
/// more than one face gets less than the mean; larmorx divides once; `reproduce` at every face.
/// Returns `NO_ERROR`, or `ERROR_NOFILE` (`MRISwriteAngleError: could not open <file>`). With
/// `DIAG_SHOW` and verbose: `writing angular error file <file>...` and `done.`.
pub fn write_angle_error(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> Result<i32, Fatal> {
    const F: &str = "MRISwriteAngleError";
    let fname = c_str(fname);
    let verbose = g.gdiag & DIAG_SHOW != 0 && diag::verbose_on(g);
    if verbose {
        let _ = write!(io.out, "writing angular error file {fname}...");
    }
    let mut fp = match open(dir, fname, "wb") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("{F}: could not open {fname}");
            return Ok(open_failed(g, io, e, ERROR_NOFILE, &m));
        }
    };
    fwrite3(&mut fp, mris.nvertices);
    fwrite3(&mut fp, mris.nfaces);
    for vno in 0..nonneg(mris.nvertices) {
        let num = mris.topology(vno).num;
        let denom = (i32::from(num) * ANGLES_PER_TRIANGLE as i32) as f32;
        let (mut error, mut fixed) = (0.0f32, 0.0f32);
        for fi in 0..usize::from(num) {
            let face = mris.face(crate::mp::vface(mris, vno, fi));
            for ano in 0..ANGLES_PER_TRIANGLE {
                let d = delta_angle(face.angle[ano], face.orig_angle[ano]).abs();
                error = add(error, d);
                fixed = add(fixed, d);
            }
            error = div(error, denom);
        }
        if num > 0 {
            fixed = div(fixed, denom);
        }
        if num > 1 && error.to_bits() != fixed.to_bits() {
            g.notices.add(
                &FS_130,
                "the angle error is the mean over the vertex's faces (FreeSurfer divides after every face)",
            );
        }
        let e = if g.notices.reproduce() { error } else { fixed };
        // DEGREES(error) * 100.0: ((error * 360) / (2 pi)) * 100, the error first
        let deg = mul64(
            div64(mul64(f64::from(e), 360.0), 2.0 * std::f64::consts::PI),
            100.0,
        );
        let i = angle_error_int(g, deg);
        fwrite2(&mut fp, i);
    }
    fp.close();
    if verbose {
        let _ = writeln!(io.out, "done.");
    }
    Ok(NO_ERROR)
}

// ---- the curvature as a w file, the value readers ------------------------------------------------

/// `MRISwriteCurvatureToWFile(mris, fname)`: a w file of every vertex's curvature: `0` (2 bytes),
/// the vertex count (3 bytes), then per vertex its number (3 bytes) and value (a big-endian
/// float). A value that is not finite: the error `MRISwriteCurvatureToWFile(<file>): val at vertex
/// <k> is notfinite` (FreeSurfer's missing space), written anyway. A file that cannot be created
/// ends the program (`Can't create file <file>` with its newline, exit status 255). With
/// `DIAG_SHOW`: `writing out surface values to <file>.` and `avg = %2.3f, stdev = %2.3f, min =
/// %2.3f, max = %2.3f` (the mean and standard deviation in `double` from the sums of the values
/// and of their `float` squares; min and max from ±1000). Returns `NO_ERROR`.
pub fn write_curvature_to_wfile(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> Result<i32, Fatal> {
    let fname = c_str(fname);
    let show = g.gdiag & DIAG_SHOW != 0;
    if show {
        let _ = writeln!(io.out, "writing out surface values to {fname}.");
    }
    let mut fp = match open(dir, fname, "wb") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_exit(
                g,
                io.err,
                ERROR_NOFILE,
                &format!("Can't create file {fname}\n"),
            )?;
            unreachable!("ErrorExit returns only through its Err");
        }
    };
    let num = mris.nvertices;
    let (mut sum, mut sum2, mut max, mut min) = (0.0f64, 0.0f64, -1000.0f64, 1000.0f64);
    fwrite2(&mut fp, 0);
    fwrite3(&mut fp, num);
    for k in 0..nonneg(mris.nvertices) {
        fwrite3(&mut fp, k as i32);
        let f = mris.vertex(k).curv;
        if !f.is_finite() {
            log::error_printf(
                g,
                io.err,
                ERROR_BADPARM,
                &format!("MRISwriteCurvatureToWFile({fname}): val at vertex {k} is notfinite"),
            );
        }
        fwrite_float(&mut fp, f);
        sum = add64(sum, f64::from(f));
        // the official build adds the running sum to the square (the square's NaN first)
        sum2 = add64(f64::from(mul(f, f)), sum2);
        if f64::from(f) > max {
            max = f64::from(f);
        }
        if f64::from(f) < min {
            min = f64::from(f);
        }
    }
    fp.close();
    sum = div64(sum, f64::from(num));
    // glibc's sqrt: EDOM for an argument below 0 (computed whether or not it is printed)
    let var = larmorx_freesurfer_core::sse::sub64(div64(sum2, f64::from(num)), mul64(sum, sum));
    if var < 0.0 {
        g.errno = EDOM;
    }
    let sd = larmorx_freesurfer_core::sse::sqrt64(var);
    if show {
        let _ = writeln!(
            io.out,
            "avg = {}, stdev = {}, min = {}, max = {}",
            fmt_f(sum, 2, 3),
            fmt_f(sd, 2, 3),
            fmt_f(min, 2, 3),
            fmt_f(max, 2, 3)
        );
    }
    Ok(NO_ERROR)
}

/// The reader of `MRISreadValuesBak` and `MRISreadImagValues`: a w file (a 2-byte latency, the
/// count, then each vertex number and value); every vertex's value first 0; a vertex number
/// outside the surface prints `<func>: vertex index out of range: <k> f=<value as %f>` on
/// standard output; a value the file does not hold is fs.io's `freadFloat` (an error, FS-167).
fn read_w_values(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
    func: &str,
    set: fn(&mut crate::mris::Vertex, f32),
) -> i32 {
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("{func}: File {fname} not found\n");
            return open_failed(g, io, e, ERROR_NOFILE, &m);
        }
    };
    let _ilat = fread2(g, &mut fp);
    for k in 0..nonneg(mris.nvertices) {
        set(mris.vertex_mut(k), 0.0);
    }
    let (_, num) = fread3(&mut fp);
    for _ in 0..num {
        let (_, k) = fread3(&mut fp);
        let f = fread_float(g, io.err, &mut fp);
        if k >= mris.nvertices || k < 0 {
            let _ = writeln!(
                io.out,
                "{func}: vertex index out of range: {k} f={}",
                fmt_f(f64::from(f), 0, 6)
            );
        } else {
            set(mris.vertex_mut(k as usize), f);
        }
    }
    fp.close();
    NO_ERROR
}

/// `MRISreadValuesBak(mris, fname)`: a w file into the vertices' `valbak` ([`read_w_values`]).
/// Returns `NO_ERROR`, or `ERROR_NOFILE` (`MRISreadValuesBak: File <file> not found` with its
/// newline).
pub fn read_values_bak(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
) -> i32 {
    read_w_values(g, io, dir, mris, fname, "MRISreadValuesBak", |v, f| {
        v.valbak = f;
    })
}

/// `MRISreadImagValues(mris, fname)`: a w file into the vertices' `imag_val`
/// ([`read_w_values`]). Returns `NO_ERROR`, or `ERROR_NOFILE` (`MRISreadImagValues: File <file>
/// not found` with its newline).
pub fn read_imag_values(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
) -> i32 {
    read_w_values(g, io, dir, mris, fname, "MRISreadImagValues", |v, f| {
        v.imag_val = f;
    })
}

// ---- the marks --------------------------------------------------------------------------------

/// `MRIScopyMarksToAnnotation(mris)`: each unripped vertex's annotation becomes its mark.
pub fn copy_marks_to_annotation(mris: &mut Mris) -> i32 {
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        v.annotation = i32::from(v.marked);
    }
    NO_ERROR
}

/// `MRISmaxMarked(mris)`: the largest mark of the unripped vertices. FS-132 (rule A): FreeSurfer
/// starts from vertex 0's mark even when vertex 0 is ripped, and reads it for a surface without
/// vertices (its first place's mark, or memory past an empty array); larmorx takes the unripped
/// vertices only (0 when there is none); `reproduce` starts from vertex 0's place (0 when the
/// surface has no place at all, with the notice).
pub fn max_marked(g: &mut Globals, mris: &Mris) -> i32 {
    let places = mris.vertices.as_ref().map_or(0, Vec::len);
    let start = if places > 0 {
        i32::from(mris.vertex(0).marked)
    } else {
        0
    };
    let mut c_max = start;
    for vno in 1..nonneg(mris.nvertices) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        if i32::from(v.marked) > c_max {
            c_max = i32::from(v.marked);
        }
    }
    let mut fixed: Option<i32> = None;
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex(vno);
        if v.ripflag == 0 {
            fixed = Some(fixed.map_or(i32::from(v.marked), |m| m.max(i32::from(v.marked))));
        }
    }
    let fixed = fixed.unwrap_or(0);
    if fixed != c_max || places == 0 {
        g.notices.add(
            &FS_132,
            if places == 0 {
                "the largest mark of a surface without vertices is 0 (FreeSurfer reads memory it does not own)"
            } else {
                "the largest mark is the unripped vertices' (FreeSurfer starts from vertex 0's, ripped or missing)"
            },
        );
    }
    if g.notices.reproduce() { c_max } else { fixed }
}

// ---- the text formats -------------------------------------------------------------------------

/// `"%f  %f  %f  %d\n"` of three floats and a rip flag.
fn line3(a: f32, b: f32, c: f32, rip: i8) -> String {
    format!(
        "{}  {}  {}  {}\n",
        fmt_f(f64::from(a), 0, 6),
        fmt_f(f64::from(b), 0, 6),
        fmt_f(f64::from(c), 0, 6),
        rip
    )
}

/// The faces of the two ASCII formats: `"%d %d %d %d\n"` (the vertices and the rip flag).
fn ascii_faces(fp: &mut CFile, mris: &Mris) {
    for fno in 0..nonneg(mris.nfaces) {
        let f = mris.face(fno);
        let mut s = String::new();
        for n in 0..VERTICES_PER_FACE {
            s.push_str(&format!("{} ", f.v[n]));
        }
        s.push_str(&format!("{}\n", f.ripflag));
        fp.put_bytes(s.as_bytes());
    }
}

/// `MRISwriteAscii(mris, fname)`: `#!ascii version of <surface's file>`, the counts, each
/// vertex's coordinates and rip flag (`%f`), each face's vertices and rip flag. Returns
/// `NO_ERROR`, or `ERROR_NOFILE` (`MRISwriteAscii: could not open file <file>`).
pub fn write_ascii(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> i32 {
    write_ascii_of(g, io, dir, mris, fname, false)
}

/// `MRISwriteNormalsAscii(mris, fname)`: [`write_ascii`] with the vertices' normals for their
/// coordinates and `(vertices are surface normals)` after the first line's name.
pub fn write_normals_ascii(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> i32 {
    write_ascii_of(g, io, dir, mris, fname, true)
}

fn write_ascii_of(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
    normals: bool,
) -> i32 {
    let fname = c_str(fname);
    let func = if normals {
        "MRISwriteNormalsAscii"
    } else {
        "MRISwriteAscii"
    };
    let mut fp = match open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("{func}: could not open file {fname}");
            return open_failed(g, io, e, ERROR_NOFILE, &m);
        }
    };
    let surf = c_str(&mris.fname);
    let head = if normals {
        format!("#!ascii version of {surf} (vertices are surface normals)\n")
    } else {
        format!("#!ascii version of {surf}\n")
    };
    fp.put_bytes(head.as_bytes());
    fp.put_bytes(format!("{} {}\n", mris.nvertices, mris.nfaces).as_bytes());
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex(vno);
        let s = if normals {
            line3(v.nx, v.ny, v.nz, v.ripflag)
        } else {
            line3(v.x, v.y, v.z, v.ripflag)
        };
        fp.put_bytes(s.as_bytes());
    }
    ascii_faces(&mut fp, mris);
    fp.close();
    NO_ERROR
}

/// `MRISwriteVTK(mris, fname)`: a legacy VTK polygon file: its header, `POINTS <n> float`, the
/// coordinates (`%.9f`, two spaces apart), `POLYGONS <nfaces> <4 nfaces>` and `3 v0 v1 v2 ` per
/// face. Returns `NO_ERROR`, or `ERROR_NOFILE` (`MRISwriteVTK: could not open file <file>`).
pub fn write_vtk(
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
            let m = format!("MRISwriteVTK: could not open file {fname}");
            return open_failed(g, io, e, ERROR_NOFILE, &m);
        }
    };
    fp.put_bytes(b"# vtk DataFile Version 1.0\n");
    fp.put_bytes(
        format!(
            "vtk output\nASCII\nDATASET POLYDATA\nPOINTS {} float\n",
            mris.nvertices
        )
        .as_bytes(),
    );
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex(vno);
        let s = format!(
            "{}  {}  {}\n",
            fmt_f(f64::from(v.x), 0, 9),
            fmt_f(f64::from(v.y), 0, 9),
            fmt_f(f64::from(v.z), 0, 9)
        );
        fp.put_bytes(s.as_bytes());
    }
    fp.put_bytes(format!("POLYGONS {} {}\n", mris.nfaces, mris.nfaces.wrapping_mul(4)).as_bytes());
    for fno in 0..nonneg(mris.nfaces) {
        let f = mris.face(fno);
        let mut s = format!("{VERTICES_PER_FACE} ");
        for n in 0..VERTICES_PER_FACE {
            s.push_str(&format!("{} ", f.v[n]));
        }
        s.push('\n');
        fp.put_bytes(s.as_bytes());
    }
    fp.close();
    NO_ERROR
}

/// `MRISwriteCurvVTK(mris, fname)`: appends the curvature to a VTK file ([`write_vtk`]'s):
/// `POINT_DATA <n>`, `FIELD FieldData 1`, `curv 1 <n> double`, one `%.9f` per vertex. Returns
/// `NO_ERROR`, or `ERROR_NOFILE` (`MRISwriteScalarVTK: could not open file <file>`, FreeSurfer's
/// older name).
pub fn write_curv_vtk(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    fname: &str,
) -> i32 {
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "a") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("MRISwriteScalarVTK: could not open file {fname}");
            return open_failed(g, io, e, ERROR_NOFILE, &m);
        }
    };
    let n = mris.nvertices;
    fp.put_bytes(format!("POINT_DATA {n}\nFIELD FieldData 1\ncurv 1 {n} double\n").as_bytes());
    for vno in 0..nonneg(mris.nvertices) {
        let s = format!("{}\n", fmt_f(f64::from(mris.vertex(vno).curv), 0, 9));
        fp.put_bytes(s.as_bytes());
    }
    fp.close();
    NO_ERROR
}
