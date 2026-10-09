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

use larmorx_core::Grid3;
use larmorx_core::element::RealElement;
use larmorx_core::parallel::{self, ThreadPoolError};
use larmorx_interp::{InterpError, Interpolation, Interpolator, Volume, is_inside};
use larmorx_transform::TransformChain;
use rayon::prelude::*;

/// Options of [`apply_transforms`].
#[derive(Clone, Debug, PartialEq)]
pub struct ApplyTransformsOptions {
    pub interpolation: Interpolation,
    /// The value of output voxels that map outside the input (`-f`; default 0).
    pub default_value: f64,
    /// Worker threads (0 = all logical CPUs). The result does not depend on it.
    pub n_threads: usize,
}

impl Default for ApplyTransformsOptions {
    fn default() -> Self {
        ApplyTransformsOptions {
            interpolation: Interpolation::Linear,
            default_value: 0.0,
            n_threads: 1,
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
    Threads(#[from] ThreadPoolError),
}

/// Resamples `input` onto the `reference` grid through `chain` (applied last first, as
/// `antsApplyTransforms` applies its `-t` options).
///
/// `input` holds one or more volumes on `input_grid` (Fortran order, `x` fastest, volumes one
/// after the other, as a 4D time series). Each volume is resampled the same way (`-e 3`) and
/// the result has as many volumes, on `reference`, in the same layout.
pub fn apply_transforms<T: RealElement>(
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
    let mapper = IndexMapper {
        chain,
        input: input_grid,
        reference,
        linear: chain.is_linear(),
    };
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
            let interpolator = Interpolator::new(
                Volume::new(volume, input_grid.size),
                &options.interpolation,
                input_grid.spacing,
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
                            interpolator.evaluate(c)
                        } else {
                            options.default_value
                        };
                    }
                },
            );
        }
        Ok(output)
    })?
}

/// Maps reference voxels to continuous indices of the input.
struct IndexMapper<'a> {
    chain: &'a TransformChain,
    input: &'a Grid3,
    reference: &'a Grid3,
    linear: bool,
}

impl IndexMapper<'_> {
    fn voxel(&self, index: [usize; 3]) -> [f64; 3] {
        let point = self.reference.voxel_to_point(index);
        self.input.point_to_index(self.chain.transform_point(point))
    }

    /// The continuous indices of output scan line `row` (`j + ny·k`).
    fn row(&self, row: usize, out: &mut [[f64; 3]]) {
        let [nx, ny, _] = self.reference.size;
        let (j, k) = (row % ny, row / ny);
        if self.linear {
            // ITK's LinearThreadedGenerateData: the line from index 0 to index nx (one past the
            // end), sampled at x / nx.
            let start = self.voxel([0, j, k]);
            let end = self.voxel([nx, j, k]);
            let vector = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
            for (x, c) in out.iter_mut().enumerate() {
                let alpha = x as f64 / nx as f64;
                *c = std::array::from_fn(|d| start[d] + alpha * vector[d]);
            }
        } else {
            for (x, c) in out.iter_mut().enumerate() {
                *c = self.voxel([x, j, k]);
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
