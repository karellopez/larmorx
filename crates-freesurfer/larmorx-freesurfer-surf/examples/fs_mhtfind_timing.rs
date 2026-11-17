// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The time each part of the probe's `flr mcl` takes on recorded surfaces (relay 43 of track D;
//! `validation/probes/fs-surfio/README.md`):
//!
//!     fs_mhtfind_timing <white> <pial> <sphere.reg> <other sphere.reg>
//!
//! prints the seconds of: the reads; `mri_surf2volseg`'s vertex table of the white surface at
//! 16 mm and `MHTfindClosestVertexNoXYZ` of every pial vertex; `mri_surf2surf`'s
//! `MHTfindClosestVertexNo2` of every vertex of the other sphere; `MHTfindClosestVertexInTable` at
//! 1 mm; `mrisProjectOntoSurface`'s two face searches of every pial vertex.

use std::time::Instant;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_surf::views::CURRENT_VERTICES;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        5,
        "usage: fs_mhtfind_timing <white> <pial> <sphere.reg> <other sphere.reg>"
    );
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let mut sg = SurfGlobals::default();
    let env = Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]);
    let (mut out, mut err) = (std::io::sink(), std::io::stderr());
    let mut io = Io {
        out: &mut out,
        err: &mut err,
        env: &env,
    };
    let t = Instant::now();
    let mut read = |name: &str| {
        readwrite::read(&mut g, &mut sg, &mut io, None, name, true)
            .expect("read")
            .expect("a surface")
    };
    let (w, p, sr, so) = (
        read(&args[1]),
        read(&args[2]),
        read(&args[3]),
        read(&args[4]),
    );
    println!("reads {:.3} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    let m = Mht::create_vertex_table_resolution(&mut g, io.err, &w, CURRENT_VERTICES, 16.0)
        .expect("table");
    let mut n = 0;
    for v in 0..p.nvertices as usize {
        let x = p.vertex(v);
        n += i32::from(
            m.find_closest_vertex_no_xyz(&mut g, io.err, &w, [x.x, x.y, x.z])
                .expect("search")
                .0
                >= 0,
        );
    }
    println!(
        "white at 16 mm, NoXYZ {:.3} s ({n})",
        t.elapsed().as_secs_f64()
    );
    let t = Instant::now();
    let m = Mht::create_vertex_table_resolution(&mut g, io.err, &sr, CURRENT_VERTICES, 16.0)
        .expect("table");
    let mut n = 0;
    for v in 0..so.nvertices {
        n += i32::from(
            m.find_closest_vertex_no2(&mut g, &mut std::io::sink(), io.err, &sr, &so, v)
                .expect("search")
                .0
                >= 0,
        );
    }
    println!(
        "sphere at 16 mm, No2 {:.3} s ({n})",
        t.elapsed().as_secs_f64()
    );
    let t = Instant::now();
    let m = Mht::create_vertex_table(&mut g, io.err, &w, CURRENT_VERTICES).expect("table");
    let mut n = 0;
    for v in 0..p.nvertices as usize {
        let x = p.vertex(v);
        n += i32::from(
            m.find_closest_vertex_in_table(&mut g, io.err, &w, [x.x, x.y, x.z], 0)
                .expect("search")
                .is_some(),
        );
    }
    println!(
        "white at 1 mm, InTable {:.3} s ({n})",
        t.elapsed().as_secs_f64()
    );
    let t = Instant::now();
    let m = Mht::create_face_table(&mut g, io.err, &w).expect("table");
    println!("face table {:.3} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    let mut wide = Vec::new();
    for v in 0..p.nvertices as usize {
        let x = p.vertex(v);
        let q = [f64::from(x.x), f64::from(x.y), f64::from(x.z)];
        let (f, d) = m
            .find_closest_face_generic(&mut g, io.err, &w, q, 8.0, 8, 1)
            .expect("search");
        if f < 0 || d > 0.5 {
            wide.push(q);
        }
    }
    println!(
        "faces at 8 mm, projected {:.3} s ({} wide)",
        t.elapsed().as_secs_f64(),
        wide.len()
    );
    let t = Instant::now();
    for q in wide {
        m.find_closest_face_generic(&mut g, io.err, &w, q, 1000.0, -1, -1)
            .expect("search");
    }
    println!("faces at 1000 mm {:.3} s", t.elapsed().as_secs_f64());
}
