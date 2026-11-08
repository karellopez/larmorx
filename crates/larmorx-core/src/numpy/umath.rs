// SPDX-License-Identifier: Apache-2.0
//! numpy's elementwise `exp` and `log` with the bits of its x86-64 Linux wheels (numpy 2.3.5 and
//! 2.5.4, measured on this project's machines: `validation/investigations/t2smap/numerics/`).
//!
//! - `float32` `log`: numpy's own kernel on CPUs with AVX2 and FMA3 or AVX-512F
//!   ([`crate::math::logf_numpy`]); on older x86-64 CPUs numpy calls the C library's `logf`
//!   (`logf@GLIBC_2.27`, [`crate::math::logf_glibc`]).
//! - `float64` `exp` and `log`: on CPUs without AVX-512 numpy calls the C library's (glibc 2.35
//!   on this project's machines), through the old symbols its manylinux wheels bind
//!   (`exp@GLIBC_2.2.5`, `log@GLIBC_2.2.5`): the same values as the current ones
//!   ([`crate::math::exp_glibc`], [`crate::math::log_glibc`]) except that the logarithm of a
//!   negative number (or `-inf`) is C's positive `NAN`, not SSE's default NaN. On CPUs with
//!   AVX-512 (AVX512_SKX) numpy uses Intel's SVML kernels instead, which differ from glibc's
//!   and from the correctly rounded results in a few per cent of arguments (`exp`); larmorx
//!   follows the first path (as `OPENBLAS_CORETYPE=Haswell` for BLAS).
//!
//! These serve `reproduce`; larmorx's default is the correctly rounded functions
//! ([`crate::math::exp`], [`crate::math::log`], [`crate::math::logf`]).

/// numpy's `float32` `log` (x86-64, AVX2/FMA3 or AVX-512F path).
#[inline]
#[must_use]
pub fn log_f32(x: f32) -> f32 {
    crate::math::logf_numpy(x)
}

/// numpy's `float64` `exp` (x86-64 Linux without AVX-512: glibc 2.35's `exp@GLIBC_2.2.5`).
#[inline]
#[must_use]
pub fn exp_f64(x: f64) -> f64 {
    crate::math::exp_glibc(x)
}

/// numpy's `float64` `log` (x86-64 Linux without AVX-512: glibc 2.35's `log@GLIBC_2.2.5`; a
/// negative argument gives C's `NAN`, `0x7ff8000000000000`).
#[inline]
#[must_use]
pub fn log_f64(x: f64) -> f64 {
    if x < 0.0 {
        f64::from_bits(0x7ff8_0000_0000_0000)
    } else {
        crate::math::log_glibc(x)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn negative_logarithms_are_cs_nan() {
        assert_eq!(super::log_f64(-1.0).to_bits(), 0x7ff8_0000_0000_0000);
        assert_eq!(
            super::log_f64(f64::NEG_INFINITY).to_bits(),
            0x7ff8_0000_0000_0000
        );
        assert_eq!(super::log_f64(-0.0), f64::NEG_INFINITY);
        assert_eq!(super::log_f64(1.0), 0.0);
        assert_eq!(super::exp_f64(0.0), 1.0);
    }
}
