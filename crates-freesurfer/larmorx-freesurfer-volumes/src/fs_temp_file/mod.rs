// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `fs_temp_file` (queue item 3.2-11): FreeSurfer 8.2.0's `io/fs_temp_file.cpp`, which makes a new
//! empty temporary file and prints its name (FreeSurfer's scripts: `` set f = `fs_temp_file
//! --suffix .txt` ``).
//!
//! - [`main`]: `main`: the program's entry point, on FreeSurfer's `ArgumentParser`
//!   (`larmorx_freesurfer_core::argparse`) and `makeTempFile`
//!   (`larmorx_freesurfer_core::utils::make_temp_file`).

pub mod main;
