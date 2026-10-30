// SPDX-License-Identifier: Apache-2.0
//! The qform's quaternion as FSL writes it (`specs/bet.md` §3.3, §8.2): FSL turns the input's
//! quaternion into its 4 x 4 matrix and back, by the NIfTI-1 standard's conversions (the
//! reference library's, public domain, with a polar decomposition of the rotation), in the
//! precision of each step that §8.2 gives: floats for the matrix, double for most arithmetic,
//! x87 extended precision for a few products, quotients and sums ([`super::x87`]). For a
//! rotation by half a turn (`a = 0`) the two signs of `(b, c, d)` are the same rotation: the
//! conversion back picks one, which the output carries.

use super::x87;

type M33 = [[f64; 3]; 3];
type F33 = [[f32; 3]; 3];

/// The rotation-and-scale part of a quaternion qform (§8.2, steps 1 and 2), as the float
/// matrix FSL holds (entries widened to double). `b`, `c`, `d` and `pixdim` are read as floats.
pub fn quatern_to_mat(b: f64, c: f64, d: f64, pixdim: [f64; 4]) -> M33 {
    let m = quatern_to_float_mat(b, c, d, pixdim);
    m.map(|row| row.map(f64::from))
}

fn quatern_to_float_mat(b: f64, c: f64, d: f64, pixdim: [f64; 4]) -> F33 {
    // 1. floats, widened
    let fl = |x: f64| f64::from(x as f32);
    let (mut b, mut c, mut d) = (fl(b), fl(c), fl(d));
    let pixdim = pixdim.map(fl);
    // 2.
    let s = (b * b + c * c) + d * d;
    let mut a = x87::sub(1.0, s);
    if x87::below_ext_1e7(a) {
        a = x87::div(1.0, s.sqrt());
        b *= a;
        c *= a;
        d *= a;
        a = 0.0;
    } else {
        a = a.sqrt();
    }
    let pos = |x: f64| if x > 0.0 { x } else { 1.0 };
    let xd = pos(pixdim[1]);
    let yd = pos(pixdim[2]);
    let mut zd = pos(pixdim[3]);
    if pixdim[0] < 0.0 {
        zd = -zd;
    }
    // the diagonal in double, rounded to float; the off-diagonal: the bracket in double, the
    // two products in extended, rounded once to float
    let diag = |x: f64, s: f64| (x * s) as f32;
    let off = |bracket: f64, s: f64| x87::mul_to_f32(2.0 * bracket, s);
    [
        [
            diag(((a * a + b * b) - c * c) - d * d, xd),
            off(b * c - a * d, yd),
            off(b * d + a * c, zd),
        ],
        [
            off(b * c + a * d, xd),
            diag(((a * a + c * c) - b * b) - d * d, yd),
            off(c * d - a * b, zd),
        ],
        [
            off(b * d - a * c, xd),
            off(c * d + a * b, yd),
            diag(((a * a + d * d) - c * c) - b * b, zd),
        ],
    ]
}

/// The determinant from the float entries, in double, left to right (§8.2, step 5).
fn det(r: &F33) -> f64 {
    let [[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]] = r.map(|row| row.map(f64::from));
    r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
        - r31 * r22 * r13
}

/// The inverse (§8.2, step 5): in double from the float entries, the determinant's reciprocal
/// in extended precision rounded to double, each entry the reciprocal times its cofactor in
/// double, stored as float.
fn inverse(r: &F33) -> F33 {
    let [[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]] = r.map(|row| row.map(f64::from));
    let mut deti =
        r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
            - r31 * r22 * r13;
    if deti != 0.0 {
        deti = x87::div(1.0, deti);
    }
    let f = |x: f64| x as f32;
    [
        [
            f(deti * (r22 * r33 - r32 * r23)),
            f(deti * (-r12 * r33 + r32 * r13)),
            f(deti * (r12 * r23 - r22 * r13)),
        ],
        [
            f(deti * (-r21 * r33 + r31 * r23)),
            f(deti * (r11 * r33 - r31 * r13)),
            f(deti * (-r11 * r23 + r21 * r13)),
        ],
        [
            f(deti * (r21 * r32 - r31 * r22)),
            f(deti * (-r11 * r32 + r31 * r12)),
            f(deti * (r11 * r22 - r21 * r12)),
        ],
    ]
}

/// The largest row sum of absolute values, summed in float.
fn rownorm(r: &F33) -> f32 {
    let mut best = 0.0f32;
    for row in r {
        let s = (row[0].abs() + row[1].abs()) + row[2].abs();
        if s > best {
            best = s;
        }
    }
    best
}

/// The largest column sum of absolute values, summed in float.
fn colnorm(r: &F33) -> f32 {
    let mut best = 0.0f32;
    for j in 0..3 {
        let s = (r[0][j].abs() + r[1][j].abs()) + r[2][j].abs();
        if s > best {
            best = s;
        }
    }
    best
}

/// The orthogonal matrix nearest `a`, in float (§8.2, step 5).
fn polar(a: &F33) -> F33 {
    let mut x = *a;
    while det(&x) == 0.0 {
        let g = (0.00001 * (0.001 + f64::from(rownorm(&x)))) as f32;
        for k in 0..3 {
            x[k][k] += g;
        }
    }
    let mut dif = 1.0f32;
    let mut k = 0;
    loop {
        let y = inverse(&x);
        let (g, gi) = if f64::from(dif) > 0.3 {
            let alp = (rownorm(&x) * colnorm(&x)).sqrt();
            let bet = (rownorm(&y) * colnorm(&y)).sqrt();
            let g = (bet / alp).sqrt();
            (g, (1.0 / f64::from(g)) as f32)
        } else {
            (1.0, 1.0)
        };
        let z: F33 =
            std::array::from_fn(|i| std::array::from_fn(|j| 0.5 * (g * x[i][j] + gi * y[j][i])));
        dif = 0.0;
        for i in 0..3 {
            for j in 0..3 {
                dif += (z[i][j] - x[i][j]).abs();
            }
        }
        k += 1;
        if k > 100 || f64::from(dif) < 3.0e-6 {
            return z;
        }
        x = z;
    }
}

/// The quaternion `(b, c, d)` (doubles: NIfTI-2 stores them, NIfTI-1 their floats) and the
/// qfac of a float rotation-and-scale matrix (§8.2, steps 4 to 6).
pub fn mat_to_quatern(r: &M33) -> ([f64; 3], f64) {
    // 4. each column divided by its length in double, rounded to float
    let mut m = *r;
    for j in 0..3 {
        let mut n = ((m[0][j] * m[0][j] + m[1][j] * m[1][j]) + m[2][j] * m[2][j]).sqrt();
        if n == 0.0 {
            for i in 0..3 {
                m[i][j] = if i == j { 1.0 } else { 0.0 };
            }
            n = 1.0;
        }
        for row in &mut m {
            row[j] /= n;
        }
    }
    let mf: F33 = m.map(|row| row.map(|v| v as f32));
    // 5.
    let p = polar(&mf);
    // 6.
    let det_p = det(&p);
    let [
        [r11, r12, mut r13],
        [r21, r22, mut r23],
        [r31, r32, mut r33],
    ] = p.map(|row| row.map(f64::from));
    let qfac = if det_p > 0.0 {
        1.0
    } else {
        r13 = -r13;
        r23 = -r23;
        r33 = -r33;
        -1.0
    };
    let q = |x: f64, by: f64| x87::div(0.25 * x, by);
    let mut a = x87::add((r11 + r22) + r33, 1.0);
    let (mut b, mut c, mut d);
    if a > 0.5 {
        a = 0.5 * a.sqrt();
        b = q(r32 - r23, a);
        c = q(r13 - r31, a);
        d = q(r21 - r12, a);
    } else {
        let xd = 1.0 + r11 - (r22 + r33);
        let yd = 1.0 + r22 - (r11 + r33);
        let zd = 1.0 + r33 - (r11 + r22);
        if xd > 1.0 {
            b = 0.5 * xd.sqrt();
            c = q(r12 + r21, b);
            d = q(r13 + r31, b);
            a = q(r32 - r23, b);
        } else if yd > 1.0 {
            c = 0.5 * yd.sqrt();
            b = q(r12 + r21, c);
            d = q(r23 + r32, c);
            a = q(r13 - r31, c);
        } else {
            d = 0.5 * zd.sqrt();
            b = q(r13 + r31, d);
            c = q(r23 + r32, d);
            a = q(r21 - r12, d);
        }
        if a < 0.0 {
            b = -b;
            c = -c;
            d = -d;
        }
    }
    ([b, c, d], qfac)
}

/// The quaternion FSL writes for the input's `(b, c, d)` and `pixdim` (its qfac and sizes).
pub fn rewritten(b: f64, c: f64, d: f64, pixdim: [f64; 4]) -> ([f64; 3], f64) {
    mat_to_quatern(&quatern_to_mat(b, c, d, pixdim))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_turns_pick_a_sign() {
        // an oblique half turn (observed: FSL writes b, c, d with the other sign)
        let (q, qfac) = rewritten(
            -0.0,
            -0.995_004_177_093_505_9,
            -0.099_833_413_958_549_5,
            [-1.0, 2.0, 2.0, 2.0],
        );
        assert_eq!(qfac, -1.0);
        assert!(q[1] > 0.0 && q[2] > 0.0, "{q:?}");
        // the identity stays
        let (q, qfac) = rewritten(0.0, 0.0, 0.0, [1.0, 2.0, 2.0, 2.0]);
        assert_eq!((q, qfac), ([0.0; 3], 1.0));
    }
}
