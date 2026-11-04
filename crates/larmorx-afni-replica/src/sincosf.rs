// SPDX-License-Identifier: Apache-2.0
//! The single-precision `sinf` and `cosf` AFNI's Linux builds call, bit for bit.
//!
//! AFNI's weighted-sinc interpolation (`thd_shift2.c`) calls the C library's `sinf` and `cosf`.
//! On Linux that is glibc, whose float sine and cosine are fast but not correctly rounded (at
//! most 0.56 ulp), so a correctly rounded function gives a different `f32` for some arguments.
//! glibc 2.35 (the C library AFNI runs with on Ubuntu 22.04) took these two functions from Arm's
//! Optimized Routines (2018) and kept their arithmetic.
//!
//! On x86-64, glibc compiles the same C code twice and picks one build at load time: with FMA
//! and AVX2 (every x86-64 CPU since 2013, and the build AFNI's oracle runs), the compiler fuses
//! each `a + b * c` into one rounding; without them (SSE2), each product and sum rounds.
//! [`sinf`] and [`cosf`] are the FMA build.
//!
//! Since 2026-11-04 the translation from Arm's MIT-licensed originals (this module's from
//! 2026-10-29) lives once, in `larmorx_core::math` (`sinf_glibc`, `cosf_glibc`; the SSE2 build
//! `sinf_glibc_sse2`, `cosf_glibc_sse2`), shared with the FSL replicas and the clean-room
//! originals' `reproduce`, and this module re-exports it. That copy is proved equal to this
//! module's translation on all 2^32 inputs of each function and each build, NaN bits included,
//! and to the C library of the development machine (glibc 2.35, FMA build)
//! (`validation/investigations/glibc-maths/`). The translation was proved, on all inputs, equal to
//! the earlier translation from glibc 2.35 that the replica used until 2026-10-29 (itself checked
//! against glibc on all inputs, both builds): `crates-gpl/tools/sincosf-verify`.

pub use larmorx_core::math::{cosf_glibc as cosf, sinf_glibc as sinf};

/// The SSE2 builds (`__sinf_sse2`, `__cosf_sse2`), which glibc runs on x86-64 CPUs without
/// FMA: for the verification tool (`crates-gpl/tools/sincosf-verify`) only.
#[cfg(feature = "verification")]
#[doc(hidden)]
pub mod sse2 {
    pub use larmorx_core::math::{cosf_glibc_sse2 as cosf, sinf_glibc_sse2 as sinf};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_values() {
        assert_eq!(sinf(0.0).to_bits(), 0.0f32.to_bits());
        assert_eq!(sinf(-0.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(cosf(0.0), 1.0);
        assert_eq!(cosf(-0.0), 1.0);
        for v in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
            assert!(sinf(v).is_nan() && cosf(v).is_nan());
        }
        let tiny = f32::from_bits(0x3970_0000);
        assert_eq!(sinf(tiny), tiny);
        assert_eq!(cosf(tiny), 1.0);
    }

    /// The C library's values at the branch limits and where it is not correctly rounded, on
    /// the arguments 3dTshift's weighted sinc passes (in [0, 32]), measured on the development
    /// machine (glibc 2.35, FMA build): `(input, sinf, cosf)` as bits, from
    /// `crates-gpl/tools/sincosf-verify --vectors 4`.
    const C_LIBRARY: &[(u32, u32, u32)] = &[
        (0x3980_0000, 0x3980_0000, 0x3f80_0000),
        (0x3f49_0fdb, 0x3f35_04f3, 0x3f35_04f3),
        (0x3f49_0fda, 0x3f35_04f3, 0x3f35_04f4),
        (0x42f0_0000, 0x3f14_a2ef, 0x3f50_6e2a),
        (0x42ef_ffff, 0x3f14_a287, 0x3f50_6e74),
        (0x3c24_017e, 0x3c24_00cb, 0x3f7f_fcb7),
        (0x3d9a_e188, 0x3d9a_bbc0, 0x3f7f_44af),
        (0x3f11_ebe9, 0x3f0a_25a7, 0x3f57_869d),
        (0x4088_f5d9, 0xbf68_70d6, 0xbed6_8b48),
    ];

    #[test]
    fn matches_the_c_library() {
        for &(x, s, c) in C_LIBRARY {
            let x = f32::from_bits(x);
            assert_eq!(sinf(x).to_bits(), s, "sinf({x:e})");
            assert_eq!(cosf(x).to_bits(), c, "cosf({x:e})");
        }
    }

    #[test]
    fn close_to_correctly_rounded() {
        // Within one float ulp of the correctly rounded double result, everywhere.
        let mut x = 1.0e-3f32;
        while x < 1.0e6 {
            for v in [x, -x] {
                let (s, c) = (
                    larmorx_core::math::sin(f64::from(v)),
                    larmorx_core::math::cos(f64::from(v)),
                );
                let ulp = |r: f64| f64::from(r as f32).abs() * f64::from(f32::EPSILON);
                assert!(
                    (f64::from(sinf(v)) - s).abs() <= ulp(s).max(1e-45),
                    "sinf({v})"
                );
                assert!(
                    (f64::from(cosf(v)) - c).abs() <= ulp(c).max(1e-45),
                    "cosf({v})"
                );
            }
            x *= 1.000_37;
        }
    }
}
