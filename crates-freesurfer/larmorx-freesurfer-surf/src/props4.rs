// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Queue item 3.1-50, part b (relay 18): the curvature bookkeeping of
//! `utils/mrisurf_metricProperties.cpp` whose dependencies are ported: the distance moved along the
//! normal (`mrisTrackTotalDistance`, `mrisTrackTotalDistanceNew`), the total variations
//! (`MRIStotalVariation`, `MRIStotalVariationDifference`), the curvature error
//! (`MRIScurvatureError`), the curvature choices (`MRISuseCurvatureDifference`, `...Max`, `...Min`,
//! `...Stretch`, `MRISuseNegCurvature`) and the point-in-spherical-triangle test of the topology
//! fixer (`containsAnotherVertexOnSphere`).
//!
//! Arithmetic through fs.core's `sse` operations in the official build's operand order
//! (`docs/findings/freesurfer-surfio.md` §20). Defect (`larmorx_core::defects`):
//! - **FS-371** (memory): `containsAnotherVertexOnSphere` lists the triangle's neighbours in 1,000
//!   places: more than 1,000 write past the list. Both modes list them all (noted).

use larmorx_core::defects::FS_371;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, NO_ERROR};
use larmorx_freesurfer_core::sse::{add, add64, div, mul, mul64, sqrt, sub, sub64};

use crate::coords::Io;
use crate::mris::{Mris, Vertex};

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// `EDOM` (Linux).
const EDOM: i32 = 33;

/// `mrisTrackTotalDistance(mris)` (`new` false: the step `dx`, `dy`, `dz`) and
/// `mrisTrackTotalDistanceNew(mris)` (`new` true: the old step `odx`, `ody`, `odz`): each unripped
/// vertex's curvature increased by the step's component along its normal, in `float` (`nc +
/// curv`).
pub fn track_total_distance(mris: &mut Mris, new: bool) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        let d = if new {
            [v.odx, v.ody, v.odz]
        } else {
            [v.dx, v.dy, v.dz]
        };
        let nc = add(add(mul(d[0], v.nx), mul(d[1], v.ny)), mul(d[2], v.nz));
        v.curv = add(nc, v.curv);
    }
    NO_ERROR
}

/// The `ErrorPrintf` both total variations print when the running sum is not finite.
fn not_finite(g: &mut Globals, io: &mut Io, var: f64, vno: usize) {
    if !var.is_finite() {
        log::error_printf(
            g,
            io.err,
            ERROR_BADPARM,
            &format!("curvature at vertex {vno} is not finite!\n"),
        );
    }
}

/// `MRIStotalVariationDifference(mris)`: the sum over unripped vertices of `area (k1 - k2)²` (the
/// difference and its magnitude in `float`, the rest in `double`), with an error message at each
/// vertex after which the sum is not finite.
pub fn total_variation_difference(g: &mut Globals, io: &mut Io, mris: &Mris) -> f64 {
    let mut var = 0.0f64;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let delta = f64::from(sub(v.k1, v.k2).abs());
        var = add64(var, mul64(mul64(delta, delta), f64::from(v.area)));
        not_finite(g, io, var, vno);
    }
    var
}

/// `MRIStotalVariation(mris)`: the sum over unripped vertices of `area (k1² + k2²)` (the squares in
/// `float`), with [`total_variation_difference`]'s message.
pub fn total_variation(g: &mut Globals, io: &mut Io, mris: &Mris) -> f64 {
    let mut var = 0.0f64;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let delta = f64::from(add(mul(v.k1, v.k1), mul(v.k2, v.k2)));
        var = add64(var, mul64(delta, f64::from(v.area)));
        not_finite(g, io, var, vno);
    }
    var
}

/// `MRIScurvatureError(mris, Kd)`: the sum over unripped vertices of `area (k1 - Kd)² + area (k2 -
/// Kd)²` in `double`.
pub fn curvature_error(mris: &Mris, kd: f64) -> f64 {
    let mut kerror = 0.0f64;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let area = f64::from(v.area);
        for k in [v.k1, v.k2] {
            let d = sub64(f64::from(k), kd);
            kerror = add64(kerror, mul64(mul64(d, d), area));
        }
    }
    kerror
}

/// The extremes the curvature choices keep: `curv > kmax`, `curv < kmin` from 100000 and -100000.
fn extremes(mris: &mut Mris, pick: impl Fn(&Vertex) -> f32) -> (f32, f32) {
    let (mut kmin, mut kmax) = (100000.0f32, -100000.0f32);
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        v.curv = pick(v);
        if v.curv > kmax {
            kmax = v.curv;
        }
        if v.curv < kmin {
            kmin = v.curv;
        }
    }
    mris.min_curv = kmin;
    mris.max_curv = kmax;
    (kmin, kmax)
}

/// `MRISuseCurvatureDifference(mris)`: each unripped vertex's curvature set to `|k1 - k2|`; the
/// extremes kept (100000 and -100000 with none).
pub fn use_curvature_difference(mris: &mut Mris) -> i32 {
    extremes(mris, |v| sub(v.k1, v.k2).abs());
    NO_ERROR
}

/// `MRISuseCurvatureMax(mris)`: each unripped vertex's curvature set to the principal curvature
/// larger in magnitude (`k2` on a tie); the extremes kept and printed (`kmin = %2.2f, kmax =
/// %2.2f`).
pub fn use_curvature_max(io: &mut Io, mris: &mut Mris) -> i32 {
    let (kmin, kmax) = extremes(mris, |v| if v.k1.abs() > v.k2.abs() { v.k1 } else { v.k2 });
    let _ = writeln!(
        io.out,
        "kmin = {}, kmax = {}",
        fmt_f(f64::from(kmin), 2, 2),
        fmt_f(f64::from(kmax), 2, 2)
    );
    NO_ERROR
}

/// `MRISuseCurvatureMin(mris)`: each unripped vertex's curvature set to the principal curvature
/// smaller in magnitude (`k1` on a tie); the extremes kept.
pub fn use_curvature_min(mris: &mut Mris) -> i32 {
    extremes(mris, |v| if v.k1.abs() > v.k2.abs() { v.k2 } else { v.k1 });
    NO_ERROR
}

/// `MRISuseCurvatureStretch(mris)`: each unripped vertex's curvature set to the largest stretch
/// `dist sqrt(orig_area / total_area) - dist_orig` over its first `vtotal` neighbours (0 at least),
/// in `float` (glibc's `sqrtf` sets `errno` to `EDOM` for a negative ratio); the extremes kept.
pub fn use_curvature_stretch(g: &mut Globals, mris: &mut Mris) -> i32 {
    let ratio = div(mris.orig_area, mris.total_area);
    if ratio < 0.0 {
        g.errno = EDOM;
    }
    let scale = sqrt(ratio);
    let (mut kmin, mut kmax) = (100000.0f32, -100000.0f32);
    for vno in 0..nverts(mris) {
        let vtotal = usize::try_from(mris.topology(vno).vtotal).unwrap_or(0);
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        let mut max_stretch = 0.0f32;
        for n in 0..vtotal {
            let dist = mul(v.dist.as_ref().expect("dist")[n], scale);
            let stretch = sub(dist, v.dist_orig.as_ref().expect("dist_orig")[n]);
            if stretch > max_stretch {
                max_stretch = stretch;
            }
        }
        v.curv = max_stretch;
        if v.curv > kmax {
            kmax = v.curv;
        }
        if v.curv < kmin {
            kmin = v.curv;
        }
    }
    mris.min_curv = kmin;
    mris.max_curv = kmax;
    NO_ERROR
}

/// `MRISuseNegCurvature(mris)`: each unripped vertex's curvature 1 when one of its faces has a
/// negative area, else 0; the extremes set to 0 and 1.
pub fn use_neg_curvature(mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let vt = mris.topology(vno);
        let faces = vt.f.as_deref().unwrap_or(&[]);
        let neg = faces[..usize::from(vt.num)]
            .iter()
            .any(|&f| mris.face(f as usize).area < 0.0);
        mris.vertex_mut(vno).curv = if neg { 1.0 } else { 0.0 };
    }
    mris.min_curv = 0.0;
    mris.max_curv = 1.0;
    NO_ERROR
}

/// `F_CROSS(a, b, d)` in `double`.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        sub64(mul64(a[1], b[2]), mul64(b[1], a[2])),
        sub64(mul64(a[2], b[0]), mul64(b[2], a[0])),
        sub64(mul64(a[0], b[1]), mul64(b[0], a[1])),
    ]
}

/// `F_DOT(a, b)` in `double`.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    add64(
        add64(mul64(a[0], b[0]), mul64(a[1], b[1])),
        mul64(a[2], b[2]),
    )
}

fn canon(v: &Vertex) -> [f32; 3] {
    [v.cx, v.cy, v.cz]
}

/// `containsAnotherVertexOnSphere(mris, vno0, vno1, vno2, mode)` (`SPHERE_INTERSECTION`): 1 when
/// a canonical position of one of the triangle's immediate neighbours (each corner's, the other two
/// corners left out; marked vertices left out; with `mode`, those whose `fixedval` is 0 too) lies
/// inside the triangle: for each edge (the midpoint `o` of its ends, `n = o x (end - start)`) on the
/// same side as the opposite corner (`((corner - o) . n) ((p - o) . n) < 0` for none); else 0. The
/// candidates are marked while they are listed and unmarked after.
pub fn contains_another_vertex_on_sphere(
    g: &mut Globals,
    mris: &mut Mris,
    vno0: i32,
    vno1: i32,
    vno2: i32,
    mode: i32,
) -> i32 {
    let mut orgn = [[0.0f64; 3]; 3];
    let mut normal = [[0.0f64; 3]; 3];
    let mut sign = [0.0f64; 3];
    let mut tri = [vno0, vno1, vno2];
    for i in 0..3 {
        if i > 0 {
            tri = [tri[1], tri[2], tri[0]];
        }
        let c0 = canon(mris.vertex(tri[0] as usize));
        let c1 = canon(mris.vertex(tri[1] as usize));
        let c2 = canon(mris.vertex(tri[2] as usize));
        let mut tangent = [0.0f64; 3];
        for k in 0..3 {
            orgn[i][k] = f64::from(add(c1[k], c2[k])) / 2.0;
            tangent[k] = f64::from(sub(c2[k], c1[k]));
        }
        normal[i] = cross(orgn[i], tangent);
        for k in 0..3 {
            tangent[k] = sub64(f64::from(c0[k]), orgn[i][k]);
        }
        sign[i] = dot(tangent, normal[i]);
    }
    let mut vertices: Vec<i32> = Vec::new();
    for (v0, v1, v2) in [(vno0, vno1, vno2), (vno1, vno0, vno2), (vno2, vno0, vno1)] {
        let vt = mris.topology(v0 as usize);
        let nbrs: Vec<i32> =
            vt.v.as_deref().unwrap_or(&[])[..usize::try_from(vt.vnum).unwrap_or(0)].to_vec();
        for nb in nbrs {
            if nb == v1 || nb == v2 {
                continue;
            }
            let vn = mris.vertex_mut(nb as usize);
            if vn.marked != 0 {
                continue;
            }
            if mode != 0 && vn.fixedval == 0 {
                continue;
            }
            vertices.push(nb);
            vn.marked = 1;
        }
    }
    if vertices.len() > 1000 {
        g.notices.add(
            &FS_371,
            format!("containsAnotherVertexOnSphere: {} neighbours are listed (FreeSurfer writes past its 1,000)", vertices.len()),
        );
    }
    for &n in &vertices {
        mris.vertex_mut(n as usize).marked = 0;
    }
    for &n in &vertices {
        let c = canon(mris.vertex(n as usize));
        let mut inside = true;
        for i in 0..3 {
            let t = [
                sub64(f64::from(c[0]), orgn[i][0]),
                sub64(f64::from(c[1]), orgn[i][1]),
                sub64(f64::from(c[2]), orgn[i][2]),
            ];
            if mul64(sign[i], dot(t, normal[i])) < 0.0 {
                inside = false;
                break;
            }
        }
        if inside {
            return 1;
        }
    }
    0
}
