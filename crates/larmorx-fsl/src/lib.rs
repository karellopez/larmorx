// SPDX-License-Identifier: Apache-2.0
//! Clean-room originals of FSL's tools for larmorx (CLAUDE.md rule 1, `docs/licensing.md`).
//!
//! - [`bet`]: brain extraction compatible with FSL's `bet2` and the `bet` script. Written from
//!   `specs/bet.md`, S. M. Smith's paper (Human Brain Mapping 17(3), 2002), FSL's
//!   documentation and black-box runs of FSL 6.0.7.23's programs, without access to FSL's
//!   source code or its bit-exact replica (see `PROVENANCE.md`).
//! - [`cli`]: the command lines (`larmorx fsl bet2`, `larmorx fsl bet`, FSL's arguments).
#![forbid(unsafe_code)]

pub mod bet;
pub mod cli;
