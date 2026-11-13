// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! larmorx-freesurfer-mri: `fs.mri` (queue item 3.1-55) of FreeSurfer 8.2.0's replica
//! (docs/freesurfer/waves.md section 4): the `MRI` structure below the readers that build it, so
//! that `larmorx-freesurfer-io` and `larmorx-freesurfer-volume` both build on it (the
//! orchestrator's decision on 3.1-25, 2026-11-13: the data structure low, the functions that read
//! files above it).
//!
//! - [`vol_geom`]: `VOL_GEOM` (the geometry an `MRI` derives from) with C++'s copy semantics, and
//!   the voxel-to-RAS matrices on it (`MRIxfmCRS2XYZ`, `extract_i_to_r`, `MRIreInitCache`,
//!   `MRIsetVox2RASFromMatrix`, `MRIp0ToCRAS`, ...).
//! - [`ctab`]: the colour table's structure an `MRI` carries, with `CTABfree` and `CTABdeepCopy`
//!   (its reading and writing stay in `fs.labels`).
//! - [`mri`]: `MRI` itself: its constructors, `MRIalloc*`, `MRIfree`, `MRIsizeof`, `MRIcopyHeader`,
//!   the geometry copies, the intents.
//! - [`voxel`]: `MRIgetVoxVal`, `MRIsetVoxVal` and their derivatives.
//!
//! Translated from FreeSurfer's source under FreeSurfer's licence (`crates-freesurfer/LICENSE`);
//! larmorx (Apache-2.0) never links this crate. `PROVENANCE.md` lists what is translated.
#![forbid(unsafe_code)]

pub mod copy;
pub mod ctab;
pub mod mri;
pub mod tkreg;
pub mod vol_geom;
pub mod voxel;

/// `STRLEN` (`const.h`): the size of the structure's name fields (`fname`, `subject_name`, ...).
pub const STRLEN: usize = 1024;

#[cfg(test)]
mod tests {
    use larmorx_core::cfmt::Silent;
    use larmorx_io::xfm::{GeneralTransform, Mni};

    use larmorx_core::defects::{
        FS_54, FS_58, FS_59, FS_60, FS_161, FS_162, FS_163, FS_164, FS_165, FS_166, UpstreamDefects,
    };
    use larmorx_freesurfer_core::globals::Globals;

    use crate::mri::{self, MRI_FLOAT, MRI_LONG, Mri, Transform4};
    use crate::vol_geom::{self, VolGeom};
    use crate::voxel;

    fn setup(mode: UpstreamDefects) -> (Globals, Vec<u8>, Vec<u8>) {
        (Globals::new(mode), Vec::new(), Vec::new())
    }

    /// A linear MINC transform of `m` (its inverse computed as volume_io computes it).
    fn lin(m: Transform4) -> GeneralTransform {
        GeneralTransform::linear(Some(&m), &Mni, &mut Silent)
    }

    fn eye() -> Transform4 {
        let mut m = [[0.0; 4]; 4];
        for (i, row) in m.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        m
    }

    #[test]
    fn the_default_geometry_and_index_tables() {
        let (mut g, mut o, mut e) = setup(UpstreamDefects::Fix);
        let m = mri::mri_alloc(&mut g, &mut o, &mut e, 4, 3, 2, MRI_FLOAT).unwrap();
        assert_eq!((m.xend, m.xstart, m.imnr1, m.fov), (2.0, -2.0, 2, 4.0));
        assert_eq!(m.bytes_total, 4 * 4 * 3 * 2);
        let xi = m.xi.as_ref().unwrap();
        assert_eq!(
            [-500, -1, 0, 3, 4, 503].map(|i| Mri::index(xi, i)),
            [0, 0, 0, 3, 3, 3]
        );
        // coronal: column -1 in x, rows -1 in s, slices +1 in a; the centre voxel at (2, 1.5, 1)
        let v2r = vol_geom::extract_i_to_r(&mut g, &mut o, &mut e, &m.vg).unwrap();
        assert_eq!(
            [
                v2r.relt(1, 1),
                v2r.relt(1, 4),
                v2r.relt(2, 4),
                v2r.relt(3, 4)
            ],
            [-1.0, 2.0, -1.0, 1.5]
        );
        assert!(o.is_empty() && e.is_empty());
    }

    #[test]
    fn fs54_a_long_voxel_is_stored_once_by_default() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (mut g, mut o, mut e) = setup(mode);
            let mut m = mri::mri_alloc(&mut g, &mut o, &mut e, 2, 1, 1, MRI_LONG).unwrap();
            voxel::mri_set_vox_val(&mut g, &mut e, &mut m, 1, 0, 0, 0, 5.0, 0).unwrap();
            let v0 = voxel::mri_get_vox_val(&mut g, &mut e, &m, 0, 0, 0, 0, 0).unwrap();
            let v1 = voxel::mri_get_vox_val(&mut g, &mut e, &m, 1, 0, 0, 0, 0).unwrap();
            assert_eq!(v1, 5.0);
            if mode == UpstreamDefects::Fix {
                assert_eq!(v0, 0.0);
            } else {
                // voxel 0's high half holds the second write's 5
                assert_eq!(v0, (5_i64 << 32) as f32);
            }
            assert!(g.notices.contains(&FS_54));
            // 3e9 fits a long: kept by default, -2^31 (nint's int) with reproduce
            voxel::mri_set_vox_val(&mut g, &mut e, &mut m, 0, 0, 0, 0, 3e9, 0).unwrap();
            let v = voxel::mri_get_vox_val(&mut g, &mut e, &m, 0, 0, 0, 0, 0).unwrap();
            assert_eq!(
                v,
                if mode == UpstreamDefects::Fix {
                    3e9
                } else {
                    -2147483648.0
                }
            );
        }
    }

    #[test]
    fn fs58_voxels_of_a_header_are_an_error() {
        let (mut g, mut o, mut e) = setup(UpstreamDefects::Reproduce);
        let m = mri::mri_alloc_header(&mut g, &mut o, &mut e, 2, 2, 2, MRI_FLOAT, 1).unwrap();
        // negative indices give outside_val before the voxels are looked at, as FreeSurfer
        assert_eq!(
            voxel::mri_get_vox_val(&mut g, &mut e, &m, -1, 0, 0, 0, 0),
            Ok(0.0)
        );
        assert!(voxel::mri_get_vox_val(&mut g, &mut e, &m, 0, 0, 0, 0, 0).is_err());
        assert!(g.notices.contains(&FS_58));
    }

    #[test]
    fn fs59_an_assigned_geometry_without_matrices() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (mut g, mut o, mut e) = setup(mode);
            let m = mri::mri_alloc_header(&mut g, &mut o, &mut e, 2, 2, 2, MRI_FLOAT, 1).unwrap();
            let mut dst = m.vg.clone();
            dst.assign(&mut g, &VolGeom::default());
            assert!(g.notices.contains(&FS_59));
            assert_eq!(dst.i_to_r.is_some(), mode == UpstreamDefects::Reproduce);
            assert_eq!(dst.r_to_i.is_some(), mode == UpstreamDefects::Reproduce);
        }
    }

    #[test]
    fn fs60_and_the_copied_transform() {
        let (mut g, mut o, mut e) = setup(UpstreamDefects::Reproduce);
        let mut src = mri::mri_alloc_header(&mut g, &mut o, &mut e, 2, 2, 2, MRI_FLOAT, 1).unwrap();
        src.transform = Some(lin(eye()));
        src.linear_transform = Some(eye());
        let copy = mri::mri_copy_header(&mut g, &mut o, &mut e, &src, None)
            .unwrap()
            .unwrap_new();
        assert_eq!(
            (copy.free_transform, copy.linear_transform),
            (1, Some(eye()))
        );
        // onto a volume that owns a transform, from one without: dropped (both modes)
        let mut dst = copy;
        let plain = mri::mri_alloc_header(&mut g, &mut o, &mut e, 2, 2, 2, MRI_FLOAT, 1).unwrap();
        mri::mri_copy_header(&mut g, &mut o, &mut e, &plain, Some(&mut dst)).unwrap();
        assert!(g.notices.contains(&FS_60));
        assert_eq!((dst.free_transform, dst.linear_transform), (0, None));
        assert!(dst.transform.is_none());
    }

    #[test]
    fn an_xfm_inverted_twice_is_copied_with_its_flag_taken_in() {
        // an inverted .xfm read by larmorx_io (MINC 1.5.1's reader), inverted again, on a header
        let text = b"MNI Transform File\nTransform_Type = Linear;\nInvert_Flag = True;\nLinear_Transform =\n 2 0 0 1\n 0 4 0 2\n 0 0 8 3;\n";
        let (mut g, mut o, mut e) = setup(UpstreamDefects::Fix);
        let mut streams = Silent;
        let t = larmorx_io::xfm::input_transform_from_bytes(
            text,
            Some(b"t.xfm"),
            &Mni,
            &mut streams,
            &mut g.notices,
        )
        .expect("a linear transform");
        // the reader takes the flag into the matrices; a later invert_general_transform sets it
        assert!(!t.inverse_flag);
        let mut t = t;
        t.invert_in_place();
        let mut src = mri::mri_alloc_header(&mut g, &mut o, &mut e, 2, 2, 2, MRI_FLOAT, 1).unwrap();
        src.linear_transform = Some(mri::get_linear_transform_ptr(&mut e, &t).unwrap());
        src.inverse_linear_transform =
            Some(mri::get_inverse_linear_transform_ptr(&mut e, &t).unwrap());
        src.transform = Some(t);
        let copy = mri::mri_copy_header(&mut g, &mut o, &mut e, &src, None)
            .unwrap()
            .unwrap_new();
        let c = copy.transform.as_ref().unwrap();
        assert!(!c.inverse_flag);
        // inverted twice: the copy's forward matrix is the file's, its inverse x / 2 - 0.5, ...
        let f = copy.linear_transform.unwrap();
        assert_eq!((f[0][0], f[0][3], f[1][1], f[2][3]), (2.0, 1.0, 4.0, 3.0));
        let i = copy.inverse_linear_transform.unwrap();
        assert_eq!(
            (i[0][0], i[0][3], i[1][1], i[2][3]),
            (0.5, -0.5, 0.25, -0.375)
        );
        assert_eq!(copy.free_transform, 1);
        assert!(e.is_empty());
    }

    #[test]
    fn fs161_fs166_a_tensor_copy_is_whole_by_default() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (mut g, mut o, mut e) = setup(mode);
            let mut src = mri::mri_alloc(&mut g, &mut o, &mut e, 2, 2, 1, mri::MRI_TENSOR).unwrap();
            for (i, b) in src.chunk.as_mut().unwrap().iter_mut().enumerate() {
                *b = i as u8 + 1;
            }
            let c = crate::copy::mri_copy(&mut g, &mut o, &mut e, &src, None)
                .unwrap()
                .unwrap_new();
            let bytes = c.chunk.as_ref().unwrap();
            if mode == UpstreamDefects::Fix {
                assert_eq!(bytes, src.chunk.as_ref().unwrap());
            } else {
                // FreeSurfer: `width` bytes, every row at the first voxel
                assert_eq!(&bytes[..3], &[1, 2, 0]);
                assert!(bytes[2..].iter().all(|&b| b == 0));
            }
            assert!(g.notices.contains(&FS_161) && g.notices.contains(&FS_166));
        }
    }

    #[test]
    fn fs162_a_copy_clips_by_default() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (mut g, mut o, mut e) = setup(mode);
            let mut src = mri::mri_alloc(&mut g, &mut o, &mut e, 2, 1, 1, MRI_FLOAT).unwrap();
            voxel::mri_set_vox_val(&mut g, &mut e, &mut src, 0, 0, 0, 0, 40000.0, 0).unwrap();
            voxel::mri_set_vox_val(&mut g, &mut e, &mut src, 1, 0, 0, 0, -5.0, 0).unwrap();
            let mut dst = mri::mri_alloc(&mut g, &mut o, &mut e, 2, 1, 1, mri::MRI_SHORT).unwrap();
            crate::copy::mri_copy(&mut g, &mut o, &mut e, &src, Some(&mut dst)).unwrap();
            let v = |g: &mut Globals, e: &mut Vec<u8>, m: &Mri, c| {
                voxel::mri_get_vox_val(g, e, m, c, 0, 0, 0, 0).unwrap()
            };
            let x = v(&mut g, &mut e, &dst, 0);
            assert_eq!(
                x,
                if mode == UpstreamDefects::Fix {
                    32767.0
                } else {
                    -25536.0
                }
            );
            assert_eq!(v(&mut g, &mut e, &dst, 1), -5.0);
            let mut dst = mri::mri_alloc(&mut g, &mut o, &mut e, 2, 1, 1, mri::MRI_UCHAR).unwrap();
            crate::copy::mri_copy(&mut g, &mut o, &mut e, &src, Some(&mut dst)).unwrap();
            let y = v(&mut g, &mut e, &dst, 1);
            assert_eq!(
                y,
                if mode == UpstreamDefects::Fix {
                    0.0
                } else {
                    251.0
                }
            );
            assert_eq!(v(&mut g, &mut e, &dst, 0), 255.0);
            assert!(g.notices.contains(&FS_162));
        }
    }

    #[test]
    fn fs163_another_extent_is_refused_by_default() {
        let (mut g, mut o, mut e) = setup(UpstreamDefects::Fix);
        let src = mri::mri_alloc(&mut g, &mut o, &mut e, 2, 2, 1, MRI_FLOAT).unwrap();
        let mut dst = mri::mri_alloc(&mut g, &mut o, &mut e, 3, 2, 1, MRI_FLOAT).unwrap();
        assert!(crate::copy::mri_copy(&mut g, &mut o, &mut e, &src, Some(&mut dst)).is_err());
        assert!(g.notices.contains(&FS_163));
        // reproduce: rows at FreeSurfer's places inside the larger destination
        let (mut g, mut o, mut e) = setup(UpstreamDefects::Reproduce);
        let mut dst = mri::mri_alloc(&mut g, &mut o, &mut e, 3, 2, 1, MRI_FLOAT).unwrap();
        assert!(crate::copy::mri_copy(&mut g, &mut o, &mut e, &src, Some(&mut dst)).is_ok());
    }

    #[test]
    fn fs164_fs165_singular_and_misshapen_registrations() {
        use larmorx_freesurfer_core::matrix::Matrix;
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let (mut g, mut o, mut e) = setup(mode);
            let v = mri::mri_alloc_header(&mut g, &mut o, &mut e, 4, 4, 4, MRI_FLOAT, 1).unwrap();
            let zero = Matrix::real(4, 4);
            let r = crate::tkreg::tkreg2native(&mut g, &mut o, &mut e, &v.vg, &v.vg, Some(&zero));
            assert_eq!(r.is_ok(), mode == UpstreamDefects::Reproduce);
            assert!(g.notices.contains(&FS_164));
            let r34 = Matrix::real(3, 4);
            let r = crate::tkreg::fix_tkreg(&mut g, &mut o, &mut e, &v.vg, Some(&r34));
            assert!(r.is_err());
            assert!(g.notices.contains(&FS_165));
        }
    }
}
