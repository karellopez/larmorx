// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The port's side of the fs-getfullpath probe (`validation/probes/fs-getfullpath/`):
//!
//!     fs_getfullpath_probe cases [<seed> <n>]         every case (or seed's n random names)
//!     fs_getfullpath_probe golden-cases               the golden cases (`tests/getfullpath_golden.rs`)
//!     fs_getfullpath_probe fixture                    the fixture's entries (hexadecimal; `/` last
//!                                                     for a folder, `U` before one only Unix holds)
//!     fs_getfullpath_probe run <cases> fix|reproduce  one line per case: the output, a tab, the
//!                                                     defects the port noted (comma-separated)
//!     fs_getfullpath_probe run-all fix|reproduce      the same for every case, numbered (the
//!                                                     Windows cross-check compares them)

#[path = "../tests/fs_getfullpath/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;

fn mode(s: &str) -> UpstreamDefects {
    match s {
        "fix" => UpstreamDefects::Fix,
        "reproduce" => UpstreamDefects::Reproduce,
        m => panic!("mode {m}"),
    }
}

fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("cases") if args.len() == 2 => cases::all_cases().iter().for_each(|l| println!("{l}")),
        Some("cases") if args.len() == 4 => {
            let seed: u64 = args[2].parse().expect("seed");
            let n: usize = args[3].parse().expect("n");
            cases::random_cases(seed, n)
                .iter()
                .for_each(|l| println!("{l}"));
        }
        Some("golden-cases") => cases::golden_cases().iter().for_each(|l| println!("{l}")),
        Some("fixture") => {
            let (portable, unix) = cases::fixture();
            portable.iter().for_each(|n| println!("P {}", hex(n)));
            unix.iter().for_each(|n| println!("U {}", hex(n)));
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
                "usage: fs_getfullpath_probe cases | golden-cases | fixture | run <cases> fix|reproduce | run-all fix|reproduce"
            );
            std::process::exit(2);
        }
    }
}
