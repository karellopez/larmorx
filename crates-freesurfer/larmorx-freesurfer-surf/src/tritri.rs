// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surf's `utils/tritri.cpp` (queue item 3.4-04, taken early with the surface hash table for
//! `mrisMarkIntersections`; relay 39 of track D): Tomas Möller's triangle-triangle intersection
//! test (1997, "A Fast Triangle-Triangle Intersection Test", Journal of Graphics Tools 2(2)) as
//! FreeSurfer carries it (its epsilon tests, `EPSILON` 1e-6 and `BEPSILON` 1e-5), the ray and
//! triangle intersections; `project_point_to_plane`, the file's last function, is
//! [`crate::props2::project_point_to_plane`] (taken earlier).
//!
//! The intersection tests answer with an integer: a NaN's bits never reach it, so plain
//! arithmetic serves; `triangle_ray_intersect`'s point goes through fs.core's `sse` operations in
//! the C expressions' order (its NaNs as the official build's).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_decomp::matrix_svd_inverse;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sqrt64, sub64};

/// `EPSILON` (`tritri.cpp`).
const EPSILON: f64 = 0.000001;
/// `BEPSILON` (`0.000001 * 10`, folded).
const BEPSILON: f64 = 0.000001 * 10.0;

type V3 = [f64; 3];

/// `(float) x` of a `double` as x86-64 converts it (`cvtsd2ss`): a NaN keeps its sign and the top
/// of its payload, quieted.
fn narrow(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits();
        f32::from_bits(
            ((b >> 32) as u32 & 0x8000_0000) | 0x7fc0_0000 | ((b >> 29) as u32 & 0x003f_ffff),
        )
    } else {
        x as f32
    }
}

/// `(double) x` of a `float` (`cvtss2sd`): a NaN keeps its sign and payload, quieted.
fn widen(x: f32) -> f64 {
    if x.is_nan() {
        let b = x.to_bits();
        f64::from_bits(
            (u64::from(b & 0x8000_0000) << 32)
                | 0x7ff8_0000_0000_0000
                | (u64::from(b & 0x003f_ffff) << 29),
        )
    } else {
        f64::from(x)
    }
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `ISECT(VV0, VV1, VV2, D0, D1, D2)`: `VV0 + (VV1 - VV0) * D0 / (D0 - D1)` and the same with
/// `VV2`, `D2`.
fn isect(vv0: f64, vv1: f64, vv2: f64, d0: f64, d1: f64, d2: f64) -> [f64; 2] {
    [
        vv0 + (vv1 - vv0) * d0 / (d0 - d1),
        vv0 + (vv2 - vv0) * d0 / (d0 - d2),
    ]
}

/// `COMPUTE_INTERVALS`: the interval of one triangle on the line, or `None` when the triangles
/// are coplanar (the macro's `return coplanar_tri_tri(...)`).
#[allow(clippy::too_many_arguments)]
fn compute_intervals(
    vv0: f64,
    vv1: f64,
    vv2: f64,
    d0: f64,
    d1: f64,
    d2: f64,
    d0d1: f64,
    d0d2: f64,
) -> Option<[f64; 2]> {
    Some(if d0d1 > 0.0 {
        isect(vv2, vv0, vv1, d2, d0, d1)
    } else if d0d2 > 0.0 {
        isect(vv1, vv0, vv2, d1, d0, d2)
    } else if d1 * d2 > 0.0 || d0 != 0.0 {
        isect(vv0, vv1, vv2, d0, d1, d2)
    } else if d1 != 0.0 {
        isect(vv1, vv0, vv2, d1, d0, d2)
    } else if d2 != 0.0 {
        isect(vv2, vv0, vv1, d2, d0, d1)
    } else {
        return None;
    })
}

/// `EDGE_EDGE_TEST(V0, U0, U1)` with the edge `Ax`, `Ay` (Franklin Antonio's test).
fn edge_edge_test(v0: V3, u0: V3, u1: V3, ax: f64, ay: f64, i0: usize, i1: usize) -> bool {
    let bx = u0[i0] - u1[i0];
    let by = u0[i1] - u1[i1];
    let cx = v0[i0] - u0[i0];
    let cy = v0[i1] - u0[i1];
    let f = ay * bx - ax * by;
    let d = by * cx - bx * cy;
    if (f > 0.0 && d >= 0.0 && d <= f) || (f < 0.0 && d <= 0.0 && d >= f) {
        let e = ax * cy - ay * cx;
        if f > 0.0 {
            if e >= 0.0 && e <= f {
                return true;
            }
        } else if e <= 0.0 && e >= f {
            return true;
        }
    }
    false
}

/// `EDGE_AGAINST_TRI_EDGES(V0, V1, U0, U1, U2)`.
fn edge_against_tri_edges(v0: V3, v1: V3, u: [V3; 3], i0: usize, i1: usize) -> bool {
    let ax = v1[i0] - v0[i0];
    let ay = v1[i1] - v0[i1];
    edge_edge_test(v0, u[0], u[1], ax, ay, i0, i1)
        || edge_edge_test(v0, u[1], u[2], ax, ay, i0, i1)
        || edge_edge_test(v0, u[2], u[0], ax, ay, i0, i1)
}

/// `POINT_IN_TRI(V0, U0, U1, U2)`: whether `V0` is inside the triangle (`d0 d1 > 0` and
/// `d0 d2 > 0`).
fn point_in_tri(v0: V3, u0: V3, u1: V3, u2: V3, i0: usize, i1: usize) -> bool {
    let side = |p: V3, q: V3| {
        let a = q[i1] - p[i1];
        let b = -(q[i0] - p[i0]);
        let c = -a * p[i0] - b * p[i1];
        a * v0[i0] + b * v0[i1] + c
    };
    let d0 = side(u0, u1);
    let d1 = side(u1, u2);
    let d2 = side(u2, u0);
    d0 * d1 > 0.0 && d0 * d2 > 0.0
}

/// `coplanar_tri_tri(N, V0, V1, V2, U0, U1, U2)`: the triangles projected onto the axis plane
/// where `N` is largest (`|N0| > |N1|`: `|N0| > |N2|` drops x, else z; otherwise `|N2| > |N1|`
/// drops z, else y), each edge of the first tested against the second's edges, then whether
/// `V0` lies in the second or `U0` in the first. 1 or 0.
pub fn coplanar_tri_tri(n: V3, v0: V3, v1: V3, v2: V3, u0: V3, u1: V3, u2: V3) -> i32 {
    let a = [n[0].abs(), n[1].abs(), n[2].abs()];
    let (i0, i1) = if a[0] > a[1] {
        if a[0] > a[2] { (1, 2) } else { (0, 1) }
    } else if a[2] > a[1] {
        (0, 1)
    } else {
        (0, 2)
    };
    let u = [u0, u1, u2];
    let hit = edge_against_tri_edges(v0, v1, u, i0, i1)
        || edge_against_tri_edges(v1, v2, u, i0, i1)
        || edge_against_tri_edges(v2, v0, u, i0, i1)
        || point_in_tri(v0, u0, u1, u2, i0, i1)
        || point_in_tri(u0, v0, v1, v2, i0, i1);
    i32::from(hit)
}

/// `tri_tri_intersect(V0, V1, V2, U0, U1, U2)`: 1 when the triangles intersect, else 0. The
/// second triangle's signed distances to the first's plane (`N1 = (V1 - V0) x (V2 - V0)`,
/// `d1 = -N1 . V0`): all of one sign (products above 0) with any magnitude above `EPSILON` gives
/// 0; each magnitude below `EPSILON` becomes 0; all three below `BEPSILON`, `BEPSILON` and
/// `EPSILON` make every distance and product 0. The same for the first triangle against the
/// second's plane. Then each triangle's interval on the line `D = N1 x N2`, projected onto
/// `D`'s largest component (the first largest of ties), and 0 when the sorted intervals do not
/// overlap; coplanar triangles (every distance 0) are [`coplanar_tri_tri`] with `N1`.
pub fn tri_tri_intersect(v0: V3, v1: V3, v2: V3, u0: V3, u1: V3, u2: V3) -> i32 {
    let e1 = sub(v1, v0);
    let e2 = sub(v2, v0);
    let n1 = cross(e1, e2);
    let d1 = -dot(n1, v0);
    let mut du0 = dot(n1, u0) + d1;
    let mut du1 = dot(n1, u1) + d1;
    let mut du2 = dot(n1, u2) + d1;
    let (fdu0, fdu1, fdu2) = (du0.abs(), du1.abs(), du2.abs());
    let mut du0du1 = du0 * du1;
    let mut du0du2 = du0 * du2;
    if (fdu0 > EPSILON || fdu1 > EPSILON || fdu2 > EPSILON) && (du0du1 > 0.0 && du0du2 > 0.0) {
        return 0;
    }
    if fdu0 < EPSILON {
        du0 = 0.0;
    }
    if fdu1 < EPSILON {
        du1 = 0.0;
    }
    if fdu2 < EPSILON {
        du2 = 0.0;
    }
    du0du1 = du0 * du1;
    du0du2 = du0 * du2;
    if fdu0 < BEPSILON && fdu1 < BEPSILON && fdu2 < EPSILON {
        (du0du1, du0du2, du0, du1, du2) = (0.0, 0.0, 0.0, 0.0, 0.0);
    }

    let e1 = sub(u1, u0);
    let e2 = sub(u2, u0);
    let n2 = cross(e1, e2);
    let d2 = -dot(n2, u0);
    let mut dv0 = dot(n2, v0) + d2;
    let mut dv1 = dot(n2, v1) + d2;
    let mut dv2 = dot(n2, v2) + d2;
    let (fdv0, fdv1, fdv2) = (dv0.abs(), dv1.abs(), dv2.abs());
    let mut dv0dv1 = dv0 * dv1;
    let mut dv0dv2 = dv0 * dv2;
    if (fdv0 > EPSILON || fdv1 > EPSILON || fdv2 > EPSILON) && (dv0dv1 > 0.0 && dv0dv2 > 0.0) {
        return 0;
    }
    if dv0.abs() < EPSILON {
        dv0 = 0.0;
    }
    if dv1.abs() < EPSILON {
        dv1 = 0.0;
    }
    if dv2.abs() < EPSILON {
        dv2 = 0.0;
    }
    dv0dv1 = dv0 * dv1;
    dv0dv2 = dv0 * dv2;
    if fdv0 < BEPSILON && fdv1 < BEPSILON && fdv2 < EPSILON {
        (dv0dv1, dv0dv2, dv0, dv1, dv2) = (0.0, 0.0, 0.0, 0.0, 0.0);
    }

    let d = cross(n1, n2);
    let mut max = d[0].abs();
    let mut index = 0usize;
    let b = d[1].abs();
    let c = d[2].abs();
    if b > max {
        (max, index) = (b, 1);
    }
    if c > max {
        index = 2;
    }
    let (vp0, vp1, vp2) = (v0[index], v1[index], v2[index]);
    let (up0, up1, up2) = (u0[index], u1[index], u2[index]);
    let Some(mut isect1) = compute_intervals(vp0, vp1, vp2, dv0, dv1, dv2, dv0dv1, dv0dv2) else {
        return coplanar_tri_tri(n1, v0, v1, v2, u0, u1, u2);
    };
    let Some(mut isect2) = compute_intervals(up0, up1, up2, du0, du1, du2, du0du1, du0du2) else {
        return coplanar_tri_tri(n1, v0, v1, v2, u0, u1, u2);
    };
    if isect1[0] > isect1[1] {
        isect1.swap(0, 1);
    }
    if isect2[0] > isect2[1] {
        isect2.swap(0, 1);
    }
    if isect1[1] < isect2[0] || isect2[1] < isect1[0] {
        return 0;
    }
    1
}

/// `sqrt(DOT(v, v))` with SSE's NaNs.
fn vlen(v: V3) -> f64 {
    sqrt64(add64(
        add64(mul64(v[0], v[0]), mul64(v[1], v[1])),
        mul64(v[2], v[2]),
    ))
}

fn dot_sse(a: V3, b: V3) -> f64 {
    add64(
        add64(mul64(a[0], b[0]), mul64(a[1], b[1])),
        mul64(a[2], b[2]),
    )
}

fn sub_sse(a: V3, b: V3) -> V3 {
    [sub64(a[0], b[0]), sub64(a[1], b[1]), sub64(a[2], b[2])]
}

fn cross_sse(a: V3, b: V3) -> V3 {
    [
        sub64(mul64(a[1], b[2]), mul64(a[2], b[1])),
        sub64(mul64(a[2], b[0]), mul64(a[0], b[2])),
        sub64(mul64(a[0], b[1]), mul64(a[1], b[0])),
    ]
}

/// `triangle_ray_intersect(orig_pt, dir, U0, U1, U2, int_pt)`: the plane's basis (`U1 - U0`, and
/// `(U1 - U0) x ((U1 - U0) x (U2 - U0))`), each normalized (a length below `FLT_EPSILON` returns
/// 0 at once, `int_pt` untouched); the 3 x 3 `float` matrix of the two basis vectors and `-dir`
/// as columns inverted by `MatrixSVDInverse` (a NULL inverse reaches `MatrixMultiply`'s fatal
/// `m1 is null`), times `orig_pt - U0` in `float`; its third element `t` gives
/// `int_pt = orig_pt + t dir`. Then for each leg in turn (U0 U1 opposite U2, U1 U2 opposite U0,
/// U2 U0 opposite U1) the point must not lie on the far side: the dot product of the leg's
/// discriminant (the opposite corner less its projection onto the leg) with the point less that
/// projection below 0 and not within 1e-15 of 0 returns 0. Else 1.
#[allow(clippy::too_many_arguments)]
pub fn triangle_ray_intersect(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    orig_pt: V3,
    dir: V3,
    u0: V3,
    u1: V3,
    u2: V3,
    int_pt: &mut V3,
) -> Result<i32, Fatal> {
    let fzero = |x: f64| x.abs() < f64::from(f32::EPSILON);
    let mut m_u = matrix_alloc(g, out, err, 3, 3, MATRIX_REAL)?.expect("a 3 x 3 matrix");
    let mut v_p = matrix_alloc(g, out, err, 3, 1, MATRIX_REAL)?.expect("a vector");
    let mut v_r = matrix_alloc(g, out, err, 3, 1, MATRIX_REAL)?.expect("a vector");
    let mut basis1 = sub_sse(u1, u0);
    let mut basis2 = sub_sse(u2, u0);
    let tmp = cross_sse(basis1, basis2);
    basis2 = cross_sse(basis1, tmp);
    let len = vlen(basis1);
    if fzero(len) {
        return Ok(0);
    }
    for b in &mut basis1 {
        *b = div64(*b, len);
    }
    let len = vlen(basis2);
    if fzero(len) {
        return Ok(0);
    }
    for b in &mut basis2 {
        *b = div64(*b, len);
    }
    for r in 0..3 {
        m_u.set_relt(r as i32 + 1, 1, narrow(basis1[r]));
        m_u.set_relt(r as i32 + 1, 2, narrow(basis2[r]));
        m_u.set_relt(r as i32 + 1, 3, narrow(-dir[r]));
    }
    let inv = matrix_svd_inverse(g, out, err, &m_u, None)?.into_new();
    let point = sub_sse(orig_pt, u0);
    for r in 0..3 {
        v_p.set_relt(r as i32 + 1, 1, narrow(point[r]));
    }
    matrix_multiply(g, out, err, inv.as_ref(), Some(&v_p), Some(&mut v_r))?;
    let t = widen(v_r.relt(3, 1));
    for k in 0..3 {
        int_pt[k] = add64(orig_pt[k], mul64(t, dir[k]));
    }
    for i in 0..3 {
        let (a, b, c) = match i {
            0 => (u0, u1, u2),
            1 => (u1, u2, u0),
            _ => (u2, u0, u1),
        };
        let mut l0 = sub_sse(b, a);
        let l1 = sub_sse(c, a);
        let len = vlen(l0);
        for x in &mut l0 {
            *x = div64(*x, len);
        }
        let d = dot_sse(l0, l1);
        let norm_proj = [mul64(l0[0], d), mul64(l0[1], d), mul64(l0[2], d)];
        let desc = sub_sse(l1, norm_proj);
        let point = sub_sse(sub_sse(*int_pt, a), norm_proj);
        let d = dot_sse(desc, point);
        if d < 0.0 && d.abs() >= 1e-15 {
            return Ok(0);
        }
    }
    Ok(1)
}

/// `intersect_RayTriangle(ray, V0, V1, V2, I)` (softsurfer's algorithm): -1 for a degenerate
/// triangle (`|u x v|` 0), else with `a = -n . (ray0 - V0)` and `b = n . dir` (`float`s): when
/// `|b| < 1e-8`, 2 if `a` is 0 (the ray in the plane) else 0; `r = a / b` below 0 gives 0;
/// `I = ray0 + r dir` (written); then the parametric coordinates in `float` (`s`, `t` from the
/// dot products `uu`, `uv`, `vv`, `wu`, `wv`, `D = uv uv - uu vv`): `s` outside [0, 1], `t`
/// below 0 or `s + t` above 1 give 0. Else 1.
pub fn intersect_ray_triangle(ray: [V3; 2], v0: V3, v1: V3, v2: V3, i_out: &mut V3) -> i32 {
    let u = sub_sse(v1, v0);
    let v = sub_sse(v2, v0);
    let n = cross_sse(u, v);
    if vlen(n) == 0.0 {
        return -1;
    }
    let mut dir = sub_sse(ray[1], ray[0]);
    let w0 = sub_sse(ray[0], v0);
    let a = narrow(-dot_sse(n, w0));
    let b = narrow(dot_sse(n, dir));
    if f64::from(b.abs()) < 1e-8 {
        return if a == 0.0 { 2 } else { 0 };
    }
    let r = larmorx_freesurfer_core::sse::div(a, b);
    if f64::from(r) < 0.0 {
        return 0;
    }
    let r = widen(r);
    for x in &mut dir {
        *x = mul64(*x, r);
    }
    for k in 0..3 {
        i_out[k] = add64(ray[0][k], dir[k]);
    }
    use larmorx_freesurfer_core::sse::{div as div32, mul as mul32, sub as sub32};
    let uu = narrow(dot_sse(u, u));
    let uv = narrow(dot_sse(u, v));
    let vv = narrow(dot_sse(v, v));
    let w = sub_sse(*i_out, v0);
    let wu = narrow(dot_sse(w, u));
    let wv = narrow(dot_sse(w, v));
    let d = sub32(mul32(uv, uv), mul32(uu, vv));
    let s = div32(sub32(mul32(uv, wv), mul32(vv, wu)), d);
    if f64::from(s) < 0.0 || f64::from(s) > 1.0 {
        return 0;
    }
    let t = div32(sub32(mul32(uv, wu), mul32(uu, wv)), d);
    if f64::from(t) < 0.0 || f64::from(larmorx_freesurfer_core::sse::add(s, t)) > 1.0 {
        return 0;
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_and_apart() {
        let v = ([0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]);
        let crossing = ([0.5, 0.5, -1.0], [0.5, 0.5, 1.0], [1.5, 0.5, 0.0]);
        assert_eq!(
            tri_tri_intersect(v.0, v.1, v.2, crossing.0, crossing.1, crossing.2),
            1
        );
        let above = ([0.5, 0.5, 1.0], [0.5, 0.5, 2.0], [1.5, 0.5, 1.5]);
        assert_eq!(
            tri_tri_intersect(v.0, v.1, v.2, above.0, above.1, above.2),
            0
        );
        let coplanar = ([0.5, 0.5, 0.0], [3.0, 0.5, 0.0], [0.5, 3.0, 0.0]);
        assert_eq!(
            tri_tri_intersect(v.0, v.1, v.2, coplanar.0, coplanar.1, coplanar.2),
            1
        );
        let ray = [[0.5, 0.5, 1.0], [0.5, 0.5, 0.0]];
        let mut p = [0.0; 3];
        assert_eq!(intersect_ray_triangle(ray, v.0, v.1, v.2, &mut p), 1);
        assert_eq!(p, [0.5, 0.5, 0.0]);
    }
}
