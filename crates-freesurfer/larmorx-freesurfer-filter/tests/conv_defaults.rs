// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The convolutions', derivatives' and classification's defaults against `reproduce` (FS-545,
//! FS-546, FS-547, FS-553, FS-554, FS-556, FS-557).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_filter::classify::mri_mark_border_voxels;
use larmorx_freesurfer_filter::conv::{MRI_WIDTH, mri_convolve1d, mri_convolve1d_byte, mri_reduce};
use larmorx_freesurfer_filter::sobel::{mri_divergence, mri_laplacian};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_SHORT, MRI_UCHAR, Mri, mri_alloc, mri_alloc_sequence,
};

const MODES: [UpstreamDefects; 2] = [UpstreamDefects::Fix, UpstreamDefects::Reproduce];

fn put(m: &mut Mri, i: usize, v: f32) {
    let size = m.bytes_per_vox;
    let c = m.chunk.as_mut().unwrap();
    match m.mtype {
        MRI_FLOAT => c[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes()),
        MRI_SHORT => c[2 * i..2 * i + 2].copy_from_slice(&(v as i16).to_le_bytes()),
        _ => c[i * size] = v as u8,
    }
}

fn floats(m: &Mri) -> Vec<f32> {
    m.chunk.as_ref().unwrap()[..m.bytes_total as usize]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
}

fn noted(g: &Globals, id: &str) -> bool {
    g.notices.as_slice().iter().any(|n| n.defect.id == id)
}

fn sink() -> (Vec<u8>, Vec<u8>) {
    (Vec::new(), Vec::new())
}

/// FS-545: a short volume's taps past a row's ends: by default the edge voxel (as for uchar and
/// float volumes); with `reproduce` the outside value (0) before it and the next row's first voxel
/// after it.
#[test]
fn fs545_short_edges() {
    let mut got = Vec::new();
    for mode in MODES {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = sink();
        let mut src = mri_alloc(&mut g, &mut o, &mut e, 4, 2, 1, MRI_SHORT).unwrap();
        for (i, v) in [1.0, 2.0, 3.0, 4.0, 10.0, 20.0, 30.0, 40.0]
            .into_iter()
            .enumerate()
        {
            put(&mut src, i, v);
        }
        let r = mri_convolve1d(
            &mut g, &mut o, &mut e, &src, None, &[1.0; 3], MRI_WIDTH, 0, 0,
        )
        .unwrap();
        let Ret::New(m) = r else { panic!() };
        assert!(noted(&g, "FS-545"));
        got.push(floats(&m)[..4].to_vec());
    }
    assert_eq!(got[0], [4.0, 6.0, 9.0, 11.0]);
    assert_eq!(got[1], [3.0, 6.0, 9.0, 17.0]);
}

/// FS-547: a float source convolved into a uchar volume: by default its values, with `reproduce`
/// the bytes of its first voxels.
#[test]
fn fs547_float_source_into_uchar() {
    let mut got = Vec::new();
    for mode in MODES {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = sink();
        let mut src = mri_alloc(&mut g, &mut o, &mut e, 3, 1, 1, MRI_FLOAT).unwrap();
        for (i, v) in [10.0, 20.0, 30.0].into_iter().enumerate() {
            put(&mut src, i, v);
        }
        let r = mri_convolve1d_byte(&mut g, &mut o, &mut e, &src, None, &[1.0], MRI_WIDTH, 0, 0)
            .unwrap();
        let Ret::New(m) = r else { panic!() };
        assert!(noted(&g, "FS-547"));
        got.push(m.chunk.unwrap()[..3].to_vec());
    }
    assert_eq!(got[0], [10, 20, 30]);
    // 10.0f is 0x41200000: its bytes 00 00 20 41
    assert_eq!(got[1], [0, 0, 32]);
}

/// FS-546: a destination frame the volume does not have: by default an error and `NULL`, with
/// `reproduce` noted (FreeSurfer writes outside the volume).
#[test]
fn fs546_destination_frame() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = sink();
        let src = mri_alloc(&mut g, &mut o, &mut e, 3, 3, 3, MRI_FLOAT).unwrap();
        let mut dst = mri_alloc(&mut g, &mut o, &mut e, 3, 3, 3, MRI_FLOAT).unwrap();
        let r = mri_convolve1d(
            &mut g,
            &mut o,
            &mut e,
            &src,
            Some(&mut dst),
            &[1.0; 3],
            MRI_WIDTH,
            0,
            1,
        )
        .unwrap();
        assert!(noted(&g, "FS-546"));
        assert_eq!(r.is_null(), mode == UpstreamDefects::Fix);
    }
}

/// FS-553: MRIreduce of one slice: by default the reduced slice, with `reproduce` depth 0.
#[test]
fn fs553_reduce_one_slice() {
    let mut depths = Vec::new();
    for mode in MODES {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = sink();
        let src = mri_alloc(&mut g, &mut o, &mut e, 4, 4, 1, MRI_UCHAR).unwrap();
        let r = mri_reduce(&mut g, &mut o, &mut e, &Environment::default(), &src, None).unwrap();
        let Ret::New(m) = r else { panic!() };
        assert!(noted(&g, "FS-553"));
        depths.push((m.vg.width, m.vg.height, m.vg.depth));
    }
    assert_eq!(depths, [(2, 2, 1), (2, 2, 0)]);
}

/// FS-554: the Laplacian's centre of frame 1: by default frame 1's, with `reproduce` frame 0's.
#[test]
fn fs554_laplacian_frames() {
    let mut got = Vec::new();
    for mode in MODES {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = sink();
        let mut src = mri_alloc_sequence(&mut g, &mut o, &mut e, 1, 1, 1, MRI_FLOAT, 2).unwrap();
        put(&mut src, 0, 1.0);
        put(&mut src, 1, 5.0);
        let Ret::New(m) = mri_laplacian(&mut g, &mut o, &mut e, &src, None).unwrap() else {
            panic!()
        };
        assert!(noted(&g, "FS-554"));
        got.push(floats(&m));
    }
    // one voxel: -6 c plus six neighbours that are the voxel itself
    assert_eq!(got[0], [0.0, 0.0]);
    assert_eq!(got[1], [0.0, 24.0]);
}

/// FS-556: MRIdivergence with a destination: by default it is filled and returned, with
/// `reproduce` a new volume.
#[test]
fn fs556_divergence_destination() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = sink();
        let src = mri_alloc(&mut g, &mut o, &mut e, 3, 3, 3, MRI_FLOAT).unwrap();
        let mut dst = mri_alloc(&mut g, &mut o, &mut e, 3, 3, 3, MRI_FLOAT).unwrap();
        let r = mri_divergence(&mut g, &mut o, &mut e, &src, Some(&mut dst)).unwrap();
        assert!(noted(&g, "FS-556"));
        assert_eq!(matches!(r, Ret::Given), mode == UpstreamDefects::Fix);
    }
}

/// FS-557: a NaN label: by default 0, with `reproduce` -2147483648 (neither matches a label).
#[test]
fn fs557_nan_label() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = sink();
        let mut src = mri_alloc(&mut g, &mut o, &mut e, 2, 1, 1, MRI_FLOAT).unwrap();
        put(&mut src, 0, 255.0);
        put(&mut src, 1, f32::NAN);
        let Ret::New(m) = mri_mark_border_voxels(&mut g, &mut o, &mut e, &src, None).unwrap()
        else {
            panic!()
        };
        assert!(noted(&g, "FS-557"));
        assert_eq!(floats(&m), [128.0, 128.0]);
    }
}
