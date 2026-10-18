// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded power of binary64 values: [`pow`].
//!
//! `pow(x, y)` is the binary64 value nearest to the exact x^y (round-to-nearest, ties-to-even),
//! so the result is the same on every platform and equals any other correctly rounded `pow`
//! (glibc's `pow` is not correctly rounded: its error bound is about 0.52 ulp).
//!
//! Port of CORE-MATH's `src/binary64/pow/pow.c` (`cr_pow`) and `pow.h`, with `dint.h` and
//! `qint.h` in [`dint`] and [`qint`], at commit 040ee482a8ca (MIT licence, © CERN and Inria,
//! authors Tom Hubrecht and Paul Zimmermann; the algorithm is that of "Towards a
//! correctly-rounded and fast power function in binary64 arithmetic", ARITH 2023), re-expressed
//! in Rust for round-to-nearest only. The algorithm and the order of every floating-point
//! operation are CORE-MATH's:
//!
//! 1. A double-double approximation of y·log x and of its exponential, with a proven error
//!    bound; if rounding the result plus and minus that bound gives the same double, it is the
//!    correctly rounded x^y.
//! 2. Otherwise 128-bit "dint" arithmetic, a rounding test, and the detection of exact and
//!    midpoint results (Lauter and Lefèvre's rounding-boundary test).
//! 3. Otherwise 256-bit "qint" arithmetic, which is accurate enough for every input.
//!
//! Differences from the C code, none of which changes a result in round-to-nearest: the
//! rounding-mode branches, `errno` and the floating-point exception flags (saved and restored
//! by the C code) are dropped; `__builtin_fma` is `f64::mul_add` (correctly rounded on every
//! target); hex-float constants are written as their exact bit patterns; and where the C code
//! prints "Unexpected worst-case found" and exits (an input that, by the authors' proof, does
//! not exist), the 256-bit result is rounded and returned.
//
// Original copyright and licence (MIT) notice (pow.c and pow.h):
//
//   Copyright (c) 2022-2025 CERN and Inria
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

#![allow(clippy::unreadable_literal, clippy::many_single_char_names)]

mod dint;
mod qint;
mod tables;

use dint::{
    Dint, LOG2, LOG2_INV, M_ONE, add_dint, add_dint_11, cmp_dint_abs, dint_fromd, dint_toi,
    mul_dint, mul_dint_11, mul_dint_21, mul_dint_int64,
};
use qint::{
    LOG2_INV_Q, LOG2_Q, M_ONE_Q, Qint, add_qint, add_qint_22, mul_qint, mul_qint_2, mul_qint_11,
    mul_qint_21, mul_qint_22, mul_qint_31, mul_qint_33, mul_qint_41, qint_fromd, qint_toi,
};
use tables::{
    INVERSE, INVERSE_2_1, INVERSE_2_2, INVERSE_3_1, INVERSE_3_2, LOG_INV, LOG_INV_2_1, LOG_INV_2_2,
    LOG_INV_3_1, LOG_INV_3_2, P_1, P_2, P_3, Q_1, Q_2, Q_3, T1, T1_2, T1_3, T2, T2_2, T2_3,
};

/// `0x1.fffffffffffffp+1023`.
const DBL_MAX: f64 = f64::MAX;
/// `0x1p-1074`.
const TINY: f64 = f64::from_bits(1);

// --- pow.h: utility functions ----------------------------------------------------------------

/// `fast_two_sum`: hi + lo = a + b exactly, assuming |a| >= |b|.
#[inline(always)]
fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let hi = a + b;
    let e = hi - a;
    (hi, b - e)
}

/// `fast_sum`: a + (bh + bl), assuming |a| >= |bh|.
#[inline(always)]
fn fast_sum(a: f64, bh: f64, bl: f64) -> (f64, f64) {
    let (hi, lo) = fast_two_sum(a, bh);
    (hi, lo + bl)
}

/// `a_mul`: hi + lo = a · b exactly.
#[inline(always)]
fn a_mul(a: f64, b: f64) -> (f64, f64) {
    let hi = a * b;
    (hi, a.mul_add(b, -hi))
}

/// `s_mul`: a · (bh + bl).
#[inline(always)]
fn s_mul(a: f64, bh: f64, bl: f64) -> (f64, f64) {
    let (hi, s) = a_mul(a, bh);
    (hi, a.mul_add(bl, s))
}

/// `d_mul`: (ah + al) · (bh + bl) - al · bl.
#[inline(always)]
fn d_mul(ah: f64, al: f64, bh: f64, bl: f64) -> (f64, f64) {
    let (hi, s) = a_mul(ah, bh);
    let t = al.mul_add(bh, s);
    (hi, ah.mul_add(bl, t))
}

/// `is_int`: x is an integer (x finite).
#[inline(always)]
fn is_int(x: f64) -> bool {
    x == x.round_ties_even()
}

/// `extract`: (e, m) with m odd and x = 2^e · m (x non-zero).
#[inline(always)]
fn extract(x: f64) -> (i64, u64) {
    let u = x.to_bits();
    let mut e = ((u >> 52) & 0x7ff) as i64;
    let mut m = (u & (!0u64 >> 12)) + if e != 0 { 1u64 << 52 } else { 0 };
    let t = m.trailing_zeros();
    m >>= t;
    e = e + i64::from(t) - (0x433 - i64::from(e == 0));
    (e, m)
}

/// `round_54`: z rounded to 54 bits, as k · 2^G.
#[inline(always)]
fn round_54(x: &Dint) -> (i64, i64) {
    let g = x.ex - 53;
    let k = (x.hi() >> 10).wrapping_add((x.hi() >> 9) & 0x1) as i64;
    (g, k)
}

/// `pow2`: x · 2^e.
#[inline(always)]
fn pow2(x: f64, e: i64) -> f64 {
    let mut x = x;
    if e & 0x1 != 0 {
        x *= 2.0;
    }
    let e2 = f64::from_bits(((((e >> 1) + 0x3ff) as u64) & 0x7ff) << 52);
    (x * e2) * e2
}

/// `dint_tod_subnormal`: a rounded to nearest, a in the subnormal range.
fn dint_tod_subnormal(a: &Dint) -> f64 {
    let ex = (-(1011 + a.ex)) as u64; // ex >= 12
    if ex >= 64 {
        let rb = a.hi() >> 63;
        let sb = (a.hi() << 1) | a.lo();
        let ret = if ex > 64 || rb == 0 || sb == 0 {
            0.0
        } else {
            TINY
        };
        return if a.sgn != 0 { -ret } else { ret };
    }
    let mut hi = a.hi() >> ex;
    let rb = (a.hi() >> (ex - 1)) & 0x1;
    let sb = u64::from((a.hi() << (65 - ex)) != 0 || a.lo() != 0);
    hi += if sb != 0 { rb } else { hi & rb };
    f64::from_bits(hi | (a.sgn << 63))
}

/// `dint_tod`: a rounded to nearest.
fn dint_tod(a: &Dint) -> f64 {
    if a.ex < -1022 {
        return dint_tod_subnormal(a);
    }
    let mut r = f64::from_bits((a.hi() >> 11) | (0x3ffu64 << 52));
    let mut rd = 0.0;
    if (a.hi() >> 10) & 0x1 != 0 {
        rd += f64::from_bits(0x3ca0000000000000); // 0x1p-53
    }
    if a.hi() & 0x3ff != 0 || a.lo() != 0 {
        rd += f64::from_bits(0x3c90000000000000); // 0x1p-54
    }
    if a.sgn != 0 {
        rd = -rd;
    }
    r = f64::from_bits(r.to_bits() | (a.sgn << 63));
    r += rd;
    let e;
    if a.ex > -1023 {
        if a.ex > 1023 {
            if a.ex == 1024 {
                r *= 2.0;
                e = f64::from_bits(0x7fe0000000000000); // 0x1p+1023
            } else {
                r = DBL_MAX;
                e = DBL_MAX;
            }
        } else {
            e = f64::from_bits((((a.ex + 1023) & 0x7ff) as u64) << 52);
        }
    } else if a.ex < -1074 {
        if a.ex == -1075 {
            r *= 0.5;
            e = TINY;
        } else {
            r = TINY;
            e = TINY;
        }
    } else {
        e = f64::from_bits(1u64 << (a.ex + 1074));
    }
    r * e
}

/// `subnormalize_qint`: round a's significand to the subnormal precision (rounding to
/// nearest).
fn subnormalize_qint(a: &mut Qint) {
    if a.ex > -1023 {
        return;
    }
    let ex = (-(1011 + a.ex)) as u64;
    let hh = a.hh();
    let mut hi = hh.wrapping_shr(ex as u32);
    let md = hh.wrapping_shr((ex - 1) as u32) & 0x1;
    let lo = u64::from(
        (hh & (!0u64).wrapping_shr(ex as u32)) != 0 || a.hl() != 0 || a.lh() != 0 || a.ll() != 0,
    );
    hi += if lo != 0 { md } else { hi & md };
    let hh = hi.wrapping_shl(ex as u32);
    a.rh = (hh as u128) << 64;
    a.rl = 0;
    if hh == 0 {
        a.ex += 1;
        a.rh = 1u128 << 127;
    }
}

/// `qint_tod`: a rounded to nearest.
fn qint_tod(a: &mut Qint) -> f64 {
    subnormalize_qint(a);
    let mut r = f64::from_bits((a.hh() >> 11) | (0x3ffu64 << 52));
    let mut rd = 0.0;
    if a.hh() & 0x400 != 0 {
        rd += f64::from_bits(0x3ca0000000000000); // 0x1p-53
    }
    if a.hh() & 0x3ff != 0 || a.hl() != 0 || a.lh() != 0 || a.ll() != 0 {
        rd += f64::from_bits(0x3c90000000000000); // 0x1p-54
    }
    if a.sgn != 0 {
        rd = -rd;
    }
    r = f64::from_bits(r.to_bits() | (a.sgn << 63));
    r += rd;
    let e;
    if a.ex > -1023 {
        if a.ex > 1023 {
            if a.ex == 1024 {
                r *= 2.0;
                e = f64::from_bits(0x7fe0000000000000);
            } else {
                r = DBL_MAX;
                e = DBL_MAX;
            }
        } else {
            e = f64::from_bits((((a.ex + 1023) & 0x7ff) as u64) << 52);
        }
    } else if a.ex < -1074 {
        if a.ex == -1075 {
            r *= 0.5;
            e = TINY;
        } else {
            r = TINY;
            e = TINY;
        }
    } else {
        e = f64::from_bits(1u64 << (a.ex + 1074));
    }
    r * e
}

// --- polynomial approximations of exp(z) ------------------------------------------------------

/// `q_1`: exp(z) for |z| < 2^-12.905, relative error below 2^-64.902632.
#[inline(always)]
fn q_1(z: f64) -> (f64, f64) {
    let mut q = Q_1[4].mul_add(z, Q_1[3]);
    q = q.mul_add(z, Q_1[2]);
    let h0 = q.mul_add(z, Q_1[1]);
    let (h1, l1) = a_mul(z, h0);
    fast_sum(Q_1[0], h1, l1)
}

/// `q_2`: exp(y) for |y| < 0.00016923, relative error below 2^-122.29.
fn q_2(y: &Dint) -> Dint {
    let mut r = mul_dint_11(y, &Q_2[0]);
    r = add_dint_11(&r, &Q_2[1], &r);
    r = mul_dint_11(y, &r);
    r = add_dint_11(&r, &Q_2[2], &r);
    r = mul_dint_11(y, &r);
    r = add_dint(&Q_2[3], &r);
    r = mul_dint(y, &r);
    r = add_dint(&Q_2[4], &r);
    r = mul_dint(y, &r);
    r = add_dint(&Q_2[5], &r);
    r = mul_dint(y, &r);
    r = add_dint(&Q_2[6], &r);
    r = mul_dint(y, &r);
    add_dint(&Q_2[7], &r)
}

/// `q_3`: exp(y) for |y| < 0.00016923, error below 2^-241.11.
fn q_3(y: &Qint) -> Qint {
    let mut r = mul_qint_11(y, &Q_3[0]);
    r = add_qint_22(&Q_3[1], &r);
    for c in &Q_3[2..7] {
        r = mul_qint_22(y, &r);
        r = add_qint_22(c, &r);
    }
    for c in &Q_3[7..12] {
        r = mul_qint_33(y, &r);
        r = add_qint(c, &r);
    }
    for c in &Q_3[12..15] {
        r = mul_qint(y, &r);
        r = add_qint(c, &r);
    }
    r
}

// --- polynomial approximations of log(1+x) ----------------------------------------------------

/// `p_1`: log(1+z) - z for |z| <= 33·2^-13, absolute error below 2^-75.492.
#[inline(always)]
fn p_1(z: f64) -> (f64, f64) {
    let (wh, wl) = a_mul(z, z);
    let t = P_1[5].mul_add(z, P_1[4]);
    let mut u = P_1[3].mul_add(z, P_1[2]);
    let mut v = P_1[1].mul_add(z, P_1[0]);
    u = t.mul_add(wh, u);
    v = u.mul_add(wh, v);
    u = v * wh;
    (-0.5 * wh, u.mul_add(z, -0.5 * wl))
}

/// `p_2`: log(1+z) for |z| <= 2^-13 (low limb of z zero), relative error below 2^-124.82.
fn p_2(z: &Dint) -> Dint {
    let mut r = mul_dint_11(z, &P_2[0]);
    r = add_dint_11(&r, &P_2[1], &r);
    r = mul_dint_11(z, &r);
    r = add_dint_11(&r, &P_2[2], &r);
    r = mul_dint_11(z, &r);
    r = add_dint_11(&r, &P_2[3], &r);
    r = mul_dint_11(z, &r);
    r = add_dint(&P_2[4], &r);
    r = mul_dint_21(&r, z);
    r = add_dint(&P_2[5], &r);
    r = mul_dint_21(&r, z);
    r = add_dint(&P_2[6], &r);
    r = mul_dint_21(&r, z);
    r = add_dint(&P_2[7], &r);
    r = mul_dint_21(&r, z);
    r = add_dint(&P_2[8], &r);
    mul_dint_21(&r, z)
}

/// `p_3`: log(1+z) for |z| <= 2^-13 (only the upper limb of z non-zero), relative error below
/// 2^-252.66.
fn p_3(z: &Qint) -> Qint {
    let mut r = mul_qint_11(&P_3[0], z);
    r = add_qint_22(&P_3[1], &r);
    for c in &P_3[2..4] {
        r = mul_qint_11(&r, z);
        r = add_qint_22(c, &r);
    }
    for c in &P_3[4..8] {
        r = mul_qint_21(&r, z);
        r = add_qint_22(c, &r);
    }
    for c in &P_3[8..14] {
        r = mul_qint_31(&r, z);
        r = add_qint(c, &r);
    }
    for c in &P_3[14..18] {
        r = mul_qint_41(&r, z);
        r = add_qint(c, &r);
    }
    mul_qint_41(&r, z)
}

// --- logarithms -------------------------------------------------------------------------------

/// `log_1`: h + l ≈ log(x) for 2^-1074 <= x <= DBL_MAX, and whether the result was
/// renormalised for an x close to 1 (`cancel`, which selects the error bound).
#[inline(always)]
fn log_1(x: f64) -> (f64, f64, usize) {
    let xu = x.to_bits();
    let mut m = xu & (!0u64 >> 12);
    let mut e = ((xu >> 52) & 0x7ff) as i64;
    let t;
    if e != 0 {
        t = f64::from_bits(m | (0x3ffu64 << 52));
        m += 1u64 << 52;
        e -= 0x3ff;
    } else {
        let k = m.leading_zeros() - 11;
        e = -0x3fe - i64::from(k);
        m <<= k;
        t = f64::from_bits(m | (0x3ffu64 << 52));
    }
    const CY: [f64; 2] = [1.0, 0.5];
    const CM: [u32; 2] = [44, 45];
    let c = usize::from(m >= 0x16a09e667f3bcd);
    e += c as i64;
    let big_e = e as f64;
    let i = (m >> CM[c]) as usize;
    let t = t * CY[c];
    let r = INVERSE[i - 181];
    let l1 = LOG_INV[i - 181][0];
    let l2 = LOG_INV[i - 181][1];
    let z = r.mul_add(t, -1.0);
    const LOG2_H: f64 = f64::from_bits(0x3fe62e42fefa3800); // 0x1.62e42fefa38p-1
    const LOG2_L: f64 = f64::from_bits(0x3d2ef35793c76730); // 0x1.ef35793c7673p-45
    let th = big_e.mul_add(LOG2_H, l1);
    let tl = big_e.mul_add(LOG2_L, l2);
    let (h, l) = fast_sum(th, z, tl);
    let (ph, pl) = p_1(z);
    let (h, l) = fast_sum(h, ph, l + pl);
    if e == 0 && l.abs() > h.abs() * f64::from_bits(0x3e70000000000000) {
        // 0x1p-24
        let (h, l) = fast_two_sum(h, l);
        return (h, l, 1);
    }
    (h, l, 0)
}

/// `log_2`: log(x), relative error below 2^-122.88.
fn log_2(x: &Dint) -> Dint {
    let mut x = *x;
    let mut big_e = x.ex;
    let i = if x.hi() > 0xb504f333f9de6484 {
        big_e += 1;
        (x.hi() >> (63 + 1 - 7)) as u16 as usize
    } else {
        (x.hi() >> (63 - 7)) as u16 as usize
    };
    x.ex -= big_e;
    let mut z = mul_dint_11(&x, &INVERSE_2_1[i - 90]);
    let j = (z.hi() >> (63 - 13 - z.ex)) as u16 as usize;
    z = mul_dint_11(&z, &INVERSE_2_2[j - 8128]);
    z = add_dint(&M_ONE, &z);
    let mut r = mul_dint_int64(&LOG2, big_e);
    let mut p = p_2(&z);
    p = add_dint(&LOG_INV_2_2[j - 8128], &p);
    p = add_dint(&LOG_INV_2_1[i - 90], &p);
    r = add_dint(&p, &r);
    r
}

/// `log_3`: log(x), relative error below 2^-250.74.
fn log_3(x: &Qint) -> Qint {
    let mut x = *x;
    let mut big_e = x.ex;
    let i = if x.hh() > 0xb504f333f9de6484 {
        big_e += 1;
        (x.hh() >> (63 + 1 - 7)) as u16 as usize
    } else {
        (x.hh() >> (63 - 7)) as u16 as usize
    };
    x.ex -= big_e;
    let mut z = mul_qint(&x, &INVERSE_3_1[i - 90]);
    let j = (z.hh() >> (63 - 13 - z.ex)) as u16 as usize;
    z = mul_qint(&z, &INVERSE_3_2[j - 8128]);
    z = add_qint(&M_ONE_Q, &z);
    let mut r = mul_qint_2(big_e, &LOG2_Q);
    let mut p = p_3(&z);
    p = add_qint(&LOG_INV_3_2[j - 8128], &p);
    p = add_qint(&LOG_INV_3_1[i - 90], &p);
    r = add_qint(&p, &r);
    r
}

// --- exponentials -----------------------------------------------------------------------------

/// `exp_1`: eh + el ≈ s · exp(rh + rl) (relative error below 2^-63.78597), or NaNs when the
/// result may underflow or overflow (deferred to the second phase).
#[inline(always)]
fn exp_1(rh: f64, rl: f64, s: f64) -> (f64, f64) {
    const RHO0: f64 = f64::from_bits(0xc0874910ee4e8a27); // -0x1.74910ee4e8a27p+9
    const RHO1: f64 = f64::from_bits(0xc08483b8cca421af); // -0x1.483b8cca421afp+9
    const RHO2: f64 = f64::from_bits(0x40862e42e709a95b); // 0x1.62e42e709a95bp+9
    const RHO3: f64 = f64::from_bits(0x40862e4316ea5df9); // 0x1.62e4316ea5df9p+9
    if rh.is_nan() || rh > RHO2 {
        if !rh.is_nan() && rh > RHO3 {
            return (DBL_MAX * s, DBL_MAX * s);
        }
        return (f64::NAN, f64::NAN);
    }
    if rh < RHO1 {
        if rh < RHO0 {
            return (0.0 * s, TINY * (0.5 * s));
        }
        return (f64::NAN, f64::NAN);
    }
    const INVLOG2: f64 = f64::from_bits(0x40b71547652b82fe); // 0x1.71547652b82fep+12
    let k = (rh * INVLOG2).round_ties_even();
    const LOG2H: f64 = f64::from_bits(0x3f262e42fefa39ef); // 0x1.62e42fefa39efp-13
    const LOG2L: f64 = f64::from_bits(0x3bbabc9e3b39803f); // 0x1.abc9e3b39803fp-68
    let zh = LOG2H.mul_add(-k, rh);
    let zl = LOG2L.mul_add(-k, rl);
    let big_k = k as i64;
    let big_m = (big_k >> 12) + 0x3ff;
    let i2 = ((big_k >> 6) & 0x3f) as usize;
    let i1 = (big_k & 0x3f) as usize;
    let (t1h, t1l) = (T1[i2][0], T1[i2][1]);
    let (t2h, t2l) = (T2[i1][0], T2[i1][1]);
    let (eh, el) = d_mul(t2h, t2l, t1h, t1l);
    let (qh, ql) = q_1(zh + zl);
    let (eh, el) = d_mul(eh, el, qh, ql);
    let d = f64::from_bits((big_m as u64) << 52) * s;
    (eh * d, el * d)
}

/// `exp_2`: exp(x) for |x| < 744.45, relative error below 2^-121.70 (a value out of range for
/// larger |x|).
fn exp_2(x: &Dint) -> Dint {
    if x.ex >= 10 {
        let mut r = *x;
        r.ex = if x.sgn == 0x1 { -1076 } else { 1025 };
        r.sgn = 0;
        return r;
    }
    let big_k = mul_dint_11(x, &LOG2_INV);
    let k = dint_toi(&big_k);
    let mut big_k = mul_dint_int64(&LOG2, k);
    big_k.ex -= 12;
    big_k.sgn = u64::from(big_k.sgn == 0);
    let y = add_dint(x, &big_k);
    let big_m = k >> 12;
    let i2 = ((k >> 6) & 0x3f) as usize;
    let i1 = (k & 0x3f) as usize;
    let mut r = q_2(&y);
    r = mul_dint(&T1_2[i2], &r);
    r = mul_dint(&T2_2[i1], &r);
    r.ex += big_m;
    r
}

/// `exp_3`: exp(x) for |x| < 744.45, relative error below 2^-241.10.
fn exp_3(x: &Qint) -> Qint {
    let big_k = mul_qint_11(x, &LOG2_INV_Q);
    let k = qint_toi(&big_k);
    let mut big_k = mul_qint_2(k, &LOG2_Q);
    big_k.ex -= 12;
    big_k.sgn = u64::from(big_k.sgn == 0);
    let y = add_qint(x, &big_k);
    let big_m = k >> 12;
    let i2 = ((k >> 6) & 0x3f) as usize;
    let i1 = (k & 0x3f) as usize;
    let mut r = q_3(&y);
    r = mul_qint(&T1_3[i2], &r);
    r = mul_qint(&T2_3[i1], &r);
    r.ex += big_m;
    r
}

// --- exact and midpoint cases -----------------------------------------------------------------

/// `exact_pow`: x^y when it fits in 54 bits (an exact or midpoint case), computed exactly;
/// Lauter and Lefèvre's `detectRoundingBoundaryCase`. `z` approximates x^y with relative error
/// below 2^-117; `exact` is whether x^y is exactly representable.
fn exact_pow(x: f64, y: f64, z: &Dint, exact: bool) -> Option<f64> {
    let s: i64 = if z.sgn != 0 { -1 } else { 1 };
    let (big_e, mut m) = extract(x);
    if m == 1 {
        let g = big_e as f64 * y;
        if is_int(g) {
            let r = if z.sgn != 0 { -1.0 } else { 1.0 };
            return Some(pow2(r, g as i64));
        }
        return None;
    }
    if !(0.0..=34.0).contains(&y) {
        return None;
    }
    let (f, n) = extract(y);
    if n > 34 || f < -5 {
        return None;
    }
    if f < 0 {
        // Case (b): check that E is divisible by 2^-F.
        if (big_e as u64) & (!0u64 >> (64 + f)) != 0 {
            return None;
        }
        let g = ((big_e >> -f) as u64).wrapping_mul(n) as i64;
        let (big_g, k) = round_54(z);
        if !exact {
            let cnt = (k as u64).leading_zeros();
            let mut dd = Dint {
                r: (((k as u64) << cnt) as u128) << 64,
                ex: big_g + 63 - i64::from(cnt),
                sgn: 1 - z.sgn,
            };
            dd = add_dint(z, &dd);
            dd.ex += 116;
            if cmp_dint_abs(&dd, z) >= 0 {
                return None;
            }
        }
        if big_g > g {
            return None;
        }
        let sh = (g - big_g) as u32;
        let ku = k as u64;
        if ku & !((!1u64).wrapping_shl(sh)) == 1u64.wrapping_shl(sh) {
            let r = (k.wrapping_shr(sh)).wrapping_mul(s) as f64;
            return Some(pow2(r, g));
        }
        return None;
    }
    // Case (a): no overflow in n << F since 2^F · n <= 34.
    let mut t = n << f;
    let mut k: i64 = 1;
    while t != 0 {
        if t & 0x1 != 0 {
            let p = i128::from(m) * i128::from(k);
            k = i64::try_from(p).ok()?;
        }
        t >>= 1;
        if t != 0 {
            m = m.checked_mul(m)?;
        }
    }
    if k >> 54 != 0 {
        return None;
    }
    let r = k.wrapping_mul(s) as f64;
    let big_g = (big_e as u64).wrapping_mul(n << f) as i64;
    Some(pow2(r, big_g))
}

/// `is_exact`: x^y is exact and exactly representable (x > 0 finite, y finite).
fn is_exact(x: f64, y: f64) -> bool {
    let v = x.to_bits();
    let w = y.to_bits();
    if (v << 1) != 0x7fe0000000000000 && (w << 22) != 0 {
        return false;
    }
    if (v << 1) == 0x7fe0000000000000 {
        return true; // |x| = 1
    }
    // xmax[y] for 1 <= y <= 33: the largest odd m such that m^y fits in 53 bits.
    const XMAX: [u64; 34] = [
        0,
        0xffffffffffffffff,
        94906265,
        208063,
        9741,
        1551,
        455,
        189,
        97,
        59,
        39,
        27,
        21,
        15,
        13,
        11,
        9,
        7,
        7,
        5,
        5,
        5,
        5,
        4,
        3,
        3,
        3,
        3,
        3,
        3,
        3,
        3,
        3,
        3,
    ];
    let split = |u: u64| -> (u64, i64) {
        let mut m = u & 0xfffffffffffff;
        let mut e = ((u << 1) >> 53) as i64 - 0x433;
        if e >= -1074 {
            m |= 0x10000000000000;
        } else {
            e += 1;
        }
        let t = m.trailing_zeros();
        (m >> t, e + i64::from(t))
    };
    if y >= 0.0 && is_int(y) {
        let (m, e) = split(v);
        if y == 0.0 || y == 1.0 {
            return true;
        }
        if m == 1 {
            let ye = y * e as f64;
            return (-1074.0..1024.0).contains(&ye);
        }
        if y < 0.0 || 33.0 < y {
            return false;
        }
        let y_int = y as i32;
        if m > XMAX[y_int as usize] {
            return false;
        }
        // m^y is computed but, as in the C code, only m's bit length is used below.
        let t = 64 - i64::from(m.leading_zeros());
        let ez = e * i64::from(y_int) + t;
        if ez <= -1074 || 1024 < ez {
            return false;
        }
        return e * i64::from(y_int) >= -1074;
    }
    let (n, mut f) = split(w);
    let (mut m, mut e) = split(v);
    if y < 0.0 {
        if m != 1 {
            return false;
        }
        let ez = if f >= 0 {
            if f < 12 {
                (n.wrapping_neg().wrapping_mul(e as u64) << f) as i64
            } else {
                1024
            }
        } else {
            let t = (e as u64).trailing_zeros();
            if -f > i64::from(t) {
                return false;
            }
            ((-e >> (-f)) as u64).wrapping_mul(n) as i64
        };
        return (-1074..1024).contains(&ez);
    }
    // y > 0 not an integer: y = n · 2^f with n odd and f < 0.
    while f != 0 {
        f += 1;
        if e & 1 != 0 {
            return false;
        }
        e /= 2;
        let dm = m as f64;
        let s = dm.sqrt().round();
        if s * s != dm {
            return false;
        }
        m = s as u64;
    }
    if m > 1 {
        if 33 < n {
            return false;
        }
        if m > XMAX[n as usize] {
            return false;
        }
    }
    let mut my = m;
    let mut n0 = n;
    while n0 > 1 {
        n0 -= 1;
        my = my.wrapping_mul(m);
    }
    let t = 64 - i64::from(my.leading_zeros());
    let en = e * i64::from(n as i32);
    -1074 <= en && en + t <= 1024
}

// --- the power function -----------------------------------------------------------------------

/// `x^y`, correctly rounded (CORE-MATH's `cr_pow`).
#[inline(always)]
pub(super) fn pow(x: f64, y: f64) -> f64 {
    let mut s = 1.0; // the sign of the result
    let x0 = x;
    let mut x = x;
    let xu = x.to_bits();
    let yu = y.to_bits();

    if xu >= 0x7ff0000000000000 || yu >= 0x7ff0000000000000 {
        let is_signaling = |v: f64| v.to_bits() & (1u64 << 51) == 0;
        if x.is_nan() {
            if y == 0.0 && !is_signaling(x) {
                return 1.0;
            }
            return x + x;
        }
        if y.is_nan() {
            if x == 1.0 && !is_signaling(y) {
                return 1.0;
            }
            return y + y;
        }
        match xu {
            0x7ff0000000000000 => {
                if y == 0.0 {
                    return 1.0;
                }
                if y < 0.0 {
                    return 0.0;
                }
                if y > 0.0 {
                    return f64::INFINITY;
                }
            }
            0xfff0000000000000 => {
                if yu == 0x7ff0000000000000 {
                    return y;
                }
                if yu == 0xfff0000000000000 {
                    return 0.0;
                }
                if is_int(y) && !is_int(y * 0.5) {
                    return if y < 0.0 { -0.0 } else { f64::NEG_INFINITY };
                }
                if y < 0.0 {
                    return 0.0;
                }
                if y > 0.0 {
                    return f64::INFINITY;
                }
            }
            _ => {}
        }
        match yu {
            0x7ff0000000000000 => {
                if x == 0.0 {
                    return 0.0;
                }
                if x == -1.0 || x == 1.0 {
                    return 1.0;
                }
                if -1.0 < x && x < 1.0 {
                    return 0.0;
                }
                if x < -1.0 || 1.0 < x {
                    return f64::INFINITY;
                }
            }
            0xfff0000000000000 => {
                if x == 0.0 {
                    return f64::INFINITY;
                }
                if x == -1.0 || x == 1.0 {
                    return 1.0;
                }
                if -1.0 < x && x < 1.0 {
                    return f64::INFINITY;
                }
                if x < -1.0 || 1.0 < x {
                    return 0.0;
                }
            }
            _ => {}
        }
    }
    // From now on, x and y are finite.

    if x <= 0.0 {
        if y == 0.0 {
            return 1.0;
        }
        match xu {
            0x0 => {
                if is_int(y) && !is_int(y * 0.5) {
                    return if y < 0.0 { f64::INFINITY } else { 0.0 };
                }
                return if y > 0.0 { 0.0 } else { f64::INFINITY };
            }
            0x8000000000000000 => {
                if is_int(y) && !is_int(y * 0.5) {
                    return if y < 0.0 { f64::NEG_INFINITY } else { -0.0 };
                }
                return if y > 0.0 { 0.0 } else { f64::INFINITY };
            }
            _ => {}
        }
        if !is_int(y) {
            return f64::NAN;
        }
        // The sign: +1 for y even, -1 for y odd.
        let y_parity = if y.abs() >= f64::from_bits(0x4340000000000000) {
            0 // |y| >= 2^53
        } else {
            (y as i64) & 0x1
        };
        s = if y_parity != 0 { -1.0 } else { 1.0 };
        x = -x;
    }

    // Phase 1 (Algorithm phase_1 of the ARITH 2023 paper).
    let (mut lh, mut ll, cancel) = log_1(x);
    // Avoid a spurious underflow or overflow in y · log(x).
    let ey = (yu >> 52) & 0x7ff;
    if !(0x36..0x7f5).contains(&ey) {
        lh = f64::NAN;
        ll = f64::NAN;
    }
    let (rh, rl) = s_mul(y, lh, ll);
    let (res_h, res_l) = exp_1(rh, rl, s);
    const ERR: [f64; 2] = [
        f64::from_bits(0x3bf2700000000000), // 0x1.27p-64 > 2^-63.797
        f64::from_bits(0x3c55700000000000), // 0x1.57p-58 > 2^-57.579
    ];
    let res_min = res_h + ERR[cancel].mul_add(-res_h, res_l);
    let res_max = res_h + ERR[cancel].mul_add(res_h, res_l);
    if res_min == res_max {
        return res_max;
    }
    // (The C code computes `exact` before the test, for its exception flags.)
    let exact = is_exact(x, y);

    // Easy cases.
    if y == 1.0 {
        return s * x;
    }
    if y == 2.0 {
        return x * x;
    }
    if y == 0.5 {
        return x.sqrt();
    }
    if y == 0.0 {
        return 1.0;
    }

    // Phase 2: 128-bit arithmetic.
    let mut big_x = dint_fromd(x);
    let big_y = dint_fromd(y);
    big_x.sgn = 0x0;
    let mut r = log_2(&big_x);
    r = mul_dint_21(&r, &big_y);
    r = exp_2(&r);
    if r.ex < -1075 {
        return 0.5 * (s * TINY);
    }
    let rd = if r.ex < -1022 {
        // The subnormal case: 2^-1075 <= R < 2^-1022.
        let ex = (-(1022 + r.ex)) as u64; // 1 <= ex <= 53
        let m = (r.lo() >> (10 + ex)) | (r.hi() << (54 - ex));
        m.wrapping_add(14) > 2 * 14
    } else {
        const ERR_BND_2: u64 = 28;
        let lo = (r.lo() >> 10) | (r.hi() << 54);
        lo.wrapping_add(ERR_BND_2) > 2 * ERR_BND_2
    };
    r.sgn = u64::from(s == -1.0);
    if rd {
        return dint_tod(&r);
    }
    if let Some(e) = exact_pow(x0, y, &r, exact) {
        return e;
    }

    // Phase 3: 256-bit arithmetic, for the hard-to-round cases.
    let mut q_x = qint_fromd(x);
    let q_y = qint_fromd(y);
    q_x.sgn = 0x0;
    let mut q_r = log_3(&q_x);
    q_r = mul_qint_41(&q_r, &q_y);
    let mut q_z = exp_3(&q_r);
    const ERR_BND_3: u64 = 60;
    let r1 = (q_z.hh() << 54) | (q_z.hl() >> 10);
    let r2 = (q_z.hl() << 54) | (q_z.lh() >> 10);
    let r3 = (q_z.lh() << 54) | (q_z.ll() >> 10);
    let rd = !((r1 == 0 && r2 == 0 && r3 <= ERR_BND_3)
        || (!r1 == 0 && !r2 == 0 && r3.wrapping_add(2 * ERR_BND_3) <= ERR_BND_3));
    if rd || q_r.ex >= -56 {
        // (With `!rd`, the C code reports an unexpected worst case and exits: proven not to
        // happen; the rounded 256-bit value is returned instead.)
        q_z.sgn = u64::from(s == -1.0);
        q_z.rl &= !0u128 << 10;
        return qint_tod(&mut q_z);
    }
    // x^y very close to 1: exp(qR) rounds to the neighbour of 1 or to 1.
    if q_r.sgn == 0x0 {
        1.0 + f64::from_bits(0x39b0000000000000) // 1 + 0x1p-100
    } else {
        1.0 - f64::from_bits(0x39b0000000000000)
    }
}

#[cfg(test)]
mod tests {
    use super::pow;

    #[test]
    fn special_values() {
        assert_eq!(pow(2.0, 10.0), 1024.0);
        assert_eq!(pow(2.0, -1074.0), f64::from_bits(1));
        assert_eq!(pow(2.0, -1075.0), 0.0);
        assert_eq!(pow(-2.0, 3.0), -8.0);
        assert!(pow(-8.0, 1.0 / 3.0).is_nan());
        assert_eq!(pow(4.0, 0.5), 2.0);
        assert_eq!(pow(0.0, -1.0), f64::INFINITY);
        assert_eq!(pow(-0.0, -1.0), f64::NEG_INFINITY);
        assert_eq!(pow(f64::NAN, 0.0), 1.0);
        assert_eq!(pow(1.0, f64::NAN), 1.0);
        assert_eq!(pow(10.0, 309.0), f64::INFINITY);
        assert_eq!(pow(9.0, 1.5), 27.0);
    }

    /// Inputs that need the second phase (results in the subnormal range) or the third
    /// (CORE-MATH's worst cases), with the results of CORE-MATH's C code (gcc 11.4, `-O2`).
    #[test]
    fn hard_cases_match_the_c_code() {
        let cases: [(u64, u64, u64); 18] = [
            (0x378c8c7382b101c7, 0x401f000000000000, 0x00000004abf7ac04),
            (0x3fe1149421fd7c4d, 0x4091a40000000000, 0x000ac5bc4ec65c38),
            (0x3fe03ef225410da4, 0x40908bd82b116e12, 0x00000050eab72b5f),
            (0x3fe612975946bdc2, 0x409daffb1bdbed79, 0x004f0ace75228edb),
            (0x3fed77b3c5b7aa4c, 0x40c1421d0d7eefa2, 0x00000000007f5cc4),
            (0x3feaefa1610a26d5, 0x40b00333b964cb26, 0x00428816f9c9f436),
            (0x3fe399c9ffd0bb64, 0x40972d3fd2c058e4, 0x000000000213ed07),
            (0x3fe5a05195fdaf3f, 0x409cefca909459ef, 0x0000000008a99a3e),
            (0x4d591b2d728c0c5e, 0xbc1b87a3765ff8d9, 0x3fefffffffffffff),
            (0x1397d0f88599a437, 0x3c3f3c79690c04b4, 0x3feffffffffffff8),
            (0x03708ea248ba2314, 0x3c47a8195111afa7, 0x3feffffffffffff0),
            (0x6719015f86e7ee6c, 0x3c2c4a4518d51c91, 0x3ff0000000000001),
            (0x193156ec9d38380b, 0x3c27d50acebd9404, 0x3feffffffffffffd),
            (0x7656b13de5c87ff7, 0xbc2e8c2b3716c169, 0x3feffffffffffffb),
            (0x77ec6f1a810d082d, 0xbc2716673d7b4444, 0x3feffffffffffffc),
            (0x7803d5a7faa7472a, 0xbc321c3a092f2c46, 0x3feffffffffffffb),
            (0x2039b7208bdd88dc, 0x3c3a35536e38e67d, 0x3feffffffffffffc),
            (0x79368e513d73ebad, 0x3c4ea064414daba9, 0x3ff0000000000009),
        ];
        for (x, y, r) in cases {
            let got = pow(f64::from_bits(x), f64::from_bits(y));
            assert_eq!(got.to_bits(), r, "pow({x:#x}, {y:#x})");
        }
    }
}
