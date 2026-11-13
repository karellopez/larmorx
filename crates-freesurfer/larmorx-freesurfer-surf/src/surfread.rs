// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisurf_io.cpp`'s file-local surface readers of queue item 3.1-48 part b
//! (`fs.surfio`): the ASCII surface (`mrisReadAsciiFile` 3342), the movie.byu surface and its
//! positions (`mrisReadGeoFile` 3403, `mrisReadGeoFilePositions` 3476), the binary triangle
//! file's positions (`mrisReadTriangleFileVertexPositionsOnly` 5387, `mrisReadTriangleFilePositions`
//! 5442), and the old surface writer's quadrangles (`quadCombine` 4341). `MRISreadOverAlloc`,
//! `MRISreadVertexPositions`, `MRISreadVerticesOnly` and `MRISwrite` call them (they wait for fs.io's
//! format identification, `mri_identify`, queue item 3.1-27); the raw surfaces they return are not
//! completed here (topology, metric properties: `MRISreadOverAlloc`'s work).
//!
//! The text readers' numbers go through one C `scanf` call each ([`Scanf`]); a number a call
//! cannot read keeps the value read before it (FreeSurfer's variables are stack slots that keep
//! it: FS-152; the first: memory contents, FS-151); a face vertex that cannot be read keeps the 0
//! `MRISalloc` gave it.
//!
//! The defects: FS-147 (a vertex of more than 255 faces), FS-150 (a text file of no line),
//! FS-151 (counts and first values from memory contents), FS-152 (values that cannot be read),
//! FS-153 (a movie.byu face count of 0: an arithmetic exception), FS-154 (a movie.byu face vertex
//! outside the surface: memory outside the arrays), FS-155 (movie.byu positions of another
//! surface), FS-156 (a negative triangle-file vertex count); fs.io's FS-49 and FS-167 (a number
//! the file ends before).

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use std::path::Path;

use larmorx_core::defects::{
    Defect, FS_147, FS_150, FS_151, FS_152, FS_153, FS_154, FS_155, FS_156,
};
use larmorx_freesurfer_core::cscan::Bytes;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{self, DIAG_SHOW};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOFILE, ERROR_NOMEMORY, Fatal, NO_ERROR,
};
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::fio::{fread_float, fread_int, fread3};

use crate::aaa::{MrisStatus, SurfGlobals, VERTICES_PER_FACE};
use crate::alloc;
use crate::coords::{self, Io};
use crate::curvio::{error_printf_bytes, fgetl_line};
use crate::dist;
use crate::formats::MRIS_TRIANGULAR_SURFACE;
use crate::mris::Mris;
use crate::mrisurf::{SmallSurface, SmallVertex};
use crate::scanf::Scanf;
use crate::sfile::{STRLEN, c_str, nonneg, open};

/// `ENOMEM` (Linux): what a failed `calloc` leaves in `errno`.
const ENOMEM: i32 = 12;

/// `MRIS_GEO_TRIANGLE_FILE` (`mrisurf.h`): the movie.byu format.
pub const MRIS_GEO_TRIANGLE_FILE: i32 = 2;

fn io_cpp() -> String {
    format!("{}/utils/mrisurf_io.cpp", diag::BUILD_ROOT)
}

fn verbose_show(g: &Globals) -> bool {
    g.gdiag & DIAG_SHOW != 0 && diag::verbose_on(g)
}

/// A `fopen` that failed: `errno` set, the message printed (`ErrorReturn`'s `ErrorPrintf`).
fn not_opened(g: &mut Globals, io: &mut Io, errno: i32, message: &str) {
    g.errno = errno;
    log::error_printf(g, io.err, ERROR_NOFILE, message);
}

/// The values one `scanf` call assigned, each kept from the call before where this one could not
/// read it (FreeSurfer's variables keep their last values: FS-152), memory contents where no call
/// read it yet (FS-151: 0 here). `Ok(None)` when the correction refuses the file (its message
/// printed).
struct Kept<const N: usize> {
    last: [Option<f64>; N],
}

impl<const N: usize> Kept<N> {
    fn new() -> Kept<N> {
        Kept { last: [None; N] }
    }

    /// The values after a call that read `got`; `what` names the values for the notices, `place`
    /// the vertex or face.
    fn take(
        &mut self,
        g: &mut Globals,
        got: [Option<f64>; N],
        fname: &str,
        place: &str,
    ) -> Option<[f64; N]> {
        let mut out = [0.0; N];
        let mut short = false;
        for k in 0..N {
            match got[k] {
                Some(x) => self.last[k] = Some(x),
                None => short = true,
            }
        }
        if short {
            let first = self.last.iter().any(Option::is_none);
            let (d, text): (&'static Defect, String) = if first {
                (
                    &FS_151,
                    format!("{fname}: {place}'s numbers that cannot be read are refused (FreeSurfer takes memory contents; 0 here)"),
                )
            } else {
                (
                    &FS_152,
                    format!("{fname}: {place}'s numbers that cannot be read are refused (FreeSurfer keeps the ones read before)"),
                )
            };
            g.notices.add(d, text);
            if !g.notices.reproduce() {
                return None;
            }
        }
        for k in 0..N {
            out[k] = self.last[k].unwrap_or(0.0);
        }
        Some(out)
    }
}

/// The error of a value the correction refuses (FS-151, FS-152): `<who>(<file>): could not read
/// <place>`; the stream closed.
fn unreadable(g: &mut Globals, io: &mut Io, fp: CFile, who: &str, fname: &str, place: &str) {
    fp.close();
    log::error_printf(
        g,
        io.err,
        ERROR_BADFILE,
        &format!("{who}({fname}): could not read {place}"),
    );
}

/// `cp = fgetl(line, n, fp)` where the reader passes `cp` to `sscanf` unchecked: `None` (the
/// correction's error printed, the stream closed) when the file holds no line (FS-150:
/// FreeSurfer's `sscanf(NULL, ...)` faults).
fn first_line(
    g: &mut Globals,
    io: &mut Io,
    fp: &mut Option<CFile>,
    n: usize,
    who: &str,
    fname: &str,
) -> Result<Option<Vec<u8>>, Fatal> {
    let f = fp.as_mut().expect("an open stream");
    if let Some((line, at)) = fgetl_line(f, n) {
        return Ok(Some(line[at..].to_vec()));
    }
    g.notices.add(
        &FS_150,
        format!("{fname}: a surface file of no line is refused (FreeSurfer crashes)"),
    );
    if g.notices.reproduce() {
        return Err(Fatal::Exit(139));
    }
    fp.take().expect("an open stream").close();
    log::error_printf(
        g,
        io.err,
        ERROR_BADFILE,
        &format!("{who}({fname}): the file holds no line"),
    );
    Ok(None)
}

/// `vertices_topology[v].num++` of a face's vertex: one byte (FS-147: past 255 the count wraps
/// around; refused by default, `false` then).
fn count_face(g: &mut Globals, mris: &mut Mris, v: usize, fname: &str) -> bool {
    if mris.topology(v).num == u8::MAX {
        g.notices.add(
            &FS_147,
            format!("{fname}: a vertex of more than 255 faces is refused (FreeSurfer's count wraps around)"),
        );
        if !g.notices.reproduce() {
            return false;
        }
    }
    let t = mris.topology_mut(v);
    t.num = t.num.wrapping_add(1);
    true
}

// ---- the ASCII surface ------------------------------------------------------------------------

/// `mrisReadAsciiFile(fname)`: a new surface (`MRISalloc(nvertices, nfaces)`, type
/// `MRIS_TRIANGULAR_SURFACE`) from a text file: a first line (`fgetl`) `<nvertices> <nfaces>`, then
/// per vertex `x y z rip` (floats and an `int` kept in the vertex's one-byte rip flag), per face
/// three vertex numbers and its rip flag, all read as free-format numbers (`%f  %f  %f  %d\n`,
/// `%d ` three times, `%d\n`). A vertex with a nonzero rip flag makes the surface a patch
/// (`patch` 1, status `MRIS_PLANE`). Returns `None` after an error: the file cannot be opened
/// (`MRISreadAsciiFile: could not open file <file>`, `ERROR_NOFILE`); a face vertex outside the
/// surface ends the program (`<program>: face <f> vertex <n>: <v> -- out of range!!! Should be in
/// [0 <nvertices - 1>]`, `ERROR_BADPARM`). The defects: FS-147, FS-150, FS-151, FS-152 (the
/// corrections' errors `mrisReadAsciiFile(<file>): the file holds no line`, `... could not read
/// the counts`, `... could not read vertex <v>`, `... could not read face <f>`, `... vertex <v> is
/// in more than 255 faces`, all `ERROR_BADFILE`).
pub fn read_ascii_file(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Mris>, Fatal> {
    const WHO: &str = "mrisReadAsciiFile";
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "r") {
        Ok(f) => Some(f),
        Err(e) => {
            not_opened(
                g,
                io,
                e,
                &format!("MRISreadAsciiFile: could not open file {fname}"),
            );
            return Ok(None);
        }
    };
    let mut patch = 0;
    let Some(cp) = first_line(g, io, &mut fp, STRLEN, WHO, fname)? else {
        return Ok(None);
    };
    let mut fp = fp.expect("an open stream");
    let (nvertices, nfaces) = {
        let mut b = Bytes::new(&cp);
        let mut sc = Scanf::new(&mut b);
        let e = &mut g.errno;
        let nv = sc.d(e, true);
        sc.text(e, b" ");
        let nf = sc.d(e, true);
        sc.text(e, b"\n");
        sc.finish(e);
        let mut counts = Kept::<2>::new();
        let got = [nv.map(f64::from), nf.map(f64::from)];
        match counts.take(g, got, fname, "the first line") {
            Some(c) => (c[0] as i32, c[1] as i32),
            None => {
                unreadable(g, io, fp, WHO, fname, "the counts");
                return Ok(None);
            }
        }
    };
    let mut mris = alloc::alloc(g, io.out, io.err, sg, nvertices, nfaces)?;
    mris.r#type = MRIS_TRIANGULAR_SURFACE;
    let file = io_cpp();
    // `x`, `y`, `z` (the loop's) and `rip` (the function's, shared with the faces)
    let mut xyz = Kept::<3>::new();
    let mut rip = Kept::<1>::new();
    for vno in 0..nonneg(mris.nvertices) {
        let mut sc = Scanf::new(&mut fp);
        let e = &mut g.errno;
        let mut got = [None; 3];
        for (k, x) in got.iter_mut().enumerate() {
            if k > 0 {
                sc.text(e, b"  ");
            }
            *x = sc.f(e, true).map(f64::from);
        }
        sc.text(e, b"  ");
        let r = sc.d(e, true).map(f64::from);
        sc.text(e, b"\n");
        sc.finish(e);
        let place = format!("vertex {vno}");
        let c = xyz.take(g, got, fname, &place);
        let r = c.and_then(|_| rip.take(g, [r], fname, &place));
        let (Some(c), Some(r)) = (c, r) else {
            unreadable(g, io, fp, WHO, fname, &place);
            return Ok(None);
        };
        let (x, y, z) = (c[0] as f32, c[1] as f32, c[2] as f32);
        coords::set_xyz(g, io, &mut mris, vno as i32, x, y, z, &file, 3362)?;
        let v = mris.vertex_mut(vno);
        v.ripflag = r[0] as i32 as i8;
        if v.ripflag != 0 {
            patch = 1;
        }
    }
    for fno in 0..nonneg(mris.nfaces) {
        for n in 0..VERTICES_PER_FACE {
            let mut sc = Scanf::new(&mut fp);
            let e = &mut g.errno;
            let got = sc.d(e, true);
            sc.text(e, b" ");
            sc.finish(e);
            match got {
                Some(v) => mris.face_mut(fno).v[n] = v,
                None => {
                    g.notices.add(
                        &FS_152,
                        format!("{fname}: a face vertex that cannot be read is refused (FreeSurfer takes vertex 0)"),
                    );
                    if !g.notices.reproduce() {
                        unreadable(g, io, fp, WHO, fname, &format!("face {fno}"));
                        return Ok(None);
                    }
                }
            }
            let v = mris.face(fno).v[n];
            if v < 0 || v >= mris.nvertices {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_BADPARM,
                    &format!(
                        "{}: face {fno} vertex {n}: {v} -- out of range!!! Should be in [0 {}]",
                        g.progname,
                        mris.nvertices - 1
                    ),
                )?;
            }
            if !count_face(g, &mut mris, v as usize, fname) {
                fp.close();
                log::error_printf(
                    g,
                    io.err,
                    ERROR_BADFILE,
                    &format!("{WHO}({fname}): vertex {v} is in more than 255 faces"),
                );
                return Ok(None);
            }
        }
        let mut sc = Scanf::new(&mut fp);
        let e = &mut g.errno;
        let r = sc.d(e, true).map(f64::from);
        sc.text(e, b"\n");
        sc.finish(e);
        let place = format!("face {fno}");
        let Some(r) = rip.take(g, [r], fname, &place) else {
            unreadable(g, io, fp, WHO, fname, &place);
            return Ok(None);
        };
        mris.face_mut(fno).ripflag = r[0] as i32 as i8;
    }
    mris.patch = patch;
    if patch != 0 {
        mris.status = MrisStatus::PLANE;
    }
    fp.close();
    Ok(Some(mris))
}

// ---- the movie.byu surface --------------------------------------------------------------------

/// The movie.byu header: `fgetl(line, 100)`, `%*d %d %d %d` (the vertex, face and edge counts),
/// the edges per face 3, then the second line (`fgetl(line, 200)`, the face count again, unused).
/// `Ok(Err(code))` after an error (its message printed, the stream closed).
#[allow(clippy::type_complexity)]
fn geo_header(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Result<(CFile, i32, i32), i32>, Fatal> {
    const WHO: &str = "mrisReadGeoFile";
    let mut fp = match open(dir, fname, "r") {
        Ok(f) => Some(f),
        Err(e) => {
            not_opened(
                g,
                io,
                e,
                &format!("mrisReadGeoFile: could not open file {fname}"),
            );
            return Ok(Err(ERROR_NOFILE));
        }
    };
    let Some(cp) = first_line(g, io, &mut fp, 100, WHO, fname)? else {
        return Ok(Err(ERROR_BADFILE));
    };
    let mut fp = fp.expect("an open stream");
    let (r, nvertices, nfaces, nedges) = {
        let mut b = Bytes::new(&cp);
        let mut sc = Scanf::new(&mut b);
        let e = &mut g.errno;
        sc.d(e, false);
        sc.text(e, b" ");
        let nv = sc.d(e, true);
        sc.text(e, b" ");
        let nf = sc.d(e, true);
        sc.text(e, b" ");
        let ne = sc.d(e, true);
        sc.text(e, b"\n");
        (sc.finish(e), nv, nf, ne)
    };
    let (Some(nvertices), Some(nfaces), Some(nedges)) = (nvertices, nfaces, nedges) else {
        debug_assert_ne!(r, 3);
        fp.close();
        let mut m = format!("mrisReadGeoFile({fname}): could not scan dimensions from '").into_bytes();
        m.extend_from_slice(&cp);
        m.push(b'\'');
        error_printf_bytes(g, io.err, ERROR_BADFILE, &m);
        return Ok(Err(ERROR_BADFILE));
    };
    if nfaces == 0 || (nfaces == -1 && nedges == i32::MIN) {
        g.notices.add(
            &FS_153,
            format!("{fname}: a movie.byu file of {nedges} edges for {nfaces} faces is refused (FreeSurfer's division ends the program)"),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(136));
        }
        fp.close();
        log::error_printf(
            g,
            io.err,
            ERROR_BADFILE,
            &format!(
                "mrisReadGeoFile({fname}): unsupported vertices/face: {nedges} edges for {nfaces} faces."
            ),
        );
        return Ok(Err(ERROR_BADFILE));
    }
    let vertices_per_face = nedges / nfaces;
    if vertices_per_face != VERTICES_PER_FACE as i32 {
        fp.close();
        log::error_printf(
            g,
            io.err,
            ERROR_BADFILE,
            &format!("mrisReadGeoFile({fname}): unsupported vertices/face {vertices_per_face}."),
        );
        return Ok(Err(ERROR_BADFILE));
    }
    let _ = fgetl_line(&mut fp, 200);
    Ok(Ok((fp, nvertices, nfaces)))
}

/// The coordinates of `mris`'s vertices from a movie.byu file after its header (`%e %e %e`, a
/// `\n` skipped after every odd vertex), `MRISsetXYZ` at the reader's `line`. `false` when the
/// correction refuses the file (its error printed, the stream closed).
#[allow(clippy::too_many_arguments)]
fn geo_coordinates(
    g: &mut Globals,
    io: &mut Io,
    fp: &mut Option<CFile>,
    mris: &mut Mris,
    who: &str,
    fname: &str,
    line: i32,
) -> Result<bool, Fatal> {
    let file = io_cpp();
    let mut xyz = Kept::<3>::new();
    for vno in 0..nonneg(mris.nvertices) {
        let f = fp.as_mut().expect("an open stream");
        let mut sc = Scanf::new(f);
        let e = &mut g.errno;
        let mut got = [None; 3];
        for (k, x) in got.iter_mut().enumerate() {
            if k > 0 {
                sc.text(e, b" ");
            }
            *x = sc.f(e, true).map(f64::from);
        }
        sc.finish(e);
        let place = format!("vertex {vno}");
        let Some(c) = xyz.take(g, got, fname, &place) else {
            unreadable(g, io, fp.take().expect("an open stream"), who, fname, &place);
            return Ok(false);
        };
        coords::set_xyz(
            g, io, mris, vno as i32, c[0] as f32, c[1] as f32, c[2] as f32, &file, line,
        )?;
        if vno % 2 == 1 {
            let f = fp.as_mut().expect("an open stream");
            let mut sc = Scanf::new(f);
            sc.text(&mut g.errno, b"\n");
            sc.finish(&mut g.errno);
        }
    }
    Ok(true)
}

/// `mrisReadGeoFile(fname)`: a new surface (`MRISalloc(nvertices, nfaces)`, type
/// `MRIS_GEO_TRIANGLE_FILE`) from a movie.byu file: the header (`<parts> <nvertices> <nfaces>
/// <nedges>`, a second line), the coordinates, then per face `a b -c` (numbered from 1, the last
/// negative), stored from 0 with the second and third swapped (the normal outwards). Returns
/// `None` after an error (`ERROR_NOFILE` or `ERROR_BADFILE`): `mrisReadGeoFile: could not open file
/// <file>`, `mrisReadGeoFile(<file>): could not scan dimensions from '<line>'`,
/// `mrisReadGeoFile(<file>): unsupported vertices/face <n>.` (the edges are not three times the
/// faces). The defects: FS-147, FS-150, FS-151, FS-152, FS-153, FS-154 (the corrections' errors
/// `mrisReadGeoFile(<file>): the file holds no line`, `... unsupported vertices/face: <e> edges for
/// <f> faces.`, `... could not read vertex <v>`, `... could not read face <f>`, `... face <f> vertex
/// <n>: <v> is not a vertex of the surface`, `... vertex <v> is in more than 255 faces`).
pub fn read_geo_file(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Mris>, Fatal> {
    const WHO: &str = "mrisReadGeoFile";
    let fname = c_str(fname);
    let (fp, nvertices, nfaces) = match geo_header(g, io, dir, fname)? {
        Ok(h) => h,
        Err(_) => return Ok(None),
    };
    let mut fp = Some(fp);
    let mut mris = alloc::alloc(g, io.out, io.err, sg, nvertices, nfaces)?;
    mris.r#type = MRIS_GEO_TRIANGLE_FILE;
    if !geo_coordinates(g, io, &mut fp, &mut mris, WHO, fname, 3437)? {
        return Ok(None);
    }
    let mut fp = fp.expect("an open stream");
    for fno in 0..nonneg(mris.nfaces) {
        for n in 0..VERTICES_PER_FACE {
            let mut sc = Scanf::new(&mut fp);
            let e = &mut g.errno;
            if n == VERTICES_PER_FACE - 1 {
                sc.text(e, b"-");
            }
            let got = sc.d(e, true);
            sc.text(e, if n == VERTICES_PER_FACE - 1 { b"\n" } else { b" " });
            sc.finish(e);
            match got {
                Some(v) => mris.face_mut(fno).v[n] = v,
                None => {
                    g.notices.add(
                        &FS_152,
                        format!("{fname}: a face vertex that cannot be read is refused (FreeSurfer keeps the one before)"),
                    );
                    if !g.notices.reproduce() {
                        unreadable(g, io, fp, WHO, fname, &format!("face {fno}"));
                        return Ok(None);
                    }
                }
            }
            let v = mris.face(fno).v[n].wrapping_sub(1);
            mris.face_mut(fno).v[n] = v;
            if v < 0 || v >= mris.nvertices {
                g.notices.add(
                    &FS_154,
                    format!("{fname}: a face of a vertex the surface does not have is refused (FreeSurfer counts it in memory outside the surface)"),
                );
                if !g.notices.reproduce() {
                    fp.close();
                    log::error_printf(
                        g,
                        io.err,
                        ERROR_BADFILE,
                        &format!(
                            "{WHO}({fname}): face {fno} vertex {n}: {v} is not a vertex of the surface"
                        ),
                    );
                    return Ok(None);
                }
                continue;
            }
            if !count_face(g, &mut mris, v as usize, fname) {
                fp.close();
                log::error_printf(
                    g,
                    io.err,
                    ERROR_BADFILE,
                    &format!("{WHO}({fname}): vertex {v} is in more than 255 faces"),
                );
                return Ok(None);
            }
        }
        mris.face_mut(fno).v.swap(1, 2);
    }
    fp.close();
    Ok(Some(mris))
}

/// `mrisReadGeoFilePositions(mris, fname)`: the coordinates of `mris`'s vertices from a movie.byu
/// file (its header as [`read_geo_file`]'s, the messages naming `mrisReadGeoFile`); FreeSurfer
/// does not compare the file's counts with the surface's (FS-155: refused by default, `<who>(<file>):
/// the file holds <n> vertices and <m> faces, the surface <n'> and <m'>`). Returns `NO_ERROR`,
/// `ERROR_NOFILE` or `ERROR_BADFILE`. The defects: FS-150, FS-151, FS-152, FS-153, FS-155.
pub fn read_geo_file_positions(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
) -> Result<i32, Fatal> {
    const WHO: &str = "mrisReadGeoFilePositions";
    let fname = c_str(fname);
    let (fp, nvertices, nfaces) = match geo_header(g, io, dir, fname)? {
        Ok(h) => h,
        Err(code) => return Ok(code),
    };
    if nvertices != mris.nvertices || nfaces != mris.nfaces {
        g.notices.add(
            &FS_155,
            format!("{fname}: movie.byu positions of another surface are refused (FreeSurfer reads them)"),
        );
        if !g.notices.reproduce() {
            fp.close();
            log::error_printf(
                g,
                io.err,
                ERROR_BADFILE,
                &format!(
                    "{WHO}({fname}): the file holds {nvertices} vertices and {nfaces} faces, the surface {} and {}",
                    mris.nvertices, mris.nfaces
                ),
            );
            return Ok(ERROR_BADFILE);
        }
    }
    let mut fp = Some(fp);
    if !geo_coordinates(g, io, &mut fp, mris, WHO, fname, 3507)? {
        return Ok(ERROR_BADFILE);
    }
    fp.expect("an open stream").close();
    Ok(NO_ERROR)
}

// ---- the binary triangle file's positions -----------------------------------------------------

/// The triangle file's start: the 3-byte magic number (not checked), the creator's line (`fgets`
/// of 199 bytes), `fscanf(fp, "\n")`, the vertex and face counts (`freadInt`).
fn triangle_counts(g: &mut Globals, fp: &mut CFile) -> (i32, i32) {
    let _ = fread3(fp);
    let _ = fp.fgets(200);
    let mut sc = Scanf::new(fp);
    sc.text(&mut g.errno, b"\n");
    sc.finish(&mut g.errno);
    let nvertices = fread_int(g, fp);
    let nfaces = fread_int(g, fp);
    (nvertices, nfaces)
}

fn show_counts(g: &Globals, io: &mut Io, fname: &str, nvertices: i32, nfaces: i32) {
    if verbose_show(g) {
        let _ = writeln!(
            io.out,
            "surface {fname}: {nvertices} vertices and {nfaces} faces."
        );
    }
}


/// `mrisReadTriangleFileVertexPositionsOnly(fname)`: the vertex positions of a binary triangle
/// file (`MRISreadVerticesOnly`'s small surface): the counts, then three big-endian floats per
/// vertex; a coordinate beyond 10000 in size, or not finite, ends the program (`<program>: vertex
/// <v> x coordinate <value>!`, `%f`, `ERROR_BADFILE`). Returns `None` after an error: the file
/// cannot be opened (`mrisReadTriangleFile(<file>): could not open file`, `ERROR_NOFILE`), a file
/// of no vertices (`MRISreadVerticesOnly: could not allocate surface`, `ERROR_NOMEMORY`; FreeSurfer's
/// message for a vertex count of 0). FS-156: a negative count (whose allocation fails, `errno`
/// `ENOMEM`) gives a surface of that count and no vertices (refused by default with the allocation
/// message).
pub fn read_triangle_file_vertex_positions_only(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<SmallSurface>, Fatal> {
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "rb") {
        Ok(f) => f,
        Err(e) => {
            not_opened(
                g,
                io,
                e,
                &format!("mrisReadTriangleFile({fname}): could not open file"),
            );
            return Ok(None);
        }
    };
    let (nvertices, nfaces) = triangle_counts(g, &mut fp);
    show_counts(g, io, fname, nvertices, nfaces);
    let no_room = |g: &mut Globals, io: &mut Io| {
        log::error_printf(
            g,
            io.err,
            ERROR_NOMEMORY,
            "MRISreadVerticesOnly: could not allocate surface",
        );
    };
    if nvertices == 0 {
        no_room(g, io);
        return Ok(None);
    }
    if nvertices < 0 {
        // the vertices' calloc fails (ENOMEM)
        g.errno = ENOMEM;
        g.notices.add(
            &FS_156,
            format!("{fname}: a triangle file of {nvertices} vertices is refused (FreeSurfer returns a surface without its vertices)"),
        );
        if !g.notices.reproduce() {
            no_room(g, io);
            return Ok(None);
        }
        fp.close();
        return Ok(Some(SmallSurface {
            nvertices,
            vertices: Vec::new(),
        }));
    }
    let mut vertices = Vec::new();
    for vno in 0..nvertices {
        let x = fread_float(g, io.err, &mut fp);
        let y = fread_float(g, io.err, &mut fp);
        let z = fread_float(g, io.err, &mut fp);
        for (c, name) in [(x, "x"), (y, "y"), (z, "z")] {
            if c.abs() > 10000.0 || !c.is_finite() {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_BADFILE,
                    &format!(
                        "{}: vertex {vno} {name} coordinate {}!",
                        g.progname,
                        fmt_f(f64::from(c), 0, 6)
                    ),
                )?;
            }
        }
        vertices.push(SmallVertex { x, y, z });
    }
    fp.close();
    Ok(Some(SmallSurface {
        nvertices,
        vertices,
    }))
}

/// `mrisReadTriangleFilePositions(mris, fname)`: the coordinates of `mris`'s vertices from a
/// binary triangle file whose counts are the surface's (else `mrisReadTriangleFile opened <file>
/// okay but surface doesn't match <surface's file>.  nvertices:<n> != mris->nvertices:<n'> ||
/// nfaces:<m> != mris->nfaces:<m'>` with its newline, `ERROR_BADPARM`); the distances freed
/// (`MRISfreeDistsButNotOrig`) first. Returns `NO_ERROR`, `ERROR_NOFILE` (`mrisReadTriangleFile(<file>):
/// could not open file`) or `ERROR_BADPARM`.
pub fn read_triangle_file_positions(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fname: &str,
) -> Result<i32, Fatal> {
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "rb") {
        Ok(f) => f,
        Err(e) => {
            not_opened(
                g,
                io,
                e,
                &format!("mrisReadTriangleFile({fname}): could not open file"),
            );
            return Ok(ERROR_NOFILE);
        }
    };
    let (nvertices, nfaces) = triangle_counts(g, &mut fp);
    if nvertices != mris.nvertices || nfaces != mris.nfaces {
        log::error_printf(
            g,
            io.err,
            ERROR_BADPARM,
            &format!(
                "mrisReadTriangleFile opened {fname} okay but surface doesn't match {}.  nvertices:{nvertices} != mris->nvertices:{} || nfaces:{nfaces} != mris->nfaces:{}\n",
                c_str(&mris.fname),
                mris.nvertices,
                mris.nfaces
            ),
        );
        return Ok(ERROR_BADPARM);
    }
    show_counts(g, io, fname, nvertices, nfaces);
    dist::free_dists_but_not_orig(mris);
    let file = io_cpp();
    for vno in 0..nvertices {
        let x = fread_float(g, io.err, &mut fp);
        let y = fread_float(g, io.err, &mut fp);
        let z = fread_float(g, io.err, &mut fp);
        coords::set_xyz(g, io, mris, vno, x, y, z, &file, 5481)?;
    }
    fp.close();
    Ok(NO_ERROR)
}

// ---- the old writer's quadrangles -------------------------------------------------------------

/// `quadCombine(quad, vA, vB)`: the quadrangle of two triangles that share an edge in opposite
/// directions (`vA`'s edge `a b` is `vB`'s `b a`): `vA`'s vertices in order with `vB`'s third
/// vertex inserted after the shared edge's first vertex. `quad` gets the vertices as they are
/// found (also when the triangles do not combine: then some of its places are written, others
/// not); `true` when exactly four were found.
pub fn quad_combine(quad: &mut [i32; 4], va: &[i32; 3], vb: &[i32; 3]) -> bool {
    let mut qi = 0;
    for vai in 0..3 {
        let edge0 = va[vai];
        let edge1 = va[if vai == 2 { 0 } else { vai + 1 }];
        if qi == 4 {
            return false;
        }
        quad[qi] = edge0;
        qi += 1;
        for vbi in 0..3 {
            if edge0 != vb[vbi] {
                continue;
            }
            let prev = vb[if vbi > 0 { vbi - 1 } else { 2 }];
            if prev != edge1 {
                break;
            }
            if qi == 4 {
                return false;
            }
            quad[qi] = vb[if vbi < 2 { vbi + 1 } else { 0 }];
            qi += 1;
        }
    }
    qi == 4
}
