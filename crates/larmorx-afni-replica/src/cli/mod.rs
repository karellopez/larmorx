// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Karel Lopez Vilaret. Written for larmorx-gpl and relicensed to Apache-2.0 by its
// author on 2026-10-29, when the AFNI replicas moved into larmorx.
//! The AFNI command lines of the replicas: `larmorx afni <program> ...` with
//! `LARMORX_IMPLEMENTATION=replica` ([`run`]), and the Python wrappers' runs inside the caller's
//! process ([`run_in`]).

pub mod automask;
mod common;
mod output;
pub mod tshift;
pub mod unifize;

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use larmorx_core::defects::{EXIT_BAD_MODE, UpstreamDefects};

use crate::environ::Settings;

/// The AFNI programs on the command line.
pub const TOOLS: &[&str] = &["3dTshift", "3dAutomask", "3dUnifize"];

/// Runs AFNI program `tool` with `args` (the arguments after the program name), with the
/// process's environment and working directory: D13's mode from `LARMORX_UPSTREAM_DEFECTS`,
/// AFNI's variables, and the threads from `OMP_NUM_THREADS`. `None` if there is no such
/// program.
pub fn run(tool: &str, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<u8> {
    match tool {
        "3dTshift" => Some(tshift::main(args, out, err)),
        "3dAutomask" => Some(automask::main(args, out, err)),
        "3dUnifize" => Some(unifize::main(args, out, err)),
        _ => None,
    }
}

/// [`run`] inside a caller's process, which must not depend on (or change) that process's
/// environment or working directory: `env` stands for the process environment (D13's mode
/// from its `LARMORX_UPSTREAM_DEFECTS`, AFNI's variables, the threads from its
/// `OMP_NUM_THREADS`), and relative file names start in `dir` when one is given. The result,
/// the messages and the exit status are those of `larmorx afni <tool>` run with that
/// environment in that directory. `None` if there is no such program.
pub fn run_in(
    tool: &str,
    args: &[String],
    env: Vec<(String, String)>,
    dir: Option<PathBuf>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<u8> {
    if !TOOLS.contains(&tool) {
        return None;
    }
    let mode = env
        .iter()
        .rev()
        .find(|(name, _)| name == larmorx_core::defects::ENV)
        .map(|(_, value)| OsString::from(value));
    let mut settings =
        Settings::from_vars(env.into_iter().map(|(n, v)| (OsString::from(n), v.into())));
    if let Some(dir) = dir {
        settings = settings.with_dir(dir);
    }
    let mode = match UpstreamDefects::from_value(mode.as_deref()) {
        Ok(mode) => mode,
        Err(e) => {
            let _ = writeln!(err, "** FATAL ERROR: {e}");
            return Some(EXIT_BAD_MODE);
        }
    };
    match tool {
        "3dTshift" => Some(tshift::main_with(args, mode, settings, out, err)),
        "3dAutomask" => Some(automask::main_with(args, mode, settings, out, err)),
        "3dUnifize" => Some(unifize::main_with(args, mode, settings, out, err)),
        _ => None,
    }
}

/// Threads: `OMP_NUM_THREADS` of the environment the program runs with (AFNI's variable, read
/// before AFNI's global options, so a `-D` does not set it), else all logical CPUs. Results do
/// not depend on it.
pub(crate) fn threads(settings: &Settings) -> usize {
    settings
        .environment("OMP_NUM_THREADS")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}
