// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-robust: `fs.robust` (wave 3.2) of FreeSurfer 8.2.0's replica
//! (docs/freesurfer/waves.md section 4): Reuter's robust registration library
//! (`mri_robust_register`'s shared files): the robust and Powell registrations, the cost functions,
//! its image and matrix helpers, the multi-image registration and quaternions.
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-freesurfer` program as a
//! separate process (`docs/licensing.md`). Empty until its queue items bring the code
//! (`docs/freesurfer/queue.md`); `PROVENANCE.md` lists what each item translates.
#![forbid(unsafe_code)]
