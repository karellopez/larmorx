// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy 2.5.4's sums over one axis and the statistics built on them (notice in `super`).
//!
//! **The order of a sum.** `numpy.add.reduce` starts every result at `+0.0` (the identity of
//! `add`, `umath/reduction.c`) and adds the operand in the order its iterator visits it:
//!
//! - when the summed axis is the innermost one left after the axes of length 1 are dropped (a
//!   reduction over the last axis, or over an outer axis when every later axis has length 1),
//!   the inner loop is a *reduce* loop and numpy sums it pairwise (`@TYPE@_pairwise_sum`,
//!   `umath/loops_utils.h.src`: blocks of up to 128 with eight accumulators, halves rounded down
//!   to a multiple of 8 above that), so the result is `0 + pairwise(x)`;
//! - otherwise the inner loop runs along a later axis and the summed axis is an outer loop, so
//!   each result is `((0 + x0) + x1) + ...` in index order.
//!
//! Arrays are C-ordered and described as `(outer, n, inner)`: the summed axis has length `n`,
//! the axes before it multiply to `outer` and those after it to `inner`.
//!
//! **Divisions by a count** (`mean`, `var`, `nanvar`'s `_divide_by_count`) are numpy's
//! `true_divide` of the float array by an `intp` count: under NEP 50 that runs in `float64` and
//! is cast back to the array's type (for `float32`, the same as a single-precision division:
//! the double rounding is innocuous).

use super::{NpFloat, div_f64};

/// numpy's `PW_BLOCKSIZE`.
const PW_BLOCKSIZE: usize = 128;

/// numpy's pairwise sum of `n` values `a[off + i * stride]` (`@TYPE@_pairwise_sum`).
pub fn pairwise_sum<T: NpFloat>(a: &[T], off: usize, n: usize, stride: usize) -> T {
    let at = |i: usize| a[off + i * stride];
    if n < 8 {
        let mut res = T::NEG_ZERO;
        for i in 0..n {
            res = res.add(at(i));
        }
        res
    } else if n <= PW_BLOCKSIZE {
        let mut r = [T::ZERO; 8];
        for (k, rk) in r.iter_mut().enumerate() {
            *rk = at(k);
        }
        let mut i = 8;
        while i < n - (n % 8) {
            for (k, rk) in r.iter_mut().enumerate() {
                *rk = rk.add(at(i + k));
            }
            i += 8;
        }
        let mut res = (r[0].add(r[1]).add(r[2].add(r[3]))).add(r[4].add(r[5]).add(r[6].add(r[7])));
        while i < n {
            res = res.add(at(i));
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        pairwise_sum(a, off, n2, stride).add(pairwise_sum(a, off + n2 * stride, n - n2, stride))
    }
}

/// `numpy.add.reduce` of a contiguous one-dimensional array: `0 + pairwise(a)`.
pub fn add_reduce<T: NpFloat>(a: &[T]) -> T {
    T::ZERO.add(pairwise_sum(a, 0, a.len(), 1))
}

/// `numpy.add.reduce(a, axis)` over the middle axis of a C-ordered `(outer, n, inner)` array;
/// the result is C-ordered `(outer, inner)` (module documentation: pairwise when `inner == 1`,
/// in order otherwise). numpy 2.3 and later; [`add_reduce_axis_in`] for numpy 2.2's order.
pub fn add_reduce_axis<T: NpFloat>(a: &[T], shape: [usize; 3]) -> Vec<T> {
    add_reduce_axis_in(a, shape, SumOrder::Pairwise)
}

/// [`add_reduce_axis`] in a release's order ([`SumOrder`]): where the summed axis is the
/// innermost in memory (`inner == 1`), numpy 2.2's buffered reduction hands the inner loop at
/// most `NPY_BUFSIZE` (8192) elements of a row, so each result is `((0 + pairwise(c0)) +
/// pairwise(c1)) + ...` over the row's chunks of 8192 (measured in numpy 2.2.6:
/// `validation/investigations/confounds/probe_reduction_orders.py`; a row of 8192 elements or
/// fewer is never cut, whatever the rows before it); numpy 2.3 and later sum the whole row
/// pairwise. Along an outer axis every release adds in index order. A cast to a wider type
/// (`int16` or `float32` data summed in `float64`) is buffered in every release, but those sums
/// are exact, so no order shows.
pub fn add_reduce_axis_in<T: NpFloat>(a: &[T], shape: [usize; 3], order: SumOrder) -> Vec<T> {
    let [outer, n, inner] = shape;
    assert_eq!(
        a.len(),
        outer * n * inner,
        "array length and shape disagree"
    );
    if inner == 1 {
        return (0..outer)
            .map(|o| add_reduce_ordered(&a[o * n..(o + 1) * n], order))
            .collect();
    }
    let mut out = vec![T::ZERO; outer * inner];
    for o in 0..outer {
        let acc = &mut out[o * inner..(o + 1) * inner];
        for e in 0..n {
            let row = &a[(o * n + e) * inner..(o * n + e + 1) * inner];
            for (s, &x) in acc.iter_mut().zip(row) {
                *s = s.add(x);
            }
        }
    }
    out
}

/// `true_divide(x, count)` as numpy runs it for an array of `T` and an `intp` count.
#[inline(always)]
fn divide_by_count<T: NpFloat>(x: T, count: i64) -> T {
    T::from_f64(div_f64(x.to_f64(), count as f64))
}

/// `numpy.mean(a, axis)` over the middle axis of `(outer, n, inner)` (`_methods._mean`: the sum,
/// then the division by the count; `n == 0` gives NaN, as numpy's "Mean of empty slice").
pub fn mean_axis<T: NpFloat>(a: &[T], shape: [usize; 3]) -> Vec<T> {
    mean_axis_in(a, shape, SumOrder::Pairwise)
}

/// [`mean_axis`] in a release's order ([`add_reduce_axis_in`]).
pub fn mean_axis_in<T: NpFloat>(a: &[T], shape: [usize; 3], order: SumOrder) -> Vec<T> {
    let n = shape[1] as i64;
    add_reduce_axis_in(a, shape, order)
        .into_iter()
        .map(|s| divide_by_count(s, n))
        .collect()
}

/// `numpy.var(a, axis, ddof=ddof)` over the middle axis (`_methods._var`: the mean with the
/// reduced axis kept, the deviations `a - mean` in `T`, their squares, their sum by the same
/// order as the first, divided by `max(n - ddof, 0)`).
pub fn var_axis<T: NpFloat>(a: &[T], shape: [usize; 3], ddof: i64) -> Vec<T> {
    var_axis_in(a, shape, ddof, SumOrder::Pairwise)
}

/// [`var_axis`] in a release's order ([`add_reduce_axis_in`]).
pub fn var_axis_in<T: NpFloat>(a: &[T], shape: [usize; 3], ddof: i64, order: SumOrder) -> Vec<T> {
    let [outer, n, inner] = shape;
    let mean = mean_axis_in(a, shape, order);
    let mut sq = Vec::with_capacity(a.len());
    for o in 0..outer {
        for e in 0..n {
            for j in 0..inner {
                let d = a[(o * n + e) * inner + j].sub(mean[o * inner + j]);
                sq.push(d.mul(d));
            }
        }
    }
    let dof = (n as i64 - ddof).max(0);
    add_reduce_axis_in(&sq, shape, order)
        .into_iter()
        .map(|s| divide_by_count(s, dof))
        .collect()
}

/// `numpy.std(a, axis, ddof=ddof)`: the square root of [`var_axis`].
pub fn std_axis<T: NpFloat>(a: &[T], shape: [usize; 3], ddof: i64) -> Vec<T> {
    std_axis_in(a, shape, ddof, SumOrder::Pairwise)
}

/// [`std_axis`] in a release's order ([`add_reduce_axis_in`]).
pub fn std_axis_in<T: NpFloat>(a: &[T], shape: [usize; 3], ddof: i64, order: SumOrder) -> Vec<T> {
    var_axis_in(a, shape, ddof, order)
        .into_iter()
        .map(NpFloat::sqrt)
        .collect()
}

/// The order numpy adds a whole contiguous array in (`np.add.reduce(a, axis=None)`, and so
/// `a.sum()`, `a.mean()`, `a.var()`, `a.std()`), which changed with numpy 2.3's rewrite of the
/// iterator's buffering (measured: `validation/investigations/sdcflows-fieldmap-b/`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SumOrder {
    /// numpy 2.3 onwards: `0 + pairwise(a)` over the whole array.
    #[default]
    Pairwise,
    /// numpy 2.2 and older: the buffered reduction's inner loop gets at most `NPY_BUFSIZE` (8192)
    /// elements, so the result is `((0 + pairwise(c0)) + pairwise(c1)) + ...` over chunks of 8192
    /// elements (fMRIPrep 25.2.6's image has numpy 2.2.6).
    Buffered,
}

impl SumOrder {
    /// The order of numpy release `version` (`"2.2.6"`, `"2.5.4"`, ...); numpy 1.x sums as 2.2.
    pub fn from_version(version: &str) -> Option<SumOrder> {
        let mut it = version.split('.');
        let major: u32 = it.next()?.parse().ok()?;
        let minor: u32 = it
            .next()?
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()?;
        Some(if major > 2 || (major == 2 && minor >= 3) {
            SumOrder::Pairwise
        } else {
            SumOrder::Buffered
        })
    }
}

/// numpy's `NPY_BUFSIZE`.
const NPY_BUFSIZE: usize = 8192;

/// `numpy.add.reduce` of a whole contiguous array in the release's order ([`SumOrder`]).
pub fn add_reduce_ordered<T: NpFloat>(a: &[T], order: SumOrder) -> T {
    match order {
        SumOrder::Pairwise => add_reduce(a),
        SumOrder::Buffered => a
            .chunks(NPY_BUFSIZE)
            .fold(T::ZERO, |acc, c| acc.add(pairwise_sum(c, 0, c.len(), 1))),
    }
}

/// `a.mean()` of a whole contiguous array in the release's order (`_methods._mean`: the sum,
/// divided by the count in `float64` and rounded to `T`; an empty array gives NaN).
pub fn mean_ordered<T: NpFloat>(a: &[T], order: SumOrder) -> T {
    divide_by_count(add_reduce_ordered(a, order), a.len() as i64)
}

/// `a.var(ddof=ddof)` of a whole contiguous array in the release's order (`_methods._var`: the
/// mean, the squared deviations, their sum divided by `max(n - ddof, 0)`).
pub fn var_ordered<T: NpFloat>(a: &[T], ddof: i64, order: SumOrder) -> T {
    let n = a.len() as i64;
    let mean = mean_ordered(a, order);
    let sq: Vec<T> = a
        .iter()
        .map(|&x| {
            let d = x.sub(mean);
            d.mul(d)
        })
        .collect();
    divide_by_count(add_reduce_ordered(&sq, order), (n - ddof).max(0))
}

/// `a.std(ddof=ddof)` of a whole contiguous array in the release's order (`_methods._var`: the
/// mean, the squared deviations, their sum divided by `max(n - ddof, 0)`, the square root).
pub fn std_ordered<T: NpFloat>(a: &[T], ddof: i64, order: SumOrder) -> T {
    var_ordered(a, ddof, order).sqrt()
}

/// `numpy.cumsum(a)` of a one-dimensional array (`add.accumulate`): the first value as it is
/// (a `-0.0` stays `-0.0`), then `out[i] = out[i - 1] + a[i]` in index order, in `T`.
pub fn cumsum<T: NpFloat>(a: &[T]) -> Vec<T> {
    let mut out = Vec::with_capacity(a.len());
    let mut acc = T::ZERO;
    for (i, &x) in a.iter().enumerate() {
        acc = if i == 0 { x } else { acc.add(x) };
        out.push(acc);
    }
    out
}

/// Which extreme [`extreme`] takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Extreme {
    /// `a.min()` (`numpy.minimum.reduce`).
    Min,
    /// `a.max()` (`numpy.maximum.reduce`).
    Max,
}

/// `a.min()` or `a.max()` of a float array (`numpy.minimum.reduce`, `numpy.maximum.reduce`): a
/// NaN anywhere gives a NaN (the first in index order; numpy keeps one of them), else the
/// smallest or largest value. Between `-0.0` and `+0.0` numpy's answer depends on the CPU's
/// SIMD kernel (the lanes' order of x86's `min`/`max` instructions, which return their second
/// operand on equal values: AVX-512, AVX2 and SSE give different signs on the same array,
/// `validation/investigations/confounds/probe_minmax_zero_sign.py`); larmorx returns IEEE
/// 754-2019's `minimum`/`maximum` instead (`-0.0 < +0.0`), the same on every machine. `None` for
/// an empty array (numpy's `ValueError: zero-size array to reduction operation minimum which has
/// no identity`).
pub fn extreme<T: NpFloat>(a: &[T], which: Extreme) -> Option<T> {
    let (&first, rest) = a.split_first()?;
    if let Some(&nan) = a.iter().find(|x| x.is_nan()) {
        return Some(nan);
    }
    let mut r = first;
    for &x in rest {
        let take = match which {
            Extreme::Min => x < r || (x == r && x.to_f64().is_sign_negative()),
            Extreme::Max => x > r || (x == r && x.to_f64().is_sign_positive()),
        };
        if take {
            r = x;
        }
    }
    Some(r)
}

/// `numpy.nansum(a, axis)`: NaNs replaced by `0`, then [`add_reduce_axis`].
pub fn nansum_axis<T: NpFloat>(a: &[T], shape: [usize; 3]) -> Vec<T> {
    let replaced: Vec<T> = a
        .iter()
        .map(|&x| if x.is_nan() { T::ZERO } else { x })
        .collect();
    add_reduce_axis(&replaced, shape)
}

/// The result of [`nanvar_axis`] and [`nanstd_axis`].
#[derive(Debug, Clone)]
pub struct NanVar<T> {
    /// The values, C-ordered `(outer, inner)`.
    pub values: Vec<T>,
    /// Whether some slice had `n - ddof <= 0` values (numpy then warns "Degrees of freedom <= 0
    /// for slice." and sets those results to NaN).
    pub bad_dof: bool,
}

/// `numpy.nanvar(a, axis, ddof=ddof)` over the middle axis (`_nanfunctions_impl.nanvar`: NaNs
/// replaced by `0`; the count of the others; their sum divided by it; `a - avg` everywhere,
/// the NaN places set back to `0`; the squares summed and divided by `count - ddof`; slices with
/// `count - ddof <= 0` give numpy's `np.nan`).
pub fn nanvar_axis<T: NpFloat>(a: &[T], shape: [usize; 3], ddof: i64) -> NanVar<T> {
    let [outer, n, inner] = shape;
    assert_eq!(
        a.len(),
        outer * n * inner,
        "array length and shape disagree"
    );
    let mut arr: Vec<T> = a
        .iter()
        .map(|&x| if x.is_nan() { T::ZERO } else { x })
        .collect();
    let mut cnt = vec![0i64; outer * inner];
    for o in 0..outer {
        for e in 0..n {
            for j in 0..inner {
                if !a[(o * n + e) * inner + j].is_nan() {
                    cnt[o * inner + j] += 1;
                }
            }
        }
    }
    let avg: Vec<T> = add_reduce_axis(&arr, shape)
        .into_iter()
        .zip(&cnt)
        .map(|(s, &c)| divide_by_count(s, c))
        .collect();
    for o in 0..outer {
        for e in 0..n {
            for j in 0..inner {
                let k = (o * n + e) * inner + j;
                let d = if a[k].is_nan() {
                    T::ZERO
                } else {
                    arr[k].sub(avg[o * inner + j])
                };
                arr[k] = d.mul(d);
            }
        }
    }
    let mut bad_dof = false;
    let values = add_reduce_axis(&arr, shape)
        .into_iter()
        .zip(&cnt)
        .map(|(s, &c)| {
            let dof = c - ddof;
            let v = divide_by_count(s, dof);
            if dof <= 0 {
                bad_dof = true;
                T::NAN
            } else {
                v
            }
        })
        .collect();
    NanVar { values, bad_dof }
}

/// `numpy.nanstd(a, axis, ddof=ddof)`: the square root of [`nanvar_axis`].
pub fn nanstd_axis<T: NpFloat>(a: &[T], shape: [usize; 3], ddof: i64) -> NanVar<T> {
    let mut v = nanvar_axis(a, shape, ddof);
    for x in &mut v.values {
        *x = x.sqrt();
    }
    v
}

/// `numpy.average(a, axis, weights=w)` over the middle axis of `(outer, n, inner)`, `a` of type
/// `A` and the weights of `float64` (the result type: `float64`): the weights' sum, the products
/// `a * w` in `float64` summed by the same order, divided by the weights' sum. `Err` where
/// numpy raises `ZeroDivisionError("Weights sum to zero, can't be normalized")` (some sum of
/// weights equal to zero).
pub fn average_axis<A: NpFloat>(
    a: &[A],
    w: &[f64],
    shape: [usize; 3],
) -> Result<Vec<f64>, ZeroWeights> {
    assert_eq!(a.len(), w.len(), "data and weights differ in length");
    let scl = add_reduce_axis(w, shape);
    if scl.contains(&0.0) {
        return Err(ZeroWeights);
    }
    let prod: Vec<f64> = a
        .iter()
        .zip(w)
        .map(|(&x, &wt)| x.to_f64().mul(wt))
        .collect();
    Ok(add_reduce_axis(&prod, shape)
        .into_iter()
        .zip(&scl)
        .map(|(s, &d)| div_f64(s, d))
        .collect())
}

/// numpy's `ZeroDivisionError` in `average`: some sum of weights is zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroWeights;

impl std::fmt::Display for ZeroWeights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Weights sum to zero, can't be normalized")
    }
}

impl std::error::Error for ZeroWeights {}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> f32 {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((*seed >> 40) as f32) / ((1u64 << 24) as f32) * 1000.0 - 300.0
    }

    #[test]
    fn pairwise_grouping_differs_from_order_above_eight() {
        // 0 + pairwise: for n < 8 the sum in order, for n >= 8 eight accumulators.
        let a: Vec<f32> = vec![1e8, 1.0, -1e8, 1.0, 0.5, 0.25, 3.0, 7.0, 1.0];
        let r = &a[..8];
        let blocks = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7])) + a[8];
        assert_eq!(add_reduce(&a), 0.0 + blocks);
        assert_ne!(blocks, a.iter().fold(0.0f32, |s, &x| s + x));
        let short = &a[..7];
        assert_eq!(add_reduce(short), short.iter().fold(-0.0f32, |s, &x| s + x));
        assert_eq!(add_reduce(&[-0.0f32]).to_bits(), 0); // +0, as numpy
    }

    #[test]
    fn numpy_2_2_sums_in_buffered_chunks() {
        let a: Vec<f32> = (0..20000)
            .map(|i| ((i * 7919) % 1000) as f32 * 0.001)
            .collect();
        let want = 0.0f32 + pairwise_sum(&a, 0, 8192, 1);
        let want = (want + pairwise_sum(&a, 8192, 8192, 1)) + pairwise_sum(&a, 16384, 3616, 1);
        assert_eq!(
            add_reduce_ordered(&a, SumOrder::Buffered).to_bits(),
            want.to_bits()
        );
        assert_eq!(SumOrder::from_version("2.2.6"), Some(SumOrder::Buffered));
        assert_eq!(SumOrder::from_version("2.3.5"), Some(SumOrder::Pairwise));
        assert_eq!(
            add_reduce_ordered(&a, SumOrder::Pairwise).to_bits(),
            add_reduce(&a).to_bits()
        );
    }

    #[test]
    fn axis_order_by_inner_length() {
        let mut s = 7u64;
        let a: Vec<f32> = (0..3 * 20 * 5).map(|_| lcg(&mut s)).collect();
        // inner > 1: in order along the summed axis
        let out = add_reduce_axis(&a, [3, 20, 5]);
        for o in 0..3 {
            for j in 0..5 {
                let mut acc = 0.0f32;
                for e in 0..20 {
                    acc += a[(o * 20 + e) * 5 + j];
                }
                assert_eq!(out[o * 5 + j].to_bits(), acc.to_bits());
            }
        }
        // inner == 1: pairwise
        let out = add_reduce_axis(&a[..60], [3, 20, 1]);
        for (o, v) in out.iter().enumerate() {
            assert_eq!(v.to_bits(), add_reduce(&a[o * 20..o * 20 + 20]).to_bits());
        }
    }

    #[test]
    fn numpy_2_2_cuts_long_rows_only() {
        // rows of 8193 (cut into 8192 + 1) and rows of 300 (never cut)
        let a: Vec<f32> = (0..2 * 8193)
            .map(|i| ((i * 7919) % 1000) as f32 * 0.37 - 100.0)
            .collect();
        let got = add_reduce_axis_in(&a, [2, 8193, 1], SumOrder::Buffered);
        for (o, g) in got.iter().enumerate() {
            let row = &a[o * 8193..(o + 1) * 8193];
            let want = (0.0f32 + pairwise_sum(row, 0, 8192, 1)) + row[8192];
            assert_eq!(g.to_bits(), want.to_bits());
        }
        let short = add_reduce_axis_in(&a[..3000], [10, 300, 1], SumOrder::Buffered);
        assert_eq!(short, add_reduce_axis(&a[..3000], [10, 300, 1]));
        let m = mean_axis_in(&a, [2, 8193, 1], SumOrder::Buffered);
        assert_eq!(m[0], (f64::from(got[0]) / 8193.0) as f32);
        assert_eq!(
            mean_ordered(&a, SumOrder::Pairwise),
            (f64::from(add_reduce(&a)) / a.len() as f64) as f32
        );
        assert!(mean_ordered::<f64>(&[], SumOrder::Pairwise).is_nan());
        assert_eq!(var_ordered(&[1.0f64, 3.0], 1, SumOrder::Pairwise), 2.0);
    }

    #[test]
    fn cumsum_and_extremes() {
        let c = cumsum(&[-0.0f64, 1.0, 2.5]);
        assert_eq!(c[0].to_bits(), (-0.0f64).to_bits()); // a copy, not 0 + -0
        assert_eq!(&c[1..], &[1.0, 3.5]);
        assert!(cumsum::<f32>(&[]).is_empty());
        let z = [0.0f64, 2.0, -0.0, 0.0];
        assert_eq!(
            extreme(&z, Extreme::Min).unwrap().to_bits(),
            (-0.0f64).to_bits()
        );
        let nz = [-0.0f32, -2.0, 0.0, -0.0];
        assert_eq!(extreme(&nz, Extreme::Max).unwrap().to_bits(), 0);
        assert!(
            extreme(&[1.0f32, f32::NAN, -3.0], Extreme::Min)
                .unwrap()
                .is_nan()
        );
        assert_eq!(extreme(&[1.0f32, 7.0, -3.0], Extreme::Max), Some(7.0));
        assert_eq!(extreme::<f64>(&[], Extreme::Max), None);
    }

    #[test]
    fn nanvar_of_empty_slices_is_numpys_nan() {
        let a = [f32::NAN, 1.0, f32::NAN, 3.0]; // columns [NaN, NaN] and [1, 3]
        let v = nanvar_axis(&a, [1, 2, 2], 0);
        assert!(v.bad_dof);
        assert_eq!(v.values[0].to_bits(), 0x7fc0_0000);
        assert_eq!(v.values[1], 1.0);
        assert_eq!(nanvar_axis(&[1.0f32, 3.0], [1, 2, 1], 0).values[0], 1.0);
    }

    #[test]
    fn average_refuses_zero_weights() {
        assert_eq!(
            average_axis(&[1.0f32, 2.0], &[0.0, 0.0], [1, 2, 1]),
            Err(ZeroWeights)
        );
        assert_eq!(
            average_axis(&[1.0f32, 2.0], &[1.0, 3.0], [1, 2, 1]),
            Ok(vec![1.75])
        );
    }
}
