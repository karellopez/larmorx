// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_pretess` (queue item 3.2-33, wave 3.2's track D): FreeSurfer 8.2.0's
//! `mri_mc/mri_pretess.cpp`, which makes a segmentation's label tessellable: no two voxels of the
//! label meet only at an edge or a corner.
//!
//! - [`main`]: the program (`main`, `get_option`, `print_help`), its entry point [`main::run`];
//! - [`tess`]: the edge, corner and background corner removals and `MRIaddEdgeVoxel`.

pub mod main;
pub mod tess;
