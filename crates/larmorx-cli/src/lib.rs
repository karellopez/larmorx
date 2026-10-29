// SPDX-License-Identifier: Apache-2.0
//! The `larmorx` multicall command line (alias `lx`), PLAN.md §4 item 3.
//!
//! Usage is `larmorx <family> <tool> [original arguments]`, where each tool accepts the
//! argument syntax of the program it replaces. Parsing lives here, once, and is shared by the
//! standalone binaries and the Python console scripts (through `larmorx._core`).
//!
//! Tools with a bit-exact replica run it by default: built in (AFNI's programs, in this
//! process), or as the program of its own package when that is found (`larmorx-nc`), else the
//! clean-room original ([`replica`], `docs/licensing.md`).
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
    ("afni", AFNI_TOOLS),
    ("ants", larmorx_ants::cli::TOOLS),
    ("fsl", FSL_TOOLS),
    ("mri", larmorx_mri::cli::TOOLS),
];

/// The FSL tools, with FSL's command lines: the replica (`larmorx-nc`) when it is installed,
/// else the clean-room original of the same program (`fsl mcflirt` is `mri hmc`); BET's
/// programs have no clean-room original yet ([`replica::replica_only`]; a test checks the list).
const FSL_TOOLS: &[&str] = &["mcflirt", "bet", "bet2", "betsurf"];

/// The AFNI tools: the clean-room originals' (`larmorx_afni::cli::TOOLS`), each with its
/// built-in replica (`larmorx_afni_replica::cli::TOOLS`; a test checks the lists).
const AFNI_TOOLS: &[&str] = &["3dTshift", "3dAutomask", "3dUnifize"];

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

/// Runs `tool` of `family`: its replica (built in, or as a separate program) when
/// [`replica::choose`] says so, else the original. `None` if there is no such tool.
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
            Ok(replica::Choice::Builtin) => {
                return replica::run_builtin(family, tool, args, out, err);
            }
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
        "fsl" => match tool {
            "mcflirt" => larmorx_mri::cli::run("hmc", args, out, err),
            _ => None,
        },
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
        .map(|r| match r.package() {
            None => format!(
                "  {} {}: built in ({})\n",
                r.family,
                r.tool,
                r.licence()
            ),
            Some(p) => format!(
                "  {} {}: {} ({}), a separate program\n",
                r.family, r.tool, p.name, p.licence
            ),
        })
        .collect();
    let mut packages: Vec<&replica::Package> = Vec::new();
    for p in replica::REPLICAS.iter().filter_map(replica::Replica::package) {
        if !packages.contains(&p) {
            packages.push(p);
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

Bit-exact replicas (docs/licensing.md):
{replicas}
Environment:
  {env:<23} auto (default): a tool's replica if it is built in or its program is
  {pad:<23} found, else the original; replica: the replica or exit 127; original:
  {pad:<23} the original
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
        assert!(
            out.contains("afni 3dTshift: built in (Apache-2.0 AND CC-BY-4.0)"),
            "{out}"
        );
        assert!(out.contains("LARMORX_IMPLEMENTATION"), "{out}");
        assert!(!out.contains("LARMORX_GPL_BIN"), "{out}");

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
        // The AFNI tools: the originals' and those with a replica only.
        let listed: Vec<&str> = larmorx_afni::cli::TOOLS
            .iter()
            .copied()
            .chain(replica::replica_only("afni"))
            .collect();
        assert_eq!(listed, AFNI_TOOLS);
        // Every AFNI tool has its clean-room original and its built-in replica.
        assert_eq!(replica::replica_only("afni").count(), 0);
        assert_eq!(larmorx_afni_replica::cli::TOOLS, AFNI_TOOLS);
        let replica = Options {
            implementation: Some(Implementation::Replica),
            ..Options::default()
        };
        let (code, _, err) =
            run_options(&["lx", "afni", "3dTshift", "-bogus", "in.nii"], &replica);
        assert_eq!(code, 1);
        assert!(err.contains("Unknown option: -bogus"), "{err}");
        for tool in ["3dAutomask", "3dUnifize"] {
            let (code, out, _) = run_capture(&["lx", "afni", tool]);
            assert_eq!(code, 0);
            assert!(out.starts_with(&format!("Usage: {tool}")), "{out}");
        }
    }

    #[test]
    fn fsl_tools_have_only_their_replica() {
        let (code, out, _) = run_capture(&["lx", "--help"]);
        assert_eq!(code, 0);
        assert!(out.contains("fsl    mcflirt, bet, bet2, betsurf"), "{out}");
        assert!(
            out.contains("fsl bet: larmorx-nc (LicenseRef-FSL), a separate program"),
            "{out}"
        );
        assert!(out.contains("LARMORX_NC_BIN"), "{out}");
        assert_eq!(
            replica::replica_only("fsl").collect::<Vec<_>>(),
            FSL_TOOLS[1..]
        );
        // Without the replica (or with `original` asked for) the tool refuses, naming the
        // package; exit 127 as for a missing program.
        let (code, _, err) = run_capture(&["lx", "fsl", "bet", "in", "out"]);
        assert_eq!(code, replica::EXIT_NOT_FOUND);
        assert!(err.contains("no clean-room original of fsl bet"), "{err}");
        assert!(err.contains("pip install larmorx-nc"), "{err}");
    }

    #[test]
    fn a_replica_that_cannot_start_exits_126() {
        let tmp = tempfile::tempdir().unwrap();
        let replica = replica::lookup("fsl", "mcflirt").unwrap();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = replica::run(
            &tmp.path().join("larmorx-nc"),
            replica,
            &[],
            replica::Output::Capture,
            &mut out,
            &mut err,
        );
        assert_eq!(code, replica::EXIT_CANNOT_RUN);
        let err = String::from_utf8(err).unwrap();
        assert!(
            err.contains("cannot run the replica of fsl mcflirt"),
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
    fn fsl_mcflirt_is_the_original_of_mri_hmc() {
        let (code, out, err) = run_capture(&["lx", "fsl", "mcflirt"]);
        assert_eq!((code, out, err), run_capture(&["lx", "mri", "hmc"]));
        assert!(replica::lookup("fsl", "mcflirt").is_some_and(|r| r.original));
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
        let (code, out, err) = run_capture(&["lx", "spm", "realign"]);
        assert_eq!(code, EXIT_USAGE);
        assert_eq!(out, "");
        assert!(err.starts_with("error: unknown command 'spm'"), "{err}");
        assert!(err.contains("Run 'lx --help'"), "{err}");

        let (code, _, err) = run_capture(&["lx", "fsl", "fslmaths"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(
            err.starts_with("error: unknown fsl tool 'fslmaths'"),
            "{err}"
        );

        let (code, _, err) = run_capture(&["lx", "afni", "3dvolreg"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(
            err.starts_with("error: unknown afni tool '3dvolreg'"),
            "{err}"
        );

        let (code, _, err) = run_capture(&["lx", "ants", "KellyKapowski"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(
            err.starts_with("error: unknown ants tool 'KellyKapowski'"),
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
