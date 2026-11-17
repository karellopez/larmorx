// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.deform's first pieces (relay 44 of track D, `project.rs`, `intersect.rs`) and fs.surf's
//! `MRIScomputeDistanceToSurface`: what they are meant to give on small surfaces read from binary
//! triangle files (a tilted grid flattened into z = 0 with its edges' lengths kept, a pial
//! projection back onto the saved positions, an intersection-free surface left alone, the signed
//! distances of an octahedron), the `MRIS_MP` projection's assertion and `canDo`'s one message per
//! status, and FS-1001 and FS-1004 in both modes (the probe's `dfm` command compares the rest with
//! FreeSurfer's own code).

#![allow(clippy::needless_range_loop)]

use std::sync::Arc;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::diag::DIAG_VERBOSE;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_deform::{intersect, project};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_surf::aaa::{MrisStatus, Opaque, SurfGlobals};
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::views::PIAL_VERTICES;
use larmorx_freesurfer_surf::{positions, readwrite, surfmri};

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
        b.extend_from_slice(b"created by me on Tue Nov 17 21:00:00 2026\n\n");
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

    fn ids(&self) -> Vec<&'static str> {
        self.g
            .notices
            .as_slice()
            .iter()
            .map(|n| n.defect.id)
            .collect()
    }
}

/// An open `w` by `h` grid of unit squares, each split into two triangles, on the plane `z = x`.
fn tilted_grid(w: i32, h: i32) -> (Vec<[f32; 3]>, Vec<[i32; 3]>) {
    let mut v = Vec::new();
    for j in 0..h {
        for i in 0..w {
            v.push([i as f32, j as f32, i as f32]);
        }
    }
    let mut f = Vec::new();
    for j in 0..h - 1 {
        for i in 0..w - 1 {
            let a = j * w + i;
            f.push([a, a + 1, a + w]);
            f.push([a + 1, a + w + 1, a + w]);
        }
    }
    (v, f)
}

/// An octahedron of radius `r` (its faces wound outwards).
fn octahedron(r: f32) -> (Vec<[f32; 3]>, Vec<[i32; 3]>) {
    let v = vec![
        [r, 0.0, 0.0],
        [-r, 0.0, 0.0],
        [0.0, r, 0.0],
        [0.0, -r, 0.0],
        [0.0, 0.0, r],
        [0.0, 0.0, -r],
    ];
    let f = vec![
        [0, 2, 4],
        [2, 1, 4],
        [1, 3, 4],
        [3, 0, 4],
        [2, 0, 5],
        [1, 2, 5],
        [3, 1, 5],
        [0, 3, 5],
    ];
    (v, f)
}

fn edge_lengths(s: &Mris, f: &[[i32; 3]]) -> Vec<f32> {
    let mut l = Vec::new();
    for q in f {
        for k in 0..3 {
            let (a, b) = (s.vertex(q[k] as usize), s.vertex(q[(k + 1) % 3] as usize));
            l.push(((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt());
        }
    }
    l
}

#[test]
fn a_tilted_grid_is_flattened_into_its_plane() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = tilted_grid(4, 3);
        let mut s = r.surface(&v, &f);
        let before = edge_lengths(&s, &f);
        let mut io = io!(r);
        let ret = project::flatten_patch(&mut r.g, &mut r.sg, &mut io, &mut s).unwrap();
        assert_eq!(ret, 0);
        assert_eq!(s.status, MrisStatus::PLANE);
        for k in 0..v.len() {
            assert_eq!(s.vertex(k).z, 0.0);
        }
        // the rotation keeps the edges (the tilt is undone, not projected away)
        for (a, b) in before.iter().zip(edge_lengths(&s, &f)) {
            assert!((a - b).abs() < 1e-4, "{a} {b}");
        }
        // centred on the mean position
        let mx: f32 = (0..v.len()).map(|k| s.vertex(k).x).sum::<f32>() / v.len() as f32;
        assert!(mx.abs() < 1e-4, "{mx}");
        assert!(r.ids().is_empty(), "{:?}", r.ids());
        assert!(r.out.is_empty());
    }
}

#[test]
fn a_plane_only_has_its_unripped_heights_cleared() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let (v, f) = tilted_grid(3, 3);
    let mut s = r.surface(&v, &f);
    s.status = MrisStatus::PLANE;
    s.vertex_mut(2).ripflag = 1;
    let mut io = io!(r);
    project::project_surface(&mut r.g, &mut r.sg, &mut io, &mut s).unwrap();
    for k in 0..v.len() {
        let p = s.vertex(k);
        assert_eq!(p.x, v[k][0]);
        assert_eq!(p.z, if k == 2 { v[k][2] } else { 0.0 });
    }
}

#[test]
fn fs1004_a_patch_whose_normals_cancel() {
    // the octahedron's vertex normals sum to exactly zero
    let (v, f) = octahedron(5.0);
    let mut fix = Run::new(UpstreamDefects::Fix);
    let mut s = fix.surface(&v, &f);
    let mut io = io!(fix);
    project::flatten_patch(&mut fix.g, &mut fix.sg, &mut io, &mut s).unwrap();
    // the flattened octahedron's two poles meet at the centre: their faces have no normal (FS-121)
    assert_eq!(fix.ids(), ["FS-1004", "FS-121"]);
    // not centred or rotated: each vertex where it was in x and y (but for FS-121's small moves of
    // the vertices without a normal), at z = 0
    for k in 0..v.len() {
        let p = s.vertex(k);
        assert!((p.x - v[k][0]).abs() < 0.01 && (p.y - v[k][1]).abs() < 0.01 && p.z == 0.0);
    }
    let mut rep = Run::new(UpstreamDefects::Reproduce);
    let mut s = rep.surface(&v, &f);
    let mut io = io!(rep);
    project::flatten_patch(&mut rep.g, &mut rep.sg, &mut io, &mut s).unwrap();
    // FreeSurfer's NaN positions then have no normal (FS-121) and NaN angles (FS-06)
    assert_eq!(rep.ids(), ["FS-1004", "FS-121", "FS-06"]);
    for k in 0..v.len() {
        let p = s.vertex(k);
        assert!(p.x.is_nan() && p.y.is_nan() && p.z == 0.0);
    }
}

#[test]
fn fs1001_the_pial_projection_without_a_table() {
    for (mode, status) in [(UpstreamDefects::Fix, 1), (UpstreamDefects::Reproduce, 139)] {
        let mut r = Run::new(mode);
        let (v, f) = octahedron(5.0);
        let mut s = r.surface(&v, &f);
        s.status = MrisStatus::PIAL_SURFACE;
        let mut io = io!(r);
        let e = project::project_surface(&mut r.g, &mut r.sg, &mut io, &mut s).unwrap_err();
        // the message's ending as ErrorExit writes it (FS-01) by default
        assert_eq!(r.ids()[0], "FS-1001");
        if status == 139 {
            assert_eq!(e, Fatal::Exit(139));
        } else {
            assert_ne!(e, Fatal::Exit(139));
            let err = String::from_utf8_lossy(&r.err);
            assert!(err.contains("has no face table"), "{err}");
        }
    }
}

#[test]
fn the_pial_projection_returns_moved_vertices_onto_the_pial_faces() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let (v, f) = tilted_grid(4, 4);
    let mut s = r.surface(&v, &f);
    let mut io = io!(r);
    positions::save_vertex_positions(&mut r.g, &mut io, &mut s, PIAL_VERTICES).unwrap();
    // each vertex lifted a little off the plane z = x (along its normal, roughly)
    for k in 0..v.len() {
        let p = s.vertex_mut(k);
        p.x -= 0.1;
        p.z += 0.1;
    }
    let m = Mht::create_face_table_resolution(&mut r.g, io.err, &s, PIAL_VERTICES, 2.0).unwrap();
    s.mht = Opaque(Some(Arc::new(m)));
    s.status = MrisStatus::PIAL_SURFACE;
    project::project_surface(&mut r.g, &mut r.sg, &mut io, &mut s).unwrap();
    for k in 0..v.len() {
        let p = s.vertex(k);
        assert!((p.z - p.x).abs() < 1e-5, "{k}: {} {}", p.x, p.z);
    }
    assert!(r.ids().is_empty(), "{:?}", r.ids());
}

#[test]
fn the_mp_projection_never_returns_and_can_do_speaks_once() {
    for (mode, status) in [(UpstreamDefects::Fix, 1), (UpstreamDefects::Reproduce, 139)] {
        for (st, line) in [(MrisStatus::SPHERE, 158), (MrisStatus::PLANE, 328)] {
            let mut r = Run::new(mode);
            let mut io = io!(r);
            let e = project::project_surface_mp(&mut r.g, &mut io, st);
            assert_eq!(e, Fatal::Exit(status));
            let out = String::from_utf8_lossy(&r.out);
            assert!(out.contains(&format!(":{line} false")), "{out}");
        }
    }
    let mut r = Run::new(UpstreamDefects::Fix);
    r.g.gdiag = DIAG_VERBOSE;
    let mut shown = 0;
    assert!(project::project_surface_can_do(
        &r.g,
        &mut r.err,
        &mut shown,
        MrisStatus::SPHERE
    ));
    for _ in 0..2 {
        assert!(!project::project_surface_can_do(
            &r.g,
            &mut r.err,
            &mut shown,
            MrisStatus::PLANE
        ));
    }
    assert_eq!(
        String::from_utf8_lossy(&r.err),
        "debug: need to process surface status 2\n"
    );
    assert_eq!(shown, 1 << 2);
}

#[test]
fn a_surface_without_intersections_is_left_alone() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let (v, f) = octahedron(5.0);
    let mut s = r.surface(&v, &f);
    let mut io = io!(r);
    assert_eq!(
        intersect::remove_intersections(&mut r.g, &mut io, &mut s, 0).unwrap(),
        0
    );
    assert!(r.out.is_empty());
    for k in 0..v.len() {
        assert_eq!(s.vertex(k).x, v[k][0]);
    }
}

#[test]
fn the_signed_distance_of_an_octahedron() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let (v, f) = octahedron(6.0);
    let mut s = r.surface(&v, &f);
    let mut io = io!(r);
    let d = surfmri::compute_distance_to_surface(&mut r.g, &mut r.sg, &mut io, &mut s, None, 1.0)
        .unwrap()
        .into_new()
        .expect("a new volume");
    let (w, h, dp) = (d.vg.width, d.vg.height, d.vg.depth);
    let at =
        |g: &mut Globals, x, y, z| mri_get_vox_val(g, &mut Vec::new(), &d, x, y, z, 0, 0).unwrap();
    // the centre is inside (negative), a corner of the padded box outside (positive)
    assert!(at(&mut r.g, w / 2, h / 2, dp / 2) < 0.0);
    assert!(at(&mut r.g, 0, 0, 0) > 0.0);
}
