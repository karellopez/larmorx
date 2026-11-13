// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-fdlibm
// Translated to Rust from fdlibm's float functions as the GNU C Library 2.35 ships them,
// sysdeps/ieee754/flt-32/s_tanhf.c and s_expm1f.c ("float version of s_tanh.c", "float version of
// s_expm1.c"), which carry only Sun's notice (reproduced below and in
// LICENSES/LicenseRef-fdlibm.txt and NOTICE). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the originals.
//! The single-precision hyperbolic tangent and `exp(x) - 1` with the bits of glibc 2.35's `tanhf`
//! and `expm1f` on x86-64, on every platform.
//!
//! glibc 2.35 has no FMA or other CPU-specific variant of these two on x86-64: its generic fdlibm
//! code, compiled for baseline x86-64 (no fused multiply-adds), is what runs (`objdump -d` of
//! Ubuntu 22.04's `libm.so.6`: no FMA instruction in either). Every operation here is one IEEE
//! single-precision operation rounded to nearest, in the C code's order (plain `*`, `+`, `/`;
//! never `mul_add`); the constants are fdlibm's decimal literals as C rounds them (to double, then
//! to float); the one NaN path uses SSE's rules (`super::x86_nan`). Not correctly rounded:
//! [`super::tanhf`] is larmorx's default; these serve `reproduce` where an original's result
//! depends on glibc's last bits. Proved equal to the system's `tanhf` and `expm1f` (glibc 2.35,
//! x86-64) on all 2^32 inputs, NaN bits included (`validation/investigations/glibc-maths/`).

// ====================================================
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//
// Developed at SunPro, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
// ====================================================

// fdlibm's constants as written (C rounds each decimal literal to double, then to float).
#![allow(clippy::excessive_precision)]

use super::x86_nan::{add_f32, div_f32, sub_f32};

const ONE: f32 = 1.0;
const TWO: f32 = 2.0;
const HUGE: f32 = 1.0e+30_f64 as f32;
const TINY: f32 = 1.0e-30_f64 as f32;

/// glibc 2.35's `tanhf(x)` (`__tanhf`), with its bits on every platform: `x` for `±0`,
/// `x·(1 + x)` below `2^-55`, `±(1 - 1e-30)` (that is `±1`) from 22, `±1` for `±∞`, a NaN
/// quieted.
#[must_use]
pub fn tanhf_glibc(x: f32) -> f32 {
    let jx = x.to_bits() as i32;
    let ix = jx & 0x7fff_ffff;
    // x is INF or NaN.
    if ix >= 0x7f80_0000 {
        return if jx >= 0 {
            add_f32(div_f32(ONE, x), ONE) // tanh(+inf) = +1
        } else {
            sub_f32(div_f32(ONE, x), ONE) // tanh(-inf) = -1; tanh(NaN) = NaN
        };
    }
    let z;
    if ix < 0x41b0_0000 {
        // |x| < 22
        if ix == 0 {
            return x; // x == +-0
        }
        if ix < 0x2400_0000 {
            // |x| < 2^-55 (math_check_force_underflow only raises the underflow flag).
            return x * (ONE + x);
        }
        if ix >= 0x3f80_0000 {
            // |x| >= 1
            let t = expm1f_glibc(TWO * x.abs());
            z = ONE - TWO / (t + TWO);
        } else {
            let t = expm1f_glibc(-TWO * x.abs());
            z = -t / (t + TWO);
        }
    } else {
        // |x| >= 22: +-1 (with the inexact flag raised).
        z = ONE - TINY;
    }
    if jx >= 0 { z } else { -z }
}

/// glibc 2.35's `expm1f(x)` (`__expm1f`), with its bits on every platform: `x` below `2^-25`,
/// `-1` below `-27·ln 2` and for `-∞`, `+∞` above `88.7216796875` (`o_threshold`) and for `+∞`,
/// a NaN quieted (`x + x`). `errno` is not modelled.
#[must_use]
pub fn expm1f_glibc(x: f32) -> f32 {
    const O_THRESHOLD: f32 = 8.8721679688e+01_f64 as f32; // 0x42b17180
    const LN2_HI: f32 = 6.9313812256e-01_f64 as f32; // 0x3f317180
    const LN2_LO: f32 = 9.0580006145e-06_f64 as f32; // 0x3717f7d1
    const INVLN2: f32 = 1.4426950216e+00_f64 as f32; // 0x3fb8aa3b
    // Scaled coefficients related to expm1.
    const Q1: f32 = -3.3333335072e-02_f64 as f32; // 0xbd088889
    const Q2: f32 = 1.5873016091e-03_f64 as f32; // 0x3ad00d01
    const Q3: f32 = -7.9365076090e-05_f64 as f32; // 0xb8a670cd
    const Q4: f32 = 4.0082177293e-06_f64 as f32; // 0x36867e54
    const Q5: f32 = -2.0109921195e-07_f64 as f32; // 0xb457edbb

    let mut x = x;
    let mut hx = x.to_bits();
    let xsb = hx & 0x8000_0000; // sign bit of x
    hx &= 0x7fff_ffff; // |x|'s bits

    // Filter out huge and non-finite arguments.
    if hx >= 0x4195_b844 {
        // |x| >= 27·ln2
        if hx >= 0x42b1_7218 {
            // |x| >= 88.721...
            if hx > 0x7f80_0000 {
                return add_f32(x, x); // NaN
            }
            if hx == 0x7f80_0000 {
                return if xsb == 0 { x } else { -1.0 }; // exp(+-inf) - 1 = {inf, -1}
            }
            if x > O_THRESHOLD {
                return HUGE * HUGE; // overflow (glibc sets errno to ERANGE)
            }
        }
        if xsb != 0 {
            // x < -27·ln2: -1 with the inexact flag raised.
            return TINY - ONE;
        }
    }

    // Argument reduction.
    let k: i32;
    let mut c = 0.0f32;
    if hx > 0x3eb1_7218 {
        // |x| > 0.5·ln2
        let (hi, lo);
        if hx < 0x3f85_1592 {
            // and |x| < 1.5·ln2
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
            // C's conversion to int truncates; |x| < 88.73 keeps it in range.
            k = (INVLN2 * x + if xsb == 0 { 0.5f32 } else { -0.5f32 }) as i32;
            let t = k as f32;
            hi = x - t * LN2_HI; // t·ln2_hi is exact here
            lo = t * LN2_LO;
        }
        x = hi - lo;
        c = (hi - x) - lo;
    } else if hx < 0x3300_0000 {
        // |x| < 2^-25: x (with the inexact flag raised when x != 0).
        let t = HUGE + x;
        return x - (t - (HUGE + x));
    } else {
        k = 0;
    }

    // x is now in the primary range.
    let hfx = 0.5f32 * x;
    let hxs = x * hfx;
    let r1 = ONE + hxs * (Q1 + hxs * (Q2 + hxs * (Q3 + hxs * (Q4 + hxs * Q5))));
    let t = 3.0f32 - r1 * hfx;
    let mut e = hxs * ((r1 - t) / (6.0f32 - x * t));
    if k == 0 {
        return x - (x * e - hxs); // c is 0
    }
    e = x * (e - c) - c;
    e -= hxs;
    if k == -1 {
        return 0.5f32 * (x - e) - 0.5f32;
    }
    if k == 1 {
        return if x < -0.25f32 {
            -2.0f32 * (e - (x + 0.5f32))
        } else {
            ONE + 2.0f32 * (x - e)
        };
    }
    // Adds k to y's exponent.
    let add_exponent = |y: f32| f32::from_bits((y.to_bits() as i32).wrapping_add(k << 23) as u32);
    if k <= -2 || k > 56 {
        // exp(x) - 1 is enough.
        let y = add_exponent(ONE - (e - x));
        return y - ONE;
    }
    if k < 23 {
        let t = f32::from_bits((0x3f80_0000 - (0x0100_0000 >> k)) as u32); // 1 - 2^-k
        add_exponent(t - (e - x))
    } else {
        let t = f32::from_bits(((0x7f - k) << 23) as u32); // 2^-k
        let mut y = x - (e + t);
        y += ONE;
        add_exponent(y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The constants as C rounds fdlibm's literals (the hex values of its comments).
    #[test]
    fn constants() {
        let bits = |v: f64| (v as f32).to_bits();
        assert_eq!(bits(8.8721679688e+01), 0x42b1_7180);
        assert_eq!(bits(6.9313812256e-01), 0x3f31_7180);
        assert_eq!(bits(9.0580006145e-06), 0x3717_f7d1);
        assert_eq!(bits(1.4426950216e+00), 0x3fb8_aa3b);
        assert_eq!(bits(-3.3333335072e-02), 0xbd08_8889);
        assert_eq!(bits(1.5873016091e-03), 0x3ad0_0d01);
        assert_eq!(bits(-7.9365076090e-05), 0xb8a6_70cd);
        assert_eq!(bits(4.0082177293e-06), 0x3686_7e54);
        assert_eq!(bits(-2.0109921195e-07), 0xb457_edbb);
    }

    #[test]
    fn special_values() {
        let (inf, nan) = (f32::INFINITY, f32::from_bits(0x7fc0_1234));
        assert_eq!(tanhf_glibc(inf), 1.0);
        assert_eq!(tanhf_glibc(-inf), -1.0);
        assert_eq!(tanhf_glibc(nan).to_bits(), 0x7fc0_1234);
        assert_eq!(
            tanhf_glibc(f32::from_bits(0xff80_0001)).to_bits(),
            0xffc0_0001
        );
        assert_eq!(tanhf_glibc(-0.0).to_bits(), 0x8000_0000);
        assert_eq!(tanhf_glibc(30.0), 1.0);
        assert_eq!(tanhf_glibc(-30.0), -1.0);
        assert_eq!(expm1f_glibc(inf), inf);
        assert_eq!(expm1f_glibc(-inf), -1.0);
        assert_eq!(expm1f_glibc(100.0), inf);
        assert_eq!(expm1f_glibc(-100.0), -1.0);
        assert_eq!(expm1f_glibc(nan).to_bits(), 0x7fc0_1234);
        assert_eq!(expm1f_glibc(1e-10), 1e-10);
        assert_eq!(expm1f_glibc(-0.0).to_bits(), 0x8000_0000);
    }

    /// Values of the C library (glibc 2.35, x86-64), one or more on each path.
    #[test]
    fn recorded_bits() {
        for (x, tanh, expm1) in [
            (0x3f00_0000u32, 0x3eec_9a9f_u32, 0x3f26_1299_u32), // 0.5
            (0x3f80_0000, 0x3f42_f7d6, 0x3fdb_f0a8),            // 1
            (0xbf80_0000, 0xbf42_f7d6, 0xbf21_d2a7),            // -1
            (0x4120_0000, 0x3f80_0000, 0x46ac_12ee),            // 10
            (0x3d4c_cccd, 0x3d4c_a128, 0x3d52_01a4),            // 0.05
            (0x42a0_0000, 0x3f80_0000, 0x792a_bbce),            // 80
            (0xc1a0_0000, 0xbf80_0000, 0xbf80_0000),            // -20
            (0x3e80_0000, 0x3e7a_cbf5, 0x3e91_6bc8),            // 0.25
            (0xbf40_0000, 0xbf22_991f, 0xbf07_12fc),            // -0.75
            (0x3f7f_ffff, 0x3f42_f7d5, 0x3fdb_f0a8),            // 1 - 2^-24
            (0x3380_0000, 0x3380_0000, 0x3380_0000),            // 2^-24
        ] {
            let x = f32::from_bits(x);
            assert_eq!(tanhf_glibc(x).to_bits(), tanh, "tanhf({x})");
            assert_eq!(expm1f_glibc(x).to_bits(), expm1, "expm1f({x})");
        }
    }

    /// FNV-1a of every 4099th input's result (the inputs `0, 4099, 8198, ...` up to 2^32 - 1),
    /// recorded from glibc 2.35 on Linux x86-64 (`validation/investigations/glibc-maths/`,
    /// `glibc-maths-verify hashes`), so every platform is checked against glibc's bits.
    fn stride_hash(f: fn(f32) -> f32) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut x = 0u64;
        while x <= u64::from(u32::MAX) {
            h = (h ^ u64::from(f(f32::from_bits(x as u32)).to_bits()))
                .wrapping_mul(0x0100_0000_01b3);
            x += 4099;
        }
        h
    }

    #[test]
    fn matches_glibc_on_a_stride() {
        assert_eq!(stride_hash(tanhf_glibc), 0x9ac0_48e4_5dab_0e6e);
        assert_eq!(stride_hash(expm1f_glibc), 0x4dce_e0f8_8de6_fdd1);
    }
}
