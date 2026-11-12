// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The divide-and-conquer bidiagonal SVD in its factored form, as `DLALSD` uses it for a
//! bidiagonal of more than 25 rows (LAPACK's `DLASDA` tree, its merge `DLASD6` with `DLASD7` and
//! `DLASD8`, and the solves `DLALSA`/`DLALS0`), both LAPACK releases, as OpenBLAS's `lapack-netlib`
//! carries them (numpy 2.5.4: LAPACK 3.12.1; numpy 2.3.5 and fMRIPrep 25.2.6's numpy: 3.12.0).
//!
//! **One merge** ([`dlasd6`]): the two halves' singular values (`D(1:NL)` and `D(NL+2:N)`, each
//! sorted by its `IDXQ`) and the two rows `VF`/`VL` of their right singular vectors' first and
//! last components; `ORGNRM = max(|ALPHA|, |BETA|, |D(i)|)` with `D(NL+1)` cleared; `D` scaled by
//! `DLASCL` and `ALPHA`, `BETA` divided by `ORGNRM`; [`dlasd7`] (the new `Z`, the merged sorted
//! list, the deflation of small `Z` entries and close values with a Givens rotation on `VF` and
//! `VL` each, recorded in `GIVCOL`/`GIVNUM` with `PERM` for `DLALS0`; for `SQRE = 1` the last row
//! rotated into the first; the release's test `|D(j) - D(jprev)| <= TOL` (3.12.0) or `D(j) -
//! D(jprev) <= TOL` (3.12.1) and its record of the deflated positions, as `DLASD2`); [`dlasd8`] (the
//! secular equation's roots by `DLASD4` for the `K` values left, the updated `Z` by the product
//! formula, `DIFL`/`DIFR` and the normalised `VF`/`VL` through `DDOT` and `DNRM2`); with `ICOMPQ =
//! 1` the poles (`D` and `DSIGMA`) copied out; `D` unscaled and `IDXQ` rebuilt by `DLAMRG`.
//!
//! **The tree** ([`dlasda`], `dlasda.f`, identical in both trees; the release enters through
//! `DLASDQ`'s `DBDSQR` and `DLASD7`): `DLASDT`'s subproblems, the leaves by `DLASDQ`, the merges
//! bottom-up by [`dlasd6`], every node's factors kept in the compact form [`SvdTree`] (`ICOMPQ =
//! 1`, as `DLALSD` asks for it). **The solves** ([`dlals0`], [`dlalsa`]; `dlals0.f`, `dlalsa.f`,
//! identical in both trees, no callee that differs): one node's left factor (Givens rotations,
//! `PERM`, the rows of the secular equation's vectors normalised by `DNRM2`, applied by
//! `DGEMV('T')`, `DLASCL`) or right factor (the transposed vectors by `DGEMV('T')`, the extra row's
//! rotation, `PERM`, the Givens rotations undone), and the tree's application: the leaves' `Uᵀ`
//! then the nodes bottom-up, or the nodes top-down then the leaves' `VTᵀ` (`DGEMM('T', 'N')` of
//! depth at most `SMLSIZ + 1`). `DLAMC3(A, B)` is the sum `A + B`, rounded once.
//!
//! Upstream files (BSD-3-Clause; the notices are in the parent module and NOTICE): OpenBLAS
//! 0.3.30 and v0.3.34-106-g446c436e `lapack-netlib/SRC/` (`dlasd6.f` and `dlasd8.f`, identical in
//! both; `dlasd7.f`, which differs in its deflation test and the record of the deflated positions;
//! `dlasda.f`, `dlals0.f`, `dlalsa.f`, identical in both; `dlasd4.f`, `dlasdq.f`, `dlasdt.f`,
//! `dlamrg.f`, `dlapy2.f`, `dlascl.f`, `dlaset.f`, `dlacpy.f`, `dlamch.f`), `interface/rot.c`,
//! `dot.c`, `nrm2.c`, `gemv.c`, `scal.c`, `copy.c`, `gemm.c`, `kernel/x86_64/drot.c`, `ddot.c`,
//! `nrm2.S`, `dgemv_t_4.c`, `dscal.c`. The logic is re-expressed in Rust; no code was copied.

// The routines keep LAPACK's names, arguments and 1-based loops, so the port can be checked line
// by line against the Fortran.
#![allow(
    clippy::too_many_arguments,
    clippy::many_single_char_names,
    clippy::needless_range_loop
)]

use super::bdsdc::{dlacpy, dlamrg, dlascl_g, dlasd4, dlasdq, dlasdt, identity};
use super::blas::ddot;
use super::blocked::IllegalValue;
use super::{EPS, LapackRelease, dlapy2, drot, sign};
use crate::openblas::level1::{dcopy, dnrm2, dscal};
use crate::openblas::level2::dgemv;
use crate::openblas::level3::{Operand, Trans, dgemm_with};
use crate::openblas::trmm::Uplo;

fn illegal(routine: &'static str, param: u32) -> Result<(), IllegalValue> {
    Err(IllegalValue { routine, param })
}

/// What [`dlasd7`] returns besides its arrays: `K` (the values not deflated), `GIVPTR` (the
/// Givens rotations recorded; with `ICOMPQ = 0` the value given) and `C`, `S` (the rotation of the
/// extra row for `SQRE = 1`; otherwise the last deflation rotation's, or the values given, as
/// LAPACK leaves them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lasd7 {
    /// `K`.
    pub k: usize,
    /// `GIVPTR`.
    pub givptr: usize,
    /// `C`.
    pub c: f64,
    /// `S`.
    pub s: f64,
}

/// The Givens record of a merge (`ICOMPQ = 1`): `PERM` (`n`, 1-based), `GIVCOL` (`ldgcol × 2`,
/// 1-based row indices) and `GIVNUM` (`ldgnum × 2`), column-major.
pub struct GivensRecord<'a> {
    /// `PERM`.
    pub perm: &'a mut [usize],
    /// `GIVCOL`.
    pub givcol: &'a mut [usize],
    /// `LDGCOL` (`>= n`).
    pub ldgcol: usize,
    /// `GIVNUM`.
    pub givnum: &'a mut [f64],
    /// `LDGNUM` (`>= n`).
    pub ldgnum: usize,
}

/// `DLASD7(ICOMPQ, NL, NR, SQRE, K, D, Z, ZW, VF, VFW, VL, VLW, ALPHA, BETA, DSIGMA, IDX, IDXP,
/// IDXQ, PERM, GIVPTR, GIVCOL, LDGCOL, GIVNUM, LDGNUM, C, S, INFO)`: the merge of the two halves
/// with deflation (module documentation). `d` (`n = nl + nr + 1` values), `z`, `vf`, `vl` (`m = n +
/// sqre`), `dsigma` (`n`), `idxq` (`n`, 1-based: the halves' sorting permutations, modified as
/// LAPACK modifies it); `rec` is written with `icompq = 1`. `givptr`, `c`, `s` start as given.
///
/// # Errors
/// `ICOMPQ` not 0 or 1 (1), `NL < 1` (2), `NR < 1` (3), `SQRE` not 0 or 1 (4), `LDGCOL < N` (22),
/// `LDGNUM < N` (24), as LAPACK.
pub fn dlasd7(
    release: LapackRelease,
    icompq: usize,
    nl: usize,
    nr: usize,
    sqre: usize,
    d: &mut [f64],
    z: &mut [f64],
    vf: &mut [f64],
    vl: &mut [f64],
    alpha: f64,
    beta: f64,
    dsigma: &mut [f64],
    idxq: &mut [usize],
    rec: &mut GivensRecord<'_>,
    givptr: usize,
    c: f64,
    s: f64,
) -> Result<Lasd7, IllegalValue> {
    let n = nl + nr + 1;
    let m = n + sqre;
    if icompq > 1 {
        illegal("DLASD7", 1)?;
    } else if nl < 1 {
        illegal("DLASD7", 2)?;
    } else if nr < 1 {
        illegal("DLASD7", 3)?;
    } else if sqre > 1 {
        illegal("DLASD7", 4)?;
    } else if rec.ldgcol < n {
        illegal("DLASD7", 22)?;
    } else if rec.ldgnum < n {
        illegal("DLASD7", 24)?;
    }
    macro_rules! at {
        ($a:expr, $i:expr) => {
            $a[$i - 1]
        };
    }
    let (mut givptr, mut c, mut s) = (givptr, c, s);
    let mut zw = vec![0.0f64; m];
    let mut vfw = vec![0.0f64; m];
    let mut vlw = vec![0.0f64; m];
    let mut idx = vec![0usize; n];
    let mut idxp = vec![0usize; n];
    let nlp1 = nl + 1;
    let nlp2 = nl + 2;
    if icompq == 1 {
        givptr = 0;
    }
    // Generate the first part of the vector Z and move the singular values in the first part of
    // D one position backward.
    let z1 = alpha * at!(vl, nlp1);
    at!(vl, nlp1) = 0.0;
    let tau = at!(vf, nlp1);
    for i in (1..=nl).rev() {
        at!(z, i + 1) = alpha * at!(vl, i);
        at!(vl, i) = 0.0;
        at!(vf, i + 1) = at!(vf, i);
        at!(d, i + 1) = at!(d, i);
        at!(idxq, i + 1) = at!(idxq, i) + 1;
    }
    at!(vf, 1) = tau;
    // Generate the second part of the vector Z.
    for i in nlp2..=m {
        at!(z, i) = beta * at!(vf, i);
        at!(vf, i) = 0.0;
    }
    // Sort the singular values into increasing order.
    for i in nlp2..=n {
        at!(idxq, i) += nlp1;
    }
    // DSIGMA, IDXC, IDXC, and ZW are used as storage space.
    for i in 2..=n {
        let q = at!(idxq, i);
        at!(dsigma, i) = at!(d, q);
        at!(zw, i) = at!(z, q);
        at!(vfw, i) = at!(vf, q);
        at!(vlw, i) = at!(vl, q);
    }
    dlamrg(nl, nr, &dsigma[1..], 1, 1, &mut idx[1..]);
    for i in 2..=n {
        let idxi = 1 + at!(idx, i);
        at!(d, i) = at!(dsigma, idxi);
        at!(z, i) = at!(zw, idxi);
        at!(vf, i) = at!(vfw, idxi);
        at!(vl, i) = at!(vlw, idxi);
    }
    // Calculate the allowable deflation tolerance.
    let eps = EPS; // DLAMCH('Epsilon')
    let tol = alpha.abs().max(beta.abs());
    let tol = 8.0 * 8.0 * eps * at!(d, n).abs().max(tol);
    // There are 2 kinds of deflation -- first a value in the z-vector is small, second two (or
    // more) singular values are very close together (their difference is small).
    let mut k = 1usize;
    let mut k2 = n + 1;
    let mut jprev = 0usize;
    'scan: {
        let mut found = false;
        for j in 2..=n {
            if at!(z, j).abs() <= tol {
                // Deflate due to small z component.
                k2 -= 1;
                at!(idxp, k2) = j;
                if j == n {
                    break 'scan;
                }
            } else {
                jprev = j;
                found = true;
                break;
            }
        }
        if !found {
            // Every j deflated (the loop ended at N inside the branch above).
            break 'scan;
        }
        let mut j = jprev;
        loop {
            j += 1;
            if j > n {
                break;
            }
            if at!(z, j).abs() <= tol {
                // Deflate due to small z component.
                k2 -= 1;
                at!(idxp, k2) = j;
            } else {
                // Check if singular values are close enough to allow deflation.
                let close = match release {
                    LapackRelease::V3_12_0 => (at!(d, j) - at!(d, jprev)).abs() <= tol,
                    LapackRelease::V3_12_1 => (at!(d, j) - at!(d, jprev)) <= tol,
                };
                if close {
                    // Deflation is possible.
                    s = at!(z, jprev);
                    c = at!(z, j);
                    // Find sqrt(a**2+b**2) without overflow or destructive underflow.
                    let tau = dlapy2(c, s);
                    at!(z, j) = tau;
                    at!(z, jprev) = 0.0;
                    c /= tau;
                    s = -s / tau;
                    // Record the appropriate Givens rotation.
                    if icompq == 1 {
                        givptr += 1;
                        let mut idxjp = at!(idxq, at!(idx, jprev) + 1);
                        let mut idxj = at!(idxq, at!(idx, j) + 1);
                        if idxjp <= nlp1 {
                            idxjp -= 1;
                        }
                        if idxj <= nlp1 {
                            idxj -= 1;
                        }
                        rec.givcol[(givptr - 1) + rec.ldgcol] = idxjp;
                        rec.givcol[givptr - 1] = idxj;
                        rec.givnum[(givptr - 1) + rec.ldgnum] = c;
                        rec.givnum[givptr - 1] = s;
                    }
                    drot(1, vf, jprev - 1, 1, j - 1, 1, c, s);
                    drot(1, vl, jprev - 1, 1, j - 1, 1, c, s);
                    k2 -= 1;
                    match release {
                        LapackRelease::V3_12_0 => at!(idxp, k2) = jprev,
                        LapackRelease::V3_12_1 => {
                            for jp in jprev..j {
                                at!(idxp, k2 + j - 1 - jp) = jp;
                            }
                        }
                    }
                    jprev = j;
                } else {
                    k += 1;
                    at!(zw, k) = at!(z, jprev);
                    at!(dsigma, k) = at!(d, jprev);
                    at!(idxp, k) = jprev;
                    jprev = j;
                }
            }
        }
        // Record the last singular value.
        k += 1;
        at!(zw, k) = at!(z, jprev);
        at!(dsigma, k) = at!(d, jprev);
        at!(idxp, k) = jprev;
    }
    // Sort the singular values into DSIGMA. The singular values which were not deflated go into
    // the first K slots of DSIGMA, except that DSIGMA(1) is treated separately.
    for j in 2..=n {
        let jp = at!(idxp, j);
        at!(dsigma, j) = at!(d, jp);
        at!(vfw, j) = at!(vf, jp);
        at!(vlw, j) = at!(vl, jp);
    }
    if icompq == 1 {
        for j in 2..=n {
            let jp = at!(idxp, j);
            let mut p = at!(idxq, at!(idx, jp) + 1);
            if p <= nlp1 {
                p -= 1;
            }
            at!(rec.perm, j) = p;
        }
    }
    // The deflated singular values go back into the last N - K slots of D.
    d[k..n].copy_from_slice(&dsigma[k..n]);
    // Determine DSIGMA(1), DSIGMA(2), Z(1), VF(1), VL(1), VF(M), and VL(M).
    at!(dsigma, 1) = 0.0;
    let hlftol = tol / 2.0;
    if at!(dsigma, 2).abs() <= hlftol {
        at!(dsigma, 2) = hlftol;
    }
    if m > n {
        at!(z, 1) = dlapy2(z1, at!(z, m));
        if at!(z, 1) <= tol {
            c = 1.0;
            s = 0.0;
            at!(z, 1) = tol;
        } else {
            c = z1 / at!(z, 1);
            s = -at!(z, m) / at!(z, 1);
        }
        drot(1, vf, m - 1, 1, 0, 1, c, s);
        drot(1, vl, m - 1, 1, 0, 1, c, s);
    } else if z1.abs() <= tol {
        at!(z, 1) = tol;
    } else {
        at!(z, 1) = z1;
    }
    // Restore Z, VF, and VL.
    z[1..k].copy_from_slice(&zw[1..k]);
    vf[1..n].copy_from_slice(&vfw[1..n]);
    vl[1..n].copy_from_slice(&vlw[1..n]);
    Ok(Lasd7 { k, givptr, c, s })
}

/// `DLASD8(ICOMPQ, K, D, Z, VF, VL, DIFL, DIFR, LDDIFR, DSIGMA, WORK, INFO)`: the `k` roots of the
/// secular equation of poles `dsigma` and weights `z` into `d`, the updated `z`, `DIFL` (`k`; `k =
/// 1` and `icompq = 1`: also `DIFL(2)`), `DIFR` (`lddifr × 2` with `icompq = 1`, else its first
/// column) and the new `vf`, `vl` (module documentation). Returns `DLASD4`'s `INFO` (0, or 1: no
/// convergence; the arrays then as LAPACK leaves them).
///
/// # Errors
/// `ICOMPQ` not 0 or 1 (1), `K < 1` (2), `LDDIFR < K` (9), as LAPACK.
pub fn dlasd8(
    icompq: usize,
    k: usize,
    d: &mut [f64],
    z: &mut [f64],
    vf: &mut [f64],
    vl: &mut [f64],
    difl: &mut [f64],
    difr: &mut [f64],
    lddifr: usize,
    dsigma: &[f64],
) -> Result<usize, IllegalValue> {
    if icompq > 1 {
        illegal("DLASD8", 1)?;
    } else if k < 1 {
        illegal("DLASD8", 2)?;
    } else if lddifr < k {
        illegal("DLASD8", 9)?;
    }
    macro_rules! at {
        ($a:expr, $i:expr) => {
            $a[$i - 1]
        };
    }
    // Quick return if possible.
    if k == 1 {
        at!(d, 1) = at!(z, 1).abs();
        at!(difl, 1) = at!(d, 1);
        if icompq == 1 {
            at!(difl, 2) = 1.0;
            difr[lddifr] = 1.0; // DIFR(1, 2)
        }
        return Ok(0);
    }
    // WORK(IWK1) = DELTA, WORK(IWK2) = DLASD4's WORK, WORK(IWK3) = the products for the new Z.
    let mut work = vec![0.0f64; 3 * k];
    let (iwk2, iwk3) = (k + 1, 2 * k + 1);
    let (iwk2i, iwk3i) = (iwk2 - 1, iwk3 - 1);
    // Normalize Z.
    let mut rho = dnrm2(k, z, 0, 1);
    let _ = dlascl_g(rho, 1.0, k, 1, z, 0, k);
    rho *= rho;
    // Initialize WORK(IWK3).
    work[iwk3 - 1..iwk3 - 1 + k].fill(1.0);
    // Compute the updated singular values, the arrays DIFL, DIFR, and the updated Z.
    for j in 1..=k {
        let (lo, hi) = work.split_at_mut(k);
        let (sigma, info) = dlasd4(k, j, dsigma, z, lo, rho, &mut hi[..k]);
        at!(d, j) = sigma;
        // If the root finder fails, report the convergence failure.
        if info != 0 {
            return Ok(info);
        }
        at!(work, iwk3i + j) = at!(work, iwk3i + j) * at!(work, j) * at!(work, iwk2i + j);
        at!(difl, j) = -at!(work, j);
        at!(difr, j) = -at!(work, j + 1);
        for i in 1..j {
            at!(work, iwk3i + i) = at!(work, iwk3i + i) * at!(work, i) * at!(work, iwk2i + i)
                / (at!(dsigma, i) - at!(dsigma, j))
                / (at!(dsigma, i) + at!(dsigma, j));
        }
        for i in j + 1..=k {
            at!(work, iwk3i + i) = at!(work, iwk3i + i) * at!(work, i) * at!(work, iwk2i + i)
                / (at!(dsigma, i) - at!(dsigma, j))
                / (at!(dsigma, i) + at!(dsigma, j));
        }
    }
    // Compute updated Z.
    for i in 1..=k {
        at!(z, i) = sign(at!(work, iwk3i + i).abs().sqrt(), at!(z, i));
    }
    // Update VF and VL.
    for j in 1..=k {
        let diflj = at!(difl, j);
        let dj = at!(d, j);
        let dsigj = -at!(dsigma, j);
        let (mut difrj, mut dsigjp) = (0.0f64, 0.0f64);
        if j < k {
            difrj = -at!(difr, j);
            dsigjp = -at!(dsigma, j + 1);
        }
        at!(work, j) = -at!(z, j) / diflj / (at!(dsigma, j) + dj);
        for i in 1..j {
            // DLAMC3(DSIGMA(I), DSIGJ): the sum, stored.
            at!(work, i) = at!(z, i) / ((at!(dsigma, i) + dsigj) - diflj) / (at!(dsigma, i) + dj);
        }
        for i in j + 1..=k {
            at!(work, i) = at!(z, i) / ((at!(dsigma, i) + dsigjp) + difrj) / (at!(dsigma, i) + dj);
        }
        let temp = dnrm2(k, &work, 0, 1);
        at!(work, iwk2i + j) = ddot(&work[..k], &vf[..k]) / temp;
        at!(work, iwk3i + j) = ddot(&work[..k], &vl[..k]) / temp;
        if icompq == 1 {
            difr[(j - 1) + lddifr] = temp; // DIFR(J, 2)
        }
    }
    vf[..k].copy_from_slice(&work[iwk2 - 1..iwk2 - 1 + k]);
    vl[..k].copy_from_slice(&work[iwk3 - 1..iwk3 - 1 + k]);
    Ok(0)
}

/// What [`dlasd6`] returns besides its arrays: `K`, `GIVPTR`, `C`, `S` (as [`dlasd7`] leaves them),
/// the scaled `ALPHA` and `BETA`, and `INFO` (0, or `DLASD4`'s 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lasd6 {
    /// `K`.
    pub k: usize,
    /// `GIVPTR`.
    pub givptr: usize,
    /// `C`.
    pub c: f64,
    /// `S`.
    pub s: f64,
    /// `ALPHA / ORGNRM`.
    pub alpha: f64,
    /// `BETA / ORGNRM`.
    pub beta: f64,
    /// LAPACK's `INFO`.
    pub info: usize,
}

/// The outputs of one merge for `DLALS0` (`ICOMPQ = 1`): `POLES` (`ldgnum × 2`), `DIFL` (`n`),
/// `DIFR` (`ldgnum × 2`; with `ICOMPQ = 0` only its first column, `n`), `Z` (`m`).
pub struct MergeFactors<'a> {
    /// `POLES`.
    pub poles: &'a mut [f64],
    /// `DIFL`.
    pub difl: &'a mut [f64],
    /// `DIFR`.
    pub difr: &'a mut [f64],
    /// `Z`.
    pub z: &'a mut [f64],
}

/// `DLASD6(ICOMPQ, NL, NR, SQRE, D, VF, VL, ALPHA, BETA, IDXQ, PERM, GIVPTR, GIVCOL, LDGCOL,
/// GIVNUM, LDGNUM, POLES, DIFL, DIFR, Z, K, C, S, WORK, IWORK, INFO)`: the merge of two adjacent
/// subproblems of `DLASDA`'s tree (module documentation). `d` (`n = nl + nr + 1`), `vf`, `vl`
/// (`m = n + sqre`), `idxq` (`n`, 1-based: on entry the halves' sorting permutations, on exit the
/// merged one), `rec` (written with `icompq = 1`), `out` (`POLES` only with `icompq = 1`); `c`, `s`
/// start as given.
///
/// # Errors
/// `ICOMPQ` not 0 or 1 (1), `NL < 1` (2), `NR < 1` (3), `SQRE` not 0 or 1 (4), `LDGCOL < N` (14),
/// `LDGNUM < N` (16), as LAPACK.
pub fn dlasd6(
    release: LapackRelease,
    icompq: usize,
    nl: usize,
    nr: usize,
    sqre: usize,
    d: &mut [f64],
    vf: &mut [f64],
    vl: &mut [f64],
    alpha: f64,
    beta: f64,
    idxq: &mut [usize],
    rec: &mut GivensRecord<'_>,
    out: &mut MergeFactors<'_>,
    givptr: usize,
    c: f64,
    s: f64,
) -> Result<Lasd6, IllegalValue> {
    let n = nl + nr + 1;
    if icompq > 1 {
        illegal("DLASD6", 1)?;
    } else if nl < 1 {
        illegal("DLASD6", 2)?;
    } else if nr < 1 {
        illegal("DLASD6", 3)?;
    } else if sqre > 1 {
        illegal("DLASD6", 4)?;
    } else if rec.ldgcol < n {
        illegal("DLASD6", 14)?;
    } else if rec.ldgnum < n {
        illegal("DLASD6", 16)?;
    }
    // Scale.
    let mut orgnrm = alpha.abs().max(beta.abs());
    d[nl] = 0.0;
    for i in 0..n {
        if d[i].abs() > orgnrm {
            orgnrm = d[i].abs();
        }
    }
    let _ = dlascl_g(orgnrm, 1.0, n, 1, d, 0, n);
    let alpha = alpha / orgnrm;
    let beta = beta / orgnrm;
    // Sort and deflate singular values.
    let mut dsigma = vec![0.0f64; n];
    let lasd7 = dlasd7(
        release,
        icompq,
        nl,
        nr,
        sqre,
        d,
        out.z,
        vf,
        vl,
        alpha,
        beta,
        &mut dsigma,
        idxq,
        rec,
        givptr,
        c,
        s,
    )?;
    let k = lasd7.k;
    // Solve Secular Equation, compute DIFL, DIFR, and update VF, VL.
    let info = dlasd8(
        icompq, k, d, out.z, vf, vl, out.difl, out.difr, rec.ldgnum, &dsigma,
    )?;
    let done = |info| Lasd6 {
        k,
        givptr: lasd7.givptr,
        c: lasd7.c,
        s: lasd7.s,
        alpha,
        beta,
        info,
    };
    // Report the possible convergence failure.
    if info != 0 {
        return Ok(done(info));
    }
    // Save the poles if ICOMPQ = 1.
    if icompq == 1 {
        let ld = rec.ldgnum;
        out.poles[..k].copy_from_slice(&d[..k]);
        out.poles[ld..ld + k].copy_from_slice(&dsigma[..k]);
    }
    // Unscale.
    let _ = dlascl_g(1.0, orgnrm, n, 1, d, 0, n);
    // Prepare the IDXQ sorting permutation.
    dlamrg(k, n - k, d, 1, -1, idxq);
    Ok(done(0))
}

/// The compact form of a bidiagonal's SVD that [`dlasda`] writes (`ICOMPQ = 1`) and [`dlalsa`]
/// applies: every real array column-major with leading dimension `LDU`, every index array with
/// `LDGCOL` (indices 1-based), as LAPACK lays them out (`NLVL` the depth of `DLASDT`'s tree).
/// With `ICOMPQ = 0` only `k`, `difl`, `difr`, `z`, `givptr`, `c` and `s`'s first entries (and
/// `difl`, `difr`, `z`'s first `n`) are referenced: the last merge's values.
pub struct SvdTree<'a> {
    /// `U` (`LDU × SMLSIZ`): the left singular vectors of the leaves.
    pub u: &'a mut [f64],
    /// `VT` (`LDU × (SMLSIZ + 1)`): the right singular vectors of the leaves, transposed.
    pub vt: &'a mut [f64],
    /// `K` (`N`): the size of each node's secular equation.
    pub k: &'a mut [usize],
    /// `DIFL` (`LDU × NLVL`).
    pub difl: &'a mut [f64],
    /// `DIFR` (`LDU × 2·NLVL`).
    pub difr: &'a mut [f64],
    /// `Z` (`LDU × NLVL`).
    pub z: &'a mut [f64],
    /// `POLES` (`LDU × 2·NLVL`).
    pub poles: &'a mut [f64],
    /// `GIVPTR` (`N`): the number of Givens rotations of each node.
    pub givptr: &'a mut [usize],
    /// `GIVCOL` (`LDGCOL × 2·NLVL`).
    pub givcol: &'a mut [usize],
    /// `PERM` (`LDGCOL × NLVL`).
    pub perm: &'a mut [usize],
    /// `GIVNUM` (`LDU × 2·NLVL`).
    pub givnum: &'a mut [f64],
    /// `C` (`N`): the rotation of each non-square node's extra row (other nodes: as given, or a
    /// deflation's rotation, as `DLASD7` leaves it).
    pub c: &'a mut [f64],
    /// `S` (`N`).
    pub s: &'a mut [f64],
}

/// Why [`dlasda`] did not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LasdaError {
    /// A parameter LAPACK's checks refuse (`XERBLA`'s message).
    #[error(transparent)]
    Illegal(#[from] IllegalValue),
    /// `ICOMPQ = 0` with `2 <= N <= SMLSIZ`: LAPACK's `DLASDQ` then finds the values alone through
    /// `DBDSQR`'s `DLASQ1` (the dqds algorithm), which is not ported (no caller of the confounds
    /// asks for it: `DLALSD` always passes `ICOMPQ = 1`, numpy's SVDs always compute vectors).
    #[error(
        "DLASDA: the singular values alone of a bidiagonal of {n} <= SMLSIZ rows (DBDSQR's \
         DLASQ1, the dqds algorithm) are not ported"
    )]
    ValuesOnlyNotPorted {
        /// The bidiagonal's order.
        n: usize,
    },
}

/// `DLASDA(ICOMPQ, SMLSIZ, N, SQRE, D, E, U, LDU, VT, K, DIFL, DIFR, Z, POLES, GIVPTR, GIVCOL,
/// LDGCOL, PERM, GIVNUM, C, S, WORK, IWORK, INFO)`: the singular values of the `n × (n + sqre)`
/// upper bidiagonal (`d`: `n` values, `e`: `n - 1 + sqre`) by divide and conquer, and with `icompq
/// = 1` the compact form of its vectors in `t` (module documentation of [`SvdTree`]): the tree by
/// `DLASDT`, the leaves by `DLASDQ` (`ICOMPQ = 1`: their `U` and `VT` from the identity in place;
/// `ICOMPQ = 0`: `VT` in a work array of leading dimension `SMLSIZ + 1`), the leaves' first and last
/// right-vector components gathered into `VF` and `VL`, `IDXQ` the identity per leaf, then the
/// merges level by level from the bottom by [`dlasd6`] with `ALPHA = D(IC)`, `BETA = E(IC)`
/// (`ICOMPQ = 1`: each node's arrays at its rows and level's columns, its `K`, `GIVPTR`, `C`, `S`
/// numbered from the last node down to the root's 1; `ICOMPQ = 0`: all at the arrays' starts).
/// `n <= smlsiz`: one `DLASDQ` on `d`, `e` and (`ICOMPQ = 1`) `t.vt`, `t.u` as given. `d` returns
/// the singular values as the merges leave them (each node's sorted by its `IDXQ`, not the whole).
/// Returns LAPACK's `INFO` (0; a leaf's `DBDSQR` or a merge's `DLASD4` that did not converge: its
/// positive `INFO`, the arrays as LAPACK leaves them).
///
/// # Errors
/// `ICOMPQ` not 0 or 1 (1), `SMLSIZ < 3` (2), `SQRE` not 0 or 1 (4), `LDU < N + SQRE` (8),
/// `LDGCOL < N` (17), as LAPACK ([`LasdaError::Illegal`]); `ICOMPQ = 0` with `2 <= N <= SMLSIZ`
/// ([`LasdaError::ValuesOnlyNotPorted`]).
pub fn dlasda(
    release: LapackRelease,
    icompq: usize,
    smlsiz: usize,
    n: usize,
    sqre: usize,
    d: &mut [f64],
    e: &mut [f64],
    ldu: usize,
    t: &mut SvdTree<'_>,
    ldgcol: usize,
) -> Result<usize, LasdaError> {
    if icompq > 1 {
        illegal("DLASDA", 1)?;
    } else if smlsiz < 3 {
        illegal("DLASDA", 2)?;
    } else if sqre > 1 {
        illegal("DLASDA", 4)?;
    } else if ldu < n + sqre {
        illegal("DLASDA", 8)?;
    } else if ldgcol < n {
        illegal("DLASDA", 17)?;
    }
    let m = n + sqre;
    let ix = |i: usize, j: usize| (i - 1) + (j - 1) * ldu;
    // If the input matrix is too small, call DLASDQ to find the singular values and vectors.
    if n <= smlsiz {
        if icompq == 0 && n >= 2 {
            return Err(LasdaError::ValuesOnlyNotPorted { n });
        }
        let info = if icompq == 0 {
            dlasdq(
                release,
                Uplo::Upper,
                sqre,
                n,
                0,
                0,
                0,
                d,
                e,
                &mut [],
                0,
                ldu,
                &mut [],
                0,
                ldu,
                &mut [],
                0,
                ldu,
            )
        } else {
            dlasdq(
                release,
                Uplo::Upper,
                sqre,
                n,
                m,
                n,
                0,
                d,
                e,
                t.vt,
                0,
                ldu,
                t.u,
                0,
                ldu,
                &mut [],
                0,
                ldu,
            )
        };
        return Ok(info?);
    }
    // Book-keeping and set up the computation tree. WORK(VF), WORK(VL), IWORK(IDXQ); the leaves'
    // VT with ICOMPQ = 0 at WORK(NWORK1) (leading dimension SMLSZP); the other work arrays are
    // the callees' own.
    let smlszp = smlsiz + 1;
    let mut vf = vec![0.0f64; m];
    let mut vl = vec![0.0f64; m];
    let mut idxq = vec![0usize; n];
    let tree = dlasdt(n, smlsiz);
    let nlvl = tree.lvl as usize;
    let nd = tree.nd;
    let node = |i: usize| {
        (
            tree.inode[i - 1] as usize,
            tree.ndiml[i - 1] as usize,
            tree.ndimr[i - 1] as usize,
        )
    };
    // For the nodes on the bottom level of the tree, solve their subproblems by DLASDQ.
    let ndb1 = nd.div_ceil(2);
    for i in ndb1..=nd {
        let (ic, nl, nr) = node(i);
        let nlp1 = nl + 1;
        let nlf = ic - nl;
        let nrf = ic + 1;
        // 0-based positions of IWORK(IDXQI + 1), WORK(VFI), WORK(VLI).
        let mut idxqi = nlf - 1;
        let mut vfi = nlf - 1;
        let mut vli = nlf - 1;
        let sqrei = 1;
        let info = if icompq == 0 {
            let mut w = vec![0.0f64; smlszp * smlszp];
            identity(nlp1, nlp1, &mut w, 0, smlszp);
            let info = dlasdq(
                release,
                Uplo::Upper,
                sqrei,
                nl,
                nlp1,
                0,
                0,
                &mut d[nlf - 1..],
                &mut e[nlf - 1..],
                &mut w,
                0,
                smlszp,
                &mut [],
                0,
                nl,
                &mut [],
                0,
                nl,
            )?;
            let itemp = nl * smlszp;
            vf[vfi..vfi + nlp1].copy_from_slice(&w[..nlp1]);
            vl[vli..vli + nlp1].copy_from_slice(&w[itemp..itemp + nlp1]);
            info
        } else {
            identity(nl, nl, t.u, ix(nlf, 1), ldu);
            identity(nlp1, nlp1, t.vt, ix(nlf, 1), ldu);
            let info = dlasdq(
                release,
                Uplo::Upper,
                sqrei,
                nl,
                nlp1,
                nl,
                0,
                &mut d[nlf - 1..],
                &mut e[nlf - 1..],
                t.vt,
                ix(nlf, 1),
                ldu,
                t.u,
                ix(nlf, 1),
                ldu,
                &mut [],
                0,
                ldu,
            )?;
            vf[vfi..vfi + nlp1].copy_from_slice(&t.vt[ix(nlf, 1)..ix(nlf, 1) + nlp1]);
            vl[vli..vli + nlp1].copy_from_slice(&t.vt[ix(nlf, nlp1)..ix(nlf, nlp1) + nlp1]);
            info
        };
        if info != 0 {
            return Ok(info);
        }
        for j in 1..=nl {
            idxq[idxqi + j - 1] = j;
        }
        let sqrei = if i == nd && sqre == 0 { 0 } else { 1 };
        idxqi += nlp1;
        vfi += nlp1;
        vli += nlp1;
        let nrp1 = nr + sqrei;
        let info = if icompq == 0 {
            let mut w = vec![0.0f64; smlszp * smlszp];
            identity(nrp1, nrp1, &mut w, 0, smlszp);
            let info = dlasdq(
                release,
                Uplo::Upper,
                sqrei,
                nr,
                nrp1,
                0,
                0,
                &mut d[nrf - 1..],
                &mut e[nrf - 1..],
                &mut w,
                0,
                smlszp,
                &mut [],
                0,
                nr,
                &mut [],
                0,
                nr,
            )?;
            let itemp = (nrp1 - 1) * smlszp;
            vf[vfi..vfi + nrp1].copy_from_slice(&w[..nrp1]);
            vl[vli..vli + nrp1].copy_from_slice(&w[itemp..itemp + nrp1]);
            info
        } else {
            identity(nr, nr, t.u, ix(nrf, 1), ldu);
            identity(nrp1, nrp1, t.vt, ix(nrf, 1), ldu);
            let info = dlasdq(
                release,
                Uplo::Upper,
                sqrei,
                nr,
                nrp1,
                nr,
                0,
                &mut d[nrf - 1..],
                &mut e[nrf - 1..],
                t.vt,
                ix(nrf, 1),
                ldu,
                t.u,
                ix(nrf, 1),
                ldu,
                &mut [],
                0,
                ldu,
            )?;
            vf[vfi..vfi + nrp1].copy_from_slice(&t.vt[ix(nrf, 1)..ix(nrf, 1) + nrp1]);
            vl[vli..vli + nrp1].copy_from_slice(&t.vt[ix(nrf, nrp1)..ix(nrf, nrp1) + nrp1]);
            info
        };
        if info != 0 {
            return Ok(info);
        }
        for j in 1..=nr {
            idxq[idxqi + j - 1] = j;
        }
    }
    // Now conquer each subproblem bottom-up.
    let mut j = 1usize << nlvl;
    for lvl in (1..=nlvl).rev() {
        let lvl2 = lvl * 2 - 1;
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
            let sqrei = if i == ll { sqre } else { 1 };
            let at = nlf - 1;
            let alpha = d[ic - 1];
            let beta = e[ic - 1];
            // The node's slot: J (ICOMPQ = 1) or the first (ICOMPQ = 0); the columns of its level.
            let (slot, c1, c2) = if icompq == 0 {
                (1, 0, 0)
            } else {
                j -= 1;
                (j, at + (lvl - 1) * ldu, at + (lvl2 - 1) * ldu)
            };
            let g1 = if icompq == 0 {
                0
            } else {
                at + (lvl - 1) * ldgcol
            };
            let g2 = if icompq == 0 {
                0
            } else {
                at + (lvl2 - 1) * ldgcol
            };
            let mut rec = GivensRecord {
                perm: &mut t.perm[g1..],
                givcol: &mut t.givcol[g2..],
                ldgcol,
                givnum: &mut t.givnum[c2..],
                ldgnum: ldu,
            };
            let mut out = MergeFactors {
                poles: &mut t.poles[c2..],
                difl: &mut t.difl[c1..],
                difr: &mut t.difr[c2..],
                z: &mut t.z[c1..],
            };
            let merged = dlasd6(
                release,
                icompq,
                nl,
                nr,
                sqrei,
                &mut d[nlf - 1..],
                &mut vf[at..],
                &mut vl[at..],
                alpha,
                beta,
                &mut idxq[at..],
                &mut rec,
                &mut out,
                t.givptr[slot - 1],
                t.c[slot - 1],
                t.s[slot - 1],
            )?;
            t.k[slot - 1] = merged.k;
            t.givptr[slot - 1] = merged.givptr;
            t.c[slot - 1] = merged.c;
            t.s[slot - 1] = merged.s;
            if merged.info != 0 {
                return Ok(merged.info);
            }
        }
    }
    Ok(0)
}

/// `DLALS0(ICOMPQ, NL, NR, SQRE, NRHS, B, LDB, BX, LDBX, PERM, GIVPTR, GIVCOL, LDGCOL, GIVNUM,
/// LDGNUM, POLES, DIFL, DIFR, Z, K, C, S, WORK, INFO)`: one node's factor of the compact SVD
/// applied to the `nrhs` columns of `b` (rows `n = nl + nr + 1`, and `m = n + sqre` for `icompq =
/// 1`; leading dimension `ldb`), `bx` (leading dimension `ldbx`) a work array: `ICOMPQ = 0` the
/// left factor (the Givens rotations, `PERM` into `BX`, then each of the `K` rows of `B` as
/// `BXᵀ·w / ‖w‖` by `DGEMV('T')`, `DNRM2` and `DLASCL`, `w` from `POLES`, `DIFL`, `DIFR`, `Z`
/// with `w(1) = -1`; the deflated rows copied), `ICOMPQ = 1` the right factor (the rows of `BX`
/// as `Bᵀ·w` per root, the extra row's rotation by `C`, `S` for `SQRE = 1`, back into `B` through
/// `PERM`, the Givens rotations undone in reverse). `perm` (`n`, 1-based), `givcol` (`ldgcol ×
/// 2`), `givnum`, `poles`, `difr` (`ldgnum × 2`), `difl` and `z` (`k`) at the node's positions.
/// Returns LAPACK's `INFO`: 0, or the last `DLASCL`'s (`-4` for a NaN norm) with `ICOMPQ = 0`.
///
/// # Errors
/// `ICOMPQ` not 0 or 1 (1), `NL < 1` (2), `NR < 1` (3), `SQRE` not 0 or 1 (4), `NRHS < 1` (5),
/// `LDB < N` (7), `LDBX < N` (9), `LDGCOL < N` (13), `LDGNUM < N` (15), `K < 1` (20), as LAPACK.
pub fn dlals0(
    icompq: usize,
    nl: usize,
    nr: usize,
    sqre: usize,
    nrhs: usize,
    b: &mut [f64],
    ldb: usize,
    bx: &mut [f64],
    ldbx: usize,
    perm: &[usize],
    givptr: usize,
    givcol: &[usize],
    ldgcol: usize,
    givnum: &[f64],
    ldgnum: usize,
    poles: &[f64],
    difl: &[f64],
    difr: &[f64],
    z: &[f64],
    k: usize,
    c: f64,
    s: f64,
) -> Result<i64, IllegalValue> {
    let n = nl + nr + 1;
    if icompq > 1 {
        illegal("DLALS0", 1)?;
    } else if nl < 1 {
        illegal("DLALS0", 2)?;
    } else if nr < 1 {
        illegal("DLALS0", 3)?;
    } else if sqre > 1 {
        illegal("DLALS0", 4)?;
    } else if nrhs < 1 {
        illegal("DLALS0", 5)?;
    } else if ldb < n {
        illegal("DLALS0", 7)?;
    } else if ldbx < n {
        illegal("DLALS0", 9)?;
    } else if ldgcol < n {
        illegal("DLALS0", 13)?;
    } else if ldgnum < n {
        illegal("DLALS0", 15)?;
    } else if k < 1 {
        illegal("DLALS0", 20)?;
    }
    let m = n + sqre;
    let nlp1 = nl + 1;
    let (ldbi, ldbxi) = (ldb as isize, ldbx as isize);
    // GIVCOL(I, 2), GIVCOL(I, 1), GIVNUM(I, 2), GIVNUM(I, 1), POLES(I, 1), POLES(I, 2), DIFR(I,
    // 1), DIFR(I, 2), 1-based I; the rows of B and BX as offsets.
    let gc = |i: usize, col: usize| givcol[(i - 1) + (col - 1) * ldgcol];
    let gn = |i: usize, col: usize| givnum[(i - 1) + (col - 1) * ldgnum];
    let pl = |i: usize, col: usize| poles[(i - 1) + (col - 1) * ldgnum];
    let dr = |i: usize, col: usize| difr[(i - 1) + (col - 1) * ldgnum];
    let mut work = vec![0.0f64; k];
    let mut info = 0i64;
    if icompq == 0 {
        // Apply back orthogonal transformations from the left. Step (1L): apply back the Givens
        // rotations performed.
        for i in 1..=givptr {
            drot(
                nrhs,
                b,
                gc(i, 2) - 1,
                ldb,
                gc(i, 1) - 1,
                ldb,
                gn(i, 2),
                gn(i, 1),
            );
        }
        // Step (2L): permute rows of B.
        dcopy(nrhs, b, nlp1 - 1, ldbi, bx, 0, ldbxi);
        for i in 2..=n {
            dcopy(nrhs, b, perm[i - 1] - 1, ldbi, bx, i - 1, ldbxi);
        }
        // Step (3L): apply the inverse of the left singular vector matrix to BX.
        if k == 1 {
            dcopy(nrhs, bx, 0, ldbxi, b, 0, ldbi);
            if z[0] < 0.0 {
                dscal(nrhs, -1.0, b, 0, ldbi);
            }
        } else {
            let (mut difrj, mut dsigjp) = (0.0f64, 0.0f64);
            for j in 1..=k {
                let diflj = difl[j - 1];
                let dj = pl(j, 1);
                let dsigj = -pl(j, 2);
                if j < k {
                    difrj = -dr(j, 1);
                    dsigjp = -pl(j + 1, 2);
                }
                work[j - 1] = if z[j - 1] == 0.0 || pl(j, 2) == 0.0 {
                    0.0
                } else {
                    -(pl(j, 2) * z[j - 1] / diflj / (pl(j, 2) + dj))
                };
                for i in 1..j {
                    work[i - 1] = if z[i - 1] == 0.0 || pl(i, 2) == 0.0 {
                        0.0
                    } else {
                        // DLAMC3(POLES(I, 2), DSIGJ): the sum, stored.
                        pl(i, 2) * z[i - 1] / ((pl(i, 2) + dsigj) - diflj) / (pl(i, 2) + dj)
                    };
                }
                for i in j + 1..=k {
                    work[i - 1] = if z[i - 1] == 0.0 || pl(i, 2) == 0.0 {
                        0.0
                    } else {
                        pl(i, 2) * z[i - 1] / ((pl(i, 2) + dsigjp) + difrj) / (pl(i, 2) + dj)
                    };
                }
                work[0] = -1.0;
                let temp = dnrm2(k, &work, 0, 1);
                dgemv(
                    Trans::T,
                    k,
                    nrhs,
                    1.0,
                    bx,
                    0,
                    ldbx,
                    &work,
                    0,
                    1,
                    0.0,
                    b,
                    j - 1,
                    ldb,
                );
                info = match dlascl_g(temp, 1.0, 1, nrhs, b, j - 1, ldb) {
                    Ok(()) => 0,
                    Err(e) => -i64::from(e.param),
                };
            }
        }
        // Move the deflated rows of BX to B also.
        if k < m.max(n) {
            dlacpy(n - k, nrhs, bx, k, ldbx, b, k, ldb);
        }
    } else {
        // Apply back the right orthogonal transformations. Step (1R): apply back the new right
        // singular vector matrix to B.
        if k == 1 {
            dcopy(nrhs, b, 0, ldbi, bx, 0, ldbxi);
        } else {
            for j in 1..=k {
                let dsigj = pl(j, 2);
                work[j - 1] = if z[j - 1] == 0.0 {
                    0.0
                } else {
                    -(z[j - 1] / difl[j - 1] / (dsigj + pl(j, 1)) / dr(j, 2))
                };
                for i in 1..j {
                    work[i - 1] = if z[j - 1] == 0.0 {
                        0.0
                    } else {
                        // DLAMC3(DSIGJ, -POLES(I + 1, 2)): the sum, stored.
                        z[j - 1]
                            / ((dsigj + -pl(i + 1, 2)) - dr(i, 1))
                            / (dsigj + pl(i, 1))
                            / dr(i, 2)
                    };
                }
                for i in j + 1..=k {
                    work[i - 1] = if z[j - 1] == 0.0 {
                        0.0
                    } else {
                        z[j - 1]
                            / ((dsigj + -pl(i, 2)) - difl[i - 1])
                            / (dsigj + pl(i, 1))
                            / dr(i, 2)
                    };
                }
                dgemv(
                    Trans::T,
                    k,
                    nrhs,
                    1.0,
                    b,
                    0,
                    ldb,
                    &work,
                    0,
                    1,
                    0.0,
                    bx,
                    j - 1,
                    ldbx,
                );
            }
        }
        // Step (2R): if SQRE = 1, apply back the rotation that is related to the right null
        // space of the subproblem.
        if sqre == 1 {
            dcopy(nrhs, b, m - 1, ldbi, bx, m - 1, ldbxi);
            drot(nrhs, bx, 0, ldbx, m - 1, ldbx, c, s);
        }
        if k < m.max(n) {
            dlacpy(n - k, nrhs, b, k, ldb, bx, k, ldbx);
        }
        // Step (3R): permute rows of B.
        dcopy(nrhs, bx, 0, ldbxi, b, nlp1 - 1, ldbi);
        if sqre == 1 {
            dcopy(nrhs, bx, m - 1, ldbxi, b, m - 1, ldbi);
        }
        for i in 2..=n {
            dcopy(nrhs, bx, i - 1, ldbxi, b, perm[i - 1] - 1, ldbi);
        }
        // Step (4R): apply back the Givens rotations performed.
        for i in (1..=givptr).rev() {
            drot(
                nrhs,
                b,
                gc(i, 2) - 1,
                ldb,
                gc(i, 1) - 1,
                ldb,
                gn(i, 2),
                -gn(i, 1),
            );
        }
    }
    Ok(info)
}

/// `DLALSA(ICOMPQ, SMLSIZ, N, NRHS, B, LDB, BX, LDBX, U, LDU, VT, K, DIFL, DIFR, Z, POLES,
/// GIVPTR, GIVCOL, LDGCOL, PERM, GIVNUM, C, S, WORK, IWORK, INFO)`: the compact SVD of
/// [`dlasda`] (`t`, read only) applied to the `nrhs` columns of `b` (`n` rows, leading dimension
/// `ldb`), the result in `bx` (leading dimension `ldbx`; `b` is a work array then): `ICOMPQ = 0`
/// the transposed left singular vectors (each leaf's `Uᵀ·B` by `DGEMM('T', 'N')`, the nodes'
/// centre rows copied, then [`dlals0`] bottom-up, each square), `ICOMPQ = 1` the right singular
/// vectors (the nodes top-down by [`dlals0`], the last of each level square and the others with
/// their extra row, then each leaf's `VTᵀ·B`). The depth of every product is at most `SMLSIZ +
/// 1`. Returns LAPACK's `INFO` (the last [`dlals0`]'s).
///
/// # Errors
/// `ICOMPQ` not 0 or 1 (1), `SMLSIZ < 3` (2), `N < SMLSIZ` (3), `NRHS < 1` (4), `LDB < N` (6),
/// `LDBX < N` (8), `LDU < N` (10), `LDGCOL < N` (19), as LAPACK.
pub fn dlalsa(
    gemm_q: usize,
    icompq: usize,
    smlsiz: usize,
    n: usize,
    nrhs: usize,
    b: &mut [f64],
    ldb: usize,
    bx: &mut [f64],
    ldbx: usize,
    ldu: usize,
    t: &SvdTree<'_>,
    ldgcol: usize,
) -> Result<i64, IllegalValue> {
    if icompq > 1 {
        illegal("DLALSA", 1)?;
    } else if smlsiz < 3 {
        illegal("DLALSA", 2)?;
    } else if n < smlsiz {
        illegal("DLALSA", 3)?;
    } else if nrhs < 1 {
        illegal("DLALSA", 4)?;
    } else if ldb < n {
        illegal("DLALSA", 6)?;
    } else if ldbx < n {
        illegal("DLALSA", 8)?;
    } else if ldu < n {
        illegal("DLALSA", 10)?;
    } else if ldgcol < n {
        illegal("DLALSA", 19)?;
    }
    // Book-keeping and setting up the computation tree.
    let tree = dlasdt(n, smlsiz);
    let nlvl = tree.lvl as usize;
    let nd = tree.nd;
    let node = |i: usize| {
        (
            tree.inode[i - 1] as usize,
            tree.ndiml[i - 1] as usize,
            tree.ndimr[i - 1] as usize,
        )
    };
    let (ldbi, ldbxi) = (ldb as isize, ldbx as isize);
    // One node's DLALS0 between `top` (its B) and `low` (its BX), from the node's row `nlf` at
    // level `lvl`, its slot `j`.
    let node_solve = |icompq: usize,
                      i: usize,
                      lvl: usize,
                      j: usize,
                      sqre: usize,
                      top: &mut [f64],
                      ldtop: usize,
                      low: &mut [f64],
                      ldlow: usize|
     -> Result<i64, IllegalValue> {
        let (ic, nl, nr) = node(i);
        let at = ic - nl - 1;
        let lvl2 = 2 * lvl - 1;
        let (c1, c2) = (at + (lvl - 1) * ldu, at + (lvl2 - 1) * ldu);
        let (g1, g2) = (at + (lvl - 1) * ldgcol, at + (lvl2 - 1) * ldgcol);
        dlals0(
            icompq,
            nl,
            nr,
            sqre,
            nrhs,
            &mut top[at..],
            ldtop,
            &mut low[at..],
            ldlow,
            &t.perm[g1..],
            t.givptr[j - 1],
            &t.givcol[g2..],
            ldgcol,
            &t.givnum[c2..],
            ldu,
            &t.poles[c2..],
            &t.difl[c1..],
            &t.difr[c2..],
            &t.z[c1..],
            t.k[j - 1],
            t.c[j - 1],
            t.s[j - 1],
        )
    };
    let leaf_product = |rows: usize,
                        from: &[f64],
                        fld: usize,
                        to: &mut [f64],
                        tld: usize,
                        v: &[f64],
                        row: usize| {
        let a = Operand {
            data: v,
            off: row - 1,
            ld: ldu,
            trans: Trans::T,
        };
        let bo = Operand {
            data: from,
            off: row - 1,
            ld: fld,
            trans: Trans::N,
        };
        dgemm_with(rows, nrhs, rows, 1.0, a, bo, 0.0, to, row - 1, tld, gemm_q);
    };
    let ndb1 = nd.div_ceil(2);
    let mut info = 0i64;
    if icompq == 0 {
        // The nodes on the bottom level of the tree: apply Uᵀ to B.
        for i in ndb1..=nd {
            let (ic, nl, nr) = node(i);
            leaf_product(nl, b, ldb, bx, ldbx, t.u, ic - nl);
            leaf_product(nr, b, ldb, bx, ldbx, t.u, ic + 1);
        }
        // The rows of B corresponding to the centres of the subproblems.
        for i in 1..=nd {
            let ic = tree.inode[i - 1] as usize;
            dcopy(nrhs, b, ic - 1, ldbi, bx, ic - 1, ldbxi);
        }
        // Finally go through the left singular vector matrices of all the other subproblems
        // bottom-up on the tree.
        let mut j = 1usize << nlvl;
        for lvl in (1..=nlvl).rev() {
            let (lf, ll) = if lvl == 1 {
                (1usize, 1usize)
            } else {
                let lf = 1usize << (lvl - 1);
                (lf, 2 * lf - 1)
            };
            for i in lf..=ll {
                j -= 1;
                info = node_solve(0, i, lvl, j, 0, bx, ldbx, b, ldb)?;
            }
        }
    } else {
        // ICOMPQ = 1: apply back the right singular vector factors, top-down.
        let mut j = 0usize;
        for lvl in 1..=nlvl {
            let (lf, ll) = if lvl == 1 {
                (1usize, 1usize)
            } else {
                let lf = 1usize << (lvl - 1);
                (lf, 2 * lf - 1)
            };
            for i in (lf..=ll).rev() {
                let sqre = if i == ll { 0 } else { 1 };
                j += 1;
                info = node_solve(1, i, lvl, j, sqre, b, ldb, bx, ldbx)?;
            }
        }
        // The nodes on the bottom level of the tree: apply VTᵀ to B.
        for i in ndb1..=nd {
            let (ic, nl, nr) = node(i);
            let nlp1 = nl + 1;
            let nrp1 = if i == nd { nr } else { nr + 1 };
            leaf_product(nlp1, b, ldb, bx, ldbx, t.vt, ic - nl);
            leaf_product(nrp1, b, ldb, bx, ldbx, t.vt, ic + 1);
        }
    }
    Ok(info)
}
