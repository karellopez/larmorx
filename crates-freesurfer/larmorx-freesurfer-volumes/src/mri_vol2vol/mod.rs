// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_vol2vol` (queue item 3.2-38): FreeSurfer 8.2.0's `mri_vol2vol/mri_vol2vol.cpp`, which
//! resamples a volume into another's field of view through a registration (tkregister's
//! `register.dat`, an `.lta`, FSL's or MNI's matrix, or the scanner coordinates), or rewrites its
//! voxel-to-RAS matrix (`--no-resample`). The v8 stream's scripts (`fs-synthmorph-reg`,
//! `mri_mcadura_seg`, `mri_vsinus_seg`) and sMRIPrep's `ApplyVolTransform` run it.
//!
//! - [`options`]: `parse_commandline`, `check_options`, `dump_options`, the usage and the help.
//! - [`main`]: `main`: the program's entry point.
//!
//! Relay 53 ports every option's reading and checks and every path but the non-linear ones,
//! which end the run (status 1) as not ported yet: the morphs (`--morph`, `--inv-morph`, `--m3z`:
//! `GCAMmorphToAtlas`/`GCAMmorphFromAtlas`), `--gcam`, `--spm-warp`, `--map-point`, the affine
//! displacement (`--disp`) and the registration cost (`--cost`).

pub mod main;
pub mod options;
