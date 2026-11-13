// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Queue item 3.1-51, the rest (relay 20), first group: the curvature choices and statistics of
//! `utils/mrisurf_metricProperties.cpp` (lines 10281-12395): the smoothed normals
//! (`MRISsmoothSurfaceNormals`, `mrisComputeBoundaryNormals`), the mean curvature from the tangent
//! planes (`MRIScomputeMeanCurvature`), the curvature choices (`MRISuseAreaErrors`,
//! `MRISuseGaussianCurvature`, `MRISuseMeanCurvature`, `MRISusePrincipalCurvature`,
//! `MRISuseK1Curvature`, `MRISuseK2Curvature`, `MRISusePrincipalCurvatureFunction`), the folding
//! and intrinsic curvature indices (`MRIScomputeCurvatureIndices`), the largest radius from the
//! bounding box's centre (`MRISmaxRadius`), the curvature's mean and spread
//! (`MRIScomputeAverageCurvature`) and the vertices of faces with a negative area
//! (`MRISmarkNegativeVertices`, `MRISripNegativeVertices`).
//!
//! Arithmetic through fs.core's `sse` operations in the official build's operand order (the unit
//! oracle's object code; `docs/findings/freesurfer-surfio.md` §22). Defects
//! (`larmorx_core::defects`):
//! - **FS-374** (wrong result): `MRISuseAreaErrors` divides each vertex's area change by a count
//!   it never increments (0): every value is infinite or a NaN. Corrected by default (the mean
//!   over the vertex's faces, 0 for a vertex without faces; noted); `reproduce` as FreeSurfer.
//! - **FS-375** (wrong result): `MRISuseK1Curvature`, `MRISuseK2Curvature` and
//!   `MRISusePrincipalCurvatureFunction` start their extremes from the first vertex's previous
//!   curvature, so the range keeps a value no vertex has any more. Corrected by default (the
//!   extremes of the new values, from the first unripped vertex's; noted when they differ);
//!   `reproduce` as FreeSurfer.
//! - **FS-376** (memory): the same three, on a surface without vertices, read the first vertex
//!   that does not exist. Both modes set the extremes to 0 (noted).

use larmorx_core::defects::{FS_374, FS_375, FS_376};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::NO_ERROR;
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sqrt64, sub, sub64};

use crate::coords::Io;
use crate::facenorm::get_face_norm;
use crate::mris::{Mris, Vertex};
use crate::props::compute_tangent_planes;

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

fn nfaces(mris: &Mris) -> usize {
    usize::try_from(mris.nfaces).unwrap_or(0)
}

/// `EDOM` (Linux).
const EDOM: i32 = 33;

/// The first `vnum` neighbours of vertex `vno`.
fn neighbours(mris: &Mris, vno: usize) -> Vec<usize> {
    let vt = mris.topology(vno);
    let n = usize::try_from(vt.vnum).unwrap_or(0);
    vt.v.as_deref().unwrap_or(&[])[..n]
        .iter()
        .map(|&x| x as usize)
        .collect()
}

/// `MRISsmoothSurfaceNormals(mris, navgs)`: `navgs` times, each unripped vertex's normal replaced
/// by the mean of its own and its unripped neighbours' (`(n + n1 + ...) / count` in `float`, kept
/// in `tdx`, `tdy`, `tdz`), then normalized (`sqrtf((x x + y y) + z z)`; a length below
/// `FLT_EPSILON` leaves the mean as it is).
pub fn smooth_surface_normals(mris: &mut Mris, navgs: i32) -> i32 {
    for _ in 0..navgs {
        for vno in 0..nverts(mris) {
            if mris.vertex(vno).ripflag != 0 {
                continue;
            }
            let v = mris.vertex(vno);
            let (mut nx, mut ny, mut nz) = (v.nx, v.ny, v.nz);
            let mut num = 0.0f32;
            for n in neighbours(mris, vno) {
                let vn = mris.vertex(n);
                if vn.ripflag != 0 {
                    continue;
                }
                num = add(num, 1.0);
                nx = add(nx, vn.nx);
                ny = add(ny, vn.ny);
                nz = add(nz, vn.nz);
            }
            num = add(num, 1.0);
            let v = mris.vertex_mut(vno);
            v.tdx = div(nx, num);
            v.tdy = div(ny, num);
            v.tdz = div(nz, num);
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            if v.ripflag != 0 {
                continue;
            }
            let len = sqrt(add(
                add(mul(v.tdx, v.tdx), mul(v.tdy, v.tdy)),
                mul(v.tdz, v.tdz),
            ));
            if len.abs() < f32::EPSILON {
                (v.nx, v.ny, v.nz) = (v.tdx, v.tdy, v.tdz);
            } else {
                v.nx = div(v.tdx, len);
                v.ny = div(v.tdy, len);
                v.nz = div(v.tdz, len);
            }
        }
    }
    NO_ERROR
}

/// `mrisComputeBoundaryNormals(mris)`: nothing (FreeSurfer's body is empty).
pub fn compute_boundary_normals(_mris: &mut Mris) -> i32 {
    NO_ERROR
}

/// `MRIScomputeMeanCurvature(mris)`: the tangent planes recomputed (`mrisComputeTangentPlanes`),
/// then each unripped vertex's `H` the half mean, over its unripped neighbours (the first `vnum`)
/// at a squared tangent-plane distance `u² + v²` of `FLT_EPSILON` or more, of `z / (u² + v²)` (`u`,
/// `v` the neighbour's offset along `e1` and `e2`, `z` along the normal; all in `float`), 0 with
/// none; the extremes (from 10000 and -10000) into `min_curv` and `max_curv`. With `DIAG_SHOW` and
/// `DIAG_VERBOSE`, each ratio above 5 in magnitude prints `<vno> --> <n>: curvature = %2.1f`; with
/// `DIAG_SHOW`, `mean curvature range [%2.3f --> %2.3f]` at the end (standard output).
pub fn compute_mean_curvature(g: &mut Globals, io: &mut Io, mris: &mut Mris) -> i32 {
    compute_tangent_planes(g, io, mris);
    let (mut hmin, mut hmax) = (10000.0f32, -10000.0f32);
    let both = DIAG_SHOW | DIAG_VERBOSE;
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let v = mris.vertex(vno);
        let n = [v.nx, v.ny, v.nz];
        let e1 = [v.e1x, v.e1y, v.e1z];
        let e2 = [v.e2x, v.e2y, v.e2z];
        let p = [v.x, v.y, v.z];
        let mut h = 0.0f32;
        let mut count = 0i32;
        for nb in neighbours(mris, vno) {
            let vn = mris.vertex(nb);
            if vn.ripflag != 0 {
                continue;
            }
            let d = [sub(vn.x, p[0]), sub(vn.y, p[1]), sub(vn.z, p[2])];
            let u = add(add(mul(e1[0], d[0]), mul(e1[1], d[1])), mul(e1[2], d[2]));
            let w = add(add(mul(e2[0], d[0]), mul(e2[1], d[1])), mul(e2[2], d[2]));
            let rsq = add(mul(u, u), mul(w, w));
            if rsq < f32::EPSILON {
                continue;
            }
            let z = add(mul(d[2], n[2]), add(mul(d[1], n[1]), mul(d[0], n[0])));
            let q = div(z, rsq);
            h = add(h, q);
            count += 1;
            if q.abs() > 5.0 && g.gdiag & both == both {
                let _ = writeln!(
                    io.out,
                    "{vno} --> {nb}: curvature = {}",
                    fmt_f(f64::from(q), 2, 1)
                );
            }
        }
        let hv = if count > 0 {
            div(mul(h, 0.5), count as f32)
        } else {
            0.0
        };
        mris.vertex_mut(vno).H = hv;
        if hv > hmax {
            hmax = hv;
        }
        if hv < hmin {
            hmin = hv;
        }
    }
    mris.min_curv = hmin;
    mris.max_curv = hmax;
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(
            io.out,
            "mean curvature range [{} --> {}]",
            fmt_f(f64::from(hmin), 2, 3),
            fmt_f(f64::from(hmax), 2, 3)
        );
    }
    NO_ERROR
}

/// `MRISuseAreaErrors(mris)`: each unripped vertex's curvature set to its faces' area change
/// `(Σ area - Σ orig_area) / n` (the faces' areas and cached original areas, `getFaceNorm`, summed
/// in `float`). FreeSurfer's `n` stays 0 (FS-374): `reproduce` divides by it; the default by the
/// face count (0 for a vertex without faces), noted when the value changes.
pub fn use_area_errors(g: &mut Globals, mris: &mut Mris) -> i32 {
    let reproduce = g.notices.reproduce();
    let mut changed = 0usize;
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let vt = mris.topology(vno);
        let num = usize::from(vt.num);
        let faces: Vec<usize> = vt.f.as_deref().unwrap_or(&[])[..num]
            .iter()
            .map(|&f| f as usize)
            .collect();
        let (mut area, mut orig_area) = (0.0f32, 0.0f32);
        for f in faces {
            area = add(area, mris.face(f).area);
            orig_area = add(orig_area, get_face_norm(mris, f).orig_area);
        }
        let diff = sub(area, orig_area);
        let theirs = div(diff, 0.0);
        let ours = if num > 0 { div(diff, num as f32) } else { 0.0 };
        if theirs.to_bits() != ours.to_bits() {
            changed += 1;
        }
        mris.vertex_mut(vno).curv = if reproduce { theirs } else { ours };
    }
    if changed > 0 {
        g.notices.add(
            &FS_374,
            format!(
                "MRISuseAreaErrors: {changed} vertices' area change divided by their face count (FreeSurfer divides by 0)"
            ),
        );
    }
    NO_ERROR
}

/// The curvature choices that keep the surface's own extremes: each unripped vertex's curvature
/// set to `pick(vertex)`; `min_curv` and `max_curv` from `extremes`.
fn use_field(mris: &mut Mris, pick: impl Fn(&Vertex) -> f32) {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        v.curv = pick(v);
    }
}

/// `MRISuseGaussianCurvature(mris)`: each unripped vertex's curvature set to `K`; the extremes
/// `Kmin`, `Kmax`.
pub fn use_gaussian_curvature(mris: &mut Mris) -> i32 {
    use_field(mris, |v| v.K);
    mris.min_curv = mris.Kmin;
    mris.max_curv = mris.Kmax;
    NO_ERROR
}

/// `MRISuseMeanCurvature(mris)`: each unripped vertex's curvature set to `H`; the extremes
/// `Hmin`, `Hmax`.
pub fn use_mean_curvature(mris: &mut Mris) -> i32 {
    use_field(mris, |v| v.H);
    mris.min_curv = mris.Hmin;
    mris.max_curv = mris.Hmax;
    NO_ERROR
}

/// `MRISusePrincipalCurvature(mris)`: each unripped vertex's curvature set to `k1`; the extremes
/// `Hmin`, `Hmax` (FreeSurfer's choice: the mean curvature's).
pub fn use_principal_curvature(mris: &mut Mris) -> i32 {
    use_field(mris, |v| v.k1);
    mris.min_curv = mris.Hmin;
    mris.max_curv = mris.Hmax;
    NO_ERROR
}

/// `MRISusePrincipalCurvatureFunction(mris, f)` (and `MRISuseK1Curvature`, `MRISuseK2Curvature`:
/// `f` giving `k1`, `k2`): each unripped vertex's curvature set to `f(k1, k2)`, the extremes kept
/// (`curv < min`, `curv > max`). FreeSurfer starts them from the first vertex's previous curvature
/// (FS-375: `reproduce`); the default from the first unripped vertex's new value (the same when
/// no vertex is unripped), noted when the extremes differ. Without vertices (FS-376) both are 0 in
/// both modes, noted.
pub fn use_principal_curvature_function(
    g: &mut Globals,
    mris: &mut Mris,
    f: &dyn Fn(f32, f32) -> f32,
) -> i32 {
    if nverts(mris) == 0 {
        g.notices.add(
            &FS_376,
            "MRISusePrincipalCurvatureFunction: a surface without vertices: the extremes are 0 (FreeSurfer reads a vertex that does not exist)",
        );
        mris.min_curv = 0.0;
        mris.max_curv = 0.0;
        return NO_ERROR;
    }
    let first = mris.vertex(0).curv;
    let (mut tmin, mut tmax) = (first, first);
    let (mut omin, mut omax) = (None::<f32>, None::<f32>);
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        v.curv = f(v.k1, v.k2);
        let c = v.curv;
        if c < tmin {
            tmin = c;
        }
        if c > tmax {
            tmax = c;
        }
        match (omin, omax) {
            (Some(a), Some(b)) => {
                if c < a {
                    omin = Some(c);
                }
                if c > b {
                    omax = Some(c);
                }
            }
            _ => {
                omin = Some(c);
                omax = Some(c);
            }
        }
    }
    let (omin, omax) = (omin.unwrap_or(tmin), omax.unwrap_or(tmax));
    if omin.to_bits() != tmin.to_bits() || omax.to_bits() != tmax.to_bits() {
        g.notices.add(
            &FS_375,
            format!(
                "MRISusePrincipalCurvatureFunction: the curvature's range is [{omin}, {omax}] (FreeSurfer keeps the first vertex's previous curvature: [{tmin}, {tmax}])"
            ),
        );
    }
    if g.notices.reproduce() {
        (mris.min_curv, mris.max_curv) = (tmin, tmax);
    } else {
        (mris.min_curv, mris.max_curv) = (omin, omax);
    }
    NO_ERROR
}

/// `MRISuseK1Curvature(mris)`: [`use_principal_curvature_function`] with `k1`.
pub fn use_k1_curvature(g: &mut Globals, mris: &mut Mris) -> i32 {
    use_principal_curvature_function(g, mris, &|k1, _| k1)
}

/// `MRISuseK2Curvature(mris)`: [`use_principal_curvature_function`] with `k2`.
pub fn use_k2_curvature(g: &mut Globals, mris: &mut Mris) -> i32 {
    use_principal_curvature_function(g, mris, &|_, k2| k2)
}

/// `MRIScomputeCurvatureIndices(mris, &ici, &fi)`: over the unripped vertices, in `double`, the
/// intrinsic curvature index `Σ K area` (vertices with `K > 0`) and the folding index `Σ area |k1|
/// (|k1| - |k2|)`, each divided by `4 pi`; returns `(ici, fi)` (0, 0 without vertices).
pub fn compute_curvature_indices(mris: &Mris) -> (f64, f64) {
    let (mut ici, mut fi) = (0.0f64, 0.0f64);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let area = f64::from(v.area);
        if v.K > 0.0 {
            ici = add64(mul64(f64::from(v.K), area), ici);
        }
        let kmax = f64::from(v.k1).abs();
        let kmin = f64::from(v.k2).abs();
        fi = add64(mul64(mul64(area, kmax), sub64(kmax, kmin)), fi);
    }
    let four_pi = 4.0 * std::f64::consts::PI;
    (div64(ici, four_pi), div64(fi, four_pi))
}

/// `MRISmaxRadius(mris)`: the bounding box of every vertex's position (ripped ones too; from
/// 10000 and -10000, in `double`) into `xlo` ... `zhi` (as `float`), and the largest distance of a
/// vertex from the box's centre `(lo + hi) / 2` (`sqrt((x² + y²) + z²)` in `double`); 0 without
/// vertices.
pub fn max_radius(mris: &mut Mris) -> f64 {
    let (mut lo, mut hi) = ([10000.0f64; 3], [-10000.0f64; 3]);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        for (k, c) in [v.x, v.y, v.z].into_iter().enumerate() {
            let c = f64::from(c);
            if c > hi[k] {
                hi[k] = c;
            }
            if c < lo[k] {
                lo[k] = c;
            }
        }
    }
    (mris.xhi, mris.yhi, mris.zhi) = (hi[0] as f32, hi[1] as f32, hi[2] as f32);
    (mris.xlo, mris.ylo, mris.zlo) = (lo[0] as f32, lo[1] as f32, lo[2] as f32);
    let c: Vec<f64> = (0..3).map(|k| mul64(add64(lo[k], hi[k]), 0.5)).collect();
    let mut radius = 0.0f64;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        let x = sub64(f64::from(v.x), c[0]);
        let y = sub64(f64::from(v.y), c[1]);
        let z = sub64(f64::from(v.z), c[2]);
        let r = sqrt64(add64(add64(mul64(x, x), mul64(y, y)), mul64(z, z)));
        if r > radius {
            radius = r;
        }
    }
    radius
}

/// `MRIScomputeAverageCurvature(mris, psigma)`: the mean of the unripped vertices' curvatures
/// (in `double`; 0 without unripped vertices) and, when `sigma` is given, their standard deviation
/// `sqrt(Σ c² / n - mean²)` (0 without unripped vertices; `sqrt` of a negative variance a NaN with
/// `errno` `EDOM`).
pub fn compute_average_curvature(g: &mut Globals, mris: &Mris, sigma: Option<&mut f64>) -> f64 {
    let (mut nv, mut total, mut total_sq) = (0.0f64, 0.0f64, 0.0f64);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let d = f64::from(v.curv);
        total_sq = add64(total_sq, mul64(d, d));
        total = add64(total, d);
        nv += 1.0;
    }
    let (mean, var) = if nv != 0.0 {
        let mean = div64(total, nv);
        (mean, sub64(div64(total_sq, nv), mul64(mean, mean)))
    } else {
        (0.0, 0.0)
    };
    if let Some(s) = sigma {
        if var < 0.0 {
            g.errno = EDOM;
        }
        *s = sqrt64(var);
    }
    mean
}

/// `MRISmarkNegativeVertices(mris, mark)`: the three vertices of every face with an area below 0
/// marked `mark`.
pub fn mark_negative_vertices(mris: &mut Mris, mark: i32) -> i32 {
    for fno in 0..nfaces(mris) {
        let f = mris.face(fno);
        if f.area < 0.0 {
            let vs = f.v;
            for vno in vs {
                mris.vertex_mut(vno as usize).marked = mark as i16;
            }
        }
    }
    NO_ERROR
}

/// `MRISripNegativeVertices(mris)`: every face with an area below 0 ripped, with its three
/// vertices.
pub fn rip_negative_vertices(mris: &mut Mris) -> i32 {
    for fno in 0..nfaces(mris) {
        if mris.face(fno).area < 0.0 {
            let vs = mris.face(fno).v;
            for vno in vs {
                mris.vertex_mut(vno as usize).ripflag = 1;
            }
            mris.face_mut(fno).ripflag = 1;
        }
    }
    NO_ERROR
}
