// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-deform: `fs.deform` (wave 3.4) of FreeSurfer 8.2.0's replica
//! (docs/freesurfer/waves.md section 4): surface deformation: the time steps, the integration, the
//! gradients and the SSE terms.
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-freesurfer` program as a
//! separate process (`docs/licensing.md`). `PROVENANCE.md` lists what each item translates; the
//! first pieces (queue item 3.4-06, `utils/mrisurf_deform.cpp`) were taken early in relay 44 of
//! track D: [`project`] (the projection of a surface back onto its kind, the flattening of a
//! patch) and [`intersect`] (`MRISremoveIntersections`); relay 48 added [`tess`] (the
//! tessellation of a volume, `utils/mrisurf_timeStep.cpp`, and `MRIvolTopoFix`, for
//! `mri_binarize`).
#![forbid(unsafe_code)]

pub mod intersect;
pub mod project;
pub mod tess;
