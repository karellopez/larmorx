// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_fill` (queue items 3.2-34 to 3.2-37, wave 3.2's track D): FreeSurfer 8.2.0's
//! `mri_fill/mri_fill.cpp`, which fills the white matter of each hemisphere from its seeds and
//! cutting planes (recon-all: `-a ../scripts/ponscc.cut.log -xform transforms/talairach.lta
//! -segmentation aseg.presurf.mgz -ctab SubCorticalMassLUT.txt wm.mgz filled.mgz`).
//!
//! In progress: the program is assembled at 3.2-37 (not yet registered).
//!
//! - [`topofix`]: 3.2-34, the topological correction of `-topofix` (`mri_topofix` and its three
//!   removals), `findMinSize`, `verifyLRSplit`.

pub mod topofix;
