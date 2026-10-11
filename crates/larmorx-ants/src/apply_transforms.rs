// SPDX-License-Identifier: Apache-2.0
//! `antsApplyTransforms`: resampling an image through a chain of transforms.
//!
//! Ported from ANTs v2.6.5 (`Examples/antsApplyTransforms.cxx`) and ITK v5.4.5
//! (`itkResampleImageFilter.hxx`). For every voxel of the reference grid, its physical point is
//! mapped through the transform chain into the input image, converted to a continuous index
//! there and interpolated; points that fall outside the input (more than half a voxel beyond
//! its grid) get the default value. As in ITK, when every transform is linear each output
//! scan line is mapped by its two ends and the indices in between are interpolated along the
//! line, which rounds slightly differently from mapping every voxel.
//!
//! **Precision.** ANTs instantiates the whole pipeline with `double`, or with `float` under
//! `--float 1` ([`Precision`]). In float, as ITK's types make it
//! (`ResampleImageFilter<Image<float>, Image<float>, float>`, `CompositeTransform<float>`):
//! - each output voxel's physical point is computed in double and stored as `Point<float>`;
//! - every transform maps float points in float arithmetic, from its parameters cast to float
//!   ([`TransformChain::with_precision`]);
//! - the continuous index is computed in double from the float point and stored as float, and
//!   so are the ends of a scan line; along the line, `vector = end − start` is a float
//!   difference and each index `start + α·vector` is computed in double and stored as float;
//! - the interpolators take float indices ([`Interpolator::with_precision`]) and return double;
//! - the output pixel is float: the interpolated value is clamped to ±`FLT_MAX`
//!   (`CastComponentWithBoundsChecking`) and rounded, the default value is a float.

use larmorx_core::element::RealElement;
use larmorx_core::parallel::{self, ThreadPoolError};
use larmorx_core::{Grid3, ItkReal, Precision};
use larmorx_interp::{InterpError, Interpolation, Interpolator, Volume, is_inside};
use larmorx_transform::{PrecisionChain, TransformChain, TransformError};
use rayon::prelude::*;

/// Options of [`apply_transforms`].
#[derive(Clone, Debug, PartialEq)]
pub struct ApplyTransformsOptions {
    pub interpolation: Interpolation,
    /// The value of output voxels that map outside the input (`-f`; default 0).
    pub default_value: f64,
    /// Worker threads (0 = all logical CPUs). The result does not depend on it.
    pub n_threads: usize,
    /// The computation type: [`Precision::Double`] (ANTs' default) or [`Precision::Float`]
    /// (`--float 1`).
    pub precision: Precision,
}

impl Default for ApplyTransformsOptions {
    fn default() -> Self {
        ApplyTransformsOptions {
            interpolation: Interpolation::Linear,
            default_value: 0.0,
            n_threads: 1,
            precision: Precision::Double,
        }
    }
}

/// [`apply_transforms`] failed.
#[derive(Debug, thiserror::Error)]
pub enum ApplyTransformsError {
    #[error("the input has {values} values, which is not a whole number of {voxels}-voxel volumes")]
    InputSize { values: usize, voxels: usize },
    #[error(transparent)]
    Interpolation(#[from] InterpError),
    #[error(transparent)]
    Transform(#[from] TransformError),
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
}

/// Resamples `input` onto the `reference` grid through `chain` (applied last first, as
/// `antsApplyTransforms` applies its `-t` options).
///
/// `input` holds one or more volumes on `input_grid` (Fortran order, `x` fastest, volumes one
/// after the other, as a 4D time series). Each volume is resampled the same way (`-e 3`) and
/// the result has as many volumes, on `reference`, in the same layout. With
/// [`Precision::Float`] the values are `f32` values (the output image's pixels), returned as
/// `f64`.
pub fn apply_transforms<T: RealElement>(
    input: &[T],
    input_grid: &Grid3,
    reference: &Grid3,
    chain: &TransformChain,
    options: &ApplyTransformsOptions,
) -> Result<Vec<f64>, ApplyTransformsError> {
    match options.precision {
        Precision::Double => apply_as::<T, f64>(input, input_grid, reference, chain, options),
        Precision::Float => apply_as::<T, f32>(input, input_grid, reference, chain, options),
    }
}

/// `ResampleImageFilter`'s output pixel from the interpolated value
/// (`CastPixelWithBoundsChecking`): in double the value itself; in float clamped to
/// `[−FLT_MAX, FLT_MAX]` (so ±∞ become ±`FLT_MAX`; NaN passes) and rounded.
#[inline]
fn output_pixel<R: ItkReal>(value: f64) -> f64 {
    match R::PRECISION {
        Precision::Double => value,
        Precision::Float => {
            let max = f64::from(f32::MAX);
            if value <= -max {
                -max
            } else if value >= max {
                max
            } else {
                f64::from(value as f32)
            }
        }
    }
}

fn apply_as<T: RealElement, R: ItkReal>(
    input: &[T],
    input_grid: &Grid3,
    reference: &Grid3,
    chain: &TransformChain,
    options: &ApplyTransformsOptions,
) -> Result<Vec<f64>, ApplyTransformsError> {
    let n_in = input_grid.len();
    if n_in == 0 || !input.len().is_multiple_of(n_in) {
        return Err(ApplyTransformsError::InputSize {
            values: input.len(),
            voxels: n_in,
        });
    }
    let n_volumes = input.len() / n_in;
    let n_out = reference.len();
    if n_out == 0 || n_volumes == 0 {
        return Ok(Vec::new());
    }
    let nx = reference.size[0];
    let points = chain.with_precision::<R>()?;
    let mapper = IndexMapper {
        linear: points.is_linear(),
        chain: points,
        input: input_grid,
        reference,
    };
    let default_value = R::from_f64(options.default_value).to_f64();
    parallel::with_threads(options.n_threads, || {
        // With several volumes, map every voxel once and reuse the indices (ITK recomputes
        // them per volume, with the same result).
        let cached: Option<Vec<[f64; 3]>> = (n_volumes > 1).then(|| {
            let mut all = vec![[0.0; 3]; n_out];
            all.par_chunks_mut(nx)
                .enumerate()
                .for_each(|(row, out)| mapper.row(row, out));
            all
        });
        let mut output = vec![0.0f64; n_out * n_volumes];
        for (volume, out) in input.chunks_exact(n_in).zip(output.chunks_mut(n_out)) {
            let interpolator = Interpolator::with_precision(
                Volume::new(volume, input_grid.size),
                &options.interpolation,
                input_grid.spacing,
                R::PRECISION,
            )?;
            out.par_chunks_mut(nx).enumerate().for_each_init(
                || vec![[0.0f64; 3]; nx],
                |buffer, (row, out_row)| {
                    let indices: &[[f64; 3]] = match &cached {
                        Some(all) => &all[row * nx..(row + 1) * nx],
                        None => {
                            mapper.row(row, buffer);
                            buffer
                        }
                    };
                    for (o, &c) in out_row.iter_mut().zip(indices) {
                        *o = if is_inside(c, input_grid.size) {
                            output_pixel::<R>(interpolator.evaluate(c))
                        } else {
                            default_value
                        };
                    }
                },
            );
        }
        Ok(output)
    })?
}

/// Maps reference voxels to continuous indices of the input, in the computation type `R`
/// (the indices are `R` values, returned as `f64`).
struct IndexMapper<'a, R> {
    chain: PrecisionChain<'a, R>,
    input: &'a Grid3,
    reference: &'a Grid3,
    linear: bool,
}

impl<R: ItkReal> IndexMapper<'_, R> {
    /// The continuous index of output voxel `index`, as `R` values: the voxel's point in
    /// double, stored as `Point<R>`, mapped by the chain in `R`, then
    /// `TransformPhysicalPointToContinuousIndex<R>` (double arithmetic, stored as `R`).
    #[inline]
    fn voxel(&self, index: [usize; 3]) -> [R; 3] {
        let point = self.reference.voxel_to_point(index).map(R::from_f64);
        let mapped = self.chain.transform_point(point);
        self.input
            .point_to_index(mapped.map(|x| x.to_f64()))
            .map(R::from_f64)
    }

    /// The continuous indices of output scan line `row` (`j + ny·k`).
    fn row(&self, row: usize, out: &mut [[f64; 3]]) {
        let [nx, ny, _] = self.reference.size;
        let (j, k) = (row % ny, row / ny);
        if self.linear {
            // ITK's LinearThreadedGenerateData: the line from index 0 to index nx (one past the
            // end), sampled at x / nx; the difference of the ends in R, each index computed in
            // double and stored as R.
            let start = self.voxel([0, j, k]);
            let end = self.voxel([nx, j, k]);
            let vector: [f64; 3] = std::array::from_fn(|d| (end[d] - start[d]).to_f64());
            let start = start.map(|s| s.to_f64());
            for (x, c) in out.iter_mut().enumerate() {
                let alpha = x as f64 / nx as f64;
                *c = std::array::from_fn(|d| R::from_f64(start[d] + alpha * vector[d]).to_f64());
            }
        } else {
            for (x, c) in out.iter_mut().enumerate() {
                *c = self.voxel([x, j, k]).map(|v| v.to_f64());
            }
        }
    }
}

/// `antsApplyTransformsToPoints`: maps physical points (LPS) through `chain`.
///
/// The chain maps points the way it maps output voxels to input positions in
/// [`apply_transforms`], so to move points from the moving to the fixed image, give the
/// transforms (or their inverses) of the opposite direction, as with ANTs.
pub fn transform_points(points: &[[f64; 3]], chain: &TransformChain) -> Vec<[f64; 3]> {
    points.iter().map(|&p| chain.transform_point(p)).collect()
}

/// `antsApplyTransforms`' output pixel types (`-u`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputType {
    Char,
    UChar,
    Short,
    Int,
    Float,
    Double,
}

impl OutputType {
    /// The `-u` names (`default` is the computation type and is handled by the caller).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "char" => OutputType::Char,
            "uchar" => OutputType::UChar,
            "short" => OutputType::Short,
            "int" => OutputType::Int,
            "float" => OutputType::Float,
            "double" => OutputType::Double,
            _ => return None,
        })
    }
}
