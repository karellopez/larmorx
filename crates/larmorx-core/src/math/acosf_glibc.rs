// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-fdlibm
// Translated to Rust from fdlibm's float arc cosine as the GNU C Library 2.35 ships it,
// sysdeps/ieee754/flt-32/e_acosf.c ("float version of e_acos.c"), which carries only Sun's notice
// (reproduced below and in LICENSES/LicenseRef-fdlibm.txt and NOTICE). Rust translation Copyright
// 2026 Karel Lopez Vilaret; modified from the original.
//! The single-precision arc cosine with the bits of glibc 2.35's `acosf` on x86-64, on every
//! platform.
//!
//! glibc 2.35's `acosf` (the symbol `acosf@@GLIBC_2.2.5`) is fdlibm's `__ieee754_acosf`, compiled
//! for baseline x86-64 (no fused multiply-adds; no CPU-specific variant), behind glibc's SVID
//! compatibility wrapper. Every operation here is one IEEE single-precision operation rounded to
//! nearest, in the C code's order; `sqrtf` is the correctly rounded square root (SSE's
//! `sqrtss`). The wrapper changes one result, measured as a black box (its code is glibc's, LGPL,
//! not translated): for `|x| > 1`, infinities included, the C library returns the positive quiet
//! NaN `0x7fc00000` (fdlibm's own `(x - x) / (x - x)` would give SSE's default NaN `0xffc00000`); a
//! NaN argument comes back quieted. Not correctly rounded: [`super::acos`] (in double, rounded
//! to float) is larmorx's default; this serves `reproduce`. Proved equal to the system's `acosf`
//! (glibc 2.35, x86-64) on all 2^32 inputs, NaN bits included
//! (`validation/investigations/glibc-maths/`).

// ====================================================
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//
// Developed at SunPro, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
// ====================================================

// fdlibm's constants as written (C rounds each decimal literal to double, then to float).
#![allow(clippy::excessive_precision, clippy::approx_constant)]

use super::x86_nan::invalid_f32;

const ONE: f32 = 1.0000000000e+00_f64 as f32; // 0x3F800000
const PI: f32 = 3.1415925026e+00_f64 as f32; // 0x40490fda
const PIO2_HI: f32 = 1.5707962513e+00_f64 as f32; // 0x3fc90fda
const PIO2_LO: f32 = 7.5497894159e-08_f64 as f32; // 0x33a22168
const PS0: f32 = 1.6666667163e-01_f64 as f32; // 0x3e2aaaab
const PS1: f32 = -3.2556581497e-01_f64 as f32; // 0xbea6b090
const PS2: f32 = 2.0121252537e-01_f64 as f32; // 0x3e4e0aa8
const PS3: f32 = -4.0055535734e-02_f64 as f32; // 0xbd241146
const PS4: f32 = 7.9153501429e-04_f64 as f32; // 0x3a4f7f04
const PS5: f32 = 3.4793309169e-05_f64 as f32; // 0x3811ef08
const QS1: f32 = -2.4033949375e+00_f64 as f32; // 0xc019d139
const QS2: f32 = 2.0209457874e+00_f64 as f32; // 0x4001572d
const QS3: f32 = -6.8828397989e-01_f64 as f32; // 0xbf303361
const QS4: f32 = 7.7038154006e-02_f64 as f32; // 0x3d9dc62e

/// The C library's `acosf(x)` (glibc 2.35, x86-64), with its bits on every platform: 0 at 1,
/// `π` rounded at -1, `π/2` rounded for `|x| <= 2^-26`, the positive quiet NaN for `|x| > 1`, a
/// NaN quieted.
#[must_use]
pub fn acosf_glibc(x: f32) -> f32 {
    if x.abs() > ONE {
        // The SVID wrapper's result for acos(|x| > 1), measured.
        return f32::from_bits(0x7fc0_0000);
    }
    ieee754_acosf(x)
}

/// fdlibm's `__ieee754_acosf(x)`, statement by statement.
#[inline]
fn ieee754_acosf(x: f32) -> f32 {
    let hx = x.to_bits() as i32;
    let ix = hx & 0x7fff_ffff;
    if ix == 0x3f80_0000 {
        // |x| == 1
        return if hx > 0 {
            0.0 // acos(1) = 0
        } else {
            PI + 2.0f32 * PIO2_LO // acos(-1) = pi
        };
    } else if ix > 0x3f80_0000 {
        // |x| > 1, or NaN: NaN
        return invalid_f32(x);
    }
    if ix < 0x3f00_0000 {
        // |x| < 0.5
        if ix <= 0x3280_0000 {
            return PIO2_HI + PIO2_LO; // |x| <= 2^-26
        }
        let z = x * x;
        let p = z * (PS0 + z * (PS1 + z * (PS2 + z * (PS3 + z * (PS4 + z * PS5)))));
        let q = ONE + z * (QS1 + z * (QS2 + z * (QS3 + z * QS4)));
        let r = p / q;
        PIO2_HI - (x - (PIO2_LO - x * r))
    } else if hx < 0 {
        // x < -0.5
        let z = (ONE + x) * 0.5f32;
        let p = z * (PS0 + z * (PS1 + z * (PS2 + z * (PS3 + z * (PS4 + z * PS5)))));
        let q = ONE + z * (QS1 + z * (QS2 + z * (QS3 + z * QS4)));
        let s = z.sqrt();
        let r = p / q;
        let w = r * s - PIO2_LO;
        PI - 2.0f32 * (s + w)
    } else {
        // x > 0.5
        let z = (ONE - x) * 0.5f32;
        let s = z.sqrt();
        let df = f32::from_bits(s.to_bits() & 0xffff_f000);
        let c = (z - df * df) / (s + df);
        let p = z * (PS0 + z * (PS1 + z * (PS2 + z * (PS3 + z * (PS4 + z * PS5)))));
        let q = ONE + z * (QS1 + z * (QS2 + z * (QS3 + z * QS4)));
        let r = p / q;
        let w = r * s + c;
        2.0f32 * (df + w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The constants as C rounds fdlibm's literals (the hex values of its comments).
    #[test]
    fn constants() {
        let got = [
            ONE, PI, PIO2_HI, PIO2_LO, PS0, PS1, PS2, PS3, PS4, PS5, QS1, QS2, QS3, QS4,
        ]
        .map(f32::to_bits);
        assert_eq!(
            got,
            [
                0x3f80_0000,
                0x4049_0fda,
                0x3fc9_0fda,
                0x33a2_2168,
                0x3e2a_aaab,
                0xbea6_b090,
                0x3e4e_0aa8,
                0xbd24_1146,
                0x3a4f_7f04,
                0x3811_ef08,
                0xc019_d139,
                0x4001_572d,
                0xbf30_3361,
                0x3d9d_c62e,
            ]
        );
    }

    /// Values of the C library (glibc 2.35, x86-64), one or more on each path.
    #[test]
    fn recorded_bits() {
        for (x, acos) in [
            (0x3f00_0000u32, 0x3f86_0a92_u32), // 0.5
            (0x3f80_0000, 0x0000_0000),        // 1
            (0xbf80_0000, 0x4049_0fdb),        // -1
            (0x4120_0000, 0x7fc0_0000),        // 10: the wrapper's NaN
            (0x3d4c_cccd, 0x3fc2_a8c5),        // 0.05
            (0x3e80_0000, 0x3fa8_b807),        // 0.25
            (0xbf40_0000, 0x401a_ce94),        // -0.75
            (0x3f7f_ffff, 0x39b5_04f3),        // 1 - 2^-24
            (0x3380_0000, 0x3fc9_0fda),        // 2^-24
            (0x7f80_0000, 0x7fc0_0000),        // inf
            (0xff80_0000, 0x7fc0_0000),        // -inf
            (0xffc0_0042, 0xffc0_0042),        // a NaN, quieted
            (0x7f80_0001, 0x7fc0_0001),
        ] {
            let x = f32::from_bits(x);
            assert_eq!(acosf_glibc(x).to_bits(), acos, "acosf({x:e})");
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
        assert_eq!(stride_hash(acosf_glibc), 0x5411_680b_5a3e_f8be);
    }
}
