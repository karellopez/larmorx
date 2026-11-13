// SPDX-License-Identifier: Apache-2.0
//! The candidate of `specs/freesurfer-maths.md` §23: larmorx's clean room of FreeSurfer's
//! mathematics (`larmorx-fs-core`'s `maths`) behind the commands and print forms of the
//! black-box probe `fs_maths_probe` (`validation/investigations/freesurfer-maths-probe/README.md`),
//! so that `compare.py compare` runs both under the same commands.
//!
//! `fs-maths-candidate batch <file>`: one result line per line of the file, as the probe prints
//! it (`<result> | x<standard output> x<standard error>`); each line runs on a process state of
//! its own (the probe's child process), `LARMORX_UPSTREAM_DEFECTS` chooses the mode, and the
//! notices of the defects a line met are added to its standard error (`upstream defect ...`).
//!
//! The commands are the crate's own test module `crates/larmorx-fs-core/tests/maths_cmd/`, which
//! the golden cases run too.

#[path = "../../../../../crates/larmorx-fs-core/tests/maths_cmd/mod.rs"]
mod maths_cmd;

use std::io::Write;
use std::process::ExitCode;

use larmorx_core::defects::UpstreamDefects;
use larmorx_fs_core::Environment;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 || args[1] != "batch" {
        eprintln!("usage: fs-maths-candidate batch <file>");
        return ExitCode::from(2);
    }
    let mode = UpstreamDefects::from_env().unwrap_or_default();
    let text = match std::fs::read(&args[2]) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("candidate: cannot open {}: {e}", args[2]);
            return ExitCode::from(2);
        }
    };
    let env = Environment::from_process();
    let mut out = std::io::BufWriter::new(std::io::stdout());
    let mut lines: Vec<&[u8]> = text.split(|&b| b == b'\n').collect();
    if text.ends_with(b"\n") {
        lines.pop();
    }
    for line in lines {
        let toks: Vec<String> = line
            .split(|&b| b == b' ' || b == b'\t')
            .filter(|t| !t.is_empty())
            .map(|t| String::from_utf8_lossy(t).into_owned())
            .collect();
        if toks.is_empty() {
            let _ = writeln!(out);
            continue;
        }
        let _ = writeln!(out, "{}", maths_cmd::run_line(&toks, mode, &env));
    }
    let _ = out.flush();
    ExitCode::SUCCESS
}
