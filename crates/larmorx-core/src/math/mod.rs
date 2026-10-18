// SPDX-License-Identifier: Apache-2.0
//! Correctly rounded transcendental functions (CLAUDE.md rule 5, decided 2026-10-09).
//!
//! Each function returns the exact result rounded to the nearest `f64` (ties to even). So
//! results are the same on every platform, and agree with glibc, and so with ITK, ANTs and
//! AFNI on Linux, except where glibc itself is not correctly rounded (about 0.1 % of
//! calls). They are pure-Rust ports of CORE-MATH (MIT, commit `040ee482a8ca`, see
//! `UPSTREAM.md`), verified against mpmath on CORE-MATH's worst-case inputs and millions of
//! random inputs (`asin`, `acos`, `atan2`, `cbrt`, `pow`: against CORE-MATH's C on its worst
//! cases and random inputs). Background: `docs/findings/platform-math.md`.

mod acos;
mod asin;
mod atan2;
mod cbrt;
mod cos;
pub(crate) mod dint;
mod exp;
mod expf;
mod log;
mod logf;
mod pow;
mod powf;
mod powi;
mod sin;
mod sinf;

pub use powi::powi;

// Speed: without hardware FMA in the generated code (baseline x86-64), every `mul_add` in the
// kernels is a call into the platform's `fma`. On x86-64 CPUs with FMA, a copy of each kernel
// compiled with FMA enabled is used instead. The results are identical: `mul_add` is
// correctly rounded on both paths, and so is every result. The kernels' helpers are
// `#[inline(always)]` so that they are compiled into the FMA copy. Other architectures
// (aarch64) have FMA in their baseline.

macro_rules! dispatch {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[inline]
        #[must_use]
        pub fn $name(x: f64) -> f64 {
            #[cfg(target_arch = "x86_64")]
            if std::arch::is_x86_feature_detected!("fma") {
                return fma::$name(x);
            }
            $name::$name(x)
        }
    };
}

dispatch!(
    /// The cosine of `x`, correctly rounded.
    cos
);
dispatch!(
    /// `e^x`, correctly rounded.
    exp
);
dispatch!(
    /// The natural logarithm of `x`, correctly rounded.
    log
);
dispatch!(
    /// The sine of `x`, correctly rounded.
    sin
);
dispatch!(
    /// The arcsine of `x`, correctly rounded (CORE-MATH's `cr_asin`). ITK's `std::asin`
    /// (glibc's `asin`), as in `Euler3DTransform::ComputeMatrixParameters`.
    asin
);
dispatch!(
    /// The arccosine of `x`, correctly rounded (CORE-MATH's `cr_acos`). ITK's `std::acos`
    /// (glibc's `acos`), as in `Rigid2DTransform::ComputeMatrixParameters`.
    acos
);
dispatch!(
    /// The cube root of `x`, correctly rounded (CORE-MATH's `cr_cbrt`). ITK's
    /// `itk::Math::cbrt` (glibc's `cbrt`, which is not correctly rounded), as in
    /// `Similarity3DTransform::SetMatrix`.
    cbrt
);

/// `atan2(y, x)`, correctly rounded (CORE-MATH's `cr_atan2`). ITK's `std::atan2` (glibc's
/// `atan2`), as in `Euler3DTransform::ComputeMatrixParameters`.
#[inline]
#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("fma") {
        return fma::atan2(y, x);
    }
    atan2::atan2(y, x)
}

/// `x^y`, correctly rounded (CORE-MATH's `cr_pow`). ITK's `std::pow` (glibc's `pow`, which is
/// not correctly rounded), as in `JensenHavrdaCharvatTsallisPointSetToPointSetMetricv4`.
#[inline]
#[must_use]
pub fn pow(x: f64, y: f64) -> f64 {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("fma") {
        return fma::pow(x, y);
    }
    pow::pow(x, y)
}

/// `x^y` for `f32`, correctly rounded (CORE-MATH's `cr_powf`). It has no FMA-compiled copy:
/// it is only used where speed does not matter (ImageMath's `^`).
pub use powf::powf;

/// `e^x` for `f32`, correctly rounded (CORE-MATH's `cr_expf`; no fused multiply-adds, so no
/// FMA-compiled copy). ITK's `std::exp(float)` (glibc's `expf`).
pub use expf::expf;

/// The natural logarithm for `f32`, correctly rounded (CORE-MATH's `cr_logf`; no fused
/// multiply-adds). ITK's `std::log(float)` (glibc's `logf`).
pub use logf::logf;

/// The sine and cosine for `f32`, correctly rounded (CORE-MATH's `cr_sinf` and `cr_cosf`).
/// ITK's `std::sin(float)` and `std::cos(float)` (glibc's `sinf`, `cosf` and `sincosf`), as in
/// `Euler3DTransform<float>`. Their only fused multiply-adds are explicit `mul_add` calls on
/// tiny arguments, so they have no FMA-compiled copy.
pub use sinf::{cosf, sinf};

/// The kernels compiled with FMA enabled. AUDIT (unsafe): each call is guarded by
/// `is_x86_feature_detected!("fma")` in `dispatch!`. Calling a `#[target_feature]` function
/// requires `unsafe` only because the feature must be present, and the guard ensures it.
#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)]
mod fma {
    macro_rules! fma_copy {
        ($name:ident) => {
            #[inline]
            pub(super) fn $name(x: f64) -> f64 {
                #[target_feature(enable = "fma")]
                fn kernel(x: f64) -> f64 {
                    super::$name::$name(x)
                }
                // SAFETY: only called after `is_x86_feature_detected!("fma")` returned true.
                unsafe { kernel(x) }
            }
        };
    }
    fma_copy!(cos);
    fma_copy!(exp);
    fma_copy!(log);
    fma_copy!(sin);
    fma_copy!(asin);
    fma_copy!(acos);
    fma_copy!(cbrt);

    #[inline]
    pub(super) fn atan2(y: f64, x: f64) -> f64 {
        #[target_feature(enable = "fma")]
        fn kernel(y: f64, x: f64) -> f64 {
            super::atan2::atan2(y, x)
        }
        // SAFETY: only called after `is_x86_feature_detected!("fma")` returned true.
        unsafe { kernel(y, x) }
    }

    #[inline]
    pub(super) fn pow(x: f64, y: f64) -> f64 {
        #[target_feature(enable = "fma")]
        fn kernel(x: f64, y: f64) -> f64 {
            super::pow::pow(x, y)
        }
        // SAFETY: only called after `is_x86_feature_detected!("fma")` returned true.
        unsafe { kernel(x, y) }
    }
}

#[cfg(test)]
mod tests {
    /// The FMA copies give the same bits as the portable kernels.
    #[test]
    fn fma_copies_are_bit_identical() {
        let mut s = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..200_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let x = f64::from_bits(s);
            for (a, b) in [
                (super::cos(x), super::cos::cos(x)),
                (super::sin(x), super::sin::sin(x)),
                (super::exp(x), super::exp::exp(x)),
                (super::log(x.abs()), super::log::log(x.abs())),
                (super::cbrt(x), super::cbrt::cbrt(x)),
                (super::atan2(x, 1.5), super::atan2::atan2(x, 1.5)),
                (super::atan2(-0.25, x), super::atan2::atan2(-0.25, x)),
                (super::pow(x.abs(), 0.75), super::pow::pow(x.abs(), 0.75)),
                (super::pow(1.0625, x), super::pow::pow(1.0625, x)),
            ] {
                assert!(
                    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()),
                    "{x:e}"
                );
            }
            // Arguments in [-1, 1] for the inverse sine and cosine.
            let u = (s >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0;
            for (a, b) in [
                (super::asin(u), super::asin::asin(u)),
                (super::acos(u), super::acos::acos(u)),
            ] {
                assert!(
                    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()),
                    "{x:e}"
                );
            }
        }
    }
}
