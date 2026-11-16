// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The label functions' corrections (relay 37 of track D, `labelsurf.rs`, FS-630 to FS-632)
//! where the probe's `lbl` writes `G` (FreeSurfer reads or writes outside the surface's vertices
//! or the label's places) or FreeSurfer crashes.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, NO_ERROR};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::label::{Label, label_alloc};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::{alloc, labelsurf};

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
            env: Environment::from_pairs(vec![("SUBJECTS_DIR".to_owned(), "/".to_owned())]),
            out: Vec::new(),
            err: Vec::new(),
        }
    }

    /// A triangle: three vertices, each the other two's neighbour.
    fn triangle(&mut self) -> Mris {
        let mut m =
            alloc::alloc(&mut self.g, &mut self.out, &mut self.err, &self.sg, 3, 1).unwrap();
        m.face_mut(0).v = [0, 1, 2];
        for vno in 0..3usize {
            let t = m.topology_mut(vno);
            t.v = Some((0..3).filter(|&k| k != vno as i32).collect::<Vec<i32>>());
            t.vnum = 2;
        }
        m
    }

    fn label(&mut self, places: i32, vnos: &[i32]) -> Label {
        let mut a = label_alloc(&mut self.g, &self.env, &mut self.err, places, None, None).unwrap();
        for (k, &v) in vnos.iter().enumerate() {
            a.lv[k].vno = v;
        }
        a.n_points = vnos.len() as i32;
        a
    }

    fn noted(&self, id: &str) -> bool {
        self.g.notices.as_slice().iter().any(|n| n.defect.id == id)
    }
}

/// FS-630: a point naming no vertex is skipped by the marking, the area and the fill in both
/// modes (FreeSurfer writes or reads outside the vertices); the other points are used.
#[test]
fn fs630_points_outside_the_surface_are_skipped() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.triangle();
        s.vertex_mut(2).area = 2.5;
        let a = r.label(3, &[-1, 2, 7]);
        assert_eq!(labelsurf::label_mark(&mut r.g, &a, &mut s), NO_ERROR);
        assert_eq!((s.vertex(0).marked, s.vertex(2).marked), (0, 1));
        assert_eq!(labelsurf::label_area(&mut r.g, &a, &s), 2.5);
        assert!(r.noted("FS-630"));
    }
}

/// A full label grows by half its places (`nint(1.5 x places)`) as the dilation adds points.
#[test]
fn a_full_label_grows_by_half() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.triangle();
        let mut a = r.label(0, &[]);
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let res = labelsurf::label_dilate(&mut r.g, &mut io, &mut a, &mut s, 1, 0, None);
        assert_eq!(res.unwrap(), NO_ERROR);
        assert_eq!((a.n_points, a.max_points), (0, 0));
        let mut a = r.label(1, &[0]);
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        labelsurf::label_dilate(&mut r.g, &mut io, &mut a, &mut s, 1, 0, None).unwrap();
        assert_eq!((a.n_points, a.max_points), (3, 3));
        assert_eq!((a.lv[1].vno, a.lv[2].vno), (1, 2));
        assert!(r.g.notices.is_empty(), "{mode:?}");
    }
}

/// FS-632: inverting a label with a point beyond the surface: no label by default, FreeSurfer's
/// crash (status 139) with `reproduce`.
#[test]
fn fs632_inverting_a_label_beyond_the_surface() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let s = r.triangle();
        let a = r.label(2, &[0, 5]);
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let res = labelsurf::mris_label_invert(&mut r.g, &mut io, &s, &a);
        if mode.reproduce() {
            assert!(matches!(res, Err(Fatal::Exit(139))));
        } else {
            assert!(res.unwrap().is_none());
        }
        assert!(r.noted("FS-632"));
    }
    // a label inside the surface: its complement
    let mut r = Run::new(UpstreamDefects::Fix);
    let s = r.triangle();
    let a = r.label(1, &[1]);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    let inv = labelsurf::mris_label_invert(&mut r.g, &mut io, &s, &a)
        .unwrap()
        .unwrap();
    assert_eq!(inv.n_points, 2);
    assert_eq!((inv.lv[0].vno, inv.lv[1].vno), (0, 2));
}

/// FS-630: a vertex index table shorter than the surface is written only within it.
#[test]
fn fs630_a_short_vertex_index_table() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.triangle();
    let mut a = r.label(3, &[0, 1, 2]);
    a.vertex_label_ind = Some(vec![9, 9]);
    a.mris = Some(1);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    labelsurf::label_erode(&mut r.g, &mut io, &mut a, &mut s, 1).unwrap();
    assert_eq!(a.vertex_label_ind, Some(vec![0, 1]));
    assert!(r.noted("FS-630"));
}

/// FS-631: a label without places gains points when a ripped vertex keeps a mark (the marks'
/// clearing skips ripped vertices): FreeSurfer writes them past the label's places; larmorx makes
/// the places, in both modes.
#[test]
fn fs631_a_label_without_places_next_to_a_kept_mark() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.triangle();
        s.vertex_mut(0).ripflag = 1;
        s.vertex_mut(0).marked = 1;
        let mut a = r.label(0, &[]);
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        labelsurf::label_dilate(&mut r.g, &mut io, &mut a, &mut s, 1, 0, None).unwrap();
        assert_eq!(a.n_points, 2, "{mode:?}");
        assert_eq!((a.lv[0].vno, a.lv[1].vno), (1, 2));
        assert!(r.noted("FS-631"));
    }
}
