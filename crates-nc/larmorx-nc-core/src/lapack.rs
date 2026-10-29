// SPDX-License-Identifier: LicenseRef-FSL AND BSD-3-Clause
// Translated from OpenBLAS 0.3.33 (BSD-3-Clause; Copyright (c) 2011-2014, The OpenBLAS Project)
// and the reference LAPACK 3.12.0 it bundles (lapack-netlib; modified BSD licence, Copyright (c)
// 1992-2023 The University of Tennessee and The University of Tennessee Research Foundation,
// 2000-2023 The University of California Berkeley, 2006-2023 The University of Colorado
// Denver). Both notices are in ../NOTICE. Part of larmorx-nc (FSL Licence terms; see LICENSE).
//! LAPACK's LU factorisation, inverse, triangular inverse and solves for matrices up to 4 × 4,
//! computed as FSL 6.0.7.23's LAPACK computes them.
//!
//! FSL 6.0.7.23's matrix layer is Armadillo 14.2.1 (fsl-armawrap 0.7.0), which sends a 4 × 4
//! `inv()` to LAPACK `dgetrf` + `dgetri` (`dtrtri` for a triangular matrix), every `inv(A) * B`
//! to `dgesv` and a 4 × 4 `det()` to `dgetrf` (`arma.rs`). FSL's `liblapack.so.3` is OpenBLAS
//! 0.3.33 (conda-forge, `DYNAMIC_ARCH`): `dgetrf`, `dgesv` and `dtrtri` are OpenBLAS's own
//! (`interface/lapack/`, `lapack/getf2`, `lapack/getrs`, `lapack/trti2`), `dgetri` is the
//! reference LAPACK's Fortran, which calls OpenBLAS's `dtrtri`, `dgemv` and `dswap`. OpenBLAS
//! picks its kernels by CPU at start-up (`OPENBLAS_CORETYPE` overrides it); this module follows
//! the **Haswell** kernels (`kernel/x86_64/KERNEL.HASWELL`; the Zen core uses the same), the
//! order the validation pins (`docs/findings/fsl-mcflirt.md`, "Armadillo 14.2.1"): their C
//! kernels were compiled with `-mavx2` but without `-mfma`, so their products and sums round
//! separately, and the hand-written kernels fuse where their assembly does (`dgemv_n`'s 4 × 2
//! kernel, the `dgemm` kernel's small tiles). The SkylakeX, Cooperlake and Sapphire Rapids cores
//! (AVX-512 CPUs) compile the same C kernels with `-march=skylake-avx512`, whose contractions
//! change the last bits (FSL-64).
//!
//! Every routine works on a column-major `n × n` array (`a[c * n + r]`, LAPACK's layout with
//! `lda = n`), `n` from 1 to 4, and follows the code path OpenBLAS takes for that size; NaNs are
//! SSE's (`sse.rs`), with each instruction's operand order. Pivots are 1-based, as LAPACK's.
//!
//! Upstream files (OpenBLAS 0.3.33 release tarball, SHA-256 `6761af1d9f5d…`):
//! `interface/lapack/getrf.c`, `gesv.c`, `trtri.c`, `interface/gemv.c`,
//! `lapack/getrf/getrf_single.c`, `lapack/getf2/getf2_k.c`, `lapack/getrs/getrs_single.c`,
//! `lapack/laswp/generic/laswp_k_4.c`, `lapack/trtri/trtri_U_single.c`, `trtri_L_single.c`,
//! `lapack/trti2/trti2_U.c`, `trti2_L.c`, `driver/level2/trmv_U.c`, `trmv_L.c`, `trsv_L.c`,
//! `trsv_U.c`, `driver/level3/trsm_L.c`, `kernel/generic/trsm_kernel_LT.c`, `trsm_kernel_LN.c`,
//! `trsm_ltcopy_4.c`, `trsm_utcopy_4.c`, `kernel/x86_64/ddot.c`, `daxpy.c`, `dscal.c`,
//! `dgemv_n_4.c`, `dgemv_n_microk_haswell-4.c`, `dgemm_kernel_4x8_haswell.S` (the 2 × N and
//! 1 × N tiles), `iamax_sse2.S`, `amax_sse2.S`, `param.h` (Haswell: `DGEMM_UNROLL_M` 4,
//! `DGEMM_UNROLL_N` 8); reference LAPACK 3.12.0: `SRC/dgetri.f`. The logic is re-expressed in
//! Rust; no code was copied.
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
//!
//! LAPACK: Copyright (c) 1992-2023 The University of Tennessee and The University of Tennessee
//! Research Foundation; 2000-2023 The University of California Berkeley; 2006-2023 The
//! University of Colorado Denver. All rights reserved. Redistribution and use in source and
//! binary forms, with or without modification, are permitted provided that the conditions of
//! its licence are met (the full text is in ../NOTICE).

use crate::sse::{DEFAULT_NAN_F64, add_f64, div_f64, mul_f64, quiet_f64, sub_f64};

/// `(row, col)` in a column-major `n × n` array.
#[inline(always)]
fn at(n: usize, r: usize, c: usize) -> usize {
    c * n + r
}

/// `vfmadd231sd dst, src2, src3` (`dst + src2 * src3`, one rounding): a NaN operand passed on
/// quieted, the multiplicands' before the addend's (`src2`, `src3`, then `dst`: measured on
/// FSL's `dgemv`, `validation/investigations/version-numerics/blas_nan_probe.py`); a NaN made
/// from numbers is the default NaN.
#[inline(always)]
fn fma231(dst: f64, src2: f64, src3: f64) -> f64 {
    let r = src2.mul_add(src3, dst);
    if !r.is_nan() {
        r
    } else if src2.is_nan() {
        quiet_f64(src2)
    } else if src3.is_nan() {
        quiet_f64(src3)
    } else if dst.is_nan() {
        quiet_f64(dst)
    } else {
        DEFAULT_NAN_F64
    }
}

/// x86's `maxsd`/`maxpd` (`d > s ? d : s`) and, for `USE_MIN`, `minsd`/`minpd` (`d < s ? d :
/// s`): `s` whenever either is NaN.
#[inline(always)]
fn max_x86(d: f64, s: f64) -> f64 {
    if d > s { d } else { s }
}

#[inline(always)]
fn min_x86(d: f64, s: f64) -> f64 {
    if d < s { d } else { s }
}

/// `IAMAX_K` (`iamax_sse2.S`, unit stride) for 1 to 4 elements: the 1-based index of the first
/// element of largest magnitude, or one past the elements it compared when none matches.
///
/// The kernel broadcasts `|x0|` to four accumulators, folds the other elements into them with
/// `maxpd` (`s` when either is NaN), reduces the lanes, then searches from the start with
/// `comisd`/`je`, which also matches an unordered (NaN) comparison; for 2 and 3 elements it
/// compares the first two and otherwise returns the next index unchecked (`getf2_k` clamps an
/// index past the end).
fn iamax(x: &[f64]) -> usize {
    let a: Vec<f64> = x.iter().map(|v| v.abs()).collect();
    let m = max_x86;
    let max = match a.len() {
        1 => a[0],
        2 => m(a[0], m(a[0], m(a[0], a[1]))),
        3 => {
            let lane = |k: usize| m(a[0], m(m(a[0], a[k]), a[0]));
            m(lane(2), lane(1))
        }
        4 => {
            let x3 = m(a[0], a[3]);
            let lane = |k: usize| m(a[0], m(m(a[0], a[k]), x3));
            m(lane(2), lane(1))
        }
        n => panic!("iamax is ported for 1 to 4 elements, not {n}"),
    };
    let hit = |v: f64| v == max || v.is_nan() || max.is_nan();
    let compared = if a.len() == 4 { 4 } else { a.len() & 2 };
    (0..compared).find(|&i| hit(a[i])).unwrap_or(compared) + 1
}

/// `AMIN_K == 0` then `IAMIN_K` on the diagonal (stride `lda + 1`), as `dtrtri` checks a
/// non-unit triangular matrix: `Some(1-based index)` when the smallest magnitude is zero
/// (`amax_sse2.S` and `iamax_sse2.S` built with `USE_MIN`, their strided paths).
fn zero_on_diagonal(d: &[f64]) -> Option<usize> {
    let a: Vec<f64> = d.iter().map(|v| v.abs()).collect();
    let n = a.len();
    let mn = min_x86;
    // Four accumulators of two lanes, each |x0|; the rest folded in by bit of the count left.
    let mut acc = [[a[0]; 2]; 4];
    let rest = n - 1;
    let mut k = 1;
    // (8 and 4 never occur for n <= 4.)
    if rest & 2 != 0 {
        acc[2] = [mn(acc[2][0], a[k]), mn(acc[2][1], a[k + 1])];
        k += 2;
    }
    let reduce = |acc: &mut [[f64; 2]; 4]| {
        acc[0] = [mn(acc[0][0], acc[1][0]), mn(acc[0][1], acc[1][1])];
        acc[2] = [mn(acc[2][0], acc[3][0]), mn(acc[2][1], acc[3][1])];
        acc[0] = [mn(acc[0][0], acc[2][0]), mn(acc[0][1], acc[2][1])];
        acc[1] = acc[0];
        acc[0] = [mn(acc[0][1], acc[1][0]), acc[0][1]];
    };
    if rest & 1 != 0 {
        acc[3] = [mn(acc[3][0], a[k]), mn(acc[3][1], a[k])];
    }
    // AMIN_K (amax_sse2.S): one reduction of the lanes at its end (none for one element).
    let amin = if n == 1 {
        a[0]
    } else {
        let mut r = acc;
        reduce(&mut r);
        r[0][0]
    };
    if amin != 0.0 {
        return None;
    }
    // IAMIN_K (iamax_sse2.S's strided path) reduces inside its branch for an odd count of the
    // elements after the first, then once more before its search (`comisd`/`je`).
    let mut r = acc;
    if rest & 1 != 0 {
        reduce(&mut r);
    }
    reduce(&mut r);
    let min = r[0][0];
    let hit = |v: f64| v == min || v.is_nan() || min.is_nan();
    let index = match n {
        1 => 1,
        2 | 3 => (0..2).find(|&i| hit(a[i])).map_or(3, |i| i + 1),
        _ => (0..4).find(|&i| hit(a[i])).map_or(5, |i| i + 1),
    };
    Some(index)
}

/// `DGEMV_N` (`dgemv_n_4.c`, Haswell, unit strides): `y += alpha · A x` for an `rows × cols`
/// block of a column-major array with leading dimension `lda` starting at `a0`.
///
/// Four rows run the block path: `dgemv_n_microk_haswell-4.c`'s 4 × 2 kernel for a pair of
/// columns (`y = fma(alpha, x1·a1 + x0·a0, y)`) and the 4 × 1 kernel for one (`y + a·(x·alpha)`);
/// fewer rows run the scalar tails (`t = Σ a·x` from 0, then `y + alpha·t`). Only the sizes LU
/// and inverse of a 4 × 4 matrix reach are ported (four rows with up to three columns).
#[allow(clippy::too_many_arguments)]
fn gemv_n(
    rows: usize,
    cols: usize,
    alpha: f64,
    a: &[f64],
    a0: usize,
    lda: usize,
    x: &[f64],
    y: &mut [f64],
) {
    if rows < 1 || cols < 1 {
        return;
    }
    let col = |k: usize, r: usize| a[a0 + k * lda + r];
    if rows == 4 {
        assert!(cols < 4, "dgemv_n's 4 x 4 kernel is not ported");
        let mut k = 0;
        if cols & 2 != 0 {
            for (r, yr) in y.iter_mut().enumerate().take(4) {
                let p0 = mul_f64(x[0], col(0, r));
                let p1 = mul_f64(x[1], col(1, r));
                let s = add_f64(p1, p0);
                *yr = fma231(*yr, s, alpha);
            }
            k = 2;
        }
        if cols & 1 != 0 {
            let xa = mul_f64(x[k], alpha);
            for (r, yr) in y.iter_mut().enumerate().take(4) {
                *yr = add_f64(*yr, mul_f64(col(k, r), xa));
            }
        }
        return;
    }
    assert!(rows < 4, "dgemv_n is ported for up to 4 rows");
    // Which operand's NaN each instruction passes on is GCC's choice per tail (measured on
    // FSL's `dgemv` with NaNs of distinct payloads, `blas_nan_probe.py`; the same for every row
    // of a tail): the one-row tail multiplies `a · x`, the two- and three-row tails `x · a`,
    // except `dgemv_n_4.c`'s own branch for three rows with `lda == 3`, which multiplies `a · x`
    // and puts the second column's term before the running sum when there are two columns; the
    // other sums put the running sum first.
    let special3 = rows == 3 && lda == 3 && cols >= 2;
    let x_first = rows >= 2 && !special3;
    for (r, yr) in y.iter_mut().enumerate().take(rows) {
        let mut t = 0.0;
        for (k, &xk) in x.iter().enumerate().take(cols) {
            let p = if x_first {
                mul_f64(xk, col(k, r))
            } else {
                mul_f64(col(k, r), xk)
            };
            t = if special3 && k == 1 {
                add_f64(p, t)
            } else {
                add_f64(t, p)
            };
        }
        // `y_ptr[0] += alpha * temp`: GCC adds y to the product (`vaddsd (y), prod, prod`).
        *yr = add_f64(mul_f64(t, alpha), *yr);
    }
}

/// `DSCAL_K` (`dscal.c`, the internal call: `dummy2 = 0`): `x = 0` when `da == 0`, else
/// `da · x` (GCC's operand order: `da`'s NaN first).
fn scal(da: f64, x: &mut [f64]) {
    for v in x {
        *v = if da == 0.0 { 0.0 } else { mul_f64(da, *v) };
    }
}

/// `DAXPY_K` (`daxpy.c`, fewer than 16 elements): `da·x + y` (GCC adds `y` to the product:
/// the product's NaN first).
fn axpy(da: f64, x: &[f64], y: &mut [f64]) {
    for (yv, &xv) in y.iter_mut().zip(x) {
        *yv = add_f64(mul_f64(da, xv), *yv);
    }
}

/// `dgetrf` (`interface/lapack/getrf.c`; one thread; `getrf_single`, whose blocking for these
/// sizes is at most `2 × DGEMM_UNROLL_N`, goes to `getf2_k`): the LU factorisation with partial
/// pivoting of a column-major `n × n` matrix, in place. Fills `ipiv` (1-based) and returns
/// LAPACK's `info` (0, or the first column with an exactly zero pivot).
///
/// `getf2_k` is left-looking: for each column, the earlier interchanges, the U part by strided
/// `ddot` (products added from 0.0 one at a time, then `+ 0.0`), the rest by `dgemv_n`'s tails,
/// the pivot by `idamax`, then (for a pivot of magnitude at least `DBL_MIN`) the row interchange
/// and the column scaled by the pivot's reciprocal; a NaN or subnormal pivot is recorded but
/// neither swapped nor divided by.
// The loops keep getf2_k's indices, so the port can be checked line by line.
#[allow(clippy::needless_range_loop)]
pub fn getrf(a: &mut [f64], n: usize, ipiv: &mut [i32]) -> i32 {
    assert!((1..=4).contains(&n) && a.len() >= n * n && ipiv.len() >= n);
    let m = n;
    let lda = n;
    let mut info = 0;
    for j in 0..n {
        let b0 = j * lda;
        let len = j.min(m);
        for i in 0..len {
            let ip = (ipiv[i] - 1) as usize;
            if ip != i {
                a.swap(b0 + i, b0 + ip);
            }
        }
        for i in 1..len {
            // DOTU_K(i, a + i, lda, b, 1): y = b (unit), x = row i (stride lda).
            let mut t1 = 0.0;
            for k in 0..i {
                t1 = add_f64(t1, mul_f64(a[b0 + k], a[at(n, i, k)]));
            }
            let dot = add_f64(t1, 0.0);
            a[b0 + i] = sub_f64(a[b0 + i], dot);
        }
        if j < m {
            // GEMV_N(m - j, j, 0, -1, a + j, lda, b, 1, b + j, 1)
            let x: Vec<f64> = a[b0..b0 + j].to_vec();
            let mut y: Vec<f64> = a[b0 + j..b0 + m].to_vec();
            gemv_n(m - j, j, -1.0, a, j, lda, &x, &mut y);
            a[b0 + j..b0 + m].copy_from_slice(&y);
            let mut jp = j + iamax(&a[b0 + j..b0 + m]);
            if jp > m {
                jp = m;
            }
            ipiv[j] = jp as i32;
            let jp = jp - 1;
            let temp1 = a[b0 + jp];
            if temp1 != 0.0 {
                if temp1.abs() >= f64::MIN_POSITIVE {
                    let r = div_f64(1.0, temp1);
                    if jp != j {
                        // SWAP_K(j + 1, ..., a + j, lda, a + jp, lda)
                        for c in 0..=j {
                            a.swap(at(n, j, c), at(n, jp, c));
                        }
                    }
                    if j + 1 < m {
                        scal(r, &mut a[b0 + j + 1..b0 + m]);
                    }
                }
            } else if info == 0 {
                info = (j + 1) as i32;
            }
        }
    }
    info
}

/// The packed values of the factor a triangular solve kernel reads: the off-diagonal entries as
/// they are, the diagonal as its reciprocal (`INV(a) = ONE / a`) or `ONE` for a unit diagonal
/// (`trsm_ltcopy_4.c`, `trsm_utcopy_4.c`).
#[inline(always)]
fn packed_diag(a: &[f64], n: usize, i: usize, unit: bool) -> f64 {
    if unit {
        1.0
    } else {
        div_f64(1.0, a[at(n, i, i)])
    }
}

/// One small tile of the Haswell `dgemm` kernel inside the triangular solves
/// (`dgemm_kernel_4x8_haswell.S`, `KERNEL{1,2}x{1,2,4}_SUB` and `SAVE`, alpha = -1): for each
/// row `r` of `rows`, `acc` folded from +0 with `vfmadd231sd acc, b, a` over `ks`, then
/// `c = (acc · alpha) + c`.
fn gemm_tile(c: &mut [f64], rows: &[usize], ks: &[usize], a: &[f64], n: usize, x: &[f64]) {
    for &r in rows {
        let mut acc = 0.0;
        for &k in ks {
            acc = fma231(acc, x[k], a[at(n, r, k)]);
        }
        c[r] = add_f64(mul_f64(acc, -1.0), c[r]);
    }
}

/// `TRSM_LNLU` for one right-hand side column `c` (`trsm_L.c` and the generic `trsm_kernel_LT`
/// with Haswell's `DGEMM_UNROLL_M` 4): forward substitution with a unit lower triangle, in blocks
/// of 4 rows, then 2, then 1, the rows above a block subtracted by the `dgemm` kernel.
fn trsm_lt_unit(a: &[f64], n: usize, c: &mut [f64]) {
    let mut kk = 0;
    let mut blocks = vec![4; n >> 2];
    for size in [2, 1] {
        if n & 3 & size != 0 {
            blocks.push(size);
        }
    }
    for size in blocks {
        let rows: Vec<usize> = (kk..kk + size).collect();
        if kk > 0 {
            let ks: Vec<usize> = (0..kk).collect();
            let solved = c.to_vec();
            gemm_tile(c, &rows, &ks, a, n, &solved);
        }
        // solve(size, ...): bb = c · diag; the rows below in the block: c -= bb · a.
        for i in kk..kk + size {
            let bb = mul_f64(c[i], packed_diag(a, n, i, true));
            c[i] = bb;
            for k in i + 1..kk + size {
                c[k] = sub_f64(c[k], mul_f64(bb, a[at(n, k, i)]));
            }
        }
        kk += size;
    }
}

/// `TRSM_LNUN` for one column (`trsm_kernel_LN`): back substitution with a non-unit upper
/// triangle, multiplying by the reciprocals of the pivots; the remainder blocks of 1 and 2 rows
/// at the bottom first, then blocks of 4.
fn trsm_ln_nonunit(a: &[f64], n: usize, c: &mut [f64]) {
    let mut kk = n;
    let mut blocks = Vec::new();
    for size in [1, 2] {
        if n & 3 & size != 0 {
            let start = (n & !(size - 1)) - size;
            blocks.push((start, size));
        }
    }
    let full = n >> 2;
    for b in 0..full {
        let start = (n & !3) - 4 * (b + 1);
        blocks.push((start, 4));
    }
    for (start, size) in blocks {
        let rows: Vec<usize> = (start..start + size).collect();
        if n > kk {
            let ks: Vec<usize> = (kk..n).collect();
            let solved = c.to_vec();
            gemm_tile(c, &rows, &ks, a, n, &solved);
        }
        for i in (start..start + size).rev() {
            // GCC multiplies the packed reciprocal by the value (its NaN first; measured).
            let bb = mul_f64(packed_diag(a, n, i, false), c[i]);
            c[i] = bb;
            for k in start..i {
                c[k] = sub_f64(c[k], mul_f64(bb, a[at(n, k, i)]));
            }
        }
        kk -= size;
    }
}

/// `TRSV_NLU` (`trsv_L.c`, one block): forward substitution of one column with `daxpy`:
/// `x[k] += (-x[i]) · L[k][i]`.
fn trsv_l_unit(a: &[f64], n: usize, x: &mut [f64]) {
    for i in 0..n {
        if i + 1 < n {
            let da = -x[i];
            let col: Vec<f64> = (i + 1..n).map(|k| a[at(n, k, i)]).collect();
            axpy(da, &col, &mut x[i + 1..n]);
        }
    }
}

/// `TRSV_NUN` (`trsv_U.c`, one block): back substitution of one column dividing by the pivots,
/// then `daxpy` with `-x[r]` on the rows above.
fn trsv_u_nonunit(a: &[f64], n: usize, x: &mut [f64]) {
    for i in 0..n {
        let r = n - 1 - i;
        x[r] = div_f64(x[r], a[at(n, r, r)]);
        if r > 0 {
            let da = -x[r];
            let col: Vec<f64> = (0..r).map(|k| a[at(n, k, r)]).collect();
            axpy(da, &col, &mut x[0..r]);
        }
    }
}

/// `dgetrs` with no transpose as `getrs_single` computes it after `dgesv`'s factorisation: the
/// row interchanges on `b` (`laswp_plus`), then for one right-hand side `trsv_NLU` and
/// `trsv_NUN`, for several `trsm_LNLU` and `trsm_LNUN` (each column independently: the kernels'
/// column blocking does not change a column's arithmetic).
pub fn getrs(lu: &[f64], n: usize, ipiv: &[i32], b: &mut [f64], nrhs: usize) {
    assert!((1..=4).contains(&n) && lu.len() >= n * n && b.len() >= n * nrhs);
    for col in b.chunks_mut(n).take(nrhs) {
        for i in 0..n {
            let ip = (ipiv[i] - 1) as usize;
            if ip != i {
                col.swap(i, ip);
            }
        }
    }
    for col in b.chunks_mut(n).take(nrhs) {
        if nrhs == 1 {
            trsv_l_unit(lu, n, col);
            trsv_u_nonunit(lu, n, col);
        } else {
            trsm_lt_unit(lu, n, col);
            trsm_ln_nonunit(lu, n, col);
        }
    }
}

/// `dgesv` (`interface/lapack/gesv.c`, one thread): [`getrf`] of `a` in place, then (when it
/// succeeded) [`getrs`] of `b`. Returns `info`.
pub fn gesv(a: &mut [f64], n: usize, ipiv: &mut [i32], b: &mut [f64], nrhs: usize) -> i32 {
    let info = getrf(a, n, ipiv);
    if info == 0 && nrhs > 0 {
        getrs(a, n, ipiv, b, nrhs);
    }
    info
}

/// `TRMV_NUN` (`trmv_U.c`, one block) on the column `x` of length `m`: `x = U x` for the
/// leading `m × m` upper triangle, with `daxpy` (`x[0..i] += x[i] · U[0..i][i]`) and the
/// diagonal product.
fn trmv_u(a: &[f64], n: usize, m: usize, x: &mut [f64]) {
    for i in 0..m {
        if i > 0 {
            let col: Vec<f64> = (0..i).map(|k| a[at(n, k, i)]).collect();
            let xi = x[i];
            axpy(xi, &col, &mut x[0..i]);
        }
        x[i] = mul_f64(x[i], a[at(n, i, i)]);
    }
}

/// `TRMV_NLN` (`trmv_L.c`, one block) on the column `x` of the trailing `m × m` lower triangle
/// starting at `(s, s)`: from the bottom, `x[r+1..] += x[r] · L[r+1..][r]`, then the diagonal.
fn trmv_l(a: &[f64], n: usize, s: usize, m: usize, x: &mut [f64]) {
    for i in 0..m {
        let r = m - i - 1;
        if i > 0 {
            let col: Vec<f64> = (r + 1..m).map(|k| a[at(n, s + k, s + r)]).collect();
            let xr = x[r];
            axpy(xr, &col, &mut x[r + 1..m]);
        }
        x[r] = mul_f64(x[r], a[at(n, s + r, s + r)]);
    }
}

/// `dtrtri` with a non-unit diagonal (`interface/lapack/trtri.c`): `info` from a zero on the
/// diagonal (`AMIN_K`, `IAMIN_K`), otherwise the inverse in place by `trti2_U` or `trti2_L`
/// (`trtri_*_single` for at most `GEMM_Q` rows): for each column, the diagonal's reciprocal
/// (`1 / a`), the column times the inverted part (`TRMV`), then scaled by minus the reciprocal.
/// Only the triangle named is read and written.
pub fn trtri(a: &mut [f64], n: usize, upper: bool) -> i32 {
    assert!((1..=4).contains(&n) && a.len() >= n * n);
    let diag: Vec<f64> = (0..n).map(|i| a[at(n, i, i)]).collect();
    if let Some(i) = zero_on_diagonal(&diag) {
        return i as i32;
    }
    if upper {
        for j in 0..n {
            let ajj = div_f64(1.0, a[at(n, j, j)]);
            a[at(n, j, j)] = ajj;
            let mut x: Vec<f64> = (0..j).map(|k| a[at(n, k, j)]).collect();
            trmv_u(a, n, j, &mut x);
            scal(-ajj, &mut x);
            for (k, v) in x.into_iter().enumerate() {
                a[at(n, k, j)] = v;
            }
        }
    } else {
        for j in (0..n).rev() {
            let ajj = div_f64(1.0, a[at(n, j, j)]);
            a[at(n, j, j)] = ajj;
            let m = n - j - 1;
            let mut x: Vec<f64> = (j + 1..n).map(|k| a[at(n, k, j)]).collect();
            trmv_l(a, n, j + 1, m, &mut x);
            scal(-ajj, &mut x);
            for (k, v) in x.into_iter().enumerate() {
                a[at(n, j + 1 + k, j)] = v;
            }
        }
    }
    0
}

/// `dgetri` (reference LAPACK 3.12.0, `SRC/dgetri.f`) after [`getrf`], with the work space
/// Armadillo gives it (16 elements: the unblocked code for these sizes): `dtrtri` of U, then
/// from the last column to the first, the part of L below the diagonal moved out and the column
/// updated by `dgemv` (`y = y - A[:, j+1..] w`, OpenBLAS's interface with beta = 1), then the
/// column interchanges (`dswap`) in reverse order. Returns `info`.
pub fn getri(a: &mut [f64], n: usize, ipiv: &[i32]) -> i32 {
    assert!((1..=4).contains(&n) && a.len() >= n * n);
    let info = trtri(a, n, true);
    if info > 0 {
        return info;
    }
    let mut work = [0.0f64; 4];
    for j in (0..n).rev() {
        for i in j + 1..n {
            work[i] = a[at(n, i, j)];
            a[at(n, i, j)] = 0.0;
        }
        if j + 1 < n {
            let x: Vec<f64> = work[j + 1..n].to_vec();
            let mut y: Vec<f64> = a[at(n, 0, j)..at(n, 0, j) + n].to_vec();
            gemv_n(n, n - j - 1, -1.0, a, at(n, 0, j + 1), n, &x, &mut y);
            a[at(n, 0, j)..at(n, 0, j) + n].copy_from_slice(&y);
        }
    }
    for j in (0..n.saturating_sub(1)).rev() {
        let jp = (ipiv[j] - 1) as usize;
        if jp != j {
            for r in 0..n {
                a.swap(at(n, r, j), at(n, r, jp));
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eye(n: usize) -> Vec<f64> {
        (0..n * n)
            .map(|i| if i % (n + 1) == 0 { 1.0 } else { 0.0 })
            .collect()
    }

    fn matmul(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
        let mut out = vec![0.0; n * n];
        for c in 0..n {
            for r in 0..n {
                out[at(n, r, c)] = (0..n).map(|k| a[at(n, r, k)] * b[at(n, k, c)]).sum();
            }
        }
        out
    }

    const A: [f64; 16] = [
        0.98, -0.06, 0.03, 0.0, 0.07, 1.01, -0.04, 0.0, -0.02, 0.05, 0.99, 0.0, 2.5, -1.75, 3.0,
        1.0,
    ];

    #[test]
    fn inverse_by_getrf_and_getri() {
        let mut lu = A.to_vec();
        let mut ipiv = [0i32; 4];
        assert_eq!(getrf(&mut lu, 4, &mut ipiv), 0);
        assert_eq!(getri(&mut lu, 4, &ipiv), 0);
        let id = matmul(&A, &lu, 4);
        for (x, y) in id.iter().zip(eye(4)) {
            assert!((x - y).abs() < 1e-14);
        }
    }

    #[test]
    fn solves_and_triangular_inverses() {
        for n in 1..=4 {
            let m: Vec<f64> = (0..n * n)
                .map(|i| {
                    if i % (n + 1) == 0 {
                        2.0 + i as f64
                    } else {
                        0.25 * (i % 3) as f64 - 0.2
                    }
                })
                .collect();
            for nrhs in 1..=n {
                let mut a = m.clone();
                let mut b: Vec<f64> = (0..n * nrhs).map(|i| i as f64 - 1.5).collect();
                let b0 = b.clone();
                let mut ipiv = [0i32; 4];
                assert_eq!(gesv(&mut a, n, &mut ipiv, &mut b, nrhs), 0);
                for c in 0..nrhs {
                    for r in 0..n {
                        let s: f64 = (0..n).map(|k| m[at(n, r, k)] * b[c * n + k]).sum();
                        assert!((s - b0[c * n + r]).abs() < 1e-12, "n {n} nrhs {nrhs}");
                    }
                }
            }
            for upper in [true, false] {
                let mut t = m.clone();
                for c in 0..n {
                    for r in 0..n {
                        if (upper && r > c) || (!upper && r < c) {
                            t[at(n, r, c)] = 0.0;
                        }
                    }
                }
                let mut ti = t.clone();
                assert_eq!(trtri(&mut ti, n, upper), 0);
                let id = matmul(&t, &ti, n);
                for (x, y) in id.iter().zip(eye(n)) {
                    assert!((x - y).abs() < 1e-14);
                }
            }
        }
    }

    #[test]
    fn singular_and_zero_diagonals() {
        let mut s = A.to_vec();
        for r in 0..4 {
            s[at(4, r, 1)] = s[at(4, r, 0)];
        }
        let mut ipiv = [0i32; 4];
        assert_eq!(getrf(&mut s, 4, &mut ipiv), 2);
        let mut t = eye(4);
        t[at(4, 2, 2)] = 0.0;
        assert_eq!(trtri(&mut t, 4, true), 3);
        let mut t = eye(3);
        t[at(3, 0, 0)] = -0.0;
        assert_eq!(trtri(&mut t, 3, false), 1);
    }
}
