// SPDX-License-Identifier: Apache-2.0
//! The numeric conventions of PRELUDE that decide the bits (`specs/prelude.md` §6): the
//! rounding to an integer, the wrap into `[-pi, pi]`, the phase intervals, the extrema, all with
//! x86-64's NaN rules on every platform (`crate::maths::num`).

use std::f64::consts::PI;

use crate::maths::num::{cvtt, nan32, nan64, to_f32, to_f64};

/// `2 pi`, the double `2 × pi` (§6.6).
pub const TWO_PI: f64 = 2.0 * PI;
/// `c1 = fl(1 / (2 pi))` (§6.3: `0.15915494f`).
pub const C1: f32 = (1.0 / (2.0 * PI)) as f32;
/// `c2 = fl(2 pi)` (§6.3: `6.2831855f`).
pub const C2: f32 = (2.0 * PI) as f32;

/// `rnd(x)` (§6.2): half away from zero, `trunc(x + 0.5)` for `x > 0`, else `trunc(x - 0.5)`,
/// the conversion x86-64's (`INT_MIN` for NaN and out-of-range values).
#[inline]
pub fn rnd(x: f64) -> i32 {
    if x > 0.0 {
        cvtt(x + 0.5)
    } else {
        cvtt(x - 0.5)
    }
}

/// `wrap(phi)` (§6.3): `phi - fl(fl(k) × c2)` with `k = rnd(fl(phi × c1))`, every operation in
/// single precision.
#[inline]
pub fn wrap(p: f32) -> f32 {
    if p.is_nan() {
        // phi × c1, k × c2 and the difference pass phi's NaN on, quieted
        return f32::from_bits(p.to_bits() | 0x0040_0000);
    }
    let x = nan32(p, C1, p * C1);
    let k = rnd(f64::from(x)) as f32;
    let m = k * C2;
    nan32(p, m, p - m)
}

/// The width of a phase interval, `w = fl(2 pi / n)` (§4.6).
#[inline]
pub fn interval_width(n: i32) -> f32 {
    (TWO_PI / f64::from(n)) as f32
}

/// A voxel's phase interval (§4.6): `trunc((phi + pi) / w) + 1` in double, clamped to `[1, n]`
/// (a NaN or an out-of-range value: `INT_MIN + 1`, so interval 1).
#[inline]
pub fn interval(p: f32, w: f32, n: i32) -> i32 {
    let b = cvtt((f64::from(p) + PI) / f64::from(w)).wrapping_add(1);
    b.clamp(1, n)
}

/// `fl(a - b)` with x86-64's NaN rules.
#[inline]
pub fn sub32(a: f32, b: f32) -> f32 {
    nan32(a, b, a - b)
}

/// `fl(a + b)` with x86-64's NaN rules.
#[inline]
pub fn add32(a: f32, b: f32) -> f32 {
    nan32(a, b, a + b)
}

/// `a + b` in double with x86-64's NaN rules.
#[inline]
pub fn add64(a: f64, b: f64) -> f64 {
    nan64(a, b, a + b)
}

/// `a - b` in double with x86-64's NaN rules.
#[inline]
pub fn sub64(a: f64, b: f64) -> f64 {
    nan64(a, b, a - b)
}

/// `a × b` in double with x86-64's NaN rules.
#[inline]
pub fn mul64(a: f64, b: f64) -> f64 {
    nan64(a, b, a * b)
}

/// `a / b` in double with x86-64's NaN rules.
#[inline]
pub fn div64(a: f64, b: f64) -> f64 {
    nan64(a, b, a / b)
}

/// A float widened (x86-64's NaN rule).
#[inline]
pub fn wide(x: f32) -> f64 {
    to_f64(x)
}

/// A double rounded to single precision (x86-64's NaN rule).
#[inline]
pub fn narrow(x: f64) -> f32 {
    to_f32(x)
}

/// The extrema of `v` as FSL takes them (§4.2): both start at the first value; a later value
/// greater than the maximum replaces it, else one less than the minimum replaces that. A NaN
/// first value makes both NaN; later NaN values are passed over. `None` for no values.
pub fn extrema_fsl(v: &[f32]) -> Option<(f32, f32)> {
    let first = *v.first()?;
    if first.is_nan() {
        return Some((first, first));
    }
    Some(extrema_from(v, first))
}

/// The extrema of the values that are not NaN (the default, FSL-272 and FSL-268): `None` when
/// every value is NaN (or there are none).
pub fn extrema_finite(v: &[f32]) -> Option<(f32, f32)> {
    let first = *v.iter().find(|x| !x.is_nan())?;
    Some(extrema_from(v, first))
}

/// The maximum and minimum of the non-NaN values of `v`, both started at `first` (parallel:
/// only the values decide, a zero's sign never matters to prelude's comparisons).
fn extrema_from(v: &[f32], first: f32) -> (f32, f32) {
    use rayon::prelude::*;
    let fold = |(mx, mn): (f32, f32), &x: &f32| {
        if x > mx {
            (x, mn)
        } else if x < mn {
            (mx, x)
        } else {
            (mx, mn)
        }
    };
    if v.len() < (1 << 16) {
        return v.iter().fold((first, first), fold);
    }
    v.par_chunks(1 << 16)
        .map(|c| c.iter().fold((first, first), fold))
        .reduce(
            || (first, first),
            |a, b| {
                (
                    if b.0 > a.0 { b.0 } else { a.0 },
                    if b.1 < a.1 { b.1 } else { a.1 },
                )
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_the_specs() {
        assert_eq!(C1, 0.159_154_94_f32);
        assert_eq!(C2, 6.283_185_5_f32);
        assert_eq!(rnd(2.5), 3);
        assert_eq!(rnd(-2.5), -3);
        assert_eq!(rnd(-0.4), 0);
        assert_eq!(rnd(f64::NAN), i32::MIN);
        assert_eq!(rnd(3e9), i32::MIN);
    }

    #[test]
    fn wrap_and_intervals() {
        assert_eq!(wrap(0.0), 0.0);
        let w = wrap(4.0);
        assert!((f64::from(w) - (4.0 - TWO_PI)).abs() < 1e-6);
        let width = interval_width(8);
        assert_eq!(interval(-3.1, width, 8), 1);
        assert_eq!(interval(3.2, width, 8), 8);
        assert_eq!(interval(f32::NAN, width, 8), 1);
        assert!(wrap(f32::NAN).is_nan());
    }

    #[test]
    fn extrema_rules() {
        let v = [f32::NAN, 1.0, -2.0];
        assert!(extrema_fsl(&v).unwrap().0.is_nan());
        assert_eq!(extrema_finite(&v), Some((1.0, -2.0)));
        assert_eq!(extrema_fsl(&[0.5, f32::NAN, 2.0, -1.0]), Some((2.0, -1.0)));
        assert_eq!(extrema_finite(&[f32::NAN]), None);
    }
}
