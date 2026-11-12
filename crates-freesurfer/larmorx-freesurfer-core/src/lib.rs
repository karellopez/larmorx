// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-core: `fs.core` (queue item 3.1-22) and `fs.maths` (3.1-23, 3.1-24) of
//! FreeSurfer 8.2.0's replica (docs/freesurfer/waves.md section 4): FreeSurfer's messages,
//! diagnostics, program name, environment, version stamps, the ROMP switch, timers, paths, signals
//! and the licence check's messages (`fs.core`); its matrices (float and long double), the numerics
//! wrappers over VNL and Cephes, `fs_powell`, `fs_lbfgs`, histograms, statistics, PDFs and random
//! fields (`fs.maths`).
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-freesurfer` program as a
//! separate process (`docs/licensing.md`). Its queue items bring the code
//! (`docs/freesurfer/queue.md`); `PROVENANCE.md` lists what each item translates.
//!
//! - [`run`]: what the program `larmorx-freesurfer` gives a program's replica (its settings, an
//!   environment table, the entry point every program crate registers); larmorx's own plumbing.
//! - `fs.core` (queue item 3.1-22; the behaviour: `specs/freesurfer-core.md`): [`globals`]
//!   (FreeSurfer's process-wide state, one value per run), [`log`] (messages, fatal errors, the
//!   error codes; FS-01), [`diag`] (the diagnostic bits and `DiagInit`; FS-02, FS-03),
//!   [`machine`] (byte swaps), [`cmdargs`], [`base`]; [`cstd`], the C library's conversions they
//!   need (larmorx's own code).
#![forbid(unsafe_code)]

pub mod base;
pub mod cmdargs;
pub mod cstd;
pub mod diag;
pub mod globals;
pub mod log;
pub mod machine;
pub mod run;
