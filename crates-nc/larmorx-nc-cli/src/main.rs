// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The `larmorx-nc` command line: the bit-exact replicas of FSL tools, with the original
//! programs' arguments (`larmorx-nc fsl <tool> [original arguments]`).
//!
//! It mirrors `larmorx` (the Apache-2.0 command line): the same family and
//! tool names, its own exit codes 0 (success) and 2 (a usage error of this program), and the
//! tool's own statuses otherwise. The programs are separate so that the licences stay separate
//! (`docs/licensing.md`).
//!
//! Each tool reads, as the original does, its environment variables (`FSLOUTPUTTYPE`, ...) once
//! into a table of settings. With `FSLOUTPUTTYPE` unset, FSL's programs stop with an error at
//! their first image (FSL-11): by default the table gives it `NIFTI_GZ` and the run ends with
//! one line naming FSL-11 when it wrote an image; `reproduce` keeps FSL's error. larmorx's own options stand before the family, as `larmorx`'s do
//! (`larmorx-nc [--n-threads N] [--upstream-defects fix|reproduce] [--implementation replica]
//! fsl <tool> ...`, the user's decision of 2026-11-11, parsed by [`larmorx_core::cli::parse`]);
//! each one given wins over the variable it replaces: the thread count over `OMP_NUM_THREADS`
//! (0, the default: every logical CPU; the results do not depend on it), D13's choice over
//! `LARMORX_UPSTREAM_DEFECTS` (`fix`, the default, or `reproduce`). `--implementation` takes
//! `replica` only, which changes nothing: this program runs the replicas
//! ([`larmorx_core::cli::ReplicaOnly`]). A tool whose original has its own thread option
//! (topup's `--nthr`) uses it when it is given, else the thread count. `larmorx` itself passes
//! its options to this program through its environment (those variables), never as options, so
//! an older `larmorx-nc` keeps working with it.
#![forbid(unsafe_code)]

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use larmorx_core::cli::{self, Overrides, ReplicaOnly};
use larmorx_core::defects::{EXIT_BAD_MODE, UpstreamDefects};
use larmorx_nc_core::settings::{FSL_VARIABLES, Settings};

/// Exit code for invalid usage (unknown command or option).
const EXIT_USAGE: u8 = 2;

/// Exit code when the output streams cannot be written.
const EXIT_IO_ERROR: u8 = 1;

/// The version in PEP 440's form (`0.1.0rc1`; Cargo's is `0.1.0-rc.1`), as pip and PyPI show the
/// larmorx-nc wheel's and `larmorx --version` prints larmorx's (build.rs).
const VERSION: &str = env!("LARMORX_NC_PEP440_VERSION");

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
    ("flirt", larmorx_nc_flirt::flirt),
    ("convert_xfm", larmorx_nc_flirt::convert_xfm),
    ("avscale", larmorx_nc_flirt::avscale),
    ("img2imgcoord", larmorx_nc_flirt::img2imgcoord),
    ("std2imgcoord", larmorx_nc_flirt::std2imgcoord),
    ("img2stdcoord", larmorx_nc_flirt::img2stdcoord),
    ("rmsdiff", larmorx_nc_flirt::rmsdiff),
    ("makerot", larmorx_nc_flirt::makerot),
    ("midtrans", larmorx_nc_flirt::midtrans),
    ("pointflirt", larmorx_nc_flirt::pointflirt),
    ("applyxfm4D", larmorx_nc_flirt::applyxfm4d),
    ("pairreg", larmorx_nc_flirt::pairreg),
    ("extracttxt", larmorx_nc_flirt::extracttxt),
    ("aff2rigid", larmorx_nc_flirt::aff2rigid),
    ("fast", larmorx_nc_fast::fast),
    ("topup", larmorx_nc_topup::topup),
    ("applytopup", larmorx_nc_topup::applytopup),
    ("fslmaths", larmorx_nc_avwutils::fslmaths),
    ("fslstats", larmorx_nc_avwutils::fslstats),
    ("fslroi", larmorx_nc_avwutils::fslroi),
    ("standard_space_roi", larmorx_nc_flirt::standard_space_roi),
    ("prelude", larmorx_nc_fugue::prelude),
];

fn tool_names() -> String {
    FSL_TOOLS
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The thread count: `--n-threads`, else `OMP_NUM_THREADS` (0, every logical CPU, when unset
/// or not a number).
fn threads(given: &Overrides) -> usize {
    given.n_threads_or(|| cli::omp_num_threads(std::env::var("OMP_NUM_THREADS").ok().as_deref()))
}

/// The tools whose originals have their own thread option, which wins when it is given (else
/// the thread count applies), as `--help` lists them.
const OWN_THREAD_OPTIONS: &str = "fsl topup --nthr";

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

/// FSL-11: the run wrote its images as `NIFTI_GZ` in place of the unset `FSLOUTPUTTYPE`
/// (FSL's program stops with an error at its first image): one line per command, in the words
/// of the clean-room originals' ([`larmorx_core::defects::fsl11_notice`]), whichever of the
/// tool's steps wrote them (`bet`'s bet2 runs, `standard_space_roi`'s flirt).
fn output_type_notice(settings: &Settings, tool: &str, err: &mut dyn Write) {
    if settings.output_type_defaulted() {
        let n = larmorx_core::defects::fsl11_notice(tool);
        let _ = writeln!(err, "larmorx-nc: {}", n.line());
    }
}

/// Runs the command line with `args` (program name first), writing to `out` and `err`.
fn run<I, S>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    run_tools(args, FSL_TOOLS, out, err)
}

/// [`run`] with the `fsl` family's `tools` (the tests' probes).
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
        .map_or_else(|| "larmorx-nc".to_owned(), |a| program_name(a.as_ref()));
    let all: Vec<String> = args.map(|a| a.as_ref().to_owned()).collect();
    let (given, k) = match cli::parse::<ReplicaOnly>(&all) {
        Ok(parsed) => parsed,
        Err(message) => return usage_error(&prog, &message, err),
    };
    let given = given.overrides();
    let mut args = all.into_iter().skip(k);
    let (code, written) = match args.next().as_deref() {
        Some("-V" | "--version") => (0, writeln!(out, "larmorx-nc {VERSION}")),
        Some("-h" | "--help") => (0, out.write_all(usage(&prog).as_bytes())),
        None => (EXIT_USAGE, err.write_all(usage(&prog).as_bytes())),
        Some("fsl") => {
            let tool = args.next();
            let rest: Vec<String> = args.collect();
            match tool.as_deref() {
                Some(tool) if let Some((name, _)) = cli::option(tool) => (
                    EXIT_USAGE,
                    writeln!(
                        err,
                        "error: {name} after the family: {prog}'s own options go before it \
                         ({prog} {name} ... fsl <tool> ...); after the tool, an option is the \
                         tool's\n\nRun '{prog} --help' for usage."
                    ),
                ),
                Some(tool) => match tools.iter().find(|(n, _)| *n == tool) {
                    Some((_, f)) => match given.upstream_defects() {
                        Ok(mode) => {
                            let settings = Settings::from_env(FSL_VARIABLES)
                                .reproducing_renames(mode.reproduce())
                                .defaulting_output_type(mode.fix());
                            let threads = threads(&given);
                            // an exception nothing in the program catches: std::terminate
                            let code = match larmorx_nc_core::exception::catch(|| {
                                f(&rest, &settings, mode, threads, out, err)
                            }) {
                                Ok(code) => code,
                                Err(e) => {
                                    let _ = write!(
                                        err,
                                        "{}terminate called after throwing an instance of \
                                         '{}'\n  what():  {}\nlarmorx-nc: {tool} aborts here \
                                         (signal 6); the replica exits with status 1\n",
                                        e.printed_text(),
                                        e.type_name,
                                        e.what
                                    );
                                    1
                                }
                            };
                            renamed_notices(&settings, mode, err);
                            output_type_notice(&settings, tool, err);
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

Usage: {prog} [options] <family> <tool> [original arguments]
       {prog} --version | --help

Tools:
  fsl    {fsl}

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
  FSLOUTPUTTYPE             the images' format, as in FSL (unset: NIFTI_GZ, .nii.gz, and a
                            notice, where FSL's programs stop with an error: FSL-11)
  OMP_NUM_THREADS           threads (default: all logical CPUs); results do not depend on it
",
        version = VERSION,
        fsl = tool_names(),
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
    fn version_and_help() {
        let (code, out, _) = run_capture(&["larmorx-nc", "--version"]);
        assert_eq!(code, 0);
        assert_eq!(out, format!("larmorx-nc {VERSION}\n"));
        let (code, out, _) = run_capture(&["dir/larmorx-nc.exe", "-h"]);
        assert_eq!(code, 0);
        assert!(out.starts_with(&format!("larmorx-nc {VERSION}: ")), "{out}");
        assert!(
            out.contains("Usage: larmorx-nc [options] <family> <tool>"),
            "{out}"
        );
        assert!(
            out.contains("fsl    mcflirt, bet, bet2, betsurf, flirt, convert_xfm, avscale, img2imgcoord, std2imgcoord, img2stdcoord, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D, pairreg, extracttxt, aff2rigid, fast, topup, applytopup, fslmaths, fslstats, fslroi, standard_space_roi"),
            "{out}"
        );
        let (code, out, err) = run_capture(&["larmorx-nc"]);
        assert_eq!((code, out.as_str()), (EXIT_USAGE, ""));
        assert!(err.contains("Usage:"), "{err}");
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

    /// What the probe tool received: the thread count and D13's mode.
    static PROBED: std::sync::Mutex<Vec<(usize, UpstreamDefects)>> =
        std::sync::Mutex::new(Vec::new());

    fn probe(
        _: &[String],
        _: &Settings,
        mode: UpstreamDefects,
        threads: usize,
        out: &mut dyn Write,
        _: &mut dyn Write,
    ) -> u8 {
        PROBED.lock().unwrap().push((threads, mode));
        let _ = writeln!(out, "probe {threads} {}", mode.name());
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

    /// larmorx's own options before the family reach the tool, over their variables (the
    /// process environment is not read for an option given).
    #[test]
    fn the_options_reach_the_tool() {
        for (args, expected) in [
            (
                &[
                    "larmorx-nc",
                    "--n-threads",
                    "5",
                    "--upstream-defects",
                    "reproduce",
                ][..],
                "probe 5 reproduce\n",
            ),
            (
                &["larmorx-nc", "--upstream-defects=fix", "--n-threads=3"][..],
                "probe 3 fix\n",
            ),
            (
                &[
                    "larmorx-nc",
                    "--implementation",
                    "replica",
                    "--n-threads",
                    "7",
                ][..],
                "probe 7 ",
            ),
        ] {
            let mut full = args.to_vec();
            full.extend(["fsl", "probe", "--n-threads", "9"]);
            let (code, out, err) = run_probe(&full);
            assert_eq!((code, err.as_str()), (0, ""), "{full:?}");
            assert!(out.starts_with(expected), "{full:?}: {out}");
        }
        let probed = PROBED.lock().unwrap().clone();
        assert!(
            probed.contains(&(5, UpstreamDefects::Reproduce)),
            "{probed:?}"
        );
        assert!(probed.contains(&(3, UpstreamDefects::Fix)), "{probed:?}");
        // 0: every logical CPU, as each tool reads it
        let (code, out, _) = run_probe(&["larmorx-nc", "--n-threads", "0", "fsl", "probe"]);
        assert_eq!(code, 0);
        assert!(out.starts_with("probe 0 "), "{out}");
    }

    #[test]
    fn wrong_options_are_usage_errors() {
        let usage = "\n\nUsage: larmorx-nc [--n-threads N] [--upstream-defects fix|reproduce] \
                     [--implementation replica] <family> <tool> [original arguments]\nRun \
                     'larmorx-nc --help' for usage.\n";
        for (args, message) in [
            (
                &["--n-threads", "x", "fsl", "probe"][..],
                "--n-threads must be a whole number of threads, 0 for every logical CPU, not 'x'",
            ),
            (
                &["--n-threads"][..],
                "--n-threads needs a value: a whole number of threads, 0 for every logical CPU",
            ),
            (
                &["--upstream-defects", "both", "fsl", "probe"][..],
                "--upstream-defects must be fix or reproduce, not 'both'",
            ),
            (
                &["--n-threads=1", "--n-threads=2", "fsl", "probe"][..],
                "--n-threads is given twice",
            ),
            (
                &["--implementation", "x", "fsl", "probe"][..],
                "--implementation must be replica, not 'x'",
            ),
        ] {
            let mut full = vec!["larmorx-nc"];
            full.extend(args);
            let (code, out, err) = run_probe(&full);
            assert_eq!((code, out.as_str()), (EXIT_USAGE, ""), "{full:?}");
            assert_eq!(err, format!("error: {message}{usage}"), "{full:?}");
        }
        for refused in ["auto", "clean-room", "original"] {
            let (code, out, err) =
                run_probe(&["larmorx-nc", "--implementation", refused, "fsl", "probe"]);
            assert_eq!((code, out.as_str()), (EXIT_USAGE, ""), "{refused}");
            assert!(
                err.starts_with(&format!(
                    "error: --implementation {refused}: this program runs the bit-exact replicas \
                     only, so --implementation takes replica here"
                )),
                "{err}"
            );
        }
        // an option of larmorx's after the family: it goes before the family
        let (code, _, err) = run_probe(&["larmorx-nc", "fsl", "--n-threads", "2", "probe"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(
            err.starts_with(
                "error: --n-threads after the family: larmorx-nc's own options go before it \
                 (larmorx-nc --n-threads ... fsl <tool> ...)"
            ),
            "{err}"
        );
        // the options with --help and --version
        let (code, out, _) = run_probe(&["larmorx-nc", "--n-threads", "2", "--version"]);
        assert_eq!((code, out), (0, format!("larmorx-nc {VERSION}\n")));
    }

    #[test]
    fn help_lists_the_options() {
        let (code, out, _) = run_capture(&["larmorx-nc", "--help"]);
        assert_eq!(code, 0);
        for line in [
            "Usage: larmorx-nc [options] <family> <tool> [original arguments]\n",
            "Options (before the family; each one given wins over the environment variable it replaces):\n",
            "  --n-threads N            threads, 0 = every logical CPU (default); replaces OMP_NUM_THREADS\n",
            "  --upstream-defects MODE  fix (default) or reproduce; replaces LARMORX_UPSTREAM_DEFECTS\n",
            "  --implementation replica changes nothing: this program runs the replicas only (auto and\n",
            "  fsl topup --nthr. The results never depend on the thread count.\n",
        ] {
            assert!(out.contains(line), "{line}\n{out}");
        }
    }

    #[test]
    fn unknown_tools() {
        let (code, _, err) = run_capture(&["larmorx-nc", "fsl", "fnirt"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(err.starts_with("error: unknown fsl tool 'fnirt'"), "{err}");
        let (code, _, err) = run_capture(&["larmorx-nc", "afni", "3dTshift"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(err.starts_with("error: unknown command 'afni'"), "{err}");
        let (code, out, _) = run_capture(&["larmorx-nc", "fsl", "mcflirt"]);
        assert_eq!(code, 1);
        assert!(out.starts_with("Usage: mcflirt"), "{out}");
    }
}
