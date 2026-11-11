// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The minimum-norm least-squares solution of a general float64 system, as numpy's x86-64 wheels
//! compute it at one BLAS thread on the Haswell kernels (`OPENBLAS_CORETYPE=Haswell`), both LAPACK
//! releases, on every platform: `numpy.linalg.lstsq(a, b, rcond)` (LAPACK's `DGELSD`), the
//! regression of nipype's `cosine_filter` (`lstsq(X, data.T)`: a `T × K` discrete-cosine design,
//! `K <= 25` in every fMRIPrep run, one right-hand side per voxel, up to 60,968).
//!
//! **`DGELSD`** ([`dgelsd`]; `dgelsd.f`, identical in both trees): its workspace query
//! ([`dgelsd_lwork`]: `MAXWRK` and `MINWRK` from `ILAENV`'s block size 32, `LIWORK`, `WLALSD`
//! with `SMLSIZ = 25` and `NLVL = max(⌊log(minmn/26)/log 2⌋ + 1, 0)` through glibc's `log`;
//! `MNTHR = ILAENV(6) = INT(REAL(min(m, n))·1.6)` in single precision); `A` and `B` scaled by
//! `DLASCL` when their largest magnitude (`DLANGE('M')`) lies outside `[SMLNUM, BIGNUM]`
//! (`SMLNUM = 2^-1022 / 2^-52`), an all-zero `A` giving `B = 0`, `S = 0`, rank 0; for `m < n` the
//! rows `m + 1 … n` of `B` cleared; then one of three paths:
//!
//! - `m >= n`: `DGEQRF` and `DORMQR('L', 'T')` on `B` when `m >= MNTHR` (then `R` with zeros below
//!   it), `DGEBRD` of `R` (or of `A`), `DORMBR('Q', 'L', 'T')` on `B`, `DLALSD('U')`,
//!   `DORMBR('P', 'L', 'N')` on `B`;
//! - `m < n`, `n >= MNTHR` and a workspace of at least `4m + m² + max(m, 2m - 4, nrhs, n - 3m,
//!   WLALSD)` (numpy's optimal one always is): `DGELQF`, `L` copied out with zeros above it (at
//!   leading dimension `m`, or `LDA` when the workspace allows), `DGEBRD` of `L`, `DORMBR('Q')`,
//!   `DLALSD('U')`, `DORMBR('P')`, rows `m + 1 … n` of `B` cleared, `DORMLQ('L', 'T')`;
//! - otherwise `DGEBRD` of `A` (lower bidiagonal), `DORMBR('Q')`, `DLALSD('L')`, `DORMBR('P')`;
//!
//! and the unscalings. Each routine gets the workspace `DGELSD` hands it, `LWORK - NWORK + 1`,
//! so the callees' short-workspace rules apply as in LAPACK (`blocked`'s documentation).
//!
//! **`DLALSD`** ([`dlalsd`]; `dlalsd.f`, identical in both trees): `RCND = 2^-53` when `RCOND` is
//! not in `(0, 1)`; `N = 1` (the scaling of `B`'s row by `1/D(1)`, no tolerance); a lower
//! bidiagonal rotated to upper by `DLARTG` with `DROT` on the rows of `B` (one row pair at a time
//! for one right-hand side, column by column for several: the same arithmetic per column);
//! `DLANST`, an all-zero bidiagonal giving `B = 0`; the scaling by `ORGNRM`; and **for `N <=
//! SMLSIZ = 25`** the identity `VT`, `DLASDQ('U', 0, N, N, 0, NRHS)` with `C = B`, `TOL = RCND ·
//! |D(IDAMAX)|`, each row of `B` cleared (`D(i) <= TOL`) or scaled by `1/D(i)` (`DLASCL`) with the
//! rank counted, `B = VTᵀ·B` by `DGEMM('T', 'N')`, `D` unscaled and sorted down by [`dlasrt`],
//! `B` unscaled. The divide-and-conquer path for `N > 25` (`DLASDA`, `DLALSA`) is not ported yet
//! ([`DgelsdError::LargePathNotPorted`]); fMRIPrep never reaches it (a cosine design of more than
//! 25 columns needs a series of more than about 27 minutes at the 128 s cutoff).
//!
//! **numpy's `lstsq`** ([`numpy_lstsq`]; `umath_linalg.cpp`'s `lstsq`, `init_gelsd`,
//! `call_gelsd`, the same in numpy 2.5.4 and 2.3.5 but `all_finite`; `_linalg.py`'s `lstsq`, the
//! same in both): `a` and `b` copied into Fortran order (`LDA = max(1, m)`, `LDB = max(1, m, n)`),
//! the workspace query and the call with its optimal `LWORK`; `rcond = None` is `2^-52·max(m, n)`;
//! no right-hand side is solved as one zero column and dropped; `m = 0` gives `x = 0`; `x` is the
//! first `n` rows of `B`; the residuals `Σ B(i, j)²` over the rows `n + 1 … m`, each product
//! rounded and summed in order, only when the rank is `n` and `m > n`; `DGELSD`'s `INFO != 0`
//! gives numpy's `LinAlgError('SVD did not converge in Linear Least Squares')`. numpy 2.5.4 refuses
//! an `a` that is not finite before LAPACK (the same error); numpy 2.3.5 and 2.2.6 hand it to
//! LAPACK, where an infinity is scaled to NaN (`docs/findings/confounds-blas.md`): larmorx gives the
//! error for every release, as for the SVD (NP-10). `b` is never checked: a NaN or an infinity in
//! it flows through the arithmetic as in LAPACK.
//!
//! The work on `B` splits over its columns wherever LAPACK's operations on one column never
//! depend on the others (the kernels' own rules, `blocked` and [`crate::openblas`]); the bits never
//! depend on the number of threads.
//!
//! Upstream files (BSD-3-Clause; the notices are in the parent module and NOTICE): OpenBLAS
//! 0.3.30 and v0.3.34-106-g446c436e `lapack-netlib/SRC/` (`dgelsd.f`, `dlalsd.f`, `dlasrt.f`,
//! `dlasdq.f`, `dlartg.f90`, `dlanst.f`, `dlange.f`, `dlascl.f`, `dlaset.f`, `dlacpy.f`,
//! `ilaenv.f`, `INSTALL/dlamch.f`, and the routines of [`super::blocked`] and [`super::bdsdc`]),
//! `interface/rot.c`, `kernel/x86_64/drot.c`, `kernel/x86_64/iamax_sse2.S`; numpy 2.5.4 and 2.3.5
//! `numpy/linalg/umath_linalg.cpp` (`lstsq`, `init_gelsd`, `call_gelsd`, `abs2`, `all_finite`,
//! `linearize_matrix`, `delinearize_matrix`) and `numpy/linalg/_linalg.py` (`lstsq`). The logic is
//! re-expressed in Rust; no code was copied.

// The routines keep LAPACK's names, arguments and 1-based loops, so the port can be checked line
// by line against the Fortran.
#![allow(
    clippy::too_many_arguments,
    clippy::many_single_char_names,
    clippy::needless_range_loop
)]

use super::bdsdc::{SMLSIZ, dlascl_g, dlasdq};
use super::blocked::{
    IllegalValue, Lapack, NB, Vect, dgebrd, dgelqf, dgeqrf, dormbr, dormlq, dormqr,
};
use super::lstsq::{Lstsq, LstsqError};
use super::svd::{dlange_m, zero_block};
use super::{EPS, SAFMIN, dlanst_max, dlartg, drot};
use crate::openblas::level1::idamax;
use crate::openblas::level3::{Operand, Trans, dgemm_with};
use crate::openblas::matmul::Array2;
use crate::openblas::trmm::{Side, Uplo};
use larmorx_core::math::log_glibc;
use rayon::prelude::*;

/// Why [`dgelsd`] or [`dlalsd`] did not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DgelsdError {
    /// A parameter LAPACK's checks refuse (`XERBLA`'s message).
    #[error(transparent)]
    Illegal(#[from] IllegalValue),
    /// `A` holds a NaN or an infinity: LAPACK has no check of its own, scales an infinity to NaN
    /// and returns what the NaN does to its arithmetic (module documentation).
    #[error("DGELSD: the matrix holds a value that is not finite")]
    NotFinite,
    /// `DLALSD`'s divide-and-conquer path (a bidiagonal of more than [`SMLSIZ`] rows) is not
    /// ported yet.
    #[error(
        "DLALSD: the divide-and-conquer path for a bidiagonal of {n} > 25 rows is not ported yet"
    )]
    LargePathNotPorted {
        /// The bidiagonal's order.
        n: usize,
    },
}

/// The order [`dlasrt`] sorts into (`ID`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    /// `'I'`: increasing.
    Increasing,
    /// `'D'`: decreasing.
    Decreasing,
}

/// `DLASRT(ID, N, D, INFO)`: `d` sorted (insertion sort on stretches of at most 20 steps,
/// otherwise quicksort with the median of the first, middle and last entries, smaller part
/// first), with LAPACK's comparisons: equal values (`+0` and `-0` among them) keep their order
/// within an insertion stretch, and a NaN stops each scan where it stands.
pub fn dlasrt(id: Sort, d: &mut [f64]) {
    const SELECT: i64 = 20;
    let n = d.len() as i64;
    if n <= 1 {
        return;
    }
    let dec = id == Sort::Decreasing;
    // D(j), 1-based.
    let at = |i: i64| (i - 1) as usize;
    let mut stack: Vec<(i64, i64)> = vec![(1, n)];
    while let Some((start, endd)) = stack.pop() {
        if endd - start <= SELECT && endd - start > 0 {
            // Do insertion sort on D(START:ENDD).
            for i in start + 1..=endd {
                let mut j = i;
                while j > start {
                    let swap = if dec {
                        d[at(j)] > d[at(j - 1)]
                    } else {
                        d[at(j)] < d[at(j - 1)]
                    };
                    if !swap {
                        break;
                    }
                    d.swap(at(j), at(j - 1));
                    j -= 1;
                }
            }
        } else if endd - start > SELECT {
            // Partition D(START:ENDD) and stack parts, largest one first. Choose partition entry
            // as median of 3.
            let d1 = d[at(start)];
            let d2 = d[at(endd)];
            let d3 = d[at((start + endd) / 2)];
            let dmnmx = if d1 < d2 {
                if d3 < d1 {
                    d1
                } else if d3 < d2 {
                    d3
                } else {
                    d2
                }
            } else if d3 < d2 {
                d2
            } else if d3 < d1 {
                d3
            } else {
                d1
            };
            let mut i = start - 1;
            let mut j = endd + 1;
            loop {
                loop {
                    j -= 1;
                    let more = if dec {
                        d[at(j)] < dmnmx
                    } else {
                        d[at(j)] > dmnmx
                    };
                    if !more {
                        break;
                    }
                }
                loop {
                    i += 1;
                    let more = if dec {
                        d[at(i)] > dmnmx
                    } else {
                        d[at(i)] < dmnmx
                    };
                    if !more {
                        break;
                    }
                }
                if i < j {
                    d.swap(at(i), at(j));
                } else {
                    break;
                }
            }
            if j - start > endd - j - 1 {
                stack.push((start, j));
                stack.push((j + 1, endd));
            } else {
                stack.push((j + 1, endd));
                stack.push((start, j));
            }
        }
    }
}

/// `ILAENV(6, 'DGELSD', ' ', M, N, NRHS, -1)`: `INT(REAL(MIN(M, N))·1.6E0)`, in single precision.
pub fn gelsd_mnthr(m: usize, n: usize) -> usize {
    ((m.min(n) as f32) * 1.6f32) as usize
}

/// `DGELSD`'s `NLVL = MAX(INT(LOG(DBLE(MINMN)/DBLE(SMLSIZ + 1))/LOG(TWO)) + 1, 0)` with
/// `MINMN = max(1, min(m, n))` (glibc's `log`; `INT` truncates towards zero).
pub fn gelsd_nlvl(minmn: usize) -> usize {
    let t = log_glibc(minmn.max(1) as f64 / (SMLSIZ + 1) as f64) / log_glibc(2.0);
    (t as i64 + 1).max(0) as usize
}

/// `DGELSD(M, N, NRHS, ...)`'s workspace query: `(MINWRK, MAXWRK, LIWORK)`, the smallest `LWORK`
/// it accepts, the optimal one (`WORK(1)` of the `LWORK = -1` query) and `IWORK(1)`. The same in
/// both LAPACK releases.
pub fn dgelsd_lwork(m: usize, n: usize, nrhs: usize) -> (usize, usize, usize) {
    let mnthr = gelsd_mnthr(m, n) as i64;
    let minmn = m.min(n).max(1) as i64;
    let nlvl = gelsd_nlvl(minmn as usize) as i64;
    let (m, n, nrhs) = (m as i64, n as i64, nrhs as i64);
    let nb = NB as i64;
    let smlsiz = SMLSIZ as i64;
    let liwork = 3 * minmn * nlvl + 11 * minmn;
    let mut minwrk = 1i64;
    let mut maxwrk = 1i64;
    let mut mm = m;
    if m >= n && m >= mnthr {
        // Path 1a: overdetermined, with many more rows than columns.
        mm = n;
        maxwrk = maxwrk.max(n + n * nb);
        maxwrk = maxwrk.max(n + nrhs * nb);
    }
    if m >= n {
        // Path 1: overdetermined or exactly determined.
        maxwrk = maxwrk.max(3 * n + (mm + n) * nb);
        maxwrk = maxwrk.max(3 * n + nrhs * nb);
        maxwrk = maxwrk.max(3 * n + (n - 1) * nb);
        let wlalsd = 9 * n + 2 * n * smlsiz + 8 * n * nlvl + n * nrhs + (smlsiz + 1).pow(2);
        maxwrk = maxwrk.max(3 * n + wlalsd);
        minwrk = (3 * n + mm).max(3 * n + nrhs).max(3 * n + wlalsd);
    }
    if n > m {
        let wlalsd = 9 * m + 2 * m * smlsiz + 8 * m * nlvl + m * nrhs + (smlsiz + 1).pow(2);
        if n >= mnthr {
            // Path 2a: underdetermined, with many more columns than rows.
            maxwrk = m + m * nb;
            maxwrk = maxwrk.max(m * m + 4 * m + 2 * m * nb);
            maxwrk = maxwrk.max(m * m + 4 * m + nrhs * nb);
            maxwrk = maxwrk.max(m * m + 4 * m + (m - 1) * nb);
            if nrhs > 1 {
                maxwrk = maxwrk.max(m * m + m + m * nrhs);
            } else {
                maxwrk = maxwrk.max(m * m + 2 * m);
            }
            maxwrk = maxwrk.max(m + nrhs * nb);
            maxwrk = maxwrk.max(m * m + 4 * m + wlalsd);
            // XXX: Ensure the Path 2a case below is triggered (LAPACK's own comment).
            maxwrk = maxwrk.max(4 * m + m * m + m.max(2 * m - 4).max(nrhs).max(n - 3 * m));
        } else {
            // Path 2: remaining underdetermined cases.
            maxwrk = 3 * m + (n + m) * nb;
            maxwrk = maxwrk.max(3 * m + nrhs * nb);
            maxwrk = maxwrk.max(3 * m + m * nb);
            maxwrk = maxwrk.max(3 * m + wlalsd);
        }
        minwrk = (3 * m + nrhs).max(3 * m + m).max(3 * m + wlalsd);
    }
    minwrk = minwrk.min(maxwrk);
    (minwrk as usize, maxwrk as usize, liwork as usize)
}

/// `DLASCL('G', 0, 0, CFROM, CTO, M, N, A, LDA, INFO)` as a Fortran caller sees it: the `INFO`
/// it leaves (0, or minus the refused parameter, nothing scaled).
fn scl(cfrom: f64, cto: f64, m: usize, n: usize, a: &mut [f64], aoff: usize, lda: usize) -> i64 {
    match dlascl_g(cfrom, cto, m, n, a, aoff, lda) {
        Ok(()) => 0,
        Err(e) => -i64::from(e.param),
    }
}

/// The columns of the `rows × ncols` block of `b` at `boff` (leading dimension `ldb`), each
/// handed to `f(j, column)` (0-based `j`) on the thread pool.
fn for_columns(
    b: &mut [f64],
    boff: usize,
    ldb: usize,
    rows: usize,
    ncols: usize,
    f: impl Fn(usize, &mut [f64]) + Sync,
) {
    if ncols == 0 || rows == 0 {
        return;
    }
    let end = boff + (ncols - 1) * ldb + rows;
    b[boff..end]
        .par_chunks_mut(ldb)
        .enumerate()
        .for_each(|(j, col)| f(j, &mut col[..rows]));
}

/// `DLALSD(UPLO, SMLSIZ, N, NRHS, D, E, B, LDB, RCOND, RANK, WORK, IWORK, INFO)`: the
/// minimum-norm solution of the `n × n` bidiagonal system (`d`: `n` values, `e`: `n - 1`, `UPLO`
/// its upper or lower form) for the `nrhs` columns of the block of `b` at `boff` (leading
/// dimension `ldb`), overwritten; `d` returns the singular values, decreasing (module
/// documentation). Returns `(RANK, INFO)` as LAPACK leaves them: `INFO = -4` for `NRHS = 0` and
/// `-8` for `LDB < max(1, N)` (XERBLA's refusals, nothing done, `RANK` not set: 0 here), `> 0`
/// when `DLASDQ` does not converge, `-4` from a refused `DLASCL` (a NaN in the bidiagonal).
///
/// # Errors
/// [`DgelsdError::LargePathNotPorted`] for `n > smlsiz` (unless the bidiagonal is zero);
/// `DLASDQ`'s argument checks (never refused here).
pub fn dlalsd(
    lp: Lapack,
    uplo: Uplo,
    smlsiz: usize,
    n: usize,
    nrhs: usize,
    d: &mut [f64],
    e: &mut [f64],
    b: &mut [f64],
    boff: usize,
    ldb: usize,
    rcond: f64,
) -> Result<(usize, i64), DgelsdError> {
    // Test the input parameters.
    if nrhs < 1 {
        return Ok((0, -4));
    }
    if ldb < 1 || ldb < n {
        return Ok((0, -8));
    }
    let eps = EPS; // DLAMCH('Epsilon')
    // Set up the tolerance.
    let rcnd = if rcond <= 0.0 || rcond >= 1.0 {
        eps
    } else {
        rcond
    };
    let mut rank = 0usize;
    let row = |i: usize| boff + (i - 1);
    // Quick return if possible.
    if n == 0 {
        return Ok((0, 0));
    } else if n == 1 {
        let info;
        if d[0] == 0.0 {
            for j in 0..nrhs {
                b[row(1) + j * ldb] = 0.0;
            }
            info = 0;
        } else {
            rank = 1;
            info = scl(d[0], 1.0, 1, nrhs, b, boff, ldb);
            d[0] = d[0].abs();
        }
        return Ok((rank, info));
    }
    // Rotate the matrix if it is lower bidiagonal.
    if uplo == Uplo::Lower {
        let mut rot = vec![(0.0f64, 0.0f64); n - 1];
        for i in 1..n {
            let (cs, sn, r) = dlartg(d[i - 1], e[i - 1]);
            d[i - 1] = r;
            e[i - 1] = sn * d[i];
            d[i] = cs * d[i];
            rot[i - 1] = (cs, sn);
        }
        // One right-hand side: each DROT right after its rotation; several: column by column,
        // the rotations in order. The same operations on each column either way.
        for_columns(b, boff, ldb, n, nrhs, |_, col| {
            for (j, &(cs, sn)) in rot.iter().enumerate() {
                drot(1, col, j, 1, j + 1, 1, cs, sn);
            }
        });
    }
    // Scale.
    let nm1 = n - 1;
    let orgnrm = dlanst_max(&d[..n], &e[..nm1]);
    if orgnrm == 0.0 {
        for j in 0..nrhs {
            for i in 1..=n {
                b[row(i) + j * ldb] = 0.0;
            }
        }
        return Ok((0, 0));
    }
    let _ = scl(orgnrm, 1.0, n, 1, d, 0, n);
    let _ = scl(orgnrm, 1.0, nm1, 1, e, 0, nm1.max(1));
    // If N is smaller than the minimum divide size SMLSIZ, then solve the problem with another
    // solver.
    if n > smlsiz {
        return Err(DgelsdError::LargePathNotPorted { n });
    }
    // VT = I (DLASET('A', N, N, 0, 1, WORK, N)).
    let mut vt = vec![0.0f64; n * n];
    for i in 0..n {
        vt[i + i * n] = 1.0;
    }
    let info = dlasdq(
        lp.release,
        Uplo::Upper,
        0,
        n,
        n,
        0,
        nrhs,
        d,
        e,
        &mut vt,
        0,
        n,
        &mut [],
        0,
        n,
        b,
        boff,
        ldb,
    )?;
    if info != 0 {
        return Ok((rank, info as i64));
    }
    // TOL = RCND·|D(IDAMAX(N, D, 1))| (D is DGELSD's S: an allocation of its own, aligned).
    let tol = rcnd * d[idamax(n, d, 0, 1, true) - 1].abs();
    let mut info = 0i64;
    for i in 1..=n {
        if d[i - 1] <= tol {
            for j in 0..nrhs {
                b[row(i) + j * ldb] = 0.0;
            }
        } else {
            info = scl(d[i - 1], 1.0, 1, nrhs, b, row(i), ldb);
            rank += 1;
        }
    }
    // B = VTᵀ·B through WORK(NWORK) (DGEMM('T', 'N'), then DLACPY).
    let mut w = vec![0.0f64; n * nrhs];
    let vto = Operand {
        data: &vt,
        off: 0,
        ld: n,
        trans: Trans::T,
    };
    let bo = Operand {
        data: b,
        off: boff,
        ld: ldb,
        trans: Trans::N,
    };
    dgemm_with(n, nrhs, n, 1.0, vto, bo, 0.0, &mut w, 0, n, lp.gemm_q);
    for_columns(b, boff, ldb, n, nrhs, |j, col| {
        col.copy_from_slice(&w[j * n..(j + 1) * n]);
    });
    // Unscale.
    let _ = info;
    let _ = scl(1.0, orgnrm, n, 1, d, 0, n);
    dlasrt(Sort::Decreasing, &mut d[..n]);
    let info = scl(orgnrm, 1.0, n, nrhs, b, boff, ldb);
    Ok((rank, info))
}

/// What [`dgelsd`] returns: the effective rank, LAPACK's `INFO` (0; `> 0` when `DLASDQ` did not
/// converge, the results then as LAPACK leaves them; `-4` when `NRHS = 0`, `DLALSD`'s refusal, the
/// rank then unset in LAPACK and 0 here), `WORK(1)` and `IWORK(1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gelsd {
    /// `RANK`.
    pub rank: usize,
    /// LAPACK's `INFO`.
    pub info: i64,
    /// `WORK(1)` on return: [`dgelsd_lwork`]'s optimal workspace.
    pub work1: usize,
    /// `IWORK(1)` on return: [`dgelsd_lwork`]'s `LIWORK`.
    pub iwork1: usize,
}

/// `DGELSD(M, N, NRHS, A, LDA, B, LDB, S, RCOND, RANK, WORK, LWORK, IWORK, INFO)`: the
/// minimum-norm solution of `min ‖b - A·x‖` for the column-major `m × n` matrix `a` (`a[i +
/// j·lda]`, overwritten) and the `nrhs` columns of `b` (`max(m, n)` rows used, leading dimension
/// `ldb`), overwritten by the `n × nrhs` solution in its first rows; the `min(m, n)` singular
/// values of `A`, decreasing, in `s` (module documentation). `lwork` is the workspace the caller
/// gives (`None`: the optimal, as numpy's query gives it).
///
/// # Errors
/// `LDA < max(1, M)` (5), `LDB < max(1, M, N)` (7), `LWORK < MINWRK` (12) as LAPACK
/// ([`DgelsdError::Illegal`]); a NaN or infinite entry in `A` ([`DgelsdError::NotFinite`]);
/// [`DgelsdError::LargePathNotPorted`] when `min(m, n) > 25`.
pub fn dgelsd(
    lp: Lapack,
    m: usize,
    n: usize,
    nrhs: usize,
    a: &mut [f64],
    lda: usize,
    b: &mut [f64],
    ldb: usize,
    s: &mut [f64],
    rcond: f64,
    lwork: Option<usize>,
) -> Result<Gelsd, DgelsdError> {
    let maxmn = m.max(n);
    let mnthr = gelsd_mnthr(m, n);
    let err = |param| {
        Err(DgelsdError::Illegal(IllegalValue {
            routine: "DGELSD",
            param,
        }))
    };
    if lda < m.max(1) {
        return err(5);
    }
    if ldb < maxmn.max(1) {
        return err(7);
    }
    let (minwrk, maxwrk, liwork) = dgelsd_lwork(m, n, nrhs);
    let lwork = lwork.unwrap_or(maxwrk);
    if lwork < minwrk {
        return err(12);
    }
    let done = |rank, info| {
        Ok(Gelsd {
            rank,
            info,
            work1: maxwrk,
            iwork1: liwork,
        })
    };
    // Quick return if possible.
    if m == 0 || n == 0 {
        return done(0, 0);
    }
    let minmn = m.min(n);
    // Get machine parameters.
    let eps = 2.0 * EPS; // DLAMCH('P')
    let sfmin = SAFMIN; // DLAMCH('S')
    let smlnum = sfmin / eps;
    let bignum = 1.0 / smlnum;
    // Scale A if max entry outside range [SMLNUM, BIGNUM].
    let anrm = dlange_m(m, n, a, lda);
    if !anrm.is_finite() {
        return Err(DgelsdError::NotFinite);
    }
    let mut iascl = 0;
    if anrm > 0.0 && anrm < smlnum {
        // Scale matrix norm up to SMLNUM.
        dlascl_g(anrm, smlnum, m, n, a, 0, lda)?;
        iascl = 1;
    } else if anrm > bignum {
        // Scale matrix norm down to BIGNUM.
        dlascl_g(anrm, bignum, m, n, a, 0, lda)?;
        iascl = 2;
    } else if anrm == 0.0 {
        // Matrix all zero. Return zero solution.
        zero_block(maxmn, nrhs, b, ldb);
        s[..minmn].fill(0.0);
        return done(0, 0);
    }
    // Scale B if max entry outside range [SMLNUM, BIGNUM].
    let bnrm = dlange_m(m, nrhs, b, ldb);
    let mut ibscl = 0;
    if bnrm > 0.0 && bnrm < smlnum {
        let _ = scl(bnrm, smlnum, m, nrhs, b, 0, ldb);
        ibscl = 1;
    } else if bnrm > bignum {
        let _ = scl(bnrm, bignum, m, nrhs, b, 0, ldb);
        ibscl = 2;
    }
    // If M < N make sure certain entries of B are zero.
    if m < n {
        for_columns(b, m, ldb, n - m, nrhs, |_, col| col.fill(0.0));
    }
    let mut e = vec![0.0f64; minmn];
    let mut tauq = vec![0.0f64; minmn];
    let mut taup = vec![0.0f64; minmn];
    // Overdetermined case.
    if m >= n {
        // Path 1 - overdetermined or exactly determined.
        let mut mm = m;
        if m >= mnthr {
            // Path 1a - overdetermined, with many more rows than columns. ITAU = 1, NWORK =
            // ITAU + N.
            mm = n;
            let lw = lwork - n;
            let mut tau = vec![0.0f64; n];
            // Compute A = Q·R.
            dgeqrf(lp, m, n, a, 0, lda, &mut tau, Some(lw))?;
            // Multiply B by transpose(Q).
            dormqr(
                lp,
                Side::Left,
                Trans::T,
                m,
                nrhs,
                n,
                a,
                0,
                lda,
                &tau,
                b,
                0,
                ldb,
                Some(lw),
            )?;
            // Zero out below R.
            for j in 1..n {
                for i in j + 1..=n {
                    a[(i - 1) + (j - 1) * lda] = 0.0;
                }
            }
        }
        // IE = 1, ITAUQ = IE + N, ITAUP = ITAUQ + N, NWORK = ITAUP + N.
        let lw = lwork - 3 * n;
        // Bidiagonalize R in A.
        dgebrd(
            lp,
            mm,
            n,
            a,
            0,
            lda,
            &mut s[..n],
            &mut e,
            &mut tauq,
            &mut taup,
            Some(lw),
        )?;
        // Multiply B by transpose of left bidiagonalizing vectors of R.
        dormbr(
            lp,
            Vect::Q,
            Side::Left,
            Trans::T,
            mm,
            nrhs,
            n,
            a,
            0,
            lda,
            &tauq,
            b,
            0,
            ldb,
            Some(lw),
        )?;
        // Solve the bidiagonal least squares problem.
        let (rank, info) = dlalsd(
            lp,
            Uplo::Upper,
            SMLSIZ,
            n,
            nrhs,
            &mut s[..n],
            &mut e,
            b,
            0,
            ldb,
            rcond,
        )?;
        if info != 0 {
            return done(rank, info);
        }
        // Multiply B by right bidiagonalizing vectors of R.
        dormbr(
            lp,
            Vect::P,
            Side::Left,
            Trans::N,
            n,
            nrhs,
            n,
            a,
            0,
            lda,
            &taup,
            b,
            0,
            ldb,
            Some(lw),
        )?;
        unscale(
            iascl, ibscl, anrm, bnrm, smlnum, bignum, n, minmn, nrhs, b, ldb, s,
        );
        return done(rank, 0);
    }
    let (mi, ni, nrhsi) = (m as i64, n as i64, nrhs as i64);
    let nlvl = gelsd_nlvl(minmn) as i64;
    let smlsiz = SMLSIZ as i64;
    let wlalsd = 9 * mi + 2 * mi * smlsiz + 8 * mi * nlvl + mi * nrhsi + (smlsiz + 1).pow(2);
    let rest = mi.max(2 * mi - 4).max(nrhsi).max(ni - 3 * mi);
    let lworki = lwork as i64;
    let (rank, info);
    if n >= mnthr && lworki >= 4 * mi + mi * mi + rest.max(wlalsd) {
        // Path 2a - underdetermined, with many more columns than rows and sufficient workspace
        // for an efficient algorithm.
        let ldai = lda as i64;
        let ldwork = if lworki
            >= (4 * mi + mi * ldai + rest)
                .max(mi * ldai + mi + mi * nrhsi)
                .max(4 * mi + mi * ldai + wlalsd)
        {
            lda
        } else {
            m
        };
        // ITAU = 1, NWORK = M + 1.
        let mut tau = vec![0.0f64; m];
        // Compute A = L·Q.
        dgelqf(lp, m, n, a, 0, lda, &mut tau, Some(lwork - m))?;
        // Copy L to WORK(IL), zeroing out above its diagonal.
        let mut l = vec![0.0f64; ldwork * m];
        for j in 1..=m {
            for i in 1..=m {
                l[(i - 1) + (j - 1) * ldwork] = if i >= j {
                    a[(i - 1) + (j - 1) * lda]
                } else {
                    0.0
                };
            }
        }
        // IE = IL + LDWORK·M, ITAUQ = IE + M, ITAUP = ITAUQ + M, NWORK = ITAUP + M.
        let lw = lwork - (m + ldwork * m + 3 * m);
        // Bidiagonalize L in WORK(IL).
        dgebrd(
            lp,
            m,
            m,
            &mut l,
            0,
            ldwork,
            &mut s[..m],
            &mut e,
            &mut tauq,
            &mut taup,
            Some(lw),
        )?;
        // Multiply B by transpose of left bidiagonalizing vectors of L.
        dormbr(
            lp,
            Vect::Q,
            Side::Left,
            Trans::T,
            m,
            nrhs,
            m,
            &mut l,
            0,
            ldwork,
            &tauq,
            b,
            0,
            ldb,
            Some(lw),
        )?;
        // Solve the bidiagonal least squares problem.
        (rank, info) = dlalsd(
            lp,
            Uplo::Upper,
            SMLSIZ,
            m,
            nrhs,
            &mut s[..m],
            &mut e,
            b,
            0,
            ldb,
            rcond,
        )?;
        if info != 0 {
            return done(rank, info);
        }
        // Multiply B by right bidiagonalizing vectors of L.
        dormbr(
            lp,
            Vect::P,
            Side::Left,
            Trans::N,
            m,
            nrhs,
            m,
            &mut l,
            0,
            ldwork,
            &taup,
            b,
            0,
            ldb,
            Some(lw),
        )?;
        // Zero out below first M rows of B.
        for_columns(b, m, ldb, n - m, nrhs, |_, col| col.fill(0.0));
        // Multiply transpose(Q) by B. NWORK = ITAU + M.
        dormlq(
            lp,
            Side::Left,
            Trans::T,
            n,
            nrhs,
            m,
            a,
            0,
            lda,
            &tau,
            b,
            0,
            ldb,
            Some(lwork - m),
        )?;
    } else {
        // Path 2 - remaining underdetermined cases. IE = 1, ITAUQ = IE + M, ITAUP = ITAUQ + M,
        // NWORK = ITAUP + M.
        let lw = lwork - 3 * m;
        // Bidiagonalize A.
        dgebrd(
            lp,
            m,
            n,
            a,
            0,
            lda,
            &mut s[..m],
            &mut e,
            &mut tauq,
            &mut taup,
            Some(lw),
        )?;
        // Multiply B by transpose of left bidiagonalizing vectors.
        dormbr(
            lp,
            Vect::Q,
            Side::Left,
            Trans::T,
            m,
            nrhs,
            n,
            a,
            0,
            lda,
            &tauq,
            b,
            0,
            ldb,
            Some(lw),
        )?;
        // Solve the bidiagonal least squares problem.
        (rank, info) = dlalsd(
            lp,
            Uplo::Lower,
            SMLSIZ,
            m,
            nrhs,
            &mut s[..m],
            &mut e,
            b,
            0,
            ldb,
            rcond,
        )?;
        if info != 0 {
            return done(rank, info);
        }
        // Multiply B by right bidiagonalizing vectors of A.
        dormbr(
            lp,
            Vect::P,
            Side::Left,
            Trans::N,
            n,
            nrhs,
            m,
            a,
            0,
            lda,
            &taup,
            b,
            0,
            ldb,
            Some(lw),
        )?;
    }
    unscale(
        iascl, ibscl, anrm, bnrm, smlnum, bignum, n, minmn, nrhs, b, ldb, s,
    );
    done(rank, 0)
}

/// `DGELSD`'s undoing of the scalings of `A` (`iascl`) and `B` (`ibscl`).
fn unscale(
    iascl: u8,
    ibscl: u8,
    anrm: f64,
    bnrm: f64,
    smlnum: f64,
    bignum: f64,
    n: usize,
    minmn: usize,
    nrhs: usize,
    b: &mut [f64],
    ldb: usize,
    s: &mut [f64],
) {
    if iascl == 1 {
        let _ = scl(anrm, smlnum, n, nrhs, b, 0, ldb);
        let _ = scl(smlnum, anrm, minmn, 1, s, 0, minmn);
    } else if iascl == 2 {
        let _ = scl(anrm, bignum, n, nrhs, b, 0, ldb);
        let _ = scl(bignum, anrm, minmn, 1, s, 0, minmn);
    }
    if ibscl == 1 {
        let _ = scl(smlnum, bnrm, n, nrhs, b, 0, ldb);
    } else if ibscl == 2 {
        let _ = scl(bignum, bnrm, n, nrhs, b, 0, ldb);
    }
}

/// `numpy.linalg.lstsq(a, b, rcond)` of the float64 `m × n` matrix `a` and the `m × nrhs` matrix
/// `b` (any strides; a 1-D `b` is one column), with LAPACK `lp` (numpy 2.5.4:
/// [`super::LapackRelease::V3_12_1`]; numpy 2.3.5 and 2.2.6: [`super::LapackRelease::V3_12_0`]);
/// `rcond = None` is numpy's default `2^-52·max(m, n)` (module documentation). `x` is `n × nrhs`,
/// C order.
///
/// # Errors
/// [`LstsqError::IncompatibleDimensions`] when `b` has another number of rows; [`LstsqError::
/// DidNotConverge`] when `DGELSD` returns `INFO != 0` or `a` is not finite;
/// [`LstsqError::Unsupported`] where `DGELSD` needs `DLALSD`'s divide-and-conquer path
/// (`min(m, n) > 25`, not ported yet).
pub fn numpy_lstsq(
    lp: Lapack,
    a: Array2<'_>,
    b: Array2<'_>,
    rcond: Option<f64>,
) -> Result<Lstsq, LstsqError> {
    let (m, n) = (a.rows, a.cols);
    if b.rows != m {
        return Err(LstsqError::IncompatibleDimensions);
    }
    let nrhs_in = b.cols;
    // _linalg.py: rcond = finfo(float64).eps · max(n, m) (exact).
    let rcond = rcond.unwrap_or(f64::EPSILON * m.max(n) as f64);
    // LAPACK cannot take nrhs = 0: numpy solves one zero column and drops it.
    let nrhs = nrhs_in.max(1);
    let lda = m.max(1);
    let ldb = m.max(n).max(1);
    // Copy the matrices in (Fortran order); B's rows past m are never read before DGELSD clears
    // them.
    let mut fa = vec![0.0f64; lda * n];
    for j in 0..n {
        for i in 0..m {
            fa[i + j * lda] = a.data[a.off + i * a.s0 + j * a.s1];
        }
    }
    let mut fb = vec![0.0f64; ldb * nrhs];
    if nrhs_in > 0 {
        for_columns(&mut fb, 0, ldb, m, nrhs, |j, col| {
            for (i, v) in col.iter_mut().enumerate() {
                *v = b.data[b.off + i * b.s0 + j * b.s1];
            }
        });
    }
    // umath_linalg.cpp (2.5.4; larmorx for every release: NP-10): `!all_finite(A) || ...`.
    if fa.iter().any(|v| !v.is_finite()) {
        return Err(LstsqError::DidNotConverge);
    }
    let k = m.min(n);
    let mut s = vec![0.0f64; k];
    let out = match dgelsd(
        lp, m, n, nrhs, &mut fa, lda, &mut fb, ldb, &mut s, rcond, None,
    ) {
        Ok(out) => out,
        Err(DgelsdError::LargePathNotPorted { .. }) => {
            return Err(LstsqError::Unsupported { m, n });
        }
        Err(_) => return Err(LstsqError::DidNotConverge),
    };
    if out.info != 0 {
        return Err(LstsqError::DidNotConverge);
    }
    let rank = out.rank;
    // x: the first n rows of B (`m == 0`: _linalg.py sets x to 0).
    let mut x = vec![0.0f64; n * nrhs_in];
    if m > 0 {
        for j in 0..nrhs_in {
            for i in 0..n {
                x[i * nrhs_in + j] = fb[i + j * ldb];
            }
        }
    }
    // The residuals (umath_linalg.cpp's abs2 over rows n.. of each column, at stride m), kept by
    // _linalg.py only when rank == n and m > n.
    let residuals = if rank == n && m > n {
        (0..nrhs_in)
            .map(|j| {
                let mut res = 0.0f64;
                for &el in &fb[j * m + n..(j + 1) * m] {
                    let sq = el * el;
                    res += sq;
                }
                res
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(Lstsq {
        x,
        residuals,
        rank,
        s,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::numpy_linalg::LapackRelease;

    fn matrix(m: usize, n: usize, seed: u64) -> Vec<f64> {
        // A plain linear congruential generator: values in (-1, 1).
        let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (0..m * n)
            .map(|_| {
                x = x
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((x >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
            })
            .collect()
    }

    fn view(a: &[f64], m: usize, n: usize) -> Array2<'_> {
        Array2 {
            data: a,
            off: 0,
            rows: m,
            cols: n,
            s0: n,
            s1: 1,
        }
    }

    /// `DLASRT` sorts both ways, by insertion and by quicksort, keeping the multiset.
    #[test]
    fn dlasrt_sorts() {
        for n in [0usize, 1, 2, 3, 20, 21, 22, 25, 60, 257] {
            let v = matrix(n, 1, n as u64);
            for id in [Sort::Increasing, Sort::Decreasing] {
                let mut d = v.clone();
                dlasrt(id, &mut d);
                let mut want = v.clone();
                want.sort_by(|a, b| a.partial_cmp(b).unwrap());
                if id == Sort::Decreasing {
                    want.reverse();
                }
                assert_eq!(d, want, "{n} {id:?}");
            }
        }
    }

    /// The crossover and the tree depth as LAPACK computes them.
    #[test]
    fn mnthr_and_nlvl() {
        assert_eq!(gelsd_mnthr(5, 9), 8);
        assert_eq!(gelsd_mnthr(10, 600), 16);
        assert_eq!(gelsd_mnthr(25, 25), 40);
        assert_eq!(gelsd_mnthr(0, 7), 0);
        for (k, lvl) in [
            (1, 0),
            (12, 0),
            (13, 0),
            (14, 1),
            (25, 1),
            (26, 1),
            (52, 2),
            (600, 5),
        ] {
            assert_eq!(gelsd_nlvl(k), lvl, "minmn = {k}");
        }
        let (minwrk, maxwrk, liwork) = dgelsd_lwork(600, 10, 60_968);
        assert!(minwrk <= maxwrk);
        assert_eq!(maxwrk, 3 * 10 + 60_968 * 32);
        assert_eq!(liwork, 110);
    }

    /// The normal equations hold for the solution of a full-rank tall system; the residuals are
    /// numpy's; a wide system's solution solves it with the smallest norm (orthogonal to the null
    /// space); every path, both releases.
    #[test]
    fn solves_every_path() {
        for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
            let lp = Lapack::new(release);
            for &(m, n, nrhs) in &[
                (1, 1, 1),
                (7, 1, 3),
                (40, 3, 5),
                (30, 20, 2),
                (26, 25, 1),
                (600, 11, 40),
                (3, 7, 2),
                (20, 25, 3),
                (5, 200, 2),
                (9, 9, 1),
            ] {
                let a = matrix(m, n, (m * 100 + n) as u64);
                let b = matrix(m, nrhs, (m + n * 7) as u64);
                let out = numpy_lstsq(lp, view(&a, m, n), view(&b, m, nrhs), None).unwrap();
                assert_eq!(out.rank, m.min(n));
                assert!(out.s.windows(2).all(|w| w[0] >= w[1]));
                for j in 0..nrhs {
                    // r = b - A x.
                    let r: Vec<f64> = (0..m)
                        .map(|i| {
                            b[i * nrhs + j]
                                - (0..n)
                                    .map(|t| a[i * n + t] * out.x[t * nrhs + j])
                                    .sum::<f64>()
                        })
                        .collect();
                    if m >= n {
                        // Aᵀ r = 0.
                        for t in 0..n {
                            let g: f64 = (0..m).map(|i| a[i * n + t] * r[i]).sum();
                            assert!(g.abs() < 1e-10, "{release:?} {m}x{n}: {g}");
                        }
                        if m > n {
                            let rr: f64 = r.iter().map(|v| v * v).sum();
                            assert!((rr - out.residuals[j]).abs() < 1e-10 * (1.0 + rr));
                        }
                    } else {
                        assert!(r.iter().all(|v| v.abs() < 1e-10), "{release:?} {m}x{n}");
                    }
                }
            }
        }
    }

    /// The bits do not depend on the number of threads.
    #[test]
    fn threads_do_not_change_bits() {
        let (m, n, nrhs) = (300, 9, 2_000);
        let a = matrix(m, n, 3);
        let b = matrix(m, nrhs, 4);
        let lp = Lapack::default();
        let run = |t| {
            larmorx_core::parallel::with_threads(t, || {
                numpy_lstsq(lp, view(&a, m, n), view(&b, m, nrhs), None).unwrap()
            })
            .unwrap()
        };
        assert_eq!(run(1), run(5));
    }

    /// The edge cases: rank deficiency, an all-zero `a`, no right-hand side, no rows, no columns,
    /// tiny and huge scalings, a non-finite `a`, mismatched rows, the large path not ported, the
    /// refused arguments.
    #[test]
    fn edges_and_refusals() {
        let lp = Lapack::default();
        let (m, n) = (12, 4);
        let mut a = matrix(m, n, 5);
        let b = matrix(m, 2, 6);
        for i in 0..m {
            a[i * n + 3] = a[i * n];
        }
        let out = numpy_lstsq(lp, view(&a, m, n), view(&b, m, 2), None).unwrap();
        assert_eq!(out.rank, 3);
        assert!(out.residuals.is_empty());
        let z = vec![0.0; m * n];
        let out = numpy_lstsq(lp, view(&z, m, n), view(&b, m, 2), None).unwrap();
        assert_eq!((out.rank, out.x.iter().all(|&v| v == 0.0)), (0, true));
        let out = numpy_lstsq(lp, view(&a, m, n), view(&b, m, 0), None).unwrap();
        assert!(out.x.is_empty() && out.residuals.is_empty() && out.s.len() == n);
        let out = numpy_lstsq(lp, view(&a, 0, n), view(&b, 0, 2), None).unwrap();
        assert_eq!((out.x.len(), out.rank, out.s.len()), (n * 2, 0, 0));
        let out = numpy_lstsq(lp, view(&a, m, 0), view(&b, m, 2), None).unwrap();
        assert!(out.x.is_empty());
        assert!(
            (out.residuals[0] - (0..m).map(|i| b[i * 2] * b[i * 2]).sum::<f64>()).abs() < 1e-12
        );
        let base = numpy_lstsq(lp, view(&a, m, 3), view(&b, m, 2), None).unwrap();
        for f in [2f64.powi(-1000), 2f64.powi(1000)] {
            let af: Vec<f64> = a.iter().map(|v| v * f).collect();
            let bf: Vec<f64> = b.iter().map(|v| v * f).collect();
            let out = numpy_lstsq(lp, view(&af, m, n), view(&b, m, 2), None).unwrap();
            assert_eq!(out.rank, 3);
            let out = numpy_lstsq(lp, view(&a, m, 3), view(&bf, m, 2), None).unwrap();
            for (x, y) in out.x.iter().zip(&base.x) {
                assert!((x / f - y).abs() < 1e-12 * (1.0 + y.abs()));
            }
        }
        let mut an = a.clone();
        an[5] = f64::NAN;
        assert!(matches!(
            numpy_lstsq(lp, view(&an, m, n), view(&b, m, 2), None),
            Err(LstsqError::DidNotConverge)
        ));
        an[5] = f64::INFINITY;
        assert!(matches!(
            numpy_lstsq(lp, view(&an, m, n), view(&b, m, 2), None),
            Err(LstsqError::DidNotConverge)
        ));
        assert!(matches!(
            numpy_lstsq(lp, view(&a, m, n), view(&b, m - 1, 2), None),
            Err(LstsqError::IncompatibleDimensions)
        ));
        let big = matrix(40, 30, 7);
        let bb = matrix(40, 1, 8);
        assert!(matches!(
            numpy_lstsq(lp, view(&big, 40, 30), view(&bb, 40, 1), None),
            Err(LstsqError::Unsupported { m: 40, n: 30 })
        ));
        let p = |r: Result<Gelsd, DgelsdError>| match r {
            Err(DgelsdError::Illegal(e)) => e.param,
            other => panic!("{other:?}"),
        };
        let (mut fa, mut fb, mut s) = (vec![0.0; 48], vec![0.0; 24], vec![0.0; 4]);
        assert_eq!(
            p(dgelsd(
                lp, 12, 4, 2, &mut fa, 11, &mut fb, 12, &mut s, -1.0, None
            )),
            5
        );
        assert_eq!(
            p(dgelsd(
                lp, 12, 4, 2, &mut fa, 12, &mut fb, 11, &mut s, -1.0, None
            )),
            7
        );
        let (minwrk, _, _) = dgelsd_lwork(12, 4, 2);
        assert_eq!(
            p(dgelsd(
                lp,
                12,
                4,
                2,
                &mut fa,
                12,
                &mut fb,
                12,
                &mut s,
                -1.0,
                Some(minwrk - 1)
            )),
            12
        );
        // NRHS = 0: DLALSD's refusal (INFO = -4).
        let mut fa = matrix(12, 4, 9);
        let out = dgelsd(lp, 12, 4, 0, &mut fa, 12, &mut fb, 12, &mut s, -1.0, None).unwrap();
        assert_eq!(out.info, -4);
    }
}
