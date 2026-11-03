// SPDX-License-Identifier: Apache-2.0
//! FAST's tissue segmentation (FSL 6.0.7.23's `fast`, FAST4), a clean-room original written
//! from `specs/fast.md`, Y. Zhang, M. Brady and S. Smith's paper ("Segmentation of brain MR
//! images through a hidden Markov random field model and the expectation-maximization
//! algorithm", IEEE TMI 20(1):45-57, 2001), FSL's FAST documentation and black-box runs of
//! FSL's program (see `PROVENANCE.md`).
//!
//! - [`single`]: one channel (§4): the mask, the log image, the initialisation, the MRF
//!   iterations, the bias field, the class order and the hard segmentation; the priors (§6);
//! - [`multi`]: several channels (§5);
//! - [`pve`]: the partial volumes (§4.8);
//! - [`bias`]: the bias field's kernels and separable smoothing (§4.5, §8.5);
//! - [`grid`]: the mask's voxels and their MRF neighbourhood (§4.2);
//! - [`stats`]: FSL's blocked sums, the percentiles, the class statistics and the Gaussian
//!   (§4.4, §8.2, §8.3);
//! - [`rng`]: the C library's random numbers FAST draws (§8.1).
//!
//! The command line is [`crate::cli::fast`].

pub mod bias;
pub mod grid;
pub mod multi;
pub mod pve;
pub mod rng;
pub mod single;
pub mod stats;

pub use single::{Params, Single, Stop, segment};
