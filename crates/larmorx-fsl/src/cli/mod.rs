// SPDX-License-Identifier: Apache-2.0
//! The command lines of the `fsl` family's clean-room originals (`larmorx fsl <tool> ...`,
//! FSL's own arguments).
//!
//! FSL's environment variables (`FSLOUTPUTTYPE`, `FSL_LOAD_NIFTI_EXTENSIONS`, `FSLDIR`, `PATH`
//! for `bet -Z`) and larmorx's (`LARMORX_UPSTREAM_DEFECTS`, `OMP_NUM_THREADS` for the thread
//! count) are read into a table of settings ([`Settings`]): the command line fills it from the
//! process environment, the Python wrappers give it explicitly (porting guide §9.1).

pub mod aff2rigid;
pub mod applyxfm4d;
pub mod avscale;
pub mod bet;
pub mod bet2;
pub mod betsurf;
pub mod bre;
pub mod common;
pub mod convert_xfm;
pub mod coords;
pub mod extracttxt;
pub mod fast;
pub mod flirt;
pub mod fmt;
pub mod makerot;
pub mod midtrans;
pub mod options;
pub mod pairreg;
pub mod pointflirt;
pub mod rmsdiff;
pub mod topup;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::defects::{self, UpstreamDefects};

/// The tools on the command line.
pub const TOOLS: &[&str] = &[
    "bet",
    "bet2",
    "betsurf",
    "flirt",
    "convert_xfm",
    "avscale",
    "img2imgcoord",
    "std2imgcoord",
    "img2stdcoord",
    "rmsdiff",
    "makerot",
    "midtrans",
    "pointflirt",
    "applyxfm4D",
    "pairreg",
    "extracttxt",
    "aff2rigid",
    "fast",
    "topup",
];

/// The settings of one run: the environment it reads and the directory relative names start
/// in.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub vars: BTreeMap<String, String>,
    pub dir: Option<PathBuf>,
    /// Standard input's bytes for the coordinate programs (`None`: the process's own).
    pub stdin: Option<Vec<u8>>,
}

impl Settings {
    /// From the process environment and directory.
    pub fn from_process() -> Settings {
        Settings {
            vars: std::env::vars_os()
                .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
                .collect(),
            dir: None,
            stdin: None,
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
        stdin: None,
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
        "betsurf" => Some(betsurf::main(args, settings, out, err)),
        "flirt" => Some(flirt::main(args, settings, out, err)),
        "convert_xfm" => Some(convert_xfm::main(args, settings, out, err)),
        "avscale" => Some(avscale::main(args, settings, out, err)),
        "img2imgcoord" => Some(coords::main(
            coords::Program::Img2Img,
            args,
            settings,
            out,
            err,
        )),
        "std2imgcoord" => Some(coords::main(
            coords::Program::Std2Img,
            args,
            settings,
            out,
            err,
        )),
        "img2stdcoord" => Some(coords::main(
            coords::Program::Img2Std,
            args,
            settings,
            out,
            err,
        )),
        "rmsdiff" => Some(rmsdiff::main(args, settings, out, err)),
        "makerot" => Some(makerot::main(args, settings, out, err)),
        "midtrans" => Some(midtrans::main(args, settings, out, err)),
        "pointflirt" => Some(pointflirt::main(args, settings, out, err)),
        "applyxfm4D" => Some(applyxfm4d::main(args, settings, out, err)),
        "pairreg" => Some(pairreg::main(args, settings, out, err)),
        "extracttxt" => Some(extracttxt::main(args, settings, out, err)),
        "aff2rigid" => Some(aff2rigid::main(args, settings, out, err)),
        "fast" => Some(fast::main(args, settings, out, err)),
        "topup" => Some(topup::main(args, settings, out, err)),
        _ => None,
    }
}
