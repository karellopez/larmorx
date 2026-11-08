// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The thin SVD of a float64 `m × 2` matrix, `scipy.linalg.svd(a, full_matrices=False)` (and
//! `numpy.linalg.svd(a, full_matrices=False)`), as LAPACK's `DGESDD` with `JOBZ = 'S'`
//! computes it with OpenBLAS's Haswell kernels, at one BLAS thread, on every platform.
//!
//! SciPy's `least_squares` (the trust-region reflective method with the `exact` solver) takes
//! this SVD of its augmented Jacobian at every iteration, and `curve_fit` one of the final
//! Jacobian for the covariance. SciPy's wheels bundle their own OpenBLAS (`scipy-openblas32`:
//! 0.3.31.dev in SciPy 1.18.1, 0.3.28 in SciPy 1.15.3, both with LAPACK 3.12.0's routines and
//! GCC 10 builds of the kernels numpy's OpenBLAS also has). For `n = 2` the call takes:
//!
//! - `DLANGE('M')` and the scaling by `DLASCL` when the largest magnitude is outside
//!   `[SMLNUM, BIGNUM]` (`SMLNUM = √(2^-1022) / 2^-52`), undone on the singular values at the
//!   end;
//! - `m >= MNTHR = ⌊2 · 11 / 6⌋ = 3`, path 3: `DGEQRF` → `DGEQR2` (two `DLARFG`, the first
//!   reflector applied to the second column), `R` copied out with zeros below it, `DORGQR` →
//!   `DORG2R` (the second reflector's column scaled by `-tau₂`, the first reflector applied to
//!   the second column, the first column scaled by `-tau₁`), `DGEBRD` → `DGEBD2` of the 2 × 2
//!   `R`, `DBDSDC('U', 'I')` → `DLASDQ` → `DBDSQR` (then `DLASDQ`'s ascending sort and
//!   `DBDSDC`'s descending selection sort), `DORMBR('Q', 'L', 'N')` and `DORMBR('P', 'R', 'T')`
//!   on the 2 × 2 factors, then `U = Q · U_R` by `DGEMM('N', 'N', m, 2, 2)` (OpenBLAS's Haswell
//!   kernel: per element `fma(q₂, w₂ⱼ, fma(q₁, w₁ⱼ, +0))`, added to the cleared `U`);
//! - `m = 2`, path 5s: `DGEBD2` of `A` itself, `DBDSDC`, the two `DORMBR` on `U` and `VT`;
//! - the reflectors as LAPACK 3.12.0 applies them (`DLARF`, the stored 1; SciPy's builds) or
//!   3.12.1 (`DLARF1F`; numpy 2.5.4's OpenBLAS 0.3.34), with the parent module's kernels for
//!   the long columns (`dgemv_t`, `dger` via `daxpy`, `dnrm2`, `dscal`).
//!
//! Upstream files (BSD-3-Clause; the notices are in the parent module and NOTICE): OpenBLAS
//! 0.3.30 and 0.3.34 `lapack-netlib/SRC/` (`dgesdd.f`, `dgeqrf.f`, `dgeqr2.f`, `dorgqr.f`,
//! `dorg2r.f`, `dlarf.f`, `dlarf1f.f`, `dlarfg.f`, `dgebrd.f`, `dgebd2.f`, `dbdsdc.f`,
//! `dlasdq.f`, `dbdsqr.f`, `dormbr.f`, `dormqr.f`, `dorm2r.f`, `dormlq.f`, `dorml2.f`,
//! `dlacpy.f`, `dlaset.f`, `dlange.f`, `dlascl.f`, `iladlc.f`, `ilaenv.f`), `interface/`
//! (`gemv.c`, `ger.c`, `scal.c`, `gemm.c`), `kernel/x86_64/` (`dgemv_t_4.c`, `daxpy.c`,
//! `dscal.c`, `nrm2.S`, `dgemm_kernel_4x8_haswell.S`); SciPy 1.18.1 and 1.15.3
//! `scipy/linalg/_decomp_svd.py`. The logic is re-expressed in Rust; no code was copied.

use super::lstsq::{dlange_max, reflect_left};
use super::{
    EPS, LapackRelease, SAFMIN, apply_reflector, dbdsqr_upper_ncc, dgebd2, dlarfg, dlascl, dscal,
    dswap,
};

/// The thin SVD `a = u · diag(s) · vt` of an `m × 2` matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct Svd2 {
    /// `U`, `m × 2`, column-major (`u[i + j·m]`).
    pub u: Vec<f64>,
    /// The singular values, decreasing.
    pub s: [f64; 2],
    /// `VT`, 2 × 2, column-major (`vt[i + 2j]` is `VT(i, j)`).
    pub vt: [f64; 4],
}

/// Why the SVD gave no result.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GesddError {
    /// SciPy's `check_finite`: `ValueError('array must not contain infs or NaNs')`.
    #[error("array must not contain infs or NaNs")]
    NotFinite,
    /// `DGESDD` returned `INFO > 0` (`DBDSQR` did not converge): `LinAlgError('SVD did not
    /// converge')`.
    #[error("SVD did not converge")]
    DidNotConverge,
    /// A shape this port does not follow (it follows `m >= 2` rows and two columns).
    #[error("gesdd is ported for at least two rows and two columns, not {m} x 2")]
    Unsupported { m: usize },
}

/// The SVD of the column-major `m × 2` matrix `a` (`a[i + j·m]`), `DGESDD('S')` with LAPACK
/// `release` (module documentation; SciPy's builds are [`LapackRelease::V3_12_0`]).
pub fn gesdd_s2(a: &[f64], m: usize, release: LapackRelease) -> Result<Svd2, GesddError> {
    const N: usize = 2;
    if m < N {
        return Err(GesddError::Unsupported { m });
    }
    assert_eq!(a.len(), m * N, "a must hold m × 2 values");
    if a.iter().any(|v| !v.is_finite()) {
        return Err(GesddError::NotFinite);
    }
    let lda = m;
    let mut a = a.to_vec();
    // Get machine constants; scale A if max element outside range [SMLNUM, BIGNUM].
    let eps = 2.0 * EPS; // DLAMCH('P')
    let smlnum = SAFMIN.sqrt() / eps;
    let bignum = 1.0 / smlnum;
    let anrm = dlange_max(&a);
    let mut iscl = false;
    if anrm > 0.0 && anrm < smlnum {
        iscl = true;
        let mut vals: Vec<&mut f64> = a.iter_mut().collect();
        dlascl(anrm, smlnum, &mut vals);
    } else if anrm > bignum {
        iscl = true;
        let mut vals: Vec<&mut f64> = a.iter_mut().collect();
        dlascl(anrm, bignum, &mut vals);
    }
    let mut u = vec![0.0f64; m * N];
    let mut vt = [0.0f64; N * N];
    let mut s;
    if m >= 3 {
        // Path 3 (M >> N, JOBZ='S'). Compute A = Q·R (DGEQRF → DGEQR2).
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
        // Copy R to WORK(IR), zeroing out below it (DLACPY('U'), DLASET('L')).
        let mut r = [a[0], 0.0, a[lda], a[lda + 1]];
        // Generate Q in A (DORGQR → DORG2R, I = 2 then 1).
        dscal(m - 2, -tau2, &mut a, lda + 2, 1);
        a[lda + 1] = 1.0 - tau2;
        a[lda] = 0.0;
        let v1: Vec<f64> = std::iter::once(1.0)
            .chain(a[1..m].iter().copied())
            .collect();
        a[0] = 1.0;
        reflect_left(release, m, &v1, tau1, &mut a[lda..2 * lda], 0, lda, 1);
        dscal(m - 1, -tau1, &mut a, 1, 1);
        a[0] = 1.0 - tau1;
        // Bidiagonalize R (DGEBRD → DGEBD2), its SVD, and the two DORMBR.
        let (d, mut e, tauq, taup) = dgebd2(release, N, N, &mut r, N);
        s = [d[0], d[1]];
        let mut ur = [0.0f64; N * N];
        if dbdsdc2(release, &mut s, &mut e, &mut ur, &mut vt) != 0 {
            return Err(GesddError::DidNotConverge);
        }
        ormbr_q_left(release, N, &r, N, &tauq, &mut ur, N);
        ormbr_p_right_t(release, &taup, &mut vt);
        // U = Q · U_R (DLACPY, DGEMM('N', 'N', M, N, N, 1, A, LDA, W, N, 0, U, LDU)).
        for j in 0..N {
            let (w1, w2) = (ur[j * N], ur[1 + j * N]);
            for i in 0..m {
                let acc = a[i + lda].mul_add(w2, a[i].mul_add(w1, 0.0));
                u[i + j * m] = 0.0 + acc * 1.0;
            }
        }
    } else {
        // Path 5s (M >= N, JOBZ='S'): bidiagonalize A itself (DGEBRD → DGEBD2).
        let (d, mut e, tauq, taup) = dgebd2(release, m, N, &mut a, lda);
        s = [d[0], d[1]];
        // DLASET(U = 0), DBDSDC into U(1:N, 1:N), then the two DORMBR.
        let mut ub = [0.0f64; N * N];
        if dbdsdc2(release, &mut s, &mut e, &mut ub, &mut vt) != 0 {
            return Err(GesddError::DidNotConverge);
        }
        for j in 0..N {
            for i in 0..N {
                u[i + j * m] = ub[i + j * N];
            }
        }
        ormbr_q_left(release, m, &a, lda, &tauq, &mut u, m);
        ormbr_p_right_t(release, &taup, &mut vt);
    }
    // Undo scaling if necessary.
    if iscl {
        let mut vals: Vec<&mut f64> = s.iter_mut().collect();
        if anrm > bignum {
            dlascl(bignum, anrm, &mut vals);
        }
        if anrm < smlnum {
            dlascl(smlnum, anrm, &mut vals);
        }
    }
    Ok(Svd2 { u, s, vt })
}

/// `DBDSDC('U', 'I', 2, D, E, U, 2, VT, 2, ...)`: `DLASET` of `U` and `VT` to the identity,
/// `DLASDQ` (→ `DBDSQR('U', 2, 2, 2, 0)`, then its ascending sort), then the descending
/// selection sort. Returns LAPACK's `info`.
fn dbdsdc2(
    release: LapackRelease,
    d: &mut [f64; 2],
    e: &mut [f64],
    u: &mut [f64; 4],
    vt: &mut [f64; 4],
) -> usize {
    const N: usize = 2;
    *u = [1.0, 0.0, 0.0, 1.0];
    *vt = [1.0, 0.0, 0.0, 1.0];
    let info = dbdsqr_upper_ncc(
        N,
        d,
        e,
        (vt, N, N),
        (u, N, N),
        (&mut [], 0, 1),
        release == LapackRelease::V3_12_1,
    );
    if info != 0 {
        return info;
    }
    // DLASDQ: sort the singular values into ascending order.
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
            dswap(N, vt, isub - 1, N, i - 1, N);
            dswap(N, u, (isub - 1) * N, 1, (i - 1) * N, 1);
        }
    }
    // DBDSDC, label 40: selection sort into decreasing order.
    for ii in 2..=N {
        let i = ii - 1;
        let mut kk = i;
        let mut p = d[i - 1];
        for j in ii..=N {
            if d[j - 1] > p {
                kk = j;
                p = d[j - 1];
            }
        }
        if kk != i {
            d[kk - 1] = d[i - 1];
            d[i - 1] = p;
            dswap(N, u, (i - 1) * N, 1, (kk - 1) * N, 1);
            dswap(N, vt, i - 1, N, kk - 1, N);
        }
    }
    0
}

/// `DORMBR('Q', 'L', 'N', MQ, 2, 2, A, LDA, TAUQ, C, LDC)` (`NQ = MQ >= K = 2`: `DORMQR` →
/// `DORM2R`, reflectors 2 then 1): `H(i)` on `C(i:MQ, 1:2)` with `v = A(i:MQ, i)`.
fn ormbr_q_left(
    release: LapackRelease,
    mq: usize,
    a: &[f64],
    lda: usize,
    tauq: &[f64],
    c: &mut [f64],
    ldc: usize,
) {
    for i in (1..=2usize).rev() {
        let mi = mq - i + 1;
        let v: Vec<f64> = (0..mi)
            .map(|r| {
                if r == 0 {
                    1.0
                } else {
                    a[(i - 1 + r) + (i - 1) * lda]
                }
            })
            .collect();
        apply_reflector(release, true, mi, 2, &v, tauq[i - 1], c, i - 1, ldc);
    }
}

/// `DORMBR('P', 'R', 'T', 2, 2, 2, A, LDA, TAUP, VT, 2)` (`NQ = K = 2`: `DORMLQ('R', 'N', 2,
/// 1, 1, A(1, 2), ...)` → `DORML2`): `G(1)` on `VT(1:2, 2)`, whose vector is its implicit 1
/// alone.
fn ormbr_p_right_t(release: LapackRelease, taup: &[f64], vt: &mut [f64]) {
    apply_reflector(release, false, 2, 1, &[1.0], taup[0], vt, 2, 2);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(a: &[f64], m: usize) {
        for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
            let out = gesdd_s2(a, m, release).unwrap();
            assert!(out.s[0] >= out.s[1] && out.s[1] >= 0.0);
            for i in 0..m {
                for j in 0..2 {
                    let mut v = 0.0;
                    for k in 0..2 {
                        v += out.u[i + k * m] * out.s[k] * out.vt[k + 2 * j];
                    }
                    assert!((v - a[i + j * m]).abs() < 1e-12, "{v} {}", a[i + j * m]);
                }
            }
            // U has orthonormal columns.
            for p in 0..2 {
                for q in 0..2 {
                    let d: f64 = (0..m).map(|i| out.u[i + p * m] * out.u[i + q * m]).sum();
                    assert!((d - if p == q { 1.0 } else { 0.0 }).abs() < 1e-12);
                }
            }
        }
    }

    #[test]
    fn reconstructs_tall_and_square() {
        let tall: Vec<f64> = (0..14).map(|k| ((k * 37 % 11) as f64) - 4.5).collect();
        check(&tall, 7);
        check(&[3.0, -1.0, 2.0, 5.0], 2);
        check(&[1.0, 2.0, 3.0, 2.0, 4.0, 6.0], 3);
    }

    #[test]
    fn errors() {
        assert_eq!(
            gesdd_s2(&[1.0, f64::NAN, 0.0, 1.0], 2, LapackRelease::V3_12_0),
            Err(GesddError::NotFinite)
        );
        assert_eq!(
            gesdd_s2(&[1.0, 2.0], 1, LapackRelease::V3_12_0),
            Err(GesddError::Unsupported { m: 1 })
        );
    }
}
