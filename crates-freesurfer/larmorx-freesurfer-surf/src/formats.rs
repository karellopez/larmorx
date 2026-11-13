// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrisurf_io.cpp`'s other surface formats and helpers of queue item 3.1-48 part a
//! (`fs.surfio`): the VTK reader (`MRISreadVTK` 2861), the movie.byu and icosahedron writers
//! (`MRISwriteGeo` 3158, `MRISwriteICO` 3225), the surface-to-Talairach matrix of a transform
//! (`getSRASToTalSRAS` 4574) and the MATLAB plots of a face and a vertex (`MatlabPlotFace` 6077,
//! `MatlabPlotVertex` 6124).
//!
//! The defects: FS-141 (an unripped face of a ripped vertex in the two writers; the movie.byu
//! writer's 300,000-vertex buffer), FS-142 (the icosahedron writer's vertex numbers), FS-143 and
//! FS-144 (VTK coordinates it cannot read), FS-145 (a VTK file that ends after `POINT_DATA` or in
//! its fields), FS-146 (field values it does not hold), FS-147 (a vertex of more than 255 faces).
//!
//! Not here: `MRISwritePatchAscii` (it asks fs.io's format identification, `mri_identify`, track
//! A's 3.1-27, for the name's type) and `MatlabPlotVertexNbhd` (fs.surf's hop lists, 3.4).

use std::path::Path;

use larmorx_core::defects::{FS_141, FS_142, FS_143, FS_144, FS_145, FS_146, FS_147};
use larmorx_freesurfer_core::cscan::Bytes;
use larmorx_freesurfer_core::cstd::{fmt_f, format_double};
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NO_MEMORY, ERROR_NOFILE, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::matrix_identity;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add64, div64, mul64};
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_mri::lta::LinearTransform;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;

use crate::aaa::SurfGlobals;
use crate::alloc;
use crate::coords::{self, Io};
use crate::curvio::{error_printf_bytes, fgetl_line};
use crate::dist;
use crate::mp_slow;
use crate::mris::Mris;
use crate::scanf::Scanf;
use crate::sfile::{STRLEN, c_str, nonneg, open};
use crate::topology;

/// `MRIS_TRIANGULAR_SURFACE` (`MRIS_ICO_SURFACE`, `mrisurf.h`).
pub const MRIS_TRIANGULAR_SURFACE: i32 = 3;

/// `sizeof(FACE)` in the official build (what a failed allocation's message prints).
const SIZEOF_FACE: i32 = 80;

/// `MRISwriteGeo`'s buffer of vertex numbers (`int vnos[300000]`).
const GEO_VNOS: usize = 300_000;

fn io_cpp() -> String {
    format!("{}/utils/mrisurf_io.cpp", diag::BUILD_ROOT)
}

/// `ErrorReturn(NULL, (ERROR_NOFILE, message))`.
fn vtk_error(g: &mut Globals, io: &mut Io, message: &str) -> Ret<Mris> {
    log::error_printf(g, io.err, ERROR_NOFILE, message);
    Ret::Null
}

/// One `sscanf(cp, fmt, ...)` of a VTK header line as `<word> %d ...`: the word, then the
/// numbers; the call's return value and the values it assigned.
fn header(
    g: &mut Globals,
    cp: &[u8],
    word: &[u8],
    n: usize,
    tail: &[u8],
) -> (i32, Vec<Option<i32>>) {
    let mut b = Bytes::new(cp);
    let mut sc = Scanf::new(&mut b);
    let e = &mut g.errno;
    sc.text(e, word);
    let mut out = Vec::new();
    for k in 0..n {
        if k > 0 {
            sc.text(e, b" ");
        }
        out.push(sc.d(e, true));
    }
    sc.text(e, tail);
    (sc.finish(e), out)
}

/// `MRISreadVTK(mris, fname, curvmri)`: a VTK polydata file as `MRISwriteVTK` writes it, into a
/// new surface (`mris` `None`: `MRISalloc(nvertices, 0)`, type `MRIS_TRIANGULAR_SURFACE`,
/// coordinates and faces from the file) or into `mris` (its vertex and face counts and faces must
/// be the file's; its coordinates are kept), the field values of `POINT_DATA` into the vertices'
/// `curv` and `val` (and the volume `curvmri`'s voxel `vno`, given), a field named `curv...` or
/// `CURV...` setting `max_curv` and `min_curv`.
///
/// The file: lines (`fgetl`, comments and blank lines passed over) until three of `vtk output`,
/// `surface file`, `ASCII`, `DATASET POLYDATA` (either case) have been seen; `POINTS <n> float`;
/// the coordinates; `POLYGONS <n> <4n>`; per face `3 a b c`; then optionally `POINT_DATA <n>`,
/// `FIELD <name> <k>` and per field `<name> <c> <n> <type>` and its `n` values. The distances are
/// freed (`MRISfreeDistsButNotOrig`). Returns the surface (`Ret::New` or `Ret::Given`) or
/// `Ret::Null` after an error (`ERROR_NOFILE`): `MRISreadVTK: could not open file <file>`, `...
/// error reading file <file>` (fewer than three header lines), `... error parsing POINTS from file
/// <file>`, `... error reading file <file>, invalid nvertices=<n>` (a count not above 0, or a
/// `POINTS` line without its number), `... no POINTS field`, `... mismatched nvertices=<m>/<n>`,
/// `... error parsing POLYGONS in file <file>` (no line, or not one of `POLYGONS`), `... invalid
/// nfaces=<n>`, `... invalid POLYGON data` (the second number not four times the first), `...
/// mismatched nfaces=<m>/<n>`, `... facepoints != 3`, `MRISreadVTK: invalid vertex num in
/// <file>, vno = <v>`, `... mismatched faces=<a>/<b>`, `... invalid nvertices=<n> in POINT_DATA
/// field, expected <m> vertices`, `... invalid nvertices=<n> in <name> field, expected <m>
/// vertices` (a field line without its third number keeps the count before it), `MRISreadVTK:
/// error parsing <name> FIELD in file <file>` (a value that is not a number). A line other than
/// `POINT_DATA` after the faces, or other than `FIELD` after it, ends the file's reading.
#[allow(clippy::too_many_arguments)]
pub fn read_vtk(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    given: Option<&mut Mris>,
    fname: &str,
    mut curvmri: Option<&mut Mri>,
) -> Result<Ret<Mris>, Fatal> {
    let fname = c_str(fname);
    let mut fp = match open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            return Ok(vtk_error(g, io, &format!("MRISreadVTK: could not open file {fname}\n")));
        }
    };
    let mut checks = 0;
    let mut found = false;
    while let Some((line, at)) = fgetl_line(&mut fp, STRLEN) {
        let cp = &line[at..];
        for p in [
            &b"VTK OUTPUT"[..],
            b"vtk output",
            b"surface file",
            b"SURFACE FILE",
            b"ASCII",
            b"ascii",
            b"DATASET POLYDATA",
            b"dataset polydata",
        ] {
            if cp.starts_with(p) {
                checks += 1;
            }
        }
        if checks == 3 {
            found = true;
            break;
        }
    }
    if !found {
        fp.close();
        return Ok(vtk_error(g, io, &format!("MRISreadVTK: error reading file {fname}\n")));
    }
    let Some((line, at)) = fgetl_line(&mut fp, STRLEN) else {
        fp.close();
        return Ok(vtk_error(
            g,
            io,
            &format!("MRISreadVTK: error parsing POINTS from file {fname}\n"),
        ));
    };
    let cp = &line[at..];
    let mut nvertices = 0;
    let mut points = |g: &mut Globals, word: &[u8]| {
        let (r, v) = header(g, cp, word, 1, b" float\n");
        if let Some(n) = v[0] {
            nvertices = n;
        }
        r
    };
    let matched = points(g, b"POINTS ") != 0 || points(g, b"points ") != 0;
    if !matched {
        fp.close();
        return Ok(vtk_error(
            g,
            io,
            &format!("MRISreadVTK: error reading file {fname}, no POINTS field\n"),
        ));
    }
    if nvertices <= 0 {
        fp.close();
        return Ok(vtk_error(
            g,
            io,
            &format!("MRISreadVTK: error reading file {fname}, invalid nvertices={nvertices}\n"),
        ));
    }
    let new = given.is_none();
    let mut fresh = None;
    let mris: &mut Mris = match given {
        Some(m) => {
            if m.nvertices != nvertices {
                fp.close();
                let msg = format!(
                    "MRISreadVTK: error reading file {fname}, mismatched nvertices={}/{nvertices}\n",
                    m.nvertices
                );
                return Ok(vtk_error(g, io, &msg));
            }
            m
        }
        None => {
            let mut m = alloc::alloc(g, io.out, io.err, sg, nvertices, 0)?;
            m.r#type = MRIS_TRIANGULAR_SURFACE;
            fresh.insert(m)
        }
    };
    dist::free_dists_but_not_orig(mris);
    let file = io_cpp();
    let mut prev: [Option<f32>; 3] = [None; 3];
    for vno in 0..nonneg(mris.nvertices) {
        let mut sc = Scanf::new(&mut fp);
        let mut got = [None; 3];
        for (k, x) in got.iter_mut().enumerate() {
            if k > 0 {
                sc.text(&mut g.errno, b" ");
            }
            *x = sc.f(&mut g.errno, true);
        }
        let r = sc.finish(&mut g.errno);
        if r != 3 {
            g.notices.add(
                &FS_143,
                format!("{fname}: a VTK vertex whose coordinates cannot be read is refused (FreeSurfer keeps the previous ones)"),
            );
            if !g.notices.reproduce() {
                fp.close();
                let msg = format!(
                    "MRISreadVTK: error reading the coordinates of vertex {vno} in file {fname}\n"
                );
                return Ok(vtk_error(g, io, &msg));
            }
        }
        let mut c = [0.0f32; 3];
        for k in 0..3 {
            if got[k].is_some() {
                prev[k] = got[k];
            }
            c[k] = match prev[k] {
                Some(x) => x,
                None => {
                    g.notices.add(
                        &FS_144,
                        format!("{fname}: the first VTK vertex's coordinates that cannot be read are memory contents in FreeSurfer (0 here)"),
                    );
                    0.0
                }
            };
        }
        if new {
            coords::set_xyz(g, io, mris, vno as i32, c[0], c[1], c[2], &file, 2964)?;
        }
    }
    let Some((line, at)) = fgetl_line(&mut fp, STRLEN) else {
        fp.close();
        return Ok(vtk_error(
            g,
            io,
            &format!("MRISreadVTK: error parsing POLYGONS in file {fname}"),
        ));
    };
    let cp = &line[at..];
    let (mut nfaces, mut somenum) = (0, 0);
    let mut polygons = |g: &mut Globals, word: &[u8]| {
        let (r, v) = header(g, cp, word, 2, b"\n");
        if let Some(n) = v[0] {
            nfaces = n;
        }
        if let Some(n) = v[1] {
            somenum = n;
        }
        r
    };
    let matched = polygons(g, b"POLYGONS ") != 0 || polygons(g, b"polygons ") != 0;
    if !matched {
        fp.close();
        return Ok(vtk_error(
            g,
            io,
            &format!("MRISreadVTK: error parsing POLYGONS in file {fname}"),
        ));
    }
    if nfaces <= 0 {
        fp.close();
        return Ok(vtk_error(
            g,
            io,
            &format!("MRISreadVTK: error reading file {fname}, invalid nfaces={nfaces}\n"),
        ));
    }
    if somenum != nfaces.wrapping_mul(4) {
        fp.close();
        return Ok(vtk_error(
            g,
            io,
            &format!("MRISreadVTK: error reading file {fname}, invalid POLYGON data\n"),
        ));
    }
    if new {
        if !alloc::allocate_faces(g, io.out, io.err, mris, nfaces)? {
            fp.close();
            log::error_exit(
                g,
                io.err,
                ERROR_NO_MEMORY,
                &format!("MRISreadVTK({nfaces}, {SIZEOF_FACE}): could not allocate faces"),
            )?;
            unreachable!("ErrorExit returns only through its Err");
        }
    } else if mris.nfaces != nfaces {
        fp.close();
        let msg = format!(
            "MRISreadVTK: error reading file {fname}, mismatched nfaces={}/{nfaces}\n",
            mris.nfaces
        );
        return Ok(vtk_error(g, io, &msg));
    }
    let mut facepoints = 0;
    for fno in 0..nonneg(mris.nfaces) {
        {
            let mut sc = Scanf::new(&mut fp);
            if let Some(p) = sc.d(&mut g.errno, true) {
                facepoints = p;
            }
            sc.text(&mut g.errno, b" ");
            sc.finish(&mut g.errno);
        }
        if facepoints != 3 {
            fp.close();
            return Ok(vtk_error(
                g,
                io,
                &format!("MRISreadVTK: error reading file {fname}, facepoints != 3\n"),
            ));
        }
        for n in 0..3 {
            let mut sc = Scanf::new(&mut fp);
            let vno = sc.d(&mut g.errno, true).unwrap_or(-1);
            sc.finish(&mut g.errno);
            if vno < 0 || vno >= nvertices {
                fp.close();
                return Ok(vtk_error(
                    g,
                    io,
                    &format!("MRISreadVTK: invalid vertex num in {fname}, vno = {vno}\n"),
                ));
            }
            if new {
                mris.face_mut(fno).v[n] = vno;
                let t = mris.topology_mut(vno as usize);
                if t.num == u8::MAX {
                    g.notices.add(
                        &FS_147,
                        format!("{fname}: a vertex of more than 255 faces is refused (FreeSurfer's count wraps around)"),
                    );
                    if !g.notices.reproduce() {
                        fp.close();
                        let msg = format!(
                            "MRISreadVTK: error reading file {fname}, vertex {vno} is in more than 255 faces\n"
                        );
                        return Ok(vtk_error(g, io, &msg));
                    }
                }
                let t = mris.topology_mut(vno as usize);
                t.num = t.num.wrapping_add(1);
            } else if mris.face(fno).v[n] != vno {
                fp.close();
                let msg = format!(
                    "MRISreadVTK: error reading file {fname}, mismatched faces={}/{vno}\n",
                    mris.face(fno).v[n]
                );
                return Ok(vtk_error(g, io, &msg));
            }
        }
    }
    if let Some((line, at)) = fgetl_line(&mut fp, STRLEN) {
        let cp = line[at..].to_vec();
        let mut nvd = 0;
        let mut point_data = |g: &mut Globals, word: &[u8]| {
            let (r, v) = header(g, &cp, word, 1, b"\n");
            if let Some(n) = v[0] {
                nvd = n;
            }
            r
        };
        if point_data(g, b"POINT_DATA ") != 0 || point_data(g, b"point_data ") != 0 {
            if nvertices != nvd {
                fp.close();
                let msg = format!(
                    "MRISreadVTK: error reading file {fname}, invalid nvertices={nvd} in POINT_DATA field, expected {nvertices} vertices\n"
                );
                return Ok(vtk_error(g, io, &msg));
            }
            let Some((line, at)) = ended(g, io, &mut fp, fname, "after its POINT_DATA line")?
            else {
                return Ok(Ret::Null);
            };
            let cp = &line[at..];
            let mut nfields = 0;
            let mut field = |g: &mut Globals, word: &[u8]| {
                let mut b = Bytes::new(cp);
                let mut sc = Scanf::new(&mut b);
                let e = &mut g.errno;
                sc.text(e, word);
                sc.s(e, usize::MAX, false);
                sc.text(e, b" ");
                if let Some(n) = sc.d(e, true) {
                    nfields = n;
                }
                sc.text(e, b"\n");
                sc.finish(e)
            };
            if field(g, b"FIELD ") != 0 || field(g, b"field ") != 0 {
                for _ in 0..nfields.max(0) {
                    let Some((line, at)) = ended(g, io, &mut fp, fname, "inside its FIELD lines")?
                    else {
                        return Ok(Ret::Null);
                    };
                    let cp = &line[at..];
                    let (r, name) = {
                        let mut b = Bytes::new(cp);
                        let mut sc = Scanf::new(&mut b);
                        let e = &mut g.errno;
                        let name = sc.s(e, usize::MAX, true);
                        sc.text(e, b" ");
                        let _field_num = sc.d(e, true);
                        sc.text(e, b" ");
                        if let Some(n) = sc.d(e, true) {
                            nvd = n;
                        }
                        sc.text(e, b" ");
                        sc.s(e, usize::MAX, false);
                        sc.text(e, b"\n");
                        (sc.finish(e), name.unwrap_or_default())
                    };
                    if r == 0 {
                        fp.close();
                        return Ok(vtk_error(
                            g,
                            io,
                            &format!("MRISreadVTK: error parsing FIELDs in file {fname}"),
                        ));
                    }
                    if nvertices != nvd {
                        fp.close();
                        let mut m = format!(
                            "MRISreadVTK: error reading file {fname}, invalid nvertices={nvd} in "
                        )
                        .into_bytes();
                        m.extend_from_slice(&name);
                        m.extend_from_slice(
                            format!(" field, expected {nvertices} vertices\n").as_bytes(),
                        );
                        error_printf_bytes(g, io.err, ERROR_NOFILE, &m);
                        return Ok(Ret::Null);
                    }
                    let is_curv = name.starts_with(b"curv") || name.starts_with(b"CURV");
                    let (mut curvmin, mut curvmax) = (10000.0f32, -10000.0f32);
                    for vno in 0..nonneg(mris.nvertices) {
                        let mut sc = Scanf::new(&mut fp);
                        let got = sc.f(&mut g.errno, true);
                        let r = sc.finish(&mut g.errno);
                        let parse_error = |g: &mut Globals, io: &mut Io| {
                            let mut m = b"MRISreadVTK: error parsing ".to_vec();
                            m.extend_from_slice(&name);
                            m.extend_from_slice(format!(" FIELD in file {fname}").as_bytes());
                            error_printf_bytes(g, io.err, ERROR_NOFILE, &m);
                        };
                        if r == 0 {
                            fp.close();
                            parse_error(g, io);
                            return Ok(Ret::Null);
                        }
                        if r < 0 {
                            g.notices.add(
                                &FS_146,
                                format!("{fname}: a VTK field the file ends inside is refused (FreeSurfer takes 0 for the missing values)"),
                            );
                            if !g.notices.reproduce() {
                                fp.close();
                                parse_error(g, io);
                                return Ok(Ret::Null);
                            }
                        }
                        let f = got.unwrap_or(0.0);
                        let v = mris.vertex_mut(vno);
                        v.curv = f;
                        v.val = f;
                        if let Some(m) = curvmri.as_deref_mut() {
                            mri_set_vox_val(g, io.err, m, vno as i32, 0, 0, 0, f, 0)?;
                        }
                        if is_curv {
                            if vno == 0 {
                                curvmin = f;
                                curvmax = f;
                            }
                            if f > curvmax {
                                curvmax = f;
                            }
                            if f < curvmin {
                                curvmin = f;
                            }
                        }
                    }
                    if is_curv {
                        mris.max_curv = curvmax;
                        mris.min_curv = curvmin;
                    }
                }
            }
        }
    }
    fp.close();
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// The next line of a VTK file's data part; FS-145 (rule B) when the file has none: FreeSurfer
/// reads through the missing line and ends with a segmentation fault (signal 11, as `reproduce`
/// ends); larmorx's error `MRISreadVTK: error reading file <file>, it ends <where>` (`None`).
fn ended(
    g: &mut Globals,
    io: &mut Io,
    fp: &mut CFile,
    fname: &str,
    place: &str,
) -> Result<Option<(Vec<u8>, usize)>, Fatal> {
    if let Some(l) = fgetl_line(fp, STRLEN) {
        return Ok(Some(l));
    }
    g.notices.add(
        &FS_145,
        format!("{fname}: a VTK file that ends {place} is refused (FreeSurfer crashes)"),
    );
    if g.notices.reproduce() {
        return Err(Fatal::Exit(139));
    }
    log::error_printf(
        g,
        io.err,
        ERROR_NOFILE,
        &format!("MRISreadVTK: error reading file {fname}, it ends {place}\n"),
    );
    Ok(None)
}

// ---- the movie.byu and icosahedron writers ---------------------------------------------------

/// FS-141: the first unripped face with a ripped vertex, if any.
fn face_of_ripped_vertex(mris: &Mris) -> Option<(usize, i32)> {
    for fno in 0..nonneg(mris.nfaces) {
        let f = mris.face(fno);
        if f.ripflag != 0 {
            continue;
        }
        for &v in &f.v {
            if mris.vertex(nonneg(v)).ripflag != 0 {
                return Some((fno, v));
            }
        }
    }
    None
}

/// FS-141 (rule B): larmorx refuses to write an unripped face that has a ripped vertex
/// (`<func>(<file>): face <fno> has the ripped vertex <vno>`, `ERROR_BADPARM`); `true` when it
/// does.
fn refuse_ripped(g: &mut Globals, io: &mut Io, mris: &Mris, func: &str, fname: &str) -> bool {
    let Some((fno, vno)) = face_of_ripped_vertex(mris) else {
        return false;
    };
    g.notices.add(
        &FS_141,
        format!("{func}: an unripped face of a ripped vertex is refused (FreeSurfer writes a number of no vertex it wrote)"),
    );
    if g.notices.reproduce() {
        return false;
    }
    log::error_printf(
        g,
        io.err,
        ERROR_BADPARM,
        &format!("{func}({fname}): face {fno} has the ripped vertex {vno}"),
    );
    true
}

fn e12_5(x: f32) -> String {
    format_double("%12.5e", f64::from(x)).expect("a double format")
}

/// `MRISwriteGeo(mris, fname)`: the movie.byu format: `    1 <nvertices> <nfaces> <3 nfaces>`,
/// `    1 <nfaces>` (the unripped ones), the unripped vertices' coordinates `%12.5e` two to a line,
/// then per unripped face its vertices' new numbers (1-based) in the order first, third, minus the
/// second. Returns `NO_ERROR`, `ERROR_NOFILE` (`MRISwriteGeo: could not open file <file>`) or, by
/// default, `ERROR_BADPARM` for an unripped face of a ripped vertex (FS-141: FreeSurfer writes a
/// number from memory contents for it, `reproduce` 1; and its 300,000-entry buffer of numbers,
/// which a larger surface overruns: larmorx's holds every vertex in both modes).
pub fn write_geo(g: &mut Globals, io: &mut Io, dir: Option<&Path>, mris: &Mris, fname: &str) -> i32 {
    const F: &str = "MRISwriteGeo";
    let fname = c_str(fname);
    let nfaces = topology::valid_faces(mris);
    let nvertices = topology::valid_vertices(mris);
    if refuse_ripped(g, io, mris, F, fname) {
        return ERROR_BADPARM;
    }
    if nonneg(mris.nvertices) > GEO_VNOS {
        g.notices.add(
            &FS_141,
            format!("{F}: a surface of {} vertices is written whole (FreeSurfer's buffer holds 300,000)", mris.nvertices),
        );
    }
    let mut fp = match open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(
                g,
                io.err,
                ERROR_NOFILE,
                &format!("{F}: could not open file {fname}"),
            );
            return ERROR_NOFILE;
        }
    };
    let mut s = format!("    1 {nvertices} {nfaces} {}\n", 3 * nfaces);
    s += &format!("    1 {nfaces}\n");
    let mut vnos = vec![0i32; nonneg(mris.nvertices)];
    let mut actual = 0;
    for (vno, slot) in vnos.iter_mut().enumerate() {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        *slot = actual;
        actual += 1;
    }
    let mut toggle = false;
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        s += &format!("{} {} {}", e12_5(v.x), e12_5(v.y), e12_5(v.z));
        s += if toggle { "\n" } else { " " };
        toggle = !toggle;
    }
    if toggle {
        s += "\n";
    }
    for fno in 0..nonneg(mris.nfaces) {
        let f = mris.face(fno);
        if f.ripflag != 0 {
            continue;
        }
        let n = |k: usize| vnos[nonneg(f.v[k])] + 1;
        s += &format!("{} {} -{}\n", n(0), n(2), n(1));
    }
    fp.put_bytes(s.as_bytes());
    fp.close();
    NO_ERROR
}

/// `MRISwriteICO(mris, fname)`: the icosahedron format: `%8d` the unripped vertices' count, per
/// unripped vertex `%8d %8.4f %8.4f %8.4f` (its number from 1 and coordinates), `%8d` the
/// unripped faces' count, per unripped face `%8d %8d %8d %8d` (its number from 1, then its first,
/// third and second vertices). FS-142 (rule A): FreeSurfer writes the faces' vertices by their
/// numbers on the surface plus one, which name other vertices once a vertex before them is
/// ripped; larmorx writes the numbers it gave them (`reproduce`: FreeSurfer's). FS-141 (rule B):
/// an unripped face of a ripped vertex is refused by default. Returns `NO_ERROR`, `ERROR_NOFILE`
/// (`MRISwriteICO: could not open file <file>`) or `ERROR_BADPARM`.
pub fn write_ico(g: &mut Globals, io: &mut Io, dir: Option<&Path>, mris: &Mris, fname: &str) -> i32 {
    const F: &str = "MRISwriteICO";
    let fname = c_str(fname);
    let nfaces = topology::valid_faces(mris);
    let nvertices = topology::valid_vertices(mris);
    if refuse_ripped(g, io, mris, F, fname) {
        return ERROR_BADPARM;
    }
    let mut fp = match open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(
                g,
                io.err,
                ERROR_NOFILE,
                &format!("{F}: could not open file {fname}"),
            );
            return ERROR_NOFILE;
        }
    };
    let mut s = format!("{nvertices:8}\n");
    let mut number = vec![0i32; nonneg(mris.nvertices)];
    let mut actual = 1;
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        number[vno] = actual;
        let f = |x: f32| fmt_f(f64::from(x), 8, 4);
        s += &format!("{actual:8} {} {} {}\n", f(v.x), f(v.y), f(v.z));
        actual += 1;
    }
    s += &format!("{nfaces:8}\n");
    let mut afno = 1;
    let mut renumbered = false;
    for fno in 0..nonneg(mris.nfaces) {
        let f = mris.face(fno);
        if f.ripflag != 0 {
            continue;
        }
        let theirs = |k: usize| f.v[k] + 1;
        let ours = |k: usize| {
            let n = number[nonneg(f.v[k])];
            if n == 0 { f.v[k] + 1 } else { n }
        };
        if (0..3).any(|k| theirs(k) != ours(k)) {
            renumbered = true;
        }
        let n: &dyn Fn(usize) -> i32 = if g.notices.reproduce() { &theirs } else { &ours };
        s += &format!("{afno:8} {:8} {:8} {:8}\n", n(0), n(2), n(1));
        afno += 1;
    }
    if renumbered {
        g.notices.add(
            &FS_142,
            format!("{F}: the faces name their vertices by the numbers written (FreeSurfer's name other vertices after a ripped one)"),
        );
    }
    fp.put_bytes(s.as_bytes());
    fp.close();
    NO_ERROR
}

// ---- the surface-to-Talairach matrix ---------------------------------------------------------

/// `getSRASToTalSRAS(lt)`: `sRASFromRAS * (m_L * RASFromSRAS)`, `RASFromSRAS` the identity with
/// the source geometry's centre (`c_r`, `c_a`, `c_s`) as its translation and `sRASFromRAS` the
/// identity with the destination's centre negated, in `float` as FreeSurfer's matrices compute
/// (fs.core's `MatrixMultiply`). `None` where a matrix cannot be made.
pub fn get_sras_to_tal_sras(
    g: &mut Globals,
    io: &mut Io,
    lt: &LinearTransform,
) -> Result<Option<Matrix>, Fatal> {
    let (out, err) = (&mut *io.out, &mut *io.err);
    let mut ras_from_sras = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?.expect("a 4 x 4 matrix");
    matrix_identity(g, out, err, 4, Some(&mut ras_from_sras))?;
    ras_from_sras.set_relt(1, 4, lt.src.c_r);
    ras_from_sras.set_relt(2, 4, lt.src.c_a);
    ras_from_sras.set_relt(3, 4, lt.src.c_s);
    let mut sras_from_ras = matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?.expect("a 4 x 4 matrix");
    matrix_identity(g, out, err, 4, Some(&mut sras_from_ras))?;
    sras_from_ras.set_relt(1, 4, -lt.dst.c_r);
    sras_from_ras.set_relt(2, 4, -lt.dst.c_a);
    sras_from_ras.set_relt(3, 4, -lt.dst.c_s);
    let Some(tmp) = matrix_multiply(g, out, err, lt.m_l.as_ref(), Some(&ras_from_sras), None)?.into_new()
    else {
        return Ok(None);
    };
    Ok(matrix_multiply(g, out, err, Some(&sras_from_ras), Some(&tmp), None)?.into_new())
}

// ---- the MATLAB plots ------------------------------------------------------------------------

fn f6_4(x: f64) -> String {
    fmt_f(x, 6, 4)
}

/// `MatlabPlotFace(fp, surf, faceno, color, NormLen)`: `% Face <fno>` on standard output; into
/// `fp` a `plot3(...)` of its three edges in `color`, a text `F<fno>` at its centre (the mean of
/// its corners, in `double`) and, for a `NormLen` not 0, the centre and its normal
/// (`mrisNormalFace` at its first corner, unit length when `UnitizeNormalFace` is set) times
/// `NormLen` in black. Returns 0.
pub fn matlab_plot_face(
    sg: &SurfGlobals,
    io: &mut Io,
    fp: &mut CFile,
    surf: &Mris,
    faceno: i32,
    color: u8,
    norm_len: f64,
) -> i32 {
    let face = surf.face(nonneg(faceno));
    let mut s = String::from("plot3(");
    let (mut cx, mut cy, mut cz) = (0.0f64, 0.0f64, 0.0f64);
    let _ = writeln!(io.out, "% Face {faceno}");
    let mut b = Vec::new();
    for n in 0..3 {
        let m = if n + 1 > 2 { 0 } else { n + 1 };
        let v1 = surf.vertex(nonneg(face.v[n]));
        let v2 = surf.vertex(nonneg(face.v[m]));
        s += &format!(
            "[{},{}],[{},{}],[{},{}],'",
            f6_4(f64::from(v1.x)),
            f6_4(f64::from(v2.x)),
            f6_4(f64::from(v1.y)),
            f6_4(f64::from(v2.y)),
            f6_4(f64::from(v1.z)),
            f6_4(f64::from(v2.z))
        );
        b.extend_from_slice(s.as_bytes());
        b.push(color);
        s = String::from("' ");
        if n != 2 {
            s += ",";
        }
        // the official build adds the running sum to the corner (the corner's NaN first)
        cx = add64(f64::from(v1.x), cx);
        cy = add64(f64::from(v1.y), cy);
        cz = add64(f64::from(v1.z), cz);
    }
    cx = div64(cx, 3.0);
    cy = div64(cy, 3.0);
    cz = div64(cz, 3.0);
    s += ");\n";
    s += &format!("h=text({},{},{},'F{faceno}');\n", f6_4(cx), f6_4(cy), f6_4(cz));
    s += "set(h,'fontsize',7,'color','";
    b.extend_from_slice(s.as_bytes());
    b.push(color);
    b.extend_from_slice(b"');\n");
    if norm_len.abs() > 0.0 {
        let snorm = mp_slow::normal_face(sg, surf, nonneg(faceno), 0);
        b.extend_from_slice(b"hold on;\n");
        b.extend_from_slice(
            format!("plot3([{}],[{}],[{}],'", f6_4(cx), f6_4(cy), f6_4(cz)).as_bytes(),
        );
        b.push(color);
        b.extend_from_slice(b"*',");
        // the official build: the normal times the length, then the product plus the centre
        let end = |c: f64, n: f32| add64(mul64(f64::from(n), norm_len), c);
        b.extend_from_slice(
            format!(
                "[{},{}],[{},{}],[{},{}],'k'",
                f6_4(cx),
                f6_4(end(cx, snorm[0])),
                f6_4(cy),
                f6_4(end(cy, snorm[1])),
                f6_4(cz),
                f6_4(end(cz, snorm[2]))
            )
            .as_bytes(),
        );
        b.extend_from_slice(b");\nhold off;\n");
    }
    fp.put_bytes(&b);
    0
}

/// `MatlabPlotVertex(fp, surf, vno, color, NormLen)`: `% Vertex <vno>` on standard output; into
/// `fp` a `plot3` of the vertex in `color` with a star, a text `V<vno>` and, for a `NormLen` not
/// 0, its normal times `NormLen` in black. Returns 0.
pub fn matlab_plot_vertex(
    io: &mut Io,
    fp: &mut CFile,
    surf: &Mris,
    vno: i32,
    color: u8,
    norm_len: f64,
) -> i32 {
    let v = surf.vertex(nonneg(vno));
    let (x, y, z) = (f64::from(v.x), f64::from(v.y), f64::from(v.z));
    let _ = writeln!(io.out, "% Vertex {vno}");
    let mut b = format!("plot3([{}],[{}],[{}],'", f6_4(x), f6_4(y), f6_4(z)).into_bytes();
    b.push(color);
    b.extend_from_slice(b"*')\n");
    b.extend_from_slice(format!("h=text({},{},{},'V{vno}');\n", f6_4(x), f6_4(y), f6_4(z)).as_bytes());
    b.extend_from_slice(b"set(h,'fontsize',7,'color','");
    b.push(color);
    b.extend_from_slice(b"');\n");
    if norm_len.abs() > 0.0 {
        // the official build: the normal times the length, then the product plus the centre
        let end = |c: f64, n: f32| add64(mul64(f64::from(n), norm_len), c);
        b.extend_from_slice(b"hold on;\n");
        b.extend_from_slice(
            format!(
                "plot3([{},{}],[{},{}],[{},{}],'k')\n",
                f6_4(x),
                f6_4(end(x, v.nx)),
                f6_4(y),
                f6_4(end(y, v.ny)),
                f6_4(z),
                f6_4(end(z, v.nz))
            )
            .as_bytes(),
        );
        b.extend_from_slice(b"hold off;\n");
    }
    fp.put_bytes(&b);
    0
}

