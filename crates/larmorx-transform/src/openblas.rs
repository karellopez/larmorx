// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy's 4×4 matrix products and inverses, computed in a fixed operation order: the order of
//! OpenBLAS 0.3.30's Haswell kernels (BSD-3-Clause; notice below and in NOTICE), the BLAS and
//! LAPACK that numpy 2.3.5's wheels bundle and run on x86-64 CPUs with AVX2 and FMA.
//!
//! nitransforms and fMRIPrep compute their 4×4 matrices with numpy (`np.linalg.inv`, `@`,
//! `.dot`), and numpy hands them to whatever BLAS and LAPACK it was built with, which pick their
//! kernels by CPU at run time. The last bits therefore differ between CPUs and platforms
//! (`docs/findings/numpy-blas.md`). This module gives every platform the bits of the Haswell
//! kernels, so larmorx's transforms are the same everywhere and bit-identical to fMRIPrep's on
//! an x86-64 machine with FMA. `f64::mul_add` is a correctly rounded fused multiply-add on every
//! target, so it reproduces the hardware's `vfmadd` exactly.
//!
//! - [`gemm`] / [`matmul4`]: `numpy.matmul` and `numpy.dot` of float64 matrices, which call
//!   `cblas_dgemm` (`numpy/_core/src/umath/matmul.c.src`, `common/cblasfuncs.c`). OpenBLAS's
//!   `dgemm_kernel_4x8_haswell.S` keeps one accumulator per output, cleared to +0 (`vxorpd`),
//!   and adds `a[i][k]·b[k][j]` for k = 0, 1, … with `vfmadd231pd` (for K ≥ 8 the first step
//!   is a `vmulpd`, which gives the same value once the last step has run); the result is then
//!   scaled by alpha = 1 and added to C, which `dgemm_beta` set to +0 because beta = 0. So
//!   each output is a fused multiply-add chain in index order, plus +0 (an exact zero is
//!   always +0).
//! - [`inv4`]: `numpy.linalg.inv`, which copies the matrix to column-major order, sets B = I
//!   and calls LAPACK `dgesv` (`numpy/linalg/umath_linalg.cpp`, `inv`). OpenBLAS replaces
//!   `dgesv` with its own (`interface/lapack/gesv.c`): one thread below 10,000 elements,
//!   `getrf_single`, which for this size goes straight to the unblocked `getf2_k`
//!   (`lapack/getrf/getrf_single.c`: blocking 8 ≤ 2 × `DGEMM_UNROLL_N`), then `getrs_single`
//!   (`lapack/getrs/getrs_single.c`): `laswp_plus`, `trsm_LNLU` and `trsm_LNUN` with the
//!   generic triangular-solve kernels (`kernel/generic/trsm_kernel_LT.c`, `_LN.c`), which
//!   multiply by reciprocals of the pivots that the packing routines compute
//!   (`trsm_ltcopy_4.c`, `trsm_utcopy_4.c`). The C kernels on that path (`ddot.c`,
//!   `dgemv_n_4.c`, `dscal.c`, the solves) were compiled with GCC 10 and `-mavx2` but not
//!   `-mfma`, so their multiplications and additions are rounded separately (checked in the
//!   disassembly of numpy's `libscipy_openblas64_`).
//!
//! Upstream files (OpenBLAS 0.3.30 release tarball, SHA-256 `27342cff5186…`):
//! `interface/gemm.c`, `driver/level3/level3.c`, `kernel/x86_64/dgemm_kernel_4x8_haswell.S`,
//! `kernel/x86_64/dgemm_beta_skylakex.c`, `interface/lapack/gesv.c`,
//! `lapack/getrf/getrf_single.c`, `lapack/getf2/getf2_k.c`, `lapack/getrs/getrs_single.c`,
//! `lapack/laswp/generic/laswp_k_4.c`, `driver/level3/trsm_L.c`,
//! `kernel/generic/trsm_kernel_LT.c`, `trsm_kernel_LN.c`, `trsm_ltcopy_4.c`,
//! `trsm_utcopy_4.c`, `kernel/x86_64/ddot.c`, `dgemv_n_4.c`, `dscal.c`, `iamax_sse2.S`
//! (`KERNEL.HASWELL` selects them). The logic is re-expressed in Rust; no code was copied.
//!
//! OpenBLAS: Copyright (c) 2011-2014, The OpenBLAS Project. All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice, this list of
//!    conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright notice, this list of
//!    conditions and the following disclaimer in the documentation and/or other materials
//!    provided with the distribution.
//! 3. Neither the name of the OpenBLAS project nor the names of its contributors may be used to
//!    endorse or promote products derived from this software without specific prior written
//!    permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS
//! OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
//! MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
//! COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
//! EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE
//! GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
//! ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
//! NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
//! OF THE POSSIBILITY OF SUCH DAMAGE.

use larmorx_core::linalg::Mat4;

/// One output of `dgemm`: `Σ a[k]·b[k]` as a fused multiply-add chain in index order, then
/// `alpha · acc + C` with alpha = 1 and C = +0 (beta = 0), which turns an exact -0 into +0.
#[inline(always)]
fn dot_fused(a: impl Iterator<Item = f64>, b: impl Iterator<Item = f64>) -> f64 {
    let mut acc = 0.0f64;
    for (x, y) in a.zip(b) {
        acc = x.mul_add(y, acc);
    }
    acc + 0.0
}

/// `a @ b` for an `m × k` matrix `a` and a `k × n` matrix `b` (row-major slices), as numpy
/// computes it with OpenBLAS's Haswell `dgemm` (module docs). Returns the `m × n` row-major
/// product. numpy uses `dsyrk` instead when `b` is `a.T` (the same memory); this is `dgemm`.
pub fn gemm(a: &[f64], b: &[f64], m: usize, k: usize, n: usize) -> Vec<f64> {
    assert_eq!(a.len(), m * k, "a must be m × k");
    assert_eq!(b.len(), k * n, "b must be k × n");
    let mut out = vec![0.0f64; m * n];
    for i in 0..m {
        for j in 0..n {
            out[i * n + j] = dot_fused(
                a[i * k..(i + 1) * k].iter().copied(),
                (0..k).map(|t| b[t * n + j]),
            );
        }
    }
    out
}

/// `a @ b` (or `a.dot(b)`) of two float64 4×4 matrices, as numpy computes it with OpenBLAS's
/// Haswell `dgemm`: `fma(a[i][3], b[3][j], fma(a[i][2], b[2][j], fma(a[i][1], b[1][j],
/// fma(a[i][0], b[0][j], +0)))) + 0`.
pub fn matmul4(a: &Mat4, b: &Mat4) -> Mat4 {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| dot_fused(a[i].iter().copied(), b.iter().map(|r| r[j])))
    })
}

/// `IAMAX_K` (`iamax_sse2.S`): the 0-based index of the first element of largest magnitude. The
/// kernel takes the maximum with `maxsd` (the second operand when either is NaN) and then
/// returns the first element equal to it; when none is (a NaN maximum), it returns the length,
/// which `getf2_k` clamps to the last element.
fn iamax(x: &[f64]) -> usize {
    let mut max = x[0].abs();
    for v in &x[1..] {
        let v = v.abs();
        // maxsd max, v: max > v ? max : v.
        max = if max > v { max } else { v };
    }
    x.iter().position(|v| v.abs() == max).unwrap_or(x.len() - 1)
}

/// `numpy.linalg.inv` of a 4×4 matrix as OpenBLAS 0.3.30 computes it (`dgesv` with B = I; module
/// docs), or `None` when LAPACK reports an exactly zero pivot (numpy raises "Singular
/// matrix").
pub fn inv4(m: &Mat4) -> Option<Mat4> {
    const N: usize = 4;
    // numpy's linearize_matrix: column-major, a[c][r] = m[r][c].
    let mut a: [[f64; N]; N] = std::array::from_fn(|c| std::array::from_fn(|r| m[r][c]));
    let mut ipiv = [0usize; N];
    if getf2(&mut a, &mut ipiv) != 0 {
        return None;
    }
    // getrs_single, B = I (column-major, b[c][r]).
    let mut b: [[f64; N]; N] =
        std::array::from_fn(|c| std::array::from_fn(|r| if r == c { 1.0 } else { 0.0 }));
    // laswp_plus: the row interchanges, in order.
    for (i, &ip) in ipiv.iter().enumerate() {
        if ip != i {
            for col in b.iter_mut() {
                col.swap(i, ip);
            }
        }
    }
    for col in b.iter_mut() {
        // trsm_LNLU (trsm_kernel_LT's solve): forward substitution, unit diagonal.
        for i in 0..N {
            let bb = col[i] * 1.0;
            col[i] = bb;
            for k in i + 1..N {
                col[k] -= bb * a[i][k];
            }
        }
        // trsm_LNUN (trsm_kernel_LN's solve): back substitution with the reciprocals of the
        // pivots computed by trsm_utcopy (INV(a) = ONE / a).
        for i in (0..N).rev() {
            let bb = col[i] * (1.0 / a[i][i]);
            col[i] = bb;
            for k in 0..i {
                col[k] -= bb * a[i][k];
            }
        }
    }
    // delinearize: out[r][c] = b[c][r].
    Some(std::array::from_fn(|r| std::array::from_fn(|c| b[c][r])))
}

/// `getf2_k` (OpenBLAS's left-looking LU with partial pivoting) on a column-major `N × N`
/// matrix, in place: `a[c][r]`. Fills `ipiv` (0-based) and returns LAPACK's `info` (0, or one
/// more than the first column with an exactly zero pivot).
// The loops keep getf2_k's indices, so the port can be checked line by line.
#[allow(clippy::needless_range_loop)]
fn getf2<const N: usize>(a: &mut [[f64; N]; N], ipiv: &mut [usize; N]) -> usize {
    let mut info = 0;
    for j in 0..N {
        // Apply the earlier interchanges to column j.
        for i in 0..j {
            let ip = ipiv[i];
            if ip != i {
                a[j].swap(i, ip);
            }
        }
        // U part of column j: b[i] -= DOTU_K(i, row i of L, b), the strided ddot: products
        // summed from 0.0 one at a time, then `temp2 + temp1` with temp2 = 0.0.
        for i in 1..j {
            let mut t = 0.0f64;
            for k in 0..i {
                t += a[j][k] * a[k][i];
            }
            let dot = 0.0 + t;
            a[j][i] -= dot;
        }
        // GEMV_N(m - j, j, alpha = -1, A[j.., 0..j], b[0..j], b[j..]): for n < 1 nothing; for
        // these sizes the scalar tail of dgemv_n_4.c: temp = Σ a·x from 0.0, y += alpha·temp.
        if j > 0 {
            for r in j..N {
                let mut t = 0.0f64;
                for k in 0..j {
                    t += a[k][r] * a[j][k];
                }
                a[j][r] += -t;
            }
        }
        let jp = j + iamax(&a[j][j..]);
        ipiv[j] = jp;
        let pivot = a[j][jp];
        if pivot != 0.0 {
            if pivot.abs() >= f64::MIN_POSITIVE {
                let r = 1.0 / pivot;
                if jp != j {
                    // SWAP_K(j + 1, ..., a + j, lda, a + jp, lda): rows j and jp, columns 0..=j.
                    for col in a.iter_mut().take(j + 1) {
                        col.swap(j, jp);
                    }
                }
                // SCAL_K(m - j - 1, r, b + j + 1): dscal sets x = 0 when r == 0.
                for v in a[j][j + 1..].iter_mut() {
                    *v = if r == 0.0 { 0.0 } else { *v * r };
                }
            }
        } else if info == 0 {
            info = j + 1;
        }
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Mat4 = [
        [0.98, 0.07, -0.02, 2.5],
        [-0.06, 1.01, 0.05, -1.75],
        [0.03, -0.04, 0.99, 3.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    fn close(a: &Mat4, b: &Mat4, tol: f64) -> bool {
        a.iter()
            .flatten()
            .zip(b.iter().flatten())
            .all(|(x, y)| (x - y).abs() <= tol)
    }

    fn eye() -> Mat4 {
        std::array::from_fn(|i| std::array::from_fn(|j| if i == j { 1.0 } else { 0.0 }))
    }

    #[test]
    fn products_are_fused_chains_without_negative_zeros() {
        let b = inv4(&A).unwrap();
        let p = matmul4(&A, &b);
        assert!(close(&p, &eye(), 1e-15));
        let expected = A[1][3].mul_add(
            b[3][2],
            A[1][2].mul_add(b[2][2], A[1][1].mul_add(b[1][2], A[1][0] * b[0][2])),
        );
        assert_eq!(p[1][2].to_bits(), (expected + 0.0).to_bits());
        // -1 · 0 is -0 in a plain product; dgemm stores +0.
        let lps: Mat4 = [
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let q = matmul4(&lps, &eye());
        assert!(
            q.iter()
                .flatten()
                .all(|v| v.to_bits() != (-0.0f64).to_bits())
        );
        let g = gemm(
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            &[1.0, 0.5, -1.0, 2.0, 0.25, 4.0],
            2,
            3,
            2,
        );
        assert_eq!(g, vec![-0.25, 16.5, 0.5, 36.0]);
    }

    #[test]
    fn inverse_inverts_and_pivots() {
        let b = inv4(&A).unwrap();
        assert!(close(&matmul4(&b, &A), &eye(), 1e-15));
        // A permutation needs pivoting; its inverse is its transpose, exactly.
        let p: Mat4 = [
            [0.0, 0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 0.0],
        ];
        let pi = inv4(&p).unwrap();
        for i in 0..4 {
            for j in 0..4 {
                assert_eq!(pi[i][j], p[j][i]);
            }
        }
        // Singular: an exactly zero pivot.
        let mut s = A;
        s[2] = s[0];
        s[2][3] = 2.5;
        assert!(inv4(&s).is_none());
        assert!(inv4(&[[0.0; 4]; 4]).is_none());
    }

    #[test]
    fn iamax_takes_the_first_maximum() {
        assert_eq!(iamax(&[1.0, -3.0, 3.0, 2.0]), 1);
        assert_eq!(iamax(&[0.0, -0.0]), 0);
        assert_eq!(iamax(&[2.0]), 0);
    }
}
