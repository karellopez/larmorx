// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! One call of C's `sscanf` or `fscanf` as the surface library's text readers make it
//! (`fs.surfio`, queue item 3.1-48): fs.core's [`Scanf`] (larmorx-freesurfer-core's `cscan`, since
//! track A's relay 14: this crate's model of relay 12 moved there, with the open
//! `validation/investigations/glibc-scanf-probe/` measurements as its unit test and glibc's
//! end-of-input rules measured further by `validation/investigations/glibc-stdio-probe/`). The
//! surface readers keep calling it through this module.

pub use larmorx_freesurfer_core::cscan::Scanf;
