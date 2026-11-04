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
mod atanf_glibc;
mod cbrt;
mod cbrtf;
mod cbrtf_glibc;
mod choice;
mod cos;
pub(crate) mod dint;
mod exp;
mod exp_glibc;
mod expf;
mod expf_glibc;
mod log;
mod log_glibc;
mod logf;
mod logf_glibc;
mod pow;
mod pow_glibc;
mod powf;
mod powf_glibc;
mod powi;
mod sin;
mod sincos_glibc;
mod sincos_glibc_tables;
mod sincosf_glibc;
mod sinf;
mod tan;
mod tanh;
mod tanh_glibc;
mod tanhf;
mod x86_nan;

pub use powi::powi;

/// The C library functions of one call of a clean-room original, with the last bits its mode
/// asks for: correctly rounded by default, glibc 2.35's with `reproduce` (GLIBC-07), and the
/// witness its notices name (`choice.rs`).
pub use choice::Libm;

/// `tanh(x)` for `f32`, correctly rounded (CORE-MATH's `cr_tanhf`; its only fused
/// multiply-adds are explicit `mul_add` calls on tiny arguments, so it has no FMA-compiled
/// copy). AFNI's 3dUnifize squash calls glibc's `tanhf`, which is not correctly rounded.
pub use tanhf::tanhf;

/// The cube root for `f32`, correctly rounded (CORE-MATH's `cr_cbrtf`; no fused
/// multiply-adds). AFNI's 3dUnifize `-EPI` calls glibc's `cbrtf`.
pub use cbrtf::cbrtf;

/// The cube root for `f32` with glibc 2.35's bits (x86-64) on every input, not correctly
/// rounded (GLIBC-05): a clean-room implementation from `specs/cbrtf.md` (Apache-2.0), for
/// `reproduce` where AFNI's result depends on glibc's last bit (3dUnifize `-EPI`'s radius).
/// No fused multiply-adds: the same bits on every platform.
pub use cbrtf_glibc::cbrtf_glibc;

/// `tanh(x)` for `f64` with glibc 2.35's bits (x86-64), not correctly rounded (GLIBC-06):
/// fdlibm's `tanh` over fdlibm's `expm1` (Sun's notice), for `reproduce` where FSL's result
/// depends on glibc's last bit (`bet2`'s smoothness weight, `specs/bet.md` §8.1). No fused
/// multiply-adds: the same bits on every platform.
pub use tanh_glibc::tanh_glibc;

// glibc 2.35's own functions from permissive code (Arm's Optimized Routines, MIT; fdlibm, Sun's
// notice), translated from those originals: the bits of glibc's x86-64 build (on a CPU with FMA,
// the build the oracles run) on every platform, NaN results included (SSE's, `x86_nan`). They are
// not correctly rounded, so larmorx's default is the correctly rounded functions above; these
// serve `reproduce`, where an original program's result depends on glibc's last bit, and the FSL
// and AFNI replicas (`larmorx-nc-core`, `larmorx-afni-replica`), which call them. Proved equal to
// glibc 2.35 itself (exhaustively for the one-argument float functions, on more than 10^9
// arguments for the others): `validation/investigations/glibc-maths/`. glibc's double `sin`,
// `cos`, `tan`, `asin`, `acos`, `atan` (IBM's LGPL code) are not among them (GLIBC-07): `sin`,
// `cos` and `sincos` with glibc's bits are a clean-room implementation of `specs/glibc-dbl.md`
// (`sin_glibc`, `cos_glibc`, `sincos_glibc` below); `tan`, `asin`, `acos` and `atan` stay
// correctly rounded in `reproduce` too (the user's decision of 2026-11-04, spec §9).

/// The arctangent for `f32` with glibc 2.35's bits (fdlibm's `atanf`, Sun's notice; no fused
/// multiply-adds, one build on x86-64). FSL's `fslmaths -atan` on float images.
pub use atanf_glibc::atanf_glibc;

/// `atan2(y, x)` for `f32` with glibc 2.35's bits (fdlibm's `atan2f`). FSL's `rotmat2euler`
/// (mcflirt's and FLIRT's Euler angles) calls `atan2` with `float` arguments.
pub use atanf_glibc::atan2f_glibc;

#[doc(hidden)]
pub use atanf_glibc::atanf_glibc_with_at0;

/// The sine and cosine for `f32` as glibc 2.35's x86-64 build without FMA computes them
/// (`__sinf_sse2`, `__cosf_sse2`, `__sincosf_sse2`: glibc's choice on x86-64 CPUs without FMA):
/// each product and sum rounded, no fused multiply-adds. [`sinf_glibc`], [`cosf_glibc`] and
/// [`sincosf_glibc`] are the FMA build, the oracles'.
pub use sincosf_glibc::{cosf_glibc_sse2, sincosf_glibc_sse2, sinf_glibc_sse2};

/// The same function with its kernel compiled with FMA on x86-64 CPUs that have it (the results
/// are identical: `mul_add` is correctly rounded on both paths; only speed differs).
macro_rules! glibc_dispatch {
    ($(#[$doc:meta])* $module:ident :: $name:ident ($($arg:ident : $t:ty),*) -> $ret:ty) => {
        $(#[$doc])*
        #[inline]
        #[must_use]
        pub fn $name($($arg: $t),*) -> $ret {
            #[cfg(target_arch = "x86_64")]
            if std::arch::is_x86_feature_detected!("fma") {
                return fma::$name($($arg),*);
            }
            $module::$name($($arg),*)
        }
    };
}

glibc_dispatch!(
    /// The sine with glibc 2.35's bits (x86-64, the FMA build glibc runs on CPUs with FMA and
    /// AVX2), not correctly rounded (GLIBC-07; [`sin`] is, and is larmorx's default), for
    /// `reproduce` only. A clean-room implementation (Apache-2.0) of `specs/glibc-dbl.md`: glibc
    /// 2.35's bits on every argument tested (the black-box record's 3.2 × 10^9 arguments and its
    /// special ones, and fresh arguments against the system's `sin`:
    /// `validation/investigations/cleanroom-glibc-dbl/`), with the spec's one theoretical
    /// exception (§5.3: table `R` by its definition, 18 low parts not glibc's; a different
    /// result is possible about once in 2^53 evaluations reading those rows, none is known).
    /// glibc's `tan`, `asin`, `acos`, `atan` stay correctly rounded in `reproduce` ([`tan`],
    /// [`asin`], [`acos`], [`atan2`]; spec §9, §10.3: they differ from glibc's on 0.05 to 0.25 %
    /// of results).
    sincos_glibc::sin_glibc(x: f64) -> f64
);
glibc_dispatch!(
    /// The cosine with glibc 2.35's bits (x86-64, the FMA build), for `reproduce` only: as
    /// [`sin_glibc`] (GLIBC-07; [`cos`] is the default).
    sincos_glibc::cos_glibc(x: f64) -> f64
);

/// `(sin x, cos x)` with glibc 2.35's bits (x86-64, `sincos`'s single build, without fused
/// multiply-adds; its own paths, so its sine differs from [`sin_glibc`]'s on some arguments in
/// `[0.85546875, 2.43)`), for `reproduce` only: as [`sin_glibc`]. No fused operation, so no
/// FMA-compiled copy.
pub use sincos_glibc::sincos_glibc;

/// The sine and cosine as glibc 2.35's x86-64 build without FMA computes them (glibc's choice
/// on x86-64 CPUs without FMA and AVX2): [`sin_glibc`] and [`cos_glibc`] with every fused
/// operation of the spec a rounded product then a rounded sum (spec §1, §11.2).
pub use sincos_glibc::{cos_glibc_sse2, sin_glibc_sse2};

glibc_dispatch!(
    /// The sine for `f32` with glibc 2.35's bits (x86-64, FMA build: `__sinf_fma`, Arm's
    /// `sinf.c`; at most 0.56 ulp). AFNI's weighted sinc (3dTshift), mcflirt's sinc kernels.
    sincosf_glibc::sinf_glibc(y: f32) -> f32
);
glibc_dispatch!(
    /// The cosine for `f32` with glibc 2.35's bits (x86-64, FMA build: `__cosf_fma`).
    sincosf_glibc::cosf_glibc(y: f32) -> f32
);
glibc_dispatch!(
    /// `(sin y, cos y)` for `f32` with glibc 2.35's bits (x86-64, FMA build: `__sincosf_fma`),
    /// the same bits as [`sinf_glibc`] and [`cosf_glibc`]. mcflirt's and FLIRT's rotation
    /// matrices.
    sincosf_glibc::sincosf_glibc(y: f32) -> (f32, f32)
);
glibc_dispatch!(
    /// The natural logarithm for `f32` with glibc 2.35's bits (x86-64, FMA build: `__logf_fma`,
    /// the current symbol `logf@@GLIBC_2.27`; 0.82 ulp).
    logf_glibc::logf_glibc(x: f32) -> f32
);
glibc_dispatch!(
    /// `e^x` for `f32` with glibc 2.35's bits (x86-64, FMA build: `__expf_fma`; 0.502 ulp).
    expf_glibc::expf_glibc(x: f32) -> f32
);
glibc_dispatch!(
    /// `x^y` for `f32` with glibc 2.35's bits (x86-64, FMA build: `__powf_fma`; 0.82 ulp).
    powf_glibc::powf_glibc(x: f32, y: f32) -> f32
);
glibc_dispatch!(
    /// `e^x` with glibc 2.35's bits (x86-64, FMA build: `__exp_fma`, Arm's `exp.c`; 0.511 ulp).
    exp_glibc::exp_glibc(x: f64) -> f64
);
glibc_dispatch!(
    /// The natural logarithm with glibc 2.35's bits (x86-64, FMA build: `__log_fma`, Arm's
    /// `log.c`, the current symbol `log@@GLIBC_2.29`; 0.519 ulp).
    log_glibc::log_glibc(x: f64) -> f64
);
glibc_dispatch!(
    /// `x^y` with glibc 2.35's bits (x86-64, FMA build: `__pow_fma`, Arm's `pow.c`; 0.52 ulp).
    pow_glibc::pow_glibc(x: f64, y: f64) -> f64
);

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
    /// The tangent of `x`, correctly rounded (CORE-MATH's `cr_tan`). FSL's `fslmaths -tan`
    /// calls glibc's `tan`, which is not correctly rounded (GLIBC-07).
    tan
);
dispatch!(
    /// The hyperbolic tangent of `x`, correctly rounded (CORE-MATH's `cr_tanh`). FSL's `bet2`
    /// calls glibc's `tanh`, which is not correctly rounded ([`tanh_glibc`] has its bits).
    tanh
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

/// [`exp`] of every value of `x`, in place, with exactly [`exp`]'s bits for each (speed: its
/// fast path four values at a time, compiled with AVX2 and FMA where the CPU has them so that
/// the four run in one vector; [`exp`] itself for the values its fast path leaves).
pub fn exp_in_place(x: &mut [f64]) {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma") {
        return fma::exp_in_place(x);
    }
    exp::exp_in_place(x)
}

/// The kernels compiled with FMA enabled. AUDIT (unsafe): each call is guarded by
/// `is_x86_feature_detected!("fma")` in `dispatch!` (and `"avx2"` for [`exp_in_place`]). Calling a `#[target_feature]` function
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

    #[inline]
    pub(super) fn exp_in_place(x: &mut [f64]) {
        #[target_feature(enable = "avx2,fma")]
        fn kernel(x: &mut [f64]) {
            super::exp::exp_in_place(x)
        }
        // SAFETY: only called after `is_x86_feature_detected!("avx2")` and `("fma")` returned
        // true.
        unsafe { kernel(x) }
    }
    fma_copy!(log);
    fma_copy!(sin);
    fma_copy!(asin);
    fma_copy!(acos);
    fma_copy!(cbrt);
    fma_copy!(tan);
    fma_copy!(tanh);

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

    /// The FMA copy of a glibc function of `super::$module` (`glibc_dispatch!`).
    macro_rules! fma_glibc {
        ($module:ident :: $name:ident ($($arg:ident : $t:ty),*) -> $ret:ty) => {
            #[inline]
            pub(super) fn $name($($arg: $t),*) -> $ret {
                #[target_feature(enable = "fma")]
                fn kernel($($arg: $t),*) -> $ret {
                    super::$module::$name($($arg),*)
                }
                // SAFETY: only called after `is_x86_feature_detected!("fma")` returned true.
                unsafe { kernel($($arg),*) }
            }
        };
    }
    fma_glibc!(sincos_glibc::sin_glibc(x: f64) -> f64);
    fma_glibc!(sincos_glibc::cos_glibc(x: f64) -> f64);
    fma_glibc!(sincosf_glibc::sinf_glibc(y: f32) -> f32);
    fma_glibc!(sincosf_glibc::cosf_glibc(y: f32) -> f32);
    fma_glibc!(sincosf_glibc::sincosf_glibc(y: f32) -> (f32, f32));
    fma_glibc!(logf_glibc::logf_glibc(x: f32) -> f32);
    fma_glibc!(expf_glibc::expf_glibc(x: f32) -> f32);
    fma_glibc!(powf_glibc::powf_glibc(x: f32, y: f32) -> f32);
    fma_glibc!(exp_glibc::exp_glibc(x: f64) -> f64);
    fma_glibc!(log_glibc::log_glibc(x: f64) -> f64);
    fma_glibc!(pow_glibc::pow_glibc(x: f64, y: f64) -> f64);
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
                (super::tan(x), super::tan::tan(x)),
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
            // tanh over every branch (|x| up to 25)
            let w = u * 25.0;
            for (a, b) in [
                (super::asin(u), super::asin::asin(u)),
                (super::acos(u), super::acos::acos(u)),
                (super::tanh(w), super::tanh::tanh(w)),
                (super::tanh(x), super::tanh::tanh(x)),
            ] {
                assert!(
                    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()),
                    "{x:e}"
                );
            }
        }
    }

    /// The glibc functions of two arguments and of doubles give glibc 2.35's results on 65,536
    /// seeded arguments each (hashes recorded from the system's functions on Linux x86-64 by
    /// `validation/investigations/glibc-maths/`, `hashes`): SplitMix64 of the index, the even
    /// ones random bit patterns, the odd ones in the functions' common ranges.
    #[test]
    fn glibc_functions_match_glibc_on_seeded_arguments() {
        fn splitmix64(i: u64) -> u64 {
            let mut z = i.wrapping_add(0x9e37_79b9_7f4a_7c15);
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }
        let unit = |r: u64| (r >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
        let fnv = |h: u64, v: u64| (h ^ v).wrapping_mul(0x0100_0000_01b3);
        let mut h = [0xcbf2_9ce4_8422_2325u64; 5];
        for k in 0..65_536u64 {
            let r = splitmix64(k);
            let r2 = splitmix64(r);
            let odd = k & 1 == 1;
            let (ex, lg, px, py, fx, fy, ay, ax) = if odd {
                (
                    unit(r) * 1500.0 - 750.0,
                    unit(r) * 0.2 + 0.9,
                    unit(r) * 1e3,
                    unit(r2) * 8.0 - 4.0,
                    unit(r) as f32 * 100.0,
                    unit(r2) as f32 * 20.0 - 10.0,
                    unit(r) as f32 * 2.0 - 1.0,
                    unit(r2) as f32 * 2.0 - 1.0,
                )
            } else {
                let (lo, hi) = (f32::from_bits(r as u32), f32::from_bits((r >> 32) as u32));
                (
                    f64::from_bits(r),
                    f64::from_bits(r & 0x7fff_ffff_ffff_ffff),
                    f64::from_bits(r),
                    f64::from_bits(r2),
                    lo,
                    hi,
                    lo,
                    hi,
                )
            };
            h[0] = fnv(h[0], super::exp_glibc(ex).to_bits());
            h[1] = fnv(h[1], super::log_glibc(lg).to_bits());
            h[2] = fnv(h[2], super::pow_glibc(px, py).to_bits());
            h[3] = fnv(h[3], u64::from(super::powf_glibc(fx, fy).to_bits()));
            h[4] = fnv(h[4], u64::from(super::atan2f_glibc(ay, ax).to_bits()));
        }
        assert_eq!(
            h,
            [
                0x30b1_722a_0e68_e056, // exp
                0x05ec_df9c_1dd2_0062, // log
                0x4693_03ea_60fd_f797, // pow
                0xed13_9943_3568_1cda, // powf
                0x0a7f_b675_4721_8aff, // atan2f
            ]
        );
    }

    /// The FMA copies of the glibc functions give the same bits as the portable kernels, NaNs
    /// included.
    #[test]
    fn glibc_fma_copies_are_bit_identical() {
        let mut s = 0x6a09_e667_f3bc_c909u64;
        for k in 0..300_000u64 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let x = f64::from_bits(s);
            let u = (s >> 11) as f64 / (1u64 << 53) as f64;
            let f = f32::from_bits(s as u32);
            let g = f32::from_bits((s >> 32) as u32);
            // Moderate arguments too, where the main paths run.
            let m = (u * 200.0 - 100.0) as f32;
            for (a, b) in [
                (super::sinf_glibc(f), super::sincosf_glibc::sinf_glibc(f)),
                (super::cosf_glibc(m), super::sincosf_glibc::cosf_glibc(m)),
                (
                    super::sincosf_glibc(f).1,
                    super::sincosf_glibc::sincosf_glibc(f).1,
                ),
                (super::logf_glibc(f), super::logf_glibc::logf_glibc(f)),
                (super::expf_glibc(m), super::expf_glibc::expf_glibc(m)),
                (
                    super::powf_glibc(f.abs(), m / 20.0),
                    super::powf_glibc::powf_glibc(f.abs(), m / 20.0),
                ),
                (super::powf_glibc(f, g), super::powf_glibc::powf_glibc(f, g)),
            ] {
                assert_eq!(a.to_bits(), b.to_bits(), "{k}: {f:e} {m:e}");
            }
            let w = u * 1500.0 - 750.0;
            for (a, b) in [
                (super::exp_glibc(w), super::exp_glibc::exp_glibc(w)),
                (super::exp_glibc(x), super::exp_glibc::exp_glibc(x)),
                (super::sin_glibc(x), super::sincos_glibc::sin_glibc(x)),
                (super::cos_glibc(x), super::sincos_glibc::cos_glibc(x)),
                (super::sin_glibc(w), super::sincos_glibc::sin_glibc(w)),
                (super::cos_glibc(w), super::sincos_glibc::cos_glibc(w)),
                (super::sin_glibc(u), super::sincos_glibc::sin_glibc(u)),
                (
                    super::cos_glibc(u + 0.8),
                    super::sincos_glibc::cos_glibc(u + 0.8),
                ),
                (super::log_glibc(x), super::log_glibc::log_glibc(x)),
                (
                    super::log_glibc(u + 0.95),
                    super::log_glibc::log_glibc(u + 0.95),
                ),
                (super::pow_glibc(x, w), super::pow_glibc::pow_glibc(x, w)),
                (
                    super::pow_glibc(u * 1e3, 1.0 / 3.0),
                    super::pow_glibc::pow_glibc(u * 1e3, 1.0 / 3.0),
                ),
            ] {
                assert_eq!(a.to_bits(), b.to_bits(), "{k}: {x:e} {w:e}");
            }
        }
    }

    /// The dispatched `exp_in_place` (AVX2 and FMA on such a CPU) gives `exp`'s bits.
    #[test]
    fn exp_in_place_is_exp() {
        let mut s = 0x2545_f491_4f6c_dd1du64;
        let mut v = Vec::new();
        for k in 0..600_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            v.push(match k % 3 {
                0 => f64::from_bits(s),
                1 => (s >> 11) as f64 / (1u64 << 53) as f64 * 1500.0 - 750.0,
                _ => -((s >> 11) as f64 / (1u64 << 53) as f64) * 40.0,
            });
        }
        for len in [v.len(), 7, 3, 0] {
            let mut got = v[..len].to_vec();
            super::exp_in_place(&mut got);
            for (&x, &y) in v[..len].iter().zip(&got) {
                let e = super::exp::exp(x);
                assert!(
                    e.to_bits() == y.to_bits() || (e.is_nan() && y.is_nan()),
                    "{x:e}"
                );
            }
        }
    }
}
