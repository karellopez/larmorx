// SPDX-License-Identifier: Apache-2.0 AND MIT
//! 128-bit floating-point ("dint") arithmetic of the second phase of [`super::pow`].
//!
//! A [`Dint`] `{r, ex, sgn}` represents `(-1)^sgn · r/2^127 · 2^ex`: a 128-bit significand
//! `r = hi:lo` whose top bit is set (0 has `hi = 0`), a signed exponent and a sign bit.
//!
//! Port of CORE-MATH's `src/binary64/pow/dint.h` at commit 040ee482a8ca (MIT licence), which
//! differs from the `dint.h` of `log` that [`crate::math::dint`] ports (different `ZERO`,
//! `LOG2_INV`, carry tests and the operations `add_dint_11`, `mul_dint_21`, `mul_dint_11`,
//! `mul_dint_int64`): so it is ported on its own. The C code's integer arithmetic is kept
//! exactly: unsigned `+`/`-` are `wrapping_*`, `__builtin_clzll` is `leading_zeros`, and the C
//! functions that write into `*r` return the new value. `add_dint_11` writes only the high limb
//! of `*r`, so it takes the previous `*r` too.
//
// Original copyright and licence (MIT) notice (dint.h):
//
//   Copyright (c) 2022, 2023 CERN and Inria
//   Authors: Tom Hubrecht and Paul Zimmermann
//
//   This file is part of the CORE-MATH project
//   (https://core-math.gitlabpages.inria.fr/).
//
//   Permission is hereby granted, free of charge, to any person obtaining a copy
//   of this software and associated documentation files (the "Software"), to deal
//   in the Software without restriction, including without limitation the rights
//   to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//   copies of the Software, and to permit persons to whom the Software is
//   furnished to do so, subject to the following conditions:
//
//   The above copyright notice and this permission notice shall be included in all
//   copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//   AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//   OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//   SOFTWARE.

/// `dint64_t`: `(-1)^sgn · (hi + lo/2^64) · 2^(ex-63)`, with `r = hi:lo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Dint {
    pub(super) r: u128,
    pub(super) ex: i64,
    pub(super) sgn: u64,
}

/// A value from the C initializer `{.hi, .lo, .ex, .sgn}`.
pub(super) const fn d(hi: u64, lo: u64, ex: i64, sgn: u64) -> Dint {
    Dint {
        r: ((hi as u128) << 64) | lo as u128,
        ex,
        sgn,
    }
}

impl Dint {
    #[inline(always)]
    pub(super) const fn hi(&self) -> u64 {
        (self.r >> 64) as u64
    }
    #[inline(always)]
    pub(super) const fn lo(&self) -> u64 {
        self.r as u64
    }
    #[inline(always)]
    fn set_hi(&mut self, hi: u64) {
        self.r = ((hi as u128) << 64) | self.lo() as u128;
    }
}

/// `M_ONE`: -1.
pub(super) const M_ONE: Dint = d(0x8000000000000000, 0x0, 0, 0x1);
/// `LOG2`: log(2) to nearest, absolute error below 2^-129.97.
pub(super) const LOG2: Dint = d(0xb17217f7d1cf79ab, 0xc9e3b39803f2f6af, -1, 0x0);
/// `LOG2_INV`: 2^12/log(2), absolute error below 2^-52.96.
pub(super) const LOG2_INV: Dint = d(0xb8aa3b295c17f0bc, 0x0, 12, 0x0);
/// `ZERO`: 0, with the exponent that makes `dint_tod(ZERO)` 0.
pub(super) const ZERO: Dint = d(0x0, 0x0, -1076, 0x0);

#[inline(always)]
fn cmp<T: PartialOrd>(a: T, b: T) -> i32 {
    i32::from(a > b) - i32::from(a < b)
}

/// `dint_zero_p`.
#[inline(always)]
fn dint_zero_p(a: &Dint) -> bool {
    a.hi() == 0
}

/// `cmp_dint_abs`: the sign of |a| - |b|.
#[inline(always)]
pub(super) fn cmp_dint_abs(a: &Dint, b: &Dint) -> i32 {
    if dint_zero_p(a) {
        return if dint_zero_p(b) { 0 } else { -1 };
    }
    if dint_zero_p(b) {
        return 1;
    }
    let c1 = cmp(a.ex, b.ex);
    if c1 != 0 { c1 } else { cmp(a.r, b.r) }
}

/// `cmp_dint_11`.
#[inline(always)]
fn cmp_dint_11(a: &Dint, b: &Dint) -> i32 {
    let c1 = cmp(a.ex, b.ex);
    if c1 != 0 { c1 } else { cmp(a.hi(), b.hi()) }
}

/// `add_dint`: a + b, error at most 2 ulps; exact in Sterbenz's case.
#[inline(always)]
pub(super) fn add_dint(a: &Dint, b: &Dint) -> Dint {
    if a.r == 0 {
        return *b;
    }
    let (mut a, mut b) = (a, b);
    match cmp_dint_abs(a, b) {
        0 => {
            if a.sgn ^ b.sgn != 0 {
                return ZERO;
            }
            let mut r = *a;
            r.ex += 1;
            return r;
        }
        -1 => std::mem::swap(&mut a, &mut b),
        _ => {}
    }
    // From now on |a| > |b|, so a.ex >= b.ex.
    let big_a = a.r;
    let mut big_b = b.r;
    let k = a.ex.wrapping_sub(b.ex) as u64;
    if k > 0 {
        big_b = if k < 128 { big_b >> k } else { 0 };
    }
    let sgn = a.sgn;
    let mut ex_r = a.ex;
    let mut c;
    if a.sgn ^ b.sgn != 0 {
        c = big_a.wrapping_sub(big_b);
        let ch = (c >> 64) as u64;
        let mut ex = if ch != 0 {
            ch.leading_zeros()
        } else {
            64 + (c as u64).leading_zeros()
        };
        if ex > 0 {
            c = if k == 1 {
                (big_a << ex).wrapping_sub(b.r << (ex - 1))
            } else {
                (big_a << ex).wrapping_sub(big_b << ex)
            };
            ex_r = ex_r.wrapping_sub(i64::from(ex));
            ex = ((c >> 64) as u64).leading_zeros();
        }
        c <<= ex;
        ex_r = ex_r.wrapping_sub(i64::from(ex));
    } else {
        c = big_a.wrapping_add(big_b);
        if c < big_a {
            c = (1u128 << 127) | (c >> 1);
            ex_r += 1;
        }
    }
    Dint {
        r: c,
        ex: ex_r,
        sgn,
    }
}

/// `add_dint_11`: as [`add_dint`] on the high limbs only, error at most 2 ulps (of 64 bits).
/// `prev` is `*r` before the call: the C code leaves its low limb as it was.
#[inline(always)]
pub(super) fn add_dint_11(prev: &Dint, a: &Dint, b: &Dint) -> Dint {
    if a.hi() == 0 {
        return *b;
    }
    if b.hi() == 0 {
        return *a;
    }
    let (mut a, mut b) = (a, b);
    match cmp_dint_11(a, b) {
        0 => {
            if a.sgn ^ b.sgn != 0 {
                return ZERO;
            }
            let mut r = *a;
            r.ex += 1;
            return r;
        }
        -1 => std::mem::swap(&mut a, &mut b),
        _ => {}
    }
    let big_a = a.hi();
    let mut big_b = b.hi();
    if a.ex > b.ex {
        let k = a.ex.wrapping_sub(b.ex) as u64;
        big_b = if k < 64 { big_b >> k } else { 0 };
    }
    let sgn = a.sgn;
    let mut r = *prev;
    r.ex = a.ex;
    let mut c: u64;
    if a.sgn ^ b.sgn != 0 {
        c = big_a.wrapping_sub(big_b);
        let mut ex = c.leading_zeros();
        if ex > 0 {
            c = (big_a << ex).wrapping_sub(big_b << ex);
            r.ex = r.ex.wrapping_sub(i64::from(ex));
            ex = c.leading_zeros();
        }
        c <<= ex;
        r.ex = r.ex.wrapping_sub(i64::from(ex));
    } else {
        c = big_a.wrapping_add(big_b);
        if c < big_a {
            c = (1u64 << 63) | (c >> 1);
            r.ex += 1;
        }
    }
    r.sgn = sgn;
    r.set_hi(c);
    r
}

/// `mul_dint`: a · b, error at most 6 ulps.
#[inline(always)]
pub(super) fn mul_dint(a: &Dint, b: &Dint) -> Dint {
    let bh = b.hi() as u128;
    let bl = b.lo() as u128;
    let m1 = a.hi() as u128 * bl;
    let m2 = a.lo() as u128 * bh;
    let mut r = a.hi() as u128 * bh;
    r += (m1 >> 64) + (m2 >> 64);
    let ex = (r >> 127) as u64;
    r <<= 1 - ex;
    Dint {
        r,
        ex: a.ex + b.ex + ex as i64,
        sgn: a.sgn ^ b.sgn,
    }
}

/// `mul_dint_21`: a · b where b's low limb is 0, error at most 2 ulps.
#[inline(always)]
pub(super) fn mul_dint_21(a: &Dint, b: &Dint) -> Dint {
    let bh = b.hi() as u128;
    let hi = a.hi() as u128 * bh;
    let lo = a.lo() as u128 * bh;
    let mut r = hi + (lo >> 64);
    let ex = (r >> 127) as u64;
    r <<= 1 - ex;
    Dint {
        r,
        ex: a.ex + b.ex + ex as i64,
        sgn: a.sgn ^ b.sgn,
    }
}

/// `mul_dint_11`: the exact product of the high limbs.
#[inline(always)]
pub(super) fn mul_dint_11(a: &Dint, b: &Dint) -> Dint {
    let mut r = a.hi() as u128 * b.hi() as u128;
    let ex = (r >> 127) as u64;
    r <<= 1 - ex;
    Dint {
        r,
        ex: a.ex + b.ex + ex as i64,
        sgn: a.sgn ^ b.sgn,
    }
}

/// `mul_dint_int64`: a · b for an integer b, error below 1 ulp.
#[inline(always)]
pub(super) fn mul_dint_int64(a: &Dint, b: i64) -> Dint {
    if b == 0 {
        return ZERO;
    }
    let c = if b < 0 {
        b.wrapping_neg() as u64
    } else {
        b as u64
    };
    let sgn = if b < 0 { u64::from(a.sgn == 0) } else { a.sgn };
    let mut ex = a.ex + 64;
    let mut r = a.hi() as u128 * c as u128;
    let rh = (r >> 64) as u64;
    let m = if rh != 0 { rh.leading_zeros() } else { 64 };
    r <<= m;
    ex -= i64::from(m);
    let mut l = a.lo() as u128 * c as u128;
    l = (l << (m - 1)) >> 63;
    r = r.wrapping_add(l);
    if r < l {
        r = (1u128 << 127) | (r >> 1);
        ex += 1;
    }
    Dint { r, ex, sgn }
}

/// `dint_fromd`: the non-zero double `b`, exactly.
#[inline(always)]
pub(super) fn dint_fromd(b: f64) -> Dint {
    let u = b.to_bits();
    let e = ((u >> 52) & 0x7ff) as i64;
    let mut hi = (u & (!0u64 >> 12)) + if e != 0 { 1u64 << 52 } else { 0 };
    let ex = e - 0x3ff;
    let t = hi.leading_zeros();
    hi <<= t;
    let ex = ex - if t > 11 { i64::from(t) - 12 } else { 0 };
    Dint {
        r: (hi as u128) << 64,
        ex,
        sgn: u64::from(b < 0.0),
    }
}

/// `dint_toi`: a rounded toward zero to an integer.
#[inline(always)]
pub(super) fn dint_toi(a: &Dint) -> i64 {
    if a.ex < 0 {
        return 0;
    }
    let r = (a.hi() >> (63 - a.ex)) as i64;
    if a.sgn != 0 { r.wrapping_neg() } else { r }
}
