// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_normalize` (queue items 3.2-24 and 3.2-25): FreeSurfer 8.2.0's
//! `mri_normalize/mri_normalize.cpp`, on the normalisation of `larmorx-freesurfer-filter`'s
//! `normalize` (`utils/mrinorm.cpp`, queue item 3.1-43).
//!
//! - [`helpers`]: the program's own functions besides `main` (3.2-24).
//! - [`options`]: its options (`get_option`) and its help (`usage_exit`) (3.2-24).
//! - [`main`]: the program itself (`main`, 3.2-25), its entry point [`main::run`].

pub mod helpers;
pub mod main;
pub mod options;
