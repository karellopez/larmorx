// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_template` (queue items 3.2-14 to 3.2-16): FreeSurfer 8.2.0's
//! `mri_robust_register/mri_robust_template.cpp`, Martin Reuter's within-subject template of
//! several images and their robust registration (fMRIPrep's `StructuralReference`: `--satit
//! --fixtp --mov <T1w>... --inittp 1 --iscale --noit --template <out> --subsample 200 --lta
//! <out>...`; recon-all's motion correction: the same with `--average 1 --iscaleout`), on
//! `fs.robust`'s `RegRobust` (`larmorx_freesurfer_robust::registration`).
//!
//! - [`options`]: `Parameters`, `parseNextCommand`, `parseCommandLine`, `printUsage`;
//! - [`multireg`]: `MultiRegistration` (`MultiRegistration.cpp`);
//! - [`main`]: `main`, `getRandomNumber`: the program's entry point.

pub mod main;
pub mod multireg;
pub mod options;
