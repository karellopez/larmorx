// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A volume resampled through a GCA morph (track A's relay 43: `GCAMmorphToAtlas`, its density
//! form, `GCAMmorphFromAtlas`, through the `TRANSFORM` functions): small cases known by hand and
//! the defaults of FS-930 to FS-932. The fs-io probe's piece 32 compares the rest with FreeSurfer's
//! own code (`validation/probes/fs-io`, queries `A`, `D`, `I`).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_gca::gcam::{Gcam, gcam_alloc, gcam_compute_original_properties};
use larmorx_freesurfer_gca::morph;
use larmorx_freesurfer_gca::transform::{self as tr, Transform, Xform};
use larmorx_freesurfer_mri::lta::MORPH_3D_TYPE;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, Mri, SAMPLE_NEAREST, SAMPLE_SINC, SAMPLE_TRILINEAR, mri_alloc_sequence,
};
use larmorx_freesurfer_mri::vol_geom::init_vol_geom;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

fn ids(g: &Globals) -> Vec<&'static str> {
    g.notices.as_slice().iter().map(|n| n.defect.id).collect()
}

/// An identity morph of `n` nodes a side, spacing 1, its atlas and image `n` voxels a side
/// (coronal, sizes 1), each node shifted by `shift` along x.
fn identity(g: &mut Globals, n: i32, shift: f64) -> Gcam {
    let mut e = Vec::new();
    let mut gc = gcam_alloc(g, &mut e, n, n, n).unwrap();
    for v in [&mut gc.image, &mut gc.atlas] {
        init_vol_geom(v);
        v.valid = 1;
        v.width = n;
        v.height = n;
        v.depth = n;
    }
    for node in &mut gc.nodes {
        node.x += shift;
    }
    gc
}

/// A float volume of `n` voxels a side, `x + 3 y + 9 z` at `(x, y, z)`.
fn ramp(g: &mut Globals, n: i32) -> Mri {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = mri_alloc_sequence(g, &mut o, &mut e, n, n, n, MRI_FLOAT, 1).unwrap();
    for x in 0..n {
        for y in 0..n {
            for z in 0..n {
                let v = (x + 3 * y + 9 * z) as f32;
                mri_set_vox_val(g, &mut e, &mut m, x, y, z, 0, v, 0).unwrap();
            }
        }
    }
    m
}

fn vox(g: &mut Globals, m: &Mri, x: i32, y: i32, z: i32) -> f32 {
    mri_get_vox_val(g, &mut Vec::new(), m, x, y, z, 0, 0).unwrap()
}

/// An identity morph carries the volume onto the atlas unchanged, but for the first slice under
/// `reproduce` (FS-932: FreeSurfer writes 0 for points at z = 0 of a 3-D volume).
#[test]
fn an_identity_morph_to_the_atlas() {
    let env = Environment::default();
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let src = ramp(&mut g, 3);
        let mut t = Transform::new(
            MORPH_3D_TYPE,
            Xform::Morph(Box::new(identity(&mut g, 3, 0.0))),
        );
        let m = tr::transform_apply(&mut g, &mut o, &mut e, &env, &mut t, &src)
            .unwrap()
            .into_new()
            .unwrap();
        assert_eq!(
            (m.vg.width, m.vg.height, m.vg.depth, m.nframes),
            (3, 3, 3, 1)
        );
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    let want = if z == 0 && mode == UpstreamDefects::Reproduce {
                        0.0
                    } else {
                        (x + 3 * y + 9 * z) as f32
                    };
                    assert_eq!(vox(&mut g, &m, x, y, z), want, "({x}, {y}, {z}) {mode:?}");
                }
            }
        }
        assert!(ids(&g).contains(&"FS-932"));
    }
}

/// A morph shifted by half a voxel, sampled with sinc (which the sampler does not support): one
/// error line per voxel; under `reproduce` each failed voxel takes the voxel before it (FS-930),
/// the first one memory contents (FS-931, 0 here); by default 0.
#[test]
fn failed_samples_take_the_previous_value_under_reproduce() {
    let env = Environment::default();
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let src = ramp(&mut g, 3);
        let gc = identity(&mut g, 3, 0.5);
        let m = morph::gcam_morph_to_atlas(
            &mut g,
            &mut o,
            &mut e,
            &env,
            &src,
            &gc,
            None,
            -1,
            SAMPLE_SINC,
        )
        .unwrap()
        .into_new()
        .unwrap();
        let text = String::from_utf8_lossy(&e).into_owned();
        let lines = text
            .matches("MRIsampleVolumeFrameType(2): unsupported interpolation type")
            .count();
        // every x + 0.5 is inside (below 3); z = 0 is outside under reproduce only (FS-932)
        let sampled = if mode == UpstreamDefects::Reproduce {
            3 * 3 * 2
        } else {
            3 * 3 * 3
        };
        assert_eq!(lines, sampled, "{mode:?}");
        // every voxel is 0: the first failure (FS-931) is 0, the others repeat it or are 0
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    assert_eq!(vox(&mut g, &m, x, y, z), 0.0);
                }
            }
        }
        let got = ids(&g);
        assert!(
            got.contains(&"FS-930") || got.contains(&"FS-931"),
            "{got:?}"
        );
    }
    // a value sampled before a failure is repeated under reproduce: the whole points of column 0
    // take their nearest voxels; the node (1, 0, 0), the next voxel in the loop's order (x
    // slowest), moved off the grid points: its sinc sample fails
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut g = Globals::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let src = ramp(&mut g, 3);
        let mut gc = identity(&mut g, 3, 0.0);
        let n = gc.node_mut(1, 0, 0);
        n.x = 1.5;
        n.z = 0.5;
        let m = morph::gcam_morph_to_atlas(
            &mut g,
            &mut o,
            &mut e,
            &env,
            &src,
            &gc,
            None,
            -1,
            SAMPLE_SINC,
        )
        .unwrap()
        .into_new()
        .unwrap();
        assert_eq!(vox(&mut g, &m, 0, 2, 2), (6 + 18) as f32);
        let want = if mode == UpstreamDefects::Reproduce {
            24.0
        } else {
            0.0
        };
        assert_eq!(vox(&mut g, &m, 1, 0, 0), want, "{mode:?}");
        assert!(ids(&g).contains(&"FS-930"));
    }
}

/// The density form multiplies by the Jacobian (1 for a morph that has not moved since its
/// original properties were stored), and an inverse resampling of the identity gives the volume
/// back (nearest neighbour).
#[test]
fn density_and_inverse_of_an_identity_morph() {
    let env = Environment::default();
    let mut g = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let src = ramp(&mut g, 3);
    let mut gc = identity(&mut g, 3, 0.0);
    gcam_compute_original_properties(&mut g, &mut o, &mut e, &env, &mut gc).unwrap();
    let mut t = Transform::new(MORPH_3D_TYPE, Xform::Morph(Box::new(gc)));
    let d =
        tr::transform_create_density_map(&mut g, &mut o, &mut e, &env, None, &mut t, &src, None)
            .unwrap()
            .into_new()
            .unwrap();
    for (x, y, z) in [(0, 0, 1), (1, 2, 2), (2, 1, 1)] {
        assert_eq!(vox(&mut g, &d, x, y, z), (x + 3 * y + 9 * z) as f32);
    }
    let inv = tr::transform_apply_inverse(&mut g, &mut o, &mut e, &env, None, &mut t, &src, None)
        .unwrap()
        .into_new()
        .unwrap();
    assert!(t.gcam().mri_xind.is_some());
    for (x, y, z) in [(0, 0, 0), (1, 2, 2), (2, 1, 0)] {
        assert_eq!(vox(&mut g, &inv, x, y, z), (x + 3 * y + 9 * z) as f32);
    }
    // TransformApplyType into a volume of another size: the program ends
    let mut wrong = mri_alloc_sequence(&mut g, &mut o, &mut e, 2, 3, 3, MRI_FLOAT, 1).unwrap();
    let r = tr::transform_apply_type(
        &mut g,
        &mut o,
        &mut e,
        &env,
        &mut t,
        &src,
        Some(&mut wrong),
        SAMPLE_TRILINEAR,
    );
    assert!(r.is_err());
    assert!(String::from_utf8_lossy(&e).contains("invalid input MRI size for GCAMmorphToAtlas()"));
    // into a volume of the atlas's size: the caller's
    let mut right = mri_alloc_sequence(&mut g, &mut o, &mut e, 3, 3, 3, MRI_FLOAT, 1).unwrap();
    let r = tr::transform_apply_type(
        &mut g,
        &mut o,
        &mut e,
        &env,
        &mut t,
        &src,
        Some(&mut right),
        SAMPLE_NEAREST,
    )
    .unwrap();
    assert!(matches!(r, Ret::Given));
    assert_eq!(vox(&mut g, &right, 2, 2, 2), 26.0);
}

/// Cubic B-spline sampling through a morph (relay 39 of track C, fs.bspline): `MRItoBSpline`'s
/// line, then each voxel the source's B-spline at the morphed point, as `MRIsampleBSpline` gives
/// it (a morph shifted by a quarter voxel along x; the last column's points lie outside).
#[test]
fn cubic_bspline_samples_through_a_morph() {
    use larmorx_freesurfer_mri::mri::SAMPLE_CUBIC_BSPLINE;
    use larmorx_freesurfer_volume::bspline::{mri_sample_bspline, mri_to_bspline};
    let env = Environment::default();
    let mut g = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let src = ramp(&mut g, 4);
    let gc = identity(&mut g, 4, 0.25);
    let m = morph::gcam_morph_to_atlas(
        &mut g,
        &mut o,
        &mut e,
        &env,
        &src,
        &gc,
        None,
        -1,
        SAMPLE_CUBIC_BSPLINE,
    )
    .unwrap()
    .into_new()
    .unwrap();
    assert_eq!(String::from_utf8_lossy(&o), "MRItoBSpline degree 3\n");
    let b = mri_to_bspline(&mut g, &mut Vec::new(), &mut Vec::new(), &src, None, 3).unwrap();
    for x in 0..4 {
        for y in 0..4 {
            for z in 1..4 {
                let want = if x < 3 {
                    let mut v = 0.0;
                    let (px, py, pz) = (f64::from(x as f32 + 0.25), f64::from(y), f64::from(z));
                    mri_sample_bspline(&mut g, &mut o, &mut e, &b, px, py, pz, 0, &mut v).unwrap();
                    v as f32
                } else {
                    // x + 0.25 = 3.25 is not below the width (4): FreeSurfer's test keeps it
                    let mut v = 0.0;
                    mri_sample_bspline(
                        &mut g,
                        &mut o,
                        &mut e,
                        &b,
                        3.25,
                        f64::from(y),
                        f64::from(z),
                        0,
                        &mut v,
                    )
                    .unwrap();
                    v as f32
                };
                assert_eq!(
                    vox(&mut g, &m, x, y, z).to_bits(),
                    want.to_bits(),
                    "({x}, {y}, {z})"
                );
            }
        }
    }
    // the B-spline interpolates: its value is near the ramp's (mirror borders bend it at the edges)
    let v = vox(&mut g, &m, 1, 2, 2);
    assert!((v - (1.25 + 6.0 + 18.0)).abs() < 0.05, "{v}");
}
