// SPDX-License-Identifier: GPL-3.0-only
// Part of larmorx-gpl (GNU GPL version 3; see crates-gpl/LICENSE). Copyright 2026 Karel Lopez
// Vilaret.
//! Parses t2smap's command line as the replica does and prints the outcome, for
//! `validation/investigations/t2smap/replica/check_parser.py` (which compares it with tedana's
//! own parser): `STATUS`, the standard output and error argparse would print, and the parsed
//! values in Python's `repr`. Width from `COLUMNS` (as the program).
use larmorx_core::python::argparse::{formatter_width, py_repr_str};
use larmorx_core::python::number::float_repr;
use larmorx_gpl_tedana::parser::{self, FitType};

fn list(v: &[String]) -> String {
    format!(
        "[{}]",
        v.iter()
            .map(|s| py_repr_str(s))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn opt(v: &Option<String>) -> String {
    v.as_deref().map_or("None".into(), py_repr_str)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let width = formatter_width(std::env::var("COLUMNS").ok().as_deref(), None);
    match parser::parse(&args, width) {
        Err(e) => {
            print!(
                "STATUS {}\nSTDOUT\n{}STDERR\n{}",
                e.status, e.stdout, e.stderr
            );
        }
        Ok(s) => {
            let b = |x: bool| if x { "True" } else { "False" };
            println!("STATUS ok\nSTDOUT\nSTDERR");
            println!("data={}", list(&s.data));
            println!(
                "tes=[{}]",
                s.tes
                    .iter()
                    .map(|t| float_repr(*t))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!("out_dir={}", py_repr_str(&s.out_dir));
            println!("prefix={}", py_repr_str(&s.prefix));
            println!("convention={}", py_repr_str(&s.convention));
            println!("verbose={}", b(s.verbose));
            println!("overwrite={}", b(s.overwrite));
            println!("mask={}", opt(&s.mask));
            println!("masktype={}", list(&s.masktype));
            println!("dummy_scans={}", s.dummy_scans);
            println!("exclude={}", opt(&s.exclude));
            println!(
                "fittype={}",
                if s.fittype == FitType::Curvefit {
                    "'curvefit'"
                } else {
                    "'loglin'"
                }
            );
            println!("fitmode={}", if s.fit_ts { "'ts'" } else { "'all'" });
            println!("combmode={}", if s.paid { "'paid'" } else { "'t2s'" });
            println!(
                "n_independent_echos={}",
                s.n_independent_echos
                    .map_or("None".into(), |v| v.to_string())
            );
            println!("n_threads={}", s.n_threads);
            println!("debug={}", b(s.debug));
            println!("quiet={}", b(s.quiet));
        }
    }
}
