// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-cortex: wave 3.5's programs of FreeSurfer 8.2.0's replica
//! (docs/freesurfer/waves.md section 4): placement, registration, parcellation and statistics
//! (`mris_place_surface`, `mris_register`, `mris_ca_label`, `mris_anatomical_stats`, ...),
//! fMRIPrep's surface programs and `bbregister` with `mri_segreg`.
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
