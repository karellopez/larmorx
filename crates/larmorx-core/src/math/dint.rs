// SPDX-License-Identifier: Apache-2.0 AND MIT
//! 128-bit floating-point ("dint") arithmetic for the accurate paths of the correctly rounded
//! functions in [`super`].
//!
//! A [`Dint64`] `{hi, lo, ex, sgn}` represents `(-1)^sgn · (hi + lo/2^64) · 2^(ex-63)`: a
//! 128-bit significand `hi:lo`, normalized so that the top bit of `hi` is set (the value 0 has
//! `hi = lo = 0`), a signed binary exponent and a sign bit.
//!
//! Port of CORE-MATH's `src/binary64/log/dint.h` at commit 040ee482a8ca (MIT licence, © CERN;
//! Tom Hubrecht's code for the correctly rounded `pow`), re-expressed in Rust for
//! round-to-nearest only. Only what `log` needs is ported; the
//! log tables `_INVERSE_2`, `_LOG_INV_2` and `P_2` that dint.h also holds live in
//! [`super::log`](mod@super::log). The operations keep the C code's integer arithmetic exactly: C's unsigned
//! `+`/`-` become `wrapping_*`, `__builtin_clzll` becomes `leading_zeros`, and the
//! `uint128_t {h, l}` union becomes a `u128` split by [`hi64`] / [`lo64`].
//!
//! Reuse: CORE-MATH's binary64 `sin`/`cos` use a *revised* dint (in `cos.c`/`sin.c`) whose
//! significand convention is `hi/2^64 ∈ [1/2, 1)` and whose `add_dint`, `mul_dint` and
//! `dint_tod` differ (truncating addition with a Sterbenz-exact case, truncated rather than
//! rounded middle products, subnormal-aware conversion). Those can reuse [`Dint64`] and the
//! 128-bit helpers, but their operations must be ported as separate functions, not swapped for
//! the ones here.
//
// Original copyright and licence (MIT) notice (dint.h):
//
//   Copyright (c) 2022 CERN.
//   Author: Tom Hubrecht
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

use std::cmp::Ordering;

/// `dint64_t`: `(-1)^sgn · (hi + lo/2^64) · 2^(ex-63)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Dint64 {
    pub(crate) hi: u64,
    pub(crate) lo: u64,
    pub(crate) ex: i64,
    pub(crate) sgn: u64,
}

impl Dint64 {
    /// A value with the fields in the order of the C initializers `{.hi, .lo, .ex, .sgn}`.
    pub(crate) const fn new(hi: u64, lo: u64, ex: i64, sgn: u64) -> Self {
        Self { hi, lo, ex, sgn }
    }
}

/// `M_ONE`: -1.
pub(crate) const M_ONE: Dint64 = Dint64::new(0x8000000000000000, 0x0, 0, 0x1);

/// `LOG2`: log(2), with absolute error less than 2^-129.97.
pub(crate) const LOG2: Dint64 = Dint64::new(0xb17217f7d1cf79ab, 0xc9e3b39803f2f6af, -1, 0x0);

/// `ZERO`: 0.
pub(crate) const ZERO: Dint64 = Dint64::new(0x0, 0x0, 0, 0x0);

/// High 64 bits of a 128-bit integer (`uint128_t.h`).
#[inline(always)]
pub(crate) const fn hi64(x: u128) -> u64 {
    (x >> 64) as u64
}

/// Low 64 bits of a 128-bit integer (`uint128_t.l`).
#[inline(always)]
pub(crate) const fn lo64(x: u128) -> u64 {
    x as u64
}

/// `h:l` as a 128-bit integer.
#[inline(always)]
pub(crate) const fn join128(h: u64, l: u64) -> u128 {
    ((h as u128) << 64) | l as u128
}

/// `addu_128`: `a + b` modulo 2^128, and whether the addition overflowed (the carry out).
#[inline(always)]
pub(crate) fn addu_128(a: u128, b: u128) -> (u128, bool) {
    let (ah, al) = (hi64(a), lo64(a));
    let (bh, bl) = (hi64(b), lo64(b));
    let rl = al.wrapping_add(bl);
    let rh = ah.wrapping_add(bh).wrapping_add(u64::from(rl < al));
    // Return the overflow
    let overflow = if rh == ah { rl < al } else { rh < ah };
    (join128(rh, rl), overflow)
}

/// `subu_128`: `a - b` modulo 2^128, and whether the subtraction underflowed (the borrow).
#[inline(always)]
pub(crate) fn subu_128(a: u128, b: u128) -> (u128, bool) {
    let c = b.wrapping_neg();
    let (ah, al) = (hi64(a), lo64(a));
    let (ch, cl) = (hi64(c), lo64(c));
    let rl = al.wrapping_add(cl);
    let rh = ah.wrapping_add(ch).wrapping_add(u64::from(rl < al));
    // Return the underflow
    let underflow = if ah != rh { rh > ah } else { rl > al };
    (join128(rh, rl), underflow)
}

/// `cmp_dint`: compares `(ex, hi, lo)` lexicographically, i.e. the magnitudes of two normalized
/// non-zero values; the signs are ignored.
#[inline(always)]
pub(crate) fn cmp_dint(a: &Dint64, b: &Dint64) -> Ordering {
    a.ex.cmp(&b.ex)
        .then_with(|| a.hi.cmp(&b.hi))
        .then_with(|| a.lo.cmp(&b.lo))
}

/// `add_dint`: `a + b`, with the shifted-out bits of the smaller operand rounded to nearest.
pub(crate) fn add_dint(a: &Dint64, b: &Dint64) -> Dint64 {
    if (a.hi | a.lo) == 0 {
        return *b;
    }

    if (b.hi | b.lo) == 0 {
        return *a;
    }

    match cmp_dint(a, b) {
        Ordering::Equal => {
            if (a.sgn ^ b.sgn) != 0 {
                return ZERO;
            }

            let mut r = *a;
            r.ex += 1;
            return r;
        }
        Ordering::Less => return add_dint(b, a),
        Ordering::Greater => {}
    }

    // From now on, |A| > |B|

    let a128 = join128(a.hi, a.lo);
    let mut b128 = join128(b.hi, b.lo);
    let mut m_ex = a.ex;

    if a.ex > b.ex {
        // C: `int sh = a->ex - b->ex;`
        let sh = (a.ex - b.ex) as i32;
        // round to nearest
        if sh <= 128 {
            b128 = b128.wrapping_add(0x1 & (b128 >> (sh - 1) as u32));
        }
        if sh < 128 {
            b128 >>= sh as u32;
        } else {
            b128 = 0;
        }
    }

    let mut c: u128;
    // C: `unsigned char sgn = a->sgn;`
    let sgn = a.sgn as u8;

    if (a.sgn ^ b.sgn) != 0 {
        // a and b have different signs C = A + (-B)
        (c, _) = subu_128(a128, b128);
    } else {
        let overflow;
        (c, overflow) = addu_128(a128, b128);
        if overflow {
            c = c.wrapping_add(u128::from(lo64(c) & 0x1));
            c = (1u128 << 127) | (c >> 1);
            m_ex += 1;
        }
    }

    // C: `uint64_t ex = C.h ? clz(C.h) : 64 + (C.l ? clz(C.l) : a->ex);` then `r->ex = m_ex - ex`
    // in 64-bit modular arithmetic.
    let ex: i64 = if hi64(c) != 0 {
        i64::from(hi64(c).leading_zeros())
    } else {
        64i64.wrapping_add(if lo64(c) != 0 {
            i64::from(lo64(c).leading_zeros())
        } else {
            a.ex
        })
    };
    // `ex` is a leading-zero count (< 128) unless C = 0, and C shifted by anything stays 0;
    // Rust's `<<` needs that case spelled out because it rejects shifts of 128 or more.
    if c != 0 {
        c <<= ex as u32;
    }

    Dint64 {
        sgn: u64::from(sgn),
        hi: hi64(c),
        lo: lo64(c),
        ex: m_ex.wrapping_sub(ex),
    }
}

/// `mul_dint`: `a · b`, with 126 bits of accuracy.
pub(crate) fn mul_dint(a: &Dint64, b: &Dint64) -> Dint64 {
    let mut t = u128::from(a.hi) * u128::from(b.hi);
    let m1 = u128::from(a.hi) * u128::from(b.lo);
    let m2 = u128::from(a.lo) * u128::from(b.hi);

    // If we only garantee 127 bits of accuracy, we improve the simplicity of the
    // code uint64_t l = ((u128)(a->lo) * (u128)(b->lo)) >> 64; m.l += l; m.h +=
    // (m.l < l);
    let (m, carry) = addu_128(m1, m2);
    // C: `t.h += addu_128(m1, m2, &m);` (adds to the high word, modulo 2^64)
    t = join128(hi64(t).wrapping_add(u64::from(carry)), lo64(t));
    t = t.wrapping_add(u128::from(hi64(m)));

    // Ensure that r->hi starts with a 1
    let ex = u64::from(hi64(t) >> 63 == 0);
    if ex != 0 {
        t <<= 1;
    }

    t = t.wrapping_add(u128::from(lo64(m) >> 63));

    Dint64 {
        hi: hi64(t),
        lo: lo64(t),
        // Exponent and sign
        ex: a.ex + b.ex - ex as i64 + 1,
        sgn: a.sgn ^ b.sgn,
    }
}

/// `mul_dint_2`: the integer `b` times `a`.
pub(crate) fn mul_dint_2(b: i64, a: &Dint64) -> Dint64 {
    if b == 0 {
        return ZERO;
    }

    let c: u64 = b.unsigned_abs();
    let sgn = if b < 0 { u64::from(a.sgn == 0) } else { a.sgn };

    let mut t = u128::from(a.hi) * u128::from(c);

    let mut m: i32 = if hi64(t) != 0 {
        hi64(t).leading_zeros() as i32
    } else {
        64
    };
    t <<= m as u32;

    // Will pose issues if b is too large but for now we assume it never happens
    // TODO: FIXME
    let mut l = u128::from(a.lo) * u128::from(c);
    l = (l << (m - 1) as u32) >> 63;

    let overflow;
    (t, overflow) = addu_128(l, t);
    if overflow {
        t = t.wrapping_add(t & 0x1);
        t = (1u128 << 127) | (t >> 1);
        m -= 1;
    }

    Dint64 {
        sgn,
        hi: hi64(t),
        lo: lo64(t),
        ex: a.ex + 64 - i64::from(m),
    }
}

#[cfg(test)]
mod tests {
    use super::{addu_128, subu_128};

    /// The word-wise carry and borrow logic of dint.h equals 128-bit modular arithmetic.
    #[test]
    fn addu_subu_match_u128_overflowing_ops() {
        let edge = [
            0u128,
            1,
            u128::from(u64::MAX),
            1u128 << 64,
            (1u128 << 64) + 1,
            1u128 << 127,
            u128::MAX - 1,
            u128::MAX,
            0x8000_0000_0000_0000_ffff_ffff_ffff_ffff,
            0xffff_ffff_ffff_ffff_0000_0000_0000_0001,
        ];
        for &a in &edge {
            for &b in &edge {
                assert_eq!(addu_128(a, b), a.overflowing_add(b), "{a:#x} + {b:#x}");
                assert_eq!(subu_128(a, b), a.overflowing_sub(b), "{a:#x} - {b:#x}");
            }
        }
    }
}
