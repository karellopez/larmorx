// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The patch readers (`patchread.rs`), the sampled energies (`sampled.rs`) and the face-table walks
//! (`overlap.rs`) of relay 45 of track D on an octahedron read from a binary triangle file: a patch
//! written and read back keeps its vertices, positions and borders and rips the others; the ASCII
//! reader's defects FS-1007, FS-1008 and FS-1010 are errors by default and FreeSurfer's with
//! `reproduce`; a GIFTI patch (FS-1006) is refused; sampling the pial positions through the
//! canonical sphere at a face's centroid gives the pial corners' mean; the minimization energy is
//! the squared distance from the white position; FS-1011 and FS-1012 end the program by default;
//! the ray along a vertex's normal meets the far face; each face's overlap list starts with it,
//! lists each face once and clears its marks.

use larmorx_core::defects::{
    FS_1006, FS_1007, FS_1008, FS_1010, FS_1011, FS_1012, UpstreamDefects,
};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_BADFILE, ERROR_UNSUPPORTED, Fatal, NO_ERROR};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_surf::aaa::{MrisStatus, SurfGlobals};
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::views::{CANONICAL_VERTICES, PIAL_VERTICES, WHITE_VERTICES};
use larmorx_freesurfer_surf::{overlap, patchread, positions, readwrite, sampled, valfile};

struct Run {
    g: Globals,
    sg: SurfGlobals,
    env: Environment,
    out: Vec<u8>,
    err: Vec<u8>,
    dir: tempfile::TempDir,
}

impl Run {
    fn new(mode: UpstreamDefects) -> Run {
        let dir = tempfile::tempdir().expect("a folder");
        std::fs::create_dir(dir.path().join("d")).expect("d");
        Run {
            g: Globals::new(mode),
            sg: SurfGlobals::default(),
            env: Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]),
            out: Vec::new(),
            err: Vec::new(),
            dir,
        }
    }

    /// An octahedron of radius `r` (turned by 30 degrees about z), written as `d/lh.s0` and read.
    fn octahedron(&mut self, r: f32) -> Mris {
        let v: Vec<[f32; 3]> = [
            [r, 0.0, 0.0],
            [-r, 0.0, 0.0],
            [0.0, r, 0.0],
            [0.0, -r, 0.0],
            [0.0, 0.0, r],
            [0.0, 0.0, -r],
        ]
        .iter()
        .map(|p| {
            let (c, s) = (0.866_025_4f32, 0.5f32);
            [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]]
        })
        .collect();
        let f: [[i32; 3]; 8] = [
            [0, 2, 4],
            [2, 1, 4],
            [1, 3, 4],
            [3, 0, 4],
            [2, 0, 5],
            [1, 2, 5],
            [3, 1, 5],
            [0, 3, 5],
        ];
        let mut b = vec![0xff, 0xff, 0xfe];
        b.extend_from_slice(b"created by me on Wed Nov 18 01:00:00 2026\n\n");
        b.extend_from_slice(&(v.len() as i32).to_be_bytes());
        b.extend_from_slice(&(f.len() as i32).to_be_bytes());
        for p in &v {
            for c in p {
                b.extend_from_slice(&c.to_be_bytes());
            }
        }
        for q in &f {
            for x in q {
                b.extend_from_slice(&x.to_be_bytes());
            }
        }
        std::fs::write(self.dir.path().join("d/lh.s0"), b).expect("the file");
        let mut io = Io {
            out: &mut self.out,
            err: &mut self.err,
            env: &self.env,
        };
        readwrite::read(
            &mut self.g,
            &mut self.sg,
            &mut io,
            Some(self.dir.path()),
            "d/lh.s0",
            true,
        )
        .unwrap()
        .expect("a surface")
    }

    fn put(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.dir.path().join(name), bytes).expect("a file");
    }

    fn read_patch(&mut self, s: &mut Mris, name: &str) -> Result<i32, Fatal> {
        let mut io = Io {
            out: &mut self.out,
            err: &mut self.err,
            env: &self.env,
        };
        patchread::read_patch(
            &mut self.g,
            &mut self.sg,
            &mut io,
            Some(self.dir.path()),
            s,
            name,
            true,
        )
    }
}

/// A patch written by `MRISwritePatch` (vertices 0 to 4 kept, 2 a border vertex) and read back:
/// the kept vertices unripped with their positions and border flags, the bottom vertex ripped, a
/// cut patch. (A patch without both poles would keep no face of the octahedron: every vertex would
/// then be ripped for having none.)
#[test]
fn a_written_patch_reads_back() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.octahedron(10.0);
    s.vertex_mut(5).ripflag = 1;
    s.vertex_mut(2).border = 1;
    s.vertex_mut(1).x = 3.25;
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    let d = Some(r.dir.path());
    assert_eq!(
        valfile::write_patch(&mut r.g, &mut io, d, &mut s, "d/lh.patch"),
        NO_ERROR
    );
    let mut t = r.octahedron(10.0);
    assert_eq!(r.read_patch(&mut t, "patch").unwrap(), NO_ERROR);
    assert_eq!(t.patch, 1);
    assert_eq!(t.status, MrisStatus::CUT);
    for v in 0..5 {
        assert_eq!(t.vertex(v).ripflag, 0, "{v}");
    }
    // the border vertex read as one (the metric properties of the cut patch mark more)
    assert_eq!(t.vertex(2).border, 1);
    assert_eq!(t.vertex(1).x, 3.25);
    assert_eq!(t.vertex(5).ripflag, 1);
}

/// The ASCII reader: a file of comments only (FS-1007), a point line without a number (FS-1008),
/// a first line without a count (FS-1010): errors by default; with `reproduce` the crash, the
/// previous point's number, memory read as 0.
#[test]
fn ascii_patch_defects() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.octahedron(10.0);
    r.put("d/lh.a.asc", b"# a comment\n\n");
    assert_eq!(r.read_patch(&mut s, "a.asc").unwrap(), ERROR_BADFILE);
    assert!(r.g.notices.contains(&FS_1007));
    r.put(
        "d/lh.b.asc",
        b"2 0\n1  vno=0\n1.0 2.0 3.0\nvno=missing\n4.0 5.0 6.0\n",
    );
    let mut s = r.octahedron(10.0);
    assert_eq!(r.read_patch(&mut s, "b.asc").unwrap(), ERROR_BADFILE);
    assert!(r.g.notices.contains(&FS_1008));
    r.put("d/lh.c.asc", b"count\n");
    let mut s = r.octahedron(10.0);
    assert_eq!(r.read_patch(&mut s, "c.asc").unwrap(), ERROR_BADFILE);
    assert!(r.g.notices.contains(&FS_1010));

    let mut r = Run::new(UpstreamDefects::Reproduce);
    let mut s = r.octahedron(10.0);
    r.put("d/lh.a.asc", b"# a comment\n\n");
    assert_eq!(r.read_patch(&mut s, "a.asc"), Err(Fatal::Exit(139)));
    r.put(
        "d/lh.b.asc",
        b"2 0\n1  vno=0\n1.0 2.0 3.0\nvno=missing\n4.0 5.0 6.0\n",
    );
    let mut s = r.octahedron(10.0);
    assert_eq!(r.read_patch(&mut s, "b.asc").unwrap(), NO_ERROR);
    // the second point is vertex 0 again: its position the second line's
    assert_eq!(
        (s.vertex(0).x, s.vertex(0).y, s.vertex(0).z),
        (4.0, 5.0, 6.0)
    );
    assert_eq!(s.vertex(1).ripflag, 1);
}

/// A GIFTI name: refused by default (FS-1006, `ERROR_UNSUPPORTED`), the surface left as it was.
#[test]
fn a_gifti_patch_is_refused() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.octahedron(10.0);
    r.put("d/lh.p.gii", b"<not a gifti file>");
    assert_eq!(r.read_patch(&mut s, "p.gii").unwrap(), ERROR_UNSUPPORTED);
    assert!(r.g.notices.contains(&FS_1006));
    assert_eq!(s.patch, 0);
    assert!((0..6).all(|v| s.vertex(v).ripflag == 0));
}

/// An octahedron of radius 10 with its canonical positions, pial positions 2 mm further out and
/// white the canonical ones; a face table of the canonical positions.
fn sphere_set_up(r: &mut Run) -> (Mris, Mht) {
    let mut s = r.octahedron(10.0);
    s.radius = 10.0;
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    positions::save_vertex_positions(&mut r.g, &mut io, &mut s, CANONICAL_VERTICES).unwrap();
    positions::save_vertex_positions(&mut r.g, &mut io, &mut s, WHITE_VERTICES).unwrap();
    for v in 0..6 {
        let p = s.vertex_mut(v);
        (p.x, p.y, p.z) = (p.x * 1.2, p.y * 1.2, p.z * 1.2);
    }
    positions::save_vertex_positions(&mut r.g, &mut io, &mut s, PIAL_VERTICES).unwrap();
    let m =
        Mht::create_face_table_resolution(&mut r.g, io.err, &s, CANONICAL_VERTICES, 2.0).unwrap();
    (s, m)
}

/// The pial position sampled at the canonical centroid of face 0 (corners 0, 2, 4) is 1.2 times
/// that centroid (a third of each pial corner), and the minimization energy of vertex 4 there is
/// its squared distance from vertex 4's white position.
#[test]
fn sampling_through_the_sphere() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let (s, m) = sphere_set_up(&mut r);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    let mut c = [0f64; 3];
    for v in [0, 2, 4] {
        let q = s.vertex(v);
        for (k, x) in [q.cx, q.cy, q.cz].into_iter().enumerate() {
            c[k] += f64::from(x) / 3.0;
        }
    }
    let (cx, cy, cz) = (c[0] as f32, c[1] as f32, c[2] as f32);
    let (_, p) =
        sampled::sample_face_coords_canonical(&mut r.g, &mut io, &m, &s, cx, cy, cz, PIAL_VERTICES)
            .unwrap();
    for k in 0..3 {
        assert!((f64::from(p[k]) - 1.2 * c[k]).abs() < 1e-3, "{p:?} {c:?}");
    }
    let e = sampled::sample_minimization_energy(&mut r.g, &mut io, &m, &s, 4, cx, cy, cz).unwrap();
    let want = (1.2 * c[0]).powi(2) + (1.2 * c[1]).powi(2) + (1.2 * c[2] - 10.0).powi(2);
    assert!((f64::from(e) - want).abs() < 1e-2, "{e} {want}");
}

/// FS-1011: a table without faces (every face ripped before it is made) gives no face: an error by
/// default; with `reproduce` a point that is not finite crashes (status 139). FS-1012: a vertex's
/// face -1 sampled by the parallel energy ends the program by default.
#[test]
fn no_face_ends_the_program() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let (mut s, m) = sphere_set_up(&mut r);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    s.vertex_mut(0).fno = -1;
    let e = sampled::sample_parallel_energy_at_vertex(&mut r.g, &mut io, &m, &s, 0);
    assert!(e.is_err());
    assert!(r.g.notices.contains(&FS_1012));
    for f in 0..8 {
        s.face_mut(f).ripflag = 1;
    }
    let empty =
        Mht::create_face_table_resolution(&mut r.g, io.err, &s, CANONICAL_VERTICES, 2.0).unwrap();
    let e = sampled::sample_face_coords_canonical(
        &mut r.g,
        &mut io,
        &empty,
        &s,
        0.0,
        0.0,
        10.0,
        PIAL_VERTICES,
    );
    assert!(e.is_err());
    assert!(r.g.notices.contains(&FS_1011));

    let mut r = Run::new(UpstreamDefects::Reproduce);
    let (s, m) = sphere_set_up(&mut r);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    let e = sampled::sample_face_coords_canonical(
        &mut r.g,
        &mut io,
        &m,
        &s,
        f32::NAN,
        0.0,
        0.0,
        PIAL_VERTICES,
    );
    assert_eq!(e, Err(Fatal::Exit(139)));
    assert!(r.g.notices.contains(&FS_1011));
}

/// The ray from just above the top vertex downwards meets the bottom vertex's faces 20 mm away (at
/// the bucket of a step of 20); the octahedron's faces (their edges compared on the canonical
/// positions, the current ones here) listed once each after it, the marks cleared.
#[test]
fn rays_and_overlaps() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.octahedron(10.0);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    positions::save_vertex_positions(&mut r.g, &mut io, &mut s, CANONICAL_VERTICES).unwrap();
    let m = Mht::create_face_table_resolution(&mut r.g, io.err, &s, 4, 1.0).unwrap();
    let top = s.vertex(4);
    let p = [top.x, top.y, top.z + 0.001];
    let mut dist = 20.0f64;
    let found = overlap::direction_triangle_intersection(
        &mut r.g,
        &mut io,
        &s,
        p,
        [0.0, 0.0, -1.0],
        &m,
        &mut dist,
        4,
    )
    .unwrap();
    assert_eq!(found, 1);
    assert!((dist - 20.001).abs() < 1e-3, "{dist}");
    for fno in 0..8 {
        let list = overlap::find_all_overlapping_faces(&mut r.g, &mut io, &mut s, &m, fno).unwrap();
        // the face first, each found once, every mark cleared (the octahedron's opposite edges lie
        // on one great circle each, which the spherical edge test may take as crossing)
        assert_eq!(list[0], fno as i32);
        let mut seen = list.clone();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), list.len());
        assert!((0..8).all(|f| s.face(f).ripflag == 0));
    }
}
