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

pub mod ctab;
pub mod mri;
pub mod vol_geom;
pub mod voxel;

/// `STRLEN` (`const.h`): the size of the structure's name fields (`fname`, `subject_name`, ...).
pub const STRLEN: usize = 1024;

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::io::Write;

    use larmorx_core::defects::{FS_54, FS_58, FS_59, FS_60, UpstreamDefects};
    use larmorx_freesurfer_core::globals::Globals;

    use crate::mri::{self, GeneralTransform, MRI_FLOAT, MRI_LONG, Mri, Transform4};
    use crate::vol_geom::{self, VolGeom};
    use crate::voxel;

    fn setup(mode: UpstreamDefects) -> (Globals, Vec<u8>, Vec<u8>) {
        (Globals::new(mode), Vec::new(), Vec::new())
    }

    #[derive(Debug, Clone)]
    struct Lin(Transform4);

    impl GeneralTransform for Lin {
        fn clone_box(&self) -> Box<dyn GeneralTransform> {
            Box::new(self.clone())
        }
        fn linear(&self, _: &mut dyn Write) -> Option<Transform4> {
            Some(self.0)
        }
        fn inverse_linear(&self, _: &mut dyn Write) -> Option<Transform4> {
            Some(self.0)
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
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
        src.transform = Some(Box::new(Lin(eye())));
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
}
