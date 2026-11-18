// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FS-12's rule where a label is read from a voxel (the user's rule of 2026-10-11: an undefined
//! float-to-integer conversion is clamped and warned): a NaN label is label 0 by default, with the
//! FS-12 notice the program prints as a warning; with `reproduce` it is x86-64's -2147483648, noted
//! too. Checked through `MRIlabelBoundingBox` (fs.volume's label functions convert every label the
//! same way) and `nint` (fs.core).

use larmorx_core::defects::{FS_12, UpstreamDefects};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MriRegion, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volume::labels::mri_label_bounding_box;

fn bounding_box_of_label_0(mode: UpstreamDefects) -> (MriRegion, Globals) {
    let mut g = Globals::new(mode);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut m = mri_alloc_sequence(&mut g, &mut out, &mut err, 4, 1, 1, MRI_FLOAT, 1).unwrap();
    for x in 0..4 {
        let v = if x == 2 { f32::NAN } else { 5.0 };
        mri_set_vox_val(&mut g, &mut err, &mut m, x, 0, 0, 0, v, 0).unwrap();
    }
    let mut region = MriRegion::default();
    mri_label_bounding_box(&mut g, &mut err, &m, 0, &mut region).unwrap();
    (region, g)
}

#[test]
fn a_nan_label_is_label_0_with_a_warning() {
    let (r, g) = bounding_box_of_label_0(UpstreamDefects::Fix);
    assert_eq!((r.x, r.dx), (2, 1), "the NaN voxel is label 0");
    assert!(g.notices.contains(&FS_12));
    let mut warnings = Vec::new();
    g.notices.print(&mut warnings);
    assert!(String::from_utf8(warnings).unwrap().contains("FS-12"));
}

#[test]
fn reproduce_reads_it_as_int_min_noted_too() {
    let (r, g) = bounding_box_of_label_0(UpstreamDefects::Reproduce);
    assert_eq!((r.x, r.dx), (4, -4), "no voxel of label 0");
    assert!(g.notices.contains(&FS_12));
}

#[test]
fn nint_of_nan() {
    let mut g = Globals::new(UpstreamDefects::Fix);
    assert_eq!(nint(&mut g, f64::NAN), 0);
    assert!(g.notices.contains(&FS_12));
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    assert_eq!(nint(&mut g, f64::NAN), i32::MIN);
    assert!(g.notices.contains(&FS_12));
}
