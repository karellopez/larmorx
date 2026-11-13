// SPDX-License-Identifier: Apache-2.0
//! fs.mri's defects (`specs/freesurfer-mri.md` §24) in both modes, against the spec's text: the
//! cases the probe cannot show (§23.1: FreeSurfer crashes or reads memory there, or larmorx
//! defines both modes: FS-53, FS-58, FS-165, FS-197, FS-198, FS-264) and one case of each other
//! defect with its notice.

use larmorx_core::defects::UpstreamDefects::{self, Fix, Reproduce};
use larmorx_fs_core::maths::Returned;
use larmorx_fs_core::maths::matrix::{MATRIX_REAL, Matrix};
use larmorx_fs_core::mri::types;
use larmorx_fs_core::mri::xform::{MincTransform, TransformKind};
use larmorx_fs_core::mri::{ColorTable, ColorTableEntry, Region, VolGeom};
use larmorx_fs_core::{Environment, Process, Stop};

fn proc(mode: UpstreamDefects) -> Process {
    Process::silent(Environment::new(), mode)
}

fn noted(p: &Process, id: &str) -> bool {
    p.notices().as_slice().iter().any(|n| n.defect.id == id)
}

fn m4(v: [f32; 16]) -> Matrix {
    let mut m = Matrix::zeros(4, 4, MATRIX_REAL);
    for (k, x) in v.iter().enumerate() {
        m.set(k as i32 / 4 + 1, k as i32 % 4 + 1, *x);
    }
    m
}

fn geom(w: i32, h: i32, d: i32, size: f32) -> VolGeom {
    VolGeom {
        width: w,
        height: h,
        depth: d,
        xsize: size,
        ysize: size,
        zsize: size,
        c: [10.0, -20.0, 30.0],
        ..VolGeom::default()
    }
}

#[test]
fn fs58_volume_without_voxels_both_modes() {
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let mut h = p.mri_alloc_header(2, 2, 2, types::FLOAT, 1).unwrap();
        assert_eq!(p.mri_get_vox_val(&h, 0, 0, 0, 0, 0), Err(Stop::Exit(252)));
        assert_eq!(
            p.mri_set_vox_val(&mut h, 0, 0, 0, 0, 1.0, 0),
            Err(Stop::Exit(252))
        );
        assert_eq!(p.mri_get_vox_d(&h, 0, 0, 0, 0, 0), Err(Stop::Exit(252)));
        let mut b = Region::default();
        assert_eq!(p.mri_bounding_box(&h, 70, &mut b), Err(Stop::Exit(252)));
        // no rows: nothing read, the empty box
        let flat = p.mri_alloc_header(3, 0, 2, types::UCHAR, 1).unwrap();
        assert_eq!(p.mri_bounding_box(&flat, 70, &mut b), Ok(0));
        assert_eq!((b.x, b.dx), (2, -1));
        let src = p.mri_alloc_sequence(2, 2, 2, types::FLOAT, 1).unwrap();
        assert_eq!(p.mri_copy(&src, Some(&mut h)), Err(Stop::Exit(252)));
        assert!(noted(&p, "FS-58"));
    }
}

#[test]
fn fs52_fs53_outside_the_volume() {
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let mut m = p.mri_alloc_sequence(2, 2, 1, types::UCHAR, 1).unwrap();
        m.voxels = Some(vec![1, 2, 3, 4]);
        m.outside_val = 9.0;
        // a column past the end of row 0 is row 1's first voxel (FS-52)
        let v = p.mri_get_vox_val(&m, 2, 0, 0, 0, 0).unwrap();
        assert_eq!(v, if mode.reproduce() { 3.0 } else { 9.0 });
        assert!(noted(&p, "FS-52"));
        p.mri_set_vox_val(&mut m, 2, 0, 0, 0, 7.0, 0).unwrap();
        let want: &[u8] = if mode.reproduce() {
            &[1, 2, 7, 4]
        } else {
            &[1, 2, 3, 4]
        };
        assert_eq!(m.voxels.as_deref(), Some(want));
        // beyond the buffer (FS-53): outside_val, or 0 with reproduce; nothing written
        let v = p.mri_get_vox_val(&m, 1, 5, 0, 0, 0).unwrap();
        assert_eq!(v, if mode.reproduce() { 0.0 } else { 9.0 });
        let before = m.voxels.clone();
        p.mri_set_vox_val(&mut m, 0, 7, 0, 0, 5.0, 0).unwrap();
        p.mri_set_vox_val(&mut m, 0, 0, 0, 3, 5.0, 0).unwrap();
        assert_eq!(m.voxels, before);
        assert!(noted(&p, "FS-53"));
    }
}

#[test]
fn fs54_long_voxels() {
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let mut m = p.mri_alloc_sequence(2, 1, 1, types::LONG, 1).unwrap();
        p.mri_set_vox_val(&mut m, 1, 0, 0, 0, -5.0, 0).unwrap();
        let v = m.voxels.clone().unwrap();
        let v0 = i64::from_le_bytes(v[0..8].try_into().unwrap());
        let v1 = i64::from_le_bytes(v[8..16].try_into().unwrap());
        assert_eq!(v1, -5);
        // FreeSurfer also stores the int into the high half of voxel 0
        assert_eq!(
            v0,
            if mode.reproduce() {
                -5i64 << 32 & !0xffff_ffff
            } else {
                0
            }
        );
        assert!(noted(&p, "FS-54"));
        // beyond the int range: the long range by default, -2^31 with reproduce
        p.mri_set_vox_val(&mut m, 0, 0, 0, 0, 1e10, 0).unwrap();
        let v = m.voxels.clone().unwrap();
        let v0 = i64::from_le_bytes(v[0..8].try_into().unwrap());
        assert_eq!(
            v0,
            if mode.reproduce() {
                -2147483648
            } else {
                10_000_000_000
            }
        );
    }
}

#[test]
fn fs55_types_without_a_size() {
    let mut p = proc(Fix);
    assert_eq!(
        p.mri_alloc_header(2, 2, 2, types::BITMAP, 1).map(|_| ()),
        Err(Stop::Exit(1))
    );
    assert!(noted(&p, "FS-55"));
    let mut p = proc(Reproduce);
    let m = p
        .mri_alloc_header(2, 2, 2, types::DOUBLE_COMPLEX, 1)
        .unwrap();
    assert_eq!(m.bytes_per_vox, u64::MAX);
    assert_eq!(m.bytes_total, 8u64.wrapping_mul(u64::MAX));
    assert_eq!(
        p.mri_alloc_sequence(1, 1, 1, types::BITMAP, 1).map(|_| ()),
        Err(Stop::Exit(1))
    );
    assert!(noted(&p, "FS-55"));
}

#[test]
fn fs56_the_registration_matrix() {
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let mut src = p.mri_alloc_header(1, 1, 1, types::UCHAR, 1).unwrap();
        src.geom.register_mat = Some(m4([2.0; 16]));
        let mut dst = p.mri_alloc_header(1, 1, 1, types::UCHAR, 1).unwrap();
        p.mri_copy_header(&src, Some(&mut dst)).unwrap();
        assert_eq!(dst.geom.register_mat.is_some(), !mode.reproduce());
        assert!(noted(&p, "FS-56"));
    }
}

#[test]
fn fs57_colour_table_copy() {
    let ct = ColorTable {
        entries: vec![
            Some(ColorTableEntry {
                name: b"a".to_vec(),
                count: 12,
                ..ColorTableEntry::default()
            }),
            None,
        ],
        idbase: 3,
        ..ColorTable::default()
    };
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let c = p.ctab_deep_copy(Some(&ct)).unwrap();
        let count = c.entries[0].as_ref().unwrap().count;
        if mode.reproduce() {
            assert_eq!((c.idbase, count), (0, 0));
        } else {
            assert_eq!((c.idbase, count), (3, 12));
        }
        assert!(noted(&p, "FS-57"));
    }
}

#[test]
fn fs59_assignment_keeps_stale_matrices() {
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let mut dst = geom(4, 4, 4, 1.0);
        p.reinit_cache(&mut dst).unwrap();
        let src = geom(2, 2, 2, 2.0);
        p.vg_assign(&mut dst, &src);
        assert_eq!(dst.i_to_r.is_some(), mode.reproduce());
        assert_eq!(dst.width, 2);
        assert!(noted(&p, "FS-59"));
    }
}

#[test]
fn fs60_freed_transform_dropped_both_modes() {
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let src = p.mri_alloc_header(2, 2, 2, types::UCHAR, 1).unwrap();
        let mut dst = p.mri_alloc_header(2, 2, 2, types::UCHAR, 1).unwrap();
        dst.transform = Some(MincTransform {
            kind: TransformKind::Linear,
            inverse_flag: false,
            linear: [1.0; 16],
            inverse: [1.0; 16],
        });
        dst.free_transform = 1;
        p.mri_copy_header(&src, Some(&mut dst)).unwrap();
        assert!(dst.transform.is_none());
        assert_eq!(dst.free_transform, 0);
        assert!(noted(&p, "FS-60"));
    }
}

#[test]
fn fs161_fs166_tensor_rows() {
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let mut src = p.mri_alloc_sequence(2, 2, 1, types::TENSOR, 1).unwrap();
        src.voxels = Some((1..=16).collect());
        let mut dst = p.mri_alloc_sequence(2, 2, 1, types::TENSOR, 1).unwrap();
        p.mri_copy(&src, Some(&mut dst)).unwrap();
        let want: Vec<u8> = if mode.reproduce() {
            let mut v = vec![0u8; 16];
            v[0] = 1;
            v[1] = 2;
            v
        } else {
            (1..=16).collect()
        };
        assert_eq!(dst.voxels, Some(want));
        assert!(noted(&p, "FS-161") && noted(&p, "FS-166"));
    }
}

#[test]
fn fs162_narrowing_conversions() {
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let mut src = p.mri_alloc_sequence(2, 1, 1, types::FLOAT, 1).unwrap();
        src.voxels = Some(
            [40000.0f32, -5.0]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
        );
        let mut dst = p.mri_alloc_sequence(2, 1, 1, types::SHORT, 1).unwrap();
        p.mri_copy(&src, Some(&mut dst)).unwrap();
        let v = dst.voxels.clone().unwrap();
        let s0 = i16::from_le_bytes([v[0], v[1]]);
        assert_eq!(s0, if mode.reproduce() { -25536 } else { 32767 });
        let mut uc = p.mri_alloc_sequence(2, 1, 1, types::UCHAR, 1).unwrap();
        p.mri_copy(&src, Some(&mut uc)).unwrap();
        let u = uc.voxels.unwrap();
        assert_eq!(u[1], if mode.reproduce() { 251 } else { 0 });
        assert!(noted(&p, "FS-162"));
    }
}

#[test]
fn fs163_destination_of_another_extent() {
    let mut p = proc(Fix);
    let src = p.mri_alloc_sequence(2, 2, 2, types::UCHAR, 1).unwrap();
    let mut dst = p.mri_alloc_sequence(3, 3, 3, types::UCHAR, 1).unwrap();
    assert_eq!(
        p.mri_copy(&src, Some(&mut dst)).map(|_| ()),
        Err(Stop::Exit(252))
    );
    assert!(noted(&p, "FS-163"));
    // reproduce: FreeSurfer's placement, the error where a row would not land inside
    let mut p = proc(Reproduce);
    assert!(p.mri_copy(&src, Some(&mut dst)).is_ok());
    let mut small = p.mri_alloc_sequence(2, 1, 1, types::UCHAR, 1).unwrap();
    assert_eq!(
        p.mri_copy(&src, Some(&mut small)).map(|_| ()),
        Err(Stop::Exit(252))
    );
    assert!(noted(&p, "FS-163"));
}

#[test]
fn fs164_registration_that_cannot_be_inverted() {
    let g = geom(4, 4, 4, 1.0);
    let zero = Matrix::zeros(4, 4, MATRIX_REAL);
    let mut p = proc(Fix);
    assert_eq!(p.tkreg_to_native(&g, &g, Some(&zero)), Err(Stop::Exit(252)));
    assert!(noted(&p, "FS-164"));
    let mut p = proc(Reproduce);
    let r = p.tkreg_to_native(&g, &g, Some(&zero)).unwrap();
    assert!(r.is_some());
    assert!(noted(&p, "FS-164"));
}

#[test]
fn fs165_old_tkregister_fix_both_modes() {
    let g = geom(4, 4, 4, 1.0);
    let mut r = Matrix::zeros(3, 4, MATRIX_REAL);
    r.set(1, 1, 1.0);
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        assert_eq!(p.fix_tkreg(&g, Some(&r)), Err(Stop::Exit(252)));
        assert!(noted(&p, "FS-165"));
    }
}

fn lt_with(p: &mut Process, kind: i32, label: i32) -> larmorx_fs_core::mri::LinearTransform {
    let mut lta = p.lta_alloc(1, None).unwrap();
    let mut lt = lta.xforms.remove(0);
    lt.kind = kind;
    lt.label = label;
    lt
}

#[test]
fn fs195_fs196_fs198_transform_copies() {
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let mut src = lt_with(&mut p, 1, 7);
        src.m_dl = Some(m4([3.0; 16]));
        src.m_last_dl = None;
        let mut dst = lt_with(&mut p, -1, 0);
        dst.m_dl = None;
        p.lt_copy(&src, Some(&mut dst)).unwrap();
        if mode.reproduce() {
            assert_eq!((dst.kind, dst.label), (-1, 0));
            assert!(dst.m_dl.is_none());
        } else {
            assert_eq!((dst.kind, dst.label), (1, 7));
            assert_eq!(dst.m_dl, src.m_dl);
        }
        // FS-198, both modes: the destination left without the matrix
        assert!(dst.m_last_dl.is_none());
        assert!(noted(&p, "FS-195") && noted(&p, "FS-196") && noted(&p, "FS-198"));
        assert_eq!(p.lt_copy(&src, None), None);
    }
}

#[test]
fn fs197_freeing_no_array_both_modes() {
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let mut none = None;
        assert_eq!(p.lta_free(&mut none), 0);
        assert!(noted(&p, "FS-197"));
    }
}

#[test]
fn fs264_matrices_of_another_size_both_modes() {
    for mode in [Fix, Reproduce] {
        let mut p = proc(mode);
        let mut g = geom(4, 4, 4, 1.0);
        let small = Matrix::zeros(3, 3, MATRIX_REAL);
        assert_eq!(
            p.set_vox2ras_from_matrix(&mut g, &small),
            Err(Stop::Exit(252))
        );
        let mut col = Matrix::zeros(4, 1, MATRIX_REAL);
        assert!(p.matrix_of_voxel_sizes(&g, Some(&mut col)).is_err());
        assert!(p.vg_to_matrix(&g, Some(&mut col)).is_err());
        assert!(p.tkreg_to_scanner(&g, Some(&mut col)).is_err());
        assert!(noted(&p, "FS-264"));
        // a larger matrix: its top-left 4 x 4, the rest kept
        let mut big = Matrix::zeros(5, 5, MATRIX_REAL);
        big.set(5, 5, 9.0);
        let r = p.matrix_of_voxel_sizes(&g, Some(&mut big)).unwrap();
        assert_eq!(r, Returned::Output);
        assert_eq!((big.at(1, 1), big.at(4, 4), big.at(5, 5)), (1.0, 1.0, 9.0));
    }
}

#[test]
fn fs331_tkregister_matrix_into_a_larger_one() {
    let g = geom(4, 4, 4, 1.0);
    let mut p = proc(Fix);
    let Returned::New(want) = p.tkreg_to_scanner(&g, None).unwrap() else {
        panic!("a new matrix")
    };
    let mut wide = Matrix::zeros(4, 5, MATRIX_REAL);
    wide.set(1, 5, 9.0);
    assert_eq!(
        p.tkreg_to_scanner(&g, Some(&mut wide)).unwrap(),
        Returned::Output
    );
    for r in 1..=4 {
        for c in 1..=4 {
            assert_eq!(wide.at(r, c), want.at(r, c));
        }
    }
    assert_eq!(wide.at(1, 5), 9.0);
    assert!(noted(&p, "FS-331"));
    // reproduce: FreeSurfer's inverse and product into the given matrix; five rows refused
    let mut p = proc(Reproduce);
    let mut tall = Matrix::zeros(5, 5, MATRIX_REAL);
    assert_eq!(
        p.tkreg_to_scanner(&g, Some(&mut tall)).unwrap(),
        Returned::Nothing
    );
    let mut wide = Matrix::zeros(4, 5, MATRIX_REAL);
    assert_eq!(
        p.tkreg_to_scanner(&g, Some(&mut wide)).unwrap(),
        Returned::Output
    );
    assert!(noted(&p, "FS-331"));
}
