// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The intensity normalisation (`mrinorm.cpp`): what the default corrects and `reproduce`
//! keeps: FS-590 (the peak check's starting peaks past the peaks found), FS-592 (the gentle
//! search's byte thresholds), FS-594 (the threshold's removal of consecutive control points),
//! FS-595 (the rejected points' message), and the uniform field's normalisation.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_filter::normalize as nz;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

fn noted(g: &Globals, id: &str) -> bool {
    g.notices.as_slice().iter().any(|n| n.defect.id == id)
}

fn vol(g: &mut Globals, n: i32, ty: i32, f: impl Fn(i32, i32, i32) -> f32) -> Mri {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = mri_alloc(g, &mut o, &mut e, n, n, n, ty).unwrap();
    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                mri_set_vox_val(g, &mut e, &mut m, x, y, z, 0, f(x, y, z), 0).unwrap();
            }
        }
    }
    m
}

/// FS-590: 23 windows (14 above the origin) but only 9 peaks: the starting peaks 7, 10 and 13
/// lie past them. The default takes them among the peaks and keeps the smooth peaks;
/// `reproduce` reads the arrays' other entries as FreeSurfer does (here 0, which drops peaks).
#[test]
fn fs590_starting_peaks_among_those_found() {
    let mni = nz::Mni {
        windows_above_t0: 14,
        windows_below_t0: 9,
        desired_wm_value: 110,
        ..nz::Mni::default()
    };
    let run = |mode| {
        let mut g = Globals::new(mode);
        let mut inp = [0.0f32; 80];
        let mut outp = [0.0f32; 80];
        for i in 0..9 {
            inp[i] = i as f32 * 5.0;
            outp[i] = 100.0 + i as f32 * 0.5;
        }
        let mut p = nz::Peaks {
            inputs: &mut inp,
            outputs: &mut outp,
        };
        let n = nz::mri_norm_check_peaks(&mut g, &mut Vec::new(), &mni, &mut p, 9).unwrap();
        (n, noted(&g, "FS-590"))
    };
    assert_eq!(run(UpstreamDefects::Fix), (9, true));
    let (n, seen) = run(UpstreamDefects::Reproduce);
    assert!(seen && n < 9, "{n}");
}

/// FS-592: with 150 above, the high thresholds 110 + 1.5 x 150 = 335 and 110 + 150 = 260 are 79
/// and 4 as bytes: FreeSurfer finds no control point in a uniform 110 volume; the default finds
/// them all.
#[test]
fn fs592_gentle_thresholds_do_not_wrap() {
    let count = |mode| {
        let mut g = Globals::new(mode);
        let src = vol(&mut g, 9, MRI_UCHAR, |_, _, _| 110.0);
        let mut st = nz::NormState::default();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let c = nz::mri_norm_gently_find_control_points(
            &mut g, &mut o, &mut e, &mut st, &src, 110.0, 150.0, 10.0, None, None,
        )
        .unwrap()
        .unwrap_new();
        let mut n = 0;
        for z in 0..9 {
            for y in 0..9 {
                for x in 0..9 {
                    n += i32::from(
                        mri_get_vox_val(&mut g, &mut e, &c, x, y, z, 0, 0).unwrap() > 0.0,
                    );
                }
            }
        }
        (n, noted(&g, "FS-592"))
    };
    assert_eq!(count(UpstreamDefects::Fix), (729, true));
    assert_eq!(count(UpstreamDefects::Reproduce), (0, true));
}

/// FS-594: three consecutive control points below `FS_THRESH_CTRL`: FreeSurfer removes the first
/// and the third, the default all three.
#[test]
fn fs594_consecutive_points_removed() {
    let dir = tempfile::tempdir().unwrap();
    // tkregister RAS of a 6^3 volume of 1 mm: the voxel (c, r, s) is (3 - c, s - 3, 3 - r)
    let pts = "3 -3 3\n2 -3 3\n1 -3 3\n0 -1 0\n";
    std::fs::write(dir.path().join("cp.dat"), pts).unwrap();
    let kept = |mode| {
        let mut g = Globals::new(mode);
        // the first three points' voxels are 0, the fourth's 50
        let mut m = vol(
            &mut g,
            6,
            MRI_UCHAR,
            |x, y, z| if z == 2 { 50.0 } else { 0.0 },
        );
        let env = Environment::from_pairs([("FS_THRESH_CTRL", "10")]);
        let mut st = nz::NormState::default();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        nz::mri_3d_use_file_control_points(
            &mut g,
            &mut o,
            &mut e,
            &env,
            Some(dir.path()),
            &mut st,
            &mut m,
            "cp.dat",
        )
        .unwrap();
        (st.ctrl.len(), noted(&g, "FS-594"))
    };
    assert_eq!(kept(UpstreamDefects::Fix), (1, true));
    assert_eq!(kept(UpstreamDefects::Reproduce), (2, true));
}

/// FS-595: two control points outside a 4^3 volume: the default's message gives 2.
#[test]
fn fs595_the_rejected_count() {
    let mut g = Globals::new(UpstreamDefects::Fix);
    let mut c = vol(&mut g, 4, MRI_UCHAR, |_, _, _| 0.0);
    let mut st = nz::NormState {
        ctrl: vec![[9, 0, 0], [1, 1, 1], [0, -1, 0]],
        ..nz::NormState::default()
    };
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let n =
        nz::mri_norm_add_file_control_points(&mut g, &mut o, &mut e, &mut st, &mut c, 255, None)
            .unwrap();
    assert_eq!(n, 1);
    let text = String::from_utf8_lossy(&e).into_owned();
    assert!(text.contains("!!!!! 2 control points rejected"), "{text}");
    assert!(noted(&g, "FS-595") && st.size_reported);
}

/// A uniform white matter volume normalises to the target with a flat bias field, in both modes.
#[test]
fn gentle_normalisation_of_a_uniform_volume() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let mut src = vol(&mut g, 9, MRI_FLOAT, |_, _, _| 100.0);
        let mut st = nz::NormState::default();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = nz::mri_3d_gentle_normalize(
            &mut g,
            &mut o,
            &mut e,
            &Environment::default(),
            None,
            &mut st,
            &mut src,
            None,
            110.0,
            nz::Dst::New,
            25.0,
            15.0,
            0,
            2.0,
            None,
        )
        .unwrap()
        .unwrap_new();
        let v = mri_get_vox_val(&mut g, &mut e, &r, 4, 4, 4, 0, 0).unwrap();
        assert_eq!(v, 110.0, "{mode:?}");
    }
}
