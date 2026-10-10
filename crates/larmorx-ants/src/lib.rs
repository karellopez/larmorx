// SPDX-License-Identifier: Apache-2.0
//! ANTs tools for larmorx (PLAN.md §5), ported from ANTs v2.6.5 and the ITK v5.4.5 filters
//! they run on. See `PROVENANCE.md`.
//!
//! - `antsApplyTransforms` is [`apply_transforms`] (ITK's `ResampleImageFilter`).
//! - `antsApplyTransformsToPoints` is [`transform_points`].
//! - `ThresholdImage` is [`threshold_image()`], `MultiplyImages` is [`multiply_images()`],
//!   `SmoothImage` is [`smooth_image()`], `ResampleImageBySpacing` is
//!   [`resample_image_by_spacing()`], `ResampleImage` is [`resample_image()`], and ImageMath's
//!   operations are in [`image_math`].
//! - [`resample`] is ITK's `ResampleImageFilter` with an identity transform in 2 to 4
//!   dimensions.
//! - [`image`] holds images as ANTs programs do, with ANTs' reading and writing rules.
//! - The original command lines are in [`cli`].
#![forbid(unsafe_code)]

pub mod apply_transforms;
pub mod cli;
pub mod image;
pub mod image_math;
pub mod multiply_images;
pub mod resample;
pub mod resample_image;
pub mod resample_image_by_spacing;
pub mod smooth_image;
pub mod threshold_image;

pub use apply_transforms::{
    ApplyTransformsError, ApplyTransformsOptions, OutputType, apply_transforms, transform_points,
};
pub use image::{AntsImage, FileStore, ImageStore};
/// The image view the spatial filters take (re-exported for the bindings).
pub use larmorx_image::VolumeRef;
pub use multiply_images::multiply_images;
pub use resample_image::{ResampleImageError, ResampleTarget, resample_image};
pub use resample_image_by_spacing::{
    ResampleBySpacingError, ResampleBySpacingOptions, ResampleBySpacingPlan,
    resample_image_by_spacing,
};
pub use smooth_image::{Smoothing, smooth_image};
pub use threshold_image::{ThresholdMode, ThresholdOutput, threshold_image};
