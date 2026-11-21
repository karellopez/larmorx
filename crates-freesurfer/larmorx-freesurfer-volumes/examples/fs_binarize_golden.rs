// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The golden cases' helpers for `validation/probes/fs-binarize/prog.py --golden`:
//!
//!     fs_binarize_golden inputs <dir>     writes the 32^3 inputs into <dir>
//!     fs_binarize_golden digest <file>    prints a written file's digest (the test's)
//!     fs_binarize_golden mask <file>      prints a standard output masked as the test masks it
//!     fs_binarize_golden cases            the cases: <k> TAB <arguments> TAB <outputs>
//!     fs_binarize_golden run <dir> <k> fix|reproduce   case k with the port: the hash lines

#[path = "../tests/mri_binarize_golden/common.rs"]
mod common;

use std::path::Path;

use larmorx_core::defects::UpstreamDefects;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match (a.get(1).map(String::as_str), a.len()) {
        (Some("inputs"), 3) => common::write_inputs(Path::new(&a[2])),
        (Some("digest"), 3) => println!("{}", common::digest(Path::new(&a[2]))),
        (Some("mask"), 3) => {
            let t = std::fs::read_to_string(&a[2]).expect("a text");
            println!(
                "{}",
                t.lines()
                    .map(common::mask_line)
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
        (Some("cases"), 2) => {
            for (k, c) in common::CASES.iter().enumerate() {
                println!("{k}\t{}\t{}", c.join(" "), common::OUTPUTS[k].join(" "));
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
                "usage: fs_binarize_golden inputs <dir> | digest <file> | mask <file> | cases | run <dir> <k> <mode>"
            );
            std::process::exit(2);
        }
    }
}
