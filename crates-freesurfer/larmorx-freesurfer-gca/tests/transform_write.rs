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
use larmorx_freesurfer_gca::gcam_write::{gcam_write, write_morph};
use larmorx_freesurfer_gca::transform::{self as tr, Transform, Xform};
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::lta::MORPH_3D_TYPE;
use larmorx_freesurfer_mri::mri::{
    MGH_VERSION, MGZ_INTENT_UNKNOWN, MGZ_INTENT_WARPMAP, MRI_FLOAT, MRI_UCHAR, Mri,
    mri_alloc_header, mri_alloc_sequence,
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

/// FS-929 (and FS-855): a warp volume whose morph geometries were never given (here a volume made
/// in memory; in files, one without the geometry tag), written as MGH, then read as a GCA morph and
/// written as NIfTI (`mri_convert`'s path, which converts the displacements to RAS through those
/// geometries): noted in both modes, the geometries' validity and sizes 0 (FreeSurfer: memory
/// contents, 0 only in a fresh process).
#[test]
fn fs929_a_warp_volume_without_morph_geometries_written() {
    let t = tempfile::tempdir().unwrap();
    let env = Environment::default();
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let mut m = warp(&mut g, [3, 2, 2], |x, y, z| [x as f32, y as f32, z as f32]);
        m.intent = MGZ_INTENT_WARPMAP;
        m.version = ((MGZ_INTENT_WARPMAP & 0xffff) << 8) | MGH_VERSION;
        m.warp_field_format = larmorx_freesurfer_gca::gcam_write::WARPFIELD_DTFMT_ABS_CRS;
        let (mut out, mut err) = (Vec::new(), Vec::new());
        // names relative to the folder (an absolute Windows name is not one MRIread takes)
        let dir = Some(t.path());
        let p = "w.mgz";
        let r = mri_write(
            &mut g,
            &mut out,
            &mut err,
            &env,
            dir,
            &mut m,
            p,
            None,
            MGZ_INTENT_UNKNOWN,
        )
        .unwrap();
        assert_eq!(r, 0);
        assert!(ids(&g).contains(&"FS-929"), "{:?}", ids(&g));
        let mut g = Globals::new(mode);
        let mut gc = gcam::gcam_read(&mut g, &mut out, &mut err, &env, dir, p)
            .unwrap()
            .unwrap();
        assert!(gc.geom_unset);
        assert!(!ids(&g).contains(&"FS-929"));
        let r = gcam_write(&mut g, &mut out, &mut err, &env, dir, &mut gc, "o.nii", 0).unwrap();
        assert_eq!(r, 0);
        let text = String::from_utf8_lossy(&out).into_owned();
        assert!(text.contains("gcam image [0 x 0 x 0]"), "{text}");
        assert!(text.contains("convert displacement from 0 to 3"), "{text}");
        assert!(ids(&g).contains(&"FS-855"), "{:?}", ids(&g));
        assert!(ids(&g).contains(&"FS-929"), "{:?}", ids(&g));
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

/// FS-875: a morph with an affine and its inverse's index volumes copied (`TransformCopy` into a
/// new transform): the default keeps both; `reproduce` loses them, as FreeSurfer does. FS-876: a
/// copy into a transform of the other kind does not use it.
#[test]
fn fs875_fs876_a_morph_copy_keeps_its_inverse_and_affine() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut t = morph(&mut g, [2, 2, 2], 1);
        let mut vol = || {
            larmorx_freesurfer_mri::mri::mri_alloc(&mut g, &mut o, &mut e, 2, 2, 2, MRI_FLOAT)
                .unwrap()
        };
        let (x, y, z) = (vol(), vol(), vol());
        let larmorx_freesurfer_core::ret::Ret::New(id) =
            larmorx_freesurfer_core::matrix_ops::matrix_identity(&mut g, &mut o, &mut e, 4, None)
                .unwrap()
        else {
            panic!("an identity matrix")
        };
        {
            let gc = t.gcam_mut();
            gc.mri_xind = Some(x);
            gc.mri_yind = Some(y);
            gc.mri_zind = Some(z);
            gc.m_affine = Some(id);
        }
        let c = tr::transform_copy(&mut g, &mut o, &mut e, &t, None)
            .unwrap()
            .unwrap();
        assert!(ids(&g).contains(&"FS-875"));
        let gc = c.gcam();
        assert_eq!(gc.mri_xind.is_some(), mode.fix(), "{mode:?}");
        assert_eq!(gc.m_affine.is_some(), mode.fix(), "{mode:?}");
        assert_eq!(gc.nodes.len(), 8);
        assert_eq!(gc.nodes[3].x, t.gcam().nodes[3].x);
        // into a linear transform array: not used
        let lta = larmorx_freesurfer_mri::lta::lta_alloc(&mut g, &mut o, &mut e, 1, None).unwrap();
        let d = Transform::new(0, Xform::Lta(lta));
        let c = tr::transform_copy(&mut g, &mut o, &mut e, &t, Some(d))
            .unwrap()
            .unwrap();
        assert!(ids(&g).contains(&"FS-876"));
        assert_eq!(c.ttype, MORPH_3D_TYPE);
    }
}

/// Relay 41: `TransformCompose` (a left and a right matrix: FS-956 decides how both apply; one of
/// them alone multiplies the first matrix), `TransformCompose` of a morph (unsupported, `NULL`),
/// FS-928 (a composition into a morph refused), `TransformVox2Ras` of a morph (the image's
/// voxel-to-RAS matrix: a coronal image of 4 voxels a side, sizes 1, centre 0) and of a linear
/// array (its type becomes RAS to RAS).
#[test]
fn compose_and_convert() {
    let t = tempfile::tempdir().unwrap();
    let env = Environment::default();
    let lta = "type      = 0\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n1 0 0 2\n0 1 0 0\n0 0 1 0\n0 0 0 1\n";
    let p = write(t.path(), "v.lta", lta.as_bytes());
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut g = Globals::new(UpstreamDefects::Fix);
    let v = tr::transform_read(&mut g, &mut o, &mut e, &env, None, &p)
        .unwrap()
        .unwrap();
    let mut right = larmorx_freesurfer_core::matrix::Matrix::real(4, 4);
    for i in 1..=4 {
        right.set_relt(i, i, 1.0);
    }
    right.set_relt(2, 4, 5.0);
    // L * right: a shift of 2 in x and 5 in y
    let c = tr::transform_compose(&mut g, &mut o, &mut e, &v, None, Some(&right), None)
        .unwrap()
        .into_new()
        .unwrap();
    let m = c.lta().xforms[0].m_l.as_ref().unwrap();
    assert_eq!((m.relt(1, 4), m.relt(2, 4)), (2.0, 5.0));
    // into itself, left and right: (left * L) * right
    let mut w = tr::transform_read(&mut g, &mut o, &mut e, &env, None, &p)
        .unwrap()
        .unwrap();
    assert!(
        tr::transform_compose_in_place(&mut g, &mut o, &mut e, &mut w, Some(&right), Some(&right))
            .unwrap()
    );
    let m = w.lta().xforms[0].m_l.as_ref().unwrap();
    assert_eq!((m.relt(1, 4), m.relt(2, 4)), (2.0, 10.0));
    // a morph cannot be composed, nor composed into
    let mut mo = morph(&mut g, [2, 2, 2], 1);
    assert!(
        tr::transform_compose(&mut g, &mut o, &mut e, &mo, None, Some(&right), None)
            .unwrap()
            .is_null()
    );
    assert!(String::from_utf8_lossy(&e).contains("TransformCompose: MORPH_3D_TYPE unsupported"));
    let r = tr::transform_compose(&mut g, &mut o, &mut e, &v, None, None, Some(&mut mo)).unwrap();
    assert!(r.is_null() && ids(&g).contains(&"FS-928"));
    // a morph's positions to RAS (type GCAM_RAS afterwards), a linear array's type
    let x0 = mo.gcam().nodes[0].x;
    assert_eq!(
        tr::transform_vox2ras(&mut g, &mut o, &mut e, &mut mo, None, None).unwrap(),
        0
    );
    assert_eq!(mo.gcam().mtype, gcam::GCAM_RAS);
    assert_ne!(mo.gcam().nodes[0].x, x0);
    let mut u = tr::transform_read(&mut g, &mut o, &mut e, &env, None, &p)
        .unwrap()
        .unwrap();
    tr::transform_vox2ras(&mut g, &mut o, &mut e, &mut u, None, None).unwrap();
    assert_eq!(u.ttype, larmorx_freesurfer_mri::lta::LINEAR_RAS_TO_RAS);
}

/// A warp volume read as a morph by its absolute name: on Windows too (relay 42: MRIread put the
/// working directory before a name without `/`, so `C:\...\w.mgz` could not be opened).
#[test]
fn a_warp_volume_read_by_its_absolute_name() {
    let t = tempfile::tempdir().unwrap();
    let env = Environment::default();
    let mut g = Globals::new(UpstreamDefects::Fix);
    let mut m = warp(&mut g, [2, 2, 1], |x, y, _| [x as f32, y as f32, 0.5]);
    m.intent = MGZ_INTENT_WARPMAP;
    m.version = ((MGZ_INTENT_WARPMAP & 0xffff) << 8) | MGH_VERSION;
    m.warp_field_format = larmorx_freesurfer_gca::gcam_write::WARPFIELD_DTFMT_ABS_CRS;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let dir = Some(t.path());
    let r = mri_write(
        &mut g,
        &mut out,
        &mut err,
        &env,
        dir,
        &mut m,
        "w.mgz",
        None,
        MGZ_INTENT_UNKNOWN,
    )
    .unwrap();
    assert_eq!(r, 0);
    let p = t.path().join("w.mgz").to_string_lossy().into_owned();
    let gc = gcam::gcam_read(&mut g, &mut out, &mut err, &env, None, &p)
        .unwrap()
        .unwrap_or_else(|| panic!("{}", String::from_utf8_lossy(&err)));
    assert_eq!((gc.width, gc.height, gc.depth), (2, 2, 1));
    assert_eq!(gc.node(1, 1, 0).x, 1.0);
}
