// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The time each step of `mrisMarkIntersections` takes on a recorded surface (relay 39 of track
//! D; `validation/probes/fs-surfio/README.md`):
//!
//!     fs_mht_timing <surface>
//!
//! prints the seconds of the surface's read, the face table (`MHTcreateFaceTable`), the test of
//! every face (`MHTdoesFaceIntersect`) with the count, and `mrisMarkIntersections` without hole
//! filling.

use std::time::Instant;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mht::Mht;
use larmorx_freesurfer_surf::{props10, readwrite};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 2, "usage: fs_mht_timing <surface>");
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
    let mut s = readwrite::read(&mut g, &mut sg, &mut io, None, &args[1], true)
        .expect("read")
        .expect("a surface");
    println!("read {:.3} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    let m = Mht::create_face_table(&mut g, io.err, &s).expect("table");
    println!(
        "face table {:.3} s ({} buckets)",
        t.elapsed().as_secs_f64(),
        m.bucket_count()
    );
    let t = Instant::now();
    let mut n = 0;
    for f in 0..s.nfaces {
        n += m.does_face_intersect(&mut g, io.err, &s, f).expect("test");
    }
    println!(
        "face tests {:.3} s ({n} faces intersect)",
        t.elapsed().as_secs_f64()
    );
    let t = Instant::now();
    let k = props10::mris_mark_intersections(&mut g, &mut io, &mut s, 0).expect("marks");
    println!(
        "mrisMarkIntersections {:.3} s ({k})",
        t.elapsed().as_secs_f64()
    );
}
