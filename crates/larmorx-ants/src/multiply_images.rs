// SPDX-License-Identifier: Apache-2.0
//! `MultiplyImages` (ANTs v2.6.5, `Examples/MultiplyImages.cxx`) for scalar images: the
//! product of two images, or of an image and a number, in `float`.
//!
//! ANTs reads both images as `itk::Vector<float, 1>` pixels and multiplies voxel by voxel at
//! the first image's indices, so the second image's geometry is ignored (it may be larger; a
//! smaller one is read outside its voxels, ANTS-07). If the second argument cannot be read as
//! an image it is taken as a number with `atof`, so a missing file multiplies by 0 (ANTS-27,
//! the command line's concern).

use larmorx_core::defects::{Notice, UpstreamDefects};
use larmorx_image::FilterError;

use crate::image_math::{Arithmetic, Operand, arithmetic};

/// The voxel-wise product of `a` (with `size`) and `b`, with a smaller second image refused
/// ([`UpstreamDefects::Fix`]) or read as ANTs reads it ([`UpstreamDefects::Reproduce`], see
/// [`crate::image_math::arithmetic::operand_values`]).
pub fn multiply_images(
    a: &[f32],
    size: &[usize],
    b: Operand<'_>,
    n_threads: usize,
    mode: UpstreamDefects,
) -> Result<(Vec<f32>, Vec<Notice>), FilterError> {
    let r = arithmetic(Arithmetic::Multiply, a, size, b, n_threads, mode)?;
    Ok((r.data, r.notices))
}
