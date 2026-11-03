// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! Bit-exact replicas of FSL's avwutils programs (FSL 6.0.7.23: fsl-avwutils 2209.6): the image
//! calculator `fslmaths` and the image statistics `fslstats`, translated from
//! FSL's source under the FSL Licence's terms (non-commercial; the licence and FSL's original
//! source travel with it: `crates-nc/LICENSE`, `crates-nc/fsl-source/`). FSL's libraries
//! (newimage, NewNifti, miscmaths, cprob) are the shared core's (`larmorx-nc-core`), as are the
//! parts of these programs that FSL's scripts run (`larmorx_nc_core::avwutils`).
//!
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-nc` program
//! (`larmorx-nc fsl fslmaths`) as a separate process (`docs/licensing.md`).
//!
//! - [`fslmaths`]: the program (`main`, `inputParser`, `loadNewImage`).
//! - [`usage`]: the usage texts.
//! - `fslstats`: the program is the shared core's (`larmorx_nc_core::fslstats`), since BET's
//!   script runs it too.
//! - [`cli`]: the command lines as `larmorx-nc` runs them.
#![forbid(unsafe_code)]
// Index loops mirror FSL's loops, so that each step can be checked against the source.
#![allow(clippy::needless_range_loop)]

pub mod cli;
pub mod fslmaths;
pub mod usage;

pub use cli::{fslmaths, fslstats};
