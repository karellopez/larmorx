// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The rest of `mrisurf_topology.cpp` (relay 42 of track D, `toprest.rs`): the corrections FS-986
//! to FS-995 and the paths the probe's random cases rarely meet, on small surfaces read from binary
//! triangle files. The corrected removals are checked against what they are meant to give: rings
//! found again through the unripped vertices only, face lists whose places name the vertex, and
//! the same Euler counts before and after a renumbering.

use std::collections::BTreeSet;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::{dist, nbhd, readwrite, toprest, valrest};

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
        b.extend_from_slice(b"created by me on Tue Nov 17 14:30:00 2026\n\n");
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
        self.g.notices.as_slice().iter().map(|n| n.defect.id).collect()
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
            let (a, b, c, d) = (j * w + i, j * w + i + 1, (j + 1) * w + i, (j + 1) * w + i + 1);
            f.push([a, b, d]);
            f.push([a, d, c]);
        }
    }
    (v, f)
}

fn octahedron() -> (Vec<[f32; 3]>, Vec<[i32; 3]>) {
    (
        vec![
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ],
        vec![
            [0, 2, 4],
            [2, 1, 4],
            [1, 3, 4],
            [3, 0, 4],
            [2, 0, 5],
            [1, 2, 5],
            [3, 1, 5],
            [0, 3, 5],
        ],
    )
}

fn list(s: &Mris, vno: usize, n: i16) -> Vec<i32> {
    let n = n.max(0) as usize;
    s.topology(vno).v.as_ref().map(|v| v[..n].to_vec()).unwrap_or_default()
}

/// The rings of `vno` up to `hops` through unripped vertices, from its faces (each face's other
/// vertices are its 1-hop neighbours).
fn rings(s: &Mris, vno: usize, hops: usize) -> Vec<BTreeSet<i32>> {
    let nbrs = |x: usize| -> BTreeSet<i32> {
        let mut r = BTreeSet::new();
        for fno in 0..s.nfaces as usize {
            let f = s.face(fno);
            if f.ripflag != 0 || !f.v.contains(&(x as i32)) {
                continue;
            }
            for &y in &f.v {
                if y != x as i32 && s.vertex(y as usize).ripflag == 0 {
                    r.insert(y);
                }
            }
        }
        r
    };
    let mut seen: BTreeSet<i32> = [vno as i32].into();
    let mut out = Vec::new();
    let mut front: BTreeSet<i32> = [vno as i32].into();
    for _ in 0..hops {
        let mut next = BTreeSet::new();
        for &x in &front {
            for y in nbrs(x as usize) {
                if seen.insert(y) {
                    next.insert(y);
                }
            }
        }
        out.push(next.clone());
        front = next;
    }
    out
}

#[test]
fn euler_counts_of_closed_and_open_surfaces() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let (v, f) = octahedron();
    let mut s = r.surface(&v, &f);
    let c = toprest::compute_euler_number(&s);
    assert_eq!((c.euler, c.nvertices, c.nfaces, c.nedges), (2, 6, 8, 12));
    assert_eq!(toprest::topological_defect_index(&s), 0);
    s.vertex_mut(4).ripflag = 1;
    valrest::set_rip_in_faces_with_ripped_vertices(&mut s);
    let c = toprest::compute_euler_number(&s);
    assert_eq!((c.euler, c.nvertices, c.nfaces, c.nedges), (1, 5, 4, 8));
    let (v, f) = grid(4, 3);
    let s = r.surface(&v, &f);
    let c = toprest::compute_euler_number(&s);
    assert_eq!((c.euler, c.nvertices, c.nfaces), (1, 12, 12));
    assert_eq!(toprest::count_total_neighbors_in_use(&s), 12 + 2 * c.nedges);
}

#[test]
fn fs986_the_removal_finds_the_rings_again_without_the_ripped_vertices() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = grid(5, 5);
        let mut s = r.surface(&v, &f);
        let mut io = io!(r);
        nbhd::set_neighborhood_size_and_dist(&mut r.g, &mut r.sg, &mut io, &mut s, 2).unwrap();
        // the distances before, by neighbour
        let before: Vec<Vec<(i32, f32)>> = (0..25)
            .map(|vno| {
                let vt = s.topology(vno);
                list(&s, vno, vt.vtotal)
                    .into_iter()
                    .zip(s.vertex(vno).dist.as_ref().unwrap().iter().copied())
                    .collect()
            })
            .collect();
        s.vertex_mut(12).ripflag = 1;
        valrest::set_rip_in_faces_with_ripped_vertices(&mut s);
        toprest::remove_ripped(&mut r.g, &mut r.sg, &mut io, &mut s).unwrap();
        assert!(r.ids().contains(&"FS-986"), "{mode:?}");
        let holds_ripped = (0..25).any(|vno| {
            s.vertex(vno).ripflag == 0 && list(&s, vno, s.topology(vno).vtotal).contains(&12)
        });
        assert_eq!(holds_ripped, mode.reproduce(), "{mode:?}");
        if mode.reproduce() {
            continue;
        }
        for vno in (0..25).filter(|&x| x != 12) {
            let vt = s.topology(vno);
            let want = rings(&s, vno, 2);
            let ring1: BTreeSet<i32> = list(&s, vno, vt.vnum).into_iter().collect();
            let ring12: BTreeSet<i32> = list(&s, vno, vt.v2num).into_iter().collect();
            assert_eq!(ring1, want[0], "vertex {vno}");
            assert_eq!(ring12, &want[0] | &want[1], "vertex {vno}");
            assert_eq!(vt.vtotal, vt.v2num, "vertex {vno}");
            let d = s.vertex(vno).dist.as_ref().unwrap();
            for (i, x) in list(&s, vno, vt.vtotal).into_iter().enumerate() {
                let old = before[vno].iter().find(|(y, _)| *y == x).expect("a neighbour before");
                assert_eq!(d[i].to_bits(), old.1.to_bits(), "vertex {vno}'s distance to {x}");
            }
            // every face kept in its list holds it at its place
            for k in 0..usize::from(vt.num) {
                let fno = vt.f.as_ref().unwrap()[k] as usize;
                assert_eq!(s.face(fno).ripflag, 0);
                let n = usize::from(vt.n.as_ref().unwrap()[k]);
                assert_eq!(s.face(fno).v[n], vno as i32);
            }
        }
    }
}

/// Every unripped face's vertices hold it in their face lists at the place they have in it, and
/// every neighbour list is symmetric over the 1-hop rings.
fn consistent(s: &Mris) -> bool {
    for fno in 0..s.nfaces as usize {
        let f = s.face(fno);
        for (n, &x) in f.v.iter().enumerate() {
            if x < 0 || x >= s.nvertices {
                return false;
            }
            let vt = s.topology(x as usize);
            let ok = (0..usize::from(vt.num)).any(|k| {
                vt.f.as_ref().unwrap()[k] == fno as i32 && usize::from(vt.n.as_ref().unwrap()[k]) == n
            });
            if !ok {
                return false;
            }
        }
    }
    (0..s.nvertices as usize).all(|a| {
        list(s, a, s.topology(a).vnum)
            .iter()
            .all(|&b| b >= 0 && b < s.nvertices && list(s, b as usize, s.topology(b as usize).vnum).contains(&(a as i32)))
    })
}

#[test]
fn fs987_fs988_the_renumbering_moves_the_lists_with_the_vertices() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = grid(4, 4);
        let mut s = r.surface(&v, &f);
        s.vertex_mut(5).ripflag = 1;
        valrest::set_rip_in_faces_with_ripped_vertices(&mut s);
        let c0 = toprest::compute_euler_number(&s);
        // MRISread's surfaces keep their distances; the renumbering takes none
        dist::free_dists_but_not_orig(&mut s);
        dist::free_dist_origs(&mut s);
        let mut io = io!(r);
        toprest::renumber_removing_ripped_faces_and_vertices(&mut r.g, &mut r.sg, &mut io, &mut s)
            .unwrap();
        let ids = r.ids();
        assert!(ids.contains(&"FS-987") && ids.contains(&"FS-988"), "{mode:?} {ids:?}");
        assert_eq!((s.nvertices, s.nfaces), (15, 18 - 6));
        assert_eq!(consistent(&s), !mode.reproduce(), "{mode:?}");
        if !mode.reproduce() {
            let c1 = toprest::compute_euler_number(&s);
            assert_eq!(c0, c1);
            // vertex 5's old neighbours lost it; the vertex now numbered 5 is the old 6
            assert_eq!(s.vertex(5).x, 2.0);
        }
    }
}

#[test]
fn fs995_kept_faces_with_ripped_vertices() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = grid(3, 3);
        let mut s = r.surface(&v, &f);
        s.vertex_mut(4).ripflag = 1;
        // MRISread's surfaces keep their distances; the renumbering takes none
        dist::free_dists_but_not_orig(&mut s);
        dist::free_dist_origs(&mut s);
        let mut io = io!(r);
        toprest::renumber_removing_ripped_faces_and_vertices(&mut r.g, &mut r.sg, &mut io, &mut s)
            .unwrap();
        assert!(r.ids().contains(&"FS-995"), "{mode:?}");
        let minus = (0..s.nfaces as usize).any(|fno| s.face(fno).v.contains(&-1));
        assert_eq!(minus, mode.reproduce(), "{mode:?}");
        if !mode.reproduce() {
            assert_eq!((s.nvertices, s.nfaces), (8, 2));
            assert!(consistent(&s));
        }
    }
}

#[test]
fn fs992_a_link_beyond_the_neighbours_in_use() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = octahedron();
        let mut s = r.surface(&v, &f);
        let mut io = io!(r);
        nbhd::set_nsize_cur(&mut r.g, &mut io, &mut s, 0, 0).unwrap();
        let b = list(&s, 0, s.topology(0).vnum)[2];
        let res = toprest::remove_vertex_link(&mut r.g, &mut r.sg, io.err, &mut s, 0, b);
        // FreeSurfer's copy runs over the memory around the list: both modes move the ring only
        assert!(r.ids().contains(&"FS-992"));
        assert_eq!(res, Ok(0));
        assert_eq!(s.topology(0).vnum, 3);
        assert!(!list(&s, 0, 3).contains(&b));
        assert_eq!(s.topology(0).vtotal, 0);
    }
}

#[test]
fn fs991_fs993_a_vertex_without_neighbours() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (mut v, f) = octahedron();
        v.push([3.0, 3.0, 3.0]);
        let mut s = r.surface(&v, &f);
        let mut io = io!(r);
        let res = toprest::is_surface_valid(&mut r.g, &mut io, &mut s, 0, 1);
        assert!(r.ids().contains(&"FS-991"));
        let missing = s.topology(6).v.is_none();
        if mode.reproduce() && missing {
            assert_eq!(res, Err(Fatal::Exit(139)));
        } else {
            assert_eq!(res, Ok(1));
            assert!(String::from_utf8_lossy(&r.err).contains("# of border vertices [ #v ~ #f ]     0"));
        }
        let l = toprest::get_spat_ordered_neighbors(&mut r.g, &s, 6);
        assert!(l.is_empty() && r.ids().contains(&"FS-993"));
    }
}

#[test]
fn the_surface_check_counts_borders_and_the_ordered_neighbours_go_around() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let (v, f) = octahedron();
    let mut s = r.surface(&v, &f);
    let mut io = io!(r);
    assert_eq!(toprest::is_surface_valid(&mut r.g, &mut io, &mut s, 0, 2), Ok(1));
    let l = toprest::get_spat_ordered_neighbors(&mut r.g, &s, 4);
    assert_eq!(l.len(), 4);
    for k in 0..4 {
        assert_eq!(toprest::edge_exists(&s, l[k] as usize, l[(k + 1) % 4]), 1);
    }
    let (v, f) = grid(3, 3);
    let mut s = r.surface(&v, &f);
    r.err.clear();
    let mut io = io!(r);
    assert_eq!(toprest::is_surface_valid(&mut r.g, &mut io, &mut s, 0, 1), Ok(0));
    assert!(String::from_utf8_lossy(&r.err).contains("# of edges with single face          16"));
    // a patch: its single-face edges still answer 0 (the second test runs after the first)
    let mut io = io!(r);
    assert_eq!(toprest::is_surface_valid(&mut r.g, &mut io, &mut s, 1, 0), Ok(0));
}

#[test]
fn triangle_links_of_a_quadrangle_surface_once() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let (v, f) = grid(3, 3);
    let mut s = r.surface(&v, &f);
    s.r#type = 0;
    let before: i32 = (0..9).map(|x| i32::from(s.topology(x).vnum)).sum();
    let mut io = io!(r);
    assert_eq!(toprest::remove_triangle_links(&mut r.g, &mut r.sg, &mut io, &mut s), Ok(0));
    let after: i32 = (0..9).map(|x| i32::from(s.topology(x).vnum)).sum();
    assert_eq!((s.triangle_links_removed, before - after), (1, 8));
    assert_eq!(toprest::remove_triangle_links(&mut r.g, &mut r.sg, &mut io, &mut s), Ok(0));
    let again: i32 = (0..9).map(|x| i32::from(s.topology(x).vnum)).sum();
    assert_eq!(again, after);
    for x in 0..9 {
        assert_eq!(s.topology(x).vtotal, s.topology(x).vnum);
    }
}

#[test]
fn fs990_removing_a_link_is_the_same_in_both_modes() {
    let run = |mode| {
        let mut r = Run::new(mode);
        let (v, f) = octahedron();
        let mut s = r.surface(&v, &f);
        toprest::remove_link(&mut r.g, &mut r.sg, &mut r.err, &mut s, 0, 4);
        let state: Vec<(i8, Vec<i32>)> = (0..6)
            .map(|x| (s.vertex(x).ripflag, list(&s, x, s.topology(x).vnum)))
            .collect();
        let faces: Vec<i8> = (0..8).map(|k| s.face(k).ripflag).collect();
        // a face removed: its links go though the faces beside it hold them the other way
        toprest::remove_face(&mut r.g, &mut r.sg, &mut r.err, &mut s, 5);
        let gone = [(1, 2), (2, 5), (5, 1)].map(|(a, b)| toprest::edge_exists(&s, a, b));
        (state, faces, gone, r.ids().contains(&"FS-990"))
    };
    let a = run(UpstreamDefects::Fix);
    let b = run(UpstreamDefects::Reproduce);
    assert_eq!(a, b);
    assert!(a.3);
    assert_eq!(a.1, vec![1, 0, 0, 1, 0, 0, 0, 0]);
    assert_eq!(a.2, [0, 0, 0]);
}
