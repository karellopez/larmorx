// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-gca: `fs.gca` (wave 3.3) of FreeSurfer 8.2.0's replica
//! (docs/freesurfer/waves.md section 4): GCA atlases and GCA morphs (`.gca`, `.m3z`), the linear
//! GCA nodes, MRI morphs, FLASH and the autoencoder.
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-freesurfer` program as a
//! separate process (`docs/licensing.md`). Its queue items (`docs/freesurfer/queue.md`, 3.3-01
//! to 3.3-09) bring the code; so far the pieces `mri_read` needs to read an atlas as a volume
//! (`readGCA`, track A's relay 35 of FreeSurfer's foundations) and a morph as a volume (`GCAMread`
//! and `mri_read`'s branch of type `MGH_MORPH`, relay 38), `TRANSFORM` and the morph writer
//! (`MRIwriteType`'s branch of type `MGH_MORPH`, relay 39), the autoencoder as a volume (`SAEread`,
//! `SAElayerWeightsToMRI` and `mri_read`'s branch of type `MGH_AUTOENCODER`, relay 41), `MRIStransform` (a surface through a `TRANSFORM`, relay 45); `PROVENANCE.md` lists what each piece
//! translates.
#![forbid(unsafe_code)]

pub mod autoencoder;
pub mod cma_names;
pub mod concat;
pub mod gca;
pub mod gcam;
pub mod gcam_write;
pub mod likely;
pub mod morph;
pub mod read;
pub mod surftransform;
pub mod transform;

/// Registers this crate's readers and writer with fs.io's `mri_read` and `MRIwriteType` (the atlas,
/// the morph and the autoencoder as volumes, a three-frame volume as a morph).
pub fn register_io() {
    likely::register_io();
    autoencoder::register_io();
    gcam::register_io();
    gcam_write::register_io();
}
