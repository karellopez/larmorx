// SPDX-License-Identifier: Apache-2.0
//! `LandmarkBasedTransformInitializer` (ITK v5.4.5,
//! `Modules/Registration/Common/include/itkLandmarkBasedTransformInitializer.hxx`): the
//! transform that best maps fixed landmarks onto moving ones, for the three transform classes
//! `antsAI` gives it.
//!
//! - [`affine`] (`AffineTransform`): the least-squares system `Q a = C` of Kim's method, each
//!   landmark weighted by `1/‖I‖_F = 1/√n` (the identity weight matrix normalised by its
//!   Frobenius norm, applied as matrix products), solved with `vnl_qr<double>` (LINPACK,
//!   [`larmorx_core::vnl_qr`]). At least `D + 1` landmarks.
//! - [`versor_rigid_3d`] (`VersorRigid3DTransform`): Horn's closed form, the versor the
//!   eigenvector of the largest eigenvalue of the 4 × 4 matrix `N` from the centred
//!   landmarks' correlation matrix, by Eigen's `SelfAdjointEigenSolver`
//!   ([`larmorx_core::eigen_sym`]); the rotation centre the fixed centroid, the translation the
//!   difference of the centroids. With fewer than 3 landmarks, no rotation.
//! - [`rigid_2d`] (`Rigid2DTransform`): the angle `atan2` of the summed cross and dot products
//!   of the centred landmarks (ITK takes `−π/2` when the dot product is within 5e-5 of 0:
//!   ITK-60), the centre and translation as in 3D. With fewer than 2 landmarks, no rotation and
//!   ITK's warning.
//!
//! Each returns the transform's matrix and offset (`GetMatrix`, `GetOffset`), as `antsAI`
//! reads them. Every sum is in ITK's order; the centroids are sums divided by the count, so
//! no landmark gives NaN (0/0), as in ITK (ITK-61 is decided by the caller).

#![allow(clippy::needless_range_loop)]

use larmorx_core::defects::{self, Notices};
use larmorx_core::eigen_sym::self_adjoint_eigen_4;
use larmorx_core::math;
use larmorx_core::vnl_qr::{RANK_DEFICIENT, VnlQr};

use crate::{ItkException, Matrix};

const FILE: &str = "Modules/Registration/Common/include/itkLandmarkBasedTransformInitializer.hxx";
const CLASS: &str = "LandmarkBasedTransformInitializer";

/// A fitted transform: `GetMatrix()` and `GetOffset()`, and what ITK printed on standard
/// error while fitting it (vnl's rank-deficiency messages, ITK's warnings).
#[derive(Clone, Debug, PartialEq)]
pub struct LandmarkFit<const D: usize> {
    pub matrix: Matrix<f64, D>,
    pub offset: [f64; D],
    pub messages: Vec<String>,
}

/// `vnl_matrix<double>` product `a · b` (`a` is `r × m`, `b` is `m × c`, both row-major):
/// each entry summed from 0 in order.
fn vnl_mul(a: &[f64], r: usize, m: usize, b: &[f64], c: usize) -> Vec<f64> {
    let mut out = vec![0.0f64; r * c];
    for i in 0..r {
        for k in 0..c {
            let mut s = 0.0f64;
            for j in 0..m {
                s += a[i * m + j] * b[j * c + k];
            }
            out[i * c + k] = s;
        }
    }
    out
}

/// `InternalInitializeTransform(AffineTransformType *)`: the affine map of `fixed` onto
/// `moving` (centre 0). Fewer than `D + 1` landmarks: ITK's exception.
pub fn affine<const D: usize>(
    fixed: &[[f64; D]],
    moving: &[[f64; D]],
) -> Result<LandmarkFit<D>, ItkException> {
    let n = moving.len();
    if n < D + 1 {
        return Err(ItkException::new(
            FILE,
            208,
            CLASS,
            format!(
                " insufficient number of landmarks, expected {} got {n}",
                D + 1
            ),
        ));
    }
    // vnlWeight = I / ‖I‖_F: every entry divided by √n (the sum of the n² squared entries is
    // n exactly), the zeros too. `q *= vnlWeight` is a vnl matrix product, each entry summed
    // from +0 over the n columns: with finite values, the diagonal term alone, or +0 when it
    // is a zero; a non-finite value times a zero weight is NaN.
    let wd = 1.0 / (n as f64).sqrt();
    let wz = 0.0 / (n as f64).sqrt();
    let weigh = |rows: usize, x: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0f64; rows * n];
        for a in 0..rows {
            let row = &x[a * n..(a + 1) * n];
            if row.iter().all(|v| v.is_finite()) {
                for k in 0..n {
                    let p = row[k] * wd;
                    out[a * n + k] = if p == 0.0 { 0.0 } else { p };
                }
            } else {
                for k in 0..n {
                    let mut s = 0.0f64;
                    for j in 0..n {
                        s += row[j] * if j == k { wd } else { wz };
                    }
                    out[a * n + k] = s;
                }
            }
        }
        out
    };
    // q: (D + 1) × n, the fixed points with a row of ones; p: D × n, the moving points.
    let mut q = vec![0.0f64; (D + 1) * n];
    for (i, f) in fixed.iter().enumerate() {
        for d in 0..D {
            q[d * n + i] = f[d];
        }
        q[D * n + i] = 1.0;
    }
    let q = weigh(D + 1, &q);
    let mut p = vec![0.0f64; D * n];
    for (i, m) in moving.iter().enumerate() {
        for d in 0..D {
            p[d * n + i] = m[d];
        }
    }
    let p = weigh(D, &p);
    // Q = Σ q_i q_iᵀ and C = Σ q_i p_iᵀ, each outer product a vnl matrix product.
    let (dq, dp) = (D + 1, D);
    let mut big_q = vec![0.0f64; dq * dq];
    let mut big_c = vec![0.0f64; dq * dp];
    for i in 0..n {
        let qi: Vec<f64> = (0..dq).map(|k| q[k * n + i]).collect();
        let pi: Vec<f64> = (0..dp).map(|k| p[k * n + i]).collect();
        let outer = vnl_mul(&qi, dq, 1, &qi, dq);
        for k in 0..dq * dq {
            big_q[k] += outer[k];
        }
        let outer = vnl_mul(&qi, dq, 1, &pi, dp);
        for k in 0..dq * dp {
            big_c[k] += outer[k];
        }
    }
    let (sol, infos) = VnlQr::new(&big_q, dq, dq).solve_matrix(&big_c, dp);
    let messages = infos
        .iter()
        .filter(|&&i| i > 0)
        .map(|i| format!("{RANK_DEFICIENT}{i}\n"))
        .collect();
    // transposeAffine is (D + 1) × D; Affine = its transpose, D × (D + 1).
    let matrix: Matrix<f64, D> = std::array::from_fn(|r| std::array::from_fn(|c| sol[c * dp + r]));
    let offset: [f64; D] = std::array::from_fn(|t| sol[D * dp + t]);
    Ok(LandmarkFit {
        matrix,
        offset,
        messages,
    })
}

/// `ComputeCentroid`: each coordinate summed in order, then divided by the count (0/0 is NaN).
fn centroid<const D: usize>(points: &[[f64; D]]) -> [f64; D] {
    let mut c = [0.0f64; D];
    for p in points {
        for d in 0..D {
            c[d] += p[d];
        }
    }
    let n = points.len() as f64;
    for d in 0..D {
        c[d] /= n;
    }
    c
}

/// `MatrixOffsetTransformBase::ComputeOffset`: `t + c − M·c`, each row from `t[i] + c[i]`.
fn compute_offset<const D: usize>(m: &Matrix<f64, D>, center: &[f64; D], t: &[f64; D]) -> [f64; D] {
    std::array::from_fn(|i| {
        let mut o = t[i] + center[i];
        for j in 0..D {
            o -= m[i][j] * center[j];
        }
        o
    })
}

/// `InternalInitializeTransform(VersorRigid3DTransformType *)`.
pub fn versor_rigid_3d(
    fixed: &[[f64; 3]],
    moving: &[[f64; 3]],
) -> Result<LandmarkFit<3>, ItkException> {
    let fc = centroid(fixed);
    let mc = centroid(moving);
    // The identity versor (x, y, z, w).
    let mut versor = [0.0f64, 0.0, 0.0, 1.0];
    if fixed.len() >= 3 {
        let mut m = [[0.0f64; 3]; 3];
        for (f, mv) in fixed.iter().zip(moving) {
            let a: [f64; 3] = std::array::from_fn(|i| f[i] - fc[i]);
            let b: [f64; 3] = std::array::from_fn(|i| mv[i] - mc[i]);
            for i in 0..3 {
                for j in 0..3 {
                    m[i][j] += a[i] * b[j];
                }
            }
        }
        // CreateMatrix.
        let mut nm = [[0.0f64; 4]; 4];
        nm[0][0] = m[0][0] + m[1][1] + m[2][2];
        nm[1][1] = m[0][0] - m[1][1] - m[2][2];
        nm[2][2] = -m[0][0] + m[1][1] - m[2][2];
        nm[3][3] = -m[0][0] - m[1][1] + m[2][2];
        nm[0][1] = m[1][2] - m[2][1];
        nm[1][0] = nm[0][1];
        nm[0][2] = m[2][0] - m[0][2];
        nm[2][0] = nm[0][2];
        nm[0][3] = m[0][1] - m[1][0];
        nm[3][0] = nm[0][3];
        nm[1][2] = m[0][1] + m[1][0];
        nm[2][1] = nm[1][2];
        nm[1][3] = m[2][0] + m[0][2];
        nm[3][1] = nm[1][3];
        nm[2][3] = m[1][2] + m[2][1];
        nm[3][2] = nm[2][3];
        let e = self_adjoint_eigen_4(&nm);
        let v = e.vectors[3];
        // versor.Set(x, y, z, w): a negative w negates all four; then Normalize.
        let (mut x, mut y, mut z, mut w) = (v[1], v[2], v[3], v[0]);
        if w < 0.0 {
            x = -x;
            y = -y;
            z = -z;
            w = -w;
        }
        let tensor = (x * x + y * y + z * z + w * w).sqrt();
        if tensor.abs() < 1e-20 {
            return Err(ItkException::new(
                "Modules/Core/Common/include/itkVersor.hxx",
                0,
                "",
                "Attempt to normalize a itk::Versor with zero tensor",
            ));
        }
        versor = [x / tensor, y / tensor, z / tensor, w / tensor];
    }
    let matrix = versor_matrix(&versor);
    let t: [f64; 3] = std::array::from_fn(|i| mc[i] - fc[i]);
    let offset = compute_offset(&matrix, &fc, &t);
    Ok(LandmarkFit {
        matrix,
        offset,
        messages: Vec::new(),
    })
}

/// `Versor<double>::GetMatrix()`.
fn versor_matrix(v: &[f64; 4]) -> Matrix<f64, 3> {
    let [x, y, z, w] = *v;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let xw = x * w;
    let yz = y * z;
    let yw = y * w;
    let zw = z * w;
    let mut m = [[0.0f64; 3]; 3];
    m[0][0] = 1.0 - 2.0 * (yy + zz);
    m[1][1] = 1.0 - 2.0 * (xx + zz);
    m[2][2] = 1.0 - 2.0 * (xx + yy);
    m[0][1] = 2.0 * (xy - zw);
    m[0][2] = 2.0 * (xz + yw);
    m[1][0] = 2.0 * (xy + zw);
    m[2][0] = 2.0 * (xz - yw);
    m[2][1] = 2.0 * (yz + xw);
    m[1][2] = 2.0 * (yz - xw);
    m
}

/// ITK's warning for fewer than 2 landmarks in 2D.
pub const FEW_LANDMARKS_2D: &str = "WARNING: In Modules/Registration/Common/include/itkLandmarkBasedTransformInitializer.hxx, line 735\nLandmarkBasedTransformInitializer (0x0): Less than 2 landmarks available. Rotation is not computed\n\n";

/// `InternalInitializeTransform(Rigid2DTransformType *)`. ITK takes the angle `−π/2` whenever
/// the summed dot product is within 5e-5 of 0, whatever the cross product's sign (ITK-60);
/// corrected (`notices.fix()`), the angle is `atan2(cross, dot)` there too, with a notice
/// where the two differ.
pub fn rigid_2d(fixed: &[[f64; 2]], moving: &[[f64; 2]], notices: &mut Notices) -> LandmarkFit<2> {
    let fc = centroid(fixed);
    let mc = centroid(moving);
    let mut messages = Vec::new();
    let mut angle = 0.0f64;
    if fixed.len() >= 2 {
        let mut s_dot = 0.0f64;
        let mut s_cross = 0.0f64;
        for (f, m) in fixed.iter().zip(moving) {
            let a = [f[0] - fc[0], f[1] - fc[1]];
            let b = [m[0] - mc[0], m[1] - mc[1]];
            s_dot += (b[0] * a[0]) + (b[1] * a[1]);
            s_cross += (b[1] * a[0]) - (b[0] * a[1]);
        }
        if s_dot.abs() > 0.00005 {
            angle = math::atan2(s_cross, s_dot);
        } else {
            // PI = 4·atan(1): atan(1) is π/4 rounded, and 4· is exact.
            let itk = -0.5 * std::f64::consts::PI;
            let fixed_angle = math::atan2(s_cross, s_dot);
            if notices.fix() {
                angle = fixed_angle;
            } else {
                angle = itk;
            }
            if fixed_angle != itk {
                notices.add(
                    &defects::ITK_60,
                    format!(
                        "the landmarks' dot product is {:e}: ITK takes the angle -pi/2, the least-squares angle is {fixed_angle}{}",
                        s_dot,
                        if notices.fix() { " (used)" } else { " (ITK's kept)" }
                    ),
                );
            }
        }
    } else {
        messages.push(FEW_LANDMARKS_2D.to_owned());
    }
    let (s, c) = (math::sin(angle), math::cos(angle));
    let matrix = [[c, -s], [s, c]];
    let t = [mc[0] - fc[0], mc[1] - fc[1]];
    let offset = compute_offset(&matrix, &fc, &t);
    LandmarkFit {
        matrix,
        offset,
        messages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn affine_recovers_an_exact_map() {
        let a = [[1.1, 0.05, -0.02], [0.03, 0.95, 0.1], [-0.04, 0.02, 1.05]];
        let b = [3.0, -7.0, 12.5];
        let fixed: Vec<[f64; 3]> = (0..12)
            .map(|i| {
                let i = f64::from(i);
                [i * 3.0 - 10.0, (i * i) % 17.0 - 5.0, (i * i * i) % 13.0 - i]
            })
            .collect();
        let moving: Vec<[f64; 3]> = fixed
            .iter()
            .map(|f| std::array::from_fn(|r| (0..3).map(|c| a[r][c] * f[c]).sum::<f64>() + b[r]))
            .collect();
        let fit = affine(&fixed, &moving).unwrap();
        for r in 0..3 {
            for c in 0..3 {
                assert!(
                    (fit.matrix[r][c] - a[r][c]).abs() < 1e-9,
                    "{:?}",
                    fit.matrix
                );
            }
            assert!((fit.offset[r] - b[r]).abs() < 1e-8, "{:?}", fit.offset);
        }
    }

    #[test]
    fn affine_needs_d_plus_one_landmarks() {
        let e = affine::<3>(&[[0.0; 3]; 3], &[[0.0; 3]; 3]).unwrap_err();
        assert_eq!(
            e.description,
            " insufficient number of landmarks, expected 4 got 3"
        );
    }

    #[test]
    fn rigid_3d_recovers_a_rotation() {
        let (ang, axis) = (0.3f64, [0.0, 0.6, 0.8]);
        let (s, c) = (ang.sin(), ang.cos());
        let k = axis;
        // Rodrigues.
        let r: [[f64; 3]; 3] = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                let kk = k[i] * k[j] * (1.0 - c);
                let skew = match (i, j) {
                    (0, 1) => -k[2],
                    (1, 0) => k[2],
                    (0, 2) => k[1],
                    (2, 0) => -k[1],
                    (1, 2) => -k[0],
                    (2, 1) => k[0],
                    _ => 0.0,
                };
                kk + if i == j { c } else { s * skew }
            })
        });
        let fixed: Vec<[f64; 3]> = (0..9)
            .map(|i| {
                let i = f64::from(i);
                [i * 7.0 - 30.0, (i * i) % 11.0 * 4.0, 12.0 - i * 2.5]
            })
            .collect();
        let moving: Vec<[f64; 3]> = fixed
            .iter()
            .map(|f| {
                std::array::from_fn(|a| {
                    (0..3).map(|b| r[a][b] * f[b]).sum::<f64>() + [5.0, -2.0, 1.0][a]
                })
            })
            .collect();
        let fit = versor_rigid_3d(&fixed, &moving).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                assert!(
                    (fit.matrix[i][j] - r[i][j]).abs() < 1e-12,
                    "{:?}",
                    fit.matrix
                );
            }
        }
        for (f, m) in fixed.iter().zip(&moving) {
            for a in 0..3 {
                let p: f64 = (0..3).map(|b| fit.matrix[a][b] * f[b]).sum::<f64>() + fit.offset[a];
                assert!((p - m[a]).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn rigid_without_landmarks_is_nan_as_in_itk() {
        let fit = versor_rigid_3d(&[], &[]).unwrap();
        assert!(fit.offset.iter().all(|v| v.is_nan()));
        let mut n = Notices::new(UpstreamDefects::Reproduce);
        let fit = rigid_2d(&[], &[], &mut n);
        assert!(fit.offset.iter().all(|v| v.is_nan()));
        assert_eq!(fit.messages, vec![FEW_LANDMARKS_2D.to_owned()]);
    }

    #[test]
    fn rigid_2d_quarter_turn() {
        // A rotation by +90°: the dot product vanishes; ITK takes -90° (ITK-60).
        let fixed = [[1.0, 0.0], [-1.0, 0.0], [0.0, 2.0], [0.0, -2.0]];
        let moving: Vec<[f64; 2]> = fixed.iter().map(|p| [-p[1], p[0]]).collect();
        let mut n = Notices::new(UpstreamDefects::Reproduce);
        let itk = rigid_2d(&fixed, &moving, &mut n);
        assert!((itk.matrix[1][0] + 1.0).abs() < 1e-15, "{:?}", itk.matrix);
        assert_eq!(n.as_slice().len(), 1);
        let mut n = Notices::new(UpstreamDefects::Fix);
        let fixed_fit = rigid_2d(&fixed, &moving, &mut n);
        assert!(
            (fixed_fit.matrix[1][0] - 1.0).abs() < 1e-15,
            "{:?}",
            fixed_fit.matrix
        );
    }
    /// The fits of ITK v5.4.5's build (GCC 14.3, ANTs' flags; glibc's maths replaced by
    /// CORE-MATH's, which the 2D rigid fit's `atan2`, `sin` and `cos` need): matrix and offset
    /// as bit patterns.
    #[test]
    fn matches_itks_build() {
        let bits3 = |m: &Matrix<f64, 3>, o: &[f64; 3]| -> Vec<u64> {
            m.iter()
                .flatten()
                .chain(o.iter())
                .map(|v| v.to_bits())
                .collect()
        };
        let bits2 = |m: &Matrix<f64, 2>, o: &[f64; 2]| -> Vec<u64> {
            m.iter()
                .flatten()
                .chain(o.iter())
                .map(|v| v.to_bits())
                .collect()
        };
        #[rustfmt::skip]
        let (f, m): (Vec<[f64; 3]>, Vec<[f64; 3]>) = (
            vec![[36.6, 36.95, 1.84], [-25.7, -53.53, -14.0], [-10.98, -54.57, -54.15], [59.9, 18.28, -31.86], [-7.81, 56.9, 47.72], [41.31, -12.91, -0.84], [21.2, -52.7, 6.67]],
            vec![[42.43, 31.24, -2.69], [-21.81, -66.7, -7.78], [-3.9, -60.63, -47.51], [69.13, 17.51, -34.57], [-7.26, 43.49, 41.58], [47.88, -22.13, -0.55], [26.46, -67.28, 11.25]],
        );
        let fit = affine(&f, &m).unwrap();
        #[rustfmt::skip]
        assert_eq!(
            bits3(&fit.matrix, &fit.offset),
            vec![0x3ff0e296e1fd4c67, 0xbf79b8d683625792, 0xbfae3088e41ca321, 0x3fa4f29c0a40de53, 0x3ff1542fea9d06ac, 0xbfc50827621b2510, 0xbf9a3eadf27430d2, 0xbfb91eb3c2b0fdde, 0x3fef725e472e17bc, 0x40109aa3b5a44e70, 0xc023f30542b5b53f, 0x3fb13b209bf3ed48]
        );
        #[rustfmt::skip]
        let (f, m): (Vec<[f64; 3]>, Vec<[f64; 3]>) = (
            vec![[-7.6, -35.61, -21.01], [36.75, -22.03, -42.12], [23.82, -6.17, 35.87], [-31.74, -21.63, 35.99], [0.85, 0.77, -31.66], [-58.26, 51.99, -49.7]],
            vec![[-3.34, -36.13, -37.01], [39.1, -28.22, -53.65], [20.29, -1.97, 32.46], [-29.86, -14.27, 25.67], [7.97, -4.63, -39.02], [-38.39, 41.63, -49.12]],
        );
        let fit = versor_rigid_3d(&f, &m).unwrap();
        #[rustfmt::skip]
        assert_eq!(
            bits3(&fit.matrix, &fit.offset),
            vec![0x3fefdda2200d38c5, 0x3fb5146b341f2562, 0xbfa46e5ca0e95045, 0xbfb4cd9fb0c5633a, 0x3fefe1b06bc0184f, 0x3f9ca67c4a9cc7ab, 0x3fa588faf4968761, 0xbf9935a6a3663d85, 0x3feff643f54b3dd6, 0x40150f84519a9af8, 0xbfffd64902302bec, 0xc01f9b26bd912c92]
        );
        let f = [
            [16.91, -47.15],
            [23.07, 16.25],
            [-14.82, 35.82],
            [-36.72, -13.14],
            [35.75, -14.34],
        ];
        let m = [
            [15.7, -37.51],
            [27.0, 17.48],
            [-5.86, 39.62],
            [-30.25, -0.62],
            [35.85, -11.08],
        ];
        let fit = affine(&f, &m).unwrap();
        #[rustfmt::skip]
        assert_eq!(
            bits2(&fit.matrix, &fit.offset),
            vec![0x3fed3bda0c02ced1, 0x3fb6e96014a1c0eb, 0xbfc09cddfd3cea72, 0x3fec28a6d8e0f1a0, 0x4011e35c92e518f9, 0x4018b4882d0188b8]
        );
        let f = [
            [-41.66, 25.55],
            [41.71, -11.85],
            [6.39, -2.46],
            [55.02, -21.93],
        ];
        let m = [
            [-31.63, 35.74],
            [49.07, -5.17],
            [14.88, 5.74],
            [61.96, -15.79],
        ];
        let fit = rigid_2d(&f, &m, &mut Notices::new(UpstreamDefects::Reproduce));
        #[rustfmt::skip]
        assert_eq!(
            bits2(&fit.matrix, &fit.offset),
            vec![0x3feff6f505cab10f, 0x3fa80ceb573a71cd, 0xbfa80ceb573a71cd, 0x3feff6f505cab10f, 0x4020b1eb3a81971e, 0x40210ae7abe5d6b2]
        );
    }
}
