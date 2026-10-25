// SPDX-License-Identifier: Apache-2.0
//! `N4BiasFieldCorrection` (ANTs v2.6.5, `Examples/N4BiasFieldCorrection.cxx`): what the
//! program does around ITK's N4 filter ([`larmorx_image::n4`]).
//!
//! 1. The intensity range of the input inside the mask (mask ≠ 0; the whole image without a
//!    mask) is recorded for the rescaling.
//! 2. With a **spline distance** (`-b [200]`), the image (and the mask and weight image) is
//!    padded with zeros so that a whole number of spans of that length covers it, and the
//!    number of control points follows from it; with a **mesh** (`-b [2x2x3]`), the control
//!    points are `mesh + order` per axis; by default 4 per axis.
//! 3. The image, mask and weights are subsampled with `ShrinkImageFilter` (`-s`, default 4;
//!    never along a fourth axis).
//! 4. N4 runs on the subsampled images and returns the control points of the log bias field.
//! 5. The log bias field is evaluated at full resolution (on the padded grid), exponentiated
//!    (`std::exp` in double, rounded to float) and divided into the image.
//! 6. With a mask, voxels where the mask is (almost) 0 keep their input value, and, unless
//!    `-r 0`, the voxels where the mask is (almost) 1 are rescaled linearly to the input's
//!    intensity range in the mask.
//! 7. Both images are cropped back to the input's grid.
//!
//! ANTs' result depends on its thread count (ITK's B-spline fit accumulates per thread);
//! larmorx computes ITK's single-thread result with any number of threads
//! (`docs/findings/ants-n4.md`).
//!
//! ITK's defects in the filter (ANTS-38, ANTS-39, ANTS-40: the zero and negative voxels left out
//! of the fit, [`larmorx_image::n4`]) are corrected unless [`N4Options::upstream_defects`] is
//! [`UpstreamDefects::Reproduce`].

use larmorx_core::defects::{Notice, UpstreamDefects};
use larmorx_image::bspline::evaluate;
use larmorx_image::itk_math::almost_equal_f32;
use larmorx_image::n4::{N4Iteration, N4Parameters, n4_bias_field};
use larmorx_image::shrink::{shrink, shrink_plan};
use larmorx_image::{FilterError, ItkGrid};
use rayon::prelude::*;

use crate::image::AntsImage;

/// How the B-spline mesh of the first fitting level is chosen (`-b`).
#[derive(Clone, Debug, PartialEq)]
pub enum BSplineMesh {
    /// ITK's default: 4 control points per axis (one span of a cubic spline).
    Default,
    /// `-b [distance]`: spans of `distance` mm, the image padded to fit them.
    Distance(f32),
    /// `-b [AxBxC]`: that many spans per axis (truncated to integers).
    Elements(Vec<f32>),
}

/// The options of `N4BiasFieldCorrection` (ANTs' defaults through [`Default`]).
#[derive(Clone, Debug, PartialEq)]
pub struct N4Options {
    /// `-s`: the shrink factor of the spatial axes (values below 1 mean 1).
    pub shrink_factor: i32,
    /// `-c [iterations, …]`: iterations per fitting level (default 50×50×50×50).
    pub iterations: Vec<u32>,
    /// `-c […, threshold]` (default 0).
    pub convergence_threshold: f32,
    /// `-b`: the initial mesh.
    pub mesh: BSplineMesh,
    /// `-b […, order]` (default 3).
    pub spline_order: u32,
    /// `-t [fwhm, …]` (default 0.15).
    pub fwhm: f32,
    /// `-t […, wienerNoise, …]` (default 0.01).
    pub wiener_noise: f32,
    /// `-t […, bins]` (default 200).
    pub bins: u32,
    /// `-r`: rescale to the input's intensity range in the mask. ANTs does this only when a
    /// mask is given (and does it by default then); without a mask this has no effect.
    pub rescale_intensities: bool,
    /// Correct the filter's defects (ANTS-38, ANTS-39, ANTS-40; the default) or reproduce them.
    pub upstream_defects: UpstreamDefects,
}

impl Default for N4Options {
    fn default() -> Self {
        N4Options {
            shrink_factor: 4,
            iterations: vec![50; 4],
            convergence_threshold: 0.0,
            mesh: BSplineMesh::Default,
            spline_order: 3,
            fwhm: 0.15,
            wiener_noise: 0.01,
            bins: 200,
            rescale_intensities: false,
            upstream_defects: UpstreamDefects::Fix,
        }
    }
}

/// [`n4_bias_field_correction`] failed.
#[derive(Debug, thiserror::Error)]
pub enum N4Error {
    /// `-b` names neither one distance nor one value per axis.
    #[error("Incorrect mesh resolution")]
    MeshResolution,
    #[error("the {what} has {actual} voxels, the image {expected}")]
    Size {
        what: &'static str,
        actual: usize,
        expected: usize,
    },
    /// ITK's `VerifyInputInformation`: the mask or weight image is on another grid (origin or
    /// spacing beyond 1e-6 of the first spacing, direction beyond 1e-6). ANTs aborts (mask)
    /// or exits 1 (weight image).
    #[error("Inputs do not occupy the same physical space! (the {0} and the image)")]
    Geometry(&'static str),
    /// ITK throws (ANTs prints "Exception caught" and exits 1).
    #[error(transparent)]
    Filter(#[from] FilterError),
    /// The padded image needs more memory than the machine has (a spline distance far beyond
    /// the image: ITK throws `MemoryAllocationError`, which ANTs does not catch: it aborts).
    #[error(transparent)]
    Memory(#[from] larmorx_core::alloc::AllocError),
}

/// The padding and control points that `-b [distance]` gives.
#[derive(Clone, Debug, PartialEq)]
pub struct DistancePadding {
    pub lower: Vec<usize>,
    pub upper: Vec<usize>,
    pub control_points: Vec<usize>,
}

/// ANTs' padding for a spline distance (`N4BiasFieldCorrection.cxx:256-273`), in float as
/// ANTs computes it.
pub fn distance_padding(
    size: &[usize],
    spacing: &[f64],
    distance: f32,
    order: u32,
) -> DistancePadding {
    let d = size.len();
    let mut out = DistancePadding {
        lower: vec![0; d],
        upper: vec![0; d],
        control_points: vec![0; d],
    };
    for i in 0..d {
        let sp = spacing[i] as f32;
        let domain = (size[i] - 1) as f32 * sp;
        let spans = (domain / distance).ceil() as u32;
        let extra = ((spans as f32 * distance - domain) / sp + 0.5) as u64 as usize;
        let lower = (0.5 * extra as f64) as usize;
        out.lower[i] = lower;
        out.upper[i] = extra - lower;
        out.control_points[i] = (spans + order) as usize;
    }
    out
}

/// What [`n4_bias_field_correction`] returns.
#[derive(Clone, Debug, PartialEq)]
pub struct N4Output {
    /// The corrected image on the input's grid.
    pub corrected: AntsImage<f32>,
    /// The bias field (`exp` of the log bias field) on the input's grid.
    pub bias_field: AntsImage<f32>,
    /// Every iteration of every fitting level.
    pub iterations: Vec<N4Iteration>,
    /// The input's intensity range in the mask (`Original intensity range`).
    pub original_range: (f32, f32),
    /// The padding of `-b [distance]` (else `None`).
    pub padding: Option<DistancePadding>,
    /// The control points of the first level.
    pub control_points: Vec<usize>,
    /// The size of the (padded) image N4 ran on before shrinking.
    pub padded_size: Vec<usize>,
    /// The upstream defects that changed this result (or, reproduced, would have).
    pub notices: Vec<Notice>,
}

/// `ImageBase::IsCongruentImageGeometry` with ITK's default tolerances: origin and spacing
/// within `1e-6 · spacing[0]` (of `a`), direction within 1e-6, element by element.
pub(crate) fn congruent(
    a: &larmorx_io::nifti::itk::ItkGeometry,
    b: &larmorx_io::nifti::itk::ItkGeometry,
) -> bool {
    let tol = (1e-6 * a.spacing[0]).abs();
    let close = |x: &[f64], y: &[f64], t: f64| {
        x.len() == y.len() && x.iter().zip(y).all(|(p, q)| (p - q).abs() <= t)
    };
    close(&a.origin, &b.origin, tol)
        && close(&a.spacing, &b.spacing, tol)
        && a.direction.len() == b.direction.len()
        && a.direction
            .iter()
            .zip(&b.direction)
            .all(|(r, s)| close(r, s, 1e-6))
}

/// Zero-padding of a Fortran-ordered image by `lower`/`upper` voxels per axis
/// (`ConstantPadImageFilter` with constant 0).
fn pad(
    data: &[f32],
    size: &[usize],
    lower: &[usize],
    upper: &[usize],
) -> Result<(Vec<f32>, Vec<usize>), larmorx_core::alloc::AllocError> {
    let d = size.len();
    let new_size: Vec<usize> = (0..d).map(|i| size[i] + lower[i] + upper[i]).collect();
    let voxels = new_size
        .iter()
        .try_fold(1usize, |n, &s| n.checked_mul(s))
        .unwrap_or(usize::MAX);
    let mut out = larmorx_core::alloc::filled(voxels, 0.0f32)?;
    let n0 = size[0];
    let rows = data.len() / n0;
    for r in 0..rows {
        let mut rest = r;
        let mut dst = lower[0];
        let mut stride = new_size[0];
        for i in 1..d {
            let k = rest % size[i];
            rest /= size[i];
            dst += (k + lower[i]) * stride;
            stride *= new_size[i];
        }
        out[dst..dst + n0].copy_from_slice(&data[r * n0..(r + 1) * n0]);
    }
    Ok((out, new_size))
}

/// The voxels of a padded image inside the original grid (`ExtractImageFilter` back to the
/// input's region).
fn crop(data: &[f32], padded: &[usize], lower: &[usize], size: &[usize]) -> Vec<f32> {
    let d = size.len();
    let n0 = size[0];
    let rows: usize = size[1..].iter().product();
    let mut out = Vec::with_capacity(n0 * rows);
    for r in 0..rows {
        let mut rest = r;
        let mut src = lower[0];
        let mut stride = padded[0];
        for i in 1..d {
            let k = rest % size[i];
            rest /= size[i];
            src += (k + lower[i]) * stride;
            stride *= padded[i];
        }
        out.extend_from_slice(&data[src..src + n0]);
    }
    out
}

/// Runs `N4BiasFieldCorrection` on `input` with an optional mask (`-x`) and weight image
/// (`-w`), both read as float images on the input's grid. `observer` sees each iteration.
pub fn n4_bias_field_correction(
    input: &AntsImage<f32>,
    mask: Option<&AntsImage<f32>>,
    weight: Option<&AntsImage<f32>>,
    options: &N4Options,
    n_threads: usize,
    observer: Option<&mut dyn FnMut(&N4Iteration)>,
) -> Result<N4Output, N4Error> {
    let g = &input.geometry;
    let d = g.ndim;
    let len = input.len();
    for (what, image) in [("mask", mask), ("weight image", weight)] {
        if let Some(m) = image {
            if !congruent(g, &m.geometry) {
                return Err(N4Error::Geometry(what));
            }
            if m.len() != len {
                return Err(N4Error::Size {
                    what,
                    actual: m.len(),
                    expected: len,
                });
            }
        }
    }
    // The input's range inside the mask (`LabelStatisticsImageFilter`, label 1 = mask ≠ 0).
    let (mut lo, mut hi) = (f32::MAX, -f32::MAX);
    for (i, &v) in input.data.iter().enumerate() {
        if mask.is_none_or(|m| m.data[i] != 0.0) {
            if v < lo {
                lo = v;
            }
            if v > hi {
                hi = v;
            }
        }
    }
    let original_range = (lo, hi);

    // The mesh, and the padding of a spline distance.
    let order = options.spline_order;
    let (control_points, padding) = match &options.mesh {
        BSplineMesh::Default => (vec![4usize; d], None),
        BSplineMesh::Distance(distance) => {
            if !(distance.is_finite() && *distance > 0.0) {
                return Err(N4Error::Filter(FilterError::Invalid(format!(
                    "the spline distance must be positive, not {distance}"
                ))));
            }
            let p = distance_padding(&g.size, &g.spacing, *distance, order);
            (p.control_points.clone(), Some(p))
        }
        BSplineMesh::Elements(mesh) => {
            if mesh.len() != d {
                return Err(N4Error::MeshResolution);
            }
            (
                mesh.iter()
                    .map(|&m| (crate::image::x86_to_i64(f64::from(m)) as u32 + order) as usize)
                    .collect(),
                None,
            )
        }
    };
    let zero = vec![0usize; d];
    let (lower, upper) = padding
        .as_ref()
        .map_or((&zero, &zero), |p| (&p.lower, &p.upper));
    let (padded_input, padded_size) = pad(&input.data, &g.size, lower, upper)?;
    let ones;
    let mask_data: &[f32] = match mask {
        Some(m) => &m.data,
        None => {
            ones = vec![1.0f32; len];
            &ones
        }
    };
    let (padded_mask, _) = pad(mask_data, &g.size, lower, upper)?;
    let padded_weight = match weight {
        Some(w) => Some(pad(&w.data, &g.size, lower, upper)?.0),
        None => None,
    };
    let padded_grid = ItkGrid {
        size: padded_size.clone(),
        start: lower.iter().map(|&l| -(l as i64)).collect(),
        spacing: g.spacing.clone(),
        origin: g.origin.clone(),
        direction: g.direction.clone(),
    };

    // Shrink (never along a fourth axis).
    let factor = options.shrink_factor.max(0) as u32 as usize;
    let mut factors = vec![factor.max(1); d];
    if options.shrink_factor < 0 {
        factors = vec![options.shrink_factor as u32 as usize; d];
    }
    if d == 4 {
        factors[3] = 1;
    }
    let plan = shrink_plan(&padded_grid, &factors)?;
    let small_input = shrink(&padded_input, &padded_grid, &plan, n_threads)?;
    let small_mask = shrink(&padded_mask, &padded_grid, &plan, n_threads)?;
    let small_weight = padded_weight
        .as_ref()
        .map(|w| shrink(w, &padded_grid, &plan, n_threads))
        .transpose()?;

    let parameters = N4Parameters {
        iterations: options.iterations.clone(),
        convergence_threshold: options.convergence_threshold,
        control_points: control_points.clone(),
        spline_order: order as usize,
        fwhm: options.fwhm,
        wiener_noise: options.wiener_noise,
        bins: options.bins,
        // ITK's source: `r` and the points' coordinates rounded to float on every axis.
        double_axes: 0,
        upstream_defects: options.upstream_defects,
    };
    let result = n4_bias_field(
        &small_input,
        &plan.output,
        Some(&small_mask),
        small_weight.as_deref(),
        &parameters,
        n_threads,
        observer,
    )?;

    // The bias field at full resolution, on the padded grid.
    let log_field = evaluate(
        &result.lattice,
        order as usize,
        &padded_size,
        &g.spacing,
        n_threads,
    )?;
    let (bias, mut corrected): (Vec<f32>, Vec<f32>) =
        larmorx_core::parallel::with_threads(n_threads, || {
            log_field
                .par_iter()
                .zip(padded_input.par_iter())
                .with_min_len(1 << 14)
                .map(|(&l, &v)| {
                    let b = larmorx_core::math::exp(f64::from(l)) as f32;
                    let c = if almost_equal_f32(b, 0.0) {
                        f32::MAX
                    } else {
                        v / b
                    };
                    (b, c)
                })
                .unzip()
        })
        .map_err(FilterError::from)?;
    if mask.is_some() {
        for (i, c) in corrected.iter_mut().enumerate() {
            if almost_equal_f32(padded_mask[i], 0.0) {
                *c = padded_input[i];
            }
        }
        if options.rescale_intensities {
            let (mut bc_lo, mut bc_hi) = (f32::MAX, -f32::MAX);
            for (i, &v) in corrected.iter().enumerate() {
                if padded_mask[i] != 0.0 {
                    if v < bc_lo {
                        bc_lo = v;
                    }
                    if v > bc_hi {
                        bc_hi = v;
                    }
                }
            }
            let slope = (hi - lo) / (bc_hi - bc_lo);
            for (i, c) in corrected.iter_mut().enumerate() {
                if almost_equal_f32(padded_mask[i], 1.0) {
                    *c = hi - slope * (bc_hi - *c);
                }
            }
        }
    }
    let corrected = crop(&corrected, &padded_size, lower, &g.size);
    let bias = crop(&bias, &padded_size, lower, &g.size);
    Ok(N4Output {
        corrected: input.derived(corrected),
        bias_field: input.derived(bias),
        iterations: result.iterations,
        original_range,
        padding,
        control_points,
        padded_size,
        notices: result.notices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_for_a_spline_distance() {
        // 64 voxels of 3.125 mm: domain 196.875 mm, one span of 200 mm, 3.125 mm missing:
        // 1 voxel, padded above.
        let p = distance_padding(&[64, 64, 34], &[3.125, 3.125, 4.0], 200.0, 3);
        assert_eq!(p.control_points, [4, 4, 4]);
        assert_eq!(p.lower, [0, 0, 8]);
        assert_eq!(p.upper, [1, 1, 9]);
    }

    /// Found by differential random testing (2026-10-25, `ants-n4:trial1:0`): `-b [200,5]` on
    /// micrometre voxels pads to 2·10⁵ voxels per axis; ANTs aborts (`MemoryAllocationError`),
    /// larmorx refuses instead of aborting the process.
    #[test]
    fn padding_beyond_memory_is_an_error() {
        let e = pad(&[1.0; 8], &[2, 2, 2], &[1 << 21; 3], &[1 << 21; 3]).unwrap_err();
        assert!(e.to_string().starts_with("Failed to allocate memory for image"));
    }

    #[test]
    fn pad_and_crop_round_trip() {
        let data: Vec<f32> = (0..24).map(|v| v as f32).collect();
        let (p, size) = pad(&data, &[4, 3, 2], &[1, 0, 2], &[2, 1, 0]).unwrap();
        assert_eq!(size, [7, 4, 4]);
        assert_eq!(crop(&p, &size, &[1, 0, 2], &[4, 3, 2]), data);
        assert_eq!(p.iter().filter(|&&v| v != 0.0).count(), 23);
    }
}
