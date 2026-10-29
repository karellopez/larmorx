// SPDX-License-Identifier: LicenseRef-FSL AND Apache-2.0
// Translated from Armadillo 14.2.1 as bundled by fsl-armawrap 0.7.0 (FSL 6.0.7.23) and Armadillo
// 10.5.1 as bundled by fsl-armawrap 0.6.0 (FSL 6.0.7.17) (Apache-2.0; Copyright 2008-2021 Conrad
// Sanderson, 2008-2016 National ICT Australia (NICTA), 2017-2020 Arroyo Consortium, 2017-2021
// Data61 / CSIRO): mul_gemm.hpp, mul_gemv.hpp, op_dot_meat.hpp, op_inv_gen_meat.hpp (14.2.1),
// op_inv_meat.hpp (10.5.1), op_det_meat.hpp, auxlib_meat.hpp (inv, inv_tr, det,
// solve_square_fast; 10.5.1's solve_square_tiny), trimat_helper.hpp, sym_helper.hpp, fn_trace.hpp,
// fn_accu.hpp, glue_times_meat.hpp. Part of larmorx-nc (FSL Licence terms; see LICENSE).
//! The small dense matrices of FSL's NEWMAT layer, evaluated the way FSL's binaries evaluate
//! them.
//!
//! FSL 6.0.7.23's `NEWMAT::Matrix` is `armawrap` 0.7.0, a thin layer over Armadillo 14.2.1
//! (6.0.7.17's was 0.6.0 over Armadillo 10.5.1); every matrix expression in newimage,
//! miscmaths, FLIRT and mcflirt is an Armadillo expression template, and the order of its
//! floating-point operations is Armadillo's. This module gives those operations, one function
//! per Armadillo code path, so that a translation of FSL code can say exactly which path an
//! expression takes:
//!
//! - products of square matrices up to 4 × 4 ([`mul_tinysq`]), of a square matrix and a vector
//!   up to 4 ([`gemv_tinysq`]) and the general small product of Armadillo's emulation
//!   ([`gemm_emul_large`]) (the same in both releases);
//! - `inv()` ([`inv`]: closed forms for 2 × 2 and 3 × 3, then a diagonal matrix's reciprocals,
//!   LAPACK's triangular inverse, or its LU inverse), `det()` ([`det`]) and `inv(A)*B` ([`solve`]:
//!   rewritten by Armadillo as a solve, LAPACK `dgesv`), the LAPACK routines as FSL 6.0.7.23's
//!   OpenBLAS computes them ([`crate::lapack`]); Armadillo 10.5.1's closed-form rules are kept
//!   in [`armadillo_10_5_1`];
//! - `dot()` ([`dot`]: two interleaved accumulators), `accu()` ([`accu`]), `trace()` ([`trace`]).
//!
//! The rules that decide which path an expression takes, as Armadillo applies them, are
//! documented on each function; the call sites in the translated code follow them (e.g.
//! `A.i() * B * C` is `solve(A, B * C)`, `A * B.i() * C` is `A * solve(B, C)`, and `A * B * C`
//! of equal square sizes is `(A * B) * C`).
//!
//! FSL's binaries were built with GCC at `-O2 -march=nocona` (13.2 for 6.0.7.17, 15.2 for
//! 6.0.7.23): no fused multiply-adds and no reassociation, so every `a*b + c` below is two
//! roundings, in the order written.

use std::fmt;

use crate::lapack;
use crate::sse::{self, S64};

/// A column-major dense matrix of `f64` (Armadillo's `Mat<double>`; NEWMAT indices are 1-based,
/// these are 0-based).
#[derive(Clone, PartialEq)]
pub struct Mat {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl fmt::Debug for Mat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Mat {}x{} [", self.rows, self.cols)?;
        for r in 0..self.rows {
            let row: Vec<String> = (0..self.cols)
                .map(|c| format!("{:?}", self.at(r, c)))
                .collect();
            writeln!(f, "  {}", row.join(", "))?;
        }
        write!(f, "]")
    }
}

impl Mat {
    /// A `rows` × `cols` matrix of zeros.
    pub fn zeros(rows: usize, cols: usize) -> Mat {
        Mat {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }

    /// The `n` × `n` identity (`IdentityMatrix(n)`).
    pub fn eye(n: usize) -> Mat {
        let mut m = Mat::zeros(n, n);
        for i in 0..n {
            m.set(i, i, 1.0);
        }
        m
    }

    /// A column vector (`ColumnVector`) with these values.
    pub fn col(values: &[f64]) -> Mat {
        Mat {
            rows: values.len(),
            cols: 1,
            data: values.to_vec(),
        }
    }

    /// A matrix from its values in column-major order (`values.len() == rows * cols`, column by
    /// column: Armadillo's and LAPACK's own layout).
    pub fn from_col_major(rows: usize, cols: usize, values: &[f64]) -> Mat {
        assert_eq!(values.len(), rows * cols);
        Mat {
            rows,
            cols,
            data: values.to_vec(),
        }
    }

    /// A matrix from row-major values (the order NEWMAT's `<<` reads them in).
    pub fn from_rows(rows: usize, cols: usize, values: &[f64]) -> Mat {
        assert_eq!(values.len(), rows * cols);
        let mut m = Mat::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, values[r * cols + c]);
            }
        }
        m
    }

    /// A 4 × 4 matrix from row-major rows.
    pub fn from4(m: [[f64; 4]; 4]) -> Mat {
        let mut out = Mat::zeros(4, 4);
        for (r, row) in m.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                out.set(r, c, *v);
            }
        }
        out
    }

    /// The rows of a 4 × 4 matrix.
    pub fn to4(&self) -> [[f64; 4]; 4] {
        assert!(self.rows == 4 && self.cols == 4);
        std::array::from_fn(|r| std::array::from_fn(|c| self.at(r, c)))
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Number of elements (`n_elem`, NEWMAT's `Storage()`).
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The column-major values (`memptr()`).
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        &mut self.data
    }

    /// Element `(r, c)`, 0-based.
    #[inline]
    pub fn at(&self, r: usize, c: usize) -> f64 {
        debug_assert!(r < self.rows && c < self.cols);
        self.data[c * self.rows + r]
    }

    #[inline]
    pub fn set(&mut self, r: usize, c: usize, v: f64) {
        debug_assert!(r < self.rows && c < self.cols);
        self.data[c * self.rows + r] = v;
    }

    /// Element `i` of a vector, 0-based (NEWMAT's 1-based `v(i+1)`).
    #[inline]
    pub fn get(&self, i: usize) -> f64 {
        self.data[i]
    }

    #[inline]
    pub fn put(&mut self, i: usize, v: f64) {
        self.data[i] = v;
    }

    /// Column `c` as a slice.
    pub fn colptr(&self, c: usize) -> &[f64] {
        &self.data[c * self.rows..(c + 1) * self.rows]
    }

    /// `SubMatrix(r0+1, r1, c0+1, c1)` copied out (a subview's `unwrap`): rows `r0..r1` and
    /// columns `c0..c1`.
    pub fn sub(&self, r0: usize, r1: usize, c0: usize, c1: usize) -> Mat {
        let mut out = Mat::zeros(r1 - r0, c1 - c0);
        for c in c0..c1 {
            for r in r0..r1 {
                out.set(r - r0, c - c0, self.at(r, c));
            }
        }
        out
    }

    /// Assigns `m` to the block starting at `(r0, c0)` (`SubMatrix(...) = m`).
    pub fn set_sub(&mut self, r0: usize, c0: usize, m: &Mat) {
        for c in 0..m.cols {
            for r in 0..m.rows {
                self.set(r0 + r, c0 + c, m.at(r, c));
            }
        }
    }

    /// The transpose, copied (`op_strans`).
    pub fn t(&self) -> Mat {
        let mut out = Mat::zeros(self.cols, self.rows);
        for c in 0..self.cols {
            for r in 0..self.rows {
                out.set(c, r, self.at(r, c));
            }
        }
        out
    }

    /// Element-wise `self + other` (`eglue_plus`).
    pub fn plus(&self, other: &Mat) -> Mat {
        assert_eq!((self.rows, self.cols), (other.rows, other.cols));
        Mat {
            rows: self.rows,
            cols: self.cols,
            data: self
                .data
                .iter()
                .zip(&other.data)
                .map(|(a, b)| sse::add_f64(*a, *b))
                .collect(),
        }
    }

    /// Element-wise `self - other` (`eglue_minus`).
    pub fn minus(&self, other: &Mat) -> Mat {
        assert_eq!((self.rows, self.cols), (other.rows, other.cols));
        Mat {
            rows: self.rows,
            cols: self.cols,
            data: self
                .data
                .iter()
                .zip(&other.data)
                .map(|(a, b)| sse::sub_f64(*a, *b))
                .collect(),
        }
    }

    /// `self * k` (`eop_scalar_times`: each element times `k`).
    pub fn scale(&self, k: f64) -> Mat {
        Mat {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().map(|a| sse::mul_f64(*a, k)).collect(),
        }
    }

    /// `self / k` (`eop_scalar_div_post`: each element divided by `k`).
    pub fn div_scalar(&self, k: f64) -> Mat {
        Mat {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().map(|a| sse::div_f64(*a, k)).collect(),
        }
    }

    /// `MaximumAbsoluteValue()`: Armadillo's `max()` of `abs()`, which starts from the most
    /// negative value and keeps strictly larger ones, so NaN elements never win; all-NaN gives
    /// `-inf`.
    pub fn max_abs(&self) -> f64 {
        let abs: Vec<f64> = self.data.iter().map(|v| v.abs()).collect();
        direct_max(&abs)
    }

    /// `SumSquare()`: `accu(square(X))`.
    pub fn sum_square(&self) -> f64 {
        let sq: Vec<f64> = self.data.iter().map(|v| sse::mul_f64(*v, *v)).collect();
        accu(&sq)
    }

    /// `SumAbsoluteValue()`: `accu(abs(X))`.
    pub fn sum_abs(&self) -> f64 {
        let a: Vec<f64> = self.data.iter().map(|v| v.abs()).collect();
        accu(&a)
    }
}

/// Armadillo's `op_max::direct_max`: starting from `-inf`, each element strictly greater than
/// the current maximum replaces it (so NaN elements never do).
pub fn direct_max(x: &[f64]) -> f64 {
    let mut max_val = f64::NEG_INFINITY;
    for &v in x {
        if v > max_val {
            max_val = v;
        }
    }
    max_val
}

/// Armadillo's `accu()` of a dense expression (`accu_proxy_linear`): two interleaved partial
/// sums, then their sum.
pub fn accu(x: &[f64]) -> f64 {
    let mut val1 = S64(0.0);
    let mut val2 = S64(0.0);
    let n = x.len();
    let mut i = 0;
    let mut j = 1;
    while j < n {
        val1 += x[i];
        val2 += x[j];
        i += 2;
        j += 2;
    }
    if i < n {
        val1 += x[i];
    }
    (val1 + val2).0
}

/// `op_dot::direct_dot_arma` (used by `dot()`, `as_scalar(a.t()*b)` and the small products for
/// up to 32 elements): two interleaved accumulators, then their sum.
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    assert!(
        a.len() <= 32,
        "dot of more than 32 elements goes to BLAS (not ported)"
    );
    let n = a.len();
    let mut val1 = S64(0.0);
    let mut val2 = S64(0.0);
    let mut i = 0;
    let mut j = 1;
    while j < n {
        val1 += S64(a[i]) * b[i];
        val2 += S64(a[j]) * b[j];
        i += 2;
        j += 2;
    }
    if i < n {
        val1 += S64(a[i]) * b[i];
    }
    (val1 + val2).0
}

/// `gemv_emul_tinysq`: `y = A x` (or `Aᵀ x` with `trans`) for a square `A` of size 1 to 4, each
/// element a left-to-right sum of products.
pub fn gemv_tinysq(a: &Mat, x: &[f64], trans: bool) -> Vec<f64> {
    let n = a.rows;
    assert!(n == a.cols && (1..=4).contains(&n) && x.len() == n);
    let am = &a.data;
    // pos<row, col>: row + col*n, or col + row*n when transposed.
    let pos = |row: usize, col: usize| {
        if trans { col + row * n } else { row + col * n }
    };
    (0..n)
        .map(|r| {
            let mut acc = S64(am[pos(r, 0)]) * x[0];
            for (c, xc) in x.iter().enumerate().skip(1) {
                acc += S64(am[pos(r, c)]) * *xc;
            }
            acc.0
        })
        .collect()
}

/// `gemm_emul_tinysq`: `C = A B` for square `A`, `B` of the same size 1 to 4: column by column,
/// [`gemv_tinysq`] of `A` with each column of `B`.
pub fn mul_tinysq(a: &Mat, b: &Mat) -> Mat {
    let n = a.rows;
    assert!(n == a.cols && n == b.rows && n == b.cols && (1..=4).contains(&n));
    let mut out = Mat::zeros(n, n);
    for c in 0..n {
        let y = gemv_tinysq(a, b.colptr(c), false);
        for (r, v) in y.into_iter().enumerate() {
            out.set(r, c, v);
        }
    }
    out
}

/// `gemm_emul_large<false, false>`: `C = A B` element by element, each a [`dot`] of a row of
/// `A` and a column of `B` (Armadillo's emulation, used by `solve()` on tiny matrices).
pub fn gemm_emul_large(a: &Mat, b: &Mat) -> Mat {
    assert_eq!(a.cols, b.rows);
    let mut out = Mat::zeros(a.rows, b.cols);
    let mut row = vec![0.0; a.cols];
    for r in 0..a.rows {
        for (k, v) in row.iter_mut().enumerate() {
            *v = a.at(r, k);
        }
        for c in 0..b.cols {
            out.set(r, c, dot(&row, b.colptr(c)));
        }
    }
    out
}

/// `glue_times::apply` for two evaluated operands, `A B` (`trans_a`: `Aᵀ B`), as Armadillo
/// dispatches it for `double`:
///
/// - a column-vector right operand: `gemv` (square `A` up to 4: [`gemv_tinysq`]);
/// - a row-vector left operand times a matrix: `gemv` of `Bᵀ` with the row;
/// - otherwise `gemm` (square operands of equal size up to 4: [`mul_tinysq`]).
///
/// Products that Armadillo hands to BLAS (OpenBLAS in FSL's build) are not ported: they panic,
/// so that a translation never silently uses another operation order.
pub fn mul(a: &Mat, b: &Mat, trans_a: bool) -> Mat {
    let (ar, ac) = if trans_a {
        (a.cols, a.rows)
    } else {
        (a.rows, a.cols)
    };
    assert_eq!(ac, b.rows, "matrix multiplication: incompatible sizes");
    if !trans_a {
        if a.rows == 1 {
            // gemv<true>(B, A): y = Bᵀ a.
            let y = gemv(b, a.as_slice(), true);
            return Mat {
                rows: 1,
                cols: b.cols,
                data: y,
            };
        }
        if b.cols == 1 {
            let y = gemv(a, b.as_slice(), false);
            return Mat::col(&y);
        }
        return gemm(a, b, false);
    }
    // trans_a: Aᵀ B.
    if a.cols == 1 {
        // gemv<true>(B, A): (Aᵀ B) is a row: y = Bᵀ a.
        let y = gemv(b, a.as_slice(), true);
        return Mat {
            rows: 1,
            cols: b.cols,
            data: y,
        };
    }
    if b.cols == 1 {
        let y = gemv(a, b.as_slice(), true);
        return Mat::col(&y);
    }
    let _ = ar;
    gemm(a, b, true)
}

/// `gemv<trans>::apply_blas_type`: square `A` up to 4 × 4 by [`gemv_tinysq`]; the rest goes to
/// BLAS (not ported).
fn gemv(a: &Mat, x: &[f64], trans: bool) -> Vec<f64> {
    if a.rows <= 4 && a.rows == a.cols {
        return gemv_tinysq(a, x, trans);
    }
    panic!(
        "a {}x{} matrix-vector product goes to BLAS in Armadillo (not ported)",
        a.rows, a.cols
    );
}

/// `gemm<trans_a, false>::apply_blas_type`: square operands of equal size up to 4 by
/// [`mul_tinysq`] (a transposed `A` is transposed first); the rest goes to BLAS (not ported).
fn gemm(a: &Mat, b: &Mat, trans_a: bool) -> Mat {
    if a.rows <= 4 && a.rows == a.cols && a.rows == b.rows && b.rows == b.cols {
        if trans_a {
            // gemm_emul_tinysq<do_trans_A=true>: gemv_emul_tinysq<true> per column.
            let n = a.rows;
            let mut out = Mat::zeros(n, n);
            for c in 0..n {
                let y = gemv_tinysq(a, b.colptr(c), true);
                for (r, v) in y.into_iter().enumerate() {
                    out.set(r, c, v);
                }
            }
            return out;
        }
        return mul_tinysq(a, b);
    }
    panic!(
        "a {}x{} by {}x{} matrix product goes to BLAS in Armadillo (not ported)",
        a.rows, a.cols, b.rows, b.cols
    );
}

/// `A B C` for evaluated operands (`glue_times::apply` with three matrices): `(A B) C` when
/// `rows(A)·cols(B) <= rows(B)·cols(C)`, else `A (B C)`.
pub fn mul3(a: &Mat, b: &Mat, c: &Mat) -> Mat {
    if a.rows * b.cols <= b.rows * c.cols {
        mul(&mul(a, b, false), c, false)
    } else {
        mul(a, &mul(b, c, false), false)
    }
}

/// `A B C D` for evaluated operands (`glue_times::apply` with four matrices): `(A B C) D` when
/// `rows(A)·cols(C) <= rows(B)·cols(D)`, else `A (B C D)`.
pub fn mul4(a: &Mat, b: &Mat, c: &Mat, d: &Mat) -> Mat {
    if a.rows * c.cols <= b.rows * d.cols {
        mul(&mul3(a, b, c), d, false)
    } else {
        mul(a, &mul3(b, c, d), false)
    }
}

/// Position of `(row, col)` in an `n` × `n` column-major array.
#[inline]
fn p(n: usize, row: usize, col: usize) -> usize {
    row + col * n
}

/// `op_det::apply_tiny`: the determinant of a square matrix up to 4 × 4 by its closed form.
pub fn det_tiny(x: &Mat) -> f64 {
    let n = x.rows;
    let xm = &x.data;
    match n {
        0 => 1.0,
        1 => xm[0],
        2 => (S64(xm[p(2, 0, 0)]) * xm[p(2, 1, 1)] - S64(xm[p(2, 0, 1)]) * xm[p(2, 1, 0)]).0,
        3 => {
            let q = |r, c| S64(xm[p(3, r, c)]);
            let val1 = q(0, 0) * (q(2, 2) * q(1, 1) - q(2, 1) * q(1, 2));
            let val2 = q(1, 0) * (q(2, 2) * q(0, 1) - q(2, 1) * q(0, 2));
            let val3 = q(2, 0) * (q(1, 2) * q(0, 1) - q(1, 1) * q(0, 2));
            (val1 - val2 + val3).0
        }
        4 => {
            let q = |r, c| S64(xm[p(4, r, c)]);
            let val_03_12 = q(0, 3) * q(1, 2);
            let val_02_13 = q(0, 2) * q(1, 3);
            let val_03_11 = q(0, 3) * q(1, 1);
            let val_01_13 = q(0, 1) * q(1, 3);
            let val_02_11 = q(0, 2) * q(1, 1);
            let val_01_12 = q(0, 1) * q(1, 2);
            let val_03_10 = q(0, 3) * q(1, 0);
            let val_00_13 = q(0, 0) * q(1, 3);
            let val_02_10 = q(0, 2) * q(1, 0);
            let val_00_12 = q(0, 0) * q(1, 2);
            let val_01_10 = q(0, 1) * q(1, 0);
            let val_00_11 = q(0, 0) * q(1, 1);
            let val_21_30 = q(2, 1) * q(3, 0);
            let val_22_30 = q(2, 2) * q(3, 0);
            let val_23_30 = q(2, 3) * q(3, 0);
            let val_20_31 = q(2, 0) * q(3, 1);
            let val_22_31 = q(2, 2) * q(3, 1);
            let val_23_31 = q(2, 3) * q(3, 1);
            let val_20_32 = q(2, 0) * q(3, 2);
            let val_21_32 = q(2, 1) * q(3, 2);
            let val_23_32 = q(2, 3) * q(3, 2);
            let val_20_33 = q(2, 0) * q(3, 3);
            let val_21_33 = q(2, 1) * q(3, 3);
            let val_22_33 = q(2, 2) * q(3, 3);
            (val_03_12 * val_21_30 - val_02_13 * val_21_30 - val_03_11 * val_22_30
                + val_01_13 * val_22_30
                + val_02_11 * val_23_30
                - val_01_12 * val_23_30
                - val_03_12 * val_20_31
                + val_02_13 * val_20_31
                + val_03_10 * val_22_31
                - val_00_13 * val_22_31
                - val_02_10 * val_23_31
                + val_00_12 * val_23_31
                + val_03_11 * val_20_32
                - val_01_13 * val_20_32
                - val_03_10 * val_21_32
                + val_00_13 * val_21_32
                + val_01_10 * val_23_32
                - val_00_11 * val_23_32
                - val_02_11 * val_20_33
                + val_01_12 * val_20_33
                + val_02_10 * val_21_33
                - val_00_12 * val_21_33
                - val_01_10 * val_22_33
                + val_00_11 * val_22_33)
                .0
        }
        _ => panic!("det_tiny: size {n}"),
    }
}

/// `det()` (Armadillo 14.2.1's `op_det::apply_direct`) of a square matrix: 0 × 0 is 1, 1 × 1
/// its element; 2 × 2 and 3 × 3 the closed form when its magnitude lies strictly between
/// machine epsilon and its reciprocal; otherwise a diagonal or triangular matrix the product of
/// its diagonal from 1, and any other matrix by LAPACK `dgetrf` ([`lapack::getrf`]): the product
/// of U's diagonal from its first element, negated for an odd number of row interchanges.
pub fn det(x: &Mat) -> f64 {
    assert_eq!(x.rows, x.cols, "det(): given matrix must be square sized");
    let n = x.rows;
    if n == 0 {
        return 1.0;
    }
    if n == 1 {
        return x.data[0];
    }
    if n <= 3 {
        let d = det_tiny(x);
        let a = d.abs();
        if a > f64::EPSILON && a < 1.0 / f64::EPSILON {
            return d;
        }
    }
    if is_diagmat(x) || is_triu(x) || is_tril(x) {
        let mut val = S64(1.0);
        for i in 0..n {
            val *= x.at(i, i);
        }
        return val.0;
    }
    assert!(n <= 4, "det(): LAPACK's path is ported up to 4 x 4");
    let mut a = x.data.clone();
    let mut ipiv = [0i32; 4];
    lapack::getrf(&mut a, n, &mut ipiv);
    let mut val = S64(a[0]);
    for i in 1..n {
        val *= a[i * n + i];
    }
    let mut sign = 1;
    for (i, &ip) in ipiv.iter().enumerate().take(n) {
        if i as i32 != ip - 1 {
            sign = -sign;
        }
    }
    if sign < 0 { -val.0 } else { val.0 }
}

/// Armadillo 14.2.1's `trimat_helper::is_triu`: at least 2 × 2 and every element below the
/// diagonal exactly 0 (the bottom-left one checked first).
fn is_triu(x: &Mat) -> bool {
    let n = x.rows;
    n >= 2 && (0..n - 1).all(|c| (c + 1..n).all(|r| x.at(r, c) == 0.0))
}

/// `trimat_helper::is_tril`: at least 2 × 2 and every element above the diagonal exactly 0.
fn is_tril(x: &Mat) -> bool {
    let n = x.rows;
    n >= 2 && (1..n).all(|c| (0..c).all(|r| x.at(r, c) == 0.0))
}

/// The determinant by LU decomposition with partial pivoting (LAPACK `dgetrf`'s pivoting and
/// elimination order; Armadillo 10.5.1's fallback below its tiny-matrix threshold).
fn det_lu(x: &Mat) -> f64 {
    let n = x.rows;
    let mut a = x.clone();
    let mut sign = 1.0;
    for k in 0..n {
        // idamax: the first index of the largest |value|.
        let mut piv = k;
        let mut best = a.at(k, k).abs();
        for r in k + 1..n {
            let v = a.at(r, k).abs();
            if v > best {
                best = v;
                piv = r;
            }
        }
        if a.at(piv, k) == 0.0 {
            return 0.0;
        }
        if piv != k {
            for c in 0..n {
                let t = a.at(k, c);
                a.set(k, c, a.at(piv, c));
                a.set(piv, c, t);
            }
            sign = -sign;
        }
        let pivot = a.at(k, k);
        let rec = sse::div_f64(1.0, pivot);
        for r in k + 1..n {
            let l = sse::mul_f64(a.at(r, k), rec);
            a.set(r, k, l);
        }
        for c in k + 1..n {
            let ukc = a.at(k, c);
            for r in k + 1..n {
                let v = (S64(a.at(r, c)) - S64(a.at(r, k)) * ukc).0;
                a.set(r, c, v);
            }
        }
    }
    // Armadillo multiplies the diagonal in order, starting from the first element.
    let mut val = S64(a.at(0, 0));
    for i in 1..n {
        val *= a.at(i, i);
    }
    (val * sign).0
}

/// Why an inverse or a solve failed (Armadillo throws "inv(): matrix is singular").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Singular;

impl fmt::Display for Singular {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("inv(): matrix is singular")
    }
}

impl std::error::Error for Singular {}

/// `op_inv::apply_tiny_noalias`: the inverse of a square matrix up to 4 × 4 by cofactors over
/// the closed-form determinant; `None` when the determinant's magnitude is outside
/// `[eps, 1/eps]` or the first row times the first column of the result is not within `1e-10`
/// of 1 (Armadillo then falls back to other methods). NaN determinants pass both tests.
pub fn inv_tiny(x: &Mat) -> Option<Mat> {
    let n = x.rows;
    assert_eq!(n, x.cols);
    let det_min = f64::EPSILON;
    let det_max = 1.0 / f64::EPSILON;
    let xm = &x.data;
    let mut out = Mat::zeros(n, n);
    match n {
        0 => return Some(out),
        1 => {
            out.data[0] = sse::div_f64(1.0, xm[0]);
            return Some(out);
        }
        2 => {
            let a = S64(xm[p(2, 0, 0)]);
            let b = S64(xm[p(2, 0, 1)]);
            let c = S64(xm[p(2, 1, 0)]);
            let d = S64(xm[p(2, 1, 1)]);
            let det_val = a * d - b * c;
            let abs_det = det_val.0.abs();
            if abs_det < det_min || abs_det > det_max {
                return None;
            }
            out.data[p(2, 0, 0)] = (d / det_val).0;
            out.data[p(2, 0, 1)] = (-b / det_val).0;
            out.data[p(2, 1, 0)] = (-c / det_val).0;
            out.data[p(2, 1, 1)] = (a / det_val).0;
            return Some(out);
        }
        3 => {
            let det_val = det_tiny(x);
            let abs_det = det_val.abs();
            if abs_det < det_min || abs_det > det_max {
                return None;
            }
            let q = |r, c| S64(xm[p(3, r, c)]);
            let o = &mut out.data;
            o[p(3, 0, 0)] = ((q(2, 2) * q(1, 1) - q(2, 1) * q(1, 2)) / det_val).0;
            o[p(3, 1, 0)] = (-(q(2, 2) * q(1, 0) - q(2, 0) * q(1, 2)) / det_val).0;
            o[p(3, 2, 0)] = ((q(2, 1) * q(1, 0) - q(2, 0) * q(1, 1)) / det_val).0;
            o[p(3, 0, 1)] = (-(q(2, 2) * q(0, 1) - q(2, 1) * q(0, 2)) / det_val).0;
            o[p(3, 1, 1)] = ((q(2, 2) * q(0, 0) - q(2, 0) * q(0, 2)) / det_val).0;
            o[p(3, 2, 1)] = (-(q(2, 1) * q(0, 0) - q(2, 0) * q(0, 1)) / det_val).0;
            o[p(3, 0, 2)] = ((q(1, 2) * q(0, 1) - q(1, 1) * q(0, 2)) / det_val).0;
            o[p(3, 1, 2)] = (-(q(1, 2) * q(0, 0) - q(1, 0) * q(0, 2)) / det_val).0;
            o[p(3, 2, 2)] = ((q(1, 1) * q(0, 0) - q(1, 0) * q(0, 1)) / det_val).0;
            let check_val =
                q(0, 0) * o[p(3, 0, 0)] + q(0, 1) * o[p(3, 1, 0)] + q(0, 2) * o[p(3, 2, 0)];
            if (1.0 - check_val).0.abs() >= 1e-10 {
                return None;
            }
            return Some(out);
        }
        4 => {}
        _ => return None,
    }
    let det_val = det_tiny(x);
    let abs_det = det_val.abs();
    if abs_det < det_min || abs_det > det_max {
        return None;
    }
    let q = |r, c| S64(xm[p(4, r, c)]);
    let det_val = S64(det_val);
    let mut o4 = [S64(0.0); 16];
    let o = &mut o4;
    o[p(4, 0, 0)] = (q(1, 2) * q(2, 3) * q(3, 1) - q(1, 3) * q(2, 2) * q(3, 1)
        + q(1, 3) * q(2, 1) * q(3, 2)
        - q(1, 1) * q(2, 3) * q(3, 2)
        - q(1, 2) * q(2, 1) * q(3, 3)
        + q(1, 1) * q(2, 2) * q(3, 3))
        / det_val;
    o[p(4, 1, 0)] =
        (q(1, 3) * q(2, 2) * q(3, 0) - q(1, 2) * q(2, 3) * q(3, 0) - q(1, 3) * q(2, 0) * q(3, 2)
            + q(1, 0) * q(2, 3) * q(3, 2)
            + q(1, 2) * q(2, 0) * q(3, 3)
            - q(1, 0) * q(2, 2) * q(3, 3))
            / det_val;
    o[p(4, 2, 0)] = (q(1, 1) * q(2, 3) * q(3, 0) - q(1, 3) * q(2, 1) * q(3, 0)
        + q(1, 3) * q(2, 0) * q(3, 1)
        - q(1, 0) * q(2, 3) * q(3, 1)
        - q(1, 1) * q(2, 0) * q(3, 3)
        + q(1, 0) * q(2, 1) * q(3, 3))
        / det_val;
    o[p(4, 3, 0)] =
        (q(1, 2) * q(2, 1) * q(3, 0) - q(1, 1) * q(2, 2) * q(3, 0) - q(1, 2) * q(2, 0) * q(3, 1)
            + q(1, 0) * q(2, 2) * q(3, 1)
            + q(1, 1) * q(2, 0) * q(3, 2)
            - q(1, 0) * q(2, 1) * q(3, 2))
            / det_val;

    o[p(4, 0, 1)] =
        (q(0, 3) * q(2, 2) * q(3, 1) - q(0, 2) * q(2, 3) * q(3, 1) - q(0, 3) * q(2, 1) * q(3, 2)
            + q(0, 1) * q(2, 3) * q(3, 2)
            + q(0, 2) * q(2, 1) * q(3, 3)
            - q(0, 1) * q(2, 2) * q(3, 3))
            / det_val;
    o[p(4, 1, 1)] = (q(0, 2) * q(2, 3) * q(3, 0) - q(0, 3) * q(2, 2) * q(3, 0)
        + q(0, 3) * q(2, 0) * q(3, 2)
        - q(0, 0) * q(2, 3) * q(3, 2)
        - q(0, 2) * q(2, 0) * q(3, 3)
        + q(0, 0) * q(2, 2) * q(3, 3))
        / det_val;
    o[p(4, 2, 1)] =
        (q(0, 3) * q(2, 1) * q(3, 0) - q(0, 1) * q(2, 3) * q(3, 0) - q(0, 3) * q(2, 0) * q(3, 1)
            + q(0, 0) * q(2, 3) * q(3, 1)
            + q(0, 1) * q(2, 0) * q(3, 3)
            - q(0, 0) * q(2, 1) * q(3, 3))
            / det_val;
    o[p(4, 3, 1)] = (q(0, 1) * q(2, 2) * q(3, 0) - q(0, 2) * q(2, 1) * q(3, 0)
        + q(0, 2) * q(2, 0) * q(3, 1)
        - q(0, 0) * q(2, 2) * q(3, 1)
        - q(0, 1) * q(2, 0) * q(3, 2)
        + q(0, 0) * q(2, 1) * q(3, 2))
        / det_val;

    o[p(4, 0, 2)] = (q(0, 2) * q(1, 3) * q(3, 1) - q(0, 3) * q(1, 2) * q(3, 1)
        + q(0, 3) * q(1, 1) * q(3, 2)
        - q(0, 1) * q(1, 3) * q(3, 2)
        - q(0, 2) * q(1, 1) * q(3, 3)
        + q(0, 1) * q(1, 2) * q(3, 3))
        / det_val;
    o[p(4, 1, 2)] =
        (q(0, 3) * q(1, 2) * q(3, 0) - q(0, 2) * q(1, 3) * q(3, 0) - q(0, 3) * q(1, 0) * q(3, 2)
            + q(0, 0) * q(1, 3) * q(3, 2)
            + q(0, 2) * q(1, 0) * q(3, 3)
            - q(0, 0) * q(1, 2) * q(3, 3))
            / det_val;
    o[p(4, 2, 2)] = (q(0, 1) * q(1, 3) * q(3, 0) - q(0, 3) * q(1, 1) * q(3, 0)
        + q(0, 3) * q(1, 0) * q(3, 1)
        - q(0, 0) * q(1, 3) * q(3, 1)
        - q(0, 1) * q(1, 0) * q(3, 3)
        + q(0, 0) * q(1, 1) * q(3, 3))
        / det_val;
    o[p(4, 3, 2)] =
        (q(0, 2) * q(1, 1) * q(3, 0) - q(0, 1) * q(1, 2) * q(3, 0) - q(0, 2) * q(1, 0) * q(3, 1)
            + q(0, 0) * q(1, 2) * q(3, 1)
            + q(0, 1) * q(1, 0) * q(3, 2)
            - q(0, 0) * q(1, 1) * q(3, 2))
            / det_val;

    o[p(4, 0, 3)] =
        (q(0, 3) * q(1, 2) * q(2, 1) - q(0, 2) * q(1, 3) * q(2, 1) - q(0, 3) * q(1, 1) * q(2, 2)
            + q(0, 1) * q(1, 3) * q(2, 2)
            + q(0, 2) * q(1, 1) * q(2, 3)
            - q(0, 1) * q(1, 2) * q(2, 3))
            / det_val;
    o[p(4, 1, 3)] = (q(0, 2) * q(1, 3) * q(2, 0) - q(0, 3) * q(1, 2) * q(2, 0)
        + q(0, 3) * q(1, 0) * q(2, 2)
        - q(0, 0) * q(1, 3) * q(2, 2)
        - q(0, 2) * q(1, 0) * q(2, 3)
        + q(0, 0) * q(1, 2) * q(2, 3))
        / det_val;
    o[p(4, 2, 3)] =
        (q(0, 3) * q(1, 1) * q(2, 0) - q(0, 1) * q(1, 3) * q(2, 0) - q(0, 3) * q(1, 0) * q(2, 1)
            + q(0, 0) * q(1, 3) * q(2, 1)
            + q(0, 1) * q(1, 0) * q(2, 3)
            - q(0, 0) * q(1, 1) * q(2, 3))
            / det_val;
    o[p(4, 3, 3)] = (q(0, 1) * q(1, 2) * q(2, 0) - q(0, 2) * q(1, 1) * q(2, 0)
        + q(0, 2) * q(1, 0) * q(2, 1)
        - q(0, 0) * q(1, 2) * q(2, 1)
        - q(0, 1) * q(1, 0) * q(2, 2)
        + q(0, 0) * q(1, 1) * q(2, 2))
        / det_val;

    for (dst, v) in out.data.iter_mut().zip(o4.iter()) {
        *dst = v.0;
    }
    let check_val = q(0, 0) * o4[p(4, 0, 0)]
        + q(0, 1) * o4[p(4, 1, 0)]
        + q(0, 2) * o4[p(4, 2, 0)]
        + q(0, 3) * o4[p(4, 3, 0)];
    if (1.0 - check_val).0.abs() >= 1e-10 {
        return None;
    }
    Some(out)
}

/// `inv()` (Armadillo 14.2.1's `op_inv_gen_full::apply_direct`, no options) of a square matrix:
///
/// - 1 × 1: the reciprocal (singular when the element is 0);
/// - 2 × 2 and 3 × 3: the closed form ([`inv_tiny_2x2`], [`inv_tiny_3x3`]) unless it declines;
/// - a diagonal matrix (every off-diagonal element exactly 0): the reciprocals of its diagonal,
///   the zeros kept as they are (singular at the first zero);
/// - an upper or lower triangular matrix: LAPACK `dtrtri` ([`lapack::trtri`]), the other
///   triangle kept;
/// - any other: LAPACK `dgetrf` then `dgetri` ([`lapack::getrf`], [`lapack::getri`]).
///
/// Armadillo's symmetric path (`dsytrf`/`dsytri`) needs at least 100 rows
/// (`sym_helper::is_approx_sym(A, 100)`), so FSL's small matrices never take it. Only sizes up
/// to 4 × 4 are ported (FSL's NEWMAT layer inverts nothing larger on the replicated paths).
/// FSL 6.0.7.17's Armadillo 10.5.1 computed these with its closed-form cofactor inverse
/// ([`armadillo_10_5_1::inv`]).
pub fn inv(x: &Mat) -> Result<Mat, Singular> {
    assert_eq!(x.rows, x.cols, "inv(): given matrix must be square sized");
    let n = x.rows;
    let mut out = x.clone();
    match n {
        0 => return Ok(out),
        1 => {
            let a = x.data[0];
            out.data[0] = sse::div_f64(1.0, a);
            return if a != 0.0 { Ok(out) } else { Err(Singular) };
        }
        2 => {
            if let Some(m) = inv_tiny_2x2(x) {
                return Ok(m);
            }
        }
        3 => {
            if let Some(m) = inv_tiny_3x3(x) {
                return Ok(m);
            }
        }
        _ => {}
    }
    if is_diagmat(x) {
        for i in 0..n {
            let v = x.at(i, i);
            let r = sse::div_f64(1.0, v);
            if v == 0.0 {
                return Err(Singular);
            }
            out.set(i, i, r);
        }
        return Ok(out);
    }
    assert!(n <= 4, "inv(): LAPACK's path is ported up to 4 x 4");
    let triu = is_triu(x);
    if triu || is_tril(x) {
        return if lapack::trtri(&mut out.data, n, triu) == 0 {
            Ok(out)
        } else {
            Err(Singular)
        };
    }
    let mut ipiv = [0i32; 4];
    if lapack::getrf(&mut out.data, n, &mut ipiv) != 0 {
        return Err(Singular);
    }
    if lapack::getri(&mut out.data, n, &ipiv) != 0 {
        return Err(Singular);
    }
    Ok(out)
}

/// Armadillo 14.2.1's `op_inv_gen_full::apply_tiny_2x2`: the closed form over `a d - b c`, or
/// `None` when the determinant's magnitude is below machine epsilon, above its reciprocal or
/// NaN.
pub fn inv_tiny_2x2(x: &Mat) -> Option<Mat> {
    let xm = &x.data;
    let a = S64(xm[p(2, 0, 0)]);
    let b = S64(xm[p(2, 0, 1)]);
    let c = S64(xm[p(2, 1, 0)]);
    let d = S64(xm[p(2, 1, 1)]);
    let det_val = a * d - b * c;
    let abs_det = det_val.0.abs();
    if abs_det < f64::EPSILON || abs_det > 1.0 / f64::EPSILON || det_val.is_nan() {
        return None;
    }
    let mut out = Mat::zeros(2, 2);
    out.data[p(2, 0, 0)] = (d / det_val).0;
    out.data[p(2, 0, 1)] = (-b / det_val).0;
    out.data[p(2, 1, 0)] = (-c / det_val).0;
    out.data[p(2, 1, 1)] = (a / det_val).0;
    Some(out)
}

/// Armadillo 14.2.1's `op_inv_gen_full::apply_tiny_3x3`: Armadillo 10.5.1's cofactor formula
/// ([`inv_tiny`] for 3 × 3), which now also declines a NaN determinant.
pub fn inv_tiny_3x3(x: &Mat) -> Option<Mat> {
    if det_tiny(x).is_nan() {
        return None;
    }
    inv_tiny(x)
}

/// `Mat::is_diagmat`: every off-diagonal element is exactly 0.
fn is_diagmat(x: &Mat) -> bool {
    for c in 0..x.cols {
        for r in 0..x.rows {
            if r != c && x.at(r, c) != 0.0 {
                return false;
            }
        }
    }
    true
}

/// The inverse by Gauss-Jordan elimination with partial pivoting, column by column of the
/// identity (the fallback of [`inv`]).
fn inv_lu(x: &Mat) -> Result<Mat, Singular> {
    let n = x.rows;
    let mut a = x.clone();
    let mut b = Mat::eye(n);
    for k in 0..n {
        let mut piv = k;
        let mut best = a.at(k, k).abs();
        for r in k + 1..n {
            let v = a.at(r, k).abs();
            if v > best {
                best = v;
                piv = r;
            }
        }
        if a.at(piv, k) == 0.0 || !a.at(piv, k).is_finite() {
            return Err(Singular);
        }
        if piv != k {
            for c in 0..n {
                let t = a.at(k, c);
                a.set(k, c, a.at(piv, c));
                a.set(piv, c, t);
                let t = b.at(k, c);
                b.set(k, c, b.at(piv, c));
                b.set(piv, c, t);
            }
        }
        let pivot = a.at(k, k);
        for c in 0..n {
            a.set(k, c, sse::div_f64(a.at(k, c), pivot));
            b.set(k, c, sse::div_f64(b.at(k, c), pivot));
        }
        for r in 0..n {
            if r == k {
                continue;
            }
            let f = S64(a.at(r, k));
            for c in 0..n {
                a.set(r, c, (S64(a.at(r, c)) - f * a.at(k, c)).0);
                b.set(r, c, (S64(b.at(r, c)) - f * b.at(k, c)).0);
            }
        }
    }
    Ok(b)
}

/// `inv(A) * B` as Armadillo 14.2.1 evaluates it (`glue_times_redirect2_helper`, also for
/// `inv(A) * B * C` and `A * inv(B) * C`): `auxlib::solve_square_fast`, LAPACK `dgesv`
/// ([`lapack::gesv`]) for every size FSL meets (the symmetric `dsytrf`/`dsytrs` path needs at
/// least 100 rows). A zero pivot is an error ("matrix multiplication: problem with matrix
/// inverse"). Only sizes up to 4 × 4 with up to 4 columns of `B` are ported. FSL 6.0.7.17's
/// Armadillo 10.5.1 multiplied `B` by its closed-form inverse ([`armadillo_10_5_1::solve`]).
pub fn solve(a: &Mat, b: &Mat) -> Result<Mat, Singular> {
    assert_eq!(a.rows, b.rows, "solve(): number of rows must be the same");
    assert_eq!(a.rows, a.cols, "inv(): given matrix must be square sized");
    let n = a.rows;
    if n == 0 || b.is_empty() {
        return Ok(Mat::zeros(a.cols, b.cols));
    }
    assert!(
        n <= 4 && b.cols <= 4,
        "solve(): LAPACK's path is ported up to 4 x 4"
    );
    let mut lu = a.data.clone();
    let mut out = b.clone();
    let mut ipiv = [0i32; 4];
    if lapack::gesv(&mut lu, n, &mut ipiv, &mut out.data, b.cols) != 0 {
        return Err(Singular);
    }
    Ok(out)
}

/// The rules of FSL 6.0.7.17's Armadillo 10.5.1 (fsl-armawrap 0.6.0) for `inv()`, `inv(A) * B`
/// and `det()`, kept beside Armadillo 14.2.1's so that the step between the two FSL releases can
/// be reproduced (`docs/findings/fsl-mcflirt.md`, "FSL 6.0.7.23"): the closed-form cofactor
/// inverse and determinant up to 4 × 4, their fallbacks otherwise.
pub mod armadillo_10_5_1 {
    use super::{Mat, Singular, det_lu, det_tiny, gemm_emul_large, inv_lu, inv_tiny, is_diagmat};
    use crate::sse;

    /// `det()` (`op_det::apply_direct`): for up to 4 × 4 the closed form, unless its magnitude
    /// is below machine epsilon, then an LU decomposition.
    pub fn det(x: &Mat) -> f64 {
        assert_eq!(x.rows, x.cols, "det(): given matrix must be square sized");
        if x.rows <= 4 {
            let d = det_tiny(x);
            if d.abs() >= f64::EPSILON {
                return d;
            }
        }
        det_lu(x)
    }

    /// `inv()` (`op_inv::apply_direct`): [`inv_tiny`] for up to 4 × 4, else (or when it
    /// declines) the inverse of a diagonal matrix as reciprocals, and otherwise by LU
    /// decomposition with partial pivoting (reached only by singular or extremely scaled
    /// matrices).
    pub fn inv(x: &Mat) -> Result<Mat, Singular> {
        assert_eq!(x.rows, x.cols, "inv(): given matrix must be square sized");
        if x.rows <= 4
            && let Some(m) = inv_tiny(x)
        {
            return Ok(m);
        }
        let n = x.rows;
        if is_diagmat(x) {
            let mut out = Mat::zeros(n, n);
            let mut ok = true;
            for i in 0..n {
                let v = x.at(i, i);
                if v == 0.0 {
                    ok = false;
                }
                out.set(i, i, sse::div_f64(1.0, v));
            }
            return if ok { Ok(out) } else { Err(Singular) };
        }
        inv_lu(x)
    }

    /// `solve(A, B)` as `auxlib::solve_square_fast` computed it: for `A` up to 4 × 4,
    /// [`inv_tiny`] of `A` times `B` by [`gemm_emul_large`]; otherwise an LU solve.
    pub fn solve(a: &Mat, b: &Mat) -> Result<Mat, Singular> {
        assert_eq!(a.rows, b.rows, "solve(): number of rows must be the same");
        if a.rows <= 4
            && let Some(ai) = inv_tiny(a)
        {
            return Ok(gemm_emul_large(&ai, b));
        }
        let ai = inv_lu(a)?;
        Ok(gemm_emul_large(&ai, b))
    }
}

/// `trace(A)` of an evaluated matrix: two interleaved partial sums of the diagonal.
pub fn trace(a: &Mat) -> f64 {
    let n = a.rows.min(a.cols);
    let mut val1 = S64(0.0);
    let mut val2 = S64(0.0);
    let mut i = 0;
    let mut j = 1;
    while j < n {
        val1 += a.at(i, i);
        val2 += a.at(j, j);
        i += 2;
        j += 2;
    }
    if i < n {
        val1 += a.at(i, i);
    }
    (val1 + val2).0
}

/// `trace(A.t() * B)`, which Armadillo computes without forming the product: the sum, column by
/// column, of `dot(A[:, k], B[:, k])`.
pub fn trace_at_b(a: &Mat, b: &Mat) -> f64 {
    assert_eq!(a.rows, b.rows);
    let n = a.cols.min(b.cols);
    let mut acc = S64(0.0);
    for k in 0..n {
        acc += dot(a.colptr(k), b.colptr(k));
    }
    acc.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiny_products_sum_left_to_right() {
        let a = Mat::from_rows(2, 2, &[1.0, 1e16, 0.0, 1.0]);
        let x = [1.0, 1.0];
        // (1*1 + 1e16*1) = 1e16 + 1 → rounds to 1e16 (even).
        assert_eq!(gemv_tinysq(&a, &x, false), vec![1e16, 1.0]);
        let i4 = Mat::eye(4);
        let m = Mat::from4([
            [1.0, 2.0, 3.0, 4.0],
            [0.5, 1.5, -2.0, 1.0],
            [0.0, 0.25, 1.0, -3.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        assert_eq!(mul_tinysq(&i4, &m), m);
        assert_eq!(mul_tinysq(&m, &i4), m);
    }

    #[test]
    fn inverse_and_solve_of_an_affine() {
        let m = Mat::from4([
            [0.99, -0.1, 0.02, 3.0],
            [0.1, 0.98, 0.05, -1.5],
            [-0.02, -0.05, 1.01, 0.75],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let mi = inv(&m).unwrap();
        let id = mul_tinysq(&m, &mi);
        for r in 0..4 {
            for c in 0..4 {
                let want = if r == c { 1.0 } else { 0.0 };
                assert!((id.at(r, c) - want).abs() < 1e-14);
            }
        }
        let s = Mat::from4([
            [3.0, 0.0, 0.0, 0.0],
            [0.0, 3.0, 0.0, 0.0],
            [0.0, 0.0, 4.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let x = solve(&s, &m).unwrap();
        assert_eq!(x, gemm_emul_large(&inv_tiny(&s).unwrap(), &m));
        // A NaN matrix stays on the closed form (NaN fails neither test).
        let nan = Mat::from4([[f64::NAN; 4]; 4]);
        assert!(
            inv_tiny(&nan)
                .unwrap()
                .as_slice()
                .iter()
                .all(|v| v.is_nan())
        );
    }

    #[test]
    fn reductions() {
        assert_eq!(dot(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]), (4.0 + 18.0) + 10.0);
        assert_eq!(
            accu(&[1.0, 2.0, 3.0, 4.0, 5.0]),
            (1.0 + 3.0 + 5.0) + (2.0 + 4.0)
        );
        assert_eq!(direct_max(&[f64::NAN, 1.0, 2.0]), 2.0);
        assert_eq!(direct_max(&[f64::NAN]), f64::NEG_INFINITY);
        let m = Mat::from_rows(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(trace(&m), 5.0);
        assert_eq!(trace_at_b(&m, &m), (1.0 + 9.0) + (4.0 + 16.0));
        assert_eq!(det(&m), -2.0);
    }
}
