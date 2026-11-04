// SPDX-License-Identifier: Apache-2.0
//! FLIRT's two parametrisations of the rotation (`specs/flirt.md` §3): Euler angles (the shared
//! engine's, `larmorx_mri::hmc::rigid`) and, with `-anglerep quaternion`, the vector part of a
//! unit quaternion. The search always uses Euler angles (§8.1).

use larmorx_core::math::Libm;
use larmorx_mri::hmc::rigid::{self, Params};

use super::cost::Log;
use super::mat::{self, IDENTITY, Mat4};

type M3 = [[f64; 3]; 3];

/// A dot product of three as Armadillo sums it (two interleaved accumulators): `(p₁ + p₃) + p₂`.
fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] * b[0] + a[2] * b[2]) + a[1] * b[1]
}

/// A 3 × 3 product in the small-matrix order (row by column, left to right).
fn mul3(a: &M3, b: &M3) -> M3 {
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = (a[i][0] * b[0][j] + a[i][1] * b[1][j]) + a[i][2] * b[2][j];
        }
    }
    out
}

/// A 3 × 3 matrix times a 3-vector (left to right).
fn mul3v(a: &M3, v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| (a[i][0] * v[0] + a[i][1] * v[1]) + a[i][2] * v[2])
}

/// A 3 × 3 linear part with a translation, as a 4 × 4 matrix.
fn affine(l: &M3, t: [f64; 3]) -> Mat4 {
    let mut m = IDENTITY;
    for i in 0..3 {
        m[i][..3].copy_from_slice(&l[i]);
        m[i][3] = t[i];
    }
    m
}

/// `l` about the centre `c`: its translation `c − l·c` (the product first).
fn about(l: &M3, c: [f64; 3]) -> Mat4 {
    let lc = mul3v(l, c);
    affine(l, [c[0] - lc[0], c[1] - lc[1], c[2] - lc[2]])
}

/// The rotation by `angle` about coordinate axis `axis` about the centre `c`, as FLIRT makes it
/// (§3): `θ = √(a·a)` rounded to single (the identity below 1e-8), `u = a/θ`, `v = (−u_y, u_x,
/// 0)/|v|` (or `(1, 0, 0)`), `w = (u × v)/|w|`, `B = [v w u]`, the rotation `(B·C)·Bᵀ` with `C`
/// the single-precision cosine and sine of `θ` in the `(v, w)` plane (`libm`'s: glibc's `cosf`
/// and `sinf` with `reproduce`, GLIBC-07), its translation `(I − R)·c`.
fn elementary(axis: usize, angle: f64, c: [f64; 3], libm: &Libm) -> Mat4 {
    let mut a = [0.0; 3];
    a[axis] = angle;
    let theta = dot3(a, a).sqrt() as f32;
    if theta < 1e-8 {
        return IDENTITY;
    }
    let t = f64::from(theta);
    let u = a.map(|x| x / t);
    let v0 = [-u[1], u[0], 0.0];
    let nv = dot3(v0, v0).sqrt();
    let v = if nv > 0.0 {
        v0.map(|x| x / nv)
    } else {
        [1.0, 0.0, 0.0]
    };
    let w0 = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let nw = dot3(w0, w0).sqrt();
    let w = w0.map(|x| x / nw);
    let b: M3 = [[v[0], w[0], u[0]], [v[1], w[1], u[1]], [v[2], w[2], u[2]]];
    let bt: M3 = [[v[0], v[1], v[2]], [w[0], w[1], w[2]], [u[0], u[1], u[2]]];
    let (cs, sn) = (f64::from(libm.cosf(theta)), f64::from(libm.sinf(theta)));
    let cm: M3 = [[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]];
    let r = mul3(&mul3(&b, &cm), &bt);
    let mut d = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            d[i][j] = if i == j { 1.0 } else { 0.0 } - r[i][j];
        }
    }
    affine(&r, mul3v(&d, c))
}

/// FLIRT's composition of the first `n` Euler parameters of `p` about `c` (§3, exactly): the
/// three elementary rotations `((I·R_x)·R_y)·R_z`, the translations added to the fourth column,
/// and with more than 6 parameters `(M·K)·S` with the skew and the scale about the centre.
pub fn euler_compose(p: &Params, n: usize, c: [f64; 3], libm: &Libm) -> Mat4 {
    let angle = |k: usize| if k < n { p[k] } else { 0.0 };
    let mut m = IDENTITY;
    for k in 0..3 {
        m = mat::mul(&m, &elementary(k, angle(k), c, libm));
    }
    for i in 0..3 {
        if 3 + i < n {
            m[i][3] += p[3 + i];
        }
    }
    if n > 6 {
        let s = match n {
            7 => [p[6]; 3],
            8 => [p[6], p[7], 1.0],
            _ => [p[6], p[7], p[8]],
        };
        let k = |i: usize| if n > 9 + i { p[9 + i] } else { 0.0 };
        let kk: M3 = [[1.0, k(0), k(1)], [0.0, 1.0, k(2)], [0.0, 0.0, 1.0]];
        let ss: M3 = [[s[0], 0.0, 0.0], [0.0, s[1], 0.0], [0.0, 0.0, s[2]]];
        m = mat::mul(&mat::mul(&m, &about(&kk, c)), &about(&ss, c));
    }
    m
}

/// FLIRT's decomposition of `m` about `c` (§3, exactly): the scales and skews from
/// single-precision steps, the rotation `A·(S⁻¹·K⁻¹)`, its angles by `atan2f` of
/// single-precision quotients, the translations `A·c + m − c`. Returns the 12 parameters and the
/// rotation. The arctangent is `libm`'s (glibc's `atan2f` with `reproduce`, GLIBC-07).
pub fn euler_decompose_parts(m: &Mat4, c: [f64; 3], libm: &Libm) -> (Params, M3) {
    let a: M3 = [0, 1, 2].map(|i| [m[i][0], m[i][1], m[i][2]]);
    let col = |j: usize| [a[0][j], a[1][j], a[2][j]];
    let (x, y, z) = (col(0), col(1), col(2));
    let sx = dot3(x, x).sqrt() as f32;
    let d = dot3(x, y) as f32;
    let yy = dot3(y, y) as f32;
    let sy = (yy - (d * d) / (sx * sx)).sqrt();
    let k1 = d / (sx * sy);
    let (sxd, syd, k1d) = (f64::from(sx), f64::from(sy), f64::from(k1));
    let x0 = x.map(|v| v / sxd);
    let y0 = [0, 1, 2].map(|i| y[i] / syd - k1d * x0[i]);
    let zz = dot3(z, z) as f32;
    let xz = dot3(x0, z) as f32;
    let yz = dot3(y0, z) as f32;
    let sz = ((zz - xz * xz) - yz * yz).sqrt();
    let (k2, k3) = (xz / sz, yz / sz);
    let (k2d, k3d) = (f64::from(k2), f64::from(k3));
    // K⁻¹, the unit upper triangle's inverse as §12 forms it (each column's entries times −1)
    let i01 = -k1d;
    let i02 = -(k3d * i01 + k2d);
    let i12 = -k3d;
    let inv: M3 = [[1.0, i01, i02], [0.0, 1.0, i12], [0.0, 0.0, 1.0]];
    let recip = [sxd, syd, f64::from(sz)].map(|v| 1.0 / v);
    let sk: M3 = [0, 1, 2].map(|i| inv[i].map(|v| v * recip[i]));
    let r = mul3(&a, &sk);
    let atan2f = |y: f32, x: f32| libm.atan2f(y, x);
    let cy = (r[0][0] * r[0][0] + r[0][1] * r[0][1]).sqrt() as f32;
    let angles = if cy >= 1e-4 {
        let cd = f64::from(cy);
        [
            atan2f((r[1][2] / cd) as f32, (r[2][2] / cd) as f32),
            atan2f((-r[0][2]) as f32, cy),
            atan2f((r[0][1] / cd) as f32, (r[0][0] / cd) as f32),
        ]
    } else {
        [
            atan2f((-r[2][1]) as f32, r[1][1] as f32),
            atan2f((-r[0][2]) as f32, 0.0),
            0.0,
        ]
    };
    let ac = mul3v(&a, c);
    let t = [0, 1, 2].map(|i| (ac[i] + m[i][3]) - c[i]);
    (
        [
            f64::from(angles[0]),
            f64::from(angles[1]),
            f64::from(angles[2]),
            t[0],
            t[1],
            t[2],
            sxd,
            syd,
            f64::from(sz),
            k1d,
            k2d,
            k3d,
        ],
        r,
    )
}

/// [`euler_decompose_parts`]'s parameters; a matrix with a NaN entry gives NaN parameters, with
/// the signs a C++ program printing them shows.
pub fn euler_decompose(m: &Mat4, c: [f64; 3], libm: &Libm) -> Params {
    if m.iter().take(3).flatten().any(|v| v.is_nan()) {
        return rigid::decompose(m, c);
    }
    euler_decompose_parts(m, c, libm).0
}

/// The rotation's parameters (`-anglerep`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AngleRep {
    #[default]
    Euler,
    Quaternion,
}

/// The rotation of the quaternion's vector part `(x, y, z)` (§3): `None` (FSL's message) when
/// `1 − x² − y² − z²` is negative.
fn quaternion_rotation(x: f64, y: f64, z: f64) -> Option<[[f64; 3]; 3]> {
    let w2 = (((1.0 - x * x) - y * y) - z * z) as f32;
    if w2 < 0.0 {
        return None;
    }
    let w = w2.sqrt();
    let (x, y, z) = (x as f32, y as f32, z as f32);
    let two = |a: f32, b: f32| (2.0 * a) * b;
    let r = [
        [
            (1.0 - two(y, y)) - two(z, z),
            two(x, y) - two(w, z),
            two(x, z) + two(w, y),
        ],
        [
            two(x, y) + two(w, z),
            (1.0 - two(x, x)) - two(z, z),
            two(y, z) - two(w, x),
        ],
        [
            two(x, z) - two(w, y),
            two(y, z) + two(w, x),
            (1.0 - two(x, x)) - two(y, y),
        ],
    ];
    Some(r.map(|row| row.map(f64::from)))
}

/// The matrix of the first `n` parameters of `p` about `centre` (§3), with `rep`'s rotation.
/// The quaternion's messages go to `log`.
pub fn compose(rep: AngleRep, p: &Params, n: usize, centre: [f64; 3], log: &Log) -> Mat4 {
    if rep == AngleRep::Euler {
        return euler_compose(p, n, centre, log.libm());
    }
    if n < 3 {
        log.push(format!("Can only do 3 or more, not {n}"));
    }
    let r = quaternion_rotation(p[0], p[1], p[2]).unwrap_or_else(|| {
        log.push("Parameters do not form a valid axis - greater than unity");
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    });
    let l = rigid::with_scales_skews(&r, p, n);
    let mut m = rigid::IDENTITY;
    let translations = match n {
        0..=3 => 3,
        4..=6 => n - 3,
        _ => 3,
    };
    for i in 0..3 {
        m[i][..3].copy_from_slice(&l[i]);
        let lc = l[i][0] * centre[0] + l[i][1] * centre[1] + l[i][2] * centre[2];
        let mut t = centre[i] - lc;
        if i < translations {
            t += p[3 + i];
        }
        m[i][3] = t;
    }
    m
}

/// The 12 parameters of `m` about `centre` (§3), with `rep`'s rotation: for quaternions the
/// rotation's entries in single precision turned into a quaternion as the NIfTI-1 reference
/// library does it (`bet::quatern`, shared with BET), its vector part. The functions are
/// `libm`'s.
pub fn decompose(rep: AngleRep, m: &Mat4, centre: [f64; 3], libm: &Libm) -> Params {
    if rep == AngleRep::Euler || m.iter().take(3).flatten().any(|v| v.is_nan()) {
        return euler_decompose(m, centre, libm);
    }
    let (mut p, r) = euler_decompose_parts(m, centre, libm);
    let rf = r.map(|row| row.map(|v| f64::from(v as f32)));
    let ([b, c, d], _) = crate::bet::quatern::mat_to_quatern(&rf);
    p[0] = b;
    p[1] = c;
    p[2] = d;
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_composition_is_the_rotation() {
        let c = [40.0, 50.0, 30.0];
        let ps: [Params; 3] = [
            [
                0.3, -0.2, 0.1, 1.0, -2.0, 0.5, 1.1, 0.9, 1.05, 0.02, -0.03, 0.01,
            ],
            [-1.2, 0.7, 2.5, 3.0, 2.0, -1.5, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
            [0.0, 1.5, 0.4, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
        ];
        for p in ps {
            let a = euler_compose(&p, 12, c, &Libm::default());
            let b = rigid::compose(&p, 12, c);
            for i in 0..3 {
                for j in 0..4 {
                    assert!(
                        (a[i][j] - b[i][j]).abs() < 1e-5,
                        "{p:?} {i} {j}: {a:?} {b:?}"
                    );
                }
            }
            let q = euler_decompose(&a, c, &Libm::default());
            let m = euler_compose(&q, 12, c, &Libm::default());
            for i in 0..3 {
                for j in 0..4 {
                    assert!((a[i][j] - m[i][j]).abs() < 1e-4, "{p:?} {q:?}");
                }
            }
        }
    }

    #[test]
    fn quaternions_round_trip() {
        let log = Log::default();
        let c = [40.0, 50.0, 30.0];
        let p: Params = [
            0.05, -0.02, 0.03, 1.0, -2.0, 0.5, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0,
        ];
        let m = compose(AngleRep::Quaternion, &p, 6, c, &log);
        let q = decompose(AngleRep::Quaternion, &m, c, log.libm());
        for k in 0..6 {
            assert!((p[k] - q[k]).abs() < 1e-5, "{k}: {} {}", p[k], q[k]);
        }
        assert!(log.take().is_empty());
        let bad: Params = [0.9, 0.9, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0];
        let m = compose(AngleRep::Quaternion, &bad, 6, c, &log);
        assert_eq!(m[0][0], 1.0);
        assert_eq!(
            log.take(),
            ["Parameters do not form a valid axis - greater than unity"]
        );
    }
}
