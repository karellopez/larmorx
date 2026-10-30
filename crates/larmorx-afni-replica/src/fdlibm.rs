// SPDX-License-Identifier: Apache-2.0 AND SunPro
// Translated to Rust from the GNU C Library 2.35's copies of fdlibm's sysdeps/ieee754/flt-32/s_tanhf.c
// and s_expm1f.c, which carry only fdlibm's notice: "Copyright (C) 1993 by Sun Microsystems, Inc.
// All rights reserved. Developed at SunPro, a Sun Microsystems, Inc. business. Permission to use,
// copy, modify, and distribute this software is freely granted, provided that this notice is
// preserved." (also in NOTICE).
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! The single-precision `tanhf` (and the `expm1f` it calls) that AFNI's Linux builds run, bit for
//! bit.
//!
//! 3dUnifize calls the C library's `tanhf` to squash white-matter values above 1300
//! (`mri_WMunifize`). On Linux that is glibc's, fdlibm's code (Sun's notice only, no LGPL text),
//! which is not correctly rounded, so a correctly rounded function gives a different `f32` for
//! some arguments. On x86-64 it has one build (no FMA variant): plain single-precision arithmetic.
//!
//! Verified on all 2^32 inputs against glibc 2.35 on x86-64, input by input
//! (`crates-gpl/tools/floatmath-verify`); glibc's `tanhf` is not correctly rounded on 118,674,314
//! of them. Until 2026-10-29 this module also held glibc's `cbrtf` (LGPL-2.1-or-later), which
//! stays in `larmorx-gpl`'s verification tools (`crates-gpl/tools/glibc-reference`); 3dUnifize
//! `-EPI` now takes CORE-MATH's correctly rounded `cbrtf` (`unifize::epi_radius`).

/// `tanhf(x)`: glibc 2.35's `__tanhf`.
pub fn tanhf(x: f32) -> f32 {
    const ONE: f32 = 1.0;
    const TWO: f32 = 2.0;
    const TINY: f32 = 1.0e-30;
    let jx = x.to_bits() as i32;
    let ix = jx & 0x7fff_ffff;
    // x is INF or NaN.
    if ix >= 0x7f80_0000 {
        return if jx >= 0 {
            ONE / x + ONE
        } else {
            ONE / x - ONE
        };
    }
    let z = if ix < 0x41b0_0000 {
        // |x| < 22.
        if ix == 0 {
            return x;
        }
        if ix < 0x2400_0000 {
            // |x| < 2^-55: tanh(small) = small.
            return x * (ONE + x);
        }
        if ix >= 0x3f80_0000 {
            // |x| >= 1.
            let t = expm1f(TWO * x.abs());
            ONE - TWO / (t + TWO)
        } else {
            let t = expm1f(-TWO * x.abs());
            -t / (t + TWO)
        }
    } else {
        // |x| >= 22: +-1.
        ONE - TINY
    };
    if jx >= 0 { z } else { -z }
}

/// `expm1f(x)`: glibc 2.35's `__expm1f` (fdlibm).
pub fn expm1f(x: f32) -> f32 {
    const HUGE: f32 = 1.0e+30;
    const TINY: f32 = 1.0e-30;
    const ONE: f32 = 1.0;
    const O_THRESHOLD: f32 = 8.872_168e1; // 0x42b17180
    const LN2_HI: f32 = f32::from_bits(0x3f31_7180);
    const LN2_LO: f32 = f32::from_bits(0x3717_f7d1);
    const INVLN2: f32 = f32::from_bits(0x3fb8_aa3b);
    const Q1: f32 = f32::from_bits(0xbd08_8889);
    const Q2: f32 = f32::from_bits(0x3ad0_0d01);
    const Q3: f32 = f32::from_bits(0xb8a6_70cd);
    const Q4: f32 = f32::from_bits(0x3686_7e54);
    const Q5: f32 = f32::from_bits(0xb457_edbb);

    let mut x = x;
    let mut hx = x.to_bits();
    let xsb = hx & 0x8000_0000;
    hx &= 0x7fff_ffff;

    // Huge and non-finite arguments.
    if hx >= 0x4195_b844 {
        // |x| >= 27*ln2.
        if hx >= 0x42b1_7218 {
            // |x| >= 88.721...
            if hx > 0x7f80_0000 {
                return x + x; // NaN
            }
            if hx == 0x7f80_0000 {
                return if xsb == 0 { x } else { -1.0 };
            }
            if x > O_THRESHOLD {
                return HUGE * HUGE; // overflow
            }
        }
        if xsb != 0 {
            // x < -27*ln2: -1.
            return TINY - ONE;
        }
    }

    // Argument reduction.
    let k: i32;
    let mut c = 0.0f32;
    if hx > 0x3eb1_7218 {
        // |x| > 0.5 ln2.
        let (hi, lo);
        if hx < 0x3F85_1592 {
            // and |x| < 1.5 ln2.
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
            // `k = invln2*x + (+-0.5)` in float, converted to int (truncated).
            k = (INVLN2 * x + if xsb == 0 { 0.5f32 } else { -0.5f32 }) as i32;
            let t = k as f32;
            hi = x - t * LN2_HI; // t*ln2_hi is exact here
            lo = t * LN2_LO;
        }
        x = hi - lo;
        c = (hi - x) - lo;
    } else if hx < 0x3300_0000 {
        // |x| < 2^-25: x.
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
    let add_exponent = |y: f32| f32::from_bits((y.to_bits() as i32).wrapping_add(k << 23) as u32);
    if k <= -2 || k > 56 {
        // exp(x) - 1.
        let y = ONE - (e - x);
        return add_exponent(y) - ONE;
    }
    if k < 23 {
        let t = f32::from_bits((0x3f80_0000 - (0x0100_0000 >> k)) as u32); // 1 - 2^-k
        let y = t - (e - x);
        add_exponent(y)
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

    #[test]
    fn special_values() {
        assert_eq!(tanhf(0.0).to_bits(), 0.0f32.to_bits());
        assert_eq!(tanhf(-0.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(tanhf(f32::INFINITY), 1.0);
        assert_eq!(tanhf(f32::NEG_INFINITY), -1.0);
        assert!(tanhf(f32::NAN).is_nan());
        assert_eq!(tanhf(30.0), 1.0);
        assert_eq!(expm1f(0.0), 0.0);
        assert_eq!(expm1f(-100.0), -1.0);
    }

    /// glibc 2.35's results where they are not the correctly rounded ones, spread over the
    /// arguments 3dUnifize passes (`crates-gpl/tools/floatmath-verify/glibc_reference.c vectors 24`).
    const GLIBC_TANHF: &[(u32, u32)] = &[
        (0x3a83_1289, 0x3a83_1287),
        (0x3ad4_482d, 0x3ad4_4820),
        (0x3b25_7b8f, 0x3b25_7b79),
        (0x3b76_b01f, 0x3b76_afd2),
        (0x3bc7_e4af, 0x3bc7_e40c),
        (0x3c19_1945, 0x3c19_1820),
        (0x3c6a_4dda, 0x3c6a_49c4),
        (0x3cbb_825f, 0x3cbb_79fe),
        (0x3d0c_b6ef, 0x3d0c_a8c4),
        (0x3d5d_eb82, 0x3d5d_b3fb),
        (0x3daf_2014, 0x3dae_b31f),
        (0x3e00_54a0, 0x3dff_5369),
        (0x3e51_892f, 0x3e4e_a8d7),
        (0x3ea2_bdc0, 0x3e9d_792d),
        (0x3ef3_f24f, 0x3ee3_05a8),
        (0x3f45_26e0, 0x3f25_a1e8),
        (0x3f96_5b7b, 0x3f53_652c),
        (0x3fe7_9028, 0x3f72_9f06),
        (0x4038_c530, 0x3f7e_6a02),
        (0x4089_fae0, 0x3f7f_e870),
        (0x40db_b3ad, 0x3f7f_ffdc),
    ];
    #[test]
    fn matches_recorded_glibc_values() {
        for &(x, y) in GLIBC_TANHF {
            let x = f32::from_bits(x);
            assert_eq!(tanhf(x).to_bits(), y, "tanhf({x:e})");
            assert_eq!(tanhf(-x).to_bits(), y | 0x8000_0000, "tanhf({:e})", -x);
        }
    }
}
