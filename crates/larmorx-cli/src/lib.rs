//! The `larmorx` multicall command line (alias `lx`), PLAN.md §4 item 3.
//!
//! Usage is `larmorx <family> <tool> [original arguments]`, where each tool accepts the
//! argument syntax of the program it replaces. Parsing lives here, once, and is shared by the
//! standalone binaries and the Python console scripts (through `larmorx._core`).
#![forbid(unsafe_code)]

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

pub use larmorx_ants::cli::{FileLoader, TransformLoader};

/// Exit code for invalid usage (unknown command or option).
pub const EXIT_USAGE: u8 = 2;

/// Exit code when the output streams cannot be written.
const EXIT_IO_ERROR: u8 = 1;

/// Tool families planned for the CLI (PLAN.md §3) that have no tool yet.
const PLANNED_FAMILIES: &str = "afni, mri";

/// Runs the CLI with `args` (program name first), writing to `out` and `err`, reading
/// transform files with [`FileLoader`].
///
/// Returns the process exit code.
pub fn run<I, S>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    run_with(args, &FileLoader, out, err)
}

/// [`run`] with another reader for transform files (the Python entry point adds `.h5`).
pub fn run_with<I, S>(
    args: I,
    loader: &dyn TransformLoader,
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
    let (code, written) = match args.next().as_ref().map(AsRef::as_ref) {
        Some("-V" | "--version") => (0, writeln!(out, "larmorx {}", larmorx_core::VERSION)),
        Some("-h" | "--help") => (0, out.write_all(usage(&prog).as_bytes())),
        None => (EXIT_USAGE, err.write_all(usage(&prog).as_bytes())),
        Some("ants") => {
            let tool = args.next().map(|a| a.as_ref().to_owned());
            let rest: Vec<String> = args.map(|a| a.as_ref().to_owned()).collect();
            match tool.as_deref() {
                Some(tool) => match larmorx_ants::cli::run(tool, &rest, loader, out, err) {
                    Some(code) => (code, Ok(())),
                    None => (
                        EXIT_USAGE,
                        writeln!(
                            err,
                            "error: unknown ants tool '{tool}' (available: {})\n\nRun '{prog} --help' for usage.",
                            larmorx_ants::cli::TOOLS.join(", ")
                        ),
                    ),
                },
                None => (
                    EXIT_USAGE,
                    writeln!(
                        err,
                        "error: missing ants tool (available: {})",
                        larmorx_ants::cli::TOOLS.join(", ")
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
            let message =
                format!("error: unknown {kind} '{arg}'\n\nRun '{prog} --help' for usage.");
            (EXIT_USAGE, writeln!(err, "{message}"))
        }
    };
    if written.is_ok() { code } else { EXIT_IO_ERROR }
}

/// Entry point of the standalone `larmorx` and `lx` binaries.
///
/// Arguments that are not valid UTF-8 are converted lossily.
pub fn main() -> ExitCode {
    let args = std::env::args_os().map(|a| a.to_string_lossy().into_owned());
    ExitCode::from(run(
        args,
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
    format!(
        "larmorx {version}: neuroimaging tools in Rust with original-compatible command lines

Usage: {prog} <family> <tool> [original arguments]
       {prog} --version | --help

Tools:
  ants   {ants}
Planned tool families: {PLANNED_FAMILIES}

Options:
  -h, --help     Print this help
  -V, --version  Print the version
",
        version = larmorx_core::VERSION,
        ants = larmorx_ants::cli::TOOLS.join(", "),
    )
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
    fn unknown_commands_and_options_are_usage_errors() {
        let (code, out, err) = run_capture(&["lx", "afni", "3dTshift"]);
        assert_eq!(code, EXIT_USAGE);
        assert_eq!(out, "");
        assert!(err.starts_with("error: unknown command 'afni'"), "{err}");
        assert!(err.contains("Run 'lx --help'"), "{err}");

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
