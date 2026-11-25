// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The surface hash's vertex search reading the vertices' positions taken beforehand
//! (`Mht::vertex_positions`, `Mht::find_closest_vertex_no_xyz_at`; relay 59 of track A, for
//! `mri_label2vol --fill-ribbon`'s speed) against the search reading the vertices' records
//! (`find_closest_vertex_no_xyz`): the same vertex, the same distance's bits, the same notices and
//! the same failures, in both modes, on grids at the default resolution and at 16 mm, with ripped
//! vertices, a vertex whose coordinate is a NaN (built from its bits), probes beyond the table and
//! probes whose voxel is not an `int` (FS-909).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_surf::views::CURRENT_VERTICES;

const MODES: [UpstreamDefects; 2] = [UpstreamDefects::Fix, UpstreamDefects::Reproduce];

/// A seeded `n` by `n` grid of `step` mm (heights in eighths) and its triangles, read through
/// `MRISread` from a binary triangle file.
fn surface(g: &mut Globals, n: i32, step: f32, seed: u64) -> Mris {
    let mut x = seed;
    let mut v = Vec::new();
    for j in 0..n {
        for i in 0..n {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let h = ((x >> 33) % 9) as f32 / 8.0;
            v.push([i as f32 * step, j as f32 * step, h]);
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
    let mut b = vec![0xff, 0xff, 0xfe];
    b.extend_from_slice(b"created by me on Tue Nov 17 18:00:00 2026\n\n");
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
    let td = tempfile::Builder::new()
        .prefix("mht pos ~")
        .tempdir()
        .expect("a folder");
    std::fs::write(td.path().join("lh.s0"), b).expect("the file");
    let env = Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut io = Io {
        out: &mut out,
        err: &mut err,
        env: &env,
    };
    let mut sg = SurfGlobals::default();
    readwrite::read(g, &mut sg, &mut io, Some(td.path()), "lh.s0", true)
        .unwrap()
        .expect("a surface")
}

/// Seeded probes in quarters of a millimetre from `lo` to `hi` on each axis.
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

/// One search both ways from the same state: (vertex, distance's bits, notices' text, status of a
/// failure) of each.
fn both(
    mode: UpstreamDefects,
    s: &Mris,
    m: &Mht,
    p: [f32; 3],
) -> [(i32, u32, String, Option<u8>); 2] {
    let pos = m.vertex_positions(s).expect("positions");
    std::array::from_fn(|k| {
        let mut g = Globals::new(mode);
        let mut err = Vec::new();
        let r = if k == 0 {
            m.find_closest_vertex_no_xyz(&mut g, &mut err, s, p)
        } else {
            m.find_closest_vertex_no_xyz_at(&mut g, &mut err, s, &pos, p)
        };
        let notices: String = g
            .notices
            .as_slice()
            .iter()
            .map(|n| format!("{} {}\n", n.defect.id, n.detail))
            .collect();
        let text = format!("{notices}{}", String::from_utf8_lossy(&err));
        match r {
            Ok((v, d)) => (v, d.to_bits(), text, None),
            Err(f) => (-2, 0, text, Some(f.status())),
        }
    })
}

#[test]
fn the_positions_give_the_same_searches() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        for (n, step, res) in [(9, 1.0f32, None), (24, 2.5, Some(16.0f32))] {
            let mut s = surface(&mut g, n, step, 7);
            // ripped vertices (and below one whose x is a NaN, from its bits)
            for v in [3usize, 10, 11, 40] {
                s.vertex_mut(v).ripflag = 1;
            }
            let m = match res {
                None => Mht::create_vertex_table(&mut g, &mut Vec::new(), &s, CURRENT_VERTICES),
                Some(r) => Mht::create_vertex_table_resolution(
                    &mut g,
                    &mut Vec::new(),
                    &s,
                    CURRENT_VERTICES,
                    r,
                ),
            }
            .unwrap();
            // after the table (whose making refuses a NaN): the searches read it
            s.vertex_mut(5).x = f32::from_bits(0x7fc0_0123);
            let span = n as f32 * step;
            // near the grid (most find a vertex within one voxel) and far around it
            let mut ps: Vec<[f32; 3]> = probes(400, -2.0, span + 2.0, 11)
                .into_iter()
                .map(|p| [p[0], p[1], p[2] / span * 2.0 - 1.0])
                .collect();
            ps.extend(probes(100, -span, 2.0 * span, 13));
            ps.push([f32::from_bits(0x7fc0_0000), 0.0, 0.0]);
            ps.push([1e30, 0.0, 0.0]);
            ps.push([0.0, -1e12, 3.0]);
            let mut found = 0;
            for p in ps {
                let [a, b] = both(mode, &s, &m, p);
                assert_eq!(a, b, "{mode:?} {p:?}");
                found += usize::from(a.0 >= 0);
            }
            assert!(found > 100, "{found}");
        }
    }
}
