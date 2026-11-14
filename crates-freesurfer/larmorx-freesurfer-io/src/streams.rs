// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! What FreeSurfer's library functions reach through the C library's globals: standard output,
//! standard error (`stdout`, `stderr`) and the environment (`getenv`), passed together. The GIFTI
//! functions ([`crate::gifti_common`], [`crate::gifti_vol`]) and larmorx-freesurfer-surf's surface
//! functions take it (moved here from the surface crate in relay 23 of track D).

use std::io::Write;

use larmorx_freesurfer_core::run::Environment;

/// Standard output, standard error and the environment of the run.
pub struct Io<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'a Environment,
}
