// SPDX-License-Identifier: Apache-2.0
//! PRELUDE's phase unwrapping (Phase Region Expanding Labeller for Unwrapping Discrete
//! Estimates; M. Jenkinson, "Fast, automated, N-dimensional phase-unwrapping algorithm",
//! Magnetic Resonance in Medicine 49:193-197, 2003), the clean-room original of FSL 6.0.7.23's
//! `prelude`, written from `specs/prelude.md`, FSL's documentation and black-box runs of FSL's
//! program, without access to FSL's source code or its bit-exact replica (`PROVENANCE.md`).
//!
//! - [`num`]: the numeric conventions (§6): `rnd`, `wrap`, the intervals, the extrema.
//! - [`components`]: connected components with roots at their first voxel (§4.4, §4.6).
//! - [`merge`]: the constraints and their merging (§4.7, §4.8), a heap instead of FSL's scans.
//! - [`volume`]: one volume (§4.4 to §4.10). The command line is [`crate::cli::prelude`].

pub mod components;
pub mod merge;
pub mod num;
pub mod volume;
