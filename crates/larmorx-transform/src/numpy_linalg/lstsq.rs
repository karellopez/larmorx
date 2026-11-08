// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! `numpy.linalg.lstsq(a, b, rcond)` for a float64 design of two columns (tedana's log-linear
//! T2*/S0 fit: `a = [1, -TE]` repeated per volume, one column of `b` per voxel), as numpy
//! 2.5.4 and 2.3.5 compute it on x86-64 with the OpenBLAS their wheels bundle, at one BLAS
//! thread, on every platform.
//!
//! numpy hands the whole batch to LAPACK's `DGELSD` in one call (`umath_linalg.cpp`'s
//! `lstsq`: `a` and `b` copied to Fortran order, `LDB = max(m, n)`, `rcond = eps · max(m, n)`
//! by default). For `n = 2` the call takes these paths, followed here operation for operation:
//!
//! - `DLANGE('M')` of `A` and of `B` (a NaN wins) and the scalings by `DLASCL` when a norm is
//!   outside `[SMLNUM, BIGNUM]` (`SMLNUM = 2^-1022 / 2^-52`); an all-zero `A` gives `x = 0`;
//! - `m >= MNTHR = 3`: `DGEQRF` → `DGEQR2` (two `DLARFG`, the first reflector applied to
//!   `A(:, 2)`), `DORMQR('L', 'T')` → `DORM2R` (both reflectors applied to all of `B`),
//!   `A(2, 1) = 0`, then `DGEBRD` → `DGEBD2` of the 2 × 2 `R` (every reflector is then the
//!   identity); `m = 2`: `DGEBD2` of `A` itself and its first reflector applied to `B`;
//! - `DLALSD('U', SMLSIZ = 25, 2, ...)`: `DLANST`, `DLASCL` of `D` and `E`, `DLASDQ` →
//!   `DBDSQR('U', 2, NCVT = 2, NRU = 0, NCC = NRHS)` with `C = B` (`DLASV2`, `DROT` on the rows
//!   of `VT` and `B`, the singular values made positive and sorted down), `DLASDQ`'s sort up,
//!   the tolerance `RCND · max|d|`, the rows of `B` scaled by `1/d` (`DLASCL`) or cleared,
//!   `DGEMM('T', 'N', 2, NRHS, 2)`, `DLACPY`, `DLASRT('D')` and the last `DLASCL`;
//! - the reflectors as LAPACK 3.12.1 applies them (`DLARF1F`: the leading 1 implicit; OpenBLAS
//!   0.3.34, numpy 2.5.4) or 3.12.0 (`DLARF`: the 1 stored; OpenBLAS 0.3.30, numpy 2.3.5), with
//!   their scans for the last nonzero entry of `v` and the last nonzero column of `B`
//!   (`ILADLC`), see [`LapackRelease`];
//! - OpenBLAS's Haswell kernels on these lengths, main loops included (OpenBLAS 0.3.30 and
//!   0.3.34 ship the same files; GCC 10 and 14 compiled them to the same arithmetic): `dgemv_t`
//!   (`dgemv_t_4.c`): the rows in blocks of 2048, then the last `m mod 4` rows; within a block,
//!   the columns in groups of four through `dgemv_t_microk_haswell-4.c` (four fused
//!   multiply-add lanes per column, one per row modulo 4, combined as
//!   `(l0 + l2) + (l1 + l3)`), then a pair through the SSE2 `dgemv_kernel_4x2` (two lanes, rows
//!   modulo 2, each product rounded), then a last column through `dgemv_kernel_4x1` (four
//!   lanes, products rounded, `(s0 + s2) + (s1 + s3)`); the last rows as GCC contracted the C
//!   (`fma(a0, x0, a1·x1)`, then `fma(a2, x2, ·)`, added to `y`; one row `fma(a, x, y)`). So
//!   a column's result depends on its place among the last `NRHS mod 4` columns, never on the
//!   other columns' values; `dger` (`generic/ger.c`: per column `AXPYU_K(m, alpha·y[j])`)
//!   with `daxpy.c`: the first `16·⌊m/16⌋` rows fused (`daxpy_microk_haswell-2.c`), the rest
//!   `y + round(a·x)`; `daxpy` with a stride (the C loop, each product rounded); `drot`'s
//!   strided loop (`x' = fma(c, x, s·y)`, `y' = fma(c, y, -(s·x))`); `dgemm` (the generic
//!   small-matrix permit returns 0, so the level-3 driver and `dgemm_kernel_4x8_haswell.S`'s
//!   `2×N` paths: per element `fma(a2, b2, fma(a1, b1, +0))`, times `alpha`, added to the
//!   cleared `C`); `dnrm2` (`nrm2.S`, x87, in the parent module);
//! - numpy 2.5.4's check that `a` is finite (`LinAlgError`; 2.3.5 has none) and its residuals
//!   (`Σ b_i²` over rows `n + 1 … m` of the transformed `B`, in order, each product rounded,
//!   only when the rank is `n` and `m > n`).
//!
//! Each column of `B` goes through its own sequence of operations (only the norms, the scans
//! and the kernel class of the last columns are shared), so the work is split over columns
//! with any number of threads and the result is the one-thread result of numpy (tedana's
//! default, `--n-threads 1`; OpenBLAS's threaded `dgemv` splits the columns and changes the
//! classes, so numpy's own results depend on its BLAS thread count).
//!
//! Upstream files (BSD-3-Clause; the notices are in the parent module and NOTICE): OpenBLAS
//! 0.3.34 and 0.3.30 `lapack-netlib/SRC/` (`dgelsd.f`, `dgeqrf.f`, `dgeqr2.f`, `dormqr.f`,
//! `dorm2r.f`, `dlarf.f`, `dlarf1f.f`, `dgebrd.f`, `dgebd2.f`, `dormbr.f`, `dormlq.f`,
//! `dorml2.f`, `dlalsd.f`, `dlasdq.f`, `dbdsqr.f`, `dlascl.f`, `dlaset.f`, `dlange.f`,
//! `dlanst.f`, `dlasrt.f`, `dlacpy.f`, `iladlc.f`, `ilaenv.f`), `interface/` (`gemv.c`,
//! `ger.c`, `axpy.c`, `scal.c`, `rot.c`, `gemm.c`), `kernel/x86_64/` (`dgemv_t_4.c`,
//! `dgemv_t_microk_haswell-4.c`, `daxpy.c`, `daxpy_microk_haswell-2.c`, `drot.c`,
//! `dgemm_kernel_4x8_haswell.S`, `KERNEL.HASWELL`), `kernel/generic/` (`ger.c`,
//! `gemm_small_matrix_permit.c`), `driver/level3/level3.c`; numpy 2.5.4 and 2.3.5
//! `numpy/linalg/umath_linalg.cpp` and `_linalg.py`. The logic is re-expressed in Rust; no code
//! was copied.

use super::{
    EPS, LapackRelease, SAFMIN, dbdsqr_upper_ncc, dgebd2, dlanst_max, dlarfg, dswap, iladlc,
};
use larmorx_core::parallel;
use rayon::prelude::*;

/// What `numpy.linalg.lstsq` returns.
#[derive(Clone, Debug, PartialEq)]
pub struct Lstsq {
    /// The solution, `n × nrhs`, row-major (numpy's `x`).
    pub x: Vec<f64>,
    /// Sums of squared residuals, one per column of `b`; empty unless the rank is `n` and
    /// `m > n` (numpy's rule).
    pub residuals: Vec<f64>,
    /// The effective rank of `a`.
    pub rank: usize,
    /// The singular values of `a`, decreasing.
    pub s: Vec<f64>,
}

/// Why `lstsq` gave no solution.
#[derive(Debug, thiserror::Error)]
pub enum LstsqError {
    /// numpy's `LinAlgError('Incompatible dimensions')`, or a buffer of the wrong length.
    #[error("Incompatible dimensions")]
    IncompatibleDimensions,
    /// numpy's `LinAlgError`: `DGELSD` returned `INFO != 0` (no convergence, or a NaN in the
    /// bidiagonal form: numpy 2.3.5 with a design that is not finite), or (numpy 2.5.4) `a`
    /// is not finite.
    #[error("SVD did not converge in Linear Least Squares")]
    DidNotConverge,
    /// A shape this port does not follow (it follows `n = 2` with `m = 0` or `m >= 2`).
    #[error(
        "lstsq is ported for a design of two columns and no or at least two rows, not {m} x {n}"
    )]
    Unsupported { m: usize, n: usize },
    #[error(transparent)]
    Threads(#[from] parallel::ThreadPoolError),
}

/// Columns per parallel task (a column's operations never depend on the task it is in).
const CHUNK_COLS: usize = 256;

/// `numpy.linalg.lstsq(a, b, rcond)` for an `m × 2` float64 `a` (row-major, as numpy holds
/// it) and `b` given column-major (`m × nrhs`, one column per right-hand side, as LAPACK holds
/// numpy's Fortran copy), which is overwritten. `rcond = None` is numpy's default,
/// `eps · max(m, n)`. `release` selects LAPACK 3.12.1 with numpy 2.5.4 (its finite check of
/// `a`) or LAPACK 3.12.0 with numpy 2.3.5. `n_threads` (0: all CPUs) never changes the result.
pub fn lstsq(
    a: &[f64],
    m: usize,
    n: usize,
    b: &mut [f64],
    nrhs: usize,
    rcond: Option<f64>,
    release: LapackRelease,
    n_threads: usize,
) -> Result<Lstsq, LstsqError> {
    if n != 2 || m == 1 {
        return Err(LstsqError::Unsupported { m, n });
    }
    if a.len() != m * n || b.len() != m * nrhs {
        return Err(LstsqError::IncompatibleDimensions);
    }
    if m == 0 {
        // No rows (tedana's fit of a series whose every volume was removed): DGELSD returns at
        // once (rank 0, no singular values) and _linalg.py sets x to 0 (`if m == 0: x[...] = 0`).
        return Ok(Lstsq {
            x: vec![0.0; n * nrhs],
            residuals: Vec::new(),
            rank: 0,
            s: Vec::new(),
        });
    }
    // _linalg.py: rcond = finfo(float64).eps * max(n, m) (exact: a power of two times m).
    let rcond = rcond.unwrap_or(f64::EPSILON * m.max(n) as f64);
    // umath_linalg.cpp (2.5.4): `!all_finite(A) || call_gelsd(...)`.
    if release == LapackRelease::V3_12_1 && a.iter().any(|v| !v.is_finite()) {
        return Err(LstsqError::DidNotConverge);
    }
    if nrhs == 0 {
        // _linalg.py: LAPACK cannot take nrhs = 0, so numpy solves one zero column and drops it.
        let mut zero = vec![0.0; m];
        let mut out =
            parallel::with_threads(n_threads, || dgelsd2(a, m, &mut zero, 1, rcond, release))??;
        out.x.clear();
        out.residuals.clear();
        return Ok(out);
    }
    parallel::with_threads(n_threads, || dgelsd2(a, m, b, nrhs, rcond, release))?
}

/// `DGELSD(M, 2, NRHS, A, M, B, M, S, RCOND, ...)` then numpy's outputs.
fn dgelsd2(
    a_rows: &[f64],
    m: usize,
    b: &mut [f64],
    nrhs: usize,
    rcond: f64,
    release: LapackRelease,
) -> Result<Lstsq, LstsqError> {
    const N: usize = 2;
    let ldb = m; // max(m, n)
    // linearize_matrix: A in Fortran order, lda = m.
    let mut a = vec![0.0f64; m * N];
    for i in 0..m {
        for j in 0..N {
            a[i + j * m] = a_rows[i * N + j];
        }
    }
    let eps = 2.0 * EPS; // DLAMCH('P')
    let sfmin = SAFMIN; // DLAMCH('S')
    let smlnum = sfmin / eps;
    let bignum = 1.0 / smlnum;
    let mut s = vec![0.0f64; N];

    // Scale A if max entry outside range [SMLNUM, BIGNUM].
    let anrm = dlange_max(&a);
    let mut iascl = 0;
    if anrm > 0.0 && anrm < smlnum {
        scale_all(&mut a, &dlascl_factors(anrm, smlnum));
        iascl = 1;
    } else if anrm > bignum {
        scale_all(&mut a, &dlascl_factors(anrm, bignum));
        iascl = 2;
    } else if anrm == 0.0 {
        // Matrix all zero: zero solution (DLASET of B and S), rank 0, straight to label 10.
        b.iter_mut().for_each(|v| *v = 0.0);
        return Ok(numpy_outputs(b, m, nrhs, 0, s));
    }
    // Scale B if max entry outside range [SMLNUM, BIGNUM].
    let bnrm = dlange_max(b);
    let mut ibscl = 0;
    if bnrm > 0.0 && bnrm < smlnum {
        scale_all(b, &dlascl_factors(bnrm, smlnum));
        ibscl = 1;
    } else if bnrm > bignum {
        scale_all(b, &dlascl_factors(bnrm, bignum));
        ibscl = 2;
    }

    // Path 1 (M >= N). MNTHR = ILAENV(6, 'DGELSD') = INT(MIN(M, N) · 1.6) = 3.
    let lda = m;
    if m >= 3 {
        // Path 1a: A = Q·R (DGEQRF → DGEQR2; NB = 32 > K = 2: unblocked).
        let mut alpha = a[0];
        let tau1 = dlarfg(m, &mut alpha, &mut a, 1, 1);
        a[0] = alpha;
        let v1: Vec<f64> = std::iter::once(1.0)
            .chain(a[1..m].iter().copied())
            .collect();
        reflect_left(release, m, &v1, tau1, &mut a[lda..2 * lda], 0, lda, 1);
        let mut alpha = a[lda + 1];
        let tau2 = dlarfg(m - 1, &mut alpha, &mut a, lda + 2, 1);
        a[lda + 1] = alpha;
        // B := Qᵀ·B (DORMQR('L', 'T') → DORM2R: H(1), then H(2)).
        reflect_left(release, m, &v1, tau1, b, 0, ldb, nrhs);
        let v2: Vec<f64> = std::iter::once(1.0)
            .chain(a[lda + 2..2 * lda].iter().copied())
            .collect();
        reflect_left(release, m - 1, &v2, tau2, b, 1, ldb, nrhs);
        // Zero out below R.
        a[1] = 0.0;
    }
    // Bidiagonalize R in A (MM = N), or A itself (MM = M = 2).
    let (mut d, mut e, tauq, taup) = dgebd2(release, 2, N, &mut a, lda);
    // B := Q_bᵀ·B (DORMBR('Q', 'L', 'T') → DORMQR → DORM2R, reflectors 1 then 2).
    for i in 1..=N {
        let v: Vec<f64> = (i..=N)
            .map(|r| {
                if r == i {
                    1.0
                } else {
                    a[(r - 1) + (i - 1) * lda]
                }
            })
            .collect();
        reflect_left(release, N - i + 1, &v, tauq[i - 1], b, i - 1, ldb, nrhs);
    }
    // Solve the bidiagonal least squares problem.
    let Some(rank) = dlalsd2(&mut d, &mut e, b, ldb, nrhs, rcond, release) else {
        return Err(LstsqError::DidNotConverge);
    };
    // B := P_b·B (DORMBR('P', 'L', 'N') → DORMLQ → DORML2 with the one reflector G(1), which
    // DLARFG(1, ...) made the identity: TAUP(1) = 0).
    debug_assert_eq!(taup[0], 0.0);
    s.copy_from_slice(&d);
    // Undo scaling.
    let rows_scale = |b: &mut [f64], f: &[f64]| {
        if f.is_empty() {
            return;
        }
        for j in 0..nrhs {
            for i in 0..N {
                for &mul in f {
                    b[i + j * ldb] *= mul;
                }
            }
        }
    };
    if iascl == 1 {
        rows_scale(b, &dlascl_factors(anrm, smlnum));
        scale_all(&mut s, &dlascl_factors(smlnum, anrm));
    } else if iascl == 2 {
        rows_scale(b, &dlascl_factors(anrm, bignum));
        scale_all(&mut s, &dlascl_factors(bignum, anrm));
    }
    if ibscl == 1 {
        rows_scale(b, &dlascl_factors(smlnum, bnrm));
    } else if ibscl == 2 {
        rows_scale(b, &dlascl_factors(bignum, bnrm));
    }
    Ok(numpy_outputs(b, m, nrhs, rank, s))
}

/// numpy's `x` (rows 1…n of `B`), residuals and rank.
fn numpy_outputs(b: &[f64], m: usize, nrhs: usize, rank: usize, s: Vec<f64>) -> Lstsq {
    const N: usize = 2;
    let mut x = vec![0.0f64; N * nrhs];
    for j in 0..nrhs {
        for i in 0..N {
            x[i * nrhs + j] = b[i + j * m];
        }
    }
    let residuals = if rank == N && m > N {
        (0..nrhs)
            .map(|j| {
                let mut res = 0.0f64;
                for &el in &b[j * m + N..(j + 1) * m] {
                    let sq = el * el;
                    res += sq;
                }
                res
            })
            .collect()
    } else {
        Vec::new()
    };
    Lstsq {
        x,
        residuals,
        rank,
        s,
    }
}

/// `DLANGE('M', ...)`: the largest magnitude, a NaN wins (`VALUE.LT.TEMP .OR. DISNAN(TEMP)`).
pub(super) fn dlange_max(a: &[f64]) -> f64 {
    let mut value = 0.0f64;
    for &v in a {
        let temp = v.abs();
        if value < temp || temp.is_nan() {
            value = temp;
        }
    }
    value
}

/// The multipliers `DLASCL(TYPE, 0, 0, CFROM, CTO, ...)` applies to every element, in order
/// (more than one when `cto / cfrom` would overflow or underflow); none when its argument
/// check rejects `cfrom` (0 or NaN) or `cto` (NaN), where LAPACK calls XERBLA and returns.
pub(super) fn dlascl_factors(cfrom: f64, cto: f64) -> Vec<f64> {
    let mut out = Vec::new();
    if cfrom == 0.0 || cfrom.is_nan() || cto.is_nan() {
        return out;
    }
    let smlnum = SAFMIN;
    let bignum = 1.0 / smlnum;
    let mut cfromc = cfrom;
    let mut ctoc = cto;
    loop {
        let cfrom1 = cfromc * smlnum;
        let (mul, done);
        if cfrom1 == cfromc {
            // CFROMC is an inf: a correctly signed zero (or a NaN when CTOC is an inf).
            mul = ctoc / cfromc;
            done = true;
        } else {
            let cto1 = ctoc / bignum;
            if cto1 == ctoc {
                // CTOC is 0 or an inf: it is the factor itself.
                mul = ctoc;
                done = true;
                cfromc = 1.0;
            } else if cfrom1.abs() > ctoc.abs() && ctoc != 0.0 {
                mul = smlnum;
                done = false;
                cfromc = cfrom1;
            } else if cto1.abs() > cfromc.abs() {
                mul = bignum;
                done = false;
                ctoc = cto1;
            } else {
                mul = ctoc / cfromc;
                done = true;
                if mul == 1.0 {
                    return out;
                }
            }
        }
        out.push(mul);
        if done {
            return out;
        }
    }
}

pub(super) fn scale_all(a: &mut [f64], factors: &[f64]) {
    for &mul in factors {
        a.iter_mut().for_each(|v| *v *= mul);
    }
}

/// `DLALSD('U', 25, 2, NRHS, D, E, B, LDB, RCOND, RANK, ...)` (`N <= SMLSIZ`). Returns the
/// rank, or `None` where it returns `INFO != 0`: `DBDSQR` does not converge, or `D`/`E` hold a
/// NaN (`DLASCL`'s argument check).
fn dlalsd2(
    d: &mut [f64],
    e: &mut [f64],
    b: &mut [f64],
    ldb: usize,
    nrhs: usize,
    rcond: f64,
    release: LapackRelease,
) -> Option<usize> {
    const N: usize = 2;
    // DLAMCH('Epsilon') is 2^-53.
    let rcnd = if rcond <= 0.0 || rcond >= 1.0 {
        EPS
    } else {
        rcond
    };
    let mut rank = 0;
    // Scale.
    let orgnrm = dlanst_max(d, &e[..N - 1]);
    if orgnrm.is_nan() {
        // DLASCL rejects CFROM = NaN (INFO = -4, XERBLA's message) in the scalings of D and E
        // and in the last one of B; DLASRT clears INFO before that last one, so DLALSD returns
        // INFO = -4 and DGELSD with it: numpy's LinAlgError (2.3.5 reaches this with a design
        // that is not finite; 2.5.4 rejects such a design first).
        return None;
    }
    if orgnrm == 0.0 {
        for j in 0..nrhs {
            b[j * ldb] = 0.0;
            b[1 + j * ldb] = 0.0;
        }
        return Some(0);
    }
    let f = dlascl_factors(orgnrm, 1.0);
    scale_all(&mut d[..N], &f);
    scale_all(&mut e[..N - 1], &f);
    // DLASET(VT = I); DLASDQ('U', 0, N, N, 0, NRHS, D, E, VT, N, U, N, B, LDB): upper and
    // square, straight to DBDSQR, then its sort into increasing order.
    let mut vt = [1.0, 0.0, 0.0, 1.0];
    let info = dbdsqr_upper_ncc(
        N,
        d,
        e,
        (&mut vt, N, N),
        (&mut [], 0, N),
        (b, nrhs, ldb),
        release == LapackRelease::V3_12_1,
    );
    if info != 0 {
        return None;
    }
    for i in 1..=N {
        let mut isub = i;
        let mut smin = d[i - 1];
        for j in i + 1..=N {
            if d[j - 1] < smin {
                isub = j;
                smin = d[j - 1];
            }
        }
        if isub != i {
            d[isub - 1] = d[i - 1];
            d[i - 1] = smin;
            dswap(N, &mut vt, isub - 1, N, i - 1, N);
            dswap(nrhs, b, isub - 1, ldb, i - 1, ldb);
        }
    }
    // TOL = RCND · |D(IDAMAX(N, D, 1))| (the first index of the largest magnitude).
    let dmax = if d[1].abs() > d[0].abs() {
        d[1].abs()
    } else {
        d[0].abs()
    };
    let tol = rcnd * dmax;
    for i in 0..N {
        if d[i] <= tol {
            for j in 0..nrhs {
                b[i + j * ldb] = 0.0;
            }
        } else {
            let f = dlascl_factors(d[i], 1.0);
            for j in 0..nrhs {
                for &mul in &f {
                    b[i + j * ldb] *= mul;
                }
            }
            rank += 1;
        }
    }
    // DGEMM('T', 'N', N, NRHS, N, 1, VT, N, B, LDB, 0, W, N), then DLACPY(W → B): OpenBLAS's
    // Haswell kernel, per element fma(VT(2,i), B(2,j), fma(VT(1,i), B(1,j), +0)), times
    // alpha = 1, added to the cleared W.
    for j in 0..nrhs {
        let (b1, b2) = (b[j * ldb], b[1 + j * ldb]);
        for i in 0..N {
            let acc = vt[1 + i * N].mul_add(b2, vt[i * N].mul_add(b1, 0.0));
            b[i + j * ldb] = 0.0 + acc * 1.0;
        }
    }
    // Unscale.
    scale_all(&mut d[..N], &dlascl_factors(1.0, orgnrm));
    // DLASRT('D', N, D): insertion sort into decreasing order.
    if d[1] > d[0] {
        d.swap(0, 1);
    }
    let f = dlascl_factors(orgnrm, 1.0);
    for j in 0..nrhs {
        for i in 0..N {
            for &mul in &f {
                b[i + j * ldb] *= mul;
            }
        }
    }
    Some(rank)
}

/// Which of `dgemv_t_4.c`'s kernels computes column `j` of `lastc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GemvClass {
    /// The first `4·⌊lastc/4⌋` columns: `dgemv_kernel_4x4` (Haswell microkernel).
    Fma4,
    /// The next two when `lastc mod 4 >= 2`: `dgemv_kernel_4x2`.
    Pair,
    /// The last one when `lastc` is odd: `dgemv_kernel_4x1`.
    Single,
}

pub(super) fn gemv_class(j: usize, lastc: usize) -> GemvClass {
    let n4 = lastc & !3;
    if j < n4 {
        GemvClass::Fma4
    } else if lastc & 2 != 0 && j < n4 + 2 {
        GemvClass::Pair
    } else {
        GemvClass::Single
    }
}

/// `dgemv_kernel_4x4` for one column: four lanes (rows modulo 4) of fused multiply-adds from
/// +0, combined as `(l0 + l2) + (l1 + l3)`.
fn dot_fma4(a: &[f64], x: &[f64]) -> f64 {
    let mut l = [0.0f64; 4];
    for (ra, rx) in a.as_chunks::<4>().0.iter().zip(x.as_chunks::<4>().0) {
        for t in 0..4 {
            l[t] = ra[t].mul_add(rx[t], l[t]);
        }
    }
    (l[0] + l[2]) + (l[1] + l[3])
}

/// `dgemv_kernel_4x2` for one column: two SSE2 lanes (rows modulo 2), each product rounded,
/// then `haddpd`.
fn dot_pair(a: &[f64], x: &[f64]) -> f64 {
    let (mut l0, mut l1) = (0.0f64, 0.0f64);
    for (ra, rx) in a.as_chunks::<2>().0.iter().zip(x.as_chunks::<2>().0) {
        let p0 = rx[0] * ra[0];
        let p1 = rx[1] * ra[1];
        l0 += p0;
        l1 += p1;
    }
    l0 + l1
}

/// `dgemv_kernel_4x1`: two SSE2 accumulators of two lanes (rows modulo 4), each product
/// rounded, then `addpd` and `haddpd`: `(s0 + s2) + (s1 + s3)`.
fn dot_single(a: &[f64], x: &[f64]) -> f64 {
    let mut s = [0.0f64; 4];
    for (ra, rx) in a.as_chunks::<4>().0.iter().zip(x.as_chunks::<4>().0) {
        for t in 0..4 {
            let p = ra[t] * rx[t];
            s[t] += p;
        }
    }
    (s[0] + s[2]) + (s[1] + s[3])
}

/// `DGEMV('T', len, lastc, 1, A, LDA, x, 1, 0, y, 1)`'s value for one column `a` (`len`
/// rows) of class `class` (`interface/gemv.c`: `y` cleared to +0; `dgemv_t_4.c`).
pub(super) fn gemv_t_column(class: GemvClass, a: &[f64], x: &[f64]) -> f64 {
    let m = a.len();
    let m3 = m & 3;
    let body = m - m3;
    let mut y = 0.0f64;
    let mut r0 = 0;
    // The row blocks: NBMAX = 2048 rows while they last, then the rest of `body`.
    while r0 < body {
        let nb = (body - r0).min(2048);
        let (ab, xb) = (&a[r0..r0 + nb], &x[r0..r0 + nb]);
        let p = match class {
            GemvClass::Fma4 => dot_fma4(ab, xb),
            GemvClass::Pair => dot_pair(ab, xb),
            GemvClass::Single => dot_single(ab, xb),
        };
        // add_y (`y + round(p · 1)`) or `*y += ybuffer · alpha` (fma(p, 1, y)): both y + p.
        y += p;
        r0 += nb;
    }
    match m3 {
        0 => y,
        1 => a[body].mul_add(x[body], y),
        2 => a[body].mul_add(x[body], a[body + 1] * x[body + 1]) + y,
        _ => {
            let t = a[body].mul_add(x[body], a[body + 1] * x[body + 1]);
            a[body + 2].mul_add(x[body + 2], t) + y
        }
    }
}

/// `AXPYU_K(len, s, x, 1, y, 1)` (`daxpy.c`): the first `16·⌊len/16⌋` elements
/// `fma(s, x, y)` (the Haswell microkernel), the rest `y + round(s·x)`.
pub(super) fn axpy_column(s: f64, x: &[f64], y: &mut [f64]) {
    let n1 = y.len() & !15;
    let (head, tail) = y.split_at_mut(n1);
    for (yi, &xi) in head.iter_mut().zip(x) {
        *yi = s.mul_add(xi, *yi);
    }
    for (yi, &xi) in tail.iter_mut().zip(&x[n1..]) {
        let p = s * xi;
        *yi += p;
    }
}

/// `H = I - tau·v·vᵀ` from the left on rows `r0 … r0 + mi - 1` of the first `ncols` columns
/// of `c` (column-major, `ldc`): `DLARF('L', mi, ncols, v, 1, tau, C(r0+1, 1), LDC)` (3.12.0,
/// `v[0] = 1` stored) or `DLARF1F` (3.12.1, `v[0] = 1` implicit). `v` holds `mi` values.
#[allow(clippy::too_many_arguments)]
pub(super) fn reflect_left(
    release: LapackRelease,
    mi: usize,
    v: &[f64],
    tau: f64,
    c: &mut [f64],
    r0: usize,
    ldc: usize,
    ncols: usize,
) {
    if mi == 0 || ncols == 0 {
        return;
    }
    let neg_tau = -tau;
    match release {
        LapackRelease::V3_12_0 => {
            if tau == 0.0 {
                return;
            }
            let mut lastv = mi;
            while lastv > 0 && v[lastv - 1] == 0.0 {
                lastv -= 1;
            }
            if lastv == 0 {
                return;
            }
            let lastc = iladlc(lastv, ncols, c, r0, ldc);
            for_columns(c, ldc, lastc, |j, col| {
                let col = &mut col[r0..r0 + lastv];
                let w = gemv_t_column(gemv_class(j, lastc), col, &v[..lastv]);
                // DGER: alpha · y[j], then AXPYU_K down the column.
                let s = neg_tau * w;
                axpy_column(s, &v[..lastv], col);
            });
        }
        LapackRelease::V3_12_1 => {
            if tau == 0.0 {
                return;
            }
            let mut lastv = mi;
            while lastv > 1 && v[lastv - 1] == 0.0 {
                lastv -= 1;
            }
            let lastc = iladlc(lastv, ncols, c, r0, ldc);
            if lastc == 0 {
                return;
            }
            if lastv == 1 {
                // C(1, 1:lastc) *= 1 - tau (DSCAL; nothing when the factor is 1).
                let f = 1.0 - tau;
                if f != 1.0 {
                    for_columns(c, ldc, lastc, |_, col| col[r0] *= f);
                }
                return;
            }
            for_columns(c, ldc, lastc, |j, col| {
                let col = &mut col[r0..r0 + lastv];
                // w = C(2:lastv, j)ᵀ v(2:lastv); w += C(1, j) (DAXPY with a stride, 1·x exact);
                // C(1, j) += round(-tau · w) (DAXPY with a stride); C(2:lastv, j) via DGER.
                let mut w = gemv_t_column(gemv_class(j, lastc), &col[1..], &v[1..lastv]);
                w += col[0];
                let p = neg_tau * w;
                col[0] += p;
                let s = neg_tau * w;
                axpy_column(s, &v[1..lastv], &mut col[1..]);
            });
        }
    }
}

/// Runs `f(j, column j)` on columns `0 … ncols - 1` of `c` (each `ldc` long), in parallel
/// chunks of whole columns.
fn for_columns(c: &mut [f64], ldc: usize, ncols: usize, f: impl Fn(usize, &mut [f64]) + Sync) {
    let used = &mut c[..ncols * ldc];
    if ncols <= CHUNK_COLS {
        for (j, col) in used.chunks_exact_mut(ldc).enumerate() {
            f(j, col);
        }
        return;
    }
    used.par_chunks_mut(CHUNK_COLS * ldc)
        .enumerate()
        .for_each(|(k, chunk)| {
            for (t, col) in chunk.chunks_exact_mut(ldc).enumerate() {
                f(k * CHUNK_COLS + t, col);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Least squares through the normal equations, for a loose check of the solution.
    fn normal_equations(a: &[f64], m: usize, col: &[f64]) -> [f64; 2] {
        let (mut s00, mut s01, mut s11, mut r0, mut r1) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for i in 0..m {
            let (x0, x1) = (a[2 * i], a[2 * i + 1]);
            s00 += x0 * x0;
            s01 += x0 * x1;
            s11 += x1 * x1;
            r0 += x0 * col[i];
            r1 += x1 * col[i];
        }
        let det = s00 * s11 - s01 * s01;
        [(s11 * r0 - s01 * r1) / det, (s00 * r1 - s01 * r0) / det]
    }

    fn design(tes: &[f64], vols: usize) -> Vec<f64> {
        let mut a = Vec::new();
        for &te in tes {
            for _ in 0..vols {
                a.extend([1.0, -te]);
            }
        }
        a
    }

    #[test]
    fn solves_tedana_like_batches() {
        for (tes, vols, nrhs) in [
            (vec![14.0, 29.96, 45.92], 200usize, 37usize),
            (vec![12.0, 28.0], 1, 9),
            (vec![10.0, 20.0, 30.0, 40.0, 50.0], 3, 6),
            (vec![10.0, 20.0, 30.0, 40.0, 50.0], 500, 5),
        ] {
            let a = design(&tes, vols);
            let m = tes.len() * vols;
            let mut b = Vec::with_capacity(m * nrhs);
            for j in 0..nrhs {
                let (ls0, t2) = (6.0 + 0.1 * j as f64, 20.0 + 3.0 * j as f64);
                for (r, &te) in tes.iter().enumerate() {
                    for t in 0..vols {
                        let noise = (((r * 31 + t * 17 + j * 7) % 13) as f64 - 6.0) * 1e-3;
                        b.push(ls0 - te / t2 + noise);
                    }
                }
            }
            let reference: Vec<[f64; 2]> = (0..nrhs)
                .map(|j| normal_equations(&a, m, &b[j * m..(j + 1) * m]))
                .collect();
            for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
                for threads in [1, 3] {
                    let mut bb = b.clone();
                    let out = lstsq(&a, m, 2, &mut bb, nrhs, None, release, threads).unwrap();
                    assert_eq!(out.rank, 2);
                    for (j, want_j) in reference.iter().enumerate() {
                        for (i, &want) in want_j.iter().enumerate() {
                            let got = out.x[i * nrhs + j];
                            assert!(
                                (got - want).abs() < 1e-9 * want.abs().max(1.0),
                                "{got} {want}"
                            );
                        }
                    }
                    assert_eq!(out.residuals.len(), if m > 2 { nrhs } else { 0 });
                    assert!(out.s[0] >= out.s[1]);
                }
            }
        }
    }

    #[test]
    fn threads_do_not_change_bits() {
        let tes = [14.0, 29.96, 45.92];
        let a = design(&tes, 40);
        let m = 120;
        let nrhs = 1003;
        let b: Vec<f64> = (0..m * nrhs)
            .map(|k| 5.0 + ((k * 2654435761usize) % 1000) as f64 * 1e-3)
            .collect();
        let run = |threads| {
            let mut bb = b.clone();
            lstsq(
                &a,
                m,
                2,
                &mut bb,
                nrhs,
                None,
                LapackRelease::V3_12_1,
                threads,
            )
            .unwrap()
        };
        let one = run(1);
        for t in [2, 7] {
            let other = run(t);
            assert!(
                one.x
                    .iter()
                    .zip(&other.x)
                    .all(|(p, q)| p.to_bits() == q.to_bits())
            );
        }
    }

    #[test]
    fn collinear_design_has_rank_one() {
        let a = design(&[30.0, 30.0], 4);
        let mut b: Vec<f64> = (0..8).map(|k| 1.0 + k as f64).collect();
        let out = lstsq(&a, 8, 2, &mut b, 1, None, LapackRelease::V3_12_1, 1).unwrap();
        assert_eq!(out.rank, 1);
        assert!(out.residuals.is_empty());
    }

    #[test]
    fn non_finite_design_and_shapes() {
        let mut a = design(&[10.0, 20.0], 2);
        a[3] = f64::INFINITY;
        let mut b = vec![1.0; 4];
        assert!(matches!(
            lstsq(&a, 4, 2, &mut b, 1, None, LapackRelease::V3_12_1, 1),
            Err(LstsqError::DidNotConverge)
        ));
        assert!(matches!(
            lstsq(
                &a[..2],
                1,
                2,
                &mut b[..1],
                1,
                None,
                LapackRelease::V3_12_1,
                1
            ),
            Err(LstsqError::Unsupported { .. })
        ));
        let a = design(&[10.0, 20.0], 2);
        let out = lstsq(&a, 4, 2, &mut [], 0, None, LapackRelease::V3_12_1, 1).unwrap();
        assert!(out.x.is_empty() && out.rank == 2);
        // No rows: numpy's x of zeros (+0), rank 0, no singular values or residuals.
        for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
            let out = lstsq(&[], 0, 2, &mut [], 3, None, release, 1).unwrap();
            assert_eq!(out.x.len(), 6);
            assert!(out.x.iter().all(|v| v.to_bits() == 0));
            assert!(out.rank == 0 && out.s.is_empty() && out.residuals.is_empty());
        }
    }

    #[test]
    fn kernel_classes() {
        assert_eq!(gemv_class(0, 1), GemvClass::Single);
        assert_eq!(gemv_class(4, 7), GemvClass::Pair);
        assert_eq!(gemv_class(5, 7), GemvClass::Pair);
        assert_eq!(gemv_class(6, 7), GemvClass::Single);
        assert_eq!(gemv_class(7, 8), GemvClass::Fma4);
        assert_eq!(gemv_class(4, 6), GemvClass::Pair);
    }
}
