// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (first from 6.0.7.17; the same source): fsl-mcflirt 2111.0 (mcflirt.cc, Globaloptions.{h,cc},
// Log.{h,cc}); see PROVENANCE.md. Copyright (C) 1999-2001 University of Oxford (Peter
// Bannister). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
// Distributed under the FSL Licence (LICENSE): non-commercial use only, these conditions passed
// on to every receiver, the original and amended source included (fsl-source/).
//! The bit-exact replica of FSL's `mcflirt` (head-motion correction), for larmorx's `larmorx-nc`
//! package.
//!
//! [`mcflirt`] runs the program with mcflirt's arguments: the same options, messages, exit
//! statuses and output files (the corrected series, `.mat` directory, `.par`, `.rms`, `-stats` and
//! `-meanvol` images), the same numbers. mcflirt's deterministic defects are corrected by
//! default and reproduced with `LARMORX_UPSTREAM_DEFECTS=reproduce` (D13; register entries
//! FSL-01, -03, -04, -05 to -10, -13, -14, -20, -21), with the same defaults as larmorx's clean-room
//! original (`larmorx mri hmc`).
#![forbid(unsafe_code)]
// Index loops mirror mcflirt's loops, so that each step can be checked against the source.
#![allow(clippy::needless_range_loop)]

pub mod options;
pub mod run;

pub use run::mcflirt;
