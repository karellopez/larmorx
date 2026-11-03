// SPDX-License-Identifier: Apache-2.0
//! fslmaths, FSL's image calculator (`specs/fslmaths.md`), written from its spec, FSL's usage
//! text and documentation and black-box runs of FSL 6.0.7.23's program, without access to
//! FSL's source code or its bit-exact replica (`PROVENANCE.md`).
//!
//! - [`num`]: the computing types and C's arithmetic and conversions on x86-64 (§2.4, §4);
//! - [`image`]: reading in a computing type and writing the output (§3);
//! - [`ops`]: the binary operations, thresholds, masks, voxelwise functions, random numbers,
//!   index and grid (§5-§8).
//!
//! The command line (`larmorx fsl fslmaths`) is [`crate::cli::fslmaths`].

pub mod image;
pub mod num;
pub mod ops;
