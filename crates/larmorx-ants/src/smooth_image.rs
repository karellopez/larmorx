// SPDX-License-Identifier: Apache-2.0
//! `SmoothImage` (ANTs v2.6.5, `Examples/SmoothImage.cxx`): Gaussian smoothing with ITK's
//! `SmoothingRecursiveGaussianImageFilter`, or median filtering with `MedianImageFilter`.

use larmorx_image::gaussian::smoothing_recursive_gaussian;
use larmorx_image::median::median;
use larmorx_image::{FilterError, VolumeRef};

/// What [`smooth_image`] does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Smoothing<'a> {
    /// Recursive Gaussian smoothing with one sigma per axis or one for all; in voxels
    /// (multiplied by each axis's spacing, in float) unless `physical`. `None` stands for a
    /// sigma list of the wrong length: ANTs prints "Incorrect sigma vector size" and smooths
    /// with the filter's default sigma, 1 mm on every axis.
    Gaussian {
        sigma: Option<&'a [f32]>,
        physical: bool,
    },
    /// Median filtering in a box of `radius` voxels on each axis (one value for all axes, or
    /// one per axis).
    Median { radius: &'a [usize] },
}

/// The sigmas (physical units) SmoothImage gives `SmoothingRecursiveGaussianImageFilter`:
/// `sigma · float(spacing)` computed in float when sigma is in voxels, else `sigma`.
pub fn physical_sigmas(sigma: &[f32], spacing: &[f64], physical: bool) -> Vec<f64> {
    (0..spacing.len())
        .map(|d| {
            let s = if sigma.len() == 1 { sigma[0] } else { sigma[d] };
            if physical {
                f64::from(s)
            } else {
                f64::from(s * spacing[d] as f32)
            }
        })
        .collect()
}

/// `SmoothImage`: smooths `input` as `SmoothImage <D> in <sigma> out <physical> <median>`
/// does.
pub fn smooth_image(
    input: VolumeRef<'_, f32>,
    smoothing: Smoothing<'_>,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let d = input.dim();
    match smoothing {
        Smoothing::Gaussian { sigma, physical } => {
            let sigmas = match sigma {
                Some(s) if s.len() == 1 || s.len() == d => {
                    physical_sigmas(s, input.spacing, physical)
                }
                Some(s) => {
                    return Err(FilterError::Invalid(format!(
                        "{} sigmas for a {d}-dimensional image (one or {d})",
                        s.len()
                    )));
                }
                None => vec![1.0; d],
            };
            smoothing_recursive_gaussian(input, &sigmas, false, n_threads)
        }
        Smoothing::Median { radius } => {
            let radius = match radius.len() {
                1 => vec![radius[0]; d],
                n if n == d => radius.to_vec(),
                n => {
                    return Err(FilterError::Invalid(format!(
                        "{n} radii for a {d}-dimensional image (one or {d})"
                    )));
                }
            };
            median(input, &radius, n_threads)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigma_in_voxels_is_scaled_in_float() {
        let s = physical_sigmas(&[1.5], &[1.2, 0.9], false);
        assert_eq!(s, [f64::from(1.5f32 * 1.2f32), f64::from(1.5f32 * 0.9f32)]);
        assert_eq!(physical_sigmas(&[1.0, 2.0], &[3.0, 3.0], true), [1.0, 2.0]);
    }

    #[test]
    fn modes() {
        let size = [6usize, 5, 4];
        let spacing = [1.0, 2.0, 1.5];
        let data: Vec<f32> = (0..120).map(|i| ((i * 17) % 23) as f32).collect();
        let v = VolumeRef::new(&data, &size, &spacing);
        let a = smooth_image(
            v,
            Smoothing::Gaussian {
                sigma: Some(&[1.0]),
                physical: false,
            },
            1,
        )
        .unwrap();
        let b = smooth_image(
            v,
            Smoothing::Gaussian {
                sigma: Some(&[1.0, 1.0, 1.0]),
                physical: false,
            },
            3,
        )
        .unwrap();
        assert_eq!(a, b);
        let m = smooth_image(v, Smoothing::Median { radius: &[1] }, 2).unwrap();
        assert_eq!(m, median(v, &[1, 1, 1], 1).unwrap());
        assert!(smooth_image(v, Smoothing::Median { radius: &[1, 1] }, 1).is_err());
    }
}
