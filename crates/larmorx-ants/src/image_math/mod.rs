// SPDX-License-Identifier: Apache-2.0
//! The Rust API of ImageMath's operations (ANTs v2.6.5, `Examples/ImageMath_Templates.hxx`),
//! one module per operation group. Each function reproduces the ANTs function it is named
//! after: voxel-wise operations work on voxel slices in Fortran order, spatial ones on a
//! [`larmorx_image::VolumeRef`] of the image's own dimensions. The command line
//! (`cli::image_math`) reads and writes the images around them.

pub mod arithmetic;
pub mod gaussian;
pub mod intensity;

pub use arithmetic::{Arithmetic, ArithmeticOutput, Operand, arithmetic, negative};
pub use gaussian::{
    UnsharpMaskOptions, gradient_magnitude, laplacian, smooth_discrete, unsharp_mask,
};
pub use intensity::{
    Normalization, TruncateOptions, Truncated, normalize, rescale, truncate_image_intensity,
};
