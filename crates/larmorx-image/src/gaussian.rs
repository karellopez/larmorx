// SPDX-License-Identifier: Apache-2.0
//! Recursive Gaussian filtering, as ITK v5.4.5 computes it for `float` images:
//! - `RecursiveGaussianImageFilter` is [`recursive_gaussian`], with the coefficients of
//!   [`RecursiveCoefficients`] (Deriche's fourth-order recursive approximation of a Gaussian
//!   and of its first and second derivatives, on one axis) and the boundary handling of
//!   `RecursiveSeparableImageFilter`;
//! - `SmoothingRecursiveGaussianImageFilter` is [`smoothing_recursive_gaussian`];
//! - `LaplacianRecursiveGaussianImageFilter` is [`laplacian_recursive_gaussian`];
//! - `GradientMagnitudeRecursiveGaussianImageFilter` is
//!   [`gradient_magnitude_recursive_gaussian`].
//!
//! `DiscreteGaussianImageFilter` is in [`crate::discrete_gaussian`].
//!
//! # Precision
//!
//! Each pass along an axis reads a line of `float` voxels into `double`, filters it in double
//! (`NumericTraits<float>::RealType`), and stores `float`. The composite filters keep their
//! intermediate images in `float` (`NumericTraits<float>::FloatType`, and the `float`
//! `InternalRealType` of the Laplacian and gradient filters), so every pass rounds to float,
//! as in ITK. The expressions are evaluated in ITK's order, term by term, without fused
//! multiply-adds; the sines, cosines and exponentials of the coefficients come from
//! `larmorx_core::math` (correctly rounded; ITK calls glibc, which agrees except in about
//! 0.1 % of calls).
//!
//! # Sigma
//!
//! Sigma is in physical units (mm): ITK divides it by the spacing of the axis it filters.

use larmorx_core::math;
use larmorx_core::parallel;
use rayon::prelude::*;

use crate::FilterError;
use crate::lines::{LineFilter, filter_lines};
use crate::volume::VolumeRef;

const CHUNK: usize = 1 << 16;

/// The order of the derivative a recursive Gaussian approximates (ITK's `GaussianOrderEnum`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GaussianOrder {
    /// Smoothing (convolution with a Gaussian).
    #[default]
    Zero,
    /// Convolution with the first derivative of a Gaussian.
    First,
    /// Convolution with the second derivative of a Gaussian.
    Second,
}

/// The coefficients of a recursive Gaussian filter along one axis
/// (`RecursiveGaussianImageFilter::SetUp`): causal (`n*`), recursive (`d*`), anti-causal
/// (`m*`), and the boundary coefficients that extend the first and last value to infinity
/// (`bn*`, `bm*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecursiveCoefficients {
    pub n: [f64; 4],
    pub d: [f64; 4],
    pub m: [f64; 4],
    pub bn: [f64; 4],
    pub bm: [f64; 4],
}

/// Deriche's coefficients, as `static_cast<double>` of ITK's literals: `(a1, b1, a2, b2)` for
/// orders 0, 1 and 2, and the shared `w1, l1, w2, l2`.
const A1: [f64; 3] = [1.3530, -0.6724, -1.3563];
const B1: [f64; 3] = [1.8151, -3.4327, 5.2318];
const A2: [f64; 3] = [-0.3531, 0.6724, 0.3446];
const B2: [f64; 3] = [0.0902, 0.6100, -2.2355];
const W1: f64 = 0.6681;
const L1: f64 = -1.3932;
const W2: f64 = 2.0787;
const L2: f64 = -1.3732;

/// `ComputeNCoefficients`: `(n0, n1, n2, n3, sn, dn, en)`.
fn n_coefficients(sigmad: f64, k: usize) -> ([f64; 4], f64, f64, f64) {
    let (a1, b1, a2, b2) = (A1[k], B1[k], A2[k], B2[k]);
    let sin1 = math::sin(W1 / sigmad);
    let sin2 = math::sin(W2 / sigmad);
    let cos1 = math::cos(W1 / sigmad);
    let cos2 = math::cos(W2 / sigmad);
    let exp1 = math::exp(L1 / sigmad);
    let exp2 = math::exp(L2 / sigmad);

    let n0 = a1 + a2;
    let mut n1 = exp2 * (b2 * sin2 - (a2 + 2.0 * a1) * cos2);
    n1 += exp1 * (b1 * sin1 - (a1 + 2.0 * a2) * cos1);
    let mut n2 = (a1 + a2) * cos2 * cos1;
    n2 -= b1 * cos2 * sin1 + b2 * cos1 * sin2;
    n2 *= 2.0 * exp1 * exp2;
    n2 += a2 * exp1 * exp1 + a1 * exp2 * exp2;
    let mut n3 = exp2 * exp1 * exp1 * (b2 * sin2 - a2 * cos2);
    n3 += exp1 * exp2 * exp2 * (b1 * sin1 - a1 * cos1);

    let sn = n0 + n1 + n2 + n3;
    let dn = n1 + 2.0 * n2 + 3.0 * n3;
    let en = n1 + 4.0 * n2 + 9.0 * n3;
    ([n0, n1, n2, n3], sn, dn, en)
}

/// `ComputeDCoefficients`: `(d1, d2, d3, d4, sd, dd, ed)`.
fn d_coefficients(sigmad: f64) -> ([f64; 4], f64, f64, f64) {
    let cos1 = math::cos(W1 / sigmad);
    let cos2 = math::cos(W2 / sigmad);
    let exp1 = math::exp(L1 / sigmad);
    let exp2 = math::exp(L2 / sigmad);

    let d4 = exp1 * exp1 * exp2 * exp2;
    let mut d3 = -2.0 * cos1 * exp1 * exp2 * exp2;
    d3 += -2.0 * cos2 * exp2 * exp1 * exp1;
    let mut d2 = 4.0 * cos2 * cos1 * exp1 * exp2;
    d2 += exp1 * exp1 + exp2 * exp2;
    let d1 = -2.0 * (exp2 * cos2 + exp1 * cos1);

    let sd = 1.0 + d1 + d2 + d3 + d4;
    let dd = d1 + 2.0 * d2 + 3.0 * d3 + 4.0 * d4;
    let ed = d1 + 4.0 * d2 + 9.0 * d3 + 16.0 * d4;
    ([d1, d2, d3, d4], sd, dd, ed)
}

impl RecursiveCoefficients {
    /// The coefficients for `sigma` (physical units) on an axis of `spacing`
    /// (`RecursiveGaussianImageFilter::SetUp`, with its checks).
    ///
    /// `normalize_across_scale` multiplies the first derivative by `sigma` and the second by
    /// `sigma²` (ITK's `NormalizeAcrossScale`); it does not change the smoothing. A negative
    /// spacing negates the first derivative.
    pub fn new(
        sigma: f64,
        spacing: f64,
        order: GaussianOrder,
        normalize_across_scale: bool,
    ) -> Result<Self, FilterError> {
        // `VerifyPreconditions` (NaN passes, as in ITK).
        if sigma <= 0.0 {
            return Err(FilterError::invalid("Sigma must be greater than zero."));
        }
        let mut spacing = spacing;
        let mut direction = 1.0;
        if spacing < 0.0 {
            direction = -1.0;
            spacing = -spacing;
        }
        if spacing < 1e-8 {
            return Err(FilterError::invalid(format!(
                "The spacing {spacing}is suspiciosly small in this image"
            )));
        }
        let sigmad = sigma / spacing;
        let mut across_scale_normalization = 1.0;
        let (d, sd, dd, ed) = d_coefficients(sigmad);
        let (n, symmetric) = match order {
            GaussianOrder::Zero => {
                let (n, sn, _, _) = n_coefficients(sigmad, 0);
                let alpha0 = 2.0 * sn / sd - n[0];
                (n.map(|v| v * (across_scale_normalization / alpha0)), true)
            }
            GaussianOrder::First => {
                if normalize_across_scale {
                    across_scale_normalization = sigma;
                }
                let (n, sn, dn, _) = n_coefficients(sigmad, 1);
                let mut alpha1 = 2.0 * (sn * dd - dn * sd) / (sd * sd);
                alpha1 *= direction;
                (n.map(|v| v * (across_scale_normalization / alpha1)), false)
            }
            GaussianOrder::Second => {
                if normalize_across_scale {
                    across_scale_normalization = sigma * sigma;
                }
                let (n_0, sn0, dn0, en0) = n_coefficients(sigmad, 0);
                let (n_2, sn2, dn2, en2) = n_coefficients(sigmad, 2);
                let beta = -(2.0 * sn2 - sd * n_2[0]) / (2.0 * sn0 - sd * n_0[0]);
                let n: [f64; 4] = std::array::from_fn(|i| n_2[i] + beta * n_0[i]);
                let sn = sn2 + beta * sn0;
                let dn = dn2 + beta * dn0;
                let en = en2 + beta * en0;
                let mut alpha2 =
                    en * sd * sd - ed * sn * sd - 2.0 * dn * dd * sd + 2.0 * dd * dd * sn;
                alpha2 /= sd * sd * sd;
                (n.map(|v| v * (across_scale_normalization / alpha2)), true)
            }
        };
        // `ComputeRemainingCoefficients`.
        let m = if symmetric {
            [
                n[1] - d[0] * n[0],
                n[2] - d[1] * n[0],
                n[3] - d[2] * n[0],
                -d[3] * n[0],
            ]
        } else {
            [
                -(n[1] - d[0] * n[0]),
                -(n[2] - d[1] * n[0]),
                -(n[3] - d[2] * n[0]),
                d[3] * n[0],
            ]
        };
        let sn = n[0] + n[1] + n[2] + n[3];
        let sm = m[0] + m[1] + m[2] + m[3];
        let sd = 1.0 + d[0] + d[1] + d[2] + d[3];
        let bn = d.map(|di| di * sn / sd);
        let bm = d.map(|di| di * sm / sd);
        Ok(RecursiveCoefficients { n, d, m, bn, bm })
    }

    /// Filters `w` lines of `n ≥ 4` values (`data[i * w + l]`) into `outs`, with `scratch`
    /// for the anti-causal part: `RecursiveSeparableImageFilter::FilterDataArray`, term by
    /// term.
    pub fn filter_lanes(
        &self,
        data: &[f64],
        outs: &mut [f64],
        scratch: &mut [f64],
        n: usize,
        w: usize,
    ) {
        assert!(
            n >= 4,
            "recursive filtering needs at least 4 values per line"
        );
        let (cn, cd, cm, bn, bm) = (&self.n, &self.d, &self.m, &self.bn, &self.bm);
        let at = |i: usize| i * w;

        // Causal pass; the first value is taken to extend to infinity before the line.
        for l in 0..w {
            let v1 = data[l];
            let x1 = data[at(1) + l];
            let x2 = data[at(2) + l];
            let x3 = data[at(3) + l];
            let mut s0 = v1 * cn[0] + v1 * cn[1] + v1 * cn[2] + v1 * cn[3];
            let mut s1 = x1 * cn[0] + v1 * cn[1] + v1 * cn[2] + v1 * cn[3];
            let mut s2 = x2 * cn[0] + x1 * cn[1] + v1 * cn[2] + v1 * cn[3];
            let mut s3 = x3 * cn[0] + x2 * cn[1] + x1 * cn[2] + v1 * cn[3];
            s0 -= v1 * bn[0] + v1 * bn[1] + v1 * bn[2] + v1 * bn[3];
            s1 -= s0 * cd[0] + v1 * bn[1] + v1 * bn[2] + v1 * bn[3];
            s2 -= s1 * cd[0] + s0 * cd[1] + v1 * bn[2] + v1 * bn[3];
            s3 -= s2 * cd[0] + s1 * cd[1] + s0 * cd[2] + v1 * bn[3];
            outs[l] = s0;
            outs[at(1) + l] = s1;
            outs[at(2) + l] = s2;
            outs[at(3) + l] = s3;
        }
        let (n0, n1, n2, n3) = (cn[0], cn[1], cn[2], cn[3]);
        let (d1, d2, d3, d4) = (cd[0], cd[1], cd[2], cd[3]);
        // The rows as equal-length slices, so that the loops over the lanes compile without
        // bounds checks; the lanes are independent, so they run side by side.
        let row = |s: &[f64], i: usize| -> std::ops::Range<usize> {
            debug_assert!(s.len() >= (i + 1) * w);
            i * w..(i + 1) * w
        };
        for i in 4..n {
            let (done, rest) = outs.split_at_mut(at(i));
            let o = &mut rest[..w];
            let (x0, x1, x2, x3) = (
                &data[row(data, i)],
                &data[row(data, i - 1)],
                &data[row(data, i - 2)],
                &data[row(data, i - 3)],
            );
            let (y1, y2, y3, y4) = (
                &done[row(done, i - 1)],
                &done[row(done, i - 2)],
                &done[row(done, i - 3)],
                &done[row(done, i - 4)],
            );
            let (x0, x1, x2, x3) = (&x0[..w], &x1[..w], &x2[..w], &x3[..w]);
            let (y1, y2, y3, y4) = (&y1[..w], &y2[..w], &y3[..w], &y4[..w]);
            for l in 0..w {
                let mut s = x0[l] * n0 + x1[l] * n1 + x2[l] * n2 + x3[l] * n3;
                s -= y1[l] * d1 + y2[l] * d2 + y3[l] * d3 + y4[l] * d4;
                o[l] = s;
            }
        }

        // Anti-causal pass; the last value is taken to extend to infinity after the line.
        for l in 0..w {
            let v2 = data[at(n - 1) + l];
            let y1 = data[at(n - 1) + l];
            let y2 = data[at(n - 2) + l];
            let y3 = data[at(n - 3) + l];
            let mut s1 = v2 * cm[0] + v2 * cm[1] + v2 * cm[2] + v2 * cm[3];
            let mut s2 = y1 * cm[0] + v2 * cm[1] + v2 * cm[2] + v2 * cm[3];
            let mut s3 = y2 * cm[0] + y1 * cm[1] + v2 * cm[2] + v2 * cm[3];
            let mut s4 = y3 * cm[0] + y2 * cm[1] + y1 * cm[2] + v2 * cm[3];
            s1 -= v2 * bm[0] + v2 * bm[1] + v2 * bm[2] + v2 * bm[3];
            s2 -= s1 * cd[0] + v2 * bm[1] + v2 * bm[2] + v2 * bm[3];
            s3 -= s2 * cd[0] + s1 * cd[1] + v2 * bm[2] + v2 * bm[3];
            s4 -= s3 * cd[0] + s2 * cd[1] + s1 * cd[2] + v2 * bm[3];
            scratch[at(n - 1) + l] = s1;
            scratch[at(n - 2) + l] = s2;
            scratch[at(n - 3) + l] = s3;
            scratch[at(n - 4) + l] = s4;
        }
        let (m1, m2, m3, m4) = (cm[0], cm[1], cm[2], cm[3]);
        for i in (1..=n - 4).rev() {
            let (head, done) = scratch.split_at_mut(at(i));
            let o = &mut head[at(i - 1)..at(i)];
            let (x0, x1, x2, x3) = (
                &data[row(data, i)],
                &data[row(data, i + 1)],
                &data[row(data, i + 2)],
                &data[row(data, i + 3)],
            );
            let (y1, y2, y3, y4) = (
                &done[row(done, 0)],
                &done[row(done, 1)],
                &done[row(done, 2)],
                &done[row(done, 3)],
            );
            let o = &mut o[..w];
            let (x0, x1, x2, x3) = (&x0[..w], &x1[..w], &x2[..w], &x3[..w]);
            let (y1, y2, y3, y4) = (&y1[..w], &y2[..w], &y3[..w], &y4[..w]);
            for l in 0..w {
                let mut s = x0[l] * m1 + x1[l] * m2 + x2[l] * m3 + x3[l] * m4;
                s -= y1[l] * d1 + y2[l] * d2 + y3[l] * d3 + y4[l] * d4;
                o[l] = s;
            }
        }

        for (o, &s) in outs[..n * w].iter_mut().zip(scratch[..n * w].iter()) {
            *o += s;
        }
    }
}

impl LineFilter for RecursiveCoefficients {
    fn filter(&self, input: &[f64], output: &mut [f64], scratch: &mut [f64], n: usize, w: usize) {
        self.filter_lanes(input, output, scratch, n, w);
    }
}

/// ITK's message for an axis with fewer than four voxels (`RecursiveSeparableImageFilter`).
fn too_short(axis: usize) -> FilterError {
    FilterError::invalid(format!(
        "The number of pixels along direction {axis} is less than 4. This filter requires a \
         minimum of four pixels along the dimension to be processed."
    ))
}

/// Checks the volume and returns its axis count.
fn check(input: &VolumeRef<'_, f32>) -> Result<usize, FilterError> {
    let d = input.dim();
    if d == 0 {
        return Err(FilterError::invalid("the image has no dimensions"));
    }
    Ok(d)
}

/// One recursive pass along `axis`, in place on `data` (`float → float`), an image with the
/// size and spacing of `shape`.
fn pass(
    data: &mut [f32],
    shape: VolumeRef<'_, f32>,
    axis: usize,
    sigma: f64,
    order: GaussianOrder,
    normalize_across_scale: bool,
    n_threads: usize,
) -> Result<(), FilterError> {
    let coefficients =
        RecursiveCoefficients::new(sigma, shape.spacing[axis], order, normalize_across_scale)?;
    if shape.size[axis] < 4 {
        return Err(too_short(axis));
    }
    filter_lines(data, shape.size, axis, &coefficients, n_threads)
}

/// `RecursiveGaussianImageFilter<float, float>` along `axis`: a Gaussian of `sigma`
/// (physical units), or its first or second derivative, applied to every line along that
/// axis. Needs at least 4 voxels along `axis`.
pub fn recursive_gaussian(
    input: VolumeRef<'_, f32>,
    axis: usize,
    sigma: f64,
    order: GaussianOrder,
    normalize_across_scale: bool,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let d = check(&input)?;
    if axis >= d {
        return Err(FilterError::invalid(
            "Direction selected for filtering is greater than ImageDimension",
        ));
    }
    let mut out = input.data.to_vec();
    pass(
        &mut out,
        input,
        axis,
        sigma,
        order,
        normalize_across_scale,
        n_threads,
    )?;
    Ok(out)
}

/// `SmoothingRecursiveGaussianImageFilter<float, float>`: Gaussian smoothing with one sigma
/// per axis (physical units), as zero-order recursive passes: the **last axis first**, then
/// the others in order, each rounding to `float`. Every axis needs at least 4 voxels.
///
/// `normalize_across_scale` is accepted for completeness; it does not change zero-order
/// filtering.
pub fn smoothing_recursive_gaussian(
    input: VolumeRef<'_, f32>,
    sigma: &[f64],
    normalize_across_scale: bool,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let d = check(&input)?;
    if sigma.len() != d {
        return Err(FilterError::invalid(format!(
            "{} sigmas for a {d}-dimensional image",
            sigma.len()
        )));
    }
    // `VerifyPreconditions` of the internal filters, then `GenerateData`'s size check.
    let order = std::iter::once(d - 1).chain(0..d - 1);
    for axis in order.clone() {
        RecursiveCoefficients::new(
            sigma[axis],
            input.spacing[axis],
            GaussianOrder::Zero,
            normalize_across_scale,
        )?;
    }
    for (axis, &n) in input.size.iter().enumerate() {
        if n < 4 {
            return Err(FilterError::invalid(format!(
                "The number of pixels along dimension {axis} is less than 4. This filter \
                 requires a minimum of four pixels along the dimension to be processed."
            )));
        }
    }
    let mut out = input.data.to_vec();
    for axis in order {
        pass(
            &mut out,
            input,
            axis,
            sigma[axis],
            GaussianOrder::Zero,
            normalize_across_scale,
            n_threads,
        )?;
    }
    Ok(out)
}

/// For each axis `dim`: the derivative of order `order` along `dim`, then smoothing along the
/// other axes in increasing order, each pass `float → float`; `combine(dim, cumulative,
/// filtered)` folds the result into the cumulative `float` image (which starts at 0).
fn derivative_sum(
    input: &VolumeRef<'_, f32>,
    sigma: f64,
    order: GaussianOrder,
    normalize_across_scale: bool,
    n_threads: usize,
    combine: impl Fn(usize, f32, f32) -> f32 + Sync,
) -> Result<Vec<f32>, FilterError> {
    let d = check(input)?;
    let mut cumulative = vec![0.0f32; input.len()];
    let mut work = vec![0.0f32; input.len()];
    for dim in 0..d {
        work.copy_from_slice(input.data);
        pass(
            &mut work,
            *input,
            dim,
            sigma,
            order,
            normalize_across_scale,
            n_threads,
        )?;
        for axis in (0..d).filter(|&a| a != dim) {
            pass(
                &mut work,
                *input,
                axis,
                sigma,
                GaussianOrder::Zero,
                normalize_across_scale,
                n_threads,
            )?;
        }
        parallel::with_threads(n_threads, || {
            cumulative
                .par_iter_mut()
                .zip(work.par_iter())
                .with_min_len(CHUNK)
                .for_each(|(a, &b)| *a = combine(dim, *a, b));
        })?;
    }
    Ok(cumulative)
}

/// `LaplacianRecursiveGaussianImageFilter<float, float>`: the sum over the axes of the second
/// derivative along the axis (smoothed along the others), each divided by the squared
/// spacing: `cumulative = float(cumulative + b · (1 / spacing²))`.
///
/// Every axis needs at least 4 voxels.
pub fn laplacian_recursive_gaussian(
    input: VolumeRef<'_, f32>,
    sigma: f64,
    normalize_across_scale: bool,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let inverse: Vec<f64> = input.spacing.iter().map(|&s| 1.0 / (s * s)).collect();
    derivative_sum(
        &input,
        sigma,
        GaussianOrder::Second,
        normalize_across_scale,
        n_threads,
        |dim, a, b| (f64::from(a) + f64::from(b) * inverse[dim]) as f32,
    )
}

/// `GradientMagnitudeRecursiveGaussianImageFilter<float, float>`: the square root of the sum
/// over the axes of the squared first derivative along the axis (smoothed along the others)
/// divided by the spacing: `cumulative = float(cumulative + (b / spacing)²)`, then
/// `float(sqrt(cumulative))`.
///
/// Every axis needs at least 4 voxels.
pub fn gradient_magnitude_recursive_gaussian(
    input: VolumeRef<'_, f32>,
    sigma: f64,
    normalize_across_scale: bool,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let spacing = input.spacing.to_vec();
    let mut out = derivative_sum(
        &input,
        sigma,
        GaussianOrder::First,
        normalize_across_scale,
        n_threads,
        |dim, a, b| {
            let g = f64::from(b) / spacing[dim];
            (f64::from(a) + g * g) as f32
        },
    )?;
    parallel::with_threads(n_threads, || {
        out.par_iter_mut()
            .with_min_len(CHUNK)
            .for_each(|v| *v = f64::from(*v).sqrt() as f32);
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A direct transcription of `FilterDataArray` for one line.
    fn filter_line(c: &RecursiveCoefficients, data: &[f64]) -> Vec<f64> {
        let ln = data.len();
        let mut s1 = vec![0.0; ln];
        let mut s2 = vec![0.0; ln];
        let v1 = data[0];
        let e = |a1: f64, b1: f64, a2: f64, b2: f64, a3: f64, b3: f64, a4: f64, b4: f64| {
            a1 * b1 + a2 * b2 + a3 * b3 + a4 * b4
        };
        let (n, d, m, bn, bm) = (c.n, c.d, c.m, c.bn, c.bm);
        s1[0] = e(v1, n[0], v1, n[1], v1, n[2], v1, n[3]);
        s1[1] = e(data[1], n[0], v1, n[1], v1, n[2], v1, n[3]);
        s1[2] = e(data[2], n[0], data[1], n[1], v1, n[2], v1, n[3]);
        s1[3] = e(data[3], n[0], data[2], n[1], data[1], n[2], v1, n[3]);
        s1[0] -= e(v1, bn[0], v1, bn[1], v1, bn[2], v1, bn[3]);
        s1[1] -= e(s1[0], d[0], v1, bn[1], v1, bn[2], v1, bn[3]);
        s1[2] -= e(s1[1], d[0], s1[0], d[1], v1, bn[2], v1, bn[3]);
        s1[3] -= e(s1[2], d[0], s1[1], d[1], s1[0], d[2], v1, bn[3]);
        for i in 4..ln {
            s1[i] = e(
                data[i],
                n[0],
                data[i - 1],
                n[1],
                data[i - 2],
                n[2],
                data[i - 3],
                n[3],
            );
            s1[i] -= e(
                s1[i - 1],
                d[0],
                s1[i - 2],
                d[1],
                s1[i - 3],
                d[2],
                s1[i - 4],
                d[3],
            );
        }
        let v2 = data[ln - 1];
        s2[ln - 1] = e(v2, m[0], v2, m[1], v2, m[2], v2, m[3]);
        s2[ln - 2] = e(data[ln - 1], m[0], v2, m[1], v2, m[2], v2, m[3]);
        s2[ln - 3] = e(data[ln - 2], m[0], data[ln - 1], m[1], v2, m[2], v2, m[3]);
        s2[ln - 4] = e(
            data[ln - 3],
            m[0],
            data[ln - 2],
            m[1],
            data[ln - 1],
            m[2],
            v2,
            m[3],
        );
        s2[ln - 1] -= e(v2, bm[0], v2, bm[1], v2, bm[2], v2, bm[3]);
        s2[ln - 2] -= e(s2[ln - 1], d[0], v2, bm[1], v2, bm[2], v2, bm[3]);
        s2[ln - 3] -= e(s2[ln - 2], d[0], s2[ln - 1], d[1], v2, bm[2], v2, bm[3]);
        s2[ln - 4] -= e(
            s2[ln - 3],
            d[0],
            s2[ln - 2],
            d[1],
            s2[ln - 1],
            d[2],
            v2,
            bm[3],
        );
        let mut i = ln - 4;
        while i > 0 {
            s2[i - 1] = e(
                data[i],
                m[0],
                data[i + 1],
                m[1],
                data[i + 2],
                m[2],
                data[i + 3],
                m[3],
            );
            s2[i - 1] -= e(
                s2[i],
                d[0],
                s2[i + 1],
                d[1],
                s2[i + 2],
                d[2],
                s2[i + 3],
                d[3],
            );
            i -= 1;
        }
        s1.iter().zip(&s2).map(|(a, b)| a + b).collect()
    }

    fn phantom(size: &[usize]) -> Vec<f32> {
        let len: usize = size.iter().product();
        (0..len)
            .map(|i| {
                let x = i as f64;
                (50.0 + 30.0 * (x * 0.37).sin() + 10.0 * (x * 0.011).cos()) as f32
            })
            .collect()
    }

    #[test]
    fn lanes_match_the_scalar_transcription() {
        for order in [
            GaussianOrder::Zero,
            GaussianOrder::First,
            GaussianOrder::Second,
        ] {
            let c = RecursiveCoefficients::new(1.5, 1.2, order, false).unwrap();
            for n in [4usize, 5, 9, 40] {
                let w = 3;
                let data: Vec<f64> = (0..n * w)
                    .map(|i| ((i * 7919) % 97) as f64 - 30.0)
                    .collect();
                let mut out = vec![0.0; n * w];
                let mut scratch = vec![0.0; n * w];
                c.filter_lanes(&data, &mut out, &mut scratch, n, w);
                for l in 0..w {
                    let line: Vec<f64> = (0..n).map(|i| data[i * w + l]).collect();
                    let expected = filter_line(&c, &line);
                    let got: Vec<f64> = (0..n).map(|i| out[i * w + l]).collect();
                    assert_eq!(got, expected, "{order:?} n {n} lane {l}");
                }
            }
        }
    }

    #[test]
    fn smoothing_preserves_constants_and_approximates_a_gaussian() {
        let size = [20usize, 12, 9];
        let spacing = [1.0, 1.5, 2.0];
        let data = vec![7.0f32; 20 * 12 * 9];
        let out = smoothing_recursive_gaussian(
            VolumeRef::new(&data, &size, &spacing),
            &[2.0, 2.0, 2.0],
            false,
            2,
        )
        .unwrap();
        assert!(
            out.iter().all(|&v| (v - 7.0).abs() < 1e-4),
            "{:?}",
            &out[..5]
        );

        // An impulse in a long line: the response is close to a Gaussian of sigma 3 voxels.
        let n = 101;
        let mut line = vec![0.0f32; n];
        line[50] = 1.0;
        let out = recursive_gaussian(
            VolumeRef::new(&line, &[n], &[1.0]),
            0,
            3.0,
            GaussianOrder::Zero,
            false,
            1,
        )
        .unwrap();
        let g = |x: f64| (-(x * x) / 18.0).exp() / (3.0 * (2.0 * std::f64::consts::PI).sqrt());
        for (i, &v) in out.iter().enumerate() {
            assert!((f64::from(v) - g(i as f64 - 50.0)).abs() < 2e-3, "{i}: {v}");
        }
        let sum: f64 = out.iter().map(|&v| f64::from(v)).sum();
        assert!((sum - 1.0).abs() < 1e-3, "{sum}");
    }

    #[test]
    fn derivatives_of_polynomials() {
        // f(x) = x² (x in mm, spacing 0.1 mm) along a long line. The single-axis filter
        // differentiates per voxel: the second derivative is 2·0.1² = 0.02, the first
        // 2·x·0.1 (away from the ends). The Laplacian divides by the squared spacing.
        let n = 200;
        let line: Vec<f32> = (0..n)
            .map(|i| ((i as f64 - 100.0) * 0.1).powi(2) as f32)
            .collect();
        let size = [n];
        let v = VolumeRef::new(&line, &size, &[0.1]);
        let second = recursive_gaussian(v, 0, 0.5, GaussianOrder::Second, false, 1).unwrap();
        assert!((second[100] - 0.02).abs() < 2e-4, "{}", second[100]);
        let first = recursive_gaussian(v, 0, 0.5, GaussianOrder::First, false, 1).unwrap();
        assert!((first[120] - 0.4).abs() < 4e-3, "{}", first[120]);
        let laplacian = laplacian_recursive_gaussian(v, 0.5, false, 1).unwrap();
        assert!((laplacian[100] - 2.0).abs() < 0.02, "{}", laplacian[100]);
        let gradient = gradient_magnitude_recursive_gaussian(v, 0.5, false, 1).unwrap();
        assert!((gradient[120] - 4.0).abs() < 0.04, "{}", gradient[120]);
    }

    #[test]
    fn composite_filters_do_not_depend_on_threads_and_check_sizes() {
        let size = [17usize, 9, 6, 5];
        let spacing = [1.2, 1.0, 2.5, 2.0];
        let data = phantom(&size);
        let v = VolumeRef::new(&data, &size, &spacing);
        let a = laplacian_recursive_gaussian(v, 1.5, false, 1).unwrap();
        assert_eq!(a, laplacian_recursive_gaussian(v, 1.5, false, 4).unwrap());
        let g = gradient_magnitude_recursive_gaussian(v, 1.0, false, 1).unwrap();
        assert_eq!(
            g,
            gradient_magnitude_recursive_gaussian(v, 1.0, false, 3).unwrap()
        );
        assert!(g.iter().all(|&x| x >= 0.0));
        let s = smoothing_recursive_gaussian(v, &[1.0, 2.0, 3.0, 4.0], false, 1).unwrap();
        assert_eq!(
            s,
            smoothing_recursive_gaussian(v, &[1.0, 2.0, 3.0, 4.0], false, 5).unwrap()
        );

        let short = [17usize, 3];
        let d2 = phantom(&short);
        let v2 = VolumeRef::new(&d2, &short, &[1.0, 1.0]);
        assert!(smoothing_recursive_gaussian(v2, &[1.0, 1.0], false, 1).is_err());
        assert!(recursive_gaussian(v2, 0, 1.0, GaussianOrder::Zero, false, 1).is_ok());
        assert!(recursive_gaussian(v2, 1, 1.0, GaussianOrder::Zero, false, 1).is_err());
        assert!(recursive_gaussian(v2, 0, 0.0, GaussianOrder::Zero, false, 1).is_err());
    }
}
