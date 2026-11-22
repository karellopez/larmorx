// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_segstats` (queue item 3.2-20, wave 3.2's track D): FreeSurfer 8.2.0's
//! `mri_segstats/mri_segstats.cpp`, on the volume functions of `larmorx-freesurfer-volume`, the
//! surfaces of `larmorx-freesurfer-surf` (`--annot`, `--slabel`, `--euler`, `--qa-stats`) and the
//! GCA reader of `larmorx-freesurfer-gca` (`--ctab-gca`). The brain-volume statistics come from
//! the subject's cache (`stats/brainvol.stats`); computing them (`ComputeBrainVolumeStats`, no
//! cache or `--no-cached`) is queue item 3.5-36 and stops with FreeSurfer's step named and "is
//! not ported yet".
//!
//! - [`options`]: the command line (`parse_commandline`, with its stand-alone options
//!   `--xfm2etiv`, `--rescale-by-seg`, `--sum-in` and `--qa-stats`), `check_options`,
//!   `dump_options` and the help.
//! - [`main`]: the program itself (`main`), its entry point [`main::run`].
//! - [`table`]: the statistics table (`STATSUMENTRY`, `utils/stats.cpp`'s `PrintSegStat`, the
//!   program's `LoadStatSumFile` and `DumpStatSumTable`).
//! - [`qa`]: `CountEdits`, `WMAnatStats` and `MRIrescaleBySeg`.
//! - [`gtm`]: `utils/gtm.cpp`'s replacement lists (`GTMdefaultSegReplacmentList`,
//!   `GTMloadReplacmentList`).

pub mod gtm;
pub mod main;
pub mod options;
pub mod qa;
pub mod table;
