// SPDX-License-Identifier: Apache-2.0
//! The command lines of the `fsl` family's clean-room originals (`larmorx fsl <tool> ...`,
//! FSL's own arguments).
//!
//! FSL's environment variables (`FSLOUTPUTTYPE`, `FSL_LOAD_NIFTI_EXTENSIONS`, `FSLDIR`, `PATH`
//! for `bet -Z`) and larmorx's (`LARMORX_UPSTREAM_DEFECTS`, `OMP_NUM_THREADS` for the thread
//! count) are read into a table of settings ([`Settings`]): the command line fills it from the
//! process environment, the Python wrappers give it explicitly (porting guide §9.1).
//!
//! **FSL-11** (`FSLOUTPUTTYPE` unset): FSL's programs stop with an error at their first image
//! (exit 1). By default every tool here writes `NIFTI_GZ` (`.nii.gz`) in its place and the run
//! names FSL-11 once, at its end, when it wrote an image ([`run_with`]); with `reproduce`, FSL's
//! message and exit 1 where FSL stops.

pub mod aff2rigid;
pub mod applytopup;
pub mod applyxfm4d;
pub mod avscale;
pub mod bet;
pub mod bet2;
pub mod bet_script;
pub mod betsurf;
pub mod bre;
pub mod common;
pub mod convert_xfm;
pub mod coords;
pub mod extracttxt;
pub mod fast;
pub mod flirt;
pub mod fmt;
pub mod fslmaths;
pub mod fslroi;
pub mod fslstats;
pub mod makerot;
pub mod midtrans;
pub mod options;
pub mod pairreg;
pub mod pointflirt;
pub mod prelude;
pub mod rmsdiff;
pub mod ssroi;
pub mod topup;
pub mod topup_debug;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
    "applytopup",
    "fslmaths",
    "fslstats",
    "fslroi",
    "standard_space_roi",
    "prelude",
];

/// The settings of one run: the environment it reads and the directory relative names start
/// in.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub vars: BTreeMap<String, String>,
    pub dir: Option<PathBuf>,
    /// Standard input's bytes for the coordinate programs (`None`: the process's own).
    pub stdin: Option<Vec<u8>>,
    /// FSL-11: whether `FSLOUTPUTTYPE`, unset, was given `NIFTI_GZ` by [`run_with`], and whether an
    /// image was written so (shared by the copies a run's steps make of its settings).
    pub output_type_default: OutputTypeDefault,
}

/// FSL-11's default of one run ([`Settings::output_type_default`]): `None` when the run reads
/// `FSLOUTPUTTYPE` as given; else whether an image was written in `NIFTI_GZ` in its place.
#[derive(Clone, Debug, Default)]
pub struct OutputTypeDefault(Option<Arc<AtomicBool>>);

impl OutputTypeDefault {
    /// Whether an image was written with `NIFTI_GZ` in place of the unset variable.
    pub fn used(&self) -> bool {
        self.0.as_ref().is_some_and(|w| w.load(Ordering::Relaxed))
    }
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
            output_type_default: OutputTypeDefault::default(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(String::as_str)
    }

    /// `FSLOUTPUTTYPE` for an image the run writes now: the variable (`None` when unset, which only
    /// `reproduce` meets: FSL's message and exit 1 follow). Under FSL-11's default ([`run_with`])
    /// it is `NIFTI_GZ`, and the run notes that an image was written so.
    pub fn output_type(&self) -> Option<&str> {
        if let Some(w) = &self.output_type_default.0 {
            w.store(true, Ordering::Relaxed);
        }
        self.get("FSLOUTPUTTYPE")
    }

    /// These settings with FSL-11's default (`FSLOUTPUTTYPE=NIFTI_GZ`) when `FSLOUTPUTTYPE` is
    /// unset and the mode is `fix`; `None` otherwise (a bad mode is the tool's own error).
    fn with_output_type_default(&self) -> Option<Settings> {
        if self.vars.contains_key("FSLOUTPUTTYPE") || self.defects() != Ok(UpstreamDefects::Fix) {
            return None;
        }
        let mut s = self.clone();
        s.vars.insert("FSLOUTPUTTYPE".into(), "NIFTI_GZ".into());
        s.output_type_default = OutputTypeDefault(Some(Arc::new(AtomicBool::new(false))));
        Some(s)
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
        output_type_default: OutputTypeDefault::default(),
    };
    run_with(tool, args, &settings, out, err)
}

/// [`run`] with the settings given. FSL-11: with `FSLOUTPUTTYPE` unset the tool runs with
/// `NIFTI_GZ` by default and the run's last line names FSL-11 when it wrote an image (the tool's
/// own steps, run through here with the variable set, name nothing); with `reproduce` the tool
/// meets the unset variable as FSL does.
pub fn run_with(
    tool: &str,
    args: &[String],
    settings: &Settings,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<u8> {
    let Some(defaulted) = settings.with_output_type_default() else {
        return dispatch(tool, args, settings, out, err);
    };
    let code = dispatch(tool, args, &defaulted, out, err);
    if defaulted.output_type_default.used() {
        common::print_notices(err, &[larmorx_mri::hmc::image::fsl11_notice(tool)]);
    }
    code
}

/// Runs `tool` with `settings` as they are.
fn dispatch(
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
        "applytopup" => Some(applytopup::main(args, settings, out, err)),
        "fslmaths" => Some(fslmaths::main(args, settings, out, err)),
        "fslstats" => Some(fslstats::main(args, settings, out, err)),
        "fslroi" => Some(fslroi::main(args, settings, out, err)),
        "standard_space_roi" => Some(ssroi::main(args, settings, out, err)),
        "prelude" => Some(prelude::main(args, settings, out, err)),
        _ => None,
    }
}
