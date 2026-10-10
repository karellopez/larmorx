// SPDX-License-Identifier: Apache-2.0
//! Head-motion correction compatible with FSL's `mcflirt` (`specs/mcflirt.md`).
//!
//! Every volume of a 4D series is registered to a reference volume with a rigid-body
//! transform by a fixed schedule of line searches on coarse reference grids
//! ([`estimate::estimate`]), then resampled with those transforms ([`resample`]). The
//! matrices follow mcflirt's convention (§4.2): they map a volume's FSL-mm coordinates
//! (voxel index times voxel size, in the "radiological" in-memory order) to the reference's.

pub mod cost;
pub mod estimate;
pub mod grid;
pub mod image;
pub mod interp;
pub mod kernels;
pub mod pipeline;
pub mod report;
pub mod resample;
pub mod rigid;
pub mod search;
pub mod stats;
pub mod volume;
pub mod world;

pub use cost::CostFunction;
pub use estimate::{Estimate, EstimateError, EstimateParams, Reference, estimate};
pub use resample::Interpolation;
pub use rigid::Mat4;
pub use volume::Volume;
