// SPDX-License-Identifier: Apache-2.0
//! ITK's `N4BiasFieldCorrectionImageFilter` (ITK v5.4.5,
//! `Modules/Filtering/BiasCorrection/include/itkN4BiasFieldCorrectionImageFilter.hxx`):
//! Tustison et al.'s N4 (*N4ITK: Improved N3 Bias Correction*, IEEE TMI 29(6), 2010).
//!
//! The filter works on the log of the image. At each iteration it
//! 1. **sharpens** the histogram of the current log image: a triangular Parzen histogram of
//!    `bins` bins, Wiener deconvolution of a Gaussian of `fwhm` (in log intensity) with
//!    vnl's FFT ([`crate::vnl_fft`]), and the conditional expectation `E(u|v)` mapped back
//!    to the voxels;
//! 2. **fits** a B-spline to the residual (log image − sharpened image) at the voxels it uses
//!    ([`crate::bspline::fit`], weighted by the confidence image), and adds the control points
//!    to the total log bias field;
//! 3. evaluates the total log bias field on the image grid and measures convergence: the
//!    coefficient of variation of `exp(old field − new field)` over the voxels it uses.
//!
//! Each fitting level runs until its iteration count or until the convergence value is not
//! above the threshold; then the control-point lattice is refined to twice its resolution
//! (except after the last level). The result is the lattice of the log bias field; ANTs
//! evaluates it at full resolution ([`crate::bspline::evaluate`]).
//!
//! **Voxels used:** those where the mask is not 0 (`UseMaskLabel` off, as ANTs runs it; NaN
//! counts as not 0) and the confidence is above 0. Only those are log-transformed, and only
//! where the value is above 0; the others keep their value.
//!
//! **Precision** is ITK's: `float` images, histogram and parameters, `double` FFTs, the
//! mixed float/double arithmetic of the convergence measure, and `std::log(float)` /
//! `std::exp(float)` as correctly rounded [`larmorx_core::math::logf`] /
//! [`larmorx_core::math::expf`] (glibc's differ in 0.02 % and 0.004 % of inputs:
//! `docs/findings/ants-n4.md`).
//!
//! **Threads:** ITK's B-spline fit accumulates per thread, so ANTs' result depends on its
//! thread count. larmorx computes ITK's single-thread result on any number of threads.

use rayon::prelude::*;

use crate::FilterError;
use crate::bspline::fit::{GridFit, GridFitPlan, gcc_double_axes};
use crate::bspline::{Lattice, evaluate, refine};
use crate::grid::{ItkGrid, vnl_floor_f32};
use crate::vnl_fft::VnlFft1d;
use larmorx_core::math;

/// The filter's parameters (ITK's defaults through [`N4Parameters::itk_defaults`]).
#[derive(Clone, Debug, PartialEq)]
pub struct N4Parameters {
    /// Maximum iterations per fitting level (`SetMaximumNumberOfIterations`); the number of
    /// levels is its length.
    pub iterations: Vec<u32>,
    /// `SetConvergenceThreshold`.
    pub convergence_threshold: f32,
    /// Control points per axis at the first level (`SetNumberOfControlPoints`); each axis
    /// has `control points − spline order` spans.
    pub control_points: Vec<usize>,
    /// `SetSplineOrder`.
    pub spline_order: usize,
    /// `SetBiasFieldFullWidthAtHalfMaximum`.
    pub fwhm: f32,
    /// `SetWienerFilterNoise`.
    pub wiener_noise: f32,
    /// `SetNumberOfHistogramBins`.
    pub bins: u32,
    /// How many leading axes the B-spline fit computes in double precision where ITK's source
    /// says float (the parametric scale `r` and the points' coordinates), as ITK's x86-64
    /// gcc 11 build does: 2 in 2D and 3D ([`gcc_double_axes`]); 0 follows the source.
    pub double_axes: usize,
}

impl N4Parameters {
    /// ITK's defaults for a `dim`-dimensional image: one level of 50 iterations, threshold
    /// 0.001, 4 control points per axis, cubic splines, FWHM 0.15, Wiener noise 0.01, 200
    /// bins.
    pub fn itk_defaults(dim: usize) -> Self {
        N4Parameters {
            iterations: vec![50],
            convergence_threshold: 0.001,
            control_points: vec![4; dim],
            spline_order: 3,
            fwhm: 0.15,
            wiener_noise: 0.01,
            bins: 200,
            double_axes: gcc_double_axes(dim),
        }
    }
}

/// One iteration, as ITK reports it to observers (`IterationEvent`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct N4Iteration {
    /// The fitting level, from 0.
    pub level: usize,
    /// The iteration within the level, from 1.
    pub iteration: u32,
    /// The convergence value after this iteration.
    pub convergence: f32,
}

/// What [`n4_bias_field`] computes.
#[derive(Clone, Debug, PartialEq)]
pub struct N4Result {
    /// The control points of the log bias field (`GetLogBiasFieldControlPointLattice`).
    pub lattice: Lattice,
    /// Every iteration, in order.
    pub iterations: Vec<N4Iteration>,
}

/// The voxels N4 uses: mask not 0 and confidence above 0.
fn included(mask: Option<&[f32]>, confidence: Option<&[f32]>, len: usize) -> Vec<usize> {
    (0..len)
        .filter(|&i| {
            mask.is_none_or(|m| m[i] != 0.0) && confidence.is_none_or(|c| f64::from(c[i]) > 0.0)
        })
        .collect()
}

/// `SharpenImage`: the sharpened log image (0 at the voxels not used).
fn sharpen(
    unsharpened: &[f32],
    voxels: &[usize],
    p: &N4Parameters,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let bins = p.bins as usize;
    // The range, as ITK finds it: a voxel that raises the maximum is not compared with the
    // minimum.
    let mut bin_max = -f32::MAX;
    let mut bin_min = f32::MAX;
    for &i in voxels {
        let v = unsharpened[i];
        if v > bin_max {
            bin_max = v;
        } else if v < bin_min {
            bin_min = v;
        }
    }
    let slope = (bin_max - bin_min) / (p.bins.wrapping_sub(1)) as f32;

    // Triangular Parzen histogram.
    let mut h = vec![0.0f32; bins];
    for &i in voxels {
        let cidx = (unsharpened[i] - bin_min) / slope;
        let idx = vnl_floor_f32(cidx) as u32 as usize;
        let offset = cidx - idx as u32 as f32;
        if offset == 0.0 {
            // ITK writes out of bounds when idx ≥ bins (undefined behaviour); larmorx skips.
            if let Some(hi) = h.get_mut(idx) {
                *hi += 1.0;
            }
        } else if idx < bins.wrapping_sub(1) {
            h[idx] = (f64::from(h[idx]) + (1.0 - f64::from(offset))) as f32;
            h[idx + 1] += offset;
        }
    }

    // Zero-pad to a power of 2.
    let ln2 = std::f64::consts::LN_2;
    let exponent = ((f64::from(math::logf(p.bins as f32)) / ln2).ceil() + 1.0) as f32;
    let padded = (f64::from(2.0f32.powi(exponent as i32)) + 0.5) as u32 as usize;
    let offset = (0.5 * padded.wrapping_sub(bins) as u32 as f64) as u32 as usize;
    let fft = VnlFft1d::new(padded).ok_or_else(|| {
        FilterError::invalid(format!(
            "N4: cannot pad {bins} histogram bins to a power of 2"
        ))
    })?;

    // Interleaved complex vectors.
    let mut vf = vec![0.0f64; 2 * padded];
    for (n, &hn) in h.iter().enumerate() {
        vf[2 * (n + offset)] = f64::from(hn);
    }
    fft.fwd_transform(&mut vf);

    // The Gaussian filter.
    let scaled_fwhm = p.fwhm / slope;
    let exp_factor = (4.0 * ln2 / f64::from(scaled_fwhm * scaled_fwhm)) as f32;
    let scale_factor = (2.0 * (ln2 / std::f64::consts::PI).sqrt() / f64::from(scaled_fwhm)) as f32;
    let mut ff = vec![0.0f64; 2 * padded];
    ff[0] = f64::from(scale_factor);
    let half = (0.5 * padded as f64) as usize;
    for n in 1..=half {
        let nf = n as f32;
        let v = f64::from(scale_factor * math::expf(-(nf * nf) * exp_factor));
        ff[2 * n] = v;
        ff[2 * (padded - n)] = v;
    }
    if padded.is_multiple_of(2) {
        let pf = padded as f32;
        ff[2 * half] = f64::from(scale_factor)
            * math::exp(0.25 * f64::from(-(pf * pf)) * f64::from(exp_factor));
    }
    fft.fwd_transform(&mut ff);

    // Wiener deconvolution: Uf = Vf · Re(conj(Ff) / (conj(Ff)·Ff + noise)). With the
    // imaginary part of the denominator exactly 0, libgcc's complex division gives
    // Re(Ff) / denominator.
    let noise = f64::from(p.wiener_noise);
    let mut u = vec![0.0f64; 2 * padded];
    for n in 0..padded {
        let (a, b) = (ff[2 * n], ff[2 * n + 1]);
        let den = (a * a - (-b) * b) + noise;
        let g = a / den;
        u[2 * n] = vf[2 * n] * g;
        u[2 * n + 1] = vf[2 * n + 1] * g;
    }
    fft.bwd_transform(&mut u);
    for n in 0..padded {
        let re = u[2 * n];
        u[2 * n] = if re < 0.0 { 0.0 } else { re };
        u[2 * n + 1] = 0.0;
    }

    // E(u|v) = (Ff ∗ (bin centre · U)) / (Ff ∗ U).
    let mut numerator = vec![0.0f64; 2 * padded];
    for n in 0..padded {
        let centre = bin_min + (n as f32 - offset as u32 as f32) * slope;
        numerator[2 * n] = f64::from(centre) * u[2 * n];
    }
    let convolve = |x: &mut [f64]| {
        fft.fwd_transform(x);
        for n in 0..padded {
            let (a, b) = (x[2 * n], x[2 * n + 1]);
            let (c, d) = (ff[2 * n], ff[2 * n + 1]);
            x[2 * n] = a * c - b * d;
            x[2 * n + 1] = a * d + b * c;
        }
        fft.bwd_transform(x);
    };
    convolve(&mut numerator);
    let mut denominator = u;
    convolve(&mut denominator);
    let e: Vec<f32> = (offset..offset + bins)
        .map(|n| {
            let den = denominator[2 * n];
            if den != 0.0 {
                (numerator[2 * n] / den) as f32
            } else {
                0.0
            }
        })
        .collect();

    // Map the voxels through E.
    let mut sharpened = vec![0.0f32; unsharpened.len()];
    let mapped: Vec<f32> = larmorx_core::parallel::with_threads(n_threads, || {
        voxels
            .par_iter()
            .with_min_len(4096)
            .map(|&i| {
                let cidx = (unsharpened[i] - bin_min) / slope;
                let idx = vnl_floor_f32(cidx) as u32 as usize;
                if idx < bins.wrapping_sub(1) {
                    e[idx] + (e[idx + 1] - e[idx]) * (cidx - idx as u32 as f32)
                } else {
                    e[bins - 1]
                }
            })
            .collect()
    })?;
    for (&i, v) in voxels.iter().zip(mapped) {
        sharpened[i] = v;
    }
    Ok(sharpened)
}

/// `CalculateConvergenceMeasurement`: the coefficient of variation of `exp(old − new)` over
/// the voxels used, with ITK's running mean and variance in float (some steps in double).
fn convergence(old: &[f32], new: &[f32], voxels: &[usize]) -> f32 {
    let mut mu = 0.0f32;
    let mut sigma = 0.0f32;
    let mut n = 0.0f32;
    for &i in voxels {
        let pixel = math::expf(old[i] - new[i]);
        n = (f64::from(n) + 1.0) as f32;
        if n > 1.0 {
            let d = pixel - mu;
            sigma =
                (f64::from(sigma) + f64::from(d * d) * (f64::from(n) - 1.0) / f64::from(n)) as f32;
        }
        mu = (f64::from(mu) * (1.0 - 1.0 / f64::from(n)) + f64::from(pixel / n)) as f32;
    }
    sigma = (f64::from(sigma) / (f64::from(n) - 1.0)).sqrt() as f32;
    sigma / mu
}

/// Runs N4 on `input` (grid `grid`: the fit's points are the voxels' physical points with an
/// identity direction, as ITK's `UpdateBiasFieldEstimate` makes them), with an optional mask
/// and confidence (weight) image on the same grid. `observer` sees every iteration as it
/// ends. Returns the control points of the log bias field.
pub fn n4_bias_field(
    input: &[f32],
    grid: &ItkGrid,
    mask: Option<&[f32]>,
    confidence: Option<&[f32]>,
    p: &N4Parameters,
    n_threads: usize,
    mut observer: Option<&mut dyn FnMut(&N4Iteration)>,
) -> Result<N4Result, FilterError> {
    let d = grid.dim();
    let len = grid.len();
    if input.len() != len {
        return Err(FilterError::SizeMismatch {
            what: "the image",
            actual: input.len(),
            expected: len,
        });
    }
    if mask.is_some_and(|m| m.len() != len) {
        return Err(FilterError::invalid(
            "If a mask image is specified, its size should be equal to the input image size",
        ));
    }
    if confidence.is_some_and(|c| c.len() != len) {
        return Err(FilterError::invalid(
            "If a confidence image is specified, its size should be equal to the input image size",
        ));
    }
    if p.control_points.len() != d {
        return Err(FilterError::invalid(format!(
            "{} control-point counts for a {d}-dimensional image",
            p.control_points.len()
        )));
    }
    if p.iterations.is_empty() {
        return Err(FilterError::invalid(
            "Number of iteration levels is not equal to the max number of levels.",
        ));
    }
    if p.bins == 0 {
        return Err(FilterError::invalid("N4 needs at least one histogram bin"));
    }
    let voxels = included(mask, confidence, len);
    let weights: Vec<f32> = match confidence {
        Some(c) => voxels.iter().map(|&i| c[i]).collect(),
        None => vec![1.0; voxels.len()],
    };

    // The log of the image at the voxels used (where positive).
    let mut log_input = input.to_vec();
    for &i in &voxels {
        if log_input[i] > 0.0 {
            log_input[i] = math::logf(log_input[i]);
        }
    }

    // The fit's points: each axis's physical coordinates as float, identity direction
    // (`TransformIndexToPhysicalPoint` into a float point: the origin rounded to float, then
    // the index term added in double and rounded again; in double throughout on the
    // `double_axes`).
    let axis_points: Vec<Vec<f32>> = (0..d)
        .map(|a| {
            let o = if a < p.double_axes {
                grid.origin[a]
            } else {
                f64::from(grid.origin[a] as f32)
            };
            (0..grid.size[a])
                .map(|x| (o + grid.spacing[a] * (grid.start[a] + x as i64) as f64) as f32)
                .collect()
        })
        .collect();
    let parametric_origin: Vec<f64> = (0..d)
        .map(|a| grid.origin[a] + grid.spacing[a] * grid.start[a] as f64)
        .collect();

    let mut log_uncorrected = log_input.clone();
    let mut log_bias = vec![0.0f32; len];
    let mut lattice: Option<Lattice> = None;
    let mut log = Vec::new();
    let levels = p.iterations.len();
    for level in 0..levels {
        let mut elapsed: u32 = 0;
        let mut measure = f32::MAX;
        // The lattice size is fixed within a level: prepare the fit once.
        let control_points = lattice
            .as_ref()
            .map_or_else(|| p.control_points.clone(), |l| l.size.clone());
        let fit = GridFit {
            axis_points: &axis_points,
            origin: &parametric_origin,
            spacing: &grid.spacing,
            size: &grid.size,
            control_points: &control_points,
            order: p.spline_order,
            double_axes: p.double_axes,
        };
        let mut plan: Option<GridFitPlan<'_>> = None;
        loop {
            let within = elapsed < p.iterations[level];
            elapsed += 1;
            if !(within && measure > p.convergence_threshold) {
                break;
            }
            let sharpened = sharpen(&log_uncorrected, &voxels, p, n_threads)?;
            let residual: Vec<f32> = voxels
                .iter()
                .map(|&i| log_uncorrected[i] - sharpened[i])
                .collect();
            if plan.is_none() {
                plan = Some(GridFitPlan::new(
                    &fit,
                    &grid.size,
                    &voxels,
                    Some(&weights),
                    n_threads,
                )?);
            }
            let phi = plan
                .as_ref()
                .expect("the plan was just made")
                .fit(&residual, n_threads)?;
            let total = match lattice.take() {
                None => phi,
                Some(mut l) => {
                    for (a, b) in l.data.iter_mut().zip(&phi.data) {
                        *a += *b;
                    }
                    l
                }
            };
            let new_bias = evaluate(&total, p.spline_order, &grid.size, &grid.spacing, n_threads)?;
            lattice = Some(total);
            measure = convergence(&log_bias, &new_bias, &voxels);
            log_bias = new_bias;
            larmorx_core::parallel::with_threads(n_threads, || {
                log_uncorrected
                    .par_iter_mut()
                    .zip(log_input.par_iter().zip(log_bias.par_iter()))
                    .with_min_len(1 << 14)
                    .for_each(|(u, (&li, &b))| *u = li - b);
            })?;
            let it = N4Iteration {
                level,
                iteration: elapsed,
                convergence: measure,
            };
            if let Some(obs) = observer.as_deref_mut() {
                obs(&it);
            }
            log.push(it);
        }
        if level + 1 < levels
            && let Some(l) = lattice.take()
        {
            lattice = Some(refine(&l, p.spline_order)?);
        }
    }
    Ok(N4Result {
        lattice: lattice.unwrap_or_else(|| Lattice::zeros(&p.control_points)),
        iterations: log,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phantom() -> (Vec<f32>, ItkGrid) {
        let size = [20usize, 18, 12];
        let mut data = Vec::new();
        for z in 0..size[2] {
            for y in 0..size[1] {
                for x in 0..size[0] {
                    let bias = 1.0 + 0.02 * x as f32 + 0.01 * y as f32 - 0.015 * z as f32;
                    let tissue = if (x + y + z) % 5 == 0 { 300.0 } else { 200.0 };
                    data.push(tissue * bias + ((x * 7 + y * 3 + z) % 11) as f32);
                }
            }
        }
        let dir = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        (
            data,
            ItkGrid::new(&size, &[2.0, 2.0, 3.0], &[-10.0, 5.0, 2.5], &dir),
        )
    }

    #[test]
    fn runs_and_does_not_depend_on_threads() {
        let (data, grid) = phantom();
        let mut p = N4Parameters::itk_defaults(3);
        p.iterations = vec![5, 4];
        p.convergence_threshold = 0.0;
        let mask: Vec<f32> = (0..data.len())
            .map(|i| f32::from(u8::from(i % 7 != 0)))
            .collect();
        let a = n4_bias_field(&data, &grid, Some(&mask), None, &p, 1, None).unwrap();
        let b = n4_bias_field(&data, &grid, Some(&mask), None, &p, 4, None).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.iterations.len(), 9);
        assert_eq!(a.lattice.size, [5, 5, 5]);
        assert!(a.lattice.data.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn convergence_stops_a_level() {
        let (data, grid) = phantom();
        let mut p = N4Parameters::itk_defaults(3);
        p.iterations = vec![50];
        p.convergence_threshold = 0.5;
        let r = n4_bias_field(&data, &grid, None, None, &p, 2, None).unwrap();
        assert_eq!(r.iterations.len(), 1);
    }
}
