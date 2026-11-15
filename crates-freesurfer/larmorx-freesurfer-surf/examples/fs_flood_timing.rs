// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The time each step of `mriflood.rs`'s surface functions takes on a recorded head (relay 32 of
//! track D, the speed work; `validation/probes/fs-surfio/README.md`):
//!
//!     fs_flood_timing <surface> <volume>
//!
//! prints the seconds of the surface's read, its metric properties, the shell into a copy of the
//! volume, the flood from outside of that shell, 2,000,000 cached conversions and the whole
//! interior (`MRISfillInterior(surface, 0, copy)`).

use std::time::Instant;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_filter::flood::mris_flood_outside;
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::mriio::mri_read;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::{mp, mriflood, readwrite, sras};
use larmorx_freesurfer_volume::arith::Pair;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 3, "usage: fs_flood_timing <surface> <volume>");
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
    mp::compute_metric_properties(&mut g, &mut sg, &mut io, &mut s).expect("mp");
    println!("metric properties {:.3} s", t.elapsed().as_secs_f64());
    let v = mri_read(
        &mut g,
        io.out,
        io.err,
        &env,
        None,
        &args[2],
        MRI_VOLUME_TYPE_UNKNOWN,
        true,
        -1,
        -1,
    )
    .expect("read")
    .expect("a volume");
    let mut d = mri_copy(&mut g, io.out, io.err, &v, None)
        .expect("copy")
        .unwrap_new();
    let t = Instant::now();
    mriflood::mris_shell(&mut g, &mut sg, &mut io, &v, &s, Some(&mut d), 1).expect("shell");
    println!("shell {:.3} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    mris_flood_outside(&mut g, io.out, io.err, Pair::Same(&mut d)).expect("flood");
    println!("flood {:.3} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    for k in 0..2_000_000 {
        let x = f64::from(k % 100) * 0.5;
        sras::mris_surface_ras_to_voxel_cached(&mut g, &mut sg, &mut io, &s, &v, x, -x, 1.0)
            .expect("conversion");
    }
    println!("2e6 cached conversions {:.3} s", t.elapsed().as_secs_f64());
    let mut d = mri_copy(&mut g, io.out, io.err, &v, None)
        .expect("copy")
        .unwrap_new();
    let t = Instant::now();
    mriflood::mris_fill_interior(&mut g, &mut sg, &mut io, &mut s, 0.0, Some(&mut d))
        .expect("interior");
    println!("interior {:.3} s", t.elapsed().as_secs_f64());
}
