// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The closest searches of the surface hash table (relay 43 of track D, `mhtfind.rs`): the
//! searches against a full scan where they cover the space exhaustively, the shells' bound, and the
//! corrections (FS-909, FS-576, FS-997 to FS-999), on small surfaces read from binary triangle
//! files.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::mhtfind;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_surf::views::CURRENT_VERTICES;

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
            env: Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]),
            out: Vec::new(),
            err: Vec::new(),
        }
    }

    /// The surface of `v` and `f` written as a binary triangle file and read by `MRISread`.
    fn surface(&mut self, v: &[[f32; 3]], f: &[[i32; 3]]) -> Mris {
        let mut b = vec![0xff, 0xff, 0xfe];
        b.extend_from_slice(b"created by me on Tue Nov 17 18:00:00 2026\n\n");
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
        let mut io = Io {
            out: &mut self.out,
            err: &mut self.err,
            env: &self.env,
        };
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

/// A `n` by `n` grid of `step` mm in the plane z = `z`, its heights a seeded ripple (eighths), and
/// its triangles.
fn grid(n: i32, step: f32, z: f32, seed: u64) -> (Vec<[f32; 3]>, Vec<[i32; 3]>) {
    let mut x = seed;
    let mut v = Vec::new();
    for j in 0..n {
        for i in 0..n {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let h = ((x >> 33) % 9) as f32 / 8.0;
            v.push([i as f32 * step, j as f32 * step, z + h]);
        }
    }
    let mut f = Vec::new();
    for j in 0..n - 1 {
        for i in 0..n - 1 {
            let (a, b, c, d) = (
                j * n + i,
                j * n + i + 1,
                (j + 1) * n + i,
                (j + 1) * n + i + 1,
            );
            f.push([a, b, d]);
            f.push([a, d, c]);
        }
    }
    (v, f)
}

/// The probes: a seeded cloud around the grid (quarters of a millimetre).
fn probes(k: usize, lo: f32, hi: f32, seed: u64) -> Vec<[f32; 3]> {
    let mut x = seed;
    (0..k)
        .map(|_| {
            std::array::from_fn(|_| {
                x = x
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let span = ((hi - lo) * 4.0) as u64 + 1;
                lo + ((x >> 33) % span) as f32 / 4.0
            })
        })
        .collect()
}

/// The nearest vertex by a full scan in `double` (the search's arithmetic): its number and squared
/// distance, and whether another vertex is as near.
fn nearest(s: &Mris, p: [f64; 3]) -> (i32, f64, bool) {
    let (mut best, mut sq, mut tie) = (-1, f64::INFINITY, false);
    for v in 0..s.nvertices {
        let x = s.vertex(v as usize);
        if x.ripflag != 0 {
            continue;
        }
        let d = (f64::from(x.x) - p[0]).powi(2)
            + (f64::from(x.y) - p[1]).powi(2)
            + (f64::from(x.z) - p[2]).powi(2);
        if d < sq {
            (best, sq, tie) = (v, d, false);
        } else if d == sq {
            tie = true;
        }
    }
    (best, sq, tie)
}

/// Where a search covers the space exhaustively (its distance limit at most what its shells reach)
/// it finds the full scan's nearest vertex, or none when that is beyond the limit; in both modes.
#[test]
fn the_vertex_search_is_the_full_scans_nearest() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = grid(9, 1.5, 0.0, 7);
        let s = r.surface(&v, &f);
        for res in [1.0f32, 2.0, 0.5] {
            let m = Mht::create_vertex_table_resolution(
                &mut r.g,
                &mut r.err,
                &s,
                CURRENT_VERTICES,
                res,
            )
            .unwrap();
            let mut checked = 0;
            for p in probes(400, -4.0, 16.0, 11) {
                let q = p.map(f64::from);
                for (dmax, shells) in [(1.0, 1), (2.0, 2), (0.25, 0), (3.0, 3), (5.0, -1)] {
                    let res = f64::from(res);
                    // the shells looked at (at most 5) and the distance limit, as the search sets them
                    let eff = if shells >= 0 {
                        shells
                    } else if dmax * 2.0 <= res {
                        0
                    } else {
                        ((dmax / res).ceil() as i32).min(5)
                    };
                    let covered = if eff >= 1 {
                        f64::from(eff) * res
                    } else {
                        0.5 * res
                    };
                    let maxd = if shells >= 0 { dmax.min(covered) } else { dmax };
                    let (vno, d) = m
                        .find_closest_vertex_generic(&mut r.g, &mut r.err, &s, q, dmax, shells)
                        .unwrap();
                    let (best, sq, tie) = nearest(&s, q);
                    if sq.sqrt() <= maxd.min(covered) * 0.999 {
                        if !tie {
                            assert_eq!(vno, best, "{mode:?} res {res} {p:?} {dmax} {shells}");
                        }
                        assert_eq!(d, sq.sqrt(), "{mode:?}");
                        checked += 1;
                    } else if sq.sqrt() > maxd * 1.001 {
                        assert_eq!((vno, d), (-1, 1e3), "{mode:?} res {res} {p:?}");
                    }
                }
            }
            assert!(checked > 100, "{checked}");
        }
    }
}

/// FS-997 (kept): `MHTfindClosestVertexNoXYZ` gives no vertex for a point more than one voxel from
/// every vertex, in both modes; `MHTfindClosestSetVertexNo` and the global table search find it.
#[test]
fn fs997_no_vertex_within_one_voxel() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = grid(4, 1.0, 0.0, 3);
        let s = r.surface(&v, &f);
        let m = Mht::create_vertex_table(&mut r.g, &mut r.err, &s, CURRENT_VERTICES).unwrap();
        let p = [1.0f32, 1.0, 6.0];
        let (vno, d) = m
            .find_closest_vertex_no_xyz(&mut r.g, &mut r.err, &s, p)
            .unwrap();
        assert_eq!((vno, d), (-1, 1000.0), "{mode:?}");
        let set = m
            .find_closest_set_vertex_no(&mut r.g, &mut r.err, &s, p)
            .unwrap();
        let glob = m
            .find_vno_of_closest_vertex_in_table(&mut r.g, &mut r.err, &s, p, 1)
            .unwrap();
        let local = m
            .find_vno_of_closest_vertex_in_table(&mut r.g, &mut r.err, &s, p, 0)
            .unwrap();
        let (best, sq, _) = nearest(&s, p.map(f64::from));
        assert_eq!((set, glob, local), (best, best, -1), "{mode:?}");
        let (bf, bd) = m
            .brute_force_closest_vertex(&mut r.g, &mut r.err, &s, p, CURRENT_VERTICES)
            .unwrap();
        assert_eq!(bf, best);
        assert!((f64::from(bd) - sq.sqrt()).abs() < 1e-5, "{bd}");
    }
}

/// FS-998, FS-576: a vertex search of a face table: FreeSurfer's own check's error by default; with
/// `reproduce` the face numbers read as vertex numbers, those beyond the vertices skipped (noted).
#[test]
fn fs998_a_search_of_the_other_kind_of_table() {
    let (v, f) = grid(3, 1.0, 0.0, 5);
    let mut r = Run::new(UpstreamDefects::Fix);
    let s = r.surface(&v, &f);
    let m = Mht::create_face_table(&mut r.g, &mut r.err, &s).unwrap();
    assert!(
        m.find_closest_vertex_generic(&mut r.g, &mut r.err, &s, [1.0, 1.0, 0.0], 1000.0, 2)
            .is_err()
    );
    assert!(r.noted("FS-998"));
    assert!(
        String::from_utf8_lossy(&r.err).contains("mht not initialized for vertices"),
        "{}",
        String::from_utf8_lossy(&r.err)
    );
    let mut r = Run::new(UpstreamDefects::Reproduce);
    let s = r.surface(&v, &f);
    let m = Mht::create_face_table(&mut r.g, &mut r.err, &s).unwrap();
    let (vno, _) = m
        .find_closest_vertex_generic(&mut r.g, &mut r.err, &s, [1.0, 1.0, 0.0], 1000.0, 2)
        .unwrap();
    // 8 faces, 9 vertices: every face number is a vertex number, so nothing beyond
    assert!((0..9).contains(&vno), "{vno}");
    assert!(r.noted("FS-998") && !r.noted("FS-576"));
    // a vertex table searched for faces: vertex 8 is beyond the 8 faces
    let mv = Mht::create_vertex_table(&mut r.g, &mut r.err, &s, CURRENT_VERTICES).unwrap();
    let (fno, _) = mv
        .find_closest_face_generic(&mut r.g, &mut r.err, &s, [2.0, 2.0, 0.0], 4.0, 4, -1)
        .unwrap();
    assert!((0..8).contains(&fno), "{fno}");
    assert!(r.noted("FS-576"));
}

/// FS-999: a surface beyond the table (1,000 mm from the origin at 1 mm): a point at one of its
/// vertices finds it by default (the edge's bucket looked up); with `reproduce` nothing, as
/// FreeSurfer.
#[test]
fn fs999_a_vertex_beyond_the_table() {
    let (mut v, f) = grid(3, 1.0, 0.0, 9);
    for p in &mut v {
        p[0] += 1005.0;
    }
    let p = v[4];
    for (mode, want) in [(UpstreamDefects::Fix, 4), (UpstreamDefects::Reproduce, -1)] {
        let mut r = Run::new(mode);
        let s = r.surface(&v, &f);
        let m = Mht::create_vertex_table(&mut r.g, &mut r.err, &s, CURRENT_VERTICES).unwrap();
        let (vno, d) = m
            .find_closest_vertex_no_xyz(&mut r.g, &mut r.err, &s, p)
            .unwrap();
        assert_eq!(vno, want, "{mode:?}");
        if want >= 0 {
            assert_eq!(d, 0.0);
        }
        assert!(r.noted("FS-999"), "{mode:?}");
    }
}

/// FS-909: a probe that is not finite: a clear error by default; with `reproduce` nothing found.
#[test]
fn fs909_a_probe_not_finite() {
    let (v, f) = grid(3, 1.0, 0.0, 1);
    let mut r = Run::new(UpstreamDefects::Fix);
    let s = r.surface(&v, &f);
    let m = Mht::create_face_table(&mut r.g, &mut r.err, &s).unwrap();
    let nan = [f64::NAN, 0.0, 0.0];
    assert!(
        m.find_closest_face_generic(&mut r.g, &mut r.err, &s, nan, 8.0, 8, 1)
            .is_err()
    );
    assert!(r.noted("FS-909"));
    let mut r = Run::new(UpstreamDefects::Reproduce);
    let s = r.surface(&v, &f);
    let m = Mht::create_face_table(&mut r.g, &mut r.err, &s).unwrap();
    let got = m
        .find_closest_face_generic(&mut r.g, &mut r.err, &s, nan, 8.0, 8, 1)
        .unwrap();
    assert_eq!(got, (-1, 1e3));
}

/// The face search: the nearest centroid, only faces the point projects into with a positive
/// `project_into_face`; a table whose faces are all ripped ends at once at 1,000 mm (the shells'
/// bound: FreeSurfer looks at 8 x 10^9 voxels), and the brute-force search agrees with a scan.
#[test]
fn the_face_search() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = grid(6, 2.0, 0.0, 13);
        let mut s = r.surface(&v, &f);
        let m = Mht::create_face_table(&mut r.g, &mut r.err, &s).unwrap();
        let p = [3.3, 4.1, 1.5];
        let (fno, d) = m
            .find_closest_face_generic(&mut r.g, &mut r.err, &s, p, 8.0, 8, -1)
            .unwrap();
        let (bf, bd) = mhtfind::brute_force_closest_face(
            &mut r.g,
            &mut r.err,
            &s,
            p.map(|c| c as f32),
            CURRENT_VERTICES,
        )
        .unwrap();
        assert_eq!(fno, bf, "{mode:?}");
        assert!((d as f32 - bd).abs() < 1e-5, "{d} {bd}");
        // outside the grid's projection: no face with the projection test
        let out = [-5.0, -5.0, 0.0];
        let (fno, _) = m
            .find_closest_face_generic(&mut r.g, &mut r.err, &s, out, 8.0, 8, 1)
            .unwrap();
        assert_eq!(fno, -1, "{mode:?}");
        let (fno, _) = m
            .find_closest_face_generic(&mut r.g, &mut r.err, &s, out, 1000.0, -1, -1)
            .unwrap();
        assert!(fno >= 0, "{mode:?}");
        for k in 0..s.nfaces as usize {
            s.face_mut(k).ripflag = 1;
        }
        let empty = Mht::create_face_table(&mut r.g, &mut r.err, &s).unwrap();
        let t0 = std::time::Instant::now();
        let got = empty
            .find_closest_face_no_generic(&mut r.g, &mut r.err, &s, p, 1000.0, -1, -1)
            .unwrap();
        assert_eq!(got, (-1, 1e3));
        assert!(t0.elapsed().as_secs_f64() < 1.0, "{:?}", t0.elapsed());
    }
}
