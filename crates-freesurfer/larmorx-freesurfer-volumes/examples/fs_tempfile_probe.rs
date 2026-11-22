// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The port's side of the fs-temp-file probe (`validation/probes/fs-temp-file/`):
//!
//!     fs_tempfile_probe cases                      every case, on standard output
//!     fs_tempfile_probe golden-cases               the golden cases (`tests/fs_temp_file_golden.rs`)
//!     fs_tempfile_probe run <cases> fix|reproduce  one line per case: the output, a tab, the
//!                                                  defects the port noted (comma-separated)
//!     fs_tempfile_probe run-all fix|reproduce      the same for every case, generated in place
//!                                                  (each line after its number: the Windows
//!                                                  cross-check compares them)

#[path = "../tests/fs_tempfile/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;

fn mode(s: &str) -> UpstreamDefects {
    match s {
        "fix" => UpstreamDefects::Fix,
        "reproduce" => UpstreamDefects::Reproduce,
        m => panic!("mode {m}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("cases") => {
            for line in cases::all_cases() {
                println!("{line}");
            }
        }
        Some("golden-cases") => {
            for line in cases::golden_cases() {
                println!("{line}");
            }
        }
        Some("run-all") if args.len() == 3 => {
            let m = mode(&args[2]);
            for (k, line) in cases::all_cases().iter().enumerate() {
                let (out, ids) = cases::run_line(line, m);
                println!("{k}\t{out}\t{}", ids.join(","));
            }
        }
        Some("run") if args.len() == 4 => {
            let m = mode(&args[3]);
            let text = std::fs::read_to_string(&args[2]).expect("cases");
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let (out, ids) = cases::run_line(line, m);
                println!("{out}\t{}", ids.join(","));
            }
        }
        _ => {
            eprintln!(
                "usage: fs_tempfile_probe cases | golden-cases | run <cases> fix|reproduce | run-all fix|reproduce"
            );
            std::process::exit(2);
        }
    }
}
