// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_fill` (queue items 3.2-34 to 3.2-37, wave 3.2's track D): FreeSurfer 8.2.0's
//! `mri_fill/mri_fill.cpp`, which fills the white matter of each hemisphere from its seeds and
//! cutting planes (recon-all: `-a ../scripts/ponscc.cut.log -xform transforms/talairach.lta
//! -segmentation aseg.presurf.mgz -ctab SubCorticalMassLUT.txt wm.mgz filled.mgz`).
//!
//! - [`main`]: 3.2-35 and 3.2-37, the program (registered as `mri_fill`) and its options.
//! - [`planes`]: 3.2-36, the cutting planes and seeds (`find_cutting_plane`, `find_pons`, ...).
//! - [`hemi`]: 3.2-36, the fills (`MRIfillVolume`, `MRIcombineHemispheres`,
//!   `MRIfillDegenerateLocations`, `fill_with_aseg`).
//! - [`topofix`]: 3.2-34, the topological correction of `-topofix` (`mri_topofix` and its three
//!   removals), `findMinSize`, `verifyLRSplit`.
//! - [`local`]: the voxel accesses and constants.

pub mod hemi;
pub mod local;
pub mod main;
pub mod planes;
pub mod topofix;
