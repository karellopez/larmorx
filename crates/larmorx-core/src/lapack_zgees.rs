// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Reference LAPACK 3.10.0's complex Schur factorization `ZGEES` (`JOBVS = 'V'`, `SORT = 'N'`,
//! `LDA = LDVS = N`), translated from the release's Fortran (`<workspace>/tags/lapack-3.10.0`,
//! BSD-3-Clause: the University of Tennessee, the University of California Berkeley, the
//! University of Colorado Denver and NAG Ltd.) as Ubuntu 22.04's `liblapack.a` and `libblas.a`
//! (3.10.0-2ubuntu1, gfortran 11) compute it, for the FreeSurfer replica's matrix square root
//! (`fs.robust`). The routines reached: `ZGEES`, `ZLANGE ('M')`, `ZLASCL ('G', 'U')`, `ZGEBAL
//! ('P')`, `ZGEHRD` (its unblocked path, `ZGEHD2`), `ZLARFG`, `ZLARF`, `ILAZLC`, `ILAZLR`,
//! `ZLACPY ('L')`, `ZUNGHR` (`ZUNGQR`'s unblocked path, `ZUNG2R`), `ZHSEQR` with `ZLAHQR` (the
//! double-shift QR of up to 75 rows), `ZGEBAK ('P', 'R')`, `ZLADIV`/`DLADIV`, `DLAPY3`,
//! `DLAMCH`, and the BLAS `ZGEMV`, `ZGERC`, `ZSCAL`, `ZDSCAL`, `ZCOPY`, `ZSWAP`, `DZNRM2` (the
//! 3.10 Fortran 90 version, Blue's scaled sums).
//!
//! The complex arithmetic is gfortran's (`crate::cplx`: the plain product, Smith's division, a
//! real promoted to `(x, +0)`), `ABS` and `SQRT` of a complex number the decided `cabs` and
//! `csqrt` (`crate::cplx`), and every operation that can meet a NaN follows x86-64's SSE rules.
//! Not ported: the blocked paths (`ZGEHRD` above 128 rows, `ZUNGQR` above 129, `ZLAQR0` above 75
//! rows) and `ZLAQR0`, which `ZHSEQR` calls when `ZLAHQR` does not converge (a matrix with a NaN
//! or an infinite element): [`Zgees::NotPorted`].

use crate::cplx::{C64, cabs, csqrt};
use crate::itk_real::ItkReal;
use crate::vnl_matrix::sse_sqrt;

/// `DLAMCH('P')`: `2^-52`.
const PREC: f64 = f64::EPSILON;
/// `DLAMCH('E')`: `2^-53`.
const EPS: f64 = f64::EPSILON / 2.0;
/// `DLAMCH('S')`: `2^-1022`.
const SAFMIN: f64 = f64::MIN_POSITIVE;

/// What `ZGEES` returned.
#[derive(Clone, Debug, PartialEq)]
pub enum Zgees {
    /// `INFO` (0: success; `1..=N`: the QR iteration failed).
    Done(i32),
    /// An argument check failed: LAPACK's `XERBLA` prints ` ** On entry to <name> parameter
    /// number <i> had an illegal value` on standard output and ends the program (Fortran's
    /// `STOP`: status 0).
    Xerbla(&'static str, i32),
    /// A path of LAPACK not ported (named).
    NotPorted(&'static str),
}

/// `XERBLA`'s line (Fortran's `( ' ** On entry to ', A, ' parameter number ', I2, ' had ', 'an
/// illegal value' )`).
pub fn xerbla_line(name: &str, info: i32) -> String {
    format!(" ** On entry to {name} parameter number {info:>2} had an illegal value\n")
}

/// Element `(i, j)` (1-based) of a column-major array with leading dimension `ld`.
#[inline(always)]
fn ix(i: usize, j: usize, ld: usize) -> usize {
    (i - 1) + (j - 1) * ld
}

/// `ZGEES('V', 'N', SELECT, N, A, N, SDIM, W, VS, N, WORK, 10 N, RWORK, BWORK, INFO)` on the
/// column-major `n x n` matrix `a` (overwritten by its Schur form `T`): `w` the eigenvalues, `vs`
/// the unitary Schur vectors (`A = VS T VS^H`).
pub fn zgees_vn(n: usize, a: &mut [C64], w: &mut [C64], vs: &mut [C64]) -> Zgees {
    if n == 0 {
        // LDA = N = 0 < MAX(1, N)
        return Zgees::Xerbla("ZGEES", 6);
    }
    let lda = n;
    let eps = PREC;
    let smlnum = sse_sqrt(SAFMIN) / eps;
    let bignum = 1.0 / smlnum;
    let anrm = zlange_m(n, n, a, lda);
    let mut scalea = false;
    let mut cscale = 0.0;
    if anrm > 0.0 && anrm < smlnum {
        scalea = true;
        cscale = smlnum;
    } else if anrm > bignum {
        scalea = true;
        cscale = bignum;
    }
    if scalea {
        zlascl(Scl::General, anrm, cscale, n, n, a, lda);
    }
    let mut scale = vec![0.0f64; n];
    let (ilo, ihi) = zgebal_p(n, a, lda, &mut scale);
    let mut tau = vec![C64::ZERO; n];
    let mut work = vec![C64::ZERO; n];
    if let Err(e) = zgehrd(n, ilo, ihi, a, lda, &mut tau, &mut work) {
        return e;
    }
    zlacpy_l(n, n, a, lda, vs, n);
    if let Err(e) = zunghr(n, ilo, ihi, vs, n, &tau, &mut work) {
        return e;
    }
    let info = match zhseqr_sv(n, ilo, ihi, a, lda, w, vs, n) {
        Ok(i) => i,
        Err(e) => return e,
    };
    zgebak_pr(n, ilo, ihi, &scale, n, vs, n);
    if scalea {
        zlascl(Scl::Upper, cscale, anrm, n, n, a, lda);
        for i in 1..=n {
            w[i - 1] = a[ix(i, i, lda)];
        }
    }
    Zgees::Done(info)
}

/// `ZLANGE('M', M, N, A, LDA, WORK)`: the largest `ABS(A(i, j))`, from 0, taking a NaN.
pub fn zlange_m(m: usize, n: usize, a: &[C64], lda: usize) -> f64 {
    let mut value = 0.0f64;
    if m.min(n) == 0 {
        return value;
    }
    for j in 1..=n {
        for i in 1..=m {
            let temp = cabs(a[ix(i, j, lda)]);
            if value < temp || temp.is_nan() {
                value = temp;
            }
        }
    }
    value
}

/// `ZLASCL`'s matrix types reached.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scl {
    /// `'G'`: the whole matrix.
    General,
    /// `'U'`: the upper triangle.
    Upper,
}

/// `ZLASCL(TYPE, 0, 0, CFROM, CTO, M, N, A, LDA, INFO)`: `A` times `CTO / CFROM` in steps that
/// neither overflow nor underflow (each element times the real factor promoted to complex).
fn zlascl(t: Scl, cfrom: f64, cto: f64, m: usize, n: usize, a: &mut [C64], lda: usize) {
    let smlnum = SAFMIN;
    let bignum = 1.0 / smlnum;
    let mut cfromc = cfrom;
    let mut ctoc = cto;
    loop {
        let cfrom1 = cfromc.sse_mul(smlnum);
        let mul;
        let done;
        if cfrom1 == cfromc {
            // CFROMC is an infinity
            mul = ctoc.sse_div(cfromc);
            done = true;
        } else {
            let cto1 = ctoc.sse_div(bignum);
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
                mul = ctoc.sse_div(cfromc);
                done = true;
            }
        }
        let f = C64::real(mul);
        for j in 1..=n {
            let top = match t {
                Scl::General => m,
                Scl::Upper => j.min(m),
            };
            for i in 1..=top {
                let k = ix(i, j, lda);
                a[k] = a[k].mul(f);
            }
        }
        if done {
            return;
        }
    }
}

/// `ZSWAP` of `n` elements at `x` (stride `incx`) and `y` (stride `incy`) of one array.
fn zswap(n: usize, a: &mut [C64], x: usize, incx: usize, y: usize, incy: usize) {
    for k in 0..n {
        a.swap(x + k * incx, y + k * incy);
    }
}

/// `ZGEBAL('P', N, A, LDA, ILO, IHI, SCALE, INFO)`: rows and columns permuted to isolate the
/// eigenvalues; `scale` the permutations' indices (as doubles) outside `ILO..IHI`, 1 inside.
fn zgebal_p(n: usize, a: &mut [C64], lda: usize, scale: &mut [f64]) -> (usize, usize) {
    let mut k = 1usize;
    let mut l = n;
    if n == 0 {
        return (k, l);
    }
    let nonzero = |a: &[C64], i: usize, j: usize| {
        let v = a[ix(i, j, lda)];
        v.re != 0.0 || v.im != 0.0
    };
    // search for rows isolating an eigenvalue and push them down
    'rows: loop {
        let mut found = None;
        for j in (1..=l).rev() {
            if (1..=l).all(|i| i == j || !nonzero(a, j, i)) {
                found = Some(j);
                break;
            }
        }
        let Some(j) = found else { break 'rows };
        let m = l;
        scale[m - 1] = j as f64;
        if j != m {
            zswap(l, a, ix(1, j, lda), 1, ix(1, m, lda), 1);
            zswap(n - k + 1, a, ix(j, k, lda), lda, ix(m, k, lda), lda);
        }
        if l == 1 {
            return (k, l);
        }
        l -= 1;
    }
    // search for columns isolating an eigenvalue and push them left
    'cols: loop {
        let mut found = None;
        for j in k..=l {
            if (k..=l).all(|i| i == j || !nonzero(a, i, j)) {
                found = Some(j);
                break;
            }
        }
        let Some(j) = found else { break 'cols };
        let m = k;
        scale[m - 1] = j as f64;
        if j != m {
            zswap(l, a, ix(1, j, lda), 1, ix(1, m, lda), 1);
            zswap(n - k + 1, a, ix(j, k, lda), lda, ix(m, k, lda), lda);
        }
        k += 1;
    }
    for s in &mut scale[k - 1..l] {
        *s = 1.0;
    }
    (k, l)
}

/// `ZGEBAK('P', 'R', N, ILO, IHI, SCALE, M, V, LDV, INFO)`: the rows of `V` permuted back.
fn zgebak_pr(n: usize, ilo: usize, ihi: usize, scale: &[f64], m: usize, v: &mut [C64], ldv: usize) {
    if n == 0 || m == 0 {
        return;
    }
    for ii in 1..=n {
        let mut i = ii;
        if i >= ilo && i <= ihi {
            continue;
        }
        if i < ilo {
            i = ilo - ii;
        }
        let k = scale[i - 1] as usize;
        if k == i {
            continue;
        }
        zswap(m, v, ix(i, 1, ldv), ldv, ix(k, 1, ldv), ldv);
    }
}

/// `ZGEHRD(N, ILO, IHI, A, LDA, TAU, WORK, LWORK, INFO)` with `LWORK = 9 N` (`ZGEES`'s): the
/// unblocked reduction to upper Hessenberg form (`ZGEHD2`), the path of `IHI - ILO + 1 <= 128`.
fn zgehrd(
    n: usize,
    ilo: usize,
    ihi: usize,
    a: &mut [C64],
    lda: usize,
    tau: &mut [C64],
    work: &mut [C64],
) -> Result<(), Zgees> {
    for t in &mut tau[..ilo - 1] {
        *t = C64::ZERO;
    }
    for i in ihi.max(1)..n {
        tau[i - 1] = C64::ZERO;
    }
    let nh = ihi as isize - ilo as isize + 1;
    if nh <= 1 {
        return Ok(());
    }
    if nh > 128 {
        return Err(Zgees::NotPorted(
            "ZGEHRD's blocked reduction (more than 128 rows)",
        ));
    }
    // ZGEHD2
    for i in ilo..ihi {
        let mut alpha = a[ix(i + 1, i, lda)];
        let xoff = ix((i + 2).min(n), i, lda);
        let t = zlarfg(ihi - i, &mut alpha, a, xoff, 1);
        tau[i - 1] = t;
        a[ix(i + 1, i, lda)] = C64::ONE;
        let v: Vec<C64> = (i + 1..=ihi).map(|r| a[ix(r, i, lda)]).collect();
        zlarf(
            Side::Right,
            ihi,
            ihi - i,
            &v,
            t,
            a,
            ix(1, i + 1, lda),
            lda,
            work,
        );
        zlarf(
            Side::Left,
            ihi - i,
            n - i,
            &v,
            t.conj(),
            a,
            ix(i + 1, i + 1, lda),
            lda,
            work,
        );
        a[ix(i + 1, i, lda)] = alpha;
    }
    Ok(())
}

/// `DZNRM2(N, X, INCX)` (LAPACK 3.10's `dznrm2.f90`): the 2-norm of the parts by Blue's
/// accumulators (sums of squares of the small, medium and big parts, scaled).
fn dznrm2(n: usize, x: &[C64], off: usize, incx: usize) -> f64 {
    let tsml = f64::from_bits((1023 - 511) << 52);
    let tbig = f64::from_bits((1023 + 486) << 52);
    let ssml = f64::from_bits((1023 + 511) << 52);
    let sbig = f64::from_bits((1023 - 486) << 52);
    if n == 0 {
        return 0.0;
    }
    let mut notbig = true;
    let (mut asml, mut amed, mut abig) = (0.0f64, 0.0f64, 0.0f64);
    let mut acc = |ax: f64, notbig: &mut bool| {
        if ax > tbig {
            let t = ax.sse_mul(sbig);
            abig = abig.sse_add(t.sse_mul(t));
            *notbig = false;
        } else if ax < tsml {
            if *notbig {
                let t = ax.sse_mul(ssml);
                asml = asml.sse_add(t.sse_mul(t));
            }
        } else {
            amed = amed.sse_add(ax.sse_mul(ax));
        }
    };
    for k in 0..n {
        let v = x[off + k * incx];
        acc(v.re.abs(), &mut notbig);
        acc(v.im.abs(), &mut notbig);
    }
    let (scl, sumsq);
    if abig > 0.0 {
        if amed > 0.0 || amed > f64::MAX || amed.is_nan() {
            abig = abig.sse_add(amed.sse_mul(sbig).sse_mul(sbig));
        }
        scl = 1.0 / sbig;
        sumsq = abig;
    } else if asml > 0.0 {
        if amed > 0.0 || amed > f64::MAX || amed.is_nan() {
            let amed = sse_sqrt(amed);
            let asml = sse_sqrt(asml).sse_div(ssml);
            let (ymin, ymax) = if asml > amed {
                (amed, asml)
            } else {
                (asml, amed)
            };
            scl = 1.0;
            let r = ymin.sse_div(ymax);
            sumsq = ymax.sse_mul(ymax).sse_mul(1.0f64.sse_add(r.sse_mul(r)));
        } else {
            scl = 1.0 / ssml;
            sumsq = asml;
        }
    } else {
        scl = 1.0;
        sumsq = amed;
    }
    scl.sse_mul(sse_sqrt(sumsq))
}

/// gfortran's `MAX(a, b)` (GCC's `MAX_EXPR`, `maxsd`: `b` unless `a > b`).
#[inline(always)]
fn fmax(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// `DLAPY3(X, Y, Z)`: `sqrt(x^2 + y^2 + z^2)` scaled by the largest magnitude.
fn dlapy3(x: f64, y: f64, z: f64) -> f64 {
    let (xa, ya, za) = (x.abs(), y.abs(), z.abs());
    let w = fmax(fmax(xa, ya), za);
    if w == 0.0 {
        return xa.sse_add(ya).sse_add(za);
    }
    let (p, q, r) = (xa.sse_div(w), ya.sse_div(w), za.sse_div(w));
    w.sse_mul(sse_sqrt(
        p.sse_mul(p).sse_add(q.sse_mul(q)).sse_add(r.sse_mul(r)),
    ))
}

/// `DLADIV(A, B, C, D, P, Q)` (LAPACK 3.10, Baudin and Smith's robust division): `p + iq = (a +
/// ib) / (c + id)`.
fn dladiv(a: f64, b: f64, c: f64, d: f64) -> (f64, f64) {
    let (mut aa, mut bb, mut cc, mut dd) = (a, b, c, d);
    let ab = fmax(a.abs(), b.abs());
    let cd = fmax(c.abs(), d.abs());
    let mut s = 1.0f64;
    let ov = f64::MAX;
    let un = SAFMIN;
    let eps = EPS;
    let be = 2.0 / (eps * eps);
    if ab >= 0.5 * ov {
        aa *= 0.5;
        bb *= 0.5;
        s *= 2.0;
    }
    if cd >= 0.5 * ov {
        cc *= 0.5;
        dd *= 0.5;
        s *= 0.5;
    }
    if ab <= un * 2.0 / eps {
        aa = aa.sse_mul(be);
        bb = bb.sse_mul(be);
        s /= be;
    }
    if cd <= un * 2.0 / eps {
        cc = cc.sse_mul(be);
        dd = dd.sse_mul(be);
        s *= be;
    }
    let (p, q);
    if d.abs() <= c.abs() {
        let (pp, qq) = dladiv1(aa, bb, cc, dd);
        p = pp;
        q = qq;
    } else {
        let (pp, qq) = dladiv1(bb, aa, dd, cc);
        p = pp;
        q = -qq;
    }
    (p.sse_mul(s), q.sse_mul(s))
}

fn dladiv1(a: f64, b: f64, c: f64, d: f64) -> (f64, f64) {
    let r = d.sse_div(c);
    let t = 1.0f64.sse_div(c.sse_add(d.sse_mul(r)));
    let p = dladiv2(a, b, c, d, r, t);
    let q = dladiv2(b, -a, c, d, r, t);
    (p, q)
}

fn dladiv2(a: f64, b: f64, c: f64, d: f64, r: f64, t: f64) -> f64 {
    if r != 0.0 {
        let br = b.sse_mul(r);
        if br != 0.0 {
            a.sse_add(br).sse_mul(t)
        } else {
            a.sse_mul(t).sse_add(b.sse_mul(t).sse_mul(r))
        }
    } else {
        a.sse_add(d.sse_mul(b.sse_div(c))).sse_mul(t)
    }
}

/// `ZLADIV(X, Y)`: `X / Y` by [`dladiv`].
fn zladiv(x: C64, y: C64) -> C64 {
    let (p, q) = dladiv(x.re, x.im, y.re, y.im);
    C64::new(p, q)
}

/// `ZSCAL(N, ZA, X, INCX)`: `x(i) = za x(i)`.
fn zscal(n: usize, za: C64, x: &mut [C64], off: usize, incx: usize) {
    for k in 0..n {
        let i = off + k * incx;
        x[i] = za.mul(x[i]);
    }
}

/// `ZDSCAL(N, DA, X, INCX)`: `x(i) = DCMPLX(da, 0) x(i)`.
fn zdscal(n: usize, da: f64, x: &mut [C64], off: usize, incx: usize) {
    zscal(n, C64::real(da), x, off, incx);
}

/// `ZLARFG(N, ALPHA, X, INCX, TAU)`: the elementary reflector `H = I - tau v v^H` with `H^H (alpha,
/// x) = (beta, 0)`, `beta` real; `x` overwritten by `v(2:n)`, `alpha` by `beta`.
fn zlarfg(n: usize, alpha: &mut C64, x: &mut [C64], off: usize, incx: usize) -> C64 {
    if n == 0 {
        return C64::ZERO;
    }
    let mut xnorm = dznrm2(n - 1, x, off, incx);
    let mut alphr = alpha.re;
    let mut alphi = alpha.im;
    if xnorm == 0.0 && alphi == 0.0 {
        return C64::ZERO;
    }
    let sign = |a: f64, b: f64| a.abs().copysign(b);
    let mut beta = -sign(dlapy3(alphr, alphi, xnorm), alphr);
    let safmin = SAFMIN / EPS;
    let rsafmn = 1.0 / safmin;
    let mut knt = 0;
    if beta.abs() < safmin {
        loop {
            knt += 1;
            zdscal(n - 1, rsafmn, x, off, incx);
            beta = beta.sse_mul(rsafmn);
            alphi = alphi.sse_mul(rsafmn);
            alphr = alphr.sse_mul(rsafmn);
            if !(beta.abs() < safmin && knt < 20) {
                break;
            }
        }
        xnorm = dznrm2(n - 1, x, off, incx);
        *alpha = C64::new(alphr, alphi);
        beta = -sign(dlapy3(alphr, alphi, xnorm), alphr);
    }
    let tau = C64::new(beta.sse_sub(alphr).sse_div(beta), -(alphi.sse_div(beta)));
    let a = zladiv(C64::ONE, alpha.sub(C64::real(beta)));
    zscal(n - 1, a, x, off, incx);
    for _ in 0..knt {
        beta = beta.sse_mul(safmin);
    }
    *alpha = C64::real(beta);
    tau
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// `ILAZLC(M, N, A, LDA)`: the last non-zero column (0 for none).
fn ilazlc(m: usize, n: usize, a: &[C64], off: usize, lda: usize) -> usize {
    let at = |i: usize, j: usize| a[off + (i - 1) + (j - 1) * lda];
    if n == 0 {
        return n;
    }
    if !at(1, n).is_zero() || !at(m, n).is_zero() {
        return n;
    }
    for c in (1..=n).rev() {
        if (1..=m).any(|i| !at(i, c).is_zero()) {
            return c;
        }
    }
    0
}

/// `ILAZLR(M, N, A, LDA)`: the last non-zero row (0 for none).
fn ilazlr(m: usize, n: usize, a: &[C64], off: usize, lda: usize) -> usize {
    let at = |i: usize, j: usize| a[off + (i - 1) + (j - 1) * lda];
    if m == 0 {
        return m;
    }
    if !at(m, 1).is_zero() || !at(m, n).is_zero() {
        return m;
    }
    let mut r = 0;
    for j in 1..=n {
        let mut i = m;
        while i >= 1 && at(i.max(1), j).is_zero() {
            i -= 1;
        }
        r = r.max(i);
    }
    r
}

/// `ZLARF(SIDE, M, N, V, 1, TAU, C, LDC, WORK)`: `C = H C` (left) or `C H` (right), `H = I - tau v
/// v^H`, `C` the `m x n` block at `off`.
#[allow(clippy::too_many_arguments)]
fn zlarf(
    side: Side,
    m: usize,
    n: usize,
    v: &[C64],
    tau: C64,
    c: &mut [C64],
    off: usize,
    ldc: usize,
    work: &mut [C64],
) {
    let mut lastv = 0usize;
    let mut lastc = 0usize;
    if !tau.is_zero() {
        lastv = if side == Side::Left { m } else { n };
        while lastv > 0 && v[lastv - 1].is_zero() {
            lastv -= 1;
        }
        lastc = if side == Side::Left {
            ilazlc(lastv, n, c, off, ldc)
        } else {
            ilazlr(m, lastv, c, off, ldc)
        };
    }
    if lastv == 0 {
        return;
    }
    let at = |i: usize, j: usize| off + (i - 1) + (j - 1) * ldc;
    let mtau = tau.neg();
    if side == Side::Left {
        // ZGEMV('C', LASTV, LASTC, ONE, C, LDC, V, 1, ZERO, WORK, 1)
        if lastc == 0 {
            return;
        }
        for (j, wj) in work.iter_mut().enumerate().take(lastc) {
            let mut temp = C64::ZERO;
            for i in 1..=lastv {
                temp = temp.add(c[at(i, j + 1)].conj().mul(v[i - 1]));
            }
            *wj = C64::ZERO.add(C64::ONE.mul(temp));
        }
        // ZGERC(LASTV, LASTC, -TAU, V, 1, WORK, 1, C, LDC)
        if mtau.is_zero() {
            return;
        }
        for j in 1..=lastc {
            let y = work[j - 1];
            if !y.is_zero() {
                let temp = mtau.mul(y.conj());
                for i in 1..=lastv {
                    let k = at(i, j);
                    c[k] = c[k].add(v[i - 1].mul(temp));
                }
            }
        }
    } else {
        // ZGEMV('N', LASTC, LASTV, ONE, C, LDC, V, 1, ZERO, WORK, 1)
        if lastc == 0 {
            return;
        }
        for w in work.iter_mut().take(lastc) {
            *w = C64::ZERO;
        }
        for j in 1..=lastv {
            let temp = C64::ONE.mul(v[j - 1]);
            for i in 1..=lastc {
                work[i - 1] = work[i - 1].add(temp.mul(c[at(i, j)]));
            }
        }
        // ZGERC(LASTC, LASTV, -TAU, WORK, 1, V, 1, C, LDC)
        if mtau.is_zero() {
            return;
        }
        for j in 1..=lastv {
            let y = v[j - 1];
            if !y.is_zero() {
                let temp = mtau.mul(y.conj());
                for i in 1..=lastc {
                    let k = at(i, j);
                    c[k] = c[k].add(work[i - 1].mul(temp));
                }
            }
        }
    }
}

/// `ZLACPY('L', M, N, A, LDA, B, LDB)`: the lower triangle (diagonal included).
fn zlacpy_l(m: usize, n: usize, a: &[C64], lda: usize, b: &mut [C64], ldb: usize) {
    for j in 1..=n {
        for i in j..=m {
            b[ix(i, j, ldb)] = a[ix(i, j, lda)];
        }
    }
}

/// `ZUNGHR(N, ILO, IHI, A, LDA, TAU, WORK, LWORK, INFO)`: the unitary matrix of `ZGEHRD`'s
/// reflectors, through `ZUNGQR`'s unblocked path (`ZUNG2R`).
fn zunghr(
    n: usize,
    ilo: usize,
    ihi: usize,
    a: &mut [C64],
    lda: usize,
    tau: &[C64],
    work: &mut [C64],
) -> Result<(), Zgees> {
    let nh = ihi - ilo;
    if n == 0 {
        return Ok(());
    }
    // shift the reflectors one column to the right; the first ILO and last N - IHI rows and
    // columns those of the unit matrix
    for j in (ilo + 1..=ihi).rev() {
        for i in 1..j {
            a[ix(i, j, lda)] = C64::ZERO;
        }
        for i in j + 1..=ihi {
            a[ix(i, j, lda)] = a[ix(i, j - 1, lda)];
        }
        for i in ihi + 1..=n {
            a[ix(i, j, lda)] = C64::ZERO;
        }
    }
    for j in 1..=ilo {
        for i in 1..=n {
            a[ix(i, j, lda)] = C64::ZERO;
        }
        a[ix(j, j, lda)] = C64::ONE;
    }
    for j in ihi + 1..=n {
        for i in 1..=n {
            a[ix(i, j, lda)] = C64::ZERO;
        }
        a[ix(j, j, lda)] = C64::ONE;
    }
    if nh > 0 {
        // ZUNGQR(NH, NH, NH, A(ILO+1, ILO+1), LDA, TAU(ILO), ...): blocked when NH > 128
        if nh > 128 {
            return Err(Zgees::NotPorted(
                "ZUNGQR's blocked path (more than 128 reflectors)",
            ));
        }
        zung2r(
            nh,
            nh,
            nh,
            a,
            ix(ilo + 1, ilo + 1, lda),
            lda,
            &tau[ilo - 1..],
            work,
        );
    }
    Ok(())
}

/// `ZUNG2R(M, N, K, A, LDA, TAU, WORK, INFO)` on the block at `off`.
#[allow(clippy::too_many_arguments)]
fn zung2r(
    m: usize,
    n: usize,
    k: usize,
    a: &mut [C64],
    off: usize,
    lda: usize,
    tau: &[C64],
    work: &mut [C64],
) {
    let at = |i: usize, j: usize| off + (i - 1) + (j - 1) * lda;
    if n == 0 {
        return;
    }
    for j in k + 1..=n {
        for l in 1..=m {
            a[at(l, j)] = C64::ZERO;
        }
        a[at(j, j)] = C64::ONE;
    }
    for i in (1..=k).rev() {
        if i < n {
            a[at(i, i)] = C64::ONE;
            let v: Vec<C64> = (i..=m).map(|r| a[at(r, i)]).collect();
            zlarf(
                Side::Left,
                m - i + 1,
                n - i,
                &v,
                tau[i - 1],
                a,
                at(i, i + 1),
                lda,
                work,
            );
        }
        if i < m {
            zscal(m - i, tau[i - 1].neg(), a, at(i + 1, i), 1);
        }
        a[at(i, i)] = C64::ONE.sub(tau[i - 1]);
        for l in 1..i {
            a[at(l, i)] = C64::ZERO;
        }
    }
}

/// `ZHSEQR('S', 'V', N, ILO, IHI, H, LDH, W, Z, LDZ, WORK, LWORK, INFO)` for `N <= 75`: the
/// Schur form of the Hessenberg matrix `H` by `ZLAHQR`, `Z` updated; `INFO` from `ZLAHQR`.
#[allow(clippy::too_many_arguments)]
fn zhseqr_sv(
    n: usize,
    ilo: usize,
    ihi: usize,
    h: &mut [C64],
    ldh: usize,
    w: &mut [C64],
    z: &mut [C64],
    ldz: usize,
) -> Result<i32, Zgees> {
    if n == 0 {
        return Ok(0);
    }
    for i in 1..ilo {
        w[i - 1] = h[ix(i, i, ldh)];
    }
    for i in ihi + 1..=n {
        w[i - 1] = h[ix(i, i, ldh)];
    }
    if ilo == ihi {
        w[ilo - 1] = h[ix(ilo, ilo, ldh)];
        return Ok(0);
    }
    // NMIN = MAX(NTINY = 15, ILAENV(12, 'ZHSEQR', ...) = IPARMQ's 75)
    if n > 75 {
        return Err(Zgees::NotPorted("ZLAQR0 (ZHSEQR above 75 rows)"));
    }
    let info = zlahqr(n, ilo, ihi, h, ldh, w, ilo, ihi, z, ldz);
    if info > 0 {
        return Err(Zgees::NotPorted(
            "ZLAQR0 (ZHSEQR's second attempt where ZLAHQR does not converge)",
        ));
    }
    if n > 2 {
        // ZLASET('L', N-2, N-2, ZERO, ZERO, H(3, 1), LDH)
        for j in 1..=n - 2 {
            for i in j..=n - 2 {
                h[ix(i + 2, j, ldh)] = C64::ZERO;
            }
        }
    }
    Ok(info)
}

/// `CABS1(z) = |re| + |im|`.
#[inline(always)]
fn cabs1(z: C64) -> f64 {
    z.re.abs().sse_add(z.im.abs())
}

/// `ZLAHQR(.TRUE., .TRUE., N, ILO, IHI, H, LDH, W, ILOZ, IHIZ, Z, LDZ, INFO)`: the double-shift
/// (single complex shift) QR iteration of LAPACK 3.10 on the Hessenberg block `ILO..IHI` of `H`,
/// the whole matrix updated (`WANTT`) and the rows `ILOZ..IHIZ` of `Z` (`WANTZ`). Returns `INFO`:
/// 0, or the row at which the iteration stopped without converging.
#[allow(clippy::too_many_arguments)]
fn zlahqr(
    n: usize,
    ilo: usize,
    ihi: usize,
    h: &mut [C64],
    ldh: usize,
    w: &mut [C64],
    iloz: usize,
    ihiz: usize,
    z: &mut [C64],
    ldz: usize,
) -> i32 {
    const KEXSH: usize = 10;
    let dat1 = 3.0f64 / 4.0;
    let hx = |i: usize, j: usize| ix(i, j, ldh);
    let zx = |i: usize, j: usize| ix(i, j, ldz);
    if n == 0 {
        return 0;
    }
    if ilo == ihi {
        w[ilo - 1] = h[hx(ilo, ilo)];
        return 0;
    }
    // clear out the trash
    for j in ilo..=ihi.saturating_sub(3) {
        h[hx(j + 2, j)] = C64::ZERO;
        h[hx(j + 3, j)] = C64::ZERO;
    }
    if ilo + 2 <= ihi {
        h[hx(ihi, ihi - 2)] = C64::ZERO;
    }
    let (jlo, jhi) = (1usize, n);
    // ensure that the subdiagonal entries are real
    for i in ilo + 1..=ihi {
        if h[hx(i, i - 1)].im != 0.0 {
            let hi = h[hx(i, i - 1)];
            let mut sc = hi.fdiv_real(cabs1(hi));
            sc = sc.conj().fdiv_real(cabs(sc));
            h[hx(i, i - 1)] = C64::real(cabs(hi));
            zscal(jhi - i + 1, sc, h, hx(i, i), ldh);
            zscal((jhi.min(i + 1)) - jlo + 1, sc.conj(), h, hx(jlo, i), 1);
            zscal(ihiz - iloz + 1, sc.conj(), z, zx(iloz, i), 1);
        }
    }
    let nh = ihi - ilo + 1;
    let nz = ihiz - iloz + 1;
    let safmin = SAFMIN;
    let ulp = PREC;
    let smlnum = safmin.sse_mul((nh as f64).sse_div(ulp));
    let (i1, i2) = (1usize, n);
    let itmax = 30 * nh.max(10);
    let mut kdefl = 0usize;
    let mut i = ihi;
    'outer: loop {
        if i < ilo {
            return 0;
        }
        let mut l = ilo;
        let mut converged = false;
        for _its in 0..=itmax {
            // look for a single small subdiagonal element
            let mut k = i;
            while k > l {
                if cabs1(h[hx(k, k - 1)]) <= smlnum {
                    break;
                }
                let mut tst = cabs1(h[hx(k - 1, k - 1)]).sse_add(cabs1(h[hx(k, k)]));
                if tst == 0.0 {
                    if k >= ilo + 2 {
                        tst = tst.sse_add(h[hx(k - 1, k - 2)].re.abs());
                    }
                    if k < ihi {
                        tst = tst.sse_add(h[hx(k + 1, k)].re.abs());
                    }
                }
                if h[hx(k, k - 1)].re.abs() <= ulp.sse_mul(tst) {
                    let (c1, c2) = (cabs1(h[hx(k, k - 1)]), cabs1(h[hx(k - 1, k)]));
                    let ab = fmax(c1, c2);
                    let ba = if c1 < c2 { c1 } else { c2 };
                    let c3 = cabs1(h[hx(k, k)]);
                    let c4 = cabs1(h[hx(k - 1, k - 1)].sub(h[hx(k, k)]));
                    let aa = fmax(c3, c4);
                    let bb = if c3 < c4 { c3 } else { c4 };
                    let s = aa.sse_add(ab);
                    let lhs = ba.sse_mul(ab.sse_div(s));
                    let rhs = fmax(smlnum, ulp.sse_mul(bb.sse_mul(aa.sse_div(s))));
                    if lhs <= rhs {
                        break;
                    }
                }
                k -= 1;
            }
            l = k;
            if l > ilo {
                // H(L, L-1) is negligible
                h[hx(l, l - 1)] = C64::ZERO;
            }
            // exit from the loop if a submatrix of order 1 has split off
            if l >= i {
                converged = true;
                break;
            }
            kdefl += 1;
            // the active submatrix is rows and columns L to I (WANTT: I1 = 1, I2 = N)
            let mut t;
            if kdefl.is_multiple_of(2 * KEXSH) {
                // exceptional shift
                let s = dat1.sse_mul(h[hx(i, i - 1)].re.abs());
                t = C64::real(s).add(h[hx(i, i)]);
            } else if kdefl.is_multiple_of(KEXSH) {
                let s = dat1.sse_mul(h[hx(l + 1, l)].re.abs());
                t = C64::real(s).add(h[hx(l, l)]);
            } else {
                // Wilkinson's shift
                t = h[hx(i, i)];
                let u = csqrt(h[hx(i - 1, i)]).mul(csqrt(h[hx(i, i - 1)]));
                let mut s = cabs1(u);
                if s != 0.0 {
                    let x = C64::real(0.5).mul(h[hx(i - 1, i - 1)].sub(t));
                    let sx = cabs1(x);
                    s = fmax(s, cabs1(x));
                    let xs = x.fdiv_real(s);
                    let us = u.fdiv_real(s);
                    let mut y = C64::real(s).mul(csqrt(xs.mul(xs).add(us.mul(us))));
                    if sx > 0.0 {
                        let q = x.fdiv_real(sx);
                        if q.re.sse_mul(y.re).sse_add(q.im.sse_mul(y.im)) < 0.0 {
                            y = y.neg();
                        }
                    }
                    t = t.sub(u.mul(zladiv(u, x.add(y))));
                }
            }
            // look for two consecutive small subdiagonal elements
            let mut m = i - 1;
            let mut v = [C64::ZERO; 2];
            let mut found = false;
            while m > l {
                let h11 = h[hx(m, m)];
                let h22 = h[hx(m + 1, m + 1)];
                let mut h11s = h11.sub(t);
                let mut h21 = h[hx(m + 1, m)].re;
                let s = cabs1(h11s).sse_add(h21.abs());
                h11s = h11s.fdiv_real(s);
                h21 = h21.sse_div(s);
                v[0] = h11s;
                v[1] = C64::real(h21);
                let h10 = h[hx(m, m - 1)].re;
                if h10.abs().sse_mul(h21.abs())
                    <= ulp.sse_mul(cabs1(h11s).sse_mul(cabs1(h11).sse_add(cabs1(h22))))
                {
                    found = true;
                    break;
                }
                m -= 1;
            }
            if !found {
                let h11 = h[hx(l, l)];
                let mut h11s = h11.sub(t);
                let mut h21 = h[hx(l + 1, l)].re;
                let s = cabs1(h11s).sse_add(h21.abs());
                h11s = h11s.fdiv_real(s);
                h21 = h21.sse_div(s);
                v[0] = h11s;
                v[1] = C64::real(h21);
                m = l;
            }
            // single-shift QR step
            for k in m..i {
                if k > m {
                    v[0] = h[hx(k, k - 1)];
                    v[1] = h[hx(k + 1, k - 1)];
                }
                let (va, vb) = v.split_at_mut(1);
                let t1 = zlarfg(2, &mut va[0], vb, 0, 1);
                if k > m {
                    h[hx(k, k - 1)] = v[0];
                    h[hx(k + 1, k - 1)] = C64::ZERO;
                }
                let v2 = v[1];
                let t2 = t1.mul(v2).re;
                let ct1 = t1.conj();
                for j in k..=i2 {
                    let sum = ct1.mul(h[hx(k, j)]).add(C64::real(t2).mul(h[hx(k + 1, j)]));
                    h[hx(k, j)] = h[hx(k, j)].sub(sum);
                    h[hx(k + 1, j)] = h[hx(k + 1, j)].sub(sum.mul(v2));
                }
                let cv2 = v2.conj();
                for j in i1..=(k + 2).min(i) {
                    let sum = t1.mul(h[hx(j, k)]).add(C64::real(t2).mul(h[hx(j, k + 1)]));
                    h[hx(j, k)] = h[hx(j, k)].sub(sum);
                    h[hx(j, k + 1)] = h[hx(j, k + 1)].sub(sum.mul(cv2));
                }
                for j in iloz..=ihiz {
                    let sum = t1.mul(z[zx(j, k)]).add(C64::real(t2).mul(z[zx(j, k + 1)]));
                    z[zx(j, k)] = z[zx(j, k)].sub(sum);
                    z[zx(j, k + 1)] = z[zx(j, k + 1)].sub(sum.mul(cv2));
                }
                if k == m && m > l {
                    // extra scaling of the sub-diagonal entries of the bulge
                    let mut temp = C64::ONE.sub(t1);
                    temp = temp.fdiv_real(cabs(temp));
                    h[hx(m + 1, m)] = h[hx(m + 1, m)].mul(temp.conj());
                    if m + 2 <= i {
                        h[hx(m + 2, m + 1)] = h[hx(m + 2, m + 1)].mul(temp);
                    }
                    for j in m..=i {
                        if j != m + 1 {
                            if i2 > j {
                                zscal(i2 - j, temp, h, hx(j, j + 1), ldh);
                            }
                            zscal(j - i1, temp.conj(), h, hx(i1, j), 1);
                            zscal(nz, temp.conj(), z, zx(iloz, j), 1);
                        }
                    }
                }
            }
            // ensure that H(I, I-1) is real
            let mut temp = h[hx(i, i - 1)];
            if temp.im != 0.0 {
                let rtemp = cabs(temp);
                h[hx(i, i - 1)] = C64::real(rtemp);
                temp = temp.fdiv_real(rtemp);
                if i2 > i {
                    zscal(i2 - i, temp.conj(), h, hx(i, i + 1), ldh);
                }
                zscal(i - i1, temp, h, hx(i1, i), 1);
                zscal(nz, temp, z, zx(iloz, i), 1);
            }
        }
        if !converged {
            return i as i32;
        }
        // H(I, I-1) is negligible: one eigenvalue has converged
        w[i - 1] = h[hx(i, i)];
        kdefl = 0;
        if l == 0 {
            return 0;
        }
        i = l - 1;
        continue 'outer;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `A = VS T VS^H` with `T` upper triangular, for a small real matrix with complex
    /// eigenvalues.
    #[test]
    fn schur_form_reproduces_the_matrix() {
        let n = 4;
        let rows = [
            [0.9, -0.3, 0.1, 1.5],
            [0.3, 0.9, 0.2, -2.0],
            [0.05, 0.1, 1.1, 0.25],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let mut a = vec![C64::ZERO; n * n];
        for i in 0..n {
            for j in 0..n {
                a[i + j * n] = C64::real(rows[i][j]);
            }
        }
        let mut w = vec![C64::ZERO; n];
        let mut vs = vec![C64::ZERO; n * n];
        assert_eq!(zgees_vn(n, &mut a, &mut w, &mut vs), Zgees::Done(0));
        for i in 0..n {
            for j in 0..i {
                assert!(a[i + j * n].is_zero(), "T({i}, {j})");
            }
        }
        for i in 0..n {
            for j in 0..n {
                let mut s = C64::ZERO;
                for k in 0..n {
                    for l in 0..n {
                        s = s.add(vs[i + k * n].mul(a[k + l * n]).mul(vs[j + l * n].conj()));
                    }
                }
                assert!(
                    (s.re - rows[i][j]).abs() < 1e-12 && s.im.abs() < 1e-12,
                    "{i} {j}"
                );
            }
        }
        assert_eq!(
            zgees_vn(0, &mut [], &mut [], &mut []),
            Zgees::Xerbla("ZGEES", 6)
        );
        assert_eq!(
            xerbla_line("ZGEES", 6),
            " ** On entry to ZGEES parameter number  6 had an illegal value\n"
        );
    }
}
