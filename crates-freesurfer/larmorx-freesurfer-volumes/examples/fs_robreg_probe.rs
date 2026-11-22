// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The port's side of the fs-robust-register probe (`validation/probes/fs-robust-register/`):
//!
//!     fs_robreg_probe cases <seed> <n>            the case file, on standard output
//!     fs_robreg_probe golden-cases                the golden cases
//!                                                (`tests/mri_robust_register_golden.rs`)
//!     fs_robreg_probe run <cases> fix|reproduce   one line per case: the output, a tab, the
//!                                                defects the port noted (comma-separated)
//!     fs_robreg_probe run-seed <seed> <n> fix|reproduce   the same for the cases of a seed,
//!                                                generated in place (each line after its number:
//!                                                the Windows cross-check compares them)

#[path = "../tests/fs_robreg/cases.rs"]
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
        Some("golden-cases") => {
            for line in cases::golden_cases() {
                println!("{line}");
            }
        }
        Some("run-seed") if args.len() == 5 => {
            let seed: u64 = args[2].parse().expect("seed");
            let n: usize = args[3].parse().expect("n");
            let mode = match args[4].as_str() {
                "fix" => UpstreamDefects::Fix,
                "reproduce" => UpstreamDefects::Reproduce,
                m => panic!("mode {m}"),
            };
            for (k, line) in cases::generate(seed, n).iter().enumerate() {
                let (out, ids) = cases::run_line(line, mode);
                println!("{k}\t{out}\t{}", ids.join(","));
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
            eprintln!(
                "usage: fs_robreg_probe cases <seed> <n> | golden-cases | run <cases> fix|reproduce | run-seed <seed> <n> fix|reproduce"
            );
            std::process::exit(2);
        }
    }
}
