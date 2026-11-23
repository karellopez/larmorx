// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The golden cases' helpers for `validation/probes/fs-cc/prog.py --golden`:
//!
//!     fs_cc_golden inputs <dir>          writes the inputs into <dir>
//!     fs_cc_golden digest <file>         prints a written file's digest (the test's)
//!     fs_cc_golden mask <out> <err>      prints two streams masked as the test masks them
//!     fs_cc_golden cases                 the cases: <k> TAB <arguments> TAB <outputs> TAB <environment>
//!     fs_cc_golden run <dir> <k> fix|reproduce   case k with the port: the hash lines

#[path = "../tests/mri_cc_golden/common.rs"]
mod common;

use std::path::Path;

use larmorx_core::defects::UpstreamDefects;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match (a.get(1).map(String::as_str), a.len()) {
        (Some("inputs"), 3) => common::write_inputs(Path::new(&a[2])),
        (Some("digest"), 3) => println!("{}", common::digest(Path::new(&a[2]))),
        (Some("mask"), 4) => {
            let o = std::fs::read_to_string(&a[2]).expect("a text");
            let e = std::fs::read_to_string(&a[3]).expect("a text");
            print!("{}", common::mask(&o, &e));
        }
        (Some("cases"), 2) => {
            for (k, c) in common::cases().iter().enumerate() {
                let env: Vec<String> = common::case_env(k)
                    .iter()
                    .map(|(a, b)| format!("{a}={b}"))
                    .collect();
                println!(
                    "{k}\t{}\t{}\t{}",
                    c.join(" "),
                    common::OUTPUTS[k].join(" "),
                    env.join(" ")
                );
            }
        }
        (Some("run"), 5) => {
            let k: usize = a[3].parse().expect("k");
            let mode = match a[4].as_str() {
                "fix" => UpstreamDefects::Fix,
                _ => UpstreamDefects::Reproduce,
            };
            let (status, text) = common::run_case(Path::new(&a[2]), k, mode);
            println!("case {k} {status}\n{text}");
        }
        _ => {
            eprintln!(
                "usage: fs_cc_golden inputs <dir> | digest <file> | mask <out> <err> | cases | run <dir> <k> <mode>"
            );
            std::process::exit(2);
        }
    }
}
