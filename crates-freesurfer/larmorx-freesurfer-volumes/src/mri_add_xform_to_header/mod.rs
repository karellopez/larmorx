// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_add_xform_to_header` (queue item 3.2-10): FreeSurfer 8.2.0's
//! `mri_add_xform_to_header/mri_add_xform_to_header.cpp`, which writes a transform's file name
//! into a volume's header (recon-all: `-c transforms/talairach.xfm orig.mgz orig.mgz` and the same
//! for `nu.mgz`).
//!
//! - [`main`]: `main`, `get_option`, the usage: the program's entry point.
//! - [`write_info`]: `MRIwriteInfo` (`utils/mriio.cpp`), the `COR-.info` writer the program uses
//!   when its output is a folder (decided for the program's crate: no other program on the
//!   default paths calls it).

pub mod main;
pub mod write_info;
