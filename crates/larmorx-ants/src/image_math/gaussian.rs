// SPDX-License-Identifier: Apache-2.0
//! ImageMath's Gaussian operations (`Examples/ImageMath_Templates.hxx`, ANTs v2.6.5):
//! `G` ([`smooth_discrete`], `SmoothImage<DIM>`), `Laplacian` ([`laplacian`],
//! `LaplacianImage`), `Grad` ([`gradient_magnitude`], `GradientImage`) and `UnsharpMask`
//! ([`unsharp_mask`], `UnsharpMaskImage`), on `float` images as ANTs reads them. The ITK
//! filters behind them are in [`larmorx_image::gaussian`] and
//! [`larmorx_image::discrete_gaussian`].

use larmorx_core::parallel;
use larmorx_image::discrete_gaussian::{DiscreteGaussianOptions, discrete_gaussian};
use larmorx_image::gaussian::{
    gradient_magnitude_recursive_gaussian, laplacian_recursive_gaussian,
    smoothing_recursive_gaussian,
};
use larmorx_image::intensity::rescale_intensity;
use larmorx_image::{FilterError, VolumeRef};
use rayon::prelude::*;

const CHUNK: usize = 1 << 16;

/// `ImageMath G`: `DiscreteGaussianImageFilter` with a variance of `sigma²` (computed in
/// float) in physical units, a maximum error of `0.01f` and kernels of at most 32 voxels on
/// each side of the centre.
///
/// `sigma` has one value for every axis or one for all. With another count ANTs prints
/// "Incorrect sigma vector size" and filters with a variance of 0, which leaves the image
/// unchanged; pass `None` for that.
pub fn smooth_discrete(
    input: VolumeRef<'_, f32>,
    sigma: Option<&[f32]>,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let d = input.dim();
    let mut options = DiscreteGaussianOptions::new(d, 0.0);
    match sigma {
        Some([s]) => options.variance = vec![f64::from(s * s); d],
        Some(s) if s.len() == d => options.variance = s.iter().map(|&v| f64::from(v * v)).collect(),
        Some(s) => {
            return Err(FilterError::Invalid(format!(
                "{} sigmas for a {d}-dimensional image (one or {d})",
                s.len()
            )));
        }
        None => {}
    }
    options.maximum_error = vec![f64::from(0.01f32); d];
    discrete_gaussian(input, &options, n_threads)
}

/// `[0, 1]` rescaling (`RescaleIntensityImageFilter`) when `normalize`.
fn rescaled(data: Vec<f32>, normalize: bool, n_threads: usize) -> Result<Vec<f32>, FilterError> {
    if normalize {
        rescale_intensity(&data, 0.0, 1.0, n_threads)
    } else {
        Ok(data)
    }
}

/// `ImageMath Laplacian`: `LaplacianRecursiveGaussianImageFilter` with `sigma` (physical
/// units), rescaled to `[0, 1]` if `normalize`. ANTs replaces a sigma `≤ 0` by 0.5; so does
/// this function.
pub fn laplacian(
    input: VolumeRef<'_, f32>,
    sigma: f32,
    normalize: bool,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let sigma = if sigma <= 0.0 { 0.5 } else { sigma };
    let data = laplacian_recursive_gaussian(input, f64::from(sigma), false, n_threads)?;
    rescaled(data, normalize, n_threads)
}

/// `ImageMath Grad`: `GradientMagnitudeRecursiveGaussianImageFilter` with `sigma` (physical
/// units), rescaled to `[0, 1]` if `normalize`. A sigma `≤ 0` becomes 0.5, as in ANTs.
pub fn gradient_magnitude(
    input: VolumeRef<'_, f32>,
    sigma: f32,
    normalize: bool,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let sigma = if sigma <= 0.0 { 0.5 } else { sigma };
    let data = gradient_magnitude_recursive_gaussian(input, f64::from(sigma), false, n_threads)?;
    rescaled(data, normalize, n_threads)
}

/// The parameters of [`unsharp_mask`], with ANTs' defaults in `Default`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnsharpMaskOptions {
    /// How much of the difference to add (default 0.5).
    pub amount: f32,
    /// The Gaussian's sigma, in voxels (multiplied by each axis's spacing) unless
    /// `radius_in_spacing_units` (default 1).
    pub radius: f32,
    /// Differences up to this size are left alone (default 0; must not be negative).
    pub threshold: f32,
    /// Whether `radius` is in physical units (default false).
    pub radius_in_spacing_units: bool,
}

impl Default for UnsharpMaskOptions {
    fn default() -> Self {
        UnsharpMaskOptions {
            amount: 0.5,
            radius: 1.0,
            threshold: 0.0,
            radius_in_spacing_units: false,
        }
    }
}

/// `ImageMath UnsharpMask`: ITK's `UnsharpMaskImageFilter<float, float>` (internal precision
/// float): `s` is the image smoothed with `SmoothingRecursiveGaussianImageFilter`, and each
/// voxel `v` becomes, in float, `v + (d − t)·amount` where `d = v − s > t`,
/// `v + (d + t)·amount` where `−d > t`, else `v`.
pub fn unsharp_mask(
    input: VolumeRef<'_, f32>,
    options: &UnsharpMaskOptions,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    if options.threshold < 0.0 {
        return Err(FilterError::Invalid(
            "Threshold must be non-negative!".into(),
        ));
    }
    let sigma: Vec<f64> = input
        .spacing
        .iter()
        .map(|&s| {
            if options.radius_in_spacing_units {
                f64::from(options.radius)
            } else {
                f64::from(options.radius) * s
            }
        })
        .collect();
    let smoothed = smoothing_recursive_gaussian(input, &sigma, false, n_threads)?;
    let (amount, t) = (options.amount, options.threshold);
    Ok(parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .zip(smoothed.par_iter())
            .with_min_len(CHUNK)
            .map(|(&v, &s)| {
                let diff = v - s;
                if diff > t {
                    v + (diff - t) * amount
                } else if -diff > t {
                    v + (diff + t) * amount
                } else {
                    v
                }
            })
            .collect()
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(size: &[usize]) -> Vec<f32> {
        let n: usize = size.iter().product();
        (0..n).map(|i| ((i * 13) % 29) as f32).collect()
    }

    #[test]
    fn operations_run_and_normalize() {
        let size = [10usize, 8, 6];
        let spacing = [1.0, 1.5, 2.0];
        let data = ramp(&size);
        let v = VolumeRef::new(&data, &size, &spacing);
        let lap = laplacian(v, 1.5, true, 2).unwrap();
        let (lo, hi) = lap
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        assert_eq!((lo, hi), (0.0, 1.0));
        assert_eq!(
            laplacian(v, 0.0, false, 1).unwrap(),
            laplacian(v, 0.5, false, 1).unwrap()
        );
        let grad = gradient_magnitude(v, 1.0, false, 3).unwrap();
        assert!(grad.iter().all(|&g| g >= 0.0));
        // No sigma: variance 0, the image is unchanged.
        assert_eq!(smooth_discrete(v, None, 1).unwrap(), data);
        let g = smooth_discrete(v, Some(&[1.0]), 1).unwrap();
        assert_eq!(g, smooth_discrete(v, Some(&[1.0, 1.0, 1.0]), 4).unwrap());
        assert!(smooth_discrete(v, Some(&[1.0, 1.0]), 1).is_err());
        let sharp = unsharp_mask(v, &UnsharpMaskOptions::default(), 2).unwrap();
        assert_eq!(sharp.len(), data.len());
        let bad = UnsharpMaskOptions {
            threshold: -1.0,
            ..Default::default()
        };
        assert!(unsharp_mask(v, &bad, 1).is_err());
    }
}
