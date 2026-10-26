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
use larmorx_image::shrink::{shrink, shrink_or_borrow, shrink_plan};
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
    /// The lattice and padding need more memory than larmorx may plan for (ANTS-162; the
    /// message says how much, and why).
    #[error("{0}")]
    BeyondMemory(String),
}

/// larmorx's working memory per control point of the largest fitting level's lattice (three
/// float lattices: the fit's numerator and denominator and the field) and per voxel of the
/// padded image (the padded input, mask and field and their subsampled copies), measured with
/// `validation/investigations/difftest/n4_memory.py` (`docs/findings/ants-n4.md`; ANTs needs 17
/// and 25).
const LATTICE_BYTES: f64 = 12.0;
/// In 2D (measured on ants-n4:20261025:406, 2026-10-25): 16.1 bytes per control point.
const LATTICE_BYTES_2D: f64 = 16.5;
const PADDED_BYTES: f64 = 21.0;

/// ANTS-162: the memory the lattices and the padded image need, checked before anything is
/// allocated; an error naming the bytes needed and their likely cause when they exceed the
/// [`larmorx_core::alloc::memory_limit`].
fn check_memory(
    g: &larmorx_io::nifti::itk::ItkGeometry,
    control_points: &[usize],
    order: u32,
    levels: usize,
    padded: &[usize],
    distance: Option<f32>,
) -> Result<(), N4Error> {
    let last: Vec<f64> = control_points
        .iter()
        .map(|&c| {
            let spans = c.saturating_sub(order as usize) as f64;
            spans * 2f64.powi(levels.saturating_sub(1) as i32) + f64::from(order)
        })
        .collect();
    let points: f64 = last.iter().product();
    let voxels: f64 = padded.iter().map(|&n| n as f64).product();
    let per_point = if g.ndim == 2 {
        LATTICE_BYTES_2D
    } else {
        LATTICE_BYTES
    };
    let needed = per_point * points + PADDED_BYTES * voxels;
    let Err(limit) = larmorx_core::alloc::check_plan(needed) else {
        return Ok(());
    };
    let short = |x: f64| {
        let s = format!("{x:.4}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    };
    let join = |v: &[f64]| v.iter().map(|&x| short(x)).collect::<Vec<_>>().join(" x ");
    let spacing = join(&g.spacing);
    let extent: Vec<f64> = (0..g.ndim)
        .map(|i| (g.size[i].saturating_sub(1)) as f64 * g.spacing[i])
        .collect();
    let unusual = g.spacing[..g.ndim.min(3)]
        .iter()
        .any(|&s| !(0.01..100.0).contains(&s.abs()));
    let time = if g.ndim > 3 {
        ", the fourth axis in s"
    } else {
        ""
    };
    let against = distance.map_or(String::new(), |d| {
        format!(" against a spline distance of {d}")
    });
    Err(N4Error::BeyondMemory(format!(
        "N4BiasFieldCorrection would need about {} (the last fitting level's B-spline lattice of \
         {} control points; the image padded to {} voxels), more than {limit}. The image's \
         voxels are {spacing} mm{time}: it spans {}{against}{}. An error (ANTS-162); \
         LARMORX_UPSTREAM_DEFECTS=reproduce attempts it as ANTs does",
        larmorx_core::alloc::gib(needed),
        join(&last),
        padded
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" x "),
        join(&extent),
        if unusual {
            " (a header whose units are metres or micrometres makes an image 1000 times larger \
             or smaller than it looks)"
        } else {
            ""
        },
    )))
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

/// [`pad`], or `data` itself when nothing is padded.
fn pad_or_borrow<'a>(
    data: &'a [f32],
    size: &[usize],
    lower: &[usize],
    upper: &[usize],
) -> Result<std::borrow::Cow<'a, [f32]>, larmorx_core::alloc::AllocError> {
    if lower.iter().chain(upper).all(|&p| p == 0) {
        Ok(std::borrow::Cow::Borrowed(data))
    } else {
        Ok(std::borrow::Cow::Owned(pad(data, size, lower, upper)?.0))
    }
}

/// [`crop`] of an image no longer needed: itself when nothing was padded.
fn crop_owned(data: Vec<f32>, padded: &[usize], lower: &[usize], size: &[usize]) -> Vec<f32> {
    if padded == size {
        data
    } else {
        crop(&data, padded, lower, size)
    }
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
    if options.upstream_defects.fix() {
        // ANTS-162: before anything is allocated (`reproduce` attempts it as ANTs does).
        let padded: Vec<usize> = (0..d)
            .map(|i| g.size[i].saturating_add(lower[i]).saturating_add(upper[i]))
            .collect();
        let distance = match &options.mesh {
            BSplineMesh::Distance(x) => Some(*x),
            _ => None,
        };
        check_memory(
            g,
            &control_points,
            order,
            options.iterations.len().max(1),
            &padded,
            distance,
        )?;
    }
    // Memory (random testing, 2026-10-25: larmorx's peak against ANTs'): without padding the
    // images are used as they are, not copied; without a mask, the image of ones is made only
    // to be shrunk; the full-resolution images of the last step are dropped as soon as they
    // have been used.
    let padded_size: Vec<usize> = (0..d).map(|i| g.size[i] + lower[i] + upper[i]).collect();
    let padded_input = pad_or_borrow(&input.data, &g.size, lower, upper)?;
    let padded_mask = match mask {
        Some(m) => Some(pad_or_borrow(&m.data, &g.size, lower, upper)?),
        None => None,
    };
    let padded_weight = match weight {
        Some(w) => Some(pad_or_borrow(&w.data, &g.size, lower, upper)?),
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
    let small_input = shrink_or_borrow(&padded_input[..], &padded_grid, &plan, n_threads)?;
    let small_mask = match &padded_mask {
        Some(m) => shrink_or_borrow(m.as_ref(), &padded_grid, &plan, n_threads)?,
        None => {
            // ANTs' mask of ones on the input's grid, padded with 0.
            let ones = vec![1.0f32; len];
            let padded_ones = pad_or_borrow(&ones, &g.size, lower, upper)?;
            std::borrow::Cow::Owned(shrink(&padded_ones, &padded_grid, &plan, n_threads)?)
        }
    };
    let small_weight = padded_weight
        .as_ref()
        .map(|w| shrink_or_borrow(w.as_ref(), &padded_grid, &plan, n_threads))
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
        // Two float images of the full-resolution grid: the last step below holds three more
        // than the fit does, so the factors' cache never raises the peak.
        factor_budget: Some(
            padded_size
                .iter()
                .product::<usize>()
                .saturating_mul(2 * size_of::<f32>()),
        ),
    };
    let result = n4_bias_field(
        &small_input,
        &plan.output,
        Some(small_mask.as_ref()),
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
    drop(small_input);
    drop(small_mask);
    drop(small_weight);
    // The bias field in place of the log field, then the corrected image.
    let mut bias = log_field;
    let mut corrected = larmorx_core::alloc::filled(bias.len(), 0.0f32)?;
    larmorx_core::parallel::with_threads(n_threads, || {
        bias.par_iter_mut()
            .zip(corrected.par_iter_mut())
            .zip(padded_input.par_iter())
            .with_min_len(1 << 14)
            .for_each(|((l, c), &v)| {
                let b = larmorx_core::math::exp(f64::from(*l)) as f32;
                *c = if almost_equal_f32(b, 0.0) {
                    f32::MAX
                } else {
                    v / b
                };
                *l = b;
            });
    })
    .map_err(FilterError::from)?;
    if let Some(padded_mask) = &padded_mask {
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
    drop(padded_mask);
    drop(padded_weight);
    drop(padded_input);
    let corrected = crop_owned(corrected, &padded_size, lower, &g.size);
    let bias = crop_owned(bias, &padded_size, lower, &g.size);
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
        assert!(
            e.to_string()
                .starts_with("Failed to allocate memory for image")
        );
    }

    /// ANTS-162 (differential random testing, 2026-10-25, `ants-n4:20261025:24`, the case that
    /// filled the machine): a 14×28×23×7 image whose header gives its voxels in metres (4×1×1 m,
    /// TR 3 s) with a spline distance of 20 mm needs 1.55·10¹⁰ control points at the first
    /// fitting level and 2.2·10¹³ at the fourth. By default larmorx stops before allocating,
    /// naming the memory needed and its likely cause; a small image is not affected.
    #[test]
    fn a_lattice_beyond_any_machine_is_refused_before_allocating() {
        use larmorx_io::nifti::itk::{GeometrySource, ItkGeometry};
        let geometry = |size: &[usize], spacing: &[f64]| ItkGeometry {
            ndim: size.len(),
            size: size.to_vec(),
            spacing: spacing.to_vec(),
            origin: vec![0.0; size.len()],
            direction: (0..size.len())
                .map(|i| {
                    (0..size.len())
                        .map(|j| f64::from(u8::from(i == j)))
                        .collect()
                })
                .collect(),
            source: GeometrySource::Default,
            flipped: vec![false; size.len()],
        };
        let size = [14usize, 28, 23, 7];
        let input = AntsImage {
            data: (0..size.iter().product::<usize>())
                .map(|i| (i % 17) as f32 + 1.0)
                .collect(),
            geometry: geometry(&size, &[4000.0, 1000.0, 1000.0, 3.0]),
            meta: Default::default(),
        };
        let options = N4Options {
            shrink_factor: 3,
            mesh: BSplineMesh::Distance(20.0),
            ..N4Options::default()
        };
        // The limit given explicitly: the test depends on no machine's memory (2026-10-26).
        let limit = 64u64 << 30;
        let start = std::time::Instant::now();
        let e = larmorx_core::alloc::with_memory_limit(limit, || {
            n4_bias_field_correction(&input, None, None, &options, 1, None)
        })
        .unwrap_err();
        let message = e.to_string();
        assert!(matches!(e, N4Error::BeyondMemory(_)), "{message}");
        assert!(message.contains("an error (ANTS-162)") || message.contains("An error (ANTS-162)"));
        assert!(
            message.contains("20803 x 10803 x 8803 x 11 control points"),
            "{message}"
        );
        assert!(message.contains("4000 x 1000 x 1000 x 3 mm"), "{message}");
        assert!(message.contains("52000 x 27000 x 22000 x 18"), "{message}");
        assert!(message.contains("metres or micrometres"), "{message}");
        assert!(
            message.contains("64.0 GiB given to this computation"),
            "{message}"
        );
        assert!(start.elapsed().as_secs() < 5, "refused before any work");
        // The same image in millimetres: a few control points, no refusal (the work is tiny).
        let mut small = input.clone();
        small.geometry = geometry(&size, &[4.0, 1.0, 1.0, 3.0]);
        let options = N4Options {
            iterations: vec![2],
            ..options
        };
        assert!(
            larmorx_core::alloc::with_memory_limit(limit, || {
                n4_bias_field_correction(&small, None, None, &options, 1, None)
            })
            .is_ok()
        );
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
