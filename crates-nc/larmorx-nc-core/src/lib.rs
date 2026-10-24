// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17 (fsl-newimage 2203.12, fsl-miscmaths 2203.2,
// fsl-newnifti 4.1.0, fsl-znzlib 2111.0) and from Armadillo 10.5.1 as fsl-armawrap 0.6.0 bundles
// it; see PROVENANCE.md for the files and functions. FSL is Copyright (C) 1999-2018 University
// of Oxford. Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
// Distributed under the FSL Licence (LICENSE): non-commercial use only, these conditions
// passed on to every receiver, the original and amended source included (fsl-source/).
//! The shared core of larmorx's FSL replicas (`larmorx-nc`): the parts of FSL's libraries the
//! registration tools are built on, translated so that they give FSL's bits.
//!
//! - [`arma`]: NEWMAT's small matrices as Armadillo 10.5.1 evaluates them (FSL's `armawrap`).
//! - [`nifti`], [`volume`], [`settings`]: NIfTI-1/2 and ANALYZE files as FSL's NIfTI library
//!   reads and writes them, `volume<float>` with newimage's properties, `FSLOUTPUTTYPE` and
//!   the other environment settings as a table.
//! - [`interp`], [`kernel`], [`newimagefns`]: newimage's interpolation (trilinear, nearest,
//!   windowed sinc, cubic B-spline), `isotropic_resample` and `affine_transform`.
//! - [`costfns`], [`optimise`], [`miscmaths`]: FLIRT's cost functions, miscmaths' line-search
//!   optimiser and the transform algebra (Euler angles, `compose_aff`/`decompose_aff`,
//!   `rms_deviation`, `read_ascii_matrix`).
//! - [`cnum`], [`x87`], [`fdlibm`]: C semantics of FSL's x86-64 build (float-to-integer
//!   conversions, `long double`, `printf` and stream formatting, glibc's `atan2f`).
//!
//! The API is written for the other FSL tools to come (FLIRT, BET, FAST, topup): a tool crate
//! (`larmorx-nc-mcflirt`, `larmorx-nc-bet`, ...) builds on these modules, and code two tools share
//! moves here.
#![forbid(unsafe_code)]
// Index loops mirror FSL's loops, so that each step can be checked against the source.
#![allow(clippy::needless_range_loop)]

pub mod arma;
pub mod cnum;
pub mod costfns;
pub mod fdlibm;
pub mod glibc_float;
pub mod interp;
pub mod kernel;
pub mod miscmaths;
pub mod newimagefns;
pub mod nifti;
pub mod optimise;
pub mod settings;
pub mod volume;
pub mod x87;

/// The version of the FSL release these translations follow.
pub const FSL_VERSION: &str = "6.0.7.17";
