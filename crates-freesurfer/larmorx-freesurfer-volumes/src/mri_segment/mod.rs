// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_segment` (queue item 3.2-31, wave 3.2's track D): FreeSurfer 8.2.0's
//! `mri_segment/mri_segment.cpp`, on fs.filter's planes of least variance (`utils/mripolv.cpp`:
//! `larmorx_freesurfer_filter::polv`), border classification (`classify`), non-parametric
//! classification (`nonpar`) and segments (`segment`), and fs.volume's histogram segmentation
//! (`MRIhistoSegment`) and bright non-white-matter search (`MRIfindBrightNonWM`).
//!
//! - [`main`]: the program (`main`, `get_option`, `usage_exit`), its entry point [`main::run`];
//! - [`local`]: the program's own functions (`MRIremoveWrongDirection`, `MRIfillBasalGanglia`,
//!   `MRIfilterMorphology`, `MRIremove1dStructures`, `is_diagonal`, `MRIfillVentricles`,
//!   `MRIrecoverBrightWhite`).

pub mod local;
pub mod main;
