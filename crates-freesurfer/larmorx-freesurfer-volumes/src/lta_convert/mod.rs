// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `lta_convert` (queue item 3.2-09): FreeSurfer 8.2.0's `lta_convert/lta_convert.cpp`, which
//! converts a linear transform between FreeSurfer's `.lta`, FSL's matrix, MINC's `.xfm`,
//! tkregister's `register.dat`, NiftyReg's, ITK's and a source-voxel matrix (recon-all:
//! `--inlta identity.nofile --src rawavg.mgz --trg orig.mgz --outlta rawavg2orig.lta --subject
//! <s>`; talairach_avi's `--inxfm ... --ltavox2vox`; bbregister's `--inlta ... --outreg`;
//! fMRIPrep's through niworkflows and sMRIPrep).
//!
//! - [`main`]: `main`, `parseNextCommand`, `parseCommandLine`, `printUsage`.
//! - [`formats`]: the readers and writers of each format.
//! - [`geom`]: `upsampleGeom`, `downsampleGeom` and `conformGeom` (`utils/transform.cpp`, decided
//!   for the program's crate: no other program on the default paths calls it).
//! - [`cxx`]: the C++ streams the program reads and writes its text formats with.

pub mod cxx;
pub mod formats;
pub mod geom;
pub mod main;
