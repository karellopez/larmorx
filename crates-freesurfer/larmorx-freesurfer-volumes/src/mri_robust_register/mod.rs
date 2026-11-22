// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_register` (queue item 3.2-13): FreeSurfer 8.2.0's
//! `mri_robust_register/mri_robust_register.cpp`, Martin Reuter's robust inverse-consistent
//! registration of two volumes (fMRIPrep's `RobustRegister`: `--satit --iscale --mov <T1.mgz>
//! --dst <T1w> --lta <out>`), on `fs.robust`'s `RegRobust`
//! (`larmorx_freesurfer_robust::registration`).
//!
//! - [`options`]: `Parameters`, `parseNextCommand`, `parseCommandLine`, `printUsage`;
//! - [`main`]: `main`, `initRegistration`: the program's entry point.

pub mod main;
pub mod options;
