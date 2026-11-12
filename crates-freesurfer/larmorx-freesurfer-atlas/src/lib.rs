// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-atlas: wave 3.3's programs and wave 3.6's `mri_ca_label` of FreeSurfer
//! 8.2.0's replica (docs/freesurfer/waves.md section 4): the atlas programs (`mri_em_register`,
//! `mri_ca_normalize`, `mri_ca_register`, `mri_ca_label`).
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
