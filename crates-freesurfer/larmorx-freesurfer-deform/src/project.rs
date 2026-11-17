// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.deform's projection of a surface back onto its kind and the flattening of a patch
//! (`utils/mrisurf_deform.cpp` 142-386 and 2172-2335, `utils/mrisurf_deform.h` 48-61; queue item
//! 3.4-06, taken early; relay 44 of track D): `transform`, `mris_project_point_into_face`,
//! `mrisProjectOntoSurface`, `mrisSurfaceProjector`'s `canDo` and `project` (`mrisProjectSurface`
//! for `MRIS` and `MRIS_MP`, `mrisProjectSurface_CanDo`), `MRISflattenPatch`.
//!
//! The arithmetic is the official build's (`mrisurf_deform.cpp.o`): the flattening sums, divides,
//! takes its square roots (`sqrtss`) and rotates in `float`, with the operand orders of the
//! object code (for a NaN's bits: the positions' rotation and the normals' are compiled with
//! different orders); the pial projection's fallback works in `double` (`lambda[1] + lambda[0]`
//! in that order). Defects: FS-1001 (the pial projection without a face table, or for a vertex
//! with no face within 1,000 mm: FreeSurfer reads through a null pointer), FS-1002, FS-1003 and
//! FS-1005 (the flattening's report of the diagnostic vertex), FS-1004 (a patch whose normals sum
//! to zero).

use std::io::Write;

use larmorx_core::defects::{FS_576, FS_1001, FS_1002, FS_1003, FS_1004, FS_1005};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{self, BUILD_ROOT, DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sqrt64, sub, sub64};
use larmorx_freesurfer_surf::aaa::{MrisStatus, SurfGlobals};
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::mp::compute_metric_properties;
use larmorx_freesurfer_surf::mp_slow::compute_normals;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::project;
use larmorx_freesurfer_surf::props2::{face_barycentric_coords, project_point_to_plane};
use larmorx_freesurfer_surf::tritri::{narrow, widen};
use larmorx_freesurfer_surf::views::{
    CANONICAL_VERTICES, CURRENT_VERTICES, FLATTENED_VERTICES, PIAL_VERTICES,
    vertex_coord2xyz_double,
};

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// `(Gdiag & DIAG_SHOW) && DIAG_VERBOSE_ON`.
fn verbose_show(g: &Globals) -> bool {
    let both = DIAG_SHOW | DIAG_VERBOSE;
    g.gdiag & both == both
}

/// `%2.1f`, `%2.2f`, `%2.3f` of a `float`.
fn f1(x: f32) -> String {
    fmt_f(f64::from(x), 2, 1)
}
fn f2(x: f32) -> String {
    fmt_f(f64::from(x), 2, 2)
}
fn f3(x: f32) -> String {
    fmt_f(f64::from(x), 2, 3)
}

/// `transform(&x, &y, &z, nx, ny, nz, d)` (`utils/mrisurf_deform.h` 48) of a vertex's position as
/// `MRISflattenPatch` compiles it: `z' = nx x + ny y + nz z`, `y' = (-ny / d) x + (nx / d) y`,
/// `x' = (y (ny nz / d) + x (nx nz / d)) - z d`, all in `float` (the operands in the object
/// code's order).
pub fn transform_position(p: (f32, f32, f32), n: (f32, f32, f32), d: f32) -> (f32, f32, f32) {
    let ((x, y, z), (nx, ny, nz)) = (p, n);
    let zp = add(add(mul(nx, x), mul(ny, y)), mul(nz, z));
    let yp = add(mul(div(-ny, d), x), mul(div(nx, d), y));
    let c1 = div(mul(nx, nz), d);
    let c2 = div(mul(ny, nz), d);
    let xp = sub(add(mul(y, c2), mul(x, c1)), mul(z, d));
    (xp, yp, zp)
}

/// The same `transform` of a vertex's normal, as compiled there: `x' = ((nx nz / d) x + (ny nz /
/// d) y) - z d` (the other operand order; the same numbers).
pub fn transform_normal(p: (f32, f32, f32), n: (f32, f32, f32), d: f32) -> (f32, f32, f32) {
    let ((x, y, z), (nx, ny, nz)) = (p, n);
    let zp = add(add(mul(nx, x), mul(ny, y)), mul(nz, z));
    let yp = add(mul(div(-ny, d), x), mul(div(nx, d), y));
    let c1 = div(mul(nx, nz), d);
    let c2 = div(mul(ny, nz), d);
    let xp = sub(add(mul(c1, x), mul(c2, y)), mul(z, d));
    (xp, yp, zp)
}

/// `mris_project_point_into_face(mris, face, which, x, y, z, &px, &py, &pz)`: the point projected
/// onto the plane of face `fno` at the coordinate set `which` (`project_point_to_plane`, the
/// corners widened to `double`); `which` one of the flattened, current, pial and canonical sets,
/// else the program ends (`mris_project_point_into_face: which <n> not supported`,
/// `ERROR_BADPARM`).
pub fn project_point_into_face(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &Mris,
    fno: usize,
    which: i32,
    p: [f64; 3],
) -> Result<[f64; 3], Fatal> {
    let face = mris.face(fno);
    let mut c = [[0.0f64; 3]; 3];
    for (k, row) in c.iter_mut().enumerate() {
        let v = mris.vertex(face.v[k] as usize);
        let (x, y, z) = match which {
            FLATTENED_VERTICES => (v.fx, v.fy, v.fz),
            CURRENT_VERTICES => (v.x, v.y, v.z),
            PIAL_VERTICES => (v.pialx, v.pialy, v.pialz),
            CANONICAL_VERTICES => (v.cx, v.cy, v.cz),
            _ => {
                log::error_exit(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!("mris_project_point_into_face: which {which} not supported"),
                )?;
                unreachable!("ErrorExit with a non-zero code always ends the call")
            }
        };
        *row = [widen(x), widen(y), widen(z)];
    }
    Ok(project_point_to_plane(p, c[0], c[1], c[2]).proj)
}

/// The face table the surface carries (`mris->mht`), if it is one.
fn surface_table(mris: &Mris) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
    mris.mht.0.clone()
}

/// FS-1001: FreeSurfer reads through a null pointer (no table, or no face found). By default the
/// program ends with `message` (`ERROR_BADPARM`); with `reproduce` as FreeSurfer's does (a
/// segmentation fault, status 139).
fn null_read(g: &mut Globals, err: &mut dyn Write, message: &str) -> Result<(), Fatal> {
    g.notices.add(&FS_1001, message.to_owned());
    if g.notices.reproduce() {
        return Err(Fatal::Exit(139));
    }
    log::error_exit(g, err, ERROR_BADPARM, message)?;
    unreachable!("ErrorExit with a non-zero code always ends the call")
}

/// FS-576: a table of vertices (FS-998) answers a vertex number beyond the surface's faces, which
/// FreeSurfer reads as a face (memory outside them): an error in both modes.
fn beyond(g: &mut Globals, err: &mut dyn Write, vno: usize, fno: i32) -> Result<(), Fatal> {
    let message = format!(
        "mrisProjectOntoSurface: vertex {vno}'s closest face {fno} is beyond the surface's faces (a table of vertices: FreeSurfer reads memory outside them)"
    );
    g.notices.add(&FS_576, message.clone());
    log::error_exit(g, err, ERROR_BADPARM, &message)?;
    unreachable!("ErrorExit with a non-zero code always ends the call")
}

/// `mrisProjectOntoSurface(mris, which_vertices)` (static; the pial status of
/// `mrisProjectSurface`): each unripped vertex, through the face table the surface carries
/// (`mris->mht`): the closest face within 8 mm and 8 shells it projects into
/// (`MHTfindClosestFaceGeneric`); when there is none or it is more than 0.5 mm away, the closest
/// face within 1,000 mm (no projection test), the vertex's barycentric coordinates in it at
/// `which_vertices` (negative ones set to 0) divided by their Euclidean length (`lambda[0]` 1 when
/// that length is below `FLT_EPSILON`), `lambda[2] = 1 - (lambda[1] + lambda[0])` (when negative
/// 0, with `lambda[1] = 1 - lambda[0]`, and when that is negative too `(1, 0, 0)`), and the vertex
/// placed at the corners' `which_vertices` positions so weighted (`double`); else the vertex
/// projected onto that face's plane at `which_vertices` ([`project_point_into_face`]). The new
/// position rounded to `float`. With `Gdiag_no` at the vertex: `v <n>:  lambda = (...)` (the
/// fallback) and `v <n>: (<old>) projected to (<new>), delta = (<old - new>)` (`%2.2f`) on
/// standard output. FS-1001.
pub fn project_onto_surface(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    which: i32,
) -> Result<(), Fatal> {
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let Some(arc) = surface_table(mris) else {
            null_read(
                g,
                io.err,
                "mrisProjectOntoSurface: the surface has no face table (FreeSurfer reads through a null pointer)",
            )?;
            return Ok(());
        };
        let Some(mht) = arc.downcast_ref::<Mht>() else {
            null_read(
                g,
                io.err,
                "mrisProjectOntoSurface: the surface's table is not a surface hash table",
            )?;
            return Ok(());
        };
        let v = mris.vertex(vno);
        let (x, y, z) = (v.x, v.y, v.z);
        let p = [widen(x), widen(y), widen(z)];
        let (mut fno, fdist) = mht.find_closest_face_generic(g, io.err, mris, p, 8.0, 8, 1)?;
        let (px, py, pz);
        if fno < 0 || fdist > 0.5 {
            fno = mht
                .find_closest_face_generic(g, io.err, mris, p, 1000.0, -1, -1)?
                .0;
            if fno >= mris.nfaces {
                return beyond(g, io.err, vno, fno);
            }
            if fno < 0 {
                null_read(
                    g,
                    io.err,
                    &format!(
                        "mrisProjectOntoSurface: vertex {vno} has no face within 1000 mm (FreeSurfer reads through a null pointer)"
                    ),
                )?;
                return Ok(());
            }
            let (_, mut l) = face_barycentric_coords(g, io, mris, fno as usize, which, p)?;
            for x in &mut l {
                if *x < 0.0 {
                    *x = 0.0;
                }
            }
            let norm = sqrt64(add64(
                add64(mul64(l[0], l[0]), mul64(l[1], l[1])),
                mul64(l[2], l[2]),
            ));
            if norm.abs() < f64::from(f32::EPSILON) {
                l[0] = 1.0;
            } else {
                l[0] = div64(l[0], norm);
                l[1] = div64(l[1], norm);
            }
            l[2] = sub64(1.0, add64(l[1], l[0]));
            if l[2] < 0.0 {
                l[2] = 0.0;
                l[1] = sub64(1.0, l[0]);
                if l[1] < 0.0 {
                    l[1] = 0.0;
                    l[0] = 1.0;
                }
            }
            let (mut sx, mut sy, mut sz) = (0.0f64, 0.0f64, 0.0f64);
            let corners = mris.face(fno as usize).v;
            for n in 0..3 {
                let vn = mris.vertex(corners[n] as usize);
                let (xv, yv, zv) = vertex_coord2xyz_double(g, io.err, vn, which)?;
                sx = add64(sx, mul64(xv, l[n]));
                sy = add64(sy, mul64(yv, l[n]));
                sz = add64(sz, mul64(l[n], zv));
            }
            (px, py, pz) = (sx, sy, sz);
            if vno as i32 == g.gdiag_no {
                let _ = writeln!(
                    io.out,
                    "v {vno}:  lambda = ({} {} {})",
                    fmt_f(l[0], 2, 2),
                    fmt_f(l[1], 2, 2),
                    fmt_f(l[2], 2, 2)
                );
            }
        } else {
            if fno >= mris.nfaces {
                return beyond(g, io.err, vno, fno);
            }
            [px, py, pz] = project_point_into_face(g, io.err, mris, fno as usize, which, p)?;
        }
        if vno as i32 == g.gdiag_no {
            let _ = writeln!(
                io.out,
                "v {vno}: ({} {} {}) projected to ({} {} {}), delta = ({} {} {})",
                f2(x),
                f2(y),
                f2(z),
                fmt_f(px, 2, 2),
                fmt_f(py, 2, 2),
                fmt_f(pz, 2, 2),
                fmt_f(sub64(widen(x), px), 2, 2),
                fmt_f(sub64(widen(y), py), 2, 2),
                fmt_f(sub64(widen(z), pz), 2, 2)
            );
        }
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (narrow(px), narrow(py), narrow(pz));
        let q = [widen(v.x), widen(v.y), widen(v.z)];
        // its sign only reaches a DiagBreak
        face_barycentric_coords(g, io, mris, fno as usize, which, q)?;
    }
    Ok(())
}

/// `mrisProjectSurface(MRIS *mris)` (`mrisSurfaceProjector<MRIS>::project`): by the surface's
/// status: a plane flattened ([`flatten_patch`]), a spherical patch projected spherically
/// (`mrisSphericalProjection`), a sphere or a parameterized sphere onto the sphere of its radius
/// (`MRISprojectOntoSphere`), an ellipsoid onto the default ellipsoid (`MRISprojectOntoEllipsoid(mris,
/// mris, 0, 0, 0)`), the pial status onto the pial surface ([`project_onto_surface`]); every other
/// status (a rigid body included) left as it is. `NO_ERROR`.
pub fn project_surface(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    match mris.status {
        MrisStatus::PLANE => {
            flatten_patch(g, sg, io, mris)?;
        }
        MrisStatus::SPHERICAL_PATCH => project::spherical_projection(mris),
        MrisStatus::PARAMETERIZED_SPHERE | MrisStatus::SPHERE => {
            let r = mris.radius;
            project::project_onto_sphere(g, io, mris, r);
        }
        MrisStatus::ELLIPSOID => {
            project::project_onto_ellipsoid(g, sg, io, mris, false, 0.0, 0.0, 0.0)?;
        }
        MrisStatus::PIAL_SURFACE => project_onto_surface(g, io, mris, PIAL_VERTICES)?,
        _ => {}
    }
    Ok(NO_ERROR)
}

/// `mrisProjectSurface(MRIS_MP *mris)` (`mrisSurfaceProjector<MRIS_MP>::project`): never returns
/// in FreeSurfer: a sphere goes to `MRISprojectOntoSphere(MRIS_MP *, radius)`, whose assertion
/// fails (`mrisurf_project.cpp` line 158), every other status to the projector's own failed
/// assertion (`mrisurf_deform.cpp` line 328; FS-03). No FreeSurfer 8.2.0 program reaches it (its
/// only caller, `MRIScomputeSSE_asThoughGradientApplied`, never takes the `MRIS_MP` path).
pub fn project_surface_mp(g: &mut Globals, io: &mut Io, status: MrisStatus) -> Fatal {
    if status == MrisStatus::SPHERE {
        return project::project_onto_sphere_mp(g, io);
    }
    diag::assert_failed(
        g,
        io.out,
        io.err,
        &format!("{BUILD_ROOT}/utils/mrisurf_deform.cpp"),
        328,
        "false",
    )
}

/// `mrisProjectSurface_CanDo(status)` (`mrisSurfaceProjector<MRIS_MP>::canDo`): whether the
/// `MRIS_MP` projection takes the status (a sphere only); for any other status, the first time
/// in the program (`shown`, the function's `static int`: bit `status mod 32`),
/// `need to process surface status <n>` through `fs::debug()` (standard error, with verbose
/// diagnostics).
pub fn project_surface_can_do(
    g: &Globals,
    err: &mut dyn Write,
    shown: &mut i32,
    status: MrisStatus,
) -> bool {
    if status == MrisStatus::SPHERE {
        return true;
    }
    let mask = 1i32.wrapping_shl(status.0 as u32);
    if *shown & mask == 0 {
        *shown |= mask;
        log::debug(
            g,
            err,
            &format!("need to process surface status {}", status.0),
        );
    }
    false
}

/// `MRISflattenPatch(mris)`: a surface already a plane has its unripped vertices' `z` set to 0.
/// Otherwise the normals computed (`MRIScomputeNormals`); the unripped vertices' mean position
/// and normal sum (`float`); the sum made a unit vector `n`; when `d = |(nx, ny)|` (`float`) is not
/// below `FLT_EPSILON`, each unripped vertex centred on the mean, rotated so that `n` points
/// along z ([`transform_position`], its normal with [`transform_normal`]), and its distance from
/// the mean before and after compared (`flatten: d1=<before>, d2=<after>` on standard output
/// when they differ by more than 0.0001); then every vertex's `z` set to 0 (ripped ones too), the
/// status a plane, `MRIScomputeMetricProperties`. With `DIAG_SHOW` and verbose diagnostics the
/// mean, the sum, the unit normal and its rotation printed (`flatten: avg p = {...}`, `flatten:
/// sum n = {...}`, `flatten: norm n = {...}`, `flatten: transformed n = {...}`); with
/// `DIAG_SHOW`, the diagnostic vertex when it is flattened (`vertex <n> flattened @ (...)`) and
/// at the end with its neighbours (FS-1002, FS-1003, FS-1005). FS-1004. `NO_ERROR`.
pub fn flatten_patch(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    let nv = nverts(mris);
    if mris.status == MrisStatus::PLANE {
        for k in 0..nv {
            let v = mris.vertex_mut(k);
            if v.ripflag == 0 {
                v.z = 0.0;
            }
        }
        return Ok(NO_ERROR);
    }
    compute_normals(g, sg, io, mris);
    let (mut x, mut y, mut z) = (0.0f32, 0.0f32, 0.0f32);
    let (mut nx, mut ny, mut nz) = (0.0f32, 0.0f32, 0.0f32);
    let mut an = 0i32;
    for k in 0..nv {
        let v = mris.vertex(k);
        if v.ripflag != 0 {
            continue;
        }
        x = add(x, v.x);
        y = add(y, v.y);
        z = add(z, v.z);
        nx = add(nx, v.nx);
        ny = add(ny, v.ny);
        nz = add(nz, v.nz);
        an += 1;
    }
    let af = an as f32;
    x = div(x, af);
    y = div(y, af);
    z = div(z, af);
    if verbose_show(g) {
        let _ = writeln!(
            io.out,
            "flatten: avg p = {{{}, {}, {}}}",
            f1(x),
            f1(y),
            f1(z)
        );
        let _ = writeln!(
            io.out,
            "flatten: sum n = {{{}, {}, {}}}",
            f2(nx),
            f2(ny),
            f2(nz)
        );
    }
    let d0 = sqrt(add(add(mul(nx, nx), mul(ny, ny)), mul(nz, nz)));
    let zero_normal = an > 0 && d0 == 0.0;
    if zero_normal {
        g.notices.add(
            &FS_1004,
            "MRISflattenPatch: the patch's normals sum to zero (FreeSurfer turns every position into NaN): flattened as it faces"
                .to_owned(),
        );
    }
    if zero_normal && g.notices.fix() {
        // as a patch already facing z (FreeSurfer's FZERO(d) case): no centring or rotation
    } else {
        nx = div(nx, d0);
        ny = div(ny, d0);
        nz = div(nz, d0);
        let s2 = add(mul(nx, nx), mul(ny, ny));
        let d = sqrt(s2);
        // FZERO(d): d is a square root (not negative, or NaN, which is not zero)
        if d >= f32::EPSILON || d.is_nan() {
            if verbose_show(g) {
                let _ = writeln!(
                    io.out,
                    "flatten: norm n = {{{}, {}, {}}}",
                    f2(nx),
                    f2(ny),
                    f2(nz)
                );
            }
            let n = (nx, ny, nz);
            for k in 0..nv {
                if mris.vertex(k).ripflag != 0 {
                    continue;
                }
                let v = mris.vertex_mut(k);
                let (vx, vy, vz) = (sub(v.x, x), sub(v.y, y), sub(v.z, z));
                let d1 = sqrt(add(add(mul(vx, vx), mul(vy, vy)), mul(vz, vz)));
                let (xp, yp, zp) = transform_position((vx, vy, vz), n, d);
                (v.x, v.y, v.z) = (xp, yp, zp);
                let d2 = sqrt(add(mul(zp, zp), add(mul(xp, xp), mul(yp, yp))));
                (v.nx, v.ny, v.nz) = transform_normal((v.nx, v.ny, v.nz), n, d);
                if f64::from(sub(d1, d2).abs()) > 0.0001 {
                    let _ = writeln!(
                        io.out,
                        "flatten: d1={}, d2={}",
                        fmt_f(f64::from(d1), 0, 6),
                        fmt_f(f64::from(d2), 0, 6)
                    );
                }
            }
            if verbose_show(g) {
                // transform(&nx, &ny, &nz, nx, ny, nz, d), as compiled for the print
                let zp = add(mul(nz, nz), s2);
                let yp = add(mul(div(nx, d), ny), mul(div(-ny, d), nx));
                let xp = sub(
                    add(mul(div(mul(nx, nz), d), nx), mul(ny, div(mul(ny, nz), d))),
                    mul(nz, d),
                );
                let _ = writeln!(
                    io.out,
                    "flatten: transformed n = {{{}, {}, {}}}",
                    f1(xp),
                    f1(yp),
                    f1(zp)
                );
            }
        }
    }
    for k in 0..nv {
        mris.vertex_mut(k).z = 0.0;
        if k as i32 == g.gdiag_no && g.gdiag & DIAG_SHOW != 0 {
            let v = mris.vertex(k);
            let _ = writeln!(
                io.out,
                "vertex {k} flattened @ ({}, {}, {})",
                f2(v.x),
                f2(v.y),
                f2(v.z)
            );
        }
    }
    mris.status = MrisStatus::PLANE;
    compute_metric_properties(g, sg, io, mris)?;
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag_no >= 0 {
        report(g, io, mris)?;
    }
    Ok(NO_ERROR)
}

/// The end of `MRISflattenPatch` with `DIAG_SHOW` and a diagnostic vertex: `<n> @ (<x>, <y>,
/// <z>): area <a>, oa <oa>, nz=<nz>, vnum=<vnum>`, a line per 1-hop neighbour with its distance
/// and original distance (`, d=<d>, od=<od>`), an empty line. FS-1002: a diagnostic vertex beyond
/// the surface's vertices (FreeSurfer reads memory outside them): no report, both modes (and its
/// distances fewer than its neighbours: the report ends there). FS-1005: a vertex with neighbours
/// but without distances (FreeSurfer reads through a null pointer): the report ends with its
/// first line by default, a segmentation fault (status 139) with `reproduce`. FS-1003: FreeSurfer
/// prints the diagnostic vertex's original area on every neighbour's line; by default each
/// neighbour's own.
fn report(g: &mut Globals, io: &mut Io, mris: &Mris) -> Result<(), Fatal> {
    let d = g.gdiag_no;
    if d >= mris.nvertices {
        g.notices.add(
            &FS_1002,
            format!(
                "MRISflattenPatch: the diagnostic vertex {d} is beyond the surface's {} vertices (FreeSurfer reads it there): no report",
                mris.nvertices
            ),
        );
        return Ok(());
    }
    let vd = d as usize;
    let v = mris.vertex(vd);
    let vt = mris.topology(vd);
    let _ = writeln!(
        io.out,
        "{d} @ ({}, {}, {}): area {}, oa {}, nz={}, vnum={}",
        f1(v.x),
        f1(v.y),
        f1(v.z),
        f3(v.area),
        f3(v.origarea),
        f3(v.nz),
        vt.vnum
    );
    let vnum = usize::try_from(vt.vnum).unwrap_or(0);
    if vnum > 0 && (v.dist.is_none() || v.dist_orig.is_none() || vt.v.is_none()) {
        g.notices.add(
            &FS_1005,
            format!(
                "MRISflattenPatch: the diagnostic vertex {d} has no distances (or neighbours) to report (FreeSurfer reads through a null pointer)"
            ),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let _ = writeln!(io.out);
        return Ok(());
    }
    let mut message = false;
    for n in 0..vnum {
        let vno = vt.v.as_ref().expect("the neighbours")[n];
        let vn = mris.vertex(vno as usize);
        let vnt = mris.topology(vno as usize);
        let oa = if g.notices.reproduce() {
            v.origarea
        } else {
            vn.origarea
        };
        if vn.origarea.to_bits() != v.origarea.to_bits() {
            message = true;
        }
        let dist = |a: &Option<Vec<f32>>| a.as_ref().and_then(|a| a.get(n).copied());
        let (Some(dn), Some(don)) = (dist(&v.dist), dist(&v.dist_orig)) else {
            g.notices.add(
                &FS_1002,
                format!(
                    "MRISflattenPatch: the diagnostic vertex {d}'s distances are fewer than its neighbours (FreeSurfer reads past them)"
                ),
            );
            break;
        };
        let _ = writeln!(
            io.out,
            "{vno} @ ({}, {}, {}): area {}, oa {}, nz={}, vnum={}, d={}, od={}",
            f1(vn.x),
            f1(vn.y),
            f1(vn.z),
            f3(vn.area),
            f3(oa),
            f3(vn.nz),
            vnt.vnum,
            f2(dn),
            f2(don)
        );
    }
    if message {
        g.notices.add(
            &FS_1003,
            format!(
                "MRISflattenPatch: the diagnostic vertex {d}'s neighbour lines print its own original area"
            ),
        );
    }
    let _ = writeln!(io.out);
    Ok(())
}
