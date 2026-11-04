// SPDX-License-Identifier: Apache-2.0 AND MIT
// Translated to Rust from Arm's Optimized Routines v19.11 (github.com/ARM-software/optimized-routines,
// tag v19.11): math/logf.c and math/logf_data.c (__logf_data). Copyright (c) 2017-2018, Arm
// Limited, under the MIT licence (its text and notice are in LICENSES/MIT-arm-optimized-routines.txt
// and NOTICE). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
//! The single-precision natural logarithm with the bits of glibc 2.35's `logf` on x86-64 (the FMA
//! build, `__logf_fma`, Arm's `logf.c` compiled with `-mfma -mavx2`), on every platform.
//!
//! GCC contracts each `a + b * c` of it into one fused multiply-add in that build; the
//! translation writes each as `mul_add`, which is correctly rounded on every platform. The NaN
//! results are SSE's (`super::x86_nan`). Not correctly rounded (Arm: 0.82 ulp; 416,908 of the
//! positive floats); [`super::logf`] is, and is larmorx's default. This one serves `reproduce`
//! (FSL's programs: mcflirt's mutual-information costs, FAST's variances, `fslmaths -log`).
//! Until 2026-11-04 the FSL replicas' core carried its own translation (`larmorx-nc-core`'s
//! `glibc_float`); it now calls this one. Proved equal to the system's `logf` (glibc 2.35,
//! x86-64) and to that translation on all 2^32 inputs, NaN bits included
//! (`validation/investigations/glibc-maths/`).
//!
//! This is the current symbol (`logf@@GLIBC_2.27`): the logarithm of a negative number is SSE's
//! default NaN (`-nan`). The older symbol an old C library's programs bind (`logf@GLIBC_2.2.5`,
//! an SVID-compatible wrapper) gives C's positive `NAN` there; the FSL replicas keep that
//! wrapper (`larmorx-nc-core`'s `logf_compat`).

use super::x86_nan::invalid_f32;

/// The double `1.mantissa * 2^exp`, `m` holding the leading 1 and the 52 fraction bits (a C
/// hexadecimal float literal `0x1.<13 hex digits>p<exp>`).
const fn hexf(m: u64, exp: i32) -> f64 {
    let frac = m & ((1u64 << 52) - 1);
    f64::from_bits((((1023 + exp) as u64) << 52) | frac)
}

/// `__logf_data.tab`: 16 entries `(1/c, log c)`.
const TAB: [(f64, f64); 16] = [
    (hexf(0x1661ec79f8f3be, 0), -hexf(0x157bf7808caade, -2)),
    (hexf(0x1571ed4aaf883d, 0), -hexf(0x12bef0a7c06ddb, -2)),
    (hexf(0x149539f0f010b0, 0), -hexf(0x101eae7f513a67, -2)),
    (hexf(0x13c995b0b80385, 0), -hexf(0x1b31d8a68224e9, -3)),
    (hexf(0x130d190c8864a5, 0), -hexf(0x16574f0ac07758, -3)),
    (hexf(0x125e227b0b8ea0, 0), -hexf(0x11aa2bc79c8100, -3)),
    (hexf(0x11bb4a4a1a343f, 0), -hexf(0x1a4e76ce8c0e5e, -4)),
    (hexf(0x112358f08ae5ba, 0), -hexf(0x11973c5a611ccc, -4)),
    (hexf(0x10953f419900a7, 0), -hexf(0x1252f438e10c1e, -5)),
    (1.0, 0.0),
    (hexf(0x1e608cfd9a47ac, -1), hexf(0x1aa5aa5df25984, -5)),
    (hexf(0x1ca4b31f026aa0, -1), hexf(0x1c5e53aa362eb4, -4)),
    (hexf(0x1b2036576afce6, -1), hexf(0x1526e57720db08, -3)),
    (hexf(0x19c2d163a1aa2d, -1), hexf(0x1bc2860d224770, -3)),
    (hexf(0x1886e6037841ed, -1), hexf(0x11058bc8a07ee1, -2)),
    (hexf(0x1767dcf5534862, -1), hexf(0x14043057b6ee09, -2)),
];
/// `__logf_data.ln2`.
const LN2: f64 = hexf(0x162e42fefa39ef, -1);
/// `__logf_data.poly`.
const POLY: [f64; 3] = [
    -hexf(0x100ea348b88334, -2),
    hexf(0x15575b0be00b6a, -2),
    -hexf(0x1ffffef20a4123, -2),
];

/// glibc 2.35's `logf(x)` on x86-64 (the FMA build), with its bits on every platform: `+0` for 1,
/// `-∞` for `±0`, `+∞` for `+∞`, SSE's default NaN for a negative number or `-∞`, a NaN quieted.
#[inline(always)]
#[must_use]
pub fn logf_glibc(x: f32) -> f32 {
    const OFF: u32 = 0x3f33_0000;
    let mut ix = x.to_bits();
    // WANT_ROUNDING: log(1) = +0 in every rounding mode.
    if ix == 0x3f80_0000 {
        return 0.0;
    }
    if ix.wrapping_sub(0x0080_0000) >= 0x7f80_0000 - 0x0080_0000 {
        // x < 0x1p-126 or inf or nan.
        if ix.wrapping_mul(2) == 0 {
            // __math_divzerof(1): -1/0.
            return f32::NEG_INFINITY;
        }
        if ix == 0x7f80_0000 {
            return x;
        }
        if (ix & 0x8000_0000) != 0 || ix.wrapping_mul(2) >= 0xff00_0000 {
            return invalid_f32(x);
        }
        // A subnormal: normalized (`x * 0x1p23f`, exact).
        ix = (x * f32::from_bits(0x4b00_0000)).to_bits();
        ix = ix.wrapping_sub(23 << 23);
    }
    // x = 2^k z, z in [OFF, 2 OFF] and exact; c near the centre of z's subinterval.
    let tmp = ix.wrapping_sub(OFF);
    let i = ((tmp >> (23 - 4)) % 16) as usize;
    let k = (tmp as i32) >> 23;
    let iz = ix.wrapping_sub(tmp & (0x1ff << 23));
    let (invc, logc) = TAB[i];
    let z = f64::from(f32::from_bits(iz));
    // log(x) = log1p(z/c - 1) + log(c) + k ln 2, every `a + b * c` fused.
    let r = z.mul_add(invc, -1.0);
    let y0 = f64::from(k).mul_add(LN2, logc);
    let r2 = r * r;
    let mut y = POLY[1].mul_add(r, POLY[2]);
    y = POLY[0].mul_add(r2, y);
    y = y.mul_add(r2, y0 + r);
    y as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_the_c_literals() {
        assert_eq!(LN2, std::f64::consts::LN_2);
    }

    #[test]
    fn special_values() {
        assert_eq!(logf_glibc(1.0).to_bits(), 0);
        assert_eq!(logf_glibc(0.0), f32::NEG_INFINITY);
        assert_eq!(logf_glibc(-0.0), f32::NEG_INFINITY);
        assert_eq!(logf_glibc(f32::INFINITY), f32::INFINITY);
        // SSE's default NaN for a NaN made from numbers, the operand quieted for a NaN, on every
        // platform.
        assert_eq!(logf_glibc(-1.0).to_bits(), 0xffc0_0000);
        assert_eq!(logf_glibc(f32::NEG_INFINITY).to_bits(), 0xffc0_0000);
        assert_eq!(
            logf_glibc(f32::from_bits(0xffc1_2345)).to_bits(),
            0xffc1_2345
        );
        assert_eq!(
            logf_glibc(f32::from_bits(0x7f80_0001)).to_bits(),
            0x7fc0_0001
        );
        let near = |a: f32, b: f32| (a - b).abs() <= 2.0 * f32::EPSILON * b.abs().max(1e-30);
        for x in [1e-40f32, 1e-30, 0.5, 2.0, 1579.0, 1e30] {
            assert!(near(logf_glibc(x), f64::from(x).ln() as f32), "{x}");
        }
    }

    /// A hash of every 4099th input, recorded from glibc 2.35 on Linux x86-64
    /// (`validation/investigations/glibc-maths/`).
    #[test]
    fn matches_glibc_on_a_stride() {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut x = 0u64;
        while x <= u64::from(u32::MAX) {
            let v = logf_glibc(f32::from_bits(x as u32));
            h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x0100_0000_01b3);
            x += 4099;
        }
        assert_eq!(h, STRIDE);
    }

    const STRIDE: u64 = 0xd0ca_1f24_4b87_9c6b;
}
