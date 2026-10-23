// SPDX-License-Identifier: GPL-3.0-or-later AND LGPL-2.1-or-later
// Translated to Rust from the GNU C Library 2.35: sysdeps/ieee754/flt-32/s_tanhf.c and
// s_expm1f.c (from fdlibm: "Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
// Developed at SunPro, a Sun Microsystems, Inc. business. Permission to use, copy, modify, and
// distribute this software is freely granted, provided that this notice is preserved."), and
// sysdeps/ieee754/flt-32/s_cbrtf.c (Copyright (C) 1997-2022 Free Software Foundation, Inc.).
// The GNU C Library is free software, licensed under the GNU Lesser General Public License
// version 2.1 or any later version; this translation is also distributed under the GNU GPL
// version 3 or later as part of larmorx-gpl (LGPL-2.1 section 3 permits applying the GPL
// instead).
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! glibc's single-precision `tanhf` and `cbrtf`, bit for bit.
//!
//! 3dUnifize calls the C library's `tanhf` to squash white-matter values above 1300
//! (`mri_WMunifize`) and `cbrtf` for the voxel size of `-EPI`'s radius. On Linux that is glibc,
//! whose float hyperbolic tangent (fdlibm's, through `expm1f`) and cube root are not correctly
//! rounded, so a correctly rounded function gives a different `f32` for some arguments. To
//! reproduce AFNI's output on every platform, larmorx-gpl carries this port of glibc 2.35's code
//! (the version AFNI 25.2.09 runs with on Ubuntu 22.04). On x86-64 these functions have one
//! build (no FMA variant): plain single-precision arithmetic, with the double-precision steps
//! the C source writes for `cbrtf`.
//!
//! Verified exhaustively against glibc 2.35 on x86-64: for all 2^32 inputs of each function,
//! the hash of the port's results equals the hash of glibc's (`tools/floatmath-verify`, with
//! `glibc_reference.c` calling glibc), and `tanhf` was also compared input by input. glibc's
//! `tanhf` is not correctly rounded on 118,674,314 inputs, its `cbrtf` on 453,492,162.

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

/// `cbrtf(x)`: glibc 2.35's `__cbrtf` (a rational approximation in double precision).
pub fn cbrtf(x: f32) -> f32 {
    const CBRT2: f64 = 1.259_921_049_894_873_164_8;
    const SQR_CBRT2: f64 = 1.587_401_051_968_199_474_8;
    const FACTOR: [f64; 5] = [1.0 / SQR_CBRT2, 1.0 / CBRT2, 1.0, CBRT2, SQR_CBRT2];

    // `xm = frexpf(fabsf(x), &xe)`: glibc's frexpf sets xe to 0 for 0, Inf and NaN.
    let (xm, xe) = frexpf(x.abs());
    if xe == 0 && (x == 0.0 || !x.is_finite()) {
        return x + x;
    }
    let xm64 = f64::from(xm);
    let u = (0.492_659_620_528_969_547
        + (0.697_570_460_207_922_770 - 0.191_502_161_678_719_066 * xm64) * xm64) as f32;
    let t2 = u * u * u;
    let (u64_, t264) = (f64::from(u), f64::from(t2));
    let ym =
        (u64_ * (t264 + 2.0 * xm64) / (2.0 * t264 + xm64) * FACTOR[(2 + xe % 3) as usize]) as f32;
    ldexpf(if x > 0.0 { ym } else { -ym }, xe / 3)
}

/// `frexpf`: the mantissa in [0.5, 1) and the exponent; 0, Inf and NaN give exponent 0 (and
/// themselves).
fn frexpf(x: f32) -> (f32, i32) {
    if x == 0.0 || !x.is_finite() {
        return (x, 0);
    }
    let bits = x.to_bits();
    let mut exp = ((bits >> 23) & 0xff) as i32;
    if exp == 0 {
        // Subnormal: normalise with 2^25.
        let (m, e) = frexpf(x * f32::from_bits(0x4c00_0000));
        return (m, e - 25);
    }
    exp -= 126;
    let m = f32::from_bits((bits & 0x807f_ffff) | (126 << 23));
    (m, exp)
}

/// `ldexpf(y, n)`: `y * 2^n`, correctly rounded (exact in double, rounded once to float).
fn ldexpf(y: f32, n: i32) -> f32 {
    (f64::from(y) * 2f64.powi(n)) as f32
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
        assert_eq!(cbrtf(27.0), 3.0);
        assert_eq!(cbrtf(-8.0), -2.0);
        assert_eq!(cbrtf(0.0).to_bits(), 0.0f32.to_bits());
        assert!(cbrtf(f32::NAN).is_nan());
        assert_eq!(cbrtf(f32::INFINITY), f32::INFINITY);
        assert_eq!(expm1f(0.0), 0.0);
        assert_eq!(expm1f(-100.0), -1.0);
    }

    /// glibc 2.35's results where they are not the correctly rounded ones, spread over the
    /// arguments 3dUnifize passes (`tools/floatmath-verify/glibc_reference.c vectors 24`).
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
    const GLIBC_CBRTF: &[(u32, u32)] = &[
        (0x3a83_127b, 0x3dcc_ccd4),
        (0x3aed_5c59, 0x3df9_a134),
        (0x3b57_a644, 0x3e18_4eda),
        (0x3bc1_f024, 0x3e39_3a1e),
        (0x3c2c_3a0e, 0x3e60_50e3),
        (0x3c96_83f2, 0x3e87_1a86),
        (0x3d00_cdd8, 0x3ea1_9b5c),
        (0x3d6b_17bb, 0x3ec5_7f86),
        (0x3dd5_61ac, 0x3ef0_ec61),
        (0x3e3f_ab85, 0x3f12_707f),
        (0x3ea9_f576, 0x3f31_4137),
        (0x3f14_3f57, 0x3f55_6188),
        (0x3f7e_896e, 0x3f7f_82e7),
        (0x3fe8_d327, 0x3f9c_3faa),
        (0x4053_1d03, 0x3fbe_8a81),
        (0x40bd_66f2, 0x3fe7_89a2),
        (0x4127_b0d2, 0x400c_0efe),
        (0x4191_fab7, 0x4028_7e11),
        (0x41fc_44a4, 0x404a_31f6),
        (0x4266_8e82, 0x4077_38a6),
        (0x42d0_d86f, 0x4096_b064),
        (0x433b_2250, 0x40b7_08f9),
        (0x43a5_6c33, 0x40dd_527c),
        (0x440f_b632, 0x4105_0955),
    ];

    #[test]
    fn matches_recorded_glibc_values() {
        for &(x, y) in GLIBC_TANHF {
            let x = f32::from_bits(x);
            assert_eq!(tanhf(x).to_bits(), y, "tanhf({x:e})");
            assert_eq!(tanhf(-x).to_bits(), y | 0x8000_0000, "tanhf({:e})", -x);
        }
        for &(x, y) in GLIBC_CBRTF {
            let x = f32::from_bits(x);
            assert_eq!(cbrtf(x).to_bits(), y, "cbrtf({x:e})");
        }
    }
}
