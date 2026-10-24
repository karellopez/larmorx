// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! `larmorx-nc-bet <family> <tool> [original arguments]`: the BET replicas as a program, with
//! the arguments of the package's `larmorx-nc` program (`larmorx-nc fsl bet ...`), which runs
//! them once the FSL replicas are joined. larmorx (Apache-2.0) runs it as a separate process
//! (`LARMORX_NC_BIN`).
#![forbid(unsafe_code)]

use std::io::{self, Write};
use std::process::ExitCode;

const EXIT_USAGE: u8 = 2;

fn usage(prog: &str) -> String {
    format!(
        "{prog} {version}: bit-exact replicas of FSL's BET (bet, bet2, betsurf; FSL 6.0.7.17), with the
original command lines (FSL Licence: non-commercial use only; see docs/licensing.md)

Usage: {prog} <family> <tool> [original arguments]
       {prog} --version | --help

Tools:
  fsl    {tools}
",
        version = env!("CARGO_PKG_VERSION"),
        tools = larmorx_nc_bet::cli::TOOLS.join(", "),
    )
}

fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let prog = "larmorx-nc-bet";
    match args.first().map(String::as_str) {
        Some("-V" | "--version") => {
            // The package's name, which larmorx records with a result (`_replica.Selected`).
            let _ = writeln!(out, "larmorx-nc {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("-h" | "--help") => {
            let _ = out.write_all(usage(prog).as_bytes());
            0
        }
        None => {
            let _ = err.write_all(usage(prog).as_bytes());
            EXIT_USAGE
        }
        Some("fsl") => match args.get(1) {
            Some(tool) => match larmorx_nc_bet::cli::run(tool, &args[2..], out, err) {
                Some(code) => code,
                None => {
                    let _ = writeln!(
                        err,
                        "error: unknown fsl tool '{tool}' (available: {})\n\nRun '{prog} --help' for usage.",
                        larmorx_nc_bet::cli::TOOLS.join(", ")
                    );
                    EXIT_USAGE
                }
            },
            None => {
                let _ = writeln!(
                    err,
                    "error: missing fsl tool (available: {})",
                    larmorx_nc_bet::cli::TOOLS.join(", ")
                );
                EXIT_USAGE
            }
        },
        Some(arg) => {
            let kind = if arg.starts_with('-') { "option" } else { "command" };
            let _ = writeln!(err, "error: unknown {kind} '{arg}'\n\nRun '{prog} --help' for usage.");
            EXIT_USAGE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    ExitCode::from(run(&args, &mut io::stdout().lock(), &mut io::stderr().lock()))
}
