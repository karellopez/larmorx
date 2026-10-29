// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The `larmorx-nc` command line: the bit-exact replicas of FSL tools, with the original
//! programs' arguments (`larmorx-nc fsl <tool> [original arguments]`).
//!
//! It mirrors `larmorx` (the Apache-2.0 command line) and `larmorx-gpl`: the same family and
//! tool names, its own exit codes 0 (success) and 2 (a usage error of this program), and the
//! tool's own statuses otherwise. The programs are separate so that the licences stay separate
//! (`docs/licensing.md`).
//!
//! Each tool reads, as the original does, its environment variables (`FSLOUTPUTTYPE`, ...) once
//! into a table of settings; the thread count comes from `OMP_NUM_THREADS` (the results do not
//! depend on it), and D13's choice from `LARMORX_UPSTREAM_DEFECTS` (`fix`, the default, or
//! `reproduce`).
#![forbid(unsafe_code)]

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use larmorx_core::defects::{EXIT_BAD_MODE, UpstreamDefects};
use larmorx_nc_core::settings::{FSL_VARIABLES, Settings};

/// Exit code for invalid usage (unknown command or option).
const EXIT_USAGE: u8 = 2;

/// Exit code when the output streams cannot be written.
const EXIT_IO_ERROR: u8 = 1;

/// A tool of the `fsl` family: its name and the function that runs it with the original
/// arguments, the settings, D13's mode and the thread count.
type ToolFn =
    fn(&[String], &Settings, UpstreamDefects, usize, &mut dyn Write, &mut dyn Write) -> u8;

/// The FSL tools this program replicates. Add one line per tool.
const FSL_TOOLS: &[(&str, ToolFn)] = &[
    ("mcflirt", larmorx_nc_mcflirt::mcflirt),
    ("bet", larmorx_nc_bet::bet),
    ("bet2", larmorx_nc_bet::bet2),
    ("betsurf", larmorx_nc_bet::betsurf),
];

fn tool_names() -> String {
    FSL_TOOLS
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The thread count of `OMP_NUM_THREADS` (0, all logical CPUs, when unset or not a number).
fn threads() -> usize {
    std::env::var("OMP_NUM_THREADS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// Runs the command line with `args` (program name first), writing to `out` and `err`.
/// FSL-60: the image names FSL 6.0.7.23's NewNifti 5.0.0 would have opened under another name
/// (a name holding ".img" before its extension), one notice per run.
fn renamed_notices(settings: &Settings, mode: UpstreamDefects, err: &mut dyn Write) {
    let renames = settings.renames();
    if let Some((given, renamed)) = renames.first() {
        let detail = if mode.reproduce() {
            format!(
                "{given} opened as {renamed}, as FSL 6.0.7.23 opens it (NewNifti replaces a \
                 name's last '.img' with '.hdr'){}",
                if renames.len() > 1 {
                    ", and other names alike"
                } else {
                    ""
                }
            )
        } else {
            format!(
                "{given} opened as given; FSL 6.0.7.23 opens {renamed} instead (NewNifti \
                 replaces a name's last '.img' with '.hdr'){}",
                if renames.len() > 1 {
                    ", and other names alike"
                } else {
                    ""
                }
            )
        };
        let n = larmorx_core::defects::Notice::new(&larmorx_core::defects::FSL_60, mode, detail);
        let _ = writeln!(err, "larmorx-nc: {}", n.line());
    }
}

fn run<I, S>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut args = args.into_iter();
    let prog = args
        .next()
        .map_or_else(|| "larmorx-nc".to_owned(), |a| program_name(a.as_ref()));
    let (code, written) = match args.next().as_ref().map(AsRef::as_ref) {
        Some("-V" | "--version") => (0, writeln!(out, "larmorx-nc {}", env!("CARGO_PKG_VERSION"))),
        Some("-h" | "--help") => (0, out.write_all(usage(&prog).as_bytes())),
        None => (EXIT_USAGE, err.write_all(usage(&prog).as_bytes())),
        Some("fsl") => {
            let tool = args.next().map(|a| a.as_ref().to_owned());
            let rest: Vec<String> = args.map(|a| a.as_ref().to_owned()).collect();
            match tool.as_deref() {
                Some(tool) => match FSL_TOOLS.iter().find(|(n, _)| *n == tool) {
                    Some((_, f)) => match UpstreamDefects::from_env() {
                        Ok(mode) => {
                            let settings = Settings::from_env(FSL_VARIABLES)
                                .reproducing_renames(mode.reproduce());
                            let code = f(&rest, &settings, mode, threads(), out, err);
                            renamed_notices(&settings, mode, err);
                            (code, Ok(()))
                        }
                        Err(e) => (EXIT_BAD_MODE, writeln!(err, "Error: {e}")),
                    },
                    None => (
                        EXIT_USAGE,
                        writeln!(
                            err,
                            "error: unknown fsl tool '{tool}' (available: {})\n\nRun '{prog} --help' for usage.",
                            tool_names()
                        ),
                    ),
                },
                None => (
                    EXIT_USAGE,
                    writeln!(err, "error: missing fsl tool (available: {})", tool_names()),
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
        .unwrap_or("larmorx-nc")
        .to_owned()
}

fn usage(prog: &str) -> String {
    format!(
        "larmorx-nc {version}: bit-exact replicas of FSL tools, with the original command lines
(FSL Licence: non-commercial use only; see LICENSE and docs/licensing.md)

Usage: {prog} <family> <tool> [original arguments]
       {prog} --version | --help

Tools:
  fsl    {fsl}

Options:
  -h, --help     Print this help
  -V, --version  Print the version

Environment:
  LARMORX_UPSTREAM_DEFECTS  fix (default): the originals' defects corrected; reproduce: their
                            results bit for bit
  OMP_NUM_THREADS           threads (default: all logical CPUs); results do not depend on it
",
        version = env!("CARGO_PKG_VERSION"),
        fsl = tool_names(),
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
        let (code, out, _) = run_capture(&["larmorx-nc", "--version"]);
        assert_eq!(code, 0);
        assert_eq!(out, format!("larmorx-nc {}\n", env!("CARGO_PKG_VERSION")));
        let (code, out, _) = run_capture(&["dir/larmorx-nc.exe", "-h"]);
        assert_eq!(code, 0);
        assert!(out.contains("Usage: larmorx-nc <family> <tool>"), "{out}");
        assert!(out.contains("fsl    mcflirt, bet, bet2, betsurf"), "{out}");
        let (code, out, err) = run_capture(&["larmorx-nc"]);
        assert_eq!((code, out.as_str()), (EXIT_USAGE, ""));
        assert!(err.contains("Usage:"), "{err}");
    }

    #[test]
    fn unknown_tools() {
        let (code, _, err) = run_capture(&["larmorx-nc", "fsl", "flirt"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(err.starts_with("error: unknown fsl tool 'flirt'"), "{err}");
        let (code, _, err) = run_capture(&["larmorx-nc", "afni", "3dTshift"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(err.starts_with("error: unknown command 'afni'"), "{err}");
        let (code, out, _) = run_capture(&["larmorx-nc", "fsl", "mcflirt"]);
        assert_eq!(code, 1);
        assert!(out.starts_with("Usage: mcflirt"), "{out}");
    }
}
