// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! The command line of the PRELUDE replica, `fsl prelude`, with FSL's own arguments, messages
//! and exit statuses, as the `larmorx-nc` program (`larmorx-nc-cli`) runs it: the original
//! arguments (without the program name), the settings it read from the environment once
//! (`FSLOUTPUTTYPE`, `FSL_LOAD_NIFTI_EXTENSIONS`) and D13's mode (`LARMORX_UPSTREAM_DEFECTS`:
//! `fix`, the default, or `reproduce`, FSL's result bit for bit). PRELUDE runs on one thread,
//! as FSL's does: the thread count is accepted and not used.

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::settings::Settings;

use crate::prelude::Run;

/// `prelude [options]`.
pub fn prelude(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    _threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec!["prelude".to_owned()];
    argv.extend(args.iter().cloned());
    let run = Run {
        fsl: settings,
        mode,
    };
    let code = crate::prelude::main(&argv, &run, out, err);
    let _ = out.flush();
    let _ = err.flush();
    code
}
