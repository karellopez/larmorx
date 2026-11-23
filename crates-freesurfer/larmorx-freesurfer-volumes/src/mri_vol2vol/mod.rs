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
//! - [`gcam`]: `MRIvol2volGCAM` and `MRIvol2volGCAM0` (`--gcam`, `--gcam0`).
//! - [`standalone`]: the options that do their work while the command line is read
//!   (`--spm-warp`, `--gcam`, `--map-point`).
//!
//! Relay 53 ported every option's reading and checks and every linear path (part 1); relay 54 the
//! non-linear paths (part 2): the morphs (`--morph`, `--inv-morph`, `--m3z`: `GCAMmorphToAtlas`,
//! `GCAMmorphFromAtlas`) and their mode filter (`--filter`), `--gcam`, `--spm-warp`,
//! `--map-point`, the affine displacement (`--disp`) and the registration cost (`--cost`).

pub mod gcam;
pub mod main;
pub mod options;
pub mod standalone;
