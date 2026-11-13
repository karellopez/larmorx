// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The `larmorx-freesurfer` command line: bit-exact replicas of FreeSurfer 8.2.0's programs, with
//! the original programs' arguments (`larmorx-freesurfer freesurfer <program> [original
//! arguments]`), distributed under FreeSurfer's licence (`crates-freesurfer/LICENSE`).
//!
//! It mirrors `larmorx` (the Apache-2.0 command line) as `larmorx-nc` and `larmorx-gpl` do: the
//! family `freesurfer` and the original program's name (`larmorx freesurfer <program>` reaches the
//! same program), its own exit statuses 0 (success) and 2 (a usage error of this program), and
//! the program's own statuses otherwise. larmorx's own options stand before the family
//! (`larmorx-freesurfer [--n-threads N] [--upstream-defects fix|reproduce] [--implementation
//! replica] freesurfer <program> ...`, the user's decision of 2026-11-11, parsed by
//! [`larmorx_core::cli::parse`]); each one given wins over the variable it replaces: D13's choice
//! over `LARMORX_UPSTREAM_DEFECTS` (`fix`, the default, or `reproduce`), the thread count over
//! `OMP_NUM_THREADS` (0, the default: every logical CPU; the results do not depend on it). A
//! program whose original has its own thread option uses it when it is given, else the thread
//! count. `--implementation` takes `replica` only, which changes nothing
//! ([`larmorx_core::cli::ReplicaOnly`]). `larmorx` passes its options to this program through
//! those variables, never as options.
//!
//! The programs come from the program crates' registries ([`REGISTRIES`]: one per wave of
//! `docs/freesurfer/waves.md`); queue item 3.1-01 made the program with none.
#![forbid(unsafe_code)]

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use larmorx_core::cli::{self, Overrides, ReplicaOnly};
use larmorx_core::defects::EXIT_BAD_MODE;
use larmorx_freesurfer_core::run::{Program, Settings};

const EXIT_USAGE: u8 = 2;
const EXIT_IO_ERROR: u8 = 1;

/// The family name, as `larmorx` names it.
const FAMILY: &str = "freesurfer";

/// The version in PEP 440's form (`0.1.0rc1`; Cargo's is `0.1.0-rc.1`), as pip and PyPI show the
/// larmorx-freesurfer wheel's and `larmorx --version` prints larmorx's (build.rs).
const VERSION: &str = env!("LARMORX_FREESURFER_PEP440_VERSION");

/// The program crates' registries, in the waves' order.
const REGISTRIES: &[&[Program]] = &[
    larmorx_freesurfer_volumes::PROGRAMS,
    larmorx_freesurfer_atlas::PROGRAMS,
    larmorx_freesurfer_topology::PROGRAMS,
    larmorx_freesurfer_cortex::PROGRAMS,
    larmorx_freesurfer_classic::PROGRAMS,
    larmorx_freesurfer_networks::PROGRAMS,
    larmorx_freesurfer_recon::PROGRAMS,
];

/// Every program, in the waves' order.
fn programs() -> Vec<Program> {
    REGISTRIES.iter().flat_map(|r| r.iter().copied()).collect()
}

fn program_names(programs: &[Program]) -> String {
    if programs.is_empty() {
        return "none yet".to_owned();
    }
    programs
        .iter()
        .map(|p| p.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The programs whose originals have their own thread option, which wins when it is given (else
/// the thread count applies), as `--help` lists them.
const OWN_THREAD_OPTIONS: &str = "none yet";

/// The thread count: `--n-threads` before the family, else `OMP_NUM_THREADS` (0, every logical
/// CPU, when unset or not a number).
fn threads(given: &Overrides) -> usize {
    given.n_threads_or(|| cli::omp_num_threads(std::env::var("OMP_NUM_THREADS").ok().as_deref()))
}

fn run<I, S>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    run_programs(args, &programs(), out, err)
}

/// [`run`] with the given `programs` (the tests' probes).
fn run_programs<I, S>(args: I, programs: &[Program], out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut args = args.into_iter();
    let prog = args.next().map_or_else(
        || "larmorx-freesurfer".to_owned(),
        |a| program_name(a.as_ref()),
    );
    let all: Vec<String> = args.map(|a| a.as_ref().to_owned()).collect();
    let (given, k) = match cli::parse::<ReplicaOnly>(&all) {
        Ok(parsed) => parsed,
        Err(message) => return usage_error(&prog, &message, err),
    };
    let given = given.overrides();
    let mut args = all.into_iter().skip(k);
    let (code, written) = match args.next().as_deref() {
        Some("-V" | "--version") => (0, writeln!(out, "larmorx-freesurfer {VERSION}")),
        Some("-h" | "--help") => (0, out.write_all(usage(&prog, programs).as_bytes())),
        None => (EXIT_USAGE, err.write_all(usage(&prog, programs).as_bytes())),
        Some(FAMILY) => {
            let program = args.next();
            let rest: Vec<String> = args.collect();
            match program.as_deref() {
                Some(arg) if let Some((name, _)) = cli::option(arg) => (
                    EXIT_USAGE,
                    writeln!(
                        err,
                        "error: {name} after the family: {prog}'s own options go before it \
                         ({prog} {name} ... {FAMILY} <program> ...); after the program, an \
                         option is the program's\n\nRun '{prog} --help' for usage."
                    ),
                ),
                Some(name) => match programs.iter().find(|p| p.name == name) {
                    Some(p) => match given.upstream_defects() {
                        Ok(mode) => {
                            let settings = Settings::of_process(threads(&given), mode);
                            ((p.run)(&rest, &settings, out, err), Ok(()))
                        }
                        Err(e) => (EXIT_BAD_MODE, writeln!(err, "Error: {e}")),
                    },
                    None => (
                        EXIT_USAGE,
                        writeln!(
                            err,
                            "error: unknown {FAMILY} program '{name}' (available: {})\n\nRun \
                             '{prog} --help' for usage.",
                            program_names(programs)
                        ),
                    ),
                },
                None => (
                    EXIT_USAGE,
                    writeln!(
                        err,
                        "error: missing {FAMILY} program (available: {})",
                        program_names(programs)
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
         replica] <family> <program> [original arguments]"
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
        .unwrap_or("larmorx-freesurfer")
        .to_owned()
}

fn usage(prog: &str, programs: &[Program]) -> String {
    format!(
        "larmorx-freesurfer {version}: bit-exact replicas of FreeSurfer 8.2.0's programs, with the
original command lines (FreeSurfer's licence: research use only; see LICENSE and
docs/licensing.md)

Usage: {prog} [options] <family> <program> [original arguments]
       {prog} --version | --help

Programs:
  {FAMILY}  {names}

Options (before the family; each one given wins over the environment variable it replaces):
  --n-threads N            threads, 0 = every logical CPU (default); replaces OMP_NUM_THREADS
  --upstream-defects MODE  fix (default) or reproduce; replaces LARMORX_UPSTREAM_DEFECTS
  --implementation replica changes nothing: this program runs the replicas only (auto and
                           clean-room are larmorx's choice: larmorx --implementation ...)
  -h, --help               Print this help
  -V, --version            Print the version
  A program whose original has its own thread option uses it when it is given, else
  --n-threads: {own}. The results never depend on the thread count.

Environment:
  LARMORX_UPSTREAM_DEFECTS  fix (default): the originals' defects corrected; reproduce: their
                            results bit for bit
  OMP_NUM_THREADS           threads (default: all logical CPUs); results do not depend on it
",
        version = VERSION,
        names = program_names(programs),
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
        let (c, o, _) = run_capture(&["larmorx-freesurfer", "--version"]);
        assert_eq!((c, o), (0, format!("larmorx-freesurfer {VERSION}\n")));
        let (c, o, _) = run_capture(&["larmorx-freesurfer", "-h"]);
        assert_eq!(
            (c, o.starts_with(&format!("larmorx-freesurfer {VERSION}: "))),
            (0, true)
        );
        let (c, o, e) = run_capture(&["larmorx-freesurfer"]);
        assert_eq!((c, o.as_str()), (EXIT_USAGE, ""));
        assert!(e.contains("Usage: larmorx-freesurfer [options]"), "{e}");
        let (c, _, e) = run_capture(&["larmorx-freesurfer", "freesurfer", "nope"]);
        assert_eq!(c, EXIT_USAGE);
        assert!(
            e.starts_with("error: unknown freesurfer program 'nope'"),
            "{e}"
        );
        let (c, _, e) = run_capture(&["larmorx-freesurfer", "freesurfer"]);
        assert_eq!(c, EXIT_USAGE);
        assert!(e.starts_with("error: missing freesurfer program"), "{e}");
        let (c, _, e) = run_capture(&["larmorx-freesurfer", "fsl", "bet"]);
        assert_eq!(c, EXIT_USAGE);
        assert!(e.starts_with("error: unknown command 'fsl'"), "{e}");
        let (c, _, e) = run_capture(&["larmorx-freesurfer", "--nope"]);
        assert_eq!(c, EXIT_USAGE);
        assert!(e.starts_with("error: unknown option '--nope'"), "{e}");
    }

    /// Every registered program has a distinct name.
    #[test]
    fn program_names_are_unique() {
        let all = programs();
        let mut names: Vec<_> = all.iter().map(|p| p.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len());
    }

    /// What the probe program received: the thread count and D13's mode.
    static PROBED: std::sync::Mutex<Vec<(usize, &'static str, Vec<String>)>> =
        std::sync::Mutex::new(Vec::new());

    fn probe(args: &[String], s: &Settings, out: &mut dyn Write, _: &mut dyn Write) -> u8 {
        PROBED
            .lock()
            .unwrap()
            .push((s.threads, s.mode.name(), args.to_vec()));
        let _ = writeln!(out, "probe {} {}", s.threads, s.mode.name());
        7
    }

    const PROBE: &[Program] = &[Program {
        name: "probe",
        run: probe,
    }];

    fn run_probe(args: &[&str]) -> (u8, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_programs(args, PROBE, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    /// larmorx's own options before the family reach the program, over their variables; after
    /// the program, every option is the program's; its exit status passes through.
    #[test]
    fn the_options_reach_the_program() {
        for (args, expected) in [
            (
                &[
                    "larmorx-freesurfer",
                    "--n-threads",
                    "5",
                    "--upstream-defects",
                    "reproduce",
                ][..],
                "probe 5 reproduce\n",
            ),
            (
                &[
                    "larmorx-freesurfer",
                    "--upstream-defects=fix",
                    "--n-threads=3",
                ][..],
                "probe 3 fix\n",
            ),
            (
                &[
                    "larmorx-freesurfer",
                    "--implementation",
                    "replica",
                    "--n-threads",
                    "0",
                ][..],
                "probe 0 ",
            ),
        ] {
            let mut full = args.to_vec();
            full.extend(["freesurfer", "probe", "--n-threads", "9", "-openmp", "4"]);
            let (code, out, err) = run_probe(&full);
            assert_eq!((code, err.as_str()), (7, ""), "{full:?}");
            assert!(out.starts_with(expected), "{full:?}: {out}");
        }
        let probed = PROBED.lock().unwrap().clone();
        let tail = ["--n-threads", "9", "-openmp", "4"]
            .map(String::from)
            .to_vec();
        assert!(
            probed.contains(&(5, "reproduce", tail.clone())),
            "{probed:?}"
        );
        assert!(probed.contains(&(3, "fix", tail)), "{probed:?}");
    }

    #[test]
    fn wrong_options_are_usage_errors() {
        let usage = "\n\nUsage: larmorx-freesurfer [--n-threads N] [--upstream-defects \
                     fix|reproduce] [--implementation replica] <family> <program> [original \
                     arguments]\nRun 'larmorx-freesurfer --help' for usage.\n";
        for (args, message) in [
            (
                &["--n-threads", "1.5", "freesurfer", "probe"][..],
                "--n-threads must be a whole number of threads, 0 for every logical CPU, not '1.5'",
            ),
            (
                &["--upstream-defects"][..],
                "--upstream-defects needs a value: fix or reproduce",
            ),
            (
                &["--implementation=x", "freesurfer", "probe"][..],
                "--implementation must be replica, not 'x'",
            ),
        ] {
            let mut full = vec!["larmorx-freesurfer"];
            full.extend(args);
            let (code, out, err) = run_probe(&full);
            assert_eq!((code, out.as_str()), (EXIT_USAGE, ""), "{full:?}");
            assert_eq!(err, format!("error: {message}{usage}"), "{full:?}");
        }
        for refused in ["auto", "clean-room", "original"] {
            let (code, _, err) = run_probe(&[
                "larmorx-freesurfer",
                "--implementation",
                refused,
                "freesurfer",
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
        let (code, _, err) = run_probe(&[
            "larmorx-freesurfer",
            "freesurfer",
            "--upstream-defects",
            "fix",
        ]);
        assert_eq!(code, EXIT_USAGE);
        assert!(
            err.starts_with(
                "error: --upstream-defects after the family: larmorx-freesurfer's own options go \
                 before it (larmorx-freesurfer --upstream-defects ... freesurfer <program> ...)"
            ),
            "{err}"
        );
    }

    #[test]
    fn help_lists_the_options() {
        let (code, out, _) = run_capture(&["larmorx-freesurfer", "--help"]);
        assert_eq!(code, 0);
        for line in [
            "Usage: larmorx-freesurfer [options] <family> <program> [original arguments]\n",
            "  --n-threads N            threads, 0 = every logical CPU (default); replaces OMP_NUM_THREADS\n",
            "  --upstream-defects MODE  fix (default) or reproduce; replaces LARMORX_UPSTREAM_DEFECTS\n",
            "  --implementation replica changes nothing: this program runs the replicas only (auto and\n",
            "  OMP_NUM_THREADS           threads (default: all logical CPUs); results do not depend on it\n",
        ] {
            assert!(out.contains(line), "{line}\n{out}");
        }
        let (_, probe_help, _) = {
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = run_programs(["larmorx-freesurfer", "--help"], PROBE, &mut out, &mut err);
            (code, String::from_utf8(out).unwrap(), err)
        };
        assert!(probe_help.contains("  freesurfer  probe\n"), "{probe_help}");
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
