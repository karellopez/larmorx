// SPDX-License-Identifier: LicenseRef-FSL AND SunPro
// Translated to Rust from fdlibm's code as the GNU C Library 2.35 ships it: the float functions
// sysdeps/ieee754/flt-32/e_atan2f.c and s_atanf.c, and the double functions
// sysdeps/ieee754/dbl-64/s_tanh.c and s_expm1.c, which carry only Sun's notice:
//
//   Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//   Developed at SunPro, a Sun Microsystems, Inc. business.
//   Permission to use, copy, modify, and distribute this software is freely granted,
//   provided that this notice is preserved.
//
// s_expm1.c: "Modified by Naohiko Shimizu/Tokai University, Japan 1997/08/25, for performance
// improvement on pipelined processors."
//
// Rust translation Copyright 2026 Karel Lopez Vilaret, part of larmorx-nc (see LICENSE).
//! fdlibm's functions as FSL's binaries get them from glibc on x86-64: `atan2f` and `atanf`
//! (float), `tanh` and `expm1` (double). glibc 2.35 has no FMA or other CPU-specific variant of
//! these four: its generic fdlibm code, compiled without fused multiply-adds, is what runs.
//!
//! FSL's `rotmat2euler` calls `atan2` with `float` arguments, which C++ resolves to `atan2f`;
//! its results start every line search of mcflirt, so they must be glibc's bits, which are
//! not always correctly rounded. `atanf` is bit-identical to glibc 2.35's on all 2^32 inputs,
//! `atan2f` on 2^32 random and rotation-like pairs and every special case
//! (`tools/glibc-float-verify`). BET's surface model calls `tanh` 2.5 million times per run;
//! `tanh` and `expm1` are checked against glibc on random inputs (`tools/libm-verify`).
//!
//! The constants are fdlibm's decimal literals as the compiler rounds them, not the hex values
//! of its comments: for `aT[0]`, `3.3333334327e-01` is `0x3eaaaaab` (glibc's binary holds it),
//! while the comment says `0x3eaaaaaa`. A port that took the comment (before 2026-10-25) gave
//! `atanf` results one ulp off for some small arguments, which moved the start of one
//! optimisation stage in two of fMRIPrep's mcflirt calls (`docs/findings/fsl-mcflirt.md`).

pub use double::{expm1, tanh};

/// glibc's `atan2f(y, x)` (`__ieee754_atan2f`).
pub fn atan2f(y: f32, x: f32) -> f32 {
    const TINY: f32 = 1.0e-30;
    const PI_O_4: f32 = f32::from_bits(0x3f49_0fdb);
    const PI_O_2: f32 = f32::from_bits(0x3fc9_0fdb);
    const PI: f32 = f32::from_bits(0x4049_0fdb);
    const PI_LO: f32 = f32::from_bits(0xb3bb_bd2e);

    let hx = x.to_bits() as i32;
    let ix = hx & 0x7fff_ffff;
    let hy = y.to_bits() as i32;
    let iy = hy & 0x7fff_ffff;
    if ix > 0x7f80_0000 || iy > 0x7f80_0000 {
        // `x + y`: glibc's binary adds `y` into `x` (`addss %xmm2,%xmm0`), so `x`'s NaN wins.
        return crate::sse::add_f32(x, y);
    }
    if hx == 0x3f80_0000 {
        return atanf(y);
    }
    let m = ((hy >> 31) & 1) | ((hx >> 30) & 2);
    if iy == 0 {
        match m {
            0 | 1 => return y,
            2 => return PI + TINY,
            _ => return -PI - TINY,
        }
    }
    if ix == 0 {
        return if hy < 0 {
            -PI_O_2 - TINY
        } else {
            PI_O_2 + TINY
        };
    }
    if ix == 0x7f80_0000 {
        if iy == 0x7f80_0000 {
            return match m {
                0 => PI_O_4 + TINY,
                1 => -PI_O_4 - TINY,
                2 => 3.0f32 * PI_O_4 + TINY,
                _ => -3.0f32 * PI_O_4 - TINY,
            };
        }
        return match m {
            0 => 0.0,
            1 => -0.0,
            2 => PI + TINY,
            _ => -PI - TINY,
        };
    }
    if iy == 0x7f80_0000 {
        return if hy < 0 {
            -PI_O_2 - TINY
        } else {
            PI_O_2 + TINY
        };
    }
    let k = (iy - ix) >> 23;
    let z = if k > 60 {
        PI_O_2 + 0.5f32 * PI_LO
    } else if hx < 0 && k < -60 {
        0.0
    } else {
        atanf((y / x).abs())
    };
    match m {
        0 => z,
        1 => f32::from_bits(z.to_bits() ^ 0x8000_0000),
        2 => PI - (z - PI_LO),
        _ => (z - PI_LO) - PI,
    }
}

/// glibc's `atanf(x)` (`__atanf`).
pub fn atanf(x: f32) -> f32 {
    atanf_with_at0(x, AT0)
}

/// `aT[0]` as the compiler rounds fdlibm's `3.3333334327e-01` (its comment says `0x3eaaaaaa`).
pub const AT0: f32 = f32::from_bits(0x3eaa_aaab);

/// [`atanf`] with another first polynomial coefficient `aT[0]`: how far fdlibm's comment value
/// (`0x3eaaaaaa`) moves the results (`tools/glibc-float-verify atanf`).
pub fn atanf_with_at0(x: f32, at0: f32) -> f32 {
    const ATANHI: [f32; 4] = [
        f32::from_bits(0x3eed_6338),
        f32::from_bits(0x3f49_0fda),
        f32::from_bits(0x3f7b_985e),
        f32::from_bits(0x3fc9_0fda),
    ];
    const ATANLO: [f32; 4] = [
        f32::from_bits(0x31ac_3769),
        f32::from_bits(0x3322_2168),
        f32::from_bits(0x3314_0fb4),
        f32::from_bits(0x33a2_2168),
    ];
    // AT[0] is not used: `at0` (AT0, 0x3eaaaaab, for glibc's atanf) takes its place.
    const AT: [f32; 11] = [
        f32::from_bits(0x3eaa_aaab),
        f32::from_bits(0xbe4c_cccd),
        f32::from_bits(0x3e12_4925),
        f32::from_bits(0xbde3_8e38),
        f32::from_bits(0x3dba_2e6e),
        f32::from_bits(0xbd9d_8795),
        f32::from_bits(0x3d88_6b35),
        f32::from_bits(0xbd6e_f16b),
        f32::from_bits(0x3d4b_da59),
        f32::from_bits(0xbd15_a221),
        f32::from_bits(0x3c85_69d7),
    ];
    const ONE: f32 = 1.0;
    const HUGE: f32 = 1.0e30;

    let hx = x.to_bits() as i32;
    let ix = hx & 0x7fff_ffff;
    let mut x = x;
    let id: i32;
    if ix >= 0x4c00_0000 {
        if ix > 0x7f80_0000 {
            return crate::sse::add_f32(x, x);
        }
        return if hx > 0 {
            ATANHI[3] + ATANLO[3]
        } else {
            -ATANHI[3] - ATANLO[3]
        };
    }
    if ix < 0x3ee0_0000 {
        if ix < 0x3100_0000 && HUGE + x > ONE {
            return x;
        }
        id = -1;
    } else {
        x = x.abs();
        if ix < 0x3f98_0000 {
            if ix < 0x3f30_0000 {
                id = 0;
                x = (2.0f32 * x - ONE) / (2.0f32 + x);
            } else {
                id = 1;
                x = (x - ONE) / (x + ONE);
            }
        } else if ix < 0x401c_0000 {
            id = 2;
            x = (x - 1.5f32) / (ONE + 1.5f32 * x);
        } else {
            id = 3;
            x = -1.0f32 / x;
        }
    }
    let z = x * x;
    let w = z * z;
    let s1 = z * (at0 + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
    let s2 = w * (AT[1] + w * (AT[3] + w * (AT[5] + w * (AT[7] + w * AT[9]))));
    if id < 0 {
        x - x * (s1 + s2)
    } else {
        let i = id as usize;
        let z = ATANHI[i] - ((x * (s1 + s2) - ATANLO[i]) - x);
        if hx < 0 { -z } else { z }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_values() {
        assert_eq!(atan2f(0.0, 1.0), 0.0);
        assert_eq!(atan2f(0.0, -1.0), f32::from_bits(0x4049_0fdb));
        assert_eq!(atan2f(1.0, 0.0), f32::from_bits(0x3fc9_0fdb));
        assert!(atan2f(f32::NAN, 1.0).is_nan());
        assert_eq!(atan2f(1.0, 1.0), atanf(1.0));
        // Two NaNs: glibc's binary passes on x's (atan2f(y, x)); one NaN: that one.
        let (p, n) = (f32::from_bits(0x7fc0_0000), f32::from_bits(0xffc0_0000));
        assert_eq!(atan2f(p, n).to_bits(), 0xffc0_0000);
        assert_eq!(atan2f(n, p).to_bits(), 0x7fc0_0000);
        assert_eq!(
            atan2f(f32::from_bits(0x7fc1_1111), 2.0).to_bits(),
            0x7fc1_1111
        );
        assert_eq!(atanf(f32::from_bits(0xffc2_2222)).to_bits(), 0xffc2_2222);
        let r = atan2f(0.3, 0.9);
        assert!((f64::from(r) - (0.3f64).atan2(0.9)).abs() < 1e-7);
        let r = atan2f(-0.3, -0.9);
        assert!((f64::from(r) - (-0.3f64).atan2(-0.9)).abs() < 1e-6);
    }

    /// glibc 2.35's bits (its libm on x86-64, called through ctypes), one argument per branch
    /// of `atanf`'s reduction.
    #[test]
    fn atanf_glibc_bits() {
        for (x, want) in [
            (0x3c7c_2fbe_u32, 0x3c7c_2aa5_u32),
            (0x3d00_0000, 0x3cff_eaae),
            (0x3e80_0000, 0x3e7a_dbb0),
            (0x3ee8_0000, 0x3ed9_d489),
            (0x3f40_0000, 0x3f24_bc7d),
            (0x3fa0_0000, 0x3f65_63e3),
            (0x4080_0000, 0x3fa9_b465),
            (0x4c80_0000, 0x3fc9_0fdb),
            (0x3000_0000, 0x3000_0000),
            (0xbe99_999a, 0xbe95_39d4),
        ] {
            let got = atanf(f32::from_bits(x)).to_bits();
            assert_eq!(
                got, want,
                "atanf({x:#010x}) = {got:#010x}, glibc {want:#010x}"
            );
        }
    }

    /// The `rotmat2euler` call where the comment's `aT[0]` (0x3eaaaaaa) gave one ulp more than
    /// glibc: fMRIPrep's ds002422 rest run, volume 166, the third stage's start
    /// (`atan2(sy, cy)`).
    #[test]
    fn atan2f_mcflirt_ds002422() {
        let got = atan2f(f32::from_bits(0x3c7c_2fbe), f32::from_bits(0x3f7f_f83d));
        assert_eq!(got.to_bits(), 0x3c7c_324a);
    }
}

/// `tanh` and `expm1` (double precision), glibc 2.35's `__tanh` and `__expm1`.
// fdlibm's literals, digit for digit
#[allow(clippy::excessive_precision, clippy::approx_constant)]
mod double {

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
            return if jx >= 0 {
                ONE / x + ONE
            } else {
                ONE / x - ONE
            };
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
            for &x in &[
                1e-20f64, 0.001, 0.3, 0.5, 0.9, 1.0, 1.5, 3.0, 10.0, 21.9, -0.7, -2.2,
            ] {
                let want = x.tanh();
                let got = tanh(x);
                assert!((got - want).abs() <= 4.0 * f64::EPSILON * want.abs(), "{x}");
                let e = expm1(x);
                assert!(
                    (e - x.exp_m1()).abs() <= 4.0 * f64::EPSILON * e.abs(),
                    "{x}"
                );
            }
        }
    }
}
