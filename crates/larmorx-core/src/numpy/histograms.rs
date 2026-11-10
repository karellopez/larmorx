// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy's `linspace` and `histogram` with a number of equal bins (`numpy/_core/function_base.py`
//! and `numpy/lib/_histograms_impl.py` of numpy 2.5.4; numpy 2.3.5 computes the same values:
//! its `_unsigned_subtract` passes the dtype instance where 2.5 passes its type), for a float32 or
//! float64 array and `range=None`, the call scikit-image's `histogram` (and so `threshold_otsu`)
//! makes for a float image. Notice in `super`.

use super::NpFloat;

/// `numpy.linspace(start, stop, num, endpoint=True, dtype=T)` for `start` and `stop` of type `T`
/// (so every step runs in `T`): `delta = stop - start`; `y = arange(num)`; with `div = num - 1 >
/// 0`, `step = delta / div` and `y *= step`, or, where the step is zero, `y /= div` then `y *=
/// delta`; with `div == 0`, `y = y * delta`; then `y += start` and the last value set to `stop`.
pub fn linspace<T: NpFloat>(start: T, stop: T, num: usize) -> Vec<T> {
    let delta = stop.sub(start);
    let mut y: Vec<T> = (0..num).map(|k| T::from_f64(k as f64)).collect();
    if num > 1 {
        let div = T::from_f64((num - 1) as f64);
        let step = delta.div(div);
        if step == T::ZERO {
            for v in &mut y {
                *v = v.div(div).mul(delta);
            }
        } else {
            for v in &mut y {
                *v = v.mul(step);
            }
        }
    } else {
        for v in &mut y {
            *v = v.mul(delta);
        }
    }
    for v in &mut y {
        *v = v.add(start);
    }
    if num > 1 {
        y[num - 1] = stop;
    }
    y
}

/// Why [`histogram`] refused, with numpy's messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistogramError {
    /// `ValueError: '`bins` must be positive, when an integer'`.
    Bins,
    /// `ValueError: autodetected range of [min, max] is not finite` (the two values formatted by
    /// the caller).
    NotFinite,
    /// `ValueError: Too many bins for data range. Cannot create N finite-sized bins.`
    TooManyBins(usize),
}

impl std::fmt::Display for HistogramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HistogramError::Bins => f.write_str("`bins` must be positive, when an integer"),
            HistogramError::NotFinite => f.write_str("autodetected range is not finite"),
            HistogramError::TooManyBins(n) => write!(
                f,
                "Too many bins for data range. Cannot create {n} finite-sized bins."
            ),
        }
    }
}

impl std::error::Error for HistogramError {}

/// numpy's minimum and maximum of a float array (`a.min()`, `a.max()`): a NaN anywhere gives a
/// NaN (numpy propagates it).
pub fn min_max<T: NpFloat>(a: &[T]) -> (T, T) {
    let (mut lo, mut hi) = (a[0], a[0]);
    for &x in &a[1..] {
        if x.is_nan() {
            return (x, x);
        }
        if x < lo {
            lo = x;
        }
        if x > hi {
            hi = x;
        }
    }
    if lo.is_nan() {
        return (lo, lo);
    }
    (lo, hi)
}

/// `numpy.histogram(a, bins=nbins)` of a one-dimensional float array (`range=None`, no weights):
/// the counts and the `nbins + 1` edges.
///
/// The outer edges are the array's minimum and maximum (`0` and `1` for an empty array; widened
/// by `0.5` on each side when equal), the edges `linspace(first, last, nbins + 1)` in `T`; each
/// value's bin is `int(((x - first) / (last - first)) * nbins)`, moved down from `nbins`, then
/// down by one where `x` is below that bin's lower edge and up by one where it reaches the next
/// edge (except in the last bin), as numpy's uniform-bin fast path computes it.
pub fn histogram<T: NpFloat>(a: &[T], nbins: usize) -> Result<(Vec<u64>, Vec<T>), HistogramError> {
    if nbins < 1 {
        return Err(HistogramError::Bins);
    }
    let (mut first, mut last) = if a.is_empty() {
        (T::from_f64(0.0), T::from_f64(1.0))
    } else {
        let (lo, hi) = min_max(a);
        if !(lo.to_f64().is_finite() && hi.to_f64().is_finite()) {
            return Err(HistogramError::NotFinite);
        }
        (lo, hi)
    };
    if first == last {
        first = first.sub(T::from_f64(0.5));
        last = last.add(T::from_f64(0.5));
    }
    let edges = linspace(first, last, nbins + 1);
    if edges.windows(2).any(|w| w[0] >= w[1]) {
        return Err(HistogramError::TooManyBins(nbins));
    }
    let denom = last.sub(first);
    let numer = T::from_f64(nbins as f64);
    let mut counts = vec![0u64; nbins];
    for &x in a {
        let f = x.sub(first).div(denom).mul(numer).to_f64();
        // `astype(np.intp)`: truncation toward zero (the values lie in [0, nbins]).
        let mut idx = f as i64;
        if idx == nbins as i64 {
            idx -= 1;
        }
        if x < edges[idx as usize] {
            idx -= 1;
        }
        if x >= edges[(idx + 1) as usize] && idx != nbins as i64 - 1 {
            idx += 1;
        }
        counts[idx as usize] += 1;
    }
    Ok((counts, edges))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linspace_steps_in_the_type() {
        let e = linspace(0.0f32, 1.0, 4);
        assert_eq!(e, vec![0.0, 1.0f32 / 3.0, 2.0 * (1.0f32 / 3.0), 1.0]);
        assert_eq!(linspace(2.0f64, 5.0, 1), vec![2.0]);
    }

    #[test]
    fn histogram_bins_and_edges() {
        let a = [0.0f32, 0.25, 0.5, 0.75, 1.0, 1.0];
        let (c, e) = histogram(&a, 4).unwrap();
        assert_eq!(c, vec![1, 1, 1, 3]);
        assert_eq!(e, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
        let (c, e) = histogram(&[3.0f64, 3.0], 2).unwrap();
        assert_eq!((c, e), (vec![0, 2], vec![2.5, 3.0, 3.5]));
        assert_eq!(
            histogram(&[f32::NAN, 1.0], 2),
            Err(HistogramError::NotFinite)
        );
    }
}
