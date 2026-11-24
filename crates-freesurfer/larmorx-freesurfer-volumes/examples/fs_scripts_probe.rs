// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The cases of `seg2cc`, `mergeseg` and `fscalc` for `validation/probes/fs-scripts/check.py`:
//!
//!     fs_scripts_probe cases                     one JSON object per line: k, prog, heavy, cwd, args, env
//!     fs_scripts_probe inputs <dir>              writes the fixture into <dir>
//!     fs_scripts_probe golden                    the golden cases' numbers
//!     fs_scripts_probe outcome <dir> <status> <out> <err> <uname>   a run's canonical text
//!     fs_scripts_probe notices <err>             the defects a port's run names (and UNMODELLED)

#[path = "../tests/fs_scripts/cases.rs"]
mod cases;

use std::path::Path;

fn json(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("cases") => {
            for (k, c) in cases::cases().iter().enumerate() {
                let args: Vec<String> = c.args.iter().map(|s| json(s)).collect();
                let env: Vec<String> = c
                    .env
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "[{}, {}]",
                            json(k),
                            v.as_deref().map_or("null".to_owned(), json)
                        )
                    })
                    .collect();
                println!(
                    "{{\"k\": {k}, \"prog\": {}, \"heavy\": {}, \"quick\": {}, \"cwd\": {}, \"args\": [{}], \"env\": [{}]}}",
                    json(c.prog),
                    c.heavy,
                    c.quick,
                    json(c.cwd),
                    args.join(", "),
                    env.join(", ")
                );
            }
        }
        Some("inputs") if a.len() == 3 => cases::write_fixture(Path::new(&a[2])),
        Some("golden") => {
            let g: Vec<String> = cases::golden_indices()
                .iter()
                .map(|k| k.to_string())
                .collect();
            println!("{}", g.join(" "));
        }
        Some("outcome") if a.len() == 7 => {
            let out = std::fs::read(&a[4]).expect("a file");
            let err = std::fs::read(&a[5]).expect("a file");
            let status: u8 = a[3].parse().expect("a status");
            print!(
                "{}",
                cases::outcome(Path::new(&a[2]), status, &out, &err, &a[6])
            );
        }
        Some("notices") if a.len() == 3 => {
            let err = std::fs::read(&a[2]).expect("a file");
            println!("{}", cases::notices(&err).join(" "));
        }
        _ => {
            eprintln!(
                "usage: fs_scripts_probe cases | inputs <dir> | outcome <dir> <status> <out> <err> <uname> | notices <err>"
            );
            std::process::exit(2);
        }
    }
}
