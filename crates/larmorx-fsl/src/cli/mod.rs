// SPDX-License-Identifier: Apache-2.0
//! The command lines of the `fsl` family's clean-room originals (`larmorx fsl <tool> ...`,
//! FSL's own arguments).
//!
//! FSL's environment variables (`FSLOUTPUTTYPE`, `FSL_LOAD_NIFTI_EXTENSIONS`, `FSLDIR`, `PATH`
//! for `bet -Z`) and larmorx's (`LARMORX_UPSTREAM_DEFECTS`, `OMP_NUM_THREADS` for the thread
//! count) are read into a table of settings ([`Settings`]): the command line fills it from the
//! process environment, the Python wrappers give it explicitly (porting guide §9.1).

pub mod bet;
pub mod bet2;
pub mod fmt;
pub mod options;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::defects::{self, UpstreamDefects};

/// The tools on the command line.
pub const TOOLS: &[&str] = &["bet", "bet2"];

/// The settings of one run: the environment it reads and the directory relative names start
/// in.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub vars: BTreeMap<String, String>,
    pub dir: Option<PathBuf>,
}

impl Settings {
    /// From the process environment and directory.
    pub fn from_process() -> Settings {
        Settings {
            vars: std::env::vars_os()
                .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
                .collect(),
            dir: None,
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(String::as_str)
    }

    /// A file name as the run opens it: relative names start in [`Settings::dir`].
    pub fn path(&self, name: &str) -> PathBuf {
        match &self.dir {
            Some(d) if Path::new(name).is_relative() => d.join(name),
            _ => PathBuf::from(name),
        }
    }

    /// The text of a path as the run names it (relative names joined to the directory).
    pub fn name(&self, name: &str) -> String {
        self.path(name).to_string_lossy().into_owned()
    }

    /// D13's mode (`LARMORX_UPSTREAM_DEFECTS`).
    pub fn defects(&self) -> Result<UpstreamDefects, defects::BadMode> {
        UpstreamDefects::from_value(self.vars.get(defects::ENV).map(std::ffi::OsStr::new))
    }

    /// The thread count: `OMP_NUM_THREADS` when it is a positive number, else 0 (all).
    pub fn threads(&self) -> usize {
        self.get("OMP_NUM_THREADS")
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(0)
    }
}

/// Runs tool `tool` with `args` (the arguments after the tool name) in this process's
/// environment. `None` if there is no such tool.
pub fn run(tool: &str, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<u8> {
    run_with(tool, args, &Settings::from_process(), out, err)
}

/// [`run`] with an environment table and a working directory given (the Python wrappers').
pub fn run_in(
    tool: &str,
    args: &[String],
    env: Vec<(String, String)>,
    dir: Option<PathBuf>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<u8> {
    let settings = Settings {
        vars: env.into_iter().collect(),
        dir,
    };
    run_with(tool, args, &settings, out, err)
}

/// [`run`] with the settings given.
pub fn run_with(
    tool: &str,
    args: &[String],
    settings: &Settings,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<u8> {
    match tool {
        "bet2" => Some(bet2::main(args, settings, out, err)),
        "bet" => Some(bet::main(args, settings, out, err)),
        _ => None,
    }
}
