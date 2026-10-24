// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! The parts of FSL's libraries that BET shares with FSL's other programs: newimage's volumes
//! and file names, NewNifti's file layer, miscmaths' and armawrap's arithmetic, utils' option
//! parser, and the C and C++ library behaviour they rely on (glibc's `tanh` and `pow`, x87
//! `long double`, `strtod` and `printf`, libstdc++'s `std::sort`).
//!
//! These modules are meant to move into the package's shared core crate (`larmorx-nc-core`)
//! when the FSL replicas are joined (see this crate's PROVENANCE.md).

pub mod arma;
pub mod cstr;
pub mod fdlibm;
pub mod filename;
pub mod glibc_pow;
pub mod nifti;
pub mod options;
pub mod stdsort;
pub mod volume;
pub mod x87;
