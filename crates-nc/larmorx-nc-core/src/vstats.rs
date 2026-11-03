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

// ------------------------------------------------------------------------------------------------
// The masked forms (`maskedIterator`, `calc_percentiles(vol, mask)`, `imageHistogram`): the
// mask's values cycled over the volume's (a 3D mask over a 4D volume), a value counted where its
// mask value is above 0.5 (`maskThreshold()`); an empty mask (no values) counts every value.

/// Whether value `k` is in `mask`.
#[inline]
pub fn in_mask(mask: &[f32], k: usize) -> bool {
    mask.is_empty() || f64::from(mask[k % mask.len()]) > crate::volops::MASK_THRESHOLD
}

/// The number of values `maskedIterator` visits.
pub fn count_masked(len: usize, mask: &[f32]) -> usize {
    if mask.is_empty() {
        return len;
    }
    (0..len).filter(|&k| in_mask(mask, k)).count()
}

/// `calculateSums(vol, mask)`: `(sum, sum of squares, empty)`, the blocks' length from the
/// volume's number of values (`max(sqrt(n), 10000) + 1`); `empty` when no value is in the mask
/// (FSL prints `ERROR:: Empty mask image`).
pub fn sums_masked<T: Arith>(values: &[T], mask: &[f32]) -> (f64, f64, bool) {
    sums_where(values, |k| in_mask(mask, k))
}

/// [`sums_masked`] of the values `keep(k)` selects.
pub fn sums_where<T: Arith>(values: &[T], keep: impl Fn(usize) -> bool) -> (f64, f64, bool) {
    let n = values.len();
    let nlim = ((n as f64).sqrt() as i64).max(10000) as usize;
    let (mut s0, mut s1) = (0f64, 0f64);
    let (mut sum, mut sum2) = (0f64, 0f64);
    let (mut cnt, mut nn) = (0usize, 0usize);
    for (k, &v) in values.iter().enumerate() {
        if !keep(k) {
            continue;
        }
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

/// `mean(mask)`: `sum(mask) / max(count, 1)`, and whether `calculateSums` found the mask empty
/// (one `ERROR:: Empty mask image`).
pub fn mean_masked<T: Arith>(values: &[T], mask: &[f32]) -> (f64, bool) {
    mean_where(values, |k| in_mask(mask, k))
}

/// [`mean_masked`] of the values `keep(k)` selects.
pub fn mean_where<T: Arith>(values: &[T], keep: impl Fn(usize) -> bool) -> (f64, bool) {
    let n = (0..values.len()).filter(|&k| keep(k)).count() as f64;
    let (s, _, empty) = sums_where(values, keep);
    (sse::div_f64(s, n.max(1.0)), empty)
}

/// `variance(mask)`: `max(0, (n / max(1, n - 1)) (sumsquares(mask) / n - mean(mask)²))`, and the
/// number of `ERROR:: Empty mask image` lines FSL prints (one, from `variance` itself, for an
/// empty mask: the variance is then 0).
pub fn variance_masked<T: Arith>(values: &[T], mask: &[f32]) -> (f64, usize) {
    variance_where(values, |k| in_mask(mask, k))
}

/// [`variance_masked`] of the values `keep(k)` selects.
pub fn variance_where<T: Arith>(values: &[T], keep: impl Fn(usize) -> bool) -> (f64, usize) {
    let n = (0..values.len()).filter(|&k| keep(k)).count();
    if n == 0 {
        return (0.0, 1);
    }
    let n = n as f64;
    let (_, s2, _) = sums_where(values, &keep);
    let (m, _) = mean_where(values, &keep);
    let inner = sse::sub_f64(sse::div_f64(s2, n), sse::mul_f64(m, m));
    let v = sse::mul_f64(sse::div_f64(n, (n - 1.0).max(1.0)), inner);
    (if 0.0 > v { 0.0 } else { v }, 0)
}

/// `calculateExtrema(vol, coords, mask)` with the offsets of the extremes; `None` when no value
/// is in the mask (FSL prints `calculateExtrema: mask is empty` and returns 0, 0 with no
/// coordinates).
pub fn extrema_masked<T: Arith>(values: &[T], mask: &[f32]) -> Option<Extrema<T>> {
    let mut it = values.iter().enumerate().filter(|(k, _)| in_mask(mask, *k));
    let (k0, &first) = it.next()?;
    let mut e = Extrema {
        max: first,
        min: first,
        imax: k0,
        imin: k0,
    };
    for (k, &v) in it {
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

/// `calc_percentiles(vol, mask, {p})`'s values: the masked ones (mask value above 0.5, the mask
/// cycled), in storage order.
pub fn percentile_values<T: Arith>(values: &[T], mask: &[f32]) -> Vec<T> {
    values
        .iter()
        .enumerate()
        .filter(|(k, _)| mask.is_empty() || f64::from(mask[k % mask.len()]) > 0.5)
        .map(|(_, &v)| v)
        .collect()
}

/// `imageHistogram(vol, bins, min, max, hist, mask)`: `None` when `max <= min` (FSL returns -1
/// and leaves the histogram as it was: unsized); otherwise each masked value counted in bin
/// `max(0, min((int)(fA v + fB), bins - 1))` with `fA = bins / (max - min)` and
/// `fB = bins (-min) / (max - min)`, and the number of values. `(int)` of a value outside
/// `int`'s range (or NaN) is x86-64's (the most negative `int`: the first bin) unless
/// `clamp_bins` (larmorx's correction: clamped, NaN as 0), which also returns how many values
/// moved. `skip_nan` leaves NaN values out.
pub fn image_histogram<T: Arith>(
    values: &[T],
    mask: &[f32],
    bins: i32,
    min: T,
    max: T,
    clamp_bins: bool,
    skip_nan: bool,
) -> Option<(Vec<f64>, i64, usize)> {
    if max <= min {
        return None;
    }
    let mut hist = vec![0f64; bins.max(0) as usize];
    let range = T::diff(max, min);
    let fa = f64::from(bins) / range;
    let fb = (f64::from(bins) * T::neg_f64(min)) / range;
    let (mut n, mut moved) = (0i64, 0usize);
    for (k, &v) in values.iter().enumerate() {
        if !in_mask(mask, k) || (skip_nan && T::is_nan(v)) {
            continue;
        }
        let x = fa * v.to_f64() + fb;
        let fsl = crate::cnum::f64_to_i32(x).min(bins - 1).max(0);
        let b = if clamp_bins {
            let c = if x.is_nan() {
                0
            } else {
                x.clamp(-2_147_483_648.0, 2_147_483_647.0) as i32
            };
            let c = c.min(bins - 1).max(0);
            if c != fsl {
                moved += 1;
            }
            c
        } else {
            fsl
        };
        hist[b as usize] += 1.0;
        n += 1;
    }
    Some((hist, n, moved))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masked_forms() {
        let v = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let m = [1.0f32, 0.0, 1.0];
        assert_eq!(count_masked(6, &m), 4);
        assert_eq!(
            sums_masked(&v, &m),
            (1.0 + 3.0 + 4.0 + 6.0, 1.0 + 9.0 + 16.0 + 36.0, false)
        );
        assert_eq!(mean_masked(&v, &m).0, 3.5);
        assert_eq!(variance_masked(&v, &[0.0f32]), (0.0, 1));
        let e = extrema_masked(&v, &m).unwrap();
        assert_eq!((e.max, e.imax, e.min, e.imin), (6.0, 5, 1.0, 0));
        assert!(extrema_masked(&v, &[0.0f32]).is_none());
        assert_eq!(percentile_values(&v, &m), vec![1.0, 3.0, 4.0, 6.0]);
        let (h, n, _) = image_histogram(&v, &[], 3, 1.0f32, 6.0, false, false).unwrap();
        assert_eq!((h, n), (vec![2.0, 2.0, 2.0], 6));
        assert!(image_histogram(&v, &[], 3, 2.0f32, 2.0, false, false).is_none());
        // a value far above the range: x86-64's conversion puts it in the first bin
        let w = [0.0f32, 1.0, 1e10];
        let (h, _, _) = image_histogram(&w, &[], 2, 0.0f32, 1.0, false, false).unwrap();
        assert_eq!(h, vec![2.0, 1.0]);
        let (h, _, moved) = image_histogram(&w, &[], 2, 0.0f32, 1.0, true, false).unwrap();
        assert_eq!((h, moved), (vec![1.0, 2.0], 1));
    }

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
