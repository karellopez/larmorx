// SPDX-License-Identifier: LicenseRef-FSL AND SunPro
// Translated to Rust from fdlibm's float functions as the GNU C Library 2.35 ships them
// (sysdeps/ieee754/flt-32/e_atan2f.c, s_atanf.c), which carry only Sun's notice:
//
//   Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//   Developed at SunPro, a Sun Microsystems, Inc. business.
//   Permission to use, copy, modify, and distribute this software is freely granted,
//   provided that this notice is preserved.
//
// Rust translation Copyright 2026 Karel Lopez Vilaret, part of larmorx-nc (see LICENSE).
//! `atan2f` and `atanf` as FSL's binaries get them from glibc on x86-64 (fdlibm's float code,
//! compiled without fused multiply-adds: glibc 2.35 has no FMA variant of these two).
//!
//! FSL's `rotmat2euler` calls `atan2` with `float` arguments, which C++ resolves to `atan2f`;
//! its results start every line search of mcflirt, so they must be glibc's bits, which are
//! not always correctly rounded. `atanf` is bit-identical to glibc 2.35's on all 2^32 inputs,
//! `atan2f` on 2^32 random and rotation-like pairs and every special case
//! (`tools/glibc-float-verify`).
//!
//! The constants are fdlibm's decimal literals as the compiler rounds them, not the hex values
//! of its comments: for `aT[0]`, `3.3333334327e-01` is `0x3eaaaaab` (glibc's binary holds it),
//! while the comment says `0x3eaaaaaa`. A port that took the comment (before 2026-10-25) gave
//! `atanf` results one ulp off for some small arguments, which moved the start of one
//! optimisation stage in two of fMRIPrep's mcflirt calls (`docs/findings/fsl-mcflirt.md`).

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
        return x + y;
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
    // aT[0] = 3.3333334327e-01 rounds to 0x3eaaaaab (fdlibm's comment says 0x3eaaaaaa).
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
            return x + x;
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
    let s1 = z * (AT[0] + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
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
