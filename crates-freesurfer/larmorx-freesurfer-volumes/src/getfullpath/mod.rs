// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `getfullpath` (queue item 3.2-12): FreeSurfer 8.2.0's tcsh script `scripts/getfullpath`, which
//! prints a file's full name (its folder's physical path, `/`, its last component); seg2cc,
//! rca-talairach and fs-synthmorph-reg run it on their inputs.
//!
//! - [`main`]: the script, both as its name says (the default) and as tcsh runs it
//!   (`reproduce`, on [`crate::csh`]).

pub mod main;
