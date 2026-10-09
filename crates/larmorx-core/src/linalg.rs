// SPDX-License-Identifier: Apache-2.0
//! Small fixed-size linear algebra (3×3 and 4×4), in f64.
//!
//! Everything here is written out explicitly, without fused multiply-add, so results are the
//! same on every platform.

/// A 3×3 matrix, row-major.
pub type Mat3 = [[f64; 3]; 3];
/// A 4×4 matrix, row-major.
pub type Mat4 = [[f64; 4]; 4];

/// The 3×3 identity matrix.
pub const IDENTITY3: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// `a · b`.
pub fn mul3(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, value) in row.iter_mut().enumerate() {
            *value = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    out
}

/// `a · b`.
pub fn mul4(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, value) in row.iter_mut().enumerate() {
            *value = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + a[i][3] * b[3][j];
        }
    }
    out
}

pub fn transpose3(m: &Mat3) -> Mat3 {
    [
        [m[0][0], m[1][0], m[2][0]],
        [m[0][1], m[1][1], m[2][1]],
        [m[0][2], m[1][2], m[2][2]],
    ]
}

pub fn det3(m: &Mat3) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Inverse of a 3×3 matrix by the adjugate, or `None` if it is singular.
pub fn inverse3(m: &Mat3) -> Option<Mat3> {
    let det = det3(m);
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    let cof =
        |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    let adj = [
        [cof(1, 2, 1, 2), -cof(0, 2, 1, 2), cof(0, 1, 1, 2)],
        [-cof(1, 2, 0, 2), cof(0, 2, 0, 2), -cof(0, 1, 0, 2)],
        [cof(1, 2, 0, 1), -cof(0, 2, 0, 1), cof(0, 1, 0, 1)],
    ];
    Some(adj.map(|row| row.map(|v| v / det)))
}

/// Inverse of a 3×3 matrix as ITK computes it (`itk::Matrix::GetInverse`): vnl's SVD
/// pseudo-inverse, ported operation for operation ([`crate::vnl_svd::vnl_inverse3`]), so
/// point ↔ index conversions of oblique images and inverted affines match ITK to the bit.
/// `None` if singular (ITK throws).
pub fn inverse3_itk(m: &Mat3) -> Option<Mat3> {
    if det3(m) == 0.0 || m.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    crate::vnl_svd::vnl_inverse3(m)
}

/// Inverse of a 4×4 matrix by Gauss-Jordan elimination with partial pivoting, or `None` if it
/// is singular.
pub fn inverse4(m: &Mat4) -> Option<Mat4> {
    let mut a = *m;
    let mut inv = [[0.0; 4]; 4];
    for (i, row) in inv.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for col in 0..4 {
        let pivot = (col..4)
            .max_by(|&r, &s| a[r][col].abs().total_cmp(&a[s][col].abs()))
            .unwrap_or(col);
        if a[pivot][col] == 0.0 || !a[pivot][col].is_finite() {
            return None;
        }
        a.swap(col, pivot);
        inv.swap(col, pivot);
        let p = a[col][col];
        for j in 0..4 {
            a[col][j] /= p;
            inv[col][j] /= p;
        }
        for r in 0..4 {
            if r != col {
                let f = a[r][col];
                if f != 0.0 {
                    for j in 0..4 {
                        a[r][j] -= f * a[col][j];
                        inv[r][j] -= f * inv[col][j];
                    }
                }
            }
        }
    }
    Some(inv)
}

/// The orthogonal factor `Q` of the polar decomposition `m = Q·P` (the closest orthogonal
/// matrix to `m` in the Frobenius norm), or `None` if `m` is singular.
///
/// Uses the scaled Newton iteration `Q ← (γQ + (γQ)⁻ᵀ) / 2` (Higham, 1986), which converges
/// quadratically to the same matrix as the SVD-based `U·Vᵀ`.
pub fn polar_orthogonal(m: &Mat3) -> Option<Mat3> {
    let mut q = *m;
    for _ in 0..100 {
        let inv = inverse3(&q)?;
        let inv_t = transpose3(&inv);
        let frob = |a: &Mat3| a.iter().flatten().map(|v| v * v).sum::<f64>().sqrt();
        let gamma = (frob(&inv) / frob(&q)).sqrt();
        let mut next = [[0.0; 3]; 3];
        let mut change = 0.0f64;
        for i in 0..3 {
            for j in 0..3 {
                next[i][j] = 0.5 * (gamma * q[i][j] + inv_t[i][j] / gamma);
                change = change.max((next[i][j] - q[i][j]).abs());
            }
        }
        q = next;
        if change <= 4.0 * f64::EPSILON {
            break;
        }
    }
    // A few unscaled steps polish the result to full precision.
    for _ in 0..3 {
        let inv_t = transpose3(&inverse3(&q)?);
        for i in 0..3 {
            for j in 0..3 {
                q[i][j] = 0.5 * (q[i][j] + inv_t[i][j]);
            }
        }
    }
    Some(q)
}

/// Eigen-decomposition of a symmetric matrix by the cyclic Jacobi method.
///
/// Returns the eigenvalues in descending order and the matching unit eigenvectors as the
/// columns of the second matrix. Each eigenvector's largest-magnitude component is made
/// positive, so the result is a deterministic function of the input.
#[allow(clippy::needless_range_loop)] // index form mirrors the Jacobi rotation formulas
pub fn symmetric_eigen<const N: usize>(m: &[[f64; N]; N]) -> ([f64; N], [[f64; N]; N]) {
    let mut a = *m;
    let mut v = [[0.0; N]; N];
    for (i, row) in v.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for _sweep in 0..64 {
        let off: f64 = (0..N)
            .flat_map(|i| (0..N).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| a[i][j] * a[i][j])
            .sum();
        if off <= f64::MIN_POSITIVE {
            break;
        }
        for p in 0..N {
            for q in p + 1..N {
                if a[p][q] == 0.0 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..N {
                    let (akp, akq) = (a[k][p], a[k][q]);
                    a[k][p] = c * akp - s * akq;
                    a[k][q] = s * akp + c * akq;
                }
                for k in 0..N {
                    let (apk, aqk) = (a[p][k], a[q][k]);
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
                for row in v.iter_mut() {
                    let (vkp, vkq) = (row[p], row[q]);
                    row[p] = c * vkp - s * vkq;
                    row[q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let mut order: [usize; N] = std::array::from_fn(|i| i);
    order.sort_by(|&i, &j| a[j][j].total_cmp(&a[i][i]));
    let values = order.map(|i| a[i][i]);
    let mut vectors = [[0.0; N]; N];
    for (col, &src) in order.iter().enumerate() {
        let largest = (0..N)
            .max_by(|&i, &j| v[i][src].abs().total_cmp(&v[j][src].abs()))
            .unwrap_or(0);
        let sign = if v[largest][src] < 0.0 { -1.0 } else { 1.0 };
        for row in 0..N {
            vectors[row][col] = sign * v[row][src];
        }
    }
    (values, vectors)
}

/// Singular values (descending) and left singular vectors (columns) of a square matrix,
/// from the eigen-decomposition of `m·mᵀ`.
pub fn svd_u<const N: usize>(m: &[[f64; N]; N]) -> ([f64; N], [[f64; N]; N]) {
    let mut mmt = [[0.0; N]; N];
    for i in 0..N {
        for j in 0..N {
            mmt[i][j] = (0..N).map(|k| m[i][k] * m[j][k]).sum();
        }
    }
    let (eig, u) = symmetric_eigen(&mmt);
    (eig.map(|e| e.max(0.0).sqrt()), u)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn itk_inverse_is_exact_for_axis_aligned_matrices() {
        let m = [[0.0, -2.7, 0.0], [0.0, 0.0, 3.3], [1.7, 0.0, 0.0]];
        let inv = inverse3_itk(&m).unwrap();
        assert_eq!(
            inv,
            [
                [0.0, 0.0, 1.0 / 1.7],
                [-1.0 / 2.7, 0.0, 0.0],
                [0.0, 1.0 / 3.3, 0.0]
            ]
        );
        assert!(inverse3_itk(&[[1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]]).is_none());
    }

    fn assert_close3(a: &Mat3, b: &Mat3, tol: f64) {
        for i in 0..3 {
            for j in 0..3 {
                assert!((a[i][j] - b[i][j]).abs() <= tol, "{a:?} != {b:?}");
            }
        }
    }

    #[test]
    fn inverse3_and_inverse4_invert() {
        let m: Mat3 = [[2.0, 1.0, 0.0], [0.5, 3.0, -1.0], [0.0, 0.25, 4.0]];
        assert_close3(&mul3(&m, &inverse3(&m).unwrap()), &IDENTITY3, 1e-15);
        let m4: Mat4 = [
            [2.0, 1.0, 0.0, 3.0],
            [0.5, 3.0, -1.0, -2.0],
            [0.0, 0.25, 4.0, 1.5],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let p = mul4(&m4, &inverse4(&m4).unwrap());
        for (i, row) in p.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                assert!((v - if i == j { 1.0 } else { 0.0 }).abs() < 1e-15);
            }
        }
        assert!(inverse3(&[[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [0.0, 0.0, 1.0]]).is_none());
        assert!(inverse4(&[[0.0; 4]; 4]).is_none());
    }

    #[test]
    fn symmetric_eigen_and_singular_values() {
        let m = [[4.0, 1.0, 0.5], [1.0, 3.0, 0.2], [0.5, 0.2, 1.0]];
        let (values, vectors) = symmetric_eigen(&m);
        assert!(values[0] >= values[1] && values[1] >= values[2]);
        for k in 0..3 {
            for i in 0..3 {
                let mv: f64 = (0..3).map(|j| m[i][j] * vectors[j][k]).sum();
                assert!((mv - values[k] * vectors[i][k]).abs() < 1e-12);
            }
        }
        let (sv, _) = svd_u(&[[2.0, 0.0, 0.0], [0.0, -3.0, 0.0], [0.0, 0.0, 0.5]]);
        assert!(
            (sv[0] - 3.0).abs() < 1e-12
                && (sv[1] - 2.0).abs() < 1e-12
                && (sv[2] - 0.5).abs() < 1e-12
        );
        let (sv, _) = svd_u(&[[1.0, 2.0], [2.0, 4.0]]);
        assert!(sv[1].abs() < 1e-7, "{sv:?}");
    }

    #[test]
    fn polar_factor_of_rotation_times_spd_is_the_rotation() {
        let (c, s) = (0.3f64.cos(), 0.3f64.sin());
        let rot: Mat3 = [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]];
        let spd: Mat3 = [[2.0, 0.1, 0.0], [0.1, 1.5, 0.2], [0.0, 0.2, 1.0]];
        let q = polar_orthogonal(&mul3(&rot, &spd)).unwrap();
        assert_close3(&q, &rot, 1e-14);
        assert_close3(&polar_orthogonal(&IDENTITY3).unwrap(), &IDENTITY3, 0.0);
        assert!(polar_orthogonal(&[[0.0; 3]; 3]).is_none());
    }
}
