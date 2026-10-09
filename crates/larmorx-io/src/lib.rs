//! File formats for larmorx (PLAN.md §5), in pure Rust.
//!
//! - [`nifti`]: NIfTI-1 and NIfTI-2 (`.nii`, `.nii.gz`, `.hdr`/`.img` pairs), read and written
//!   with nibabel's semantics.
//! - [`gzip`]: parallel gzip compression whose output does not depend on the thread count.
//!
//! MGH/MGZ, GIFTI, CIFTI-2, FreeSurfer surfaces and ITK transforms follow.
#![forbid(unsafe_code)]

pub mod gzip;
pub mod nifti;
