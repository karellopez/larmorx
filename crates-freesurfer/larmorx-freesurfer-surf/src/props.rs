// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Queue item 3.1-50, part b (relay 17): the first group of `utils/mrisurf_metricProperties.cpp`'s
//! properties whose dependencies are ported: the common edge of two faces
//! (`FACES_commonEdgeLength_find`, with `VERTICES_commonInFaces_find` of fs.surf's
//! `mrisurf_topology.cpp` taken early), the tangent planes (`mrisComputeTangentPlanes`,
//! `MRISstoreTangentPlanes`), the plane fitted to vertices (`mrisComputeOptimalPlane`), the face
//! centroids, the face-area and vertex-spacing statistics with `MRISprintTessellationStats`, the
//! negative-face counts, the closest vertices, and the curvature edits (`MRISsetCurvature`,
//! `MRIStrinarizeCurvature`, `MRISscaleCurvature`, `MRISbinarizeCurvature`,
//! `MRISthresholdCurvature`, `MRISnormalizeField`, `MRIShistoThresholdCurvature`).
//!
//! Float arithmetic goes through fs.core's `sse` operations (SSE's NaN with the compiled operand
//! order), C++'s `float` overloads (`sqrt` of a `float` is `sqrtss`) as the official build compiles
//! them. Defects (`larmorx_core::defects`; `docs/findings/freesurfer-surfio.md` §19):
//! - **FS-363** (wrong result): `mrisComputeOptimalPlane` sets the centre back to 0 before it fills
//!   the scatter matrix (the plane is fitted through the origin, its centre returned as 0, ripped
//!   vertices included). Fixed by default: the scatter about the unripped vertices' mean, which is
//!   returned; `reproduce` as FreeSurfer.
//! - **FS-364** (memory): `MRIShistoThresholdCurvature` of a surface whose unripped curvatures are
//!   all equal, or one of which is not finite, indexes its histogram far outside its 1,000 bins. By
//!   default such a sample is not counted (noted); `reproduce` ends with a segmentation fault (exit
//!   status 139), as the official build does.
//! - **FS-365** (memory): the same function, with no curvature counted (every vertex ripped, or by
//!   default every sample refused by FS-364), finds no peak and reads the histogram one place
//!   before its start. Both modes clip nothing (noted); FreeSurfer's reads have no effect when no
//!   vertex is unripped, the only way to reach them in `reproduce`.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_363, FS_364, FS_365};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{DIAG_WRITE, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{
    histo_alloc, histo_find_highest_peak_in_region, histo_make_pdf_in_place, histo_plot,
};
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::matrix_decomp::matrix_eigen_system;
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sqrt64, sub, sub64};
use larmorx_freesurfer_core::utils::nint;

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::mp;
use crate::mris::{Face, Mris};
use crate::views::{
    CANONICAL_VERTICES, CURRENT_VERTICES, ORIGINAL_VERTICES, PIAL_VERTICES, TARGET_VERTICES,
    WHITE_VERTICES,
};

/// `NORM_NONE`, `NORM_MAX` (`include/mrisurf.h`).
pub const NORM_NONE: i32 = -1;
pub const NORM_MAX: i32 = 2;

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

fn nfaces(mris: &Mris) -> usize {
    usize::try_from(mris.nfaces).unwrap_or(0)
}

/// `EDOM` (Linux).
const EDOM: i32 = 33;

/// glibc's `sqrt` of a `double`: `errno` `EDOM` for an argument below 0 (a variance that rounding
/// made negative).
fn sqrt64_errno(g: &mut Globals, x: f64) -> f64 {
    if x < 0.0 {
        g.errno = EDOM;
    }
    sqrt64(x)
}

/// `FZERO(f)` with glibc's `FLT_EPSILON`.
fn fzero(x: f32) -> bool {
    x.abs() < f32::EPSILON
}

/// `V3_CROSS_PRODUCT(a, b, c)`: `a.y b.z - a.z b.y`, `a.z b.x - a.x b.z`, `a.x b.y - a.y b.x`,
/// with `mrisComputeTangentPlanes`'s compiled operand orders: the first product as `b.z a.y`, and
/// for `e2` (`second`) the second as `b.y a.z`.
fn v3_cross(a: [f32; 3], b: [f32; 3], second: bool) -> [f32; 3] {
    let p2 = if second {
        mul(b[1], a[2])
    } else {
        mul(a[2], b[1])
    };
    [
        sub(mul(b[2], a[1]), p2),
        sub(mul(a[2], b[0]), mul(a[0], b[2])),
        sub(mul(a[0], b[1]), mul(a[1], b[0])),
    ]
}

/// `V3_NORMALIZE(v, v)` as `mrisComputeTangentPlanes` compiles it: `len = sqrt((y y + x x) + z z)`,
/// 1 when `FZERO(len)`, else `1 / len`; then `x len`, `y len`, `len z`.
fn v3_normalize(v: [f32; 3]) -> [f32; 3] {
    let len = sqrt(add(add(mul(v[1], v[1]), mul(v[0], v[0])), mul(v[2], v[2])));
    let s = if fzero(len) { 1.0 } else { div(1.0, len) };
    [mul(v[0], s), mul(v[1], s), mul(s, v[2])]
}

/// `VERTICES_commonInFaces_find(fi, fj, v)` (`utils/mrisurf_topology.cpp`, fs.surf's, taken early):
/// the vertices of `fi`, in its order, that `fj` also has, as `float`s in a 3-vector of -1s; their
/// number (0 to 3).
pub fn vertices_common_in_faces(fi: &Face, fj: &Face) -> (i32, [f32; 3]) {
    let mut v = [-1.0f32; 3];
    let mut k = 0usize;
    for i in 0..3 {
        if fj.v.contains(&fi.v[i]) {
            v[k] = fi.v[i] as f32;
            k += 1;
        }
    }
    (k as i32, v)
}

/// `FACES_commonEdgeLength_find(mris, fi, fj)`: the length (`V3_LEN` in `float`) of the edge from
/// the first to the second common vertex ([`vertices_common_in_faces`], the vertex numbers through
/// a `float`); other than two common vertices ends the program (`FACES_commonEdgeLength_find: No
/// common edge found! <commonVertices> = <n>`, code -4).
pub fn faces_common_edge_length_find(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    fi: &Face,
    fj: &Face,
) -> Result<f32, Fatal> {
    let (k, v) = vertices_common_in_faces(fi, fj);
    if k != 2 {
        log::error_exit(
            g,
            io.err,
            -4,
            &format!("FACES_commonEdgeLength_find: No common edge found! <commonVertices> = {k}\n"),
        )?;
    }
    let o = mris.vertex(v[0] as i32 as usize);
    let i = mris.vertex(v[1] as i32 as usize);
    // `V3_LEN(I - O)`, compiled as `sqrt(z² + (x² + y²))`
    Ok(fdist([i.x, i.y, i.z], [o.x, o.y, o.z]))
}

/// `mrisComputeTangentPlanes(mris)`: for every vertex (ripped too), with `n` its normal, `e1 = n x
/// (ny, nz, nx)`; when `VectorLen(e1) < 0.001`, `e1 = n x (ny, -nz, nx)` instead; with `e1`'s
/// squared length below 1e-15 and verbose diagnostics, `vertex <vno>: degenerate tangent plane` on
/// standard error; `e2 = n x e1`; both normalized (`V3_NORMALIZE`) into `e1` and `e2`.
pub fn compute_tangent_planes(g: &mut Globals, io: &mut Io, mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        let n = [v.nx, v.ny, v.nz];
        let mut e1 = v3_cross(n, [v.ny, v.nz, v.nx], false);
        // VectorLen: the sum from the first row, `float`
        let len = sqrt(add(
            add(mul(e1[0], e1[0]), mul(e1[1], e1[1])),
            mul(e1[2], e1[2]),
        ));
        if len < 0.001 {
            e1 = v3_cross(n, [v.ny, -v.nz, v.nx], false);
        }
        let len_sq = add(add(mul(e1[0], e1[0]), mul(e1[1], e1[1])), mul(e1[2], e1[2]));
        if f64::from(len_sq).abs() < 1e-15 && verbose_on(g) {
            let _ = writeln!(io.err, "vertex {vno}: degenerate tangent plane");
        }
        let e2 = v3_cross(n, e1, true);
        let e1 = v3_normalize(e1);
        let e2 = v3_normalize(e2);
        let v = mris.vertex_mut(vno);
        (v.e1x, v.e1y, v.e1z) = (e1[0], e1[1], e1[2]);
        (v.e2x, v.e2y, v.e2z) = (e2[0], e2[1], e2[2]);
    }
    NO_ERROR
}

/// `MRISstoreTangentPlanes(mris, which)`: each unripped vertex's `e1` and `e2` copied into `pe1`
/// and `pe2` (`which` unused).
pub fn store_tangent_planes(mris: &mut Mris, _which_vertices: i32) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        (v.pe1x, v.pe1y, v.pe1z) = (v.e1x, v.e1y, v.e1z);
        (v.pe2x, v.pe2y, v.pe2z) = (v.e2x, v.e2y, v.e2z);
    }
    NO_ERROR
}

/// `*MATRIX_RELT(m, r, c) += x` for a `float` element and a `double` `x`.
fn acc(m: &mut Matrix, r: i32, c: i32, x: f64) {
    let e = m.relt_mut(r, c);
    *e = add64(f64::from(*e), x) as f32;
}

/// What [`compute_optimal_plane`] gives: the unit normal and the centre.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Plane {
    pub normal: [f64; 3],
    pub centre: [f64; 3],
}

/// `mrisComputeOptimalPlane(mris, vertices, n, ...)`: with fewer than 4 unripped vertices among
/// `vertices`, the error `mrisComputeOptimalPlane: must specify at least 4 non-ripped verices`
/// (`ERROR_BADPARM`) and `None`. Otherwise the 3 x 3 scatter matrix (each element a `float` to
/// which the `double` product is added) of the vertices' positions about the centre, its
/// eigenvectors (fs.core's `MatrixEigenSystem`), the third column (the smallest eigenvalue's) made
/// unit length in `double`. FS-363: FreeSurfer's centre is 0 and its scatter takes every listed
/// vertex; by default the centre is the unripped vertices' mean (the sum divided by their number)
/// and the scatter takes them only.
pub fn compute_optimal_plane(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    vertices: &[i32],
) -> Result<Option<Plane>, Fatal> {
    let (mut xc, mut yc, mut zc) = (0.0f64, 0.0f64, 0.0f64);
    let mut vnum = 0i32;
    for &vno in vertices {
        let v = mris.vertex(vno as usize);
        if v.ripflag != 0 {
            continue;
        }
        xc = add64(xc, f64::from(v.x));
        yc = add64(yc, f64::from(v.y));
        zc = add64(zc, f64::from(v.z));
        vnum += 1;
    }
    if vnum < 4 {
        log::error_printf(
            g,
            io.err,
            ERROR_BADPARM,
            "mrisComputeOptimalPlane: must specify at least 4 non-ripped verices\n",
        );
        return Ok(None);
    }
    g.notices.add(
        &FS_363,
        "mrisComputeOptimalPlane: the plane is fitted about the vertices' mean (FreeSurfer fits it through the origin and returns the centre 0)",
    );
    let reproduce = g.notices.reproduce();
    if reproduce {
        (xc, yc, zc) = (0.0, 0.0, 0.0);
    } else {
        let n = f64::from(vnum);
        (xc, yc, zc) = (div64(xc, n), div64(yc, n), div64(zc, n));
    }
    let mut m = matrix_alloc(g, io.out, io.err, 3, 3, MATRIX_REAL)?.expect("a 3 x 3 matrix");
    for &vno in vertices {
        let v = mris.vertex(vno as usize);
        if !reproduce && v.ripflag != 0 {
            continue;
        }
        let dx = sub64(f64::from(v.x), xc);
        let dy = sub64(f64::from(v.y), yc);
        let dz = sub64(f64::from(v.z), zc);
        acc(&mut m, 1, 1, mul64(dx, dx));
        acc(&mut m, 2, 2, mul64(dy, dy));
        acc(&mut m, 3, 3, mul64(dz, dz));
        acc(&mut m, 2, 1, mul64(dx, dy));
        acc(&mut m, 1, 2, mul64(dx, dy));
        acc(&mut m, 3, 1, mul64(dx, dz));
        acc(&mut m, 1, 3, mul64(dx, dz));
        acc(&mut m, 3, 2, mul64(dy, dz));
        acc(&mut m, 2, 3, mul64(dy, dz));
    }
    let mut evalues = [0.0f32; 3];
    let mut ev = matrix_alloc(g, io.out, io.err, 3, 3, MATRIX_REAL)?.expect("a 3 x 3 matrix");
    matrix_eigen_system(g, io.out, io.err, &m, &mut evalues, Some(&mut ev))?;
    let (a, b, c) = (
        f64::from(ev.relt(1, 3)),
        f64::from(ev.relt(2, 3)),
        f64::from(ev.relt(3, 3)),
    );
    let norm = sqrt64(add64(add64(mul64(a, a), mul64(b, b)), mul64(c, c)));
    Ok(Some(Plane {
        normal: [div64(a, norm), div64(b, norm), div64(c, norm)],
        centre: [xc, yc, zc],
    }))
}

/// Which positions a face centroid takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Centroid {
    /// `mrisCalculateFaceCentroid`: the current positions.
    Current,
    /// `mrisCalculateCanonicalFaceCentroid`: the canonical (`cx`, `cy`, `cz`).
    Canonical,
    /// `mrisCalculateOriginalFaceCentroid` (file-local, unused in FreeSurfer 8.2.0): the original.
    Original,
}

/// The face centroids (`mrisCalculateFaceCentroid` and its two siblings): with the face's vertices
/// `v0`, `v1`, `v2`, the midpoint `m = (p1 + p2) / 2`, then `(p0 + m) / 2`, in `float`.
pub fn face_centroid(mris: &Mris, fno: usize, which: Centroid) -> [f32; 3] {
    let f = mris.face(fno);
    let p = |k: usize| {
        let v = mris.vertex(f.v[k] as usize);
        match which {
            Centroid::Current => [v.x, v.y, v.z],
            Centroid::Canonical => [v.cx, v.cy, v.cz],
            Centroid::Original => [v.origx, v.origy, v.origz],
        }
    };
    let (p0, p1, p2) = (p(0), p(1), p(2));
    let mut r = [0.0f32; 3];
    for k in 0..3 {
        // compiled as `(m + p0) * 0.5f` (the same value as the division)
        let m = div(add(p1[k], p2[k]), 2.0);
        r[k] = div(add(m, p0[k]), 2.0);
    }
    r
}

/// What the statistics functions give: the mean, the standard deviation, the smallest and largest,
/// and (vertex spacing) the vertex and neighbour of the largest (`None`: not written, no distance
/// exceeding -1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub mean: f64,
    pub sigma: f64,
    pub min: f64,
    pub max: f64,
    pub vno: Option<i32>,
    pub vno2: Option<i32>,
}

/// `MRIScomputeFaceAreaStats(mris, &sigma, &min, &max)`: the metric properties first
/// (`MRIScomputeMetricProperties`); with `s = orig_area / total_area` (in `float`; 1 for a patch),
/// over the unripped faces each area `s * area` (`double`): the mean (sum over count: 0/0 a NaN),
/// `sqrt(sum of squares / count - mean²)`, the smallest (from 1000) and largest (from -1).
pub fn compute_face_area_stats(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<Stats, Fatal> {
    mp::compute_metric_properties(g, sg, io, mris)?;
    let area_scale = if mris.patch != 0 {
        1.0
    } else {
        f64::from(div(mris.orig_area, mris.total_area))
    };
    let (mut min_area, mut max_area) = (1000.0f64, -1.0f64);
    let (mut var, mut nf, mut total) = (0.0f64, 0.0f64, 0.0f64);
    for fno in 0..nfaces(mris) {
        let f = mris.face(fno);
        if f.ripflag != 0 {
            continue;
        }
        let area = mul64(f64::from(f.area), area_scale);
        total = add64(total, area);
        nf = add64(nf, 1.0);
        var = add64(var, mul64(area, area));
        if area > max_area {
            max_area = area;
        }
        if area < min_area {
            min_area = area;
        }
    }
    let mean = div64(total, nf);
    let sigma = sqrt64_errno(g, sub64(div64(var, nf), mul64(mean, mean)));
    Ok(Stats {
        mean,
        sigma,
        min: min_area,
        max: max_area,
        vno: None,
        vno2: None,
    })
}

/// The squared distance `SQR(a.x - b.x) + SQR(a.y - b.y) + SQR(a.z - b.z)` in `float`, compiled
/// as `dz² + (dx² + dy²)` (every distance of this file), its root (`sqrtss`).
fn fdist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (dx, dy, dz) = (sub(a[0], b[0]), sub(a[1], b[1]), sub(a[2], b[2]));
    sqrt(add(mul(dz, dz), add(mul(dx, dx), mul(dy, dy))))
}

/// The statistics over the unripped vertices' first `count(vertex)` neighbours: the distance
/// `dist(vertex, neighbour)` widened to `double`; the largest (from -1) records the vertex and the
/// neighbour.
/// The vertex-spacing statistics' dead scale `sqrt(orig_area / total_area)` (not for a patch):
/// overwritten by 1, but its `sqrtf` still sets `errno` to `EDOM` for a negative ratio.
fn dead_dist_scale(g: &mut Globals, mris: &Mris) {
    if mris.patch == 0 && div(mris.orig_area, mris.total_area) < 0.0 {
        g.errno = EDOM;
    }
}

fn spacing_stats(
    g: &mut Globals,
    mris: &Mris,
    count: impl Fn(&Mris, usize) -> usize,
    mut dist: impl FnMut(&mut Globals, &Mris, usize, usize) -> Result<f64, Fatal>,
) -> Result<Stats, Fatal> {
    let (mut min_dist, mut max_dist) = (1000.0f64, -1.0f64);
    let (mut var, mut nv, mut total) = (0.0f64, 0.0f64, 0.0f64);
    let (mut vno_max, mut vno2_max) = (None, None);
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        for n in 0..count(mris, vno) {
            let vn = mris.topology(vno).v.as_ref().expect("a neighbour list")[n];
            nv = add64(nv, 1.0);
            // dist_scale is 1.0f in both functions
            let d = mul64(dist(g, mris, vno, vn as usize)?, 1.0);
            if d > max_dist {
                vno_max = Some(vno as i32);
                vno2_max = Some(vn);
                max_dist = d;
            }
            if d < min_dist {
                min_dist = d;
            }
            total = add64(total, d);
            var = add64(var, mul64(d, d));
        }
    }
    let mean = div64(total, nv);
    Ok(Stats {
        mean,
        sigma: sqrt64_errno(g, sub64(div64(var, nv), mul64(mean, mean))),
        min: min_dist,
        max: max_dist,
        vno: vno_max,
        vno2: vno2_max,
    })
}

/// `MRIScomputeVertexSpacingStats(mris, ..., which)`: the metric properties first; over each
/// unripped vertex's 1-hop neighbours (`vnum`), the `float` distance in the positions `which`
/// names (original, current, white, pial, target; any other ends the program at the first
/// neighbour: `MRIScomputeVertexSpacingStats: unsupported vertex set <which>`, `ERROR_BADPARM`); the
/// mean, `sqrt(sum of squares / n - mean²)`, smallest (from 1000), largest (from -1) with its vertex
/// and neighbour (`vno`, `vno2`: 0 when no distance exceeds -1). The scale computed from the areas
/// is overwritten by 1 (as FreeSurfer).
pub fn compute_vertex_spacing_stats(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    which: i32,
) -> Result<Stats, Fatal> {
    mp::compute_metric_properties(g, sg, io, mris)?;
    dead_dist_scale(g, mris);
    let err = &mut *io.err;
    spacing_stats(
        g,
        mris,
        |m, vno| usize::try_from(m.topology(vno).vnum).unwrap_or(0),
        |g, m, a, b| {
            let (v, vn) = (m.vertex(a), m.vertex(b));
            let d = match which {
                ORIGINAL_VERTICES => {
                    fdist([vn.origx, vn.origy, vn.origz], [v.origx, v.origy, v.origz])
                }
                CURRENT_VERTICES => fdist([vn.x, vn.y, vn.z], [v.x, v.y, v.z]),
                WHITE_VERTICES => fdist(
                    [vn.whitex, vn.whitey, vn.whitez],
                    [v.whitex, v.whitey, v.whitez],
                ),
                PIAL_VERTICES => fdist([vn.pialx, vn.pialy, vn.pialz], [v.pialx, v.pialy, v.pialz]),
                TARGET_VERTICES => {
                    fdist([vn.targx, vn.targy, vn.targz], [v.targx, v.targy, v.targz])
                }
                _ => {
                    log::error_exit(
                        g,
                        err,
                        ERROR_BADPARM,
                        &format!("MRIScomputeVertexSpacingStats: unsupported vertex set {which}"),
                    )?;
                    0.0
                }
            };
            Ok(f64::from(d))
        },
    )
}

/// `MRIScomputeTotalVertexSpacingStats(mris, ...)`: as [`compute_vertex_spacing_stats`] with the
/// current positions over every neighbour in use (`vtotal`).
pub fn compute_total_vertex_spacing_stats(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<Stats, Fatal> {
    mp::compute_metric_properties(g, sg, io, mris)?;
    dead_dist_scale(g, mris);
    spacing_stats(
        g,
        mris,
        |m, vno| usize::try_from(m.topology(vno).vtotal).unwrap_or(0),
        |_, m, a, b| {
            let (v, vn) = (m.vertex(a), m.vertex(b));
            Ok(f64::from(fdist([vn.x, vn.y, vn.z], [v.x, v.y, v.z])))
        },
    )
}

/// `MRIScountNegativeFaces(mris)` and `MRIScountNegativeTriangles(mris)`: the unripped faces whose
/// area is below 0.
pub fn count_negative_faces(mris: &Mris) -> i32 {
    (0..nfaces(mris))
        .filter(|&f| mris.face(f).ripflag == 0 && mris.face(f).area < 0.0)
        .count() as i32
}

/// `MRISprintTessellationStats(mris, fp)`: [`compute_vertex_spacing_stats`] of the current
/// positions, then into `fp` `vertex spacing <mean> +- <sigma> (<min>--><max>) (max @ vno <vno> -->
/// <vno2>)` (each `%2.2f`); [`compute_face_area_stats`] and `face area <mean> +- <sigma>
/// (<min>--><max>)`; when the largest face area exceeds 20, each 1-hop neighbour of the vertex
/// `vno` farther than 20 (the `float` distance) on standard output: a tab, `<vno> --> <n> = <d>
/// mm` (`%2.1f`).
pub fn print_tessellation_stats(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    fp: &mut dyn Write,
) -> Result<i32, Fatal> {
    let s = compute_vertex_spacing_stats(g, sg, io, mris, CURRENT_VERTICES)?;
    // the caller's `vno = vno2 = 0`
    let (svno, svno2) = (s.vno.unwrap_or(0), s.vno2.unwrap_or(0));
    let f2 = |x: f64| fmt_f(x, 2, 2);
    let _ = writeln!(
        fp,
        "vertex spacing {} +- {} ({}-->{}) (max @ vno {} --> {})",
        f2(s.mean),
        f2(s.sigma),
        f2(s.min),
        f2(s.max),
        svno,
        svno2
    );
    let a = compute_face_area_stats(g, sg, io, mris)?;
    let _ = writeln!(
        fp,
        "face area {} +- {} ({}-->{})",
        f2(a.mean),
        f2(a.sigma),
        f2(a.min),
        f2(a.max)
    );
    if a.max > 20.0 {
        let vno = svno as usize;
        let vt = mris.topology(vno);
        for n in 0..usize::try_from(vt.vnum).unwrap_or(0) {
            let vn = vt.v.as_ref().expect("a neighbour list")[n];
            let (v, w) = (mris.vertex(vno), mris.vertex(vn as usize));
            let dist = fdist([w.x, w.y, w.z], [v.x, v.y, v.z]);
            if dist > 20.0 {
                let _ = writeln!(
                    io.out,
                    "\t{} --> {} = {} mm",
                    vno,
                    vn,
                    fmt_f(f64::from(dist), 2, 1)
                );
            }
        }
    }
    Ok(NO_ERROR)
}

/// The closest-vertex searches: over the unripped vertices in order, the `float` distance
/// `sqrt(dx² + dy² + dz²)` from `(x, y, z)` with `d = position - point`; the first strictly
/// closest from 10000. `MRISfindClosestVertex(mris, x, y, z, &dmin, which)` takes the current
/// positions for `CURRENT_VERTICES`, the canonical for `CANONICAL_VERTICES`; any other ends the
/// program at the first unripped vertex (`MRISfindClosestVertex: unsupported vertex set <which>`,
/// `ERROR_UNSUPPORTED`). Returns the vertex (-1 for none) and the distance (10000 for none).
pub fn find_closest_vertex(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    p: [f32; 3],
    which: i32,
) -> Result<(i32, f32), Fatal> {
    let (mut min_v, mut min_d) = (-1i32, 10000.0f32);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let q = match which {
            CURRENT_VERTICES => [v.x, v.y, v.z],
            CANONICAL_VERTICES => [v.cx, v.cy, v.cz],
            _ => {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_UNSUPPORTED,
                    &format!("MRISfindClosestVertex: unsupported vertex set {which}\n"),
                )?;
                [v.x, v.y, v.z]
            }
        };
        let d = fdist(q, p);
        if d < min_d {
            min_d = d;
            min_v = vno as i32;
        }
    }
    Ok((min_v, min_d))
}

/// Which positions [`find_closest_in`] searches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Positions {
    /// `MRISfindClosestOriginalVertex`.
    Original,
    /// `MRISfindClosestCanonicalVertex`.
    Canonical,
    /// `MRISfindClosestWhiteVertex`.
    White,
}

/// `MRISfindClosestOriginalVertex(mris, x, y, z)`, `MRISfindClosestCanonicalVertex` and
/// `MRISfindClosestWhiteVertex`: as [`find_closest_vertex`] in those positions; the vertex only.
pub fn find_closest_in(mris: &Mris, p: [f32; 3], which: Positions) -> i32 {
    let (mut min_v, mut min_d) = (-1i32, 10000.0f32);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let q = match which {
            Positions::Original => [v.origx, v.origy, v.origz],
            Positions::Canonical => [v.cx, v.cy, v.cz],
            Positions::White => [v.whitex, v.whitey, v.whitez],
        };
        let d = fdist(q, p);
        if d < min_d {
            min_d = d;
            min_v = vno as i32;
        }
    }
    min_v
}

// ---- the curvature edits -----------------------------------------------------------------------

/// `MRISsetCurvature(mris, val)`: every unripped vertex's curvature `val`.
pub fn set_curvature(mris: &mut Mris, val: f32) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            v.curv = val;
        }
    }
    NO_ERROR
}

/// `MRIStrinarizeCurvature(mris, t)`: each unripped curvature below `-t` becomes -1, above `t` 1,
/// any other (NaN too) 0.
pub fn trinarize_curvature(mris: &mut Mris, t: f32) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        v.curv = if v.curv < -t {
            -1.0
        } else if v.curv > t {
            1.0
        } else {
            0.0
        };
    }
    NO_ERROR
}

/// `MRISscaleCurvature(mris, scale)`: the unripped curvatures' mean `m` (`double` sum divided by
/// their number), then each `(curv - m) * scale + m` in `double`, stored as `float`.
pub fn scale_curvature(mris: &mut Mris, scale: f32) -> i32 {
    let (mut mean, mut n) = (0.0f64, 0i32);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        n += 1;
        mean = add64(mean, f64::from(v.curv));
    }
    mean = div64(mean, f64::from(n));
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        v.curv = add64(
            mul64(sub64(f64::from(v.curv), mean), f64::from(scale)),
            mean,
        ) as f32;
    }
    NO_ERROR
}

/// `MRISbinarizeCurvature(mris, thresh, low, high, use_abs)`: for each unripped vertex, its
/// curvature (its absolute value with `use_abs`) below `thresh` gives `low`, else `-high` for a
/// negative curvature and `high` otherwise.
pub fn binarize_curvature(mris: &mut Mris, thresh: f32, low: f32, high: f32, use_abs: bool) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        let val = if use_abs { v.curv.abs() } else { v.curv };
        v.curv = if val < thresh {
            low
        } else if v.curv < 0.0 {
            -high
        } else {
            high
        };
    }
    NO_ERROR
}

/// `MRISthresholdCurvature(mris, thresh, use_abs)`: an unripped curvature (its absolute value with
/// `use_abs`) below `thresh` becomes 0.
pub fn threshold_curvature(mris: &mut Mris, thresh: f32, use_abs: bool) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        let val = if use_abs { v.curv.abs() } else { v.curv };
        if val < thresh {
            v.curv = 0.0;
        }
    }
    NO_ERROR
}

/// `MRISnormalizeField(mris, distance_field, which_norm)`: nothing for `NORM_NONE`; for a distance
/// field or `NORM_MAX`, the unripped curvatures' largest value above 0 (from 0); unless it is 0 to
/// within `FLT_EPSILON`, `normalizing max <max> to be in [0 1]` (`%2.1f`) on standard output and
/// each unripped curvature divided by it; otherwise `MRISnormalizeCurvature(mris, which_norm)`.
pub fn normalize_field(io: &mut Io, mris: &mut Mris, distance_field: bool, which_norm: i32) {
    if which_norm == NORM_NONE {
        return;
    }
    if distance_field || which_norm == NORM_MAX {
        let mut max_value = 0.0f32;
        for vno in 0..nverts(mris) {
            let v = mris.vertex(vno);
            if v.ripflag == 0 && v.curv > max_value {
                max_value = v.curv;
            }
        }
        if !fzero(max_value) {
            let _ = writeln!(
                io.out,
                "normalizing max {} to be in [0 1]",
                fmt_f(f64::from(max_value), 2, 1)
            );
            for vno in 0..nverts(mris) {
                let v = mris.vertex_mut(vno);
                if v.ripflag == 0 {
                    v.curv = div(v.curv, max_value);
                }
            }
        }
    } else {
        crate::metric::normalize_curvature(io, mris, which_norm);
    }
}

/// `MRIShistoThresholdCurvature(mris, thresh_pct)`: the unripped curvatures' range (`double`, from
/// 1000 and -1000); a histogram of 1,000 bins, `scale = 999 / (max - min)`, `bin_size = 1 / scale`
/// (`float`), each unripped vertex counted at `nint(scale * (curv - min))` (FS-364 outside the
/// bins), bin `b`'s value `b / scale + min`; normalized to a probability (`HISTOmakePDF`); with
/// `DIAG_WRITE` and verbose diagnostics `curv.plt` written in `dir`; from the highest peak
/// (`HISTOfindHighestPeakInRegion` over all bins) upwards the total `T` of the counts from it, and
/// the first bin (to the last but one) at which the running total exceeds `thresh_pct * T` gives
/// the upper limit; downwards (to bin 1) likewise the lower limit. Each unripped curvature above
/// the upper limit or below the lower becomes that limit (`float`), counted; `min_curv` and
/// `max_curv` the new range (from 10000 and -10000). With any changed, `<n> vertices thresholded to
/// be in [<low> <high>]` (`%2.2f`) on standard error.
pub fn histo_threshold_curvature(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    thresh_pct: f32,
) -> Result<i32, Fatal> {
    let mut h = histo_alloc(g, io.err, 1000)?;
    let (mut min_curv, mut max_curv) = (1000.0f64, -1000.0f64);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        if f64::from(v.curv) > max_curv {
            max_curv = f64::from(v.curv);
        }
        if f64::from(v.curv) < min_curv {
            min_curv = f64::from(v.curv);
        }
    }
    let nbins = h.nbins;
    let curv_scale = div64(f64::from(nbins - 1), sub64(max_curv, min_curv));
    h.bin_size = div64(1.0, curv_scale) as f32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let bin = nint(g, mul64(curv_scale, sub64(f64::from(v.curv), min_curv)));
        if !(0..nbins).contains(&bin) {
            g.notices.add(
                &FS_364,
                format!("MRIShistoThresholdCurvature: vertex {vno}'s curvature falls in bin {bin}, outside the histogram's 1,000 (not counted; FreeSurfer writes outside it)"),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
            continue;
        }
        let c = &mut h.counts[bin as usize];
        *c = add(*c, 1.0);
    }
    for bin in 0..nbins {
        h.bins[bin as usize] = add64(div64(f64::from(bin), curv_scale), min_curv) as f32;
    }
    // the first zbin, overwritten (its conversion notes FS-12 for a NaN)
    let _ = nint(g, mul64(curv_scale, sub64(0.0, min_curv)));
    histo_make_pdf_in_place(&mut h);
    if g.gdiag & DIAG_WRITE != 0 && verbose_on(g) {
        histo_plot(&h, dir, "curv.plt");
    }
    let zbin = histo_find_highest_peak_in_region(&h, 0, nbins);
    if zbin < 0 {
        // no count above 0: nothing is clipped (the limits are the measured range)
        g.notices.add(
            &FS_365,
            "MRIShistoThresholdCurvature: no curvature counted, so no peak (FreeSurfer reads the histogram one place before its start; no unripped vertex for its limits)",
        );
        clip_curvature(io, mris, min_curv, max_curv);
        return Ok(NO_ERROR);
    }
    let mut total = 0.0f64;
    let mut bin = zbin;
    while bin < nbins {
        total = add64(total, f64::from(h.counts[bin as usize]));
        bin += 1;
    }
    let mut thresh = mul64(f64::from(thresh_pct), total);
    total = 0.0;
    bin = zbin;
    while bin < nbins - 1 {
        total = add64(total, f64::from(h.counts[bin as usize]));
        if total > thresh {
            break;
        }
        bin += 1;
    }
    max_curv = f64::from(h.bins[bin as usize]);
    total = 0.0;
    bin = zbin;
    while bin > 0 {
        total = add64(total, f64::from(h.counts[bin as usize]));
        bin -= 1;
    }
    thresh = mul64(f64::from(thresh_pct), total);
    total = 0.0;
    bin = zbin;
    while bin > 0 {
        total = add64(total, f64::from(h.counts[bin as usize]));
        if total > thresh {
            break;
        }
        bin -= 1;
    }
    min_curv = f64::from(h.bins[bin as usize]);
    clip_curvature(io, mris, min_curv, max_curv);
    Ok(NO_ERROR)
}

/// `MRIShistoThresholdCurvature`'s last step: each unripped curvature above `max_curv` or below
/// `min_curv` set to it (counted), the surface's range from the result, the message.
fn clip_curvature(io: &mut Io, mris: &mut Mris, min_curv: f64, max_curv: f64) {
    mris.min_curv = 10000.0;
    mris.max_curv = -10000.0;
    let mut nthresh = 0;
    for vno in 0..nverts(mris) {
        let (lo, hi) = (mris.min_curv, mris.max_curv);
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        if f64::from(v.curv) > max_curv {
            v.curv = max_curv as f32;
            nthresh += 1;
        }
        if f64::from(v.curv) < min_curv {
            v.curv = min_curv as f32;
            nthresh += 1;
        }
        let c = v.curv;
        if c < lo {
            mris.min_curv = c;
        }
        if c > hi {
            mris.max_curv = c;
        }
    }
    if nthresh > 0 {
        let _ = writeln!(
            io.err,
            "{nthresh} vertices thresholded to be in [{} {}]",
            fmt_f(min_curv, 2, 2),
            fmt_f(max_curv, 2, 2)
        );
    }
}
