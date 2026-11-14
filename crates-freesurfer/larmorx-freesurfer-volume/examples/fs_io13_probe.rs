// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The port's side of the fs.io probe's piece 13 (`validation/probes/fs-io/`, `check.py --package
//! larmorx-freesurfer-volume --example fs_io13_probe`): the label coordinate conversions and one
//! frame of a volume written, larmorx-freesurfer-volume's `label_coords` and `io_frames`.
//!
//!     fs_io13_probe cases <seed> <n> [portable]  the case file, on standard output
//!     fs_io13_probe recorded                     the recorded test's cases (seed 1's 1,200)
//!     fs_io13_probe run <cases> fix|reproduce    one line per case: the output, a tab, the
//!                                                defects the port noted (comma-separated)

#[path = "../../larmorx-freesurfer-io/tests/fs_io/cases.rs"]
mod cases;
#[path = "../tests/fs_io13/piece13.rs"]
mod piece13;

use larmorx_core::defects::UpstreamDefects;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("cases") if args.len() == 4 || args.len() == 5 => {
            let seed: u64 = args[2].parse().expect("seed");
            let n: usize = args[3].parse().expect("n");
            for line in piece13::cases(seed, n) {
                println!("{line}");
            }
        }
        Some("recorded") if args.len() == 2 => {
            for line in piece13::cases(1, piece13::RECORDED) {
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
                let (out, ids) = cases::run_line_with(line, mode, &w, Some(piece13::run));
                println!("{out}\t{}", ids.join(","));
            }
            cases::remove_fixture(&w.fix);
            let _ = std::fs::remove_dir_all(&base);
        }
        _ => {
            eprintln!(
                "usage: fs_io13_probe cases <seed> <n> [portable] | recorded | run <cases> fix|reproduce"
            );
            std::process::exit(2);
        }
    }
}
