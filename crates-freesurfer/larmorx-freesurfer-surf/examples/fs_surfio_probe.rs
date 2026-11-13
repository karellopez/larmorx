// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The port's side of the fs.surfio probe (`validation/probes/fs-surfio/`):
//!
//!     fs_surfio_probe cases <seed> <n>            the case file, on standard output
//!     fs_surfio_probe run <cases> fix|reproduce   one line per case: the output, a tab, the
//!                                                 defects the port noted (comma-separated)

#[path = "../tests/fs_surfio/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("cases") if args.len() == 4 => {
            let seed: u64 = args[2].parse().expect("seed");
            let n: usize = args[3].parse().expect("n");
            for line in cases::generate(seed, n) {
                println!("{line}");
            }
        }
        Some("run") if args.len() == 4 => {
            let mode = match args[3].as_str() {
                "fix" => UpstreamDefects::Fix,
                "reproduce" => UpstreamDefects::Reproduce,
                m => panic!("mode {m}"),
            };
            let text = std::fs::read_to_string(&args[2]).expect("cases");
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let (out, ids) = cases::run_line(line, mode);
                println!("{out}\t{}", ids.join(","));
            }
        }
        _ => {
            eprintln!("usage: fs_surfio_probe cases <seed> <n> | run <cases> fix|reproduce");
            std::process::exit(2);
        }
    }
}
