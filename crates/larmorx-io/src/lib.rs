//! File formats for larmorx (PLAN.md §5).
//!
//! This crate will read and write NIfTI-1/2, MGH/MGZ, GIFTI, CIFTI-2, FreeSurfer
//! surf/curv/label/annot/LTA files and ITK transforms, in pure Rust. In phase L0 it is empty;
//! NIfTI I/O is the next step (PLAN.md §15, step 5).
#![forbid(unsafe_code)]
