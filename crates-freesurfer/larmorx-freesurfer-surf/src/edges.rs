// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Whether two edges cross on the sphere (`fs.surfio`, queue item 3.1-47; `utils/mrisurf_base.cpp`
//! 1049-1290, the `SPHERE_INTERSECTION` version `include/mrisurf_base.h` selects): `edgesIntersect`
//! and `edgesIntersectStable`, on the vertices' canonical coordinates (`cx`, `cy`, `cz`), in
//! `double` with `F_CROSS` and `F_DOT`'s operation order (`include/mrisurf_base.h`). The topology
//! fixer (`mrisurf_defect.cpp`) and the overlap removal (`mrisurf_metricProperties.cpp`) call it.
//!
//! Only comparisons leave these functions, so a NaN's payload never shows: a NaN makes every
//! comparison false, as in C.

use crate::mris::Mris;

/// `EDGE` (`include/topo_parms.h`): two vertex numbers, a length and a use flag.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edge {
    pub vno1: i32,
    pub vno2: i32,
    pub len: f32,
    pub used: i16,
}

type V3 = [f64; 3];

/// `F_DOT(a, b)`: `a0*b0 + a1*b1 + a2*b2`, left to right.
fn dot(a: &V3, b: &V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `F_CROSS(a, b, d)`: `d0 = a1*b2 - b1*a2`, `d1 = a2*b0 - b2*a0`, `d2 = a0*b1 - b0*a1`.
fn cross(a: &V3, b: &V3) -> V3 {
    [
        a[1] * b[2] - b[1] * a[2],
        a[2] * b[0] - b[2] * a[0],
        a[0] * b[1] - b[0] * a[1],
    ]
}

/// `MIN(a, b)` and `MAX(a, b)` (`a < b ? a : b`, `a > b ? a : b`).
fn cmin(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}
fn cmax(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

fn canon(mris: &Mris, vno: i32) -> V3 {
    let v = mris.vertex(vno as usize);
    [f64::from(v.cx), f64::from(v.cy), f64::from(v.cz)]
}

fn shares_a_vertex(e1: &Edge, e2: &Edge) -> bool {
    e1.vno1 == e2.vno1 || e1.vno1 == e2.vno2 || e1.vno2 == e2.vno1 || e1.vno2 == e2.vno2
}

/// The decision both versions share: the signs of the products `a = a0*a1` and `b = a2*a3`, and,
/// when both are 0 (or NaN), whether the projections on the normal of `u` and `n0` overlap.
#[allow(clippy::too_many_arguments)]
fn decide(a0: f64, a1: f64, a2: f64, a3: f64, u0: &V3, u1: &V3, u2: &V3, u3: &V3, n0: &V3) -> i32 {
    let a = a0 * a1;
    let b = a2 * a3;
    if a > 0.0 {
        return 0;
    }
    if a < 0.0 {
        return i32::from(b <= 0.0 || b.is_nan());
    }
    if b > 0.0 {
        return 0;
    }
    if b < 0.0 {
        return 1;
    }
    let u = [
        u0[0] + u1[0] + u2[0] + u3[0],
        u0[1] + u1[1] + u2[1] + u3[1],
        u0[2] + u1[2] + u2[2] + u3[2],
    ];
    let n2 = cross(&u, n0);
    let (c0, c1, c2, c3) = (dot(u0, &n2), dot(u1, &n2), dot(u2, &n2), dot(u3, &n2));
    let (x1_min, x1_max) = (cmin(c0, c1), cmax(c0, c1));
    let (x2_min, x2_max) = (cmin(c2, c3), cmax(c2, c3));
    i32::from(x2_max >= x1_min && x2_min <= x1_max)
}

/// `edgesIntersect(mris, edge1, edge2)`: 0 for edges that share a vertex; otherwise the planes
/// through the origin and each edge (normals `n0 = u0 x u1`, `n1 = u2 x u3`): the ends' dot
/// products with the other edge's normal (`a0`, `a1` on `n1`; `a2`, `a3` on `n0`). When any of the
/// four is below 1e-7 in magnitude, [`edges_intersect_stable`] decides. Otherwise 0 when `a0*a1 >
/// 0`; when `a0*a1 < 0`, 0 for `a2*a3 > 0`, else 1; when neither, 0 for `a2*a3 > 0`, 1 for `< 0`,
/// else whether the ends' projections on `(u0+u1+u2+u3) x n0` overlap.
pub fn edges_intersect(mris: &Mris, edge1: &Edge, edge2: &Edge) -> i32 {
    if shares_a_vertex(edge1, edge2) {
        return 0;
    }
    let (u0, u1) = (canon(mris, edge1.vno1), canon(mris, edge1.vno2));
    let n0 = cross(&u0, &u1);
    let (u2, u3) = (canon(mris, edge2.vno1), canon(mris, edge2.vno2));
    let n1 = cross(&u2, &u3);
    let a0 = dot(&u0, &n1);
    let a1 = dot(&u1, &n1);
    let a2 = dot(&u2, &n0);
    let a3 = dot(&u3, &n0);
    const SMALL_VALUES: f64 = 0.0000001;
    if a0.abs() < SMALL_VALUES
        || a1.abs() < SMALL_VALUES
        || a2.abs() < SMALL_VALUES
        || a3.abs() < SMALL_VALUES
    {
        return edges_intersect_stable(mris, edge1, edge2);
    }
    decide(a0, a1, a2, a3, &u0, &u1, &u2, &u3, &n0)
}

/// `edgesIntersectStable(mris, edge1, edge2)`: as [`edges_intersect`] with each edge's plane
/// through its midpoint `og = (v1 + v2) / 2` (normal `og x (v2 - v1)`) and the ends measured from
/// the other edge's midpoint. The midpoint's sum and the edge's difference are `float`
/// operations (C adds and subtracts the two `float` coordinates before widening); the rest is
/// `double`.
pub fn edges_intersect_stable(mris: &Mris, edge1: &Edge, edge2: &Edge) -> i32 {
    if shares_a_vertex(edge1, edge2) {
        return 0;
    }
    let c = |vno: i32| {
        let v = mris.vertex(vno as usize);
        [v.cx, v.cy, v.cz]
    };
    let mid = |p: &[f32; 3], q: &[f32; 3]| {
        [
            f64::from(p[0] + q[0]) / 2.0,
            f64::from(p[1] + q[1]) / 2.0,
            f64::from(p[2] + q[2]) / 2.0,
        ]
    };
    let diff = |p: &[f32; 3], q: &[f32; 3]| {
        [
            f64::from(q[0] - p[0]),
            f64::from(q[1] - p[1]),
            f64::from(q[2] - p[2]),
        ]
    };
    let sub = |p: &[f32; 3], o: &V3| {
        [
            f64::from(p[0]) - o[0],
            f64::from(p[1]) - o[1],
            f64::from(p[2]) - o[2],
        ]
    };
    let (p1, p2) = (c(edge1.vno1), c(edge1.vno2));
    let og0 = mid(&p1, &p2);
    let n0 = cross(&og0, &diff(&p1, &p2));
    let (q1, q2) = (c(edge2.vno1), c(edge2.vno2));
    let og1 = mid(&q1, &q2);
    let n1 = cross(&og1, &diff(&q1, &q2));
    let (u0, u1) = (sub(&p1, &og1), sub(&p2, &og1));
    let (u2, u3) = (sub(&q1, &og0), sub(&q2, &og0));
    let a0 = dot(&u0, &n1);
    let a1 = dot(&u1, &n1);
    let a2 = dot(&u2, &n0);
    let a3 = dot(&u3, &n0);
    decide(a0, a1, a2, a3, &u0, &u1, &u2, &u3, &n0)
}
