// SPDX-License-Identifier: LicenseRef-FSL AND SunPro
// Copyright 2026 Karel Lopez Vilaret (the translation).
//
// Translated from glibc 2.35's sysdeps/ieee754/dbl-64/s_tanh.c and s_expm1.c, fdlibm's code,
// which carry this notice:
//
// ====================================================
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//
// Developed at SunPro, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
// ====================================================
//
// s_expm1.c: "Modified by Naohiko Shimizu/Tokai University, Japan 1997/08/25, for performance
// improvement on pipelined processors."
//! The C library's `tanh` (double) as FSL's programs call it on x86-64 Linux: glibc 2.35 has no
//! CPU-specific variant of the scalar `tanh` and `expm1` (only the generic fdlibm code, compiled
//! without FMA), so this translation gives its bits on every input (checked against glibc on
//! random inputs by `tools/libm-verify`). BET's surface model calls it 2.5 million times per run.

const ONE: f64 = 1.0;
const TWO: f64 = 2.0;
const TINY: f64 = 1.0e-300;
const HUGE: f64 = 1.0e+300;
const O_THRESHOLD: f64 = 7.097_827_128_933_839_730_96e2;
const LN2_HI: f64 = 6.931_471_803_691_238_164_90e-1;
const LN2_LO: f64 = 1.908_214_929_270_587_700_02e-10;
const INVLN2: f64 = 1.442_695_040_888_963_387_00;
const Q: [f64; 6] = [
    1.0,
    -3.333_333_333_333_313_164_28e-2,
    1.587_301_587_254_814_601_65e-3,
    -7.936_507_578_674_879_424_73e-5,
    4.008_217_827_329_362_395_52e-6,
    -2.010_992_181_836_243_713_26e-7,
];

fn high(x: f64) -> u32 {
    (x.to_bits() >> 32) as u32
}

fn low(x: f64) -> u32 {
    x.to_bits() as u32
}

fn with_high(x: f64, hi: u32) -> f64 {
    f64::from_bits((u64::from(hi) << 32) | (x.to_bits() & 0xffff_ffff))
}

/// C's conversion of a double to `int32_t` on x86-64 (`cvttsd2si`): truncation, and `INT_MIN`
/// for values out of range or NaN.
fn to_i32(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// `expm1(x)`, glibc 2.35's `__expm1`.
pub fn expm1(x: f64) -> f64 {
    let mut x = x;
    let mut hx = high(x);
    let xsb = hx & 0x8000_0000;
    let y = if xsb == 0 { x } else { -x };
    let _ = y;
    hx &= 0x7fff_ffff;

    // filter out huge and non-finite argument
    if hx >= 0x4043_687A {
        if hx >= 0x4086_2E42 {
            if hx >= 0x7ff0_0000 {
                if ((hx & 0xfffff) | low(x)) != 0 {
                    return x + x; // NaN
                }
                return if xsb == 0 { x } else { -1.0 };
            }
            if x > O_THRESHOLD {
                return HUGE * HUGE; // overflow
            }
        }
        if xsb != 0 {
            // x < -56*ln2, return -1.0 with inexact
            return TINY - ONE;
        }
    }

    let (k, c);
    if hx > 0x3fd6_2e42 {
        // |x| > 0.5 ln2
        let (hi, lo);
        if hx < 0x3FF0_A2B2 {
            // and |x| < 1.5 ln2
            if xsb == 0 {
                hi = x - LN2_HI;
                lo = LN2_LO;
                k = 1;
            } else {
                hi = x + LN2_HI;
                lo = -LN2_LO;
                k = -1;
            }
        } else {
            k = to_i32(INVLN2 * x + if xsb == 0 { 0.5 } else { -0.5 });
            let t = f64::from(k);
            hi = x - t * LN2_HI; // t*ln2_hi is exact here
            lo = t * LN2_LO;
        }
        x = hi - lo;
        c = (hi - x) - lo;
    } else if hx < 0x3c90_0000 {
        // when |x|<2**-54, return x
        let t = HUGE + x;
        return x - (t - (HUGE + x));
    } else {
        k = 0;
        c = 0.0;
    }

    // x is now in primary range
    let hfx = 0.5 * x;
    let hxs = x * hfx;
    let r1a = ONE + hxs * Q[1];
    let h2 = hxs * hxs;
    let r2 = Q[2] + hxs * Q[3];
    let h4 = h2 * h2;
    let r3 = Q[4] + hxs * Q[5];
    let r1 = r1a + h2 * r2 + h4 * r3;
    let t = 3.0 - r1 * hfx;
    let mut e = hxs * ((r1 - t) / (6.0 - x * t));
    if k == 0 {
        return x - (x * e - hxs); // c is 0
    }
    e = x * (e - c) - c;
    e -= hxs;
    if k == -1 {
        return 0.5 * (x - e) - 0.5;
    }
    if k == 1 {
        if x < -0.25 {
            return -2.0 * (e - (x + 0.5));
        }
        return ONE + 2.0 * (x - e);
    }
    if k <= -2 || k > 56 {
        // suffice to return exp(x)-1
        let mut y = ONE - (e - x);
        let hi = high(y);
        y = with_high(y, hi.wrapping_add((k as u32) << 20));
        return y - ONE;
    }
    let mut y;
    if k < 20 {
        let t = with_high(ONE, 0x3ff0_0000 - (0x0020_0000 >> k)); // t=1-2^-k
        y = t - (e - x);
        let hi = high(y);
        y = with_high(y, hi.wrapping_add((k as u32) << 20));
    } else {
        let t = with_high(ONE, (0x3ff - k as u32) << 20); // 2^-k
        y = x - (e + t);
        y += ONE;
        let hi = high(y);
        y = with_high(y, hi.wrapping_add((k as u32) << 20));
    }
    y
}

/// `tanh(x)`, glibc 2.35's `__tanh`.
pub fn tanh(x: f64) -> f64 {
    let jx = high(x) as i32;
    let lx = low(x);
    let ix = jx & 0x7fff_ffff;

    // x is INF or NaN
    if ix >= 0x7ff0_0000 {
        return if jx >= 0 { ONE / x + ONE } else { ONE / x - ONE };
    }

    let z;
    if ix < 0x4036_0000 {
        // |x|<22
        if (ix as u32 | lx) == 0 {
            return x; // x == +-0
        }
        if ix < 0x3c80_0000 {
            // |x|<2**-55
            return x * (ONE + x);
        }
        if ix >= 0x3ff0_0000 {
            // |x|>=1
            let t = expm1(TWO * x.abs());
            z = ONE - TWO / (t + TWO);
        } else {
            let t = expm1(-TWO * x.abs());
            z = -t / (t + TWO);
        }
    } else {
        // |x| > 22, return +-1
        z = ONE - TINY;
    }
    if jx >= 0 { z } else { -z }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_values() {
        assert_eq!(tanh(0.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(tanh(-0.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(tanh(f64::INFINITY), 1.0);
        assert_eq!(tanh(f64::NEG_INFINITY), -1.0);
        assert!(tanh(f64::NAN).is_nan());
        assert_eq!(tanh(30.0), 1.0);
        assert_eq!(expm1(0.0), 0.0);
        assert_eq!(expm1(-1000.0), -1.0);
    }

    #[test]
    fn close_to_the_true_values() {
        for &x in &[1e-20, 0.001, 0.3, 0.5, 0.9, 1.0, 1.5, 3.0, 10.0, 21.9, -0.7, -2.2] {
            let want = (f64::exp(2.0 * x) - 1.0) / (f64::exp(2.0 * x) + 1.0);
            let got = tanh(x);
            assert!((got - want).abs() <= 4.0 * f64::EPSILON * want.abs().max(1e-300), "{x}");
            let e = expm1(x);
            assert!((e - (f64::exp(x) - 1.0)).abs() <= 1e-12 * e.abs().max(1e-20), "{x}");
        }
    }
}
