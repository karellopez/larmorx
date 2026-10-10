// SPDX-License-Identifier: Apache-2.0
//! `ThresholdImage` (ANTs v2.6.5, `Examples/ThresholdImage.cxx`): binary thresholds and Otsu
//! multi-thresholds, on `float` images.
//!
//! - [`threshold_image`] with [`ThresholdMode::Range`]: ITK's `BinaryThresholdImageFilter`
//!   (`inside` where `lower ≤ v ≤ upper`, else `outside`).
//! - [`ThresholdMode::Otsu`] without a mask: ITK's `OtsuMultipleThresholdsImageFilter` (128
//!   bins); voxels are labelled `0..=n`.
//! - [`ThresholdMode::Otsu`] with a mask: ANTs' own code. A 200-bin histogram of the voxels
//!   whose mask value is exactly 1 (`LabelStatisticsImageFilter`), Otsu thresholds on it,
//!   then each masked voxel gets `i + 1` for the first threshold it is below (compared in
//!   float), or `n + 1`; voxels outside the mask are 0. The histogram range comes from a loop
//!   that starts the maximum at `FLT_MIN` (the smallest positive float) and updates it only
//!   when a voxel does not lower the minimum.
//! - `Kmeans` is not supported yet.

use larmorx_core::parallel;
use larmorx_image::FilterError;
use larmorx_image::statistics::label_histogram;
use larmorx_image::threshold::{
    binary_threshold, check_otsu, otsu_multiple_thresholds, otsu_thresholds,
};
use rayon::prelude::*;

const CHUNK: usize = 1 << 16;

/// Bins of the masked Otsu histogram (`numberOfBins` in `OtsuThreshold`).
pub const MASKED_OTSU_BINS: usize = 200;
/// Bins of ITK's `OtsuMultipleThresholdsImageFilter` (its default).
pub const OTSU_BINS: usize = 128;

/// What [`threshold_image`] does.
#[derive(Clone, Copy, Debug)]
pub enum ThresholdMode<'a> {
    /// `ThresholdImage d in out lower upper [inside=1] [outside=0]`.
    Range {
        lower: f32,
        upper: f32,
        inside: f32,
        outside: f32,
    },
    /// `ThresholdImage d in out Otsu n [mask]`: the mask is read as `int`; label 1 is
    /// the region.
    Otsu {
        thresholds: usize,
        mask: Option<&'a [i32]>,
    },
}

/// What [`threshold_image`] returns.
#[derive(Clone, Debug, PartialEq)]
pub struct ThresholdOutput {
    pub data: Vec<f32>,
    /// The Otsu thresholds (empty for a range threshold).
    pub thresholds: Vec<f64>,
}

/// `ThresholdImage` on the voxels of a `float` image.
pub fn threshold_image(
    data: &[f32],
    mode: ThresholdMode<'_>,
    n_threads: usize,
) -> Result<ThresholdOutput, FilterError> {
    match mode {
        ThresholdMode::Range {
            lower,
            upper,
            inside,
            outside,
        } => Ok(ThresholdOutput {
            data: binary_threshold(data, lower, upper, inside, outside, n_threads)?,
            thresholds: Vec::new(),
        }),
        ThresholdMode::Otsu {
            thresholds,
            mask: None,
        } => {
            let r = otsu_multiple_thresholds(data, thresholds, OTSU_BINS, n_threads)?;
            Ok(ThresholdOutput {
                data: r.labels,
                thresholds: r.thresholds,
            })
        }
        ThresholdMode::Otsu {
            thresholds: n,
            mask: Some(mask),
        } => masked_otsu(data, mask, n, n_threads),
    }
}

/// ANTs' `OtsuThreshold` with a mask (`ThresholdImage.cxx`).
fn masked_otsu(
    data: &[f32],
    mask: &[i32],
    n: usize,
    n_threads: usize,
) -> Result<ThresholdOutput, FilterError> {
    if mask.len() != data.len() {
        return Err(FilterError::SizeMismatch {
            what: "mask",
            actual: mask.len(),
            expected: data.len(),
        });
    }
    check_otsu(n, MASKED_OTSU_BINS)?;
    let (mut min, mut max) = (f32::MAX, f32::MIN_POSITIVE);
    for (&v, &m) in data.iter().zip(mask) {
        if m == 1 {
            if v < min {
                min = v;
            } else if v > max {
                max = v;
            }
        }
    }
    let histogram = label_histogram(
        data,
        mask,
        1,
        MASKED_OTSU_BINS,
        f64::from(min),
        f64::from(max),
        n_threads,
    )?
    .ok_or_else(|| {
        FilterError::Invalid(
            "Otsu with a mask: no voxel has mask value 1 (ANTs crashes here)".into(),
        )
    })?;
    let thresholds = otsu_thresholds(&histogram, n, false, false);
    let stored: Vec<f32> = thresholds.iter().map(|&t| t as f32).collect();
    let labels = parallel::with_threads(n_threads, || {
        data.par_iter()
            .zip(mask.par_iter())
            .with_min_len(CHUNK)
            .map(|(&v, &m)| {
                if m != 1 {
                    return 0.0;
                }
                stored
                    .iter()
                    .position(|&t| v < t)
                    .map_or(stored.len() as f32 + 1.0, |i| i as f32 + 1.0)
            })
            .collect()
    })?;
    Ok(ThresholdOutput {
        data: labels,
        thresholds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_and_otsu() {
        let mut data: Vec<f32> = (0..400).map(|i| 10.0 + (i % 9) as f32).collect();
        data.extend((0..400).map(|i| 80.0 + (i % 13) as f32));
        let r = threshold_image(
            &data,
            ThresholdMode::Range {
                lower: 50.0,
                upper: 1e9,
                inside: 1.0,
                outside: 0.0,
            },
            2,
        )
        .unwrap();
        assert_eq!(r.data.iter().sum::<f32>(), 400.0);
        let o = threshold_image(
            &data,
            ThresholdMode::Otsu {
                thresholds: 1,
                mask: None,
            },
            2,
        )
        .unwrap();
        assert_eq!(o.data.iter().sum::<f32>(), 400.0);
        let mask: Vec<i32> = (0..800).map(|i| i32::from(i % 2 == 0)).collect();
        let m = threshold_image(
            &data,
            ThresholdMode::Otsu {
                thresholds: 1,
                mask: Some(&mask),
            },
            3,
        )
        .unwrap();
        assert!(m.data.iter().step_by(2).all(|&l| l == 1.0 || l == 2.0));
        assert!(m.data.iter().skip(1).step_by(2).all(|&l| l == 0.0));
        assert_eq!(m.data[0], 1.0);
        assert_eq!(m.data[798], 2.0);
    }
}
