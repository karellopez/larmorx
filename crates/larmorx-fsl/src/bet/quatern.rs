// SPDX-License-Identifier: Apache-2.0
//! The qform's quaternion as FSL writes it (`specs/bet.md` §3.3, §8; observed): FSL turns the
//! input's quaternion into its 4 x 4 matrix and back, by the NIfTI-1 standard's conversions
//! (the reference library's `quatern_to_mat44` and `mat44_to_quatern`, public domain, with a
//! polar decomposition of the rotation). For a rotation by half a turn (`a = 0`) the two signs
//! of `(b, c, d)` are the same rotation: the conversion back picks one, which the output
//! carries. Computed in double precision (FSL's library uses x87 extended precision in places,
//! which can change a last bit).

type M33 = [[f64; 3]; 3];

/// The rotation-and-scale part and the qfac of a quaternion qform (`quatern_to_mat44`),
/// rounded to float as the standard's `mat44` holds it.
pub fn quatern_to_mat(b: f64, c: f64, d: f64, pixdim: [f64; 4]) -> M33 {
    let (mut b, mut c, mut d) = (b, c, d);
    let mut a = 1.0 - (b * b + c * c + d * d);
    if a < 1.0e-7 {
        a = 1.0 / (b * b + c * c + d * d).sqrt();
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
    let f = |x: f64| f64::from(x as f32);
    [
        [
            f((a * a + b * b - c * c - d * d) * xd),
            f(2.0 * (b * c - a * d) * yd),
            f(2.0 * (b * d + a * c) * zd),
        ],
        [
            f(2.0 * (b * c + a * d) * xd),
            f((a * a + c * c - b * b - d * d) * yd),
            f(2.0 * (c * d - a * b) * zd),
        ],
        [
            f(2.0 * (b * d - a * c) * xd),
            f(2.0 * (c * d + a * b) * yd),
            f((a * a + d * d - c * c - b * b) * zd),
        ],
    ]
}

fn det(r: &M33) -> f64 {
    r[0][0] * r[1][1] * r[2][2] - r[0][0] * r[2][1] * r[1][2] - r[1][0] * r[0][1] * r[2][2]
        + r[1][0] * r[2][1] * r[0][2]
        + r[2][0] * r[0][1] * r[1][2]
        - r[2][0] * r[1][1] * r[0][2]
}

fn inverse(r: &M33) -> M33 {
    let [[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]] = *r;
    let mut deti =
        r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
            - r31 * r22 * r13;
    if deti != 0.0 {
        deti = 1.0 / deti;
    }
    [
        [
            deti * (r22 * r33 - r32 * r23),
            deti * (-r12 * r33 + r32 * r13),
            deti * (r12 * r23 - r22 * r13),
        ],
        [
            deti * (-r21 * r33 + r31 * r23),
            deti * (r11 * r33 - r31 * r13),
            deti * (-r11 * r23 + r21 * r13),
        ],
        [
            deti * (r21 * r32 - r31 * r22),
            deti * (-r11 * r32 + r31 * r12),
            deti * (r11 * r22 - r21 * r12),
        ],
    ]
}

fn rownorm(r: &M33) -> f64 {
    r.iter()
        .map(|row| row.iter().map(|v| v.abs()).sum::<f64>())
        .fold(0.0, f64::max)
}

fn colnorm(r: &M33) -> f64 {
    (0..3)
        .map(|j| (0..3).map(|i| r[i][j].abs()).sum::<f64>())
        .fold(0.0, f64::max)
}

/// The orthogonal matrix nearest `a` (the standard's iterative polar decomposition).
fn polar(a: &M33) -> M33 {
    let mut x = *a;
    let mut gam = det(&x);
    while gam == 0.0 {
        gam = 0.00001 * (0.001 + rownorm(&x));
        for k in 0..3 {
            x[k][k] += gam;
        }
        gam = det(&x);
    }
    let mut dif = 1.0f64;
    let mut k = 0;
    loop {
        let y = inverse(&x);
        let (g, gi) = if dif > 0.3 {
            let alp = (rownorm(&x) * colnorm(&x)).sqrt();
            let bet = (rownorm(&y) * colnorm(&y)).sqrt();
            let g = (bet / alp).sqrt();
            (g, 1.0 / g)
        } else {
            (1.0, 1.0)
        };
        let z: M33 =
            std::array::from_fn(|i| std::array::from_fn(|j| 0.5 * (g * x[i][j] + gi * y[j][i])));
        dif = (0..3)
            .flat_map(|i| (0..3).map(move |j| (i, j)))
            .map(|(i, j)| (z[i][j] - x[i][j]).abs())
            .sum();
        k += 1;
        if k > 100 || dif < 3.0e-6 {
            return z;
        }
        x = z;
    }
}

/// `mat44_to_quatern`: the quaternion `(b, c, d)` (as float) and the qfac of a rotation-and-
/// scale matrix.
pub fn mat_to_quatern(r: &M33) -> ([f32; 3], f64) {
    let mut m = *r;
    for j in 0..3 {
        let mut n = (m[0][j] * m[0][j] + m[1][j] * m[1][j] + m[2][j] * m[2][j]).sqrt();
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
    let p = polar(&m);
    let [
        [r11, r12, mut r13],
        [r21, r22, mut r23],
        [r31, r32, mut r33],
    ] = p;
    let qfac = if det(&p) > 0.0 {
        1.0
    } else {
        r13 = -r13;
        r23 = -r23;
        r33 = -r33;
        -1.0
    };
    let mut a = r11 + r22 + r33 + 1.0;
    let (mut b, mut c, mut d);
    if a > 0.5 {
        a = 0.5 * a.sqrt();
        b = 0.25 * (r32 - r23) / a;
        c = 0.25 * (r13 - r31) / a;
        d = 0.25 * (r21 - r12) / a;
    } else {
        let xd = 1.0 + r11 - (r22 + r33);
        let yd = 1.0 + r22 - (r11 + r33);
        let zd = 1.0 + r33 - (r11 + r22);
        if xd > 1.0 {
            b = 0.5 * xd.sqrt();
            c = 0.25 * (r12 + r21) / b;
            d = 0.25 * (r13 + r31) / b;
            a = 0.25 * (r32 - r23) / b;
        } else if yd > 1.0 {
            c = 0.5 * yd.sqrt();
            b = 0.25 * (r12 + r21) / c;
            d = 0.25 * (r23 + r32) / c;
            a = 0.25 * (r13 - r31) / c;
        } else {
            d = 0.5 * zd.sqrt();
            b = 0.25 * (r13 + r31) / d;
            c = 0.25 * (r23 + r32) / d;
            a = 0.25 * (r21 - r12) / d;
        }
        if a < 0.0 {
            b = -b;
            c = -c;
            d = -d;
        }
    }
    ([b as f32, c as f32, d as f32], qfac)
}

/// The quaternion FSL writes for the input's `(b, c, d)` and `pixdim` (its qfac and sizes).
pub fn rewritten(b: f64, c: f64, d: f64, pixdim: [f64; 4]) -> ([f32; 3], f64) {
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
