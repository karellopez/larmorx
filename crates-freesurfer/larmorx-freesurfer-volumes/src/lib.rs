// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-volumes: wave 3.2's programs of FreeSurfer 8.2.0's replica
//! (docs/freesurfer/waves.md section 4): the volume programs both streams run (`mri_convert`,
//! `mri_info`, `mri_normalize`, `mri_segment`, `mri_fill`, ...), fMRIPrep's `mri_coreg`,
//! `mri_robust_register` and `mri_robust_template`, and the scripts around them
//! (`mri_nu_correct.mni`, `seg2cc`, ...).
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-freesurfer` program as a
//! separate process (`docs/licensing.md`). Empty until its queue items bring the code
//! (`docs/freesurfer/queue.md`); `PROVENANCE.md` lists what each item translates.
//!
//! [`PROGRAMS`] lists the programs of this crate that `larmorx-freesurfer` runs (one line each).
#![forbid(unsafe_code)]

use larmorx_freesurfer_core::run::Program;

/// The programs of this crate, by their original names.
pub const PROGRAMS: &[Program] = &[];
