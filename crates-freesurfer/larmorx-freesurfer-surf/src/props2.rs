// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Queue item 3.1-50, part b (relay 18): the second group of `utils/mrisurf_metricProperties.cpp`'s
//! properties: the barycentric coordinates of a point on a face (`face_barycentric_coords`, both
//! forms, with `project_point_to_plane` of fs.surf's `utils/tritri.cpp` taken early), the samples
//! on a face (`MRISsampleFaceCoords`, `MRISsampleFace`, `MRISsampleFaceNormal`), the tangent
//! planes' RMS height (`MRISrmsTPHeight`), the analytic distances (`MRIScomputeAnalyticDistanceError`,
//! `MRISstoreAnalyticDistances`, `MRISdisturbOriginalDistances`), the symmetric mean distance of
//! the zero crossings (`MRIScomputeHausdorffDistance`), the principal curvatures' ratio and
//! contrast (`MRISuseCurvatureRatio`, `MRISuseCurvatureContrast`) and the brute-force distance to
//! a triangle (`StuffVertexCoords`, `StuffFaceCoords`, `MinDistToTriangleBF`).
//!
//! Arithmetic goes through fs.core's `sse` operations in the official build's operand order (the
//! unit oracle's object code; `docs/findings/freesurfer-surfio.md` §20). Defects
//! (`larmorx_core::defects`):
//! - **FS-366** (wrong result): `face_barycentric_coords` of a point on a face whose projected
//!   triangle is degenerate (twice its area below 1e-15) gives the coordinates 0, 0, 0 (`1 / 3` in
//!   integers) where the other degenerate case gives a third each. Fixed by default (a third each,
//!   noted); `reproduce` as FreeSurfer.
//! - **FS-367** (hang): `MinDistToTriangleBF` with a step `dL` below 2^-52 (zero and negative steps
//!   included) can loop for ever. Refused in both modes (a message and `None`); every other step,
//!   a NaN included, as FreeSurfer.

use std::io::Write;

use larmorx_core::defects::{FS_366, FS_367};
use larmorx_freesurfer_core::cstd::{atof, fmt_f};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::matrix_ops::vector3_angle;
use larmorx_freesurfer_core::random::random_number;
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sqrt, sqrt64, sub, sub64};

use crate::aaa::MrisStatus;
use crate::coords::Io;
use crate::metric::percent_distance_error;
use crate::mris::Mris;
use crate::props::compute_tangent_planes;
use crate::topology::valid_vertices;
use crate::views::vertex_coord2xyz_float;

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// `FZERO` of a `double` (`fabs(d) < FLT_EPSILON`).
fn fzero64(x: f64) -> bool {
    x.abs() < f64::from(f32::EPSILON)
}

/// `FZERO` of a `float`.
fn fzero(x: f32) -> bool {
    x.abs() < f32::EPSILON
}

/// `DZERO` (`fabs(d) < 1e-15`).
fn dzero(x: f64) -> bool {
    x.abs() < 1e-15
}

/// `2 pi` as the official build folds `M_PI * 2.0` (and `2.0 * M_PI`).
const TWO_PI: f64 = std::f64::consts::PI * 2.0;

/// `sqrt(DOT(v, v))` of `tritri.h` in `double`: `(x x + y y) + z z`.
fn vlen(v: [f64; 3]) -> f64 {
    sqrt64(add64(
        add64(mul64(v[0], v[0]), mul64(v[1], v[1])),
        mul64(v[2], v[2]),
    ))
}

/// `(V0 + V1 + V2) / 3`, `project_point_to_plane`'s answer for a degenerate triangle.
fn third_sum(v0: [f64; 3], v1: [f64; 3], v2: [f64; 3]) -> [f64; 3] {
    let mut p = [0.0; 3];
    for k in 0..3 {
        p[k] = div64(add64(add64(v0[k], v1[k]), v2[k]), 3.0);
    }
    p
}

/// What [`project_point_to_plane`] gives.
#[derive(Clone, Copy, Debug)]
pub struct Projection {
    /// 0, or -1 when `V1 - V0` (or what is left of `V2 - V0` after removing it) has a length
    /// below 1e-15, -2 when `V2 - V0` has.
    pub ret: i32,
    /// The projection; the triangle's centroid when `ret` is negative.
    pub proj: [f64; 3],
    /// The orthonormal basis `e1`, `e2` of the plane (only when `ret` is 0).
    pub basis: Option<([f64; 3], [f64; 3])>,
    /// The caller's `point` afterwards: `point - V0`, or `V0 + V1` for a degenerate triangle.
    pub point: [f64; 3],
}

/// `project_point_to_plane(point, V0, V1, V2, proj, pe1, pe2)` (`utils/tritri.cpp`, fs.surf's,
/// taken early): `e1 = (V1 - V0) / |V1 - V0|`, `e2 = (V2 - V0) / |V2 - V0|` with `e1`'s part
/// removed and normalized again (each step's length below 1e-15 ends with -1, -2, -1 and the
/// centroid); then `proj = V0 + (e1 . p) e1 + (e2 . p) e2` with `p = point - V0` (written into the
/// caller's `point`). `SCALAR_MUL(v, 1.0 / norm, v)` expands to `v * 1.0 / norm`: a division.
pub fn project_point_to_plane(
    point: [f64; 3],
    v0: [f64; 3],
    v1: [f64; 3],
    v2: [f64; 3],
) -> Projection {
    let degenerate = |ret: i32| {
        let p = [
            add64(v0[0], v1[0]),
            add64(v0[1], v1[1]),
            add64(v0[2], v1[2]),
        ];
        Projection {
            ret,
            proj: third_sum(v0, v1, v2),
            basis: None,
            point: p,
        }
    };
    let mut e1 = [
        sub64(v1[0], v0[0]),
        sub64(v1[1], v0[1]),
        sub64(v1[2], v0[2]),
    ];
    let norm = vlen(e1);
    for x in &mut e1 {
        *x = div64(*x, norm);
    }
    if dzero(norm) {
        return degenerate(-1);
    }
    let mut e2 = [
        sub64(v2[0], v0[0]),
        sub64(v2[1], v0[1]),
        sub64(v2[2], v0[2]),
    ];
    let norm = vlen(e2);
    for x in &mut e2 {
        *x = div64(*x, norm);
    }
    if dzero(norm) {
        return degenerate(-2);
    }
    let d1 = add64(
        add64(mul64(e1[0], e2[0]), mul64(e1[1], e2[1])),
        mul64(e1[2], e2[2]),
    );
    // `tmp = e1 * d1`, its third element compiled as `d1 * e1[2]`
    let tmp = [mul64(e1[0], d1), mul64(e1[1], d1), mul64(d1, e1[2])];
    for k in 0..3 {
        e2[k] = sub64(e2[k], tmp[k]);
    }
    let norm = vlen(e2);
    if dzero(norm) {
        return degenerate(-1);
    }
    for x in &mut e2 {
        *x = div64(*x, norm);
    }
    let p = [
        sub64(point[0], v0[0]),
        sub64(point[1], v0[1]),
        sub64(point[2], v0[2]),
    ];
    let d1 = add64(
        add64(mul64(e1[0], p[0]), mul64(e1[1], p[1])),
        mul64(e1[2], p[2]),
    );
    // `DOT(e2, point)` compiled as `(p1 e2_1 + p0 e2_0) + p2 e2_2`
    let d2 = add64(
        add64(mul64(p[1], e2[1]), mul64(p[0], e2[0])),
        mul64(p[2], e2[2]),
    );
    let mut proj = [0.0; 3];
    for k in 0..3 {
        // `e2 * d2 + e1 * d1`, then `+ V0`
        proj[k] = add64(add64(mul64(e2[k], d2), mul64(e1[k], d1)), v0[k]);
    }
    Projection {
        ret: 0,
        proj,
        basis: Some((e1, e2)),
        point: p,
    }
}

/// `face_barycentric_coords(V0, V1, V2, cx, cy, cz, &l1, &l2, &l3)`: the point projected onto the
/// triangle's plane ([`project_point_to_plane`]; its failure gives -3 and a third each), its and
/// the corners' coordinates in the plane's basis, `detT = (x1 - x3)(y2 - y3) - (y1 - y3)(x2 - x3)`
/// (below 1e-15 in magnitude: -2 and FS-366), `l1`, `l2` and `l3 = 1 - l1 - l2`; 1 when all three
/// are strictly between 0 and 1, -1 when one of them (its distance to 0 or 1 in `float`) lies
/// outside by `FLT_EPSILON` or more, 0 otherwise (on an edge). The coordinates are always given.
pub fn face_barycentric_coords_raw(
    g: &mut Globals,
    v0: [f64; 3],
    v1: [f64; 3],
    v2: [f64; 3],
    c: [f64; 3],
) -> (i32, [f64; 3]) {
    let pr = project_point_to_plane(c, v0, v1, v2);
    let Some((e1, e2)) = pr.basis else {
        return (-3, [1.0 / 3.0; 3]);
    };
    let proj = pr.proj;
    let dot_f = |a: [f64; 3], b: [f64; 3]| {
        add64(
            add64(mul64(a[0], b[0]), mul64(a[1], b[1])),
            mul64(a[2], b[2]),
        )
    };
    let x = dot_f(proj, e1);
    let y = dot_f(proj, e2);
    // the corners' coordinates in the compiled operand orders
    let x1 = add64(
        mul64(e1[2], v0[2]),
        add64(mul64(e1[1], v0[1]), mul64(e1[0], v0[0])),
    );
    let x2 = add64(
        add64(mul64(e1[0], v1[0]), mul64(e1[1], v1[1])),
        mul64(e1[2], v1[2]),
    );
    let x3 = add64(
        add64(mul64(e1[1], v2[1]), mul64(e1[0], v2[0])),
        mul64(e1[2], v2[2]),
    );
    let y1 = add64(
        add64(mul64(e2[1], v0[1]), mul64(e2[0], v0[0])),
        mul64(e2[2], v0[2]),
    );
    let y2 = add64(
        mul64(v1[2], e2[2]),
        add64(mul64(e2[0], v1[0]), mul64(v1[1], e2[1])),
    );
    let y3 = add64(
        add64(mul64(v2[0], e2[0]), mul64(v2[1], e2[1])),
        mul64(v2[2], e2[2]),
    );
    let det = sub64(
        mul64(sub64(x1, x3), sub64(y2, y3)),
        mul64(sub64(y1, y3), sub64(x2, x3)),
    );
    if dzero(det) {
        g.notices.add(
            &FS_366,
            "face_barycentric_coords: a degenerate projected triangle gives a third for each coordinate (FreeSurfer gives 0, 0, 0)",
        );
        let l = if g.notices.reproduce() {
            0.0
        } else {
            1.0 / 3.0
        };
        return (-2, [l; 3]);
    }
    let l1 = div64(
        add64(
            mul64(sub64(x3, x2), sub64(y, y3)),
            mul64(sub64(y2, y3), sub64(x, x3)),
        ),
        det,
    );
    let l2 = div64(
        add64(
            mul64(sub64(y3, y1), sub64(x, x3)),
            mul64(sub64(x1, x3), sub64(y, y3)),
        ),
        det,
    );
    let l3 = sub64(sub64(1.0, l1), l2);
    let inside = |l: f64| l > 0.0 && l < 1.0;
    let ret = if inside(l1) && inside(l2) && inside(l3) {
        1
    } else {
        let d = |l: f64| {
            let one = sub64(1.0, l);
            (if l < one { l } else { one }) as f32
        };
        let out = |l: f32| l < 0.0 && !fzero(l);
        if out(d(l1)) || out(d(l2)) || out(d(l3)) {
            -1
        } else {
            0
        }
    };
    (ret, [l1, l2, l3])
}

/// The three corners of face `fno` in the coordinate set `which` (`MRISvertexCoord2XYZ_float`,
/// widened to `double`).
fn corners(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    fno: usize,
    which: i32,
) -> Result<[[f64; 3]; 3], Fatal> {
    let mut d = [[0.0; 3]; 3];
    for (i, row) in d.iter_mut().enumerate() {
        let v = mris.vertex(mris.face(fno).v[i] as usize);
        let (x, y, z) = vertex_coord2xyz_float(g, io.err, v, which)?;
        *row = [f64::from(x), f64::from(y), f64::from(z)];
    }
    Ok(d)
}

/// `face_barycentric_coords(mris, fno, which_vertices, cx, cy, cz, ...)`: the corners of the
/// face in the coordinate set `which_vertices` (an unsupported set ends the program as
/// `MRISvertexCoord2XYZ_float` does), then [`face_barycentric_coords_raw`].
pub fn face_barycentric_coords(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    fno: usize,
    which: i32,
    c: [f64; 3],
) -> Result<(i32, [f64; 3]), Fatal> {
    let d = corners(g, io, mris, fno, which)?;
    Ok(face_barycentric_coords_raw(g, d[0], d[1], d[2], c))
}

/// `MRISsampleFaceCoords(mris, fno, x, y, z, which_coords, which_barycentric, ...)`: the
/// barycentric coordinates of the point in the set `which_barycentric` (a third each when
/// negative), then the coordinates of the set `which_coords` at that place, summed in `float` per
/// corner (`(xv l) + px`); with [`face_barycentric_coords`]'s answer.
pub fn sample_face_coords(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    fno: usize,
    p: [f64; 3],
    which_coords: i32,
    which_barycentric: i32,
) -> Result<(i32, [f32; 3]), Fatal> {
    let (ret, mut lambda) = face_barycentric_coords(g, io, mris, fno, which_barycentric, p)?;
    if ret < 0 {
        lambda = [1.0 / 3.0; 3];
    }
    let mut out = [0.0f32; 3];
    for (n, l) in lambda.iter().enumerate() {
        let v = mris.vertex(mris.face(fno).v[n] as usize);
        let (xv, yv, zv) = vertex_coord2xyz_float(g, io.err, v, which_coords)?;
        for (o, c) in out.iter_mut().zip([xv, yv, zv]) {
            *o = add64(mul64(f64::from(c), *l), f64::from(*o)) as f32;
        }
    }
    Ok((ret, out))
}

/// `MRISsampleFace(mris, fno, which, x, y, z, val0, val1, val2)`: `l0 val0 + l1 val1 + l2 val2`
/// with the barycentric coordinates of the point in the set `which` whatever they are (a
/// degenerate face's too), each term `val l` added in turn to 0; each corner's coordinates of
/// `which` are read once more (an unsupported set ends the program).
#[allow(clippy::too_many_arguments)]
pub fn sample_face(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    fno: usize,
    which: i32,
    p: [f64; 3],
    vals: [f64; 3],
) -> Result<f64, Fatal> {
    let (_, lambda) = face_barycentric_coords(g, io, mris, fno, which, p)?;
    let mut val = 0.0f64;
    for n in 0..3 {
        let v = mris.vertex(mris.face(fno).v[n] as usize);
        vertex_coord2xyz_float(g, io.err, v, which)?;
        val = add64(val, mul64(vals[n], lambda[n]));
    }
    Ok(val)
}

/// The `float` distance from canonical corner `(cx, cy, cz)` to the point, as
/// `MRISsampleFaceNormal` computes it: each difference in `double`, narrowed, then
/// `sqrt(z² + (x² + y²))` in `float`.
fn canonical_distance(v: &crate::mris::Vertex, p: [f64; 3]) -> f32 {
    let dx = sub64(f64::from(v.cx), p[0]) as f32;
    let dy = sub64(f64::from(v.cy), p[1]) as f32;
    let dz = sub64(f64::from(v.cz), p[2]) as f32;
    sqrt(add(mul(dz, dz), add(mul(dx, dx), mul(dy, dy))))
}

/// `MRISsampleFaceNormal(mris, fno, x, y, z, ...)`: the normals of the face's corners weighted by
/// `1 - 2 d / dtotal` (the corner's canonical distance `d` to the point over their sum; weights
/// below 0 left out), in `float`.
pub fn sample_face_normal(mris: &Mris, fno: usize, p: [f64; 3]) -> [f32; 3] {
    let face = mris.face(fno);
    let mut dtotal = 0.0f32;
    for n in 0..3 {
        let d = canonical_distance(mris.vertex(face.v[n] as usize), p);
        if d < 0.0 {
            continue;
        }
        dtotal = add(d, dtotal);
    }
    let mut out = [0.0f32; 3];
    for n in 0..3 {
        let v = mris.vertex(face.v[n] as usize);
        let d = canonical_distance(v, p);
        let w = sub(1.0, div(add(d, d), dtotal));
        if w < 0.0 {
            continue;
        }
        out[0] = add(mul(v.nx, w), out[0]);
        out[1] = add(mul(v.ny, w), out[1]);
        out[2] = add(mul(w, v.nz), out[2]);
    }
    out
}

/// `MRISrmsTPHeight(mris)`: 0 for a plane (`MRIS_PLANE`); otherwise the tangent planes are
/// recomputed (`mrisComputeTangentPlanes`) and the root of the mean, over every unripped
/// neighbour (at a squared distance of `FLT_EPSILON` or more) of every unripped vertex, of the
/// squared height `(n . d)² / |d|²` (`d` the neighbour's offset in `float`, the rest in `double`);
/// 0/0 (a NaN) when no neighbour counts.
pub fn rms_tp_height(g: &mut Globals, io: &mut Io, mris: &mut Mris) -> f64 {
    if mris.status == MrisStatus::PLANE {
        return 0.0;
    }
    let mut avg = 0.0f64;
    let mut total = 0i32;
    compute_tangent_planes(g, io, mris);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let vt = mris.topology(vno);
        if vt.vtotal <= 0 {
            continue;
        }
        let (nx, ny, nz) = (f64::from(v.nx), f64::from(v.ny), f64::from(v.nz));
        let nbrs = vt.v.as_deref().unwrap_or(&[]);
        for &n in &nbrs[..vt.vtotal as usize] {
            let vn = mris.vertex(n as usize);
            if vn.ripflag != 0 {
                continue;
            }
            let x = f64::from(sub(vn.x, v.x));
            let y = f64::from(sub(vn.y, v.y));
            let z = f64::from(sub(vn.z, v.z));
            let d = add64(add64(mul64(x, x), mul64(y, y)), mul64(z, z));
            if fzero64(d) {
                continue;
            }
            let dot = add64(add64(mul64(x, nx), mul64(y, ny)), mul64(z, nz));
            avg = add64(div64(mul64(dot, dot), d), avg);
            total += 1;
        }
    }
    sqrt64(div64(avg, f64::from(total)))
}

/// A 3-vector (`VectorAlloc(3, MATRIX_REAL)`) holding `x`, `y`, `z` (`VECTOR_LOAD`).
fn v3(g: &mut Globals, io: &mut Io, x: f32, y: f32, z: f32) -> Result<Matrix, Fatal> {
    let mut m = matrix_alloc(g, io.out, io.err, 3, 1, MATRIX_REAL)?.expect("a 3-vector");
    m.set_relt(1, 1, x);
    m.set_relt(2, 1, y);
    m.set_relt(3, 1, z);
    Ok(m)
}

/// The analytic distance from vertex `v` to neighbour `vn` (the original positions): the
/// straight-line distance (`sqrt(z² + (x² + y²))` in `float`) for a plane and every `which` that is
/// not a sphere; the great-circle distance `circumference angle / 2 pi` for `MRIS_SPHERE` and
/// `MRIS_PARAMETERIZED_SPHERE` (`angle` from `Vector3Angle`, the product in `float` as `angle
/// circumference`, then divided in `double`).
fn analytic_distance(
    g: &mut Globals,
    io: &mut Io,
    sphere: Option<(&Matrix, f32)>,
    v: &crate::mris::Vertex,
    vn: &crate::mris::Vertex,
) -> Result<f32, Fatal> {
    Ok(match sphere {
        None => {
            let xd = sub(v.origx, vn.origx);
            let yd = sub(v.origy, vn.origy);
            let zd = sub(v.origz, vn.origz);
            sqrt(add(mul(zd, zd), add(mul(xd, xd), mul(yd, yd))))
        }
        Some((v1, circumference)) => {
            let v2 = v3(g, io, vn.origx, vn.origy, vn.origz)?;
            let angle = vector3_angle(g, io.err, v1, &v2).abs() as f32;
            div64(f64::from(mul(angle, circumference)), TWO_PI) as f32
        }
    })
}

/// The circumference `2 pi |orig|` of a sphere through the vertex's original position:
/// `sqrt(z² + (x² + y²))` in `float`, multiplied by `2 pi` in `double`, narrowed.
fn circumference_of(v: &crate::mris::Vertex) -> f32 {
    let len = sqrt(add(
        mul(v.origz, v.origz),
        add(mul(v.origx, v.origx), mul(v.origy, v.origy)),
    ));
    mul64(f64::from(len), TWO_PI) as f32
}

/// Whether `which` is a sphere's (`MRIS_SPHERE`, `MRIS_PARAMETERIZED_SPHERE`).
fn is_sphere(which: i32) -> bool {
    which == MrisStatus::SPHERE.0 || which == MrisStatus::PARAMETERIZED_SPHERE.0
}

/// `MRIScomputeAnalyticDistanceError(mris, which, fp)`: over every unripped neighbour (the first
/// `vtotal`) of every unripped vertex, the analytic distance `odist` ([`analytic_distance`];
/// the sphere's circumference from the first vertex whose own is not below `FLT_EPSILON`, kept
/// from then on) against `dist_orig` and `dist`: the means of `|dist_orig - odist|`, `|dist -
/// odist|`, their signed means and the mean `odist`; prints `mean orig = %2.3f mm (%%%2.2f), final
/// = %2.3f mm (%%%2.2f)` and `signed mean orig error = %2.3f, final mean error = %2.3f` on
/// standard output and, with `fp`, the line of seven numbers (the valid vertices' percentage,
/// `FS_DISTURB_DISTANCES`, the two percentages and errors, `MRISpercentDistanceError`); returns
/// the final error over the mean distance. The vertices' distance arrays must hold `vtotal`
/// numbers.
pub fn compute_analytic_distance_error(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    which: i32,
    fp: Option<&mut dyn Write>,
) -> Result<f64, Fatal> {
    let mut v1 = v3(g, io, 0.0, 0.0, 0.0)?;
    let (mut mean_orig_error, mut mean_error, mut mean) = (0.0f64, 0.0f64, 0.0f64);
    let (mut smean_orig_error, mut smean_error) = (0.0f64, 0.0f64);
    let mut circumference = 0.0f32;
    let mut ndists = 0i32;
    let sphere = is_sphere(which);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        if sphere {
            v1.set_relt(1, 1, v.origx);
            v1.set_relt(2, 1, v.origy);
            v1.set_relt(3, 1, v.origz);
            if fzero(circumference) {
                circumference = circumference_of(v);
            }
        }
        let vt = mris.topology(vno);
        let nbrs = vt.v.as_deref().unwrap_or(&[]);
        for n in 0..usize::try_from(vt.vtotal).unwrap_or(0) {
            let vn = mris.vertex(nbrs[n] as usize);
            if vn.ripflag != 0 {
                continue;
            }
            let odist = analytic_distance(g, io, sphere.then_some((&v1, circumference)), v, vn)?;
            let dorig = sub(v.dist_orig.as_ref().expect("dist_orig")[n], odist);
            let dcur = sub(v.dist.as_ref().expect("dist")[n], odist);
            smean_orig_error = add64(smean_orig_error, f64::from(dorig));
            mean_orig_error = add64(mean_orig_error, f64::from(dorig.abs()));
            mean = add64(mean, f64::from(if sphere { odist.abs() } else { odist }));
            smean_error = add64(smean_error, f64::from(dcur));
            mean_error = add64(mean_error, f64::from(dcur.abs()));
            ndists += 1;
        }
    }
    let n = f64::from(ndists);
    let mean_error = div64(mean_error, n);
    let mean_orig_error = div64(mean_orig_error, n);
    let mean = div64(mean, n);
    let smean_orig_error = div64(smean_orig_error, n);
    let pct = div64(mean_error, mean);
    let pct_orig = div64(mean_orig_error, mean);
    let smean_error = div64(smean_error, n);
    let _ = writeln!(
        io.out,
        "mean orig = {} mm (%{}), final = {} mm (%{})",
        fmt_f(mean_orig_error, 2, 3),
        fmt_f(mul64(pct_orig, 100.0), 2, 2),
        fmt_f(mean_error, 2, 3),
        fmt_f(mul64(pct, 100.0), 2, 2)
    );
    let _ = writeln!(
        io.out,
        "signed mean orig error = {}, final mean error = {}",
        fmt_f(smean_orig_error, 2, 3),
        fmt_f(smean_error, 2, 3)
    );
    if let Some(fp) = fp {
        let disturb = match io.env.get("FS_DISTURB_DISTANCES") {
            Some(s) => atof(s.as_encoded_bytes()) as f32,
            None => 0.0,
        };
        let measured = percent_distance_error(g, io, mris)? as f32;
        let valid = div(
            mul(valid_vertices(mris) as f32, 100.0),
            mris.nvertices as f32,
        );
        let _ = writeln!(
            fp,
            "{}  {}  {}  {}  {}  {}  {}",
            fmt_f(f64::from(valid), 2, 3),
            fmt_f(f64::from(disturb), 2, 3),
            fmt_f(mul64(100.0, pct_orig), 2, 3),
            fmt_f(mean_orig_error, 2, 3),
            fmt_f(mul64(100.0, pct), 2, 3),
            fmt_f(mean_error, 2, 3),
            fmt_f(f64::from(measured), 2, 3)
        );
    }
    Ok(pct)
}

/// `MRISstoreAnalyticDistances(mris, which)`: each unripped vertex's `dist_orig` set to the
/// analytic distances of its unripped neighbours (the first `vtotal`; ripped neighbours keep
/// theirs), as [`compute_analytic_distance_error`] computes them; returns 0.
pub fn store_analytic_distances(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    which: i32,
) -> Result<f64, Fatal> {
    let mut v1 = v3(g, io, 0.0, 0.0, 0.0)?;
    let mut circumference = 0.0f32;
    let sphere = is_sphere(which);
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let v = mris.vertex(vno).clone();
        if sphere {
            v1.set_relt(1, 1, v.origx);
            v1.set_relt(2, 1, v.origy);
            v1.set_relt(3, 1, v.origz);
            if fzero(circumference) {
                circumference = circumference_of(&v);
            }
        }
        let vtotal = usize::try_from(mris.topology(vno).vtotal).unwrap_or(0);
        for n in 0..vtotal {
            let nb = mris.topology(vno).v.as_ref().expect("neighbours")[n] as usize;
            let vn = mris.vertex(nb);
            if vn.ripflag != 0 {
                continue;
            }
            let vn = vn.clone();
            let d = analytic_distance(g, io, sphere.then_some((&v1, circumference)), &v, &vn)?;
            mris.vertex_mut(vno).dist_orig.as_mut().expect("dist_orig")[n] = d;
        }
    }
    Ok(0.0)
}

/// `MRISdisturbOriginalDistances(mris, max_pct)`: each unripped vertex's first `vtotal` original
/// distances multiplied (in `double`, narrowed) by `randomNumber(-max_pct / 100, max_pct / 100) +
/// 1`, one draw per distance.
pub fn disturb_original_distances(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    max_pct: f64,
) -> Result<i32, Fatal> {
    let lo = div64(-max_pct, 100.0);
    let hi = div64(max_pct, 100.0);
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let vtotal = usize::try_from(mris.topology(vno).vtotal).unwrap_or(0);
        for n in 0..vtotal {
            let r = random_number(g, io.env, io.out, io.err, lo, hi)?;
            let d = &mut mris.vertex_mut(vno).dist_orig.as_mut().expect("dist_orig")[n];
            *d = mul64(f64::from(*d), add64(r, 1.0)) as f32;
        }
    }
    Ok(NO_ERROR)
}

/// The fraction of the way from `a` (the value at this end) to the zero between `a` and `b`
/// (`-a / (b - a)` in `float`, as `-a / (-a + b)` compiles), widened.
fn zero_fraction(a: f32, b: f32) -> f64 {
    f64::from(div(-a, sub(b, a)))
}

/// `MRIScomputeHausdorffDistance(mris, mode)` (`mode` unused): over every pair of unripped
/// immediate neighbours (`vnum`) of unripped vertices, at each zero crossing of `val` the value of
/// `|val2|` interpolated at the crossing, and at each zero crossing of `val2` that of `|val|`;
/// their mean (0 with none).
pub fn compute_hausdorff_distance(mris: &Mris, _mode: i32) -> f64 {
    let mut hdist = 0.0f64;
    let mut num = 0i32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let vt = mris.topology(vno);
        let nbrs = vt.v.as_deref().unwrap_or(&[]);
        for &nb in &nbrs[..usize::try_from(vt.vnum).unwrap_or(0)] {
            let vn = mris.vertex(nb as usize);
            if vn.ripflag != 0 {
                continue;
            }
            if mul(vn.val, v.val) < 0.0 {
                let dist = if vn.val < 0.0 {
                    sub64(1.0, zero_fraction(vn.val, v.val))
                } else {
                    zero_fraction(v.val, vn.val)
                };
                let d = add64(
                    mul64(dist, f64::from(v.val2.abs())),
                    mul64(sub64(1.0, dist), f64::from(vn.val2.abs())),
                );
                num += 1;
                hdist = add64(hdist, d);
            }
            if mul(v.val2, vn.val2) < 0.0 {
                let dist = if vn.val2 < 0.0 {
                    sub64(1.0, zero_fraction(vn.val2, v.val2))
                } else {
                    zero_fraction(v.val2, vn.val2)
                };
                num += 1;
                let d = add64(
                    mul64(f64::from(v.val.abs()), dist),
                    mul64(f64::from(vn.val.abs()), sub64(1.0, dist)),
                );
                hdist = add64(hdist, d);
            }
        }
    }
    if num > 0 {
        hdist = div64(hdist, f64::from(num));
    }
    hdist
}

/// `MRISuseCurvatureRatio(mris)`: each unripped vertex's curvature set to `|k1 / k2|` with `k1` the
/// principal curvature larger in magnitude (where `k2` is not below `FLT_EPSILON` in magnitude);
/// then, where it is, 0 when `k1` is too, else the smallest ratio for `k1 < 0` and the largest
/// otherwise; the surface's extremes set to those (10000 and -10000 with none).
pub fn use_curvature_ratio(mris: &mut Mris) -> i32 {
    let mut min_curv = 10000.0f32;
    let mut max_curv = -min_curv;
    let order = |v: &crate::mris::Vertex| {
        if v.k1.abs() > v.k2.abs() {
            (v.k1, v.k2)
        } else {
            (v.k2, v.k1)
        }
    };
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        let (k1, k2) = order(v);
        if !fzero(k2) {
            let curv = div(k1, k2).abs();
            if curv < min_curv {
                min_curv = curv;
            }
            if curv > max_curv {
                max_curv = curv;
            }
            v.curv = curv;
        }
    }
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        let (k1, k2) = order(v);
        if fzero(k2) {
            v.curv = if fzero(k1) {
                0.0
            } else if k1 < 0.0 {
                min_curv
            } else {
                max_curv
            };
        }
    }
    mris.min_curv = min_curv;
    mris.max_curv = max_curv;
    NO_ERROR
}

/// `MRISuseCurvatureContrast(mris)`: with `m` the smallest principal curvature of the unripped
/// vertices (10000 at most), each unripped vertex's curvature set to `(k1 - k2) / (k1 + k2)` with
/// `k1 >= k2` both less `m`; the surface's extremes set to those (10000 and -10000 with none).
pub fn use_curvature_contrast(mris: &mut Mris) -> i32 {
    let mut min_k = 10000.0f32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        if v.k1 < min_k {
            min_k = v.k1;
        }
        if v.k2 < min_k {
            min_k = v.k2;
        }
    }
    let mut min_curv = 10000.0f32;
    let mut max_curv = -min_curv;
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        let (k1, k2) = if v.k1 > v.k2 {
            (v.k1, v.k2)
        } else {
            (v.k2, v.k1)
        };
        let (k1, k2) = (sub(k1, min_k), sub(k2, min_k));
        let curv = div(sub(k1, k2), add(k1, k2));
        if curv < min_curv {
            min_curv = curv;
        }
        if curv > max_curv {
            max_curv = curv;
        }
        v.curv = curv;
    }
    mris.min_curv = min_curv;
    mris.max_curv = max_curv;
    NO_ERROR
}

/// `StuffVertexCoords(surf, vno, p)`: the vertex's current position in `double`.
pub fn stuff_vertex_coords(mris: &Mris, vno: usize) -> [f64; 3] {
    let v = mris.vertex(vno);
    [f64::from(v.x), f64::from(v.y), f64::from(v.z)]
}

/// `StuffFaceCoords(surf, fno, corner, p)`: corner `corner` of face `fno`, as
/// [`stuff_vertex_coords`].
pub fn stuff_face_coords(mris: &Mris, fno: usize, corner: usize) -> [f64; 3] {
    stuff_vertex_coords(mris, mris.face(fno).v[corner] as usize)
}

/// The smallest step [`min_dist_to_triangle_bf`] takes (FS-367): below it the loops can stop
/// advancing.
pub const MIN_DIST_STEP: f64 = f64::EPSILON;

/// `MinDistToTriangleBF(p1, p2, p3, ptest, pmin, dL)`: the smallest distance from `ptest` to the
/// barycentric grid `l1 p1 + l2 p2 + l3 p3` (`l1`, `l2` from 0 by `dL` while below 1, `l3 = 1 -
/// (l2 + l1)` between 0 and 1), 1e11 when no point is nearer; with the nearest grid point
/// (`None` where no point is nearer than 1e11: `pmin` unwritten). A step below 2^-52 is refused
/// (FS-367): the message `MinDistToTriangleBF: step <dL> is too small` on standard error and
/// `None`.
pub fn min_dist_to_triangle_bf(
    g: &mut Globals,
    io: &mut Io,
    p: [[f64; 3]; 3],
    ptest: [f64; 3],
    dl: f64,
) -> Option<(f64, Option<[f64; 3]>)> {
    if dl < MIN_DIST_STEP {
        g.notices.add(
            &FS_367,
            format!(
                "MinDistToTriangleBF: a step of {dl:e} is refused (FreeSurfer can loop for ever)"
            ),
        );
        let _ = writeln!(io.err, "MinDistToTriangleBF: step {dl:e} is too small");
        return None;
    }
    let [p1, p2, p3] = p;
    let mut dmin = 10e10f64;
    let mut pmin = None;
    let mut l1 = 0.0f64;
    while l1 < 1.0 {
        let mut l2 = 0.0f64;
        while l2 < 1.0 {
            let l3 = sub64(1.0, add64(l2, l1));
            if !(l3 < 0.0 || l3 > 1.0) {
                let mut r = [0.0f64; 3];
                for k in 0..2 {
                    r[k] = add64(add64(mul64(p1[k], l1), mul64(p2[k], l2)), mul64(p3[k], l3));
                }
                r[2] = add64(mul64(l3, p3[2]), add64(mul64(l1, p1[2]), mul64(l2, p2[2])));
                let pr: Vec<f64> = (0..3).map(|k| sub64(ptest[k], r[k])).collect();
                let d = sqrt64(add64(
                    add64(add64(mul64(pr[0], pr[0]), 0.0), mul64(pr[1], pr[1])),
                    mul64(pr[2], pr[2]),
                ));
                if d < dmin {
                    dmin = d;
                    pmin = Some(r);
                }
            }
            l2 = add64(l2, dl);
        }
        l1 = add64(l1, dl);
    }
    Some((dmin, pmin))
}
