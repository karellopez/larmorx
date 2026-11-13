// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-mri: `fs.mri` (queue item 3.1-55) of FreeSurfer 8.2.0's replica
//! (docs/freesurfer/waves.md section 4): the `MRI` structure below the readers that build it, so
//! that `larmorx-freesurfer-io` and `larmorx-freesurfer-volume` both build on it (the
//! orchestrator's decision on 3.1-25, 2026-11-13: the data structure low, the functions that read
//! files above it).
//!
//! - [`vol_geom`]: `VOL_GEOM` (the geometry an `MRI` derives from) with C++'s copy semantics, and
//!   the voxel-to-RAS matrices on it (`MRIxfmCRS2XYZ`, `extract_i_to_r`, `MRIreInitCache`,
//!   `MRIsetVox2RASFromMatrix`, `MRIp0ToCRAS`, ...).
//! - [`ctab`]: the colour table's structure an `MRI` carries, with `CTABfree` and `CTABdeepCopy`
//!   (its reading and writing stay in `fs.labels`).
//! - [`mri`]: `MRI` itself: its constructors, `MRIalloc*`, `MRIfree`, `MRIsizeof`, `MRIcopyHeader`,
//!   the geometry copies, the intents.
//! - [`voxel`]: `MRIgetVoxVal`, `MRIsetVoxVal` and their derivatives.
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate. `PROVENANCE.md` lists what is translated.
#![forbid(unsafe_code)]

pub mod ctab;
pub mod mri;
pub mod vol_geom;
pub mod voxel;

/// `STRLEN` (`const.h`): the size of the structure's name fields (`fname`, `subject_name`, ...).
pub const STRLEN: usize = 1024;
