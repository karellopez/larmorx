// SPDX-License-Identifier: GPL-3.0-or-later
// Part of larmorx-gpl (GNU GPL; see crates-gpl/LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The `larmorx-gpl` command line: bit-exact replicas of tools of the GPL licence family, with
//! the original programs' arguments (`larmorx-gpl <family> <tool> [original arguments]`):
//! today tedana's `t2smap` (`larmorx-gpl tedana t2smap ...`, GPL-3.0-only: the binary as a whole
//! is distributed under the GNU GPL version 3).
//!
//! It mirrors `larmorx` (the Apache-2.0 command line): the same family and tool names, its own
//! exit statuses 0 (success) and 2 (a usage error of this program), and the tool's own statuses
//! otherwise. D13's choice comes from `LARMORX_UPSTREAM_DEFECTS` (`fix`, the default, or
//! `reproduce`); the numpy and SciPy tedana's arithmetic follows from `LARMORX_TEDANA_LIBRARIES`
//! (`newest`, the default: numpy 2.5.4 and SciPy 1.18.1; `shared`: numpy 2.3.5 and SciPy 1.15.3).
#![forbid(unsafe_code)]

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use larmorx_core::defects::{EXIT_BAD_MODE, UpstreamDefects};
use larmorx_gpl_tedana::decay::Libraries;
use larmorx_gpl_tedana::workflow::Env;

const EXIT_USAGE: u8 = 2;
const EXIT_IO_ERROR: u8 = 1;

/// The version in PEP 440's form (`0.1.0rc1`; Cargo's is `0.1.0-rc.1`), as pip and PyPI show the
/// larmorx-gpl wheel's and `larmorx --version` prints larmorx's (build.rs).
const VERSION: &str = env!("LARMORX_GPL_PEP440_VERSION");

/// A tool: its name and the function that runs it with the original arguments.
type ToolFn = fn(&[String], &Env, &mut dyn Write, &mut dyn Write) -> u8;

const TEDANA_TOOLS: &[(&str, ToolFn)] = &[("t2smap", larmorx_gpl_tedana::t2smap)];

fn tool_names() -> String {
    TEDANA_TOOLS
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

fn libraries() -> Result<Libraries, String> {
    match std::env::var("LARMORX_TEDANA_LIBRARIES").ok().as_deref() {
        None | Some("" | "newest") => Ok(Libraries::Newest),
        Some("shared") => Ok(Libraries::Shared),
        Some(other) => Err(format!(
            "LARMORX_TEDANA_LIBRARIES={other:?}: expected newest or shared"
        )),
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
        .map_or_else(|| "larmorx-gpl".to_owned(), |a| program_name(a.as_ref()));
    let (code, written) = match args.next().as_ref().map(AsRef::as_ref) {
        Some("-V" | "--version") => (0, writeln!(out, "larmorx-gpl {VERSION}")),
        Some("-h" | "--help") => (0, out.write_all(usage(&prog).as_bytes())),
        None => (EXIT_USAGE, err.write_all(usage(&prog).as_bytes())),
        Some("tedana") => {
            let tool = args.next().map(|a| a.as_ref().to_owned());
            let rest: Vec<String> = args.map(|a| a.as_ref().to_owned()).collect();
            match tool.as_deref() {
                Some(tool) => match TEDANA_TOOLS.iter().find(|(n, _)| *n == tool) {
                    Some((_, f)) => match (UpstreamDefects::from_env(), libraries()) {
                        (Ok(mode), Ok(libraries)) => {
                            let env = Env {
                                columns: std::env::var("COLUMNS").ok(),
                                terminal_columns: None,
                                mode,
                                libraries,
                            };
                            (f(&rest, &env, out, err), Ok(()))
                        }
                        (Err(e), _) => (EXIT_BAD_MODE, writeln!(err, "Error: {e}")),
                        (_, Err(e)) => (EXIT_USAGE, writeln!(err, "Error: {e}")),
                    },
                    None => (
                        EXIT_USAGE,
                        writeln!(
                            err,
                            "error: unknown tedana tool '{tool}' (available: {})\n\nRun '{prog} \
                             --help' for usage.",
                            tool_names()
                        ),
                    ),
                },
                None => (
                    EXIT_USAGE,
                    writeln!(
                        err,
                        "error: missing tedana tool (available: {})",
                        tool_names()
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
        "larmorx-gpl {version}: bit-exact replicas of GPL-family tools, with the original command
lines (GNU General Public License version 3; see LICENSE and docs/licensing.md)

Usage: {prog} <family> <tool> [original arguments]
       {prog} --version | --help

Tools:
  tedana    {tedana}

Options:
  -h, --help     Print this help
  -V, --version  Print the version

Environment:
  LARMORX_UPSTREAM_DEFECTS  fix (default): the originals' defects corrected; reproduce: their
                            results bit for bit
  LARMORX_TEDANA_LIBRARIES  newest (default): numpy 2.5.4 and SciPy 1.18.1's arithmetic;
                            shared: numpy 2.3.5 and SciPy 1.15.3's
",
        version = VERSION,
        tedana = tool_names(),
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
    fn version_help_and_errors() {
        let (c, o, _) = run_capture(&["larmorx-gpl", "--version"]);
        assert_eq!((c, o), (0, format!("larmorx-gpl {VERSION}\n")));
        let (c, o, _) = run_capture(&["larmorx-gpl", "-h"]);
        assert_eq!(
            (c, o.starts_with(&format!("larmorx-gpl {VERSION}: "))),
            (0, true)
        );
        let (c, _, e) = run_capture(&["larmorx-gpl", "tedana", "nope"]);
        assert_eq!(c, EXIT_USAGE);
        assert!(e.contains("unknown tedana tool 'nope'"));
        let (c, o, _) = run_capture(&["larmorx-gpl", "tedana", "t2smap", "-v"]);
        assert_eq!((c, o.as_str()), (0, "t2smap v26.0.3\n"));
    }

    /// The version is Cargo's in PEP 440's form, the wheel's: the pre-release `-alpha.N`,
    /// `-beta.N` or `-rc.N` written `aN`, `bN` or `rcN` (checked here independently of build.rs).
    #[test]
    fn version_in_pep440_form() {
        let cargo = env!("CARGO_PKG_VERSION");
        let expected = cargo
            .replacen("-alpha.", "a", 1)
            .replacen("-beta.", "b", 1)
            .replacen("-rc.", "rc", 1);
        assert_eq!(VERSION, expected, "Cargo's version {cargo}");
        assert!(!VERSION.contains('-'), "{VERSION}");
    }
}
