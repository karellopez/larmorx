// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! The command line of the topup replica, `fsl topup`, with FSL's own arguments, messages and
//! exit statuses, as the `larmorx-nc` program (`larmorx-nc-cli`) runs it: the original
//! arguments (without the program name), the settings it read from the environment once
//! (`FSLOUTPUTTYPE`, `FSL_LOAD_NIFTI_EXTENSIONS`, `FSLDIR`), D13's mode
//! (`LARMORX_UPSTREAM_DEFECTS`: `fix`, the default, or `reproduce`, FSL's result bit for bit)
//! and the thread count (`OMP_NUM_THREADS`; 0: every logical CPU; the results do not depend on
//! it).

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::settings::Settings;

use crate::topup::Run;

/// `0` (unset) as every logical CPU.
fn thread_count(threads: usize) -> usize {
    if threads > 0 {
        threads
    } else {
        std::thread::available_parallelism().map_or(1, usize::from)
    }
}

/// `topup --imain=... --datain=... [options]`.
pub fn topup(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec!["topup".to_owned()];
    argv.extend(args.iter().cloned());
    let run = Run {
        fsl: settings,
        mode,
        threads: thread_count(threads),
    };
    let code = crate::topup::main(&argv, &run, out, err);
    let _ = out.flush();
    let _ = err.flush();
    code
}
