// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The single-precision `sinf`, `cosf`, `sincosf`, `logf`, `expf` and `powf` that FSL's Linux
//! binaries call: glibc 2.35's, as its x86-64 build selects them on a CPU with FMA (`__sinf_fma`,
//! `__cosf_fma`, `__sincosf_fma`, `__logf_fma`, `__expf_fma`, `__powf_fma`, compiled with `-mfma
//! -mavx2`).
//!
//! glibc took these functions from Arm's Optimized Routines (2018) and kept their arithmetic.
//! Since 2026-11-04 their translation from Arm's MIT-licensed originals lives once, in the main
//! package's `larmorx_core::math` (`sinf_glibc`, `cosf_glibc`, `sincosf_glibc`, `logf_glibc`,
//! `expf_glibc`, `powf_glibc`), and this module re-exports it under the C names. That copy was
//! this module's own translation until then, proved equal to it on all 2^32 inputs of each
//! one-argument function and on more than 10^9 pairs of `powf` (NaN bits included), and to glibc
//! itself (`validation/investigations/glibc-maths/`); `tools/glibc-float-verify` keeps comparing
//! them with the C library on all 2^32 inputs. Each fused multiply-add of the FMA build is a
//! `mul_add`, correctly rounded on every platform, and the NaN results are SSE's, so the results
//! are the same bits everywhere.
//!
//! They are not correctly rounded (sinf and cosf to 0.56 ulp, logf to 0.82 ulp): mcflirt's
//! results as built depend on their roundings (`docs/findings/fsl-mcflirt.md`), so the replica
//! uses them where FSL calls the C library's float functions. The SVID-compatible wrapper FSL's
//! binaries bind for `logf` ([`logf_compat`]) is this crate's.

pub use larmorx_core::math::{
    cosf_glibc as cosf, expf_glibc as expf, logf_glibc as logf, powf_glibc as powf,
    sincosf_glibc as sincosf, sinf_glibc as sinf,
};

/// `logf@GLIBC_2.2.5`, the symbol FSL's binaries bind (they are built against an older C
/// library): glibc's SVID-compatible wrapper (`w_logf_compat.c`) around [`logf`]. For `x <= 0`
/// it returns `__kernel_standard_f`'s values: `-inf` for `±0`, and for a negative number (or
/// `-inf`) C's `NAN`, the positive quiet NaN, where [`logf`] gives SSE's default NaN (`-nan`).
#[must_use]
pub fn logf_compat(x: f32) -> f32 {
    if x <= 0.0 {
        if x == 0.0 {
            return f32::NEG_INFINITY;
        }
        return f32::from_bits(0x7fc0_0000);
    }
    logf(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_values() {
        assert!(sinf(f32::INFINITY).is_nan());
        assert!(cosf(f32::NAN).is_nan());
        assert_eq!(sinf(-0.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(cosf(0.0), 1.0);
        assert_eq!(logf(1.0).to_bits(), 0);
        assert_eq!(logf(0.0), f32::NEG_INFINITY);
        assert!(logf(-1.0).is_nan());
        // The bits glibc's x86-64 build gives (SSE's default NaN for a NaN made from numbers,
        // the operand quieted for a NaN), on every platform.
        assert_eq!(sinf(f32::INFINITY).to_bits(), 0xffc0_0000);
        assert_eq!(cosf(f32::NEG_INFINITY).to_bits(), 0xffc0_0000);
        let (s, c) = sincosf(f32::INFINITY);
        assert_eq!((s.to_bits(), c.to_bits()), (0xffc0_0000, 0xffc0_0000));
        assert_eq!(logf(-1.0).to_bits(), 0xffc0_0000);
        assert_eq!(sinf(f32::from_bits(0x7fa0_0001)).to_bits(), 0x7fe0_0001);
        assert_eq!(logf(f32::from_bits(0xffc1_2345)).to_bits(), 0xffc1_2345);
        assert_eq!(logf(f32::INFINITY), f32::INFINITY);
        let (s, c) = sincosf(1e-30);
        assert_eq!((s, c), (1e-30, 1.0));
    }

    /// `powf@GLIBC_2.2.5`'s values on Linux x86-64 (glibc 2.35, FMA build, `tools/libm-verify`'s
    /// reference): the main path, a negative base, poles, a subnormal base, overflow, underflow.
    #[test]
    fn powf_values() {
        let cases: [(f32, f32, u32); 11] = [
            (2.0, 0.5, 0x3fb5_04f3),
            (3.7, 2.0, 0x415b_0a3e),
            (0.04, 2.5, 0x39a7_c5ac),
            (1234.0, 0.6667, 0x42e6_263e),
            (-8.0, 3.0, 0xc400_0000),
            (-8.0, 0.5, 0xffc0_0000),
            (0.0, -1.0, 0x7f80_0000),
            (-0.0, -3.0, 0xff80_0000),
            (1e-40, 0.5, 0x1e3c_e4e7),
            (10.0, 40.0, 0x7f80_0000),
            (10.0, -50.0, 0),
        ];
        for (x, y, want) in cases {
            assert_eq!(powf(x, y).to_bits(), want, "powf({x:e}, {y:e})");
        }
        assert_eq!(powf(f32::NAN, 0.0), 1.0);
        assert_eq!(powf(1.0, f32::NAN), 1.0);
        assert!(powf(f32::NAN, 2.0).is_nan());
        assert_eq!(powf(0.5, f32::INFINITY), 0.0);
        assert_eq!(powf(2.0, f32::INFINITY), f32::INFINITY);
    }

    /// Values glibc 2.35 (FMA build) returns, from the verification tool on Linux x86-64; among
    /// them inputs where it is not correctly rounded.
    #[test]
    fn known_values() {
        let near = |a: f32, b: f32| (a - b).abs() <= 2.0 * f32::EPSILON * b.abs().max(1e-30);
        for x in [0.3f32, 1.0, 2.5, 100.0, 1e6, -7.25] {
            let (s, c) = sincosf(x);
            assert_eq!(s.to_bits(), sinf(x).to_bits(), "{x}");
            assert_eq!(c.to_bits(), cosf(x).to_bits(), "{x}");
            assert!(near(s, f64::from(x).sin() as f32), "{x}");
            assert!(near(c, f64::from(x).cos() as f32), "{x}");
        }
        for x in [1e-30f32, 0.5, 1.0, 2.0, 1579.0, 1e30] {
            assert!(near(logf(x), f64::from(x).ln() as f32), "{x}");
        }
    }
}
