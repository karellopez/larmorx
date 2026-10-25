// SPDX-License-Identifier: Apache-2.0
//! `DenoiseImage` (ANTs v2.6.5, `Examples/DenoiseImage.cxx`): what the program does around
//! ITK's adaptive non-local means filter ([`larmorx_image::adaptive_denoising`]).
//!
//! 1. The input (read as `float`) is subsampled with `ShrinkImageFilter` by the shrink factor
//!    along **every** axis (time too, in 4D); a factor of 1 (the default) keeps every voxel.
//! 2. `AdaptiveNonLocalMeansDenoisingImageFilter` runs on the subsampled image, with the mask
//!    (read as `unsigned char`) indexed by the subsampled image's voxel indices. ITK checks
//!    that the mask is on the subsampled image's grid, so a mask works only on that grid.
//! 3. The noise is `subsampled − denoised`, resampled onto the input's grid with
//!    `ResampleImageFilter<float, float, float>` (an `IdentityTransform<float>` and a
//!    `LinearInterpolateImageFunction` with `float` coordinates; 0 outside).
//! 4. The output is `input − noise`, in float. So even with a shrink factor of 1 the output
//!    is `input − (input − denoised)`, which is not always `denoised` to the last bit.
//!
//! ANTs' result depends on its thread count (ITK's threads add to shared voxels without
//! locks); larmorx computes ITK's single-thread result with any number of threads
//! (`docs/findings/ants-denoise.md`).
//!
//! The filter's Rician correction factor overflows at a high signal-to-noise ratio and ITK
//! then sets the bias to 0 (ANTS-43), and with a mask the voxels outside it that patches reach
//! keep an unsmoothed, uncorrected bias (ANTS-44): both corrected unless
//! [`DenoiseOptions::upstream_defects`] is [`UpstreamDefects::Reproduce`].

use larmorx_core::IndexFloor;
use larmorx_core::defects::{Notice, Notices, UpstreamDefects};
use larmorx_image::adaptive_denoising::{
    AdaptiveNlmOutput, AdaptiveNlmParameters, MaskRef, adaptive_non_local_means,
};
use larmorx_image::shrink::{shrink_or_borrow, shrink_plan};
use larmorx_image::{FilterError, ItkGrid, VolumeRef};
use larmorx_interp::Interpolation;
use larmorx_io::nifti::itk::ItkGeometry;
use rayon::prelude::*;

use crate::image::{AntsImage, Pixel};
use crate::n4_bias_field_correction::congruent;
use crate::resample::{Coordinates, ResampleError, resample_identity_with};

/// The noise model (`-n`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoiseModel {
    /// `-n Rician`: the patches' squared values are averaged and a bias is subtracted before
    /// the square root (what sMRIPrep uses).
    Rician,
    /// `-n Gaussian`: the patches' values are averaged (`DenoiseImage`'s default).
    Gaussian,
}

/// The options of `DenoiseImage` (ANTs' defaults through [`Default`]).
#[derive(Clone, Debug, PartialEq)]
pub struct DenoiseOptions {
    /// `-n` (default Gaussian).
    pub noise_model: NoiseModel,
    /// `-s`: the shrink factor of every axis (default 1; 0 means 1).
    pub shrink_factor: i32,
    /// `-p`: the patch radius per axis (default 1); empty means 1 on every axis.
    pub patch_radius: Vec<u32>,
    /// `-r`: the search radius per axis (default 2); empty means 2 on every axis.
    pub search_radius: Vec<u32>,
    /// Correct the filter's defects (ANTS-43, ANTS-44; the default) or reproduce them.
    pub upstream_defects: UpstreamDefects,
}

impl Default for DenoiseOptions {
    fn default() -> Self {
        DenoiseOptions {
            noise_model: NoiseModel::Gaussian,
            shrink_factor: 1,
            patch_radius: Vec::new(),
            search_radius: Vec::new(),
            upstream_defects: UpstreamDefects::Fix,
        }
    }
}

/// [`denoise_image`] failed.
#[derive(Debug, thiserror::Error)]
pub enum DenoiseError {
    /// A radius has neither one value nor one per axis ("Patch radius specified
    /// incorrectly", "Search radius specified incorrectly").
    #[error("{what} radius specified incorrectly: {count} values for {dim} dimensions")]
    Radius {
        what: &'static str,
        count: usize,
        dim: usize,
    },
    /// ITK's `VerifyInputInformation`: the mask is not on the (subsampled) image's grid
    /// (origin or spacing beyond 1e-6 of the first spacing, direction beyond 1e-6).
    #[error("Inputs do not occupy the same physical space! (the mask and the image)")]
    Geometry,
    /// The mask's region does not contain the image's.
    #[error(
        "the mask ({mask:?}) does not cover the image ({image:?}): ITK's requested region is outside the mask's largest possible region"
    )]
    MaskRegion { mask: Vec<usize>, image: Vec<usize> },
    #[error(transparent)]
    Filter(#[from] FilterError),
    #[error(transparent)]
    Resample(#[from] ResampleError),
}

/// What [`denoise_image`] returns.
#[derive(Clone, Debug, PartialEq)]
pub struct DenoiseOutput {
    /// The denoised image on the input's grid (`input − noise`).
    pub denoised: AntsImage<f32>,
    /// The noise on the input's grid (`-o [denoised,noise]`).
    pub noise: AntsImage<f32>,
    /// The size of the image the filter ran on (the input's, unless shrunk).
    pub shrunk_size: Vec<usize>,
    /// The upstream defects that changed this result (or, reproduced, would have).
    pub notices: Vec<Notice>,
}

/// A radius option: one value for every axis, or one per axis.
fn radius(
    values: &[u32],
    default: usize,
    dim: usize,
    what: &'static str,
) -> Result<Vec<usize>, DenoiseError> {
    match values.len() {
        0 => Ok(vec![default; dim]),
        1 => Ok(vec![values[0] as usize; dim]),
        n if n == dim => Ok(values.iter().map(|&v| v as usize).collect()),
        n => Err(DenoiseError::Radius {
            what,
            count: n,
            dim,
        }),
    }
}

fn geometry_of(grid: &ItkGrid, like: &ItkGeometry) -> ItkGeometry {
    ItkGeometry {
        ndim: grid.dim(),
        size: grid.size.clone(),
        spacing: grid.spacing.clone(),
        origin: grid.origin.clone(),
        direction: grid.direction.clone(),
        source: like.source,
        flipped: like.flipped.clone(),
    }
}

/// Runs `DenoiseImage` on `input` with an optional mask (`-x`, read as `unsigned char`).
pub fn denoise_image(
    input: &AntsImage<f32>,
    mask: Option<&AntsImage<u8>>,
    options: &DenoiseOptions,
    n_threads: usize,
) -> Result<DenoiseOutput, DenoiseError> {
    let g = &input.geometry;
    let d = g.ndim;
    let patch = radius(&options.patch_radius, 1, d, "Patch")?;
    let search = radius(&options.search_radius, 2, d, "Search")?;

    // ShrinkImageFilter::SetShrinkFactors(unsigned int): every axis, 0 → 1, a negative int
    // read as unsigned.
    let factor = options.shrink_factor as u32 as usize;
    let grid = ItkGrid::new(&g.size, &g.spacing, &g.origin, &g.direction);
    let plan = shrink_plan(&grid, &vec![factor; d])?;
    // A shrink factor of 1 uses the image as it is, not a copy (random testing, 2026-10-25:
    // DenoiseImage's peak memory against ANTs').
    let shrunk = shrink_or_borrow(&input.data, &grid, &plan, n_threads)?;
    let small = geometry_of(&plan.output, g);

    let mask_ref = match mask {
        Some(m) => {
            if m.dim() != d || !congruent(&small, &m.geometry) {
                return Err(DenoiseError::Geometry);
            }
            if (0..d).any(|k| m.geometry.size[k] < small.size[k]) {
                return Err(DenoiseError::MaskRegion {
                    mask: m.geometry.size.clone(),
                    image: small.size.clone(),
                });
            }
            Some(MaskRef {
                data: &m.data,
                size: &m.geometry.size,
            })
        }
        None => None,
    };
    let mut parameters =
        AdaptiveNlmParameters::denoise_image(d, options.noise_model == NoiseModel::Rician);
    parameters.patch_radius = patch;
    parameters.search_radius = search;
    parameters.upstream_defects = options.upstream_defects;
    let result = adaptive_non_local_means(
        VolumeRef::new(&shrunk[..], &small.size, &small.spacing),
        mask_ref,
        &parameters,
        n_threads,
    )?;

    // The noise at the filter's resolution (in place of the filter's output, which is not
    // needed again), then on the input's grid.
    let AdaptiveNlmOutput {
        denoised: mut small_noise,
        rician_bias,
        notices: filter_notices,
    } = result;
    drop(rician_bias);
    larmorx_core::parallel::with_threads(n_threads, || {
        small_noise
            .par_iter_mut()
            .zip(shrunk.par_iter())
            .with_min_len(1 << 14)
            .for_each(|(b, &a)| *b = a - *b);
    })
    .map_err(FilterError::from)?;
    drop(shrunk);
    let floor = IndexFloor::new(options.upstream_defects);
    let noise = resample_identity_with(
        &small_noise,
        &small,
        g,
        &Interpolation::Linear,
        0.0f32,
        f32::from_f64_bounded,
        Coordinates::Float,
        &floor,
        n_threads,
    )?;
    drop(small_noise);
    let mut notices = Notices::new(options.upstream_defects);
    notices.extend(filter_notices);
    floor.notice(&mut notices, "output voxels");
    let denoised: Vec<f32> = larmorx_core::parallel::with_threads(n_threads, || {
        input
            .data
            .par_iter()
            .zip(noise.par_iter())
            .with_min_len(1 << 14)
            .map(|(&a, &b)| a - b)
            .collect()
    })
    .map_err(FilterError::from)?;
    Ok(DenoiseOutput {
        denoised: input.derived(denoised),
        noise: input.derived(noise),
        shrunk_size: small.size,
        notices: notices.into_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_io::nifti::itk::{GeometrySource, ItkMeta};

    fn image(size: &[usize], data: Vec<f32>) -> AntsImage<f32> {
        let d = size.len();
        AntsImage {
            data,
            geometry: ItkGeometry {
                ndim: d,
                size: size.to_vec(),
                spacing: vec![1.0; d],
                origin: vec![0.0; d],
                direction: (0..d)
                    .map(|i| (0..d).map(|j| f64::from(u8::from(i == j))).collect())
                    .collect(),
                source: GeometrySource::Default,
                flipped: vec![false; d],
            },
            meta: ItkMeta::default(),
        }
    }

    #[test]
    fn radii_follow_ants() {
        assert_eq!(radius(&[], 2, 3, "Search").unwrap(), [2, 2, 2]);
        assert_eq!(radius(&[3], 1, 3, "Patch").unwrap(), [3, 3, 3]);
        assert_eq!(radius(&[1, 2], 1, 2, "Patch").unwrap(), [1, 2]);
        assert!(radius(&[1, 2], 1, 3, "Patch").is_err());
    }

    #[test]
    fn noise_and_output_add_up_and_threads_do_not_matter() {
        let size = [12usize, 10, 8];
        let data: Vec<f32> = (0..960)
            .map(|i| 100.0 + ((i * 7919) % 97) as f32 + if i % 11 == 0 { 0.0 } else { 40.0 })
            .collect();
        let input = image(&size, data);
        for model in [NoiseModel::Rician, NoiseModel::Gaussian] {
            for shrink in [1, 2] {
                let options = DenoiseOptions {
                    noise_model: model,
                    shrink_factor: shrink,
                    ..DenoiseOptions::default()
                };
                let a = denoise_image(&input, None, &options, 1).unwrap();
                let b = denoise_image(&input, None, &options, 4).unwrap();
                assert_eq!(a, b);
                for i in 0..input.len() {
                    assert_eq!(a.denoised.data[i], input.data[i] - a.noise.data[i]);
                }
            }
        }
    }

    #[test]
    fn a_mask_must_be_on_the_filtered_grid() {
        let size = [8usize, 8, 6];
        let input = image(&size, (0..384).map(|i| 50.0 + (i % 13) as f32).collect());
        let mut mask = image(&size, vec![0.0; 384]).with_data(vec![1u8; 384]);
        let options = DenoiseOptions::default();
        assert!(denoise_image(&input, Some(&mask), &options, 1).is_ok());
        // Shrunk by 2, the image's spacing is 2: the full-resolution mask is rejected.
        let shrunk = DenoiseOptions {
            shrink_factor: 2,
            ..DenoiseOptions::default()
        };
        assert!(matches!(
            denoise_image(&input, Some(&mask), &shrunk, 1),
            Err(DenoiseError::Geometry)
        ));
        mask.geometry.origin[1] = 0.5;
        assert!(matches!(
            denoise_image(&input, Some(&mask), &options, 1),
            Err(DenoiseError::Geometry)
        ));
    }
}
