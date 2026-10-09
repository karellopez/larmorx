//! Correctly rounded transcendental functions (CLAUDE.md rule 5, decided 2026-10-09).
//!
//! Each function returns the exact result rounded to the nearest `f64` (ties to even). So
//! results are the same on every platform, and agree with glibc, and so with ITK, ANTs and
//! AFNI on Linux, except where glibc itself is not correctly rounded (about 0.1 % of
//! calls). They are pure-Rust ports of CORE-MATH (MIT, commit `040ee482a8ca`, see
//! `UPSTREAM.md`), verified against mpmath on CORE-MATH's worst-case inputs and millions of
//! random inputs. Background: `docs/findings/platform-math.md`.

mod cos;
pub(crate) mod dint;
mod exp;
mod log;
mod sin;

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
            ] {
                assert!(
                    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()),
                    "{x:e}"
                );
            }
        }
    }
}
