// SPDX-License-Identifier: Apache-2.0
//! The `larmorx` multicall command line (alias `lx`), PLAN.md §4 item 3.
//!
//! Usage is `larmorx <family> <tool> [original arguments]`, where each tool accepts the
//! argument syntax of the program it replaces. Parsing lives here, once, and is shared by the
//! standalone binaries and the Python console scripts (through `larmorx._core`).
//!
//! Tools with a bit-exact replica in another package (`larmorx-gpl`) run that package's
//! program as a separate process when it is found, and the clean-room original otherwise
//! ([`replica`], `docs/licensing.md`).
#![forbid(unsafe_code)]

pub mod replica;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub use larmorx_ants::cli::{FileLoader, TransformLoader};

use replica::Implementation;

/// Exit code for invalid usage (unknown command or option).
pub const EXIT_USAGE: u8 = 2;

/// Exit code when the output streams cannot be written.
const EXIT_IO_ERROR: u8 = 1;

/// The tool families and their tools, as [`run_with`] dispatches them. Add a new family here
/// and to `run_tool`; every tool listed needs a golden case (`tests/golden/registry.py`,
/// `docs/validation/golden.md`), which the golden coverage test checks through this list.
pub const FAMILIES: &[(&str, &[&str])] = &[
    ("afni", larmorx_afni::cli::TOOLS),
    ("ants", larmorx_ants::cli::TOOLS),
    ("mri", larmorx_mri::cli::TOOLS),
];

/// How a command line runs, besides its arguments.
pub struct Options<'a> {
    /// Reads transform files (the Python entry point adds `.h5`).
    pub loader: &'a dyn TransformLoader,
    /// The implementation of tools that have a replica; `None` reads
    /// [`replica::IMPLEMENTATION_ENV`] (default `auto`).
    pub implementation: Option<Implementation>,
    /// Where to look for replica programs after their environment variable and before `PATH`
    /// (the Python environment's scripts directory, or the directory of this binary).
    pub replica_dirs: Vec<PathBuf>,
    /// How a replica's output reaches the user.
    pub replica_output: replica::Output,
}

impl Default for Options<'_> {
    /// Transform files read with [`FileLoader`]; the implementation from the environment;
    /// replicas looked for through their environment variable and `PATH`; their output
    /// captured and written to the `out` and `err` writers.
    fn default() -> Self {
        Options {
            loader: &FileLoader,
            implementation: None,
            replica_dirs: Vec::new(),
            replica_output: replica::Output::Capture,
        }
    }
}

/// Runs the CLI with `args` (program name first), writing to `out` and `err`, with the
/// default [`Options`].
///
/// Returns the process exit code.
pub fn run<I, S>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    run_with(args, &Options::default(), out, err)
}

/// [`run`] with other [`Options`].
pub fn run_with<I, S>(
    args: I,
    options: &Options<'_>,
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
        .map_or_else(|| "larmorx".to_owned(), |a| program_name(a.as_ref()));
    let first = args.next().map(|a| a.as_ref().to_owned());
    let family = first
        .as_deref()
        .and_then(|f| FAMILIES.iter().find(|(name, _)| *name == f));
    let (code, written) = match (first.as_deref(), family) {
        (Some("-V" | "--version"), _) => (0, writeln!(out, "larmorx {}", larmorx_core::VERSION)),
        (Some("-h" | "--help"), _) => (0, out.write_all(usage(&prog).as_bytes())),
        (None, _) => (EXIT_USAGE, err.write_all(usage(&prog).as_bytes())),
        (Some(_), Some((family, tools))) => {
            let tool = args.next().map(|a| a.as_ref().to_owned());
            let rest: Vec<String> = args.map(|a| a.as_ref().to_owned()).collect();
            match tool.as_deref() {
                Some(tool) => match run_tool(family, tool, &rest, options, out, err) {
                    Some(code) => (code, Ok(())),
                    None => (
                        EXIT_USAGE,
                        writeln!(
                            err,
                            "error: unknown {family} tool '{tool}' (available: {})\n\nRun '{prog} --help' for usage.",
                            tools.join(", ")
                        ),
                    ),
                },
                None => (
                    EXIT_USAGE,
                    writeln!(
                        err,
                        "error: missing {family} tool (available: {})",
                        tools.join(", ")
                    ),
                ),
            }
        }
        (Some(arg), None) => {
            let kind = if arg.starts_with('-') {
                "option"
            } else {
                "command"
            };
            let message =
                format!("error: unknown {kind} '{arg}'\n\nRun '{prog} --help' for usage.");
            (EXIT_USAGE, writeln!(err, "{message}"))
        }
    };
    if written.is_ok() { code } else { EXIT_IO_ERROR }
}

/// Runs `tool` of `family`: its replica as a separate program when [`replica::choose`] says
/// so, else the original. `None` if there is no such tool.
fn run_tool(
    family: &str,
    tool: &str,
    args: &[String],
    options: &Options<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<u8> {
    if let Some(replica) = replica::lookup(family, tool) {
        let choice = options
            .implementation
            .map_or_else(Implementation::from_env, Ok)
            .and_then(|implementation| {
                replica::choose(implementation, replica, &options.replica_dirs)
            });
        match choice {
            Ok(replica::Choice::Original) => {}
            Ok(replica::Choice::Replica(program)) => {
                return Some(replica::run(
                    &program,
                    replica,
                    args,
                    options.replica_output,
                    out,
                    err,
                ));
            }
            Err(e) => {
                let written = writeln!(err, "error: {e}");
                return Some(if written.is_ok() {
                    e.exit_code()
                } else {
                    EXIT_IO_ERROR
                });
            }
        }
    }
    match family {
        "afni" => larmorx_afni::cli::run(tool, args, out, err),
        "ants" => larmorx_ants::cli::run(tool, args, options.loader, out, err),
        "mri" => larmorx_mri::cli::run(tool, args, out, err),
        _ => None,
    }
}

/// Entry point of the standalone `larmorx` and `lx` binaries. Replica programs are also looked
/// for next to the binary, and write to its standard output and error directly.
///
/// Arguments that are not valid UTF-8 are converted lossily.
pub fn main() -> ExitCode {
    let args = std::env::args_os().map(|a| a.to_string_lossy().into_owned());
    let options = Options {
        replica_dirs: std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .into_iter()
            .collect(),
        replica_output: replica::Output::Inherit,
        ..Options::default()
    };
    ExitCode::from(run_with(
        args,
        &options,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ))
}

/// The name the CLI was invoked as (`larmorx` or `lx`), without directory or `.exe` suffix.
fn program_name(argv0: &str) -> String {
    Path::new(argv0)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("larmorx")
        .to_owned()
}

fn usage(prog: &str) -> String {
    let tools: String = FAMILIES
        .iter()
        .map(|(family, tools)| format!("  {family:<6} {}\n", tools.join(", ")))
        .collect();
    let replicas: String = replica::REPLICAS
        .iter()
        .map(|r| {
            format!(
                "  {} {}: {} ({})\n",
                r.family, r.tool, r.package.name, r.package.licence
            )
        })
        .collect();
    let mut packages: Vec<&replica::Package> = Vec::new();
    for r in replica::REPLICAS {
        if !packages.contains(&r.package) {
            packages.push(r.package);
        }
    }
    let programs: String = packages
        .iter()
        .map(|p| format!("  {:<23} path of the {} program\n", p.env, p.program))
        .collect();
    format!(
        "larmorx {version}: neuroimaging tools in Rust with original-compatible command lines

Usage: {prog} <family> <tool> [original arguments]
       {prog} --version | --help

Tools:
{tools}
Options:
  -h, --help     Print this help
  -V, --version  Print the version

Bit-exact replicas, in separate packages, run as separate programs (docs/licensing.md):
{replicas}
Environment:
  {env:<23} auto (default): a tool's replica if its program is found, else the
  {pad:<23} original; replica: the replica or exit 127; original: the original
{programs}",
        version = larmorx_core::VERSION,
        env = replica::IMPLEMENTATION_ENV,
        pad = "",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_capture(args: &[&str]) -> (u8, String, String) {
        // The original, whatever replicas this machine has installed.
        let options = Options {
            implementation: Some(Implementation::Original),
            ..Options::default()
        };
        run_options(args, &options)
    }

    fn run_options(args: &[&str], options: &Options<'_>) -> (u8, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(args, options, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn version_flags_print_the_version() {
        for flag in ["--version", "-V"] {
            let (code, out, err) = run_capture(&["larmorx", flag]);
            assert_eq!(code, 0);
            assert_eq!(out, format!("larmorx {}\n", larmorx_core::VERSION));
            assert_eq!(err, "");
        }
    }

    #[test]
    fn help_uses_the_invoked_program_name() {
        let (code, out, _) = run_capture(&["larmorx", "--help"]);
        assert_eq!(code, 0);
        assert!(out.contains("Usage: larmorx <family> <tool>"), "{out}");
        assert!(out.contains("afni 3dTshift: larmorx-gpl"), "{out}");
        assert!(out.contains("LARMORX_IMPLEMENTATION"), "{out}");
        assert!(out.contains("LARMORX_GPL_BIN"), "{out}");

        let (code, out, _) = run_capture(&["some/dir/lx.exe", "-h"]);
        assert_eq!(code, 0);
        assert!(out.contains("Usage: lx <family> <tool>"), "{out}");
    }

    #[test]
    fn no_arguments_print_usage_to_stderr() {
        let (code, out, err) = run_capture(&["lx"]);
        assert_eq!(code, EXIT_USAGE);
        assert_eq!(out, "");
        assert!(err.contains("Usage: lx"), "{err}");

        let (code, _, err) = run_capture(&[]);
        assert_eq!(code, EXIT_USAGE);
        assert!(err.contains("Usage: larmorx"), "{err}");
    }

    #[test]
    fn afni_tools_run() {
        let (code, out, _) = run_capture(&["lx", "afni", "3dTshift"]);
        assert_eq!(code, 0);
        assert!(out.starts_with("Usage: 3dTshift"), "{out}");
        let (code, _, err) = run_capture(&["lx", "afni", "3dTshift", "-bogus", "in.nii"]);
        assert_eq!(code, 1);
        assert!(err.contains("Unknown option: -bogus"), "{err}");
        let (code, out, _) = run_capture(&["lx", "--help"]);
        assert_eq!(code, 0);
        assert!(out.contains("afni   3dTshift"), "{out}");
    }

    #[test]
    fn a_replica_that_cannot_start_exits_126() {
        let tmp = tempfile::tempdir().unwrap();
        let replica = replica::lookup("afni", "3dTshift").unwrap();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = replica::run(
            &tmp.path().join("larmorx-gpl"),
            replica,
            &[],
            replica::Output::Capture,
            &mut out,
            &mut err,
        );
        assert_eq!(code, replica::EXIT_CANNOT_RUN);
        let err = String::from_utf8(err).unwrap();
        assert!(
            err.contains("cannot run the replica of afni 3dTshift"),
            "{err}"
        );
    }

    #[test]
    fn mri_tools_run() {
        let (code, out, _) = run_capture(&["lx", "mri", "hmc"]);
        assert_eq!(code, 1);
        assert!(out.starts_with("Usage: larmorx mri hmc"), "{out}");
        let (code, _, err) = run_capture(&["lx", "mri", "bet"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(err.starts_with("error: unknown mri tool 'bet'"), "{err}");
        let (code, out, _) = run_capture(&["lx", "--help"]);
        assert_eq!(code, 0);
        assert!(out.contains("mri    hmc"), "{out}");
    }

    #[test]
    fn every_listed_tool_is_dispatched() {
        let (_, help, _) = run_capture(&["lx", "--help"]);
        for (family, tools) in FAMILIES {
            assert!(
                help.contains(&format!("  {family:<6} {}\n", tools.join(", "))),
                "{help}"
            );
            for tool in *tools {
                let (code, _, err) = run_capture(&["lx", family, tool]);
                assert_ne!(code, EXIT_USAGE, "{family} {tool}: {err}");
                assert!(!err.contains("unknown"), "{family} {tool}: {err}");
            }
        }
        // Every replica belongs to a listed tool.
        for r in replica::REPLICAS {
            assert!(
                FAMILIES
                    .iter()
                    .any(|(f, tools)| *f == r.family && tools.contains(&r.tool)),
                "{} {}",
                r.family,
                r.tool
            );
        }
    }

    #[test]
    fn unknown_commands_and_options_are_usage_errors() {
        let (code, out, err) = run_capture(&["lx", "fsl", "bet"]);
        assert_eq!(code, EXIT_USAGE);
        assert_eq!(out, "");
        assert!(err.starts_with("error: unknown command 'fsl'"), "{err}");
        assert!(err.contains("Run 'lx --help'"), "{err}");

        let (code, _, err) = run_capture(&["lx", "afni", "3dvolreg"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(
            err.starts_with("error: unknown afni tool '3dvolreg'"),
            "{err}"
        );

        let (code, _, err) = run_capture(&["lx", "ants", "antsRegistration"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(
            err.starts_with("error: unknown ants tool 'antsRegistration'"),
            "{err}"
        );

        let (code, _, err) = run_capture(&["larmorx", "--frobnicate"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(
            err.starts_with("error: unknown option '--frobnicate'"),
            "{err}"
        );
    }
}
