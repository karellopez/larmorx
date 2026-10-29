// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-newimage 2601.0, fsl-miscmaths 2412.6,
// fsl-newnifti 5.0.0, fsl-znzlib 2511.3, fsl-meshclass 2111.0, fsl-utils 2412.6,
// fsl-avwutils 2209.6; first translated from FSL 6.0.7.17), from Armadillo 10.5.1 as
// fsl-armawrap 0.6.0 bundles it (FSL 6.0.7.23's 0.7.0 bundles 14.2.1, whose LAPACK path for
// 4 x 4 inverses is not followed yet), from fslpy 3.27.0's file-name rules, and from the C library routines FSL's binaries call (glibc 2.35:
// fdlibm's `atan2f`, `atanf`, `tanh`, `expm1`; `pow`, `exp`, `sinf`, `cosf`, `logf`); see
// PROVENANCE.md for the files and functions. FSL is Copyright (C) 1999-2018 University of Oxford.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals. Distributed
// under the FSL Licence (LICENSE): non-commercial use only, these conditions passed on to every
// receiver, the original and amended source included (fsl-source/).
//! The shared core of larmorx's FSL replicas (`larmorx-nc`): the parts of FSL's libraries the
//! replicated tools are built on, translated so that they give FSL's bits. Each FSL library is
//! translated once, here; the tool crates (`larmorx-nc-mcflirt`, `larmorx-nc-bet`, ...) build on
//! these modules.
//!
//! - [`arma`]: NEWMAT's small matrices as Armadillo 10.5.1 evaluates them (FSL's `armawrap`).
//! - [`nifti`], [`volume`], [`settings`]: NIfTI-1/2 and ANALYZE files as FSL's NIfTI library
//!   reads and writes them, `volume<T>` with newimage's properties, `FSLOUTPUTTYPE` and the
//!   other environment settings as a table.
//! - [`interp`], [`kernel`], [`newimagefns`]: newimage's interpolation (trilinear, nearest,
//!   windowed sinc, cubic B-spline), `isotropic_resample`, `affine_transform` and the
//!   separable Gaussian `smooth`.
//! - [`costfns`], [`optimise`], [`miscmaths`]: FLIRT's cost functions, miscmaths' line-search
//!   optimiser and the transform algebra (Euler angles, `compose_aff`/`decompose_aff`,
//!   `rms_deviation`, `read_ascii_matrix`).
//! - [`mesh`]: meshclass (triangulated surfaces, their vertex normals and intensity profiles,
//!   the self-intersection test, the file formats FSL reads and writes).
//! - [`options`]: fsl-utils' command-line options (`Utilities::OptionParser`).
//! - [`avwutils`]: the parts of `fslval`, `fslroi`, `fslmerge`, `fslmaths` and `fslstats` (and
//!   fslpy's `imtest`, `remove_ext`, `imglob`) that FSL's scripts run.
//! - [`cnum`], [`x87`], [`sse`], [`fdlibm`], [`glibc_float`], [`glibc_double`], [`istream`],
//!   [`stdsort`]: C and C++ semantics of FSL's x86-64 build (float-to-integer conversions,
//!   `strtod`, `printf` and stream formatting, `long double`, SSE's NaNs, glibc's maths as it
//!   runs on FSL's machines, libstdc++'s formatted input, `std::sort` and `std::nth_element`).
#![forbid(unsafe_code)]
// Index loops mirror FSL's loops, so that each step can be checked against the source.
#![allow(clippy::needless_range_loop)]

pub mod arma;
pub mod avwutils;
pub mod cnum;
pub mod costfns;
pub mod fdlibm;
pub mod glibc_double;
pub mod glibc_float;
pub mod interp;
pub mod istream;
pub mod kernel;
pub mod mesh;
pub mod miscmaths;
pub mod newimagefns;
pub mod nifti;
pub mod optimise;
pub mod options;
pub mod settings;
pub mod sse;
pub mod stdsort;
pub mod volume;
pub mod x87;

/// The version of the FSL release these translations follow.
pub const FSL_VERSION: &str = "6.0.7.23";
