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
//! MGH/MGZ, GIFTI, CIFTI-2 and FreeSurfer surfaces follow.
#![forbid(unsafe_code)]

pub mod gzip;
pub mod itk_hdf5;
pub mod itk_output;
pub mod itk_transform;
mod itk_transform_names;
pub mod nifti;
