// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_binarize` (queue item 3.2-19, wave 3.2's track D): FreeSurfer 8.2.0's
//! `mri_binarize/mri_binarize.cpp`, on the volume functions of `larmorx-freesurfer-volume` and
//! `-filter` and the tessellation of `larmorx-freesurfer-deform` (`--surf`, `--fix-vol-topo`).
//!
//! - [`options`]: the command line (`parse_commandline`, with its three stand-alone options
//!   `--ohe`, `--crop-around-ras` and `--dilate-vertex(-sum)`), `check_options`, `dump_options`
//!   and the help.
//! - [`main`]: the program itself (`main`), its entry point [`main::run`].
//! - [`help`]: the usage and help texts.

pub mod help;
pub mod main;
pub mod options;
