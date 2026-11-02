// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! A bit-exact replica of FSL's FAST (FSL 6.0.7.23: fsl-fast4 2111.3), FMRIB's Automated
//! Segmentation Tool, translated from FSL's source under the FSL Licence's terms
//! (non-commercial; the licence and FSL's original source travel with it: `crates-nc/LICENSE`,
//! `crates-nc/fsl-source/`). FSL's libraries (newimage, NewNifti, utils) are the shared core's
//! (`larmorx-nc-core`).
//!
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-nc` program
//! (`larmorx-nc fsl fast`) as a separate process (`docs/licensing.md`).
//!
//! - [`fast`]: the command line, the input, the files written.
//! - [`mriseg`]: the single-channel segmentation (`ZMRISegmentation`).
//! - [`multi_mriseg`]: the multi-channel segmentation (`ZMRIMULTISegmentation`, `-S n`).
//! - [`cli`]: the command line (`fsl fast`) as `larmorx-nc` runs it.
#![forbid(unsafe_code)]
// Index loops mirror FSL's loops, so that each step can be checked against the source.
#![allow(clippy::needless_range_loop)]

pub mod cli;
pub mod fast;
pub mod mriseg;
pub mod multi_mriseg;

pub use cli::fast;
