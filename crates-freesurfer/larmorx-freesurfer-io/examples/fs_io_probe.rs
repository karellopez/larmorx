// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The port's side of the fs.io probe (`validation/probes/fs-io/`):
//!
//!     fs_io_probe cases <seed> <n> [portable]  the case file, on standard output
//!     fs_io_probe recorded                     the recorded test's cases (seed 1's 4,800 portable
//!                                              ones without the heavy ones, `cases::HEAVY_SEED1`)
//!     fs_io_probe run <cases> fix|reproduce    one line per case: the output, a tab, the
//!                                              defects the port noted (comma-separated); the
//!                                              files in <cases>.port/

#[path = "../tests/fs_io/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("cases") if args.len() == 4 || args.len() == 5 => {
            let seed: u64 = args[2].parse().expect("seed");
            let n: usize = args[3].parse().expect("n");
            let portable = args.get(4).map(String::as_str) == Some("portable");
            for line in cases::generate(seed, n, portable) {
                println!("{line}");
            }
        }
        Some("recorded") if args.len() == 2 => {
            for line in cases::recorded_cases() {
                println!("{line}");
            }
        }
        Some("run") if args.len() == 4 => {
            let mode = match args[3].as_str() {
                "fix" => UpstreamDefects::Fix,
                "reproduce" => UpstreamDefects::Reproduce,
                m => panic!("mode {m}"),
            };
            let base = std::path::PathBuf::from(format!("{}.port", args[2]));
            let w = cases::Work {
                fix: base.join("fix"),
                dat: base.join("dat"),
            };
            cases::make_fixture(&w.fix);
            let text = std::fs::read_to_string(&args[2]).expect("cases");
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let (out, ids) = cases::run_line(line, mode, &w);
                println!("{out}\t{}", ids.join(","));
            }
            cases::remove_fixture(&w.fix);
            let _ = std::fs::remove_dir_all(&base);
        }
        _ => {
            eprintln!(
                "usage: fs_io_probe cases <seed> <n> [portable] | recorded | run <cases> fix|reproduce"
            );
            std::process::exit(2);
        }
    }
}
