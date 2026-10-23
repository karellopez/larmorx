// SPDX-License-Identifier: Apache-2.0 AND MPL-2.0
//! Eigen's `SelfAdjointEigenSolver` of a 4 × 4 row-major double matrix, as ITK's
//! `SymmetricEigenAnalysisFixedDimension<4, itk::Matrix<double, 4, 4>, ...>` calls it (ITK
//! v5.4.5 bundles Eigen 3.4.90): the eigenvalues in ascending order and the eigenvectors,
//! returned by ITK as the rows of its eigenvector matrix. `LandmarkBasedTransformInitializer`
//! takes the versor of Horn's closed-form rigid fit from the eigenvector of the largest
//! eigenvalue.
//!
//! The solver (`SelfAdjointEigenSolver::compute`) works on its own **column-major** copy of
//! the matrix (`EigenvectorsType` is `Matrix<Scalar, Size, Size, ColMajor>` whatever the
//! input's storage order):
//! 1. the lower triangle of the matrix, divided by its largest absolute entry (the upper
//!    triangle set to 0);
//! 2. Householder tridiagonalization (`tridiagonalization_inplace`, the generic path: Eigen
//!    has a special one only for 3 × 3 matrices): per column, `makeHouseholderInPlace`, the
//!    self-adjoint matrix-vector product (`selfadjoint_matrix_vector_product`, column-major,
//!    lower triangle: column by column, the diagonal entry first, then the entries below it),
//!    the dot product correction and the rank-2 update (`selfadjoint_rank2_update_selector`,
//!    its `Lower` form), then the Householder sequence accumulated in place
//!    (`HouseholderSequence::evalTo`, `applyHouseholderOnTheLeft`);
//! 3. the implicit symmetric QR iteration with Wilkinson shifts (`computeFromTridiagonal_impl`,
//!    `tridiagonal_qr_step`, `JacobiRotation::makeGivens`, `applyOnTheRight`), at most
//!    30 · n sweeps, then a selection sort of the eigenvalues with their vectors;
//! 4. the eigenvalues multiplied back by the scale.
//!
//! **Vectorisation.** ANTs' build compiles Eigen for SSE4.2 (`-march=corei7`, packets of two
//! doubles, no fused multiply-add). For a 4 × 4 matrix every kernel takes its scalar path or
//! one whose order of operations is the scalar one: the self-adjoint product's two-column
//! packet loop only runs on blocks of more than 8 rows (`bound = max(0, size − 8) & ~1` is 0
//! here, so every column goes through the single-column loop); the reductions over the
//! columns (`squaredNorm` over at most 2 entries, `dot` over at most 3, the coefficient-wise
//! product `essential.adjoint() * bottom` over at most 2) start with one packet of the first
//! two terms, `a₀ + a₁`, then add the rest in order, which is the sequential order; the
//! rank-2 update and the Givens rotations (vectorised along the contiguous columns) compute
//! every entry with its own multiplies and adds, as the scalar formulas do. The same
//! operations are written here, in Eigen's order. Only `sqrt` and the four basic operations
//! are used, which IEEE arithmetic rounds exactly: the result is the same on every platform.
//! Verified against the solver in ITK's build (the GCC 14.3 oracle's flags) on random,
//! degenerate and Horn matrices: [`self_adjoint_eigen_4`]'s tests hold some of its outputs.
//!
//! Translated from Eigen 3.4.90 as bundled with ITK v5.4.5
//! (`Modules/ThirdParty/Eigen3/src/itkeigen/Eigen/src/`): `Eigenvalues/SelfAdjointEigenSolver.h`,
//! `Eigenvalues/Tridiagonalization.h`, `Householder/Householder.h`,
//! `Householder/HouseholderSequence.h`, `Jacobi/Jacobi.h`,
//! `Core/products/SelfadjointMatrixVector.h`, `Core/products/SelfadjointRank2Update.h`,
//! `Core/MathFunctionsImpl.h` (`positive_real_hypot`), and ITK's
//! `Modules/Core/Common/include/itkSymmetricEigenAnalysis.h`.
//!
//! Eigen's notice (MPL-2.0; this file is a Modification of those files and is distributed
//! under the Mozilla Public License 2.0, which applies to it together with Apache-2.0):
//!
//!   This file is part of Eigen, a lightweight C++ template library for linear algebra.
//!   Copyright (C) 2008-2010 Gael Guennebaud <gael.guennebaud@inria.fr>
//!   Copyright (C) 2010 Jitse Niesen <jitse@maths.leeds.ac.uk>
//!   Copyright (C) 2009 Benoit Jacob <jacob.benoit.1@gmail.com>
//!   This Source Code Form is subject to the terms of the Mozilla Public License v. 2.0. If a
//!   copy of the MPL was not distributed with this file, You can obtain one at
//!   http://mozilla.org/MPL/2.0/.

#![allow(clippy::needless_range_loop)]

const N: usize = 4;

/// The result of [`self_adjoint_eigen_4`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelfAdjointEigen4 {
    /// The eigenvalues in ascending order (`solver.eigenvalues()`).
    pub values: [f64; N],
    /// ITK's `EigenVectors[row][col]`: row `k` is the eigenvector of `values[k]`
    /// (`solver.eigenvectors()` transposed).
    pub vectors: [[f64; N]; N],
    /// Whether the QR iteration converged (Eigen's `Success`); when it did not, Eigen leaves
    /// the eigenvalues unsorted.
    pub converged: bool,
}

/// `positive_real_hypot(|x|, |y|)` (Eigen's `numext::hypot` for reals).
fn hypot(x: f64, y: f64) -> f64 {
    let (x, y) = (x.abs(), y.abs());
    if x.is_infinite() || y.is_infinite() {
        return f64::INFINITY;
    }
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    // numext::maxi(x, y) = std::max(x, y); numext::mini(y, x) = std::min(y, x).
    let p = if x < y { y } else { x };
    if p == 0.0 {
        return 0.0;
    }
    let q = if x < y { x } else { y };
    let qp = q / p;
    p * (1.0 + qp * qp).sqrt()
}

/// `JacobiRotation<double>::makeGivens(p, q)` (real case): `(c, s)`.
fn make_givens(p: f64, q: f64) -> (f64, f64) {
    if q == 0.0 {
        (if p < 0.0 { -1.0 } else { 1.0 }, 0.0)
    } else if p == 0.0 {
        (0.0, if q < 0.0 { 1.0 } else { -1.0 })
    } else if p.abs() > q.abs() {
        let t = q / p;
        let mut u = (1.0 + t * t).sqrt();
        if p < 0.0 {
            u = -u;
        }
        let c = 1.0 / u;
        (c, -t * c)
    } else {
        let t = p / q;
        let mut u = (1.0 + t * t).sqrt();
        if q < 0.0 {
            u = -u;
        }
        let s = -1.0 / u;
        (-t * s, s)
    }
}

/// `tridiagonalization_inplace(matA, hCoeffs)` on the row-major matrix `m` (only its lower
/// triangle is read and written).
fn tridiagonalize(m: &mut [[f64; N]; N], h_coeffs: &mut [f64; N - 1]) {
    for i in 0..N - 1 {
        let rs = N - i - 1;
        let r0 = i + 1;
        // makeHouseholderInPlace on v = column i, rows i+1..N.
        let c0 = m[r0][i];
        let tail_sq_norm = if rs == 1 {
            0.0
        } else {
            let mut s = m[r0 + 1][i] * m[r0 + 1][i];
            for r in r0 + 2..N {
                s += m[r][i] * m[r][i];
            }
            s
        };
        let tol = f64::MIN_POSITIVE;
        let (h, beta);
        if tail_sq_norm <= tol {
            h = 0.0;
            beta = c0;
            for r in r0 + 1..N {
                m[r][i] = 0.0;
            }
        } else {
            let mut b = (c0 * c0 + tail_sq_norm).sqrt();
            if c0 >= 0.0 {
                b = -b;
            }
            let d = c0 - b;
            for r in r0 + 1..N {
                m[r][i] /= d;
            }
            h = (b - c0) / b;
            beta = b;
        }
        m[r0][i] = 1.0;
        let v: Vec<f64> = (0..rs).map(|k| m[r0 + k][i]).collect();
        // hc = A_br.selfadjointView<Lower>() * (h * v): the destination zeroed, the scalar
        // factor h taken out of the right-hand side as alpha, then the single-column loop of
        // `selfadjoint_matrix_vector_product` (column-major, lower: not the "first
        // triangular" case; A0 is column j of the block, read from its diagonal down).
        let mut hc = vec![0.0f64; rs];
        for j in 0..rs {
            let t1 = h * v[j];
            let mut t2 = 0.0f64;
            hc[j] += m[r0 + j][r0 + j] * t1;
            for k in j + 1..rs {
                let a = m[r0 + k][r0 + j];
                hc[k] += a * t1;
                t2 += a * v[k];
            }
            hc[j] += h * t2;
        }
        // hc += (h * -0.5 * hc.dot(v)) * v.
        let mut dot = hc[0] * v[0];
        for k in 1..rs {
            dot += hc[k] * v[k];
        }
        let c = h * -0.5 * dot;
        for k in 0..rs {
            hc[k] += c * v[k];
        }
        // A_br.selfadjointView<Lower>().rankUpdate(v, hc, -1): the `Lower` selector on the
        // column-major block, column c from its diagonal down,
        // `(alpha·v_c)·hc_r + (alpha·hc_c)·v_r`.
        for c in 0..rs {
            let cu = -v[c];
            let cv = -hc[c];
            for r in c..rs {
                m[r0 + r][r0 + c] += cu * hc[r] + cv * v[r];
            }
        }
        m[r0][i] = beta;
        h_coeffs[i] = h;
    }
}

/// `HouseholderSequence(mat, hCoeffs).setLength(N − 1).setShift(1).evalTo(mat)`, in place.
fn accumulate_q(m: &mut [[f64; N]; N], h_coeffs: &[f64; N - 1]) {
    for i in 0..N {
        m[i][i] = 1.0;
        for j in i + 1..N {
            m[i][j] = 0.0;
        }
    }
    for k in (0..N - 1).rev() {
        let corner = N - k - 1;
        let c0 = N - corner; // first row and column of the corner
        let tau = h_coeffs[k];
        // essentialVector(k): column k, rows k + 2 .. N.
        let essential: Vec<f64> = (k + 2..N).map(|r| m[r][k]).collect();
        if corner == 1 {
            let f = 1.0 - tau;
            m[c0][c0] *= f;
        } else if tau != 0.0 {
            let cols = corner;
            let mut tmp = vec![0.0f64; cols];
            for c in 0..cols {
                let mut s = essential[0] * m[c0 + 1][c0 + c];
                for r in 1..essential.len() {
                    s += essential[r] * m[c0 + 1 + r][c0 + c];
                }
                tmp[c] = s;
            }
            for c in 0..cols {
                tmp[c] += m[c0][c0 + c];
            }
            for c in 0..cols {
                m[c0][c0 + c] -= tau * tmp[c];
            }
            for r in 0..essential.len() {
                let f = tau * essential[r];
                for c in 0..cols {
                    m[c0 + 1 + r][c0 + c] -= f * tmp[c];
                }
            }
        }
        for r in k + 1..N {
            m[r][k] = 0.0;
        }
    }
}

/// `tridiagonal_qr_step<RowMajor>` on `diag`, `subdiag` from `start` to `end`, the rotations
/// applied to the row-major eigenvector matrix `q`.
fn qr_step(
    diag: &mut [f64; N],
    subdiag: &mut [f64; N - 1],
    start: usize,
    end: usize,
    q: &mut [[f64; N]; N],
) {
    let td = (diag[end - 1] - diag[end]) * 0.5;
    let e = subdiag[end - 1];
    let mut mu = diag[end];
    if td == 0.0 {
        mu -= e.abs();
    } else if e != 0.0 {
        let e2 = e * e;
        let h = hypot(td, e);
        if e2 == 0.0 {
            mu -= e / ((td + if td > 0.0 { h } else { -h }) / e);
        } else {
            mu -= e2 / (td + if td > 0.0 { h } else { -h });
        }
    }
    let mut x = diag[start] - mu;
    let mut z = subdiag[start];
    let mut k = start;
    while k < end && z != 0.0 {
        let (c, s) = make_givens(x, z);
        let sdk = s * diag[k] + c * subdiag[k];
        let dkp1 = s * subdiag[k] + c * diag[k + 1];
        diag[k] = c * (c * diag[k] - s * subdiag[k]) - s * (c * subdiag[k] - s * diag[k + 1]);
        diag[k + 1] = s * sdk + c * dkp1;
        subdiag[k] = c * sdk - s * dkp1;
        if k > start {
            subdiag[k - 1] = c * subdiag[k - 1] - s * z;
        }
        x = subdiag[k];
        if k < end - 1 {
            z = -s * subdiag[k + 1];
            subdiag[k + 1] = c * subdiag[k + 1];
        }
        // q.applyOnTheRight(k, k + 1, rot): the rotation's transpose (c, −s) applied to
        // columns k and k + 1, row by row (`apply_rotation_in_the_plane` returns at once for
        // the identity rotation).
        let s_t = -s;
        if !(c == 1.0 && s_t == 0.0) {
            for row in q.iter_mut() {
                let xi = row[k];
                let yi = row[k + 1];
                row[k] = c * xi + s_t * yi;
                row[k + 1] = -s_t * xi + c * yi;
            }
        }
        k += 1;
    }
}

/// `SelfAdjointEigenSolver<Matrix<double, 4, 4, RowMajor>>(a)` with eigenvectors, as ITK's
/// `SymmetricEigenAnalysisFixedDimension<4>::ComputeEigenValuesAndVectors` returns it (values
/// in ascending order; `vectors[k]` the eigenvector of `values[k]`). Only the lower triangle
/// of `a` is read.
pub fn self_adjoint_eigen_4(a: &[[f64; N]; N]) -> SelfAdjointEigen4 {
    let t = tridiagonal_4(a);
    let (mut diag, mut subdiag, mut m, scale) = (t.diag, t.subdiag, t.q, t.scale);
    // computeFromTridiagonal_impl.
    let considered_zero = f64::MIN_POSITIVE;
    let precision_inv = 1.0 / f64::EPSILON;
    let max_iterations = 30usize;
    let mut end = N - 1;
    let mut start = 0usize;
    let mut iter = 0usize;
    while end > 0 {
        for i in start..end {
            if subdiag[i].abs() < considered_zero {
                subdiag[i] = 0.0;
            } else {
                let scaled = precision_inv * subdiag[i];
                if scaled * scaled <= diag[i].abs() + diag[i + 1].abs() {
                    subdiag[i] = 0.0;
                }
            }
        }
        while end > 0 && subdiag[end - 1] == 0.0 {
            end -= 1;
        }
        if end == 0 {
            break;
        }
        iter += 1;
        if iter > max_iterations * N {
            break;
        }
        start = end - 1;
        while start > 0 && subdiag[start - 1] != 0.0 {
            start -= 1;
        }
        qr_step(&mut diag, &mut subdiag, start, end, &mut m);
    }
    let converged = iter <= max_iterations * N;
    if converged {
        for i in 0..N - 1 {
            // diag.segment(i, N − i).minCoeff(&k): the first smallest.
            let mut k = 0usize;
            let mut best = diag[i];
            for j in 1..N - i {
                if diag[i + j] < best {
                    best = diag[i + j];
                    k = j;
                }
            }
            if k > 0 {
                diag.swap(i, i + k);
                for row in m.iter_mut() {
                    row.swap(i, i + k);
                }
            }
        }
    }
    let values: [f64; N] = std::array::from_fn(|i| diag[i] * scale);
    // ITK: EigenVectors[row][col] = eigenVectors(col, row).
    let vectors: [[f64; N]; N] = std::array::from_fn(|r| std::array::from_fn(|c| m[c][r]));
    SelfAdjointEigen4 {
        values,
        vectors,
        converged,
    }
}

/// The solver's first stage, [`self_adjoint_eigen_4`] before its QR iteration: the scale,
/// the tridiagonal matrix, the Householder coefficients and the accumulated `Q` (row-major).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tridiagonal4 {
    pub scale: f64,
    pub diag: [f64; N],
    pub subdiag: [f64; N - 1],
    pub h_coeffs: [f64; N - 1],
    pub q: [[f64; N]; N],
}

/// [`Tridiagonal4`] of `a` (its lower triangle).
pub fn tridiagonal_4(a: &[[f64; N]; N]) -> Tridiagonal4 {
    // mat = the lower triangle; scale = its largest absolute entry.
    let mut m = [[0.0f64; N]; N];
    let mut scale = 0.0f64;
    for i in 0..N {
        for j in 0..=i {
            m[i][j] = a[i][j];
        }
    }
    for row in &m {
        for &v in row {
            let av = v.abs();
            if av > scale {
                scale = av;
            }
        }
    }
    if scale == 0.0 {
        scale = 1.0;
    }
    for i in 0..N {
        for j in 0..=i {
            m[i][j] /= scale;
        }
    }
    let mut h_coeffs = [0.0f64; N - 1];
    tridiagonalize(&mut m, &mut h_coeffs);
    let diag: [f64; N] = std::array::from_fn(|i| m[i][i]);
    let subdiag: [f64; N - 1] = std::array::from_fn(|i| m[i + 1][i]);
    accumulate_q(&mut m, &h_coeffs);
    Tridiagonal4 {
        scale,
        diag,
        subdiag,
        h_coeffs,
        q: m,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn residual(a: &[[f64; 4]; 4], e: &SelfAdjointEigen4) -> f64 {
        let mut worst = 0.0f64;
        for k in 0..4 {
            let v = e.vectors[k];
            for i in 0..4 {
                let av: f64 = (0..4).map(|j| a[i][j] * v[j]).sum();
                worst = worst.max((av - e.values[k] * v[i]).abs());
            }
        }
        worst
    }

    #[test]
    fn diagonalizes_a_symmetric_matrix() {
        let a = [
            [4.0, 1.0, -2.0, 0.5],
            [1.0, 3.0, 0.25, 1.0],
            [-2.0, 0.25, 6.0, -1.5],
            [0.5, 1.0, -1.5, 2.0],
        ];
        let e = self_adjoint_eigen_4(&a);
        assert!(e.converged);
        assert!(e.values.windows(2).all(|w| w[0] <= w[1]), "{:?}", e.values);
        assert!(residual(&a, &e) < 1e-13, "{}", residual(&a, &e));
        let trace: f64 = e.values.iter().sum();
        assert!((trace - 15.0).abs() < 1e-13);
    }

    #[test]
    fn handles_the_zero_and_identity_matrices() {
        let z = self_adjoint_eigen_4(&[[0.0; 4]; 4]);
        assert_eq!(z.values, [0.0; 4]);
        let id: [[f64; 4]; 4] =
            std::array::from_fn(|i| std::array::from_fn(|j| f64::from(u8::from(i == j))));
        let e = self_adjoint_eigen_4(&id);
        assert_eq!(e.values, [1.0; 4]);
        assert_eq!(e.vectors, id);
    }
    /// The solver in ITK v5.4.5's build (GCC 14.3, ANTs' flags) on three matrices: a random
    /// one, a Horn matrix of landmarks, and one with a tiny off-diagonal pair (the values and
    /// eigenvectors as bit patterns, ITK's rows).
    #[test]
    fn matches_itks_build() {
        #[rustfmt::skip]
        let cases: [([[f64; 4]; 4], [u64; 20]); 3] = [
            (
                [[0.21034862167163396, 1.6785614224246017, -1.5824060734556005, 0.39218242611156273], [1.678561422424602, 1.876755150529869, 0.8462630442748599, 0.44000271354112197], [-1.5824060734556005, 0.8462630442748599, 2.003380526751736, 0.8545636295814946], [0.39218242611156273, 0.44000271354112186, 0.8545636295814945, -1.090484298953238]],
                [0xbffffffffffffffd, 0xbff0000000000000, 0x4007fffffffffffe, 0x4008000000000000, 0xbfe4bbacc364a8a9, 0x3fd4287959d4b2d0, 0xbfdc00b67a9f0e40, 0x3fe13832ec941586, 0xbfda9852e4f1828a, 0x3fd957cd3ac50f40, 0xbfb96816350918f6, 0xbfea029412bc4272, 0x3fe46da3cfb0a470, 0x3fe27a4f99c1ecd6, 0xbfe0470f266d0570, 0x3f9175574da52f7a, 0x0000000000000000, 0x3fe4810c55016327, 0x3fe7839ffb8ff9f6, 0x3fcc78ddb1608c85],
            ),
            (
                [[18011.547619047622, 92061.07142857143, -81920.30952380953, 21502.738095238103], [92061.07142857143, -58574.880952380954, -56571.26190476191, 19717.309523809527], [-81920.30952380953, -56571.26190476191, 7550.595238095229, -81872.78571428572], [21502.738095238103, 19717.309523809527, -81872.78571428572, 33012.738095238106]],
                [0xc0fd53f3690637e7, 0xc0f570929a013404, 0x40d6bd1f568a3e72, 0x41068a9f16b26e27, 0x3fe1bc5785e7aa36, 0xbfea9c8ee91a4efd, 0x3f8261e9f95c30cf, 0x3fa16ee9896f6cc5, 0xbfd47b31ba6dbcd6, 0xbfcebea36c863f87, 0xbfe99d7d189a3295, 0xbfdc8f4af7a6e918, 0xbfe0eb931f04802d, 0xbfd49b7fa127dd6b, 0xbfbfd0a20b53e2cf, 0x3fe8d054ebe995f4, 0x3fe1d7261bf800a2, 0x3fd88ad6255d416c, 0xbfe2c2bd86e5be2c, 0x3fdc81ff5b6bcf09],
            ),
            (
                [[2.0, 0.0, 0.0, 0.0], [0.0, 2.0, 0.0, 0.0], [0.0, 0.0, -1.0, 1e-13], [0.0, 0.0, 1e-13, 3.0]],
                [0xbff0000000000000, 0x4000000000000000, 0x4000000000000000, 0x4008000000000000, 0x0000000000000000, 0x0000000000000000, 0xbff0000000000000, 0x3d1c25c268497682, 0x0000000000000000, 0x3ff0000000000000, 0x0000000000000000, 0x0000000000000000, 0x3ff0000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x8000000000000000, 0x8000000000000000, 0xbd1c25c268497682, 0xbff0000000000000],
            ),
        ];
        for (a, want) in cases {
            let e = self_adjoint_eigen_4(&a);
            let got: Vec<u64> = e
                .values
                .iter()
                .chain(e.vectors.iter().flatten())
                .map(|v| v.to_bits())
                .collect();
            assert_eq!(got, want.to_vec(), "{a:?}");
        }
    }
}
