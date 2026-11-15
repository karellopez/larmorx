// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/rfa.cpp` (queue item 3.1-43): `MRIcountCSFInNbhd` (690) and its CSF labels (680), the
//! random-forest features' CSF count (`mri_rf_label`, `mri_rf_train`).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_volume::cma::{
    CSF, Fourth_Ventricle, Left_Inf_Lat_Vent, Left_Lateral_Ventricle, Right_Inf_Lat_Vent,
    Right_Lateral_Ventricle, Third_Ventricle, Unknown,
};
use larmorx_freesurfer_volume::mriset::mri_count_val_in_nbhd;

/// rfa.cpp's `csf_labels`, in its order.
pub const CSF_LABELS: [i32; 8] = [
    CSF,
    Unknown,
    Left_Lateral_Ventricle,
    Right_Lateral_Ventricle,
    Left_Inf_Lat_Vent,
    Right_Inf_Lat_Vent,
    Third_Ventricle,
    Fourth_Ventricle,
];

/// `MRIcountCSFInNbhd(mri_seg, wsize, x, y, z)`: the voxels of the `wsize` window around `(x, y,
/// z)` labelled CSF, unknown (0), a lateral or inferior lateral ventricle, or the third or fourth
/// ventricle, counted label by label in that order ([`mri_count_val_in_nbhd`]: positions clamped
/// by the index tables, its error past them), the total an `unsigned int` returned as an `int`.
pub fn mri_count_csf_in_nbhd(
    g: &mut Globals,
    err: &mut dyn Write,
    mri_seg: &Mri,
    wsize: i32,
    x: i32,
    y: i32,
    z: i32,
) -> Result<i32, Fatal> {
    let mut total = 0u32;
    for &label in &CSF_LABELS {
        let n = mri_count_val_in_nbhd(g, err, mri_seg, wsize, x, y, z, label)?;
        total = total.wrapping_add(n as u32);
    }
    Ok(total as i32)
}
