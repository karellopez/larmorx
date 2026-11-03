// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! The command lines of the avwutils replicas (`fsl fslmaths`, `fsl fslstats`), with FSL's own arguments,
//! messages and exit statuses, as the `larmorx-nc` program (`larmorx-nc-cli`) runs them: the
//! original arguments (without the program name), the settings read from the environment once
//! (`FSLOUTPUTTYPE`, `FSL_LOAD_NIFTI_EXTENSIONS`), D13's mode (`LARMORX_UPSTREAM_DEFECTS`:
//! `fix`, the default, or `reproduce`, FSL's result bit for bit) and the thread count (unused:
//! fslmaths runs on one thread).

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::exception;
use larmorx_nc_core::settings::Settings;

use crate::fslmaths::{Run, Stop};

/// `fslmaths [-dt <datatype>] <first_input> [operations and inputs] <output> [-odt <datatype>]`.
/// An exception nothing in fslmaths catches unwinds to the `larmorx-nc` program, which prints
/// the C++ runtime's text (`exception::throw`).
pub fn fslmaths(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    _threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec!["fslmaths".to_owned()];
    argv.extend(args.iter().cloned());
    let run = Run {
        fsl: settings,
        mode,
    };
    let r = crate::fslmaths::main(&argv, &run, out, err);
    let _ = out.flush();
    let _ = err.flush();
    match r {
        Ok(code) | Err(Stop::Exit(code)) => code,
        Err(Stop::Exception(e)) => exception::throw(e),
    }
}

/// `fslstats [preoptions] <input> [options]`: the program (`larmorx_nc_core::fslstats`, which
/// BET's script also runs) with its own messages and exit statuses; every exception is caught
/// as fslstats catches it (status 255). fslstats runs on one thread.
pub fn fslstats(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    _threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec!["fslstats".to_owned()];
    argv.extend(args.iter().cloned());
    let run = larmorx_nc_core::fslstats::Run {
        fsl: settings,
        mode,
    };
    larmorx_nc_core::fslstats::main(&argv, &run, out, err)
}
