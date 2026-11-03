// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl topup`: FSL 6.0.7.23's `topup` command line (`specs/topup.md` §2, §3, §5, §6,
//! §8), on the clean-room estimation [`crate::topup`].
//!
//! Options are `--name=value` words (`-v` and `-h` also short); per-level options take
//! comma-separated lists; `--config` reads more options from a file, the command line winning.
//! The settings are checked in FSL's order with its messages (after the usage text), the log of
//! the settings is written, the field estimated level by level, and the outputs written in
//! FSL's order.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_io::nifti::NiftiHeader;
use larmorx_mri::hmc::files;

use super::Settings;
use super::bet2::{FSLOUTPUTTYPE_UNSET, resolve_image};
use super::common::{image_exception, print_notices, read_rows};
use super::fmt::g;
use super::options::atof;
use crate::bet::image::{self, Output, Values};
use crate::topup::{self, LevelSpec, Minimiser, Params, Progress, RegMod, Scan};

/// The usage text (standard error, before every option or setting error).
pub const USAGE: &str = "\nPart of FSL (ID: 2412.6-dirty)\ntopup\n\nUsage: \ntopup --imain=<some 4D image> --datain=<text file> --config=<text file with parameters> --out=my_topup_results\n\n\nCompulsory arguments (You MUST set one or more of):\n\t--imain\t\tname of 4D file with images\n\nOptional arguments (You may optionally specify one or more of):\n\t--datain\tname of text file with PE directions/times\n\t--acqp\t\talternative way to specify text file with PE directions/times\n\t--out\t\tbase-name of output files (spline coefficients (Hz) and movement parameters)\n\t--fout\t\tname of image file with field (Hz)\n\t--iout\t\tname of 4D image file with unwarped images\n\t--featout\t\tbase-name of output for export to FEAT\n\t--logout\tName of log-file\n\t--warpres\t(approximate) resolution (in mm) of warp basis for the different sub-sampling levels, default 10\n\t--subsamp\tsub-sampling scheme, default 1\n\t--fwhm\t\tFWHM (in mm) of gaussian smoothing kernel, default 8\n\t--config\tName of config file specifying command line arguments\n\t--miter\t\tMax # of non-linear iterations, default 5\n\t--lambda\tWeight of regularisation, default depending on --ssqlambda and --regmod switches. See user documetation.\n\t--ssqlambda\tIf set (=1), lambda is weighted by current ssq, default 1\n\t--regmod\tModel for regularisation of warp-field [membrane_energy bending_energy], default bending_energy\n\t--estmov\tEstimate movements if set, default 1 (true)\n\t--minmet\tMinimisation method 0=Levenberg-Marquardt, 1=Scaled Conjugate Gradient, default 0 (LM)\n\t--splineorder\tOrder of spline, 2->Qadratic spline, 3->Cubic spline. Default=3\n\t--numprec\tPrecision for representing Hessian, double or float. Default double\n\t--interp\tImage interpolation model, linear or spline. Default spline\n\t--scale\t\tIf set (=1), the images are individually scaled to a common mean, default 0 (false)\n\t--regrid\t\tIf set (=1), the calculations are done in a different grid, default 1 (true)\n\t--nthr\t\tNumber of threads to use (cannot be greater than numbers of hardware cores), default 1\n\t-h,--help\tdisplay help info\n\t-v,--verbose\tPrint diagonostic information while running\n\t-h,--help\tdisplay help info\n\n\n";

/// The `descrip` of the images written.
pub const DESCRIP: &str = "larmorx fsl topup";

/// How an option's value is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Text,
    Int,
    FloatList,
    IntList,
    Flag,
}

/// The options: long name, kind, the help text the log repeats (`None`: hidden).
const OPTIONS: &[(&str, Kind, Option<&str>)] = &[
    ("imain", Kind::Text, Some("\tname of 4D file with images")),
    (
        "datain",
        Kind::Text,
        Some("name of text file with PE directions/times"),
    ),
    (
        "acqp",
        Kind::Text,
        Some("\talternative way to specify text file with PE directions/times"),
    ),
    (
        "out",
        Kind::Text,
        Some("\tbase-name of output files (spline coefficients (Hz) and movement parameters)"),
    ),
    (
        "fout",
        Kind::Text,
        Some("\tname of image file with field (Hz)"),
    ),
    (
        "iout",
        Kind::Text,
        Some("\tname of 4D image file with unwarped images"),
    ),
    (
        "featout",
        Kind::Text,
        Some("\tbase-name of output for export to FEAT"),
    ),
    ("logout", Kind::Text, Some("Name of log-file")),
    (
        "warpres",
        Kind::FloatList,
        Some(
            "(approximate) resolution (in mm) of warp basis for the different sub-sampling levels, default 10",
        ),
    ),
    (
        "subsamp",
        Kind::IntList,
        Some("sub-sampling scheme, default 1"),
    ),
    (
        "fwhm",
        Kind::FloatList,
        Some("\tFWHM (in mm) of gaussian smoothing kernel, default 8"),
    ),
    (
        "config",
        Kind::Text,
        Some("Name of config file specifying command line arguments"),
    ),
    (
        "miter",
        Kind::IntList,
        Some("\tMax # of non-linear iterations, default 5"),
    ),
    (
        "lambda",
        Kind::FloatList,
        Some(
            "Weight of regularisation, default depending on --ssqlambda and --regmod switches. See user documetation.",
        ),
    ),
    (
        "ssqlambda",
        Kind::Int,
        Some("If set (=1), lambda is weighted by current ssq, default 1"),
    ),
    (
        "regmod",
        Kind::Text,
        Some(
            "Model for regularisation of warp-field [membrane_energy bending_energy], default bending_energy",
        ),
    ),
    (
        "estmov",
        Kind::IntList,
        Some("Estimate movements if set, default 1 (true)"),
    ),
    (
        "minmet",
        Kind::IntList,
        Some(
            "Minimisation method 0=Levenberg-Marquardt, 1=Scaled Conjugate Gradient, default 0 (LM)",
        ),
    ),
    (
        "splineorder",
        Kind::Int,
        Some("Order of spline, 2->Qadratic spline, 3->Cubic spline. Default=3"),
    ),
    (
        "numprec",
        Kind::Text,
        Some("Precision for representing Hessian, double or float. Default double"),
    ),
    (
        "interp",
        Kind::Text,
        Some("Image interpolation model, linear or spline. Default spline"),
    ),
    (
        "scale",
        Kind::Int,
        Some(
            "\tIf set (=1), the images are individually scaled to a common mean, default 0 (false)",
        ),
    ),
    (
        "regrid",
        Kind::Int,
        Some("\tIf set (=1), the calculations are done in a different grid, default 1 (true)"),
    ),
    (
        "nthr",
        Kind::Int,
        Some(
            "\tNumber of threads to use (cannot be greater than numbers of hardware cores), default 1",
        ),
    ),
    ("help", Kind::Flag, None),
    ("verbose", Kind::Flag, None),
    ("dfout", Kind::Text, None),
    ("rbmout", Kind::Text, None),
    ("jacout", Kind::Text, None),
    ("debug", Kind::Int, None),
    ("trace", Kind::Flag, None),
    ("profile", Kind::Flag, None),
];

/// The words given for each option (the last occurrence wins), in the order of [`OPTIONS`].
#[derive(Clone, Debug, Default)]
struct Given {
    values: std::collections::BTreeMap<&'static str, String>,
}

impl Given {
    fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    fn has(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }
}

/// An option word parsed (`Err`: FSL's message after the usage).
fn parse_word(word: &str, given: &mut Given) -> Result<(), String> {
    let (name, value) = if let Some(rest) = word.strip_prefix("--") {
        match rest.split_once('=') {
            Some((n, v)) => (n.to_owned(), Some(v.to_owned())),
            None => (rest.to_owned(), None),
        }
    } else if word == "-v" {
        ("verbose".to_owned(), None)
    } else if word == "-h" {
        ("help".to_owned(), None)
    } else {
        return Err(format!("{word}: Option doesn't exist!"));
    };
    let Some(&(key, kind, _)) = OPTIONS.iter().find(|o| o.0 == name) else {
        let shown = word.split_once('=').map_or(word, |(n, _)| n);
        return Err(format!("{shown}: Option doesn't exist!"));
    };
    let display = format!("--{key}");
    match kind {
        Kind::Flag => {
            given.values.insert(key, String::new());
        }
        _ => {
            let Some(v) = value else {
                return Err(format!("{display}: Missing non-optional argument!"));
            };
            if kind == Kind::Int && read_int(&v).is_none() {
                return Err(format!("{display}: Couldn't set_value! valstr=\"{v}\"!"));
            }
            given.values.insert(key, v);
        }
    }
    Ok(())
}

/// An integer as a C++ stream reads one: leading blanks, a sign and digits (the rest ignored).
fn read_int(v: &str) -> Option<i64> {
    let t = v.trim_start();
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == start {
        return None;
    }
    t[..i]
        .parse::<i64>()
        .ok()
        .map(|x| x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)))
}

/// C's `atoi` (the leading number, else 0).
fn atoi(v: &str) -> i64 {
    read_int(v).unwrap_or(0)
}

fn float_list(v: &str) -> Vec<f64> {
    v.split(',').map(atof).collect()
}

fn int_list(v: &str) -> Vec<i64> {
    v.split(',').map(atoi).collect()
}

/// Prints the usage and a message; exit status 1.
fn usage_error(err: &mut dyn Write, notices: &[Notice], message: &str) -> u8 {
    print_notices(err, notices);
    let _ = err.write_all(USAGE.as_bytes());
    if !message.is_empty() {
        let _ = writeln!(err, "\n{message}");
    }
    1
}

/// A refusal of the original (larmorx's message), exit status 1.
fn refuse(err: &mut dyn Write, notices: &[Notice], why: &str) -> u8 {
    print_notices(err, notices);
    let _ = writeln!(err, "larmorx fsl topup: {why} (larmorx)");
    1
}

/// Finds a configuration file (§2.5): as named, with `.cnf`, or in `$FSLDIR/etc/flirtsch/`.
fn find_config(settings: &Settings, name: &str) -> Result<Option<std::path::PathBuf>, ()> {
    let p = settings.path(name);
    if p.exists() {
        return Ok(Some(p));
    }
    let has_ext = std::path::Path::new(name).extension().is_some();
    if !has_ext {
        let q = settings.path(&format!("{name}.cnf"));
        if q.exists() {
            return Ok(Some(q));
        }
    }
    let has_dir = name.contains('/');
    if !has_dir {
        let Some(fsldir) = settings.get("FSLDIR") else {
            return Err(());
        };
        let base = std::path::Path::new(fsldir).join("etc").join("flirtsch");
        let r = base.join(name);
        if r.exists() {
            return Ok(Some(r));
        }
        if !has_ext {
            let s = base.join(format!("{name}.cnf"));
            if s.exists() {
                return Ok(Some(s));
            }
        }
    }
    Ok(None)
}

/// The `topup` command line.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl topup: {e}");
            return 1;
        }
    };
    let mut notices: Vec<Notice> = Vec::new();
    if args.is_empty() {
        return usage_error(err, &notices, "");
    }
    let mut given = Given::default();
    for a in args {
        if let Err(m) = parse_word(a, &mut given) {
            return usage_error(err, &notices, &m);
        }
    }
    if given.has("help") {
        return usage_error(err, &notices, "");
    }
    if !given.has("imain") {
        return usage_error(err, &notices, "");
    }
    // the configuration: its options where the command line has none
    if let Some(cfg) = given.get("config").map(str::to_owned) {
        let path = match find_config(settings, &cfg) {
            Ok(Some(p)) => p,
            Ok(None) => {
                let _ = writeln!(err, "Cannot find config-file {cfg}");
                return 1;
            }
            Err(()) => {
                let n = Notice::new(
                    &defects::FSL_121,
                    mode,
                    "FSLDIR is unset and the configuration is not found here",
                );
                return match mode {
                    UpstreamDefects::Fix => refuse(
                        err,
                        &[n],
                        &format!(
                            "--config={cfg}: no such file here, and FSLDIR is unset (FSL-121)"
                        ),
                    ),
                    UpstreamDefects::Reproduce => {
                        print_notices(err, &[n]);
                        let _ = writeln!(err, "basic_string: construction from null is not valid");
                        1
                    }
                };
            }
        };
        if path.is_file() {
            let text = std::fs::read(&path).unwrap_or_default();
            let text = String::from_utf8_lossy(&text);
            let mut from_file = Given::default();
            for line in text.lines() {
                let Some(word) = line.split_whitespace().next() else {
                    continue;
                };
                if word.starts_with('#') {
                    continue;
                }
                if let Err(m) = parse_word(word, &mut from_file) {
                    return usage_error(err, &notices, &m);
                }
            }
            for (k, v) in from_file.values {
                given.values.entry(k).or_insert(v);
            }
        }
    }
    run(&given, settings, mode, &mut notices, out, err)
}

/// The settings after the parse.
struct Settled {
    imain: String,
    out: String,
    logout: String,
    params: Params,
    scans: Vec<Scan>,
    volumes: Vec<Vec<f32>>,
    input: image::Input,
    verbose: bool,
    n_threads: usize,
}

fn run(
    given: &Given,
    settings: &Settings,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let s = match settle(given, settings, mode, notices, err) {
        Ok(s) => s,
        Err(code) => return code,
    };
    write_log(given, settings, &s.logout);
    if let Some(d) = given.get("debug") {
        let level = atoi(d);
        if !(0..=3).contains(&level) {
            print_notices(err, notices);
            let _ = writeln!(
                err,
                "Error occurred when preparing to run topup\nException thrown with message: Topup::TopupCF::SetDebug: Debug level out of range"
            );
            return 1;
        }
        if level > 0 {
            return refuse(
                err,
                notices,
                "--debug is not implemented yet in the clean-room original",
            );
        }
    }
    for flag in ["trace", "profile"] {
        if given.has(flag) {
            return refuse(
                err,
                notices,
                &format!("--{flag} is not implemented yet in the clean-room original"),
            );
        }
    }
    let dims = s.input.vol.shape;
    let voxel = s.input.vol.voxel;
    // on the call's thread pool; the streams buffered there and copied after
    let mut obuf: Vec<u8> = Vec::new();
    let mut ebuf: Vec<u8> = Vec::new();
    let mut ns = std::mem::take(notices);
    let pooled = larmorx_core::parallel::with_threads(s.n_threads, || {
        let est = {
            let mut progress = Printer {
                out: &mut obuf,
                verbose: s.verbose,
            };
            topup::estimate(&s.volumes, dims, voxel, &s.scans, &s.params, &mut progress)
        };
        write_outputs(given, settings, mode, &s, &est, &mut ns, &mut ebuf)
    });
    let _ = out.write_all(&obuf);
    let _ = err.write_all(&ebuf);
    match pooled {
        Ok(code) => code,
        Err(e) => refuse(err, &ns, &e.to_string()),
    }
}

/// `-v`'s lines.
struct Printer<'a> {
    out: &'a mut Vec<u8>,
    verbose: bool,
}

impl Progress for Printer<'_> {
    fn evaluated(&mut self, level: &topup::model::Level, e: &topup::model::Eval) {
        if self.verbose {
            let reg = e.cost - e.ssd;
            let _ = writeln!(
                self.out,
                "SSD = {}\tn = {}\tReg = {}\tCost = {}",
                g(e.ssd),
                level.n_mask,
                g(reg),
                g(e.cost)
            );
        }
    }

    fn next_level(&mut self, _index: usize) {
        if self.verbose {
            let _ = writeln!(self.out, "***Going to next resolution level***");
        }
    }
}

/// The checks of §2.6 in FSL's order; the inputs read.
fn settle(
    given: &Given,
    settings: &Settings,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
    err: &mut dyn Write,
) -> Result<Settled, u8> {
    let imain = given.get("imain").unwrap_or_default().to_owned();
    // the image
    let files = match resolve_image(settings, &imain, mode, notices) {
        Ok(f) => f,
        Err(e) => {
            let (lines, what) = image_exception(&imain, &e);
            let _ = err.write_all(lines.as_bytes());
            return Err(usage_error(err, notices, &what));
        }
    };
    let (vols, input) = match image::read(&files, settings.threads()) {
        Ok(r) => r,
        Err(e) => {
            let (lines, what) = image_exception(&imain, &e);
            let _ = err.write_all(lines.as_bytes());
            return Err(usage_error(err, notices, &what));
        }
    };
    let datain = given.get("datain").filter(|v| !v.is_empty());
    let acqp = given.get("acqp").filter(|v| !v.is_empty());
    let clp = |m: &str| format!("Topup::topup_clp::topup_clp: {m}");
    let acq_name = match (datain, acqp) {
        (Some(_), Some(_)) => {
            return Err(usage_error(
                err,
                notices,
                &clp("Only one of --datain and --acqp can be set."),
            ));
        }
        (None, None) => {
            return Err(usage_error(
                err,
                notices,
                &clp("Exactly one of --datain and --acqp must be set."),
            ));
        }
        (Some(d), None) | (None, Some(d)) => d.to_owned(),
    };
    let reader_error = |err: &mut dyn Write, notices: &[Notice]| {
        usage_error(
            err,
            notices,
            &format!("TopupFileIO::TopupDatafileReader:: error reading file {acq_name}"),
        )
    };
    let rows = match read_rows(settings, &acq_name, err) {
        Ok(r) => r,
        Err(_) => return Err(reader_error(err, notices)),
    };
    let ncols = rows.first().map_or(0, Vec::len);
    let acq: Vec<[f64; 4]> = if !rows.is_empty() && ncols == 4 {
        rows.iter().map(|r| [r[0], r[1], r[2], r[3]]).collect()
    } else if rows.len() == 4 && ncols > 0 {
        (0..ncols)
            .map(|c| [rows[0][c], rows[1][c], rows[2][c], rows[3][c]])
            .collect()
    } else {
        return Err(reader_error(err, notices));
    };
    if acq
        .iter()
        .any(|r| ((r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt() - 1.0).abs() > 0.01)
    {
        return Err(usage_error(
            err,
            notices,
            "TopupFileIO::TopupDatafileReader:: phase-encode vectors must be unity length",
        ));
    }
    let n_vols = vols.len();
    if acq.len() != n_vols {
        let shown = match (datain, mode) {
            (Some(d), _) => d.to_owned(),
            (None, UpstreamDefects::Reproduce) => String::new(),
            (None, UpstreamDefects::Fix) => acq_name.clone(),
        };
        if datain.is_none() {
            notices.push(Notice::new(
                &defects::FSL_120,
                mode,
                match mode {
                    UpstreamDefects::Fix => {
                        "the message names the --acqp file (FSL's names the empty --datain)"
                    }
                    UpstreamDefects::Reproduce => {
                        "the message names the empty --datain, as FSL's does"
                    }
                },
            ));
        }
        return Err(usage_error(
            err,
            notices,
            &clp(&format!("Mismatch between {imain} and {shown}")),
        ));
    }
    // read-out times: the last row below 1e-6 excused
    let last_zero = acq.iter().rposition(|r| r[3] < 1e-6);
    if acq
        .iter()
        .enumerate()
        .any(|(i, r)| !(0.01..=0.2).contains(&r[3]) && Some(i) != last_zero)
    {
        return Err(usage_error(
            err,
            notices,
            &clp("Unrealistic read-out time, should be in range 0.01-0.2 s"),
        ));
    }
    // the levels
    let warpres = given.get("warpres").map_or(vec![10.0], float_list);
    let subsamp = given.get("subsamp").map_or(vec![1], int_list);
    let fwhm = given.get("fwhm").map_or(vec![8.0], float_list);
    let miter = given.get("miter").map_or(vec![5], int_list);
    let lambda = given.get("lambda").map_or(vec![0.0], float_list);
    let estmov = given.get("estmov").map_or(vec![1], int_list);
    let minmet = given.get("minmet").map_or(vec![0], int_list);
    let lens = [
        ("warpres", warpres.len()),
        ("subsamp", subsamp.len()),
        ("fwhm", fwhm.len()),
        ("miter", miter.len()),
        ("lambda", lambda.len()),
        ("estmov", estmov.len()),
        ("minmet", minmet.len()),
    ];
    let n_levels = lens.iter().map(|l| l.1).max().unwrap_or(1);
    if let Some(bad) = lens.iter().find(|l| l.1 != 1 && l.1 != n_levels) {
        // FSL names the first list that is not of the levels' length, warpres first
        let first = lens.iter().find(|l| l.1 != n_levels).unwrap_or(bad);
        let _ = first;
        return Err(usage_error(
            err,
            notices,
            &clp(&format!(
                "Mismatch between --{} and other parameters ",
                lens.iter()
                    .find(|l| l.1 != n_levels)
                    .map_or("warpres", |l| l.0)
            )),
        ));
    }
    let at = |v: &[f64], l: usize| if v.len() == 1 { v[0] } else { v[l] };
    let ati = |v: &[i64], l: usize| if v.len() == 1 { v[0] } else { v[l] };
    let dims = input.vol.shape;
    for l in 0..n_levels {
        let s = ati(&subsamp, l);
        if s < 1 || dims.iter().any(|&n| n % s as usize != 0) {
            return Err(usage_error(
                err,
                notices,
                &clp("Subsampling levels incompatible with image data."),
            ));
        }
    }
    if (0..n_levels).any(|l| !(0..=1).contains(&ati(&minmet, l))) {
        return Err(usage_error(
            err,
            notices,
            &clp("Unrecognized minimization method code."),
        ));
    }
    for (name, allowed) in [
        ("numprec", &["double", "float"][..]),
        ("regmod", &["membrane_energy", "bending_energy"][..]),
        ("interp", &["linear", "spline"][..]),
    ] {
        if let Some(v) = given.get(name)
            && !allowed.contains(&v)
        {
            return Err(usage_error(
                err,
                notices,
                &clp(&format!("Unrecognized value {v} as argument to --{name}.")),
            ));
        }
    }
    let splineorder = given.get("splineorder").map_or(3, atoi);
    if splineorder != 2 && splineorder != 3 {
        return Err(usage_error(
            err,
            notices,
            &clp("Only quadratic or cubic splines allowed."),
        ));
    }
    // what this original does not do yet
    let not_yet = |what: &str| format!("{what} is not implemented yet in the clean-room original");
    if (0..n_levels).any(|l| ati(&estmov, l) != 0) {
        return Err(refuse(
            err,
            notices,
            &not_yet("--estmov=1 (the movements' estimation)"),
        ));
    }
    if splineorder == 2 {
        return Err(refuse(err, notices, &not_yet("--splineorder=2")));
    }
    for (name, v) in [
        ("numprec", "float"),
        ("interp", "linear"),
        ("regmod", "membrane_energy"),
    ] {
        if given.get(name) == Some(v) {
            return Err(refuse(err, notices, &not_yet(&format!("--{name}={v}"))));
        }
    }
    for l in 0..n_levels {
        if ati(&miter, l) < 0
            || at(&warpres, l) <= 0.0
            || at(&fwhm, l) < 0.0
            || at(&lambda, l) < 0.0
        {
            return Err(refuse(
                err,
                notices,
                &not_yet("a negative --miter, --warpres, --fwhm or --lambda"),
            ));
        }
    }
    let smax = (0..n_levels).map(|l| ati(&subsamp, l)).max().unwrap_or(1);
    if given.get("regrid").map_or(1, atoi) != 0
        && (0..n_levels).any(|l| smax % ati(&subsamp, l) != 0)
    {
        return Err(refuse(
            err,
            notices,
            &not_yet("subsamplings whose largest the others do not divide (FSL-118)"),
        ));
    }
    let levels: Vec<LevelSpec> = (0..n_levels)
        .map(|l| LevelSpec {
            warpres: at(&warpres, l),
            subsamp: ati(&subsamp, l) as usize,
            fwhm: at(&fwhm, l),
            miter: ati(&miter, l).max(0) as usize,
            lambda: at(&lambda, l),
            estmov: ati(&estmov, l) != 0,
            minmet: if ati(&minmet, l) == 1 {
                Minimiser::ScaledConjugateGradients
            } else {
                Minimiser::LevenbergMarquardt
            },
        })
        .collect();
    let params = Params {
        levels,
        ssqlambda: given.get("ssqlambda").map_or(1, atoi) != 0,
        regmod: if given.get("regmod") == Some("membrane_energy") {
            RegMod::Membrane
        } else {
            RegMod::Bending
        },
        scale: given.get("scale").map_or(0, atoi) != 0,
        regrid: given.get("regrid").map_or(1, atoi) != 0,
    };
    let scans: Vec<Scan> = acq
        .iter()
        .map(|r| Scan {
            pe: [r[0], r[1], r[2]],
            t: r[3],
        })
        .collect();
    if scans.iter().any(|s| s.pe[2] != 0.0) {
        print_notices(err, notices);
        let _ = writeln!(
            err,
            "Error occurred when preparing to run topup\nException thrown with message: Topup::TopupScan::TopupScan: third element of pevec must be zero"
        );
        return Err(1);
    }
    let stem = default_stem(&imain);
    let out = given.get("out").map_or_else(|| stem.clone(), str::to_owned);
    let logout = given
        .get("logout")
        .map_or_else(|| format!("{stem}.topup_log"), str::to_owned);
    let n_threads = match given.get("nthr") {
        Some(v) => {
            let hw = std::thread::available_parallelism().map_or(1, |n| n.get());
            let n = atoi(v);
            if n == 1 {
                1
            } else if n == 0 {
                settings
                    .get("FSL_NUM_THREADS")
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .filter(|&x| x > 0)
                    .unwrap_or(1)
            } else if n < 0 {
                hw
            } else {
                (n as usize).min(hw)
            }
        }
        None => settings.threads(),
    };
    Ok(Settled {
        imain,
        out,
        logout,
        params,
        scans,
        volumes: vols.into_iter().map(|v| v.data).collect(),
        input,
        verbose: given.has("verbose"),
        n_threads,
    })
}

/// `--imain`'s path and name before the first `.` of its name.
fn default_stem(imain: &str) -> String {
    let cut = imain.rfind('/').map_or(0, |i| i + 1);
    match imain[cut..].find('.') {
        Some(i) => imain[..cut + i].to_owned(),
        None => imain.to_owned(),
    }
}

/// The log of the settings (§2.7); skipped silently when it cannot be written.
fn write_log(given: &Given, settings: &Settings, logout: &str) {
    let mut text = String::new();
    let shown = |name: &str| -> Option<String> {
        let v = given.get(name);
        Some(match name {
            "imain" | "datain" | "acqp" | "out" => v.unwrap_or_default().to_owned(),
            "fout" | "iout" | "featout" => v?.to_owned(),
            "warpres" => v.map_or("10".into(), |s| join_f(&float_list(s))),
            "fwhm" => v.map_or("8".into(), |s| join_f(&float_list(s))),
            "lambda" => v.map_or("0".into(), |s| join_f(&float_list(s))),
            "subsamp" => v.map_or("1".into(), |s| join_i(&int_list(s))),
            "miter" => v.map_or("5".into(), |s| join_i(&int_list(s))),
            "estmov" => v.map_or("1".into(), |s| join_i(&int_list(s))),
            "minmet" => v.map_or("0".into(), |s| join_i(&int_list(s))),
            "ssqlambda" | "regrid" => v.map_or(1, atoi).to_string(),
            "scale" => v.map_or(0, atoi).to_string(),
            "nthr" => v.map_or(1, atoi).to_string(),
            "splineorder" => v.map_or(3, atoi).to_string(),
            "regmod" => v.unwrap_or("bending_energy").to_owned(),
            "numprec" => v.unwrap_or("double").to_owned(),
            "interp" => v.unwrap_or("spline").to_owned(),
            _ => return None,
        })
    };
    let order = [
        "imain",
        "datain",
        "acqp",
        "out",
        "fout",
        "iout",
        "featout",
        "warpres",
        "subsamp",
        "fwhm",
        "miter",
        "lambda",
        "ssqlambda",
        "estmov",
        "minmet",
        "regmod",
        "splineorder",
        "numprec",
        "interp",
        "scale",
        "regrid",
        "nthr",
    ];
    for name in order {
        let Some(value) = shown(name) else { continue };
        let help = OPTIONS
            .iter()
            .find(|o| o.0 == name)
            .and_then(|o| o.2)
            .unwrap_or("");
        text.push_str(&format!("# {help}\n--{name}={value}\n"));
    }
    let _ = std::fs::write(settings.path(logout), text);
}

fn join_f(v: &[f64]) -> String {
    v.iter()
        .map(|&x| g(f64::from(x as f32)))
        .collect::<Vec<_>>()
        .join(",")
}

fn join_i(v: &[i64]) -> String {
    v.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
}

/// Numbers as the movement and matrix files print them (`%.10g`) followed by two blanks.
fn g10(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let sci = format!("{:.9e}", v);
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let x: i32 = exp.parse().unwrap_or(0);
    let strip = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s.to_owned()
        }
    };
    if !(-4..10).contains(&x) {
        let sign = if x < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", strip(mantissa), x.abs())
    } else {
        let decimals = (9 - x).max(0) as usize;
        strip(&format!("{v:.decimals$}"))
    }
}

fn write_outputs(
    given: &Given,
    settings: &Settings,
    mode: UpstreamDefects,
    s: &Settled,
    est: &topup::Estimate,
    notices: &mut Vec<Notice>,
    err: &mut dyn Write,
) -> u8 {
    let input = &s.input;
    let dims = input.vol.shape;
    let nvox = dims[0] * dims[1] * dims[2];
    let n_vols = s.scans.len();
    let (level, ev) = topup::full_resolution(est, &s.scans, s.params.regmod);
    let gscale = est.prepared.g / 100.0;
    let n_threads = s.n_threads;
    let extensions = settings
        .get("FSL_LOAD_NIFTI_EXTENSIONS")
        .is_some_and(|v| atof(v) != 0.0);
    // the output type, checked before the first image
    let ot = match settings.get("FSLOUTPUTTYPE") {
        None => {
            print_notices(err, notices);
            let _ = err.write_all(FSLOUTPUTTYPE_UNSET.as_bytes());
            return 1;
        }
        Some(t) if !image::OUTPUT_TYPES.contains(&t) => {
            print_notices(err, notices);
            let _ = writeln!(err, "{}", image::unknown_output_type_message(t));
            return 1;
        }
        Some(t) => t.to_owned(),
    };
    let write_failed = |err: &mut dyn Write, notices: &[Notice], name: &str| -> u8 {
        print_notices(err, notices);
        let _ = writeln!(
            err,
            "Error occured while writing output from topup\nException thrown with message: Error: cant open file {name}"
        );
        1
    };
    // one image
    let put = |stem: &str,
               header: &NiftiHeader,
               flipped: bool,
               shape: [usize; 3],
               volumes: usize,
               cal: Option<(f64, f64)>,
               rewrite: bool,
               data: Vec<f32>,
               notices: &mut Vec<Notice>|
     -> Result<(), String> {
        let stem = settings.name(files::strip_image_extension(stem));
        let Some((name, version)) = image::output_file(&stem, &ot) else {
            return Err(stem);
        };
        let o = Output {
            template: header,
            flipped,
            shape,
            volumes,
            cal,
            extensions,
            defects: mode,
            n_threads,
            descrip: DESCRIP,
            rewrite_quaternion: rewrite,
        };
        let shown = name
            .strip_prefix(&format!(
                "{}/",
                settings
                    .dir
                    .as_ref()
                    .map_or(String::new(), |d| d.to_string_lossy().into_owned())
            ))
            .unwrap_or(&name)
            .to_owned();
        match image::write(&name, version, &o, Values::F32(data)) {
            Ok(ns) => {
                for x in ns {
                    defects::push_once(notices, x);
                }
                Ok(())
            }
            Err(_) => Err(shown),
        }
    };
    // the coefficients (§6)
    let f = &est.field;
    let cdims = f.coef_dims();
    let mut ch = input.header.clone();
    ch.intent_code = 2016;
    ch.intent_p1 = s.input.vol.voxel[0];
    ch.intent_p2 = s.input.vol.voxel[1];
    ch.intent_p3 = s.input.vol.voxel[2];
    ch.pixdim = [
        1.0,
        f.axes[0].k as f64,
        f.axes[1].k as f64,
        f.axes[2].k as f64,
        1.0,
        1.0,
        1.0,
        1.0,
    ];
    ch.qform_code = 1;
    ch.sform_code = 0;
    ch.quatern_b = 0.0;
    ch.quatern_c = 0.0;
    ch.quatern_d = 0.0;
    ch.qoffset_x = dims[0] as f64;
    ch.qoffset_y = dims[1] as f64;
    ch.qoffset_z = dims[2] as f64;
    ch.srow_x = [1.0, 0.0, 0.0, 0.0];
    ch.srow_y = [0.0, 1.0, 0.0, 0.0];
    ch.srow_z = [0.0, 0.0, 1.0, 0.0];
    ch.cal_min = 0.0;
    ch.cal_max = 0.0;
    let coef: Vec<f32> = f.coef.iter().map(|&v| v as f32).collect();
    if let Err(name) = put(
        &format!("{}_fieldcoef", s.out),
        &ch,
        false,
        cdims,
        1,
        None,
        false,
        coef,
        notices,
    ) {
        return write_failed(err, notices, &name);
    }
    // the movements: none estimated
    let movpar: String = (0..n_vols)
        .map(|_| {
            (0..6)
                .map(|_| format!("{}  ", g10(0.0)))
                .collect::<String>()
                + "\n"
        })
        .collect();
    let movpar_name = format!("{}_movpar.txt", s.out);
    if std::fs::write(settings.path(&movpar_name), movpar).is_err() {
        return write_failed(err, notices, &movpar_name);
    }
    let field32: Vec<f32> = ev.h.iter().map(|&v| v as f32).collect();
    let mask = level.mask();
    // the field (FSL-119: the intent parameters)
    if let Some(fout) = given.get("fout") {
        let mut fh = input.header.clone();
        fh.intent_code = 2018;
        let (p1, p2, p3) = (fh.intent_p1, fh.intent_p2, fh.intent_p3);
        if (p1, p2, p3) != (0.0, p1, p2) {
            notices.push(Notice::new(
                &defects::FSL_119,
                mode,
                match mode {
                    UpstreamDefects::Fix => "the field map keeps the input's intent parameters (FSL's are shifted by one place)",
                    UpstreamDefects::Reproduce => "the field map's intent parameters are the input's shifted by one place, as FSL's",
                },
            ));
            if mode == UpstreamDefects::Reproduce {
                fh.intent_p1 = 0.0;
                fh.intent_p2 = p1;
                fh.intent_p3 = p2;
            }
        }
        let _ = p3;
        if let Err(name) = put(
            fout,
            &fh,
            input.flipped,
            dims,
            1,
            Some((0.0, 0.0)),
            true,
            field32.clone(),
            notices,
        ) {
            return write_failed(err, notices, &name);
        }
    }
    // the corrected volumes and their mean
    let corrected: Vec<f32> = (0..n_vols)
        .flat_map(|i| {
            ev.u[i]
                .iter()
                .zip(mask)
                .map(|(&u, &m)| if m { (u * gscale) as f32 } else { 0.0 })
                .collect::<Vec<f32>>()
        })
        .collect();
    let mean: Vec<f32> = (0..nvox)
        .map(|v| {
            if mask[v] {
                ((0..n_vols).map(|i| ev.u[i][v]).sum::<f64>() / n_vols as f64 * gscale) as f32
            } else {
                (ev.u[0][v] * gscale) as f32
            }
        })
        .collect();
    let mut ih = input.header.clone();
    ih.intent_code = 0;
    if let Some(iout) = given.get("iout") {
        if let Err(name) = put(
            iout,
            &ih,
            input.flipped,
            dims,
            n_vols,
            None,
            true,
            corrected,
            notices,
        ) {
            return write_failed(err, notices, &name);
        }
        if let Err(name) = put(
            &format!("{iout}_mean"),
            &ih,
            input.flipped,
            dims,
            1,
            None,
            true,
            mean.clone(),
            notices,
        ) {
            return write_failed(err, notices, &name);
        }
    }
    if let Some(feat) = given.get("featout") {
        let rads: Vec<f32> =
            ev.h.iter()
                .map(|&v| (v * 2.0 * std::f64::consts::PI) as f32)
                .collect();
        let mut fh = input.header.clone();
        fh.intent_code = 2018;
        if let Err(name) = put(
            &format!("{feat}_fieldmap_rads"),
            &fh,
            input.flipped,
            dims,
            1,
            Some((0.0, 0.0)),
            true,
            rads,
            notices,
        ) {
            return write_failed(err, notices, &name);
        }
        if let Err(name) = put(
            &format!("{feat}_fieldmap_mag"),
            &ih,
            input.flipped,
            dims,
            1,
            None,
            true,
            mean,
            notices,
        ) {
            return write_failed(err, notices, &name);
        }
    }
    // images without transforms: the displacement fields and the Jacobians
    let mut bare = input.header.clone();
    bare.qform_code = 0;
    bare.sform_code = 0;
    bare.quatern_b = 0.0;
    bare.quatern_c = 0.0;
    bare.quatern_d = 0.0;
    bare.qoffset_x = 0.0;
    bare.qoffset_y = 0.0;
    bare.qoffset_z = 0.0;
    bare.srow_x = [1.0, 0.0, 0.0, 0.0];
    bare.srow_y = [0.0, 1.0, 0.0, 0.0];
    bare.srow_z = [0.0, 0.0, 1.0, 0.0];
    bare.pixdim = [
        1.0,
        input.vol.voxel[0],
        input.vol.voxel[1],
        input.vol.voxel[2],
        1.0,
        1.0,
        1.0,
        1.0,
    ];
    bare.intent_code = 0;
    bare.intent_p1 = 0.0;
    bare.intent_p2 = 0.0;
    bare.intent_p3 = 0.0;
    if let Some(df) = given.get("dfout") {
        for (i, sc) in s.scans.iter().enumerate() {
            let mut data = Vec::with_capacity(3 * nvox);
            for a in 0..3 {
                let factor = sc.t * sc.pe[a] * input.vol.voxel[a];
                data.extend(ev.h.iter().map(|&h| (h * factor) as f32));
            }
            if let Err(name) = put(
                &format!("{df}_{:02}", i + 1),
                &bare,
                false,
                dims,
                3,
                None,
                false,
                data,
                notices,
            ) {
                return write_failed(err, notices, &name);
            }
        }
    }
    if let Some(rbm) = given.get("rbmout") {
        for i in 0..n_vols {
            let name = format!("{rbm}_{:02}.mat", i + 1);
            let text: String = (0..4)
                .map(|r| {
                    (0..4)
                        .map(|c| format!("{}  ", g10(if r == c { 1.0 } else { 0.0 })))
                        .collect::<String>()
                        + "\n"
                })
                .collect();
            if std::fs::write(settings.path(&name), text).is_err() {
                let _ = writeln!(err, "Could not open file {name} for writing");
            }
        }
    }
    if let Some(jac) = given.get("jacout") {
        for i in 0..n_vols {
            let data: Vec<f32> = ev.jac[i].iter().map(|&v| v as f32).collect();
            if let Err(name) = put(
                &format!("{jac}_{:02}", i + 1),
                &bare,
                false,
                dims,
                1,
                None,
                false,
                data,
                notices,
            ) {
                return write_failed(err, notices, &name);
            }
        }
    }
    let _ = &s.imain;
    print_notices(err, notices);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_as_fsl_prints_them() {
        assert_eq!(g10(0.0), "0");
        assert_eq!(g10(1.0), "1");
        assert_eq!(g10(0.123456789012), "0.123456789");
        assert_eq!(g10(-1.5e-7), "-1.5e-07");
        assert_eq!(default_stem("dir.x/half.nii.gz"), "dir.x/half");
        assert_eq!(read_int(" 12x"), Some(12));
        assert_eq!(read_int("x"), None);
    }
}
