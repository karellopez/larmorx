// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09 (see PROVENANCE.md for the files and functions).
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released under the GNU GPL version 2 or any later version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! Bit-exact replicas of AFNI tools for larmorx (the `larmorx-gpl` package,
//! GPL-3.0-or-later).
//!
//! These are step-by-step translations of AFNI 25.2.09's C source, written to give the same
//! output bits as AFNI. The Apache-2.0 `larmorx` package has clean-room originals of the same
//! tools (`larmorx-afni`); see `docs/licensing.md`.
//!
//! - `3dTshift` (slice-timing correction): [`tshift::tshift`] on a [`dataset::Dataset`] read
//!   and written with AFNI's NIfTI rules ([`dataset::read`], [`dataset::write`]); the command
//!   line is [`cli::tshift`].
//! - Its numerical kernels: AFNI's FFT ([`csfft`]), the time-shift interpolators ([`shift`]),
//!   detrending ([`detrend`]), and a port of glibc's `sinf`/`cosf` ([`glibc_sincosf`]) for the
//!   weighted-sinc method.
//! - Inputs: slice patterns ([`tpattern`]) and AFNI's 1D text files ([`oned`]).
#![forbid(unsafe_code)]

pub mod afni_ext;
pub mod cli;
pub mod cnum;
pub mod csfft;
pub mod dataset;
pub mod detrend;
pub mod glibc_sincosf;
pub mod oned;
pub mod shift;
pub mod tpattern;
pub mod tshift;
