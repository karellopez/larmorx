// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The C library's `pow`, `exp` and `log` as FSL's programs call them on the oracle machine (x86-64
//! Linux, glibc 2.35, a CPU with FMA): BET's initial radius (`pow(x, 1/3)`, `pow(x, 0.5)`) and its
//! threshold exponent (`pow(f, 0.275)`); FAST's log intensities and Gaussian likelihoods (`log`,
//! `exp`); newimage's Gaussian kernels (`exp`). glibc's are not correctly rounded: about 0.09 % of
//! `pow`'s calls differ from the correctly rounded result in their last bit, so the replicas carry
//! glibc's.
//!
//! glibc 2.35's `pow`, `exp` and `log` (`__pow_fma`, `__exp_fma`, `__log_fma`: glibc picks the
//! variant by CPU at load time, this is the oracle's) are Arm's Optimized Routines, contributed by
//! Arm. Since 2026-11-04 their translation from Arm's MIT originals lives once, in the main
//! package's `larmorx_core::math` (`exp_glibc`, `log_glibc`, `pow_glibc`), and this module
//! re-exports it under the C names. That copy was this module's own translation until then, proved
//! equal to it and to glibc itself on more than 10^9 arguments of each function
//! (`validation/investigations/glibc-maths/`); `tools/libm-verify` keeps comparing them with the C
//! library as FSL's binaries bind it. The SVID-compatible wrapper FSL's binaries bind for `log`
//! ([`log_compat`]) is this crate's.

pub use larmorx_core::math::{exp_glibc as exp, log_glibc as log, pow_glibc as pow};

/// `log@GLIBC_2.2.5`, the symbol FSL's binaries bind (they are built against an older C
/// library): glibc's SVID-compatible wrapper (`w_log_compat.c`) around [`log`]. For `x <= 0` it
/// returns `__kernel_standard`'s values: `-inf` for `±0`, and for a negative number (or `-inf`)
/// C's `NAN`, the positive quiet NaN, where [`log`] gives SSE's default NaN (`-nan`). A NaN goes
/// to [`log`] (quieted).
#[must_use]
pub fn log_compat(x: f64) -> f64 {
    if x <= 0.0 {
        if x == 0.0 {
            return f64::NEG_INFINITY;
        }
        return f64::from_bits(0x7ff8_0000_0000_0000);
    }
    log(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_values() {
        assert_eq!(exp(0.0), 1.0);
        assert_eq!(exp(-0.0), 1.0);
        assert_eq!(exp(1.0), std::f64::consts::E);
        assert_eq!(exp(f64::NEG_INFINITY), 0.0);
        assert_eq!(exp(f64::INFINITY), f64::INFINITY);
        assert!(exp(f64::NAN).is_nan());
        assert_eq!(exp(1000.0), f64::INFINITY);
        assert_eq!(exp(-1000.0), 0.0);
        assert!(exp(-740.0) > 0.0 && exp(-740.0) < 1e-320);
        assert!(exp(709.0) > 8.2e307);
        for &x in &[-0.5f64, -2.7222222222222223, 3.3, -12.5, 1e-10] {
            let q = x.exp();
            assert!((exp(x) - q).abs() <= 2e-16 * q, "{x}");
        }
    }

    #[test]
    fn exact_and_special_values() {
        assert_eq!(pow(10.0, 2.0), 100.0);
        assert_eq!(pow(10.0, 11.0), 1e11);
        assert_eq!(pow(2.0, 0.5), std::f64::consts::SQRT_2);
        assert_eq!(pow(0.0, 0.275), 0.0);
        assert_eq!(pow(1.0, f64::NAN), 1.0);
        assert!(pow(-2.0, 0.5).is_nan());
        assert_eq!(pow(-2.0, 3.0), -8.0);
        assert_eq!(pow(f64::INFINITY, -1.0), 0.0);
        assert_eq!(pow(1e300, 2.0), f64::INFINITY);
        assert_eq!(pow(1e-300, 2.0), 0.0);
        assert_eq!(pow(5e-324, 0.5), 2.2227587494850775e-162);
    }

    #[test]
    fn close_to_the_true_value() {
        for &(x, y) in &[
            (0.2f64, 0.275f64),
            (123456.7, 1.0 / 3.0),
            (8.0, 1.0 / 3.0),
            (0.5, 0.275),
        ] {
            let p = pow(x, y);
            let q = (y * x.ln()).exp();
            assert!((p - q).abs() <= 1e-15 * q, "{x} {y} {p} {q}");
        }
    }
}
