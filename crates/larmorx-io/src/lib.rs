// SPDX-License-Identifier: Apache-2.0
//! File formats for larmorx (PLAN.md §5), in pure Rust.
//!
//! - [`nifti`]: NIfTI-1 and NIfTI-2 (`.nii`, `.nii.gz`, `.hdr`/`.img` pairs), read and written
//!   with nibabel's semantics.
//! - [`gzip`]: parallel gzip compression whose output does not depend on the thread count.
//! - [`itk_transform`]: ITK transform files (text, MATLAB, HDF5, NIfTI displacement fields).
//! - [`itk_hdf5`]: ITK's HDF5 transform files, read and written as ITK v5.4.5 reads and writes
//!   them (through `larmorx-hdf5`).
//!
//! - [`znz`]: znzlib's files, plain or gzip (nifticlib 2.0.0 over zlib 1.2.11's file layer).
//! - [`xfm`]: MNI transform files (`.xfm`), read and written as MINC 1.5.1's volume_io does.
//!
//! - [`dicom`]: DICOM files opened as CTN 3.0.6's object library opens them (`DCM_OpenFile`).
//! - [`gifti`]: GIFTI files, read and written as gifticlib 1.12 does, over [`xml`], a pure-Rust
//!   parser with expat 2.0.1's events and failures.
//!
//! MGH/MGZ, CIFTI-2 and FreeSurfer surfaces follow.
#![forbid(unsafe_code)]

pub mod dicom;
pub mod gifti;
pub mod gzip;
pub mod itk_hdf5;
pub mod itk_output;
pub mod itk_transform;
mod itk_transform_names;
pub mod nifti;
pub mod xfm;
pub mod xml;
pub mod znz;
