// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRIStransform` (track A's relay 45): small cases known by hand, and the defaults of FS-939 to
//! FS-942. The fs-gcam probe compares the rest with FreeSurfer's own code
//! (`validation/probes/fs-gcam`, `tests/fs_gca_official.rs`).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_gca::gcam::{GCAM_VOX, gcam_alloc};
use larmorx_freesurfer_gca::surftransform::{Volumes, mris_transform};
use larmorx_freesurfer_gca::transform::{Transform, Xform};
use larmorx_freesurfer_mri::lta::{
    LINEAR_PHYSVOX_TO_PHYSVOX, LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, MORPH_3D_TYPE, lta_alloc,
};
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc_header};
use larmorx_freesurfer_mri::vol_geom::{VolGeom, reinit_cache};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::alloc as salloc;
use larmorx_freesurfer_surf::mris::Mris;

fn ids(g: &Globals) -> Vec<&'static str> {
    g.notices.as_slice().iter().map(|n| n.defect.id).collect()
}

/// A coronal geometry of `n` voxels a side, voxel size 1, centre `c`.
fn coronal(n: i32, c: [f32; 3]) -> VolGeom {
    VolGeom {
        valid: 1,
        width: n,
        height: n,
        depth: n,
        ras_good_flag: 1,
        c_r: c[0],
        c_a: c[1],
        c_s: c[2],
        ..VolGeom::default()
    }
}

fn surface(g: &mut Globals, pts: &[[f32; 3]], real: i32) -> Mris {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = salloc::alloc(
        g,
        &mut o,
        &mut e,
        &SurfGlobals::default(),
        pts.len() as i32,
        0,
    )
    .expect("a surface");
    m.useRealRAS = real;
    m.vg = coronal(8, [0.0; 3]);
    for (i, p) in pts.iter().enumerate() {
        let v = m.vertex_mut(i);
        (v.x, v.y, v.z) = (p[0], p[1], p[2]);
    }
    m
}

fn volume(g: &mut Globals, vg: &VolGeom) -> Mri {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = mri_alloc_header(
        g, &mut o, &mut e, vg.width, vg.height, vg.depth, MRI_UCHAR, 1,
    )
    .expect("a header");
    let d = &mut m.vg;
    (d.c_r, d.c_a, d.c_s) = (vg.c_r, vg.c_a, vg.c_s);
    d.ras_good_flag = 1;
    reinit_cache(g, &mut o, &mut e, d).expect("the cache");
    m
}

/// A linear array of one transform of `ltype`: the identity shifted by `shift`, valid source and
/// destination geometries of 8 voxels a side.
fn linear(g: &mut Globals, ltype: i32, shift: [f32; 3]) -> Transform {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut lta = lta_alloc(g, &mut o, &mut e, 1, None).expect("an array");
    lta.ltype = ltype;
    let lt = &mut lta.xforms[0];
    lt.ltype = ltype;
    let m = lt.m_l.as_mut().expect("a matrix");
    for (r, s) in shift.iter().enumerate() {
        m.set_relt(r as i32 + 1, 4, *s);
    }
    lt.src = coronal(8, [0.0; 3]);
    lt.dst = coronal(8, [0.0; 3]);
    Transform::new(ltype, Xform::Lta(lta))
}

fn run(
    g: &mut Globals,
    mris: &mut Mris,
    vols: Volumes<'_>,
    t: &mut Transform,
) -> (Result<i32, Fatal>, String) {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let r = mris_transform(g, &mut o, &mut e, &Environment::default(), mris, vols, t);
    o.extend_from_slice(&e);
    (r, String::from_utf8_lossy(&o).into_owned())
}

fn xyz(m: &Mris, vno: usize) -> [f32; 3] {
    let v = m.vertex(vno);
    [v.x, v.y, v.z]
}

#[test]
fn a_ras_shift_of_a_surface_in_scanner_coordinates() {
    let mut g = Globals::new(UpstreamDefects::Fix);
    let mut s = surface(&mut g, &[[1.0, 2.0, 3.0], [-4.0, 0.5, 0.0]], 1);
    let mut t = linear(&mut g, LINEAR_RAS_TO_RAS, [10.0, -2.0, 0.5]);
    let (r, text) = run(&mut g, &mut s, Volumes::Separate(None, None), &mut t);
    assert_eq!(r.unwrap(), 0);
    assert_eq!(text, "INFO:try to get src info from transform.\n");
    assert_eq!(xyz(&s, 0), [11.0, 0.0, 3.5]);
    assert_eq!(xyz(&s, 1), [6.0, -1.5, 0.5]);
    assert_eq!((s.xlo, s.xhi, s.zlo, s.zhi), (6.0, 11.0, 0.5, 3.5));
    assert_eq!(s.vg.valid, 1);
    assert!(ids(&g).is_empty());
}

#[test]
fn a_tkregister_surface_through_a_ras_shift_moves_by_the_shift() {
    // tkregister RAS to scanner RAS (centre c), the shift, back (the destination's centre c):
    // the surface moves by the shift
    let mut g = Globals::new(UpstreamDefects::Fix);
    let mut s = surface(&mut g, &[[1.0, 2.0, 3.0]], 0);
    let mut t = linear(&mut g, LINEAR_RAS_TO_RAS, [4.0, 0.0, -1.0]);
    if let Xform::Lta(l) = &mut t.xform {
        l.xforms[0].src = coronal(8, [5.0, 6.0, 7.0]);
        l.xforms[0].dst = coronal(8, [5.0, 6.0, 7.0]);
    }
    let (r, _) = run(&mut g, &mut s, Volumes::Separate(None, None), &mut t);
    assert_eq!(r.unwrap(), 0);
    assert_eq!(xyz(&s, 0), [5.0, 2.0, 2.0]);
}

#[test]
fn a_voxel_identity_between_equal_volumes_keeps_the_surface() {
    let mut g = Globals::new(UpstreamDefects::Fix);
    let vg = coronal(8, [1.0, -2.0, 3.0]);
    let mut v = volume(&mut g, &vg);
    let mut s = surface(&mut g, &[[1.0, 2.0, 3.0], [-3.5, 0.25, 2.0]], 0);
    let mut t = linear(&mut g, LINEAR_VOX_TO_VOX, [0.0; 3]);
    let (r, text) = run(&mut g, &mut s, Volumes::Same(&mut v), &mut t);
    assert_eq!(r.unwrap(), 0);
    assert_eq!(text, "");
    assert_eq!(xyz(&s, 0), [1.0, 2.0, 3.0]);
    assert_eq!(xyz(&s, 1), [-3.5, 0.25, 2.0]);
    assert_eq!((s.vg.c_r, s.vg.c_a, s.vg.c_s), (1.0, -2.0, 3.0));
}

#[test]
fn fs942_another_linear_type_ends_the_program_in_both_modes() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let mut s = surface(&mut g, &[[1.0, 2.0, 3.0]], 0);
        let mut t = linear(&mut g, LINEAR_PHYSVOX_TO_PHYSVOX, [0.0; 3]);
        let (r, text) = run(&mut g, &mut s, Volumes::Separate(None, None), &mut t);
        assert!(matches!(r, Err(Fatal::Exit(_))));
        assert!(text.contains("a linear transform of type 2"), "{text}");
        assert_eq!(ids(&g), ["FS-942", "FS-01"]);
        // every vertex ripped: FreeSurfer never applies the missing matrix
        let mut g = Globals::new(mode);
        let mut s = surface(&mut g, &[[1.0, 2.0, 3.0]], 0);
        s.vertex_mut(0).ripflag = 1;
        let (r, _) = run(&mut g, &mut s, Volumes::Separate(None, None), &mut t);
        assert_eq!(r.unwrap(), 0);
        assert_eq!(xyz(&s, 0), [1.0, 2.0, 3.0]);
        assert!(ids(&g).is_empty());
    }
}

#[test]
fn fs858_a_linear_array_of_the_morph_type_ends_the_program() {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let mut s = surface(&mut g, &[[1.0, 2.0, 3.0]], 0);
    let mut t = linear(&mut g, LINEAR_RAS_TO_RAS, [0.0; 3]);
    t.ttype = MORPH_3D_TYPE;
    let (r, _) = run(&mut g, &mut s, Volumes::Separate(None, None), &mut t);
    assert!(matches!(r, Err(Fatal::Exit(_))));
    assert_eq!(ids(&g)[0], "FS-858");
}

#[test]
fn fs941_the_centre_warning_compares_the_volume_by_default() {
    let warn = "WARNING: c_(ras) values are not equal for the input volume and the transform src.";
    for (mode, shown) in [
        (UpstreamDefects::Fix, true),
        (UpstreamDefects::Reproduce, false),
    ] {
        let mut g = Globals::new(mode);
        let mut v = volume(&mut g, &coronal(8, [9.0, 0.0, 0.0]));
        let mut s = surface(&mut g, &[[1.0, 2.0, 3.0]], 0);
        s.vg.valid = 0;
        let mut t = linear(&mut g, LINEAR_RAS_TO_RAS, [0.0; 3]);
        let (r, text) = run(&mut g, &mut s, Volumes::Same(&mut v), &mut t);
        assert_eq!(r.unwrap(), 0);
        assert_eq!(text.contains(warn), shown, "{text}");
        assert_eq!(ids(&g), ["FS-941"]);
    }
}

/// An identity morph of 2 nodes a side (spacing 1), its image and atlas 2 voxels a side.
fn morph(g: &mut Globals) -> Transform {
    let mut e = Vec::new();
    let mut gc = gcam_alloc(g, &mut e, 2, 2, 2).expect("a morph");
    gc.mtype = GCAM_VOX;
    for v in [&mut gc.image, &mut gc.atlas] {
        *v = coronal(2, [0.0; 3]);
    }
    Transform::new(MORPH_3D_TYPE, Xform::Morph(Box::new(gc)))
}

#[test]
fn an_identity_morph_keeps_the_vertices_inside_it() {
    let mut g = Globals::new(UpstreamDefects::Fix);
    // tkregister RAS of the 2-voxel atlas: column 1 - x, slice y + 1, row 1 - z
    let mut s = surface(&mut g, &[[0.5, -0.5, 0.25], [0.0, 0.0, 0.0]], 0);
    let mut t = morph(&mut g);
    let (r, text) = run(&mut g, &mut s, Volumes::Separate(None, None), &mut t);
    assert_eq!(r.unwrap(), 0, "{text}");
    assert_eq!(xyz(&s, 0), [0.5, -0.5, 0.25]);
    assert_eq!(xyz(&s, 1), [0.0, 0.0, 0.0]);
    assert!(ids(&g).is_empty());
}

#[test]
fn fs939_fs940_a_vertex_outside_the_morph() {
    // the second vertex is outside (column 1 - 5 < 0): FreeSurfer gives it the first one's sample
    let pts = [[0.5, -0.5, 0.25], [5.0, 0.0, 0.0]];
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let mut s = surface(&mut g, &pts, 0);
    let mut t = morph(&mut g);
    let (r, _) = run(&mut g, &mut s, Volumes::Separate(None, None), &mut t);
    assert_eq!(r.unwrap(), 0);
    assert_eq!(xyz(&s, 1), [0.5, -0.5, 0.25]);
    assert_eq!(ids(&g), ["FS-939"]);
    let mut g = Globals::new(UpstreamDefects::Fix);
    let mut s = surface(&mut g, &pts, 0);
    let (r, text) = run(&mut g, &mut s, Volumes::Separate(None, None), &mut t);
    assert!(matches!(r, Err(Fatal::Exit(_))));
    assert!(text.contains("vertex 1 "), "{text}");
    // first: memory contents in FreeSurfer, the voxel (0, 0, 0) here
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let mut s = surface(&mut g, &[[5.0, 0.0, 0.0]], 0);
    let (r, _) = run(&mut g, &mut s, Volumes::Separate(None, None), &mut t);
    assert_eq!(r.unwrap(), 0);
    assert_eq!(xyz(&s, 0), [1.0, -1.0, 1.0]);
    assert_eq!(ids(&g), ["FS-940"]);
}
