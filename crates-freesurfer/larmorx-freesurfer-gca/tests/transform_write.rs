// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `TRANSFORM`s and the GCA morph writer (track A's relay 39): the paths the fs-io probe's piece 32
//! cannot compare (the default corrections of FS-856 to FS-862, where FreeSurfer crashes or reads
//! memory) and small cases known by hand. The probe compares the rest with FreeSurfer's own code
//! (`validation/probes/fs-io`), and the recorded heads' morphs too.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_BADPARM, ERROR_UNSUPPORTED};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_gca::gcam::{self, GCAM_POSITION_INVALID, gcam_alloc};
use larmorx_freesurfer_gca::gcam_write::write_morph;
use larmorx_freesurfer_gca::transform::{self as tr, Transform, Xform};
use larmorx_freesurfer_mri::lta::MORPH_3D_TYPE;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc_header, mri_alloc_sequence,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

fn ids(g: &Globals) -> Vec<&'static str> {
    g.notices.as_slice().iter().map(|n| n.defect.id).collect()
}

fn write(dir: &Path, name: &str, b: &[u8]) -> String {
    let p = dir.join(name);
    std::fs::File::create(&p).unwrap().write_all(b).unwrap();
    p.to_string_lossy().into_owned()
}

/// A three-frame float volume of `whd` voxels, frame `f` at `(x, y, z)` = `disp(x, y, z)[f]`.
fn warp(g: &mut Globals, whd: [i32; 3], disp: impl Fn(i32, i32, i32) -> [f32; 3]) -> Mri {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m =
        mri_alloc_sequence(g, &mut o, &mut e, whd[0], whd[1], whd[2], MRI_FLOAT, 3).unwrap();
    for x in 0..whd[0] {
        for y in 0..whd[1] {
            for z in 0..whd[2] {
                let d = disp(x, y, z);
                for (f, v) in d.into_iter().enumerate() {
                    mri_set_vox_val(g, &mut e, &mut m, x, y, z, f as i32, v, 0).unwrap();
                }
            }
        }
    }
    m
}

#[test]
fn a_written_morph_reads_back_as_its_displacements() {
    let t = tempfile::tempdir().unwrap();
    let env = Environment::default();
    for name in ["a.m3z", "a.m3d"] {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let m = warp(&mut g, [3, 2, 2], |x, y, z| {
            [0.25 * x as f32, -0.5, y as f32 + z as f32]
        });
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let p = t.path().join(name).to_string_lossy().into_owned();
        let r = write_morph(&mut g, &mut out, &mut err, &env, None, &m, &p).unwrap();
        assert_eq!(r, 0);
        let text = String::from_utf8_lossy(&out).into_owned();
        assert!(text.contains("GCAMreadWarpFromMRI(): warp and gcam have the same dims"));
        assert!(text.contains(&format!("GCAMwrite({p})")));
        // compressed only when the name holds .m3z
        let bytes = std::fs::read(&p).unwrap();
        assert_eq!(bytes[..2] == [0x1f, 0x8b], name.ends_with(".m3z"));
        // the morph read back: the nodes moved by the frames, as displacements in voxels
        let gc = gcam::gcam_read(&mut g, &mut out, &mut err, &env, None, &p)
            .unwrap()
            .unwrap();
        assert_eq!((gc.width, gc.height, gc.depth, gc.spacing), (3, 2, 2, 1));
        let n = gc.node(2, 1, 1);
        assert_eq!((n.origx, n.origy, n.origz), (2.0, 1.0, 1.0));
        assert_eq!((n.x, n.y, n.z), (2.5, 0.5, 3.0));
        // the identity's affine matrix, determinant 1
        assert_eq!(gc.det, 1.0);
    }
}

#[test]
fn fs856_a_morph_writer_that_writes_nothing_is_an_error_by_default() {
    let t = tempfile::tempdir().unwrap();
    let env = Environment::default();
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let m = warp(&mut g, [2, 2, 1], |_, _, _| [0.0; 3]);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        // a folder that does not exist, then a name of another format
        let p = t.path().join("no/o.m3z").to_string_lossy().into_owned();
        let r = write_morph(&mut g, &mut out, &mut err, &env, None, &m, &p).unwrap();
        let q = t.path().join("o.img").to_string_lossy().into_owned();
        let r2 = write_morph(&mut g, &mut out, &mut err, &env, None, &m, &q).unwrap();
        let e = String::from_utf8_lossy(&err).into_owned();
        assert!(e.contains("could not open file"), "{e}");
        assert!(!Path::new(&q).exists());
        assert!(ids(&g).contains(&"FS-856"));
        if mode.fix() {
            assert_eq!((r, r2), (ERROR_BADPARM, ERROR_BADPARM));
            assert!(e.contains("wrote nothing: the name is not .m3z, .m3d, .mgz or .nii"));
        } else {
            assert_eq!((r, r2), (0, 0));
        }
    }
}

/// A morph transform of `whd` nodes of spacing `sp`, each moved by +1 in x, with an image of
/// `whd` voxels.
fn morph(g: &mut Globals, whd: [i32; 3], sp: i32) -> Transform {
    let mut e = Vec::new();
    let mut gc = gcam_alloc(g, &mut e, whd[0], whd[1], whd[2]).unwrap();
    gc.spacing = sp;
    for n in &mut gc.nodes {
        n.x += 1.0;
    }
    gc.image.width = whd[0];
    gc.image.height = whd[1];
    gc.image.depth = whd[2];
    Transform::new(MORPH_3D_TYPE, Xform::Morph(Box::new(gc)))
}

#[test]
fn inverse_samples_of_a_morph() {
    let env = Environment::default();
    let mut g = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut t = morph(&mut g, [3, 3, 3], 2);
    // the node at voxel / spacing (integers), clamped to the grid
    let (r, p) =
        tr::transform_sample_inverse(&mut g, &mut o, &mut e, &env, &mut t, 3, 5, 9).unwrap();
    assert_eq!((r, p), (0, [2.0, 2.0, 2.0]));
    t.gcam_mut().node_mut(1, 2, 2).invalid = 1;
    let (r, _) =
        tr::transform_sample_inverse(&mut g, &mut o, &mut e, &env, &mut t, 2, 4, 4).unwrap();
    assert_eq!(r, ERROR_BADPARM);
    // the trilinear sample of the nodes (GCAMsampleMorph): (1, 1, 1) / 2 lies between nodes 0 and 1
    let (r, p) = tr::gcam_sample_morph(t.gcam(), 1.0, 1.0, 1.0);
    assert_eq!((r, p), (0, [1.5, 0.5, 0.5]));
    // outside the grid, or next to a node of invalid position
    assert_eq!(
        tr::gcam_sample_morph(t.gcam(), 6.0, 0.0, 0.0).0,
        ERROR_BADPARM
    );
    t.gcam_mut().node_mut(0, 0, 0).invalid = GCAM_POSITION_INVALID;
    assert_eq!(
        tr::gcam_sample_morph(t.gcam(), 1.0, 1.0, 1.0).0,
        ERROR_BADPARM
    );
    // a morph without inverse index volumes
    let (r, p) = tr::transform_sample(&mut g, &mut o, &mut e, &env, &mut t, 1.0, 1.0, 1.0).unwrap();
    assert_eq!((r, p), (ERROR_UNSUPPORTED, [0.0; 3]));
    assert!(String::from_utf8_lossy(&e).contains("gcam has not been inverted!"));
}

#[test]
fn fs857_fs861_inverse_samples_freesurfer_crashes_on_end_the_program() {
    let env = Environment::default();
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut g = Globals::new(mode);
        let mut t = morph(&mut g, [2, 2, 2], 0);
        assert!(
            tr::transform_sample_inverse(&mut g, &mut o, &mut e, &env, &mut t, 1, 1, 1).is_err()
        );
        assert!(ids(&g).contains(&"FS-857"));
        let mut g = Globals::new(mode);
        let mut t = morph(&mut g, [0, 2, 2], 1);
        assert!(
            tr::transform_sample_inverse(&mut g, &mut o, &mut e, &env, &mut t, 1, 1, 1).is_err()
        );
        assert!(ids(&g).contains(&"FS-861"));
        let mut g = Globals::new(mode);
        let mut none = None;
        assert_eq!(tr::transform_free(&mut g, &mut none), 0);
        assert!(ids(&g).contains(&"FS-860"));
    }
}

#[test]
fn fs862_the_nearest_sample_stays_inside_by_default() {
    let env = Environment::default();
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut t = morph(&mut g, [2, 2, 2], 1);
        // inverse index volumes on an image of 2 x 2 x 2 voxels (the nodes moved by +1 in x)
        let mut img = mri_alloc_header(&mut g, &mut o, &mut e, 2, 2, 2, MRI_UCHAR, 1).unwrap();
        img.vg.xsize = 1.0;
        tr::transform_invert(&mut g, &mut o, &mut e, &env, None, &mut t, Some(&img)).unwrap();
        // within half a voxel of the far face in z: FreeSurfer reads slice 2 of 2
        let (r, p) =
            tr::transform_sample_real(&mut g, &mut o, &mut e, &env, &mut t, 0.0, 0.0, 1.5).unwrap();
        assert_eq!(r, 0);
        assert!(ids(&g).contains(&"FS-862"));
        if mode.fix() {
            let xi = t.gcam().mri_zind.as_ref().unwrap();
            let want = mri_get_vox_val(&mut g, &mut e, xi, 0, 0, 1, 0, 0).unwrap();
            assert_eq!(p[2], want);
        }
    }
}

#[test]
fn fs858_fs859_linear_files_freesurfer_misreads() {
    let t = tempfile::tempdir().unwrap();
    let env = Environment::default();
    let lta = |ty: i32, n: i32| {
        let mut s = format!("type      = {ty}\nnxforms   = {n}\n");
        for _ in 0..n {
            s += "mean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n";
            s += "1 0 0 2\n0 1 0 0\n0 0 1 0\n0 0 0 1\n";
        }
        s
    };
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut g = Globals::new(mode);
        let p = write(t.path(), "m.lta", lta(11, 1).as_bytes());
        assert!(
            tr::transform_read(&mut g, &mut o, &mut e, &env, None, &p)
                .unwrap()
                .is_none()
        );
        assert!(ids(&g).contains(&"FS-858"));
        let p = write(t.path(), "z.lta", lta(0, 0).as_bytes());
        let mut z = tr::transform_read(&mut g, &mut o, &mut e, &env, None, &p)
            .unwrap()
            .unwrap();
        assert!(tr::transform_sample(&mut g, &mut o, &mut e, &env, &mut z, 1.0, 1.0, 1.0).is_err());
        assert!(ids(&g).contains(&"FS-859"));
        // a voxel-to-voxel matrix shifting x by 2, and its inverse
        let p = write(t.path(), "v.lta", lta(0, 1).as_bytes());
        let mut v = tr::transform_read(&mut g, &mut o, &mut e, &env, None, &p)
            .unwrap()
            .unwrap();
        let (_, q) =
            tr::transform_sample(&mut g, &mut o, &mut e, &env, &mut v, 1.0, 2.0, 3.0).unwrap();
        assert_eq!(q, [3.0, 2.0, 3.0]);
        tr::transform_invert(&mut g, &mut o, &mut e, &env, None, &mut v, None).unwrap();
        let (_, q) =
            tr::transform_sample_inverse(&mut g, &mut o, &mut e, &env, &mut v, 1, 2, 3).unwrap();
        assert_eq!(q, [-1.0, 2.0, 3.0]);
        let (_, q) =
            tr::transform_sample_real(&mut g, &mut o, &mut e, &env, &mut v, -5.0, 2.0, 3.0)
                .unwrap();
        assert_eq!(q, [0.0, 2.0, 3.0]);
        // identity.nofile: no file, an identity RAS-to-RAS transform
        let id = tr::transform_read(&mut g, &mut o, &mut e, &env, None, "x/identity.nofile")
            .unwrap()
            .unwrap();
        assert_eq!(id.ttype, 1);
    }
}
