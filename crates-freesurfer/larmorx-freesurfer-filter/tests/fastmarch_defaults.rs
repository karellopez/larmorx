// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fast-marching distance maps' defaults against `reproduce` (FS-539, FS-540, FS-542, FS-543,
//! FS-544).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_filter::fastmarch::{
    DTRANS_MODE_INSIDE, DTRANS_MODE_SIGNED, mri_distance_transform, mri_extract_distance_map,
};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc};

fn vol(g: &mut Globals, w: i32, h: i32, d: i32, ty: i32, on: &[(i32, i32, i32)]) -> Mri {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut m = mri_alloc(g, &mut out, &mut err, w, h, d, ty).unwrap();
    for &(x, y, z) in on {
        let i = ((z * h + y) * w + x) as usize;
        let c = m.chunk.as_mut().unwrap();
        if ty == MRI_FLOAT {
            c[4 * i..4 * i + 4].copy_from_slice(&1.0f32.to_le_bytes());
        } else {
            c[i] = 1;
        }
    }
    m
}

fn floats(m: &Mri) -> Vec<f32> {
    let n = (m.vg.width * m.vg.height * m.vg.depth) as usize;
    m.chunk.as_ref().unwrap()[..4 * n]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
}

fn map(
    mode: UpstreamDefects,
    w: i32,
    h: i32,
    d: i32,
    on: &[(i32, i32, i32)],
) -> (Vec<f32>, Globals) {
    let mut g = Globals::new(mode);
    let src = vol(&mut g, w, h, d, MRI_UCHAR, on);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r =
        mri_extract_distance_map(&mut g, &mut out, &mut err, &src, None, 1, 20.0, 3, None).unwrap();
    let Ret::New(m) = r else { panic!() };
    (floats(&m), g)
}

fn noted(g: &Globals, id: &str) -> bool {
    g.notices.as_slice().iter().any(|n| n.defect.id == id)
}

/// FS-539 and FS-540: a volume one voxel wide, high or deep is a two-dimensional map by default
/// (no neighbour beyond the edge), the same in each orientation; `reproduce` reads the next row as
/// the neighbour beyond a one-voxel-wide edge.
#[test]
fn one_voxel_axes_are_two_dimensional_by_default() {
    let lab = [(1, 2), (2, 2), (3, 1)];
    let (wide, g1) = map(UpstreamDefects::Fix, 1, 5, 4, &lab.map(|(a, b)| (0, a, b)));
    let (high, g2) = map(UpstreamDefects::Fix, 5, 1, 4, &lab.map(|(a, b)| (a, 0, b)));
    let (deep, g3) = map(UpstreamDefects::Fix, 5, 4, 1, &lab.map(|(a, b)| (a, b, 0)));
    assert!(noted(&g1, "FS-539") && noted(&g2, "FS-540") && noted(&g3, "FS-540"));
    // the same grid of 5 x 4 positions (a, b) in each orientation
    for a in 0..5usize {
        for b in 0..4usize {
            let v = wide[b * 5 + a];
            assert_eq!(v.to_bits(), high[b * 5 + a].to_bits(), "({a}, {b})");
            assert_eq!(v.to_bits(), deep[b * 5 + a].to_bits(), "({a}, {b})");
        }
    }
    let (rep, g4) = map(
        UpstreamDefects::Reproduce,
        1,
        5,
        4,
        &lab.map(|(a, b)| (0, a, b)),
    );
    assert!(noted(&g4, "FS-539"));
    assert_ne!(rep, wide);
}

/// FS-543: DTRANS_MODE_INSIDE gives the distances inside the label (0 outside) by default, the
/// unsigned map on both sides with `reproduce`.
#[test]
fn inside_mode_is_inside() {
    let on: Vec<(i32, i32, i32)> = (1..5)
        .flat_map(|x| (1..5).flat_map(move |y| (1..5).map(move |z| (x, y, z))))
        .collect();
    for (mode, outside_positive) in [
        (UpstreamDefects::Fix, false),
        (UpstreamDefects::Reproduce, true),
    ] {
        let mut g = Globals::new(mode);
        let src = vol(&mut g, 6, 6, 6, MRI_UCHAR, &on);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let r = mri_distance_transform(
            &mut g,
            &mut out,
            &mut err,
            &Environment::default(),
            &src,
            None,
            1,
            10.0,
            DTRANS_MODE_INSIDE,
            None,
        )
        .unwrap();
        let Ret::New(m) = r else { panic!() };
        let v = floats(&m);
        assert!(noted(&g, "FS-543"));
        // a corner voxel lies outside the label
        assert_eq!(v[0] > 0.0, outside_positive, "{mode}");
        // the label's centre is inside: negative by default, positive (unsigned) with reproduce
        let c = v[(3 * 6 + 3) * 6 + 3];
        assert_eq!(c < 0.0, !outside_positive, "{mode}");
    }
}

/// FS-544: an unknown mode is an error by default; FreeSurfer's map of zeros with `reproduce`.
#[test]
fn unknown_mode() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let src = vol(&mut g, 4, 4, 4, MRI_UCHAR, &[(1, 1, 1)]);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let r = mri_extract_distance_map(&mut g, &mut out, &mut err, &src, None, 1, 5.0, 7, None);
        assert!(noted(&g, "FS-544"));
        match mode {
            UpstreamDefects::Fix => assert!(r.is_err()),
            _ => {
                let Ret::New(m) = r.unwrap() else { panic!() };
                assert!(floats(&m).iter().all(|&v| v.to_bits() == 0));
            }
        }
        let r = mri_distance_transform(
            &mut g,
            &mut out,
            &mut err,
            &Environment::default(),
            &src,
            None,
            1,
            5.0,
            0,
            None,
        );
        assert_eq!(r.is_err(), mode == UpstreamDefects::Fix);
        let _ = DTRANS_MODE_SIGNED;
    }
}

/// FS-542: a NaN voxel of a float label volume is -2147483648 with `reproduce` (it never matches
/// label 0), 0 by default (it matches).
#[test]
fn nan_labels() {
    for (mode, matches) in [
        (UpstreamDefects::Fix, true),
        (UpstreamDefects::Reproduce, false),
    ] {
        let mut g = Globals::new(mode);
        let mut src = vol(&mut g, 5, 5, 5, MRI_FLOAT, &[]);
        let c = src.chunk.as_mut().unwrap();
        // every voxel 1 but the centre, NaN
        for v in c.as_chunks_mut::<4>().0 {
            *v = 1.0f32.to_le_bytes();
        }
        let i = (2 * 5 + 2) * 5 + 2;
        c[4 * i..4 * i + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let r = mri_extract_distance_map(&mut g, &mut out, &mut err, &src, None, 0, 5.0, 1, None)
            .unwrap();
        let Ret::New(m) = r else { panic!() };
        let v = floats(&m);
        assert!(noted(&g, "FS-542"));
        // with a label voxel the centre's neighbours are on its boundary (0.5)
        assert_eq!(v[i + 1] == 0.5, matches, "{mode}");
    }
}
