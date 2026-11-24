// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_label2vol` (queue item 3.2-40): FreeSurfer 8.2.0's `mri_label2vol/mri_label2vol.cpp`,
//! which turns labels, a surface annotation or a segmentation into a volume (recon-all's
//! `defect2seg`: `--defects <surf> <defect_labels> <template> <offset> <merge> <out> <label>`, in
//! both streams; `seg2cc`'s `--seg <seg> --temp <vol> --regheader --o <out>` for heads that are
//! not conformed).
//!
//! - [`options`]: `parse_commandline`, `check_options`, `dump_options`, the usage, help and
//!   version texts.
//! - [`main`]: `main`, `get_crs`, `is_surface_label`, `load_annotation` (the program's own
//!   functions) and the `--defects` path.
//! - [`surface`]: the surface library's functions only this program reaches in wave 3.2:
//!   `MRISloadSurfSubject` (`utils/mrisutils.cpp`), `ProjNormDist` and `ProjNormFracThick`
//!   (`utils/resample.cpp`), `annotation2label` (`utils/annotation.cpp`).

pub mod main;
pub mod options;
pub mod surface;
