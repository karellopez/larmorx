// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! The `larmorx-gpl` command line: the bit-exact replicas of GPL-licensed tools, with the
//! original programs' arguments (`larmorx-gpl <family> <tool> [original arguments]`).
//!
//! It mirrors `larmorx` (the Apache-2.0 command line): the same families, tool names, exit
//! codes (0 success, 1 the tool failed, 2 a usage error) and messages. The two are separate
//! programs so that the licences stay separate (`docs/licensing.md`).
#![forbid(unsafe_code)]

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

/// Exit code for invalid usage (unknown command or option).
const EXIT_USAGE: u8 = 2;

/// Exit code when the output streams cannot be written.
const EXIT_IO_ERROR: u8 = 1;

/// Runs the command line with `args` (program name first), writing to `out` and `err`.
/// Returns the process exit code.
fn run<I, S>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut args = args.into_iter();
    let prog = args
        .next()
        .map_or_else(|| "larmorx-gpl".to_owned(), |a| program_name(a.as_ref()));
    let (code, written) = match args.next().as_ref().map(AsRef::as_ref) {
        Some("-V" | "--version") => (
            0,
            writeln!(out, "larmorx-gpl {}", env!("CARGO_PKG_VERSION")),
        ),
        Some("-h" | "--help") => (0, out.write_all(usage(&prog).as_bytes())),
        None => (EXIT_USAGE, err.write_all(usage(&prog).as_bytes())),
        Some("afni") => {
            let tool = args.next().map(|a| a.as_ref().to_owned());
            let rest: Vec<String> = args.map(|a| a.as_ref().to_owned()).collect();
            match tool.as_deref() {
                Some(tool) => match larmorx_gpl_afni::cli::run(tool, &rest, out, err) {
                    Some(code) => (code, Ok(())),
                    None => (
                        EXIT_USAGE,
                        writeln!(
                            err,
                            "error: unknown afni tool '{tool}' (available: {})\n\nRun '{prog} --help' for usage.",
                            larmorx_gpl_afni::cli::TOOLS.join(", ")
                        ),
                    ),
                },
                None => (
                    EXIT_USAGE,
                    writeln!(
                        err,
                        "error: missing afni tool (available: {})",
                        larmorx_gpl_afni::cli::TOOLS.join(", ")
                    ),
                ),
            }
        }
        Some(arg) => {
            let kind = if arg.starts_with('-') {
                "option"
            } else {
                "command"
            };
            (
                EXIT_USAGE,
                writeln!(
                    err,
                    "error: unknown {kind} '{arg}'\n\nRun '{prog} --help' for usage."
                ),
            )
        }
    };
    if written.is_ok() { code } else { EXIT_IO_ERROR }
}

/// The name the program was invoked as, without directory or `.exe` suffix.
fn program_name(argv0: &str) -> String {
    Path::new(argv0)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("larmorx-gpl")
        .to_owned()
}

fn usage(prog: &str) -> String {
    format!(
        "larmorx-gpl {version}: bit-exact replicas of GPL-licensed neuroimaging tools, with the
original command lines (GNU GPL version 3 or later; see docs/licensing.md)

Usage: {prog} <family> <tool> [original arguments]
       {prog} --version | --help

Tools:
  afni   {afni}

Options:
  -h, --help     Print this help
  -V, --version  Print the version
",
        version = env!("CARGO_PKG_VERSION"),
        afni = larmorx_gpl_afni::cli::TOOLS.join(", "),
    )
}

fn main() -> ExitCode {
    let args = std::env::args_os().map(|a| a.to_string_lossy().into_owned());
    ExitCode::from(run(
        args,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_capture(args: &[&str]) -> (u8, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(args, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn version_and_help() {
        let (code, out, _) = run_capture(&["larmorx-gpl", "--version"]);
        assert_eq!(code, 0);
        assert_eq!(out, format!("larmorx-gpl {}\n", env!("CARGO_PKG_VERSION")));
        let (code, out, _) = run_capture(&["dir/larmorx-gpl.exe", "-h"]);
        assert_eq!(code, 0);
        assert!(out.contains("Usage: larmorx-gpl <family> <tool>"), "{out}");
        assert!(out.contains("afni   3dTshift"), "{out}");
        let (code, out, err) = run_capture(&["larmorx-gpl"]);
        assert_eq!((code, out.as_str()), (EXIT_USAGE, ""));
        assert!(err.contains("Usage:"), "{err}");
    }

    #[test]
    fn afni_tools() {
        let (code, out, _) = run_capture(&["larmorx-gpl", "afni", "3dTshift"]);
        assert_eq!(code, 0);
        assert!(out.starts_with("Usage: 3dTshift"), "{out}");
        let (code, _, err) = run_capture(&["larmorx-gpl", "afni", "3dTshift", "-bogus", "x.nii"]);
        assert_eq!(code, 1);
        assert!(err.contains("Unknown option: -bogus"), "{err}");
        let (code, _, err) = run_capture(&["larmorx-gpl", "afni", "3dvolreg"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(
            err.starts_with("error: unknown afni tool '3dvolreg'"),
            "{err}"
        );
        let (code, _, err) = run_capture(&["larmorx-gpl", "fsl", "bet"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(err.starts_with("error: unknown command 'fsl'"), "{err}");
    }
}
