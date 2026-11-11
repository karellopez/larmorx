// SPDX-License-Identifier: GPL-3.0-or-later
// Part of larmorx-gpl (GNU GPL; see crates-gpl/LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The `larmorx-gpl` command line: bit-exact replicas of tools of the GPL licence family, with
//! the original programs' arguments (`larmorx-gpl <family> <tool> [original arguments]`):
//! today tedana's `t2smap` (`larmorx-gpl tedana t2smap ...`, GPL-3.0-only: the binary as a whole
//! is distributed under the GNU GPL version 3).
//!
//! It mirrors `larmorx` (the Apache-2.0 command line): the same family and tool names, its own
//! exit statuses 0 (success) and 2 (a usage error of this program), and the tool's own statuses
//! otherwise. larmorx's own options stand before the family, as `larmorx`'s do (`larmorx-gpl
//! [--n-threads N] [--upstream-defects fix|reproduce] [--implementation replica] tedana <tool>
//! ...`, the user's decision of 2026-11-11, parsed by [`larmorx_core::cli::parse`]); each one
//! given wins over the variable it replaces: D13's choice over `LARMORX_UPSTREAM_DEFECTS` (`fix`,
//! the default, or `reproduce`), the thread count over `OMP_NUM_THREADS` (0, the default: every
//! logical CPU; the results do not depend on it). A tool whose original has its own thread
//! option (t2smap's `--n-threads`, after the tool) uses it when it is given, else the thread
//! count. `--implementation` takes `replica` only, which changes nothing
//! ([`larmorx_core::cli::ReplicaOnly`]). `larmorx` passes its options to this program through
//! those variables, never as options. The numpy and SciPy tedana's arithmetic follows from
//! `LARMORX_TEDANA_LIBRARIES` (`newest`, the default: numpy 2.5.4 and SciPy 1.18.1; `shared`:
//! numpy 2.3.5 and SciPy 1.15.3).
#![forbid(unsafe_code)]

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use larmorx_core::cli::{self, Overrides, ReplicaOnly};
use larmorx_core::defects::EXIT_BAD_MODE;
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

/// The tools whose originals have their own thread option, which wins when it is given (else
/// the thread count applies), as `--help` lists them.
const OWN_THREAD_OPTIONS: &str = "tedana t2smap --n-threads";

/// The thread count: `--n-threads` before the family, else `OMP_NUM_THREADS` (0, every logical
/// CPU, when unset or not a number).
fn threads(given: &Overrides) -> usize {
    given.n_threads_or(|| cli::omp_num_threads(std::env::var("OMP_NUM_THREADS").ok().as_deref()))
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
    run_tools(args, TEDANA_TOOLS, out, err)
}

/// [`run`] with the `tedana` family's `tools` (the tests' probes).
fn run_tools<I, S>(
    args: I,
    tools: &[(&str, ToolFn)],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut args = args.into_iter();
    let prog = args
        .next()
        .map_or_else(|| "larmorx-gpl".to_owned(), |a| program_name(a.as_ref()));
    let all: Vec<String> = args.map(|a| a.as_ref().to_owned()).collect();
    let (given, k) = match cli::parse::<ReplicaOnly>(&all) {
        Ok(parsed) => parsed,
        Err(message) => return usage_error(&prog, &message, err),
    };
    let given = given.overrides();
    let mut args = all.into_iter().skip(k);
    let (code, written) = match args.next().as_deref() {
        Some("-V" | "--version") => (0, writeln!(out, "larmorx-gpl {VERSION}")),
        Some("-h" | "--help") => (0, out.write_all(usage(&prog).as_bytes())),
        None => (EXIT_USAGE, err.write_all(usage(&prog).as_bytes())),
        Some("tedana") => {
            let tool = args.next();
            let rest: Vec<String> = args.collect();
            match tool.as_deref() {
                Some(tool) if let Some((name, _)) = cli::option(tool) => (
                    EXIT_USAGE,
                    writeln!(
                        err,
                        "error: {name} after the family: {prog}'s own options go before it \
                         ({prog} {name} ... tedana <tool> ...); after the tool, an option is the \
                         tool's\n\nRun '{prog} --help' for usage."
                    ),
                ),
                Some(tool) => match tools.iter().find(|(n, _)| *n == tool) {
                    Some((_, f)) => match (given.upstream_defects(), libraries()) {
                        (Ok(mode), Ok(libraries)) => {
                            let env = Env {
                                columns: std::env::var("COLUMNS").ok(),
                                terminal_columns: None,
                                mode,
                                libraries,
                                threads: threads(&given),
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

/// The usage line with larmorx's own options.
fn usage_line(prog: &str) -> String {
    format!(
        "Usage: {prog} [--n-threads N] [--upstream-defects fix|reproduce] [--implementation \
         replica] <family> <tool> [original arguments]"
    )
}

/// A usage error of larmorx's own options: `error: <message>`, the usage line, a pointer to
/// `--help`; exit [`EXIT_USAGE`] (as `larmorx`'s).
fn usage_error(prog: &str, message: &str, err: &mut dyn Write) -> u8 {
    let written = writeln!(
        err,
        "error: {message}\n\n{}\nRun '{prog} --help' for usage.",
        usage_line(prog)
    );
    if written.is_ok() {
        EXIT_USAGE
    } else {
        EXIT_IO_ERROR
    }
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

Usage: {prog} [options] <family> <tool> [original arguments]
       {prog} --version | --help

Tools:
  tedana    {tedana}

Options (before the family; each one given wins over the environment variable it replaces):
  --n-threads N            threads, 0 = every logical CPU (default); replaces OMP_NUM_THREADS
  --upstream-defects MODE  fix (default) or reproduce; replaces LARMORX_UPSTREAM_DEFECTS
  --implementation replica changes nothing: this program runs the replicas only (auto and
                           clean-room are larmorx's choice: larmorx --implementation ...)
  -h, --help               Print this help
  -V, --version            Print the version
  A tool whose original has its own thread option uses it when it is given, else --n-threads:
  {own}. The results never depend on the thread count.

Environment:
  LARMORX_UPSTREAM_DEFECTS  fix (default): the originals' defects corrected; reproduce: their
                            results bit for bit
  OMP_NUM_THREADS           threads (default: all logical CPUs); results do not depend on it
  LARMORX_TEDANA_LIBRARIES  newest (default): numpy 2.5.4 and SciPy 1.18.1's arithmetic;
                            shared: numpy 2.3.5 and SciPy 1.15.3's
",
        version = VERSION,
        tedana = tool_names(),
        own = OWN_THREAD_OPTIONS,
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

    /// What the probe tool received: the thread count and D13's mode.
    static PROBED: std::sync::Mutex<Vec<(usize, &'static str)>> = std::sync::Mutex::new(Vec::new());

    fn probe(_: &[String], env: &Env, out: &mut dyn Write, _: &mut dyn Write) -> u8 {
        PROBED.lock().unwrap().push((env.threads, env.mode.name()));
        let _ = writeln!(out, "probe {} {}", env.threads, env.mode.name());
        0
    }

    fn run_probe(args: &[&str]) -> (u8, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_tools(args, &[("probe", probe)], &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    /// larmorx's own options before the family reach the tool, over their variables; after
    /// the tool, `--n-threads` is the tool's (t2smap's own).
    #[test]
    fn the_options_reach_the_tool() {
        for (args, expected) in [
            (
                &[
                    "larmorx-gpl",
                    "--n-threads",
                    "5",
                    "--upstream-defects",
                    "reproduce",
                ][..],
                "probe 5 reproduce\n",
            ),
            (
                &["larmorx-gpl", "--upstream-defects=fix", "--n-threads=3"][..],
                "probe 3 fix\n",
            ),
            (
                &[
                    "larmorx-gpl",
                    "--implementation",
                    "replica",
                    "--n-threads",
                    "0",
                ][..],
                "probe 0 ",
            ),
        ] {
            let mut full = args.to_vec();
            full.extend(["tedana", "probe", "--n-threads", "9"]);
            let (code, out, err) = run_probe(&full);
            assert_eq!((code, err.as_str()), (0, ""), "{full:?}");
            assert!(out.starts_with(expected), "{full:?}: {out}");
        }
        let probed = PROBED.lock().unwrap().clone();
        assert!(probed.contains(&(5, "reproduce")), "{probed:?}");
        assert!(probed.contains(&(3, "fix")), "{probed:?}");
    }

    #[test]
    fn wrong_options_are_usage_errors() {
        let usage = "\n\nUsage: larmorx-gpl [--n-threads N] [--upstream-defects fix|reproduce] \
                     [--implementation replica] <family> <tool> [original arguments]\nRun \
                     'larmorx-gpl --help' for usage.\n";
        for (args, message) in [
            (
                &["--n-threads", "1.5", "tedana", "probe"][..],
                "--n-threads must be a whole number of threads, 0 for every logical CPU, not '1.5'",
            ),
            (
                &["--upstream-defects"][..],
                "--upstream-defects needs a value: fix or reproduce",
            ),
            (
                &["--implementation=x", "tedana", "probe"][..],
                "--implementation must be replica, not 'x'",
            ),
        ] {
            let mut full = vec!["larmorx-gpl"];
            full.extend(args);
            let (code, out, err) = run_probe(&full);
            assert_eq!((code, out.as_str()), (EXIT_USAGE, ""), "{full:?}");
            assert_eq!(err, format!("error: {message}{usage}"), "{full:?}");
        }
        for refused in ["auto", "clean-room", "original"] {
            let (code, _, err) = run_probe(&[
                "larmorx-gpl",
                "--implementation",
                refused,
                "tedana",
                "probe",
            ]);
            assert_eq!(code, EXIT_USAGE, "{refused}");
            assert!(
                err.starts_with(&format!(
                    "error: --implementation {refused}: this program runs the bit-exact replicas \
                     only"
                )),
                "{err}"
            );
        }
        let (code, _, err) = run_probe(&["larmorx-gpl", "tedana", "--upstream-defects", "fix"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(
            err.starts_with(
                "error: --upstream-defects after the family: larmorx-gpl's own options go before \
                 it (larmorx-gpl --upstream-defects ... tedana <tool> ...)"
            ),
            "{err}"
        );
    }

    #[test]
    fn help_lists_the_options() {
        let (code, out, _) = run_capture(&["larmorx-gpl", "--help"]);
        assert_eq!(code, 0);
        for line in [
            "Usage: larmorx-gpl [options] <family> <tool> [original arguments]\n",
            "  --n-threads N            threads, 0 = every logical CPU (default); replaces OMP_NUM_THREADS\n",
            "  --upstream-defects MODE  fix (default) or reproduce; replaces LARMORX_UPSTREAM_DEFECTS\n",
            "  --implementation replica changes nothing: this program runs the replicas only (auto and\n",
            "  tedana t2smap --n-threads. The results never depend on the thread count.\n",
            "  OMP_NUM_THREADS           threads (default: all logical CPUs); results do not depend on it\n",
        ] {
            assert!(out.contains(line), "{line}\n{out}");
        }
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
