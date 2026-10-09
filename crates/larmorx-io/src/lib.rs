//! File formats for larmorx (PLAN.md §5), in pure Rust.
//!
//! - [`nifti`]: NIfTI-1 and NIfTI-2 (`.nii`, `.nii.gz`, `.hdr`/`.img` pairs), read and written
//!   with nibabel's semantics.
//! - [`gzip`]: parallel gzip compression whose output does not depend on the thread count.
//! - [`itk_transform`]: ITK transform files (text, MATLAB, NIfTI displacement fields).
//!
//! MGH/MGZ, GIFTI, CIFTI-2 and FreeSurfer surfaces follow.
#![forbid(unsafe_code)]

pub mod gzip;
pub mod itk_transform;
pub mod nifti;
