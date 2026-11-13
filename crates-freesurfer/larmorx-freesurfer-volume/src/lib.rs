// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-volume: `fs.volume` (3.1-35 to 3.1-41) and `fs.bspline` (wave 3.2) of
//! FreeSurfer 8.2.0's replica (docs/freesurfer/waves.md section 4): the `MRI` structure, voxels and
//! frames, conforming, regions, voxel lists, resampling, transforms (LTA, `.xfm`, FSL matrices),
//! the Talairach helpers, warp fields and CMA labels (`fs.volume`); EPFL's B-splines as Reuter's
//! `mriBSpline.cpp` writes them, crediting EPFL (`fs.bspline`).
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-freesurfer` program as a
//! separate process (`docs/licensing.md`). `PROVENANCE.md` lists what each item translates.
#![forbid(unsafe_code)]

pub mod arith;
mod rows;
pub mod vgeom;
