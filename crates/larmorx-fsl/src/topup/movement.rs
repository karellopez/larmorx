// SPDX-License-Identifier: Apache-2.0
//! The volumes' rigid-body movements (`specs/topup.md` §4.3, §6): six parameters per volume,
//! three translations (mm) and three rotations (radians).
//!
//! Observed (black-box runs of FSL 6.0.7.23, `docs/findings/fsl-topup-original.md`):
//! - the rotation is `R = Rx(rx) · Ry(ry) · Rz(rz)` with `Rx = [[1, 0, 0], [0, c, s], [0, -s,
//!   c]]`, `Ry = [[c, 0, -s], [0, 1, 0], [s, 0, c]]`, `Rz = [[c, s, 0], [-s, c, 0], [0, 0, 1]]`
//!   (FSL's `--rbmout` matrices of the four-volume case, rotations up to 0.05 rad: 1.5e-7);
//! - `--rbmout`'s matrix is `[R | t + c - R c]`, `c` the input's centre `((n - 1) · voxel) / 2`;
//! - a volume is sampled at `R^T (x - c - t) + c` (the inverse of that matrix), `c` the centre of
//!   the image being interpolated (the level's subsampled, regridded grid: "its own grid"), and
//!   the field's displacement is added after the movement: FSL's corrected volumes at every
//!   cost evaluation of `debug/movements` agree with this to 2e-6.
//!
//! FSL builds its matrices from single-precision angles, cosines and sines with an axis entry
//! `q²` slightly different from 1 (`specs/topup.md` §4.3, spec writer 2026-11-03); its movement
//! derivatives are forward differences of the sampling positions those matrices give (§4.6):
//! FSL-201, whose `reproduce` uses [`fsl_matrix`] and [`Motion::fsl`]; the default builds the
//! rotations in double precision and takes the exact derivatives.

/// A 3 × 3 matrix, by rows.
pub type Mat3 = [[f64; 3]; 3];

fn mul(a: &Mat3, b: &Mat3) -> Mat3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

/// The three elementary rotations and their derivatives.
fn elementary(r: [f64; 3]) -> ([Mat3; 3], [Mat3; 3]) {
    // correctly rounded (rule 5): the same bits on every platform
    let sc = |a: f64| (larmorx_core::math::sin(a), larmorx_core::math::cos(a));
    let (sx, cx) = sc(r[0]);
    let (sy, cy) = sc(r[1]);
    let (sz, cz) = sc(r[2]);
    let rx = [[1.0, 0.0, 0.0], [0.0, cx, sx], [0.0, -sx, cx]];
    let ry = [[cy, 0.0, -sy], [0.0, 1.0, 0.0], [sy, 0.0, cy]];
    let rz = [[cz, sz, 0.0], [-sz, cz, 0.0], [0.0, 0.0, 1.0]];
    let dx = [[0.0, 0.0, 0.0], [0.0, -sx, cx], [0.0, -cx, -sx]];
    let dy = [[-sy, 0.0, -cy], [0.0, 0.0, 0.0], [cy, 0.0, -sy]];
    let dz = [[-sz, cz, 0.0], [-cz, -sz, 0.0], [0.0, 0.0, 0.0]];
    ([rx, ry, rz], [dx, dy, dz])
}

/// `R(rx, ry, rz)`.
pub fn rotation(r: [f64; 3]) -> Mat3 {
    let (e, _) = elementary(r);
    mul(&mul(&e[0], &e[1]), &e[2])
}

/// `∂R / ∂r_k` for `k` = x, y, z.
pub fn rotation_derivatives(r: [f64; 3]) -> [Mat3; 3] {
    let (e, d) = elementary(r);
    [
        mul(&mul(&d[0], &e[1]), &e[2]),
        mul(&mul(&e[0], &d[1]), &e[2]),
        mul(&mul(&e[0], &e[1]), &d[2]),
    ]
}

/// A volume's movement as the sampling uses it: `P = R^T x + offset`, and what its derivatives
/// need.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub p: [f64; 6],
    /// `R^T`.
    pub rt: Mat3,
    /// `c - R^T (c + t)`.
    pub offset: [f64; 3],
    /// `c + t` (the lever arms are `x - c - t`).
    pub ct: [f64; 3],
    /// `(∂R / ∂r_k)^T`.
    pub drt: [Mat3; 3],
}

impl Motion {
    /// The movement `p` (tx, ty, tz, rx, ry, rz) about the centre `c` (mm).
    pub fn new(p: [f64; 6], c: [f64; 3]) -> Motion {
        let r = rotation([p[3], p[4], p[5]]);
        let rt: Mat3 = std::array::from_fn(|i| std::array::from_fn(|j| r[j][i]));
        let ct: [f64; 3] = std::array::from_fn(|a| c[a] + p[a]);
        let offset = std::array::from_fn(|i| c[i] - (0..3).map(|k| rt[i][k] * ct[k]).sum::<f64>());
        let d = rotation_derivatives([p[3], p[4], p[5]]);
        let drt = d.map(|m| std::array::from_fn(|i| std::array::from_fn(|j| m[j][i])));
        Motion {
            p,
            rt,
            offset,
            ct,
            drt,
        }
    }

    /// The movement `p` sampled as FSL's matrices sample it (FSL-201's `reproduce`, §4.3): the
    /// inverse of [`fsl_matrix`] about `c`, the centre with its product `(n - 1) v` in single
    /// precision. Its [`Motion::derivative`] is the default's exact one; `reproduce` takes FSL's
    /// forward differences instead ([`Motion::fsl_difference`]).
    pub fn fsl(p: [f64; 6], c: [f64; 3]) -> Motion {
        let exact = Motion::new(p, c);
        match invert_affine(&fsl_matrix(p, c)) {
            Some((rt, offset)) => Motion {
                rt,
                offset,
                ..exact
            },
            None => exact,
        }
    }

    /// FSL's movement derivative of parameter `k` at `x` (mm), before the image's derivative
    /// (`specs/topup.md` §4.6): the forward difference of the sampling positions `P(p + δ e_k) −
    /// P(p)` of FSL's matrices ([`Motion::fsl`]) about `c`, `δ` 1e-4 mm for the translations and
    /// 1e-5 rad for the rotations; returns the difference (mm) and `δ`.
    #[inline]
    pub fn fsl_difference(&self, ahead: &Motion, x: [f64; 3]) -> [f64; 3] {
        let a = ahead.apply(x);
        let b = self.apply(x);
        std::array::from_fn(|i| a[i] - b[i])
    }

    /// FSL's forward-difference step of parameter `k` (§4.6).
    pub fn fsl_step(k: usize) -> f64 {
        if k < 3 { 1e-4 } else { 1e-5 }
    }

    /// Whether every parameter is 0 (the sampling is then the unmoved grid's, exactly).
    pub fn is_identity(&self) -> bool {
        self.p.iter().all(|&v| v == 0.0)
    }

    /// The moved position of `x` (mm).
    #[inline]
    pub fn apply(&self, x: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|i| {
            self.rt[i][0] * x[0] + self.rt[i][1] * x[1] + self.rt[i][2] * x[2] + self.offset[i]
        })
    }

    /// `∂P / ∂p_k` at `x` (mm per mm, or mm per radian).
    #[inline]
    pub fn derivative(&self, x: [f64; 3], k: usize) -> [f64; 3] {
        if k < 3 {
            // P = R^T (x - c - t) + c: -R^T e_k
            std::array::from_fn(|i| -self.rt[i][k])
        } else {
            let y: [f64; 3] = std::array::from_fn(|a| x[a] - self.ct[a]);
            let m = &self.drt[k - 3];
            std::array::from_fn(|i| m[i][0] * y[0] + m[i][1] * y[1] + m[i][2] * y[2])
        }
    }
}

/// One of FSL's elementary rotations (`specs/topup.md` §4.3, spec writer 2026-11-03) about
/// `axis` by `r` radians: `θ = |r|` rounded to single precision (the identity when `θ < 1e-8`),
/// its cosine and sine the C library's single-precision `cosf` and `sinf` of `θ` (glibc 2.35's
/// bits, `larmorx_core::math`), the sine with the sign of `r`, and the entry on the rotation's
/// own axis `q²` with `q = r / θ` (double precision).
fn fsl_elementary(axis: usize, r: f64) -> Mat3 {
    let theta = r.abs() as f32;
    let mut e = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    if f64::from(theta) < 1e-8 {
        return e;
    }
    let c = f64::from(larmorx_core::math::cosf_glibc(theta));
    let s = f64::from(larmorx_core::math::sinf_glibc(theta));
    let s = if r < 0.0 { -s } else { s };
    let q = r / f64::from(theta);
    let q2 = q * q;
    match axis {
        0 => e = [[q2, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]],
        1 => e = [[c, 0.0, -s], [0.0, q2, 0.0], [s, 0.0, c]],
        _ => e = [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, q2]],
    }
    e
}

/// FSL's movement matrix (`specs/topup.md` §4.3, spec writer 2026-11-03): each elementary
/// rotation about the centre `c` (mm) as `[E, (I − E) c]`, the three multiplied in the order x,
/// y, z, then the translations added to the last column; by rows. FSL-201's `reproduce` and
/// `--rbmout` in that mode use it; the default's [`matrix`] differs by less than 1e-7 in every
/// entry.
pub fn fsl_matrix(p: [f64; 6], c: [f64; 3]) -> [[f64; 4]; 4] {
    let affine = |e: &Mat3| -> [[f64; 4]; 4] {
        let mut m = [[0.0; 4]; 4];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] = e[i][j];
            }
            m[i][3] = c[i] - (0..3).map(|k| e[i][k] * c[k]).sum::<f64>();
        }
        m[3][3] = 1.0;
        m
    };
    let mul4 = |a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]| -> [[f64; 4]; 4] {
        std::array::from_fn(|i| std::array::from_fn(|j| (0..4).map(|k| a[i][k] * b[k][j]).sum()))
    };
    let ax = affine(&fsl_elementary(0, p[3]));
    let ay = affine(&fsl_elementary(1, p[4]));
    let az = affine(&fsl_elementary(2, p[5]));
    let mut m = mul4(&mul4(&ax, &ay), &az);
    for i in 0..3 {
        m[i][3] += p[i];
    }
    m
}

/// The sampling `P = A y + b` that inverts the affine `m` (by rows): `A = m⁻¹` of its 3 × 3 part
/// (cofactors, double precision), `b = −A t`; `None` when that part is singular.
pub fn invert_affine(m: &[[f64; 4]; 4]) -> Option<(Mat3, [f64; 3])> {
    let r: Mat3 = std::array::from_fn(|i| std::array::from_fn(|j| m[i][j]));
    let det = r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
        - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
        + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    let cof = |i: usize, j: usize| {
        let (i1, i2) = ((i + 1) % 3, (i + 2) % 3);
        let (j1, j2) = ((j + 1) % 3, (j + 2) % 3);
        r[i1][j1] * r[i2][j2] - r[i1][j2] * r[i2][j1]
    };
    let a: Mat3 = std::array::from_fn(|i| std::array::from_fn(|j| cof(j, i) / det));
    let t = [m[0][3], m[1][3], m[2][3]];
    let b = std::array::from_fn(|i| -(0..3).map(|k| a[i][k] * t[k]).sum::<f64>());
    Some((a, b))
}

/// `--rbmout`'s matrix of the movement `p` about the centre `c`: `[R | t + c - R c]`.
pub fn matrix(p: [f64; 6], c: [f64; 3]) -> [[f64; 4]; 4] {
    let r = rotation([p[3], p[4], p[5]]);
    let mut m = [[0.0; 4]; 4];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = r[i][j];
        }
        m[i][3] = p[i] + c[i] - (0..3).map(|k| r[i][k] * c[k]).sum::<f64>();
    }
    m[3][3] = 1.0;
    m
}

/// The parameters each volume estimates (§4.3): none for the first; six for every other,
/// except that, when every phase encoding lies along one axis, the first volume encoded with
/// the opposite sign of the first volume's leaves out the translation along that axis (it is
/// not estimable). Returns `(volume, parameter)` pairs in order.
pub fn free_parameters(pe: &[[f64; 3]]) -> Vec<(usize, usize)> {
    let axes: Vec<usize> = (0..3).filter(|&a| pe.iter().any(|p| p[a] != 0.0)).collect();
    let mut skip: Option<(usize, usize)> = None;
    if axes.len() == 1 && !pe.is_empty() {
        let a = axes[0];
        let s0 = pe[0][a].signum();
        if let Some(i) = (1..pe.len()).find(|&i| pe[i][a] != 0.0 && pe[i][a].signum() != s0) {
            skip = Some((i, a));
        }
    }
    let mut out = Vec::new();
    for i in 1..pe.len() {
        for k in 0..6 {
            if skip == Some((i, k)) {
                continue;
            }
            out.push((i, k));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivatives_match_differences() {
        let p = [0.3, -0.2, 0.5, 0.04, -0.03, 0.02];
        let c = [90.0, 95.0, 60.0];
        let m = Motion::new(p, c);
        let x = [12.0, 40.0, 33.0];
        for k in 0..6 {
            let mut q = p;
            q[k] += 1e-6;
            let mut r = p;
            r[k] -= 1e-6;
            let a = Motion::new(q, c).apply(x);
            let b = Motion::new(r, c).apply(x);
            let d = m.derivative(x, k);
            for i in 0..3 {
                let fd = (a[i] - b[i]) / 2e-6;
                assert!((fd - d[i]).abs() < 1e-6, "{k} {i}: {fd} {}", d[i]);
            }
        }
    }

    #[test]
    fn fsls_matrices_are_within_1e7_of_the_exact_ones() {
        let p = [0.3, -0.2, 0.5, 0.04, -0.03, 0.02];
        let c = [90.0, 95.0, 60.0];
        let a = matrix(p, c);
        let b = fsl_matrix(p, c);
        for i in 0..3 {
            for j in 0..3 {
                assert!((a[i][j] - b[i][j]).abs() < 2e-7, "{i} {j}");
                assert_ne!(b[i][i], 1.0);
            }
            assert!((a[i][3] - b[i][3]).abs() < 1e-4, "{i}");
        }
        // a zero angle is the identity exactly
        let z = fsl_matrix([1.0, 2.0, 3.0, 0.0, 0.0, 0.0], c);
        for i in 0..3 {
            for j in 0..3 {
                assert_eq!(z[i][j], if i == j { 1.0 } else { 0.0 });
            }
            assert_eq!(z[i][3], [1.0, 2.0, 3.0][i]);
        }
        // the forward difference of FSL's rotations is not the exact derivative: the axis
        // entry's rounding adds a scaling along the rotation's axis (§4.6, FSL-201)
        let zero = [0.0; 6];
        let m0 = Motion::fsl(zero, c);
        let mut q = zero;
        q[3] = Motion::fsl_step(3);
        let m1 = Motion::fsl(q, c);
        let x = [12.0, 40.0, 33.0];
        let d = m0.fsl_difference(&m1, x);
        let exact = Motion::new(zero, c).derivative(x, 3);
        let fd_x = d[0] / Motion::fsl_step(3);
        assert!(exact[0] == 0.0 && fd_x.abs() > 1e-2, "{fd_x}");
        assert!((d[1] / Motion::fsl_step(3) - exact[1]).abs() < 1e-2 * exact[1].abs().max(1.0));
    }

    #[test]
    fn the_matrix_inverts_the_sampling() {
        let p = [0.3, -0.2, 0.5, 0.04, -0.03, 0.02];
        let c = [90.0, 95.0, 60.0];
        let m = matrix(p, c);
        let x = [12.0, 40.0, 33.0];
        let y = Motion::new(p, c).apply(x);
        let back: [f64; 3] =
            std::array::from_fn(|i| (0..3).map(|j| m[i][j] * y[j]).sum::<f64>() + m[i][3]);
        for i in 0..3 {
            assert!((back[i] - x[i]).abs() < 1e-9);
        }
    }

    #[test]
    fn the_reversed_scan_leaves_out_its_translation() {
        let y = [[0.0, 1.0, 0.0], [0.0, -1.0, 0.0]];
        assert_eq!(
            free_parameters(&y),
            vec![(1, 0), (1, 2), (1, 3), (1, 4), (1, 5)]
        );
        let four = [
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
        ];
        assert_eq!(free_parameters(&four).len(), 18);
        let same = [[0.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
        assert_eq!(free_parameters(&same).len(), 6);
    }
}
