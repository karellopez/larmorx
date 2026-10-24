// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! The command lines of the BET replicas: `fsl bet`, `fsl bet2`, `fsl betsurf`, with FSL's own
//! arguments, messages and exit statuses.
//!
//! Settings come from the environment, read once: `FSLOUTPUTTYPE` and
//! `FSL_LOAD_NIFTI_EXTENSIONS` (FSL's), `LARMORX_UPSTREAM_DEFECTS` (D13: `fix`, the default, or
//! `reproduce`, FSL's result bit for bit). `FSLDIR` is not needed: the replica runs none of
//! FSL's programs.

use std::io::Write;

use larmorx_core::defects::{self, UpstreamDefects};

use crate::fsl::volume::FslSettings;
use crate::{bet, bet2, betsurf};

/// The tools, as `larmorx-nc fsl <tool>` names them.
pub const TOOLS: &[&str] = &["bet", "bet2", "betsurf"];

/// Runs `tool` with its original arguments (without the program name); `None` for an unknown
/// tool.
pub fn run(tool: &str, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<u8> {
    if !TOOLS.contains(&tool) {
        return None;
    }
    let mode = match UpstreamDefects::from_env() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "error: {e}");
            return Some(defects::EXIT_BAD_MODE);
        }
    };
    let fsl = FslSettings::from_env();
    Some(run_with(tool, args, &fsl, mode, out, err))
}

/// [`run`] with the settings given.
pub fn run_with(
    tool: &str,
    args: &[String],
    fsl: &FslSettings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec![tool.to_owned()];
    argv.extend(args.iter().cloned());
    let run = bet2::Run { fsl, mode };
    let code = match tool {
        "bet2" => bet2::main(&argv, &run, out, err),
        "bet" => bet::main(&argv, &run, out, err),
        "betsurf" => betsurf::main(&argv, &run, out, err),
        _ => 2,
    };
    let _ = out.flush();
    let _ = err.flush();
    code
}
