// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A GCA morph read as a volume (`mri_read` of a `.m3z` name, the frame by the name's `#`) and
//! written as an uncompressed MGH file, for comparisons with FreeSurfer's own `mri_convert
//! <morph> <volume>.mgh` (track A's relay 38: `docs/findings/freesurfer-io.md` Part 33). The
//! notices go to standard error.
//!
//!     cargo run --release --example m3z_volume -p larmorx-freesurfer-gca -- <morph>[#<frame>] <out.mgh> [fix|reproduce]

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = if args.get(3).map(String::as_str) == Some("reproduce") {
        UpstreamDefects::Reproduce
    } else {
        UpstreamDefects::Fix
    };
    larmorx_freesurfer_gca::register_io();
    let mut g = Globals::new(mode);
    g.mriio_strip_pound = 1;
    let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
    let env = Environment::from_pairs(std::env::vars().filter(|(k, _)| {
        matches!(k.as_str(), "FS_SKIP_TAGS" | "SHOW_NEG" | "FS_LEAVE_NANS")
    }));
    let m = larmorx_freesurfer_io::mriio::mri_read(
        &mut g, &mut out, &mut err, &env, None, &args[1], -1, true, -1, -1,
    );
    let mut m = match m {
        Ok(Some(m)) => m,
        Ok(None) => {
            eprintln!("no volume");
            std::process::exit(1);
        }
        Err(Fatal::Exit(code)) => std::process::exit(code & 0xff),
        Err(other) => {
            eprintln!("no volume: {other:?}");
            std::process::exit(134);
        }
    };
    let r = larmorx_freesurfer_io::mriio::mri_write(
        &mut g, &mut out, &mut err, &env, None, &mut m, &args[2], None, -1,
    );
    if !matches!(r, Ok(0)) {
        eprintln!("not written: {r:?}");
        std::process::exit(1);
    }
    for n in g.notices.as_slice() {
        eprintln!("notice {}: {}", n.defect.id, n.detail);
    }
}
