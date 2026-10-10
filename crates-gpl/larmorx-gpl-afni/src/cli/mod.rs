// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! The AFNI command lines of the replicas (`larmorx-gpl afni <program> ...`).

pub mod tshift;

use std::io::Write;

/// The AFNI programs on the command line.
pub const TOOLS: &[&str] = &["3dTshift"];

/// Runs AFNI program `tool` with `args` (the arguments after the program name). `None` if
/// there is no such program.
pub fn run(tool: &str, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<u8> {
    match tool {
        "3dTshift" => Some(tshift::main(args, out, err)),
        _ => None,
    }
}

/// Threads for the command line: `OMP_NUM_THREADS` if set (AFNI's variable), else all logical
/// CPUs. Results do not depend on it.
pub(crate) fn threads() -> usize {
    std::env::var("OMP_NUM_THREADS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}
