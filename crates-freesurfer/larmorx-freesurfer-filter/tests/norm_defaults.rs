// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The Voronoi diagram and the soap bubble (`mrinorm.cpp`): a single control point spreads its
//! value; FS-583 and FS-585.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_filter::norm;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri, mri_alloc};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::Pair;

const MODES: [UpstreamDefects; 2] = [UpstreamDefects::Fix, UpstreamDefects::Reproduce];

fn noted(g: &Globals, id: &str) -> bool {
    g.notices.as_slice().iter().any(|n| n.defect.id == id)
}

fn vol(g: &mut Globals, ty: i32, f: impl Fn(i32, i32, i32) -> f32) -> Mri {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = mri_alloc(g, &mut o, &mut e, 5, 5, 5, ty).unwrap();
    for z in 0..5 {
        for y in 0..5 {
            for x in 0..5 {
                mri_set_vox_val(g, &mut e, &mut m, x, y, z, 0, f(x, y, z), 0).unwrap();
            }
        }
    }
    m
}

fn values(g: &mut Globals, m: &Mri) -> Vec<f32> {
    let mut e = Vec::new();
    let mut v = Vec::new();
    for z in 0..5 {
        for y in 0..5 {
            for x in 0..5 {
                v.push(mri_get_vox_val(g, &mut e, m, x, y, z, 0, 0).unwrap());
            }
        }
    }
    v
}

/// One control point at the centre: every voxel takes its value; the control volume's marks are
/// left as they were.
#[test]
fn one_control_point_spreads() {
    for mode in MODES {
        for ty in [MRI_FLOAT, MRI_UCHAR] {
            let mut g = Globals::new(mode);
            let src = vol(&mut g, ty, |x, y, z| (x + 10 * y + 3 * z) as f32);
            let centre = |x: i32, y: i32, z: i32| (x, y, z) == (2, 2, 2);
            let mut ctrl = vol(&mut g, MRI_UCHAR, |x, y, z| {
                f32::from(u8::from(centre(x, y, z)))
            });
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let r = norm::mri_build_voronoi_diagram(&mut g, &mut o, &mut e, &src, &mut ctrl, None)
                .unwrap();
            let Ret::New(d) = r else {
                panic!("a new volume")
            };
            assert!(values(&mut g, &d).iter().all(|&v| v == 28.0));
            let c = values(&mut g, &ctrl);
            assert_eq!(c.iter().filter(|&&v| v == 1.0).count(), 1);
            assert_eq!(c.iter().filter(|&&v| v != 0.0 && v != 1.0).count(), 0);
        }
    }
}

/// FS-583: a 16-bit source without a destination: a clone by default, an error with `reproduce`.
#[test]
fn fs583_short_without_destination() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let src = vol(&mut g, MRI_SHORT, |x, _, _| x as f32);
        let mut ctrl = vol(&mut g, MRI_UCHAR, |x, _, _| f32::from(u8::from(x == 0)));
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = norm::mri_build_voronoi_diagram(&mut g, &mut o, &mut e, &src, &mut ctrl, None);
        assert!(noted(&g, "FS-583"));
        assert_eq!(r.is_ok(), mode == UpstreamDefects::Fix);
    }
}

/// FS-585: the soap bubble of an `int` volume averages its bytes with `reproduce`, its values by
/// default (a constant volume stays constant).
#[test]
fn fs585_int_soap_bubble() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let mut src = vol(&mut g, MRI_INT, |_, _, _| 1000.0);
        let ctrl = vol(&mut g, MRI_UCHAR, |_, _, _| 0.0);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        norm::mri_soap_bubble(&mut g, &mut o, &mut e, Pair::Same(&mut src), &ctrl, 2, 0.0).unwrap();
        assert!(noted(&g, "FS-585"));
        let constant = values(&mut g, &src).iter().all(|&v| v == 1000.0);
        assert_eq!(constant, mode == UpstreamDefects::Fix);
    }
}
