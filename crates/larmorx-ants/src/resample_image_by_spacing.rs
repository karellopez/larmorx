// SPDX-License-Identifier: Apache-2.0
//! `ResampleImageBySpacing` (ANTs v2.6.5, `Examples/ResampleImageBySpacing.cxx`): optional
//! Gaussian smoothing, then resampling onto a grid with the same origin and direction and a new
//! spacing, by linear or nearest-neighbour interpolation.
//!
//! What ANTs computes, and this reproduces:
//! - **Smoothing** (on by default): for each axis `d` in order, `sigma = float(out_spacing[d] /
//!   in_spacing[d] − 1)`, and where it is positive, a zero-order `RecursiveGaussianImageFilter`
//!   with that sigma along `d`. ITK takes the sigma in **physical units** and divides it by the
//!   spacing again, so the smoothing is `(out/in − 1) / in` voxels: one voxel for 1 → 2 mm,
//!   half a voxel for 2 → 4 mm.
//! - **The output grid** keeps the input's origin and direction, takes the new spacing, and
//!   has `size[d] = (size_t)(in_size[d] · in_spacing[d] / out_spacing[d] + add_voxels)`
//!   (truncated).
//! - **Outside the input** (the output may extend past it), voxels get the input's value at
//!   index `(1, 1, …)`, before smoothing.
//!
//! **ANTS-28.** ITK cannot smooth an axis of fewer than 4 voxels and throws; ANTs catches the
//! exception ("Exception catched !") and resamples the smoothing filter's output buffer,
//! which was allocated but never written: its values depend on memory and change from run to
//! run. With [`UpstreamDefects::Fix`] (the default) that is an error, with ITK's message. With
//! [`UpstreamDefects::Reproduce`] the run goes on as ANTs' does, with the unwritten buffer
//! filled with 0, and a notice that says so.

use larmorx_core::IndexFloor;
use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};
use larmorx_image::FilterError;
use larmorx_image::gaussian::{GaussianOrder, recursive_gaussian};
use larmorx_io::nifti::itk::ItkGeometry;

use crate::image::AntsImage;
use crate::resample::{ResampleError, resample_identity};
use larmorx_interp::Interpolation;

/// The parameters of [`resample_image_by_spacing`].
#[derive(Clone, Debug, PartialEq)]
pub struct ResampleBySpacingOptions {
    /// The output spacing, one value per axis (the time spacing too for a 4D image).
    pub spacing: Vec<f64>,
    /// Smooth before resampling (ANTs' default: yes).
    pub smooth: bool,
    /// Voxels added to (or, if negative, removed from) the output size on every axis.
    pub add_voxels: i32,
    /// Nearest-neighbour interpolation instead of linear.
    pub nearest: bool,
}

/// [`resample_image_by_spacing`] failed.
#[derive(Debug, thiserror::Error)]
pub enum ResampleBySpacingError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Filter(#[from] FilterError),
    #[error(transparent)]
    Resample(#[from] ResampleError),
}

/// The plan of a [`resample_image_by_spacing`] run: what ANTs prints before it resamples.
#[derive(Clone, Debug, PartialEq)]
pub struct ResampleBySpacingPlan {
    /// The smoothing sigma of each axis (`float`, physical units; axes with a sigma ≤ 0 are not
    /// smoothed). Empty without smoothing.
    pub sigmas: Vec<f32>,
    /// The output grid.
    pub geometry: ItkGeometry,
}

/// The sigmas and output grid ANTs computes for `input` (see the module docs).
pub fn plan(
    input: &ItkGeometry,
    options: &ResampleBySpacingOptions,
) -> Result<ResampleBySpacingPlan, ResampleBySpacingError> {
    let d = input.ndim;
    if options.spacing.len() != d {
        return Err(ResampleBySpacingError::Invalid(format!(
            "{} output spacings for a {d}-dimensional image",
            options.spacing.len()
        )));
    }
    let sigmas = if options.smooth {
        (0..d)
            .map(|k| (options.spacing[k] / input.spacing[k] - 1.0) as f32)
            .collect()
    } else {
        Vec::new()
    };
    let mut geometry = input.clone();
    geometry.spacing = options.spacing.clone();
    for k in 0..d {
        let size = input.size[k] as f64 * input.spacing[k] / options.spacing[k]
            + f64::from(options.add_voxels);
        // `static_cast<size_t>`: undefined for negative or huge values.
        if !(0.0..1e12).contains(&size) {
            return Err(ResampleBySpacingError::Invalid(format!(
                "an output size of {size} voxels along axis {k} (ANTs' conversion to an unsigned size is undefined)"
            )));
        }
        geometry.size[k] = size as usize;
    }
    Ok(ResampleBySpacingPlan {
        sigmas,
        geometry: geometry.stored_in_new_image(),
    })
}

/// The axes whose smoothing ITK refuses (a positive sigma on an axis of fewer than 4 voxels;
/// ANTS-28).
pub fn unsmoothable_axes(input: &ItkGeometry, plan: &ResampleBySpacingPlan) -> Vec<usize> {
    plan.sigmas
        .iter()
        .enumerate()
        .filter(|&(k, &sigma)| sigma > 0.0 && input.size[k] < 4)
        .map(|(k, _)| k)
        .collect()
}

/// `ResampleImageBySpacing`: `input` smoothed (if `options.smooth`) and resampled onto the
/// grid of [`plan`]. Returns the image, the plan and the notices of the defects it met
/// (ANTS-28, see the module).
pub fn resample_image_by_spacing(
    input: &AntsImage<f32>,
    options: &ResampleBySpacingOptions,
    n_threads: usize,
    mode: UpstreamDefects,
) -> Result<(AntsImage<f32>, ResampleBySpacingPlan, Vec<Notice>), ResampleBySpacingError> {
    let mut notices = Notices::new(mode);
    let plan = plan(&input.geometry, options)?;
    let d = input.dim();
    // `inputImage->GetPixel({1, 1, ...})`, read before smoothing.
    if input.size().iter().any(|&n| n < 2) {
        return Err(ResampleBySpacingError::Invalid(
            "the default value is the voxel at index (1, 1, ...), outside an image with an axis of one voxel (ANTs reads outside the image)".into(),
        ));
    }
    let strides = larmorx_image::strides(input.size());
    let default_value = input.data[strides.iter().sum::<usize>()];
    let mut smoothed: Option<Vec<f32>> = None;
    for (k, &sigma) in plan.sigmas.iter().enumerate() {
        if sigma > 0.0 && input.size()[k] < 4 && mode.reproduce() {
            // ITK throws before writing its output; ANTs goes on with that buffer.
            notices.add(
                &defects::ANTS_28,
                format!(
                    "the smoothing of axis {k} ({} voxels) fails as in ANTs, which then \
                     resamples the filter's never-written buffer (its values depend on memory): \
                     here 0",
                    input.size()[k]
                ),
            );
            smoothed = Some(vec![0.0; input.data.len()]);
            continue;
        }
        if sigma > 0.0 {
            let current = smoothed.as_deref().unwrap_or(&input.data);
            let view =
                larmorx_image::VolumeRef::new(current, input.size(), &input.geometry.spacing);
            smoothed = Some(recursive_gaussian(
                view,
                k,
                f64::from(sigma),
                GaussianOrder::Zero,
                false,
                n_threads,
            )?);
        }
    }
    let source = smoothed.as_deref().unwrap_or(&input.data);
    let interpolation = if options.nearest {
        Interpolation::NearestNeighbor
    } else {
        Interpolation::Linear
    };
    // `ResampleImageFilter<float, float>`: the double result cast to float (a linear or
    // nearest value of float voxels is always within float's range).
    let floor = IndexFloor::new(mode);
    let data = resample_identity(
        source,
        &input.geometry,
        &plan.geometry,
        &interpolation,
        default_value,
        |v| v as f32,
        &floor,
        n_threads,
    )?;
    floor.notice(&mut notices, "output voxels");
    debug_assert_eq!(plan.geometry.ndim, d);
    let image = AntsImage {
        data,
        geometry: plan.geometry.clone(),
        meta: Default::default(),
    };
    Ok((image, plan, notices.into_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_io::nifti::itk::GeometrySource;

    fn image(size: &[usize], spacing: &[f64]) -> AntsImage<f32> {
        let d = size.len();
        let n: usize = size.iter().product();
        AntsImage {
            data: (0..n).map(|i| ((i * 37) % 53) as f32).collect(),
            geometry: ItkGeometry {
                ndim: d,
                size: size.to_vec(),
                spacing: spacing.to_vec(),
                origin: vec![1.0; d],
                direction: (0..d)
                    .map(|i| (0..d).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
                    .collect(),
                source: GeometrySource::Default,
                flipped: vec![false; d],
            },
            meta: Default::default(),
        }
    }

    #[test]
    fn sizes_sigmas_and_threads() {
        let input = image(&[10, 9, 8], &[1.0, 1.5, 2.0]);
        let options = ResampleBySpacingOptions {
            spacing: vec![2.0, 1.5, 3.0],
            smooth: true,
            add_voxels: 0,
            nearest: false,
        };
        let fix = UpstreamDefects::Fix;
        let (a, plan, notices) = resample_image_by_spacing(&input, &options, 1, fix).unwrap();
        assert_eq!(plan.sigmas, [1.0, 0.0, 0.5]);
        assert_eq!(a.size(), [5, 9, 5]);
        assert!(notices.is_empty());
        let (b, _, _) = resample_image_by_spacing(&input, &options, 4, fix).unwrap();
        assert_eq!(a.data, b.data);
        let more = ResampleBySpacingOptions {
            add_voxels: 2,
            nearest: true,
            smooth: false,
            ..options
        };
        let (c, plan, _) = resample_image_by_spacing(&input, &more, 2, fix).unwrap();
        assert!(plan.sigmas.is_empty());
        assert_eq!(c.size(), [7, 11, 7]);
        // Beyond the input: the value at (1, 1, 1).
        assert_eq!(*c.data.last().unwrap(), input.data[1 + 10 + 90]);
    }

    #[test]
    fn thin_smoothed_axes() {
        // ANTS-28: the third axis has 3 voxels and a positive sigma.
        let input = image(&[10, 9, 3], &[1.0, 1.0, 1.0]);
        let options = ResampleBySpacingOptions {
            spacing: vec![1.0, 1.0, 2.0],
            smooth: true,
            add_voxels: 2,
            nearest: false,
        };
        let e = resample_image_by_spacing(&input, &options, 1, UpstreamDefects::Fix);
        assert!(e.unwrap_err().to_string().contains("less than 4"));
        let (out, plan, notices) =
            resample_image_by_spacing(&input, &options, 1, UpstreamDefects::Reproduce).unwrap();
        assert_eq!(unsmoothable_axes(&input.geometry, &plan), [2]);
        assert_eq!(notices[0].defect.id, "ANTS-28");
        // Inside the input the zeros of the unwritten buffer, beyond it the value at (1, 1, 1).
        assert_eq!(out.data[0], 0.0);
        assert_eq!(*out.data.last().unwrap(), input.data[1 + 10 + 90]);
    }
}
