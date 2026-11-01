// SPDX-License-Identifier: LicenseRef-FSL AND BSD-3-Clause
// Translated from the reference LAPACK bundled with OpenBLAS 0.3.33 (`lapack-netlib/SRC`,
// modified BSD licence, Copyright (c) 1992-2023 The University of Tennessee and The University
// of Tennessee Research Foundation, 2000-2023 The University of California Berkeley, 2006-2023
// The University of Colorado Denver; the notice is in ../lapack.rs and ../../NOTICE). Part of
// larmorx-nc (FSL Licence terms; see LICENSE).
//! The reference LAPACK routines behind Armadillo's `eig_sym` (`dsyevd`) and `svd_econ` /
//! `pinv` (`dgesdd`), as FSL 6.0.7.23's `liblapack.so.3` (OpenBLAS 0.3.33, conda-forge) runs
//! them: the Fortran of OpenBLAS's `lapack-netlib` (updated towards LAPACK 3.13.0: `dlarf1f`,
//! `dlarf1l`, the Fortran 90 `dlartg`), compiled by gfortran for the x86-64 baseline (no fused
//! multiply-adds, every operation rounded in the order Fortran evaluates it: left to right,
//! parentheses kept), on OpenBLAS's BLAS ([`super::blas`]).
//!
//! Only the code paths these drivers reach for FSL's sizes are translated: the unblocked
//! routines (every block size `ILAENV` gives, 32, exceeds the matrices FLIRT, `midtrans` and
//! `pointflirt` decompose: at most 12 × 12), `dsteqr` for the tridiagonal eigenproblem
//! (`dstedc` hands it every matrix of up to 25 rows), `dbdsqr` through `dlasdq` for the
//! bidiagonal SVD (`dbdsdc` likewise), and `dgesdd`'s paths 3 (`m ≥ 11n/6`: QR first) and 5 for
//! `jobz = 'S'` with `m ≥ n` (Armadillo transposes a wide matrix first). Indices follow the
//! Fortran (1-based names, 0-based storage), so that each line can be checked against the
//! source. The logic is re-expressed in Rust; no code was copied.

use super::blas::{self, MatRef};

/// `DLAMCH('E')`: `EPSILON(0d0) * 0.5`.
pub const EPS: f64 = f64::EPSILON * 0.5;
/// `DLAMCH('P')`: `EPS * radix`.
pub const PREC: f64 = f64::EPSILON;
/// `DLAMCH('S')`: `TINY(0d0)` (its reciprocal does not overflow).
pub const SAFMIN: f64 = f64::MIN_POSITIVE;
/// `DLAMCH('O')`: `HUGE(0d0)`.
pub const HUGEVAL: f64 = f64::MAX;

/// Fortran's `SIGN(A, B)`: `|A|` with `B`'s sign bit (gfortran's `copysign`).
#[inline(always)]
fn sign(a: f64, b: f64) -> f64 {
    a.abs().copysign(b)
}

/// gfortran's `MAX(a, b)` / `MIN(a, b)` on finite values.
#[inline(always)]
fn fmax(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

#[inline(always)]
fn fmin(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

/// `DLAPY2`: `sqrt(x² + y²)` without needless overflow.
pub fn dlapy2(x: f64, y: f64) -> f64 {
    let mut r = 0.0;
    if x.is_nan() {
        r = x;
    }
    if y.is_nan() {
        r = y;
    }
    if !(x.is_nan() || y.is_nan()) {
        let xabs = x.abs();
        let yabs = y.abs();
        let w = fmax(xabs, yabs);
        let z = fmin(xabs, yabs);
        if z == 0.0 || w > HUGEVAL {
            r = w;
        } else {
            let q = z / w;
            r = w * (1.0 + q * q).sqrt();
        }
    }
    r
}

/// `DLARTG` (the Fortran 90 version): the plane rotation `(c, s, r)` with
/// `[c s; -s c]·[f; g] = [r; 0]`.
pub fn dlartg(f: f64, g: f64) -> (f64, f64, f64) {
    let safmin = SAFMIN;
    let safmax = 1.0 / SAFMIN;
    let rtmin = safmin.sqrt();
    let rtmax = (safmax / 2.0).sqrt();
    let f1 = f.abs();
    let g1 = g.abs();
    if g == 0.0 {
        (1.0, 0.0, f)
    } else if f == 0.0 {
        (0.0, sign(1.0, g), g1)
    } else if f1 > rtmin && f1 < rtmax && g1 > rtmin && g1 < rtmax {
        let d = (f * f + g * g).sqrt();
        let c = f1 / d;
        let r = sign(d, f);
        let s = g / r;
        (c, s, r)
    } else {
        let u = fmin(safmax, fmax(fmax(safmin, f1), g1));
        let fs = f / u;
        let gs = g / u;
        let d = (fs * fs + gs * gs).sqrt();
        let c = fs.abs() / d;
        let r = sign(d, f);
        let s = gs / r;
        (c, s, r * u)
    }
}

/// `DLAS2`: the singular values `(ssmin, ssmax)` of `[f g; 0 h]`.
pub fn dlas2(f: f64, g: f64, h: f64) -> (f64, f64) {
    let fa = f.abs();
    let ga = g.abs();
    let ha = h.abs();
    let fhmn = fmin(fa, ha);
    let fhmx = fmax(fa, ha);
    if fhmn == 0.0 {
        let ssmax = if fhmx == 0.0 {
            ga
        } else {
            let q = fmin(fhmx, ga) / fmax(fhmx, ga);
            fmax(fhmx, ga) * (1.0 + q * q).sqrt()
        };
        (0.0, ssmax)
    } else if ga < fhmx {
        let as_ = 1.0 + fhmn / fhmx;
        let at = (fhmx - fhmn) / fhmx;
        let q = ga / fhmx;
        let au = q * q;
        let c = 2.0 / ((as_ * as_ + au).sqrt() + (at * at + au).sqrt());
        (fhmn * c, fhmx / c)
    } else {
        let au = fhmx / ga;
        if au == 0.0 {
            ((fhmn * fhmx) / ga, ga)
        } else {
            let as_ = 1.0 + fhmn / fhmx;
            let at = (fhmx - fhmn) / fhmx;
            let p = as_ * au;
            let q = at * au;
            let c = 1.0 / ((1.0 + p * p).sqrt() + (1.0 + q * q).sqrt());
            let mut ssmin = (fhmn * c) * au;
            ssmin += ssmin;
            (ssmin, ga / (c + c))
        }
    }
}

/// `DLASV2`: the SVD of `[f g; 0 h]`: `(ssmin, ssmax, snr, csr, snl, csl)`.
pub fn dlasv2(f: f64, g: f64, h: f64) -> (f64, f64, f64, f64, f64, f64) {
    let mut ft = f;
    let mut fa = ft.abs();
    let mut ht = h;
    let mut ha = h.abs();
    let mut pmax = 1;
    let swap = ha > fa;
    if swap {
        pmax = 3;
        std::mem::swap(&mut ft, &mut ht);
        std::mem::swap(&mut fa, &mut ha);
    }
    let gt = g;
    let ga = gt.abs();
    let (mut ssmin, mut ssmax, clt, crt, slt, srt);
    if ga == 0.0 {
        ssmin = ha;
        ssmax = fa;
        clt = 1.0;
        crt = 1.0;
        slt = 0.0;
        srt = 0.0;
    } else {
        let mut gasmal = true;
        let mut v = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        if ga > fa {
            pmax = 2;
            if (fa / ga) < EPS {
                gasmal = false;
                let smax = ga;
                let smin = if ha > 1.0 {
                    fa / (ga / ha)
                } else {
                    (fa / ga) * ha
                };
                v = (smin, smax, 1.0, ft / gt, ht / gt, 1.0);
            }
        }
        if gasmal {
            let d = fa - ha;
            let mut l = if d == fa { 1.0 } else { d / fa };
            let m = gt / ft;
            let mut t = 2.0 - l;
            let mm = m * m;
            let tt = t * t;
            let s = (tt + mm).sqrt();
            let r = if l == 0.0 {
                m.abs()
            } else {
                (l * l + mm).sqrt()
            };
            let a = 0.5 * (s + r);
            let smin = ha / a;
            let smax = fa * a;
            if mm == 0.0 {
                if l == 0.0 {
                    t = sign(2.0, ft) * sign(1.0, gt);
                } else {
                    t = gt / sign(d, ft) + m / t;
                }
            } else {
                t = (m / (s + t) + m / (r + l)) * (1.0 + a);
            }
            l = (t * t + 4.0).sqrt();
            let crt_ = 2.0 / l;
            let srt_ = t / l;
            let clt_ = (crt_ + srt_ * m) / a;
            let slt_ = ((ht / ft) * srt_) / a;
            v = (smin, smax, clt_, crt_, slt_, srt_);
        }
        ssmin = v.0;
        ssmax = v.1;
        clt = v.2;
        crt = v.3;
        slt = v.4;
        srt = v.5;
    }
    let (csl, snl, csr, snr) = if swap {
        (srt, crt, slt, clt)
    } else {
        (clt, slt, crt, srt)
    };
    let tsign = match pmax {
        1 => sign(1.0, csr) * sign(1.0, csl) * sign(1.0, f),
        2 => sign(1.0, snr) * sign(1.0, csl) * sign(1.0, g),
        _ => sign(1.0, snr) * sign(1.0, snl) * sign(1.0, h),
    };
    ssmax = sign(ssmax, tsign);
    ssmin = sign(ssmin, tsign * sign(1.0, f) * sign(1.0, h));
    (ssmin, ssmax, snr, csr, snl, csl)
}

/// `DLAEV2`: the eigen-decomposition of `[a b; b c]`: `(rt1, rt2, cs1, sn1)`.
pub fn dlaev2(a: f64, b: f64, c: f64) -> (f64, f64, f64, f64) {
    let sm = a + c;
    let df = a - c;
    let adf = df.abs();
    let tb = b + b;
    let ab = tb.abs();
    let (acmx, acmn) = if a.abs() > c.abs() { (a, c) } else { (c, a) };
    let rt = if adf > ab {
        let q = ab / adf;
        adf * (1.0 + q * q).sqrt()
    } else if adf < ab {
        let q = adf / ab;
        ab * (1.0 + q * q).sqrt()
    } else {
        ab * 2f64.sqrt()
    };
    let (rt1, rt2, sgn1);
    if sm < 0.0 {
        rt1 = 0.5 * (sm - rt);
        sgn1 = -1;
        rt2 = (acmx / rt1) * acmn - (b / rt1) * b;
    } else if sm > 0.0 {
        rt1 = 0.5 * (sm + rt);
        sgn1 = 1;
        rt2 = (acmx / rt1) * acmn - (b / rt1) * b;
    } else {
        rt1 = 0.5 * rt;
        rt2 = -(0.5 * rt);
        sgn1 = 1;
    }
    let (cs, sgn2) = if df >= 0.0 {
        (df + rt, 1)
    } else {
        (df - rt, -1)
    };
    let acs = cs.abs();
    let (mut cs1, mut sn1);
    if acs > ab {
        let ct = -tb / cs;
        sn1 = 1.0 / (1.0 + ct * ct).sqrt();
        cs1 = ct * sn1;
    } else if ab == 0.0 {
        cs1 = 1.0;
        sn1 = 0.0;
    } else {
        let tn = -cs / tb;
        cs1 = 1.0 / (1.0 + tn * tn).sqrt();
        sn1 = tn * cs1;
    }
    if sgn1 == sgn2 {
        let tn = cs1;
        cs1 = -sn1;
        sn1 = tn;
    }
    (rt1, rt2, cs1, sn1)
}

/// `DLASCL`'s matrix types used here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scl {
    /// `'G'`: a full matrix.
    General,
    /// `'U'`: the upper triangle.
    Upper,
}

/// `DLASCL`: multiplies the `m × n` matrix by `cto/cfrom` without overflow, in as many steps as
/// it needs.
pub fn dlascl(kind: Scl, cfrom: f64, cto: f64, m: usize, n: usize, a: &mut [f64], am: MatRef) {
    if n == 0 || m == 0 {
        return;
    }
    let smlnum = SAFMIN;
    let bignum = 1.0 / smlnum;
    let mut cfromc = cfrom;
    let mut ctoc = cto;
    loop {
        let cfrom1 = cfromc * smlnum;
        let (mul, done);
        if cfrom1 == cfromc {
            mul = ctoc / cfromc;
            done = true;
        } else {
            let cto1 = ctoc / bignum;
            if cto1 == ctoc {
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
                    return;
                }
            }
        }
        for j in 0..n {
            let rows = match kind {
                Scl::General => 0..m,
                Scl::Upper => 0..(j + 1).min(m),
            };
            for i in rows {
                a[am.at(i, j)] *= mul;
            }
        }
        if done {
            break;
        }
    }
}

/// `DLASR` with `PIVOT = 'V'`: the plane rotations `(c[j], s[j])` applied to the pairs of rows
/// (`left`) or columns `j, j + 1`, forward or backward.
#[allow(clippy::too_many_arguments)]
pub fn dlasr(
    left: bool,
    forward: bool,
    m: usize,
    n: usize,
    c: &[f64],
    s: &[f64],
    a: &mut [f64],
    am: MatRef,
) {
    if m == 0 || n == 0 {
        return;
    }
    let count = if left { m - 1 } else { n - 1 };
    let order: Vec<usize> = if forward {
        (0..count).collect()
    } else {
        (0..count).rev().collect()
    };
    for j in order {
        let ct = c[j];
        let st = s[j];
        if ct != 1.0 || st != 0.0 {
            if left {
                for i in 0..n {
                    let (p, q) = (am.at(j + 1, i), am.at(j, i));
                    let temp = a[p];
                    a[p] = ct * temp - st * a[q];
                    a[q] = st * temp + ct * a[q];
                }
            } else {
                for i in 0..m {
                    let (p, q) = (am.at(i, j + 1), am.at(i, j));
                    let temp = a[p];
                    a[p] = ct * temp - st * a[q];
                    a[q] = st * temp + ct * a[q];
                }
            }
        }
    }
}

/// `DLASET`: the strictly lower (`Some(false)`), strictly upper (`Some(true)`) or every
/// (`None`) off-diagonal element set to `alpha`, the diagonal to `beta`.
#[allow(clippy::too_many_arguments)]
pub fn dlaset(
    upper: Option<bool>,
    m: usize,
    n: usize,
    alpha: f64,
    beta: f64,
    a: &mut [f64],
    am: MatRef,
) {
    match upper {
        Some(true) => {
            for j in 1..n {
                for i in 0..j.min(m) {
                    a[am.at(i, j)] = alpha;
                }
            }
        }
        Some(false) => {
            for j in 0..m.min(n) {
                for i in j + 1..m {
                    a[am.at(i, j)] = alpha;
                }
            }
        }
        None => {
            for j in 0..n {
                for i in 0..m {
                    a[am.at(i, j)] = alpha;
                }
            }
        }
    }
    for i in 0..m.min(n) {
        a[am.at(i, i)] = beta;
    }
}

/// Exchanges rows `r1` and `r2` (over `n` columns) or columns (over `m` rows): `DSWAP`.
fn swap_cols(a: &mut [f64], am: MatRef, rows: usize, c1: usize, c2: usize) {
    for r in 0..rows {
        a.swap(am.at(r, c1), am.at(r, c2));
    }
}

fn swap_rows(a: &mut [f64], am: MatRef, cols: usize, r1: usize, r2: usize) {
    for c in 0..cols {
        a.swap(am.at(r1, c), am.at(r2, c));
    }
}

/// `DLANST('M')`: the largest magnitude of `d` and `e` (a NaN wins).
fn dlanst_m(d: &[f64], e: &[f64]) -> f64 {
    let n = d.len();
    if n == 0 {
        return 0.0;
    }
    let mut anorm = d[n - 1].abs();
    for i in 0..n - 1 {
        let sum = d[i].abs();
        if anorm < sum || sum.is_nan() {
            anorm = sum;
        }
        let sum = e[i].abs();
        if anorm < sum || sum.is_nan() {
            anorm = sum;
        }
    }
    anorm
}

/// `DLARFG`: the elementary reflector `H` with `H·[alpha; x] = [beta; 0]`; `alpha` becomes
/// `beta`, `x` the reflector's tail (`x` gathered, `incx` its stride for `dnrm2` and `dscal`).
/// Returns `tau`.
pub fn dlarfg(alpha: &mut f64, x: &mut [f64], incx: usize) -> f64 {
    let n = x.len() + 1;
    if n <= 1 {
        return 0.0;
    }
    let mut xnorm = blas::dnrm2(x, incx);
    if xnorm == 0.0 {
        return 0.0;
    }
    let mut beta = -sign(dlapy2(*alpha, xnorm), *alpha);
    let safmin = SAFMIN / EPS;
    let mut knt = 0;
    if beta.abs() < safmin {
        let rsafmn = 1.0 / safmin;
        loop {
            knt += 1;
            blas::dscal(rsafmn, x);
            beta *= rsafmn;
            *alpha *= rsafmn;
            if !(beta.abs() < safmin && knt < 20) {
                break;
            }
        }
        xnorm = blas::dnrm2(x, incx);
        beta = -sign(dlapy2(*alpha, xnorm), *alpha);
    }
    let tau = (beta - *alpha) / beta;
    blas::dscal(1.0 / (*alpha - beta), x);
    for _ in 0..knt {
        beta *= safmin;
    }
    *alpha = beta;
    tau
}

/// `ILADLC`: the last non-zero column of the `m × n` matrix (0 when none).
fn iladlc(m: usize, n: usize, c: &[f64], cm: MatRef) -> usize {
    if n == 0 {
        return 0;
    }
    if c[cm.at(0, n - 1)] != 0.0 || c[cm.at(m - 1, n - 1)] != 0.0 {
        return n;
    }
    for j in (1..=n).rev() {
        for i in 0..m {
            if c[cm.at(i, j - 1)] != 0.0 {
                return j;
            }
        }
    }
    0
}

/// `ILADLR`: the last non-zero row of the `m × n` matrix (0 when none).
fn iladlr(m: usize, n: usize, c: &[f64], cm: MatRef) -> usize {
    if m == 0 {
        return 0;
    }
    if c[cm.at(m - 1, 0)] != 0.0 || c[cm.at(m - 1, n - 1)] != 0.0 {
        return m;
    }
    let mut r = 0;
    for j in 0..n {
        let mut i = m;
        while i >= 1 && c[cm.at(i - 1, j)] == 0.0 {
            i -= 1;
        }
        r = r.max(i);
    }
    r
}

/// A row of `n` elements of a column-major matrix, gathered.
fn row(c: &[f64], cm: MatRef, r: usize, n: usize) -> Vec<f64> {
    (0..n).map(|j| c[cm.at(r, j)]).collect()
}

fn put_row(c: &mut [f64], cm: MatRef, r: usize, v: &[f64]) {
    for (j, &x) in v.iter().enumerate() {
        c[cm.at(r, j)] = x;
    }
}

fn col(c: &[f64], cm: MatRef, j: usize, m: usize) -> Vec<f64> {
    (0..m).map(|i| c[cm.at(i, j)]).collect()
}

fn put_col(c: &mut [f64], cm: MatRef, j: usize, v: &[f64]) {
    for (i, &x) in v.iter().enumerate() {
        c[cm.at(i, j)] = x;
    }
}

/// `DLARF1F`: `C = H·C` (`left`) or `C·H` for the `m × n` matrix `C` and
/// `H = I − tau·v·vᵀ` with `v[0] = 1` implied (`v` gathered, `incv` its stride: only `dgemv`'s
/// and `dger`'s code paths depend on it).
#[allow(clippy::too_many_arguments)]
pub fn dlarf1f(
    left: bool,
    m: usize,
    n: usize,
    v: &[f64],
    incv: usize,
    tau: f64,
    c: &mut [f64],
    cm: MatRef,
) {
    let mut lastv = 1;
    let mut lastc = 0;
    if tau != 0.0 {
        lastv = if left { m } else { n };
        while lastv > 1 && v[lastv - 1] == 0.0 {
            lastv -= 1;
        }
        lastc = if left {
            iladlc(lastv, n, c, cm)
        } else {
            iladlr(m, lastv, c, cm)
        };
    }
    if lastc == 0 {
        return;
    }
    let one_minus = 1.0 - tau;
    if left {
        if lastv == 1 {
            let mut r = row(c, cm, 0, lastc);
            blas::dscal(one_minus, &mut r);
            put_row(c, cm, 0, &r);
        } else {
            let mut work = vec![0.0; lastc];
            let sub = MatRef {
                off: cm.at(1, 0),
                ld: cm.ld,
            };
            blas::dgemv_t(
                lastv - 1,
                lastc,
                1.0,
                c,
                sub,
                &v[1..lastv],
                incv,
                0.0,
                &mut work,
            );
            let r0 = row(c, cm, 0, lastc);
            blas::daxpy(1.0, &r0, cm.ld, &mut work, 1);
            let mut r0 = r0;
            blas::daxpy(-tau, &work, 1, &mut r0, cm.ld);
            put_row(c, cm, 0, &r0);
            blas::dger(-tau, &v[1..lastv], &work, c, sub);
        }
    } else if lastv == 1 {
        let mut k = col(c, cm, 0, lastc);
        blas::dscal(one_minus, &mut k);
        put_col(c, cm, 0, &k);
    } else {
        let mut work = vec![0.0; lastc];
        let sub = MatRef {
            off: cm.at(0, 1),
            ld: cm.ld,
        };
        blas::dgemv_n(
            lastc,
            lastv - 1,
            1.0,
            c,
            sub,
            &v[1..lastv],
            incv,
            0.0,
            &mut work,
        );
        let k0 = col(c, cm, 0, lastc);
        blas::daxpy(1.0, &k0, 1, &mut work, 1);
        let mut k0 = k0;
        blas::daxpy(-tau, &work, 1, &mut k0, 1);
        put_col(c, cm, 0, &k0);
        blas::dger(-tau, &work, &v[1..lastv], c, sub);
    }
}

/// `DLARF1L`: as [`dlarf1f`] with the implied unit element last (`v[m-1]` or `v[n-1]`).
#[allow(clippy::too_many_arguments)]
pub fn dlarf1l(
    left: bool,
    m: usize,
    n: usize,
    v: &[f64],
    incv: usize,
    tau: f64,
    c: &mut [f64],
    cm: MatRef,
) {
    let mut firstv = 1;
    let mut lastc = 0;
    let mut lastv = 0;
    if tau != 0.0 {
        lastv = if left { m } else { n };
        while lastv > firstv && v[firstv - 1] == 0.0 {
            firstv += 1;
        }
        lastc = if left {
            iladlc(lastv, n, c, cm)
        } else {
            iladlr(m, lastv, c, cm)
        };
    }
    if lastc == 0 {
        return;
    }
    let one_minus = 1.0 - tau;
    let vv = &v[firstv - 1..lastv - 1];
    if left {
        if lastv == 0 {
            return;
        }
        if lastv == firstv {
            let mut r = row(c, cm, firstv - 1, lastc);
            blas::dscal(one_minus, &mut r);
            put_row(c, cm, firstv - 1, &r);
        } else {
            let mut work = vec![0.0; lastc];
            let sub = MatRef {
                off: cm.at(firstv - 1, 0),
                ld: cm.ld,
            };
            blas::dgemv_t(lastv - firstv, lastc, 1.0, c, sub, vv, incv, 0.0, &mut work);
            let rl = row(c, cm, lastv - 1, lastc);
            blas::daxpy(1.0, &rl, cm.ld, &mut work, 1);
            let mut rl = rl;
            blas::daxpy(-tau, &work, 1, &mut rl, cm.ld);
            put_row(c, cm, lastv - 1, &rl);
            blas::dger(-tau, vv, &work, c, sub);
        }
    } else {
        if lastv == 0 {
            return;
        }
        if lastv == firstv {
            let mut k = col(c, cm, 0, lastc);
            blas::dscal(one_minus, &mut k);
            put_col(c, cm, 0, &k);
        } else {
            let mut work = vec![0.0; lastc];
            let sub = MatRef {
                off: cm.at(0, firstv - 1),
                ld: cm.ld,
            };
            blas::dgemv_n(lastc, lastv - firstv, 1.0, c, sub, vv, incv, 0.0, &mut work);
            let kl = col(c, cm, lastv - 1, lastc);
            blas::daxpy(1.0, &kl, 1, &mut work, 1);
            let mut kl = kl;
            blas::daxpy(-tau, &work, 1, &mut kl, 1);
            put_col(c, cm, lastv - 1, &kl);
            blas::dger(-tau, &work, vv, c, sub);
        }
    }
}

// ------------------------------------------------------------------------------------------
// The symmetric eigenproblem: dsyevd('V', 'U').

/// `DSYTD2` with the upper triangle: the tridiagonal form `Qᵀ A Q`; `d`, `e` its diagonals,
/// `tau` the reflectors' scalars, the reflectors in `A` above the superdiagonal.
pub fn dsytd2_u(
    n: usize,
    a: &mut [f64],
    am: MatRef,
    d: &mut [f64],
    e: &mut [f64],
    tau: &mut [f64],
) {
    if n == 0 {
        return;
    }
    for i in (1..n).rev() {
        // DLARFG(I, A(I,I+1), A(1,I+1), 1, TAUI): alpha = A(i, i+1), x = A(1..i-1, i+1).
        let mut alpha = a[am.at(i - 1, i)];
        let mut x = col(a, am, i, i - 1);
        let taui = dlarfg(&mut alpha, &mut x, 1);
        a[am.at(i - 1, i)] = alpha;
        for (r, &v) in x.iter().enumerate() {
            a[am.at(r, i)] = v;
        }
        e[i - 1] = a[am.at(i - 1, i)];
        if taui != 0.0 {
            a[am.at(i - 1, i)] = 1.0;
            let v = col(a, am, i, i);
            let mut t = vec![0.0; i];
            blas::dsymv_u(taui, a, am, &v, 0.0, &mut t);
            let alpha2 = -((0.5 * taui) * blas::ddot(&t, 1, &v, 1));
            blas::daxpy(alpha2, &v, 1, &mut t, 1);
            blas::dsyr2_u(-1.0, &v, &t, a, am);
            a[am.at(i - 1, i)] = e[i - 1];
            tau[..i].copy_from_slice(&t);
        }
        d[i] = a[am.at(i, i)];
        tau[i - 1] = taui;
    }
    d[0] = a[am.at(0, 0)];
}

/// `DSTEQR` with `COMPZ = 'I'`: the eigenvalues (ascending, in `d`) and eigenvectors (`z`,
/// `n × n`, set to the identity first) of the symmetric tridiagonal matrix `(d, e)` by the
/// implicit QL or QR method. Returns `info` (the number of off-diagonal elements left when it
/// did not converge in `30n` iterations).
pub fn dsteqr_i(n: usize, d: &mut [f64], e: &mut [f64], z: &mut [f64], zm: MatRef) -> usize {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        z[zm.at(0, 0)] = 1.0;
        return 0;
    }
    let eps = EPS;
    let eps2 = eps * eps;
    let safmin = SAFMIN;
    let safmax = 1.0 / safmin;
    let ssfmax = safmax.sqrt() / 3.0;
    let ssfmin = safmin.sqrt() / eps2;
    dlaset(None, n, n, 0.0, 1.0, z, zm);
    let mut work = vec![0.0f64; 2 * (n - 1)];
    let nmaxit = n * 30;
    let mut jtot = 0;
    let mut l1 = 1usize;
    let nm1 = n - 1;
    // 1-based accessors.
    macro_rules! dd {
        ($i:expr) => {
            d[$i - 1]
        };
    }
    macro_rules! ee {
        ($i:expr) => {
            e[$i - 1]
        };
    }
    'outer: loop {
        // label 10
        if l1 > n {
            break 'outer;
        }
        if l1 > 1 {
            ee!(l1 - 1) = 0.0;
        }
        let mut m = n;
        if l1 <= nm1 {
            for mm in l1..=nm1 {
                let tst = ee!(mm).abs();
                if tst == 0.0 {
                    m = mm;
                    break;
                }
                if tst <= (dd!(mm).abs().sqrt() * dd!(mm + 1).abs().sqrt()) * eps {
                    ee!(mm) = 0.0;
                    m = mm;
                    break;
                }
            }
        }
        // label 30
        let mut l = l1;
        let lsv = l;
        let mut lend = m;
        let lendsv = lend;
        l1 = m + 1;
        if lend == l {
            continue 'outer;
        }
        let anorm = dlanst_m(&d[l - 1..lend], &e[l - 1..lend - 1]);
        let mut iscale = 0;
        if anorm == 0.0 {
            continue 'outer;
        }
        let dm = MatRef { off: 0, ld: n };
        if anorm > ssfmax {
            iscale = 1;
            dlascl(
                Scl::General,
                anorm,
                ssfmax,
                lend - l + 1,
                1,
                &mut d[l - 1..],
                dm,
            );
            dlascl(
                Scl::General,
                anorm,
                ssfmax,
                lend - l,
                1,
                &mut e[l - 1..],
                dm,
            );
        } else if anorm < ssfmin {
            iscale = 2;
            dlascl(
                Scl::General,
                anorm,
                ssfmin,
                lend - l + 1,
                1,
                &mut d[l - 1..],
                dm,
            );
            dlascl(
                Scl::General,
                anorm,
                ssfmin,
                lend - l,
                1,
                &mut e[l - 1..],
                dm,
            );
        }
        if dd!(lend).abs() < dd!(l).abs() {
            lend = lsv;
            l = lendsv;
        }
        if lend > l {
            // QL iteration: label 40.
            loop {
                let mut m = lend;
                if l != lend {
                    for mm in l..lend {
                        let t = ee!(mm).abs();
                        let tst = t * t;
                        if tst <= (eps2 * dd!(mm).abs()) * dd!(mm + 1).abs() + safmin {
                            m = mm;
                            break;
                        }
                    }
                }
                if m < lend {
                    ee!(m) = 0.0;
                }
                let mut p = dd!(l);
                if m == l {
                    // label 80
                    dd!(l) = p;
                    l += 1;
                    if l <= lend {
                        continue;
                    }
                    break;
                }
                if m == l + 1 {
                    let (rt1, rt2, c, s) = dlaev2(dd!(l), ee!(l), dd!(l + 1));
                    work[l - 1] = c;
                    work[n - 1 + l - 1] = s;
                    let zsub = MatRef {
                        off: zm.at(0, l - 1),
                        ld: zm.ld,
                    };
                    let (cw, sw) = work.split_at(n - 1);
                    dlasr(false, false, n, 2, &cw[l - 1..], &sw[l - 1..], z, zsub);
                    dd!(l) = rt1;
                    dd!(l + 1) = rt2;
                    ee!(l) = 0.0;
                    l += 2;
                    if l <= lend {
                        continue;
                    }
                    break;
                }
                if jtot == nmaxit {
                    break;
                }
                jtot += 1;
                let mut g = (dd!(l + 1) - p) / (2.0 * ee!(l));
                let mut r = dlapy2(g, 1.0);
                g = (dd!(m) - p) + (ee!(l) / (g + sign(r, g)));
                let mut s = 1.0;
                let mut c = 1.0;
                p = 0.0;
                let mm1 = m - 1;
                for i in (l..=mm1).rev() {
                    let f = s * ee!(i);
                    let b = c * ee!(i);
                    let (c2, s2, r2) = dlartg(g, f);
                    c = c2;
                    s = s2;
                    r = r2;
                    if i != m - 1 {
                        ee!(i + 1) = r;
                    }
                    g = dd!(i + 1) - p;
                    r = (dd!(i) - g) * s + (2.0 * c) * b;
                    p = s * r;
                    dd!(i + 1) = g + p;
                    g = c * r - b;
                    work[i - 1] = c;
                    work[n - 1 + i - 1] = -s;
                }
                let mm = m - l + 1;
                let zsub = MatRef {
                    off: zm.at(0, l - 1),
                    ld: zm.ld,
                };
                let (cw, sw) = work.split_at(n - 1);
                dlasr(false, false, n, mm, &cw[l - 1..], &sw[l - 1..], z, zsub);
                dd!(l) -= p;
                ee!(l) = g;
            }
        } else {
            // QR iteration: label 90.
            loop {
                let mut m = lend;
                if l != lend {
                    let lendp1 = lend + 1;
                    let mut found = false;
                    let mut mm = l;
                    while mm >= lendp1 {
                        let t = ee!(mm - 1).abs();
                        let tst = t * t;
                        if tst <= (eps2 * dd!(mm).abs()) * dd!(mm - 1).abs() + safmin {
                            found = true;
                            break;
                        }
                        mm -= 1;
                    }
                    if found {
                        m = mm;
                    }
                }
                if m > lend {
                    ee!(m - 1) = 0.0;
                }
                let mut p = dd!(l);
                if m == l {
                    // label 130
                    dd!(l) = p;
                    if l == 1 {
                        break;
                    }
                    l -= 1;
                    if l >= lend {
                        continue;
                    }
                    break;
                }
                if m + 1 == l {
                    let (rt1, rt2, c, s) = dlaev2(dd!(l - 1), ee!(l - 1), dd!(l));
                    work[m - 1] = c;
                    work[n - 1 + m - 1] = s;
                    let zsub = MatRef {
                        off: zm.at(0, l - 2),
                        ld: zm.ld,
                    };
                    let (cw, sw) = work.split_at(n - 1);
                    dlasr(false, true, n, 2, &cw[m - 1..], &sw[m - 1..], z, zsub);
                    dd!(l - 1) = rt1;
                    dd!(l) = rt2;
                    ee!(l - 1) = 0.0;
                    if l < 2 + lend {
                        break;
                    }
                    l -= 2;
                    continue;
                }
                if jtot == nmaxit {
                    break;
                }
                jtot += 1;
                let mut g = (dd!(l - 1) - p) / (2.0 * ee!(l - 1));
                let mut r = dlapy2(g, 1.0);
                g = (dd!(m) - p) + (ee!(l - 1) / (g + sign(r, g)));
                let mut s = 1.0;
                let mut c = 1.0;
                p = 0.0;
                let lm1 = l - 1;
                for i in m..=lm1 {
                    let f = s * ee!(i);
                    let b = c * ee!(i);
                    let (c2, s2, r2) = dlartg(g, f);
                    c = c2;
                    s = s2;
                    r = r2;
                    if i != m {
                        ee!(i - 1) = r;
                    }
                    g = dd!(i) - p;
                    r = (dd!(i + 1) - g) * s + (2.0 * c) * b;
                    p = s * r;
                    dd!(i) = g + p;
                    g = c * r - b;
                    work[i - 1] = c;
                    work[n - 1 + i - 1] = s;
                }
                let mm = l - m + 1;
                let zsub = MatRef {
                    off: zm.at(0, m - 1),
                    ld: zm.ld,
                };
                let (cw, sw) = work.split_at(n - 1);
                dlasr(false, true, n, mm, &cw[m - 1..], &sw[m - 1..], z, zsub);
                dd!(l) -= p;
                ee!(lm1) = g;
            }
        }
        // label 140
        let dm = MatRef { off: 0, ld: n };
        if iscale == 1 {
            dlascl(
                Scl::General,
                ssfmax,
                anorm,
                lendsv - lsv + 1,
                1,
                &mut d[lsv - 1..],
                dm,
            );
            dlascl(
                Scl::General,
                ssfmax,
                anorm,
                lendsv - lsv,
                1,
                &mut e[lsv - 1..],
                dm,
            );
        } else if iscale == 2 {
            dlascl(
                Scl::General,
                ssfmin,
                anorm,
                lendsv - lsv + 1,
                1,
                &mut d[lsv - 1..],
                dm,
            );
            dlascl(
                Scl::General,
                ssfmin,
                anorm,
                lendsv - lsv,
                1,
                &mut e[lsv - 1..],
                dm,
            );
        }
        if jtot >= nmaxit {
            let mut info = 0;
            for i in 0..n - 1 {
                if e[i] != 0.0 {
                    info += 1;
                }
            }
            return info;
        }
    }
    // label 160: selection sort, ascending, with the vectors.
    for ii in 2..=n {
        let i = ii - 1;
        let mut k = i;
        let mut p = dd!(i);
        for j in ii..=n {
            if dd!(j) < p {
                k = j;
                p = dd!(j);
            }
        }
        if k != i {
            dd!(k) = dd!(i);
            dd!(i) = p;
            swap_cols(z, zm, n, i - 1, k - 1);
        }
    }
    0
}

/// `DORM2L`: `C = Q·C` (`left`, no transpose) for the `m × n` matrix `C`, `Q` the product of the
/// `k` reflectors stored in the columns of `A` (`m × k` for `left`) with their unit element last.
#[allow(clippy::too_many_arguments)]
pub fn dorm2l_ln(
    m: usize,
    n: usize,
    k: usize,
    a: &[f64],
    am: MatRef,
    tau: &[f64],
    c: &mut [f64],
    cm: MatRef,
) {
    if m == 0 || n == 0 || k == 0 {
        return;
    }
    // LEFT and NOTRAN: I1 = K, I2 = 1, I3 = -1.
    for i in (1..=k).rev() {
        let mi = m - k + i;
        let v = col(a, am, i - 1, mi);
        dlarf1l(true, mi, n, &v, 1, tau[i - 1], c, cm);
    }
}

/// `DSYEVD` with `JOBZ = 'V'`, `UPLO = 'U'` for the `n × n` matrix in `a` (`lda = n`): the
/// eigenvalues ascending in `w`, the eigenvectors in `a`'s columns. Returns `info`.
pub fn dsyevd_vu(n: usize, a: &mut [f64], w: &mut [f64]) -> usize {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        w[0] = a[0];
        a[0] = 1.0;
        return 0;
    }
    let am = MatRef { off: 0, ld: n };
    let safmin = SAFMIN;
    let eps = PREC;
    let smlnum = safmin / eps;
    let bignum = 1.0 / smlnum;
    let rmin = smlnum.sqrt();
    let rmax = bignum.sqrt();
    // DLANSY('M', 'U')
    let mut anrm = 0.0f64;
    for j in 0..n {
        for i in 0..=j {
            let sum = a[am.at(i, j)].abs();
            if anrm < sum || sum.is_nan() {
                anrm = sum;
            }
        }
    }
    let mut iscale = false;
    let mut sigma = 1.0;
    if anrm > 0.0 && anrm < rmin {
        iscale = true;
        sigma = rmin / anrm;
    } else if anrm > rmax {
        iscale = true;
        sigma = rmax / anrm;
    }
    if iscale {
        dlascl(Scl::Upper, 1.0, sigma, n, n, a, am);
    }
    let mut e = vec![0.0; n];
    let mut tau = vec![0.0; n];
    // DSYTRD: NB = 32 > N, so DSYTD2 on the whole matrix.
    dsytd2_u(n, a, am, w, &mut e, &mut tau);
    let mut z = vec![0.0; n * n];
    let zm = MatRef { off: 0, ld: n };
    // DSTEDC('I'): N <= SMLSIZ (25), so DSTEQR.
    let info = dsteqr_i(n, w, &mut e, &mut z, zm);
    // DORMTR('L', 'U', 'N'): DORMQL on the n-1 reflectors in A(1, 2), C = Z; NB >= K: DORM2L.
    let asub = MatRef {
        off: am.at(0, 1),
        ld: n,
    };
    dorm2l_ln(n - 1, n, n - 1, a, asub, &tau, &mut z, zm);
    a[..n * n].copy_from_slice(&z);
    if iscale {
        blas::dscal(1.0 / sigma, &mut w[..n]);
    }
    info
}

// ------------------------------------------------------------------------------------------
// The singular value decomposition: dgesdd('S') for m >= n.

/// `DGEBD2` for `m ≥ n`: the upper bidiagonal form `Qᵀ A P`; `d`, `e` its diagonals, the
/// reflectors in `A` (`Q`'s below the diagonal, `P`'s right of the superdiagonal) with their
/// scalars `tauq`, `taup`.
#[allow(clippy::too_many_arguments)]
pub fn dgebd2(
    m: usize,
    n: usize,
    a: &mut [f64],
    am: MatRef,
    d: &mut [f64],
    e: &mut [f64],
    tauq: &mut [f64],
    taup: &mut [f64],
) {
    assert!(m >= n, "dgebd2 is translated for m >= n");
    for i in 1..=n {
        // DLARFG(M-I+1, A(I,I), A(MIN(I+1,M),I), 1, TAUQ(I))
        let mut alpha = a[am.at(i - 1, i - 1)];
        let mut x: Vec<f64> = (i..m).map(|r| a[am.at(r, i - 1)]).collect();
        tauq[i - 1] = dlarfg(&mut alpha, &mut x, 1);
        a[am.at(i - 1, i - 1)] = alpha;
        for (k, &v) in x.iter().enumerate() {
            a[am.at(i + k, i - 1)] = v;
        }
        d[i - 1] = a[am.at(i - 1, i - 1)];
        if i < n {
            // DLARF1F('Left', M-I+1, N-I, A(I,I), 1, TAUQ(I), A(I,I+1), LDA)
            let v: Vec<f64> = (i - 1..m).map(|r| a[am.at(r, i - 1)]).collect();
            let cm = MatRef {
                off: am.at(i - 1, i),
                ld: am.ld,
            };
            dlarf1f(true, m - i + 1, n - i, &v, 1, tauq[i - 1], a, cm);
            // DLARFG(N-I, A(I,I+1), A(I,MIN(I+2,N)), LDA, TAUP(I))
            let mut alpha = a[am.at(i - 1, i)];
            let mut x: Vec<f64> = (i + 1..n).map(|c| a[am.at(i - 1, c)]).collect();
            taup[i - 1] = dlarfg(&mut alpha, &mut x, am.ld);
            a[am.at(i - 1, i)] = alpha;
            for (k, &v) in x.iter().enumerate() {
                a[am.at(i - 1, i + 1 + k)] = v;
            }
            e[i - 1] = a[am.at(i - 1, i)];
            // DLARF1F('Right', M-I, N-I, A(I,I+1), LDA, TAUP(I), A(I+1,I+1), LDA)
            let v: Vec<f64> = (i..n).map(|c| a[am.at(i - 1, c)]).collect();
            let cm = MatRef {
                off: am.at(i, i),
                ld: am.ld,
            };
            dlarf1f(false, m - i, n - i, &v, am.ld, taup[i - 1], a, cm);
        } else {
            taup[i - 1] = 0.0;
        }
    }
}

/// `DGEQR2`: `A = Q·R`, the reflectors below the diagonal with their scalars `tau`.
pub fn dgeqr2(m: usize, n: usize, a: &mut [f64], am: MatRef, tau: &mut [f64]) {
    let k = m.min(n);
    for i in 1..=k {
        let mut alpha = a[am.at(i - 1, i - 1)];
        let mut x: Vec<f64> = (i..m).map(|r| a[am.at(r, i - 1)]).collect();
        tau[i - 1] = dlarfg(&mut alpha, &mut x, 1);
        a[am.at(i - 1, i - 1)] = alpha;
        for (kk, &v) in x.iter().enumerate() {
            a[am.at(i + kk, i - 1)] = v;
        }
        if i < n {
            let v: Vec<f64> = (i - 1..m).map(|r| a[am.at(r, i - 1)]).collect();
            let cm = MatRef {
                off: am.at(i - 1, i),
                ld: am.ld,
            };
            dlarf1f(true, m - i + 1, n - i, &v, 1, tau[i - 1], a, cm);
        }
    }
}

/// `DORG2R`: the first `n` columns of `Q` from `k` reflectors, in place.
pub fn dorg2r(m: usize, n: usize, k: usize, a: &mut [f64], am: MatRef, tau: &[f64]) {
    if n == 0 {
        return;
    }
    for j in k + 1..=n {
        for l in 0..m {
            a[am.at(l, j - 1)] = 0.0;
        }
        a[am.at(j - 1, j - 1)] = 1.0;
    }
    for i in (1..=k).rev() {
        if i < n {
            let v: Vec<f64> = (i - 1..m).map(|r| a[am.at(r, i - 1)]).collect();
            let cm = MatRef {
                off: am.at(i - 1, i),
                ld: am.ld,
            };
            dlarf1f(true, m - i + 1, n - i, &v, 1, tau[i - 1], a, cm);
        }
        if i < m {
            let mut x: Vec<f64> = (i..m).map(|r| a[am.at(r, i - 1)]).collect();
            blas::dscal(-tau[i - 1], &mut x);
            for (kk, &v) in x.iter().enumerate() {
                a[am.at(i + kk, i - 1)] = v;
            }
        }
        a[am.at(i - 1, i - 1)] = 1.0 - tau[i - 1];
        for l in 0..i - 1 {
            a[am.at(l, i - 1)] = 0.0;
        }
    }
}

/// `DORM2R` with `SIDE = 'L'`, `TRANS = 'N'`: `C = Q·C` for the `m × n` matrix `C`, `Q` from the
/// `k` reflectors in `A`'s columns.
#[allow(clippy::too_many_arguments)]
fn dorm2r_ln(
    m: usize,
    n: usize,
    k: usize,
    a: &[f64],
    am: MatRef,
    tau: &[f64],
    c: &mut [f64],
    cm: MatRef,
) {
    if m == 0 || n == 0 || k == 0 {
        return;
    }
    for i in (1..=k).rev() {
        let mi = m - i + 1;
        let v: Vec<f64> = (i - 1..m).map(|r| a[am.at(r, i - 1)]).collect();
        let sub = MatRef {
            off: cm.at(i - 1, 0),
            ld: cm.ld,
        };
        dlarf1f(true, mi, n, &v, 1, tau[i - 1], c, sub);
    }
}

/// `DORML2` with `SIDE = 'R'`, `TRANS = 'N'`: `C = C·Q` for the `m × n` matrix `C`, `Q` from the
/// `k` reflectors in `A`'s rows.
#[allow(clippy::too_many_arguments)]
fn dorml2_rn(
    m: usize,
    n: usize,
    k: usize,
    a: &[f64],
    am: MatRef,
    tau: &[f64],
    c: &mut [f64],
    cm: MatRef,
) {
    if m == 0 || n == 0 || k == 0 {
        return;
    }
    for i in (1..=k).rev() {
        let ni = n - i + 1;
        let v: Vec<f64> = (i - 1..n).map(|cc| a[am.at(i - 1, cc)]).collect();
        let sub = MatRef {
            off: cm.at(0, i - 1),
            ld: cm.ld,
        };
        dlarf1f(false, m, ni, &v, am.ld, tau[i - 1], c, sub);
    }
}

/// `DBDSQR` with `UPLO = 'U'` and no `C` (`ncc = 0`): the singular values of the upper
/// bidiagonal `(d, e)` by implicit zero-shift or shifted QR sweeps, applied to the `ncvt`
/// columns of `VT` (rows rotated) and the `nru` rows of `U` (columns rotated); `d` then sorted
/// decreasing with the vectors. Returns `info`.
#[allow(clippy::too_many_arguments)]
pub fn dbdsqr_u(
    n: usize,
    ncvt: usize,
    nru: usize,
    d: &mut [f64],
    e: &mut [f64],
    vt: &mut [f64],
    vtm: MatRef,
    u: &mut [f64],
    um: MatRef,
) -> usize {
    if n == 0 {
        return 0;
    }
    macro_rules! dd {
        ($i:expr) => {
            d[$i - 1]
        };
    }
    macro_rules! ee {
        ($i:expr) => {
            e[$i - 1]
        };
    }
    let maxitr = 6usize;
    if n > 1 {
        let rotate = ncvt > 0 || nru > 0;
        assert!(rotate, "dbdsqr without vectors (dlasq1) is not translated");
        let nm1 = n - 1;
        let nm12 = nm1 + nm1;
        let nm13 = nm12 + nm1;
        let mut work = vec![0.0f64; 4 * nm1];
        let mut idir = 0;
        let eps = EPS;
        let unfl = SAFMIN;
        let tolmul = fmax(10.0, fmin(100.0, crate::glibc_double::pow(eps, -0.125)));
        let tol = tolmul * eps;
        // TOL >= 0: the relative-accuracy threshold.
        let mut sminoa = dd!(1).abs();
        if sminoa != 0.0 {
            let mut mu = sminoa;
            for i in 2..=n {
                mu = dd!(i).abs() * (mu / (mu + ee!(i - 1).abs()));
                sminoa = fmin(sminoa, mu);
                if sminoa == 0.0 {
                    break;
                }
            }
        }
        sminoa /= (n as f64).sqrt();
        let thresh = fmax(
            tol * sminoa,
            (maxitr as f64) * ((n as f64) * ((n as f64) * unfl)),
        );
        let maxitdivn = maxitr * n;
        let mut iterdivn = 0usize;
        let mut iter: isize = -1;
        let mut oldll: isize = -1;
        let mut oldm: isize = -1;
        let mut m = n;
        let mut smin;
        let vt_rows = |vt: &mut [f64], r1: usize, r2: usize, c: f64, s: f64| {
            // DROT(NCVT, VT(r1,1), LDVT, VT(r2,1), LDVT, c, s)
            let mut x: Vec<f64> = (0..ncvt).map(|j| vt[vtm.at(r1 - 1, j)]).collect();
            let mut y: Vec<f64> = (0..ncvt).map(|j| vt[vtm.at(r2 - 1, j)]).collect();
            blas::drot(&mut x, vtm.ld, &mut y, vtm.ld, c, s);
            for j in 0..ncvt {
                vt[vtm.at(r1 - 1, j)] = x[j];
                vt[vtm.at(r2 - 1, j)] = y[j];
            }
        };
        let u_cols = |u: &mut [f64], c1: usize, c2: usize, c: f64, s: f64| {
            let mut x: Vec<f64> = (0..nru).map(|i| u[um.at(i, c1 - 1)]).collect();
            let mut y: Vec<f64> = (0..nru).map(|i| u[um.at(i, c2 - 1)]).collect();
            blas::drot(&mut x, 1, &mut y, 1, c, s);
            for i in 0..nru {
                u[um.at(i, c1 - 1)] = x[i];
                u[um.at(i, c2 - 1)] = y[i];
            }
        };
        'sixty: loop {
            if m <= 1 {
                break 'sixty;
            }
            if iter >= n as isize {
                iter -= n as isize;
                iterdivn += 1;
                if iterdivn >= maxitdivn {
                    let mut info = 0;
                    for i in 0..n - 1 {
                        if e[i] != 0.0 {
                            info += 1;
                        }
                    }
                    return info;
                }
            }
            let mut smax = dd!(m).abs();
            let mut ll = 0usize;
            let mut split = false;
            for lll in 1..m {
                let l = m - lll;
                let abss = dd!(l).abs();
                let abse = ee!(l).abs();
                if abse <= thresh {
                    ll = l;
                    split = true;
                    break;
                }
                smax = fmax(fmax(smax, abss), abse);
            }
            if split {
                // label 80
                ee!(ll) = 0.0;
                if ll == m - 1 {
                    m -= 1;
                    continue 'sixty;
                }
            }
            // label 90
            ll += 1;
            if ll == m - 1 {
                let (sigmn, sigmx, sinr, cosr, sinl, cosl) = dlasv2(dd!(m - 1), ee!(m - 1), dd!(m));
                dd!(m - 1) = sigmx;
                ee!(m - 1) = 0.0;
                dd!(m) = sigmn;
                if ncvt > 0 {
                    vt_rows(vt, m - 1, m, cosr, sinr);
                }
                if nru > 0 {
                    u_cols(u, m - 1, m, cosl, sinl);
                }
                m -= 2;
                continue 'sixty;
            }
            if ll as isize > oldm || (m as isize) < oldll {
                idir = if dd!(ll).abs() >= dd!(m).abs() { 1 } else { 2 };
            }
            if idir == 1 {
                if ee!(m - 1).abs() <= tol.abs() * dd!(m).abs() {
                    ee!(m - 1) = 0.0;
                    continue 'sixty;
                }
                let mut mu = dd!(ll).abs();
                smin = mu;
                for lll in ll..m {
                    if ee!(lll).abs() <= tol * mu {
                        ee!(lll) = 0.0;
                        continue 'sixty;
                    }
                    mu = dd!(lll + 1).abs() * (mu / (mu + ee!(lll).abs()));
                    smin = fmin(smin, mu);
                }
            } else {
                if ee!(ll).abs() <= tol.abs() * dd!(ll).abs() {
                    ee!(ll) = 0.0;
                    continue 'sixty;
                }
                let mut mu = dd!(m).abs();
                smin = mu;
                for lll in (ll..m).rev() {
                    if ee!(lll).abs() <= tol * mu {
                        ee!(lll) = 0.0;
                        continue 'sixty;
                    }
                    mu = dd!(lll).abs() * (mu / (mu + ee!(lll).abs()));
                    smin = fmin(smin, mu);
                }
            }
            oldll = ll as isize;
            oldm = m as isize;
            let mut shift;
            if ((n as f64) * tol) * (smin / smax) <= fmax(eps, 0.01 * tol) {
                shift = 0.0;
            } else {
                let sll;
                if idir == 1 {
                    sll = dd!(ll).abs();
                    shift = dlas2(dd!(m - 1), ee!(m - 1), dd!(m)).0;
                } else {
                    sll = dd!(m).abs();
                    shift = dlas2(dd!(ll), ee!(ll), dd!(ll + 1)).0;
                }
                if sll > 0.0 {
                    let q = shift / sll;
                    if q * q < eps {
                        shift = 0.0;
                    }
                }
            }
            iter += (m - ll) as isize;
            let lenr = m - ll + 1;
            let vsub = MatRef {
                off: vtm.at(ll - 1, 0),
                ld: vtm.ld,
            };
            let usub = MatRef {
                off: um.at(0, ll - 1),
                ld: um.ld,
            };
            if shift == 0.0 {
                if idir == 1 {
                    let mut cs = 1.0;
                    let mut oldcs = 1.0;
                    let mut oldsn = 0.0;
                    for i in ll..m {
                        let (c1, sn, r) = dlartg(dd!(i) * cs, ee!(i));
                        cs = c1;
                        if i > ll {
                            ee!(i - 1) = oldsn * r;
                        }
                        let (c2, s2, r2) = dlartg(oldcs * r, dd!(i + 1) * sn);
                        oldcs = c2;
                        oldsn = s2;
                        dd!(i) = r2;
                        work[i - ll] = cs;
                        work[i - ll + nm1] = sn;
                        work[i - ll + nm12] = oldcs;
                        work[i - ll + nm13] = oldsn;
                    }
                    let h = dd!(m) * cs;
                    dd!(m) = h * oldcs;
                    ee!(m - 1) = h * oldsn;
                    let (w1, rest) = work.split_at(nm1);
                    let (w2, rest) = rest.split_at(nm1);
                    let (w3, w4) = rest.split_at(nm1);
                    if ncvt > 0 {
                        dlasr(true, true, lenr, ncvt, w1, w2, vt, vsub);
                    }
                    if nru > 0 {
                        dlasr(false, true, nru, lenr, w3, w4, u, usub);
                    }
                    if ee!(m - 1).abs() <= thresh {
                        ee!(m - 1) = 0.0;
                    }
                } else {
                    let mut cs = 1.0;
                    let mut oldcs = 1.0;
                    let mut oldsn = 0.0;
                    for i in (ll + 1..=m).rev() {
                        let (c1, sn, r) = dlartg(dd!(i) * cs, ee!(i - 1));
                        cs = c1;
                        if i < m {
                            ee!(i) = oldsn * r;
                        }
                        let (c2, s2, r2) = dlartg(oldcs * r, dd!(i - 1) * sn);
                        oldcs = c2;
                        oldsn = s2;
                        dd!(i) = r2;
                        work[i - ll - 1] = cs;
                        work[i - ll - 1 + nm1] = -sn;
                        work[i - ll - 1 + nm12] = oldcs;
                        work[i - ll - 1 + nm13] = -oldsn;
                    }
                    let h = dd!(ll) * cs;
                    dd!(ll) = h * oldcs;
                    ee!(ll) = h * oldsn;
                    let (w1, rest) = work.split_at(nm1);
                    let (w2, rest) = rest.split_at(nm1);
                    let (w3, w4) = rest.split_at(nm1);
                    if ncvt > 0 {
                        dlasr(true, false, lenr, ncvt, w3, w4, vt, vsub);
                    }
                    if nru > 0 {
                        dlasr(false, false, nru, lenr, w1, w2, u, usub);
                    }
                    if ee!(ll).abs() <= thresh {
                        ee!(ll) = 0.0;
                    }
                }
            } else if idir == 1 {
                let mut f = (dd!(ll).abs() - shift) * (sign(1.0, dd!(ll)) + shift / dd!(ll));
                let mut g = ee!(ll);
                for i in ll..m {
                    let (cosr, sinr, r) = dlartg(f, g);
                    if i > ll {
                        ee!(i - 1) = r;
                    }
                    f = cosr * dd!(i) + sinr * ee!(i);
                    ee!(i) = cosr * ee!(i) - sinr * dd!(i);
                    g = sinr * dd!(i + 1);
                    dd!(i + 1) *= cosr;
                    let (cosl, sinl, r2) = dlartg(f, g);
                    dd!(i) = r2;
                    f = cosl * ee!(i) + sinl * dd!(i + 1);
                    dd!(i + 1) = cosl * dd!(i + 1) - sinl * ee!(i);
                    if i < m - 1 {
                        g = sinl * ee!(i + 1);
                        ee!(i + 1) *= cosl;
                    }
                    work[i - ll] = cosr;
                    work[i - ll + nm1] = sinr;
                    work[i - ll + nm12] = cosl;
                    work[i - ll + nm13] = sinl;
                }
                ee!(m - 1) = f;
                let (w1, rest) = work.split_at(nm1);
                let (w2, rest) = rest.split_at(nm1);
                let (w3, w4) = rest.split_at(nm1);
                if ncvt > 0 {
                    dlasr(true, true, lenr, ncvt, w1, w2, vt, vsub);
                }
                if nru > 0 {
                    dlasr(false, true, nru, lenr, w3, w4, u, usub);
                }
                if ee!(m - 1).abs() <= thresh {
                    ee!(m - 1) = 0.0;
                }
            } else {
                let mut f = (dd!(m).abs() - shift) * (sign(1.0, dd!(m)) + shift / dd!(m));
                let mut g = ee!(m - 1);
                for i in (ll + 1..=m).rev() {
                    let (cosr, sinr, r) = dlartg(f, g);
                    if i < m {
                        ee!(i) = r;
                    }
                    f = cosr * dd!(i) + sinr * ee!(i - 1);
                    ee!(i - 1) = cosr * ee!(i - 1) - sinr * dd!(i);
                    g = sinr * dd!(i - 1);
                    dd!(i - 1) *= cosr;
                    let (cosl, sinl, r2) = dlartg(f, g);
                    dd!(i) = r2;
                    f = cosl * ee!(i - 1) + sinl * dd!(i - 1);
                    dd!(i - 1) = cosl * dd!(i - 1) - sinl * ee!(i - 1);
                    if i > ll + 1 {
                        g = sinl * ee!(i - 2);
                        ee!(i - 2) *= cosl;
                    }
                    work[i - ll - 1] = cosr;
                    work[i - ll - 1 + nm1] = -sinr;
                    work[i - ll - 1 + nm12] = cosl;
                    work[i - ll - 1 + nm13] = -sinl;
                }
                ee!(ll) = f;
                if ee!(ll).abs() <= thresh {
                    ee!(ll) = 0.0;
                }
                let (w1, rest) = work.split_at(nm1);
                let (w2, rest) = rest.split_at(nm1);
                let (w3, w4) = rest.split_at(nm1);
                if ncvt > 0 {
                    dlasr(true, false, lenr, ncvt, w3, w4, vt, vsub);
                }
                if nru > 0 {
                    dlasr(false, false, nru, lenr, w1, w2, u, usub);
                }
            }
        }
    }
    // label 160: make the values non-negative, then sort decreasing.
    for i in 1..=n {
        if dd!(i) < 0.0 {
            dd!(i) = -dd!(i);
            for j in 0..ncvt {
                vt[vtm.at(i - 1, j)] *= -1.0;
            }
        }
    }
    for i in 1..n {
        let mut isub = 1;
        let mut smin = dd!(1);
        for j in 2..=n + 1 - i {
            if dd!(j) <= smin {
                isub = j;
                smin = dd!(j);
            }
        }
        if isub != n + 1 - i {
            dd!(isub) = dd!(n + 1 - i);
            dd!(n + 1 - i) = smin;
            if ncvt > 0 {
                swap_rows(vt, vtm, ncvt, isub - 1, n - i);
            }
            if nru > 0 {
                swap_cols(u, um, nru, isub - 1, n - i);
            }
        }
    }
    0
}

/// `DBDSDC('U', 'I')` for `n ≤ 25` (`SMLSIZ`): `U` and `VT` (`n × n`) set to the identity, then
/// `DLASDQ` (`DBDSQR` and an ascending sort), then the values sorted decreasing with the
/// vectors. Returns `info`.
#[allow(clippy::too_many_arguments)]
pub fn dbdsdc_ui(
    n: usize,
    d: &mut [f64],
    e: &mut [f64],
    u: &mut [f64],
    um: MatRef,
    vt: &mut [f64],
    vtm: MatRef,
) -> usize {
    if n == 0 {
        return 0;
    }
    assert!(
        n <= 25,
        "dbdsdc's divide and conquer (n > 25) is not translated"
    );
    if n == 1 {
        u[um.at(0, 0)] = sign(1.0, d[0]);
        vt[vtm.at(0, 0)] = 1.0;
        d[0] = d[0].abs();
        return 0;
    }
    dlaset(None, n, n, 0.0, 1.0, u, um);
    dlaset(None, n, n, 0.0, 1.0, vt, vtm);
    // DLASDQ('U', SQRE = 0, N, NCVT = N, NRU = N, NCC = 0): DBDSQR, then ascending.
    let info = dbdsqr_u(n, n, n, d, e, vt, vtm, u, um);
    for i in 1..=n {
        let mut isub = i;
        let mut smin = d[i - 1];
        for j in i + 1..=n {
            if d[j - 1] < smin {
                isub = j;
                smin = d[j - 1];
            }
        }
        if isub != i {
            d[isub - 1] = d[i - 1];
            d[i - 1] = smin;
            swap_rows(vt, vtm, n, isub - 1, i - 1);
            swap_cols(u, um, n, isub - 1, i - 1);
        }
    }
    // label 40: decreasing.
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
            swap_cols(u, um, n, i - 1, kk - 1);
            swap_rows(vt, vtm, n, i - 1, kk - 1);
        }
    }
    info
}

/// `DGESDD` with `JOBZ = 'S'` for `m ≥ n` (`lda = m`, `ldu = m`, `ldvt = n`): `A = U·diag(s)·Vᵀ`,
/// `s` decreasing; `a` is overwritten. Returns `info` (`-4` for a NaN in `A`).
pub fn dgesdd_s(
    m: usize,
    n: usize,
    a: &mut [f64],
    s: &mut [f64],
    u: &mut [f64],
    vt: &mut [f64],
) -> i64 {
    assert!(
        m >= n,
        "dgesdd is translated for m >= n (Armadillo transposes a wide matrix)"
    );
    if m == 0 || n == 0 {
        return 0;
    }
    let minmn = n;
    let mnthr = (minmn as f64 * 11.0 / 6.0) as usize;
    let am = MatRef { off: 0, ld: m };
    let um = MatRef { off: 0, ld: m };
    let vtm = MatRef { off: 0, ld: n };
    let eps = PREC;
    let smlnum = SAFMIN.sqrt() / eps;
    let bignum = 1.0 / smlnum;
    // DLANGE('M')
    let mut anrm = 0.0f64;
    for j in 0..n {
        for i in 0..m {
            let t = a[am.at(i, j)].abs();
            if anrm < t || t.is_nan() {
                anrm = t;
            }
        }
    }
    if anrm.is_nan() {
        return -4;
    }
    let mut iscl = false;
    if anrm > 0.0 && anrm < smlnum {
        iscl = true;
        dlascl(Scl::General, anrm, smlnum, m, n, a, am);
    } else if anrm > bignum {
        iscl = true;
        dlascl(Scl::General, anrm, bignum, m, n, a, am);
    }
    let mut e = vec![0.0; n];
    let mut tauq = vec![0.0; n];
    let mut taup = vec![0.0; n];
    let info;
    if m >= mnthr {
        // Path 3 (M >> N, JOBZ = 'S').
        let mut tau = vec![0.0; n];
        // DGEQRF: NB = 32 >= K, so DGEQR2.
        dgeqr2(m, n, a, am, &mut tau);
        let rm = MatRef { off: 0, ld: n };
        // DLACPY('U') of R, then DLASET('L', N-1, N-1, 0, 0) below its diagonal.
        let mut r = vec![0.0; n * n];
        for j in 0..n {
            for i in 0..=j {
                r[rm.at(i, j)] = a[am.at(i, j)];
            }
        }
        // DORGQR: unblocked, DORG2R.
        dorg2r(m, n, n, a, am, &tau);
        dgebd2(n, n, &mut r, rm, s, &mut e, &mut tauq, &mut taup);
        info = dbdsdc_ui(n, s, &mut e, u, um, vt, vtm);
        // DORMBR('Q', 'L', 'N', N, N, N, R): DORMQR, unblocked.
        dorm2r_ln(n, n, n, &r, rm, &tauq, u, um);
        // DORMBR('P', 'R', 'T', N, N, N, R).
        dormbr_prt(n, n, &r, rm, &taup, vt, vtm);
        let mut w = vec![0.0; n * n];
        for j in 0..n {
            for i in 0..n {
                w[rm.at(i, j)] = u[um.at(i, j)];
            }
        }
        blas::dgemm(false, false, m, n, n, 1.0, a, am, &w, rm, 0.0, u, um);
    } else {
        // Path 5 (M >= N, but not much larger), JOBZ = 'S'.
        dgebd2(m, n, a, am, s, &mut e, &mut tauq, &mut taup);
        dlaset(None, m, n, 0.0, 0.0, u, um);
        info = dbdsdc_ui(n, s, &mut e, u, um, vt, vtm);
        // DORMBR('Q', 'L', 'N', M, N, N): NQ = M >= K, DORMQR, unblocked.
        dorm2r_ln(m, n, n, a, am, &tauq, u, um);
        dormbr_prt(n, n, a, am, &taup, vt, vtm);
    }
    if iscl {
        let sm = MatRef { off: 0, ld: minmn };
        if anrm > bignum {
            dlascl(Scl::General, bignum, anrm, minmn, 1, s, sm);
        }
        if anrm < smlnum {
            dlascl(Scl::General, smlnum, anrm, minmn, 1, s, sm);
        }
    }
    info as i64
}

/// `DORMBR('P', 'R', 'T', m, n, n)` for a square `P` (`nq = n = k`): `C = C·Pᵀ` by `DORMLQ` with
/// `TRANS = 'N'` on the `n − 1` reflectors in `A(1, 2)`, columns 2 to `n` of `C`.
fn dormbr_prt(m: usize, n: usize, a: &[f64], am: MatRef, tau: &[f64], c: &mut [f64], cm: MatRef) {
    if m == 0 || n <= 1 {
        return;
    }
    let asub = MatRef {
        off: am.at(0, 1),
        ld: am.ld,
    };
    let csub = MatRef {
        off: cm.at(0, 1),
        ld: cm.ld,
    };
    // DORMLQ: NB = 32 >= K, so DORML2.
    dorml2_rn(m, n - 1, n - 1, a, asub, tau, c, csub);
}
