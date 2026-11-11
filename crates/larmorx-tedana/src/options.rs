// SPDX-License-Identifier: Apache-2.0
//! The options of a run (`specs/t2smap.md` §2.2, §14).

use std::path::PathBuf;

use larmorx_core::defects::UpstreamDefects;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskType {
    Dropout,
    Decay,
    None,
}

impl MaskType {
    pub fn parse(s: &str) -> Option<MaskType> {
        Some(match s {
            "dropout" => MaskType::Dropout,
            "decay" => MaskType::Decay,
            "none" => MaskType::None,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitType {
    Loglin,
    Curvefit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitMode {
    All,
    Ts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombMode {
    T2s,
    Paid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Convention {
    Bids,
    Orig,
}

/// Whose numpy and SciPy results are reproduced where their releases differ (§13.4–§13.6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Libraries {
    /// numpy 2.5.4, SciPy 1.18.1 (the default).
    #[default]
    Newest,
    /// numpy 2.3.5, SciPy 1.15.3 (the shared environment).
    Shared,
}

impl Libraries {
    /// The environment variable of the command line (§14).
    pub const ENV: &'static str = "LARMORX_TEDANA_LIBRARIES";

    pub fn parse(s: &str) -> Option<Libraries> {
        match s {
            "newest" => Some(Libraries::Newest),
            "shared" => Some(Libraries::Shared),
            _ => None,
        }
    }
}

/// One run of `t2smap` (§2.2): the parsed options, and larmorx's own settings.
#[derive(Clone, Debug)]
pub struct Options {
    /// `-d`, as given.
    pub data: Vec<String>,
    /// `-e`, as given (seconds or milliseconds).
    pub tes: Vec<f64>,
    pub out_dir: String,
    pub prefix: String,
    pub convention: Convention,
    pub verbose: bool,
    pub overwrite: bool,
    pub mask: Option<String>,
    pub masktype: Vec<MaskType>,
    pub dummy_scans: i128,
    pub exclude: Option<String>,
    pub fittype: FitType,
    pub fitmode: FitMode,
    pub combmode: CombMode,
    pub n_independent_echos: Option<i128>,
    /// `--n-threads` (≤ 0: every logical CPU). The results do not depend on it.
    pub n_threads: i128,
    pub debug: bool,
    pub quiet: bool,
    pub libraries: Libraries,
    pub upstream_defects: UpstreamDefects,
    /// The working directory relative paths resolve against (`None`: the process's).
    pub cwd: Option<PathBuf>,
    /// The words written to `t2smap_call.sh` after `t2smap ` (§11.6).
    pub call_args: Vec<String>,
}

impl Options {
    /// The defaults of §2.2 for these echoes and echo times.
    pub fn new(data: Vec<String>, tes: Vec<f64>) -> Options {
        Options {
            data,
            tes,
            out_dir: ".".into(),
            prefix: String::new(),
            convention: Convention::Bids,
            verbose: false,
            overwrite: false,
            mask: None,
            masktype: vec![MaskType::Dropout],
            dummy_scans: 0,
            exclude: None,
            fittype: FitType::Loglin,
            fitmode: FitMode::All,
            combmode: CombMode::T2s,
            n_independent_echos: None,
            n_threads: 1,
            debug: false,
            quiet: false,
            libraries: Libraries::Newest,
            upstream_defects: UpstreamDefects::Fix,
            cwd: None,
            call_args: Vec::new(),
        }
    }

    /// The thread count rayon gets (0: every logical CPU).
    pub fn threads(&self) -> usize {
        if self.n_threads <= 0 {
            0
        } else {
            usize::try_from(self.n_threads)
                .unwrap_or(usize::MAX)
                .min(4096)
        }
    }
}
