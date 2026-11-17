// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The soap bubbles of `mrisurf_deform.cpp` (relay 42 of track D, `soap.rs`): FS-996 in both modes
//! and what the bubbles are meant to give, on small grids read from binary triangle files: free
//! vertices relaxed to the mean of their neighbours with the fixed ones unmoved, the marks restored,
//! the original and target bubbles' early end.

#![allow(clippy::needless_range_loop)]

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_surf::soap::{self, Positions};

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
        b.extend_from_slice(b"created by me on Tue Nov 17 14:45:00 2026\n\n");
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

/// An open `w` by `h` grid of squares, each split into two triangles.
fn grid(w: i32, h: i32) -> (Vec<[f32; 3]>, Vec<[i32; 3]>) {
    let mut v = Vec::new();
    for j in 0..h {
        for i in 0..w {
            v.push([i as f32, j as f32, ((i * 3 + j) % 4) as f32 / 8.0]);
        }
    }
    let mut f = Vec::new();
    for j in 0..h - 1 {
        for i in 0..w - 1 {
            let (a, b, c, d) = (
                j * w + i,
                j * w + i + 1,
                (j + 1) * w + i,
                (j + 1) * w + i + 1,
            );
            f.push([a, b, d]);
            f.push([a, d, c]);
        }
    }
    (v, f)
}

/// A 5 by 5 grid whose interior (9 vertices) is free and lifted by 1, its border fixed (mark 1).
fn lifted(r: &mut Run) -> Mris {
    let (v, f) = grid(5, 5);
    let mut s = r.surface(&v, &f);
    for vno in 0..25 {
        let (i, j) = (vno % 5, vno / 5);
        let inner = (1..4).contains(&i) && (1..4).contains(&j);
        let x = s.vertex_mut(vno);
        x.marked = if inner { 0 } else { 1 };
        if inner {
            x.z += 1.0;
        }
    }
    s
}

#[test]
fn the_free_vertices_relax_and_the_fixed_stay() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = lifted(&mut r);
        let before: Vec<(f32, f32, f32)> = (0..25)
            .map(|v| (s.vertex(v).x, s.vertex(v).y, s.vertex(v).z))
            .collect();
        let mut io = io!(r);
        assert_eq!(
            soap::soap_bubble_vertex_positions(&mut r.g, &mut io, &mut s, 500),
            0
        );
        for vno in 0..25 {
            let x = s.vertex(vno);
            let (i, j) = (vno % 5, vno / 5);
            if !((1..4).contains(&i) && (1..4).contains(&j)) {
                assert_eq!((x.x, x.y, x.z), before[vno], "fixed vertex {vno}");
                assert_eq!(x.marked, 1);
                continue;
            }
            // the marks restored; every free vertex the mean of itself and its neighbours
            assert_eq!(x.marked, 0);
            let ring =
                s.topology(vno).v.as_ref().unwrap()[..s.topology(vno).vnum as usize].to_vec();
            let n = ring.len() as f32 + 1.0;
            let mz = (x.z + ring.iter().map(|&w| s.vertex(w as usize).z).sum::<f32>()) / n;
            assert!(
                (x.z - mz).abs() < 1e-4,
                "vertex {vno}: {} against {mz}",
                x.z
            );
        }
        // the weighted bubble: a ripped neighbour weighs 20
        let mut s = lifted(&mut r);
        s.vertex_mut(0).ripflag = 1;
        let mut io = io!(r);
        assert_eq!(
            soap::weighted_soap_bubble_vertex_positions(&mut r.g, &mut io, &mut s, 1),
            Ok(0)
        );
        let (a, b) = (s.vertex(6), s.vertex(12));
        assert_eq!(a.marked, 2);
        assert!(
            a.z < b.z,
            "the vertex next to the ripped one is pulled down more"
        );
    }
}

#[test]
fn fs996_a_diagnostic_vertex_beyond_the_surface() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = lifted(&mut r);
        r.g.gdiag_no = 25;
        let mut io = io!(r);
        soap::soap_bubble_vertex_positions(&mut r.g, &mut io, &mut s, 2);
        assert!(r.ids().contains(&"FS-996"));
        let mut s = lifted(&mut r);
        r.g.gdiag_no = 12;
        r.out.clear();
        let mut io = io!(r);
        soap::soap_bubble_vertex_positions(&mut r.g, &mut io, &mut s, 1);
        let text = String::from_utf8_lossy(&r.out).into_owned();
        assert!(text.starts_with("v 12 @ (2.0, 2.0, "), "{text}");
        assert!(text.contains("iter 0: moving v 12 from"), "{text}");
    }
}

#[test]
fn the_original_and_target_bubbles_end_after_six_idle_passes() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = lifted(&mut r);
    for vno in 0..25 {
        let x = s.vertex_mut(vno);
        (x.targx, x.targy, x.targz) = (x.x, x.y, x.z + 3.0);
    }
    r.g.gdiag = larmorx_freesurfer_core::diag::DIAG_SHOW;
    let mut io = io!(r);
    assert_eq!(
        soap::soap_bubble_positions(&mut r.g, &mut io, &mut s, 100, Positions::Target),
        Ok(0)
    );
    let text = String::from_utf8_lossy(&r.out).into_owned();
    // the free vertices are reached from the fixed ring in two passes; then six passes mark none
    assert_eq!(text.lines().count(), 8, "{text}");
    assert!(text.starts_with("0: 8 vertices marked\n1: 1 vertices marked\n2: 0 vertices marked\n"));
    for vno in 0..25 {
        let x = s.vertex(vno);
        let (i, j) = (vno % 5, vno / 5);
        if (1..4).contains(&i) && (1..4).contains(&j) {
            // a free vertex: unmarked again, its target the mean of the border's
            assert_eq!(x.marked, 0);
            assert!(
                x.targz.is_finite() && x.targz < 3.5 && x.targz > 2.5,
                "{vno}: {}",
                x.targz
            );
        } else {
            assert_eq!((x.marked, x.targz), (1, x.z + 3.0));
        }
    }
}
