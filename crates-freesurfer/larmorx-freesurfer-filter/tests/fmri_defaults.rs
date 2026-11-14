// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The frame functions' defaults against `reproduce` (FS-563, FS-564, FS-565, FS-566, FS-567,
//! FS-569).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_filter::fmri;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc_sequence};

const MODES: [UpstreamDefects; 2] = [UpstreamDefects::Fix, UpstreamDefects::Reproduce];

fn noted(g: &Globals, id: &str) -> bool {
    g.notices.as_slice().iter().any(|n| n.defect.id == id)
}

fn floats(m: &Mri) -> Vec<f32> {
    m.chunk.as_ref().unwrap()[..m.bytes_total as usize]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
}

/// A float series of `w` voxels and `nf` frames, voxel `i` of frame `f` `v(i, f)`.
fn series(g: &mut Globals, w: i32, nf: i32, v: impl Fn(i32, i32) -> f32) -> Mri {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = mri_alloc_sequence(g, &mut o, &mut e, w, 1, 1, MRI_FLOAT, nf).unwrap();
    let c = m.chunk.as_mut().unwrap();
    for f in 0..nf {
        for i in 0..w {
            let k = 4 * (f * w + i) as usize;
            c[k..k + 4].copy_from_slice(&v(i, f).to_le_bytes());
        }
    }
    m
}

/// FS-563: a constant voxel's AR(1) is 0 by default; with `reproduce` 0/0 (SSE's default NaN).
#[test]
fn fs563_zero_variance() {
    let mut got = Vec::new();
    for mode in MODES {
        let mut g = Globals::new(mode);
        let src = series(&mut g, 2, 4, |i, f| if i == 0 { 3.0 } else { f as f32 });
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let Ret::New(a) =
            fmri::fmri_temporal_ar1(&mut g, &mut o, &mut e, &src, -1.0, None, None).unwrap()
        else {
            panic!()
        };
        assert!(noted(&g, "FS-563"));
        got.push(floats(&a));
    }
    assert_eq!(got[0][0], 0.0);
    assert_eq!(got[1][0].to_bits(), 0xffc0_0000);
    assert_eq!(got[0][1].to_bits(), got[1][1].to_bits());
}

/// FS-564: the AR(1) of a single frame cannot be computed: an error in both modes.
#[test]
fn fs564_single_frame() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let src = series(&mut g, 2, 1, |_, _| 1.0);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = fmri::fmri_temporal_ar1(&mut g, &mut o, &mut e, &src, 0.0, None, None);
        assert!(r.is_err());
        assert!(noted(&g, "FS-564"));
        assert!(String::from_utf8_lossy(&o).contains("Lag out of bound 1"));
    }
}

/// FS-565: a NaN degrees-of-freedom adjustment: by default no degrees of freedom (an error);
/// with `reproduce` x86-64's -2147483648, a lag of 1 then dividing by 2147483647.
#[test]
fn fs565_nan_adjustment() {
    let mut out = Vec::new();
    for mode in MODES {
        let mut g = Globals::new(mode);
        let src = series(&mut g, 1, 3, |_, f| f as f32 + 1.0);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r =
            fmri::fmri_covariance(&mut g, &mut o, &mut e, &src, 1, f32::NAN, None, None).unwrap();
        assert!(noted(&g, "FS-565"));
        out.push((r, String::from_utf8_lossy(&o).into_owned()));
    }
    assert!(out[0].0.is_null());
    assert!(out[0].1.contains("DOFLag = -1 <= 0 (DOF=0, Lag=1)"));
    let Ret::New(m) = &out[1].0 else { panic!() };
    assert_eq!(floats(m)[0], (8.0f64 / 2147483647.0) as f32);
}

/// FS-566: one time point: the element past the matrix is dropped by default; the matrix itself
/// is the same in both modes.
#[test]
fn fs566_one_time_point() {
    let mut got = Vec::new();
    for mode in MODES {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let m = fmri::asl_interp_matrix(&mut g, &mut o, &mut e, 1)
            .unwrap()
            .unwrap();
        assert!(noted(&g, "FS-566"));
        got.push((m.rows, m.cols, m.relt(1, 1)));
    }
    assert_eq!(got[0], (1, 1, 1.0));
    assert_eq!(got[0], got[1]);
}

/// FS-567: a matrix with fewer columns than voxels: by default an error and `Null`; with
/// `reproduce` the later voxels land in the next rows' elements.
#[test]
fn fs567_small_matrix() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let src = series(&mut g, 3, 2, |i, f| (10 * f + i) as f32);
        let mut m = larmorx_freesurfer_core::matrix::Matrix::real(2, 2);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = fmri::fmri_to_matrix(&mut g, &mut o, &mut e, &src, Some(&mut m)).unwrap();
        assert!(noted(&g, "FS-567"));
        if mode == UpstreamDefects::Fix {
            assert!(r.is_null());
            assert_eq!(m.data(), &[0.0; 4]);
        } else {
            assert_eq!(r, Ret::Given);
            // voxel 2's frame 0 is element (1, 3), the first of row 2: frame 1 of voxel 0 is
            // written there first and then overwritten
            assert_eq!(m.data(), &[0.0, 1.0, 2.0, 11.0]);
        }
    }
}

/// FS-569: a negative step from a start below the stop never ends in FreeSurfer: an error.
#[test]
fn fs569_negative_step() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let src = series(&mut g, 1, 4, |_, f| f as f32);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = fmri::fmri_sub_sample(&mut g, &mut o, &mut e, &src, 0, -2, 1, None);
        assert!(r.is_err());
        assert!(noted(&g, "FS-569"));
    }
}
