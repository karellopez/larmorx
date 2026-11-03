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
