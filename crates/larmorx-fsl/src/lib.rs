// SPDX-License-Identifier: Apache-2.0
//! Clean-room originals of FSL's tools for larmorx (CLAUDE.md rule 1, `docs/licensing.md`).
//!
//! - [`bet`]: brain extraction compatible with FSL's `bet2` and the `bet` script. Written from
//!   `specs/bet.md`, S. M. Smith's paper (Human Brain Mapping 17(3), 2002), FSL's
//!   documentation and black-box runs of FSL 6.0.7.23's programs, without access to FSL's
//!   source code or its bit-exact replica (see `PROVENANCE.md`).
//! - [`cli`]: the command lines (`larmorx fsl bet2`, `larmorx fsl bet`, FSL's arguments).
#![forbid(unsafe_code)]
// Fixed 3-vectors and 3 x 3 matrices read best indexed; comparisons written so that NaN takes
// the branch FSL's own comparisons give it.
#![allow(clippy::needless_range_loop, clippy::neg_cmp_op_on_partial_ord)]

pub mod bet;
pub mod cli;
