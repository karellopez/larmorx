// SPDX-License-Identifier: Apache-2.0
//! Histograms and image statistics, as ITK v5.4.5 computes them:
//! - [`Histogram`], `itk::Statistics::Histogram<double>` in one dimension (bin bounds, the bin
//!   search, frequencies, `Quantile`);
//! - [`scalar_image_histogram`], `ScalarImageToHistogramGenerator` (automatic bounds with
//!   ITK's margin, as `OtsuMultipleThresholdsImageFilter` builds its histogram);
//! - [`label_histogram`], the per-label histogram of `LabelStatisticsImageFilter`;
//! - [`minimum_maximum`], `MinimumMaximumImageCalculator`.
//!
//! Frequencies are integer counts, so the histograms are built in parallel and do not depend
//! on the thread count.

use larmorx_core::parallel;
use rayon::prelude::*;

use crate::FilterError;

/// Voxels per parallel task when counting.
const CHUNK: usize = 1 << 16;

/// A one-dimensional `itk::Statistics::Histogram<double>`: bins with explicit bounds and
/// integer frequencies.
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    /// Lower bound of each bin.
    pub bin_min: Vec<f64>,
    /// Upper bound of each bin.
    pub bin_max: Vec<f64>,
    /// Count of each bin.
    pub frequency: Vec<u64>,
    /// ITK's `ClipBinsAtEnds` (on by default): values outside the bins are not counted. Off,
    /// they go to the first or last bin.
    pub clip_bins_at_ends: bool,
}

impl Histogram {
    /// `Histogram::Initialize(size, lower, upper)`: `size` bins of equal width between `lower`
    /// and `upper`. As in ITK, the width is computed in single precision
    /// (`(float(upper) − float(lower)) / float(size)`), each bound is
    /// `lower + float(j · width)` in double, and the last bin ends exactly at `upper`.
    pub fn new(size: usize, lower: f64, upper: f64) -> Histogram {
        let mut bin_min = vec![0.0; size];
        let mut bin_max = vec![0.0; size];
        if size > 0 {
            let interval = ((upper as f32) - (lower as f32)) / (size as f32);
            for j in 0..size - 1 {
                bin_min[j] = lower + f64::from(j as f32 * interval);
                bin_max[j] = lower + f64::from((j as f32 + 1.0) * interval);
            }
            bin_min[size - 1] = lower + f64::from((size as f32 - 1.0) * interval);
            bin_max[size - 1] = upper;
        }
        Histogram {
            bin_min,
            bin_max,
            frequency: vec![0; size],
            clip_bins_at_ends: true,
        }
    }

    pub fn len(&self) -> usize {
        self.frequency.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frequency.is_empty()
    }

    /// `Histogram::GetIndex`: the bin of `value`, or `None` when it falls outside the bins
    /// and they are clipped. ITK's binary search is reproduced step by step, including the
    /// last bin taking values equal (within `AlmostEquals`) to its upper bound.
    pub fn index(&self, value: f64) -> Option<usize> {
        let n = self.len();
        if n == 0 {
            return None;
        }
        if value < self.bin_min[0] {
            return if self.clip_bins_at_ends {
                None
            } else {
                Some(0)
            };
        }
        let mut end = n as i64 - 1;
        if value >= self.bin_max[end as usize] {
            return if !self.clip_bins_at_ends
                || crate::itk_math::almost_equal_f64(value, self.bin_max[end as usize])
            {
                Some(n - 1)
            } else {
                None
            };
        }
        let mut begin: i64 = 0;
        let mut mid = (end + 1) / 2;
        let mut median = self.bin_min[mid as usize];
        loop {
            if value < median {
                end = mid - 1;
            } else if value > median {
                let m = mid as usize;
                if value < self.bin_max[m] && value >= self.bin_min[m] {
                    return Some(m);
                }
                begin = mid + 1;
            } else {
                return Some(mid as usize);
            }
            mid = begin + (end - begin) / 2;
            // Out of range only for bins with gaps, which `new` never makes (ITK would read
            // past the end). NaN returns the middle bin above, as in ITK.
            median = *self.bin_min.get(usize::try_from(mid).ok()?)?;
        }
    }

    /// Adds `count` to the bin of `value` (`IncreaseFrequencyOfMeasurement`); values outside
    /// clipped bins are dropped. Returns whether the value was counted.
    pub fn add(&mut self, value: f64, count: u64) -> bool {
        match self.index(value) {
            Some(i) => {
                self.frequency[i] += count;
                true
            }
            None => false,
        }
    }

    /// `GetTotalFrequency`.
    pub fn total_frequency(&self) -> u64 {
        self.frequency.iter().sum()
    }

    /// `GetMeasurement(n, 0)` (and `GetMeasurementVector`): the middle of bin `n`,
    /// `(min + max) / 2`.
    pub fn measurement(&self, n: usize) -> f64 {
        (self.bin_min[n] + self.bin_max[n]) / 2.0
    }

    /// `Histogram::Quantile(0, p)`: walks the cumulative frequencies from the low end (for
    /// `p < 0.5`) or the high end, and interpolates linearly within the bin where the
    /// cumulative proportion reaches `p`.
    pub fn quantile(&self, p: f64) -> f64 {
        let size = self.len();
        let total = self.total_frequency() as f64;
        let mut cumulated = 0.0f64;
        let mut f_n;
        let mut p_n_prev;
        if p < 0.5 {
            let mut n = 0usize;
            let mut p_n = 0.0;
            loop {
                f_n = self.frequency[n] as f64;
                cumulated += f_n;
                p_n_prev = p_n;
                p_n = cumulated / total;
                n += 1;
                if !(n < size && p_n < p) {
                    break;
                }
            }
            let proportion = f_n / total;
            let (min, max) = (self.bin_min[n - 1], self.bin_max[n - 1]);
            min + ((p - p_n_prev) / proportion) * (max - min)
        } else {
            let mut n = size as i64 - 1;
            let mut m = 0usize;
            let mut p_n = 1.0;
            loop {
                f_n = self.frequency[n as usize] as f64;
                cumulated += f_n;
                p_n_prev = p_n;
                p_n = 1.0 - cumulated / total;
                n -= 1;
                m += 1;
                if !(m < size && p_n > p) {
                    break;
                }
            }
            let proportion = f_n / total;
            let i = (n + 1) as usize;
            let (min, max) = (self.bin_min[i], self.bin_max[i]);
            max - ((p_n_prev - p) / proportion) * (max - min)
        }
    }

    /// Adds the counts of `other` (same bins).
    fn merge(mut self, other: Histogram) -> Histogram {
        for (a, b) in self.frequency.iter_mut().zip(other.frequency) {
            *a += b;
        }
        self
    }
}

/// `MinimumMaximumImageCalculator`: the smallest and largest value, starting from
/// `(f32::MAX, −f32::MAX)` and comparing with `<` and `>` (so NaN is skipped, and an image of
/// NaN gives the starting values).
pub fn minimum_maximum(data: &[f32], n_threads: usize) -> Result<(f32, f32), FilterError> {
    let fold = |acc: (f32, f32), &v: &f32| {
        let (mut min, mut max) = acc;
        if v > max {
            max = v;
        }
        if v < min {
            min = v;
        }
        (min, max)
    };
    let init = (f32::MAX, -f32::MAX);
    Ok(parallel::with_threads(n_threads, || {
        data.par_chunks(CHUNK)
            .map(|c| c.iter().fold(init, fold))
            .reduce(
                || init,
                |a, b| {
                    let min = if b.0 < a.0 { b.0 } else { a.0 };
                    let max = if b.1 > a.1 { b.1 } else { a.1 };
                    (min, max)
                },
            )
    })?)
}

/// `ScalarImageToHistogramGenerator` with `bins` bins and ITK's defaults (automatic bounds,
/// marginal scale 100): the bounds are the sample's minimum and maximum (found as
/// `Algorithm::FindSampleBound` does, from the first value), the upper one raised by
/// `(max − min) / bins / 100` so the maximum falls inside the last bin. Bins are clipped at
/// the ends unless that margin would overflow.
pub fn scalar_image_histogram(
    data: &[f32],
    bins: usize,
    n_threads: usize,
) -> Result<Histogram, FilterError> {
    const MARGINAL_SCALE: f64 = 100.0;
    if data.is_empty() {
        return Ok(Histogram::new(bins, 0.0, 0.0));
    }
    // FindSampleBound: min = max = first, then `<` else `>` (order-independent for
    // non-NaN values, so it can run in parallel).
    let first = data[0];
    let (lower, upper) = parallel::with_threads(n_threads, || {
        data.par_chunks(CHUNK)
            .map(|c| {
                c.iter().fold((first, first), |(lo, hi), &v| {
                    if v < lo {
                        (v, hi)
                    } else if v > hi {
                        (lo, v)
                    } else {
                        (lo, hi)
                    }
                })
            })
            .reduce(
                || (first, first),
                |a, b| {
                    let lo = if b.0 < a.0 { b.0 } else { a.0 };
                    let hi = if b.1 > a.1 { b.1 } else { a.1 };
                    (lo, hi)
                },
            )
    })?;
    let margin = (f64::from(upper - lower) / bins as f64) / MARGINAL_SCALE;
    let (h_upper, clip) = if f64::MAX - f64::from(upper) > margin {
        (f64::from(upper) + margin, true)
    } else {
        (f64::from(upper), false)
    };
    let mut template = Histogram::new(bins, f64::from(lower), h_upper);
    template.clip_bins_at_ends = clip;
    count(data, &template, n_threads, |_| true)
}

/// The histogram `LabelStatisticsImageFilter` keeps for `label` (`UseHistograms` on,
/// `SetHistogramParameters(bins, lower, upper)`): the values of the voxels with that label,
/// counted into `bins` bins between `lower` and `upper`, clipped at the ends. `None` if no
/// voxel has the label (ITK then has no histogram for it).
pub fn label_histogram(
    data: &[f32],
    labels: &[i32],
    label: i32,
    bins: usize,
    lower: f64,
    upper: f64,
    n_threads: usize,
) -> Result<Option<Histogram>, FilterError> {
    if labels.len() != data.len() {
        return Err(FilterError::SizeMismatch {
            what: "label image",
            actual: labels.len(),
            expected: data.len(),
        });
    }
    let present = parallel::with_threads(n_threads, || labels.par_iter().any(|&l| l == label))?;
    if !present {
        return Ok(None);
    }
    let template = Histogram::new(bins, lower, upper);
    let h = count_labelled(data, labels, label, &template, n_threads)?;
    Ok(Some(h))
}

/// Counts every value of `data` for which `keep(index)` holds into a copy of `template`.
fn count(
    data: &[f32],
    template: &Histogram,
    n_threads: usize,
    keep: impl Fn(usize) -> bool + Sync,
) -> Result<Histogram, FilterError> {
    Ok(parallel::with_threads(n_threads, || {
        data.par_chunks(CHUNK)
            .enumerate()
            .map(|(c, chunk)| {
                let mut h = template.clone();
                for (k, &v) in chunk.iter().enumerate() {
                    if keep(c * CHUNK + k) {
                        h.add(f64::from(v), 1);
                    }
                }
                h
            })
            .reduce(|| template.clone(), Histogram::merge)
    })?)
}

fn count_labelled(
    data: &[f32],
    labels: &[i32],
    label: i32,
    template: &Histogram,
    n_threads: usize,
) -> Result<Histogram, FilterError> {
    count(data, template, n_threads, |i| labels[i] == label)
}

/// Minimum and maximum of the voxels with `label`, as `LabelStatisticsImageFilter` reports
/// them (`GetMinimum`, `GetMaximum`, in double), or ITK's defaults for a missing label
/// (`f32::MAX` and `−f32::MAX`).
pub fn label_minimum_maximum(
    data: &[f32],
    labels: &[i32],
    label: i32,
    n_threads: usize,
) -> Result<(f64, f64), FilterError> {
    if labels.len() != data.len() {
        return Err(FilterError::SizeMismatch {
            what: "label image",
            actual: labels.len(),
            expected: data.len(),
        });
    }
    let init = (f64::MAX, -f64::MAX, false);
    let (min, max, found) = parallel::with_threads(n_threads, || {
        data.par_chunks(CHUNK)
            .zip(labels.par_chunks(CHUNK))
            .map(|(d, l)| {
                d.iter()
                    .zip(l)
                    .filter(|(_, l)| **l == label)
                    .fold(init, |(lo, hi, _), (&v, _)| {
                        let v = f64::from(v);
                        (
                            if v < lo { v } else { lo },
                            if v > hi { v } else { hi },
                            true,
                        )
                    })
            })
            .reduce(
                || init,
                |a, b| {
                    (
                        if b.0 < a.0 { b.0 } else { a.0 },
                        if b.1 > a.1 { b.1 } else { a.1 },
                        a.2 || b.2,
                    )
                },
            )
    })?;
    Ok(if found {
        (min, max)
    } else {
        (f64::from(f32::MAX), -f64::from(f32::MAX))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bins_follow_itk_initialize() {
        let h = Histogram::new(4, 0.0, 1.0);
        assert_eq!(h.bin_min, [0.0, 0.25, 0.5, 0.75]);
        assert_eq!(h.bin_max, [0.25, 0.5, 0.75, 1.0]);
        // The width is single precision: 0.1 is not exact in float.
        let h = Histogram::new(3, 0.0, 0.3);
        assert_eq!(h.bin_max[0], f64::from(0.3f32 / 3.0));
    }

    #[test]
    fn index_and_clipping() {
        let mut h = Histogram::new(4, 0.0, 1.0);
        assert_eq!(h.index(-0.1), None);
        assert_eq!(h.index(0.0), Some(0));
        assert_eq!(h.index(0.25), Some(1));
        assert_eq!(h.index(0.6), Some(2));
        assert_eq!(h.index(1.0), Some(3)); // the upper bound belongs to the last bin
        assert_eq!(h.index(1.1), None);
        h.clip_bins_at_ends = false;
        assert_eq!(h.index(-5.0), Some(0));
        assert_eq!(h.index(5.0), Some(3));
    }

    #[test]
    fn quantiles_interpolate_within_bins() {
        let mut h = Histogram::new(4, 0.0, 4.0);
        h.frequency = vec![1, 1, 1, 1];
        assert_eq!(h.quantile(0.25), 1.0);
        assert_eq!(h.quantile(0.125), 0.5);
        assert_eq!(h.quantile(0.75), 3.0);
        assert_eq!(h.quantile(0.875), 3.5);
    }

    #[test]
    fn histograms_do_not_depend_on_threads() {
        let data: Vec<f32> = (0..300_000u64)
            .map(|i| ((i * 7919) % 1000) as f32 * 0.37)
            .collect();
        let a = scalar_image_histogram(&data, 128, 1).unwrap();
        let b = scalar_image_histogram(&data, 128, 7).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.total_frequency(), data.len() as u64);
        let labels: Vec<i32> = (0..data.len()).map(|i| (i % 3) as i32).collect();
        let c = label_histogram(&data, &labels, 1, 64, 0.0, 400.0, 3)
            .unwrap()
            .unwrap();
        assert_eq!(c.total_frequency(), 100_000);
        assert!(
            label_histogram(&data, &labels, 9, 64, 0.0, 1.0, 1)
                .unwrap()
                .is_none()
        );
        assert_eq!(minimum_maximum(&data, 4).unwrap(), (0.0, 999.0 * 0.37));
    }
}
