// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-io: `fs.io` (3.1-25 to 3.1-32) and `fs.labels` (3.1-33, 3.1-34) of FreeSurfer
//! 8.2.0's replica (docs/freesurfer/waves.md section 4): the volume formats (`MRIread`/`MRIwrite`:
//! MGH/MGZ, NIfTI, ANALYZE, the format probe, the other readers; FreeSurfer's tags, `.mat`, `.npy`,
//! GIFTI volumes), FreeSurfer's changes to MINC's files, and colour tables, annotations, labels,
//! `register.dat`, control points, point sets and CSV (`fs.labels`).
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-freesurfer` program as a
//! separate process (`docs/licensing.md`). Empty until its queue items bring the code
//! (`docs/freesurfer/queue.md`); `PROVENANCE.md` lists what each item translates.
#![forbid(unsafe_code)]

pub mod bfileio;
pub mod bruker;
pub mod cfile;
pub mod colortab;
pub mod colortab_bin;
pub mod dicom_probe;
pub mod dti;
pub mod fio;
pub mod fscnpy;
pub mod fstagsio;
pub mod gifti;
pub mod identify;
pub mod image;
pub mod matfile;
pub mod minc;
pub mod mosaic;
pub mod mriio;
pub mod mriio_old;
pub mod registerio;
pub mod signa;
pub mod tags;
