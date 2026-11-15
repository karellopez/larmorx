// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRIread` of one volume, for measurements (memory, time) beside FreeSurfer's `mri_info`:
//!
//!     read_volume <name> [fix|reproduce]
//!
//! prints `<width> <height> <depth> <frames> type <type>` and the defects noted; FreeSurfer's
//! messages on standard output and error; the program's end with FreeSurfer's status.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = if args.get(2).map(String::as_str) == Some("reproduce") {
        UpstreamDefects::Reproduce
    } else {
        UpstreamDefects::Fix
    };
    let mut g = Globals::new(mode);
    let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
    let env = Environment::default();
    let m = larmorx_freesurfer_io::mriio::mri_read_volume(
        &mut g, &mut out, &mut err, &env, None, &args[1],
    );
    let ids: Vec<&str> = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    match m {
        Ok(Some(m)) => println!(
            "{} {} {} {} type {} noted [{}]",
            m.vg.width,
            m.vg.height,
            m.vg.depth,
            m.nframes,
            m.mtype,
            ids.join(",")
        ),
        Ok(None) => {
            println!("no volume noted [{}]", ids.join(","));
            std::process::exit(1);
        }
        Err(Fatal::Exit(code)) => {
            println!("noted [{}]", ids.join(","));
            std::process::exit(code & 0xff)
        }
        Err(other) => {
            eprintln!("no volume: {other:?}");
            std::process::exit(134);
        }
    }
}
