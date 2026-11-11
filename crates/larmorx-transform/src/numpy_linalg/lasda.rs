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
//! Upstream files (BSD-3-Clause; the notices are in the parent module and NOTICE): OpenBLAS
//! 0.3.30 and v0.3.34-106-g446c436e `lapack-netlib/SRC/` (`dlasd6.f` and `dlasd8.f`, identical in
//! both; `dlasd7.f`, which differs in its deflation test and the record of the deflated positions;
//! `dlasd4.f`, `dlamrg.f`, `dlapy2.f`, `dlascl.f`, `dlaset.f`, `dlamch.f`), `interface/rot.c`,
//! `dot.c`, `nrm2.c`, `kernel/x86_64/drot.c`, `ddot.c`, `nrm2.S`. The logic is re-expressed in Rust;
//! no code was copied.

// The routines keep LAPACK's names, arguments and 1-based loops, so the port can be checked line
// by line against the Fortran.
#![allow(
    clippy::too_many_arguments,
    clippy::many_single_char_names,
    clippy::needless_range_loop
)]

use super::bdsdc::{dlamrg, dlascl_g, dlasd4};
use super::blas::ddot;
use super::blocked::IllegalValue;
use super::{EPS, LapackRelease, dlapy2, drot, sign};
use crate::openblas::level1::dnrm2;

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
