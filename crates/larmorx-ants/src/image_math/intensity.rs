// SPDX-License-Identifier: Apache-2.0
//! ImageMath's intensity operations (`Examples/ImageMath_Templates.hxx`, ANTs v2.6.5):
//! `TruncateImageIntensity`, `Normalize`, `RescaleImage`, `ThresholdAtMean` and
//! `ReplaceVoxelValue`.

use larmorx_core::parallel;
use larmorx_image::FilterError;
use larmorx_image::intensity::rescale_intensity;
use larmorx_image::itk_math::almost_equal_f32;
use larmorx_image::statistics::label_histogram;
use rayon::prelude::*;

const CHUNK: usize = 1 << 16;

/// The parameters of [`truncate_image_intensity`] (ANTs' defaults are in `Default`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TruncateOptions {
    /// The lower quantile (`lowerQuantile`, default 0.025).
    pub lower_quantile: f32,
    /// The upper quantile (`upperQuantile`; default `1 − lower_quantile`, so 0.975).
    pub upper_quantile: f32,
    /// Histogram bins (`numberOfBins`, default 64).
    pub bins: usize,
}

impl Default for TruncateOptions {
    fn default() -> Self {
        TruncateOptions {
            lower_quantile: 0.025,
            upper_quantile: 1.0 - 0.025f32,
            bins: 64,
        }
    }
}

/// What [`truncate_image_intensity`] returns.
#[derive(Clone, Debug, PartialEq)]
pub struct Truncated {
    pub data: Vec<f32>,
    /// The intensities the voxels were clipped to.
    pub lower: f32,
    pub upper: f32,
}

/// `TruncateImageIntensity`: clips every voxel to the `lower_quantile` and `upper_quantile`
/// of a histogram of the positive, finite voxels inside `mask` (all voxels without one).
///
/// As ANTs computes it:
/// - the voxels counted are those with a value `> 0` where the mask is `≥ 0.5` (the mask is
///   read as `int`, so `≥ 1`), and not NaN or infinite;
/// - the histogram range is found with `if (v < min) min = v; else if (v > max) max = v;`
///   from `min = FLT_MAX`, `max = −FLT_MAX`, before the infinite values are excluded: a
///   voxel that lowers the minimum is never compared with the maximum (so if the first such
///   voxel is the brightest, the maximum is the next brightest, and the brightest falls
///   outside the histogram), and `+inf` becomes the maximum;
/// - `bins` bins between those bounds (`LabelStatisticsImageFilter`), and
///   `Histogram::Quantile` for both quantiles, in double, rounded to float;
/// - every voxel, inside the mask or not, is clipped to `[lower, upper]` (NaN stays).
///
/// Without a counted voxel ANTs crashes (it has no histogram for label 1); this is an error.
pub fn truncate_image_intensity(
    data: &[f32],
    mask: Option<&[i32]>,
    options: &TruncateOptions,
    n_threads: usize,
) -> Result<Truncated, FilterError> {
    if let Some(m) = mask
        && m.len() != data.len()
    {
        return Err(FilterError::SizeMismatch {
            what: "mask",
            actual: m.len(),
            expected: data.len(),
        });
    }
    if options.bins == 0 {
        return Err(FilterError::Invalid(
            "TruncateImageIntensity needs at least one histogram bin".into(),
        ));
    }
    let mut labels = vec![0i32; data.len()];
    let (mut min, mut max) = (f32::MAX, -f32::MAX);
    for (i, &v) in data.iter().enumerate() {
        let m = mask.map_or(1, |m| m[i]);
        if v > 0.0 && f64::from(m) >= 0.5 {
            if v < min {
                min = v;
            } else if v > max {
                max = v;
            }
            labels[i] = 1;
        }
        if v.is_nan() || v.is_infinite() {
            labels[i] = 0;
        }
    }
    let histogram = label_histogram(
        data,
        &labels,
        1,
        options.bins,
        f64::from(min),
        f64::from(max),
        n_threads,
    )?
    .ok_or_else(|| {
        FilterError::Invalid(
            "TruncateImageIntensity: no positive, finite voxel in the mask (ANTs crashes here)"
                .into(),
        )
    })?;
    let lower = histogram.quantile(f64::from(options.lower_quantile)) as f32;
    let upper = histogram.quantile(f64::from(options.upper_quantile)) as f32;
    let data = parallel::with_threads(n_threads, || {
        data.par_iter()
            .with_min_len(CHUNK)
            .map(|&v| {
                let mut v = v;
                if v < lower {
                    v = lower;
                }
                if v > upper {
                    v = upper;
                }
                v
            })
            .collect()
    })?;
    Ok(Truncated { data, lower, upper })
}

/// How [`normalize`] scales the image.
#[derive(Clone, Copy, Debug)]
pub enum Normalization<'a> {
    /// `Normalize img` (or an option that reads as 0): `(v − min) / (max − min)`, with the
    /// range found from `max = 0`, `min = 1e9` (so an image with no positive voxel keeps
    /// `max = 0`, and one above 1e9 keeps `min = 1e9`).
    Range,
    /// `Normalize img <non-zero number>`: divide by the mean of all voxels.
    Mean,
    /// `Normalize img mask`: divide by the mean of the voxels where the mask (read as float)
    /// is not zero (within `FloatAlmostEqual`).
    MaskMean(&'a [f32]),
}

/// `NormalizeImage` (`Normalize`). Sums are accumulated in float, voxel by voxel in image
/// order, as ANTs does (the count of the masked mean is a float too, exact up to 2^24
/// voxels).
pub fn normalize(
    data: &[f32],
    mode: Normalization<'_>,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let map = |f: &(dyn Fn(f32) -> f32 + Sync)| -> Result<Vec<f32>, FilterError> {
        Ok(parallel::with_threads(n_threads, || {
            data.par_iter().with_min_len(CHUNK).map(|&v| f(v)).collect()
        })?)
    };
    match mode {
        Normalization::MaskMean(mask) => {
            if mask.len() != data.len() {
                return Err(FilterError::SizeMismatch {
                    what: "mask",
                    actual: mask.len(),
                    expected: data.len(),
                });
            }
            let (mut sum, mut count) = (0.0f32, 0.0f32);
            for (&m, &v) in mask.iter().zip(data) {
                if !almost_equal_f32(m, 0.0) {
                    sum += v;
                    count += 1.0;
                }
            }
            let mean = sum / count;
            map(&|v| v / mean)
        }
        Normalization::Range | Normalization::Mean => {
            let (mut max, mut min, mut mean) = (0.0f32, 1.0e9f32, 0.0f32);
            for &v in data {
                mean += v;
                if v > max {
                    max = v;
                }
                if v < min {
                    min = v;
                }
            }
            mean /= data.len() as f32;
            match mode {
                Normalization::Range => map(&|v| (v - min) / (max - min)),
                _ => map(&|v| v / mean),
            }
        }
    }
}

/// `RescaleImage`: ITK's `RescaleIntensityImageFilter` onto `[min, max]`
/// ([`larmorx_image::intensity::rescale_intensity`]).
pub fn rescale(
    data: &[f32],
    min: f32,
    max: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    rescale_intensity(data, min, max, n_threads)
}

/// `ThresholdAtMean`: `ImageMath d out ThresholdAtMean image [fraction=1]`
/// (`ImageMath_Templates.hxx:794-847`): 1 where `mean · fraction ≤ v ≤ max`, else 0
/// (`BinaryThresholdImageFilter`).
///
/// The mean is a float sum in image order divided by the voxel count (as float); the maximum
/// starts at `-1e9` (an image whose voxels are all at or below it keeps that). A lower bound
/// above the maximum (a fraction above `max / mean`) makes ITK throw, and ANTs abort.
pub fn threshold_at_mean(
    data: &[f32],
    fraction: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let mut mean = 0.0f32;
    let mut max = -1.0e9f32;
    let mut min = 1.0e9f32;
    for &v in data {
        mean += v;
        if v > max {
            max = v;
        } else if v < min {
            min = v;
        }
    }
    if !data.is_empty() {
        mean /= data.len() as f32;
    }
    larmorx_image::threshold::binary_threshold(data, mean * fraction, max, 1.0f32, 0.0, n_threads)
}

/// `ReplaceVoxelValue`: `ImageMath d out ReplaceVoxelValue image low high value`
/// (`ImageMath_Templates.hxx:9549-9586`): voxels with `low ≤ v ≤ high` become `value`, the
/// others keep theirs.
pub fn replace_voxel_value(
    data: &[f32],
    low: f32,
    high: f32,
    value: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    Ok(parallel::with_threads(n_threads, || {
        data.par_iter()
            .with_min_len(CHUNK)
            .map(|&v| if v >= low && v <= high { value } else { v })
            .collect()
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_at_mean_and_replace() {
        let data = [1.0f32, 2.0, 3.0, 6.0];
        assert_eq!(
            threshold_at_mean(&data, 1.0, 1).unwrap(),
            vec![0., 0., 1., 1.]
        );
        assert_eq!(
            threshold_at_mean(&data, 0.5, 2).unwrap(),
            vec![0., 1., 1., 1.]
        );
        assert!(threshold_at_mean(&data, 3.0, 1).is_err());
        assert_eq!(
            replace_voxel_value(&data, 2.0, 3.0, -1.0, 1).unwrap(),
            vec![1., -1., -1., 6.]
        );
    }

    #[test]
    fn truncation_clips_to_quantiles() {
        let data: Vec<f32> = (1..=1000).map(|i| i as f32).collect();
        let t = truncate_image_intensity(&data, None, &TruncateOptions::default(), 3).unwrap();
        assert!(t.lower > 1.0 && t.lower < 50.0, "{t:?}");
        assert!(t.upper > 950.0 && t.upper < 1000.0);
        assert_eq!(t.data[0], t.lower);
        assert_eq!(t.data[999], t.upper);
        assert_eq!(t.data[500], 501.0);
        // No positive voxel: an error (ANTs crashes).
        assert!(
            truncate_image_intensity(&[0.0, -1.0], None, &TruncateOptions::default(), 1).is_err()
        );
    }

    #[test]
    fn normalization_modes() {
        let d = [1.0f32, 2.0, 3.0, 4.0];
        assert_eq!(
            normalize(&d, Normalization::Range, 1).unwrap(),
            [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]
        );
        assert_eq!(
            normalize(&d, Normalization::Mean, 1).unwrap(),
            [0.4, 0.8, 1.2, 1.6]
        );
        let mask = [0.0f32, 1.0, 1.0, 0.0];
        assert_eq!(
            normalize(&d, Normalization::MaskMean(&mask), 1).unwrap(),
            [0.4, 0.8, 1.2, 1.6]
        );
        // All negative: max stays 0.
        let out = normalize(&[-4.0, -2.0], Normalization::Range, 1).unwrap();
        assert_eq!(out, [0.0, 0.5]);
    }
}
