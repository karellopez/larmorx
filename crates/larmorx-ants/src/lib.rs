// SPDX-License-Identifier: Apache-2.0
//! ANTs tools for larmorx (PLAN.md §5), ported from ANTs v2.6.5 and the ITK v5.4.5 filters
//! they run on. See `PROVENANCE.md`.
//!
//! - `antsApplyTransforms` is [`apply_transforms`] (ITK's `ResampleImageFilter`).
//! - `antsApplyTransformsToPoints` is [`transform_points`].
//! - `ThresholdImage` is [`threshold_image()`], `MultiplyImages` is [`multiply_images()`],
//!   and ImageMath's operations are in [`image_math`].
//! - [`image`] holds images as ANTs programs do, with ANTs' reading and writing rules.
//! - The original command lines are in [`cli`].
#![forbid(unsafe_code)]

pub mod apply_transforms;
pub mod cli;
pub mod image;
pub mod image_math;
pub mod multiply_images;
pub mod threshold_image;

pub use apply_transforms::{
    ApplyTransformsError, ApplyTransformsOptions, OutputType, apply_transforms, transform_points,
};
pub use image::{AntsImage, FileStore, ImageStore};
pub use multiply_images::multiply_images;
pub use threshold_image::{ThresholdMode, ThresholdOutput, threshold_image};
