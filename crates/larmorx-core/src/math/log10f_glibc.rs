// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-fdlibm
// Translated to Rust from fdlibm's e_log10f.c as the GNU C Library 2.35 ships it,
// sysdeps/ieee754/flt-32/e_log10f.c ("float version of e_log10.c"), which carries only Sun's
// notice (reproduced below and in LICENSES/LicenseRef-fdlibm.txt and NOTICE). Rust translation
// Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! The base-10 logarithm for binary32 with the bits of glibc 2.35's `log10f` on x86-64, on every
//! platform.
//!
//! glibc 2.35's `log10f` (`log10f@@GLIBC_2.2.5`, its only symbol) is fdlibm's `__ieee754_log10f`
//! behind glibc's SVID-compatible wrapper: a subnormal argument scaled by 2^25, `k` the exponent
//! (one more below 1), the significand put into [1, 2) or [0.5, 1), then `k log10_2lo + ivln10
//! logf(x)` plus `k log10_2hi`, each product and sum one IEEE single operation in that order
//! (compiled for baseline x86-64: no fused multiply-add; one build), and `logf(x)` glibc's own
//! natural logarithm, which on x86-64 with FMA is the FMA build of Arm's `logf.c`
//! ([`super::logf_glibc`]). The wrapper (LGPL; not translated: its results measured) answers for
//! `x <= 0`: `-inf` for a zero of either sign, C's `NAN` (`0x7fc00000`, positive) for a negative
//! number or `-inf`. A NaN argument gives itself quieted (sign kept); `+inf` gives `+inf`. Not
//! correctly rounded; it serves `reproduce` and the replicas (FreeSurfer's `GCAMwriteMRI`, whose
//! C++ calls the float overload). Proved equal to the system's `log10f` (glibc 2.35, x86-64) on
//! all 2^32 inputs, NaN bits included (`validation/investigations/glibc-maths/`, `verify float`).

// ====================================================
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//
// Developed at SunPro, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
// ====================================================

use super::logf_glibc::logf_glibc;

/// `two25`, 2^25.
const TWO25: f32 = f32::from_bits(0x4c00_0000);
/// `ivln10`, 1/ln(10) rounded.
const IVLN10: f32 = f32::from_bits(0x3ede_5bd9);
/// `log10_2hi`, the leading bits of log10(2).
const LOG10_2HI: f32 = f32::from_bits(0x3e9a_2080);
/// `log10_2lo`, log10(2) - log10_2hi.
const LOG10_2LO: f32 = f32::from_bits(0x3554_27db);

/// `log10f(x)` with glibc 2.35's bits (module documentation).
pub fn log10f_glibc(x: f32) -> f32 {
    if x.is_nan() {
        // `x + x` (positive) or `(x - x) / (x - x)` (negative): SSE gives x quieted
        return f32::from_bits(x.to_bits() | 0x0040_0000);
    }
    // glibc's wrapper (`islessequal(x, 0.0f)`): its results, measured
    if x <= 0.0 {
        if x == 0.0 {
            return f32::NEG_INFINITY;
        }
        return f32::from_bits(0x7fc0_0000);
    }
    let mut x = x;
    let mut hx = x.to_bits() as i32;
    let mut k: i32 = 0;
    if hx < 0x0080_0000 {
        // a subnormal number, scaled up
        k -= 25;
        x *= TWO25;
        hx = x.to_bits() as i32;
    }
    if hx >= 0x7f80_0000 {
        // +inf
        return x + x;
    }
    k += (hx >> 23) - 127;
    let i = ((k as u32) & 0x8000_0000) >> 31;
    let hx = (hx & 0x007f_ffff) | ((0x7f - i as i32) << 23);
    let y = (k + i as i32) as f32;
    let x = f32::from_bits(hx as u32);
    let z = y * LOG10_2LO + IVLN10 * logf_glibc(x);
    z + y * LOG10_2HI
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_values() {
        assert_eq!(log10f_glibc(0.0), f32::NEG_INFINITY);
        assert_eq!(log10f_glibc(-0.0), f32::NEG_INFINITY);
        assert_eq!(log10f_glibc(f32::INFINITY), f32::INFINITY);
        assert_eq!(log10f_glibc(-1.0).to_bits(), 0x7fc0_0000);
        assert_eq!(log10f_glibc(f32::NEG_INFINITY).to_bits(), 0x7fc0_0000);
        assert_eq!(
            log10f_glibc(f32::from_bits(0x7f80_0001)).to_bits(),
            0x7fc0_0001
        );
        assert_eq!(
            log10f_glibc(f32::from_bits(0xff80_0123)).to_bits(),
            0xffc0_0123
        );
        for n in 0..=10 {
            assert_eq!(log10f_glibc(10f32.powi(n)), n as f32, "10^{n}");
        }
    }

    /// The bits of every 257th float bit pattern, hashed: the hash recorded from this port after
    /// the verifier's `verify float` found 0 differences on all 2^32 inputs (every platform must
    /// give it).
    #[test]
    fn sampled_hash() {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for i in (0..=u32::MAX).step_by(257) {
            h = (h ^ u64::from(log10f_glibc(f32::from_bits(i)).to_bits()))
                .wrapping_mul(0x0100_0000_01b3);
        }
        assert_eq!(h, SAMPLED_HASH);
    }

    const SAMPLED_HASH: u64 = 0x9bc9_21a9_ac3a_45e9;
}
