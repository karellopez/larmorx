// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_concat` (queue item 3.2-39): FreeSurfer 8.2.0's `mri_concat/mri_concat.cpp`, which
//! concatenates volumes frame by frame and reduces or combines their frames (recon-all:
//! `pctsurfcon`'s percent contrast `<wm> <gm> --paired-diff-norm --mul 100`, and the v8 stream's
//! `mri_mcadura_seg`, `<lh> <rh> --sum --ctab mca-dura.ctab`).
//!
//! - [`options`]: `parse_commandline`, `check_options`, the usage, help and version texts.
//! - [`main`]: `main`, `GroupedMeanMatrix` and `MRIfirstNonZero` (the program's own functions):
//!   the program's entry point.

pub mod main;
pub mod options;
