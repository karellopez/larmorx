// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FS-1233: `MRIfindBrightNonWM` with a T1 or white-matter volume that is not of bytes. FreeSurfer
//! reads both, and writes and dilates the labels, as the bytes of the volumes' rows (`reproduce`);
//! by default the labels are those of the byte volumes holding the same values, written in the
//! T1's type. Inputs made with integer arithmetic.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri, mri_alloc};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::mri2ops::mri_find_bright_non_wm;

const N: i32 = 12;

/// A volume of `mtype` holding `f(x, y, z)`.
fn volume(g: &mut Globals, mtype: i32, f: impl Fn(i32, i32, i32) -> i32) -> Mri {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = mri_alloc(g, &mut o, &mut e, N, N, N, mtype).expect("a volume");
    for z in 0..N {
        for y in 0..N {
            for x in 0..N {
                let v = f(x, y, z) as f32;
                mri_set_vox_val(g, &mut e, &mut m, x, y, z, 0, v, 0).expect("set");
            }
        }
    }
    m
}

/// The T1: background 0, a dim head (60) with a white core (110), a bright blob (200) and a
/// brighter rim (120, above the dilation's 100) outside the white matter.
fn t1_value(x: i32, y: i32, z: i32) -> i32 {
    if (7..10).contains(&x) && (2..5).contains(&y) && (3..7).contains(&z) {
        200
    } else if (6..11).contains(&x) && (1..6).contains(&y) && (2..8).contains(&z) {
        120
    } else if (1..11).contains(&x) && (1..11).contains(&y) && (1..11).contains(&z) {
        if (3..6).contains(&x) && (5..9).contains(&y) && (3..9).contains(&z) {
            110
        } else {
            60
        }
    } else {
        0
    }
}

/// The white matter: 110 in the T1's white core and in a band of the first three columns beside
/// the blob's rows (far from the blob: read as the bytes of a float row, its voxels land on the
/// blob's columns).
fn wm_value(x: i32, y: i32, z: i32) -> i32 {
    let core = (3..6).contains(&x) && (5..9).contains(&y) && (3..9).contains(&z);
    let band = (0..3).contains(&x) && (1..6).contains(&y) && (2..8).contains(&z);
    if core || band { 110 } else { 0 }
}

/// The labels' values (frame 0) and the notices' IDs of one call.
fn labels(mode: UpstreamDefects, t1_type: i32, wm_type: i32) -> (i32, Vec<f32>, Vec<String>) {
    let mut g = Globals::new(mode);
    let t1 = volume(&mut g, t1_type, t1_value);
    let wm = volume(&mut g, wm_type, wm_value);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let lab = mri_find_bright_non_wm(&mut g, &mut o, &mut e, &t1, &wm)
        .expect("no fatal")
        .expect("a volume");
    let mut v = Vec::new();
    for z in 0..N {
        for y in 0..N {
            for x in 0..N {
                v.push(mri_get_vox_val(&mut g, &mut e, &lab, x, y, z, 0, 0).expect("get"));
            }
        }
    }
    let ids = g
        .notices
        .as_slice()
        .iter()
        .map(|n| n.defect.id.to_owned())
        .collect();
    (lab.mtype, v, ids)
}

#[test]
fn bytes_give_bright_labels_without_a_notice() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (t, v, ids) = labels(mode, MRI_UCHAR, MRI_UCHAR);
        assert_eq!(t, MRI_UCHAR);
        assert!(!ids.iter().any(|i| i == "FS-1233"));
        assert_eq!(v.iter().filter(|&&x| x == 130.0).count(), 36);
        assert!(v.iter().any(|&x| x == 100.0));
    }
}

#[test]
fn other_types_by_default_give_the_bytes_labels_in_the_t1s_type() {
    let (_, want, _) = labels(UpstreamDefects::Fix, MRI_UCHAR, MRI_UCHAR);
    for (t1, wm) in [
        (MRI_FLOAT, MRI_UCHAR),
        (MRI_SHORT, MRI_UCHAR),
        (MRI_INT, MRI_UCHAR),
        (MRI_UCHAR, MRI_FLOAT),
        (MRI_FLOAT, MRI_SHORT),
    ] {
        let (t, v, ids) = labels(UpstreamDefects::Fix, t1, wm);
        assert_eq!(t, t1);
        assert!(v == want, "T1 {t1}, wm {wm}: not the byte volumes' labels");
        assert!(ids.iter().any(|i| i == "FS-1233"));
        let (t, v, ids) = labels(UpstreamDefects::Reproduce, t1, wm);
        assert_eq!(t, t1);
        assert!(
            v != want,
            "T1 {t1}, wm {wm}: FreeSurfer's bytes give the same labels"
        );
        assert!(ids.iter().any(|i| i == "FS-1233"));
    }
}

/// FreeSurfer's float T1: no voxel reads 130 (its labels are bytes inside the floats).
#[test]
fn reproduce_of_a_float_t1_shows_no_bright_voxel() {
    let (_, v, _) = labels(UpstreamDefects::Reproduce, MRI_FLOAT, MRI_UCHAR);
    assert!(!v.contains(&130.0));
}
