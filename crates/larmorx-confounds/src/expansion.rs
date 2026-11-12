// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy's float64 `a ** o` for the integer exponents of niworkflows' `exponential_terms`
//! (`ExpandModel`'s `(x)^3`, `(x)^^4`, `(x)^2-5`, ...), as numpy 2.2.6, 2.3.5 and 2.5.4 compute
//! it (numpy is BSD-3-Clause, Copyright (c) 2005-2026, NumPy Developers; the dispatch below
//! follows `numpy/_core/src/multiarray/number.c`'s `fast_scalar_power` and
//! `umath/loops_umath_fp.dispatch.c.src`'s `DOUBLE_power`).
//!
//! pandas computes `DataFrame ** o` (a Python integer `o`) block by block with numpy's
//! `ndarray.__pow__`: an exponent of 2 is numpy's `square` (`x * x`, exact); any other one is
//! the `power` ufunc with `o` as a float64 scalar, whose loop takes 1 for an exponent of 0
//! (numpy 2.5.4's shortcut; numpy 2.2.6's C library `pow(x, 0)` is 1 too, NaN included) and the
//! C library's `pow` otherwise. On x86-64 CPUs with AVX-512, numpy's wheels call Intel's SVML
//! `pow` instead (NP-04's class: the results depend on the CPU); the reference is the path
//! without AVX-512, glibc 2.35's `pow` (`larmorx_core::math::pow_glibc`, GLIBC-07), which
//! `upstream_defects="reproduce"` follows. By default larmorx takes the correctly rounded `pow`
//! (CORE-MATH's `cr_pow`, `larmorx_core::math::pow`), the same on every platform.

use larmorx_core::defects::UpstreamDefects;
use larmorx_core::math;

/// numpy's float64 `x ** o` for one value and an exponent `o` (a non-negative integer as
/// niworkflows' grammar gives it, converted to float64 as numpy converts a Python integer):
/// `1` for 0, `x * x` for 2, `pow(x, o)` otherwise; `glibc` chooses glibc 2.35's `pow` (numpy's
/// on CPUs without AVX-512) over the correctly rounded one.
#[inline]
#[must_use]
pub fn power_one(x: f64, exponent: f64, glibc: bool) -> f64 {
    if exponent == 0.0 {
        1.0
    } else if exponent == 2.0 {
        x * x
    } else if glibc {
        math::pow_glibc(x, exponent)
    } else {
        math::pow(x, exponent)
    }
}

/// numpy's float64 `values ** exponent` as [`power_one`] computes each value, in `mode`
/// (`Fix`: the correctly rounded `pow`; `Reproduce`: glibc's), and the number of values whose
/// bits differ between the two modes (GLIBC-07's notice; NaNs compared as NaNs).
#[must_use]
pub fn power(values: &[f64], exponent: f64, mode: UpstreamDefects) -> (Vec<f64>, usize) {
    let glibc = !mode.fix();
    let mut differ = 0;
    let out = values
        .iter()
        .map(|&x| {
            let mine = power_one(x, exponent, glibc);
            let other = power_one(x, exponent, !glibc);
            if mine.to_bits() != other.to_bits() && !(mine.is_nan() && other.is_nan()) {
                differ += 1;
            }
            mine
        })
        .collect();
    (out, differ)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_and_pow() {
        let v = [0.5, -3.0, f64::NAN, f64::INFINITY, -0.0, 1e200];
        let (zero, _) = power(&v, 0.0, UpstreamDefects::Fix);
        assert!(zero.iter().all(|&x| x == 1.0));
        let (sq, d) = power(&v, 2.0, UpstreamDefects::Reproduce);
        assert_eq!(d, 0);
        assert_eq!(sq[0], 0.25);
        assert_eq!(sq[1], 9.0);
        assert!(sq[2].is_nan());
        assert_eq!(sq[5], f64::INFINITY);
        let (cube, _) = power(&v, 3.0, UpstreamDefects::Fix);
        assert_eq!(cube[1], -27.0);
        assert_eq!(cube[4].to_bits(), (-0.0f64).to_bits());
        assert_eq!(cube[3], f64::INFINITY);
    }

    #[test]
    fn the_two_modes_differ_somewhere() {
        // glibc's pow is not correctly rounded: some cubes of seeded values differ in the last
        // bit, and the count says how many.
        let mut x = 0x9e37_79b9_7f4a_7c15_u64;
        let values: Vec<f64> = (0..20_000)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x >> 11) as f64 / (1u64 << 53) as f64 * 4.0 - 2.0
            })
            .collect();
        let (fix, d1) = power(&values, 3.0, UpstreamDefects::Fix);
        let (rep, d2) = power(&values, 3.0, UpstreamDefects::Reproduce);
        assert_eq!(d1, d2);
        let n = fix
            .iter()
            .zip(&rep)
            .filter(|(a, b)| a.to_bits() != b.to_bits())
            .count();
        assert_eq!(n, d1);
    }
}
