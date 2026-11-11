// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy 2.5.4's percentiles and medians (with numpy 2.3.5's where they differ) and SciPy
//! 1.18.1's `scoreatpercentile` (notice in `super`).
//!
//! The order statistics are selected, not sorted (numpy partitions; SciPy sorts): the value at
//! a rank does not depend on how it is found, except the sign of a zero when `-0.0` and `+0.0`
//! both occur and the payload of a NaN when NaNs with different payloads do (neither is
//! reproduced: the first NaN of the input is returned).

use super::{NpFloat, NumpyRelease, div_f64};

/// The `k`-th smallest of `v` (`0 <= k < v.len()`, no NaN in `v`).
fn kth<T: NpFloat>(v: &mut [T], k: usize) -> T {
    *v.select_nth_unstable_by(k, |a, b| a.partial_cmp(b).expect("no NaN"))
        .1
}

/// `numpy.percentile(a, q, method="higher")` of a one-dimensional array, `q` a Python number
/// in `[0, 100]`: `q / 100` in `float64`, the rank `ceil((n - 1) * q)`, the value of that rank.
/// A NaN in `a` gives a NaN (numpy: the last element after partitioning, a NaN). `None` where
/// numpy raises: `q` outside `[0, 100]` (`ValueError: Percentiles must be in the range [0,
/// 100]`) or `a` empty (`IndexError: index -1 is out of bounds for axis 0 with size 0`).
pub fn percentile_higher<T: NpFloat>(a: &[T], q: f64) -> Option<T> {
    let q = q / 100.0;
    if !(0.0..=1.0).contains(&q) || a.is_empty() {
        return None;
    }
    if let Some(&nan) = a.iter().find(|x| x.is_nan()) {
        return Some(nan);
    }
    let rank = ((a.len() - 1) as f64 * q).ceil() as usize;
    let mut v = a.to_vec();
    Some(kth(&mut v, rank))
}

/// The result of [`nanpercentile_linear_axis0`].
#[derive(Debug, Clone)]
pub struct Percentiles {
    /// The values, C-ordered `(q.len(), columns)`.
    pub values: Vec<f64>,
    /// numpy's result type: `float32` when the first column has no number (then every value was
    /// rounded to `float32`: `apply_along_axis` takes its output type from the first column's
    /// result), `float64` otherwise.
    pub float32: bool,
    /// Whether some column had no number (numpy warns "All-NaN slice encountered").
    pub all_nan_column: bool,
}

/// `numpy.nanpercentile(a, q, axis=0)` of a C-ordered `(rows, columns)` `float32` array, `q` a
/// list of numbers in `[0, 100]` (so not a "weak" scalar: the interpolation runs in `float64`),
/// the default `"linear"` method, column by column (`apply_along_axis` of `_nanquantile_1d`):
/// the NaNs removed; the virtual index `(n - 1) * q`; its neighbours `floor` and `floor + 1`,
/// both the last rank when the index is `>= n - 1`; `gamma = index - previous` (with numpy's
/// `-1` for the last rank); `_lerp`: `a + (b - a) * gamma`, or `b - (b - a) * (1 - gamma)` where
/// `gamma >= 0.5`, with `b - a` in `float32`. A column without numbers gives NaN. `None` where
/// numpy raises (`q` outside `[0, 100]`).
pub fn nanpercentile_linear_axis0(
    a: &[f32],
    rows: usize,
    cols: usize,
    q: &[f64],
) -> Option<Percentiles> {
    assert_eq!(a.len(), rows * cols, "array length and shape disagree");
    let q: Vec<f64> = q.iter().map(|&p| p / 100.0).collect();
    if q.iter().any(|p| !(0.0..=1.0).contains(p)) {
        return None;
    }
    let mut values = vec![0.0f64; q.len() * cols];
    let mut float32 = false;
    let mut all_nan_column = false;
    let mut col = Vec::with_capacity(rows);
    for j in 0..cols {
        col.clear();
        col.extend((0..rows).map(|i| a[i * cols + j]).filter(|x| !x.is_nan()));
        let n = col.len();
        if n == 0 {
            all_nan_column = true;
            if j == 0 {
                float32 = true;
            }
            for (i, _) in q.iter().enumerate() {
                values[i * cols + j] = f32::NAN.to_f64();
            }
            continue;
        }
        for (i, &p) in q.iter().enumerate() {
            let vi = (n - 1) as f64 * p;
            let (prev, next, prev_idx) = if vi >= (n - 1) as f64 {
                (n - 1, n - 1, -1.0)
            } else {
                // gamma subtracts the integer index (`astype(np.intp)`): q = -0.0 keeps a -0
                // weight, so a -0 lower neighbour stays -0 (`-0 + d * -0`)
                let f = vi.floor() as usize;
                (f, f + 1, f as f64)
            };
            let gamma = vi - prev_idx;
            let lo = kth(&mut col, prev);
            let hi = kth(&mut col, next);
            let d = hi.sub(lo).to_f64();
            let mut r = lo.to_f64().add(d.mul(gamma));
            if gamma >= 0.5 {
                r = hi.to_f64().sub(d.mul(1.0f64.sub(gamma)));
            }
            values[i * cols + j] = r;
        }
    }
    if float32 {
        for v in &mut values {
            *v = f32::from_f64(*v).to_f64();
        }
    }
    Some(Percentiles {
        values,
        float32,
        all_nan_column,
    })
}

/// SciPy's `scipy.stats.scoreatpercentile(a, per, interpolation_method="lower")` of a
/// one-dimensional `float64` array: the index `per / 100 * (n - 1)`, rounded down when it is not
/// an integer; the value of that rank of the sorted array (NaNs sorted last), as
/// `add.reduce(sorted[i:i + 1] * 1) / 1.0` (so `+0.0` for a zero). An empty array gives NaN.
/// `None` where SciPy raises (`ValueError: percentile must be in the range [0, 100]`).
pub fn scoreatpercentile_lower(a: &[f64], per: f64) -> Option<f64> {
    if a.is_empty() {
        return Some(f64::NAN);
    }
    if !(0.0..=100.0).contains(&per) {
        return None;
    }
    let mut idx = per / 100.0 * (a.len() - 1) as f64;
    if idx.trunc() != idx {
        idx = idx.floor();
    }
    let i = idx as usize;
    let mut v: Vec<f64> = a.iter().copied().filter(|x| !x.is_nan()).collect();
    let x = if i < v.len() {
        kth(&mut v, i)
    } else {
        *a.iter().find(|x| x.is_nan()).expect("a NaN at that rank")
    };
    Some(div_f64(0.0f64.add(x), 1.0))
}

/// Why numpy raises in [`percentile_linear_weak`], [`percentile_linear`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PercentileError {
    /// `ValueError: Percentiles must be in the range [0, 100]`.
    Range,
    /// `IndexError: index -1 is out of bounds for axis 0 with size 0` (an empty array).
    Empty,
}

impl std::fmt::Display for PercentileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PercentileError::Range => "Percentiles must be in the range [0, 100]",
            PercentileError::Empty => "index -1 is out of bounds for axis 0 with size 0",
        })
    }
}

impl std::error::Error for PercentileError {}

/// The neighbouring ranks of a virtual index (`_get_indexes`): `floor(vi)` and `floor(vi) + 1`,
/// both the last rank (numpy's `-1`) when `vi >= n - 1`, both the first when `vi < 0`; and the
/// value `gamma` is measured from (`floor(vi)`, or numpy's `-1` at the last rank).
fn neighbours(vi: f64, last: f64, n: usize) -> (usize, usize, f64) {
    if vi >= last {
        (n - 1, n - 1, -1.0)
    } else if vi < 0.0 {
        (0, 0, 0.0)
    } else {
        // gamma subtracts the integer index (`astype(np.intp)`): +0 where vi is -0
        let f = vi.floor() as usize;
        (f, f + 1, f as f64)
    }
}

/// `numpy.percentile(a, q)` (the default `"linear"` method, `axis=None`) of an array of
/// `float32` or `float64` values, `q` a Python `int` or `float` (`weak_q`), with the release's
/// arithmetic ([`NumpyRelease`]); the result has the array's type, as numpy's.
///
/// - numpy 2.5 (and every release for `float64`): `q / 100`, the virtual index `(n - 1) * q`,
///   its neighbours and the weight `gamma = vi - floor(vi)` in `float64` (`vi + 1` at the last
///   rank); then `_lerp` in the array's type with `gamma` rounded to it: `a + (b - a) * gamma`,
///   or `b - (b - a) * (1 - gamma)` (`1 - gamma` in `float64`, then rounded) where `gamma >= 0.5`.
/// - numpy 2.2 and 2.3, `float32`: `q` rounded to `float32` and divided by `float32(100)`;
///   `(n - 1)` rounded to `float32` and multiplied by it in `float32`; `gamma` the difference of
///   the index and its integer floor in `float64` rounded to `float32`; `_lerp` in `float32`
///   (`1 - gamma` too).
///
/// A NaN in `a` gives a NaN (the first in `a`; numpy's is the one its partition leaves last).
pub fn percentile_linear_weak<T: NpFloat>(
    a: &[T],
    q: f64,
    release: NumpyRelease,
) -> Result<T, PercentileError> {
    let single32 = std::mem::size_of::<T>() == 4 && release == NumpyRelease::V2_3;
    let n = a.len();
    // q / 100 and the range check, in the release's type.
    let (qq, valid) = if single32 {
        let q32 = (q as f32) / 100.0f32;
        (f64::from(q32), (0.0..=1.0).contains(&q32))
    } else {
        let q64 = q / 100.0;
        (q64, (0.0..=1.0).contains(&q64))
    };
    if !valid {
        return Err(PercentileError::Range);
    }
    if n == 0 {
        return Err(PercentileError::Empty);
    }
    if let Some(&nan) = a.iter().find(|x| x.is_nan()) {
        return Ok(nan);
    }
    let (prev, next, gamma64) = if single32 {
        let vi = ((n - 1) as f32) * (qq as f32);
        let last = (n - 1) as f32;
        let (p, nx, pi) = neighbours(f64::from(vi), f64::from(last), n);
        // gamma = vi - previous_indexes: float32 minus intp, in float64, cast back to float32.
        (p, nx, f64::from((f64::from(vi) - pi) as f32))
    } else {
        let vi = (n - 1) as f64 * qq;
        let (p, nx, pi) = neighbours(vi, (n - 1) as f64, n);
        (p, nx, vi - pi)
    };
    let mut v = a.to_vec();
    let lo = kth(&mut v, prev);
    let hi = kth(&mut v, next);
    let d = hi.sub(lo);
    let g = T::from_f64(gamma64);
    let mut r = lo.add(d.mul(g));
    if gamma64 >= 0.5 {
        let one_minus = if single32 {
            T::from_f64(1.0).sub(g)
        } else {
            T::from_f64(1.0 - gamma64)
        };
        r = hi.sub(d.mul(one_minus));
    }
    Ok(r)
}

/// `numpy.percentile(a, q)` (the default `"linear"` method, `axis=None`) of an array of
/// `float32` or `float64` values, `q` a list (or numpy array) of numbers: `q / 100`, the virtual
/// indexes, their neighbours and weights in `float64`; `_lerp` in `float64` with `b - a` in the
/// array's type (the same in numpy 2.3 and 2.5). The results are `float64`. A NaN in `a` makes
/// every result that NaN.
pub fn percentile_linear<T: NpFloat>(a: &[T], q: &[f64]) -> Result<Vec<f64>, PercentileError> {
    let q: Vec<f64> = q.iter().map(|&p| p / 100.0).collect();
    if q.iter().any(|p| !(0.0..=1.0).contains(p)) {
        return Err(PercentileError::Range);
    }
    let n = a.len();
    if n == 0 {
        return Err(PercentileError::Empty);
    }
    if let Some(&nan) = a.iter().find(|x| x.is_nan()) {
        return Ok(vec![nan.to_f64(); q.len()]);
    }
    let mut v = a.to_vec();
    let mut out = Vec::with_capacity(q.len());
    for &p in &q {
        let vi = (n - 1) as f64 * p;
        let (prev, next, pi) = neighbours(vi, (n - 1) as f64, n);
        let gamma = vi - pi;
        let lo = kth(&mut v, prev);
        let hi = kth(&mut v, next);
        let d = hi.sub(lo).to_f64();
        let mut r = lo.to_f64().add(d.mul(gamma));
        if gamma >= 0.5 {
            r = hi.to_f64().sub(d.mul(1.0f64.sub(gamma)));
        }
        out.push(r);
    }
    Ok(out)
}

/// `numpy.median(a)` (`axis=None`) of an array of `float32` or `float64` values (`_median`): the
/// middle order statistic (or the two middle ones), then `numpy.mean` of them (`0 + (-0 + x)`,
/// or `0 + ((-0 + x0) + x1)`, divided by the count in `float64` and rounded to the array's
/// type). A NaN in `a` gives a NaN (`_median_nancheck`); an empty array gives NaN (numpy's
/// "Mean of empty slice").
pub fn median<T: NpFloat>(a: &[T]) -> T {
    if a.is_empty() {
        return T::NAN;
    }
    if let Some(&nan) = a.iter().find(|x| x.is_nan()) {
        return nan;
    }
    let mut v = a.to_vec();
    let mid = middle(&mut v);
    super::reduce::mean_axis(&mid, [1, mid.len(), 1])[0]
}

/// The middle order statistic(s) of `v` (no NaN): one for an odd length, the two middle ones in
/// rank order for an even one.
fn middle<T: NpFloat>(v: &mut [T]) -> Vec<T> {
    let n = v.len();
    let h = n / 2;
    if n % 2 == 1 {
        vec![kth(v, h)]
    } else {
        let hi = kth(v, h);
        // after the selection the lower middle one is the largest of the first half
        let mut lo = v[0];
        for &x in &v[1..h] {
            if x > lo {
                lo = x;
            }
        }
        vec![lo, hi]
    }
}

/// `numpy.median(a, axis)` over the middle axis of a C-ordered `(outer, n, inner)` array (the
/// memory order of the array numpy partitions: a Fortran-ordered `(x, y, z, t)` volume series
/// along `t` is `(1, t, x·y·z)`): per column the middle order statistic(s), then `numpy.mean`
/// over the axis of the selected slab (`(outer, 1 or 2, inner)`, summed in order when `inner >
/// 1`); a column with a NaN gives a NaN. The result is C-ordered `(outer, inner)`; `n == 0`
/// gives NaN everywhere.
pub fn median_axis<T: NpFloat>(a: &[T], shape: [usize; 3]) -> Vec<T> {
    let [outer, n, inner] = shape;
    assert_eq!(
        a.len(),
        outer * n * inner,
        "array length and shape disagree"
    );
    if n == 0 {
        return vec![T::NAN; outer * inner];
    }
    let k = if n % 2 == 1 { 1 } else { 2 };
    let mut slab = vec![T::ZERO; outer * k * inner];
    let mut nans: Vec<Option<T>> = vec![None; outer * inner];
    let mut col = Vec::with_capacity(n);
    for o in 0..outer {
        for i in 0..inner {
            col.clear();
            col.extend((0..n).map(|e| a[(o * n + e) * inner + i]));
            if let Some(&nan) = col.iter().find(|x| x.is_nan()) {
                nans[o * inner + i] = Some(nan);
                continue;
            }
            for (e, x) in middle(&mut col).into_iter().enumerate() {
                slab[(o * k + e) * inner + i] = x;
            }
        }
    }
    let mut out = super::reduce::mean_axis(&slab, [outer, k, inner]);
    for (r, nan) in out.iter_mut().zip(nans) {
        if let Some(x) = nan {
            *r = x;
        }
    }
    out
}

/// The rank methods of `numpy.percentile` that take an order statistic without interpolating.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RankMethod {
    /// `method="lower"`: the rank `floor((n - 1) * q)` (nipype's `compute_dvars`).
    Lower,
    /// `method="higher"`: the rank `ceil((n - 1) * q)`.
    Higher,
}

/// `numpy.percentile(a, q, axis, method="lower" | "higher")` over the middle axis of a C-ordered
/// `(outer, n, inner)` array (its memory order: numpy copies `a` and partitions along the axis,
/// so only the values matter), `q` a Python `int` or `float`; the result is C-ordered `(outer,
/// inner)`, in the array's type (`take` of the partitioned array).
///
/// The rank (`_QuantileMethods`' `get_virtual_index`, then `astype(np.intp)`) is computed in the
/// release's type: numpy 2.2 and 2.3 divide `q` by `a.dtype.type(100)`, so for `float32` data
/// `q` is rounded to `float32`, divided in `float32`, and `(n - 1) * q` (the count rounded to
/// `float32`) and its `floor`/`ceil` are `float32`; numpy 2.5 divides by the Python `100`, in
/// `float64` (and every release computes in `float64` for `float64` data). A column holding a
/// NaN gives that NaN (numpy's `slices_having_nans`: the NaN its partition leaves last; the
/// first in the column here). `Err(Range)` for `q / 100` outside `[0, 1]` (numpy's
/// `ValueError`), `Err(Empty)` for `n == 0` (numpy's `IndexError` from `take`).
pub fn percentile_rank_axis<T: NpFloat>(
    a: &[T],
    shape: [usize; 3],
    q: f64,
    method: RankMethod,
    release: NumpyRelease,
) -> Result<Vec<T>, PercentileError> {
    let [outer, n, inner] = shape;
    assert_eq!(
        a.len(),
        outer * n * inner,
        "array length and shape disagree"
    );
    let single32 = std::mem::size_of::<T>() == 4 && release == NumpyRelease::V2_3;
    let round = |x: f64| match method {
        RankMethod::Lower => x.floor(),
        RankMethod::Higher => x.ceil(),
    };
    let rank = if single32 {
        let q32 = (q as f32) / 100.0f32;
        if !(0.0..=1.0).contains(&q32) {
            return Err(PercentileError::Range);
        }
        if n == 0 {
            return Err(PercentileError::Empty);
        }
        // float32 product, float32 floor/ceil (exact: an integer-valued float32 stays one)
        round(f64::from(((n - 1) as f32) * q32))
    } else {
        let q64 = q / 100.0;
        if !(0.0..=1.0).contains(&q64) {
            return Err(PercentileError::Range);
        }
        if n == 0 {
            return Err(PercentileError::Empty);
        }
        round((n - 1) as f64 * q64)
    };
    // q in [0, 1] keeps the rank in [0, n - 1]; only a float32 count above 2^24 (numpy 2.2 and
    // 2.3) can round past it, where numpy's `take` raises: not followed, the last rank taken.
    let rank = (rank as usize).min(n - 1);
    let mut out = Vec::with_capacity(outer * inner);
    let mut col = Vec::with_capacity(n);
    for o in 0..outer {
        for i in 0..inner {
            col.clear();
            col.extend((0..n).map(|e| a[(o * n + e) * inner + i]));
            if let Some(&nan) = col.iter().find(|x| x.is_nan()) {
                out.push(nan);
                continue;
            }
            out.push(kth(&mut col, rank));
        }
    }
    Ok(out)
}

/// The side of [`searchsorted`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// `side="left"` (the default): the first index whose value is not less than the key.
    Left,
    /// `side="right"`: the first index whose value is greater than the key.
    Right,
}

/// `numpy.searchsorted(a, v, side)` of a one-dimensional float array and one key of its type,
/// with numpy's NaN-aware order (`a < b || (b is NaN and a is not)`: NaNs sort last) and each
/// release's bisection, so that the index is numpy's even where `a` is not sorted: numpy 2.2 and
/// 2.3 narrow `[min, max)` around midpoints `min + (max - min) / 2`; numpy 2.5 halves an
/// interval from the base `0` (`npysort/binsearch.cpp`). On a sorted `a` both give the same
/// index. An empty `a` gives `0`.
pub fn searchsorted<T: NpFloat>(a: &[T], v: T, side: Side, release: NumpyRelease) -> usize {
    let less = |x: T, y: T| x < y || (y.is_nan() && !x.is_nan());
    let cmp = |x: T, y: T| match side {
        Side::Left => less(x, y),
        Side::Right => !less(y, x),
    };
    let n = a.len();
    if n == 0 {
        return 0;
    }
    match release {
        NumpyRelease::V2_3 => {
            let (mut lo, mut hi) = (0usize, n);
            while lo < hi {
                let mid = lo + ((hi - lo) >> 1);
                if cmp(a[mid], v) {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            lo
        }
        NumpyRelease::V2_5 => {
            let mut len = n;
            let mut base = 0usize;
            while len > 1 {
                let half = len >> 1;
                len -= half;
                if cmp(a[base + half], v) {
                    base += half;
                }
            }
            base + usize::from(cmp(a[base], v))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weak_percentiles_of_both_releases() {
        let a = [5.0f32, 1.0, 4.0, 2.0, 3.0, 0.5, 7.25];
        for rel in [NumpyRelease::V2_3, NumpyRelease::V2_5] {
            assert_eq!(percentile_linear_weak(&a, 0.0, rel), Ok(0.5));
            assert_eq!(percentile_linear_weak(&a, 100.0, rel), Ok(7.25));
            assert_eq!(percentile_linear_weak(&a, 50.0, rel), Ok(3.0));
            assert_eq!(
                percentile_linear_weak(&a, 100.5, rel),
                Err(PercentileError::Range)
            );
            assert_eq!(
                percentile_linear_weak::<f32>(&[], 5.0, rel),
                Err(PercentileError::Empty)
            );
        }
        // 2.3 rounds q to float32 first: 100.000001 is valid there, not in 2.5
        assert!(percentile_linear_weak(&a, 100.000_001, NumpyRelease::V2_3).is_ok());
        assert!(percentile_linear_weak(&a, 100.000_001, NumpyRelease::V2_5).is_err());
        assert!(
            percentile_linear_weak(&[1.0f32, f32::NAN], 10.0, NumpyRelease::V2_5)
                .unwrap()
                .is_nan()
        );
        assert_eq!(
            percentile_linear(&a, &[0.0, 50.0, 100.0]),
            Ok(vec![0.5, 3.0, 7.25])
        );
    }

    #[test]
    fn medians() {
        assert_eq!(median(&[3.0f32, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[4.0f32, 1.0, 3.0, 2.0]), 2.5);
        assert_eq!(median(&[-0.0f32]).to_bits(), 0); // 0 + (-0 + -0) = +0
        assert!(median::<f32>(&[]).is_nan());
        assert!(median(&[1.0f64, f64::NAN, 2.0]).is_nan());
        // (outer, n, inner) = (1, 4, 2): columns [1, 3, 2, 9] and [0, -1, 5, 2]
        let a = [1.0f32, 0.0, 3.0, -1.0, 2.0, 5.0, 9.0, 2.0];
        assert_eq!(median_axis(&a, [1, 4, 2]), vec![2.5, 1.0]);
        let b = [1.0f32, f32::NAN, 3.0, 4.0, 2.0, 6.0];
        let m = median_axis(&b, [1, 3, 2]);
        assert_eq!(m[0], 2.0);
        assert!(m[1].is_nan());
    }

    #[test]
    fn higher_and_lower_ranks() {
        let a = [5.0f32, 1.0, 4.0, 2.0, 3.0];
        assert_eq!(percentile_higher(&a, 33.0), Some(3.0)); // ceil(4 * 0.33) = 2
        assert_eq!(percentile_higher(&a, 0.0), Some(1.0));
        assert_eq!(percentile_higher(&a, 100.0), Some(5.0));
        assert_eq!(percentile_higher(&a, 101.0), None);
        assert!(
            percentile_higher(&[1.0f32, f32::NAN], 33.0)
                .unwrap()
                .is_nan()
        );
        let b = [3.0f64, -0.0, 1.0, 2.0];
        assert_eq!(scoreatpercentile_lower(&b, 99.5), Some(2.0)); // floor(3 * 0.995) = 2
        assert_eq!(scoreatpercentile_lower(&b, 10.0).unwrap().to_bits(), 0); // -0 -> +0
        assert!(scoreatpercentile_lower(&[], 50.0).unwrap().is_nan());
        assert_eq!(scoreatpercentile_lower(&b, -1.0), None);
    }

    #[test]
    fn rank_percentiles_along_an_axis() {
        // (outer, n, inner) = (2, 5, 1): rows [5, 1, 4, 2, 3] and [0, NaN, 1, 2, 3]
        let a = [5.0f32, 1.0, 4.0, 2.0, 3.0, 0.0, f32::NAN, 1.0, 2.0, 3.0];
        for rel in [NumpyRelease::V2_3, NumpyRelease::V2_5] {
            let lo = percentile_rank_axis(&a, [2, 5, 1], 75.0, RankMethod::Lower, rel).unwrap();
            assert_eq!(lo[0], 4.0); // floor(4 * 0.75) = 3
            assert!(lo[1].is_nan());
            let hi = percentile_rank_axis(&a, [2, 5, 1], 30.0, RankMethod::Higher, rel).unwrap();
            assert_eq!(hi[0], 3.0); // ceil(1.2) = 2
            assert_eq!(
                percentile_rank_axis(&a, [2, 5, 1], 101.0, RankMethod::Lower, rel),
                Err(PercentileError::Range)
            );
            assert_eq!(
                percentile_rank_axis::<f32>(&[], [1, 0, 1], 5.0, RankMethod::Lower, rel),
                Err(PercentileError::Empty)
            );
        }
        // along an outer axis: (1, 3, 2), columns [3, 1, 2] and [6, 5, 4]
        let b = [3.0f64, 6.0, 1.0, 5.0, 2.0, 4.0];
        let r = percentile_rank_axis(&b, [1, 3, 2], 50.0, RankMethod::Lower, NumpyRelease::V2_5);
        assert_eq!(r.unwrap(), vec![2.0, 5.0]);
    }

    #[test]
    fn searchsorted_both_bisections() {
        let a = [0.1f64, 0.4, 0.4, 0.7, 1.0];
        for rel in [NumpyRelease::V2_3, NumpyRelease::V2_5] {
            assert_eq!(searchsorted(&a, 0.4, Side::Left, rel), 1);
            assert_eq!(searchsorted(&a, 0.4, Side::Right, rel), 3);
            assert_eq!(searchsorted(&a, 0.0, Side::Left, rel), 0);
            assert_eq!(searchsorted(&a, 2.0, Side::Left, rel), 5);
            assert_eq!(searchsorted(&a, f64::NAN, Side::Left, rel), 5);
            assert_eq!(searchsorted(&[f64::NAN; 4], 0.5, Side::Left, rel), 0);
            assert_eq!(searchsorted::<f64>(&[], 0.5, Side::Left, rel), 0);
        }
    }

    #[test]
    fn nanpercentile_columns() {
        // two columns: [0, 10, NaN, 20] and [NaN, NaN, NaN, NaN]
        let a = [
            0.0f32,
            f32::NAN,
            10.0,
            f32::NAN,
            f32::NAN,
            f32::NAN,
            20.0,
            f32::NAN,
        ];
        let p = nanpercentile_linear_axis0(&a, 4, 2, &[0.0, 25.0, 50.0, 100.0]).unwrap();
        assert!(!p.float32 && p.all_nan_column);
        assert_eq!(
            [p.values[0], p.values[2], p.values[4], p.values[6]],
            [0.0, 5.0, 10.0, 20.0]
        );
        assert!(p.values[1].is_nan());
        // the first column without numbers makes the result float32
        let b = [f32::NAN, 0.1, f32::NAN, 0.2];
        let p = nanpercentile_linear_axis0(&b, 2, 2, &[50.0]).unwrap();
        assert!(p.float32);
        let d = f64::from(0.2f32 - 0.1f32);
        assert_eq!(
            p.values[1],
            f64::from(f32::from_f64(f64::from(0.2f32) - d * 0.5))
        );
        // q = -0.0: numpy's weight is -0 (vi = -0 minus the integer 0), so a -0 lower
        // neighbour gives -0 (numpy 2.3.5 and 2.5.4: `nanpercentile([[-0.], [1.]], [-0.],
        // axis=0)` is `[[-0.]]`)
        let c = [-0.0f32, 1.0];
        let p = nanpercentile_linear_axis0(&c, 2, 1, &[-0.0]).unwrap();
        assert_eq!(p.values[0].to_bits(), (-0.0f64).to_bits());
    }
}
