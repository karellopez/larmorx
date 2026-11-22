// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_info` (queue item 3.2-08): FreeSurfer 8.2.0's `mri_info/mri_info.cpp`, the volume
//! information report.
//!
//! - [`options`]: `main`'s start, `parse_commandline`, `check_options` and the usage and help.
//! - [`main`]: the rest of `main`, `do_file`, `PrettyMatrixPrint`, `__printTagsToFile` and
//!   `__reportWarp`: the program's entry point.

pub mod main;
pub mod options;
