// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! `vnl_svd<std::complex<double>>`, as ITK 4.13.2's VNL computes it: LINPACK's complex singular
//! value decomposition `zsvdc` as translated by f2c in VXL's `v3p/netlib` (with the reference
//! BLAS kernels it calls, `zscal`, `zaxpy`, `zdotc`, `zdrot`, `zswap`, `dznrm2`, `drotg`, and
//! libf2c's `z_abs` and `z_div`), then the decomposition `vnl_svd` keeps (`U`, `W` = `|s|`, `V`)
//! and its two solvers, [`VnlCSvd::solve_matrix`] and [`VnlCSvd::solve_vector`]. The replicas' complex
//! least-squares systems are solved with them.
//!
//! The f2c translations are followed statement by statement, in their order and with their
//! 1-based indices: LINPACK's complex arithmetic is written out part by part in C (no runtime
//! call), libf2c's `abs` is `x >= 0 ? x : -x`, `f__cabs` (`z_abs`) scales by the larger part and
//! `z_div` is Smith's division (a zero divisor ends the program: `complex division by zero`, then
//! `abort`). `vnl_svd`'s own complex arithmetic is C++'s (`crate::cplx`: `__divdc3`, `__muldc3`
//! where both parts of a product are NaN; `std::abs` is `cabs`, the decided correctly rounded
//! `hypot`).
//!
//! Ported from VXL as bundled with ITK v4.13.2 (BSD licence; LINPACK, BLAS and libf2c from
//! netlib): `core/vnl/algo/vnl_svd.hxx`, `v3p/netlib/linpack/zsvdc.c`,
//! `v3p/netlib/blas/{zscal,zaxpy,zdotc,zdrot,zswap,dznrm2,drotg,dcabs1}.c`,
//! `v3p/netlib/libf2c/{z_abs,z_div,cabs,d_cnjg,sig_die}.c`.

// Original copyright and licence (BSD-3-Clause) notice of VXL
// (Modules/ThirdParty/VNL/src/vxl/core/vxl_copyright.h in ITK v4.13.2):
//
//   Copyright 2000-2013 VXL Contributors
//   All rights reserved.
//
//   Redistribution and use in source and binary forms, with or without
//   modification, are permitted provided that the following conditions
//   are met:
//
//   * Redistributions of source code must retain the above copyright
//     notice, this list of conditions and the following disclaimer.
//
//   * Redistributions in binary form must reproduce the above copyright
//     notice, this list of conditions and the following disclaimer in the
//     documentation and/or other materials provided with the distribution.
//
//   * Neither the names of the copyright holders nor the names of their
//     contributors may be used to endorse or promote products derived
//     from this software without specific prior written permission.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
//   "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
//   LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
//   FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
//   COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
//   INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
//   (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//   SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
//   HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
//   STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
//   OF THE POSSIBILITY OF SUCH DAMAGE.
//
// LINPACK (zsvdc), the reference BLAS and libf2c are netlib software, distributed by VXL as above.

// The loops keep LINPACK's 1-based Fortran indices, so the port can be checked line by line
// against zsvdc.c.
#![allow(clippy::needless_range_loop, clippy::too_many_lines)]

use crate::cplx::{C64, cabs, cxx_div, cxx_mul};
use crate::vnl_matrix::VnlCMatrix;

/// libf2c's `abs(x)`: `x >= 0 ? x : -x` (a NaN's sign flipped, `-0` kept).
#[inline(always)]
fn fabs(x: f64) -> f64 {
    if x >= 0.0 { x } else { -x }
}

/// libf2c's `f__cabs(real, imag)` (`z_abs`): both made non-negative, the larger first; the
/// larger alone when adding the smaller does not change it, otherwise `real sqrt(1 + (imag /
/// real)^2)`.
pub fn f2c_cabs(re: f64, im: f64) -> f64 {
    let (mut real, mut imag) = (re, im);
    if real < 0.0 {
        real = -real;
    }
    if imag < 0.0 {
        imag = -imag;
    }
    if imag > real {
        std::mem::swap(&mut real, &mut imag);
    }
    if real + imag == real {
        return real;
    }
    let temp = imag / real;
    real * (1.0 + temp * temp).sqrt()
}

/// libf2c's `z_div` dividing by zero: it prints `complex division by zero` and aborts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DivisionByZero;

/// libf2c's `z_div(c, a, b)` (`IEEE_COMPLEX_DIVIDE` not defined): Smith's division on `|br| <=
/// |bi|` (libf2c's `abs`); a zero divisor (`|br| <= |bi| = 0`) is [`DivisionByZero`].
pub fn z_div(a: C64, b: C64) -> Result<C64, DivisionByZero> {
    let mut abr = b.re;
    if abr < 0.0 {
        abr = -abr;
    }
    let mut abi = b.im;
    if abi < 0.0 {
        abi = -abi;
    }
    if abr <= abi {
        if abi == 0.0 {
            return Err(DivisionByZero);
        }
        let ratio = b.re / b.im;
        let den = b.im * (1.0 + ratio * ratio);
        let cr = (a.re * ratio + a.im) / den;
        let ci = (a.im * ratio - a.re) / den;
        Ok(C64::new(cr, ci))
    } else {
        let ratio = b.im / b.re;
        let den = b.re * (1.0 + ratio * ratio);
        let cr = (a.re + a.im * ratio) / den;
        let ci = (a.im - a.re * ratio) / den;
        Ok(C64::new(cr, ci))
    }
}

/// `d_cnjg`: `(re, -im)`.
#[inline(always)]
fn cnjg(z: C64) -> C64 {
    C64::new(z.re, -z.im)
}

/// LINPACK's complex product as f2c writes it: `(ar br - ai bi, ar bi + ai br)`.
#[inline(always)]
fn zmul(a: C64, b: C64) -> C64 {
    C64::new(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
}

/// zsvdc's test of a zero element (`dabs(dreal(z)) + dabs(dimag(z)) == 0`, `dimag` as the
/// real part of `z (0, -1)`): true exactly when both parts are zero.
#[inline(always)]
fn is_zero(z: C64) -> bool {
    z.re == 0.0 && z.im == 0.0
}

/// VXL's `dznrm2` (the 1982 version: one scaled sum of squares over both parts, real part first).
fn dznrm2(x: &[C64]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    let (mut scale, mut ssq) = (0.0f64, 1.0f64);
    let mut add = |v: f64| {
        if v != 0.0 {
            let temp = fabs(v);
            if scale < temp {
                let d = scale / temp;
                ssq = ssq * (d * d) + 1.0;
                scale = temp;
            } else {
                let d = temp / scale;
                ssq += d * d;
            }
        }
    };
    for z in x {
        add(z.re);
        add(z.im);
    }
    scale * ssq.sqrt()
}

/// `zscal`: `x(i) = za x(i)`.
fn zscal(za: C64, x: &mut [C64]) {
    for xi in x {
        *xi = zmul(za, *xi);
    }
}

/// `zaxpy`: `y(i) = y(i) + za x(i)`; nothing when `|re(za)| + |im(za)| == 0` (`dcabs1`).
fn zaxpy(za: C64, x: &[C64], y: &mut [C64]) {
    if x.is_empty() || fabs(za.re) + fabs(za.im) == 0.0 {
        return;
    }
    for (yi, &xi) in y.iter_mut().zip(x) {
        let p = zmul(za, xi);
        *yi = C64::new(yi.re + p.re, yi.im + p.im);
    }
}

/// `zdotc`: `sum conj(x(i)) y(i)` from `(0, 0)`.
fn zdotc(x: &[C64], y: &[C64]) -> C64 {
    let mut t = C64::new(0.0, 0.0);
    for (&xi, &yi) in x.iter().zip(y) {
        let p = zmul(cnjg(xi), yi);
        t = C64::new(t.re + p.re, t.im + p.im);
    }
    t
}

/// `zdrot`: the plane rotation `(c x + s y, c y - s x)` with real `c` and `s`.
fn zdrot(x: &mut [C64], y: &mut [C64], c: f64, s: f64) {
    for (xi, yi) in x.iter_mut().zip(y.iter_mut()) {
        let t = C64::new(c * xi.re + s * yi.re, c * xi.im + s * yi.im);
        *yi = C64::new(c * yi.re - s * xi.re, c * yi.im - s * xi.im);
        *xi = t;
    }
}

/// `drotg(da, db, c, s)`: returns `(da, db, c, s)` as it leaves them (`r`, `z`, `c`, `s`).
fn drotg(a: f64, b: f64) -> (f64, f64, f64, f64) {
    let roe = if fabs(a) > fabs(b) { a } else { b };
    let scale = fabs(a) + fabs(b);
    if scale == 0.0 {
        return (0.0, 0.0, 1.0, 0.0);
    }
    let (d1, d2) = (a / scale, b / scale);
    let mut r = scale * (d1 * d1 + d2 * d2).sqrt();
    r *= if roe >= 0.0 { 1.0 } else { -1.0 };
    let c = a / r;
    let s = b / r;
    let mut z = 1.0;
    if fabs(a) > fabs(b) {
        z = s;
    }
    if fabs(b) >= fabs(a) && c != 0.0 {
        z = 1.0 / c;
    }
    (r, z, c, s)
}

/// A column-major complex matrix with LINPACK's 1-based indexing.
struct F {
    d: Vec<C64>,
    ld: usize,
}

impl F {
    fn new(rows: usize, cols: usize) -> F {
        F {
            d: vec![C64::new(0.0, 0.0); rows * cols],
            ld: rows,
        }
    }

    #[inline(always)]
    fn k(&self, i: usize, j: usize) -> usize {
        (i - 1) + (j - 1) * self.ld
    }

    #[inline(always)]
    fn at(&self, i: usize, j: usize) -> C64 {
        self.d[self.k(i, j)]
    }

    #[inline(always)]
    fn set(&mut self, i: usize, j: usize, v: C64) {
        let k = self.k(i, j);
        self.d[k] = v;
    }

    /// Rows `i..i + len` of column `j`.
    fn col(&self, i: usize, j: usize, len: usize) -> &[C64] {
        let k = self.k(i, j);
        &self.d[k..k + len]
    }

    fn col_mut(&mut self, i: usize, j: usize, len: usize) -> &mut [C64] {
        let k = self.k(i, j);
        &mut self.d[k..k + len]
    }

    /// Rows `i..i + len` of columns `ja` and `jb` (`ja != jb`), mutable.
    fn two(&mut self, i: usize, ja: usize, jb: usize, len: usize) -> (&mut [C64], &mut [C64]) {
        let (ka, kb) = (self.k(i, ja), self.k(i, jb));
        if ka < kb {
            let (lo, hi) = self.d.split_at_mut(kb);
            (&mut lo[ka..ka + len], &mut hi[..len])
        } else {
            let (lo, hi) = self.d.split_at_mut(ka);
            (&mut hi[..len], &mut lo[kb..kb + len])
        }
    }
}

/// What `zsvdc` (`job = 21`) leaves: `s` (`min(n + 1, p)` complex singular values, 1-based in
/// `s[1..]`), `u` (`n x p`) and `v` (`p x p`), column-major, and `info`; `cycle` when its
/// iteration came back to a state it had been in (an infinite or NaN entry: VNL would never
/// return, ITK-116; `info` is then the singular value it was working on).
struct Zsvdc {
    s: Vec<C64>,
    u: Vec<C64>,
    v: Vec<C64>,
    info: usize,
    cycle: bool,
}

/// LINPACK's `zsvdc(x, n, n, p, s, e, u, n, v, p, work, 21, info)` on `x` (column-major `n x
/// p`, `n >= 1`, `p >= 1`).
fn zsvdc(mut x: F, n: usize, p: usize) -> Result<Zsvdc, DivisionByZero> {
    let mmlen = (n + 1).min(p);
    let mut s = vec![C64::ZERO; mmlen.max(p) + 2];
    let mut e = vec![C64::ZERO; p + 2];
    let mut u = F::new(n, p);
    let mut v = F::new(p, p);
    let mut work = vec![C64::ZERO; n + 1];
    let maxit = 1000;
    // job = 21: jobu = 2, U's min(n, p) columns, V wanted
    let ncu = n.min(p);
    let (wantu, wantv) = (true, true);
    let one = C64::new(1.0, 0.0);
    let mone = C64::new(-1.0, 0.0);
    let nct = (n - 1).min(p);
    let nrt = 0.max((p as isize - 2).min(n as isize)) as usize;
    let lu = nct.max(nrt);
    for l in 1..=lu {
        let lp1 = l + 1;
        if l <= nct {
            s[l] = C64::new(dznrm2(x.col(l, l, n - l + 1)), 0.0);
            if !is_zero(s[l]) {
                if !is_zero(x.at(l, l)) {
                    let d3 = f2c_cabs(s[l].re, s[l].im);
                    let xll = x.at(l, l);
                    let d4 = f2c_cabs(xll.re, xll.im);
                    let z3 = C64::new(xll.re / d4, xll.im / d4);
                    s[l] = C64::new(d3 * z3.re, d3 * z3.im);
                }
                let r = z_div(one, s[l])?;
                zscal(r, x.col_mut(l, l, n - l + 1));
                let xll = x.at(l, l);
                x.set(l, l, C64::new(xll.re + 1.0, xll.im + 0.0));
            }
            s[l] = C64::new(-s[l].re, -s[l].im);
        }
        if p >= lp1 {
            for j in lp1..=p {
                if l <= nct && !is_zero(s[l]) {
                    let z3 = zdotc(x.col(l, l, n - l + 1), x.col(l, j, n - l + 1));
                    let t = z_div(C64::new(-z3.re, -z3.im), x.at(l, l))?;
                    let (xl, xj) = x.two(l, l, j, n - l + 1);
                    zaxpy(t, xl, xj);
                }
                e[j] = cnjg(x.at(l, j));
            }
        }
        if wantu && l <= nct {
            for i in l..=n {
                u.set(i, l, x.at(i, l));
            }
        }
        if l <= nrt {
            e[l] = C64::new(dznrm2(&e[lp1..=p]), 0.0);
            if !is_zero(e[l]) {
                if !is_zero(e[lp1]) {
                    let d3 = f2c_cabs(e[l].re, e[l].im);
                    let d4 = f2c_cabs(e[lp1].re, e[lp1].im);
                    let z3 = C64::new(e[lp1].re / d4, e[lp1].im / d4);
                    e[l] = C64::new(d3 * z3.re, d3 * z3.im);
                }
                let r = z_div(one, e[l])?;
                zscal(r, &mut e[lp1..=p]);
                e[lp1] = C64::new(e[lp1].re + 1.0, e[lp1].im + 0.0);
            }
            let c = cnjg(e[l]);
            e[l] = C64::new(-c.re, -c.im);
            if lp1 <= n && !is_zero(e[l]) {
                for i in lp1..=n {
                    work[i] = C64::ZERO;
                }
                for j in lp1..=p {
                    zaxpy(e[j], x.col(lp1, j, n - l), &mut work[lp1..=n]);
                }
                for j in lp1..=p {
                    let q = z_div(C64::new(-e[j].re, -e[j].im), e[lp1])?;
                    zaxpy(cnjg(q), &work[lp1..=n], x.col_mut(lp1, j, n - l));
                }
            }
            if wantv {
                for i in lp1..=p {
                    v.set(i, l, e[i]);
                }
            }
        }
    }
    // the final bidiagonal matrix of order m
    let mut m = p.min(n + 1);
    let nctp1 = nct + 1;
    let nrtp1 = nrt + 1;
    if nct < p {
        s[nctp1] = x.at(nctp1, nctp1);
    }
    if n < m {
        s[m] = C64::ZERO;
    }
    if nrtp1 < m {
        e[nrtp1] = x.at(nrtp1, m);
    }
    e[m] = C64::ZERO;
    if wantu {
        if ncu >= nctp1 {
            for j in nctp1..=ncu {
                for i in 1..=n {
                    u.set(i, j, C64::ZERO);
                }
                u.set(j, j, one);
            }
        }
        if nct >= 1 {
            for ll in 1..=nct {
                let l = nct - ll + 1;
                if !is_zero(s[l]) {
                    let lp1 = l + 1;
                    if ncu >= lp1 {
                        for j in lp1..=ncu {
                            let z3 = zdotc(u.col(l, l, n - l + 1), u.col(l, j, n - l + 1));
                            let t = z_div(C64::new(-z3.re, -z3.im), u.at(l, l))?;
                            let (ul, uj) = u.two(l, l, j, n - l + 1);
                            zaxpy(t, ul, uj);
                        }
                    }
                    zscal(mone, u.col_mut(l, l, n - l + 1));
                    let ull = u.at(l, l);
                    u.set(l, l, C64::new(ull.re + 1.0, ull.im + 0.0));
                    for i in 1..l {
                        u.set(i, l, C64::ZERO);
                    }
                } else {
                    for i in 1..=n {
                        u.set(i, l, C64::ZERO);
                    }
                    u.set(l, l, one);
                }
            }
        }
    }
    if wantv {
        for ll in 1..=p {
            let l = p - ll + 1;
            let lp1 = l + 1;
            if l <= nrt && !is_zero(e[l]) {
                for j in lp1..=p {
                    let z3 = zdotc(v.col(lp1, l, p - l), v.col(lp1, j, p - l));
                    let t = z_div(C64::new(-z3.re, -z3.im), v.at(lp1, l))?;
                    let (vl, vj) = v.two(lp1, l, j, p - l);
                    zaxpy(t, vl, vj);
                }
            }
            for i in 1..=p {
                v.set(i, l, C64::ZERO);
            }
            v.set(l, l, one);
        }
    }
    // the singular values and the superdiagonal made real
    for i in 1..=m {
        if !is_zero(s[i]) {
            let t = C64::new(f2c_cabs(s[i].re, s[i].im), 0.0);
            let r = z_div(s[i], t)?;
            s[i] = t;
            if i < m {
                e[i] = z_div(e[i], r)?;
            }
            if wantu {
                zscal(r, u.col_mut(1, i, n));
            }
        }
        if i == m {
            break;
        }
        if !is_zero(e[i]) {
            let t = C64::new(f2c_cabs(e[i].re, e[i].im), 0.0);
            let r = z_div(t, e[i])?;
            e[i] = t;
            s[i + 1] = zmul(s[i + 1], r);
            if wantv {
                zscal(r, v.col_mut(1, i + 1, p));
            }
        }
    }
    // the main iteration loop for the singular values (zero real parts: every s and e is real
    // from here on, its imaginary part 0)
    let mm = m;
    let mut iter = 0;
    let mut info = 0usize;
    let mut cycle = false;
    let state = |s: &[C64], e: &[C64], m: usize, iter: usize| -> Vec<u64> {
        let b = |y: f64| if y.is_nan() { u64::MAX } else { y.to_bits() };
        let mut v: Vec<u64> = s
            .iter()
            .chain(e.iter())
            .flat_map(|z| [b(z.re), b(z.im)])
            .collect();
        v.push(m as u64);
        v.push(iter as u64);
        v
    };
    let mut saved = state(&s, &e, m, iter);
    let (mut power, mut steps) = (1u64, 0u64);
    while m != 0 {
        if iter >= maxit {
            info = m;
            break;
        }
        let mut l = 0usize;
        for ll in 1..=m {
            l = m - ll;
            if l == 0 {
                break;
            }
            let test = f2c_cabs(s[l].re, s[l].im) + f2c_cabs(s[l + 1].re, s[l + 1].im);
            let ztest = test + f2c_cabs(e[l].re, e[l].im);
            if ztest == test {
                e[l] = C64::ZERO;
                break;
            }
        }
        let kase;
        if l == m - 1 {
            kase = 4;
        } else {
            let lp1 = l + 1;
            let mp1 = m + 1;
            let mut ls = 0usize;
            for lls in lp1..=mp1 {
                ls = m + lp1 - lls;
                if ls == l {
                    break;
                }
                let mut test = 0.0f64;
                if ls != m {
                    test += f2c_cabs(e[ls].re, e[ls].im);
                }
                if ls != l + 1 {
                    test += f2c_cabs(e[ls - 1].re, e[ls - 1].im);
                }
                let ztest = test + f2c_cabs(s[ls].re, s[ls].im);
                if ztest == test {
                    s[ls] = C64::ZERO;
                    break;
                }
            }
            if ls == l {
                kase = 3;
            } else if ls == m {
                kase = 1;
            } else {
                kase = 2;
                l = ls;
            }
        }
        l += 1;
        match kase {
            1 => {
                // deflate negligible s(m)
                let mm1 = m - 1;
                let mut f = e[m - 1].re;
                e[m - 1] = C64::ZERO;
                for kk in l..=mm1 {
                    let k = mm1 - kk + l;
                    let (t1, z, cs, sn) = drotg(s[k].re, f);
                    f = z;
                    s[k] = C64::new(t1, 0.0);
                    if k != l {
                        f = -sn * e[k - 1].re;
                        e[k - 1] = C64::new(cs * e[k - 1].re, cs * e[k - 1].im);
                    }
                    if wantv {
                        let (vk, vm) = v.two(1, k, m, p);
                        zdrot(vk, vm, cs, sn);
                    }
                }
            }
            2 => {
                // split at negligible s(l)
                let mut f = e[l - 1].re;
                e[l - 1] = C64::ZERO;
                for k in l..=m {
                    let (t1, _, cs, sn) = drotg(s[k].re, f);
                    s[k] = C64::new(t1, 0.0);
                    // drotg leaves its z in f, which this assignment replaces
                    f = -sn * e[k].re;
                    e[k] = C64::new(cs * e[k].re, cs * e[k].im);
                    if wantu {
                        let (uk, ul) = u.two(1, k, l - 1, n);
                        zdrot(uk, ul, cs, sn);
                    }
                }
            }
            3 => {
                // one QR step
                let mut scale = f2c_cabs(s[m].re, s[m].im);
                for w in [
                    f2c_cabs(s[m - 1].re, s[m - 1].im),
                    f2c_cabs(e[m - 1].re, e[m - 1].im),
                    f2c_cabs(s[l].re, s[l].im),
                    f2c_cabs(e[l].re, e[l].im),
                ] {
                    // f2c's max(a, b): a >= b ? a : b
                    scale = if scale >= w { scale } else { w };
                }
                let sm = s[m].re / scale;
                let smm1 = s[m - 1].re / scale;
                let emm1 = e[m - 1].re / scale;
                let sl = s[l].re / scale;
                let el = e[l].re / scale;
                let b = ((smm1 + sm) * (smm1 - sm) + emm1 * emm1) / 2.0;
                let d1 = sm * emm1;
                let c = d1 * d1;
                let mut shift = 0.0;
                if !(b == 0.0 && c == 0.0) {
                    shift = (b * b + c).sqrt();
                    if b < 0.0 {
                        shift = -shift;
                    }
                    shift = c / (b + shift);
                }
                let mut f = (sl + sm) * (sl - sm) + shift;
                let mut g = sl * el;
                let mm1 = m - 1;
                for k in l..=mm1 {
                    let (r, _, cs, sn) = drotg(f, g);
                    f = r;
                    if k != l {
                        e[k - 1] = C64::new(f, 0.0);
                    }
                    f = cs * s[k].re + sn * e[k].re;
                    let z2 = C64::new(cs * e[k].re, cs * e[k].im);
                    let z3 = C64::new(sn * s[k].re, sn * s[k].im);
                    e[k] = C64::new(z2.re - z3.re, z2.im - z3.im);
                    g = sn * s[k + 1].re;
                    s[k + 1] = C64::new(cs * s[k + 1].re, cs * s[k + 1].im);
                    if wantv {
                        let (vk, vk1) = v.two(1, k, k + 1, p);
                        zdrot(vk, vk1, cs, sn);
                    }
                    let (r, _, cs, sn) = drotg(f, g);
                    f = r;
                    s[k] = C64::new(f, 0.0);
                    f = cs * e[k].re + sn * s[k + 1].re;
                    let d1 = -sn;
                    let z2 = C64::new(d1 * e[k].re, d1 * e[k].im);
                    let z3 = C64::new(cs * s[k + 1].re, cs * s[k + 1].im);
                    s[k + 1] = C64::new(z2.re + z3.re, z2.im + z3.im);
                    g = sn * e[k + 1].re;
                    e[k + 1] = C64::new(cs * e[k + 1].re, cs * e[k + 1].im);
                    if wantu && k < n {
                        let (uk, uk1) = u.two(1, k, k + 1, n);
                        zdrot(uk, uk1, cs, sn);
                    }
                }
                e[m - 1] = C64::new(f, 0.0);
                iter += 1;
            }
            _ => {
                // convergence: the singular value made non-negative, then ordered
                if !(s[l].re >= 0.0) {
                    s[l] = C64::new(-s[l].re, -s[l].im);
                    if wantv {
                        zscal(mone, v.col_mut(1, l, p));
                    }
                }
                while l != mm && !(s[l].re >= s[l + 1].re) {
                    s.swap(l, l + 1);
                    if wantv && l < p {
                        let (a, b) = v.two(1, l, l + 1, p);
                        a.swap_with_slice(b);
                    }
                    if wantu && l < n {
                        let (a, b) = u.two(1, l, l + 1, n);
                        a.swap_with_slice(b);
                    }
                    l += 1;
                }
                iter = 0;
                m -= 1;
            }
        }
        let now = state(&s, &e, m, iter);
        if now == saved {
            info = m;
            cycle = true;
            break;
        }
        steps += 1;
        if steps == power {
            saved = now;
            power = power.saturating_mul(2);
            steps = 0;
        }
    }
    Ok(Zsvdc {
        s,
        u: u.d,
        v: v.d,
        info,
        cycle,
    })
}

/// `vnl_svd<std::complex<double>>(M)` (`zero_out_tol` 0): `U` (`rows x cols`), the singular
/// values `W` (`|s|` by `cabs`, 0 past `min(rows + 1, cols)`) and `V` (`cols x cols`), row-major
/// as VNL keeps them; LINPACK's `info` (0, or the singular value it did not converge on: VNL then
/// prints [`suspicious_zsvdc`] and keeps the decomposition reached).
#[derive(Clone, Debug, PartialEq)]
pub struct VnlCSvd {
    pub u: VnlCMatrix,
    pub w: Vec<f64>,
    pub v: VnlCMatrix,
    pub info: usize,
    /// VNL would never return (ITK-116): LINPACK's iteration came back to a state it had been in.
    pub never_returns: bool,
}

/// [`VnlCSvd`] of `m` (`rows >= 1`, `cols >= 1`), or [`DivisionByZero`] where LINPACK's `z_div`
/// divides by zero (the C code prints `complex division by zero` and aborts).
pub fn vnl_svd_complex(m: &VnlCMatrix) -> Result<VnlCSvd, DivisionByZero> {
    let (n, p) = (m.rows, m.cols);
    assert!(n >= 1 && p >= 1, "vnl_svd: a matrix with elements");
    // vnl_fortran_copy: column-major
    let mut x = F::new(n, p);
    for i in 0..n {
        for j in 0..p {
            x.set(i + 1, j + 1, m.at(i, j));
        }
    }
    let r = zsvdc(x, n, p)?;
    let mm = (n + 1).min(p);
    let mut u = VnlCMatrix::zeros(n, p);
    for j in 0..p {
        for i in 0..n {
            *u.at_mut(i, j) = r.u[j * n + i];
        }
    }
    let mut v = VnlCMatrix::zeros(p, p);
    for j in 0..p {
        for i in 0..p {
            *v.at_mut(i, j) = r.v[j * p + i];
        }
    }
    // W_(j, j) = std::abs(wspace(j)); zero_out_absolute(0) keeps every |w| > 0 and sets the rest
    // to 0
    let w = (0..p)
        .map(|j| {
            let x = if j < mm { cabs(r.s[j + 1]) } else { 0.0 };
            if x.abs() <= 0.0 { 0.0 } else { x }
        })
        .collect();
    Ok(VnlCSvd {
        u,
        w,
        v,
        info: r.info,
        never_returns: r.cycle,
    })
}

impl VnlCSvd {
    /// `solve(B)` (a matrix): `X = U^H B`, each row `i` times `1 / W(i)` where `W(i) != 0`
    /// (C++'s complex `1 / (w, 0)` and products), then `V X`.
    pub fn solve_matrix(&self, b: &VnlCMatrix) -> VnlCMatrix {
        let mut x = self.u.conjugate_transpose().mul(b);
        for i in 0..x.rows {
            let mut weight = C64::real(self.w[i]);
            if !(weight.re == 0.0 && weight.im == 0.0) {
                weight = cxx_div(C64::ONE, weight);
            }
            for j in 0..x.cols {
                let k = i * x.cols + j;
                x.data[k] = cxx_mul(x.data[k], weight);
            }
        }
        self.v.mul(&x)
    }

    /// `solve(y)` (a vector): `x = U^H y`, each `x(i) / W(i)` where `W(i) != 0` (C++'s complex
    /// quotient) and 0 elsewhere, then `V x` (VNL's matrix-vector products: each row's sum from
    /// `(0, 0)` in column order).
    pub fn solve_vector(&self, y: &[C64]) -> Vec<C64> {
        let uh = self.u.conjugate_transpose();
        let mut x = mat_vec(&uh, y);
        for (i, xi) in x.iter_mut().enumerate() {
            let weight = C64::real(self.w[i]);
            *xi = if !(weight.re == 0.0 && weight.im == 0.0) {
                cxx_div(*xi, weight)
            } else {
                C64::ZERO
            };
        }
        mat_vec(&self.v, &x)
    }
}

/// `vnl_matrix<std::complex<double>> * vnl_vector<std::complex<double>>` (`vnl_sse`'s generic
/// `matrix_x_vector`): each row's `som += m(i, j) v(j)` from `(0, 0)`, `j` in order.
pub fn mat_vec(m: &VnlCMatrix, v: &[C64]) -> Vec<C64> {
    assert_eq!(m.cols, v.len(), "matrix_x_vector: sizes");
    (0..m.rows)
        .map(|i| {
            let mut som = C64::ZERO;
            for (j, &vj) in v.iter().enumerate() {
                som = som.add(cxx_mul(m.at(i, j), vj));
            }
            som
        })
        .collect()
}

/// The warning `vnl_svd<std::complex<double>>` prints on standard error when LINPACK does not
/// converge (ITK 4.13.2's `vnl_svd.hxx`: its `__FILE__` given as `file`): the return value, the
/// size, then `vnl_matlab_print(std::cerr, M, "M", vnl_matlab_print_format_long)`.
pub fn suspicious_zsvdc(file: &str, m: &VnlCMatrix, info: usize) -> String {
    let mut s = format!(
        "{file}: suspicious return value ({info}) from SVDC\n{file}: M is {}x{}\n",
        m.rows, m.cols
    );
    s.push_str(&crate::vnl_matrix::matlab_print_long_complex(m, "M"));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cm(rows: usize, cols: usize, v: &[(f64, f64)]) -> VnlCMatrix {
        VnlCMatrix {
            rows,
            cols,
            data: v.iter().map(|&(a, b)| C64::new(a, b)).collect(),
        }
    }

    /// The decomposition reconstructs the matrix, and `solve` inverts a well-conditioned one.
    #[test]
    fn decomposes_and_solves() {
        let a = cm(
            3,
            3,
            &[
                (2.0, 0.5),
                (0.25, -1.0),
                (0.0, 0.125),
                (-1.0, 0.0),
                (3.0, 1.0),
                (0.5, 0.5),
                (0.125, -0.25),
                (1.0, 0.0),
                (1.5, -0.75),
            ],
        );
        let svd = vnl_svd_complex(&a).expect("no zero divisor");
        assert_eq!(svd.info, 0);
        assert!(svd.w.windows(2).all(|w| w[0] >= w[1]));
        let b = cm(3, 1, &[(1.0, 0.0), (0.0, 1.0), (-1.0, 2.0)]);
        let x = svd.solve_matrix(&b);
        let back = a.mul(&x);
        for k in 0..3 {
            assert!((back.data[k].re - b.data[k].re).abs() < 1e-13);
            assert!((back.data[k].im - b.data[k].im).abs() < 1e-13);
        }
        let y: Vec<C64> = b.data.clone();
        // the vector solve divides by W where the matrix solve multiplies by 1 / W
        let xv = svd.solve_vector(&y);
        for k in 0..3 {
            assert!((xv[k].re - x.data[k].re).abs() < 1e-15);
            assert!((xv[k].im - x.data[k].im).abs() < 1e-15);
        }
    }

    /// libf2c's division: Smith's on both branches, and the zero divisor.
    #[test]
    fn f2c_division() {
        assert_eq!(
            z_div(C64::new(1.0, 2.0), C64::new(3.0, 4.0)),
            Ok(C64::new(0.44, 0.08))
        );
        assert_eq!(
            z_div(C64::new(1.0, 0.0), C64::new(0.0, 0.0)),
            Err(DivisionByZero)
        );
        assert_eq!(f2c_cabs(-3.0, 4.0), 5.0);
        assert_eq!(f2c_cabs(1.0, 1e-20), 1.0);
    }
}
