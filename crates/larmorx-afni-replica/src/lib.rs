// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05 (see PROVENANCE.md for the files and functions). Major
// portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released by AFNI 26.3.05 under the Creative Commons Attribution 4.0 International licence (CC BY
// 4.0).
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05 (first
// translated from AFNI_25.2.09).
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! Bit-exact replicas of AFNI tools for larmorx (Apache-2.0, with CC BY 4.0's attribution for
//! the Medical College of Wisconsin's code; `PROVENANCE.md`, `docs/licensing.md`).
//!
//! These are step-by-step translations of AFNI 26.3.05's C source, written to give the same
//! output bits as AFNI. larmorx also has clean-room originals of the same tools, built for speed
//! (`larmorx-afni`); `implementation="auto" | "replica" | "original"` chooses (the command line:
//! `LARMORX_IMPLEMENTATION`). Until 2026-10-29 this crate was the GPL-3.0-or-later package
//! `larmorx-gpl`, run as a separate program; AFNI 26.3.05 releases MCW's code under CC BY 4.0,
//! and the replicas now run inside larmorx.
//!
//! - `3dTshift` (slice-timing correction): [`tshift::tshift`] on a [`dataset::Dataset`] read
//!   and written with AFNI's NIfTI rules ([`dataset::read`], [`dataset::write`]); the command
//!   line is [`cli::tshift`].
//! - Its numerical kernels: AFNI's FFT ([`csfft`]), the time-shift interpolators ([`shift`]),
//!   detrending ([`detrend`]), and the C library's `sinf`/`cosf` as glibc runs them, translated
//!   from Arm's Optimized Routines ([`sincosf`]), for the weighted-sinc method.
//! - Inputs: slice patterns ([`tpattern`]) and AFNI's 1D text files ([`oned`]).
//! - `3dAutomask` (a brain mask): AFNI's automask and clip level ([`mask`]) and the program's
//!   own steps ([`automask`]); the command line is [`cli::automask`].
//! - `3dUnifize` (white-matter uniformization): the local percentile mean, the scalings and the
//!   T2 inversion ([`unifize`]), with fdlibm's `tanhf` as glibc runs it ([`fdlibm`]); the
//!   command line is [`cli::unifize`].
//! - AFNI's environment variables and global options (`-Dname=value`, `-overwrite`), as a
//!   settings table that never changes the process environment ([`environ`]), and AFNI's
//!   messages ([`message`]).
//! - [`cli::run`] runs a program with the process's environment and directory (the `larmorx`
//!   command line); [`cli::run_in`] with the environment, directory, mode and threads given (the
//!   Python wrappers, in the caller's process).
#![forbid(unsafe_code)]

pub mod afni_ext;
pub mod automask;
pub mod axes;
pub mod cli;
pub mod cnum;
pub mod csfft;
pub mod dataset;
pub mod detrend;
pub mod environ;
pub mod fdlibm;
pub mod mask;
pub mod message;
pub mod oned;
pub mod shift;
pub mod sincosf;
pub mod tpattern;
pub mod tshift;
pub mod unifize;
pub mod x87;
