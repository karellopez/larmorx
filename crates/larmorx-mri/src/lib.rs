// SPDX-License-Identifier: Apache-2.0
//! Clean-room MRI tools for larmorx (PLAN.md §3.3), named by function rather than by the tool
//! they are compatible with.
//!
//! - [`hmc`]: head-motion correction, compatible with FSL's `mcflirt` (its options, its
//!   matrices and its numbers). Written from `specs/mcflirt.md`, the published papers, FSL's
//!   documentation and black-box runs of the FSL binary, without access to FSL's source code
//!   (see `PROVENANCE.md`).
//! - [`cli`]: the command lines (`larmorx mri hmc`, accepting mcflirt-style options).
#![forbid(unsafe_code)]

pub mod cli;
pub mod hmc;
