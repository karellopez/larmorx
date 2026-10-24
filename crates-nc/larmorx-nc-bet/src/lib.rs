// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! Bit-exact replicas of FSL's BET (FSL 6.0.7.17: fsl-bet2 2111.8): the `bet` script, `bet2`
//! and `betsurf`, translated from FSL's source under the FSL Licence's terms (non-commercial;
//! the licence and FSL's original source travel with it: `crates-nc/LICENSE`,
//! `crates-nc/fsl-source/`).
//!
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-nc` program (here, for
//! the time being, `larmorx-nc-bet`) as a separate process (`docs/licensing.md`).
//!
//! - [`bet2`]: the surface model, its options, messages and outputs.
//! - [`bet`]: the `bet` script's options and variations (`-R`, `-F`, `-Z`; the ones that need
//!   FLIRT or FAST refuse and name them).
//! - [`betsurf`]: the skull and scalp surfaces.
//! - [`fsl`]: FSL's library behaviour these depend on, and [`mesh`]: its triangle meshes.
//! - [`cli`]: the command lines (`fsl bet`, `fsl bet2`, `fsl betsurf`).
#![forbid(unsafe_code)]

pub mod bet;
pub mod bet2;
pub mod betsurf;
pub mod cli;
pub mod fsl;
pub mod mesh;
