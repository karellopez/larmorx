// SPDX-License-Identifier: Apache-2.0
//! ANTs' `PreprocessImage` (`Examples/itkantsRegistrationHelper.hxx`, ANTs v2.6.5): each
//! metric image's intensities are windowed to `[0, 1]` between two quantiles of its
//! histogram, and the moving image's are optionally matched to the fixed image's histogram.
//!
//! - `ImageToHistogramFilter` (ITK v5.4.5, `itkImageToHistogramFilter.hxx`): 256 bins of
//!   `Histogram<double>` between the image's minimum and its maximum plus a margin of a
//!   tenth of a bin (`MarginalScale` 10); `Histogram::Quantile`. ANTs stores each quantile
//!   as `float`.
//! - `IntensityWindowingImageFilter` (`itkIntensityWindowingImageFilter.hxx`).
//! - `HistogramMatchingImageFilter` (`itkHistogramMatchingImageFilter.hxx`): 256 levels, 12
//!   match points, thresholds at the mean intensity, its histograms of the pixel type `R`.

use larmorx_image::statistics::Histogram;

use crate::Real;

/// `ImageToHistogramFilter` with automatic bounds, `bins` bins and `MarginalScale`
/// `marginal_scale`, as ANTs builds it.
pub fn image_histogram<R: Real>(data: &[R], bins: usize, marginal_scale: f64) -> Histogram {
    let mut min = R::MAX.to_f64();
    let mut max = (-R::MAX).to_f64();
    for &v in data {
        let m = v.to_f64();
        // std::min(m, min) and std::max(m, max).
        min = if min < m { min } else { m };
        max = if m < max { max } else { m };
    }
    let margin = ((max - min) / bins as f64) / marginal_scale;
    let mut clip = true;
    if f64::MAX - max > margin {
        max += margin;
    } else {
        clip = false;
    }
    let mut h = Histogram::new(bins, min, max);
    h.clip_bins_at_ends = clip;
    for &v in data {
        h.add(v.to_f64(), 1);
    }
    h
}

/// `IntensityWindowingImageFilter`: values below `window_min` become `out_min`, above
/// `window_max` `out_max`, the others `x·scale + shift` in double.
pub fn window<R: Real>(data: &[R], window_min: R, window_max: R, out_min: R, out_max: R) -> Vec<R> {
    let scale = (out_max.to_f64() - out_min.to_f64()) / (window_max.to_f64() - window_min.to_f64());
    let shift = out_min.to_f64() - window_min.to_f64() * scale;
    data.iter()
        .map(|&x| {
            if x < window_min {
                out_min
            } else if x > window_max {
                out_max
            } else {
                R::from_f64(x.to_f64() * scale + shift)
            }
        })
        .collect()
}

/// `PreprocessImage`: the quantiles (as `float`), the window to `[0, 1]`, then histogram
/// matching to `reference` when given.
pub fn preprocess<R: Real>(
    data: &[R],
    lower_quantile: f32,
    upper_quantile: f32,
    reference: Option<&[R]>,
) -> Vec<R> {
    let h = image_histogram(data, 256, 10.0);
    let lower = h.quantile(f64::from(lower_quantile)) as f32;
    let upper = h.quantile(f64::from(upper_quantile)) as f32;
    let windowed = window(
        data,
        R::from_f64(f64::from(lower)),
        R::from_f64(f64::from(upper)),
        R::ZERO,
        R::ONE,
    );
    match reference {
        Some(r) => histogram_match(&windowed, r, 256, 12, true),
        None => windowed,
    }
}

/// `Histogram<R>` as `HistogramMatchingImageFilter` builds it: bins of the pixel type.
struct BinsR<R> {
    min: Vec<R>,
    max: Vec<R>,
    frequency: Vec<u64>,
}

impl<R: Real> BinsR<R> {
    /// `Initialize(size, lower, upper)` (the width in float), then the first bin's lower and
    /// last bin's upper bound set to the image's true range.
    fn new(size: usize, lower: R, upper: R, true_min: R, true_max: R) -> Self {
        let interval = (lower.to_f64() as f32, upper.to_f64() as f32);
        let interval = (interval.1 - interval.0) / size as f32;
        let mut min = vec![R::ZERO; size];
        let mut max = vec![R::ZERO; size];
        // `lowerBound + float(j)·interval` in the pixel type (a float bound plus the float
        // product, or a double bound plus it).
        let bound = |j: f32| lower + R::from_f64(f64::from(j * interval));
        for j in 0..size - 1 {
            min[j] = bound(j as f32);
            max[j] = bound(j as f32 + 1.0);
        }
        min[size - 1] = bound(size as f32 - 1.0);
        max[size - 1] = upper;
        min[0] = true_min;
        max[size - 1] = true_max;
        BinsR {
            min,
            max,
            frequency: vec![0; size],
        }
    }

    /// `GetIndex` (bins clipped at the ends).
    fn index(&self, value: R) -> Option<usize> {
        let n = self.frequency.len();
        if value < self.min[0] {
            return None;
        }
        let mut end = n as i64 - 1;
        if value >= self.max[end as usize] {
            return almost_equal(value, self.max[end as usize]).then_some(n - 1);
        }
        let mut begin: i64 = 0;
        let mut mid = (end + 1) / 2;
        let mut median = self.min[mid as usize];
        loop {
            if value < median {
                end = mid - 1;
            } else if value > median {
                let m = mid as usize;
                if value < self.max[m] && value >= self.min[m] {
                    return Some(m);
                }
                begin = mid + 1;
            } else {
                return Some(mid as usize);
            }
            mid = begin + (end - begin) / 2;
            median = *self.min.get(usize::try_from(mid).ok()?)?;
        }
    }

    /// `Quantile(0, p)`.
    fn quantile(&self, p: f64) -> f64 {
        let h = Histogram {
            bin_min: self.min.iter().map(|v| v.to_f64()).collect(),
            bin_max: self.max.iter().map(|v| v.to_f64()).collect(),
            frequency: self.frequency.clone(),
            clip_bins_at_ends: true,
        };
        h.quantile(p)
    }
}

/// `Math::AlmostEquals` of two values of the pixel type (within 4 units in the last place,
/// as `FloatAlmostEqual`).
fn almost_equal<R: Real>(a: R, b: R) -> bool {
    match R::PRECISION {
        larmorx_core::Precision::Double => {
            larmorx_image::itk_math::almost_equal_f64(a.to_f64(), b.to_f64())
        }
        larmorx_core::Precision::Float => {
            larmorx_image::itk_math::almost_equal_f32(a.to_f64() as f32, b.to_f64() as f32)
        }
    }
}

/// `ComputeMinMaxMean`: the first value as minimum and maximum, the sum in double, the mean
/// stored in `R`.
fn min_max_mean<R: Real>(data: &[R]) -> (R, R, R) {
    let mut min = data[0];
    let mut max = data[0];
    let mut sum = 0.0f64;
    for &v in data {
        sum += v.to_f64();
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    (min, max, R::from_f64(sum / data.len() as f64))
}

/// `ConstructHistogramFromIntensityRange`: `levels` bins between `valid_min` and
/// `valid_max`, counting the values within that range.
fn intensity_histogram<R: Real>(
    data: &[R],
    levels: usize,
    valid_min: R,
    valid_max: R,
    true_min: R,
    true_max: R,
) -> BinsR<R> {
    let mut h = BinsR::new(levels, valid_min, valid_max, true_min, true_max);
    for &v in data {
        if v.to_f64() >= valid_min.to_f64()
            && v.to_f64() <= valid_max.to_f64()
            && let Some(i) = h.index(v)
        {
            h.frequency[i] += 1;
        }
    }
    h
}

/// `HistogramMatchingImageFilter` (reference histogram from the reference image).
pub fn histogram_match<R: Real>(
    source: &[R],
    reference: &[R],
    levels: usize,
    match_points: usize,
    threshold_at_mean: bool,
) -> Vec<R> {
    let (ref_min, ref_max, ref_mean) = min_max_mean(reference);
    let ref_threshold = if threshold_at_mean { ref_mean } else { ref_min };
    let ref_hist = intensity_histogram(reference, levels, ref_threshold, ref_max, ref_min, ref_max);
    let (src_min, src_max, src_mean) = min_max_mean(source);
    let src_threshold = if threshold_at_mean { src_mean } else { src_min };
    let src_hist = intensity_histogram(source, levels, src_threshold, src_max, src_min, src_max);
    let n = match_points;
    let mut table = vec![vec![0.0f64; n + 2]; 2];
    table[0][0] = src_threshold.to_f64();
    table[1][0] = ref_threshold.to_f64();
    table[0][n + 1] = src_max.to_f64();
    table[1][n + 1] = ref_max.to_f64();
    let delta = 1.0 / (n as f64 + 1.0);
    for j in 1..n + 1 {
        table[0][j] = src_hist.quantile(j as f64 * delta);
        table[1][j] = ref_hist.quantile(j as f64 * delta);
    }
    let not_zero = |d: f64| !larmorx_image::itk_math::almost_equal_f64(d, 0.0);
    let mut gradients = vec![0.0f64; n + 1];
    for j in 0..n + 1 {
        let denominator = table[0][j + 1] - table[0][j];
        if not_zero(denominator) {
            gradients[j] = table[1][j + 1] - table[1][j];
            gradients[j] /= denominator;
        }
    }
    let lower_gradient = {
        let denominator = table[0][0] - src_min.to_f64();
        if not_zero(denominator) {
            (table[1][0] - ref_min.to_f64()) / denominator
        } else {
            0.0
        }
    };
    let upper_gradient = {
        let denominator = table[0][n + 1] - src_max.to_f64();
        if not_zero(denominator) {
            (table[1][n + 1] - ref_max.to_f64()) / denominator
        } else {
            0.0
        }
    };
    source
        .iter()
        .map(|&v| {
            let x = v.to_f64();
            let mut j = 0;
            while j < n + 2 {
                if x < table[0][j] {
                    break;
                }
                j += 1;
            }
            let mapped = if j == 0 {
                ref_min.to_f64() + (x - src_min.to_f64()) * lower_gradient
            } else if j == n + 2 {
                ref_max.to_f64() + (x - src_max.to_f64()) * upper_gradient
            } else {
                table[1][j - 1] + (x - table[0][j - 1]) * gradients[j - 1]
            };
            R::from_f64(mapped)
        })
        .collect()
}
