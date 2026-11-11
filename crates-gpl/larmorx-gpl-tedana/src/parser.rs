// SPDX-License-Identifier: GPL-3.0-only
// Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA Developers; GNU LGPL 2.1), under the
// GNU GPL version 3 as section 3 of the LGPL 2.1 allows; modified 2026 by Karel Lopez Vilaret.
//! `tedana.workflows.t2smap._get_parser` and `_main`'s use of it: every option, its help, its
//! default and its checks (`tedana.workflows.parser_utils.is_valid_file`), and the settings the
//! workflow receives.

use std::path::Path;

use larmorx_core::python::argparse::{
    Argument, Conversion, Exit, Namespace, Nargs, Parser, Type, Value,
};

/// `is_valid_file(parser, arg)`: `parser.error("The file <arg> does not exist!")` unless `arg`
/// names a regular file (symbolic links followed, as `os.path.isfile`).
fn valid_file() -> Type {
    Type::Custom(
        "<lambda>".into(),
        Box::new(|s: &str| {
            if !s.contains('\0') && Path::new(s).is_file() {
                Conversion::Ok(Value::Str(s.to_owned()))
            } else {
                Conversion::ParserError(format!("The file {s} does not exist!"))
            }
        }),
    )
}

/// t2smap's parser (`_get_parser()`), with `prog` `t2smap` as tedana's console script has it.
pub fn parser() -> Parser {
    let mut p = Parser::new("t2smap", true);
    let req = p.add_group("Required Arguments");
    p.add(
        Some(req),
        Argument::store(&["-d"], "data")
            .nargs(Nargs::OneOrMore)
            .metavar("FILE")
            .ty(valid_file())
            .help(
                "Multi-echo dataset for analysis. A set of echo-specific files in ascending \
                 order. The TEs of the data should match the TEs listed in the -e argument.",
            )
            .required(),
    );
    p.add(
        Some(req),
        Argument::store(&["-e"], "tes")
            .nargs(Nargs::OneOrMore)
            .metavar("TE")
            .ty(Type::Float)
            .help(
                "Ascending echo times in seconds (per BIDS convention). E.g., 0.015 0.039 0.063. \
                 Millisecond values (e.g., 15.0 39.0 63.0) are still accepted but deprecated.",
            )
            .required(),
    );
    let out = p.add_group("Output Control");
    p.add(
        Some(out),
        Argument::store(&["--out-dir"], "out_dir")
            .metavar("PATH")
            .help("Output directory.")
            .default(Value::Str(".".into())),
    );
    p.add(
        Some(out),
        Argument::store(&["--prefix"], "prefix")
            .help("Prefix for filenames generated.")
            .default(Value::Str(String::new())),
    );
    p.add(
        Some(out),
        Argument::store(&["--convention"], "convention")
            .choices(&["orig", "bids"])
            .help("Filenaming convention. \"bids\" will use the latest BIDS derivatives version.")
            .default(Value::Str("bids".into())),
    );
    p.add(
        Some(out),
        Argument::store_true(&["--verbose"], "verbose")
            .help("Generate intermediate and additional files."),
    );
    p.add(
        Some(out),
        Argument::store_true(&["--overwrite", "-f"], "overwrite")
            .help("Force overwriting of files."),
    );
    let mask = p.add_group("Temporal and Spatial Masking");
    p.add(
        Some(mask),
        Argument::store(&["--mask"], "mask")
            .metavar("FILE")
            .ty(valid_file())
            .help(
                "Binary mask of voxels to include in TE Dependent ANAlysis. Must be in the same \
                 space as `data`. If an explicit mask is not provided, then Nilearn's \
                 compute_epi_mask function will be used to derive a mask from the first echo's \
                 data. Providing a mask is recommended.",
            ),
    );
    p.add(
        Some(mask),
        Argument::store(&["--masktype"], "masktype")
            .nargs(Nargs::OneOrMore)
            .help(
                "Method(s) by which to define the adaptive mask. The adaptive mask starts with \
                 the mask from '--mask', when provided. It identifies voxels that have good data \
                 in all vs a subset of echoes. \"dropout\" removes voxels with much lower voxels \
                 than other voxels within each echo. \"decay\" removes voxels where the raw \
                 signal does not decay across echoes. Users can specify one, both, or neither of \
                 the models.",
            )
            .choices(&["dropout", "decay", "none"])
            .default(Value::List(vec![Value::Str("dropout".into())])),
    );
    p.add(
        Some(mask),
        Argument::store(&["--dummy-scans"], "dummy_scans")
            .metavar("INT")
            .ty(Type::Int)
            .help("Number of dummy scans to remove from the beginning of the data.")
            .default(Value::Int(0)),
    );
    p.add(
        Some(mask),
        Argument::store(&["--exclude"], "exclude").help(
            "Volume indices to exclude from adaptive mask generation and T2* and S0 estimation, \
             but which will be retained in the optimally combined data. Can be individual \
             indices (e.g., '0,5,10'), ranges (e.g., '5:10'), or a mix of the two (e.g., \
             '0,5:10,15'). Indices are 0-based. As in Python lists, ranges are start-inclusive \
             and end-exclusive (for example, '0:5' includes the first [0] through fifth [4] \
             timepoints). Default is to not exclude any volumes.",
        ),
    );
    let decay = p.add_group("Decay Model Fitting and Optimal Combination");
    p.add(
        Some(decay),
        Argument::store(&["--fittype"], "fittype")
            .choices(&["loglin", "curvefit"])
            .help(
                "Desired T2*/S0 fitting method. \"loglin\" means that a linear model is fit to \
                 the log of the data. \"curvefit\" means that a more computationally demanding \
                 monoexponential model is fit to the raw data. ",
            )
            .default(Value::Str("loglin".into())),
    );
    p.add(
        Some(decay),
        Argument::store(&["--fitmode"], "fitmode")
            .choices(&["all", "ts"])
            .help(
                "Monoexponential model fitting scheme. \"all\" means that the model is fit, per \
                 voxel, across all timepoints. \"ts\" means that the model is fit, per voxel and \
                 per timepoint.",
            )
            .default(Value::Str("all".into())),
    );
    p.add(
        Some(decay),
        Argument::store(&["--combmode"], "combmode")
            .choices(&["t2s", "paid"])
            .help("Combination scheme for TEs: \"t2s\" (Posse 1999), \"paid\" (Poser)")
            .default(Value::Str("t2s".into())),
    );
    let comp = p.add_group("Component Selection");
    p.add(
        Some(comp),
        Argument::store(&["--n-independent-echos"], "n_independent_echos")
            .metavar("INT")
            .ty(Type::Int)
            .help(
                "Number of independent echoes to use in goodness of fit metrics (fstat). \
                 Primarily used for EPTI acquisitions, which have dependency across echoes. If \
                 not provided, number of echoes will be used.",
            ),
    );
    let perf = p.add_group("Performance Control");
    p.add(
        Some(perf),
        Argument::store(&["--n-threads"], "n_threads")
            .metavar("INT")
            .ty(Type::Int)
            .help(
                "Number of threads to use. Used by threadpoolctl to set the parameter outside of \
                 the workflow function. Higher numbers of threads tend to slow down performance \
                 on typical datasets.",
            )
            .default(Value::Int(1)),
    );
    p.add(
        Some(perf),
        Argument::store_true(&["--debug"], "debug").help(
            "Logs in the terminal will have increased verbosity, and will also be written into a \
             TSV file in the output directory.",
        ),
    );
    p.add(None, Argument::store_true(&["--quiet"], "quiet").hidden());
    p.add(
        None,
        Argument::version(
            &["-v", "--version"],
            &format!("t2smap v{}", crate::TEDANA_VERSION),
        ),
    );
    p
}

/// `--fittype`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitType {
    Loglin,
    Curvefit,
}

/// The workflow's arguments (`vars(options)`), typed.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub data: Vec<String>,
    pub tes: Vec<f64>,
    pub out_dir: String,
    pub prefix: String,
    /// `orig` or `bids`.
    pub convention: String,
    pub verbose: bool,
    pub overwrite: bool,
    pub mask: Option<String>,
    pub masktype: Vec<String>,
    pub dummy_scans: i128,
    pub exclude: Option<String>,
    pub fittype: FitType,
    /// `fitmode == "ts"`.
    pub fit_ts: bool,
    /// `combmode == "paid"`.
    pub paid: bool,
    pub n_independent_echos: Option<i128>,
    /// `--n-threads` (`-1` means no limit, `None` in `threadpool_limits`).
    pub n_threads: i128,
    /// Whether `--n-threads` was given (larmorx's addition): when it was not, the run's thread
    /// count applies (`workflow::Env::threads`, larmorx's own `--n-threads`), not tedana's
    /// default of 1. The results do not depend on it.
    pub n_threads_given: bool,
    pub debug: bool,
    pub quiet: bool,
}

fn strings(v: &Value) -> Vec<String> {
    v.as_list()
        .unwrap_or_default()
        .iter()
        .map(|s| s.as_str().unwrap_or_default().to_owned())
        .collect()
}

impl Settings {
    fn from_namespace(ns: &Namespace, given: &[String]) -> Settings {
        let s = |d: &str| ns.get(d).as_str().unwrap_or_default().to_owned();
        let opt = |d: &str| ns.get(d).as_str().map(str::to_owned);
        let int = |d: &str| match ns.get(d) {
            Value::Int(i) => Some(*i),
            _ => None,
        };
        Settings {
            data: strings(ns.get("data")),
            tes: ns
                .get("tes")
                .as_list()
                .unwrap_or_default()
                .iter()
                .map(|v| match v {
                    Value::Float(f) => *f,
                    _ => f64::NAN,
                })
                .collect(),
            out_dir: s("out_dir"),
            prefix: s("prefix"),
            convention: s("convention"),
            verbose: ns.get("verbose").as_bool(),
            overwrite: ns.get("overwrite").as_bool(),
            mask: opt("mask"),
            masktype: strings(ns.get("masktype")),
            dummy_scans: int("dummy_scans").unwrap_or(0),
            exclude: opt("exclude"),
            fittype: if s("fittype") == "curvefit" {
                FitType::Curvefit
            } else {
                FitType::Loglin
            },
            fit_ts: s("fitmode") == "ts",
            paid: s("combmode") == "paid",
            n_independent_echos: int("n_independent_echos"),
            n_threads: int("n_threads").unwrap_or(1),
            n_threads_given: given.iter().any(|d| d == "n_threads"),
            debug: ns.get("debug").as_bool(),
            quiet: ns.get("quiet").as_bool(),
        }
    }
}

/// Parses `t2smap`'s arguments (without the program name) at argparse's formatter `width`
/// (`larmorx_core::python::argparse::formatter_width`).
pub fn parse(args: &[String], width: isize) -> Result<Settings, Exit> {
    parser()
        .parse_given(args, width)
        .map(|(ns, given)| Settings::from_namespace(&ns, &given))
}
