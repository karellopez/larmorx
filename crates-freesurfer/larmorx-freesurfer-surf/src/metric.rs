// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Averaging, the sphere's centre, the neighbour distances, the areas and the curvature
//! operations (`fs.surfio`, queue item 3.1-49, parts b and c): FreeSurfer 8.2.0's
//! `utils/mrisurf_metricProperties.cpp` lines 1007-1842 and 1884-3165 without what computes the
//! metric properties, reads the face-normal cache, or needs a transform, a parameterization or the
//! saved vertex positions: `MRISaverageVertexPositions` (with its two workers),
//! `MRISsmoothSurface`'s averaging ([`smooth_surface_positions`]), `MRIScenterSphere`,
//! `MRISclearOrigDistances`, `MRISscaleDistances`, `mrisCheckDist`, `mrisCheckDistOrig`,
//! `mrisComputeVertexDistances`, `mrisComputeOriginalVertexDistances` and
//! `mrisComputeOriginalVertexDistancesIfNecessaryWkr` (both workers from
//! `utils/mrisComputeVertexDistancesWkr_extracted.h`), `MRISpercentDistanceError`,
//! `MRISclearOrigArea`, `MRISclearOrigAreaAndVal2`, `mrisComputeArea`, `MRISzeroNegativeAreas`,
//! `load_orig_triangle_vertices`, `load_triangle_vertices`, `MRISextractVertexCoords`,
//! `MRISclearCurvature`, `MRISclearCurvAndVal2`, `MRISrectifyCurvature`, `MRISscaleCurvatures`,
//! `MRISsmoothCurvatures`, `MRISzeroMeanCurvature`, `MRISnormalizeCurvature`,
//! `MRISnormalizeCurvatureVariance`, `mrisCountCompressed`, `MRISstoreMeanCurvature`,
//! `MRISsaveXYZ`, `MRISloadXYZ`, `MRISfreeXYZ`, `MRISpopXYZ`; with `mrisComputeCurvatureMinMax`
//! (lines 9113-9136, queue item 3.1-50's) and `utils/mrisurf_vals.cpp`'s
//! `MRISextractCurvatureVector` (lines 677-685); since 3.1-50's part a also `MRIScomputeOrigArea`,
//! `MRISsetOrigArea`, `MRISrestoreOldPositions`, `MRISstoreCurrentPositions`,
//! `MRISstoreMetricProperties` and `MRISrestoreMetricProperties`, which wait for the face-normal
//! cache ([`facenorm`]) and the saved vertex positions ([`positions`]).
//!
//! Every floating-point operation that can make a NaN, or meet two, goes through fs.core's
//! [`sse`] helpers in the official build's operand order (read from the unit oracle's object
//! code where GCC 8.4 swaps a commutative operation's operands, `docs/findings/freesurfer-surfio.md`
//! §9), so the NaNs are the official build's on every platform.
//!
//! Not here yet: `MRIScomputeMetricProperties` (it runs `utils/mrisurf_metricProperties_fast.h`
//! and `_slow.h`, queue items 3.1-51 and 3.1-52), so `MRISsmoothSurface`'s last step,
//! `MRISrecenter` and `MRISscaleBrainArea`; `MRISltaMultiply`, `MRIStransform` (an `LTA`, a `TRANSFORM`: fs.volume); `MRISnonmaxSuppress`
//! (a spherical parameterization: fs.surf).

// The loops index as FreeSurfer's C does (one vertex number for several arrays).
#![allow(clippy::needless_range_loop)]

use larmorx_core::defects::FS_114;
use larmorx_freesurfer_core::diag::{self, DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::matrix_ops::xyz_normalized_load;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::{LogicProblemResponse, cope_with_logic_problem2};

use crate::aaa::{self, DistanceFormula, mris_sort_compare_float};
use crate::coords::{self, Io, Xyz, metric_properties_cpp};
use crate::dist;
use crate::facenorm;
use crate::mris::Mris;
use crate::positions;
use crate::topology;
use crate::views::{
    CANONICAL_VERTICES, CURRENT_VERTICES, FLATTENED_VERTICES, INFLATED_VERTICES, LAYERIV_VERTICES,
    ORIGINAL_VERTICES, PIAL_VERTICES, TARGET_VERTICES, TMP_VERTICES, TMP2_VERTICES, WHITE_VERTICES,
};

/// `NORM_NONE`, `NORM_MEAN`, `NORM_MEDIAN` (`include/mrisurf.h`).
pub const NORM_NONE: i32 = -1;
pub const NORM_MEAN: i32 = 0;
pub const NORM_MEDIAN: i32 = 1;

/// `DEFAULT_RADIUS` (`include/mrisurf.h`, a `float`).
pub const DEFAULT_RADIUS: f32 = 100.0;

/// `FLT_EPSILON`: `FZERO(f)` is `fabs(f) < FLT_EPSILON` (`include/macros.h`).
const FLT_EPSILON: f32 = f32::EPSILON;

fn fzero(x: f64) -> bool {
    x.abs() < f64::from(FLT_EPSILON)
}

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

fn nfaces(mris: &Mris) -> usize {
    usize::try_from(mris.nfaces).unwrap_or(0)
}

/// `utils/mrisComputeVertexDistancesWkr_extracted.h` under the official build's root.
fn extracted_h() -> String {
    format!(
        "{}/utils/mrisComputeVertexDistancesWkr_extracted.h",
        diag::BUILD_ROOT
    )
}

/// A neighbour's index as a vertex number (FreeSurfer reads `vertices[vt->v[n]]` unchecked).
fn nbr(mris: &Mris, vno: usize, n: usize) -> usize {
    let v = mris
        .topology(vno)
        .v
        .as_ref()
        .expect("VERTEX_TOPOLOGY::v is NULL")[n];
    usize::try_from(v).expect("a neighbour index outside the surface")
}

// ---- averaging ---------------------------------------------------------------------------------

/// `MRISaverageVertexPositionsWkr_part1(mris, navgs, scale, includeCenter, 0, 1, ...)`: frees
/// the distances (not the original ones), exports the current coordinates and averages them
/// `navgs` times: each vertex's new position is the mean of its own (with `include_center`) and
/// its unripped 1-hop neighbours' positions (none for a ripped vertex), in `float`, divided by
/// their count (0 without the centre: `0 / 0`, a NaN); with `scale` other than 1 the vertex moves
/// `scale` of the way there (`p + scale (mean - p)`). Returns the coordinates.
fn average_wkr_part1(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    navgs: i32,
    scale: f32,
    include_center: bool,
) -> Result<Xyz, Fatal> {
    dist::free_dists_but_not_orig(mris);
    let (mut px, mut py, mut pz) = coords::export_xyz(g, io, mris)?;
    let n = nverts(mris);
    let (mut qx, mut qy, mut qz) = (vec![0.0f32; n], vec![0.0f32; n], vec![0.0f32; n]);
    let mut navgs = navgs;
    while navgs > 0 {
        navgs -= 1;
        for vno in 0..n {
            let (mut x, mut y, mut z, mut count) = if include_center {
                (px[vno], py[vno], pz[vno], 1i32)
            } else {
                (0.0f32, 0.0f32, 0.0f32, 0i32)
            };
            let vnum = usize::try_from(mris.topology(vno).vnum).unwrap_or(0);
            let ripped = mris.vertex(vno).ripflag != 0;
            if include_center && (ripped || vnum == 0) {
                // the official build stores the vertex's own position undivided here (its count
                // is known to be 1): a signalling NaN stays unquieted
                (qx[vno], qy[vno], qz[vno]) = (x, y, z);
                continue;
            }
            if !ripped {
                for k in 0..vnum {
                    let vno2 = nbr(mris, vno, k);
                    if mris.vertex(vno2).ripflag != 0 {
                        continue;
                    }
                    count += 1;
                    x = sse::add(x, px[vno2]);
                    y = sse::add(y, py[vno2]);
                    z = sse::add(z, pz[vno2]);
                }
            }
            let c = count as f32;
            x = sse::div(x, c);
            y = sse::div(y, c);
            z = sse::div(z, c);
            if scale == 1.0 {
                (qx[vno], qy[vno], qz[vno]) = (x, y, z);
            } else {
                // `p + scale * (x - p)` as `(x - p) * scale + p`
                qx[vno] = sse::add(sse::mul(sse::sub(x, px[vno]), scale), px[vno]);
                qy[vno] = sse::add(sse::mul(sse::sub(y, py[vno]), scale), py[vno]);
                qz[vno] = sse::add(sse::mul(sse::sub(z, pz[vno]), scale), pz[vno]);
            }
        }
        std::mem::swap(&mut px, &mut qx);
        std::mem::swap(&mut py, &mut qy);
        std::mem::swap(&mut pz, &mut qz);
    }
    Ok((px, py, pz))
}

/// `MRISaverageVertexPositions(mris, navgs)`: the current coordinates replaced by `navgs` rounds
/// of averaging with each vertex's unripped 1-hop neighbours (the vertex itself included), with
/// the bounding box and centre (`MRISimportXYZ`); the distances (not the original ones) freed.
/// Returns `NO_ERROR`.
pub fn average_vertex_positions(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    navgs: i32,
) -> Result<i32, Fatal> {
    let (px, py, pz) = average_wkr_part1(g, io, mris, navgs, 1.0, true)?;
    coords::import_xyz(g, io, mris, &px, &py, &pz)?;
    Ok(0)
}

/// `MRISsmoothSurface(mris, niter, step)` up to its last step, `MRIScomputeMetricProperties`
/// (queue items 3.1-51 and 3.1-52; the caller computes them): the logic problem
/// `MRISsmoothSurface was calling MRIScomputeMetricProperties too soon` (line 1172, fixed by
/// `FREESURFER_fix_MRISsmoothSurface1`; it changes nothing), the step cut to 1, then `niter`
/// rounds that move each vertex `step` of the way to the mean of its unripped 1-hop neighbours
/// (itself not included: a ripped vertex, or one with none, gets `p + step (NaN - p)`), with the
/// bounding box and centre; the distances (not the original ones) freed.
pub fn smooth_surface_positions(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    niter: i32,
    step: f32,
) -> Result<(), Fatal> {
    cope_with_logic_problem2(
        g,
        io.env,
        io.out,
        io.err,
        Some("FREESURFER_fix_MRISsmoothSurface1"),
        "MRISsmoothSurface was calling MRIScomputeMetricProperties too soon",
        &metric_properties_cpp(),
        1172,
        "MRISsmoothSurface",
    )?;
    let step = if step > 1.0 { 1.0 } else { step };
    let (px, py, pz) = average_wkr_part1(g, io, mris, niter, step, false)?;
    coords::import_xyz(g, io, mris, &px, &py, &pz)
}

// ---- the sphere's centre -----------------------------------------------------------------------

fn sqr(x: f64) -> f64 {
    sse::mul64(x, x)
}

/// `SQR(x - cx) + SQR(y - cy) + SQR(z - cz)` of a vertex (in `double`): `(x² + y²) + z²`, or with
/// `y_first` `(y² + x²) + z²` (the official build's order in `estimateNRG`'s inlined copies).
#[allow(clippy::too_many_arguments)]
fn r2_of(x: f32, y: f32, z: f32, cx: f64, cy: f64, cz: f64, y_first: bool) -> f64 {
    let (a, b) = (
        sqr(sse::sub64(f64::from(x), cx)),
        sqr(sse::sub64(f64::from(y), cy)),
    );
    let ab = if y_first {
        sse::add64(b, a)
    } else {
        sse::add64(a, b)
    };
    sse::add64(ab, sqr(sse::sub64(f64::from(z), cz)))
}

/// `estimateNRG(mris, cx, cy, cz, R2)`: `sum((|v - c|² - R2)²)` over every vertex.
fn estimate_nrg(mris: &Mris, cx: f64, cy: f64, cz: f64, r2: f64) -> f64 {
    let mut nrg = 0.0f64;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        let e = sse::sub64(r2_of(v.x, v.y, v.z, cx, cy, cz, true), r2);
        nrg = sse::add64(nrg, sqr(e));
    }
    nrg
}

/// `estimateSquaredRadius(mris, cx, cy, cz)`: the mean of `|v - c|²` over every vertex.
fn estimate_squared_radius(mris: &Mris, cx: f64, cy: f64, cz: f64) -> f64 {
    let mut r2 = 0.0f64;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        r2 = sse::add64(r2, r2_of(v.x, v.y, v.z, cx, cy, cz, false));
    }
    sse::div64(r2, f64::from(mris.nvertices))
}

/// `computeGradient(mris, cx, cy, cz, R2, ...)`: the mean of `(|v - c|² - R2) (v - c)` (the
/// products `dx e`, `dy e`, `e dz` in the official build's order).
fn compute_gradient(mris: &Mris, cx: f64, cy: f64, cz: f64, r2: f64) -> (f64, f64, f64) {
    let (mut tx, mut ty, mut tz) = (0.0f64, 0.0f64, 0.0f64);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        let e = sse::sub64(r2_of(v.x, v.y, v.z, cx, cy, cz, false), r2);
        tx = sse::add64(tx, sse::mul64(sse::sub64(f64::from(v.x), cx), e));
        ty = sse::add64(ty, sse::mul64(sse::sub64(f64::from(v.y), cy), e));
        tz = sse::add64(tz, sse::mul64(e, sse::sub64(f64::from(v.z), cz)));
    }
    let n = f64::from(mris.nvertices);
    (sse::div64(tx, n), sse::div64(ty, n), sse::div64(tz, n))
}

/// `MRIScenterSphere(mris)` (`MRIScenterSphere_old`): `Finding true center and radius of
/// Spherical Surface...` on standard error; the centre and squared radius that minimise
/// `sum((|v - c|² - R2)²)` over every vertex (in `double`), from the bounding box's centre, or
/// the origin with R2 = 10000, or the origin with its own mean squared radius, whichever has the
/// least energy, by at most 100 steps along the gradient (at most 1 mm, halved until the energy
/// falls or the step is below 1e-11); then every vertex moved to `100 / sqrt(R2) (v - c)` and
/// scaled twice onto the sphere of radius 100 (`DEFAULT_RADIUS / sqrt(x² + y² + z²)`, the squares
/// in `float`); `done\nSurface centered at (0,0,0) with radius 100.0 in <n> iterations` on
/// standard error. A surface with no vertex: FreeSurfer's search never ends (its energy is 0, its
/// gradient `0 / 0`); here, in both modes, the error `MRIScenterSphere: the surface has no
/// vertex, so it has no centre` (code -4) after the first message (FS-114, rule B).
pub fn center_sphere(g: &mut Globals, io: &mut Io, mris: &mut Mris) -> Result<(), Fatal> {
    let _ = write!(
        io.err,
        "Finding true center and radius of Spherical Surface..."
    );
    if mris.nvertices <= 0 {
        g.notices.add(
            &FS_114,
            "a surface without vertices is refused: the original's search for its centre never ends",
        );
        log::error_exit(
            g,
            io.err,
            ERROR_BADPARM,
            "MRIScenterSphere: the surface has no vertex, so it has no centre",
        )?;
        return Ok(());
    }
    let (mut xhi, mut yhi, mut zhi) = (-10000.0f64, -10000.0f64, -10000.0f64);
    let (mut xlo, mut ylo, mut zlo) = (10000.0f64, 10000.0f64, 10000.0f64);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        let (x, y, z) = (f64::from(v.x), f64::from(v.y), f64::from(v.z));
        if x > xhi {
            xhi = x;
        }
        if x < xlo {
            xlo = x;
        }
        if y > yhi {
            yhi = y;
        }
        if y < ylo {
            ylo = y;
        }
        if z > zhi {
            zhi = z;
        }
        if z < zlo {
            zlo = z;
        }
    }
    let mut x = sse::div64(sse::add64(xlo, xhi), 2.0);
    let mut y = sse::div64(sse::add64(ylo, yhi), 2.0);
    let mut z = sse::div64(sse::add64(zlo, zhi), 2.0);
    let mut r2 = estimate_squared_radius(mris, x, y, z);
    let mut nrg = estimate_nrg(mris, x, y, z, r2);
    if estimate_nrg(mris, 0.0, 0.0, 0.0, 10000.0) < nrg {
        (x, y, z, r2) = (0.0, 0.0, 0.0, 10000.0);
        nrg = estimate_nrg(mris, x, y, z, r2);
    }
    if estimate_nrg(
        mris,
        0.0,
        0.0,
        0.0,
        estimate_squared_radius(mris, 0.0, 0.0, 0.0),
    ) < nrg
    {
        (x, y, z) = (0.0, 0.0, 0.0);
        r2 = estimate_squared_radius(mris, x, y, z);
        nrg = estimate_nrg(mris, x, y, z, r2);
    }
    let mut last_nrg = sse::add64(nrg, 1.0);
    let mut niters = 0;
    while nrg < last_nrg {
        niters += 1;
        if niters > 100 {
            break;
        }
        last_nrg = nrg;
        let (mut dx, mut dy, mut dz) = compute_gradient(mris, x, y, z, r2);
        let mut d = sse::sqrt64(sse::add64(sse::add64(sqr(dx), sqr(dy)), sqr(dz)));
        if d > 1.0 {
            dx = sse::div64(dx, d);
            dy = sse::div64(dy, d);
            dz = sse::div64(dz, d);
        }
        let mut epsilon = 2.0f64;
        while nrg >= last_nrg {
            epsilon /= 2.0;
            // `x + epsilon * dx` as `dx * epsilon + x`
            let (ex, ey, ez) = (
                sse::add64(sse::mul64(dx, epsilon), x),
                sse::add64(sse::mul64(dy, epsilon), y),
                sse::add64(sse::mul64(dz, epsilon), z),
            );
            nrg = estimate_nrg(mris, ex, ey, ez, estimate_squared_radius(mris, ex, ey, ez));
            d = sse::sqrt64(sse::add64(sse::add64(sqr(dx), sqr(dy)), sqr(dz)));
            if sse::mul64(epsilon, d) < 0.000_000_000_01 {
                break;
            }
        }
        if nrg < last_nrg {
            x = sse::add64(sse::mul64(dx, epsilon), x);
            y = sse::add64(sse::mul64(dy, epsilon), y);
            z = sse::add64(sse::mul64(dz, epsilon), z);
            r2 = estimate_squared_radius(mris, x, y, z);
        } else {
            nrg = estimate_nrg(mris, x, y, z, r2);
        }
    }
    let radius = 100.0f64;
    let scale = sse::sqrt64(sse::div64(sqr(radius), r2));
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        // `cx + scale * (v - x)` as `(v - x) * scale + cx`
        v.x = sse::add64(sse::mul64(sse::sub64(f64::from(v.x), x), scale), 0.0) as f32;
        v.y = sse::add64(sse::mul64(sse::sub64(f64::from(v.y), y), scale), 0.0) as f32;
        v.z = sse::add64(sse::mul64(sse::sub64(f64::from(v.z), z), scale), 0.0) as f32;
    }
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        for _ in 0..2 {
            // `SQR(x) + SQR(y) + SQR(z)` in float as `(y² + x²) + z²`
            let r2 = f64::from(sse::add(
                sse::add(sse::mul(v.y, v.y), sse::mul(v.x, v.x)),
                sse::mul(v.z, v.z),
            ));
            let scale = sse::div64(f64::from(DEFAULT_RADIUS), sse::sqrt64(r2));
            v.x = sse::mul64(f64::from(v.x), scale) as f32;
            v.y = sse::mul64(f64::from(v.y), scale) as f32;
            v.z = sse::mul64(f64::from(v.z), scale) as f32;
        }
    }
    let _ = writeln!(
        io.err,
        "done\nSurface centered at (0,0,0) with radius 100.0 in {niters} iterations"
    );
    Ok(())
}

// ---- the distances -----------------------------------------------------------------------------

/// `MRISclearOrigDistances(mris)`: every unripped vertex's first `vtotal` original distances 0.
/// Returns `NO_ERROR`.
pub fn clear_orig_distances(mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let vtotal = usize::try_from(mris.topology(vno).vtotal).unwrap_or(0);
        if vtotal == 0 {
            continue;
        }
        let d = mris
            .vertex_mut(vno)
            .dist_orig
            .as_mut()
            .expect("VERTEX::dist_orig is NULL");
        for x in &mut d[..vtotal] {
            *x = 0.0;
        }
    }
    0
}

/// `MRISscaleDistances(mris, scale)`: every unripped vertex's distances in use
/// (`mrisVertexVSize`) multiplied by `scale`. Returns `NO_ERROR`.
pub fn scale_distances(mris: &mut Mris, scale: f32) -> i32 {
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let vsize = usize::try_from(topology::vertex_vsize(mris, vno)).unwrap_or(0);
        if vsize == 0 {
            continue;
        }
        let d = mris
            .vertex_mut(vno)
            .dist
            .as_mut()
            .expect("VERTEX::dist is NULL");
        for x in &mut d[..vsize] {
            *x = sse::mul(*x, scale);
        }
    }
    0
}

/// `XYZApproxAngle_knownLength(n, x2, y2, z2, norm)` with the official build's operand order in
/// every product (`utils/matrix.cpp` compiled: `n.x*x2`, `n.y*y2`, but `z2*n.z`), which fs.core's
/// [`larmorx_freesurfer_core::matrix_ops::xyz_approx_angle_known_length`] takes as `n.z*z2`: the
/// two differ only where both are NaNs (HANDOFF for fs.core's owner; until it changes this copy
/// keeps the distances' NaNs the official build's).
pub(crate) fn approx_angle(
    n: &larmorx_freesurfer_core::matrix_ops::Xyz,
    x2: f32,
    y2: f32,
    z2: f32,
    norm: f32,
) -> f32 {
    let dot = sse::add64(
        sse::add64(
            sse::mul64(f64::from(n.x), f64::from(x2)),
            sse::mul64(f64::from(n.y), f64::from(y2)),
        ),
        sse::mul64(f64::from(z2), f64::from(n.z)),
    ) as f32;
    let mut norm = norm;
    if dot.abs() > norm {
        norm = dot.abs();
    }
    let acos_input = f64::from(sse::div(dot, norm));
    if acos_input < f64::from(0.99f32) {
        larmorx_core::math::acos(acos_input) as f32
    } else {
        sse::sqrt64(sse::mul64(2.0, sse::sub64(1.0, acos_input))) as f32
    }
}

/// The per-vertex values of the sphere's formula (`PerVertexInfo`).
#[derive(Clone, Copy, Default)]
struct PerVertexInfo {
    ripped: bool,
    ripped_or_zero_length: bool,
    unnormalized: [f32; 3],
    length: f32,
    normalized: larmorx_freesurfer_core::matrix_ops::Xyz,
}

/// `mrisComputeVertexDistancesWkr(mris, new_dist_nsize, check)` (`orig` false: from the current
/// coordinates and `status` into `dist`, made with `MRISmakeDist`) and
/// `mrisComputeOriginalVertexDistancesWkr` (`orig` true: from the original coordinates and
/// `origxyz_status` into `dist_orig`, `MRISmakeDistOrig`), as
/// `utils/mrisComputeVertexDistancesWkr_extracted.h` defines both: `new_dist_nsize` outside
/// `1..=3` fails a `cheapAssert` (lines 15, 16; FS-03). For each unripped vertex, its distance to
/// each of its first `vtotal` neighbours (`check`: of its first `VERTEXvnum(vt, new_dist_nsize)`,
/// and only where the distances exist): with the plane formula `sqrt(xd² + yd² + zd²)` (in
/// `float`); with the sphere's formula (a
/// spherical status) the angle `|XYZApproxAngle_knownLength|` between the two position vectors
/// times the vertex's own radius, 0 where the neighbour is ripped or at the origin. `check`
/// compares with the stored distances instead of storing them. When no unripped vertex has a
/// non-zero x (counted where the distances are computed), the logic problem `all xyz inputs zero
/// - cant fix` (line 232). Returns whether every compared distance was equal.
pub fn compute_vertex_distances_wkr(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    new_dist_nsize: i32,
    check: bool,
    orig: bool,
) -> Result<bool, Fatal> {
    for (ok, line, tst) in [
        (0 < new_dist_nsize, 15, "0 < new_dist_nsize"),
        (new_dist_nsize <= 3, 16, "new_dist_nsize <= 3"),
    ] {
        if !ok {
            return Err(diag::assert_failed(
                g,
                io.out,
                io.err,
                &extracted_h(),
                line,
                tst,
            ));
        }
    }
    let status = if orig {
        mris.origxyz_status
    } else {
        mris.status
    };
    let formula = aaa::status_distance_formula(g, io.out, io.err, status)?;
    let n = nverts(mris);
    let pos = |m: &Mris, vno: usize| -> [f32; 3] {
        let v = m.vertex(vno);
        if orig {
            [v.origx, v.origy, v.origz]
        } else {
            [v.x, v.y, v.z]
        }
    };
    let make = |m: &mut Mris, vno: usize| {
        if orig {
            dist::make_dist_orig(m, vno);
        } else {
            dist::make_dist(m, vno);
        }
    };
    let has_output = |m: &Mris, vno: usize| -> bool {
        let v = m.vertex(vno);
        if orig {
            v.dist_orig.is_some()
        } else {
            v.dist.is_some()
        }
    };
    let mut nonzero_x = 0i32;
    let mut errors = 0i32;
    // the count of distances of a vertex: vtotal, or (check) VERTEXvnum(vt, new_dist_nsize)
    let count = |m: &Mris, vno: usize| -> usize {
        let vt = m.topology(vno);
        let c = if check {
            match new_dist_nsize {
                1 => vt.vnum,
                2 => vt.v2num,
                _ => vt.v3num,
            }
        } else {
            vt.vtotal
        };
        usize::try_from(c).unwrap_or(0)
    };
    match formula {
        DistanceFormula::Formula0 => {
            for vno in 0..n {
                if mris.vertex(vno).ripflag != 0 {
                    continue;
                }
                let [vx, vy, vz] = pos(mris, vno);
                if !check {
                    make(mris, vno);
                }
                if check && !has_output(mris, vno) {
                    continue;
                }
                if vx != 0.0 {
                    nonzero_x += 1;
                }
                let total = count(mris, vno);
                let mut ds = Vec::with_capacity(total);
                for k in 0..total {
                    let [nx, ny, nz] = pos(mris, nbr(mris, vno, k));
                    let (xd, yd, zd) = (sse::sub(vx, nx), sse::sub(vy, ny), sse::sub(vz, nz));
                    // `xd*xd + yd*yd + zd*zd` as the official build sums it: `zd² + (yd² + xd²)`
                    // where it stores, `zd² + (xd² + yd²)` where it checks; `sqrtss`
                    let (x2, y2, z2) = (sse::mul(xd, xd), sse::mul(yd, yd), sse::mul(zd, zd));
                    let d = if check {
                        sse::add(z2, sse::add(x2, y2))
                    } else {
                        sse::add(z2, sse::add(y2, x2))
                    };
                    ds.push(sse::sqrt(d));
                }
                store_or_check(mris, vno, orig, check, &ds, &mut errors);
            }
        }
        DistanceFormula::Formula1 => {
            let mut info = vec![PerVertexInfo::default(); n];
            for vno in 0..n {
                let ripped = mris.vertex(vno).ripflag != 0;
                info[vno].ripped = ripped;
                info[vno].ripped_or_zero_length = ripped;
                if ripped {
                    continue;
                }
                let [vx, vy, vz] = pos(mris, vno);
                if vx != 0.0 {
                    nonzero_x += 1;
                }
                info[vno].unnormalized = [vx, vy, vz];
                let (nrm, len) = xyz_normalized_load(vx, vy, vz);
                info[vno].normalized = nrm;
                info[vno].length = len;
                if fzero(f64::from(len)) {
                    info[vno].ripped_or_zero_length = true;
                }
            }
            for vno in 0..n {
                let vi = info[vno];
                if vi.ripped {
                    continue;
                }
                if !check {
                    make(mris, vno);
                }
                if check && !has_output(mris, vno) {
                    continue;
                }
                let radius = vi.length;
                let total = count(mris, vno);
                let mut ds = Vec::with_capacity(total);
                for k in 0..total {
                    let vi2 = info[nbr(mris, vno, k)];
                    let mut d = 0.0f32;
                    if !vi2.ripped_or_zero_length {
                        let [x2, y2, z2] = vi2.unnormalized;
                        let angle = approx_angle(&vi.normalized, x2, y2, z2, vi2.length).abs();
                        d = sse::mul(angle, radius);
                    }
                    ds.push(d);
                }
                store_or_check(mris, vno, orig, check, &ds, &mut errors);
            }
        }
    }
    if nonzero_x == 0 {
        cope_with_logic_problem2(
            g,
            io.env,
            io.out,
            io.err,
            None,
            "all xyz inputs zero - cant fix",
            &extracted_h(),
            232,
            if orig {
                "mrisComputeOriginalVertexDistancesWkr"
            } else {
                "mrisComputeVertexDistancesWkr"
            },
        )?;
    }
    Ok(errors == 0)
}

/// Stores a vertex's distances (`check` false), or counts those that differ from the stored ones.
fn store_or_check(
    mris: &mut Mris,
    vno: usize,
    orig: bool,
    check: bool,
    ds: &[f32],
    errors: &mut i32,
) {
    if ds.is_empty() {
        return;
    }
    let v = mris.vertex_mut(vno);
    let out = if orig { &mut v.dist_orig } else { &mut v.dist };
    let out = out.as_mut().expect("the distances are NULL");
    for (k, &d) in ds.iter().enumerate() {
        if !check {
            out[k] = d;
        } else if out[k] != d {
            *errors += 1;
        }
    }
}

/// `mrisCheckDist(mris)`: true without distances (`dist_nsize` 0), else whether the stored
/// distances are those the current coordinates give at `dist_nsize` hops.
pub fn check_dist(g: &mut Globals, io: &mut Io, mris: &mut Mris) -> Result<bool, Fatal> {
    if mris.dist_nsize == 0 {
        return Ok(true);
    }
    let nsize = i32::from(mris.dist_nsize);
    compute_vertex_distances_wkr(g, io, mris, nsize, true, false)
}

/// `mrisCheckDistOrig(mris)`: as [`check_dist`] for the original distances (`dist_orig_nsize`).
pub fn check_dist_orig(g: &mut Globals, io: &mut Io, mris: &mut Mris) -> Result<bool, Fatal> {
    if mris.dist_orig_nsize == 0 {
        return Ok(true);
    }
    let nsize = i32::from(mris.dist_orig_nsize);
    compute_vertex_distances_wkr(g, io, mris, nsize, true, true)
}

/// `mrisComputeVertexDistances(mris)`: the distances at `nsize` hops; `dist_nsize` becomes
/// `nsize`. Returns `NO_ERROR`.
pub fn compute_vertex_distances(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    let nsize = i32::from(mris.nsize);
    compute_vertex_distances_wkr(g, io, mris, nsize, false, false)?;
    mris.dist_nsize = mris.nsize;
    Ok(0)
}

/// `mrisComputeOriginalVertexDistances(mris)`: the original distances at `nsize` hops;
/// `dist_orig_nsize` becomes `nsize`. Returns `NO_ERROR`.
pub fn compute_original_vertex_distances(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    let nsize = i32::from(mris.nsize);
    compute_vertex_distances_wkr(g, io, mris, nsize, false, true)?;
    mris.dist_orig_nsize = mris.nsize;
    Ok(0)
}

/// `mrisComputeOriginalVertexDistancesIfNecessaryWkr(mris, laterTime, file, line)`: nothing when
/// the original distances are allocated (`dist_alloced_flags` bit 2); otherwise the logic problem
/// `dist_orig not already computed` (line 1628, fixed by `FREESURFER_fix_missing_dist_orig`), the
/// original distances computed, and, unless fixed, every vertex's original-distance buffer set
/// to 0 for its whole capacity (FreeSurfer's earlier behaviour: zeros in the allocated buffers).
pub fn compute_original_vertex_distances_if_necessary(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<(), Fatal> {
    if mris.dist_alloced_flags & 2 != 0 {
        return Ok(());
    }
    let (response, _) = cope_with_logic_problem2(
        g,
        io.env,
        io.out,
        io.err,
        Some("FREESURFER_fix_missing_dist_orig"),
        "dist_orig not already computed",
        &metric_properties_cpp(),
        1628,
        "mrisComputeOriginalVertexDistancesIfNecessaryWkr",
    )?;
    compute_original_vertex_distances(g, io, mris)?;
    if response == LogicProblemResponse::Fix {
        return Ok(());
    }
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        let cap = usize::try_from(v.dist_orig_capacity).unwrap_or(0);
        if let Some(d) = v.dist_orig.as_mut() {
            for x in &mut d[..cap] {
                *x = 0.0;
            }
        }
    }
    Ok(())
}

/// `MRISpercentDistanceError(mris)`: 100 times the mean absolute difference between the
/// distances (scaled by `sqrt(orig_area / total_area)`, 1 for a patch) and the original
/// distances over every unripped vertex's first `vtotal` neighbours (0 for a vertex without
/// them), divided by the mean original distance (1000 when that mean is below `FLT_EPSILON`);
/// a surface not a patch whose total or original area is not positive fails a `cheapAssert`
/// (lines 1682, 1683; FS-03); with `DIAG_VERBOSE`, the debug messages `MRISpercentDistanceError
/// called when all dist zero` and `... all dist_orig zero` when those sums are 0.
pub fn percent_distance_error(g: &mut Globals, io: &mut Io, mris: &Mris) -> Result<f64, Fatal> {
    let dist_scale = if mris.patch != 0 {
        1.0f64
    } else {
        for (ok, line, tst) in [
            (
                f64::from(mris.total_area) > 0.0,
                1682,
                "mris->total_area > 0.0",
            ),
            (
                f64::from(mris.orig_area) > 0.0,
                1683,
                "mris->orig_area > 0.0",
            ),
        ] {
            if !ok {
                return Err(diag::assert_failed(
                    g,
                    io.out,
                    io.err,
                    &metric_properties_cpp(),
                    line,
                    tst,
                ));
            }
        }
        // in float (`sqrtss`), then widened
        f64::from(sse::sqrt(sse::div(mris.orig_area, mris.total_area)))
    };
    let (mut mean_dist, mut mean_odist, mut mean_error, mut pct) = (0.0f64, 0.0, 0.0, 0.0);
    let mut nnbrs = 0i32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let vtotal = usize::try_from(mris.topology(vno).vtotal).unwrap_or(0);
        for k in 0..vtotal {
            let dist = match &v.dist {
                None => 0.0,
                Some(d) => sse::mul64(f64::from(d[k]), dist_scale),
            };
            let odist = match &v.dist_orig {
                None => 0.0,
                Some(d) => f64::from(d[k]),
            };
            nnbrs += 1;
            mean_dist = sse::add64(mean_dist, dist);
            mean_odist = sse::add64(mean_odist, odist);
            let e = sse::sub64(dist, odist).abs();
            // `mean_error += fabs(...)` as `fabs(...) + mean_error`
            mean_error = sse::add64(e, mean_error);
            if !fzero(odist) {
                pct = sse::add64(pct, sse::div64(e, odist));
            }
        }
    }
    if mean_dist == 0.0 {
        log::debug(
            g,
            io.err,
            "MRISpercentDistanceError called when all dist zero",
        );
    }
    if mean_odist == 0.0 {
        log::debug(
            g,
            io.err,
            "MRISpercentDistanceError called when all dist_orig zero",
        );
    }
    if nnbrs == 0 {
        nnbrs = 1;
    }
    let _ = pct;
    let mean_odist = sse::div64(mean_odist, f64::from(nnbrs));
    let mean_error = sse::div64(mean_error, f64::from(nnbrs));
    let pct = if !fzero(mean_odist) {
        sse::div64(mean_error, mean_odist)
    } else {
        1000.0
    };
    Ok(sse::mul64(100.0, pct))
}

// ---- the areas ---------------------------------------------------------------------------------

/// `MRISclearOrigArea(mris)`: every unripped vertex's `origarea` 0. Returns `NO_ERROR`.
pub fn clear_orig_area(mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            v.origarea = 0.0;
        }
    }
    0
}

/// `MRISclearOrigAreaAndVal2(mris)`: every unripped vertex's `origarea` and `val2` 0.
pub fn clear_orig_area_and_val2(mris: &mut Mris) {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            v.origarea = 0.0;
            v.val2 = 0.0;
        }
    }
}

/// `mrisComputeArea(mris, fac, n)`: the face's area at its corner `n` (0, 1 or 2): the length of
/// the cross product of the edges into and out of the corner (in `float`), negative when it
/// points away from the corner's position vector.
pub fn compute_area(mris: &Mris, fac: usize, n: usize) -> f32 {
    let n0 = if n == 0 { 2 } else { n - 1 };
    let n1 = if n == 2 { 0 } else { n + 1 };
    let f = mris.face(fac);
    let at = |k: usize| {
        let v = mris.vertex(usize::try_from(f.v[k]).expect("a face's vertex"));
        [v.x, v.y, v.z]
    };
    let (p, p0, p1) = (at(n), at(n0), at(n1));
    let v0 = [
        sse::sub(p[0], p0[0]),
        sse::sub(p[1], p0[1]),
        sse::sub(p[2], p0[2]),
    ];
    let v1 = [
        sse::sub(p1[0], p[0]),
        sse::sub(p1[1], p[1]),
        sse::sub(p1[2], p[2]),
    ];
    // the official build's form: `-a*b + c*d` as `c*d - a*b`, the products' operands swapped
    // where GCC swapped them (`v1[0]*v0[2]` as `v0[2]*v1[0]`, `y*d2` as `d2*y`, `z*d3` as `d3*z`)
    let d1 = sse::sub(sse::mul(v0[1], v1[2]), sse::mul(v1[1], v0[2]));
    let d2 = sse::sub(sse::mul(v0[2], v1[0]), sse::mul(v1[2], v0[0]));
    let d3 = sse::sub(sse::mul(v0[0], v1[1]), sse::mul(v1[0], v0[1]));
    let dot = sse::add(
        sse::add(sse::mul(p[0], d1), sse::mul(d2, p[1])),
        sse::mul(d3, p[2]),
    );
    let len = sse::sqrt(sse::add(
        sse::add(sse::mul(d1, d1), sse::mul(d2, d2)),
        sse::mul(d3, d3),
    ));
    if dot < 0.0 { -len } else { len }
}

/// `MRISzeroNegativeAreas(mris)`: every unripped vertex's and face's negative area 0. Returns
/// `NO_ERROR`.
pub fn zero_negative_areas(mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 && v.area < 0.0 {
            v.area = 0.0;
        }
    }
    for fno in 0..nfaces(mris) {
        let f = mris.face_mut(fno);
        if f.ripflag == 0 && f.area < 0.0 {
            f.area = 0.0;
        }
    }
    0
}

/// `MRIScomputeOrigArea(mris)`: the sum (in float) of the unripped faces' original areas from
/// the face-normal cache ([`facenorm::get_face_norm`]: a deferred entry is computed first).
pub fn compute_orig_area(mris: &mut Mris) -> f32 {
    let mut orig_area = 0.0f32;
    for fno in 0..nfaces(mris) {
        if mris.face(fno).ripflag != 0 {
            continue;
        }
        let e = facenorm::get_face_norm(mris, fno);
        orig_area = sse::add(orig_area, e.orig_area);
    }
    orig_area
}

/// `MRISsetOrigArea(mris)`: the surface's `orig_area` becomes [`compute_orig_area`].
pub fn set_orig_area(mris: &mut Mris) {
    mris.orig_area = compute_orig_area(mris);
}

// ---- triangle corners and coordinate sets ------------------------------------------------------

/// `load_orig_triangle_vertices(mris, fno, U0, U1, U2)`: the face's three corners' original
/// coordinates, as `double`.
pub fn load_orig_triangle_vertices(mris: &Mris, fno: usize) -> [[f64; 3]; 3] {
    let f = mris.face(fno);
    let mut u = [[0.0f64; 3]; 3];
    for k in 0..3 {
        let v = mris.vertex(usize::try_from(f.v[k]).expect("a face's vertex"));
        u[k] = [f64::from(v.origx), f64::from(v.origy), f64::from(v.origz)];
    }
    u
}

/// `load_triangle_vertices(mris, fno, U0, U1, U2, which)`: the face's three corners' white
/// (`WHITE_VERTICES`), pial (`PIAL_VERTICES`) or current (any other `which`) coordinates.
pub fn load_triangle_vertices(mris: &Mris, fno: usize, which: i32) -> [[f64; 3]; 3] {
    let f = mris.face(fno);
    let mut u = [[0.0f64; 3]; 3];
    for k in 0..3 {
        let v = mris.vertex(usize::try_from(f.v[k]).expect("a face's vertex"));
        let p = match which {
            WHITE_VERTICES => [v.whitex, v.whitey, v.whitez],
            PIAL_VERTICES => [v.pialx, v.pialy, v.pialz],
            _ => [v.x, v.y, v.z],
        };
        u[k] = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])];
    }
    u
}

/// `MRISextractVertexCoords(mris, locations, which)`: one of the vertices' coordinate sets
/// (current, target, white, layer IV, pial, inflated, flattened, canonical, original, the two
/// temporary ones) as three arrays; any other `which` is the fatal error
/// `MRISextractVertexCoords: which <n> not supported` (code -3) at the first vertex (none
/// without vertices). The arrays start as `locations` gives them.
pub fn extract_vertex_coords(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    locations: &mut [Vec<f32>; 3],
    which: i32,
) -> Result<i32, Fatal> {
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        let p = match which {
            CURRENT_VERTICES => [v.x, v.y, v.z],
            TARGET_VERTICES => [v.targx, v.targy, v.targz],
            WHITE_VERTICES => [v.whitex, v.whitey, v.whitez],
            LAYERIV_VERTICES => [v.l4x, v.l4y, v.l4z],
            PIAL_VERTICES => [v.pialx, v.pialy, v.pialz],
            INFLATED_VERTICES => [v.infx, v.infy, v.infz],
            FLATTENED_VERTICES => [v.fx, v.fy, v.fz],
            CANONICAL_VERTICES => [v.cx, v.cy, v.cz],
            ORIGINAL_VERTICES => [v.origx, v.origy, v.origz],
            TMP2_VERTICES => [v.t2x, v.t2y, v.t2z],
            TMP_VERTICES => [v.tx, v.ty, v.tz],
            _ => {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_UNSUPPORTED,
                    &format!("MRISextractVertexCoords: which {which} not supported"),
                )?;
                continue;
            }
        };
        for k in 0..3 {
            locations[k][vno] = p[k];
        }
    }
    Ok(0)
}

// ---- the curvatures ----------------------------------------------------------------------------

/// `mrisComputeCurvatureMinMax(mris)`: the surface's `max_curv` and `min_curv` from the unripped
/// vertices' curvatures (the first one's both, then each above or below); unchanged without one.
/// Returns `NO_ERROR`.
pub fn compute_curvature_min_max(mris: &mut Mris) -> i32 {
    let mut found = false;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let c = v.curv;
        if !found {
            mris.max_curv = c;
            mris.min_curv = c;
            found = true;
        } else {
            if c > mris.max_curv {
                mris.max_curv = c;
            }
            if c < mris.min_curv {
                mris.min_curv = c;
            }
        }
    }
    0
}

/// `MRISclearCurvature(mris)`: every unripped vertex's `curv` 0. Returns `NO_ERROR`.
pub fn clear_curvature(mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            v.curv = 0.0;
        }
    }
    0
}

/// `MRISclearCurvAndVal2(mris)`: every unripped vertex's `curv` and `val2` 0.
pub fn clear_curv_and_val2(mris: &mut Mris) {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            v.curv = 0.0;
            v.val2 = 0.0;
        }
    }
}

/// `MRISrectifyCurvature(mris)`: every vertex's `curv` (ripped too) its absolute value (the
/// sign bit cleared), then the curvatures' range. Returns `NO_ERROR`.
pub fn rectify_curvature(mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        v.curv = v.curv.abs();
    }
    compute_curvature_min_max(mris);
    0
}

/// `MRISscaleCurvatures(mris, min_curv, max_curv)`: the unripped vertices' curvatures stretched
/// about their mean (in `double`) by `(max_curv - min_curv) / (old max - old min)` (the first
/// difference in `float`; the old range from -100000 and 100000). Returns `NO_ERROR`.
pub fn scale_curvatures(mris: &mut Mris, min_curv: f32, max_curv: f32) -> i32 {
    let (mut old_min, mut old_max) = (100000.0f64, -100000.0f64);
    let mut mean = 0.0f64;
    let mut vtotal = 0i32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        vtotal += 1;
        let c = f64::from(v.curv);
        mean = sse::add64(mean, c);
        if c > old_max {
            old_max = c;
        }
        if c < old_min {
            old_min = c;
        }
    }
    mean = sse::div64(mean, f64::from(vtotal));
    let scale = sse::div64(
        f64::from(sse::sub(max_curv, min_curv)),
        sse::sub64(old_max, old_min),
    );
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        v.curv = sse::add64(sse::mul64(sse::sub64(f64::from(v.curv), mean), scale), mean) as f32;
    }
    0
}

/// `MRISsmoothCurvatures(mris, niterations)`: `niterations` rounds of each unripped vertex's mean
/// curvature `H` replaced by `0.4 H + 0.25 sum(1-hop H) + 0.05 sum(2-hop H)` (the 2-hop ring from
/// `vnum` to `v2num`) divided by `0.4 + 0.25 vnum + 0.05 (v2num - vnum)` (in `float`); the sums in
/// `double`, the result through `d`. Returns `NO_ERROR`.
pub fn smooth_curvatures(mris: &mut Mris, niterations: i32) -> i32 {
    let k_a = 0.4f32;
    let kernel = [k_a, 0.25f32, 0.25f32 - k_a / 2.0];
    for _ in 0..niterations.max(0) {
        for vno in 0..nverts(mris) {
            if mris.vertex(vno).ripflag != 0 {
                continue;
            }
            let vt = mris.topology(vno);
            let (vnum, v2num) = (i32::from(vt.vnum), i32::from(vt.v2num));
            let mut h = f64::from(sse::mul(kernel[0], mris.vertex(vno).H));
            let mut g = f64::from(kernel[1]);
            let mut vn = 0i32;
            while vn < vnum {
                let nh = mris.vertex(nbr(mris, vno, vn as usize)).H;
                h = sse::add64(h, sse::mul64(g, f64::from(nh)));
                vn += 1;
            }
            g = f64::from(kernel[2]);
            while vn < v2num {
                let nh = mris.vertex(nbr(mris, vno, vn as usize)).H;
                h = sse::add64(h, sse::mul64(g, f64::from(nh)));
                vn += 1;
            }
            let norm = sse::add(
                sse::add(kernel[0], sse::mul(vnum as f32, kernel[1])),
                sse::mul((v2num - vnum) as f32, kernel[2]),
            );
            mris.vertex_mut(vno).d = sse::div64(h, f64::from(norm)) as f32;
        }
        for vno in 0..nverts(mris) {
            let v = mris.vertex_mut(vno);
            if v.ripflag == 0 {
                v.H = v.d;
            }
        }
    }
    0
}

/// The mean of the unripped vertices' curvatures (in `double`) and their count.
fn curv_mean(mris: &Mris) -> (f64, i32) {
    let (mut mean, mut vtotal) = (0.0f64, 0i32);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        mean = sse::add64(mean, f64::from(v.curv));
        vtotal += 1;
    }
    (sse::div64(mean, f64::from(vtotal)), vtotal)
}

/// The unripped vertices' variance about `centre` (in `double`), divided by `vtotal`, and its
/// square root.
fn curv_std(mris: &Mris, centre: f64, vtotal: i32) -> f64 {
    let mut var = 0.0f64;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let s = sse::sub64(f64::from(v.curv), centre);
        var = sse::add64(var, sse::mul64(s, s));
    }
    sse::sqrt64(sse::div64(var, f64::from(vtotal)))
}

/// `MRISzeroMeanCurvature(mris)`: the unripped vertices' mean curvature (in `double`) subtracted
/// from theirs (`curvature mean = %2.3f` on standard output with `DIAG_SHOW` and
/// `DIAG_VERBOSE`), then the curvatures' range. Returns `NO_ERROR`.
pub fn zero_mean_curvature(g: &Globals, io: &mut Io, mris: &mut Mris) -> i32 {
    let (mean, _) = curv_mean(mris);
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let _ = writeln!(io.out, "curvature mean = {}", cfmt_f(mean));
    }
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            v.curv = sse::sub64(f64::from(v.curv), mean) as f32;
        }
    }
    compute_curvature_min_max(mris);
    0
}

/// `%2.3f`.
fn cfmt_f(x: f64) -> String {
    larmorx_freesurfer_core::cstd::fmt_f(x, 2, 3)
}

/// `MRISextractCurvatureVector(mris, curvs)` (`utils/mrisurf_vals.cpp`): every vertex's `curv`.
pub fn extract_curvature_vector(mris: &Mris) -> Vec<f32> {
    (0..nverts(mris)).map(|vno| mris.vertex(vno).curv).collect()
}

/// `qsort(curvs, n, sizeof(float), mris_sort_compare_float)` as glibc 2.35's `qsort` sorts an
/// array of 4-byte elements whose temporary buffer it can allocate (`specs/glibc-qsort.md` §4):
/// a top-down merge sort, the first half `n / 2` long, the first half's element taken while the
/// comparison gives 0 or less (a NaN compares equal to everything).
fn qsort_floats(v: &mut [f32]) {
    let n = v.len();
    if n <= 1 {
        return;
    }
    let n1 = n / 2;
    {
        let (a, b) = v.split_at_mut(n1);
        qsort_floats(a);
        qsort_floats(b);
    }
    let mut tmp = Vec::with_capacity(n);
    let (mut i, mut j) = (0, n1);
    while i < n1 && j < n {
        if mris_sort_compare_float(v[i], v[j]) <= 0 {
            tmp.push(v[i]);
            i += 1;
        } else {
            tmp.push(v[j]);
            j += 1;
        }
    }
    tmp.extend_from_slice(&v[i..n1]);
    tmp.extend_from_slice(&v[j..n]);
    v.copy_from_slice(&tmp);
}

/// `MRISnormalizeCurvature(mris, which_norm)`: nothing for `NORM_NONE`; with `NORM_MEDIAN` the
/// unripped vertices' curvatures become `(curv - median) / std` (the median the middle element,
/// index `nvertices / 2`, of every vertex's curvature sorted by `qsort`; `std` the root mean
/// square about it), with `curvature median = %2.3f, std = %2.3f` on standard output; with any
/// other value `(curv - mean) / std` and `curvature mean = %2.3f, std = %2.3f` (all in
/// `double`); then the curvatures' range. A surface with no vertex reads past its sorted copy
/// for the median (the generator never asks). Returns `NO_ERROR`.
pub fn normalize_curvature(io: &mut Io, mris: &mut Mris, which_norm: i32) -> i32 {
    if which_norm == NORM_NONE {
        return 0;
    }
    let (centre, std, label) = if which_norm == NORM_MEDIAN {
        let mut curvs = extract_curvature_vector(mris);
        qsort_floats(&mut curvs);
        let median = f64::from(curvs[nverts(mris) / 2]);
        let vtotal = (0..nverts(mris))
            .filter(|&vno| mris.vertex(vno).ripflag == 0)
            .count() as i32;
        (median, curv_std(mris, median, vtotal), "median")
    } else {
        let (mean, vtotal) = curv_mean(mris);
        (mean, curv_std(mris, mean, vtotal), "mean")
    };
    let _ = writeln!(
        io.out,
        "curvature {label} = {}, std = {}",
        cfmt_f(centre),
        cfmt_f(std)
    );
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            v.curv = sse::div64(sse::sub64(f64::from(v.curv), centre), std) as f32;
        }
    }
    compute_curvature_min_max(mris);
    0
}

/// `MRISnormalizeCurvatureVariance(mris)`: the unripped vertices' curvatures become
/// `(curv - mean) / std + mean` (in `double`; `curvature mean = %2.3f, std = %2.3f` on standard
/// output with `DIAG_SHOW` and `DIAG_VERBOSE`), then the curvatures' range. Returns `NO_ERROR`.
pub fn normalize_curvature_variance(g: &Globals, io: &mut Io, mris: &mut Mris) -> i32 {
    let (mean, vtotal) = curv_mean(mris);
    let std = curv_std(mris, mean, vtotal);
    if g.gdiag & DIAG_SHOW != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let _ = writeln!(
            io.out,
            "curvature mean = {}, std = {}",
            cfmt_f(mean),
            cfmt_f(std)
        );
    }
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            v.curv = sse::add64(sse::div64(sse::sub64(f64::from(v.curv), mean), std), mean) as f32;
        }
    }
    compute_curvature_min_max(mris);
    0
}

/// `mrisCountCompressed(mris, min_dist)`: the number of (vertex, 1-hop neighbour) pairs, both
/// unripped, closer than `min_dist` (the differences in `float`, the distance in `double`).
pub fn count_compressed(mris: &Mris, min_dist: f64) -> i32 {
    let mut num = 0;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let vnum = usize::try_from(mris.topology(vno).vnum).unwrap_or(0);
        for k in 0..vnum {
            let vn = mris.vertex(nbr(mris, vno, k));
            if vn.ripflag != 0 {
                continue;
            }
            let dx = f64::from(sse::sub(vn.x, v.x));
            let dy = f64::from(sse::sub(vn.y, v.y));
            let dz = f64::from(sse::sub(vn.z, v.z));
            let d = sse::sqrt64(sse::add64(
                sse::add64(sse::mul64(dx, dx), sse::mul64(dy, dy)),
                sse::mul64(dz, dz),
            ));
            if d < min_dist {
                num += 1;
            }
        }
    }
    num
}

/// `MRISstoreMeanCurvature(mris)`: every unripped vertex's `H` its `curv`. Returns `NO_ERROR`.
pub fn store_mean_curvature(mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag == 0 {
            v.H = v.curv;
        }
    }
    0
}

// ---- storing and restoring the metric properties ---------------------------------------------

/// `MRISrestoreOldPositions(mris)`: [`positions::restore_vertex_positions`] of the temporary
/// set.
pub fn restore_old_positions(g: &mut Globals, io: &mut Io, mris: &mut Mris) -> Result<i32, Fatal> {
    positions::restore_vertex_positions(g, io, mris, TMP_VERTICES)
}

/// `MRISstoreCurrentPositions(mris)`: [`positions::save_vertex_positions`] into the temporary
/// set.
pub fn store_current_positions(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    positions::save_vertex_positions(g, io, mris, TMP_VERTICES)
}

/// `MRISstoreMetricProperties(mris)`: every unripped vertex's `origarea` its `area` and, when it
/// has distances, its original distances (made, zeroed, when `NULL`: `MRISmakeDistOrig`) the
/// distances in use (`mrisVertexVSize` of them); every unripped face's cached original area its
/// `area` ([`facenorm::set_face_orig_area`]) and its original angles its angles; the surface's
/// `orig_area` its `total_area`. Returns `NO_ERROR`. Reading or writing past a buffer is
/// FreeSurfer's invalid access: a panic.
pub fn store_metric_properties(mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        {
            let v = mris.vertex_mut(vno);
            v.origarea = v.area;
        }
        if mris.vertex(vno).dist.is_none() {
            continue;
        }
        if mris.vertex(vno).dist_orig.is_none() {
            dist::make_dist_orig(mris, vno);
        }
        let vsize = usize::try_from(topology::vertex_vsize(mris, vno)).unwrap_or(0);
        if vsize == 0 {
            continue;
        }
        let v = mris.vertex_mut(vno);
        let src = v.dist.as_ref().expect("VERTEX::dist is NULL");
        let dst = v.dist_orig.as_mut().expect("VERTEX::dist_orig is NULL");
        dst[..vsize].copy_from_slice(&src[..vsize]);
    }
    for fno in 0..nfaces(mris) {
        if mris.face(fno).ripflag != 0 {
            continue;
        }
        let area = mris.face(fno).area;
        facenorm::set_face_orig_area(mris, fno, area);
        let f = mris.face_mut(fno);
        let angle = f.angle;
        aaa::copy_angles_per_triangle(&mut f.orig_angle, &angle);
    }
    mris.orig_area = mris.total_area;
    0
}

/// `MRISrestoreMetricProperties(mris)`: every unripped vertex's `area` its `origarea` and its
/// distances in use its original distances; every unripped face's `area` its cached original area
/// ([`facenorm::get_face_orig_area`], FS-116) and its angles its original angles; the surface's
/// `total_area` its `orig_area`. Returns `NO_ERROR`. FreeSurfer writes the distances without
/// checking that they exist: a vertex in use with no distance or original-distance buffer (or a
/// short one) is its invalid access, a panic.
pub fn restore_metric_properties(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let vsize = usize::try_from(topology::vertex_vsize(mris, vno)).unwrap_or(0);
        let v = mris.vertex_mut(vno);
        v.area = v.origarea;
        if vsize == 0 {
            continue;
        }
        let src = v.dist_orig.as_ref().expect("VERTEX::dist_orig is NULL");
        let dst = v.dist.as_mut().expect("VERTEX::dist is NULL");
        dst[..vsize].copy_from_slice(&src[..vsize]);
    }
    for fno in 0..nfaces(mris) {
        if mris.face(fno).ripflag != 0 {
            continue;
        }
        let area = facenorm::get_face_orig_area(g, io, mris, fno)?;
        let f = mris.face_mut(fno);
        f.area = area;
        let orig = f.orig_angle;
        aaa::copy_angles_per_triangle(&mut f.angle, &orig);
    }
    mris.total_area = mris.orig_area;
    Ok(0)
}

// ---- saved coordinates -------------------------------------------------------------------------

/// `MRISsavedXYZ`: a surface's current coordinates, saved.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedXyz {
    pub nvertices: i32,
    pub p: Vec<[f32; 3]>,
}

/// `MRISsaveXYZ(mris)`: the current coordinates of every vertex.
pub fn save_xyz(mris: &Mris) -> SavedXyz {
    SavedXyz {
        nvertices: mris.nvertices,
        p: (0..nverts(mris))
            .map(|vno| {
                let v = mris.vertex(vno);
                [v.x, v.y, v.z]
            })
            .collect(),
    }
}

/// `MRISloadXYZ(mris, saved)`: the saved coordinates back, vertex by vertex (`MRISsetXYZ` at
/// line 3148: its logic problem when the distances are allocated); a surface whose vertex count
/// changed fails `cheapAssert(nvertices == mris->nvertices)` (line 3145; FS-03).
pub fn load_xyz(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    saved: &SavedXyz,
) -> Result<(), Fatal> {
    if saved.nvertices != mris.nvertices {
        return Err(diag::assert_failed(
            g,
            io.out,
            io.err,
            &metric_properties_cpp(),
            3145,
            "nvertices == mris->nvertices",
        ));
    }
    let file = metric_properties_cpp();
    for (vno, p) in saved.p.iter().enumerate() {
        coords::set_xyz(g, io, mris, vno as i32, p[0], p[1], p[2], &file, 3148)?;
    }
    Ok(())
}

/// `MRISfreeXYZ(mris, &saved)`: the saved coordinates freed (`None`).
pub fn free_xyz(saved: &mut Option<SavedXyz>) {
    *saved = None;
}

/// `MRISpopXYZ(mris, &saved)`: [`load_xyz`], then [`free_xyz`].
pub fn pop_xyz(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    saved: &mut Option<SavedXyz>,
) -> Result<(), Fatal> {
    load_xyz(g, io, mris, saved.as_ref().expect("MRISsavedXYZ is NULL"))?;
    free_xyz(saved);
    Ok(())
}
