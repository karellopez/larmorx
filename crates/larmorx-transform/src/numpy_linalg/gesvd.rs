// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! SciPy's `linalg.svd(a, lapack_driver='gesvd')` of a float64 matrix and nipype's `fallback_svd`
//! and `_full_rank` on top of it, as SciPy's x86-64 wheels compute them at one BLAS thread on the
//! Haswell kernels (`OPENBLAS_CORETYPE=Haswell`), on every platform: the decomposition nipype's
//! CompCor and cosine filter fall back to when `numpy.linalg.svd` raises `LinAlgError`.
//!
//! **SciPy's own LAPACK** ([`ScipyLibrary`]). SciPy's wheels bundle their own OpenBLAS
//! (scipy-openblas32: 32-bit integers, not numpy's library): SciPy 1.18.1 the snapshot
//! v0.3.31-22-g5ffbf38b (scipy-openblas32 0.3.31.22.0), SciPy 1.15.3 the v0.3.28 release
//! (0.3.28.0.2). Every LAPACK routine `DGESVD` reaches is the same Fortran in both as in
//! OpenBLAS 0.3.30 (LAPACK 3.12.0, [`LapackRelease::V3_12_0`]: reflectors through `DLARF`), but
//! for `DLARFT`: 0.3.28 has the column-by-column algorithm (the recursive one came with
//! OpenBLAS 0.3.29), [`DlarftForm::ColumnByColumn`]. Neither library has OpenBLAS 0.3.34's
//! `GEMM_Q = 512` rule for AMD Zen 4 and 5, so both block the depth of `DGEMM` by 256
//! ([`crate::openblas::level3::GEMM_Q`]) on every machine. The BLAS kernels and drivers on these
//! paths are those of OpenBLAS 0.3.30 (the files differ only in threading heuristics and
//! other architectures), ported in [`crate::openblas`].
//!
//! **`DGESVD`** ([`dgesvd`]; `JOBU = JOBVT`, `'S'` or `'A'`, the two forms SciPy calls with
//! vectors): `A` scaled by `DLASCL` when its largest magnitude lies outside `[SMLNUM, BIGNUM]`,
//! then, with `MNTHR = INT(REAL(MIN(M, N))·1.6E0)` (`ILAENV(6)`):
//!
//! - `m >= n`, `m >= MNTHR`: path 6 (`'S'`) or 9 (`'A'`): `DGEQRF`; with a workspace of at least
//!   `N·N + MAX(4N, BDSPAC)` (path 9: `MAX(N + M, 4N, BDSPAC)`) `R` is copied into `WORK(IU)`
//!   (leading dimension `LDA` when the workspace allows it, else `N`), `DGEBRD` of it, `DORGBR`
//!   `'Q'` in `WORK(IU)` and `'P'` in `VT`, `DBDSQR('U')`, then `U = Q·WORK(IU)` by `DGEMM` (`Q`
//!   from `DORGQR` in `A` for path 6, in `U` for path 9, whose product goes through `A`);
//!   otherwise `Q` is generated in `U`, `R` reduced in `VT`, `DORMBR('Q', 'R', 'N')` applies the
//!   bidiagonal reduction's `Q` to `U`, `DBDSQR('U')` rotates `U` itself;
//! - `m >= n`, `m < MNTHR`: path 10: `DGEBRD` of `A`, `DORGBR('Q')` into `U` (`N` or `M`
//!   columns), `DORGBR('P')` into `VT`, `DBDSQR('U')`;
//! - `m < n`: the transposed paths 6t, 9t (`DGELQF`, `L` reduced, `VT = WORK(IU)·Q`) and 10t
//!   (`DGEBRD` of `A`: lower bidiagonal, `DBDSQR('L')`).
//!
//! Each routine gets `LWORK - IWORK + 1` (its offset in `WORK`), so the callees' short-workspace
//! rules apply as in LAPACK; the workspace query ([`dgesvd_lwork`]) asks the callees' queries.
//! `DBDSQR` ([`dbdsqr`]) rotates a lower bidiagonal to upper first (`DLARTG`, the rotations
//! applied to `U` and `C` by `DLASR`), then runs the implicit zero-shift QR iteration of the
//! parent module (`DBDSQR` 3.12.0: a `-0` singular value stays `-0`). Without vectors `DBDSQR`
//! takes the dqds algorithm (`DLASQ1`), not ported: [`dbdsqr`] refuses it and `JOBU = JOBVT =
//! 'N'` has no entry here. A non-finite `A` is refused ([`DgesvdError::NotFinite`]): LAPACK would
//! run on NaN (an infinity scaled by `DLASCL` becomes NaN), whose course depends on how gfortran
//! orders NaN operands; SciPy never hands it one (`check_finite`).
//!
//! **SciPy's `svd(a, full_matrices, compute_uv=True, lapack_driver='gesvd')`** ([`scipy_svd`];
//! SciPy 1.15.3 `_decomp_svd.py` with `flapack_gen.pyf.src`'s `dgesvd`/`dgesvd_lwork` and
//! `_compute_lwork`; SciPy 1.18.1 `_decomp_svd.py` with `src/_linalg_svd.hh`'s `_svd_gesvd`): a
//! non-finite entry is `ValueError('array must not contain infs or NaNs')` (`check_finite`); an
//! empty matrix gives identities (`full_matrices`) or empty factors without LAPACK; a factor too
//! large for 32-bit indices is SciPy's `ValueError`; otherwise the matrix is copied into Fortran
//! order (`LDA = M`, `LDU = M`, `LDVT = N` or `MIN(M, N)`), the workspace query's `WORK(1)`
//! truncated to an integer is `LWORK`, and `INFO > 0` is `LinAlgError` ("SVD did not converge",
//! 1.18.1: "... for slice = 0.").
//!
//! **nipype's `fallback_svd(a, full_matrices=False)`** ([`nipype_fallback_svd`]; nipype 1.12.0
//! `algorithms/confounds.py`): `numpy.linalg.svd` ([`super::svd::numpy_svd`]), and on its
//! `LinAlgError` SciPy's `svd` with the `gesvd` driver; **`_full_rank(X, cmax)`**
//! ([`nipype_full_rank`]): the condition number `max(s) / min(s)` of that SVD; at `cmax` or above
//! the singular values are raised by `(smax - cmax·smin) / (cmax - 1)` and `X` rebuilt as
//! `np.dot(U, np.dot(np.diag(s), V))` (numpy's products through [`crate::openblas::matmul`]).
//!
//! Upstream files (BSD-3-Clause; the notices are in the parent module and NOTICE): OpenBLAS
//! 0.3.30, v0.3.31-22-g5ffbf38b and 0.3.28 `lapack-netlib/SRC/` (`dgesvd.f`, `dbdsqr.f`,
//! `dlartg.f90`, `dlasr.f`, `dlange.f`, `dlascl.f`, `dlacpy.f`, `dlaset.f`, `ilaenv.f`, `dlarft.f`
//! of 0.3.28, and the routines of [`super::blocked`]); SciPy 1.15.3 and 1.18.1 (BSD-3-Clause,
//! Copyright (c) 2001-2002 Enthought, Inc. 2003, SciPy Developers) `scipy/linalg/_decomp_svd.py`,
//! `flapack_gen.pyf.src`, `lapack.py`, `src/_linalg_svd.hh`, `src/_batched_linalg_module.cc`,
//! `scipy/_lib/_util.py`; nipype 1.12.0 (Apache-2.0, Copyright (c) 2009-2016, Nipype developers
//! and contributors) `nipype/algorithms/confounds.py` (`fallback_svd`, `_full_rank`). The logic
//! is re-expressed in Rust; no code was copied.

// The routines keep LAPACK's names, arguments and paths, so the port can be checked line by line
// against the Fortran.
#![allow(clippy::too_many_arguments, clippy::many_single_char_names)]

use super::bdsdc::{dlacpy, dlascl_g};
use super::blocked::{
    DlarftForm, IllegalValue, Lapack, Vect, dgebrd, dgelqf, dgeqrf, dorgbr, dorglq, dorgqr, dormbr,
    gebrd_lwork_opt, lwork_opt, org_lwork_opt, orgbr_lwork_opt,
};
use super::gelsd::gelsd_mnthr;
use super::svd::{Svd, dlange_m, numpy_svd};
use super::{EPS, LapackRelease, SAFMIN, dbdsqr_upper_ncc, dlartg, dlasr};
use crate::openblas::level3::{GEMM_Q, Operand, Trans, dgemm_with};
use crate::openblas::matmul::{Array2, numpy_matmul_with};
use crate::openblas::trmm::{Side, Uplo};

/// The OpenBLAS a SciPy wheel bundles (module documentation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScipyLibrary {
    /// SciPy 1.18.1: scipy-openblas32 0.3.31.22.0 (OpenBLAS v0.3.31-22-g5ffbf38b). The default.
    #[default]
    V1_18,
    /// SciPy 1.15.3: scipy-openblas32 0.3.28.0.2 (OpenBLAS 0.3.28: the column-by-column
    /// `DLARFT`).
    V1_15,
}

impl ScipyLibrary {
    /// The LAPACK settings of the library: LAPACK 3.12.0, `GEMM_Q` 256, and 0.3.28's `DLARFT` for
    /// SciPy 1.15.3.
    pub const fn lapack(self) -> Lapack {
        Lapack {
            release: LapackRelease::V3_12_0,
            gemm_q: GEMM_Q,
            dlarft: match self {
                Self::V1_18 => DlarftForm::Recursive,
                Self::V1_15 => DlarftForm::ColumnByColumn,
            },
        }
    }
}

/// `JOBU = JOBVT` of [`dgesvd`]: the vectors SciPy asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Job {
    /// `'S'`: the first `min(m, n)` columns of `U` and rows of `VT` (`full_matrices=False`).
    Some,
    /// `'A'`: all `m` columns of `U` and `n` rows of `VT` (`full_matrices=True`).
    All,
}

/// Why [`dgesvd`] or [`dbdsqr`] did not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DgesvdError {
    /// A parameter LAPACK's checks refuse (`XERBLA`'s message).
    #[error(transparent)]
    Illegal(#[from] IllegalValue),
    /// `A` holds a NaN or an infinity (module documentation).
    #[error("DGESVD: the matrix holds a non-finite value")]
    NotFinite,
    /// `DBDSQR` without vectors (`NCVT = NRU = NCC = 0`, `N >= 2`) takes `DLASQ1`, not ported.
    #[error("DBDSQR: singular values only (DLASQ1) are not ported")]
    ValuesOnlyNotPorted,
}

/// What [`dgesvd`] returns: LAPACK's `INFO` (0; `> 0`: `DBDSQR` did not converge, `INFO`
/// superdiagonals of the bidiagonal form did not reach zero) and `WORK(1)`, the optimal workspace;
/// `superdiag` holds `WORK(2:MIN(M, N))` (the unconverged superdiagonal, unscaled as LAPACK
/// does) when `INFO > 0`, else it is empty.
#[derive(Clone, Debug, PartialEq)]
pub struct Gesvd {
    /// LAPACK's `INFO`.
    pub info: usize,
    /// `WORK(1)` on return: [`dgesvd_lwork`]'s optimal workspace.
    pub work1: usize,
    /// `WORK(2:MIN(M, N))` when `INFO > 0`.
    pub superdiag: Vec<f64>,
}

/// `DGESVD`'s workspace sizes for `JOBU = JOBVT = job`: `MINWRK`, `MAXWRK` (`WORK(1)` of the
/// `LWORK = -1` query), the path's `WRKBL` (0 on paths 10 and 10t) and `MNTHR`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Query {
    minwrk: usize,
    maxwrk: usize,
    wrkbl: usize,
    mnthr: usize,
}

fn query(job: Job, m: usize, n: usize) -> Query {
    let minmn = m.min(n);
    if minmn == 0 {
        return Query {
            minwrk: 1,
            maxwrk: 1,
            wrkbl: 0,
            mnthr: 0,
        };
    }
    let all = job == Job::All;
    // ILAENV(6, 'DGESVD', JOBU // JOBVT, M, N, 0, 0) = INT(REAL(MIN(M, N))·1.6E0).
    let mnthr = gelsd_mnthr(m, n);
    let (minwrk, mut maxwrk, wrkbl);
    if m >= n {
        let bdspac = 5 * n;
        let geqrf = lwork_opt(m, n, false);
        let orgqr_n = org_lwork_opt(m, n, false);
        let orgqr_m = org_lwork_opt(m, m, false);
        let gebrd_nn = gebrd_lwork_opt(n, n);
        let orgbr_p = orgbr_lwork_opt(Vect::P, n, n, n);
        let orgbr_q = orgbr_lwork_opt(Vect::Q, n, n, n);
        if m >= mnthr {
            // Path 6 (JOBU = 'S', JOBVT = 'S') or path 9 (JOBU = 'A', JOBVT = 'A').
            wrkbl = (n + geqrf)
                .max(n + if all { orgqr_m } else { orgqr_n })
                .max(3 * n + gebrd_nn)
                .max(3 * n + orgbr_q)
                .max(3 * n + orgbr_p)
                .max(bdspac);
            maxwrk = n * n + wrkbl;
            minwrk = (3 * n + m).max(bdspac);
        } else {
            // Path 10 (M at least N, but not much larger).
            wrkbl = 0;
            maxwrk = 3 * n + gebrd_lwork_opt(m, n);
            let orgbr_q = orgbr_lwork_opt(Vect::Q, m, if all { m } else { n }, n);
            maxwrk = maxwrk.max(3 * n + orgbr_q);
            maxwrk = maxwrk.max(3 * n + orgbr_p);
            maxwrk = maxwrk.max(bdspac);
            minwrk = (3 * n + m).max(bdspac);
        }
    } else {
        let bdspac = 5 * m;
        let gelqf = lwork_opt(m, n, true);
        let orglq_n = org_lwork_opt(n, n, true);
        let orglq_m = org_lwork_opt(m, n, true);
        let gebrd_mm = gebrd_lwork_opt(m, m);
        let orgbr_p = orgbr_lwork_opt(Vect::P, m, m, m);
        let orgbr_q = orgbr_lwork_opt(Vect::Q, m, m, m);
        if n >= mnthr {
            // Path 6t (JOBU = 'S', JOBVT = 'S') or path 9t (JOBU = 'A', JOBVT = 'A').
            wrkbl = (m + gelqf)
                .max(m + if all { orglq_n } else { orglq_m })
                .max(3 * m + gebrd_mm)
                .max(3 * m + orgbr_p)
                .max(3 * m + orgbr_q)
                .max(bdspac);
            maxwrk = m * m + wrkbl;
            minwrk = (3 * m + n).max(bdspac);
        } else {
            // Path 10t (N greater than M, but not much larger).
            wrkbl = 0;
            maxwrk = 3 * m + gebrd_lwork_opt(m, n);
            let orgbr_p = orgbr_lwork_opt(Vect::P, if all { n } else { m }, n, m);
            maxwrk = maxwrk.max(3 * m + orgbr_p);
            maxwrk = maxwrk.max(3 * m + orgbr_q);
            maxwrk = maxwrk.max(bdspac);
            minwrk = (3 * m + n).max(bdspac);
        }
    }
    Query {
        minwrk,
        maxwrk: maxwrk.max(minwrk),
        wrkbl,
        mnthr,
    }
}

/// `DGESVD(JOB, JOB, M, N, ...)`'s workspace query: `(MINWRK, MAXWRK)`, the smallest workspace it
/// accepts and the optimal one (`WORK(1)` of the `LWORK = -1` query; both 1 for an empty matrix).
pub fn dgesvd_lwork(job: Job, m: usize, n: usize) -> (usize, usize) {
    let q = query(job, m, n);
    (q.minwrk, q.maxwrk)
}

#[inline(always)]
fn ix(ld: usize, i: usize, j: usize) -> usize {
    (i - 1) + (j - 1) * ld
}

/// `DLACPY('U', M, N, A, LDA, B, LDB)`: the upper triangle or trapezoid.
fn lacpy_upper(m: usize, n: usize, a: &[f64], lda: usize, b: &mut [f64], ldb: usize) {
    for j in 1..=n {
        for i in 1..=j.min(m) {
            b[ix(ldb, i, j)] = a[ix(lda, i, j)];
        }
    }
}

/// `DLACPY('L', M, N, A, LDA, B, LDB)`: the lower triangle or trapezoid.
fn lacpy_lower(m: usize, n: usize, a: &[f64], lda: usize, b: &mut [f64], ldb: usize) {
    for j in 1..=n {
        for i in j..=m {
            b[ix(ldb, i, j)] = a[ix(lda, i, j)];
        }
    }
}

/// `DLASET('L', N-1, N-1, 0, 0, A(2, 1), LDA)`: zeros below the diagonal of the `n × n` block.
fn zero_below(n: usize, a: &mut [f64], lda: usize) {
    for j in 1..n {
        for i in j + 1..=n {
            a[ix(lda, i, j)] = 0.0;
        }
    }
}

/// `DLASET('U', M-1, M-1, 0, 0, A(1, 2), LDA)`: zeros above the diagonal of the `m × m` block.
fn zero_above(m: usize, a: &mut [f64], lda: usize) {
    for j in 2..=m {
        for i in 1..j {
            a[ix(lda, i, j)] = 0.0;
        }
    }
}

/// `DGEMM('N', 'N', M, N, K, 1, A, LDA, B, LDB, 0, C, LDC)` on blocks at offset 0.
fn gemm_nn(
    lp: Lapack,
    m: usize,
    n: usize,
    k: usize,
    a: &[f64],
    lda: usize,
    b: &[f64],
    ldb: usize,
    c: &mut [f64],
    ldc: usize,
) {
    dgemm_with(
        m,
        n,
        k,
        1.0,
        Operand {
            data: a,
            off: 0,
            ld: lda,
            trans: Trans::N,
        },
        Operand {
            data: b,
            off: 0,
            ld: ldb,
            trans: Trans::N,
        },
        0.0,
        c,
        0,
        ldc,
        lp.gemm_q,
    );
}

/// `DBDSQR(UPLO, N, NCVT, NRU, NCC, D, E, VT, LDVT, U, LDU, C, LDC, WORK, INFO)` of LAPACK 3.12.0
/// (`release` 3.12.1 makes a `-0` singular value `+0`): the singular values of the `n × n`
/// bidiagonal matrix (`d`: `n` values, `e`: `n - 1`, overwritten) descending in `d`, `VT`
/// (`ncvt` columns, leading dimension `ldvt`) premultiplied by `Pᵀ`, `U` (`nru` rows, `ldu`)
/// postmultiplied by `Q`, `C` (`ncc` columns, `ldc`) premultiplied by `Qᵀ`, each block at offset
/// 0. A lower bidiagonal matrix is first rotated to upper (module documentation). Returns `INFO`
/// (0, or the number of superdiagonals that did not converge).
///
/// # Errors
/// `N`, `NCVT`, `NRU`, `NCC` below zero cannot arise; `LDVT < 1` or `< max(1, N)` with vectors
/// (9), `LDU < max(1, NRU)` (11), `LDC` as `LDVT` (13) as LAPACK; no vectors with `N >= 2`
/// ([`DgesvdError::ValuesOnlyNotPorted`]).
pub fn dbdsqr(
    release: LapackRelease,
    uplo: Uplo,
    n: usize,
    ncvt: usize,
    nru: usize,
    ncc: usize,
    d: &mut [f64],
    e: &mut [f64],
    (vt, ldvt): (&mut [f64], usize),
    (u, ldu): (&mut [f64], usize),
    (c, ldc): (&mut [f64], usize),
) -> Result<usize, DgesvdError> {
    let err = |param| {
        Err(DgesvdError::Illegal(IllegalValue {
            routine: "DBDSQR",
            param,
        }))
    };
    if (ncvt == 0 && ldvt < 1) || (ncvt > 0 && ldvt < n.max(1)) {
        return err(9);
    }
    if ldu < nru.max(1) {
        return err(11);
    }
    if (ncc == 0 && ldc < 1) || (ncc > 0 && ldc < n.max(1)) {
        return err(13);
    }
    if n == 0 {
        return Ok(0);
    }
    let rotate = ncvt > 0 || nru > 0 || ncc > 0;
    if n > 1 && !rotate {
        return Err(DgesvdError::ValuesOnlyNotPorted);
    }
    if n > 1 && uplo == Uplo::Lower {
        // Rotate to upper bidiagonal by Givens rotations on the left.
        let nm1 = n - 1;
        let mut cs = vec![0.0f64; nm1];
        let mut sn = vec![0.0f64; nm1];
        for i in 1..n {
            let (c1, s1, r) = dlartg(d[i - 1], e[i - 1]);
            d[i - 1] = r;
            e[i - 1] = s1 * d[i];
            d[i] = c1 * d[i];
            cs[i - 1] = c1;
            sn[i - 1] = s1;
        }
        // Update singular vectors if desired.
        if nru > 0 {
            dlasr(false, true, nru, n, &cs, &sn, u, 0, ldu);
        }
        if ncc > 0 {
            dlasr(true, true, n, ncc, &cs, &sn, c, 0, ldc);
        }
    }
    Ok(dbdsqr_upper_ncc(
        n,
        d,
        e,
        (vt, ncvt, ldvt),
        (u, nru, ldu),
        (c, ncc, ldc),
        release == LapackRelease::V3_12_1,
    ))
}

/// `DGESVD(JOB, JOB, M, N, A, LDA, S, U, LDU, VT, LDVT, WORK, LWORK, INFO)`: the SVD `A =
/// U·diag(S)·VT` of the column-major `m × n` matrix `a` (`a[i + j·lda]`, overwritten), the
/// `min(m, n)` singular values descending in `s`, `U` (`m × min(m, n)` for [`Job::Some`], `m ×
/// m` for [`Job::All`]; leading dimension `ldu`) and `VT` (`min(m, n) × n` or `n × n`; `ldvt`)
/// written into `u` and `vt` (their entries outside those blocks are kept), as LAPACK computes
/// them with the settings `lp` ([`ScipyLibrary::lapack`] for SciPy's libraries; module
/// documentation). `lwork` is the workspace the caller gives (`None`: the optimal, as SciPy's
/// query gives it). Returns `INFO`, `WORK(1)` and the unconverged superdiagonal ([`Gesvd`]).
///
/// # Errors
/// `LDA < max(1, M)` (6), `LDU < max(1, M)` (9), `LDVT < max(1, min(M, N))` (`'S'`) or `< max(1,
/// N)` (`'A'`) (11), `LWORK < MINWRK` (13) as LAPACK ([`DgesvdError::Illegal`]); a non-finite
/// entry ([`DgesvdError::NotFinite`]).
pub fn dgesvd(
    lp: Lapack,
    job: Job,
    m: usize,
    n: usize,
    a: &mut [f64],
    lda: usize,
    s: &mut [f64],
    u: &mut [f64],
    ldu: usize,
    vt: &mut [f64],
    ldvt: usize,
    lwork: Option<usize>,
) -> Result<Gesvd, DgesvdError> {
    let minmn = m.min(n);
    let all = job == Job::All;
    let err = |param| {
        Err(DgesvdError::Illegal(IllegalValue {
            routine: "DGESVD",
            param,
        }))
    };
    if lda < m.max(1) {
        return err(6);
    }
    if ldu < 1 || ldu < m {
        return err(9);
    }
    if ldvt < 1 || (all && ldvt < n) || (!all && ldvt < minmn) {
        return err(11);
    }
    let q = query(job, m, n);
    let lwork = lwork.unwrap_or(q.maxwrk);
    if lwork < q.minwrk {
        return err(13);
    }
    let done = |info: usize, superdiag: Vec<f64>| {
        Ok(Gesvd {
            info,
            work1: q.maxwrk,
            superdiag,
        })
    };
    // Quick return if possible.
    if m == 0 || n == 0 {
        return done(0, Vec::new());
    }
    // Get machine constants.
    let eps = 2.0 * EPS; // DLAMCH('P')
    let smlnum = SAFMIN.sqrt() / eps;
    let bignum = 1.0 / smlnum;
    // Scale A if max element outside range [SMLNUM, BIGNUM].
    let anrm = dlange_m(m, n, a, lda);
    if !anrm.is_finite() {
        return Err(DgesvdError::NotFinite);
    }
    let mut iscl = false;
    if anrm > 0.0 && anrm < smlnum {
        iscl = true;
        dlascl_g(anrm, smlnum, m, n, a, 0, lda)?;
    } else if anrm > bignum {
        iscl = true;
        dlascl_g(anrm, bignum, m, n, a, 0, lda)?;
    }
    let mut e = vec![0.0f64; minmn];
    let mut tau = vec![0.0f64; minmn];
    let mut tauq = vec![0.0f64; minmn];
    let mut taup = vec![0.0f64; minmn];
    let rel = lp.release;
    let none = || (Vec::<f64>::new(), 1usize);
    let info;
    if m >= n {
        if m >= q.mnthr {
            // Path 6 (JOBU = JOBVT = 'S') or path 9 ('A'): N right singular vectors in VT, N
            // (path 6) or M (path 9) left singular vectors in U.
            let fast = if all {
                lwork >= n * n + (n + m).max(4 * n).max(5 * n)
            } else {
                lwork >= n * n + (4 * n).max(5 * n)
            };
            if fast {
                // Sufficient workspace for a fast algorithm: WORK(IU) is LDA by N or N by N.
                let ldwrku = if lwork >= q.wrkbl + lda * n { lda } else { n };
                let mut wu = vec![0.0f64; ldwrku * n];
                // IWORK = ITAU + N, ITAU = IU + LDWRKU·N.
                let lw = lwork - (ldwrku * n + n);
                // Compute A = Q·R.
                dgeqrf(lp, m, n, a, 0, lda, &mut tau, Some(lw))?;
                if all {
                    // Copy the result to U and generate Q (M by M) in U.
                    lacpy_lower(m, n, a, lda, u, ldu);
                    dorgqr(lp, m, m, n, u, 0, ldu, &tau, Some(lw))?;
                }
                // Copy R to WORK(IU), zeroing out below it.
                lacpy_upper(n, n, a, lda, &mut wu, ldwrku);
                zero_below(n, &mut wu, ldwrku);
                if !all {
                    // Generate Q in A.
                    dorgqr(lp, m, n, n, a, 0, lda, &tau, Some(lw))?;
                }
                // IE = ITAU, ITAUQ = IE + N, ITAUP = ITAUQ + N, IWORK = ITAUP + N.
                let lw = lwork - (ldwrku * n + 3 * n);
                // Bidiagonalize R in WORK(IU), copying the result to VT.
                dgebrd(
                    lp,
                    n,
                    n,
                    &mut wu,
                    0,
                    ldwrku,
                    s,
                    &mut e,
                    &mut tauq,
                    &mut taup,
                    Some(lw),
                )?;
                lacpy_upper(n, n, &wu, ldwrku, vt, ldvt);
                // Generate left bidiagonalizing vectors in WORK(IU), right ones in VT.
                dorgbr(lp, Vect::Q, n, n, n, &mut wu, 0, ldwrku, &tauq, Some(lw))?;
                dorgbr(lp, Vect::P, n, n, n, vt, 0, ldvt, &taup, Some(lw))?;
                // Bidiagonal QR iteration: left singular vectors of R in WORK(IU), right ones
                // in VT.
                let (mut c, ldc) = none();
                info = dbdsqr(
                    rel,
                    Uplo::Upper,
                    n,
                    n,
                    n,
                    0,
                    s,
                    &mut e,
                    (vt, ldvt),
                    (&mut wu, ldwrku),
                    (&mut c, ldc),
                )?;
                if all {
                    // Multiply Q in U by the left singular vectors of R in WORK(IU), storing
                    // the result in A, then copy it to U.
                    gemm_nn(lp, m, n, n, u, ldu, &wu, ldwrku, a, lda);
                    dlacpy(m, n, a, 0, lda, u, 0, ldu);
                } else {
                    // Multiply Q in A by the left singular vectors of R in WORK(IU), storing
                    // the result in U.
                    gemm_nn(lp, m, n, n, a, lda, &wu, ldwrku, u, ldu);
                }
            } else {
                // Insufficient workspace for a fast algorithm. IWORK = ITAU + N, ITAU = 1.
                let lw = lwork - n;
                // Compute A = Q·R, copying the result to U.
                dgeqrf(lp, m, n, a, 0, lda, &mut tau, Some(lw))?;
                lacpy_lower(m, n, a, lda, u, ldu);
                // Generate Q in U.
                dorgqr(lp, m, if all { m } else { n }, n, u, 0, ldu, &tau, Some(lw))?;
                // Copy R to VT, zeroing out below it.
                lacpy_upper(n, n, a, lda, vt, ldvt);
                if n > 1 {
                    zero_below(n, vt, ldvt);
                }
                // IE = ITAU, ITAUQ = IE + N, ITAUP = ITAUQ + N, IWORK = ITAUP + N.
                let lw = lwork - 3 * n;
                // Bidiagonalize R in VT.
                dgebrd(
                    lp,
                    n,
                    n,
                    vt,
                    0,
                    ldvt,
                    s,
                    &mut e,
                    &mut tauq,
                    &mut taup,
                    Some(lw),
                )?;
                // Multiply Q in U by the left bidiagonalizing vectors in VT.
                dormbr(
                    lp,
                    Vect::Q,
                    Side::Right,
                    Trans::N,
                    m,
                    n,
                    n,
                    vt,
                    0,
                    ldvt,
                    &tauq,
                    u,
                    0,
                    ldu,
                    Some(lw),
                )?;
                // Generate right bidiagonalizing vectors in VT.
                dorgbr(lp, Vect::P, n, n, n, vt, 0, ldvt, &taup, Some(lw))?;
                // Bidiagonal QR iteration: left singular vectors of A in U, right ones in VT.
                let (mut c, ldc) = none();
                info = dbdsqr(
                    rel,
                    Uplo::Upper,
                    n,
                    n,
                    m,
                    0,
                    s,
                    &mut e,
                    (vt, ldvt),
                    (u, ldu),
                    (&mut c, ldc),
                )?;
            }
        } else {
            // Path 10 (M at least N, but not much larger): reduce to bidiagonal form without
            // QR decomposition. IE = 1, ITAUQ = IE + N, ITAUP = ITAUQ + N, IWORK = ITAUP + N.
            let lw = lwork - 3 * n;
            dgebrd(
                lp,
                m,
                n,
                a,
                0,
                lda,
                s,
                &mut e,
                &mut tauq,
                &mut taup,
                Some(lw),
            )?;
            // Copy the result to U and generate the left bidiagonalizing vectors in U.
            lacpy_lower(m, n, a, lda, u, ldu);
            let ncu = if all { m } else { n };
            dorgbr(lp, Vect::Q, m, ncu, n, u, 0, ldu, &tauq, Some(lw))?;
            // Copy the result to VT and generate the right bidiagonalizing vectors in VT.
            lacpy_upper(n, n, a, lda, vt, ldvt);
            dorgbr(lp, Vect::P, n, n, n, vt, 0, ldvt, &taup, Some(lw))?;
            // Bidiagonal QR iteration: left singular vectors in U, right ones in VT.
            let (mut c, ldc) = none();
            info = dbdsqr(
                rel,
                Uplo::Upper,
                n,
                n,
                m,
                0,
                s,
                &mut e,
                (vt, ldvt),
                (u, ldu),
                (&mut c, ldc),
            )?;
        }
    } else if n >= q.mnthr {
        // Path 6t (JOBU = JOBVT = 'S') or path 9t ('A'): M left singular vectors in U, M
        // (path 6t) or N (path 9t) right singular vectors in VT.
        let fast = if all {
            lwork >= m * m + (n + m).max(4 * m).max(5 * m)
        } else {
            lwork >= m * m + (4 * m).max(5 * m)
        };
        if fast {
            // Sufficient workspace for a fast algorithm: WORK(IU) is LDA by M or M by M.
            let ldwrku = if lwork >= q.wrkbl + lda * m { lda } else { m };
            let mut wu = vec![0.0f64; ldwrku * m];
            // IWORK = ITAU + M, ITAU = IU + LDWRKU·M.
            let lw = lwork - (ldwrku * m + m);
            // Compute A = L·Q.
            dgelqf(lp, m, n, a, 0, lda, &mut tau, Some(lw))?;
            if all {
                // Copy the result to VT and generate Q (N by N) in VT.
                lacpy_upper(m, n, a, lda, vt, ldvt);
                dorglq(lp, n, n, m, vt, 0, ldvt, &tau, Some(lw))?;
            }
            // Copy L to WORK(IU), zeroing out above it.
            lacpy_lower(m, m, a, lda, &mut wu, ldwrku);
            zero_above(m, &mut wu, ldwrku);
            if !all {
                // Generate Q in A.
                dorglq(lp, m, n, m, a, 0, lda, &tau, Some(lw))?;
            }
            // IE = ITAU, ITAUQ = IE + M, ITAUP = ITAUQ + M, IWORK = ITAUP + M.
            let lw = lwork - (ldwrku * m + 3 * m);
            // Bidiagonalize L in WORK(IU), copying the result to U.
            dgebrd(
                lp,
                m,
                m,
                &mut wu,
                0,
                ldwrku,
                s,
                &mut e,
                &mut tauq,
                &mut taup,
                Some(lw),
            )?;
            lacpy_lower(m, m, &wu, ldwrku, u, ldu);
            // Generate right bidiagonalizing vectors in WORK(IU), left ones in U.
            dorgbr(lp, Vect::P, m, m, m, &mut wu, 0, ldwrku, &taup, Some(lw))?;
            dorgbr(lp, Vect::Q, m, m, m, u, 0, ldu, &tauq, Some(lw))?;
            // Bidiagonal QR iteration: left singular vectors of L in U, right ones in WORK(IU).
            let (mut c, ldc) = none();
            info = dbdsqr(
                rel,
                Uplo::Upper,
                m,
                m,
                m,
                0,
                s,
                &mut e,
                (&mut wu, ldwrku),
                (u, ldu),
                (&mut c, ldc),
            )?;
            if all {
                // Multiply the right singular vectors of L in WORK(IU) by Q in VT, storing the
                // result in A, then copy it to VT.
                gemm_nn(lp, m, n, m, &wu, ldwrku, vt, ldvt, a, lda);
                dlacpy(m, n, a, 0, lda, vt, 0, ldvt);
            } else {
                // Multiply the right singular vectors of L in WORK(IU) by Q in A, storing the
                // result in VT.
                gemm_nn(lp, m, n, m, &wu, ldwrku, a, lda, vt, ldvt);
            }
        } else {
            // Insufficient workspace for a fast algorithm. IWORK = ITAU + M, ITAU = 1.
            let lw = lwork - m;
            // Compute A = L·Q, copying the result to VT.
            dgelqf(lp, m, n, a, 0, lda, &mut tau, Some(lw))?;
            lacpy_upper(m, n, a, lda, vt, ldvt);
            // Generate Q in VT.
            dorglq(
                lp,
                if all { n } else { m },
                n,
                m,
                vt,
                0,
                ldvt,
                &tau,
                Some(lw),
            )?;
            // Copy L to U, zeroing out above it.
            lacpy_lower(m, m, a, lda, u, ldu);
            zero_above(m, u, ldu);
            // IE = ITAU, ITAUQ = IE + M, ITAUP = ITAUQ + M, IWORK = ITAUP + M.
            let lw = lwork - 3 * m;
            // Bidiagonalize L in U.
            dgebrd(
                lp,
                m,
                m,
                u,
                0,
                ldu,
                s,
                &mut e,
                &mut tauq,
                &mut taup,
                Some(lw),
            )?;
            // Multiply the right bidiagonalizing vectors in U by Q in VT.
            dormbr(
                lp,
                Vect::P,
                Side::Left,
                Trans::T,
                m,
                n,
                m,
                u,
                0,
                ldu,
                &taup,
                vt,
                0,
                ldvt,
                Some(lw),
            )?;
            // Generate left bidiagonalizing vectors in U.
            dorgbr(lp, Vect::Q, m, m, m, u, 0, ldu, &tauq, Some(lw))?;
            // Bidiagonal QR iteration: left singular vectors of A in U, right ones in VT.
            let (mut c, ldc) = none();
            info = dbdsqr(
                rel,
                Uplo::Upper,
                m,
                n,
                m,
                0,
                s,
                &mut e,
                (vt, ldvt),
                (u, ldu),
                (&mut c, ldc),
            )?;
        }
    } else {
        // Path 10t (N greater than M, but not much larger): reduce to bidiagonal form without
        // LQ decomposition. IE = 1, ITAUQ = IE + M, ITAUP = ITAUQ + M, IWORK = ITAUP + M.
        let lw = lwork - 3 * m;
        dgebrd(
            lp,
            m,
            n,
            a,
            0,
            lda,
            s,
            &mut e,
            &mut tauq,
            &mut taup,
            Some(lw),
        )?;
        // Copy the result to U and generate the left bidiagonalizing vectors in U.
        lacpy_lower(m, m, a, lda, u, ldu);
        dorgbr(lp, Vect::Q, m, m, n, u, 0, ldu, &tauq, Some(lw))?;
        // Copy the result to VT and generate the right bidiagonalizing vectors in VT.
        lacpy_upper(m, n, a, lda, vt, ldvt);
        let nrvt = if all { n } else { m };
        dorgbr(lp, Vect::P, nrvt, n, m, vt, 0, ldvt, &taup, Some(lw))?;
        // Bidiagonal QR iteration: left singular vectors in U, right ones in VT.
        let (mut c, ldc) = none();
        info = dbdsqr(
            rel,
            Uplo::Lower,
            m,
            n,
            m,
            0,
            s,
            &mut e,
            (vt, ldvt),
            (u, ldu),
            (&mut c, ldc),
        )?;
    }
    // If DBDSQR failed to converge, the unconverged superdiagonals go to WORK(2:MINMN).
    let mut superdiag = if info != 0 {
        e[..minmn - 1].to_vec()
    } else {
        Vec::new()
    };
    // Undo scaling if necessary.
    if iscl {
        if anrm > bignum {
            dlascl_g(bignum, anrm, minmn, 1, s, 0, minmn)?;
        }
        if info != 0 && anrm > bignum {
            dlascl_g(bignum, anrm, minmn - 1, 1, &mut superdiag, 0, minmn)?;
        }
        if anrm < smlnum {
            dlascl_g(smlnum, anrm, minmn, 1, s, 0, minmn)?;
        }
        if info != 0 && anrm < smlnum {
            dlascl_g(smlnum, anrm, minmn - 1, 1, &mut superdiag, 0, minmn)?;
        }
    }
    done(info, superdiag)
}

// ------------------------------------------------------------------------------------------
// SciPy's svd with the gesvd driver

/// Why [`scipy_svd`] (and so [`nipype_fallback_svd`], [`nipype_full_rank`]) raised.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScipySvdError {
    /// `check_finite`: numpy's `asarray_chkfinite` (`ValueError`).
    #[error("array must not contain infs or NaNs")]
    NotFinite,
    /// A factor too large for SciPy's 32-bit LAPACK (`ValueError`; SciPy's message, `message`).
    #[error("{message}")]
    TooLarge {
        /// SciPy's text.
        message: String,
    },
    /// `DGESVD`'s `INFO > 0` (`LinAlgError`; SciPy's message for the library's version).
    #[error("{}", match .0 { ScipyLibrary::V1_18 => "SVD did not converge for slice = 0.", ScipyLibrary::V1_15 => "SVD did not converge" })]
    DidNotConverge(ScipyLibrary),
    /// numpy's `ValueError` of `max()` on the empty `s` ([`nipype_full_rank`] only).
    #[error("zero-size array to reduction operation maximum which has no identity")]
    EmptyReduction,
}

impl ScipySvdError {
    /// Whether SciPy (or numpy) raises it as `LinAlgError` (else `ValueError`).
    pub fn is_linalg_error(&self) -> bool {
        matches!(self, Self::DidNotConverge(_))
    }
}

/// The SVD `a = u · diag(s) · vt` in SciPy's layout (C order).
#[derive(Clone, Debug, PartialEq)]
pub struct ScipySvd {
    /// `U`: `m × ucols` (`ucols = m` with `full_matrices`, else `min(m, n)`), `u[i·ucols + j]`.
    pub u: Vec<f64>,
    /// Columns of `U`.
    pub ucols: usize,
    /// The `min(m, n)` singular values, descending.
    pub s: Vec<f64>,
    /// `VT`: `vtrows × n` (`vtrows = n` with `full_matrices`, else `min(m, n)`), `vt[i·n + j]`.
    pub vt: Vec<f64>,
    /// Rows of `VT`.
    pub vtrows: usize,
}

fn too_large(lib: ScipyLibrary, m: usize, n: usize, full: bool) -> Option<String> {
    const INT32_MAX: usize = i32::MAX as usize;
    let (max_mn, min_mn) = if m > n { (m, n) } else { (n, m) };
    if full {
        if max_mn * max_mn > INT32_MAX {
            return Some(match lib {
                ScipyLibrary::V1_15 => format!(
                    "Indexing a matrix size {max_mn} x {max_mn} would incur integer overflow in \
                     LAPACK. Try using numpy.linalg.svd instead."
                ),
                ScipyLibrary::V1_18 => format!(
                    "Indexing a matrix size {max_mn} x {max_mn} would incur integer overflow in \
                     LAPACK. Instead, either use using numpy.linalg.svd or buildSciPy with ILP64 \
                     support."
                ),
            });
        }
    } else {
        let sz = (m * min_mn).max(n * min_mn);
        if sz > INT32_MAX {
            return Some(match lib {
                ScipyLibrary::V1_15 => format!(
                    "Indexing a matrix of {sz} elements would incur an in integer overflow in \
                     LAPACK. Try using numpy.linalg.svd instead."
                ),
                ScipyLibrary::V1_18 => format!(
                    "Indexing a matrix of {sz} elements would incur an in integer overflow in \
                     LAPACK. Instead, either use using numpy.linalg.svd or buildSciPy with ILP64 \
                     support."
                ),
            });
        }
    }
    None
}

/// `scipy.linalg.svd(a, full_matrices, compute_uv=True, lapack_driver='gesvd')` of the float64
/// matrix `a` (any strides) with SciPy `lib`'s library (module documentation).
///
/// # Errors
/// [`ScipySvdError::NotFinite`], [`ScipySvdError::TooLarge`], [`ScipySvdError::DidNotConverge`]
/// as SciPy raises them.
pub fn scipy_svd(
    lib: ScipyLibrary,
    a: Array2<'_>,
    full_matrices: bool,
) -> Result<ScipySvd, ScipySvdError> {
    let (m, n) = (a.rows, a.cols);
    let k = m.min(n);
    // check_finite (numpy's asarray_chkfinite).
    for j in 0..n {
        for i in 0..m {
            if !a.data[a.off + i * a.s0 + j * a.s1].is_finite() {
                return Err(ScipySvdError::NotFinite);
            }
        }
    }
    let (ucols, vtrows) = if full_matrices { (m, n) } else { (k, k) };
    // Accommodate an empty matrix.
    if m == 0 || n == 0 {
        let eye = |r: usize, c: usize| {
            let mut x = vec![0.0f64; r * c];
            for i in 0..r.min(c) {
                x[i * c + i] = 1.0;
            }
            x
        };
        return Ok(ScipySvd {
            u: eye(m, ucols),
            ucols,
            s: Vec::new(),
            vt: eye(vtrows, n),
            vtrows,
        });
    }
    if let Some(message) = too_large(lib, m, n, full_matrices) {
        return Err(ScipySvdError::TooLarge { message });
    }
    let job = if full_matrices { Job::All } else { Job::Some };
    let ldu = m;
    let ldvt = vtrows;
    // Copy the matrix in (Fortran order, LDA = M).
    let mut fa = vec![0.0f64; m * n];
    for j in 0..n {
        for i in 0..m {
            fa[i + j * m] = a.data[a.off + i * a.s0 + j * a.s1];
        }
    }
    let mut s = vec![0.0f64; k];
    let mut fu = vec![0.0f64; ldu * ucols];
    let mut fvt = vec![0.0f64; ldvt * n];
    // The query's WORK(1), truncated to an integer, is LWORK.
    let lwork = dgesvd_lwork(job, m, n).1;
    let out = dgesvd(
        lib.lapack(),
        job,
        m,
        n,
        &mut fa,
        m,
        &mut s,
        &mut fu,
        ldu,
        &mut fvt,
        ldvt,
        Some(lwork),
    );
    match out {
        Ok(g) if g.info == 0 => {}
        Ok(_) => return Err(ScipySvdError::DidNotConverge(lib)),
        // The arguments are always legal and the matrix finite here.
        Err(e) => unreachable!("scipy_svd: {e}"),
    }
    // Copy the results out (C order).
    let mut u = vec![0.0f64; m * ucols];
    for i in 0..m {
        for j in 0..ucols {
            u[i * ucols + j] = fu[i + j * ldu];
        }
    }
    let mut vt = vec![0.0f64; vtrows * n];
    for i in 0..vtrows {
        for j in 0..n {
            vt[i * n + j] = fvt[i + j * ldvt];
        }
    }
    Ok(ScipySvd {
        u,
        ucols,
        s,
        vt,
        vtrows,
    })
}

// ------------------------------------------------------------------------------------------
// nipype's callers

/// nipype's `fallback_svd(a, full_matrices=False)`: `numpy.linalg.svd(a, full_matrices=False)`
/// with numpy's LAPACK `numpy` ([`super::svd::numpy_svd`]), and on its `LinAlgError`
/// `scipy.linalg.svd(a, full_matrices=False, lapack_driver='gesvd')` with SciPy `scipy`'s
/// library ([`scipy_svd`]). Returns the SVD and whether SciPy's fallback computed it.
///
/// # Errors
/// SciPy's errors ([`ScipySvdError`]): a non-finite `a` ends there (numpy raises `LinAlgError`
/// for it, SciPy `ValueError`).
pub fn nipype_fallback_svd(
    numpy: Lapack,
    scipy: ScipyLibrary,
    a: Array2<'_>,
) -> Result<(Svd, bool), ScipySvdError> {
    if let Ok(svd) = numpy_svd(numpy, a) {
        return Ok((svd, false));
    }
    let r = scipy_svd(scipy, a, false)?;
    Ok((
        Svd {
            k: r.ucols,
            u: r.u,
            s: r.s,
            vt: r.vt,
        },
        true,
    ))
}

/// What [`nipype_full_rank`] returns.
#[derive(Clone, Debug, PartialEq)]
pub struct FullRank {
    /// `X`, `m × n`, C order: the input's values, or the regularized matrix.
    pub x: Vec<f64>,
    /// The condition number `max(s) / min(s)`, or `cmax` when regularized.
    pub c: f64,
    /// Whether `X` was regularized (nipype then logs "Matrix is singular at working precision,
    /// regularizing...").
    pub regularized: bool,
}

/// nipype's `_full_rank(X, cmax)` (module documentation) of the float64 matrix `x` (any strides),
/// with numpy's LAPACK `numpy` and SciPy `scipy`'s library for [`nipype_fallback_svd`].
///
/// # Errors
/// [`nipype_fallback_svd`]'s; [`ScipySvdError::EmptyReduction`] when `x` has no rows or columns
/// (`s.max()` of an empty array).
pub fn nipype_full_rank(
    numpy: Lapack,
    scipy: ScipyLibrary,
    x: Array2<'_>,
    cmax: f64,
) -> Result<FullRank, ScipySvdError> {
    let (m, n) = (x.rows, x.cols);
    let input = || {
        let mut v = vec![0.0f64; m * n];
        for i in 0..m {
            for j in 0..n {
                v[i * n + j] = x.data[x.off + i * x.s0 + j * x.s1];
            }
        }
        v
    };
    let (svd, _) = nipype_fallback_svd(numpy, scipy, x)?;
    let Svd { k, u, s, vt } = svd;
    if k == 0 {
        return Err(ScipySvdError::EmptyReduction);
    }
    // numpy's max and min (s holds no NaN).
    let smax = s.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let smin = s.iter().copied().fold(f64::INFINITY, f64::min);
    let c = smax / smin;
    if c < cmax {
        return Ok(FullRank {
            x: input(),
            c,
            regularized: false,
        });
    }
    let lda = (smax - cmax * smin) / (cmax - 1.0);
    // np.dot(np.diag(s + lda), V): k × n.
    let mut diag = vec![0.0f64; k * k];
    for i in 0..k {
        diag[i * k + i] = s[i] + lda;
    }
    let dv = numpy_matmul_with(
        Array2 {
            data: &diag,
            off: 0,
            rows: k,
            cols: k,
            s0: k,
            s1: 1,
        },
        Array2 {
            data: &vt,
            off: 0,
            rows: k,
            cols: n,
            s0: n,
            s1: 1,
        },
        numpy.gemm_q,
    );
    // np.dot(U, ·): m × n.
    let xr = numpy_matmul_with(
        Array2 {
            data: &u,
            off: 0,
            rows: m,
            cols: k,
            s0: k,
            s1: 1,
        },
        Array2 {
            data: &dv,
            off: 0,
            rows: k,
            cols: n,
            s0: n,
            s1: 1,
        },
        numpy.gemm_q,
    );
    Ok(FullRank {
        x: xr,
        c: cmax,
        regularized: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// `u·diag(s)·vt` rebuilds `a`; `u` and `vt` orthonormal (all their columns and rows with
    /// `full_matrices`); the values descend.
    fn check(a: &[f64], m: usize, n: usize, r: &ScipySvd) {
        let k = m.min(n);
        assert!(r.s.windows(2).all(|w| w[0] >= w[1]));
        let mut worst = 0.0f64;
        for i in 0..m {
            for j in 0..n {
                let mut acc = 0.0;
                for t in 0..k {
                    acc += r.u[i * r.ucols + t] * r.s[t] * r.vt[t * n + j];
                }
                worst = worst.max((acc - a[i * n + j]).abs());
            }
        }
        assert!(worst < 1e-12, "{m}x{n}: {worst}");
        for p in 0..r.ucols {
            for q in 0..r.ucols {
                let uu: f64 = (0..m)
                    .map(|i| r.u[i * r.ucols + p] * r.u[i * r.ucols + q])
                    .sum();
                assert!((uu - if p == q { 1.0 } else { 0.0 }).abs() < 1e-12);
            }
        }
        for p in 0..r.vtrows {
            for q in 0..r.vtrows {
                let vv: f64 = (0..n).map(|j| r.vt[p * n + j] * r.vt[q * n + j]).sum();
                assert!((vv - if p == q { 1.0 } else { 0.0 }).abs() < 1e-12);
            }
        }
    }

    /// Every path (6, 9, 10, 6t, 9t, 10t), blocked and unblocked, in both libraries.
    #[test]
    fn rebuilds_every_path() {
        for lib in [ScipyLibrary::V1_18, ScipyLibrary::V1_15] {
            for &(m, n) in &[
                (1, 1),
                (7, 1),
                (1, 7),
                (40, 3),
                (30, 20),
                (26, 25),
                (300, 140),
                (150, 140),
                (3, 40),
                (20, 30),
                (140, 300),
                (140, 150),
            ] {
                let a = matrix(m, n, (m * 1000 + n) as u64);
                for full in [false, true] {
                    let r = scipy_svd(lib, view(&a, m, n), full).unwrap();
                    check(&a, m, n, &r);
                }
            }
        }
    }

    /// The workspace branches (fast with `LDWRKU = LDA` or the short one, the slow algorithm)
    /// give the same factorization up to rounding, and the bits never depend on the threads.
    #[test]
    fn workspaces_and_threads() {
        let lp = ScipyLibrary::V1_18.lapack();
        for &(m, n) in &[(90, 40), (40, 90)] {
            let a = matrix(m, n, 11);
            let mut fa = vec![0.0f64; m * n];
            for j in 0..n {
                for i in 0..m {
                    fa[i + j * m] = a[i * n + j];
                }
            }
            for job in [Job::Some, Job::All] {
                let (minwrk, maxwrk) = dgesvd_lwork(job, m, n);
                let k = m.min(n);
                let (uc, vr) = if job == Job::All { (m, n) } else { (k, k) };
                let run = |lwork: usize, threads: usize| {
                    let (mut b, mut s) = (fa.clone(), vec![0.0f64; k]);
                    let (mut u, mut vt) = (vec![0.0f64; m * uc], vec![0.0f64; vr * n]);
                    larmorx_core::parallel::with_threads(threads, || {
                        dgesvd(
                            lp,
                            job,
                            m,
                            n,
                            &mut b,
                            m,
                            &mut s,
                            &mut u,
                            m,
                            &mut vt,
                            vr,
                            Some(lwork),
                        )
                        .unwrap()
                    })
                    .unwrap();
                    (s, u, vt)
                };
                let base = run(maxwrk, 1);
                assert_eq!(base, run(maxwrk, 5));
                for lwork in [minwrk, maxwrk + m * n, (minwrk + maxwrk) / 2] {
                    let (s, _, _) = run(lwork, 1);
                    for (x, y) in s.iter().zip(&base.0) {
                        assert!((x - y).abs() < 1e-13);
                    }
                }
            }
        }
    }

    /// A lower bidiagonal matrix and its transpose have the same singular values; `DBDSQR`'s
    /// refusals.
    #[test]
    fn dbdsqr_lower_and_refusals() {
        let n = 9;
        let d0 = matrix(n, 1, 3);
        let e0 = matrix(n - 1, 1, 4);
        let solve = |uplo| {
            let (mut d, mut e) = (d0.clone(), e0.clone());
            let mut vt = vec![0.0f64; n * n];
            let mut u = vec![0.0f64; n * n];
            for i in 0..n {
                vt[i + i * n] = 1.0;
                u[i + i * n] = 1.0;
            }
            let info = dbdsqr(
                LapackRelease::V3_12_0,
                uplo,
                n,
                n,
                n,
                0,
                &mut d,
                &mut e,
                (&mut vt, n),
                (&mut u, n),
                (&mut [], 1),
            )
            .unwrap();
            assert_eq!(info, 0);
            d
        };
        for (x, y) in solve(Uplo::Upper).iter().zip(&solve(Uplo::Lower)) {
            assert!((x - y).abs() < 1e-13);
        }
        let (mut d, mut e) = (d0.clone(), e0.clone());
        assert_eq!(
            dbdsqr(
                LapackRelease::V3_12_0,
                Uplo::Upper,
                n,
                0,
                0,
                0,
                &mut d,
                &mut e,
                (&mut [], 1),
                (&mut [], 1),
                (&mut [], 1),
            ),
            Err(DgesvdError::ValuesOnlyNotPorted)
        );
        let p = |r: Result<usize, DgesvdError>| match r {
            Err(DgesvdError::Illegal(e)) => e.param,
            other => panic!("{other:?}"),
        };
        let mut vt = vec![0.0f64; n * n];
        assert_eq!(
            p(dbdsqr(
                LapackRelease::V3_12_0,
                Uplo::Upper,
                n,
                n,
                0,
                0,
                &mut d,
                &mut e,
                (&mut vt, n - 1),
                (&mut [], 1),
                (&mut [], 1),
            )),
            9
        );
    }

    /// SciPy's refusals and empty matrices; nipype's fallback and regularization.
    #[test]
    fn scipy_and_nipype() {
        let lib = ScipyLibrary::V1_18;
        let mut a = matrix(6, 4, 7);
        let empty = scipy_svd(lib, view(&a, 0, 3), true).unwrap();
        assert_eq!((empty.ucols, empty.vtrows, empty.vt.len()), (0, 3, 9));
        a[5] = f64::NAN;
        assert_eq!(
            scipy_svd(lib, view(&a, 6, 4), false),
            Err(ScipySvdError::NotFinite)
        );
        let numpy = Lapack::default();
        // numpy's LinAlgError on a non-finite matrix ends in SciPy's ValueError.
        assert_eq!(
            nipype_fallback_svd(numpy, lib, view(&a, 6, 4)),
            Err(ScipySvdError::NotFinite)
        );
        // Two equal columns: singular, regularized; a well-conditioned matrix is kept.
        let t = 20;
        let mut x = matrix(t, 3, 9);
        for i in 0..t {
            x[i * 3 + 2] = x[i * 3 + 1];
        }
        let r = nipype_full_rank(numpy, lib, view(&x, t, 3), 1e15).unwrap();
        assert!(r.regularized && r.c == 1e15);
        let y = matrix(t, 3, 10);
        let r = nipype_full_rank(numpy, lib, view(&y, t, 3), 1e15).unwrap();
        assert!(!r.regularized && r.x == y && r.c < 1e15);
        assert_eq!(
            nipype_full_rank(numpy, lib, view(&y, 0, 3), 1e15),
            Err(ScipySvdError::EmptyReduction)
        );
        assert!(too_large(lib, 50_000, 50_000, true).is_some());
        assert!(too_large(lib, 50_000, 3, false).is_none());
    }
}
