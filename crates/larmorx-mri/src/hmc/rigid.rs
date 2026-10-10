// SPDX-License-Identifier: Apache-2.0
//! Transform parameters and matrices (`specs/mcflirt.md` §4.4).
//!
//! The search variable is the 12-vector `(rx, ry, rz, tx, ty, tz, sx, sy, sz, kxy, kxz, kyz)`:
//! angles in radians, translations in mm, scales and skews. A vector and a centre `c` (an
//! FSL-mm point) mean the matrix `M(x) = L·(x − c) + c + t`, with `L = R·K·S`,
//! `R = Rx(rx)·Ry(ry)·Rz(rz)` (rotations by `−rx`, `−ry`, `−rz` in the usual right-handed
//! sense), `K` the unit upper-triangular skew and `S` the diagonal scales.

use larmorx_core::math;

/// A 4 × 4 matrix, row-major, double precision.
pub type Mat4 = [[f64; 4]; 4];

/// The 12 parameters.
pub type Params = [f64; 12];

/// The 4 × 4 identity.
pub const IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// The parameters of the identity: no rotation or translation, unit scales, no skew.
pub const IDENTITY_PARAMS: Params = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0];

/// An all-NaN transform (the result of a volume with a NaN voxel, §12): the top three rows
/// NaN, the last `0 0 0 1`.
pub const NAN_MATRIX: Mat4 = [
    [f64::NAN; 4],
    [f64::NAN; 4],
    [f64::NAN; 4],
    [0.0, 0.0, 0.0, 1.0],
];

/// `a · b`. NaN entries spread through the products (a NaN times 0 is NaN), as in a plain
/// matrix product.
pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, value) in row.iter_mut().enumerate() {
            *value = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + a[i][3] * b[3][j];
        }
    }
    out
}

/// The inverse of an affine matrix (last row `0 0 0 1`), by the adjugate of its linear part.
/// Singular or non-finite matrices give non-finite entries.
pub fn inverse(m: &Mat4) -> Mat4 {
    let a = |i: usize, j: usize| m[i][j];
    let cof = |r0: usize, r1: usize, c0: usize, c1: usize| a(r0, c0) * a(r1, c1) - a(r0, c1) * a(r1, c0);
    let adj = [
        [cof(1, 2, 1, 2), -cof(0, 2, 1, 2), cof(0, 1, 1, 2)],
        [-cof(1, 2, 0, 2), cof(0, 2, 0, 2), -cof(0, 1, 0, 2)],
        [cof(1, 2, 0, 1), -cof(0, 2, 0, 1), cof(0, 1, 0, 1)],
    ];
    let det = a(0, 0) * adj[0][0] + a(0, 1) * adj[1][0] + a(0, 2) * adj[2][0];
    let mut out = IDENTITY;
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = adj[i][j] / det;
        }
    }
    for i in 0..3 {
        out[i][3] = -(out[i][0] * m[0][3] + out[i][1] * m[1][3] + out[i][2] * m[2][3]);
    }
    out
}

/// A diagonal matrix `diag(d, 1)`.
pub fn diagonal(d: [f64; 3]) -> Mat4 {
    let mut m = IDENTITY;
    for (k, &v) in d.iter().enumerate() {
        m[k][k] = v;
    }
    m
}

/// A translation by `t`.
fn translation(t: [f64; 3]) -> Mat4 {
    let mut m = IDENTITY;
    for (k, &v) in t.iter().enumerate() {
        m[k][3] = v;
    }
    m
}

/// `R = Rx(rx)·Ry(ry)·Rz(rz)`, with `Rx(a) = [1 0 0; 0 cos a  sin a; 0 −sin a  cos a]` and
/// likewise (§4.4).
pub fn rotation(rx: f64, ry: f64, rz: f64) -> [[f64; 3]; 3] {
    let v = super::cost::variant();
    let sc = |a: f64| -> (f64, f64) {
        if v & 512 != 0 {
            let af = f64::from(a as f32);
            (f64::from(math::sin(af) as f32), f64::from(math::cos(af) as f32))
        } else if v & 256 != 0 {
            (f64::from(math::sin(a) as f32), f64::from(math::cos(a) as f32))
        } else if v & 2048 != 0 {
            (math::sin(a), f64::from(math::cos(a) as f32))
        } else {
            (math::sin(a), math::cos(a))
        }
    };
    let (sa, ca) = sc(rx);
    let (sb, cb) = sc(ry);
    let (sg, cg) = sc(rz);
    let mx = [[1.0, 0.0, 0.0], [0.0, ca, sa], [0.0, -sa, ca]];
    let my = [[cb, 0.0, -sb], [0.0, 1.0, 0.0], [sb, 0.0, cb]];
    let mz = [[cg, sg, 0.0], [-sg, cg, 0.0], [0.0, 0.0, 1.0]];
    if v & 4096 != 0 {
        let f = |m: [[f64; 3]; 3]| m.map(|r| r.map(|x| f64::from(x as f32)));
        return f(mul3(&f(mul3(&mx, &my)), &mz));
    }
    if v & 8192 != 0 {
        return mul3(&mz, &mul3(&my, &mx));
    }
    if v & 16384 != 0 {
        return mul3(&mx, &mul3(&my, &mz));
    }
    mul3(&mul3(&mx, &my), &mz)
}

fn mul3(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, value) in row.iter_mut().enumerate() {
            *value = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    out
}

fn embed(l: &[[f64; 3]; 3]) -> Mat4 {
    let mut m = IDENTITY;
    for i in 0..3 {
        m[i][..3].copy_from_slice(&l[i]);
    }
    m
}

/// The scales of a parameter vector composed with `n` entries: none below 7, one scale for all
/// three axes with 7, then `sx`, `sy`, `sz` as far as `n` reaches.
fn scales(p: &Params, n: usize) -> [f64; 3] {
    match n {
        0..=6 => [1.0; 3],
        7 => [p[6]; 3],
        8 => [p[6], p[7], 1.0],
        _ => [p[6], p[7], p[8]],
    }
}

/// The matrix of the first `n` parameters of `p` (at least the six rigid ones) about `centre`:
/// `T(c + t) · L · T(−c)`, as plain 4 × 4 products (so a NaN centre makes the top three rows
/// NaN), with the last row set to `0 0 0 1`.
pub fn compose(p: &Params, n: usize, centre: [f64; 3]) -> Mat4 {
    let mut linear = rotation(p[0], p[1], p[2]);
    if n > 6 {
        let s = scales(p, n);
        let skew = [
            [1.0, if n > 9 { p[9] } else { 0.0 }, if n > 10 { p[10] } else { 0.0 }],
            [0.0, 1.0, if n > 11 { p[11] } else { 0.0 }],
            [0.0, 0.0, 1.0],
        ];
        let ks = mul3(&skew, &[[s[0], 0.0, 0.0], [0.0, s[1], 0.0], [0.0, 0.0, s[2]]]);
        linear = mul3(&linear, &ks);
    }
    if super::cost::variant() & 32768 != 0 {
        // translation c + t - L c in f64, as an explicit formula
        let mut m = embed(&linear);
        for i in 0..3 {
            let lc = linear[i][0] * centre[0] + linear[i][1] * centre[1] + linear[i][2] * centre[2];
            m[i][3] = centre[i] + p[3 + i] - lc;
        }
        return m;
    }
    let shift = [centre[0] + p[3], centre[1] + p[4], centre[2] + p[5]];
    let mut m = mul(
        &mul(&translation(shift), &embed(&linear)),
        &translation(centre.map(|v| -v)),
    );
    // The last row of an affine stays exact (a NaN centre makes only the top rows NaN).
    m[3] = [0.0, 0.0, 0.0, 1.0];
    m
}

/// `atan2` (the `libm` crate's: `larmorx_core::math` has no correctly rounded port of it yet).
fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// The Euler angles of a rotation matrix (§4.4), from float32 intermediates.
fn euler(r: &[[f64; 3]; 3]) -> [f64; 3] {
    let cy = (r[0][0] * r[0][0] + r[0][1] * r[0][1]).sqrt() as f32;
    if cy < 1e-4 {
        let rx = atan2(-r[2][1], r[1][1]);
        let ry = atan2(-r[0][2], 0.0);
        [rx, ry, 0.0]
    } else {
        let c = f64::from(cy);
        let (cx, sx) = ((r[2][2] / c) as f32, (r[1][2] / c) as f32);
        let (cz, sz) = ((r[0][0] / c) as f32, (r[0][1] / c) as f32);
        [
            atan2(f64::from(sx), f64::from(cx)),
            atan2(-r[0][2], c),
            atan2(f64::from(sz), f64::from(cz)),
        ]
    }
}

/// The 12 parameters of `m` about `centre` (§4.4): `L = R·K·S` factored like a QR
/// decomposition (columns of `L` orthogonalised in order), the Euler angles of `R` from float32
/// intermediates, and `t = L·c + m − c`. A rigid `m` gives unit scales and no skew.
///
/// A matrix with a NaN entry gives NaN parameters, with the signs a C++ program printing them
/// shows: `nan -nan nan nan nan nan`.
pub fn decompose(m: &Mat4, centre: [f64; 3]) -> Params {
    if m.iter().take(3).flatten().any(|v| v.is_nan()) {
        let mut p = [f64::NAN; 12];
        p[1] = -f64::NAN;
        return p;
    }
    let l = [0, 1, 2].map(|i| [m[i][0], m[i][1], m[i][2]]);
    let col = |j: usize| [l[0][j], l[1][j], l[2][j]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let norm = |a: [f64; 3]| dot(a, a).sqrt();
    // Gram-Schmidt: L = Q·U, U = K·S upper triangular.
    let (c0, c1, c2) = (col(0), col(1), col(2));
    let sx = norm(c0);
    let q0 = c0.map(|v| v / sx);
    let u01 = dot(q0, c1);
    let v1 = [0, 1, 2].map(|k| c1[k] - u01 * q0[k]);
    let sy = norm(v1);
    let q1 = v1.map(|v| v / sy);
    let (u02, u12) = (dot(q0, c2), dot(q1, c2));
    let v2 = [0, 1, 2].map(|k| c2[k] - u02 * q0[k] - u12 * q1[k]);
    let sz = norm(v2);
    let q2 = v2.map(|v| v / sz);
    let rigid = (sx - 1.0).abs() < 1e-9
        && (sy - 1.0).abs() < 1e-9
        && (sz - 1.0).abs() < 1e-9
        && u01.abs() < 1e-9
        && u02.abs() < 1e-9
        && u12.abs() < 1e-9;
    let (r, s, k) = if rigid {
        (l, [1.0; 3], [0.0; 3])
    } else {
        let q = [0, 1, 2].map(|i| [q0[i], q1[i], q2[i]]);
        (q, [sx, sy, sz], [u01 / sy, u02 / sz, u12 / sz])
    };
    let angles = euler(&r);
    let t = [0, 1, 2].map(|i| {
        let lc = l[i][0] * centre[0] + l[i][1] * centre[1] + l[i][2] * centre[2];
        lc + m[i][3] - centre[i]
    });
    [
        angles[0], angles[1], angles[2], t[0], t[1], t[2], s[0], s[1], s[2], k[0], k[1], k[2],
    ]
}

/// Whether every parameter is finite.
pub fn is_finite(p: &Params) -> bool {
    p.iter().all(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Mat4, b: &Mat4, tol: f64) -> bool {
        a.iter().flatten().zip(b.iter().flatten()).all(|(x, y)| (x - y).abs() <= tol)
    }

    #[test]
    fn rotations_follow_the_sign_convention() {
        let r = rotation(0.04, 0.0, 0.0);
        assert!((r[1][2] - math::sin(0.04)).abs() < 1e-17);
        assert!((r[2][1] + math::sin(0.04)).abs() < 1e-17);
        let r = rotation(0.0, 0.04, 0.0);
        assert!((r[0][2] + math::sin(0.04)).abs() < 1e-17);
        let r = rotation(0.0, 0.0, 0.04);
        assert!((r[0][1] - math::sin(0.04)).abs() < 1e-17);
    }

    #[test]
    fn decomposition_inverts_composition() {
        let c = [80.0, 90.0, 60.0];
        let p: Params = [0.02, -0.03, 0.01, 1.5, -2.0, 0.5, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0];
        let m = compose(&p, 6, c);
        let q = decompose(&m, c);
        for k in 0..6 {
            assert!((p[k] - q[k]).abs() < 1e-6, "{k}: {} {}", p[k], q[k]);
        }
        assert!(close(&compose(&q, 6, c), &m, 1e-5));
        // An affine matrix round-trips through all 12 parameters.
        let p: Params = [0.02, -0.03, 0.01, 1.5, -2.0, 0.5, 1.1, 0.9, 1.05, 0.02, -0.01, 0.03];
        let m = compose(&p, 12, c);
        let q = decompose(&m, c);
        assert!(close(&compose(&q, 12, c), &m, 1e-5));
    }

    #[test]
    fn the_identity_decomposes_to_signed_zeros() {
        let p = decompose(&IDENTITY, [10.0, 20.0, 30.0]);
        assert_eq!(p[..6], [0.0; 6]);
        assert!(p[1].is_sign_negative() && p[0].is_sign_positive());
        assert!(p[3..6].iter().all(|v| v.is_sign_positive()));
    }

    #[test]
    fn a_nan_centre_gives_a_nan_matrix() {
        let m = compose(&IDENTITY_PARAMS, 6, [f64::NAN; 3]);
        assert!(m[..3].iter().flatten().all(|v| v.is_nan()));
        assert_eq!(m[3], [0.0, 0.0, 0.0, 1.0]);
        let p = decompose(&m, [0.0; 3]);
        assert!(p.iter().all(|v| v.is_nan()));
        assert!(p[1].is_sign_negative() && p[0].is_sign_positive());
    }

    #[test]
    fn inverse_of_an_affine() {
        let c = [10.0, -5.0, 3.0];
        let p: Params = [0.1, 0.2, -0.3, 1.0, 2.0, 3.0, 1.2, 0.8, 1.0, 0.1, 0.0, 0.0];
        let m = compose(&p, 12, c);
        assert!(close(&mul(&m, &inverse(&m)), &IDENTITY, 1e-12));
    }
}
