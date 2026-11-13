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
//! separate process (`docs/licensing.md`). Empty until its queue items bring the code
//! (`docs/freesurfer/queue.md`); `PROVENANCE.md` lists what each item translates.
#![forbid(unsafe_code)]
