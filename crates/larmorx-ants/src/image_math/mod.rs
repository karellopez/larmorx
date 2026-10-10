// SPDX-License-Identifier: Apache-2.0
//! The Rust API of ImageMath's operations (ANTs v2.6.5, `Examples/ImageMath_Templates.hxx`),
//! one module per operation group. Each function works on voxel slices in Fortran order and
//! reproduces the ANTs function it is named after; the command line
//! (`cli::image_math`) reads and writes the images around them.

pub mod arithmetic;
pub mod intensity;

pub use arithmetic::{Arithmetic, ArithmeticOutput, Operand, arithmetic, negative};
pub use intensity::{
    Normalization, TruncateOptions, Truncated, normalize, rescale, truncate_image_intensity,
};
