// SPDX-License-Identifier: Apache-2.0
//! `MultiplyImages` (ANTs v2.6.5, `Examples/MultiplyImages.cxx`) for scalar images: the
//! product of two images, or of an image and a number, in `float`.
//!
//! ANTs reads both images as `itk::Vector<float, 1>` pixels and multiplies voxel by voxel at
//! the first image's indices, so the second image's geometry is ignored (it may be larger,
//! not smaller). If the second argument cannot be read as an image it is taken as a number
//! with `atof`, so a missing file multiplies by 0.

use larmorx_image::FilterError;

use crate::image_math::{Arithmetic, Operand, arithmetic};

/// The voxel-wise product of `a` (with `size`) and `b`.
pub fn multiply_images(
    a: &[f32],
    size: &[usize],
    b: Operand<'_>,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    Ok(arithmetic(Arithmetic::Multiply, a, size, b, n_threads)?.data)
}
