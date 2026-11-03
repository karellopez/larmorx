// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-basisfield 2203.4). FSL is Copyright (C) 2007
// University of Oxford. Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the
// originals. Distributed under the FSL Licence (see LICENSE).
//! fsl-basisfield: fields on a voxel grid as sums of B-splines ([`splines`]: the 1D and 3D
//! kernels; [`splinefield`]: the field, its derivatives, `Jᵀe`, `JᵀJ`, the membrane and bending
//! energies with their gradients and Hessians), as topup (and FNIRT's family) use them.

// The loops keep FSL's paired counters (field and kernel indices advanced together), so that each
// step can be checked against the source.
#![allow(clippy::explicit_counter_loop)]

pub mod splinefield;
pub mod splines;

pub use splinefield::{Csc, FieldIndex, SplineField, basisfield_error};
pub use splines::{Spline1D, Spline3D};
