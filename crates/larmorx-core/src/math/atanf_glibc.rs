// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-fdlibm
// Translated to Rust from fdlibm's float functions as the GNU C Library 2.35 ships them,
// sysdeps/ieee754/flt-32/s_atanf.c and e_atan2f.c ("float version of s_atan.c", "float version of
// e_atan2.c"), which carry only Sun's notice (reproduced below and in LICENSES/LicenseRef-fdlibm.txt
// and NOTICE). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
//! The single-precision arctangents with the bits of glibc 2.35's `atanf` and `atan2f` on x86-64,
//! on every platform.
//!
//! glibc 2.35 has no FMA or other CPU-specific variant of these two: its generic fdlibm code,
//! compiled without fused multiply-adds, is what runs. Every operation here is one IEEE
//! single-precision operation rounded to nearest, in the C code's order (plain `*`, `+`, `/`;
//! never `mul_add`), and the NaN results are SSE's (`super::x86_nan`), so the bits are the same
//! on every platform. Not correctly rounded; [`super::atan2`] (in double, rounded to float) is
//! larmorx's default. These serve `reproduce` (FSL's `rotmat2euler`, whose `atan2` of `float`
//! arguments is `atan2f`: mcflirt's and FLIRT's Euler angles; `fslmaths -atan`). Until 2026-11-04
//! the FSL replicas' core carried its own translation (`larmorx-nc-core`'s `fdlibm`); it now calls
//! this one. `atanf` is proved equal to the system's (glibc 2.35, x86-64) and to that translation
//! on all 2^32 inputs, `atan2f` on more than 10^9 pairs and every pair of special values, NaN
//! bits included (`validation/investigations/glibc-maths/`).
//!
//! The constants are fdlibm's decimal literals as the compiler rounds them, not the hex values of
//! its comments: for `aT[0]`, `3.3333334327e-01` is `0x3eaaaaab` (glibc's binary holds it), while
//! the comment says `0x3eaaaaaa`. A port that took the comment (before 2026-10-25) gave `atanf`
//! results one ulp off for some small arguments (`docs/findings/fsl-mcflirt.md`).

// ====================================================
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//
// Developed at SunPro, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
// ====================================================

use super::x86_nan::add_f32;

/// `aT[0]` as the compiler rounds fdlibm's `3.3333334327e-01` (its comment says `0x3eaaaaaa`).
const AT0: f32 = f32::from_bits(0x3eaa_aaab);

/// glibc 2.35's `atanf(x)` (`__atanf`), with its bits on every platform: `x` for `|x| < 2^-29`,
/// `±(π/2)` rounded for `|x| >= 2^25` and `±∞`, a NaN quieted.
#[inline]
#[must_use]
pub fn atanf_glibc(x: f32) -> f32 {
    atanf_with_at0(x, AT0)
}

/// [`atanf_glibc`] with another first polynomial coefficient `aT[0]`: how far fdlibm's comment
/// value (`0x3eaaaaaa`) moves the results (the FSL replicas' `tools/glibc-float-verify atanf`).
/// Not an API.
#[doc(hidden)]
#[inline]
#[must_use]
pub fn atanf_glibc_with_at0(x: f32, at0: f32) -> f32 {
    atanf_with_at0(x, at0)
}

#[inline(always)]
fn atanf_with_at0(x: f32, at0: f32) -> f32 {
    /// `atanhi`: atan(0.5), atan(1), atan(1.5), atan(inf), high parts.
    const ATANHI: [f32; 4] = [
        f32::from_bits(0x3eed_6338),
        f32::from_bits(0x3f49_0fda),
        f32::from_bits(0x3f7b_985e),
        f32::from_bits(0x3fc9_0fda),
    ];
    /// `atanlo`: their low parts.
    const ATANLO: [f32; 4] = [
        f32::from_bits(0x31ac_3769),
        f32::from_bits(0x3322_2168),
        f32::from_bits(0x3314_0fb4),
        f32::from_bits(0x33a2_2168),
    ];
    /// `aT`: the polynomial; `AT[0]` is not used (`at0` takes its place).
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
        // |x| >= 2^25
        if ix > 0x7f80_0000 {
            return add_f32(x, x); // NaN
        }
        return if hx > 0 {
            ATANHI[3] + ATANLO[3]
        } else {
            -ATANHI[3] - ATANLO[3]
        };
    }
    if ix < 0x3ee0_0000 {
        // |x| < 0.4375
        if ix < 0x3100_0000 && HUGE + x > ONE {
            // |x| < 2^-29 (raise inexact)
            return x;
        }
        id = -1;
    } else {
        x = x.abs();
        if ix < 0x3f98_0000 {
            // |x| < 1.1875
            if ix < 0x3f30_0000 {
                // 7/16 <= |x| < 11/16
                id = 0;
                x = (2.0f32 * x - ONE) / (2.0f32 + x);
            } else {
                // 11/16 <= |x| < 19/16
                id = 1;
                x = (x - ONE) / (x + ONE);
            }
        } else if ix < 0x401c_0000 {
            // |x| < 2.4375
            id = 2;
            x = (x - 1.5f32) / (ONE + 1.5f32 * x);
        } else {
            // 2.4375 <= |x| < 2^25
            id = 3;
            x = -1.0f32 / x;
        }
    }
    // end of argument reduction
    let z = x * x;
    let w = z * z;
    // the sum of aT[i] z^(i+1), odd and even polynomials
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

/// glibc 2.35's `atan2f(y, x)` (`__ieee754_atan2f`; the `atan2f` wrapper only sets `errno`), with
/// its bits on every platform. Of two NaN arguments, `x`'s is passed on quieted (glibc's binary
/// computes `x + y` with `x` the destination).
#[inline]
#[must_use]
pub fn atan2f_glibc(y: f32, x: f32) -> f32 {
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
        // x or y is NaN
        return add_f32(x, y);
    }
    if hx == 0x3f80_0000 {
        // x = 1.0
        return atanf_glibc(y);
    }
    let m = ((hy >> 31) & 1) | ((hx >> 30) & 2); // 2 * sign(x) + sign(y)
    if iy == 0 {
        // y = 0
        match m {
            0 | 1 => return y,
            2 => return PI + TINY,
            _ => return -PI - TINY,
        }
    }
    if ix == 0 {
        // x = 0
        return if hy < 0 {
            -PI_O_2 - TINY
        } else {
            PI_O_2 + TINY
        };
    }
    if ix == 0x7f80_0000 {
        // x is INF
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
        // y is INF
        return if hy < 0 {
            -PI_O_2 - TINY
        } else {
            PI_O_2 + TINY
        };
    }
    // compute y/x
    let k = (iy - ix) >> 23;
    let z = if k > 60 {
        // |y/x| > 2^60
        PI_O_2 + 0.5f32 * PI_LO
    } else if hx < 0 && k < -60 {
        // |y|/x < -2^60
        0.0
    } else {
        // safe to do y/x
        atanf_glibc((y / x).abs())
    };
    match m {
        0 => z,                                         // atan(+,+)
        1 => f32::from_bits(z.to_bits() ^ 0x8000_0000), // atan(-,+)
        2 => PI - (z - PI_LO),                          // atan(+,-)
        _ => (z - PI_LO) - PI,                          // atan(-,-)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_values() {
        assert_eq!(atan2f_glibc(0.0, 1.0), 0.0);
        assert_eq!(atan2f_glibc(0.0, -1.0), f32::from_bits(0x4049_0fdb));
        assert_eq!(atan2f_glibc(1.0, 0.0), f32::from_bits(0x3fc9_0fdb));
        assert_eq!(atan2f_glibc(1.0, 1.0), atanf_glibc(1.0));
        // Two NaNs: glibc's binary passes on x's (atan2f(y, x)); one NaN: that one.
        let (p, n) = (f32::from_bits(0x7fc0_0000), f32::from_bits(0xffc0_0000));
        assert_eq!(atan2f_glibc(p, n).to_bits(), 0xffc0_0000);
        assert_eq!(atan2f_glibc(n, p).to_bits(), 0x7fc0_0000);
        assert_eq!(
            atan2f_glibc(f32::from_bits(0x7fc1_1111), 2.0).to_bits(),
            0x7fc1_1111
        );
        assert_eq!(
            atanf_glibc(f32::from_bits(0xffc2_2222)).to_bits(),
            0xffc2_2222
        );
        assert_eq!(
            atanf_glibc(f32::from_bits(0x7f80_0003)).to_bits(),
            0x7fc0_0003
        );
        let r = atan2f_glibc(0.3, 0.9);
        assert!((f64::from(r) - (0.3f64).atan2(0.9)).abs() < 1e-7);
        let r = atan2f_glibc(-0.3, -0.9);
        assert!((f64::from(r) - (-0.3f64).atan2(-0.9)).abs() < 1e-6);
    }

    /// glibc 2.35's bits (its libm on x86-64, called through ctypes), one argument per branch of
    /// `atanf`'s reduction.
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
            let got = atanf_glibc(f32::from_bits(x)).to_bits();
            assert_eq!(
                got, want,
                "atanf({x:#010x}) = {got:#010x}, glibc {want:#010x}"
            );
        }
    }

    /// The `rotmat2euler` call where the comment's `aT[0]` (0x3eaaaaaa) gave one ulp more than
    /// glibc: fMRIPrep's ds002422 rest run, volume 166, the third stage's start (`atan2(sy, cy)`).
    #[test]
    fn atan2f_mcflirt_ds002422() {
        let got = atan2f_glibc(f32::from_bits(0x3c7c_2fbe), f32::from_bits(0x3f7f_f83d));
        assert_eq!(got.to_bits(), 0x3c7c_324a);
        assert_eq!(
            atanf_glibc_with_at0(f32::from_bits(0x3c7c_2fbe), AT0).to_bits(),
            atanf_glibc(f32::from_bits(0x3c7c_2fbe)).to_bits()
        );
    }

    /// A hash of every 4099th input of `atanf`, recorded from glibc 2.35 on Linux x86-64
    /// (`validation/investigations/glibc-maths/`).
    #[test]
    fn matches_glibc_on_a_stride() {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut x = 0u64;
        while x <= u64::from(u32::MAX) {
            let v = atanf_glibc(f32::from_bits(x as u32));
            h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x0100_0000_01b3);
            x += 4099;
        }
        assert_eq!(h, STRIDE);
    }

    const STRIDE: u64 = 0xafb4_3af7_ad2c_4d90;
}
