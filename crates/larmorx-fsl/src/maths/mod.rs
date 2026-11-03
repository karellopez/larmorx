// SPDX-License-Identifier: Apache-2.0
//! fslmaths, FSL's image calculator (`specs/fslmaths.md`), written from its spec, FSL's usage
//! text and documentation and black-box runs of FSL 6.0.7.23's program, without access to
//! FSL's source code or its bit-exact replica (`PROVENANCE.md`).
//!
//! - [`num`]: the computing types and C's arithmetic and conversions on x86-64 (§2.4, §4);
//! - [`image`]: reading in a computing type and writing the output (§3);
//! - [`ops`]: the binary operations, thresholds, masks, voxelwise functions, random numbers,
//!   index and grid (§5-§8);
//! - [`kernel`]: the kernels of the spatial filters (§9);
//! - [`filters`]: the morphological filters, `-dilall` and the convolutions (§10.1, §10.2);
//! - [`spatial`]: subsampling, edges, the difference of Gaussians, hole filling and the
//!   intensity normalisations (§10.3-§10.7);
//! - [`reduce`]: the reductions along a dimension (§11);
//! - [`stats`]: the statistical transforms and `-randn` (§12, §8.2);
//! - [`regions`]: `-roi` and `-bptf` (§13);
//! - [`tfce`]: threshold-free cluster enhancement (§14).
//!
//! The command line (`larmorx fsl fslmaths`) is [`crate::cli::fslmaths`].

pub mod filters;
pub mod image;
pub mod kernel;
pub mod num;
pub mod ops;
pub mod reduce;
pub mod regions;
pub mod spatial;
pub mod stats;
pub mod tfce;
