// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-filter: `fs.filter` (wave 3.1) of FreeSurfer 8.2.0's replica
//! (docs/freesurfer/waves.md section 4): volume filters and morphology, intensity normalisation
//! (with Burkardt's spline), segmentation helpers, flood fill, connected components, fast marching
//! and clusters.
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-freesurfer` program as a
//! separate process (`docs/licensing.md`). `PROVENANCE.md` lists what each queue item
//! (`docs/freesurfer/queue.md`) translates.
#![forbid(unsafe_code)]

pub mod cc;
pub mod classify;
pub mod conv;
pub mod fastmarch;
pub(crate) mod mem;
pub mod sobel;
