// SPDX-License-Identifier: Apache-2.0
//! `ResampleImageFilter<Image<R, D>, Image<R, D>, R>` with a linear transform (ITK v5.4.5,
//! `itkResampleImageFilter.hxx`, `LinearThreadedGenerateData`): the warped images
//! `antsRegistration` writes in any dimension.
//!
//! For a linear transform ITK maps only two points per scan line (along the first axis): the
//! line's first voxel and the voxel one past its end, each `TransformIndexToPhysicalPoint` in
//! double, mapped by the transform in `R`, and turned into a continuous index of the input in
//! `R`. Each voxel of the line is then `start + α·(end − start)` with `α = (i − first) / size`
//! in double, stored in `R`. Inside the input's buffer the interpolated value is cast with
//! bounds checking (clamped to the float range for float pixels); outside, the default value.

use larmorx_core::IndexFloor;
use larmorx_interp::nd::{InterpolationNd, InterpolatorNd};
use rayon::prelude::*;

use crate::interp::at_cindex_with;
use crate::transform::Composite;
use crate::{Image, Real, Space};

/// The interpolators of the warped images (`make_interpolator_snip.tmpl`), with the
/// coordinate type `R`.
#[derive(Clone, Debug, PartialEq)]
pub enum Interpolator<const D: usize> {
    /// `LinearInterpolateImageFunction<Image<R>, R>`.
    Linear,
    /// `NearestNeighborInterpolateImageFunction<Image<R>, R>`: the voxel at the continuous
    /// index rounded half up on each axis.
    NearestNeighbor,
    /// `LabelImageGenericInterpolateImageFunction<Image<R>, LinearInterpolateImageFunction, R>`:
    /// for each label of the image (ascending), the linear interpolation of its indicator; the
    /// label with the strictly highest value wins, from label 0 at 0.
    GenericLabel,
    /// B-spline, Gaussian, label-Gaussian or windowed sinc ([`larmorx_interp::nd`]).
    Other(InterpolationNd<D>),
}

/// `CastPixelWithBoundsChecking`: the interpolator's double value as is for double pixels,
/// clamped to the float range for float pixels.
fn cast<R: Real>(v: f64) -> R {
    if R::PRECISION == larmorx_core::Precision::Double {
        return R::from_f64(v);
    }
    let max = R::MAX.to_f64();
    if v <= -max {
        -R::MAX
    } else if v >= max {
        R::MAX
    } else {
        R::from_f64(v)
    }
}

/// The generic-label interpolation at `c`. Only the labels among the voxels the linear
/// interpolator reads (the corners of the cell, clamped to the image) can have a non-zero
/// value and win, so only those are evaluated, in ascending order.
fn generic_label<R: Real, const D: usize>(
    image: &Image<R, D>,
    c: &[R; D],
    floor: &IndexFloor,
) -> f64 {
    let space = &image.space;
    let base: [i64; D] = std::array::from_fn(|d| c[d].itk_floor().max(space.start[d]));
    let mut labels: Vec<f64> = Vec::with_capacity(1 << D);
    for corner in 0..(1usize << D) {
        let idx: [i64; D] = std::array::from_fn(|d| {
            let last = space.start[d] + space.size[d] as i64 - 1;
            (base[d] + ((corner >> d) & 1) as i64).min(last)
        });
        labels.push(image.data[space.offset(&idx)].to_f64());
    }
    labels.sort_by(|a, b| a.total_cmp(b));
    labels.dedup_by(|a, b| a.to_bits() == b.to_bits());
    let (mut best_value, mut best_label) = (0.0f64, 0.0f64);
    for label in labels {
        let value = at_cindex_with(space, c, floor, |o| {
            if image.data[o].to_f64() == label {
                1.0
            } else {
                0.0
            }
        });
        if value > best_value {
            best_value = value;
            best_label = label;
        }
    }
    best_label
}

/// `ResampleImageFilter::NonlinearThreadedGenerateData` (a transform that is not linear: a
/// composite holding a displacement field): for each output voxel, its physical point (double)
/// mapped by `transform` (a point of `R`), turned into a continuous index of the input in `R`;
/// inside the input's buffer the value `evaluate` gives there, else `default`. In parallel,
/// in output order.
pub fn resample_nonlinear_with<R: Real, const D: usize, T: Copy + Send + Sync>(
    input: &Space<D>,
    output: &Space<D>,
    transform: &Composite<R, D>,
    default: T,
    evaluate: impl Fn(&[R; D]) -> T + Sync,
) -> Vec<T> {
    (0..output.len())
        .into_par_iter()
        .map(|o| {
            let idx = output.index_of(o);
            let p = output.index_to_point(&idx);
            let pr: [R; D] = std::array::from_fn(|k| R::from_f64(p[k]));
            let q = transform.transform_point(&pr);
            let qf: [f64; D] = std::array::from_fn(|k| q[k].to_f64());
            let c = input.point_to_cindex::<R>(&qf);
            if input.is_inside_buffer(&c) {
                evaluate(&c)
            } else {
                default
            }
        })
        .collect()
}

/// `ResampleImageFilter<Image<unsigned char>, Image<unsigned char>, R>` with
/// `NearestNeighborInterpolateImageFunction` and default 0, through a transform that is not
/// linear (SyN's resampled masks): the voxel at the continuous index rounded half up.
pub fn resample_mask_nearest<R: Real, const D: usize>(
    mask: &Image<u8, D>,
    output: &Space<D>,
    transform: &Composite<R, D>,
) -> Image<u8, D> {
    let data = resample_nonlinear_with(&mask.space, output, transform, 0u8, |c| {
        let idx: [i64; D] = std::array::from_fn(|k| c[k].itk_round_half_up());
        mask.data[mask.space.offset(&idx)]
    });
    Image::new(output.clone(), data)
}

/// [`resample_linear`] for a transform that is not linear (per voxel,
/// [`resample_nonlinear_with`]).
pub fn resample_nonlinear<R: Real, const D: usize>(
    input: &Image<R, D>,
    output: &Space<D>,
    transform: &Composite<R, D>,
    interpolator: &Interpolator<D>,
    default: R,
    floor: &IndexFloor,
) -> Result<Vec<R>, String> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let other = match interpolator {
        Interpolator::Other(method) => Some(
            InterpolatorNd::with_precision(
                &input.data,
                input.space.size,
                method,
                input.space.spacing,
                R::PRECISION,
            )
            .map_err(|e| e.to_string())?,
        ),
        _ => None,
    };
    let data = &input.data;
    let space = &input.space;
    Ok(resample_nonlinear_with(
        space,
        output,
        transform,
        default,
        |c| match (interpolator, &other) {
            (Interpolator::Linear, _) => {
                cast(at_cindex_with(space, c, floor, |o| data[o].to_f64()))
            }
            (Interpolator::NearestNeighbor, _) => {
                let idx: [i64; D] = std::array::from_fn(|k| c[k].itk_round_half_up());
                cast(data[space.offset(&idx)].to_f64())
            }
            (Interpolator::GenericLabel, _) => cast(generic_label(input, c, floor)),
            (Interpolator::Other(_), Some(interp)) => {
                let ci: [f64; D] = std::array::from_fn(|k| c[k].to_f64() - space.start[k] as f64);
                cast(interp.evaluate(ci, floor))
            }
            (Interpolator::Other(_), None) => unreachable!("prepared above"),
        },
    ))
}

/// `input` resampled onto `output` through `transform` (a linear composite), with
/// `interpolator` and the value `default` outside the input. Fails where the interpolator
/// cannot be built (a B-spline order above 5, a non-positive Gaussian sigma or alpha).
pub fn resample_linear<R: Real, const D: usize>(
    input: &Image<R, D>,
    output: &Space<D>,
    transform: &Composite<R, D>,
    interpolator: &Interpolator<D>,
    default: R,
    floor: &IndexFloor,
) -> Result<Vec<R>, String> {
    let n0 = output.size[0];
    if n0 == 0 || output.is_empty() {
        return Ok(Vec::new());
    }
    let other = match interpolator {
        Interpolator::Other(method) => Some(
            InterpolatorNd::with_precision(
                &input.data,
                input.space.size,
                method,
                input.space.spacing,
                R::PRECISION,
            )
            .map_err(|e| e.to_string())?,
        ),
        _ => None,
    };
    let first = output.start[0];
    let size0 = n0 as f64;
    let transform_index = |idx: &[i64; D]| -> [R; D] {
        let p = output.index_to_point(idx);
        let pr: [R; D] = std::array::from_fn(|k| R::from_f64(p[k]));
        let q = transform.transform_point(&pr);
        let qf: [f64; D] = std::array::from_fn(|k| q[k].to_f64());
        input.space.point_to_cindex::<R>(&qf)
    };
    let data = &input.data;
    let space = &input.space;
    let evaluate = |c: &[R; D]| -> R {
        if !space.is_inside_buffer(c) {
            return default;
        }
        match (interpolator, &other) {
            (Interpolator::Linear, _) => {
                cast(at_cindex_with(space, c, floor, |o| data[o].to_f64()))
            }
            (Interpolator::NearestNeighbor, _) => {
                let idx: [i64; D] = std::array::from_fn(|k| c[k].itk_round_half_up());
                cast(data[space.offset(&idx)].to_f64())
            }
            (Interpolator::GenericLabel, _) => cast(generic_label(input, c, floor)),
            (Interpolator::Other(_), Some(interp)) => {
                // The interpolators index the buffer from 0.
                let ci: [f64; D] = std::array::from_fn(|k| c[k].to_f64() - space.start[k] as f64);
                cast(interp.evaluate(ci, floor))
            }
            (Interpolator::Other(_), None) => unreachable!("prepared above"),
        }
    };
    let lines = output.len() / n0;
    let rows: Vec<Vec<R>> = (0..lines)
        .into_par_iter()
        .map(|line| {
            let mut index = output.index_of(line * n0);
            index[0] = first;
            let start = transform_index(&index);
            index[0] = first + n0 as i64;
            let end = transform_index(&index);
            let vector: [R; D] = std::array::from_fn(|k| end[k] - start[k]);
            (0..n0)
                .map(|x| {
                    let alpha = x as f64 / size0;
                    let c: [R; D] = std::array::from_fn(|k| {
                        R::from_f64(start[k].to_f64() + alpha * vector[k].to_f64())
                    });
                    evaluate(&c)
                })
                .collect()
        })
        .collect();
    Ok(rows.into_iter().flatten().collect())
}
