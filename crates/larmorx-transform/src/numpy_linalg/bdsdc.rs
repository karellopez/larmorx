// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! LAPACK's divide-and-conquer SVD of a bidiagonal matrix with its singular vectors (`DBDSDC`
//! with `COMPQ = 'I'`, both bidiagonal forms) and every routine under it: `DLASD0` (the
//! subproblem tree), `DLASDT` (the tree), `DLASD1` (one merge: `DLASD2` deflates, `DLASD3`
//! solves the secular equation with `DLASD4`, `DLASD5` and `DLAED6`, then multiplies the
//! vectors with `DGEMM`), `DLASDQ` (the leaves: `DBDSQR` of at most `SMLSIZ` = 25 rows),
//! `DLAMRG` (merging two sorted lists) and `DLASCL` (`TYPE = 'G'`), as numpy's x86-64 wheels run
//! them at one BLAS thread on the Haswell kernels (`OPENBLAS_CORETYPE=Haswell`), both LAPACK
//! releases, on every platform: the singular values and vectors of `numpy.linalg.svd`
//! (`DGESDD`) and `pinv` on the confounds' sizes (fMRIPrep's CompCor: bidiagonal matrices of up
//! to 600 rows).
//!
//! **The two releases** ([`LapackRelease`]; `lapack-netlib/SRC/` of OpenBLAS 0.3.30, numpy
//! 2.3.5's wheels, and of OpenBLAS v0.3.34-106-g446c436e, numpy 2.5.4's):
//!
//! - `DLASD2`'s deflation: 3.12.0 deflates a singular value whose `z` component is at most
//!   `TOL = 8·ε·max(|α|, |β|, |d(n)|)` and merges two neighbours whose difference is at most
//!   `TOL` in magnitude; 3.12.1 uses `64·ε·…` and the signed difference (the values are sorted:
//!   the same test), and when a merge follows values deflated for their small `z` it records the
//!   deflated positions again in order (`IDXP(K2 + J - 1 - JP) = JP` for `JP = JPREV … J - 1`,
//!   where 3.12.0 writes `IDXP(K2) = JPREV` alone): the order of the deflated vectors changes.
//! - `DBDSQR` (the leaves): 3.12.1 makes a `-0` singular value `+0` when the values are made
//!   positive; 3.12.0 keeps it.
//! - `DBDSDC`, `DLASCL` and `DLACPY` differ only in layout (and, for `DLASCL`/`DLACPY`, in the
//!   bound of a loop over the columns of a lower triangle that does nothing past `min(m, n)`):
//!   the same arithmetic. `DLASD0`, `DLASD1`, `DLASD3`, `DLASD4`, `DLASD5`, `DLASDQ`, `DLASDT`,
//!   `DLAMRG` and `DLAED6` are identical in both trees.
//!
//! **What decides the bits.** gfortran compiled LAPACK for the x86-64 baseline (no fused
//! multiply-adds), so every operation of these routines is rounded on its own, in the source's
//! order (Fortran's left-to-right evaluation; `x**2` is `x·x`); `SQRT` is the correctly rounded
//! square root, `LOG` (in `DLASDT`'s tree depth) glibc's `log` ([`log_glibc`]: the depth
//! `INT(log(n/26)/log(2)) + 1` sits on a rounding boundary where `n/26` is a power of two). The
//! BLAS calls go to the ported Haswell kernels: `DGEMM` ([`dgemm_with`], with the depth block of
//! [`Lapack`]), `DNRM2` (x87), `DROT` (fused), `DSWAP`, `DCOPY`. `DLASD3`'s products are parallel
//! over blocks of their output; the bits never depend on the number of threads. Only
//! `COMPQ = 'I'` is ported: `COMPQ = 'N'` (`DLASDQ` → `DBDSQR` → `DLASQ1`, numpy's `svd` without
//! vectors) and `'P'` (`DLASDA`, the compact form no numpy function asks for) are not on the
//! confounds' paths.
//!
//! Upstream files (BSD-3-Clause; the notices are in the parent module and NOTICE): OpenBLAS
//! 0.3.30 and v0.3.34-106-g446c436e `lapack-netlib/SRC/` (`dbdsdc.f`, `dlasd0.f`, `dlasd1.f`,
//! `dlasd2.f` (both), `dlasd3.f`, `dlasd4.f`, `dlasd5.f`, `dlaed6.f`, `dlasdq.f`, `dlasdt.f`,
//! `dlamrg.f`, `dlascl.f`, `dlacpy.f`, `dlaset.f`, `dbdsqr.f` (both), `ilaenv.f`,
//! `INSTALL/dlamch.f`); the BLAS files of [`crate::openblas`]. The logic is re-expressed in Rust;
//! no code was copied.

// The routines keep LAPACK's names, arguments and 1-based loops, so the port can be checked
// line by line against the Fortran.
#![allow(
    clippy::too_many_arguments,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::needless_late_init,
    clippy::collapsible_else_if
)]

use super::blocked::{IllegalValue, Lapack};
use super::{
    EPS, LapackRelease, SAFMIN, dbdsqr_upper_ncc, dlanst_max, dlapy2, dlartg, dlasr, drot, dswap,
    sign,
};
use crate::openblas::level1::{dcopy, dnrm2};
use crate::openblas::level3::{Operand, Trans, dgemm_with};
use crate::openblas::trmm::Uplo;
use larmorx_core::math::log_glibc;

/// `ILAENV(9, 'DBDSDC', ...)`: the largest subproblem solved by `DLASDQ` (both releases).
pub const SMLSIZ: usize = 25;

#[inline(always)]
fn ix(off: usize, ld: usize, i: usize, j: usize) -> usize {
    off + (i - 1) + (j - 1) * ld
}

fn illegal(routine: &'static str, info: u32) -> Result<(), IllegalValue> {
    if info == 0 {
        Ok(())
    } else {
        Err(IllegalValue {
            routine,
            param: info,
        })
    }
}

// ------------------------------------------------------------------------------------------
// Auxiliaries

/// `DLASCL('G', 0, 0, CFROM, CTO, M, N, A, LDA, INFO)`: `A *= CTO/CFROM` in steps that never
/// overflow or underflow (the `m × n` block of `a` at `aoff`). Refuses a zero or NaN `cfrom`
/// (`INFO = -4`), a NaN `cto` (-5) and `lda < max(1, m)` (-9), as LAPACK.
pub fn dlascl_g(
    cfrom: f64,
    cto: f64,
    m: usize,
    n: usize,
    a: &mut [f64],
    aoff: usize,
    lda: usize,
) -> Result<(), IllegalValue> {
    let info = if cfrom == 0.0 || cfrom.is_nan() {
        4
    } else if cto.is_nan() {
        5
    } else if lda < m.max(1) {
        9
    } else {
        0
    };
    illegal("DLASCL", info)?;
    if n == 0 || m == 0 {
        return Ok(());
    }
    let smlnum = SAFMIN;
    let bignum = 1.0 / smlnum;
    let mut cfromc = cfrom;
    let mut ctoc = cto;
    loop {
        let cfrom1 = cfromc * smlnum;
        let (mul, done);
        if cfrom1 == cfromc {
            // CFROMC is an infinity.
            mul = ctoc / cfromc;
            done = true;
        } else {
            let cto1 = ctoc / bignum;
            if cto1 == ctoc {
                // CTOC is 0 or an infinity.
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
                    return Ok(());
                }
            }
        }
        for j in 1..=n {
            for i in 1..=m {
                a[ix(aoff, lda, i, j)] *= mul;
            }
        }
        if done {
            return Ok(());
        }
    }
}

/// `DLASET('A', M, N, 0, 1, A, LDA)`: the identity in the `m × n` block at `aoff`.
pub(super) fn identity(m: usize, n: usize, a: &mut [f64], aoff: usize, lda: usize) {
    for j in 1..=n {
        for i in 1..=m {
            a[ix(aoff, lda, i, j)] = if i == j { 1.0 } else { 0.0 };
        }
    }
}

/// `DLACPY(UPLO, M, N, A, LDA, B, LDB)` with any `UPLO` but `'U'` and `'L'`: the whole block.
pub(super) fn dlacpy(
    m: usize,
    n: usize,
    a: &[f64],
    aoff: usize,
    lda: usize,
    b: &mut [f64],
    boff: usize,
    ldb: usize,
) {
    for j in 1..=n {
        for i in 1..=m {
            b[ix(boff, ldb, i, j)] = a[ix(aoff, lda, i, j)];
        }
    }
}

/// `DLAMRG(N1, N2, A, DTRD1, DTRD2, INDEX)`: the permutation (1-based, into `index[0..n1 +
/// n2]`) that merges the two sorted lists `a[0..n1]` (ascending when `dtrd1 > 0`, else
/// descending) and `a[n1..n1 + n2]` (likewise with `dtrd2`) into one ascending list; on a tie
/// the first list's entry comes first.
pub fn dlamrg(n1: usize, n2: usize, a: &[f64], dtrd1: i64, dtrd2: i64, index: &mut [usize]) {
    let (mut n1sv, mut n2sv) = (n1, n2);
    let mut ind1: i64 = if dtrd1 > 0 { 1 } else { n1 as i64 };
    let mut ind2: i64 = if dtrd2 > 0 {
        1 + n1 as i64
    } else {
        (n1 + n2) as i64
    };
    let mut i = 0usize;
    while n1sv > 0 && n2sv > 0 {
        if a[(ind1 - 1) as usize] <= a[(ind2 - 1) as usize] {
            index[i] = ind1 as usize;
            i += 1;
            ind1 += dtrd1;
            n1sv -= 1;
        } else {
            index[i] = ind2 as usize;
            i += 1;
            ind2 += dtrd2;
            n2sv -= 1;
        }
    }
    if n1sv == 0 {
        for _ in 0..n2sv {
            index[i] = ind2 as usize;
            i += 1;
            ind2 += dtrd2;
        }
    } else {
        for _ in 0..n1sv {
            index[i] = ind1 as usize;
            i += 1;
            ind1 += dtrd1;
        }
    }
}

/// The tree of `DLASDT(N, LVL, ND, INODE, NDIML, NDIMR, MSUB)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    /// `LVL`: the number of levels.
    pub lvl: i64,
    /// `ND`: the number of nodes.
    pub nd: usize,
    /// `INODE`: the centre of each node's subproblem (1-based rows).
    pub inode: Vec<i64>,
    /// `NDIML`: the size of each node's left child.
    pub ndiml: Vec<i64>,
    /// `NDIMR`: the size of each node's right child.
    pub ndimr: Vec<i64>,
}

/// `DLASDT(N, LVL, ND, INODE, NDIML, NDIMR, MSUB)`: the subproblem tree of an `n`-row problem
/// with leaves of at most `msub` rows. The depth uses glibc's `log` as gfortran calls it.
pub fn dlasdt(n: usize, msub: usize) -> Tree {
    let maxn = n.max(1);
    let temp = log_glibc(maxn as f64 / (msub + 1) as f64) / log_glibc(2.0);
    // INT: truncation towards zero.
    let lvl = temp as i64 + 1;
    let len = n.max(1);
    let mut inode = vec![0i64; len];
    let mut ndiml = vec![0i64; len];
    let mut ndimr = vec![0i64; len];
    let n = n as i64;
    let i = n / 2;
    inode[0] = i + 1;
    ndiml[0] = i;
    ndimr[0] = n - i - 1;
    let (mut il, mut ir, mut llst) = (0i64, 1i64, 1i64);
    for _nlvl in 1..lvl {
        for i in 0..llst {
            il += 2;
            ir += 2;
            let ncrnt = (llst + i - 1) as usize;
            let (l, r) = ((il - 1) as usize, (ir - 1) as usize);
            ndiml[l] = ndiml[ncrnt] / 2;
            ndimr[l] = ndiml[ncrnt] - ndiml[l] - 1;
            inode[l] = inode[ncrnt] - ndimr[l] - 1;
            ndiml[r] = ndimr[ncrnt] / 2;
            ndimr[r] = ndimr[ncrnt] - ndiml[r] - 1;
            inode[r] = inode[ncrnt] + ndiml[r] + 1;
        }
        llst *= 2;
    }
    Tree {
        lvl,
        nd: (llst * 2 - 1) as usize,
        inode,
        ndiml,
        ndimr,
    }
}

// ------------------------------------------------------------------------------------------
// The secular equation

/// `DLAED6(KNITER, ORGATI, RHO, D, Z, FINIT, TAU, INFO)`: one root of the rational function
/// `f(x) = rho + Σ z(i)/(d(i) - x)` near `finit`'s side; returns `(tau, info)` (`info = 1`: no
/// convergence in 40 steps).
pub fn dlaed6(
    kniter: usize,
    orgati: bool,
    rho: f64,
    d: &[f64; 3],
    z: &[f64; 3],
    finit: f64,
) -> (f64, usize) {
    const MAXIT: usize = 40;
    let (mut lbd, mut ubd) = if orgati { (d[1], d[2]) } else { (d[0], d[1]) };
    if finit < 0.0 {
        lbd = 0.0;
    } else {
        ubd = 0.0;
    }
    let niter = 1usize;
    let mut tau = 0.0f64;
    if kniter == 2 {
        let (mut a, mut b, mut c);
        if orgati {
            let temp = (d[2] - d[1]) / 2.0;
            c = rho + z[0] / ((d[0] - d[1]) - temp);
            a = c * (d[1] + d[2]) + z[1] + z[2];
            b = c * d[1] * d[2] + z[1] * d[2] + z[2] * d[1];
        } else {
            let temp = (d[0] - d[1]) / 2.0;
            c = rho + z[2] / ((d[2] - d[1]) - temp);
            a = c * (d[0] + d[1]) + z[0] + z[1];
            b = c * d[0] * d[1] + z[0] * d[1] + z[1] * d[0];
        }
        let temp = max3(a.abs(), b.abs(), c.abs());
        a /= temp;
        b /= temp;
        c /= temp;
        if c == 0.0 {
            tau = b / a;
        } else if a <= 0.0 {
            tau = (a - (a * a - 4.0 * b * c).abs().sqrt()) / (2.0 * c);
        } else {
            tau = 2.0 * b / (a + (a * a - 4.0 * b * c).abs().sqrt());
        }
        if tau < lbd || tau > ubd {
            tau = (lbd + ubd) / 2.0;
        }
        if d[0] == tau || d[1] == tau || d[2] == tau {
            tau = 0.0;
        } else {
            let temp = finit
                + tau * z[0] / (d[0] * (d[0] - tau))
                + tau * z[1] / (d[1] * (d[1] - tau))
                + tau * z[2] / (d[2] * (d[2] - tau));
            if temp <= 0.0 {
                lbd = tau;
            } else {
                ubd = tau;
            }
            if finit.abs() <= temp.abs() {
                tau = 0.0;
            }
        }
    }
    let eps = EPS;
    // SMALL1 = BASE**(INT(LOG(SAFMIN)/LOG(BASE)/THREE)) = 2**(-340).
    let small1 = small1();
    let sminv1 = 1.0 / small1;
    let small2 = small1 * small1;
    let sminv2 = sminv1 * sminv1;
    let temp = if orgati {
        (d[1] - tau).abs().min((d[2] - tau).abs())
    } else {
        (d[0] - tau).abs().min((d[1] - tau).abs())
    };
    let mut scale = false;
    let mut sclinv = 0.0;
    let mut dscale = *d;
    let mut zscale = *z;
    if temp <= small1 {
        scale = true;
        let sclfac;
        if temp <= small2 {
            sclfac = sminv2;
            sclinv = small2;
        } else {
            sclfac = sminv1;
            sclinv = small1;
        }
        for i in 0..3 {
            dscale[i] = d[i] * sclfac;
            zscale[i] = z[i] * sclfac;
        }
        tau *= sclfac;
        lbd *= sclfac;
        ubd *= sclfac;
    }
    let (mut fc, mut df, mut ddf) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..3 {
        let temp = 1.0 / (dscale[i] - tau);
        let temp1 = zscale[i] * temp;
        let temp2 = temp1 * temp;
        let temp3 = temp2 * temp;
        fc += temp1 / dscale[i];
        df += temp2;
        ddf += temp3;
    }
    let mut f = finit + tau * fc;
    let mut info = 0;
    'done: {
        if f.abs() <= 0.0 {
            break 'done;
        }
        if f <= 0.0 {
            lbd = tau;
        } else {
            ubd = tau;
        }
        for _niter in niter + 1..=MAXIT {
            let (temp1, temp2) = if orgati {
                (dscale[1] - tau, dscale[2] - tau)
            } else {
                (dscale[0] - tau, dscale[1] - tau)
            };
            let mut a = (temp1 + temp2) * f - temp1 * temp2 * df;
            let mut b = temp1 * temp2 * f;
            let mut c = f - (temp1 + temp2) * df + temp1 * temp2 * ddf;
            let temp = max3(a.abs(), b.abs(), c.abs());
            a /= temp;
            b /= temp;
            c /= temp;
            let mut eta = if c == 0.0 {
                b / a
            } else if a <= 0.0 {
                (a - (a * a - 4.0 * b * c).abs().sqrt()) / (2.0 * c)
            } else {
                2.0 * b / (a + (a * a - 4.0 * b * c).abs().sqrt())
            };
            if f * eta >= 0.0 {
                eta = -f / df;
            }
            tau += eta;
            if tau < lbd || tau > ubd {
                tau = (lbd + ubd) / 2.0;
            }
            fc = 0.0;
            let mut erretm = 0.0f64;
            df = 0.0;
            ddf = 0.0;
            for i in 0..3 {
                if dscale[i] - tau != 0.0 {
                    let temp = 1.0 / (dscale[i] - tau);
                    let temp1 = zscale[i] * temp;
                    let temp2 = temp1 * temp;
                    let temp3 = temp2 * temp;
                    let temp4 = temp1 / dscale[i];
                    fc += temp4;
                    erretm += temp4.abs();
                    df += temp2;
                    ddf += temp3;
                } else {
                    break 'done;
                }
            }
            f = finit + tau * fc;
            erretm = 8.0 * (finit.abs() + tau.abs() * erretm) + tau.abs() * df;
            if f.abs() <= 4.0 * eps * erretm || (ubd - lbd) <= 4.0 * eps * tau.abs() {
                break 'done;
            }
            if f <= 0.0 {
                lbd = tau;
            } else {
                ubd = tau;
            }
        }
        info = 1;
    }
    if scale {
        tau *= sclinv;
    }
    (tau, info)
}

/// Fortran's `MAX(A, B, C)` of finite values.
#[inline]
fn max3(a: f64, b: f64, c: f64) -> f64 {
    a.max(b).max(c)
}

/// `DLAED6`'s `SMALL1 = BASE**(INT(LOG(SAFMIN)/LOG(BASE)/THREE))` with glibc's `log`.
fn small1() -> f64 {
    let e = (log_glibc(SAFMIN) / log_glibc(2.0) / 3.0) as i32;
    2f64.powi(e)
}

/// `DLASD5(I, D, Z, DELTA, RHO, DSIGMA, WORK)`: the `i`-th singular value (1 or 2) of the 2 × 2
/// secular equation; `delta` and `work` (2 values each) receive `d(j) - σ` and `d(j) + σ`.
/// Returns `σ`.
pub fn dlasd5(
    i: usize,
    d: &[f64],
    z: &[f64],
    delta: &mut [f64],
    rho: f64,
    work: &mut [f64],
) -> f64 {
    let del = d[1] - d[0];
    let delsq = del * (d[1] + d[0]);
    let dsigma;
    if i == 1 {
        let w = 1.0
            + 4.0 * rho * (z[1] * z[1] / (d[0] + 3.0 * d[1]) - z[0] * z[0] / (3.0 * d[0] + d[1]))
                / del;
        if w > 0.0 {
            let b = delsq + rho * (z[0] * z[0] + z[1] * z[1]);
            let c = rho * z[0] * z[0] * delsq;
            let mut tau = 2.0 * c / (b + (b * b - 4.0 * c).abs().sqrt());
            tau = tau / (d[0] + (d[0] * d[0] + tau).sqrt());
            dsigma = d[0] + tau;
            delta[0] = -tau;
            delta[1] = del - tau;
            work[0] = 2.0 * d[0] + tau;
            work[1] = (d[0] + tau) + d[1];
        } else {
            let b = -delsq + rho * (z[0] * z[0] + z[1] * z[1]);
            let c = rho * z[1] * z[1] * delsq;
            let mut tau = if b > 0.0 {
                -2.0 * c / (b + (b * b + 4.0 * c).sqrt())
            } else {
                (b - (b * b + 4.0 * c).sqrt()) / 2.0
            };
            tau = tau / (d[1] + (d[1] * d[1] + tau).abs().sqrt());
            dsigma = d[1] + tau;
            delta[0] = -(del + tau);
            delta[1] = -tau;
            work[0] = d[0] + tau + d[1];
            work[1] = 2.0 * d[1] + tau;
        }
    } else {
        let b = -delsq + rho * (z[0] * z[0] + z[1] * z[1]);
        let c = rho * z[1] * z[1] * delsq;
        let mut tau = if b > 0.0 {
            (b + (b * b + 4.0 * c).sqrt()) / 2.0
        } else {
            2.0 * c / (-b + (b * b + 4.0 * c).sqrt())
        };
        tau = tau / (d[1] + (d[1] * d[1] + tau).sqrt());
        dsigma = d[1] + tau;
        delta[0] = -(del + tau);
        delta[1] = -tau;
        work[0] = d[0] + tau + d[1];
        work[1] = 2.0 * d[1] + tau;
    }
    dsigma
}

/// The scalar state `DLASD4`'s iterations recompute after each step.
struct Sums {
    psi: f64,
    dpsi: f64,
    phi: f64,
    dphi: f64,
    erretm: f64,
}

/// `PSI`, `DPSI` and `ERRETM` over `1..=lo`, then `PHI`, `DPHI` over `n` down to `hi` (adding
/// to `ERRETM` after its absolute value), with `Z(J)/(WORK(J)·DELTA(J))` as LAPACK forms them.
fn sums(n: usize, lo: usize, hi: usize, z: &[f64], work: &[f64], delta: &[f64]) -> Sums {
    let (mut psi, mut dpsi, mut erretm) = (0.0f64, 0.0f64, 0.0f64);
    for j in 1..=lo {
        let temp = z[j - 1] / (work[j - 1] * delta[j - 1]);
        psi += z[j - 1] * temp;
        dpsi += temp * temp;
        erretm += psi;
    }
    erretm = erretm.abs();
    let (mut phi, mut dphi) = (0.0f64, 0.0f64);
    let mut j = n;
    while j >= hi && j >= 1 {
        let temp = z[j - 1] / (work[j - 1] * delta[j - 1]);
        phi += z[j - 1] * temp;
        dphi += temp * temp;
        erretm += phi;
        j -= 1;
    }
    Sums {
        psi,
        dpsi,
        phi,
        dphi,
        erretm,
    }
}

/// `DLASD4(N, I, D, Z, DELTA, RHO, SIGMA, WORK, INFO)`: the `i`-th (1-based) singular value
/// `σ` of the secular equation `1 + rho·Σ z(j)²/((d(j) - σ)(d(j) + σ)) = 0` (`d` ascending,
/// `d(1) = 0` as `DLASD3` calls it); `delta` and `work` (`n` values each) receive `d(j) - σ` and
/// `d(j) + σ` as LAPACK accumulates them. Returns `(σ, info)` (`info = 1`: no convergence in
/// 400 steps).
pub fn dlasd4(
    n: usize,
    i: usize,
    d: &[f64],
    z: &[f64],
    delta: &mut [f64],
    rho: f64,
    work: &mut [f64],
) -> (f64, usize) {
    const MAXIT: usize = 400;
    macro_rules! dv {
        ($j:expr) => {
            d[$j - 1]
        };
    }
    macro_rules! zv {
        ($j:expr) => {
            z[$j - 1]
        };
    }
    macro_rules! de {
        ($j:expr) => {
            delta[$j - 1]
        };
    }
    macro_rules! wk {
        ($j:expr) => {
            work[$j - 1]
        };
    }
    if n == 1 {
        let sigma = (dv!(1) * dv!(1) + rho * zv!(1) * zv!(1)).sqrt();
        de!(1) = 1.0;
        wk!(1) = 1.0;
        return (sigma, 0);
    }
    if n == 2 {
        return (dlasd5(i, d, z, delta, rho, work), 0);
    }
    let eps = EPS;
    let rhoinv = 1.0 / rho;
    let mut tau2;
    let mut sigma;
    if i == n {
        // The last singular value.
        let ii = n - 1;
        let mut niter = 1usize;
        let temp = rho / 2.0;
        let temp1 = temp / (dv!(n) + (dv!(n) * dv!(n) + temp).sqrt());
        for j in 1..=n {
            wk!(j) = dv!(j) + dv!(n) + temp1;
            de!(j) = (dv!(j) - dv!(n)) - temp1;
        }
        let mut psi = 0.0f64;
        for j in 1..=n - 2 {
            psi += zv!(j) * zv!(j) / (de!(j) * wk!(j));
        }
        let c = rhoinv + psi;
        let w = c + zv!(ii) * zv!(ii) / (de!(ii) * wk!(ii)) + zv!(n) * zv!(n) / (de!(n) * wk!(n));
        let mut tau;
        let quadratic = |c: f64| -> f64 {
            let delsq = (dv!(n) - dv!(n - 1)) * (dv!(n) + dv!(n - 1));
            let a = -c * delsq + zv!(n - 1) * zv!(n - 1) + zv!(n) * zv!(n);
            let b = zv!(n) * zv!(n) * delsq;
            if a < 0.0 {
                2.0 * b / ((a * a + 4.0 * b * c).sqrt() - a)
            } else {
                (a + (a * a + 4.0 * b * c).sqrt()) / (2.0 * c)
            }
        };
        if w <= 0.0 {
            let temp1 = (dv!(n) * dv!(n) + rho).sqrt();
            let temp = zv!(n - 1) * zv!(n - 1)
                / ((dv!(n - 1) + temp1) * (dv!(n) - dv!(n - 1) + rho / (dv!(n) + temp1)))
                + zv!(n) * zv!(n) / rho;
            if c <= temp {
                tau = rho;
            } else {
                tau2 = quadratic(c);
                tau = tau2 / (dv!(n) + (dv!(n) * dv!(n) + tau2).sqrt());
            }
        } else {
            tau2 = quadratic(c);
            tau = tau2 / (dv!(n) + (dv!(n) * dv!(n) + tau2).sqrt());
        }
        sigma = dv!(n) + tau;
        for j in 1..=n {
            de!(j) = (dv!(j) - dv!(n)) - tau;
            wk!(j) = dv!(j) + dv!(n) + tau;
        }
        // The sums over 1..II (DELTA(J)*WORK(J)) and the last term.
        let (mut psi, mut dpsi, mut erretm) = (0.0f64, 0.0f64, 0.0f64);
        for j in 1..=ii {
            let temp = zv!(j) / (de!(j) * wk!(j));
            psi += zv!(j) * temp;
            dpsi += temp * temp;
            erretm += psi;
        }
        erretm = erretm.abs();
        let temp = zv!(n) / (de!(n) * wk!(n));
        let mut phi = zv!(n) * temp;
        let mut dphi = temp * temp;
        erretm = 8.0 * (-phi - psi) + erretm - phi + rhoinv;
        let mut w = rhoinv + phi + psi;
        if w.abs() <= eps * erretm {
            return (sigma, 0);
        }
        niter += 1;
        let dtnsq1 = wk!(n - 1) * de!(n - 1);
        let dtnsq = wk!(n) * de!(n);
        let mut c = w - dtnsq1 * dpsi - dtnsq * dphi;
        let a = (dtnsq + dtnsq1) * w - dtnsq * dtnsq1 * (dpsi + dphi);
        let b = dtnsq * dtnsq1 * w;
        if c < 0.0 {
            c = c.abs();
        }
        let mut eta = if c == 0.0 {
            rho - sigma * sigma
        } else if a >= 0.0 {
            (a + (a * a - 4.0 * b * c).abs().sqrt()) / (2.0 * c)
        } else {
            2.0 * b / (a - (a * a - 4.0 * b * c).abs().sqrt())
        };
        if w * eta > 0.0 {
            eta = -w / (dpsi + dphi);
        }
        let temp = eta - dtnsq;
        if temp > rho {
            eta = rho + dtnsq;
        }
        eta /= sigma + (eta + sigma * sigma).sqrt();
        tau += eta;
        sigma += eta;
        let _ = tau;
        for j in 1..=n {
            de!(j) -= eta;
            wk!(j) += eta;
        }
        let s = sums(n, ii, n + 1, z, work, delta);
        let (mut psi, mut dpsi, mut erretm) = (s.psi, s.dpsi, s.erretm);
        tau2 = wk!(n) * de!(n);
        let temp = zv!(n) / tau2;
        phi = zv!(n) * temp;
        dphi = temp * temp;
        erretm = 8.0 * (-phi - psi) + erretm - phi + rhoinv;
        w = rhoinv + phi + psi;
        for _niter in niter + 1..=MAXIT {
            if w.abs() <= eps * erretm {
                return (sigma, 0);
            }
            let dtnsq1 = wk!(n - 1) * de!(n - 1);
            let dtnsq = wk!(n) * de!(n);
            let c = w - dtnsq1 * dpsi - dtnsq * dphi;
            let a = (dtnsq + dtnsq1) * w - dtnsq1 * dtnsq * (dpsi + dphi);
            let b = dtnsq1 * dtnsq * w;
            let mut eta = if a >= 0.0 {
                (a + (a * a - 4.0 * b * c).abs().sqrt()) / (2.0 * c)
            } else {
                2.0 * b / (a - (a * a - 4.0 * b * c).abs().sqrt())
            };
            if w * eta > 0.0 {
                eta = -w / (dpsi + dphi);
            }
            let temp = eta - dtnsq;
            if temp <= 0.0 {
                eta /= 2.0;
            }
            eta /= sigma + (eta + sigma * sigma).sqrt();
            sigma += eta;
            for j in 1..=n {
                de!(j) -= eta;
                wk!(j) += eta;
            }
            let s = sums(n, ii, n + 1, z, work, delta);
            psi = s.psi;
            dpsi = s.dpsi;
            erretm = s.erretm;
            tau2 = wk!(n) * de!(n);
            let temp = zv!(n) / tau2;
            phi = zv!(n) * temp;
            dphi = temp * temp;
            erretm = 8.0 * (-phi - psi) + erretm - phi + rhoinv;
            w = rhoinv + phi + psi;
        }
        return (sigma, 1);
    }

    // 1 <= I < N.
    let mut niter = 1usize;
    let ip1 = i + 1;
    let delsq = (dv!(ip1) - dv!(i)) * (dv!(ip1) + dv!(i));
    let delsq2 = delsq / 2.0;
    let sq2 = ((dv!(i) * dv!(i) + dv!(ip1) * dv!(ip1)) / 2.0).sqrt();
    let temp = delsq2 / (dv!(i) + sq2);
    for j in 1..=n {
        wk!(j) = dv!(j) + dv!(i) + temp;
        de!(j) = (dv!(j) - dv!(i)) - temp;
    }
    let mut psi = 0.0f64;
    for j in 1..i {
        psi += zv!(j) * zv!(j) / (wk!(j) * de!(j));
    }
    let mut phi = 0.0f64;
    let mut j = n;
    while j >= i + 2 {
        phi += zv!(j) * zv!(j) / (wk!(j) * de!(j));
        j -= 1;
    }
    let c = rhoinv + psi + phi;
    let w = c + zv!(i) * zv!(i) / (wk!(i) * de!(i)) + zv!(ip1) * zv!(ip1) / (wk!(ip1) * de!(ip1));
    let mut geomavg = false;
    let orgati;
    let ii;
    let mut sglb;
    let mut sgub;
    let mut tau;
    if w > 0.0 {
        // d(i)^2 < the i-th sigma^2 < (d(i)^2 + d(i+1)^2)/2.
        orgati = true;
        ii = i;
        sglb = 0.0;
        sgub = delsq2 / (dv!(i) + sq2);
        let a = c * delsq + zv!(i) * zv!(i) + zv!(ip1) * zv!(ip1);
        let b = zv!(i) * zv!(i) * delsq;
        tau2 = if a > 0.0 {
            2.0 * b / (a + (a * a - 4.0 * b * c).abs().sqrt())
        } else {
            (a - (a * a - 4.0 * b * c).abs().sqrt()) / (2.0 * c)
        };
        tau = tau2 / (dv!(i) + (dv!(i) * dv!(i) + tau2).sqrt());
        let temp = eps.sqrt();
        if dv!(i) <= temp * dv!(ip1) && zv!(i).abs() <= temp && dv!(i) > 0.0 {
            tau = (10.0 * dv!(i)).min(sgub);
            geomavg = true;
        }
    } else {
        // (d(i)^2 + d(i+1)^2)/2 <= the i-th sigma^2 < d(i+1)^2.
        orgati = false;
        ii = ip1;
        sglb = -delsq2 / (dv!(ii) + sq2);
        sgub = 0.0;
        let a = c * delsq - zv!(i) * zv!(i) - zv!(ip1) * zv!(ip1);
        let b = zv!(ip1) * zv!(ip1) * delsq;
        tau2 = if a < 0.0 {
            2.0 * b / (a - (a * a + 4.0 * b * c).abs().sqrt())
        } else {
            -(a + (a * a + 4.0 * b * c).abs().sqrt()) / (2.0 * c)
        };
        tau = tau2 / (dv!(ip1) + (dv!(ip1) * dv!(ip1) + tau2).abs().sqrt());
    }
    sigma = dv!(ii) + tau;
    for j in 1..=n {
        wk!(j) = dv!(j) + dv!(ii) + tau;
        de!(j) = (dv!(j) - dv!(ii)) - tau;
    }
    let iim1 = ii - 1;
    let iip1 = ii + 1;
    let s = sums(n, iim1, iip1, z, work, delta);
    let (mut psi, mut dpsi, mut phi, mut dphi, mut erretm) =
        (s.psi, s.dpsi, s.phi, s.dphi, s.erretm);
    let _ = phi;
    let mut w = rhoinv + phi + psi;
    let mut swtch3 = false;
    if orgati {
        if w < 0.0 {
            swtch3 = true;
        }
    } else if w > 0.0 {
        swtch3 = true;
    }
    if ii == 1 || ii == n {
        swtch3 = false;
    }
    let temp = zv!(ii) / (wk!(ii) * de!(ii));
    let mut dw = dpsi + dphi + temp * temp;
    let temp = zv!(ii) * temp;
    w += temp;
    erretm = 8.0 * (phi - psi) + erretm + 2.0 * rhoinv + 3.0 * temp.abs();
    if w.abs() <= eps * erretm {
        return (sigma, 0);
    }
    if w <= 0.0 {
        sglb = sglb.max(tau);
    } else {
        sgub = sgub.min(tau);
    }
    niter += 1;
    // The step without the switch (the fixed-weight method on the two poles).
    let simple = |w: f64, dw: f64, dpsi: f64, dphi: f64, work: &[f64], delta: &[f64]| -> f64 {
        let dtipsq = work[ip1 - 1] * delta[ip1 - 1];
        let dtisq = work[i - 1] * delta[i - 1];
        let c = if orgati {
            let q = zv!(i) / dtisq;
            w - dtipsq * dw + delsq * (q * q)
        } else {
            let q = zv!(ip1) / dtipsq;
            w - dtisq * dw - delsq * (q * q)
        };
        let mut a = (dtipsq + dtisq) * w - dtipsq * dtisq * dw;
        let b = dtipsq * dtisq * w;
        if c == 0.0 {
            if a == 0.0 {
                a = if orgati {
                    zv!(i) * zv!(i) + dtipsq * dtipsq * (dpsi + dphi)
                } else {
                    zv!(ip1) * zv!(ip1) + dtisq * dtisq * (dpsi + dphi)
                };
            }
            b / a
        } else if a <= 0.0 {
            (a - (a * a - 4.0 * b * c).abs().sqrt()) / (2.0 * c)
        } else {
            2.0 * b / (a + (a * a - 4.0 * b * c).abs().sqrt())
        }
    };
    let mut eta;
    if !swtch3 {
        eta = simple(w, dw, dpsi, dphi, work, delta);
    } else {
        // Interpolation using the three nearest poles.
        let dtiim = wk!(iim1) * de!(iim1);
        let dtiip = wk!(iip1) * de!(iip1);
        let temp = rhoinv + psi + phi;
        let mut zz = [0.0f64; 3];
        let c;
        if orgati {
            let mut temp1 = zv!(iim1) / dtiim;
            temp1 *= temp1;
            c = (temp - dtiip * (dpsi + dphi))
                - (dv!(iim1) - dv!(iip1)) * (dv!(iim1) + dv!(iip1)) * temp1;
            zz[0] = zv!(iim1) * zv!(iim1);
            zz[2] = if dpsi < temp1 {
                dtiip * dtiip * dphi
            } else {
                dtiip * dtiip * ((dpsi - temp1) + dphi)
            };
        } else {
            let mut temp1 = zv!(iip1) / dtiip;
            temp1 *= temp1;
            c = (temp - dtiim * (dpsi + dphi))
                - (dv!(iip1) - dv!(iim1)) * (dv!(iim1) + dv!(iip1)) * temp1;
            zz[0] = if dphi < temp1 {
                dtiim * dtiim * dpsi
            } else {
                dtiim * dtiim * (dpsi + (dphi - temp1))
            };
            zz[2] = zv!(iip1) * zv!(iip1);
        }
        zz[1] = zv!(ii) * zv!(ii);
        let dd = [dtiim, de!(ii) * wk!(ii), dtiip];
        let (e, info) = dlaed6(niter, orgati, c, &dd, &zz, w);
        eta = e;
        if info != 0 {
            // DLAED6 failed: switch back to two poles.
            swtch3 = false;
            eta = simple(w, dw, dpsi, dphi, work, delta);
        }
    }
    // The step must make the function decrease (or increase) towards the root.
    if w * eta >= 0.0 {
        eta = -w / dw;
    }
    eta /= sigma + (sigma * sigma + eta).sqrt();
    let temp = tau + eta;
    if temp > sgub || temp < sglb {
        if w < 0.0 {
            eta = (sgub - tau) / 2.0;
        } else {
            eta = (sglb - tau) / 2.0;
        }
        if geomavg {
            if w < 0.0 {
                if tau > 0.0 {
                    eta = (sgub * tau).sqrt() - tau;
                }
            } else if sglb > 0.0 {
                eta = (sglb * tau).sqrt() - tau;
            }
        }
    }
    let mut prew = w;
    tau += eta;
    sigma += eta;
    for j in 1..=n {
        wk!(j) += eta;
        de!(j) -= eta;
    }
    let s = sums(n, iim1, iip1, z, work, delta);
    psi = s.psi;
    dpsi = s.dpsi;
    phi = s.phi;
    dphi = s.dphi;
    erretm = s.erretm;
    tau2 = wk!(ii) * de!(ii);
    let temp = zv!(ii) / tau2;
    dw = dpsi + dphi + temp * temp;
    let temp = zv!(ii) * temp;
    w = rhoinv + phi + psi + temp;
    erretm = 8.0 * (phi - psi) + erretm + 2.0 * rhoinv + 3.0 * temp.abs();
    let mut swtch = false;
    if orgati {
        if -w > prew.abs() / 10.0 {
            swtch = true;
        }
    } else if w > prew.abs() / 10.0 {
        swtch = true;
    }
    // Main loop to update the values of the array DELTA.
    for niter in niter + 1..=MAXIT {
        if w.abs() <= eps * erretm {
            return (sigma, 0);
        }
        if w <= 0.0 {
            sglb = sglb.max(tau);
        } else {
            sgub = sgub.min(tau);
        }
        // The two-pole step, with the switched weights when SWTCH.
        let two_poles =
            |w: f64, dw: f64, dpsi: &mut f64, dphi: &mut f64, work: &[f64], delta: &[f64]| -> f64 {
                let dtipsq = work[ip1 - 1] * delta[ip1 - 1];
                let dtisq = work[i - 1] * delta[i - 1];
                let c;
                if !swtch {
                    c = if orgati {
                        let q = zv!(i) / dtisq;
                        w - dtipsq * dw + delsq * (q * q)
                    } else {
                        let q = zv!(ip1) / dtipsq;
                        w - dtisq * dw - delsq * (q * q)
                    };
                } else {
                    let temp = zv!(ii) / (work[ii - 1] * delta[ii - 1]);
                    if orgati {
                        *dpsi += temp * temp;
                    } else {
                        *dphi += temp * temp;
                    }
                    c = w - dtisq * *dpsi - dtipsq * *dphi;
                }
                let mut a = (dtipsq + dtisq) * w - dtipsq * dtisq * dw;
                let b = dtipsq * dtisq * w;
                if c == 0.0 {
                    if a == 0.0 {
                        a = if !swtch {
                            if orgati {
                                zv!(i) * zv!(i) + dtipsq * dtipsq * (*dpsi + *dphi)
                            } else {
                                zv!(ip1) * zv!(ip1) + dtisq * dtisq * (*dpsi + *dphi)
                            }
                        } else {
                            dtisq * dtisq * *dpsi + dtipsq * dtipsq * *dphi
                        };
                    }
                    b / a
                } else if a <= 0.0 {
                    (a - (a * a - 4.0 * b * c).abs().sqrt()) / (2.0 * c)
                } else {
                    2.0 * b / (a + (a * a - 4.0 * b * c).abs().sqrt())
                }
            };
        let mut eta;
        if !swtch3 {
            eta = two_poles(w, dw, &mut dpsi, &mut dphi, work, delta);
        } else {
            // Interpolation using the three nearest poles.
            let dtiim = wk!(iim1) * de!(iim1);
            let dtiip = wk!(iip1) * de!(iip1);
            let temp = rhoinv + psi + phi;
            let mut zz = [0.0f64; 3];
            let c;
            if swtch {
                c = temp - dtiim * dpsi - dtiip * dphi;
                zz[0] = dtiim * dtiim * dpsi;
                zz[2] = dtiip * dtiip * dphi;
            } else if orgati {
                let mut temp1 = zv!(iim1) / dtiim;
                temp1 *= temp1;
                let temp2 = (dv!(iim1) - dv!(iip1)) * (dv!(iim1) + dv!(iip1)) * temp1;
                c = temp - dtiip * (dpsi + dphi) - temp2;
                zz[0] = zv!(iim1) * zv!(iim1);
                zz[2] = if dpsi < temp1 {
                    dtiip * dtiip * dphi
                } else {
                    dtiip * dtiip * ((dpsi - temp1) + dphi)
                };
            } else {
                let mut temp1 = zv!(iip1) / dtiip;
                temp1 *= temp1;
                let temp2 = (dv!(iip1) - dv!(iim1)) * (dv!(iim1) + dv!(iip1)) * temp1;
                c = temp - dtiim * (dpsi + dphi) - temp2;
                zz[0] = if dphi < temp1 {
                    dtiim * dtiim * dpsi
                } else {
                    dtiim * dtiim * (dpsi + (dphi - temp1))
                };
                zz[2] = zv!(iip1) * zv!(iip1);
            }
            zz[1] = zv!(ii) * zv!(ii);
            let dd = [dtiim, de!(ii) * wk!(ii), dtiip];
            let (e, info) = dlaed6(niter, orgati, c, &dd, &zz, w);
            eta = e;
            if info != 0 {
                // DLAED6 failed: switch back to two poles.
                swtch3 = false;
                eta = two_poles(w, dw, &mut dpsi, &mut dphi, work, delta);
            }
        }
        // The step must make the function decrease (or increase) towards the root.
        if w * eta >= 0.0 {
            eta = -w / dw;
        }
        eta /= sigma + (sigma * sigma + eta).sqrt();
        let temp = tau + eta;
        if temp > sgub || temp < sglb {
            if w < 0.0 {
                eta = (sgub - tau) / 2.0;
            } else {
                eta = (sglb - tau) / 2.0;
            }
            if geomavg {
                if w < 0.0 {
                    if tau > 0.0 {
                        eta = (sgub * tau).sqrt() - tau;
                    }
                } else if sglb > 0.0 {
                    eta = (sglb * tau).sqrt() - tau;
                }
            }
        }
        prew = w;
        tau += eta;
        sigma += eta;
        for j in 1..=n {
            wk!(j) += eta;
            de!(j) -= eta;
        }
        let s = sums(n, iim1, iip1, z, work, delta);
        psi = s.psi;
        dpsi = s.dpsi;
        phi = s.phi;
        dphi = s.dphi;
        erretm = s.erretm;
        tau2 = wk!(ii) * de!(ii);
        let temp = zv!(ii) / tau2;
        dw = dpsi + dphi + temp * temp;
        let temp = zv!(ii) * temp;
        w = rhoinv + phi + psi + temp;
        erretm = 8.0 * (phi - psi) + erretm + 2.0 * rhoinv + 3.0 * temp.abs();
        if w * prew > 0.0 && w.abs() > prew.abs() / 10.0 {
            swtch = !swtch;
        }
    }
    (sigma, 1)
}

// ------------------------------------------------------------------------------------------
// One merge: DLASD2, DLASD3, DLASD1

/// What `DLASD2` leaves for `DLASD3`: the number of non-deflated values and the column types'
/// counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deflation {
    /// `K`: the order of the secular equation (the values not deflated).
    pub k: usize,
    /// `CTOT(1:4)`: how many columns of each type (1: nonzero in the upper half only, 2: in the
    /// lower half only, 3: dense, 4: deflated).
    pub ctot: [usize; 4],
}

/// The arrays of one merge `DLASD2` fills (sizes from `n = nl + nr + 1`, `m = n + sqre`).
pub struct MergeWork<'a> {
    /// `Z` (`m`).
    pub z: &'a mut [f64],
    /// `DSIGMA` (`n`).
    pub dsigma: &'a mut [f64],
    /// `U2` (`ldu2 × n`).
    pub u2: &'a mut [f64],
    /// `LDU2` (`>= n`).
    pub ldu2: usize,
    /// `VT2` (`ldvt2 × m`).
    pub vt2: &'a mut [f64],
    /// `LDVT2` (`>= m`).
    pub ldvt2: usize,
    /// `IDXP` (`n`, 1-based positions).
    pub idxp: &'a mut [usize],
    /// `IDX` (`n`).
    pub idx: &'a mut [usize],
    /// `IDXC` (`n`).
    pub idxc: &'a mut [usize],
    /// `COLTYP` (`max(n, 4)`: on exit its first four entries are `CTOT`; LAPACK writes them
    /// into the next work array's first entry when `n = 3`, which nothing reads again).
    pub coltyp: &'a mut [usize],
}

/// `DLASD2(NL, NR, SQRE, K, D, Z, ALPHA, BETA, U, LDU, VT, LDVT, DSIGMA, U2, LDU2, VT2, LDVT2,
/// IDXP, IDX, IDXC, IDXQ, COLTYP, INFO)`: merges the two halves' singular values into one
/// sorted list, deflates (the release's tolerance and order, module documentation) and arranges
/// the vectors by column type. `d` holds the `n` values (`d[nl]` ignored), `u`/`vt` the
/// `n × n` and `m × m` blocks at `uoff`/`vtoff`, `idxq` the halves' sorting permutations.
pub fn dlasd2(
    release: LapackRelease,
    nl: usize,
    nr: usize,
    sqre: usize,
    d: &mut [f64],
    alpha: f64,
    beta: f64,
    u: &mut [f64],
    uoff: usize,
    ldu: usize,
    vt: &mut [f64],
    vtoff: usize,
    ldvt: usize,
    w: MergeWork<'_>,
    idxq: &mut [usize],
) -> Result<Deflation, IllegalValue> {
    let n = nl + nr + 1;
    let m = n + sqre;
    let mut info = if nl < 1 {
        1
    } else if nr < 1 {
        2
    } else if sqre > 1 {
        3
    } else {
        0
    };
    if ldu < n {
        info = 10;
    } else if ldvt < m {
        info = 12;
    } else if w.ldu2 < n {
        info = 15;
    } else if w.ldvt2 < m {
        info = 17;
    }
    illegal("DLASD2", info)?;
    let MergeWork {
        z,
        dsigma,
        u2,
        ldu2,
        vt2,
        ldvt2,
        idxp,
        idx,
        idxc,
        coltyp,
    } = w;
    macro_rules! dv {
        ($j:expr) => {
            d[$j - 1]
        };
    }
    macro_rules! zv {
        ($j:expr) => {
            z[$j - 1]
        };
    }
    let vat = |i: usize, j: usize| ix(vtoff, ldvt, i, j);
    let uat = |i: usize, j: usize| ix(uoff, ldu, i, j);
    let nlp1 = nl + 1;
    let nlp2 = nl + 2;
    // Generate the first part of the vector Z; and move the singular values in the first part
    // of D one position backward.
    let z1 = alpha * vt[vat(nlp1, nlp1)];
    zv!(1) = z1;
    for i in (1..=nl).rev() {
        zv!(i + 1) = alpha * vt[vat(i, nlp1)];
        dv!(i + 1) = dv!(i);
        idxq[i] = idxq[i - 1] + 1;
    }
    // Generate the second part of the vector Z.
    for i in nlp2..=m {
        zv!(i) = beta * vt[vat(i, nlp2)];
    }
    // Initialize some reference arrays.
    for i in 2..=nlp1 {
        coltyp[i - 1] = 1;
    }
    for i in nlp2..=n {
        coltyp[i - 1] = 2;
    }
    // Sort the singular values into increasing order.
    for i in nlp2..=n {
        idxq[i - 1] += nlp1;
    }
    // DSIGMA, IDXC, IDXC, and the first column of U2 are used as storage space.
    for i in 2..=n {
        let q = idxq[i - 1];
        dsigma[i - 1] = dv!(q);
        u2[ix(0, ldu2, i, 1)] = zv!(q);
        idxc[i - 1] = coltyp[q - 1];
    }
    dlamrg(nl, nr, &dsigma[1..], 1, 1, &mut idx[1..]);
    for i in 2..=n {
        let idxi = 1 + idx[i - 1];
        dv!(i) = dsigma[idxi - 1];
        zv!(i) = u2[ix(0, ldu2, idxi, 1)];
        coltyp[i - 1] = idxc[idxi - 1];
    }
    // Calculate the allowable deflation tolerance.
    let eps = EPS;
    let mut tol = alpha.abs().max(beta.abs());
    tol = match release {
        LapackRelease::V3_12_1 => 64.0 * eps * dv!(n).abs().max(tol),
        LapackRelease::V3_12_0 => 8.0 * eps * dv!(n).abs().max(tol),
    };
    // Deflation.
    let mut k = 1usize;
    let mut k2 = n + 1;
    let mut jprev = 0usize;
    let mut all_deflated = false;
    for j in 2..=n {
        if zv!(j).abs() <= tol {
            // Deflate due to small z component.
            k2 -= 1;
            idxp[k2 - 1] = j;
            coltyp[j - 1] = 4;
            if j == n {
                all_deflated = true;
                break;
            }
        } else {
            jprev = j;
            break;
        }
    }
    if !all_deflated {
        let mut j = jprev;
        loop {
            j += 1;
            if j > n {
                break;
            }
            if zv!(j).abs() <= tol {
                // Deflate due to small z component.
                k2 -= 1;
                idxp[k2 - 1] = j;
                coltyp[j - 1] = 4;
                continue;
            }
            // Check if singular values are close enough to allow deflation.
            let close = match release {
                LapackRelease::V3_12_1 => (dv!(j) - dv!(jprev)) <= tol,
                LapackRelease::V3_12_0 => (dv!(j) - dv!(jprev)).abs() <= tol,
            };
            if close {
                // Deflation is possible.
                let mut s = zv!(jprev);
                let mut c = zv!(j);
                // Find sqrt(a**2+b**2) without overflow or destructive underflow.
                let tau = dlapy2(c, s);
                c /= tau;
                s = -s / tau;
                zv!(j) = tau;
                zv!(jprev) = 0.0;
                // Apply back the Givens rotation to the left and right singular vector
                // matrices.
                let mut idxjp = idxq[idx[jprev - 1]];
                let mut idxj = idxq[idx[j - 1]];
                if idxjp <= nlp1 {
                    idxjp -= 1;
                }
                if idxj <= nlp1 {
                    idxj -= 1;
                }
                drot(n, u, uat(1, idxjp), 1, uat(1, idxj), 1, c, s);
                drot(m, vt, vat(idxjp, 1), ldvt, vat(idxj, 1), ldvt, c, s);
                if coltyp[j - 1] != coltyp[jprev - 1] {
                    coltyp[j - 1] = 3;
                }
                coltyp[jprev - 1] = 4;
                k2 -= 1;
                match release {
                    LapackRelease::V3_12_1 => {
                        for jp in jprev..j {
                            idxp[k2 + j - 1 - jp - 1] = jp;
                        }
                    }
                    LapackRelease::V3_12_0 => idxp[k2 - 1] = jprev,
                }
                jprev = j;
            } else {
                k += 1;
                u2[ix(0, ldu2, k, 1)] = zv!(jprev);
                dsigma[k - 1] = dv!(jprev);
                idxp[k - 1] = jprev;
                jprev = j;
            }
        }
        // Record the last singular value.
        k += 1;
        u2[ix(0, ldu2, k, 1)] = zv!(jprev);
        dsigma[k - 1] = dv!(jprev);
        idxp[k - 1] = jprev;
    }
    // Count up the total number of the various types of columns, then form a permutation
    // which positions the four column types into four groups of uniform structure (although
    // one or more of these groups may be empty).
    let mut ctot = [0usize; 4];
    for j in 2..=n {
        ctot[coltyp[j - 1] - 1] += 1;
    }
    // PSM(*) = Position in SubMatrix (of types 1 through 4).
    let mut psm = [2usize, 0, 0, 0];
    psm[1] = 2 + ctot[0];
    psm[2] = psm[1] + ctot[1];
    psm[3] = psm[2] + ctot[2];
    // Fill out the IDXC array so that the permutation which it induces will place all type-1
    // columns first, all type-2 columns next, then all type-3's, and finally all type-4's,
    // starting from the second column. This applies similarly to the rows of VT.
    for j in 2..=n {
        let jp = idxp[j - 1];
        let ct = coltyp[jp - 1];
        idxc[psm[ct - 1] - 1] = j;
        psm[ct - 1] += 1;
    }
    // Sort the singular values and corresponding singular vectors into DSIGMA, U2, and VT2
    // respectively. The singular values/vectors which were not deflated go into the first K
    // slots of DSIGMA, U2, and VT2 respectively, while those which were deflated go into the
    // last N - K slots, except that the first column/row will be treated separately.
    for j in 2..=n {
        let jp = idxp[j - 1];
        dsigma[j - 1] = dv!(jp);
        let mut idxj = idxq[idx[idxp[idxc[j - 1] - 1] - 1]];
        if idxj <= nlp1 {
            idxj -= 1;
        }
        dcopy(n, u, uat(1, idxj), 1, u2, ix(0, ldu2, 1, j), 1);
        dcopy(
            m,
            vt,
            vat(idxj, 1),
            ldvt as isize,
            vt2,
            ix(0, ldvt2, j, 1),
            ldvt2 as isize,
        );
    }
    // Determine DSIGMA(1), DSIGMA(2) and Z(1).
    dsigma[0] = 0.0;
    let hlftol = tol / 2.0;
    if dsigma[1].abs() <= hlftol {
        dsigma[1] = hlftol;
    }
    let (mut c, mut s) = (0.0f64, 0.0f64);
    if m > n {
        zv!(1) = dlapy2(z1, zv!(m));
        if zv!(1) <= tol {
            c = 1.0;
            s = 0.0;
            zv!(1) = tol;
        } else {
            c = z1 / zv!(1);
            s = zv!(m) / zv!(1);
        }
    } else if z1.abs() <= tol {
        zv!(1) = tol;
    } else {
        zv!(1) = z1;
    }
    // Move the rest of the updating row to Z.
    for t in 0..k - 1 {
        z[1 + t] = u2[ix(0, ldu2, 2 + t, 1)];
    }
    // Determine the first column of U2, the first row of VT2 and the last row of VT.
    for i in 1..=n {
        u2[ix(0, ldu2, i, 1)] = 0.0;
    }
    u2[ix(0, ldu2, nlp1, 1)] = 1.0;
    if m > n {
        for i in 1..=nlp1 {
            vt[vat(m, i)] = -s * vt[vat(nlp1, i)];
            vt2[ix(0, ldvt2, 1, i)] = c * vt[vat(nlp1, i)];
        }
        for i in nlp2..=m {
            vt2[ix(0, ldvt2, 1, i)] = s * vt[vat(m, i)];
            vt[vat(m, i)] = c * vt[vat(m, i)];
        }
    } else {
        dcopy(
            m,
            vt,
            vat(nlp1, 1),
            ldvt as isize,
            vt2,
            ix(0, ldvt2, 1, 1),
            ldvt2 as isize,
        );
    }
    if m > n {
        dcopy(
            m,
            vt,
            vat(m, 1),
            ldvt as isize,
            vt2,
            ix(0, ldvt2, m, 1),
            ldvt2 as isize,
        );
    }
    // The deflated singular values and their singular vectors go into the back of D, U, and V
    // respectively.
    if n > k {
        d[k..n].copy_from_slice(&dsigma[k..n]);
        dlacpy(
            n,
            n - k,
            u2,
            ix(0, ldu2, 1, k + 1),
            ldu2,
            u,
            uat(1, k + 1),
            ldu,
        );
        dlacpy(
            n - k,
            m,
            vt2,
            ix(0, ldvt2, k + 1, 1),
            ldvt2,
            vt,
            vat(k + 1, 1),
            ldvt,
        );
    }
    // Copy CTOT into COLTYP for referencing in DLASD3.
    coltyp[..4].copy_from_slice(&ctot);
    Ok(Deflation { k, ctot })
}

/// `DGEMM('N', 'N', M, N, K, 1, A(aoff), LDA, B(boff), LDB, BETA, C(coff), LDC)`.
fn gemm_nn(
    gemm_q: usize,
    m: usize,
    n: usize,
    k: usize,
    a: (&[f64], usize, usize),
    b: (&[f64], usize, usize),
    beta: f64,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) {
    fn op((data, off, ld): (&[f64], usize, usize)) -> Operand<'_> {
        Operand {
            data,
            off,
            ld,
            trans: Trans::N,
        }
    }
    dgemm_with(m, n, k, 1.0, op(a), op(b), beta, c, coff, ldc, gemm_q);
}

/// `DLASD3(NL, NR, SQRE, K, D, Q, LDQ, DSIGMA, U, LDU, U2, LDU2, VT, LDVT, VT2, LDVT2, IDXC,
/// CTOT, Z, INFO)`: the `k` singular values of the secular equation (`DLASD4` for each), then
/// the updated singular vectors (`DGEMM` on the column types' blocks). `q` has room for `ldq ×
/// k` (`ldq >= k`); `ctot` is `DLASD2`'s. Returns `DLASD4`'s `info` (0, or 1 when one value did
/// not converge: the routine stops there).
pub fn dlasd3(
    gemm_q: usize,
    nl: usize,
    nr: usize,
    sqre: usize,
    k: usize,
    d: &mut [f64],
    q: &mut [f64],
    ldq: usize,
    dsigma: &[f64],
    u: &mut [f64],
    uoff: usize,
    ldu: usize,
    u2: &[f64],
    ldu2: usize,
    vt: &mut [f64],
    vtoff: usize,
    ldvt: usize,
    vt2: &mut [f64],
    ldvt2: usize,
    idxc: &[usize],
    ctot: &[usize; 4],
    z: &mut [f64],
) -> Result<usize, IllegalValue> {
    let n = nl + nr + 1;
    let m = n + sqre;
    let mut info = if nl < 1 {
        1
    } else if nr < 1 {
        2
    } else if sqre > 1 {
        3
    } else {
        0
    };
    let nlp1 = nl + 1;
    let nlp2 = nl + 2;
    if k < 1 || k > n {
        info = 4;
    } else if ldq < k {
        info = 7;
    } else if ldu < n {
        info = 10;
    } else if ldu2 < n {
        info = 12;
    } else if ldvt < m {
        info = 14;
    } else if ldvt2 < m {
        info = 16;
    }
    illegal("DLASD3", info)?;
    let uat = |i: usize, j: usize| ix(uoff, ldu, i, j);
    let vat = |i: usize, j: usize| ix(vtoff, ldvt, i, j);
    let qat = |i: usize, j: usize| ix(0, ldq, i, j);
    let u2at = |i: usize, j: usize| ix(0, ldu2, i, j);
    let vt2at = |i: usize, j: usize| ix(0, ldvt2, i, j);
    // Quick return if possible.
    if k == 1 {
        d[0] = z[0].abs();
        dcopy(
            m,
            vt2,
            vt2at(1, 1),
            ldvt2 as isize,
            vt,
            vat(1, 1),
            ldvt as isize,
        );
        if z[0] > 0.0 {
            dcopy(n, u2, u2at(1, 1), 1, u, uat(1, 1), 1);
        } else {
            for i in 1..=n {
                u[uat(i, 1)] = -u2[u2at(i, 1)];
            }
        }
        return Ok(0);
    }
    // Keep a copy of Z.
    q[..k].copy_from_slice(&z[..k]);
    // Normalize Z.
    let mut rho = dnrm2(k, z, 0, 1);
    let _ = dlascl_g(rho, 1.0, k, 1, z, 0, k);
    rho *= rho;
    // Find the new singular values.
    for j in 1..=k {
        let (delta, work) = (uat(1, j), vat(1, j));
        let (sigma, inf) = dlasd4(
            k,
            j,
            dsigma,
            z,
            &mut u[delta..delta + k],
            rho,
            &mut vt[work..work + k],
        );
        d[j - 1] = sigma;
        if inf != 0 {
            return Ok(inf);
        }
    }
    // Compute updated Z.
    for i in 1..=k {
        let mut zi = u[uat(i, k)] * vt[vat(i, k)];
        for j in 1..i {
            zi = zi
                * (u[uat(i, j)] * vt[vat(i, j)]
                    / (dsigma[i - 1] - dsigma[j - 1])
                    / (dsigma[i - 1] + dsigma[j - 1]));
        }
        for j in i..k {
            zi = zi
                * (u[uat(i, j)] * vt[vat(i, j)]
                    / (dsigma[i - 1] - dsigma[j])
                    / (dsigma[i - 1] + dsigma[j]));
        }
        z[i - 1] = sign(zi.abs().sqrt(), q[qat(i, 1)]);
    }
    // Compute left singular vectors of the modified diagonal matrix, and store related
    // information for the right singular vectors.
    for i in 1..=k {
        vt[vat(1, i)] = z[0] / u[uat(1, i)] / vt[vat(1, i)];
        u[uat(1, i)] = -1.0;
        for j in 2..=k {
            vt[vat(j, i)] = z[j - 1] / u[uat(j, i)] / vt[vat(j, i)];
            u[uat(j, i)] = dsigma[j - 1] * vt[vat(j, i)];
        }
        let temp = dnrm2(k, u, uat(1, i), 1);
        q[qat(1, i)] = u[uat(1, i)] / temp;
        for j in 2..=k {
            let jc = idxc[j - 1];
            q[qat(j, i)] = u[uat(jc, i)] / temp;
        }
    }
    // Update the left singular vector matrix.
    'vt: {
        if k == 2 {
            gemm_nn(
                gemm_q,
                n,
                k,
                k,
                (u2, 0, ldu2),
                (q, 0, ldq),
                0.0,
                u,
                uat(1, 1),
                ldu,
            );
            break 'vt;
        }
        if ctot[0] > 0 {
            gemm_nn(
                gemm_q,
                nl,
                k,
                ctot[0],
                (u2, u2at(1, 2), ldu2),
                (q, qat(2, 1), ldq),
                0.0,
                u,
                uat(1, 1),
                ldu,
            );
            if ctot[2] > 0 {
                let ktemp = 2 + ctot[0] + ctot[1];
                gemm_nn(
                    gemm_q,
                    nl,
                    k,
                    ctot[2],
                    (u2, u2at(1, ktemp), ldu2),
                    (q, qat(ktemp, 1), ldq),
                    1.0,
                    u,
                    uat(1, 1),
                    ldu,
                );
            }
        } else if ctot[2] > 0 {
            let ktemp = 2 + ctot[0] + ctot[1];
            gemm_nn(
                gemm_q,
                nl,
                k,
                ctot[2],
                (u2, u2at(1, ktemp), ldu2),
                (q, qat(ktemp, 1), ldq),
                0.0,
                u,
                uat(1, 1),
                ldu,
            );
        } else {
            dlacpy(nl, k, u2, 0, ldu2, u, uat(1, 1), ldu);
        }
        dcopy(k, q, 0, ldq as isize, u, uat(nlp1, 1), ldu as isize);
        let ktemp = 2 + ctot[0];
        let ctemp = ctot[1] + ctot[2];
        gemm_nn(
            gemm_q,
            nr,
            k,
            ctemp,
            (u2, u2at(nlp2, ktemp), ldu2),
            (q, qat(ktemp, 1), ldq),
            0.0,
            u,
            uat(nlp2, 1),
            ldu,
        );
    }
    // Generate the right singular vectors.
    for i in 1..=k {
        let temp = dnrm2(k, vt, vat(1, i), 1);
        q[qat(i, 1)] = vt[vat(1, i)] / temp;
        for j in 2..=k {
            let jc = idxc[j - 1];
            q[qat(i, j)] = vt[vat(jc, i)] / temp;
        }
    }
    // Update the right singular vector matrix.
    if k == 2 {
        gemm_nn(
            gemm_q,
            k,
            m,
            k,
            (q, 0, ldq),
            (vt2, 0, ldvt2),
            0.0,
            vt,
            vat(1, 1),
            ldvt,
        );
        return Ok(0);
    }
    let ktemp = 1 + ctot[0];
    gemm_nn(
        gemm_q,
        k,
        nlp1,
        ktemp,
        (q, qat(1, 1), ldq),
        (vt2, vt2at(1, 1), ldvt2),
        0.0,
        vt,
        vat(1, 1),
        ldvt,
    );
    let ktemp = 2 + ctot[0] + ctot[1];
    if ktemp <= ldvt2 {
        // With CTOT(3) = 0 this is a product of depth 0 (C unchanged), whose operand A starts
        // past Q's K columns: LAPACK's WORK array, never read.
        let qoff = qat(1, ktemp).min(q.len());
        gemm_nn(
            gemm_q,
            k,
            nlp1,
            ctot[2],
            (q, qoff, ldq),
            (vt2, vt2at(ktemp, 1), ldvt2),
            1.0,
            vt,
            vat(1, 1),
            ldvt,
        );
    }
    let ktemp = ctot[0] + 1;
    let nrp1 = nr + sqre;
    if ktemp > 1 {
        for i in 1..=k {
            q[qat(i, ktemp)] = q[qat(i, 1)];
        }
        for i in nlp2..=m {
            vt2[vt2at(ktemp, i)] = vt2[vt2at(1, i)];
        }
    }
    let ctemp = 1 + ctot[1] + ctot[2];
    gemm_nn(
        gemm_q,
        k,
        nrp1,
        ctemp,
        (q, qat(1, ktemp), ldq),
        (vt2, vt2at(ktemp, nlp2), ldvt2),
        0.0,
        vt,
        vat(1, nlp2),
        ldvt,
    );
    Ok(0)
}

/// `DLASD1(NL, NR, SQRE, D, ALPHA, BETA, U, LDU, VT, LDVT, IDXQ, IWORK, WORK, INFO)`: the SVD of
/// the `n × m` upper bidiagonal matrix made of two solved halves (`d[0..nl]`, `d[nl + 1..n]`
/// ascending, with their vectors in the blocks of `u` and `vt`) and the row `alpha`, `beta`
/// between them (`n = nl + nr + 1`, `m = n + sqre`). On exit `d` holds the values (`DLASD3`'s
/// `k` first, then the deflated ones), `idxq` the permutation that sorts them ascending, and
/// `alpha`, `beta` LAPACK's scaled values. Returns `DLASD3`'s `info`.
pub fn dlasd1(
    lp: Lapack,
    nl: usize,
    nr: usize,
    sqre: usize,
    d: &mut [f64],
    alpha: &mut f64,
    beta: &mut f64,
    u: &mut [f64],
    uoff: usize,
    ldu: usize,
    vt: &mut [f64],
    vtoff: usize,
    ldvt: usize,
    idxq: &mut [usize],
) -> Result<usize, IllegalValue> {
    let info = if nl < 1 {
        1
    } else if nr < 1 {
        2
    } else if sqre > 1 {
        3
    } else {
        0
    };
    illegal("DLASD1", info)?;
    let n = nl + nr + 1;
    let m = n + sqre;
    let (ldu2, ldvt2) = (n, m);
    // Scale.
    let mut orgnrm = alpha.abs().max(beta.abs());
    d[nl] = 0.0;
    for i in 1..=n {
        if d[i - 1].abs() > orgnrm {
            orgnrm = d[i - 1].abs();
        }
    }
    // A zero ORGNRM makes DLASCL refuse (its INFO is overwritten): nothing is scaled.
    let _ = dlascl_g(orgnrm, 1.0, n, 1, d, 0, n);
    *alpha /= orgnrm;
    *beta /= orgnrm;
    // Deflate singular values.
    let mut z = vec![0.0f64; m];
    let mut dsigma = vec![0.0f64; n];
    let mut u2 = vec![0.0f64; ldu2 * n];
    let mut vt2 = vec![0.0f64; ldvt2 * m];
    let (mut idxp, mut idx, mut idxc, mut coltyp) = (
        vec![0usize; n],
        vec![0usize; n],
        vec![0usize; n],
        vec![0usize; n.max(4)],
    );
    let defl = dlasd2(
        lp.release,
        nl,
        nr,
        sqre,
        d,
        *alpha,
        *beta,
        u,
        uoff,
        ldu,
        vt,
        vtoff,
        ldvt,
        MergeWork {
            z: &mut z,
            dsigma: &mut dsigma,
            u2: &mut u2,
            ldu2,
            vt2: &mut vt2,
            ldvt2,
            idxp: &mut idxp,
            idx: &mut idx,
            idxc: &mut idxc,
            coltyp: &mut coltyp,
        },
        idxq,
    )?;
    // Solve the secular equation and update the singular vectors.
    let k = defl.k;
    let ldq = k;
    let mut q = vec![0.0f64; ldq * k];
    let info = dlasd3(
        lp.gemm_q, nl, nr, sqre, k, d, &mut q, ldq, &dsigma, u, uoff, ldu, &u2, ldu2, vt, vtoff,
        ldvt, &mut vt2, ldvt2, &idxc, &defl.ctot, &mut z,
    )?;
    if info != 0 {
        return Ok(info);
    }
    // Unscale.
    let _ = dlascl_g(1.0, orgnrm, n, 1, d, 0, n);
    // Prepare the IDXQ sorting permutation.
    dlamrg(k, n - k, d, 1, -1, idxq);
    Ok(0)
}

// ------------------------------------------------------------------------------------------
// The leaves, the tree and the driver

/// `DLASDQ(UPLO, SQRE, N, NCVT, NRU, NCC, D, E, VT, LDVT, U, LDU, C, LDC, WORK, INFO)`: the SVD
/// of an `n × (n + sqre)` bidiagonal matrix by `DBDSQR`, its rotations applied to `vt`
/// (`ncvt` columns, the block at `vtoff`), `u` (`nru` rows) and `c` (`ncc` columns), the values
/// sorted ascending. `d` has `n` values, `e` `n - 1 + sqre`. Returns `DBDSQR`'s `info`. With no
/// vectors at all (`ncvt = nru = ncc = 0`, `n >= 2`) LAPACK's `DBDSQR` finds the values by
/// `DLASQ1` (the dqds algorithm), which this port does not follow: no caller asks for it
/// ([`super::lasda::dlasda`] refuses that case).
pub fn dlasdq(
    release: LapackRelease,
    uplo: Uplo,
    sqre: usize,
    n: usize,
    ncvt: usize,
    nru: usize,
    ncc: usize,
    d: &mut [f64],
    e: &mut [f64],
    vt: &mut [f64],
    vtoff: usize,
    ldvt: usize,
    u: &mut [f64],
    uoff: usize,
    ldu: usize,
    c: &mut [f64],
    coff: usize,
    ldc: usize,
) -> Result<usize, IllegalValue> {
    let info = if sqre > 1 {
        2
    } else if (ncvt == 0 && ldvt < 1) || (ncvt > 0 && ldvt < n.max(1)) {
        10
    } else if ldu < nru.max(1) {
        12
    } else if (ncc == 0 && ldc < 1) || (ncc > 0 && ldc < n.max(1)) {
        14
    } else {
        0
    };
    illegal("DLASDQ", info)?;
    if n == 0 {
        return Ok(0);
    }
    // ROTATE is true if any singular vectors desired, false otherwise.
    let rotate = ncvt > 0 || nru > 0 || ncc > 0;
    let np1 = n + 1;
    let mut sqre1 = sqre;
    let mut lower = uplo == Uplo::Lower;
    let mut work = vec![0.0f64; 2 * n];
    // If matrix non-square upper bidiagonal, rotate to be lower bidiagonal. The rotations are
    // on the right.
    if !lower && sqre1 == 1 {
        for i in 1..n {
            let (cs, sn, r) = dlartg(d[i - 1], e[i - 1]);
            d[i - 1] = r;
            e[i - 1] = sn * d[i];
            d[i] = cs * d[i];
            if rotate {
                work[i - 1] = cs;
                work[n + i - 1] = sn;
            }
        }
        let (cs, sn, r) = dlartg(d[n - 1], e[n - 1]);
        d[n - 1] = r;
        e[n - 1] = 0.0;
        if rotate {
            work[n - 1] = cs;
            work[2 * n - 1] = sn;
        }
        lower = true;
        sqre1 = 0;
        // Update singular vectors if desired.
        if ncvt > 0 {
            let (cw, sw) = work.split_at(n);
            dlasr(true, true, np1, ncvt, cw, sw, vt, vtoff, ldvt);
        }
    }
    // If matrix lower bidiagonal, rotate to be upper bidiagonal by applying Givens rotations on
    // the left.
    if lower {
        for i in 1..n {
            let (cs, sn, r) = dlartg(d[i - 1], e[i - 1]);
            d[i - 1] = r;
            e[i - 1] = sn * d[i];
            d[i] = cs * d[i];
            if rotate {
                work[i - 1] = cs;
                work[n + i - 1] = sn;
            }
        }
        // If matrix (N+1)-by-N lower bidiagonal, one additional rotation is needed.
        if sqre1 == 1 {
            let (cs, sn, r) = dlartg(d[n - 1], e[n - 1]);
            d[n - 1] = r;
            if rotate {
                work[n - 1] = cs;
                work[2 * n - 1] = sn;
            }
        }
        // Update singular vectors if desired.
        let (cw, sw) = work.split_at(n);
        if nru > 0 {
            let cols = if sqre1 == 0 { n } else { np1 };
            dlasr(false, true, nru, cols, cw, sw, u, uoff, ldu);
        }
        if ncc > 0 {
            let rows = if sqre1 == 0 { n } else { np1 };
            dlasr(true, true, rows, ncc, cw, sw, c, coff, ldc);
        }
    }
    // Call DBDSQR to compute the SVD of the reduced real N-by-N upper bidiagonal matrix.
    let empty: &mut [f64] = &mut [];
    let vts = if ncvt > 0 {
        &mut vt[vtoff..]
    } else {
        &mut *empty
    };
    let mut empty2: [f64; 0] = [];
    let us = if nru > 0 {
        &mut u[uoff..]
    } else {
        &mut empty2[..]
    };
    let mut empty3: [f64; 0] = [];
    let cs = if ncc > 0 {
        &mut c[coff..]
    } else {
        &mut empty3[..]
    };
    let info = dbdsqr_upper_ncc(
        n,
        d,
        e,
        (vts, ncvt, ldvt),
        (us, nru, ldu),
        (cs, ncc, ldc),
        release == LapackRelease::V3_12_1,
    );
    // Sort the singular values into ascending order (insertion sort on singular values, but
    // only one transposition per singular vector).
    for i in 1..=n {
        // Scan for smallest D(I).
        let mut isub = i;
        let mut smin = d[i - 1];
        for j in i + 1..=n {
            if d[j - 1] < smin {
                isub = j;
                smin = d[j - 1];
            }
        }
        if isub != i {
            // Swap singular values and vectors.
            d[isub - 1] = d[i - 1];
            d[i - 1] = smin;
            if ncvt > 0 {
                dswap(
                    ncvt,
                    vt,
                    ix(vtoff, ldvt, isub, 1),
                    ldvt,
                    ix(vtoff, ldvt, i, 1),
                    ldvt,
                );
            }
            if nru > 0 {
                dswap(nru, u, ix(uoff, ldu, 1, isub), 1, ix(uoff, ldu, 1, i), 1);
            }
            if ncc > 0 {
                dswap(
                    ncc,
                    c,
                    ix(coff, ldc, isub, 1),
                    ldc,
                    ix(coff, ldc, i, 1),
                    ldc,
                );
            }
        }
    }
    Ok(info)
}

/// `DLASD0(N, SQRE, D, E, U, LDU, VT, LDVT, SMLSIZ, IWORK, WORK, INFO)`: the SVD of an `n × (n +
/// sqre)` upper bidiagonal matrix by divide and conquer (leaves of at most `smlsiz` rows by
/// [`dlasdq`], merges by [`dlasd1`]); `u` (`n × n`) and `vt` (`m × m`) at `uoff`/`vtoff` receive
/// the vectors (their blocks are overwritten), `d` the values (ascending within the tree's
/// sorting, as LAPACK leaves them). Returns the first nonzero `info` of a leaf or merge.
pub fn dlasd0(
    lp: Lapack,
    n: usize,
    sqre: usize,
    d: &mut [f64],
    e: &mut [f64],
    u: &mut [f64],
    uoff: usize,
    ldu: usize,
    vt: &mut [f64],
    vtoff: usize,
    ldvt: usize,
    smlsiz: usize,
) -> Result<usize, IllegalValue> {
    let mut info = if sqre > 1 { 2 } else { 0 };
    let m = n + sqre;
    if ldu < n {
        info = 6;
    } else if ldvt < m {
        info = 8;
    } else if smlsiz < 3 {
        info = 9;
    }
    illegal("DLASD0", info)?;
    // If the input matrix is too small, call DLASDQ to find the SVD.
    if n <= smlsiz {
        return dlasdq(
            lp.release,
            Uplo::Upper,
            sqre,
            n,
            m,
            n,
            0,
            d,
            e,
            vt,
            vtoff,
            ldvt,
            u,
            uoff,
            ldu,
            &mut [],
            0,
            ldu,
        );
    }
    // Set up the computation tree.
    let tree = dlasdt(n, smlsiz);
    let nlvl = tree.lvl;
    let nd = tree.nd;
    let node = |i: usize| {
        (
            tree.inode[i - 1] as usize,
            tree.ndiml[i - 1] as usize,
            tree.ndimr[i - 1] as usize,
        )
    };
    let mut idxq = vec![0usize; n];
    // For the nodes on the bottom level of the tree, solve their subproblems by DLASDQ.
    let ndb1 = nd.div_ceil(2);
    for i in ndb1..=nd {
        let (ic, nl, nr) = node(i);
        let nlp1 = nl + 1;
        let nlf = ic - nl;
        let nrf = ic + 1;
        let info = dlasdq(
            lp.release,
            Uplo::Upper,
            1,
            nl,
            nlp1,
            nl,
            0,
            &mut d[nlf - 1..],
            &mut e[nlf - 1..],
            vt,
            ix(vtoff, ldvt, nlf, nlf),
            ldvt,
            u,
            ix(uoff, ldu, nlf, nlf),
            ldu,
            &mut [],
            0,
            ldu,
        )?;
        if info != 0 {
            return Ok(info);
        }
        for j in 1..=nl {
            idxq[nlf + j - 2] = j;
        }
        let sqrei = if i == nd { sqre } else { 1 };
        let nrp1 = nr + sqrei;
        let info = dlasdq(
            lp.release,
            Uplo::Upper,
            sqrei,
            nr,
            nrp1,
            nr,
            0,
            &mut d[nrf - 1..],
            &mut e[nrf - 1..],
            vt,
            ix(vtoff, ldvt, nrf, nrf),
            ldvt,
            u,
            ix(uoff, ldu, nrf, nrf),
            ldu,
            &mut [],
            0,
            ldu,
        )?;
        if info != 0 {
            return Ok(info);
        }
        for j in 1..=nr {
            idxq[ic + j - 1] = j;
        }
    }
    // Now conquer each subproblem bottom-up.
    let mut lvl = nlvl;
    while lvl >= 1 {
        // Find the first node LF and last node LL on the current level LVL.
        let (lf, ll) = if lvl == 1 {
            (1usize, 1usize)
        } else {
            let lf = 1usize << (lvl - 1);
            (lf, 2 * lf - 1)
        };
        for i in lf..=ll {
            let (ic, nl, nr) = node(i);
            let nlf = ic - nl;
            let sqrei = if sqre == 0 && i == ll { sqre } else { 1 };
            let mut alpha = d[ic - 1];
            let mut beta = e[ic - 1];
            let info = dlasd1(
                lp,
                nl,
                nr,
                sqrei,
                &mut d[nlf - 1..],
                &mut alpha,
                &mut beta,
                u,
                ix(uoff, ldu, nlf, nlf),
                ldu,
                vt,
                ix(vtoff, ldvt, nlf, nlf),
                ldvt,
                &mut idxq[nlf - 1..],
            )?;
            if info != 0 {
                return Ok(info);
            }
        }
        lvl -= 1;
    }
    Ok(0)
}

/// `DBDSDC(UPLO, 'I', N, D, E, U, LDU, VT, LDVT, Q, IQ, WORK, IWORK, INFO)`: the singular values
/// and vectors of an `n × n` upper or lower bidiagonal matrix (`d`: `n` values, `e`: `n - 1`),
/// `B = U·diag(σ)·VT`, the values descending in `d`, `u` and `vt` (`n × n` at offset 0, leading
/// dimensions `ldu`, `ldvt`) overwritten. Divide and conquer above [`SMLSIZ`] rows, `DLASDQ`
/// below. Returns LAPACK's `info` (0; else a leaf's `DBDSQR` or a merge's `DLASD4` did not
/// converge), or the refused argument (`LDU < N`: 7, `LDVT < N`: 9).
pub fn dbdsdc(
    lp: Lapack,
    uplo: Uplo,
    n: usize,
    d: &mut [f64],
    e: &mut [f64],
    u: &mut [f64],
    ldu: usize,
    vt: &mut [f64],
    ldvt: usize,
) -> Result<usize, IllegalValue> {
    let info = if ldu < 1 || ldu < n {
        7
    } else if ldvt < 1 || ldvt < n {
        9
    } else {
        0
    };
    illegal("DBDSDC", info)?;
    if n == 0 {
        return Ok(0);
    }
    let smlsiz = SMLSIZ;
    if n == 1 {
        u[0] = sign(1.0, d[0]);
        vt[0] = 1.0;
        d[0] = d[0].abs();
        return Ok(0);
    }
    let nm1 = n - 1;
    let lower = uplo == Uplo::Lower;
    // If matrix lower bidiagonal, rotate to be upper bidiagonal by applying Givens rotations on
    // the left (kept to be applied to U at the end).
    let mut rot = vec![0.0f64; if lower { 2 * nm1 } else { 0 }];
    if lower {
        for i in 1..n {
            let (cs, sn, r) = dlartg(d[i - 1], e[i - 1]);
            d[i - 1] = r;
            e[i - 1] = sn * d[i];
            d[i] = cs * d[i];
            rot[i - 1] = cs;
            rot[nm1 + i - 1] = -sn;
        }
    }
    let mut info = 0;
    'sort: {
        // If matrix is small enough, call DLASDQ to find the SVD.
        if n <= smlsiz {
            identity(n, n, u, 0, ldu);
            identity(n, n, vt, 0, ldvt);
            info = dlasdq(
                lp.release,
                Uplo::Upper,
                0,
                n,
                n,
                n,
                0,
                d,
                e,
                vt,
                0,
                ldvt,
                u,
                0,
                ldu,
                &mut [],
                0,
                ldu,
            )?;
            break 'sort;
        }
        identity(n, n, u, 0, ldu);
        identity(n, n, vt, 0, ldvt);
        // Scale.
        let orgnrm = dlanst_max(&d[..n], &e[..nm1]);
        if orgnrm == 0.0 {
            return Ok(0);
        }
        // A NaN ORGNRM makes DLASCL refuse (XERBLA's message; DBDSDC ignores its IERR): the
        // values stay unscaled.
        let _ = dlascl_g(orgnrm, 1.0, n, 1, d, 0, n);
        let _ = dlascl_g(orgnrm, 1.0, nm1, 1, e, 0, nm1);
        let eps = 0.9 * EPS;
        for i in 1..=n {
            if d[i - 1].abs() < eps {
                d[i - 1] = sign(eps, d[i - 1]);
            }
        }
        let mut start = 1usize;
        let sqre = 0;
        for i in 1..=nm1 {
            if e[i - 1].abs() < eps || i == nm1 {
                // Subproblem found. First determine its size and then apply divide and conquer
                // on it.
                let nsize;
                if i < nm1 {
                    // A subproblem with E(I) small for I < NM1.
                    nsize = i - start + 1;
                } else if e[i - 1].abs() >= eps {
                    // A subproblem with E(NM1) not too small but I = NM1.
                    nsize = n - start + 1;
                } else {
                    // A subproblem with E(NM1) small. This implies a 1-by-1 subproblem at
                    // D(N). Solve this 1-by-1 problem first.
                    nsize = i - start + 1;
                    u[ix(0, ldu, n, n)] = sign(1.0, d[n - 1]);
                    vt[ix(0, ldvt, n, n)] = 1.0;
                    d[n - 1] = d[n - 1].abs();
                }
                let inf = dlasd0(
                    lp,
                    nsize,
                    sqre,
                    &mut d[start - 1..],
                    &mut e[start - 1..],
                    u,
                    ix(0, ldu, start, start),
                    ldu,
                    vt,
                    ix(0, ldvt, start, start),
                    ldvt,
                    smlsiz,
                )?;
                if inf != 0 {
                    return Ok(inf);
                }
                start = i + 1;
            }
        }
        // Unscale.
        let _ = dlascl_g(1.0, orgnrm, n, 1, d, 0, n);
    }
    // Label 40: use selection sort to minimize swaps of singular vectors.
    for ii in 2..=n {
        let i = ii - 1;
        let mut kk = i;
        let mut p = d[i - 1];
        for j in ii..=n {
            if d[j - 1] > p {
                kk = j;
                p = d[j - 1];
            }
        }
        if kk != i {
            d[kk - 1] = d[i - 1];
            d[i - 1] = p;
            dswap(n, u, ix(0, ldu, 1, i), 1, ix(0, ldu, 1, kk), 1);
            dswap(n, vt, ix(0, ldvt, i, 1), ldvt, ix(0, ldvt, kk, 1), ldvt);
        }
    }
    // If B is lower bidiagonal, update U by those Givens rotations which rotated B to be upper
    // bidiagonal.
    if lower {
        let (cw, sw) = rot.split_at(nm1);
        dlasr(true, false, n, n, cw, sw, u, 0, ldu);
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng(seed: u64) -> impl FnMut() -> f64 {
        let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        }
    }

    #[test]
    fn small1_is_two_to_minus_340() {
        assert_eq!(small1(), 2f64.powi(-340));
        assert_eq!(small1().to_bits(), 0x2AB0_0000_0000_0000);
    }

    #[test]
    fn dlasdt_depth_at_powers_of_two() {
        // n / 26 a power of two: glibc's log gives the exact quotient here.
        for (n, lvl) in [(26, 1), (52, 2), (104, 3), (208, 4), (416, 5), (832, 6)] {
            assert_eq!(dlasdt(n, 25).lvl, lvl, "n = {n}");
        }
        assert_eq!(dlasdt(51, 25).lvl, 1);
        let t = dlasdt(600, 25);
        assert_eq!(t.nd, (1 << t.lvl) - 1);
    }

    fn reconstruct_error(
        upper: bool,
        n: usize,
        d0: &[f64],
        e0: &[f64],
        s: &[f64],
        u: &[f64],
        vt: &[f64],
    ) -> f64 {
        let mut worst = 0.0f64;
        for i in 0..n {
            for j in 0..n {
                let mut b = 0.0;
                if i == j {
                    b = d0[i];
                } else if upper && j == i + 1 {
                    b = e0[i];
                } else if !upper && i == j + 1 {
                    b = e0[j];
                }
                let mut r = 0.0;
                for k in 0..n {
                    r += u[i + k * n] * s[k] * vt[k + j * n];
                }
                worst = worst.max((r - b).abs());
            }
        }
        worst
    }

    #[test]
    fn dbdsdc_reconstructs_and_sorts() {
        for (seed, &n) in [1usize, 2, 3, 25, 26, 27, 60, 131].iter().enumerate() {
            for upper in [true, false] {
                for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
                    let mut r = rng(seed as u64 + 7);
                    let d0: Vec<f64> = (0..n).map(|_| r()).collect();
                    let e0: Vec<f64> = (0..n.saturating_sub(1)).map(|_| r()).collect();
                    let (mut d, mut e) = (d0.clone(), e0.clone());
                    let (mut u, mut vt) = (vec![0.0; n * n], vec![0.0; n * n]);
                    let uplo = if upper { Uplo::Upper } else { Uplo::Lower };
                    let info = dbdsdc(
                        Lapack::new(release),
                        uplo,
                        n,
                        &mut d,
                        &mut e,
                        &mut u,
                        n,
                        &mut vt,
                        n,
                    )
                    .unwrap();
                    assert_eq!(info, 0);
                    assert!(
                        d.windows(2).all(|w| w[0] >= w[1]),
                        "n = {n}: not descending"
                    );
                    assert!(d.iter().all(|v| *v >= 0.0));
                    let err = reconstruct_error(upper, n, &d0, &e0, &d, &u, &vt);
                    assert!(err < 1e-13 * (n as f64), "n = {n} upper = {upper}: {err}");
                }
            }
        }
    }

    #[test]
    fn dbdsdc_threads_do_not_change_the_bits() {
        let n = 300;
        let mut r = rng(11);
        let d0: Vec<f64> = (0..n).map(|_| r()).collect();
        let e0: Vec<f64> = (0..n - 1).map(|_| r()).collect();
        let run = |threads: usize| {
            let (mut d, mut e) = (d0.clone(), e0.clone());
            let (mut u, mut vt) = (vec![0.0; n * n], vec![0.0; n * n]);
            larmorx_core::parallel::with_threads(threads, || {
                dbdsdc(
                    Lapack::default(),
                    Uplo::Upper,
                    n,
                    &mut d,
                    &mut e,
                    &mut u,
                    n,
                    &mut vt,
                    n,
                )
            })
            .unwrap()
            .unwrap();
            (d, u, vt)
        };
        let one = run(1);
        let five = run(5);
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&one.0), bits(&five.0));
        assert_eq!(bits(&one.1), bits(&five.1));
        assert_eq!(bits(&one.2), bits(&five.2));
    }

    #[test]
    fn deflation_with_repeated_values() {
        // Equal diagonal and tiny off-diagonal entries: the merges deflate (both releases).
        let n = 80;
        for release in [LapackRelease::V3_12_0, LapackRelease::V3_12_1] {
            let d0 = vec![1.0f64; n];
            let e0: Vec<f64> = (0..n - 1).map(|i| 1e-9 * ((i % 7) as f64 - 3.0)).collect();
            let (mut d, mut e) = (d0.clone(), e0.clone());
            let (mut u, mut vt) = (vec![0.0; n * n], vec![0.0; n * n]);
            let info = dbdsdc(
                Lapack::new(release),
                Uplo::Upper,
                n,
                &mut d,
                &mut e,
                &mut u,
                n,
                &mut vt,
                n,
            )
            .unwrap();
            assert_eq!(info, 0);
            let err = reconstruct_error(true, n, &d0, &e0, &d, &u, &vt);
            assert!(err < 1e-12, "{err}");
        }
    }

    #[test]
    fn illegal_arguments() {
        let mut d = vec![1.0; 3];
        let mut e = vec![1.0; 2];
        let (mut u, mut vt) = (vec![0.0; 9], vec![0.0; 9]);
        let err = dbdsdc(
            Lapack::default(),
            Uplo::Upper,
            3,
            &mut d,
            &mut e,
            &mut u,
            2,
            &mut vt,
            3,
        )
        .unwrap_err();
        assert_eq!((err.routine, err.param), ("DBDSDC", 7));
        let err = dlascl_g(0.0, 1.0, 1, 1, &mut d, 0, 1).unwrap_err();
        assert_eq!(err.param, 4);
    }
}
