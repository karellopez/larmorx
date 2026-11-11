// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The thin SVD of a general float64 matrix and the pseudo-inverse built on it, as numpy's x86-64
//! wheels compute them at one BLAS thread on the Haswell kernels (`OPENBLAS_CORETYPE=Haswell`),
//! both LAPACK releases, on every platform: `numpy.linalg.svd(a, full_matrices=False)` (LAPACK's
//! `DGESDD` with `JOBZ = 'S'`) and `numpy.linalg.pinv(a)`, the decompositions of fMRIPrep's
//! CompCor (nipype's `compute_noise_components`: up to 600 × 60,968) and of nipype's
//! `regress_poly` (the pseudo-inverse of a `T × (degree + 1)` Legendre basis).
//!
//! **`DGESDD('S')`** ([`dgesdd_s`]): `A` (`m × n`, column-major) is scaled by `DLASCL` when its
//! largest magnitude (`DLANGE('M')`; a NaN gives `INFO = -4`) lies outside `[SMLNUM, BIGNUM]`
//! (`SMLNUM = √(2^-1022) / 2^-52`), the singular values unscaled at the end; then one of four
//! paths, chosen by `MNTHR = ⌊min(m, n)·11/6⌋`:
//!
//! - path 3 (`m >= n`, `m >= MNTHR`): `DGEQRF`, `R` copied out with zeros below it, `DORGQR`
//!   (`Q` in `A`), `DGEBRD` of `R`, `DBDSDC('U', 'I')` into `U` and `VT`, `DORMBR('Q', 'L',
//!   'N')` on `U`, `DORMBR('P', 'R', 'T')` on `VT`, then `U = Q·U` by `DGEMM`;
//! - path 5 (`m >= n`, `m < MNTHR`): `DGEBRD` of `A`, `U` cleared, `DBDSDC('U', 'I')`, the two
//!   `DORMBR` (`U` of `m × n`);
//! - path 3t (`m < n`, `n >= MNTHR`): `DGELQF`, `L` copied out with zeros above it, `DORGLQ`
//!   (`Q` in `A`), `DGEBRD` of `L`, `DBDSDC('U', 'I')`, the two `DORMBR`, then `VT = VT·Q` by
//!   `DGEMM`;
//! - path 5t (`m < n`, `n < MNTHR`): `DGEBRD` of `A` (lower bidiagonal), `VT` cleared,
//!   `DBDSDC('L', 'I')`, the two `DORMBR` (`VT` of `m × n`).
//!
//! Each routine gets the workspace `DGESDD` hands it, `LWORK - NWORK + 1` (its offset in
//! `WORK`), so the short-workspace rules of the callees apply as in LAPACK: `DORMBR` passes its
//! `LWORK` on to `DORMQR`/`DORMLQ`, which with the optimal `LWORK` of small matrices lower their
//! block size by each release's rule (`blocked`'s documentation). `DGESDD`'s own workspace query
//! ([`dgesdd_s_lwork`]) asks the callees' queries, which are the same in both releases; so is
//! `dgesdd.f`. The routines it calls and the BLAS kernels under them are the ports of
//! [`super::blocked`], [`super::bdsdc`] and [`crate::openblas`]; the bits never depend on the
//! number of threads. An infinite entry (no NaN) makes LAPACK scale `A` to NaN (`DLASCL` with an
//! infinite `CFROM` multiplies by 0) and then loop forever or return garbage (numpy issue 32591):
//! [`dgesdd_s`] refuses it ([`DgesddError::Infinite`]).
//!
//! **numpy's `svd`** ([`numpy_svd`]; `umath_linalg.cpp`'s `svd_wrapper`, `init_gesdd`, the
//! same in numpy 2.5.4 and 2.3.5 but for one check): the matrix copied into Fortran order (`LDA =
//! LDU = max(m, 1)`, `LDVT = max(1, min(m, n))`), the workspace query and the call with its
//! optimal `LWORK`, the results copied back in C order; `INFO != 0` gives numpy's `LinAlgError('SVD
//! did not converge')`. numpy 2.5.4 refuses a non-finite matrix before LAPACK (`all_finite`: the
//! same error); numpy 2.3.5 and 2.2.6 hand it to LAPACK, which returns `INFO = -4` for a NaN (the
//! same error) but hangs, prints `DLASCL`'s messages and returns infinite or NaN values silently,
//! or errs for an infinity (measured on this machine): larmorx gives the error for every release
//! (`docs/findings/confounds-blas.md`).
//!
//! **numpy's `pinv`** ([`numpy_pinv`]; `_linalg.py`, the same in numpy 2.5.4 and 2.3.5): `svd(a,
//! full_matrices=False)`, `cutoff = rcond·max(s)`, `s = 1/s` where `s > cutoff` and 0 elsewhere,
//! then `matmul(vt.T, s[:, None]·u.T)` (the product's right operand in F order, as numpy's
//! multiply lays it out) through [`crate::openblas::matmul`]; an empty matrix gives an empty
//! result.
//!
//! Upstream files (BSD-3-Clause; the notices are in the parent module and NOTICE): OpenBLAS
//! 0.3.30 and v0.3.34-106-g446c436e `lapack-netlib/SRC/` (`dgesdd.f`, identical in both;
//! `dlange.f`, `dlascl.f`, `dlaset.f`, `dlacpy.f`, `la_xisnan.F90`, `droundup_lwork.f`,
//! `xerbla.f`, and the routines of [`super::blocked`] and [`super::bdsdc`]); numpy 2.5.4 and 2.3.5
//! `numpy/linalg/umath_linalg.cpp` (`svd_wrapper`, `init_gesdd`, `all_finite`, `linearize_matrix`,
//! `delinearize_matrix`) and `numpy/linalg/_linalg.py` (`svd`, `pinv`). The logic is re-expressed
//! in Rust; no code was copied.

// The routine keeps LAPACK's names, arguments and paths, so the port can be checked line by line
// against the Fortran.
#![allow(clippy::too_many_arguments, clippy::many_single_char_names)]

use super::bdsdc::{dbdsdc, dlascl_g};
use super::blocked::{
    IllegalValue, Lapack, Vect, dgebrd, dgelqf, dgeqrf, dorglq, dorgqr, dormbr, gebrd_lwork_opt,
    lwork_opt, org_lwork_opt, ormbr_lwork_opt,
};
use super::{EPS, SAFMIN};
use crate::openblas::level3::{Operand, Trans, dgemm_with};
use crate::openblas::matmul::{Array2, numpy_matmul_with};
use crate::openblas::trmm::{Side, Uplo};

/// Why [`dgesdd_s`] did not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DgesddError {
    /// A parameter LAPACK's checks refuse (`XERBLA`'s message).
    #[error(transparent)]
    Illegal(#[from] IllegalValue),
    /// `A` holds an infinity (and no NaN): LAPACK would scale it to NaN and loop forever or
    /// return garbage (module documentation).
    #[error("DGESDD: the matrix holds an infinite value")]
    Infinite,
}

/// What [`dgesdd_s`] returns: LAPACK's `INFO` (0; `> 0` when `DBDSDC` did not converge, the
/// results then as LAPACK leaves them; -4 when `A` holds a NaN, nothing computed) and `WORK(1)`,
/// the optimal workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gesdd {
    /// LAPACK's `INFO`.
    pub info: i64,
    /// `WORK(1)` on return: [`dgesdd_s_lwork`]'s optimal workspace.
    pub work1: usize,
}

/// `DGESDD('S', M, N, ...)`'s workspace query: `(MINWRK, MAXWRK)`, the smallest workspace it
/// accepts and the optimal one (`WORK(1)` of the `LWORK = -1` query; both 1 for an empty matrix).
/// The same in both LAPACK releases.
pub fn dgesdd_s_lwork(m: usize, n: usize) -> (usize, usize) {
    let minmn = m.min(n);
    if minmn == 0 {
        return (1, 1);
    }
    let mnthr = minmn * 11 / 6;
    let (minwrk, maxwrk);
    if m >= n {
        let bdspac = 3 * n * n + 4 * n;
        let gebrd_mn = gebrd_lwork_opt(m, n);
        let gebrd_nn = gebrd_lwork_opt(n, n);
        let geqrf_mn = lwork_opt(m, n, false);
        let orgqr_mn = org_lwork_opt(m, n, false);
        let ormbr_prt_nn = ormbr_lwork_opt(Side::Right, n, n);
        let ormbr_qln_nn = ormbr_lwork_opt(Side::Left, n, n);
        let ormbr_qln_mn = ormbr_lwork_opt(Side::Left, m, n);
        if m >= mnthr {
            // Path 3 (M >> N, JOBZ = 'S').
            let wrkbl = (n + geqrf_mn)
                .max(n + orgqr_mn)
                .max(3 * n + gebrd_nn)
                .max(3 * n + ormbr_qln_nn)
                .max(3 * n + ormbr_prt_nn)
                .max(3 * n + bdspac);
            maxwrk = wrkbl + n * n;
            minwrk = bdspac + n * n + 3 * n;
        } else {
            // Path 5 (M >= N, but not much larger).
            let wrkbl = (3 * n + gebrd_mn)
                .max(3 * n + ormbr_qln_mn)
                .max(3 * n + ormbr_prt_nn);
            maxwrk = wrkbl.max(3 * n + bdspac);
            minwrk = 3 * n + m.max(bdspac);
        }
    } else {
        let bdspac = 3 * m * m + 4 * m;
        let gebrd_mn = gebrd_lwork_opt(m, n);
        let gebrd_mm = gebrd_lwork_opt(m, m);
        let gelqf_mn = lwork_opt(m, n, true);
        let orglq_mn = org_lwork_opt(m, n, true);
        let ormbr_prt_mm = ormbr_lwork_opt(Side::Right, m, m);
        let ormbr_prt_mn = ormbr_lwork_opt(Side::Right, m, n);
        let ormbr_qln_mm = ormbr_lwork_opt(Side::Left, m, m);
        if n >= mnthr {
            // Path 3t (N >> M, JOBZ = 'S').
            let wrkbl = (m + gelqf_mn)
                .max(m + orglq_mn)
                .max(3 * m + gebrd_mm)
                .max(3 * m + ormbr_qln_mm)
                .max(3 * m + ormbr_prt_mm)
                .max(3 * m + bdspac);
            maxwrk = wrkbl + m * m;
            minwrk = bdspac + m * m + 3 * m;
        } else {
            // Path 5t (N > M, but not much larger).
            let wrkbl = (3 * m + gebrd_mn)
                .max(3 * m + ormbr_qln_mm)
                .max(3 * m + ormbr_prt_mn);
            maxwrk = wrkbl.max(3 * m + bdspac);
            minwrk = 3 * m + n.max(bdspac);
        }
    }
    (minwrk, maxwrk.max(minwrk))
}

#[inline(always)]
fn ix(ld: usize, i: usize, j: usize) -> usize {
    (i - 1) + (j - 1) * ld
}

/// `DLANGE('M', M, N, A, LDA, WORK)`: the largest magnitude, NaN when any entry is NaN.
pub(super) fn dlange_m(m: usize, n: usize, a: &[f64], lda: usize) -> f64 {
    let mut value = 0.0f64;
    for j in 1..=n {
        for i in 1..=m {
            let temp = a[ix(lda, i, j)].abs();
            if value < temp || temp.is_nan() {
                value = temp;
            }
        }
    }
    value
}

/// `DLASET('F', M, N, 0, 0, A, LDA)`: zeros in the `m × n` block.
pub(super) fn zero_block(m: usize, n: usize, a: &mut [f64], lda: usize) {
    for j in 1..=n {
        for i in 1..=m {
            a[ix(lda, i, j)] = 0.0;
        }
    }
}

/// `DLACPY('F', M, N, A, LDA, B, LDB)`.
pub(super) fn copy_block(m: usize, n: usize, a: &[f64], lda: usize, b: &mut [f64], ldb: usize) {
    for j in 1..=n {
        for i in 1..=m {
            b[ix(ldb, i, j)] = a[ix(lda, i, j)];
        }
    }
}

/// `DGESDD('S', M, N, A, LDA, S, U, LDU, VT, LDVT, WORK, LWORK, IWORK, INFO)`: the thin SVD `A =
/// U·diag(S)·VT` of the column-major `m × n` matrix `a` (`a[i + j·lda]`, overwritten), the
/// `min(m, n)` singular values descending in `s`, `U` (`m × min(m, n)`, leading dimension `ldu`)
/// and `VT` (`min(m, n) × n`, `ldvt`) written into `u` and `vt` (their entries outside those
/// blocks are kept), as LAPACK computes them (module documentation). `lwork` is the workspace the
/// caller gives (`None`: the optimal, as numpy's query gives it). Returns `INFO` and `WORK(1)`
/// ([`Gesdd`]).
///
/// # Errors
/// `LDA < max(1, M)` (5), `LDU < max(1, M)` (8), `LDVT < max(1, min(M, N))` (10), `LWORK <
/// MINWRK` (12) as LAPACK ([`DgesddError::Illegal`]); an infinite entry
/// ([`DgesddError::Infinite`]).
pub fn dgesdd_s(
    lp: Lapack,
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
) -> Result<Gesdd, DgesddError> {
    let minmn = m.min(n);
    let err = |param| {
        Err(DgesddError::Illegal(IllegalValue {
            routine: "DGESDD",
            param,
        }))
    };
    if lda < m.max(1) {
        return err(5);
    }
    if ldu < 1 || ldu < m {
        return err(8);
    }
    if ldvt < 1 || ldvt < minmn {
        return err(10);
    }
    let (minwrk, maxwrk) = dgesdd_s_lwork(m, n);
    let lwork = lwork.unwrap_or(maxwrk);
    if lwork < minwrk {
        return err(12);
    }
    let done = |info| {
        Ok(Gesdd {
            info,
            work1: maxwrk,
        })
    };
    // Quick return if possible.
    if m == 0 || n == 0 {
        return done(0);
    }
    // Get machine constants.
    let eps = 2.0 * EPS; // DLAMCH('P')
    let smlnum = SAFMIN.sqrt() / eps;
    let bignum = 1.0 / smlnum;
    // Scale A if max element outside range [SMLNUM, BIGNUM].
    let anrm = dlange_m(m, n, a, lda);
    if anrm.is_nan() {
        return done(-4);
    }
    if anrm.is_infinite() {
        return Err(DgesddError::Infinite);
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
    let mut tauq = vec![0.0f64; minmn];
    let mut taup = vec![0.0f64; minmn];
    let info;
    if m >= n {
        // A has at least as many rows as columns.
        if m >= minmn * 11 / 6 {
            // Path 3 (M >> N, JOBZ = 'S'): N left singular vectors to be computed in U and N
            // right singular vectors in VT. WORK(IR) is N by N.
            let ldwrkr = n;
            let mut r = vec![0.0f64; ldwrkr * n];
            let mut tau = vec![0.0f64; n];
            // NWORK = ITAU + N, ITAU = IR + LDWRKR·N.
            let lw = lwork - (ldwrkr * n + n);
            // Compute A = Q·R.
            dgeqrf(lp, m, n, a, 0, lda, &mut tau, Some(lw))?;
            // Copy R to WORK(IR), zeroing out below it.
            for j in 1..=n {
                for i in 1..=n {
                    r[ix(ldwrkr, i, j)] = if i <= j { a[ix(lda, i, j)] } else { 0.0 };
                }
            }
            // Generate Q in A.
            dorgqr(lp, m, n, n, a, 0, lda, &tau, Some(lw))?;
            // IE = ITAU, ITAUQ = IE + N, ITAUP = ITAUQ + N, NWORK = ITAUP + N.
            let lw = lwork - (ldwrkr * n + 3 * n);
            // Bidiagonalize R in WORK(IR).
            dgebrd(
                lp,
                n,
                n,
                &mut r,
                0,
                ldwrkr,
                s,
                &mut e,
                &mut tauq,
                &mut taup,
                Some(lw),
            )?;
            // Perform bidiagonal SVD, computing left singular vectors of bidiagonal matrix in
            // U and computing right singular vectors of bidiagonal matrix in VT.
            info = dbdsdc(lp, Uplo::Upper, n, s, &mut e, u, ldu, vt, ldvt)?;
            // Overwrite U by left singular vectors of R and VT by right singular vectors of R.
            dormbr(
                lp,
                Vect::Q,
                Side::Left,
                Trans::N,
                n,
                n,
                n,
                &mut r,
                0,
                ldwrkr,
                &tauq,
                u,
                0,
                ldu,
                Some(lw),
            )?;
            dormbr(
                lp,
                Vect::P,
                Side::Right,
                Trans::T,
                n,
                n,
                n,
                &mut r,
                0,
                ldwrkr,
                &taup,
                vt,
                0,
                ldvt,
                Some(lw),
            )?;
            // Multiply Q in A by left singular vectors of R in WORK(IR), storing result in U.
            copy_block(n, n, u, ldu, &mut r, ldwrkr);
            let qa = Operand {
                data: a,
                off: 0,
                ld: lda,
                trans: Trans::N,
            };
            let ur = Operand {
                data: &r,
                off: 0,
                ld: ldwrkr,
                trans: Trans::N,
            };
            dgemm_with(m, n, n, 1.0, qa, ur, 0.0, u, 0, ldu, lp.gemm_q);
        } else {
            // Path 5 (M >= N, but not much larger): reduce to bidiagonal form without QR
            // decomposition. IE = 1, ITAUQ = IE + N, ITAUP = ITAUQ + N, NWORK = ITAUP + N.
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
            // Perform bidiagonal SVD, computing left singular vectors of bidiagonal matrix in
            // U and computing right singular vectors of bidiagonal matrix in VT.
            zero_block(m, n, u, ldu);
            info = dbdsdc(lp, Uplo::Upper, n, s, &mut e, u, ldu, vt, ldvt)?;
            // Overwrite U by left singular vectors of A and VT by right singular vectors of A.
            dormbr(
                lp,
                Vect::Q,
                Side::Left,
                Trans::N,
                m,
                n,
                n,
                a,
                0,
                lda,
                &tauq,
                u,
                0,
                ldu,
                Some(lw),
            )?;
            dormbr(
                lp,
                Vect::P,
                Side::Right,
                Trans::T,
                n,
                n,
                n,
                a,
                0,
                lda,
                &taup,
                vt,
                0,
                ldvt,
                Some(lw),
            )?;
        }
    } else {
        // A has more columns than rows.
        if n >= minmn * 11 / 6 {
            // Path 3t (N >> M, JOBZ = 'S'): M right singular vectors to be computed in VT and M
            // left singular vectors in U. WORK(IL) is M by M.
            let ldwrkl = m;
            let mut l = vec![0.0f64; ldwrkl * m];
            let mut tau = vec![0.0f64; m];
            // NWORK = ITAU + M, ITAU = IL + LDWRKL·M.
            let lw = lwork - (ldwrkl * m + m);
            // Compute A = L·Q.
            dgelqf(lp, m, n, a, 0, lda, &mut tau, Some(lw))?;
            // Copy L to WORK(IL), zeroing out above it.
            for j in 1..=m {
                for i in 1..=m {
                    l[ix(ldwrkl, i, j)] = if i >= j { a[ix(lda, i, j)] } else { 0.0 };
                }
            }
            // Generate Q in A.
            dorglq(lp, m, n, m, a, 0, lda, &tau, Some(lw))?;
            // IE = ITAU, ITAUQ = IE + M, ITAUP = ITAUQ + M, NWORK = ITAUP + M.
            let lw = lwork - (ldwrkl * m + 3 * m);
            // Bidiagonalize L in WORK(IU).
            dgebrd(
                lp,
                m,
                m,
                &mut l,
                0,
                ldwrkl,
                s,
                &mut e,
                &mut tauq,
                &mut taup,
                Some(lw),
            )?;
            // Perform bidiagonal SVD, computing left singular vectors of bidiagonal matrix in
            // U and computing right singular vectors of bidiagonal matrix in VT.
            info = dbdsdc(lp, Uplo::Upper, m, s, &mut e, u, ldu, vt, ldvt)?;
            // Overwrite U by left singular vectors of L and VT by right singular vectors of L.
            dormbr(
                lp,
                Vect::Q,
                Side::Left,
                Trans::N,
                m,
                m,
                m,
                &mut l,
                0,
                ldwrkl,
                &tauq,
                u,
                0,
                ldu,
                Some(lw),
            )?;
            dormbr(
                lp,
                Vect::P,
                Side::Right,
                Trans::T,
                m,
                m,
                m,
                &mut l,
                0,
                ldwrkl,
                &taup,
                vt,
                0,
                ldvt,
                Some(lw),
            )?;
            // Multiply right singular vectors of L in WORK(IL) by Q in A, storing result in VT.
            copy_block(m, m, vt, ldvt, &mut l, ldwrkl);
            let vl = Operand {
                data: &l,
                off: 0,
                ld: ldwrkl,
                trans: Trans::N,
            };
            let qa = Operand {
                data: a,
                off: 0,
                ld: lda,
                trans: Trans::N,
            };
            dgemm_with(m, n, m, 1.0, vl, qa, 0.0, vt, 0, ldvt, lp.gemm_q);
        } else {
            // Path 5t (N > M, but not much larger): reduce to bidiagonal form without LQ
            // decomposition. IE = 1, ITAUQ = IE + M, ITAUP = ITAUQ + M, NWORK = ITAUP + M.
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
            // Perform bidiagonal SVD, computing left singular vectors of bidiagonal matrix in
            // U and computing right singular vectors of bidiagonal matrix in VT.
            zero_block(m, n, vt, ldvt);
            info = dbdsdc(lp, Uplo::Lower, m, s, &mut e, u, ldu, vt, ldvt)?;
            // Overwrite U by left singular vectors of A and VT by right singular vectors of A.
            dormbr(
                lp,
                Vect::Q,
                Side::Left,
                Trans::N,
                m,
                m,
                n,
                a,
                0,
                lda,
                &tauq,
                u,
                0,
                ldu,
                Some(lw),
            )?;
            dormbr(
                lp,
                Vect::P,
                Side::Right,
                Trans::T,
                m,
                n,
                m,
                a,
                0,
                lda,
                &taup,
                vt,
                0,
                ldvt,
                Some(lw),
            )?;
        }
    }
    // Undo scaling if necessary.
    if iscl {
        if anrm > bignum {
            dlascl_g(bignum, anrm, minmn, 1, s, 0, minmn)?;
        }
        if anrm < smlnum {
            dlascl_g(smlnum, anrm, minmn, 1, s, 0, minmn)?;
        }
    }
    done(info as i64)
}

/// numpy's `LinAlgError('SVD did not converge')`, raised by [`numpy_svd`] and [`numpy_pinv`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("SVD did not converge")]
pub struct SvdDidNotConverge;

/// The thin SVD `a = u · diag(s) · vt` in numpy's layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Svd {
    /// `k = min(m, n)`.
    pub k: usize,
    /// `U`, `m × k`, C order (`u[i·k + j]`).
    pub u: Vec<f64>,
    /// The `k` singular values, descending.
    pub s: Vec<f64>,
    /// `VT`, `k × n`, C order (`vt[i·n + j]`).
    pub vt: Vec<f64>,
}

/// `numpy.linalg.svd(a, full_matrices=False)` of the float64 matrix `a` (any strides), with
/// LAPACK `lp` (numpy 2.5.4: [`super::LapackRelease::V3_12_1`]; numpy 2.3.5 and 2.2.6:
/// [`super::LapackRelease::V3_12_0`]) (module documentation).
///
/// # Errors
/// [`SvdDidNotConverge`] when `DGESDD` returns `INFO != 0` or `a` holds a non-finite value.
pub fn numpy_svd(lp: Lapack, a: Array2<'_>) -> Result<Svd, SvdDidNotConverge> {
    let (m, n) = (a.rows, a.cols);
    let k = m.min(n);
    let ld = m.max(1);
    let ldvt = k.max(1);
    // Copy the matrix in (Fortran order).
    let mut fa = vec![0.0f64; m * n];
    for j in 0..n {
        for i in 0..m {
            fa[i + j * m] = a.data[a.off + i * a.s0 + j * a.s1];
        }
    }
    if fa.iter().any(|v| !v.is_finite()) {
        return Err(SvdDidNotConverge);
    }
    let mut s = vec![0.0f64; k];
    let mut fu = vec![0.0f64; ld * k];
    let mut fvt = vec![0.0f64; ldvt * n];
    let out = dgesdd_s(
        lp, m, n, &mut fa, ld, &mut s, &mut fu, ld, &mut fvt, ldvt, None,
    )
    .map_err(|_| SvdDidNotConverge)?;
    if out.info != 0 {
        return Err(SvdDidNotConverge);
    }
    // Copy the results out (C order).
    let mut u = vec![0.0f64; m * k];
    for i in 0..m {
        for j in 0..k {
            u[i * k + j] = fu[i + j * ld];
        }
    }
    let mut vt = vec![0.0f64; k * n];
    for i in 0..k {
        for j in 0..n {
            vt[i * n + j] = fvt[i + j * ldvt];
        }
    }
    Ok(Svd { k, u, s, vt })
}

/// `numpy.linalg.pinv(a, rcond)` of the float64 matrix `a` (any strides; numpy's default `rcond`
/// is `1e-15`, its `rtol=None` gives `max(m, n)·2^-52`): `n × m`, C order (module
/// documentation).
///
/// # Errors
/// [`SvdDidNotConverge`], from [`numpy_svd`].
pub fn numpy_pinv(lp: Lapack, a: Array2<'_>, rcond: f64) -> Result<Vec<f64>, SvdDidNotConverge> {
    let (m, n) = (a.rows, a.cols);
    if m == 0 || n == 0 {
        return Ok(Vec::new());
    }
    let Svd { k, u, mut s, vt } = numpy_svd(lp, a)?;
    // Discard small singular values.
    let smax = s.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let cutoff = rcond * smax;
    for v in &mut s {
        *v = if *v > cutoff { 1.0 / *v } else { 0.0 };
    }
    // multiply(s[..., newaxis], transpose(u)): k × m, laid out in F order (u's memory).
    let mut x = vec![0.0f64; m * k];
    for j in 0..m {
        for i in 0..k {
            x[j * k + i] = s[i] * u[j * k + i];
        }
    }
    let vtt = Array2 {
        data: &vt,
        off: 0,
        rows: n,
        cols: k,
        s0: 1,
        s1: n,
    };
    let xs = Array2 {
        data: &x,
        off: 0,
        rows: k,
        cols: m,
        s0: 1,
        s1: k,
    };
    Ok(numpy_matmul_with(vtt, xs, lp.gemm_q))
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

    /// `u·diag(s)·vt` rebuilds `a`, `u` and `vt` are orthonormal, the values descend; every path.
    #[test]
    fn rebuilds_every_path() {
        for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
            let lp = Lapack::new(release);
            for &(m, n) in &[
                (1, 1),
                (7, 1),
                (40, 3),
                (30, 20),
                (26, 25),
                (150, 140),
                (300, 140),
                (3, 40),
                (20, 30),
                (140, 150),
                (140, 300),
            ] {
                let a = matrix(m, n, (m * 1000 + n) as u64);
                let svd = numpy_svd(lp, view(&a, m, n)).unwrap();
                let k = m.min(n);
                assert!(svd.s.windows(2).all(|w| w[0] >= w[1]));
                let mut worst = 0.0f64;
                for i in 0..m {
                    for j in 0..n {
                        let mut acc = 0.0;
                        for t in 0..k {
                            acc += svd.u[i * k + t] * svd.s[t] * svd.vt[t * n + j];
                        }
                        worst = worst.max((acc - a[i * n + j]).abs());
                    }
                }
                assert!(worst < 1e-12, "{release:?} {m}x{n}: {worst}");
                for p in 0..k {
                    for q in 0..k {
                        let uu: f64 = (0..m).map(|i| svd.u[i * k + p] * svd.u[i * k + q]).sum();
                        let vv: f64 = (0..n).map(|j| svd.vt[p * n + j] * svd.vt[q * n + j]).sum();
                        let id = if p == q { 1.0 } else { 0.0 };
                        assert!((uu - id).abs() < 1e-12 && (vv - id).abs() < 1e-12);
                    }
                }
            }
        }
    }

    /// The bits do not depend on the number of threads.
    #[test]
    fn threads_do_not_change_bits() {
        let a = matrix(60, 700, 3);
        let lp = Lapack::default();
        let run = |t| {
            larmorx_core::parallel::with_threads(t, || numpy_svd(lp, view(&a, 60, 700)).unwrap())
                .unwrap()
        };
        assert_eq!(run(1), run(5));
    }

    /// `pinv` of a tall full-rank matrix is a left inverse; a zero column gives a zero row.
    #[test]
    fn pinv_left_inverse() {
        let (m, n) = (50, 3);
        let mut a = matrix(m, n, 9);
        let lp = Lapack::default();
        let p = numpy_pinv(lp, view(&a, m, n), 1e-15).unwrap();
        for r in 0..n {
            for c in 0..n {
                let v: f64 = (0..m).map(|i| p[r * m + i] * a[i * n + c]).sum();
                assert!((v - if r == c { 1.0 } else { 0.0 }).abs() < 1e-12);
            }
        }
        for i in 0..m {
            a[i * n + 2] = 0.0;
        }
        let p = numpy_pinv(lp, view(&a, m, n), 1e-15).unwrap();
        assert!(p[2 * m..3 * m].iter().all(|&v| v == 0.0));
        assert!(numpy_pinv(lp, view(&a, 0, 3), 1e-15).unwrap().is_empty());
    }

    /// The scaling of tiny and huge matrices, NaN (`INFO = -4`), infinity and the refused
    /// arguments.
    #[test]
    fn scaling_and_refusals() {
        let lp = Lapack::default();
        let (m, n) = (9, 4);
        let a = matrix(m, n, 5);
        let base = numpy_svd(lp, view(&a, m, n)).unwrap();
        for f in [2f64.powi(-600), 2f64.powi(600)] {
            let b: Vec<f64> = a.iter().map(|v| v * f).collect();
            let svd = numpy_svd(lp, view(&b, m, n)).unwrap();
            for (x, y) in svd.s.iter().zip(&base.s) {
                assert!((x / f - y).abs() < 1e-14 * y);
            }
        }
        let mut b = a.clone();
        b[3] = f64::NAN;
        assert_eq!(numpy_svd(lp, view(&b, m, n)), Err(SvdDidNotConverge));
        let (mut s, mut u, mut vt) = (vec![0.0; 4], vec![0.0; 36], vec![0.0; 16]);
        let mut f = b.clone();
        let out = dgesdd_s(lp, m, n, &mut f, m, &mut s, &mut u, m, &mut vt, 4, None).unwrap();
        assert_eq!(out.info, -4);
        f[3] = f64::INFINITY;
        assert_eq!(
            dgesdd_s(lp, m, n, &mut f, m, &mut s, &mut u, m, &mut vt, 4, None),
            Err(DgesddError::Infinite)
        );
        let p = |r: Result<Gesdd, DgesddError>| match r {
            Err(DgesddError::Illegal(e)) => e.param,
            other => panic!("{other:?}"),
        };
        let mut f = a.clone();
        assert_eq!(
            p(dgesdd_s(
                lp,
                m,
                n,
                &mut f,
                m - 1,
                &mut s,
                &mut u,
                m,
                &mut vt,
                4,
                None
            )),
            5
        );
        assert_eq!(
            p(dgesdd_s(
                lp,
                m,
                n,
                &mut f,
                m,
                &mut s,
                &mut u,
                m - 1,
                &mut vt,
                4,
                None
            )),
            8
        );
        assert_eq!(
            p(dgesdd_s(
                lp, m, n, &mut f, m, &mut s, &mut u, m, &mut vt, 3, None
            )),
            10
        );
        let (minwrk, maxwrk) = dgesdd_s_lwork(m, n);
        assert!(minwrk <= maxwrk);
        assert_eq!(
            p(dgesdd_s(
                lp,
                m,
                n,
                &mut f,
                m,
                &mut s,
                &mut u,
                m,
                &mut vt,
                4,
                Some(minwrk - 1)
            )),
            12
        );
        assert_eq!(dgesdd_s_lwork(0, 5), (1, 1));
    }
}
