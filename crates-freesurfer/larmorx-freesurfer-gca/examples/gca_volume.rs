// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A GCA atlas read as a volume (`mri_read` of type 22, the frame by the name's `#`), its voxels
//! printed one per line (`x y z frame value-bits value`), or with a third argument written there
//! as raw little-endian `float`s (x fastest, then y, z, frame), for comparisons with FreeSurfer's
//! own `mri_convert <atlas> <volume>.mgz` (track A's relay 35: `docs/findings/freesurfer-io.md`
//! Part 29).
//!
//!     cargo run --release --example gca_volume -p larmorx-freesurfer-gca -- <atlas>[#<frame>] [fix|reproduce] [<raw>]

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = if args.get(2).map(String::as_str) == Some("reproduce") {
        UpstreamDefects::Reproduce
    } else {
        UpstreamDefects::Fix
    };
    larmorx_freesurfer_gca::register_io();
    let mut g = Globals::new(mode);
    g.mriio_strip_pound = 1;
    let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
    let env = Environment::default();
    let m = larmorx_freesurfer_io::mriio::mri_read(
        &mut g, &mut out, &mut err, &env, None, &args[1], 22, true, -1, -1,
    );
    let Ok(Some(m)) = m else {
        eprintln!("no volume");
        std::process::exit(1);
    };
    println!(
        "{} {} {} {} type {}",
        m.vg.width, m.vg.height, m.vg.depth, m.nframes, m.mtype
    );
    let mut raw = Vec::new();
    let print = args.len() < 4;
    for f in 0..m.nframes {
        for z in 0..m.vg.depth {
            for y in 0..m.vg.height {
                for x in 0..m.vg.width {
                    let v =
                        mri_get_vox_val(&mut g, &mut err, &m, x, y, z, f, 0).unwrap_or(f32::NAN);
                    if print {
                        println!("{x} {y} {z} {f} {:08x} {v}", v.to_bits());
                    } else {
                        raw.extend_from_slice(&v.to_le_bytes());
                    }
                }
            }
        }
    }
    if !print {
        std::fs::write(&args[3], &raw).expect("the raw output");
    }
    for n in g.notices.as_slice() {
        eprintln!("notice {}: {}", n.defect.id, n.detail);
    }
}
