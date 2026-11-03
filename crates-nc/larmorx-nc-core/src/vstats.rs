// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-newimage 2601.0 (newimage.cc: calculateSums,
// calculateExtrema, percentile_vec, calc_percentiles; newimage.h: mean(), variance(),
// stddev(); variance(mask) for the corrected form). Copyright (C) 1999-2008 University of
// Oxford (Mark Jenkinson, Matthew Webster). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! newimage's statistics of all the values of a volume, for every element type (fslmaths'
//! reductions along a dimension, `-ranknorm`, `-cpval`; fslstats).
//!
//! - [`sums`]: `calculateSums`: the sum and the sum of squares in doubles, accumulated in
//!   blocks of `max(sqrt(n), 10000) + 1` values added to the totals.
//! - [`extrema`]: `calculateExtrema`: from the first value, each strictly larger value a new
//!   maximum, else each strictly smaller one a new minimum, with the first index of each
//!   (a NaN first value keeps both NaN: FSL-153; [`extrema_without_nan`] is the correction).
//! - [`mean`], [`variance`]: `mean()` (`sum / max(1, n)`) and `variance()` without a mask,
//!   `(n / (n - 1)) (sumsquares / n - mean^2)` (FSL-167: below 0 by rounding, or NaN for one
//!   value; the corrected form is the masked `variance(mask)`'s `max(0, (n / max(1, n - 1))
//!   (...))`).
//! - [`percentile`]: `percentile_vec`: the values sorted by libstdc++'s `std::sort`, the value
//!   at `(unsigned int)((float) n * p)` (at most `n - 1`); 0 for no values.

use crate::sse;
use crate::stdsort::{self, Outside};
use crate::volops::Arith;

/// `calculateSums(vol, volume<char>())`: `(sum, sum of squares, empty)`, in FSL's blocks;
/// `empty` when there is no value (FSL prints `ERROR:: Empty mask image`).
pub fn sums<T: Arith>(values: &[T]) -> (f64, f64, bool) {
    let n = values.len();
    let nlim = ((n as f64).sqrt() as i64).max(10000) as usize;
    let (mut s0, mut s1) = (0f64, 0f64);
    let (mut sum, mut sum2) = (0f64, 0f64);
    let (mut cnt, mut nn) = (0usize, 0usize);
    for &v in values {
        let value = v.to_f64();
        sum = sse::add_f64(sum, value);
        sum2 = sse::add_f64(sum2, sse::mul_f64(value, value));
        cnt += 1;
        if cnt > nlim {
            nn += 1;
            s0 = sse::add_f64(s0, sum);
            s1 = sse::add_f64(s1, sum2);
            sum = 0.0;
            sum2 = 0.0;
            cnt = 0;
        }
    }
    let empty = cnt + nn == 0;
    (sse::add_f64(s0, sum), sse::add_f64(s1, sum2), empty)
}

/// `mean()`: `sum() / max(1, n)`.
pub fn mean<T: Arith>(values: &[T]) -> f64 {
    sse::div_f64(sums(values).0, (values.len() as f64).max(1.0))
}

/// `variance()` without a mask: `(n / (n - 1)) (sumsquares() / n - mean() mean())`; with
/// `corrected` (FSL-167's correction) the form of `variance(mask)`: `max(0, (n / max(1, n - 1))
/// (...))`, so 0 for one value and never below 0.
pub fn variance<T: Arith>(values: &[T], corrected: bool) -> f64 {
    let n = values.len() as f64;
    let m = mean(values);
    let inner = sse::sub_f64(sse::div_f64(sums(values).1, n), sse::mul_f64(m, m));
    if corrected {
        let v = sse::mul_f64(sse::div_f64(n, (n - 1.0).max(1.0)), inner);
        // MISCMATHS::Max(0, v): `0 > v ? 0 : v` keeps a NaN
        if 0.0 > v { 0.0 } else { v }
    } else {
        sse::mul_f64(sse::div_f64(n, n - 1.0), inner)
    }
}

/// `stddev()`: the square root of [`variance`].
pub fn stddev<T: Arith>(values: &[T], corrected: bool) -> f64 {
    sse::sqrt_f64(variance(values, corrected))
}

/// Whether [`variance`]'s two forms differ (FSL-167 applies): one value, or a variance rounded
/// below 0.
pub fn variance_defect<T: Arith>(values: &[T]) -> bool {
    variance(values, false).to_bits() != variance(values, true).to_bits()
}

/// The extrema `calculateExtrema` finds, with the index of each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extrema<T> {
    pub max: T,
    pub min: T,
    pub imax: usize,
    pub imin: usize,
}

/// `calculateExtrema(vol, coords, volume<char>())`; `None` for no value (FSL then returns 0, 0).
pub fn extrema<T: Arith>(values: &[T]) -> Option<Extrema<T>> {
    let first = *values.first()?;
    let mut e = Extrema {
        max: first,
        min: first,
        imax: 0,
        imin: 0,
    };
    for (k, &v) in values.iter().enumerate().skip(1) {
        if v > e.max {
            e.max = v;
            e.imax = k;
        } else if v < e.min {
            e.min = v;
            e.imin = k;
        }
    }
    Some(e)
}

/// [`extrema`] from the first value that is not NaN (FSL-153's correction: NaN values left
/// out); `None` when every value is NaN (or there is none).
pub fn extrema_without_nan<T: Arith>(values: &[T]) -> Option<Extrema<T>> {
    let start = values.iter().position(|&v| !T::is_nan(v))?;
    let mut e = extrema(&values[start..])?;
    e.imax += start;
    e.imin += start;
    Some(e)
}

/// `percentile_vec(hist, {p})`: `hist` sorted in place by `std::sort` (libstdc++'s sequence);
/// `Err(Outside)` where a NaN makes libstdc++ step outside the vector (C++ reads memory it does
/// not own). No values give 0.
pub fn percentile<T: Arith>(hist: &mut [T], p: f32) -> Result<T, Outside> {
    let numbins = hist.len();
    if numbins == 0 {
        return Ok(T::from_int(0));
    }
    stdsort::sort_checked(hist)?;
    Ok(hist[percentile_index(numbins, p)])
}

/// `(unsigned int)((float) numbins * p)`, at most `numbins - 1`. GCC converts through a 64-bit
/// `cvttss2si` and keeps the low 32 bits: a NaN product gives 0 (FSL-151's NaN, which the
/// correction also maps to 0), a negative one its two's complement (the last bin).
pub fn percentile_index(numbins: usize, p: f32) -> usize {
    let f = sse::mul_f32(numbins as f32, p);
    let k = if f.is_nan() {
        0
    } else {
        (f as i64) as u32 as usize
    };
    k.min(numbins - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_and_moments() {
        let v = [1.0f32, 2.0, 3.0, 4.0];
        assert_eq!(sums(&v), (10.0, 30.0, false));
        assert_eq!(mean(&v), 2.5);
        assert_eq!(variance(&v, false), (4.0 / 3.0) * (7.5 - 6.25));
        assert!(variance(&[2.0f32], false).is_nan());
        assert_eq!(variance(&[2.0f32], true), 0.0);
        assert!(variance_defect(&[2.0f32]));
        assert!(!variance_defect(&v));
    }

    #[test]
    fn extrema_and_percentiles() {
        let v = [f32::NAN, 3.0, -1.0, 3.0];
        let e = extrema(&v).unwrap();
        assert!(e.max.is_nan() && e.imax == 0);
        let f = extrema_without_nan(&v).unwrap();
        assert_eq!((f.max, f.imax, f.min, f.imin), (3.0, 1, -1.0, 2));
        let mut h = [5i16, 1, 4, 2];
        assert_eq!(percentile(&mut h, 0.5).unwrap(), 4);
        assert_eq!(percentile_index(4, 1.0), 3);
        assert_eq!(percentile_index(4, f32::NAN), 0);
        let mut empty: [f32; 0] = [];
        assert_eq!(percentile(&mut empty, 0.5).unwrap(), 0.0);
    }
}
