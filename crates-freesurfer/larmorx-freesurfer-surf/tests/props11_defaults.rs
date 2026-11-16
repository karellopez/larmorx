// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The corrections and refusals of group (e) (relay 37 of track D, `props11.rs`, FS-626 to
//! FS-629): what the default gives against what `reproduce` gives where the probe's `mpx` cannot
//! compare (FreeSurfer reads or writes memory it does not own, or crashes; `mpx` writes `G` for
//! those volumes, and compares the rest with FreeSurfer's own build).

// The fixture indexes the vertices and their positions with one number, as the C does.
#![allow(clippy::needless_range_loop)]

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::histo_alloc;
use larmorx_freesurfer_core::log::{ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::{alloc, lta, props11};

struct Run {
    g: Globals,
    sg: SurfGlobals,
    env: Environment,
    out: Vec<u8>,
    err: Vec<u8>,
}

impl Run {
    fn new(mode: UpstreamDefects) -> Run {
        Run {
            g: Globals::new(mode),
            sg: SurfGlobals::default(),
            env: Environment::from_pairs(Vec::<(String, String)>::new()),
            out: Vec::new(),
            err: Vec::new(),
        }
    }

    /// A triangle (0,0,0), (2,0,0), (0,1,0): three vertices, each the other two's neighbour.
    fn triangle(&mut self, nv: i32) -> Mris {
        let mut m = alloc::alloc(&mut self.g, &mut self.out, &mut self.err, &self.sg, nv, 1)
            .expect("a surface");
        m.face_mut(0).v = [0, 1, 2];
        let p = [[0.0f32, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        for vno in 0..nv as usize {
            let t = m.topology_mut(vno);
            t.v = Some((0..3).filter(|&k| k != vno as i32).collect::<Vec<i32>>());
            t.vnum = 2;
            t.f = Some(vec![0]);
            t.n = Some(vec![vno as u8]);
            t.num = 1;
            t.vtotal = 2;
            let v = m.vertex_mut(vno);
            (v.x, v.y, v.z) = (p[vno][0], p[vno][1], p[vno][2]);
            (v.origx, v.origy, v.origz) = (v.x, v.y, v.z);
        }
        m
    }

    fn volume(&mut self, w: i32, h: i32, ty: i32) -> Mri {
        mri_alloc(&mut self.g, &mut self.out, &mut self.err, w, h, 1, ty).expect("a volume")
    }

    fn noted(&self, id: &str) -> bool {
        self.g.notices.as_slice().iter().any(|n| n.defect.id == id)
    }
}

fn identity() -> Matrix {
    let mut m = Matrix::real(4, 4);
    for k in 1..=4 {
        m.set_relt(k, k, 1.0);
    }
    m
}

/// FS-629: a Talairach transform without its matrix: an error by default (the surface kept),
/// FreeSurfer's crash (status 139) with `reproduce`.
#[test]
fn fs629_a_transform_without_its_matrix() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.triangle(3);
        s.lta = Some(lta::lta_alloc(&mut r.g, &mut r.out, &mut r.err, 1, None).unwrap());
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let res = props11::talairach_transform_in_place(&mut r.g, &mut io, &mut s);
        if mode.reproduce() {
            assert!(matches!(res, Err(Fatal::Exit(139))), "{res:?}");
        } else {
            assert_eq!(res.unwrap(), ERROR_BADPARM);
            assert_eq!(s.vertex(1).x, 2.0);
        }
        assert!(r.noted("FS-629"));
    }
}

/// FS-629: a destination with fewer vertices than the source: an error in both modes.
#[test]
fn fs629_a_smaller_destination() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut src = r.triangle(3);
        src.lta = Some(lta::lta_alloc(&mut r.g, &mut r.out, &mut r.err, 1, None).unwrap());
        src.SRASToTalSRAS_ = Some(identity());
        let mut dst = alloc::alloc(&mut r.g, &mut r.out, &mut r.err, &r.sg, 2, 0).unwrap();
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let res = props11::talairach_transform(&mut r.g, &mut r.sg, &mut io, &src, Some(&mut dst));
        assert!(matches!(res, Ok(Ret::Null)), "{mode:?}");
        assert!(r.noted("FS-629"));
    }
}

/// The move itself: a translation by (1, 2, 3) through the matrix, the box and its centre of the
/// moved vertices, the distances freed.
#[test]
fn talairach_moves_the_vertices_and_their_box() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.triangle(3);
    s.lta = Some(lta::lta_alloc(&mut r.g, &mut r.out, &mut r.err, 1, None).unwrap());
    let mut m = identity();
    for (k, x) in [1.0f32, 2.0, 3.0].into_iter().enumerate() {
        m.set_relt(k as i32 + 1, 4, x);
    }
    s.SRASToTalSRAS_ = Some(m);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    let res = props11::talairach_transform_in_place(&mut r.g, &mut io, &mut s).unwrap();
    assert_eq!(res, NO_ERROR);
    assert_eq!(
        (s.vertex(1).x, s.vertex(1).y, s.vertex(1).z),
        (3.0, 2.0, 3.0)
    );
    assert_eq!(
        (s.xlo, s.xhi, s.ylo, s.yhi, s.zlo, s.zhi),
        (1.0, 3.0, 2.0, 3.0, 3.0, 3.0)
    );
    assert_eq!((s.xctr, s.yctr, s.zctr), (2.0, 2.5, 3.0));
    assert_eq!(s.dist_alloced_flags & 1, 0);
    assert!(r.g.notices.is_empty());
}

/// FS-626: the normal-dot histogram needs 3 bins: an error by default (nothing changed), computed
/// with `reproduce`.
#[test]
fn fs626_a_normal_dot_histogram_of_two_bins() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.triangle(3);
        let mut h = histo_alloc(&mut r.g, &mut r.err, 2).unwrap();
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let res = props11::compute_normal_dot_distribution(
            &mut r.g, &mut r.sg, &mut io, None, &mut s, &mut h,
        )
        .unwrap();
        assert!(r.noted("FS-626"));
        if mode.reproduce() {
            assert_eq!(res, NO_ERROR);
        } else {
            assert_eq!(res, ERROR_BADPARM);
            assert_eq!(h.bin_size, 0.0);
            assert_eq!(h.counts, vec![0.0, 0.0]);
        }
    }
}

/// FS-627 is kept in both modes: the same histogram, the notice in both.
#[test]
fn fs627_the_normal_dot_formula_is_kept() {
    let mut results = Vec::new();
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.triangle(3);
        // a slanted triangle: dy ny dz nz and dy ny + dz nz differ
        s.vertex_mut(2).z = 1.0;
        s.vertex_mut(2).origz = 1.0;
        let mut h = histo_alloc(&mut r.g, &mut r.err, 10).unwrap();
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let res = props11::compute_normal_dot_distribution(
            &mut r.g, &mut r.sg, &mut io, None, &mut s, &mut h,
        )
        .unwrap();
        assert_eq!(res, NO_ERROR);
        assert!(r.noted("FS-627"));
        results.push((h.bin_size.to_bits(), h.counts.clone()));
    }
    assert_eq!(results[0], results[1]);
}

/// FS-628: a k1-k2 volume narrower than 100 or not of floats is refused in both modes; a larger
/// one is normalised over its whole first slice by default, over the 100 x 100 corner with
/// `reproduce`. FS-626: a curvature histogram without bins is refused by default.
#[test]
fn fs628_the_k1_k2_volume() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        for (w, h, ty) in [(50, 100, MRI_FLOAT), (100, 100, MRI_UCHAR)] {
            let mut r = Run::new(mode);
            let mut s = r.triangle(3);
            let mut m = r.volume(w, h, ty);
            let mut h1 = histo_alloc(&mut r.g, &mut r.err, 100).unwrap();
            let mut h2 = histo_alloc(&mut r.g, &mut r.err, 100).unwrap();
            let mut io = Io {
                out: &mut r.out,
                err: &mut r.err,
                env: &r.env,
            };
            let res = props11::compute_principal_curvature_distributions(
                &mut r.g, &mut r.sg, &mut io, None, &mut s, &mut h1, &mut h2, &mut m,
            )
            .unwrap();
            assert_eq!(res, ERROR_BADPARM, "{mode:?} {w} {ty}");
            assert!(r.noted("FS-628"));
        }
        let mut r = Run::new(mode);
        let mut s = r.triangle(3);
        let mut m = r.volume(120, 100, MRI_FLOAT);
        let mut h1 = histo_alloc(&mut r.g, &mut r.err, 100).unwrap();
        let mut h2 = histo_alloc(&mut r.g, &mut r.err, 100).unwrap();
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let res = props11::compute_principal_curvature_distributions(
            &mut r.g, &mut r.sg, &mut io, None, &mut s, &mut h1, &mut h2, &mut m,
        )
        .unwrap();
        assert_eq!(res, NO_ERROR);
        assert!(r.noted("FS-628"));
        let (mut corner, mut all) = (0f64, 0f64);
        for x in 0..120 {
            for y in 0..100 {
                let v =
                    f64::from(mri_get_vox_val(&mut r.g, &mut r.err, &m, x, y, 0, 0, 0).unwrap());
                all += v;
                if x < 100 {
                    corner += v;
                }
            }
        }
        if mode.reproduce() {
            assert!((corner - 1.0).abs() < 1e-3, "{corner}");
            // the columns past 100 keep their raw values (zeros: no vertex lands there)
            assert!((all - corner).abs() < 1e-9, "{all} {corner}");
        } else {
            assert!((all - 1.0).abs() < 1e-3, "{all}");
            assert!(all - corner > 0.1, "{all} {corner}");
        }
    }
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.triangle(3);
        let mut m = r.volume(100, 100, MRI_FLOAT);
        let mut h1 = histo_alloc(&mut r.g, &mut r.err, 0).unwrap();
        let mut h2 = histo_alloc(&mut r.g, &mut r.err, 100).unwrap();
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let res = props11::compute_principal_curvature_distributions(
            &mut r.g, &mut r.sg, &mut io, None, &mut s, &mut h1, &mut h2, &mut m,
        )
        .unwrap();
        assert!(r.noted("FS-626"));
        assert_eq!(
            res,
            if mode.reproduce() {
                NO_ERROR
            } else {
                ERROR_BADPARM
            }
        );
    }
}

/// `MRISvertexNormalToVoxelScaled` of a vertex without a normal: `ERROR_BADPARM` and no vector.
#[test]
fn a_zero_normal_has_no_scaled_voxel_normal() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let s = r.triangle(3);
    let m = r.volume(4, 4, MRI_FLOAT);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    let (code, p) =
        props11::vertex_normal_to_voxel_scaled(&mut r.g, &mut r.sg, &mut io, &s, s.vertex(0), &m)
            .unwrap();
    assert_eq!((code, p), (ERROR_BADPARM, None));
}
