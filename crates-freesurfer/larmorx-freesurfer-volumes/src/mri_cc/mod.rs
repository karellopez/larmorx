// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_cc` (queue item 3.2-27, wave 3.2's track D): FreeSurfer 8.2.0's `mri_cc/mri_cc.cpp`, the
//! corpus callosum's segmentation, on the volume functions of `larmorx-freesurfer-volume`, the
//! segments of `larmorx-freesurfer-filter` and the matrices of `larmorx-freesurfer-core`.
//!
//! - [`main`]: the program (`main`, `get_option`, `print_usage`), its entry point [`main::run`].
//! - [`aseg`]: the path recon-all's `seg2cc` runs (`-aseg`): `find_cc_with_aseg`,
//!   `cc_cutting_plane_correct`, `remove_fornix_new` and `find_voxels_close_to_both_hemis`.

pub mod aseg;
pub mod main;
