// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mris_calc` (queue item 3.2-21, wave 3.2's track D): FreeSurfer 8.2.0's
//! `mris_calc/mris_calc.cpp`, whole: its volume operations (`mri_nu_correct.mni`'s rescale,
//! `label-cortex`'s masks) and its curvature files (`vertexvol`'s areas, queue item 3.5-37's
//! checks), on the volume reader and writer of `larmorx-freesurfer-io` and the column printing
//! of `larmorx-freesurfer-surf`.
//!
//! - [`ops`]: the operations of one and two values and the running statistics, with the
//!   official build's arithmetic and NaNs.
//! - [`main`]: the program itself (`main`, the options, the readers and writers), its entry
//!   point [`main::run`].

pub mod main;
pub mod ops;
