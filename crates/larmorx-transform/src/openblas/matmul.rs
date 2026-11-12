// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy's `matmul` (`a @ b`) of two 2-D float64 arrays as numpy 2.5.4 and 2.3.5 dispatch it
//! (`numpy/_core/src/umath/matmul.c.src`, `DOUBLE_matmul`, `DOUBLE_gemv`,
//! `DOUBLE_matmul_matrixmatrix`, `DOUBLE_matmul_inner_noblas`; `arraytypes.c.src`, `DOUBLE_dot`)
//! onto OpenBLAS's Haswell kernels ([`super::level3`], [`super::level2`]), into a C-ordered
//! result:
//!
//! - an empty dimension: zeros (the loop without BLAS);
//! - `(1 × n) @ (n × 1)`: `0. + ddot` (`DOUBLE_dot`);
//! - one inner element with a vector side (`n = 1`, `m = 1` or `p = 1`), or an outer product
//!   (`n = 1`): the loop without BLAS, each output `0 + a·b`;
//! - a row vector times a matrix (`m = 1`): `dgemv`, transposed when the matrix's rows are
//!   strided (F order), not transposed when it is C-ordered (CBLAS turns the row-major call
//!   around); a matrix times a column (`p = 1`): `dgemv_t` for a C-ordered matrix, `dgemv_n`
//!   for an F-ordered one;
//! - otherwise `cblas_dgemm(RowMajor, ...)`, which OpenBLAS computes as the column-major
//!   `Cᵀ = op(B)ᵀ·op(A)ᵀ` (so the kernel's rows are numpy's columns), each operand `'N'` when
//!   C-ordered and `'T'` when F-ordered; an operand that is neither is first copied (numpy's
//!   `DOUBLE_matrix_copy`, to the order its strides lean to). numpy calls `dsyrk` instead when
//!   both operands are one buffer seen as `x` and `x.T` (`x @ x.T`, `x.T @ x`): [`syrk_case`]
//!   tells, `super::level3::dsyrk` computes it.
//!
//! Strides are in elements and positive. Upstream files: numpy 2.5.4 and 2.3.5 `matmul.c.src`
//! (the dispatch is the same for positive strides), `arraytypes.c.src`; OpenBLAS 0.3.34 and
//! 0.3.30 `interface/gemm.c` (CBLAS), `interface/gemv.c` (BSD-3-Clause; the notices are in the
//! parent module and NOTICE). The logic is re-expressed in Rust; no code was copied.

use super::level2::{ddot, dgemv};
use super::level3::{GEMM_Q, Operand, Trans, dgemm_with, dsyrk_lower_with};

/// A 2-D float64 array: `data[off + i·s0 + j·s1]` for `i < rows`, `j < cols`.
#[derive(Clone, Copy, Debug)]
pub struct Array2<'a> {
    /// The storage.
    pub data: &'a [f64],
    /// The offset of element `(0, 0)`.
    pub off: usize,
    /// Rows.
    pub rows: usize,
    /// Columns.
    pub cols: usize,
    /// The stride between rows, in elements.
    pub s0: usize,
    /// The stride between columns, in elements.
    pub s1: usize,
}

/// numpy's `is_blasable2d(stride1, stride2, d1, d2)` in elements: the fast axis contiguous,
/// the slow one at least as long as the fast dimension.
fn blasable(s1: usize, s2: usize, d2: usize) -> bool {
    s2 == 1 && s1 >= d2
}

/// Whether numpy's `matmul` of `a` and `b` takes `dsyrk` (the same data, `m = p`, and the
/// strides of a matrix and its transpose): `Some(trans)` with the `TRANS` of the row-major
/// `cblas_dsyrk(Upper)` call, after which numpy copies the upper triangle to the lower one.
pub fn syrk_case(a: Array2<'_>, b: Array2<'_>, same_data: bool) -> Option<Trans> {
    let (m, n, p) = (a.rows, a.cols, b.cols);
    if !same_data || a.off != b.off || m != p || m <= 1 || n <= 1 {
        return None;
    }
    let t1 = if blasable(a.s0, a.s1, n) {
        Trans::N
    } else if blasable(a.s1, a.s0, m) {
        Trans::T
    } else {
        return None;
    };
    let t2 = if blasable(b.s0, b.s1, p) {
        Trans::N
    } else if blasable(b.s1, b.s0, n) {
        Trans::T
    } else {
        return None;
    };
    (a.s0 == b.s1 && a.s1 == b.s0 && t1 != t2).then_some(t1)
}

/// numpy's `dsyrk` case of `a @ b` ([`syrk_case`] gave `trans`): `cblas_dsyrk(RowMajor, Upper,
/// trans, m, n, 1, a, lda, 0, out, m)`, which OpenBLAS computes as the column-major lower
/// triangle with the other `TRANS` ([`super::level3::dsyrk_lower`]), then numpy's copy of
/// the triangle (`out[j·m + i] = out[i·m + j]` for `j > i`). C-ordered `m × m`.
///
/// # Panics
/// When `m > SYRK_MAX_N`.
pub fn numpy_syrk(a: Array2<'_>, trans: Trans) -> Vec<f64> {
    numpy_syrk_with(a, trans, GEMM_Q)
}

/// [`numpy_syrk`] with the depth blocked by `gemm_q` (`super::level3::dgemm_with`).
pub fn numpy_syrk_with(a: Array2<'_>, trans: Trans, gemm_q: usize) -> Vec<f64> {
    let (m, n) = (a.rows, a.cols);
    let mut out = vec![0.0f64; m * m];
    match trans {
        Trans::N => dsyrk_lower_with(
            Trans::T,
            m,
            n,
            1.0,
            a.data,
            a.off,
            a.s0,
            0.0,
            &mut out,
            0,
            m,
            gemm_q,
        ),
        Trans::T => dsyrk_lower_with(
            Trans::N,
            m,
            n,
            1.0,
            a.data,
            a.off,
            a.s1,
            0.0,
            &mut out,
            0,
            m,
            gemm_q,
        ),
    }
    for i in 0..m {
        for j in i + 1..m {
            out[j * m + i] = out[i * m + j];
        }
    }
    out
}

/// The values of `x` in C order (`DOUBLE_matrix_copy` lays them out in the order its strides
/// lean to; the values are the same, so only the layout passed on to BLAS follows numpy).
fn copy_c(x: Array2<'_>) -> Vec<f64> {
    let mut out = Vec::with_capacity(x.rows * x.cols);
    for i in 0..x.rows {
        for j in 0..x.cols {
            out.push(x.data[x.off + i * x.s0 + j * x.s1]);
        }
    }
    out
}

/// `a @ b` (module documentation) for `a` of `m × n` and `b` of `n × p`, C-ordered `m × p`.
/// Callers check [`syrk_case`] first.
///
/// # Panics
/// When the inner dimensions differ.
pub fn numpy_matmul(a: Array2<'_>, b: Array2<'_>) -> Vec<f64> {
    numpy_matmul_with(a, b, GEMM_Q)
}

/// [`numpy_matmul`] with the depth blocked by `gemm_q` (`super::level3::dgemm_with`).
///
/// # Panics
/// When the inner dimensions differ.
pub fn numpy_matmul_with(a: Array2<'_>, b: Array2<'_>, gemm_q: usize) -> Vec<f64> {
    assert_eq!(a.cols, b.rows, "matmul: inner dimensions differ");
    let (m, n, p) = (a.rows, a.cols, b.cols);
    let mut out = vec![0.0f64; m * p];
    if m == 0 || n == 0 || p == 0 {
        return out;
    }
    let av = |i: usize, j: usize| a.data[a.off + i * a.s0 + j * a.s1];
    let bv = |i: usize, j: usize| b.data[b.off + i * b.s0 + j * b.s1];
    if m == 1 && p == 1 {
        out[0] = 0.0 + ddot(n, a.data, a.off, a.s1, b.data, b.off, b.s0);
        return out;
    }
    if n == 1 {
        for i in 0..m {
            for j in 0..p {
                out[i * p + j] = 0.0 + av(i, 0) * bv(0, j);
            }
        }
        return out;
    }
    let a_blas = blasable(a.s0, a.s1, n) || blasable(a.s1, a.s0, m);
    let b_blas = blasable(b.s0, b.s1, p) || blasable(b.s1, b.s0, n);
    if m == 1 && b_blas {
        // vector @ matrix: gemv(B, is2_p, is2_n, x = a's row).
        if blasable(b.s1, b.s0, n) {
            dgemv(
                Trans::T,
                n,
                p,
                1.0,
                b.data,
                b.off,
                b.s1,
                a.data,
                a.off,
                a.s1,
                0.0,
                &mut out,
                0,
                1,
            );
        } else {
            dgemv(
                Trans::N,
                p,
                n,
                1.0,
                b.data,
                b.off,
                b.s0,
                a.data,
                a.off,
                a.s1,
                0.0,
                &mut out,
                0,
                1,
            );
        }
        return out;
    }
    if p == 1 && a_blas {
        // matrix @ vector.
        if blasable(a.s0, a.s1, n) {
            dgemv(
                Trans::T,
                n,
                m,
                1.0,
                a.data,
                a.off,
                a.s0,
                b.data,
                b.off,
                b.s0,
                0.0,
                &mut out,
                0,
                1,
            );
        } else {
            dgemv(
                Trans::N,
                m,
                n,
                1.0,
                a.data,
                a.off,
                a.s1,
                b.data,
                b.off,
                b.s0,
                0.0,
                &mut out,
                0,
                1,
            );
        }
        return out;
    }
    if m == 1 || p == 1 {
        // A vector side with a non-BLAS operand: the loop without BLAS, from 0, in order.
        for i in 0..m {
            for j in 0..p {
                let mut s = 0.0f64;
                for t in 0..n {
                    s += av(i, t) * bv(t, j);
                }
                out[i * p + j] = s;
            }
        }
        return out;
    }
    // Matrix @ matrix: operands that BLAS cannot take are copied (C order here).
    let (a_copy, b_copy);
    let (a_data, a_off, a_s0, a_s1) = if a_blas {
        (a.data, a.off, a.s0, a.s1)
    } else {
        a_copy = copy_c(a);
        (&a_copy[..], 0, n, 1)
    };
    let (b_data, b_off, b_s0, b_s1) = if b_blas {
        (b.data, b.off, b.s0, b.s1)
    } else {
        b_copy = copy_c(b);
        (&b_copy[..], 0, p, 1)
    };
    // RowMajor: trans1 'N' (lda = is1_m) for C order, 'T' (lda = is1_n) otherwise.
    let (t1, lda) = if blasable(a_s0, a_s1, n) {
        (Trans::N, a_s0)
    } else {
        (Trans::T, a_s1)
    };
    let (t2, ldb) = if blasable(b_s0, b_s1, p) {
        (Trans::N, b_s0)
    } else {
        (Trans::T, b_s1)
    };
    // cblas_dgemm(RowMajor) → column-major: m' = p, n' = m, A' = B (trans2), B' = A (trans1).
    let a_col = Operand {
        data: b_data,
        off: b_off,
        ld: ldb,
        trans: t2,
    };
    let b_col = Operand {
        data: a_data,
        off: a_off,
        ld: lda,
        trans: t1,
    };
    dgemm_with(p, m, n, 1.0, a_col, b_col, 0.0, &mut out, 0, p, gemm_q);
    out
}
