// SPDX-License-Identifier: Apache-2.0
//! Thresholds, as ITK v5.4.5 computes them:
//! - [`binary_threshold`]: `BinaryThresholdImageFilter` (inside if `lower ≤ v ≤ upper`);
//! - [`otsu_thresholds`]: `OtsuMultipleThresholdsCalculator` on a histogram (exhaustive
//!   search over threshold combinations, maximising the between-class variance);
//! - [`threshold_labels`]: `ThresholdLabelerImageFilter` (the bucket of each value);
//! - [`otsu_multiple_thresholds`]: `OtsuMultipleThresholdsImageFilter`, the three together.
//!
//! Voxel-wise work runs in parallel; the results do not depend on the thread count.

use larmorx_core::parallel;
use rayon::prelude::*;

use crate::FilterError;
use crate::statistics::{Histogram, scalar_image_histogram};

const CHUNK: usize = 1 << 16;

/// `BinaryThresholdImageFilter`: `inside` where `lower ≤ v ≤ upper`, else `outside` (NaN is
/// outside). The comparison is in the input type, as ITK's functor compares `TInput` values.
/// `lower > upper` is an error, with ITK's message.
pub fn binary_threshold<T, U>(
    input: &[T],
    lower: T,
    upper: T,
    inside: U,
    outside: U,
    n_threads: usize,
) -> Result<Vec<U>, FilterError>
where
    T: Copy + PartialOrd + Send + Sync,
    U: Copy + Send + Sync,
{
    if lower > upper {
        return Err(FilterError::invalid(
            "Lower threshold cannot be greater than upper threshold.",
        ));
    }
    Ok(parallel::with_threads(n_threads, || {
        input
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| {
                if lower <= v && v <= upper {
                    inside
                } else {
                    outside
                }
            })
            .collect()
    })?)
}

/// `OtsuMultipleThresholdsCalculator::Compute`: the `n` thresholds that maximise the
/// between-class variance of `histogram`, found by trying every combination of bin indices in
/// ITK's order and keeping a new maximum only when it is larger by more than one ulp. Each
/// threshold is the upper bound of its bin, or the bin's middle with `return_bin_midpoint`
/// (ITK 5's default is the upper bound; ITKv4 compatibility builds return the middle).
/// `valley_emphasis` weights the variance as Ng (2006) proposes.
pub fn otsu_thresholds(
    histogram: &Histogram,
    n: usize,
    valley_emphasis: bool,
    return_bin_midpoint: bool,
) -> Vec<f64> {
    let bins = histogram.len();
    let freq = |i: usize| histogram.frequency[i];
    let global_frequency = histogram.total_frequency();
    let mut global_mean = 0.0f64;
    for i in 0..bins {
        global_mean += histogram.measurement(i) * freq(i) as f64;
    }
    global_mean /= global_frequency as f64;

    let classes = n + 1;
    let mut thresholds: Vec<usize> = (0..n).collect();
    let mut best = thresholds.clone();
    let mut class_frequency = vec![0u64; classes];
    let mut freq_sum = 0u64;
    for j in 0..classes - 1 {
        class_frequency[j] = freq(thresholds[j]);
        freq_sum += class_frequency[j];
    }
    class_frequency[classes - 1] = global_frequency.wrapping_sub(freq_sum);
    let pdf: Vec<f64> = (0..bins)
        .map(|j| freq(j) as f64 / global_frequency as f64)
        .collect();
    let mut class_mean = vec![0.0f64; classes];
    let mut mean_sum = 0.0f64;
    for j in 0..classes - 1 {
        class_mean[j] = if class_frequency[j] > 0 {
            histogram.measurement(j)
        } else {
            0.0
        };
        mean_sum += class_mean[j] * class_frequency[j] as f64;
    }
    class_mean[classes - 1] = if class_frequency[classes - 1] > 0 {
        (global_mean * global_frequency as f64 - mean_sum) / class_frequency[classes - 1] as f64
    } else {
        0.0
    };
    let variance = |class_frequency: &[u64], class_mean: &[f64], thresholds: &[usize]| {
        let mut v = 0.0f64;
        for j in 0..classes {
            v += class_frequency[j] as f64 * (class_mean[j] * class_mean[j]);
        }
        v /= global_frequency as f64;
        if valley_emphasis {
            let mut factor = 0.0;
            for &t in thresholds.iter().take(classes - 1) {
                factor += pdf[t];
            }
            v *= 1.0 - factor;
        }
        v
    };
    // ITK's initial valley factor keeps only the last threshold's weight (an assignment where
    // the loop below accumulates).
    let mut max_var = {
        let mut v = 0.0f64;
        for j in 0..classes {
            v += class_frequency[j] as f64 * (class_mean[j] * class_mean[j]);
        }
        v /= global_frequency as f64;
        if valley_emphasis {
            let factor = thresholds.last().map_or(0.0, |&t| pdf[t]);
            v *= 1.0 - factor;
        }
        v
    };
    while increment_thresholds(
        histogram,
        &mut thresholds,
        global_mean,
        &mut class_mean,
        &mut class_frequency,
    ) {
        let v = variance(&class_frequency, &class_mean, &thresholds);
        if v > max_var
            && !crate::itk_math::float_almost_equal_f64(max_var, v, 1, 0.1 * f64::EPSILON)
        {
            max_var = v;
            best.clone_from(&thresholds);
        }
    }
    best.iter()
        .map(|&t| {
            if return_bin_midpoint {
                histogram.measurement(t)
            } else {
                histogram.bin_max[t]
            }
        })
        .collect()
}

/// `OtsuMultipleThresholdsCalculator::IncrementThresholds`: the next combination of
/// threshold indices, updating class means and frequencies incrementally.
fn increment_thresholds(
    histogram: &Histogram,
    thresholds: &mut [usize],
    global_mean: f64,
    class_mean: &mut [f64],
    class_frequency: &mut [u64],
) -> bool {
    let bins = histogram.len();
    let n = thresholds.len();
    let classes = class_mean.len();
    let freq = |i: usize| histogram.frequency[i];
    for j in (0..n).rev() {
        // `bins - 2 - (n - 1 - j)` in ITK's unsigned arithmetic.
        let limit = bins.wrapping_sub(2).wrapping_sub(n - 1 - j);
        if thresholds[j] < limit {
            thresholds[j] += 1;
            let mean_old = class_mean[j];
            let freq_old = class_frequency[j];
            class_frequency[j] += freq(thresholds[j]);
            class_mean[j] = if class_frequency[j] > 0 {
                (mean_old * freq_old as f64
                    + histogram.measurement(thresholds[j]) * freq(thresholds[j]) as f64)
                    / class_frequency[j] as f64
            } else {
                0.0
            };
            for k in j + 1..n {
                thresholds[k] = thresholds[k - 1] + 1;
                class_frequency[k] = freq(thresholds[k]);
                class_mean[k] = if class_frequency[k] > 0 {
                    histogram.measurement(thresholds[k])
                } else {
                    0.0
                };
            }
            let total = histogram.total_frequency();
            class_frequency[classes - 1] = total;
            class_mean[classes - 1] = global_mean * total as f64;
            for k in 0..classes - 1 {
                class_frequency[classes - 1] =
                    class_frequency[classes - 1].wrapping_sub(class_frequency[k]);
                class_mean[classes - 1] -= class_mean[k] * class_frequency[k] as f64;
            }
            if class_frequency[classes - 1] > 0 {
                class_mean[classes - 1] /= class_frequency[classes - 1] as f64;
            } else {
                class_mean[classes - 1] = 0.0;
            }
            return true;
        } else if j == 0 {
            return false;
        }
    }
    true
}

/// `ThresholdLabelerImageFilter`: for each value, the index of the first threshold it does not
/// exceed (`v ≤ t`, compared in double), plus `offset`; values above every threshold (and NaN)
/// get `thresholds.len() + offset`. The thresholds must be sorted (ITK checks them as stored
/// in the input type, `float` here).
pub fn threshold_labels(
    input: &[f32],
    thresholds: &[f64],
    offset: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let stored: Vec<f32> = thresholds.iter().map(|&t| t as f32).collect();
    if stored.windows(2).any(|w| w[0] > w[1]) {
        return Err(FilterError::invalid("Thresholds must be sorted."));
    }
    Ok(parallel::with_threads(n_threads, || {
        input
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| {
                let a = f64::from(v);
                let (mut low, mut high) = (0usize, thresholds.len());
                while low < high {
                    let mid = (low + high) / 2;
                    if a <= thresholds[mid] {
                        high = mid;
                    } else {
                        low = mid + 1;
                    }
                }
                low as f32 + offset
            })
            .collect()
    })?)
}

/// ITK loops forever with no threshold (`IncrementThresholds` never returns false) and reads
/// past the histogram with as many thresholds as bins; both are errors here.
pub fn check_otsu(n: usize, bins: usize) -> Result<(), FilterError> {
    if n == 0 || n >= bins {
        return Err(FilterError::invalid(format!(
            "Otsu needs 1 to {} thresholds for {bins} histogram bins, not {n}",
            bins.saturating_sub(1)
        )));
    }
    Ok(())
}

/// The result of [`otsu_multiple_thresholds`].
#[derive(Clone, Debug, PartialEq)]
pub struct OtsuResult {
    /// Labels `0..=n`, one per voxel.
    pub labels: Vec<f32>,
    /// The thresholds (upper bounds of the chosen bins).
    pub thresholds: Vec<f64>,
}

/// `OtsuMultipleThresholdsImageFilter<float, float>` with ITK 5's defaults (no valley
/// emphasis, label offset 0, bin upper bounds as thresholds): a `bins`-bin histogram of the
/// image (`ScalarImageToHistogramGenerator`), `n` Otsu thresholds, and each voxel labelled by
/// its bucket.
pub fn otsu_multiple_thresholds(
    input: &[f32],
    n: usize,
    bins: usize,
    n_threads: usize,
) -> Result<OtsuResult, FilterError> {
    check_otsu(n, bins)?;
    let histogram = scalar_image_histogram(input, bins, n_threads)?;
    let thresholds = otsu_thresholds(&histogram, n, false, false);
    let labels = threshold_labels(input, &thresholds, 0.0, n_threads)?;
    Ok(OtsuResult { labels, thresholds })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_threshold_is_inclusive() {
        let v = [0.0f32, 0.5, 0.75, 1.0, 1.5, f32::NAN];
        let out = binary_threshold(&v, 0.5, 1.0, 1.0f32, 0.0, 2).unwrap();
        assert_eq!(out, [0.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
        assert!(binary_threshold(&v, 2.0, 1.0, 1.0f32, 0.0, 1).is_err());
    }

    #[test]
    fn otsu_separates_two_clusters() {
        let mut v: Vec<f32> = (0..500).map(|i| 10.0 + (i % 7) as f32).collect();
        v.extend((0..500).map(|i| 100.0 + (i % 11) as f32));
        let r = otsu_multiple_thresholds(&v, 1, 128, 3).unwrap();
        assert_eq!(r.thresholds.len(), 1);
        assert!(r.thresholds[0] > 16.0 && r.thresholds[0] < 100.0);
        assert!(r.labels[..500].iter().all(|&l| l == 0.0));
        assert!(r.labels[500..].iter().all(|&l| l == 1.0));
        let r3 = otsu_multiple_thresholds(&v, 3, 128, 1).unwrap();
        assert_eq!(r3.thresholds.len(), 3);
        assert!(r3.thresholds.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn labeler_puts_ties_in_the_lower_bucket() {
        let out = threshold_labels(&[1.0, 2.0, 2.5, 3.0, 9.0], &[2.0, 3.0], 0.0, 1).unwrap();
        assert_eq!(out, [0.0, 0.0, 1.0, 1.0, 2.0]);
        assert!(threshold_labels(&[1.0], &[3.0, 2.0], 0.0, 1).is_err());
    }
}
