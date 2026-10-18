// SPDX-License-Identifier: Apache-2.0 AND MIT
//! 256-bit floating-point ("qint") arithmetic of the third phase of [`super::pow`].
//!
//! A [`Qint`] `{rh, rl, ex, sgn}` represents `(-1)^sgn · (hh + hl/2^64 + lh/2^128 + ll/2^192)
//! · 2^(ex-63)`, with `rh = hh:hl` and `rl = lh:ll`.
//!
//! Port of CORE-MATH's `src/binary64/pow/qint.h` at commit 040ee482a8ca (MIT licence). The C
//! code's integer arithmetic is kept exactly (unsigned `+`/`-` are `wrapping_*`,
//! `__builtin_clzll` is `leading_zeros`); the C functions that write into `*r` return the new
//! value (every one of them writes all of `*r`).
//
// Original copyright and licence (MIT) notice (qint.h):
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

/// `qint64_t`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Qint {
    pub(super) rh: u128,
    pub(super) rl: u128,
    pub(super) ex: i64,
    pub(super) sgn: u64,
}

/// A value from the C initializer `{.hh, .hl, .lh, .ll, .ex, .sgn}`.
pub(super) const fn q(hh: u64, hl: u64, lh: u64, ll: u64, ex: i64, sgn: u64) -> Qint {
    Qint {
        rh: ((hh as u128) << 64) | hl as u128,
        rl: ((lh as u128) << 64) | ll as u128,
        ex,
        sgn,
    }
}

const LOW64: u128 = 0xffff_ffff_ffff_ffff;

impl Qint {
    #[inline(always)]
    pub(super) const fn hh(&self) -> u64 {
        (self.rh >> 64) as u64
    }
    #[inline(always)]
    pub(super) const fn hl(&self) -> u64 {
        self.rh as u64
    }
    #[inline(always)]
    pub(super) const fn lh(&self) -> u64 {
        (self.rl >> 64) as u64
    }
    #[inline(always)]
    pub(super) const fn ll(&self) -> u64 {
        self.rl as u64
    }
}

/// `M_ONE_Q`: -1.
pub(super) const M_ONE_Q: Qint = q(0x8000000000000000, 0x0, 0x0, 0x0, 0, 0x1);
/// `LOG2_Q`: log(2), absolute error below 2^-256.14.
pub(super) const LOG2_Q: Qint = q(
    0xb17217f7d1cf79ab,
    0xc9e3b39803f2f6af,
    0x40f343267298b62d,
    0x8a0d175b8baafa2b,
    -1,
    0x0,
);
/// `LOG2_INV_Q`: 2^12/log(2), absolute error below 2^-52.96.
pub(super) const LOG2_INV_Q: Qint = q(0xb8aa3b295c17f0bc, 0x0, 0x0, 0x0, 12, 0x0);
/// `ZERO_Q`: 0.
pub(super) const ZERO_Q: Qint = q(0x0, 0x0, 0x0, 0x0, 0, 0x0);

#[inline(always)]
fn cmp<T: PartialOrd>(a: T, b: T) -> i32 {
    i32::from(a > b) - i32::from(a < b)
}

/// `cmp_qint`: the sign of |a| - |b| (non-zero operands).
#[inline(always)]
fn cmp_qint(a: &Qint, b: &Qint) -> i32 {
    let c = cmp(a.ex, b.ex);
    if c != 0 {
        return c;
    }
    let c = cmp(a.rh, b.rh);
    if c != 0 { c } else { cmp(a.rl, b.rl) }
}

/// `cmp_qint_22`: as [`cmp_qint`] on the upper two limbs.
#[inline(always)]
fn cmp_qint_22(a: &Qint, b: &Qint) -> i32 {
    let c = cmp(a.ex, b.ex);
    if c != 0 { c } else { cmp(a.rh, b.rh) }
}

/// The number of leading zeros of the 256-bit `ch:cl` (as the C code's nested `clzll`).
#[inline(always)]
fn clz256(ch: u128, cl: u128) -> u32 {
    if ch != 0 {
        ch.leading_zeros()
    } else {
        128 + cl.leading_zeros()
    }
}

/// `add_qint`: a + b, error at most 2 ulps; exact in Sterbenz's case.
pub(super) fn add_qint(a: &Qint, b: &Qint) -> Qint {
    if a.rh == 0 && a.rl == 0 {
        return *b;
    }
    if b.rh == 0 && b.rl == 0 {
        return *a;
    }
    match cmp_qint(a, b) {
        0 => {
            if a.sgn ^ b.sgn != 0 {
                return ZERO_Q;
            }
            let mut r = *a;
            r.ex += 1;
            return r;
        }
        -1 => return add_qint(b, a),
        _ => {}
    }
    let (mut ah, mut al, mut bh, mut bl) = (a.rh, a.rl, b.rh, b.rl);
    let k = a.ex - b.ex;
    if k > 0 {
        if k >= 128 {
            bl = if k < 256 { bh >> (k - 128) } else { 0 };
            bh = 0;
        } else {
            bl = (bl >> k) | (bh << (128 - k));
            bh >>= k;
        }
    }
    let sgn = a.sgn;
    let mut ex_r = a.ex;
    let (mut ch, mut cl);
    if a.sgn ^ b.sgn != 0 {
        ch = ah.wrapping_sub(bh);
        cl = al.wrapping_sub(bl);
        if cl > al {
            ch = ch.wrapping_sub(1);
        }
        let mut ex = clz256(ch, cl);
        if ex > 0 {
            if ex >= 128 {
                ah = al << (ex - 128);
                al = 0;
            } else {
                ah = (ah << ex) | (al >> (128 - ex));
                al <<= ex;
            }
            let sh = i64::from(ex) - k;
            bh = b.rh;
            bl = b.rl;
            if sh >= 0 {
                if sh >= 128 {
                    bh = bl << (sh - 128);
                    bl = 0;
                } else if sh > 0 {
                    bh = (bh << sh) | (bl >> (128 - sh));
                    bl <<= sh;
                }
            } else {
                let j = -sh;
                if j >= 128 {
                    bl = bh >> (j - 128);
                    bh = 0;
                } else {
                    bl = (bh << (128 - j)) | (bl >> j);
                    bh >>= j;
                }
            }
            ex_r -= i64::from(ex);
            ch = ah.wrapping_sub(bh);
            cl = al.wrapping_sub(bl);
            if cl > al {
                ch = ch.wrapping_sub(1);
            }
            ex = clz256(ch, cl);
        }
        if ex != 0 {
            ch = (ch << ex) | (cl >> (128 - ex));
            cl <<= ex;
        }
        ex_r -= i64::from(ex);
    } else {
        let (sum_h, carry_h) = ah.overflowing_add(bh);
        ch = sum_h;
        let mut cy = u32::from(carry_h);
        let (sum_l, carry_l) = al.overflowing_add(bl);
        cl = sum_l;
        if carry_l {
            ch = ch.wrapping_add(1);
            cy += u32::from(ch == 0);
        }
        if cy != 0 {
            cl = (ch << 127) | (cl >> 1);
            ch = (1u128 << 127) | (ch >> 1);
            ex_r += 1;
        }
    }
    Qint {
        rh: ch,
        rl: cl,
        ex: ex_r,
        sgn,
    }
}

/// `add_qint_22`: as [`add_qint`] on the upper two limbs, error below 2 ulps (of 128 bits).
pub(super) fn add_qint_22(a: &Qint, b: &Qint) -> Qint {
    if a.rh == 0 {
        return *b;
    }
    if b.rh == 0 {
        return *a;
    }
    match cmp_qint_22(a, b) {
        0 => {
            if a.sgn ^ b.sgn != 0 {
                return ZERO_Q;
            }
            let mut r = *a;
            r.ex += 1;
            return r;
        }
        -1 => return add_qint_22(b, a),
        _ => {}
    }
    let mut ah = a.rh;
    let mut bh = b.rh;
    let k = a.ex.wrapping_sub(b.ex) as u64;
    if k > 0 {
        bh = if k >= 128 { 0 } else { bh >> k };
    }
    let sgn = a.sgn;
    let mut ex_r = a.ex;
    let mut ch;
    if a.sgn ^ b.sgn != 0 {
        ch = ah.wrapping_sub(bh);
        let mut ex = u64::from(ch.leading_zeros());
        if ex > 0 {
            ah <<= ex;
            bh = if ex >= k {
                b.rh << (ex - k)
            } else {
                b.rh >> (k - ex)
            };
            ex_r = ex_r.wrapping_sub(ex as i64);
            ch = ah.wrapping_sub(bh);
            ex = u64::from(ch.leading_zeros());
        }
        ch <<= ex;
        ex_r = ex_r.wrapping_sub(ex as i64);
    } else {
        let (sum, carry) = ah.overflowing_add(bh);
        ch = sum;
        if carry {
            ch = (1u128 << 127) | (ch >> 1);
            ex_r += 1;
        }
    }
    Qint {
        rh: ch,
        rl: 0,
        ex: ex_r,
        sgn,
    }
}

/// The common end of the `mul_qint*` functions: normalise `t6:t5` and set the exponent and
/// sign.
#[inline(always)]
fn mul_end(t6: u128, t5: u128, a: &Qint, b: &Qint) -> Qint {
    let ex = u64::from(t6 >> 127 == 0);
    let (rh, rl) = if ex != 0 {
        ((t6 << 1) | (t5 >> 127), t5 << 1)
    } else {
        (t6, t5)
    };
    Qint {
        rh,
        rl,
        ex: a.ex + b.ex + 1 - ex as i64,
        sgn: a.sgn ^ b.sgn,
    }
}

#[inline(always)]
fn addc(a: u128, b: u128) -> (u128, u128) {
    let (s, c) = a.overflowing_add(b);
    (s, u128::from(c))
}

#[inline(always)]
fn m(a: u64, b: u64) -> u128 {
    a as u128 * b as u128
}

/// `mul_qint`: a · b, error below 14 ulps.
pub(super) fn mul_qint(a: &Qint, b: &Qint) -> Qint {
    let r33 = m(a.hh(), b.hh());
    let r32 = m(a.hh(), b.hl());
    let r23 = m(a.hl(), b.hh());
    let r31 = m(a.hh(), b.lh());
    let r13 = m(a.lh(), b.hh());
    let r22 = m(a.hl(), b.hl());
    let r30 = m(a.hh(), b.ll());
    let r03 = m(a.ll(), b.hh());
    let r21 = m(a.hl(), b.lh());
    let r12 = m(a.lh(), b.hl());
    let t3 = (r12 >> 64) + (r21 >> 64) + (r03 >> 64) + (r30 >> 64);
    let (t4, c) = addc(r22, t3);
    let mut c4 = c;
    let (t4, c) = addc(r13, t4);
    c4 += c;
    let (t4, c) = addc(r31, t4);
    c4 += c;
    let (t5, c) = addc(r23, t4 >> 64);
    let mut c5 = c;
    let (t5, c) = addc(r32, t5);
    c5 += c;
    let t6 = r33.wrapping_add((c5 << 64) | (t5 >> 64)).wrapping_add(c4);
    let t5 = (t5 << 64) | (t4 & LOW64);
    mul_end(t6, t5, a, b)
}

/// `mul_qint_33`: as [`mul_qint`] on the upper three limbs, error below 6 ulps.
pub(super) fn mul_qint_33(a: &Qint, b: &Qint) -> Qint {
    let r33 = m(a.hh(), b.hh());
    let r32 = m(a.hh(), b.hl());
    let r23 = m(a.hl(), b.hh());
    let r31 = m(a.hh(), b.lh());
    let r13 = m(a.lh(), b.hh());
    let r22 = m(a.hl(), b.hl());
    let r21 = m(a.hl(), b.lh());
    let r12 = m(a.lh(), b.hl());
    let t3 = (r12 >> 64) + (r21 >> 64);
    let (t4, c) = addc(r22, t3);
    let mut c4 = c;
    let (t4, c) = addc(r13, t4);
    c4 += c;
    let (t4, c) = addc(r31, t4);
    c4 += c;
    let (t5, c) = addc(r23, t4 >> 64);
    let mut c5 = c;
    let (t5, c) = addc(r32, t5);
    c5 += c;
    let t6 = r33.wrapping_add((c5 << 64) | (t5 >> 64)).wrapping_add(c4);
    let t5 = (t5 << 64) | (t4 & LOW64);
    mul_end(t6, t5, a, b)
}

/// `mul_qint_41`: a · (upper limb of b), error below 2 ulps.
pub(super) fn mul_qint_41(a: &Qint, b: &Qint) -> Qint {
    let r33 = m(a.hh(), b.hh());
    let r23 = m(a.hl(), b.hh());
    let r13 = m(a.lh(), b.hh());
    let r03 = m(a.ll(), b.hh());
    let t3 = r03 >> 64;
    let (t4, c4) = addc(r13, t3);
    let (t5, c5) = addc(r23, t4 >> 64);
    let t6 = r33.wrapping_add((c5 << 64) | (t5 >> 64)).wrapping_add(c4);
    let t5 = (t5 << 64) | (t4 & LOW64);
    mul_end(t6, t5, a, b)
}

/// `mul_qint_31`: (upper three limbs of a) · (upper limb of b), exact.
pub(super) fn mul_qint_31(a: &Qint, b: &Qint) -> Qint {
    let r33 = m(a.hh(), b.hh());
    let r23 = m(a.hl(), b.hh());
    let r13 = m(a.lh(), b.hh());
    let t4 = r13;
    let (t5, c5) = addc(r23, t4 >> 64);
    let t6 = r33.wrapping_add((c5 << 64) | (t5 >> 64));
    let t5 = (t5 << 64) | (t4 & LOW64);
    mul_end(t6, t5, a, b)
}

/// `mul_qint_22`: the upper two limbs of a and b, exact.
pub(super) fn mul_qint_22(a: &Qint, b: &Qint) -> Qint {
    let r33 = m(a.hh(), b.hh());
    let r32 = m(a.hh(), b.hl());
    let r23 = m(a.hl(), b.hh());
    let r22 = m(a.hl(), b.hl());
    let t4 = r22;
    let (t5, c) = addc(r23, t4 >> 64);
    let mut c5 = c;
    let (t5, c) = addc(r32, t5);
    c5 += c;
    let t6 = r33.wrapping_add((c5 << 64) | (t5 >> 64));
    let t5 = (t5 << 64) | (t4 & LOW64);
    mul_end(t6, t5, a, b)
}

/// `mul_qint_21`: (upper two limbs of a) · (upper limb of b), exact.
pub(super) fn mul_qint_21(a: &Qint, b: &Qint) -> Qint {
    let r33 = m(a.hh(), b.hh());
    let r23 = m(a.hl(), b.hh());
    let t6 = r33.wrapping_add(r23 >> 64);
    let t5 = r23 << 64;
    mul_end(t6, t5, a, b)
}

/// `mul_qint_11`: the upper limbs, exact.
pub(super) fn mul_qint_11(a: &Qint, b: &Qint) -> Qint {
    let t6 = m(a.hh(), b.hh());
    let ex = u64::from(t6 >> 127 == 0);
    Qint {
        rh: t6 << ex,
        rl: 0,
        ex: a.ex + b.ex + 1 - ex as i64,
        sgn: a.sgn ^ b.sgn,
    }
}

/// `mul_qint_2`: the integer b times a, error below 2 ulps.
pub(super) fn mul_qint_2(b: i64, a: &Qint) -> Qint {
    if b == 0 {
        return ZERO_Q;
    }
    let mut c = if b < 0 {
        b.wrapping_neg() as u64
    } else {
        b as u64
    };
    let sgn = u64::from(b < 0) ^ a.sgn;
    if c == 1 {
        let mut r = *a;
        r.sgn = sgn;
        return r;
    }
    let mut ex_r = a.ex + 64;
    let k = c.leading_zeros();
    c <<= k;
    ex_r -= i64::from(k);
    let mut t3 = m(a.hh(), c);
    let mut t2 = m(a.hl(), c);
    let mut t1 = m(a.lh(), c);
    let t0 = m(a.ll(), c);
    let t = t0 >> 64;
    let (s, cy) = addc(t, t1);
    t1 = s;
    let t = (cy << 64) | (t1 >> 64);
    let (s, cy) = addc(t, t2);
    t2 = s;
    t3 = t3.wrapping_add((cy << 64) | (t2 >> 64));
    let ex = ((t3 >> 64) as u64).leading_zeros();
    t2 = (t2 << 64) | (t1 & LOW64);
    let (rh, rl) = if ex != 0 {
        ex_r -= 1;
        ((t3 << 1) | (t2 >> 127), t2 << 1)
    } else {
        (t3, t2)
    };
    Qint {
        rh,
        rl,
        ex: ex_r,
        sgn,
    }
}

/// `qint_fromd`: the non-zero double `b`, exactly.
pub(super) fn qint_fromd(b: f64) -> Qint {
    let u = b.to_bits();
    let e = ((u >> 52) & 0x7ff) as i64;
    let mut hh = (u & (!0u64 >> 12)) + if e != 0 { 1u64 << 52 } else { 0 };
    let t = hh.leading_zeros();
    let ex = e - 0x3ff - if t > 11 { i64::from(t) - 12 } else { 0 };
    hh <<= t;
    Qint {
        rh: (hh as u128) << 64,
        rl: 0,
        ex,
        sgn: u64::from(b < 0.0),
    }
}

/// `qint_toi`: a rounded toward zero to an integer.
pub(super) fn qint_toi(a: &Qint) -> i64 {
    if a.ex < 0 {
        return 0;
    }
    let r = (a.hh() >> (63 - a.ex)) as i64;
    if a.sgn != 0 { r.wrapping_neg() } else { r }
}
