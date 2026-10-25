// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! The command lines of the BET replicas: `fsl bet`, `fsl bet2`, `fsl betsurf`, with FSL's own
//! arguments, messages and exit statuses, as the `larmorx-nc` program (`larmorx-nc-cli`) runs
//! them: the original arguments (without the program name), the settings it read from the
//! environment once (`FSLOUTPUTTYPE`, `FSL_LOAD_NIFTI_EXTENSIONS`), D13's mode
//! (`LARMORX_UPSTREAM_DEFECTS`: `fix`, the default, or `reproduce`, FSL's result bit for bit)
//! and the thread count (`OMP_NUM_THREADS`; 0: every logical CPU; the results do not depend on
//! it). `FSLDIR` is not needed: the replica runs none of FSL's programs.

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::settings::Settings;

use crate::bet2::Run;

/// The tools, as `larmorx-nc fsl <tool>` names them.
pub const TOOLS: &[&str] = &["bet", "bet2", "betsurf"];

/// `0` (unset) as every logical CPU.
fn thread_count(threads: usize) -> usize {
    if threads > 0 {
        threads
    } else {
        std::thread::available_parallelism().map_or(1, usize::from)
    }
}

fn run_tool(
    tool: &str,
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec![tool.to_owned()];
    argv.extend(args.iter().cloned());
    let run = Run {
        fsl: settings,
        mode,
        threads: thread_count(threads),
    };
    let code = match tool {
        "bet2" => crate::bet2::main(&argv, &run, out, err),
        "bet" => crate::bet::main(&argv, &run, out, err),
        _ => crate::betsurf::main(&argv, &run, out, err),
    };
    let _ = out.flush();
    let _ = err.flush();
    code
}

/// `bet <input> <output> [options]` (the `bet` script).
pub fn bet(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    run_tool("bet", args, settings, mode, threads, out, err)
}

/// `bet2 <input> <output> [options]`.
pub fn bet2(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    run_tool("bet2", args, settings, mode, threads, out, err)
}

/// `betsurf [options] <t1> [t2] <bet_mesh.vtk> <t1_to_standard_mat> <output>`.
pub fn betsurf(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    run_tool("betsurf", args, settings, mode, threads, out, err)
}
