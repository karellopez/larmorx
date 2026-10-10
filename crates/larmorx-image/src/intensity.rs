// SPDX-License-Identifier: Apache-2.0
//! Intensity mappings, as ITK v5.4.5 computes them:
//! - [`rescale_intensity`]: `RescaleIntensityImageFilter<float, float>`.

use larmorx_core::parallel;
use rayon::prelude::*;

use crate::FilterError;
use crate::itk_math::almost_equal_f32;
use crate::statistics::minimum_maximum;

const CHUNK: usize = 1 << 16;

/// `RescaleIntensityImageFilter`: maps the image's range linearly onto
/// `[out_min, out_max]`.
///
/// As in ITK: the input range comes from `MinimumMaximumImageCalculator` (NaN is skipped);
/// the scale is `(out_max − out_min) / (in_max − in_min)` in double, or
/// `(out_max − out_min) / in_max` when the range is a single value other than 0 (both within
/// `FloatAlmostEqual`), or else 0; the shift is `out_min − in_min · scale`; each voxel is
/// `float(v · scale + shift)`, clamped to `[out_min, out_max]` (NaN passes through).
/// `out_min > out_max` is an error with ITK's message.
pub fn rescale_intensity(
    input: &[f32],
    out_min: f32,
    out_max: f32,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    if out_min > out_max {
        return Err(FilterError::invalid(
            "Minimum output value cannot be greater than Maximum output value.",
        ));
    }
    let (in_min, in_max) = minimum_maximum(input, n_threads)?;
    let range = f64::from(out_max) - f64::from(out_min);
    let scale = if !almost_equal_f32(in_min, in_max) {
        range / (f64::from(in_max) - f64::from(in_min))
    } else if !almost_equal_f32(in_max, 0.0) {
        range / f64::from(in_max)
    } else {
        0.0
    };
    let shift = f64::from(out_min) - f64::from(in_min) * scale;
    Ok(parallel::with_threads(n_threads, || {
        input
            .par_iter()
            .with_min_len(CHUNK)
            .map(|&v| {
                let mut r = (f64::from(v) * scale + shift) as f32;
                r = if r > out_max { out_max } else { r };
                if r < out_min { out_min } else { r }
            })
            .collect()
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescales_the_range() {
        let out = rescale_intensity(&[2.0, 4.0, 6.0], 0.0, 1.0, 1).unwrap();
        assert_eq!(out, [0.0, 0.5, 1.0]);
        // A constant image other than 0: scale = range / value, shift = out_min - value *
        // scale, so every voxel maps to out_min.
        let out = rescale_intensity(&[5.0, 5.0], 0.0, 10.0, 1).unwrap();
        assert_eq!(out, [0.0, 0.0]);
        let out = rescale_intensity(&[0.0, 0.0], 1.0, 10.0, 1).unwrap();
        assert_eq!(out, [1.0, 1.0]);
        assert!(rescale_intensity(&[1.0], 2.0, 1.0, 1).is_err());
    }
}
