// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-fdlibm
// Translated to Rust from fdlibm's e_log10.c as the GNU C Library 2.35 ships it,
// sysdeps/ieee754/dbl-64/e_log10.c ("@(#)e_log10.c 5.1 93/09/24"), which carries only Sun's
// notice (reproduced below and in LICENSES/LicenseRef-fdlibm.txt and NOTICE). Rust translation
// Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! The base-10 logarithm for binary64 with the bits of glibc 2.35's `log10` on x86-64, on every
//! platform.
//!
//! glibc 2.35's `log10` (`log10@@GLIBC_2.2.5`, its only symbol) is fdlibm's `__ieee754_log10`
//! behind glibc's SVID-compatible wrapper: `n = ilogb(x)` (one more below 1), `x` scaled into
//! [1, 2) or [0.5, 1), then `n log10_2lo + ivln10 log(x)` plus `n log10_2hi`, each product and sum
//! one IEEE double operation in that order (compiled for baseline x86-64: no fused multiply-add;
//! one build), and `log(x)` glibc's own natural logarithm, which on x86-64 with FMA is the FMA
//! build of Arm's `log.c` ([`super::log_glibc`]). The wrapper (LGPL; not translated: its results
//! measured) answers for `x <= 0`: `-inf` for a zero of either sign, C's `NAN` (`0x7ff8...`,
//! positive) for a negative number or `-inf`. A NaN argument gives itself quieted (sign kept); `+inf`
//! gives `+inf`. Not correctly rounded; larmorx has no correctly rounded `log10` yet (CORE-MATH's
//! `cr_log10` is not ported), so its callers use this one in both modes and say so. Proved equal
//! to the system's `log10` (glibc 2.35, x86-64) on the seeded families and special values of
//! `validation/investigations/glibc-maths/` (`verify double`).

// ====================================================
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//
// Developed at SunPro, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
// ====================================================

/// `two54`, 2^54.
const TWO54: f64 = f64::from_bits(0x4350_0000_0000_0000);
/// `ivln10`, 1/ln(10) rounded.
const IVLN10: f64 = f64::from_bits(0x3fdb_cb7b_1526_e50e);
/// `log10_2hi`, the leading 40 bits of log10(2).
const LOG10_2HI: f64 = f64::from_bits(0x3fd3_4413_509f_6000);
/// `log10_2lo`, log10(2) - log10_2hi.
const LOG10_2LO: f64 = f64::from_bits(0x3d59_fef3_11f1_2b36);

/// `log10(x)` with glibc 2.35's bits (module documentation).
pub fn log10_glibc(x: f64) -> f64 {
    if x.is_nan() {
        // `x + x` (positive) or `(x - x) / (x - x)` (negative): SSE gives x quieted
        return f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000);
    }
    // glibc's wrapper (`islessequal(x, 0.0)`): its results, measured
    if x <= 0.0 {
        if x == 0.0 {
            return f64::NEG_INFINITY;
        }
        return f64::from_bits(0x7ff8_0000_0000_0000);
    }
    let mut x = x;
    let mut hx = x.to_bits() as i64;
    let mut k: i32 = 0;
    if hx < 0x0010_0000_0000_0000 {
        // a subnormal number, scaled up
        k -= 54;
        x *= TWO54;
        hx = x.to_bits() as i64;
    }
    if hx >= 0x7ff0_0000_0000_0000 {
        return x + x;
    }
    k += ((hx >> 52) - 1023) as i32;
    let i = i64::from(k < 0);
    hx = (hx & 0x000f_ffff_ffff_ffff) | ((0x3ff - i) << 52);
    let y = f64::from(k + i as i32);
    x = f64::from_bits(hx as u64);
    let z = y * LOG10_2LO + IVLN10 * super::log_glibc(x);
    z + y * LOG10_2HI
}

#[cfg(test)]
mod tests {
    use super::log10_glibc;

    #[test]
    fn special_values() {
        assert_eq!(log10_glibc(0.0), f64::NEG_INFINITY);
        assert_eq!(log10_glibc(-0.0), f64::NEG_INFINITY);
        assert_eq!(log10_glibc(f64::INFINITY), f64::INFINITY);
        assert_eq!(log10_glibc(-1.0).to_bits(), 0x7ff8_0000_0000_0000);
        assert_eq!(
            log10_glibc(f64::NEG_INFINITY).to_bits(),
            0x7ff8_0000_0000_0000
        );
        assert_eq!(
            log10_glibc(f64::from_bits(0x7ff0_0000_0000_0001)).to_bits(),
            0x7ff8_0000_0000_0001
        );
        assert_eq!(
            log10_glibc(f64::from_bits(0xfff0_0000_0000_0123)).to_bits(),
            0xfff8_0000_0000_0123
        );
        for n in 0..=22 {
            assert_eq!(log10_glibc(10f64.powi(n)), f64::from(n), "10^{n}");
        }
    }

    /// The bits of 65,536 seeded arguments (uniform in (0, 1e3), random bit patterns, subnormals)
    /// as glibc 2.35 gives them on Linux x86-64 (the hash recorded from this port after the
    /// verifier's `verify double` found 0 differences): every platform must give them.
    #[test]
    fn seeded_hash() {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut s = 0x1234_5678_9abc_def0u64;
        for i in 0..65_536u64 {
            s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            let r = z ^ (z >> 31);
            let x = match i % 3 {
                0 => (r >> 11) as f64 / (1u64 << 53) as f64 * 1e3,
                1 => f64::from_bits(r),
                _ => f64::from_bits(r & 0x000f_ffff_ffff_ffff),
            };
            h = (h ^ log10_glibc(x).to_bits()).wrapping_mul(0x0100_0000_01b3);
        }
        assert_eq!(h, SEEDED_HASH);
    }

    // glibc 2.35's own `log10` (called through ctypes on the same arguments) gives this hash too.
    const SEEDED_HASH: u64 = 0x596a_a57c_7f00_86f3;
}
