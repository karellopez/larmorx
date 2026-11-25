// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_coreg` (queue item 3.2-17, wave 3.2's track C): FreeSurfer 8.2.0's
//! `mri_coreg/mri_coreg.cpp`, the linear registration fMRIPrep runs (`MRICoreg`, and
//! `bbregister --init-coreg`), on the volume functions of `larmorx-freesurfer-volume` and
//! `-filter`, Powell's minimiser of `larmorx-freesurfer-core` and the random draws of
//! larmorx-core's `ranlux` (the clean room of `specs/ranlux.md`, which `RFspecInit`'s
//! RANLUX389 stands for).
//!
//! - [`options`]: the command line (`parse_commandline`, `check_options`, `dump_options`, the
//!   usage and help texts) and its stand-alone options ([`standalone`]: `--mat2par`,
//!   `--mat2par-vox`, `--mat2rot`, `--par2mat`, `--lrrev`, `--rms`, each its work at once).
//! - [`coreg`]: the registration (`COREG` and its functions: the joint histogram, the cost, the
//!   brute-force search, Powell's minimisation, the preprocessing).
//! - [`main`]: the program itself (`main`), its entry point [`main::run`].

pub mod coreg;
pub mod main;
pub mod options;
pub mod standalone;
