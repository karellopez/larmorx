// SPDX-License-Identifier: LicenseRef-FSL AND BSD-3-Clause
// Translated from OpenBLAS 0.3.33 (BSD-3-Clause; Copyright (c) 2011-2014, The OpenBLAS Project;
// the notice is in ../lapack.rs and ../../NOTICE). Part of larmorx-nc (FSL Licence terms; see
// LICENSE).
//! OpenBLAS's LU factorisation and solve for matrices of any size up to `GEMM_Q` (256) rows, one
//! thread, on the **Haswell** kernels: `dgetrf` (`lapack/getrf/getrf_single.c`: recursive
//! panels of `blocking = ⌈mn/2⌉₈` columns (at most `GEMM_Q`), `getf2_k` once a panel is at most
//! 16 columns wide; the trailing columns get the panel's interchanges, its unit lower triangle
//! solved by `trsm_kernel_LT`, and the rows below updated by the GEMM kernel; the later
//! interchanges applied to the earlier columns at the end), `dgetrs` for several right-hand
//! sides (`lapack/getrs/getrs_single.c`: the interchanges, then `TRSM_LNLU` and `TRSM_LNUN`,
//! `driver/level3/trsm_L.c`, which for up to `GEMM_Q` rows is one `trsm_kernel_LT` and one
//! `trsm_kernel_LN` over all the columns) and `dgesv` (`interface/lapack/gesv.c`: one thread
//! below 100 × 100, and FSL's programs run OpenBLAS on one thread anyway).
//!
//! The arithmetic of the pieces: `getf2_k` as in [`super::getrf`] (strided `ddot`, `dgemv_n`'s
//! kernel with `alpha = −1`, `IAMAX`, the column scaled by the pivot's reciprocal; a subnormal
//! pivot neither swapped nor divided by); the GEMM kernel (`dgemm_kernel_4x8_haswell.S`) sums
//! each element's products in the order of the inner index (the first by `vmulpd`, the rest by
//! `vfmadd231pd`) and adds `acc·α` to the element (two roundings, `α = −1`); the triangular
//! kernels (`kernel/generic/trsm_kernel_LT.c`, `trsm_kernel_LN.c`, compiled without FMA) take the
//! rows in blocks of `GEMM_UNROLL_M` (4: from the top for `LT`, then a block of 2 and one of 1
//! for the remainder; for `LN` the remainder's blocks of 1 and 2 at the bottom first, then blocks
//! of 4 upwards): the solved rows outside the block by the GEMM kernel, then inside it each
//! row multiplied by the packed diagonal (1 for a unit triangle, `1/u_ii` for `LN`) and
//! subtracted (`c −= x·l`, two roundings) from the block's later (`LT`) or earlier (`LN`) rows.
//! Packing and the column blocking do not change any value and are not modelled. NaN payloads
//! are not modelled (Armadillo refuses non-finite matrices only in some paths; FSL's callers
//! here never pass them).

use super::blas::{MatRef, ddot, dgemv_n};

const GEMM_UNROLL_M: usize = 4;
const GEMM_UNROLL_N: usize = 8;
const GEMM_Q: usize = 256;

/// `IAMAX_K` with unit stride: the 1-based index of the first element of largest magnitude (the
/// kernel's short paths, with their quirks, for up to 8 elements: [`super::iamax`]).
fn iamax_any(x: &[f64]) -> usize {
    if x.len() <= 8 {
        return super::iamax(x);
    }
    let mut best = 0usize;
    let mut bv = x[0].abs();
    for (i, v) in x.iter().enumerate().skip(1) {
        if v.abs() > bv {
            bv = v.abs();
            best = i;
        }
    }
    best + 1
}

/// `LASWP_PLUS`: the interchanges `k1..=k2` (1-based rows, `ipiv` 1-based) applied in order to
/// the columns `cols` of the column-major array `a` (leading dimension `lda`).
fn laswp_plus(
    a: &mut [f64],
    lda: usize,
    cols: std::ops::Range<usize>,
    k1: usize,
    k2: usize,
    ipiv: &[i32],
) {
    for c in cols {
        for i in k1..=k2 {
            let ip = ipiv[i - 1] as usize;
            if ip != i {
                a.swap(c * lda + i - 1, c * lda + ip - 1);
            }
        }
    }
}

/// How `dgemm_kernel_4x8_haswell.S` sums one element's products, by the element's tile: a
/// 4-row tile of a 12-, 8- or 4-column tile with at least 8 products starts from the first
/// product (`vmulpd`) and adds the rest by fused multiply-adds in order; a 4-row tile of the
/// single-column tile keeps four partial sums (the products of each group of 8 by their index
/// modulo 4, the last `k mod 8` into the first), added as `(s0 + s1) + (s2 + s3)`; every other
/// tile adds all its products by fused multiply-adds onto +0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KSum {
    First,
    Zero,
    Four,
}

/// The width of the column tile of column `c` in a kernel call of `n` columns: 12 for the
/// columns in blocks of 24, then 8 for blocks of 8, then 4, 2 and 1 by the bits of the rest.
pub(crate) fn col_tile(c: usize, n: usize) -> usize {
    let n24 = n / 24 * 24;
    if c < n24 {
        return 12;
    }
    let rem = n - n24;
    let c = c - n24;
    let n8 = rem / 8 * 8;
    if c < n8 {
        return 8;
    }
    let r = rem - n8;
    let mut c = c - n8;
    for w in [4usize, 2, 1] {
        if r & w != 0 {
            if c < w {
                return w;
            }
            c -= w;
        }
    }
    1
}

/// The summation of the element `(r, c)` of a kernel call of `m × n` elements and `k` products.
pub(crate) fn tile_sum(r: usize, c: usize, m: usize, n: usize, k: usize) -> KSum {
    let row4 = r < (m & !3);
    let w = col_tile(c, n);
    if row4 && w == 1 {
        KSum::Four
    } else if row4 && w >= 4 && k >= 8 {
        KSum::First
    } else {
        KSum::Zero
    }
}

/// The sum of the `k` products `a(l)·b(l)` the way `kind` says.
#[inline]
pub(crate) fn ksum(kind: KSum, k: usize, ab: impl Fn(usize) -> (f64, f64)) -> f64 {
    match kind {
        KSum::First => {
            let (a0, b0) = ab(0);
            let mut acc = a0 * b0;
            for l in 1..k {
                let (a, b) = ab(l);
                acc = a.mul_add(b, acc);
            }
            acc
        }
        KSum::Zero => {
            let mut acc = 0.0f64;
            for l in 0..k {
                let (a, b) = ab(l);
                acc = a.mul_add(b, acc);
            }
            acc
        }
        KSum::Four => {
            let mut lane = [0.0f64; 4];
            let g = k / 8 * 8;
            for l in 0..g {
                let (a, b) = ab(l);
                lane[l % 4] = a.mul_add(b, lane[l % 4]);
            }
            for l in g..k {
                let (a, b) = ab(l);
                lane[0] = a.mul_add(b, lane[0]);
            }
            (lane[0] + lane[1]) + (lane[2] + lane[3])
        }
    }
}

/// The kernel's column blocks inside one call of `n` columns of a triangular kernel: blocks of
/// `GEMM_UNROLL_N` (8), then 4, 2 and 1 by the bits of the rest (as `(start, width)`).
fn trsm_col_blocks(n: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut c = 0;
    for _ in 0..n / GEMM_UNROLL_N {
        v.push((c, GEMM_UNROLL_N));
        c += GEMM_UNROLL_N;
    }
    let mut w = GEMM_UNROLL_N >> 1;
    while w > 0 {
        if n & w != 0 {
            v.push((c, w));
            c += w;
        }
        w >>= 1;
    }
    v
}

/// The summation inside a triangular kernel's GEMM call for a block of `rows` rows and
/// `width` columns over `k` solved rows.
fn trsm_ksum(rows: usize, width: usize, k: usize) -> KSum {
    tile_sum(0, 0, rows, width, k)
}

/// The row blocks of `trsm_kernel_LT` for `m` rows: 4 at a time from the top, then a block of
/// 2 and one of 1 for the remainder (as `(start, size)`).
fn lt_blocks(m: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut kk = 0;
    for _ in 0..m >> 2 {
        v.push((kk, GEMM_UNROLL_M));
        kk += GEMM_UNROLL_M;
    }
    if m & (GEMM_UNROLL_M - 1) != 0 {
        let mut i = GEMM_UNROLL_M >> 1;
        while i > 0 {
            if m & i != 0 {
                v.push((kk, i));
                kk += i;
            }
            i >>= 1;
        }
    }
    v
}

/// The row blocks of `trsm_kernel_LN` for `m` rows: the remainder's block of 1 and then of 2
/// at the bottom, then blocks of 4 upwards.
fn ln_blocks(m: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    if m & (GEMM_UNROLL_M - 1) != 0 {
        let mut i = 1;
        while i < GEMM_UNROLL_M {
            if m & i != 0 {
                v.push(((m & !(i - 1)) - i, i));
            }
            i *= 2;
        }
    }
    let full = m >> 2;
    for b in 0..full {
        v.push((
            (m & !(GEMM_UNROLL_M - 1)) - GEMM_UNROLL_M * (b + 1),
            GEMM_UNROLL_M,
        ));
    }
    v
}

/// `trsm_kernel_LT` (offset 0) with a unit lower triangle, called once per `(start, width)` of
/// `calls` (columns of `b`): `X = L⁻¹B` for the `m` rows of `B` from row `br` of `b`, `L` the
/// `m × m` block at `(lr, lr)` of `l` (`None`: of `b` itself, a disjoint region).
#[allow(clippy::too_many_arguments)]
fn trsm_lt_unit(
    l: Option<&[f64]>,
    b: &mut [f64],
    lda: usize,
    lr: usize,
    m: usize,
    ldb: usize,
    br: usize,
    calls: &[(usize, usize)],
) {
    let lat = |b: &[f64], r: usize, c: usize| match l {
        Some(l) => l[(lr + c) * lda + lr + r],
        None => b[(lr + c) * lda + lr + r],
    };
    for &(c0, cw) in calls {
        for (bc, width) in trsm_col_blocks(cw) {
            for (kk, size) in lt_blocks(m) {
                let kind = trsm_ksum(size, width, kk);
                for j in c0 + bc..c0 + bc + width {
                    let col = j * ldb + br;
                    if kk > 0 {
                        for r in kk..kk + size {
                            let acc = ksum(kind, kk, |q| (lat(b, r, q), b[col + q]));
                            b[col + r] += -acc;
                        }
                    }
                    for i in kk..kk + size {
                        // `bb *= aa` with the unit diagonal's 1.
                        let bb = b[col + i] * 1.0;
                        b[col + i] = bb;
                        for k in i + 1..kk + size {
                            let lv = lat(b, k, i);
                            b[col + k] -= bb * lv;
                        }
                    }
                }
            }
        }
    }
}

/// `trsm_kernel_LN` (offset 0) with a non-unit upper triangle, called once per `(start,
/// width)` of `calls`: `X = U⁻¹B` for the `m` rows of `b` from row 0, `U` the leading `m × m`
/// block of `u`; each row multiplied by the packed reciprocal `1/u_ii`.
fn trsm_ln_nonunit(
    u: &[f64],
    ldu: usize,
    m: usize,
    b: &mut [f64],
    ldb: usize,
    calls: &[(usize, usize)],
) {
    let uat = |r: usize, c: usize| u[c * ldu + r];
    for &(c0, cw) in calls {
        for (bc, width) in trsm_col_blocks(cw) {
            for (start, size) in ln_blocks(m) {
                let kk = start + size;
                let kind = trsm_ksum(size, width, m - kk);
                for j in c0 + bc..c0 + bc + width {
                    let col = j * ldb;
                    if m > kk {
                        for r in start..kk {
                            let acc = ksum(kind, m - kk, |q| (uat(r, kk + q), b[col + kk + q]));
                            b[col + r] += -acc;
                        }
                    }
                    for i in (start..kk).rev() {
                        let inv = 1.0 / uat(i, i);
                        let bb = inv * b[col + i];
                        b[col + i] = bb;
                        for k in start..i {
                            b[col + k] -= bb * uat(k, i);
                        }
                    }
                }
            }
        }
    }
}

/// `getf2_k` on the panel `range` (global rows and columns from `range.0`, columns up to
/// `range.1`) of the `m_all × n_all` column-major array `a` (`lda`), or the whole of it.
/// Returns `info` (local to the panel).
fn getf2(
    a: &mut [f64],
    lda: usize,
    m_all: usize,
    n_all: usize,
    ipiv: &mut [i32],
    range: Option<(usize, usize)>,
) -> i32 {
    let (m, n, off) = match range {
        None => (m_all as i64, n_all as i64, 0usize),
        Some((r0, r1)) => (m_all as i64 - r0 as i64, (r1 - r0) as i64, r0),
    };
    let mut info = 0i32;
    if m <= 0 || n <= 0 {
        // getf2_k's loops do nothing for an empty panel.
        return info;
    }
    let (m, n) = (m as usize, n as usize);
    // Local (row, col) of the panel -> index in a.
    let ix = |r: usize, c: usize| (off + c) * lda + off + r;
    for j in 0..n {
        let len = j.min(m);
        for i in 0..len {
            let ip = (ipiv[i + off] - 1) as usize - off;
            if ip != i {
                a.swap(ix(i, j), ix(ip, j));
            }
        }
        for i in 1..len {
            let x: Vec<f64> = (0..i).map(|k| a[ix(i, k)]).collect();
            let y: Vec<f64> = (0..i).map(|k| a[ix(k, j)]).collect();
            let d = ddot(&x, lda, &y, 1);
            a[ix(i, j)] -= d;
        }
        if j < m {
            if j > 0 {
                let x: Vec<f64> = (0..j).map(|k| a[ix(k, j)]).collect();
                let mut y: Vec<f64> = (j..m).map(|r| a[ix(r, j)]).collect();
                let am = MatRef {
                    off: ix(j, 0),
                    ld: lda,
                };
                dgemv_n(m - j, j, -1.0, a, am, &x, 1, 1.0, &mut y);
                for (k, v) in y.into_iter().enumerate() {
                    a[ix(j + k, j)] = v;
                }
            }
            let col: Vec<f64> = (j..m).map(|r| a[ix(r, j)]).collect();
            let mut jp = j + iamax_any(&col);
            if jp > m {
                jp = m;
            }
            ipiv[j + off] = (jp + off) as i32;
            let jp = jp - 1;
            let temp1 = a[ix(jp, j)];
            if temp1 != 0.0 {
                if temp1.abs() >= f64::MIN_POSITIVE {
                    let t = 1.0 / temp1;
                    if jp != j {
                        for c in 0..=j {
                            a.swap(ix(j, c), ix(jp, c));
                        }
                    }
                    for r in j + 1..m {
                        a[ix(r, j)] *= t;
                    }
                }
            } else if info == 0 {
                info = j as i32 + 1;
            }
        }
    }
    info
}

/// `getrf_single` (recursive) on the panel `range` of the `m_all × n_all` array `a`, or the
/// whole of it. Returns `info`.
fn getrf_rec(
    a: &mut [f64],
    lda: usize,
    m_all: usize,
    n_all: usize,
    ipiv: &mut [i32],
    range: Option<(usize, usize)>,
) -> i32 {
    let (m, n, off) = match range {
        None => (m_all as i64, n_all as i64, 0usize),
        Some((r0, r1)) => (m_all as i64 - r0 as i64, (r1 - r0) as i64, r0),
    };
    if m <= 0 || n <= 0 {
        return 0;
    }
    let (m, n) = (m as usize, n as usize);
    let mn = m.min(n);
    let mut blocking = (mn / 2).div_ceil(GEMM_UNROLL_N) * GEMM_UNROLL_N;
    if blocking > GEMM_Q {
        blocking = GEMM_Q;
    }
    if blocking <= GEMM_UNROLL_N * 2 {
        return getf2(a, lda, m_all, n_all, ipiv, range);
    }
    let mut info = 0i32;
    let mut j = 0;
    while j < mn {
        let jb = (mn - j).min(blocking);
        let iinfo = getrf_rec(a, lda, m_all, n_all, ipiv, Some((off + j, off + j + jb)));
        if iinfo != 0 && info == 0 {
            info = iinfo + j as i32;
        }
        if j + jb < n {
            let js = j + jb;
            // The trailing columns in chunks of GEMM_UNROLL_N: the panel's interchanges, then
            // its unit lower triangle solved.
            let mut jjs = js;
            while jjs < n {
                let min_jj = (n - jjs).min(GEMM_UNROLL_N);
                laswp_plus(
                    a,
                    lda,
                    off + jjs..off + jjs + min_jj,
                    j + off + 1,
                    j + jb + off,
                    ipiv,
                );
                trsm_lt_unit(
                    None,
                    a,
                    lda,
                    off + j,
                    jb,
                    lda,
                    off + j,
                    &[(off + jjs, min_jj)],
                );
                jjs += min_jj;
            }
            // The rows below: C −= L₂₁·U₁₂, one GEMM kernel call of (m − j − jb) × (n − js).
            let (cm, cn) = (m - j - jb, n - js);
            for c in js..n {
                let gc = off + c;
                for r in j + jb..m {
                    let gr = off + r;
                    let kind = tile_sum(r - j - jb, c - js, cm, cn, jb);
                    let acc = ksum(kind, jb, |l| {
                        (a[(off + j + l) * lda + gr], a[gc * lda + off + j + l])
                    });
                    a[gc * lda + gr] += -acc;
                }
            }
        }
        j += jb;
    }
    let mut j = 0;
    while j < mn {
        let jb = (mn - j).min(blocking);
        if j + jb < mn {
            laswp_plus(
                a,
                lda,
                off + j..off + j + jb,
                j + jb + off + 1,
                mn + off,
                ipiv,
            );
        }
        j += jb;
    }
    info
}

/// `dgetrf` (`interface/lapack/getrf.c`, one thread) of the column-major `m × n` array `a`
/// (leading dimension `lda`), at most `GEMM_Q` columns and rows: the LU factors in place, the
/// 1-based pivots in `ipiv`. Returns `info` (0, or the first column with an exactly zero
/// pivot).
pub fn getrf_any(a: &mut [f64], m: usize, n: usize, lda: usize, ipiv: &mut [i32]) -> i32 {
    assert!(
        m <= GEMM_Q && n <= GEMM_Q,
        "getrf_any: OpenBLAS's blocking beyond {GEMM_Q} rows or columns is not ported"
    );
    if m == 0 || n == 0 {
        return 0;
    }
    getrf_rec(a, lda, m, n, ipiv, None)
}

/// `dgetrs('N')` for `nrhs` ≥ 2 right-hand sides (`getrs_N_single`): the interchanges, then
/// `X = U⁻¹L⁻¹B` by the triangular kernels, for `n` ≤ `GEMM_Q`.
pub fn getrs_any(
    lu: &[f64],
    n: usize,
    lda: usize,
    ipiv: &[i32],
    b: &mut [f64],
    ldb: usize,
    nrhs: usize,
) {
    assert!(
        n <= GEMM_Q,
        "getrs_any: more than {GEMM_Q} rows is not ported"
    );
    assert!(
        nrhs >= 2,
        "getrs_any: one right-hand side takes TRSV (not this path)"
    );
    laswp_plus(b, ldb, 0..nrhs, 1, n, ipiv);
    // trsm_L.c: the columns in chunks of 24 while that many remain, then of 8, then the rest.
    let mut calls = Vec::new();
    let mut jjs = 0;
    while jjs < nrhs {
        let rem = nrhs - jjs;
        let w = if rem >= 3 * GEMM_UNROLL_N {
            3 * GEMM_UNROLL_N
        } else if rem > GEMM_UNROLL_N {
            GEMM_UNROLL_N
        } else {
            rem
        };
        calls.push((jjs, w));
        jjs += w;
    }
    trsm_lt_unit(Some(lu), b, lda, 0, n, ldb, 0, &calls);
    trsm_ln_nonunit(lu, lda, n, b, ldb, &calls);
}

/// `dgesv` of the `n × n` matrix `a` (overwritten by its LU factors) and the `n × nrhs`
/// right-hand sides `b` (overwritten by the solution), `nrhs` ≥ 2 and `n` ≤ `GEMM_Q`. Returns
/// `info`.
pub fn gesv_any(a: &mut [f64], n: usize, ipiv: &mut [i32], b: &mut [f64], nrhs: usize) -> i32 {
    let info = getrf_any(a, n, n, n, ipiv);
    if info == 0 && nrhs > 0 {
        getrs_any(a, n, n, ipiv, b, n, nrhs);
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks() {
        assert_eq!(lt_blocks(7), vec![(0, 4), (4, 2), (6, 1)]);
        assert_eq!(ln_blocks(7), vec![(6, 1), (4, 2), (0, 4)]);
        assert_eq!(ln_blocks(9), vec![(8, 1), (4, 4), (0, 4)]);
    }

    #[test]
    fn solves_a_system() {
        let n = 40;
        let mut a = vec![0.0; n * n];
        for c in 0..n {
            for r in 0..n {
                a[c * n + r] =
                    ((r * 7 + c * 13) % 17) as f64 / 17.0 + if r == c { 3.0 } else { 0.0 };
            }
        }
        let a0 = a.clone();
        let mut b: Vec<f64> = (0..n * 3).map(|i| (i % 5) as f64 - 2.0).collect();
        let b0 = b.clone();
        let mut ipiv = vec![0i32; n];
        assert_eq!(gesv_any(&mut a, n, &mut ipiv, &mut b, 3), 0);
        for j in 0..3 {
            for r in 0..n {
                let s: f64 = (0..n).map(|c| a0[c * n + r] * b[j * n + c]).sum();
                assert!((s - b0[j * n + r]).abs() < 1e-9);
            }
        }
    }
}
