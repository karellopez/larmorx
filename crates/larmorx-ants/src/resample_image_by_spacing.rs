// SPDX-License-Identifier: Apache-2.0
//! `ResampleImageBySpacing` (ANTs v2.6.5, `Examples/ResampleImageBySpacing.cxx`): optional
//! Gaussian smoothing, then resampling onto a grid with the same origin and direction and a new
//! spacing, by linear or nearest-neighbour interpolation.
//!
//! What ANTs computes:
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
//! **ANTS-32.** The sigma `out/in − 1` is a number of input voxels (the downsampling factor
//! less one: no smoothing at the same spacing, one voxel for a factor of 2); taking it in
//! millimetres makes the smoothing depend on the units of the spacing, and the source's
//! comment on the outside value says "zero regions without source". Corrected (the default):
//! the sigma is `out/in − 1` input voxels (`out − in` millimetres), and voxels beyond the
//! input are 0; each named where it changes the result. [`UpstreamDefects::Reproduce`] gives
//! ANTs' result.
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
    /// The smoothing sigma of each axis as ANTs computes it, `float(out/in − 1)` (axes with a
    /// sigma ≤ 0 are not smoothed). Empty without smoothing.
    pub sigmas: Vec<f32>,
    /// The output grid.
    pub geometry: ItkGeometry,
}

impl ResampleBySpacingPlan {
    /// The sigma of axis `k` in physical units, as the smoothing filter receives it: ANTs'
    /// `sigmas[k]` itself ([`UpstreamDefects::Reproduce`]), or that many input voxels,
    /// `sigmas[k] · in_spacing[k]` (ANTS-32 corrected, the default).
    pub fn physical_sigma(&self, k: usize, input: &ItkGeometry, mode: UpstreamDefects) -> f64 {
        let s = f64::from(self.sigmas[k]);
        if mode.fix() { s * input.spacing[k] } else { s }
    }
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
/// (ANTS-28, ANTS-32, see the module).
pub fn resample_image_by_spacing(
    input: &AntsImage<f32>,
    options: &ResampleBySpacingOptions,
    n_threads: usize,
    mode: UpstreamDefects,
) -> Result<(AntsImage<f32>, ResampleBySpacingPlan, Vec<Notice>), ResampleBySpacingError> {
    let mut notices = Notices::new(mode);
    let plan = plan(&input.geometry, options)?;
    let d = input.dim();
    // ANTs' outside value: `inputImage->GetPixel({1, 1, ...})`, read before smoothing.
    let thin = input.size().iter().any(|&n| n < 2);
    if thin && mode.reproduce() {
        return Err(ResampleBySpacingError::Invalid(
            "the default value is the voxel at index (1, 1, ...), outside an image with an axis of one voxel (ANTs reads outside the image)".into(),
        ));
    }
    let ants_default = if thin {
        None
    } else {
        let strides = larmorx_image::strides(input.size());
        Some(input.data[strides.iter().sum::<usize>()])
    };
    let default_value = if mode.fix() {
        0.0
    } else {
        ants_default.expect("checked above")
    };
    // ANTS-32's sigmas: named where the units change the smoothing.
    let rescaled: Vec<String> = plan
        .sigmas
        .iter()
        .enumerate()
        .filter(|&(k, &s)| {
            s > 0.0
                && plan.physical_sigma(k, &input.geometry, UpstreamDefects::Fix)
                    != plan.physical_sigma(k, &input.geometry, UpstreamDefects::Reproduce)
        })
        .map(|(k, &s)| {
            format!(
                "axis {k}: {} voxel(s), {} mm; ANTs {} voxel(s)",
                f64::from(s),
                plan.physical_sigma(k, &input.geometry, UpstreamDefects::Fix),
                f64::from(s) / input.geometry.spacing[k]
            )
        })
        .collect();
    // ANTS-32's details, in one notice at the end.
    let mut ants_32: Vec<String> = Vec::new();
    if !rescaled.is_empty() {
        ants_32.push(if mode.fix() {
            format!(
                "the smoothing sigma out/in - 1 is in input voxels ({}); ANTs takes it in \
                 millimetres",
                rescaled.join("; ")
            )
        } else {
            format!(
                "the smoothing sigma out/in - 1 is taken in millimetres, as in ANTs ({})",
                rescaled.join("; ")
            )
        });
    }
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
                plan.physical_sigma(k, &input.geometry, mode),
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
    let resample = |default_value: f32, floor: &IndexFloor| {
        resample_identity(
            source,
            &input.geometry,
            &plan.geometry,
            &interpolation,
            default_value,
            |v| v as f32,
            floor,
            n_threads,
        )
    };
    let data = resample(default_value, &floor)?;
    floor.notice(&mut notices, "output voxels");
    // ANTS-32's outside value: the voxels beyond the input are those the other default
    // changes (an axis of one voxel has no index 1: ANTs reads outside the image there).
    let other = match ants_default {
        _ if mode.reproduce() => 0.0,
        Some(v) => v,
        None => f32::MAX,
    };
    if other.to_bits() != default_value.to_bits() {
        let alternative = resample(other, &IndexFloor::new(mode))?;
        let outside = data
            .iter()
            .zip(&alternative)
            .filter(|(a, b)| a.to_bits() != b.to_bits())
            .count();
        if outside > 0 {
            let ants_value = match ants_default {
                Some(v) => format!("the input's value at index (1, 1, ...), {v}"),
                None => "a value read outside this image (it has an axis of one voxel)".into(),
            };
            ants_32.push(if mode.fix() {
                format!(
                    "{outside} output voxel(s) beyond the input are 0; ANTs gives them \
                     {ants_value}"
                )
            } else {
                format!(
                    "{outside} output voxel(s) beyond the input get {ants_value}, as in ANTs, \
                     not 0"
                )
            });
        }
    }
    if !ants_32.is_empty() {
        notices.add(&defects::ANTS_32, ants_32.join("; "));
    }
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
        let (fix, rep) = (UpstreamDefects::Fix, UpstreamDefects::Reproduce);
        let (a, plan, notices) = resample_image_by_spacing(&input, &options, 1, fix).unwrap();
        assert_eq!(plan.sigmas, [1.0, 0.0, 0.5]);
        assert_eq!(a.size(), [5, 9, 5]);
        // ANTS-32: axis 2 (2 mm) is smoothed by half a voxel (1 mm); ANTs by 0.5 mm.
        assert_eq!(plan.physical_sigma(2, &input.geometry, fix), 1.0);
        assert_eq!(plan.physical_sigma(2, &input.geometry, rep), 0.5);
        assert_eq!(plan.physical_sigma(0, &input.geometry, fix), 1.0);
        assert_eq!(notices[0].defect.id, "ANTS-32");
        assert!(notices[0].detail.contains("axis 2"), "{notices:?}");
        assert!(!notices[0].detail.contains("axis 0"), "{notices:?}");
        let (b, _, _) = resample_image_by_spacing(&input, &options, 4, fix).unwrap();
        assert_eq!(a.data, b.data);
        let (ants, _, notices) = resample_image_by_spacing(&input, &options, 1, rep).unwrap();
        assert_ne!(ants.data, a.data);
        assert_eq!(notices[0].defect.id, "ANTS-32");
        let more = ResampleBySpacingOptions {
            add_voxels: 2,
            nearest: true,
            smooth: false,
            ..options.clone()
        };
        let (c, plan, notices) = resample_image_by_spacing(&input, &more, 2, rep).unwrap();
        assert!(plan.sigmas.is_empty());
        assert_eq!(c.size(), [7, 11, 7]);
        // Beyond the input: ANTs' value at (1, 1, 1); corrected, 0.
        assert_eq!(*c.data.last().unwrap(), input.data[1 + 10 + 90]);
        assert!(
            notices[0].detail.contains("beyond the input"),
            "{notices:?}"
        );
        let (d, _, notices) = resample_image_by_spacing(&input, &more, 2, fix).unwrap();
        assert_eq!(*d.data.last().unwrap(), 0.0);
        assert!(notices[0].detail.contains("are 0"), "{notices:?}");
        let inside = c.data.iter().zip(&d.data).filter(|(x, y)| x == y).count();
        assert!(inside > 0 && inside < c.data.len());
        // An axis of one voxel: ANTs reads index 1 outside the image; corrected, no read.
        let flat = image(&[10, 9, 1], &[1.0, 1.0, 1.0]);
        let flat_options = ResampleBySpacingOptions {
            spacing: vec![1.0, 1.0, 1.0],
            smooth: false,
            add_voxels: 1,
            nearest: false,
        };
        assert!(resample_image_by_spacing(&flat, &flat_options, 1, rep).is_err());
        let (e, _, notices) = resample_image_by_spacing(&flat, &flat_options, 1, fix).unwrap();
        assert_eq!(e.size(), [11, 10, 2]);
        assert_eq!(*e.data.last().unwrap(), 0.0);
        assert!(
            notices[0].detail.contains("axis of one voxel"),
            "{notices:?}"
        );
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
