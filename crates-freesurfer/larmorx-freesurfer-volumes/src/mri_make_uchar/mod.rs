// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_make_uchar` (queue item 3.2-22, wave 3.2's track D): FreeSurfer 8.2.0's
//! `mri_convert/mri_make_uchar.cpp`, on the volume functions of `larmorx-freesurfer-volume`
//! (`LTAread`, `MRIcloneDifferentType`, `MRIvoxelToTalairach`, `LTAtransformAtPoint`,
//! `MRIhistogram`) and the histograms of `larmorx-freesurfer-core`.
//!
//! - [`main`]: the program (`main`, `get_option`, `usage_exit`, `MRIconvertToUchar`), its entry
//!   point [`main::run`].

pub mod main;
