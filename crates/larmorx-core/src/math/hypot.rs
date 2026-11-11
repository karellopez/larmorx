// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded Euclidean distance for binary64 (`hypot`).
//!
//! Port of CORE-MATH `src/binary64/hypot/hypot.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), re-expressed in Rust.
//! Round-to-nearest only (`op == om`): the rounding-mode detection, the floating-point flag
//! handling (`get_flags`/`set_flags`, the underflow trigger) and `errno` of the C file are dropped,
//! with the code that only directed roundings reach. The operations and their order are kept
//! exactly; `__builtin_fma` is [`f64::mul_add`], `__builtin_sqrt` is [`f64::sqrt`] (both
//! correctly rounded), and the C's unsigned 64-bit arithmetic, which wraps, is written with
//! wrapping operations (the squares of 53- to 57-bit integers overflow; only their small
//! differences are used).
//!
//! Original licence of hypot.c (MIT):
//!
//! Copyright (c) 2022-2026 Alexei Sibidanov <sibid@uvic.ca>.
//!
//! This file is part of the CORE-MATH project
//! (<https://core-math.gitlabpages.inria.fr/>).
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

#![allow(clippy::unreadable_literal, clippy::manual_range_contains)]

/// `fasttwosum(x, y)`: `(s, e)` with `s + e = x + y` exactly (when `|x| >= |y|`).
#[inline(always)]
fn fasttwosum(x: f64, y: f64) -> (f64, f64) {
    let s = x + y;
    let z = s - x;
    (s, y - z)
}

/// Both `x` and `y` subnormal: `a` and `b` their encodings, `2^52 > a >= b > 0`.
#[cold]
#[inline(never)]
fn as_hypot_denorm(a: u64, b: u64) -> f64 {
    let af = a as i64 as f64;
    let bf = b as i64 as f64;
    // af and bf are x and y multiplied by 2^1074, thus integers.
    let a = a << 1;
    let b = b << 1;
    let rm = (af * af + bf * bf).sqrt() as u64;
    let mut tm = (rm << 1) as i64;
    // D = a^2 + b^2 - tm^2 (the wrapping arithmetic of the C's u64, then signed).
    let mut d = a
        .wrapping_mul(a)
        .wrapping_add(b.wrapping_mul(b))
        .wrapping_sub((tm as u64).wrapping_mul(tm as u64)) as i64;
    while d > 2 * tm {
        // tm too small: (tm + 1)^2 = tm^2 + 2 tm + 1.
        d -= 2 * tm + 1;
        tm += 1;
    }
    while d < 0 {
        // tm too large: (tm - 1)^2 = tm^2 - 2 tm + 1.
        d += 2 * tm - 1;
        tm -= 1;
    }
    // tm = floor(sqrt(a^2 + b^2)), 0 <= D < 2 tm + 1; no midpoint can occur here.
    let rb = (tm & 1) as u64; // the round bit for rm
    let sb = d != 0; // the sticky bit for rm
    let mut rm = (tm >> 1) as u64; // truncate the low bit
    if sb {
        // Rounding to nearest.
        rm += rb;
    }
    f64::from_bits(rm)
}

/// The square root refined exactly: `x^2 + y^2` fits in 128 bits (here mod 2^64, which is
/// enough for the small differences compared), the approximation is squared and adjusted.
#[cold]
#[inline(never)]
fn as_hypot_hard(x: f64, y: f64) -> f64 {
    let xi = x.to_bits();
    let yi = y.to_bits();
    let mut bm = (xi & (!0u64 >> 12)) | 1 << 52;
    let mut lm = (yi & (!0u64 >> 12)) | 1 << 52;
    let be = (xi >> 52) as i32;
    let le = (yi >> 52) as i32;
    let ri = (x * x + y * y).sqrt().to_bits();
    const BS: i32 = 2;
    let mut rm = ri & (!0u64 >> 12);
    let mut re = (ri >> 52) as i32 - 0x3ff;
    rm |= 1 << 52;
    for _ in 0..3 {
        if rm == 1 << 52 {
            rm = !0u64 >> 11;
            re -= 1;
        } else {
            rm -= 1;
        }
    }
    bm <<= BS;
    let mut m2 = bm.wrapping_mul(bm);
    let de = be - le;
    let mut ls = BS - de;
    if ls >= 0 {
        lm <<= ls;
        m2 = m2.wrapping_add(lm.wrapping_mul(lm));
    } else {
        let lm2 = u128::from(lm) * u128::from(lm);
        ls *= 2;
        m2 = m2.wrapping_add((lm2 >> -ls) as u64);
        m2 |= u64::from((lm2 << (128 + ls)) != 0);
    }
    let k = BS + re;
    let mut d: i64;
    loop {
        rm += 1 + u64::from(rm >= 1 << 53);
        let tm = rm << k;
        let rm2 = tm.wrapping_mul(tm);
        d = m2.wrapping_sub(rm2) as i64;
        if d <= 0 {
            break;
        }
    }
    if d != 0 {
        // Rounding to nearest.
        let tm = (rm << k).wrapping_sub(1u64 << (k - i32::from(rm <= 1 << 53)));
        d = m2.wrapping_sub(tm.wrapping_mul(tm)) as i64;
        if d != 0 {
            rm = rm.wrapping_add((d >> 63) as u64);
        } else {
            rm -= rm & (1u64 << (1 - i32::from(rm <= 1 << 53)));
        }
    }
    if rm >= 1 << 53 {
        rm >>= 1;
        re += 1;
    }
    let e = i64::from(be) - 1 + i64::from(re);
    f64::from_bits(((e << 52) as u64).wrapping_add(rm))
}

/// `sqrt(x^2 + y^2)`, correctly rounded (CORE-MATH's `cr_hypot`).
pub(super) fn hypot(x: f64, y: f64) -> f64 {
    let xi = x.to_bits();
    let yi = y.to_bits();
    let emsk: u64 = 0x7ff << 52;
    let mut ex = xi & emsk;
    let mut ey = yi & emsk;
    // emsk corresponds to the upper bits of NaN and Inf (apart from the sign bit).
    let x = x.abs();
    let y = y.abs();
    if ex == emsk || ey == emsk {
        // Either x or y is NaN or Inf.
        let wx = xi << 1;
        let wy = yi << 1;
        let wm = emsk << 1;
        let ninf = (wx == wm) ^ (wy == wm);
        let nqnn = ((wx >> 52) == 0xfff) ^ ((wy >> 52) == 0xfff);
        // ninf: only one of x and y is ±Inf; nqnn: only one is a quiet NaN. IEEE 754 says
        // hypot(±Inf, qNaN) = hypot(qNaN, ±Inf) = +Inf.
        if ninf && nqnn {
            return if wx == wm { x * x } else { y * y };
        }
        return x + y; // inf, nan
    }
    // Both x and y are finite.
    let u = if x > y { x } else { y };
    let v = if x > y { y } else { x };
    let mut xd = u.to_bits();
    let mut yd = v.to_bits();
    ey = yd;
    if ey >> 52 == 0 {
        // y is subnormal.
        if ey == 0 {
            return f64::from_bits(xd);
        }
        ex = xd;
        if ex >> 52 == 0 {
            // x is subnormal too (x = 0 would mean y = 0, tested above).
            return as_hypot_denorm(ex, ey);
        }
        let nz = i64::from(ey.leading_zeros());
        ey <<= nz - 11;
        ey &= !0u64 >> 12;
        ey = ey.wrapping_sub(((nz - 12) << 52) as u64);
        yd = ey;
    }
    let de = xd.wrapping_sub(yd);
    if de > (27u64 << 52) {
        return f64::from_bits(0x3e40_0000_0000_0000).mul_add(v, u); // fma(2^-27, v, u)
    }
    let off = ((0x3ffi64) << 52) - (xd & emsk) as i64;
    xd = xd.wrapping_add(off as u64);
    yd = yd.wrapping_add(off as u64);
    let x = f64::from_bits(xd);
    let y = f64::from_bits(yd);
    let x2 = x * x;
    let dx2 = x.mul_add(x, -x2);
    let y2 = y * y;
    let dy2 = y.mul_add(y, -y2);
    let r2 = x2 + y2;
    let ir2 = 0.5 / r2;
    let dr2 = ((x2 - r2) + y2) + (dx2 + dy2);
    let th = r2.sqrt();
    let rsqrt = th * ir2;
    let dz = dr2 - th.mul_add(th, -r2);
    let tl = rsqrt * dz;
    let (th, tl) = fasttwosum(th, tl);
    let mut thd = th.to_bits();
    let tld = tl.abs().to_bits();
    ex = thd;
    ey = tld;
    ex &= 0x7ff << 52;
    let aidr = ey.wrapping_add(0x3fe << 52).wrapping_sub(ex);
    let mid = (aidr.wrapping_sub(0x3c90_0000_0000_0000).wrapping_add(16)) >> 5;
    let midm = (aidr.wrapping_sub(0x3c80_0000_0000_0000).wrapping_add(16)) >> 5;
    if mid == 0 || midm == 0 || aidr < 0x39b0_0000_0000_0000 || aidr > 0x3c9f_ffff_ffff_ff80 {
        thd = as_hypot_hard(x, y).to_bits();
    }
    thd = thd.wrapping_sub(off as u64);
    if thd >= 0x7ff << 52 {
        // hypot(x, y) >= 2^1024.
        return f64::INFINITY;
    }
    f64::from_bits(thd)
}

#[cfg(test)]
mod tests {
    use super::hypot;

    /// Exact cases (Pythagorean triples at every scale, subnormal ones too), zeros, signs and
    /// the special values.
    #[test]
    fn exact_and_special() {
        for &(a, b, c) in &[(3.0, 4.0, 5.0), (5.0, 12.0, 13.0), (20.0, 21.0, 29.0)] {
            for s in [-1074, -1060, -600, -1, 0, 1, 600, 1000] {
                let f = 2f64.powi(s);
                if (c * f).is_finite() && a * f > 0.0 {
                    assert_eq!(hypot(a * f, -b * f), c * f, "{a} {b} 2^{s}");
                }
            }
        }
        assert_eq!(hypot(0.0, -0.0).to_bits(), 0);
        assert_eq!(hypot(-7.5, 0.0), 7.5);
        assert_eq!(hypot(f64::INFINITY, f64::NAN), f64::INFINITY);
        assert_eq!(hypot(f64::NAN, f64::NEG_INFINITY), f64::INFINITY);
        assert!(hypot(f64::NAN, 1.0).is_nan());
        assert_eq!(hypot(f64::MAX, f64::MAX), f64::INFINITY);
        assert_eq!(hypot(1.0, 1e-300), 1.0);
        assert_eq!(
            hypot(f64::MIN_POSITIVE, f64::from_bits(1)),
            f64::MIN_POSITIVE
        );
    }
}
