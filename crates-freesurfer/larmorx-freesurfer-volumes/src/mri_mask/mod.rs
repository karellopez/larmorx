// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_mask` (queue item 3.2-26, wave 3.2's track D): FreeSurfer 8.2.0's `mri_mask/mri_mask.cpp`,
//! on the volume functions of `larmorx-freesurfer-volume` (`MRImask`, the crops, dilation and
//! erosion) and the transforms of `larmorx-freesurfer-gca` (`TransformRead`, `TransformApplyType`).
//!
//! - [`main`]: the program (`main`, `get_option`, `usage`), its entry point [`main::run`].

pub mod main;
