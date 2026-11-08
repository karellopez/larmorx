// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! numpy 2.5.4's percentiles and SciPy 1.18.1's `scoreatpercentile` (notice in `super`).
//!
//! The order statistics are selected, not sorted (numpy partitions; SciPy sorts): the value at
//! a rank does not depend on how it is found, except the sign of a zero when `-0.0` and `+0.0`
//! both occur and the payload of a NaN when NaNs with different payloads do (neither is
//! reproduced: the first NaN of the input is returned).

use super::{NpFloat, div_f64};

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
                let f = vi.floor();
                (f as usize, f as usize + 1, f)
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

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
