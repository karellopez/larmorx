// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_edit_wm_with_aseg` (queue item 3.2-32, wave 3.2's track D): FreeSurfer 8.2.0's
//! `mri_edit_wm_with_aseg/mri_edit_wm_with_aseg.cpp`, on fs.volume's label copies, dilations,
//! `FixSubCortMassHA`, `MRIfixEntoWM` and `LabelAmygalaCortalJunction`.
//!
//! - [`main`]: the program (`main`, `get_option`, `usage_exit`), its entry point [`main::run`];
//! - [`edit`]: `edit_segmentation`;
//! - [`paths`]: `remove_paths_to_cortex` and its region's helpers;
//! - [`local`]: the voxel accesses and the small functions (`neighborLabel`, the distances and
//!   edges, `MRInonfilledInNbhd`, `spackle_wm_superior_to_mtl`, `KeepHAILVCP`).

pub mod edit;
pub mod local;
pub mod main;
pub mod paths;
