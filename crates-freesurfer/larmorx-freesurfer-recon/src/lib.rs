// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-recon: wave 3.7 of FreeSurfer 8.2.0's replica (docs/freesurfer/waves.md
//! section 4): `recon-all` (its steps, flags, checks, logs and the resume logic sMRIPrep uses) and
//! its helpers, which run the other crates' programs in-process.
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
