// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! Bit-exact replicas of FSL's BET (FSL 6.0.7.17: fsl-bet2 2111.8): the `bet` script, `bet2`
//! and `betsurf`, translated from FSL's source under the FSL Licence's terms (non-commercial;
//! the licence and FSL's original source travel with it: `crates-nc/LICENSE`,
//! `crates-nc/fsl-source/`). FSL's libraries (newimage, NewNifti, meshclass, utils, avwutils)
//! are the shared core's (`larmorx-nc-core`).
//!
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-nc` program
//! (`larmorx-nc fsl bet|bet2|betsurf`) as a separate process (`docs/licensing.md`).
//!
//! - [`bet2`]: the surface model, its options, messages and outputs.
//! - [`bet`]: the `bet` script's options and variations (`-R`, `-F`, `-Z`; the ones that need
//!   FLIRT or FAST refuse and name them).
//! - [`betsurf`]: the skull and scalp surfaces.
//! - [`cli`]: the command lines (`fsl bet`, `fsl bet2`, `fsl betsurf`) as `larmorx-nc` runs them.
#![forbid(unsafe_code)]

pub mod bet;
pub mod bet2;
pub mod betsurf;
pub mod cli;

pub use cli::{bet, bet2, betsurf};
