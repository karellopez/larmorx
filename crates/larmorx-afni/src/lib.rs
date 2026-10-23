// SPDX-License-Identifier: Apache-2.0
//! AFNI-compatible tools for larmorx (PLAN.md §5).
//!
//! AFNI's programs are GPL-2, so these are **clean-room** re-implementations: they were
//! written from behaviour specs (`specs/<tool>.md`), AFNI's published documentation and
//! black-box runs of the AFNI binaries, without access to AFNI's source code. See
//! `PROVENANCE.md`.
//!
//! - `3dTshift` (slice-timing correction) is [`tshift::tshift`], on datasets held the way AFNI
//!   holds them ([`dataset::Bricks`]), read and written with AFNI's NIfTI rules
//!   ([`dataset::read`], [`dataset::write`]).
//! - The original command lines are in [`cli`].
#![forbid(unsafe_code)]

pub mod automask;
pub mod cfmt;
pub mod cli;
pub mod dataset;
pub mod fft;
pub mod geometry;
pub mod input;
pub mod msg;
pub mod oned;
pub mod output;
pub mod settings;
pub mod timing;
pub mod tshift;
pub mod x87;

pub use dataset::{BrickData, Bricks};
pub use tshift::{Method, Restore, TshiftError, TshiftParams, tshift};
