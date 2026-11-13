// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRIS::lta`'s type (`LTA *`, `include/transform.h`'s `LINEAR_TRANSFORM_ARRAY`) and the two
//! things a surface does with it: `LTAcopy(lta, NULL)` in `MRIScopyMetadata` and `LTAfree` in
//! `MRISdtr`.
//!
//! FreeSurfer's structure itself, from `larmorx-freesurfer-mri`'s [`larmorx_freesurfer_mri::lta`]
//! (track A took it early from fs.volume's queue item 3.1-40, 2026-11-13); until then this module
//! held a placeholder of the four fields a surface's copy and free carried over (relay 9 of
//! fs.surfio). Its `LTAcopy` and `LTAfree` are fs.mri's: [`lta_copy`] and [`lta_free`].

pub use larmorx_freesurfer_mri::lta::{LinearTransform, Lta, lta_alloc, lta_copy, lta_free};
