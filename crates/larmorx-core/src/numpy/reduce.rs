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
/// in order otherwise).
pub fn add_reduce_axis<T: NpFloat>(a: &[T], shape: [usize; 3]) -> Vec<T> {
    let [outer, n, inner] = shape;
    assert_eq!(
        a.len(),
        outer * n * inner,
        "array length and shape disagree"
    );
    if inner == 1 {
        return (0..outer)
            .map(|o| T::ZERO.add(pairwise_sum(a, o * n, n, 1)))
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
    let n = shape[1] as i64;
    add_reduce_axis(a, shape)
        .into_iter()
        .map(|s| divide_by_count(s, n))
        .collect()
}

/// `numpy.var(a, axis, ddof=ddof)` over the middle axis (`_methods._var`: the mean with the
/// reduced axis kept, the deviations `a - mean` in `T`, their squares, their sum by the same
/// order as the first, divided by `max(n - ddof, 0)`).
pub fn var_axis<T: NpFloat>(a: &[T], shape: [usize; 3], ddof: i64) -> Vec<T> {
    let [outer, n, inner] = shape;
    let mean = mean_axis(a, shape);
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
    add_reduce_axis(&sq, shape)
        .into_iter()
        .map(|s| divide_by_count(s, dof))
        .collect()
}

/// `numpy.std(a, axis, ddof=ddof)`: the square root of [`var_axis`].
pub fn std_axis<T: NpFloat>(a: &[T], shape: [usize; 3], ddof: i64) -> Vec<T> {
    var_axis(a, shape, ddof)
        .into_iter()
        .map(NpFloat::sqrt)
        .collect()
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
