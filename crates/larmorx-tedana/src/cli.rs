// SPDX-License-Identifier: Apache-2.0
//! `t2smap`'s command line (`specs/t2smap.md` §2, §14): tedana's argparse parser, its help and
//! errors, the environment's settings, the run, the traceback and the notices.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_core::python::argparse::{
    Argument, Conversion, Namespace, Nargs, Parser, Type, Value, formatter_width,
};

use crate::input::resolve;
use crate::log::Log;
use crate::options::{CombMode, Convention, FitMode, FitType, Libraries, MaskType, Options};

/// tedana's version line (`-v`).
pub const VERSION: &str = "t2smap v26.0.3";

/// The environment as the command line reads it (§9.1 of the porting guide): the variables,
/// matched by name (whatever their case on Windows), and the working directory.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub vars: HashMap<String, String>,
    pub dir: Option<PathBuf>,
    /// The width of the terminal standard output writes to, when it is one: argparse's width
    /// when `COLUMNS` is unset (§2.3; Python's `shutil.get_terminal_size`). The program that
    /// owns the terminal fills it; without it the width is 80.
    pub terminal_columns: Option<usize>,
}

impl Settings {
    /// The process's environment and working directory.
    pub fn from_process() -> Settings {
        Settings {
            vars: std::env::vars().collect(),
            dir: None,
            terminal_columns: None,
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        if cfg!(windows) {
            self.vars
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        } else {
            self.vars.get(name).map(String::as_str)
        }
    }
}

fn is_valid_file(dir: Option<PathBuf>) -> Type {
    Type::Custom(
        "is_valid_file".into(),
        Box::new(move |s: &str| {
            if resolve(dir.as_deref(), s).is_file() {
                Conversion::Ok(Value::Str(s.to_owned()))
            } else {
                Conversion::ParserError(format!("The file {s} does not exist!"))
            }
        }),
    )
}

/// tedana 26.0.3's parser (§2.2, §2.3).
pub fn parser(dir: Option<PathBuf>) -> Parser {
    let mut p = Parser::new("t2smap", true);
    let req = p.add_group("Required Arguments");
    p.add(
        Some(req),
        Argument::store(&["-d"], "data")
            .nargs(Nargs::OneOrMore)
            .metavar("FILE")
            .ty(is_valid_file(dir.clone()))
            .required()
            .help(
                "Multi-echo dataset for analysis. A set of echo-specific files in ascending \
                 order. The TEs of the data should match the TEs listed in the -e argument.",
            ),
    );
    p.add(
        Some(req),
        Argument::store(&["-e"], "tes")
            .nargs(Nargs::OneOrMore)
            .metavar("TE")
            .ty(Type::Float)
            .required()
            .help(
                "Ascending echo times in seconds (per BIDS convention). E.g., 0.015 0.039 \
                 0.063. Millisecond values (e.g., 15.0 39.0 63.0) are still accepted but \
                 deprecated.",
            ),
    );
    let out = p.add_group("Output Control");
    p.add(
        Some(out),
        Argument::store(&["--out-dir"], "out_dir")
            .metavar("PATH")
            .default(Value::Str(".".into()))
            .help("Output directory."),
    );
    p.add(
        Some(out),
        Argument::store(&["--prefix"], "prefix")
            .metavar("PREFIX")
            .default(Value::Str(String::new()))
            .help("Prefix for filenames generated."),
    );
    p.add(
        Some(out),
        Argument::store(&["--convention"], "convention")
            .choices(&["orig", "bids"])
            .default(Value::Str("bids".into()))
            .help("Filenaming convention. \"bids\" will use the latest BIDS derivatives version."),
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
    let masking = p.add_group("Temporal and Spatial Masking");
    p.add(
        Some(masking),
        Argument::store(&["--mask"], "mask")
            .metavar("FILE")
            .ty(is_valid_file(dir))
            .help(
                "Binary mask of voxels to include in TE Dependent ANAlysis. Must be in the same \
                 space as `data`. If an explicit mask is not provided, then Nilearn's \
                 compute_epi_mask function will be used to derive a mask from the first echo's \
                 data. Providing a mask is recommended.",
            ),
    );
    p.add(
        Some(masking),
        Argument::store(&["--masktype"], "masktype")
            .nargs(Nargs::OneOrMore)
            .choices(&["dropout", "decay", "none"])
            .default(Value::List(vec![Value::Str("dropout".into())]))
            .help(
                "Method(s) by which to define the adaptive mask. The adaptive mask starts with \
                 the mask from '--mask', when provided. It identifies voxels that have good data \
                 in all vs a subset of echoes. \"dropout\" removes voxels with much lower voxels \
                 than other voxels within each echo. \"decay\" removes voxels where the raw \
                 signal does not decay across echoes. Users can specify one, both, or neither \
                 of the models.",
            ),
    );
    p.add(
        Some(masking),
        Argument::store(&["--dummy-scans"], "dummy_scans")
            .metavar("INT")
            .ty(Type::Int)
            .default(Value::Int(0))
            .help("Number of dummy scans to remove from the beginning of the data."),
    );
    p.add(
        Some(masking),
        Argument::store(&["--exclude"], "exclude").help(
            "Volume indices to exclude from adaptive mask generation and T2* and S0 \
             estimation, but which will be retained in the optimally combined data. Can be \
             individual indices (e.g., '0,5,10'), ranges (e.g., '5:10'), or a mix of the two \
             (e.g., '0,5:10,15'). Indices are 0-based. As in Python lists, ranges are \
             start-inclusive and end-exclusive (for example, '0:5' includes the first [0] \
             through fifth [4] timepoints). Default is to not exclude any volumes.",
        ),
    );
    let fit = p.add_group("Decay Model Fitting and Optimal Combination");
    p.add(
        Some(fit),
        Argument::store(&["--fittype"], "fittype")
            .choices(&["loglin", "curvefit"])
            .default(Value::Str("loglin".into()))
            .help(
                "Desired T2*/S0 fitting method. \"loglin\" means that a linear model is fit to \
                 the log of the data. \"curvefit\" means that a more computationally demanding \
                 monoexponential model is fit to the raw data.",
            ),
    );
    p.add(
        Some(fit),
        Argument::store(&["--fitmode"], "fitmode")
            .choices(&["all", "ts"])
            .default(Value::Str("all".into()))
            .help(
                "Monoexponential model fitting scheme. \"all\" means that the model is fit, per \
                 voxel, across all timepoints. \"ts\" means that the model is fit, per voxel and \
                 per timepoint.",
            ),
    );
    p.add(
        Some(fit),
        Argument::store(&["--combmode"], "combmode")
            .choices(&["t2s", "paid"])
            .default(Value::Str("t2s".into()))
            .help("Combination scheme for TEs: \"t2s\" (Posse 1999), \"paid\" (Poser)"),
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
            .default(Value::Int(1))
            .help(
                "Number of threads to use. Used by threadpoolctl to set the parameter outside of \
                 the workflow function. Higher numbers of threads tend to slow down performance \
                 on typical datasets.",
            ),
    );
    p.add(
        Some(perf),
        Argument::store_true(&["--debug"], "debug").help(
            "Logs in the terminal will have increased verbosity, and will also be written into \
             a TSV file in the output directory.",
        ),
    );
    p.add(
        Some(perf),
        Argument::store_true(&["--quiet"], "quiet").hidden(),
    );
    p.add(None, Argument::version(&["-v", "--version"], VERSION));
    p
}

fn strings(v: &Value) -> Vec<String> {
    v.as_list()
        .unwrap_or(&[])
        .iter()
        .map(|x| x.as_str().unwrap_or_default().to_owned())
        .collect()
}

fn int(v: &Value) -> Option<i128> {
    match v {
        Value::Int(i) => Some(*i),
        _ => None,
    }
}

/// The run's options from the parsed values.
pub fn options(ns: &Namespace, args: &[String], settings: &Settings) -> Options {
    let tes: Vec<f64> = ns
        .get("tes")
        .as_list()
        .unwrap_or(&[])
        .iter()
        .map(|x| match x {
            Value::Float(f) => *f,
            _ => f64::NAN,
        })
        .collect();
    let mut o = Options::new(strings(ns.get("data")), tes);
    o.out_dir = ns.get("out_dir").as_str().unwrap_or(".").to_owned();
    o.prefix = ns.get("prefix").as_str().unwrap_or("").to_owned();
    o.convention = if ns.get("convention").as_str() == Some("orig") {
        Convention::Orig
    } else {
        Convention::Bids
    };
    o.verbose = ns.get("verbose").as_bool();
    o.overwrite = ns.get("overwrite").as_bool();
    o.mask = ns.get("mask").as_str().map(str::to_owned);
    o.masktype = strings(ns.get("masktype"))
        .iter()
        .filter_map(|s| MaskType::parse(s))
        .collect();
    o.dummy_scans = int(ns.get("dummy_scans")).unwrap_or(0);
    o.exclude = ns.get("exclude").as_str().map(str::to_owned);
    o.fittype = if ns.get("fittype").as_str() == Some("curvefit") {
        FitType::Curvefit
    } else {
        FitType::Loglin
    };
    o.fitmode = if ns.get("fitmode").as_str() == Some("ts") {
        FitMode::Ts
    } else {
        FitMode::All
    };
    o.combmode = if ns.get("combmode").as_str() == Some("paid") {
        CombMode::Paid
    } else {
        CombMode::T2s
    };
    o.n_independent_echos = int(ns.get("n_independent_echos"));
    o.n_threads = int(ns.get("n_threads")).unwrap_or(1);
    o.debug = ns.get("debug").as_bool();
    o.quiet = ns.get("quiet").as_bool();
    o.cwd = settings.dir.clone();
    o.call_args = args.to_vec();
    o
}

/// `larmorx tedana t2smap <args>` (`args` after `t2smap`): the exit status.
pub fn run(
    args: &[String],
    settings: &Settings,
    out: &mut dyn Write,
    err: &mut (dyn Write + Send),
) -> u8 {
    let mode =
        match UpstreamDefects::from_value(settings.get(defects::ENV).map(std::ffi::OsStr::new)) {
            Ok(m) => m,
            Err(e) => {
                let _ = writeln!(err, "larmorx: {e}");
                return defects::EXIT_BAD_MODE;
            }
        };
    let libraries = match settings.get(Libraries::ENV).filter(|v| !v.is_empty()) {
        None => Libraries::Newest,
        Some(v) => match Libraries::parse(v) {
            Some(l) => l,
            None => {
                let _ = writeln!(
                    err,
                    "larmorx: {} must be 'newest' or 'shared', not '{v}'",
                    Libraries::ENV
                );
                return 2;
            }
        },
    };
    let width = formatter_width(settings.get("COLUMNS"), settings.terminal_columns);
    let ns = match parser(settings.dir.clone()).parse(args, width) {
        Ok(ns) => ns,
        Err(exit) => {
            let _ = out.write_all(exit.stdout.as_bytes());
            let _ = err.write_all(exit.stderr.as_bytes());
            return u8::try_from(exit.status).unwrap_or(1);
        }
    };
    let mut o = options(&ns, args, settings);
    o.libraries = libraries;
    o.upstream_defects = mode;
    let mut notices = Notices::new(mode);
    let status = {
        let mut log = Log::new(err, o.debug, o.quiet);
        match crate::t2smap(&o, &mut log, &mut notices) {
            Ok(_) => 0,
            Err(e) => {
                log.raw(&e.traceback());
                1
            }
        }
    };
    notices.print(err);
    status
}
