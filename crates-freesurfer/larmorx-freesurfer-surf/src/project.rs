// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surf's projections of a surface onto a sphere, a cylinder and an ellipsoid
//! (`utils/mrisurf_project.cpp` whole; queue item 3.4-04, taken early; relay 43 of track D):
//! `mrisSphericalProjectXYZ`, `mrisSphericalProjection`, `MRISprojectOntoSphere` (its three forms),
//! `mrisAssignFaces`, `MRISprojectOntoCylinder`, `MRISprojectOntoEllipsoid`,
//! `MRISradialProjectOntoEllipsoid`.
//!
//! The arithmetic is the official build's (`mrisurf_project.cpp.o`): the spherical projection of a
//! point and the radial ellipsoid take the square root in `float` (`sqrtss`), the sphere in
//! `double`, the cylinder's `-1.0 + radius / sqrt(...)` is made in `float` (exact either way), the
//! orthographic ellipsoid takes its root in `double` and rounds it back; the operand orders (for a
//! NaN's bits) are the object code's.
//!
//! `MRISprojectOntoEllipsoid(src, dst, ...)` and the radial one are offered for a destination that
//! is the source (`mrisProjectSurface`'s) or a new copy of it (`NULL`); no FreeSurfer caller passes
//! a third surface.

use std::io::Write;

use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sqrt64, sub, sub64};

use crate::aaa::{MrisStatus, SurfGlobals};
use crate::base::project_point_onto_sphere;
use crate::coords::{self, Io};
use crate::dist;
use crate::mht::Mht;
use crate::mp_slow::update_ellipsoid_surface;
use crate::mris::Mris;
use crate::props8;
use crate::tritri::narrow;

/// `DEFAULT_RADIUS`.
pub const DEFAULT_RADIUS: f64 = 100.0;
/// `DEFAULT_A`, `DEFAULT_B`, `DEFAULT_C` (all 122 mm).
pub const DEFAULT_ABC: f32 = 122.0;

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// `FZERO` of a `float` (`fabs(f) < FLT_EPSILON` in `float`).
fn fzero(x: f32) -> bool {
    x.abs() < f32::EPSILON
}

/// `(Gdiag & DIAG_SHOW) && DIAG_VERBOSE_ON`.
fn verbose_show(g: &Globals) -> bool {
    let both = DIAG_SHOW | DIAG_VERBOSE;
    g.gdiag & both == both
}

/// `mrisSphericalProjectXYZ(xs, ys, zs, &xd, &yd, &zd)`: twice, `dist` the `float` square root of
/// `xs² + ys² + zs²` (in `float`) widened, `lambda = 100 / dist`, each coordinate `(float)(x x
/// lambda)` in `double`.
pub fn spherical_project_xyz(xs: f32, ys: f32, zs: f32) -> (f32, f32, f32) {
    let once = |x: f32, y: f32, z: f32| {
        let s = add(add(mul(x, x), mul(y, y)), mul(z, z));
        let lambda = div64(DEFAULT_RADIUS, f64::from(sqrt(s)));
        (
            narrow(mul64(f64::from(x), lambda)),
            narrow(mul64(f64::from(y), lambda)),
            narrow(mul64(f64::from(z), lambda)),
        )
    };
    let (x, y, z) = once(xs, ys, zs);
    once(x, y, z)
}

/// `mrisSphericalProjection(mris)`: every unripped vertex's position projected
/// ([`spherical_project_xyz`]) and copied to its canonical position.
pub fn spherical_projection(mris: &mut Mris) {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        (v.x, v.y, v.z) = spherical_project_xyz(v.x, v.y, v.z);
        (v.cx, v.cy, v.cz) = (v.x, v.y, v.z);
    }
}

/// `MRISprojectOntoSphere(mris, r)`: `r` 100 when `|r| < FLT_EPSILON`; the surface centred
/// (`MRIScenter`) unless it is a sphere or a parameterized sphere; `radius` set to `r`; the
/// distances released but the original ones (`MRISfreeDistsButNotOrig`); every unripped vertex
/// moved (in `double`) by `d (x, y, z)`, `d = 1 - r / |p|` (0 when `|p| < FLT_EPSILON`); with
/// `DIAG_SHOW` and verbose diagnostics `sphere_project: total dist = <sum of the moves>` on standard
/// output; the status a sphere (a parameterized sphere stays one); `MRISupdateEllipsoidSurface`.
pub fn project_onto_sphere(g: &mut Globals, io: &mut Io, mris: &mut Mris, r: f64) -> i32 {
    let r = if r.abs() < f64::from(f32::EPSILON) {
        DEFAULT_RADIUS
    } else {
        r
    };
    if mris.status != MrisStatus::SPHERE && mris.status != MrisStatus::PARAMETERIZED_SPHERE {
        coords::center(mris);
    }
    mris.radius = r;
    dist::free_dists_but_not_orig(mris);
    let show = verbose_show(g);
    let mut total = 0f64;
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        let (x, y, z) = (f64::from(v.x), f64::from(v.y), f64::from(v.z));
        let d2 = add64(add64(mul64(x, x), mul64(y, y)), mul64(z, z));
        let dist = sqrt64(d2);
        let d = if dist.abs() < f64::from(f32::EPSILON) {
            0.0
        } else {
            sub64(1.0, div64(r, dist))
        };
        let (dx, dy, dz) = (mul64(x, d), mul64(y, d), mul64(d, z));
        v.x = narrow(sub64(x, dx));
        v.y = narrow(sub64(y, dy));
        v.z = narrow(sub64(z, dz));
        if show {
            let s = add64(add64(mul64(dy, dy), mul64(dx, dx)), mul64(dz, dz));
            total = add64(sqrt64(s), total);
        }
    }
    if show {
        let _ = writeln!(io.out, "sphere_project: total dist = {}", fmt_f(total, 0, 6));
    }
    mris.status = if mris.status == MrisStatus::PARAMETERIZED_SPHERE {
        MrisStatus::PARAMETERIZED_SPHERE
    } else {
        MrisStatus::SPHERE
    };
    update_ellipsoid_surface(mris);
    NO_ERROR
}

/// `MRISprojectOntoSphere(mris_src, mris_dst, r)`: FreeSurfer asserts the two are one surface
/// (`same`; else `cheapAssert(mris_src == mris_dst)`, `mrisurf_project.cpp` line 165, FS-03), then
/// [`project_onto_sphere`].
pub fn project_onto_sphere_src_dst(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    same: bool,
    r: f64,
) -> Result<i32, Fatal> {
    if !same {
        return Err(larmorx_freesurfer_core::diag::assert_failed(
            g,
            io.out,
            io.err,
            &format!(
                "{}/utils/mrisurf_project.cpp",
                larmorx_freesurfer_core::diag::BUILD_ROOT
            ),
            165,
            "mris_src == mris_dst",
        ));
    }
    Ok(project_onto_sphere(g, io, mris, r))
}

/// `MRISprojectOntoSphere(MRIS_MP *mris, r)`: not offered in FreeSurfer
/// (`cheapAssert(false)`, `mrisurf_project.cpp` line 158, FS-03).
pub fn project_onto_sphere_mp(g: &mut Globals, io: &mut Io) -> Fatal {
    larmorx_freesurfer_core::diag::assert_failed(
        g,
        io.out,
        io.err,
        &format!(
            "{}/utils/mrisurf_project.cpp",
            larmorx_freesurfer_core::diag::BUILD_ROOT
        ),
        158,
        "false",
    )
}

/// `mrisAssignFaces(mris, mht, which_vertices)`: every unripped vertex's current position
/// projected onto the sphere of the surface's radius (`project_point_onto_sphere`, the radius
/// rounded to `float`), then its face (`v->fno`): the closest face projected into within 8 mm and
/// 8 shells (`MHTfindClosestFaceGeneric`), else the closest within 1,000 mm. `which_vertices` is
/// not used (the table's own position set decides).
pub fn assign_faces(
    g: &mut Globals,
    err: &mut dyn Write,
    mris: &mut Mris,
    mht: &Mht,
    _which_vertices: i32,
) -> Result<(), Fatal> {
    let radius = narrow(mris.radius);
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = project_point_onto_sphere(v.x, v.y, v.z, radius);
        let p = [f64::from(v.x), f64::from(v.y), f64::from(v.z)];
        let (mut fno, _) = mht.find_closest_face_generic(g, err, mris, p, 8.0, 8, 1)?;
        if fno < 0 {
            fno = mht.find_closest_face_generic(g, err, mris, p, 1000.0, -1, -1)?.0;
        }
        mris.vertex_mut(vno).fno = fno;
    }
    Ok(())
}

/// `MRISprojectOntoCylinder(mris, radius)`: the surface centred; every vertex (ripped ones too):
/// `d = radius / sqrt(x² + z²) - 1` in `float`; a `d` that is not finite prints `point
/// (<x>,<y>,<z>) cannot be projected on cylinder` as an error (`ERROR_BADPARM`, `%2.2f`) and is
/// used as it is; `x += d x`, `z += d z` in `float`; then `MRISupdateSurface`. `NO_ERROR`.
pub fn project_onto_cylinder(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&std::path::Path>,
    mris: &mut Mris,
    radius: f32,
) -> Result<i32, Fatal> {
    coords::center(mris);
    for vno in 0..nverts(mris) {
        let (x, y, z) = {
            let v = mris.vertex(vno);
            (v.x, v.y, v.z)
        };
        let s = add(mul(x, x), mul(z, z));
        let d = sub(div(radius, sqrt(s)), 1.0);
        if !d.is_finite() {
            log::error_printf(
                g,
                io.err,
                ERROR_BADPARM,
                &format!(
                    "point ({},{},{}) cannot be projected on cylinder",
                    fmt_f(f64::from(x), 2, 2),
                    fmt_f(f64::from(y), 2, 2),
                    fmt_f(f64::from(z), 2, 2)
                ),
            );
        }
        let v = mris.vertex_mut(vno);
        v.x = add(mul(x, d), x);
        v.z = add(mul(d, z), z);
    }
    props8::update_surface(g, sg, io, dir, mris)?;
    Ok(NO_ERROR)
}

/// The defaults of a zero first axis (`FZERO(a)`: all three 122 mm).
fn axes(a: f32, b: f32, c: f32) -> (f32, f32, f32) {
    if fzero(a) {
        (DEFAULT_ABC, DEFAULT_ABC, DEFAULT_ABC)
    } else {
        (a, b, c)
    }
}

/// The status after an ellipsoid projection: a sphere when the three axes agree (`FZERO(a - b)` and
/// `FZERO(b - c)`), else an ellipsoid.
fn ellipsoid_status(a: f32, b: f32, c: f32) -> MrisStatus {
    if fzero(sub(a, b)) && fzero(sub(b, c)) {
        MrisStatus::SPHERE
    } else {
        MrisStatus::ELLIPSOID
    }
}

/// `MRISradialProjectOntoEllipsoid(src, dst, a, b, c)` with the destination the source (`new`
/// false) or a copy of it (`MRISclone`, `new` true: returned): axes as [`project_onto_ellipsoid`];
/// each vertex (ripped ones too) of the source, relative to the destination's centre (`xctr`,
/// ...), scaled onto the ellipsoid along the line from the centre: `abc x1 / sqrt(b²c² x1² + a²c²
/// y1² + a²b² z1²)` in `float` (the root in `float`); then `MRISupdateEllipsoidSurface` and the
/// status (a sphere when the axes agree).
pub fn radial_project_onto_ellipsoid(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    new: bool,
    a: f32,
    b: f32,
    c: f32,
) -> Result<Option<Mris>, Fatal> {
    let (a, b, c) = axes(a, b, c);
    if new {
        let mut dst = props8::clone(g, sg, io, mris)?;
        radial(mris, Some(&mut dst), a, b, c);
        return Ok(Some(dst));
    }
    radial(mris, None, a, b, c);
    Ok(None)
}

/// The radial projection's loop and ending (`dst` `None`: the source itself).
fn radial(src: &mut Mris, dst: Option<&mut Mris>, a: f32, b: f32, c: f32) {
    let ab = mul(a, b);
    let abc = mul(ab, c);
    let aa = mul(a, a);
    let asq_bsq = mul(mul(aa, b), b);
    let bsq_csq = mul(mul(mul(b, b), c), c);
    let asq_csq = mul(mul(aa, c), c);
    let n = nverts(src);
    let finish = |m: &mut Mris| {
        update_ellipsoid_surface(m);
        m.status = ellipsoid_status(a, b, c);
    };
    let one = |p: (f32, f32, f32), ctr: (f32, f32, f32)| {
        let x1 = sub(p.0, ctr.0);
        let y1 = sub(p.1, ctr.1);
        let z1 = sub(p.2, ctr.2);
        let xt = mul(mul(x1, x1), bsq_csq);
        let yt = mul(mul(y1, y1), asq_csq);
        let zt = mul(mul(z1, z1), asq_bsq);
        let denom = sqrt(add(add(yt, xt), zt));
        (
            div(mul(x1, abc), denom),
            div(mul(y1, abc), denom),
            div(mul(z1, abc), denom),
        )
    };
    match dst {
        None => {
            let ctr = (src.xctr, src.yctr, src.zctr);
            for vno in 0..n {
                let v = src.vertex_mut(vno);
                (v.x, v.y, v.z) = one((v.x, v.y, v.z), ctr);
            }
            finish(src);
        }
        Some(d) => {
            let ctr = (d.xctr, d.yctr, d.zctr);
            for vno in 0..n {
                let s = src.vertex(vno);
                let q = one((s.x, s.y, s.z), ctr);
                let v = d.vertex_mut(vno);
                (v.x, v.y, v.z) = q;
            }
            finish(d);
        }
    }
}

/// `MRISprojectOntoEllipsoid(src, dst, a, b, c)` with the destination the source (`new` false) or a
/// copy of it (`MRISclone`, `new` true: returned): `a b c` 122 mm each when `|a| < FLT_EPSILON`; the
/// destination centred, its `a b c` set; each vertex (ripped ones too) moved orthographically onto
/// the ellipsoid, in `float`: `f = x²/a⁶ + y²/b⁶ + z²/c⁶`, `g = 2 (x²/a⁴ + y²/b⁴ + z²/c⁴)`, `h =
/// x²/a² + y²/b² + z²/c² - 1`, `d = ((float) sqrt(g² - 4 f h) - g) / (2 f)` (the root in
/// `double`), `x += d x / a²` (and `y`, `z`). A `d` that is not finite prints `point (<x>,<y>,<z>)
/// cannot be projected on ell (<a>,<b>,<c>...` as an error (`ERROR_BADPARM`, `%2.2f`, `%2.0f`) and
/// the rest is the radial projection's ([`radial_project_onto_ellipsoid`]) from the source (the
/// destination's vertices already moved kept where the source is the destination). With
/// `DIAG_SHOW` and verbose diagnostics `ellipsoid_project: avgdist = <mean move>` on standard
/// output. Then `MRISupdateEllipsoidSurface` and the status (a sphere when the axes agree).
#[allow(clippy::too_many_arguments)]
pub fn project_onto_ellipsoid(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    new: bool,
    a: f32,
    b: f32,
    c: f32,
) -> Result<Option<Mris>, Fatal> {
    let (a, b, c) = axes(a, b, c);
    let mut copy = if new {
        Some(props8::clone(g, sg, io, mris)?)
    } else {
        None
    };
    let failed = {
        let dst: &mut Mris = match copy.as_mut() {
            Some(d) => d,
            None => &mut *mris,
        };
        orthographic(g, io, dst, a, b, c)
    };
    if failed {
        radial(mris, copy.as_mut(), a, b, c);
    }
    Ok(copy)
}

/// The orthographic projection's loop and ending on the destination; `true` when a vertex could
/// not be projected (its message printed: the caller goes on radially).
fn orthographic(g: &mut Globals, io: &mut Io, dst: &mut Mris, a: f32, b: f32, c: f32) -> bool {
    coords::center(dst);
    (dst.a, dst.b, dst.c) = (a, b, c);
    let (a2, b2, c2) = (mul(a, a), mul(b, b), mul(c, c));
    let (a4, b4, c4) = (mul(a2, a2), mul(b2, b2), mul(c2, c2));
    let (a6, b6, c6) = (mul(a2, a4), mul(b2, b4), mul(c2, c4));
    let show = verbose_show(g);
    let mut avgdist = 0f32;
    for vno in 0..nverts(dst) {
        let (x, y, z) = {
            let v = dst.vertex(vno);
            (v.x, v.y, v.z)
        };
        let (x2, y2, z2) = (mul(x, x), mul(y, y), mul(z, z));
        let f = add(add(div(x2, a6), div(y2, b6)), div(z2, c6));
        let gg = add(add(div(x2, a4), div(y2, b4)), div(z2, c4));
        let gg = add(gg, gg);
        let h = sub(add(add(div(x2, a2), div(y2, b2)), div(z2, c2)), 1.0);
        let disc = sub(mul(gg, gg), mul(h, mul(4.0, f)));
        let root = narrow(sqrt64(f64::from(disc)));
        let d = div(sub(root, gg), add(f, f));
        if !d.is_finite() {
            log::error_printf(
                g,
                io.err,
                ERROR_BADPARM,
                &format!(
                    "point ({},{},{}) cannot be projected on ell ({},{},{}...\n",
                    fmt_f(f64::from(x), 2, 2),
                    fmt_f(f64::from(y), 2, 2),
                    fmt_f(f64::from(z), 2, 2),
                    fmt_f(f64::from(a), 2, 0),
                    fmt_f(f64::from(b), 2, 0),
                    fmt_f(f64::from(c), 2, 0)
                ),
            );
            return true;
        }
        let dx = div(mul(x, d), a2);
        let dy = div(mul(y, d), b2);
        let dz = div(mul(d, z), c2);
        let v = dst.vertex_mut(vno);
        v.x = add(x, dx);
        v.y = add(y, dy);
        v.z = add(z, dz);
        if show {
            let s = add(mul(dz, dz), add(mul(dx, dx), mul(dy, dy)));
            let dist = narrow(sqrt64(f64::from(s)));
            avgdist = add(dist, avgdist);
        }
    }
    if show {
        let mean = div(avgdist, dst.nvertices as f32);
        let _ = writeln!(
            io.out,
            "ellipsoid_project: avgdist = {}",
            fmt_f(f64::from(mean), 0, 6)
        );
    }
    update_ellipsoid_surface(dst);
    dst.status = ellipsoid_status(a, b, c);
    false
}
