// SPDX-License-Identifier: Apache-2.0
//! ANTs tools for larmorx (PLAN.md §5), ported from ANTs v2.6.5 and the ITK v5.4.5 filters
//! they run on. See `PROVENANCE.md`.
//!
//! - `antsApplyTransforms` is [`apply_transforms`] (ITK's `ResampleImageFilter`).
//! - `antsApplyTransformsToPoints` is [`transform_points`].
//! - The original command lines are in [`cli`].
#![forbid(unsafe_code)]

pub mod apply_transforms;
pub mod cli;

pub use apply_transforms::{
    ApplyTransformsError, ApplyTransformsOptions, OutputType, apply_transforms, transform_points,
};
