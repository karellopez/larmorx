// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_convert` (queue items 3.2-04 to 3.2-07): FreeSurfer 8.2.0's `mri_convert/mri_convert.cpp`.
//!
//! - [`csv`]: `include/csv.h`'s CSV reader (3.2-04).
//! - [`stat_table`]: the stats tables of `--in_stats_table` and `--out_stats_table`
//!   (`utils/stats.cpp`, `utils/fsgdf.cpp`'s item count; 3.2-04, taken from 3.2-07).
//! - [`options`]: `main`'s start, its option loop and the checks before the input is read, with
//!   `get_ints`, `get_floats`, `get_string` and the help (3.2-05).
//! - [`main`]: the rest of `main` and `applyGaussianFilter`, the program's entry point (3.2-05).
//! - [`lta_dump`]: `transform.cpp`'s `LTAdump`, `LTAdumpLinearTransform` and
//!   `LTAtransformTypeName` (3.2-07's lines).
//! - [`otl`]: the outline reader `MRIreadOtl` (3.2-06).
//!
//! `include/affine.h`, the rest of 3.2-04, is `larmorx-freesurfer-core`'s `affine` (wave 3.1).

pub mod csv;
pub mod lta_dump;
pub mod main;
pub mod options;
pub mod otl;
pub mod stat_table;
