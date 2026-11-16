// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The surface hash table's corrections (relay 39 of track D, `mht.rs`, FS-909 to FS-913), the
//! hole filling's (`vals.rs`, FS-914, FS-915) and `mrisMarkIntersections`' (`props10.rs`, FS-914,
//! FS-916), on small surfaces read from binary triangle files.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mht::{Mht, TABLE_CENTER};
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::views::CURRENT_VERTICES;
use larmorx_freesurfer_surf::{props10, readwrite, vals};

struct Run {
    g: Globals,
    sg: SurfGlobals,
    env: Environment,
    out: Vec<u8>,
    err: Vec<u8>,
}

macro_rules! io {
    ($r:expr) => {
        Io {
            out: &mut $r.out,
            err: &mut $r.err,
            env: &$r.env,
        }
    };
}

impl Run {
    fn new(mode: UpstreamDefects) -> Run {
        Run {
            g: Globals::new(mode),
            sg: SurfGlobals::default(),
            env: Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]),
            out: Vec::new(),
            err: Vec::new(),
        }
    }

    /// The surface of `v` and `f` written as a binary triangle file and read by `MRISread`.
    fn surface(&mut self, v: &[[f32; 3]], f: &[[i32; 3]]) -> Mris {
        let mut b = vec![0xff, 0xff, 0xfe];
        b.extend_from_slice(b"created by me on Sun Nov 15 06:10:00 2026\n\n");
        b.extend_from_slice(&(v.len() as i32).to_be_bytes());
        b.extend_from_slice(&(f.len() as i32).to_be_bytes());
        for p in v {
            for c in p {
                b.extend_from_slice(&c.to_be_bytes());
            }
        }
        for q in f {
            for x in q {
                b.extend_from_slice(&x.to_be_bytes());
            }
        }
        let td = tempfile::tempdir().expect("a folder");
        std::fs::write(td.path().join("lh.s0"), b).expect("the file");
        let mut io = io!(self);
        readwrite::read(
            &mut self.g,
            &mut self.sg,
            &mut io,
            Some(td.path()),
            "lh.s0",
            true,
        )
        .unwrap()
        .expect("a surface")
    }

    fn noted(&self, id: &str) -> bool {
        self.g.notices.as_slice().iter().any(|n| n.defect.id == id)
    }
}

/// Two triangles crossing each other (faces 0 and 1), moved by `d` along x.
fn crossing(d: f32) -> (Vec<[f32; 3]>, Vec<[i32; 3]>) {
    let v = vec![
        [d, 0.0, 0.0],
        [d + 2.0, 0.0, 0.0],
        [d, 2.0, 0.0],
        [d + 0.5, 0.5, -1.0],
        [d + 0.5, 0.5, 1.0],
        [d + 1.5, 0.5, 0.0],
    ];
    (v, vec![[0, 1, 2], [3, 4, 5]])
}

fn intersections(r: &mut Run, s: &Mris, res: f32) -> Result<Vec<i32>, Fatal> {
    let m = Mht::create_face_table_resolution(&mut r.g, &mut r.err, s, CURRENT_VERTICES, res)?;
    (0..s.nfaces)
        .map(|f| m.does_face_intersect(&mut r.g, &mut r.err, s, f))
        .collect()
}

/// Near the origin the crossing is found in both modes, and nothing is noted.
#[test]
fn a_crossing_near_the_origin() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = crossing(0.0);
        let s = r.surface(&v, &f);
        assert_eq!(intersections(&mut r, &s, 1.0).unwrap(), vec![1, 1]);
        for id in ["FS-909", "FS-910", "FS-911", "FS-912", "FS-913"] {
            assert!(!r.noted(id), "{mode:?} {id}");
        }
    }
}

/// FS-909: a resolution of 0: a clear error by default; with `reproduce` every face in the first
/// bucket and the crossing missed, as FreeSurfer.
#[test]
fn fs909_a_resolution_of_zero() {
    let (v, f) = crossing(0.0);
    let mut r = Run::new(UpstreamDefects::Fix);
    let s = r.surface(&v, &f);
    assert!(intersections(&mut r, &s, 0.0).is_err());
    assert!(r.noted("FS-909"));
    let mut r = Run::new(UpstreamDefects::Reproduce);
    let s = r.surface(&v, &f);
    let m =
        Mht::create_face_table_resolution(&mut r.g, &mut r.err, &s, CURRENT_VERTICES, 0.0).unwrap();
    assert_eq!(m.bucket(0, 0, 0), Some(&[0, 1][..]));
    assert_eq!(m.bucket_count(), 1);
    assert_eq!(intersections(&mut r, &s, 0.0).unwrap(), vec![0, 0]);
    assert!(r.noted("FS-909"));
}

/// FS-910: faces 1,500 voxels from the origin: stored at the table's edge; the crossing found by
/// default, missed with `reproduce`.
#[test]
fn fs910_faces_beyond_the_table() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = crossing(1500.0);
        let s = r.surface(&v, &f);
        let m = Mht::create_face_table(&mut r.g, &mut r.err, &s).unwrap();
        assert!(m.bucket(1999, TABLE_CENTER, TABLE_CENTER).is_some());
        let expect = if mode.reproduce() { 0 } else { 1 };
        assert_eq!(
            intersections(&mut r, &s, 1.0).unwrap(),
            vec![expect, expect],
            "{mode:?}"
        );
        assert!(r.noted("FS-910"));
    }
}

/// FS-911: a 150 mm triangle touches more than 10,000 voxels at 1 mm: FreeSurfer lists the first
/// 10,000 (`reproduce`, with its messages); by default every one.
#[test]
fn fs911_a_face_of_more_than_ten_thousand_voxels() {
    let v = [[0.0, 0.0, 0.0], [150.0, 0.0, 0.0], [0.0, 150.0, 0.0]];
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let s = r.surface(&v, &[[0, 1, 2]]);
        let m = Mht::create_face_table(&mut r.g, &mut r.err, &s).unwrap();
        assert!(r.noted("FS-911"));
        if mode.reproduce() {
            assert_eq!(m.bucket_count(), 10_000);
            assert!(String::from_utf8_lossy(&r.err).contains("complete list too big!"));
        } else {
            assert!(m.bucket_count() > 10_000);
        }
    }
}

/// FS-912: a triangle over 11,250 faces: FreeSurfer ends the program (status 1, `reproduce`); by
/// default the test goes on.
#[test]
fn fs912_more_than_ten_thousand_faces_near_one() {
    let n = 76i32;
    let mut v = vec![[-1.0, -1.0, 0.0], [40.0, -1.0, 0.0], [-1.0, 40.0, 0.0]];
    let mut f = vec![[0, 1, 2]];
    for j in 0..n {
        for i in 0..n {
            v.push([i as f32 * 0.25, j as f32 * 0.25, 0.01]);
        }
    }
    for j in 0..n - 1 {
        for i in 0..n - 1 {
            let (a, b, c, d) = (
                3 + j * n + i,
                4 + j * n + i,
                3 + (j + 1) * n + i,
                4 + (j + 1) * n + i,
            );
            f.push([a, b, d]);
            f.push([a, d, c]);
        }
    }
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let s = r.surface(&v, &f);
        let m = Mht::create_face_table(&mut r.g, &mut r.err, &s).unwrap();
        let got = m.does_face_intersect(&mut r.g, &mut r.err, &s, 0);
        if mode.reproduce() {
            assert_eq!(got, Err(Fatal::Exit(1)));
        } else {
            assert_eq!(got, Ok(0));
        }
        assert!(r.noted("FS-912"));
    }
}

/// FS-913: a corner that is not finite next to finite ones: FreeSurfer's sampling runs for hours;
/// refused with `reproduce` (FS-909's error first by default).
#[test]
fn fs913_a_sampling_that_runs_for_hours() {
    // MRISread refuses a NaN in the file: set after the read
    let v = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
    let mut r = Run::new(UpstreamDefects::Reproduce);
    let mut s = r.surface(&v, &[[0, 1, 2]]);
    s.vertex_mut(0).x = f32::NAN;
    assert!(Mht::create_face_table(&mut r.g, &mut r.err, &s).is_err());
    assert!(r.noted("FS-913"));
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.surface(&v, &[[0, 1, 2]]);
    s.vertex_mut(0).x = f32::NAN;
    assert!(Mht::create_face_table(&mut r.g, &mut r.err, &s).is_err());
    assert!(r.noted("FS-909"));
}

/// A 4 x 4 grid of squares (25 vertices, 32 faces).
fn grid() -> (Vec<[f32; 3]>, Vec<[i32; 3]>) {
    let mut v = Vec::new();
    for j in 0..5 {
        for i in 0..5 {
            v.push([i as f32, j as f32, 0.0]);
        }
    }
    let mut f = Vec::new();
    for j in 0..4 {
        for i in 0..4 {
            let (a, b, c, d) = (
                j * 5 + i,
                j * 5 + i + 1,
                (j + 1) * 5 + i,
                (j + 1) * 5 + i + 1,
            );
            f.push([a, b, d]);
            f.push([a, d, c]);
        }
    }
    (v, f)
}

/// `MRISfillHoles` of the grid with the caller's map `value(vertex)` at 0.5: what it returns, the
/// centre vertex's map value afterwards, what it printed.
fn fill(mode: UpstreamDefects, value: impl Fn(i32) -> f32) -> (Run, i32, f32, String) {
    let (v, f) = grid();
    let mut r = Run::new(mode);
    let mut s = r.surface(&v, &f);
    let mut m =
        mri_alloc_sequence(&mut r.g, &mut r.out, &mut r.err, 25, 1, 1, MRI_FLOAT, 1).unwrap();
    for k in 0..25 {
        mri_set_vox_val(&mut r.g, &mut r.err, &mut m, k, 0, 0, 0, value(k), 0).unwrap();
    }
    let mut io = io!(r);
    let n = vals::fill_holes(&mut r.g, &mut io, &mut s, None, Some(&mut m), 0.5).unwrap();
    let centre = mri_get_vox_val(&mut r.g, &mut r.err, &m, 12, 0, 0, 0, 0).unwrap();
    let printed = String::from_utf8_lossy(&r.out).into_owned();
    (r, n, centre, printed)
}

/// FS-915: a ring of the map (1) around a centre at the threshold (0.5), the border outside it
/// (0): the centre is a hole filled by default (taken as outside the map), left with `reproduce`
/// (in neither map).
#[test]
fn fs915_a_hole_at_the_threshold() {
    let ring = |k: i32| {
        let (i, j) = (k % 5, k / 5);
        if i == 0 || j == 0 || i == 4 || j == 4 {
            0.0
        } else if k == 12 {
            0.5
        } else {
            1.0
        }
    };
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, n, centre, printed) = fill(mode, ring);
        assert!(r.noted("FS-915"));
        if mode.reproduce() {
            assert_eq!((n, centre), (0, 0.0));
            assert!(printed.contains("found 0 holes"), "{printed}");
        } else {
            assert_eq!((n, centre), (1, 1.0));
            assert!(printed.contains("found 1 holes"), "{printed}");
        }
    }
}

/// FS-914 (kept in both modes): no vertex below the threshold prints `found -1 holes`.
#[test]
fn fs914_no_vertex_below_the_threshold() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, n, centre, printed) = fill(mode, |_| 1.0);
        assert_eq!((n, centre), (0, 1.0));
        assert!(printed.contains("found -1 holes"));
        assert!(r.noted("FS-914"));
        assert!(!r.noted("FS-915"));
    }
}

/// Two crossing triangles marked in both modes; with hole filling `Found 1 holes` for the one
/// region left (FS-914 noted, kept); a ripped vertex's old mark kept (FS-916).
#[test]
fn marking_the_intersections() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (mut v, mut f) = crossing(0.0);
        v.extend_from_slice(&[[5.0, 5.0, 0.0], [6.0, 5.0, 0.0], [5.0, 6.0, 0.0]]);
        f.push([6, 7, 8]);
        let mut s = r.surface(&v, &f);
        s.vertex_mut(8).ripflag = 1;
        s.vertex_mut(8).marked = 1;
        let mut io = io!(r);
        let n = props10::mris_mark_intersections(&mut r.g, &mut io, &mut s, 1).unwrap();
        assert_eq!(n, 2);
        let marked: Vec<i16> = (0..9).map(|k| s.vertex(k).marked).collect();
        assert_eq!(marked, vec![1, 1, 1, 1, 1, 1, 0, 0, 1]);
        assert!(String::from_utf8_lossy(&r.out).contains("  Found 1 holes"));
        assert!(r.noted("FS-914"));
        assert!(r.noted("FS-916"));
    }
}
