// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Transforms composed and inverted in place (track A's relay 43: `TransformConcat` with
//! `GCAMconcat2`/`GCAMconcat3`, `TransformInvertReplace` with `GCAMfillInverse`): small cases known
//! by hand and the refusals of FS-935 and FS-936. The fs-io probe's piece 32 compares the rest with
//! FreeSurfer's own code (`validation/probes/fs-io`, queries `K` and `P`).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_gca::gcam::{GCAM_VOX, Gcam, gcam_alloc};
use larmorx_freesurfer_gca::transform::{self as tr, Transform, Xform};
use larmorx_freesurfer_mri::lta::{LINEAR_VOX_TO_VOX, MORPH_3D_TYPE, lta_alloc};
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, mri_alloc_header, mri_copy_vol_geom_to_mri};
use larmorx_freesurfer_mri::vol_geom::{VolGeom, init_vol_geom};

fn ids(g: &Globals) -> Vec<&'static str> {
    g.notices.as_slice().iter().map(|n| n.defect.id).collect()
}

fn geom(n: i32) -> VolGeom {
    let mut v = VolGeom::default();
    init_vol_geom(&mut v);
    v.valid = 1;
    v.width = n;
    v.height = n;
    v.depth = n;
    v
}

/// An identity morph of `n` nodes a side and `spacing`, its image and atlas `n * spacing` voxels a
/// side, each node at its coordinates times the spacing plus `shift` along x.
fn morph(g: &mut Globals, n: i32, spacing: i32, shift: f64) -> Gcam {
    let mut e = Vec::new();
    let mut gc = gcam_alloc(g, &mut e, n, n, n).unwrap();
    gc.spacing = spacing;
    gc.image = geom(n * spacing.max(1));
    gc.atlas = geom(n * spacing.max(1));
    for node in &mut gc.nodes {
        node.x = node.x * f64::from(spacing) + shift;
        node.y *= f64::from(spacing);
        node.z *= f64::from(spacing);
    }
    gc
}

fn identity(g: &mut Globals, vg: &VolGeom) -> Transform {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut l = lta_alloc(g, &mut o, &mut e, 1, None).unwrap();
    l.ltype = LINEAR_VOX_TO_VOX;
    l.xforms[0].src.assign(g, vg);
    l.xforms[0].dst.assign(g, vg);
    Transform::new(LINEAR_VOX_TO_VOX, Xform::Lta(l))
}

/// An identity linear transform on the morph's image, then the morph (`GCAMconcat3(lta, gcam,
/// NULL)`): the morph again, on a grid of the atlas extent divided by the spacing; two morphs, the
/// first's atlas the second's image (`GCAMconcat2`): the positions carried through both.
#[test]
fn compositions_with_identities_and_of_two_morphs() {
    let env = Environment::default();
    let mut g = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let gc = morph(&mut g, 3, 1, 0.0);
    let image = gc.image.clone();
    let i = identity(&mut g, &image);
    let t = Transform::new(MORPH_3D_TYPE, Xform::Morph(Box::new(gc)));
    let mut arr = vec![i, t];
    let c = tr::transform_concat(&mut g, &mut o, &mut e, &env, &mut arr).unwrap();
    assert_eq!(c.ttype, MORPH_3D_TYPE);
    let m = c.gcam();
    assert_eq!(
        (m.width, m.height, m.depth, m.spacing, m.mtype),
        (3, 3, 3, 1, GCAM_VOX)
    );
    let n = m.node(1, 2, 0);
    assert_eq!((n.x, n.y, n.z), (1.0, 2.0, 0.0));
    assert_eq!((n.origx, n.origy, n.origz), (1.0, 2.0, 0.0));
    assert_eq!((n.xn, n.yn, n.zn), (1, 2, 0));
    // two morphs: shifted by 0.25 and by 0.5 along x: 0.75 in all, where the samples stay inside
    let a = Transform::new(
        MORPH_3D_TYPE,
        Xform::Morph(Box::new(morph(&mut g, 3, 1, 0.25))),
    );
    let b = Transform::new(
        MORPH_3D_TYPE,
        Xform::Morph(Box::new(morph(&mut g, 3, 1, 0.5))),
    );
    let mut arr = vec![a, b];
    let c = tr::transform_concat(&mut g, &mut o, &mut e, &env, &mut arr).unwrap();
    let m = c.gcam();
    let n = m.node(0, 1, 1);
    assert_eq!((n.x, n.y, n.z), (0.75, 1.0, 1.0));
    // the last column's point 2.5 takes the first morph's last node (its neighbour clamped)
    let n = m.node(2, 1, 1);
    assert_eq!((n.invalid, n.x, n.origx), (0, 2.25, 2.0));
}

/// FS-935: a morph of spacing 0 composed with a linear transform: FreeSurfer divides by the
/// spacing (a division fault): refused in both modes.
#[test]
fn fs935_a_morph_of_spacing_0_composed_is_refused() {
    let env = Environment::default();
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let gc = morph(&mut g, 2, 0, 0.0);
        let i = identity(&mut g, &gc.image.clone());
        let t = Transform::new(MORPH_3D_TYPE, Xform::Morph(Box::new(gc)));
        let mut arr = vec![i, t];
        assert!(tr::transform_concat(&mut g, &mut o, &mut e, &env, &mut arr).is_err());
        assert!(ids(&g).contains(&"FS-935"));
    }
}

/// `TransformInvertReplace` of a morph with its inverse made on a header of its image: an inverse
/// morph of the image's extent (the geometries exchanged); of a morph without inverse whose image
/// names no readable volume: `ERROR: reading` and FS-936 (refused in both modes); of a linear
/// array: its inverse.
#[test]
fn inverses_replace_the_transform() {
    let env = Environment::default();
    let mut g = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut gc = morph(&mut g, 3, 1, 0.0);
    gc.atlas = geom(3);
    gc.atlas.c_r = 5.0;
    let mut t = Transform::new(MORPH_3D_TYPE, Xform::Morph(Box::new(gc)));
    let mut h = mri_alloc_header(&mut g, &mut o, &mut e, 3, 3, 3, MRI_UCHAR, 1).unwrap();
    let image = t.gcam().image.clone();
    mri_copy_vol_geom_to_mri(&mut h, &image);
    tr::transform_invert(&mut g, &mut o, &mut e, &env, None, &mut t, Some(&h)).unwrap();
    tr::transform_invert_replace(&mut g, &mut o, &mut e, &env, None, &mut t, None).unwrap();
    let m = t.gcam();
    assert_eq!((m.width, m.height, m.depth), (3, 3, 3));
    assert_eq!((m.image.c_r, m.atlas.c_r), (5.0, 0.0));
    let n = m.node(1, 1, 1);
    assert_eq!((n.x, n.xn), (1.0, 1));
    assert!(String::from_utf8_lossy(&o).contains("Allocating inv_gcam...(3, 3, 3)"));
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let gc = morph(&mut g, 2, 1, 0.0);
        let mut t = Transform::new(MORPH_3D_TYPE, Xform::Morph(Box::new(gc)));
        let r = tr::transform_invert_replace(&mut g, &mut o, &mut e, &env, None, &mut t, None);
        assert!(r.is_err());
        assert!(String::from_utf8_lossy(&o).contains("ERROR: reading unknown"));
        assert!(ids(&g).contains(&"FS-936"));
    }
    let mut g = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut l = identity(&mut g, &geom(4));
    l.lta_mut().xforms[0]
        .m_l
        .as_mut()
        .unwrap()
        .set_relt(1, 4, 3.0);
    tr::transform_invert_replace(&mut g, &mut o, &mut e, &env, None, &mut l, None).unwrap();
    assert_eq!(l.lta().xforms[0].m_l.as_ref().unwrap().relt(1, 4), -3.0);
}
