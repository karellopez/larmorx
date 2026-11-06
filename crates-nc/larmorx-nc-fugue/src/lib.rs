// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! A bit-exact replica of FSL's PRELUDE (FSL 6.0.7.23: fsl-fugue 2201.5), Phase Region
//! Expanding Labeller for Unwrapping Discrete Estimates (Jenkinson 2003, "Fast, automated,
//! N-dimensional phase-unwrapping algorithm", MRM 49:193-197), translated from FSL's source
//! under the FSL Licence's terms (non-commercial; the licence and FSL's original source travel
//! with it: `crates-nc/LICENSE`, `crates-nc/fsl-source/`). FSL's libraries (newimage,
//! NewNifti, utils) are the shared core's (`larmorx-nc-core`).
//!
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-nc` program
//! (`larmorx-nc fsl prelude`) as a separate process (`docs/licensing.md`).
//!
//! - [`prelude`]: the command line, the inputs, the files written.
//! - [`unwarpfns`]: the masks, phase ramps, labels and the unwrapping (`unwarpfns.cc`).
//! - [`cli`]: the command line (`fsl prelude`) as `larmorx-nc` runs it.
#![forbid(unsafe_code)]
// Index loops mirror FSL's loops, so that each step can be checked against the source.
#![allow(clippy::needless_range_loop)]

pub mod cli;
pub mod prelude;
pub mod unwarpfns;

pub use cli::prelude;
