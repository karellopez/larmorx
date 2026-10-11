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
//!   when a voxel does not lower the minimum (defect ANTS-25): reproduced with
//!   [`UpstreamDefects::Reproduce`]; corrected (the default), the range is the masked voxels'
//!   true minimum and maximum.
//! - `Kmeans` is not supported yet.

use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};
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
    /// The upstream defects that changed this result (or, reproduced, would have).
    pub notices: Vec<Notice>,
}

/// `ThresholdImage` on the voxels of a `float` image, with ANTs' defects corrected or
/// reproduced as `defects_mode` says (see the module).
pub fn threshold_image(
    data: &[f32],
    mode: ThresholdMode<'_>,
    n_threads: usize,
    defects_mode: UpstreamDefects,
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
            notices: Vec::new(),
        }),
        ThresholdMode::Otsu {
            thresholds,
            mask: None,
        } => {
            let r = otsu_multiple_thresholds(data, thresholds, OTSU_BINS, n_threads)?;
            Ok(ThresholdOutput {
                data: r.labels,
                thresholds: r.thresholds,
                notices: Vec::new(),
            })
        }
        ThresholdMode::Otsu {
            thresholds: n,
            mask: Some(mask),
        } => masked_otsu(data, mask, n, n_threads, defects_mode),
    }
}

/// ANTs' `OtsuThreshold` with a mask (`ThresholdImage.cxx`).
fn masked_otsu(
    data: &[f32],
    mask: &[i32],
    n: usize,
    n_threads: usize,
    mode: UpstreamDefects,
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
    let (mut true_min, mut true_max) = (f32::INFINITY, f32::NEG_INFINITY);
    for (&v, &m) in data.iter().zip(mask) {
        if m == 1 {
            if v < min {
                min = v;
            } else if v > max {
                max = v;
            }
            true_min = true_min.min(v);
            true_max = true_max.max(v);
        }
    }
    let mut notices = Notices::new(mode);
    if true_min <= true_max && (min, max) != (true_min, true_max) {
        notices.add(
            &defects::ANTS_25,
            format!(
                "the histogram's range is [{true_min}, {true_max}]; ANTs finds [{min}, {max}]{}",
                if mode.fix() { "" } else { ", and uses it" }
            ),
        );
        if mode.fix() {
            (min, max) = (true_min, true_max);
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
        notices: notices.into_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masked_otsu_range() {
        // ANTS-25: the first masked voxel is the smallest, so ANTs never compares it with the
        // maximum... and the maximum starts at FLT_MIN: with all values negative it stays there.
        let data = [-5.0f32, -4.0, -3.0, -2.0, -1.0, -4.5];
        let mask = [1i32; 6];
        let otsu = |mode| {
            threshold_image(
                &data,
                ThresholdMode::Otsu {
                    thresholds: 1,
                    mask: Some(&mask),
                },
                1,
                mode,
            )
            .unwrap()
        };
        let (f, r) = (otsu(UpstreamDefects::Fix), otsu(UpstreamDefects::Reproduce));
        assert_eq!(f.notices[0].defect.id, "ANTS-25");
        assert_eq!(r.notices[0].defect.id, "ANTS-25");
        assert_ne!(f.thresholds, r.thresholds);
        assert!(
            f.thresholds[0] > -5.0 && f.thresholds[0] < -1.0,
            "{:?}",
            f.thresholds
        );
    }

    #[test]
    fn range_and_otsu() {
        let mode = UpstreamDefects::Fix;
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
            mode,
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
            mode,
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
            mode,
        )
        .unwrap();
        assert!(m.data.iter().step_by(2).all(|&l| l == 1.0 || l == 2.0));
        assert!(m.data.iter().skip(1).step_by(2).all(|&l| l == 0.0));
        assert_eq!(m.data[0], 1.0);
        assert_eq!(m.data[798], 2.0);
    }
}
