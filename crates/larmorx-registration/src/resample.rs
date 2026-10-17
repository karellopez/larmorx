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
//! bounds checking (clamped to `R`'s range); outside, the default value.

use larmorx_core::IndexFloor;
use rayon::prelude::*;

use crate::interp::at_cindex_with;
use crate::transform::Composite;
use crate::{Image, Real, Space};

/// The interpolators this resampler has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolator {
    /// `LinearInterpolateImageFunction<Image<R>, R>`.
    Linear,
    /// `NearestNeighborInterpolateImageFunction<Image<R>, R>`: the voxel at the continuous
    /// index rounded half up on each axis.
    NearestNeighbor,
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

/// `input` resampled onto `output` through `transform` (a linear composite), with
/// `interpolator` and the value `default` outside the input.
pub fn resample_linear<R: Real, const D: usize>(
    input: &Image<R, D>,
    output: &Space<D>,
    transform: &Composite<R, D>,
    interpolator: Interpolator,
    default: R,
    floor: &IndexFloor,
) -> Vec<R> {
    let n0 = output.size[0];
    if n0 == 0 || output.is_empty() {
        return Vec::new();
    }
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
        match interpolator {
            Interpolator::Linear => cast(at_cindex_with(space, c, floor, |o| data[o].to_f64())),
            Interpolator::NearestNeighbor => {
                let idx: [i64; D] = std::array::from_fn(|k| c[k].itk_round_half_up());
                cast(data[space.offset(&idx)].to_f64())
            }
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
    rows.into_iter().flatten().collect()
}
