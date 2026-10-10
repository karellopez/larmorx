// SPDX-License-Identifier: Apache-2.0
//! Image filters for larmorx (PLAN.md §5), ported from ITK v5.4.5 as ANTs v2.6.5 uses them.
//!
//! Each filter reproduces the ITK class it is named after: the same algorithm, edge
//! handling, pixel and accumulator precision, and operation order, so that ANTs tools built
//! on them (ImageMath, ThresholdImage, SmoothImage, ...) match ANTs bit for bit where
//! possible. The provenance of each module is in `PROVENANCE.md`.
//!
//! **Conventions** shared by every module:
//! - Spatial filters take a [`VolumeRef`] (a borrowed [`Volume`]): voxels in Fortran order
//!   (first axis fastest), the size and the spacing, in **as many dimensions as the ITK image**
//!   (2, 3 or 4 for ANTs). ITK's filters are `D`-dimensional: a 4D image is filtered along
//!   time too, and a 2D image is not a 3D image with one slice. Spacing matters for filters
//!   whose parameters are physical, such as Gaussian sigmas.
//! - The pixel precision is the one ITK uses for the same instantiation. ANTs reads images as
//!   `float`, and ITK's `NumericTraits<float>::RealType` is `double`, so many filters
//!   accumulate in `f64` and store `f32`.
//! - Filters that can run in parallel take an explicit `n_threads` (0 = all CPUs). Their
//!   results never depend on it.
//! - Transcendental functions come from `larmorx_core::math`, which is correctly rounded.
//! - Filters return [`FilterError`]; where ITK throws for the same arguments, the message is
//!   ITK's. Float comparisons that ITK makes with `itk::Math::FloatAlmostEqual` use
//!   [`itk_math`].
//! - Voxel-wise and histogram filters take flat slices in Fortran order (they do not depend on
//!   the image shape).
#![forbid(unsafe_code)]

pub mod components;
pub mod discrete_gaussian;
pub mod distance;
mod error;
pub mod gaussian;
pub mod intensity;
pub mod itk_math;
mod lines;
pub mod median;
pub mod morphology;
pub mod statistics;
pub mod threshold;
mod volume;

pub use error::FilterError;
pub use volume::{Volume, VolumeRef, strides};
