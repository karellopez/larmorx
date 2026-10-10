// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09 src/3dTshift.c (main, TS_copy_input_to_output) and
// src/thd_filestuff.c (THD_filename_ok).
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released under the GNU GPL version 2 or any later version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! `3dTshift` with its original arguments: the option scan, checks, messages and output of
//! AFNI's `main()`, step by step.
//!
//! Messages go where AFNI sends them: `printf` lines (`++ updating time offset to ...` and
//! most `-verbose` lines) to standard output; warnings, errors and `++ Wrote output` to
//! standard error.
//!
//! Differences from AFNI, shared with `larmorx afni 3dTshift` (the larmorx conventions):
//! - input and output are NIfTI only, and `-prefix` must end in `.nii` or `.nii.gz` (AFNI
//!   writes its own format otherwise, and by default, with the prefix `tshift`);
//! - an existing output file is an error (AFNI prints "dataset NOT written to disk" and exits
//!   with status 0);
//! - `-voxshift`, sub-brick selectors and AFNI's own formats are not supported;
//! - AFNI's history extension is not written, and the version banner is not printed.

use std::io::Write;
use std::path::Path;

use crate::cnum::{fmt_g, strtod};
use crate::dataset::{self, Dataset, TimeAxis, TimeUnits};
use crate::shift::Method;
use crate::tpattern::{TpatternError, parse_tpattern};
use crate::tshift::{Rlt, TshiftParams, fft_length, slice_shift, tshift};

use super::threads;

const USAGE: &str = "\
Usage: 3dTshift [options] dataset   (larmorx-gpl: replica of AFNI 25.2.09's 3dTshift)

Shifts voxel time series from the input dataset so that the separate slices are aligned
to the same temporal origin: detrend -> interpolate -> retrend. The results are those of
AFNI's 3dTshift, bit for bit. The dataset and the output are NIfTI files.

Options (before the dataset):
  -verbose        print lots of messages while the program runs
  -TR ddd         use ddd as the TR (suffix 's' for seconds, 'ms' for milliseconds)
  -tzero zzz      align each slice to time offset zzz (default: the mean slice offset)
  -slice nnn      align each slice to the time offset of slice nnn
  -prefix ppp     output file name, ending in .nii or .nii.gz; it must not exist
  -ignore ii      ignore the first ii points (kept unchanged in the output)
  -rlt            do not add the mean and linear trend back after shifting
  -rlt+           add only the mean back
  -no_detrend     do not remove or restore the linear trend (needs a non-Fourier method
                  given before it)
  -Fourier -linear -cubic -quintic -heptic -wsinc5 -wsinc9
                  interpolation method (default -Fourier)
  -tpattern ttt   slice timing: alt+z altplus alt+z2 alt-z altminus alt-z2 seq+z seqplus
                  seq-z seqminus zero simult, or @file / '@1D: ...' with one offset per
                  slice (default: the slice timing in the dataset header)
";

/// Runs `3dTshift` with `args`; returns the exit status.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if args.is_empty() || args[0] == "-help" {
        let _ = out.write_all(USAGE.as_bytes());
        return 0;
    }
    let mut io = Io { out, err };
    match run(args, &mut io) {
        Ok(()) => 0,
        Err(Fail::Fatal(message)) => {
            let _ = writeln!(io.err, "** FATAL ERROR: {message}");
            1
        }
        Err(Fail::Error(message)) => {
            let _ = writeln!(io.err, "** ERROR: {message}");
            1
        }
    }
}

/// How 3dTshift stops: `ERROR_exit` ("** FATAL ERROR: ...") or `ERROR_message` followed by
/// `exit(1)` ("** ERROR: ...").
enum Fail {
    Fatal(String),
    Error(String),
}

impl From<String> for Fail {
    fn from(m: String) -> Fail {
        Fail::Fatal(m)
    }
}

impl From<&str> for Fail {
    fn from(m: &str) -> Fail {
        Fail::Fatal(m.to_owned())
    }
}

struct Io<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
}

impl Io<'_> {
    /// `printf(...)`.
    fn print(&mut self, s: impl std::fmt::Display) {
        let _ = writeln!(self.out, "{s}");
    }

    /// `WARNING_message(...)`.
    fn warn(&mut self, s: impl std::fmt::Display) {
        let _ = writeln!(self.err, "*+ WARNING: {s}");
    }

    /// `INFO_message(...)`.
    fn info(&mut self, s: impl std::fmt::Display) {
        let _ = writeln!(self.err, "++ {s}");
    }

    /// `ININFO_message(...)`.
    fn ininfo(&mut self, s: impl std::fmt::Display) {
        let _ = writeln!(self.err, " + {s}");
    }
}

/// The command line after the option scan (AFNI's `TS_*` globals).
struct Options {
    verbose: u32,
    bad: bool,
    ignore: i64,
    method: Method,
    tr: f32,
    tunits: TimeUnits,
    tzero: f32,
    slice: i64,
    prefix: String,
    rlt: Rlt,
    detrend: bool,
    tpattern: Option<String>,
    dataset: String,
}

/// `THD_filename_ok`: no control characters, blanks, shell metacharacters or non-ASCII bytes.
fn filename_ok(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.len() > 6 && name.starts_with("3dcalc") {
        return true;
    }
    !name.bytes().any(|c| {
        c.is_ascii_control() || c == b' ' || b";*?&|\"><'[](){}!".contains(&c) || c & 128 != 0
    })
}

/// A method option: `strncmp(arg, name, 4)` against two spellings, or for weighted sinc
/// `strncasecmp(arg, name, 7)`.
fn method_option(arg: &str) -> Option<Method> {
    let b = arg.as_bytes();
    let n4 = |names: [&str; 2]| b.len() >= 4 && names.iter().any(|n| b[..4] == n.as_bytes()[..4]);
    let n7 = |name: &str| b.len() >= 7 && b[..7].eq_ignore_ascii_case(name.as_bytes());
    if n4(["-Fourier", "-fourier"]) {
        Some(Method::Fourier)
    } else if n4(["-cubic", "-Cubic"]) {
        Some(Method::Cubic)
    } else if n4(["-quintic", "-Quintic"]) {
        Some(Method::Quintic)
    } else if n4(["-heptic", "-Heptic"]) {
        Some(Method::Heptic)
    } else if n7("-wsinc5") {
        Some(Method::Wsinc5)
    } else if n7("-wsinc9") {
        Some(Method::Wsinc9)
    } else if n4(["-linear", "-Linear"]) {
        Some(Method::Linear)
    } else {
        None
    }
}

/// `(int) strtod(s, NULL)`: truncation toward zero (saturating where C is undefined).
fn strtod_int(s: &str) -> i64 {
    strtod(s).0 as i64
}

fn parse(args: &[String], io: &mut Io<'_>) -> Result<Options, Fail> {
    let mut o = Options {
        verbose: 0,
        bad: false,
        ignore: 0,
        method: Method::Fourier,
        tr: 0.0,
        tunits: TimeUnits::Sec,
        tzero: -1.0,
        slice: -1,
        prefix: "tshift".to_owned(),
        rlt: Rlt::Trend,
        detrend: true,
        tpattern: None,
        dataset: String::new(),
    };
    let mut nopt = 0;
    while nopt < args.len() && args[nopt].starts_with('-') {
        let arg = args[nopt].as_str();
        // The value after an option; AFNI checks `++nopt >= argc` for most options.
        let value = |nopt: &mut usize, name: &str| -> Result<String, Fail> {
            *nopt += 1;
            args.get(*nopt)
                .cloned()
                .ok_or_else(|| Fail::Fatal(format!("{name} needs an argument!")))
        };
        if arg == "-voxshift" {
            return Err("-voxshift is not supported by larmorx-gpl".into());
        }
        if arg == "-BAD" {
            o.bad = true;
            nopt += 1;
            continue;
        }
        if arg.len() >= 5 && arg.as_bytes()[..5] == *b"-verb" {
            o.verbose += 1;
            nopt += 1;
            continue;
        }
        if arg == "-ignore" {
            o.ignore = strtod_int(&value(&mut nopt, "-ignore")?);
            if o.ignore < 0 {
                return Err(format!("-ignore value {} is negative!", o.ignore).into());
            }
            nopt += 1;
            continue;
        }
        if let Some(method) = method_option(arg) {
            o.method = method;
            nopt += 1;
            continue;
        }
        match arg {
            "-TR" => {
                let v = value(&mut nopt, "-TR")?;
                let (tr, used) = strtod(&v);
                o.tr = tr as f32;
                if o.tr <= 0.0 {
                    return Err(format!("illegal value '{v}' after -TR!").into());
                }
                let rest = &v[used..];
                if rest.eq_ignore_ascii_case("ms") || rest.eq_ignore_ascii_case("msec") {
                    o.tunits = TimeUnits::Msec;
                    io.warn("TR expressed in milliseconds is deprecated [not wanted].");
                } else if rest.eq_ignore_ascii_case("s") || rest.eq_ignore_ascii_case("sec") {
                    o.tunits = TimeUnits::Sec;
                } else if rest.eq_ignore_ascii_case("Hz") || rest.eq_ignore_ascii_case("Hertz") {
                    o.tunits = TimeUnits::Hz;
                }
            }
            "-tzero" => {
                let v = value(&mut nopt, "-tzero")?;
                o.tzero = strtod(&v).0 as f32;
                if o.tzero < 0.0 {
                    return Err(format!("illegal value '{v}' after -tzero!").into());
                }
            }
            "-slice" => {
                let v = value(&mut nopt, "-slice")?;
                o.slice = strtod_int(&v);
                if o.slice < 0 {
                    return Err(format!("illegal value '{v}' after -slice!").into());
                }
            }
            "-prefix" => {
                let v = value(&mut nopt, "-prefix")?;
                if !filename_ok(&v) {
                    return Err(format!("illegal value '{v}' after -prefix").into());
                }
                o.prefix = v;
            }
            "-rlt" => {
                if !o.detrend {
                    return Err("cannot use both -rlt and -no_detrend".into());
                }
                o.rlt = Rlt::Nothing;
            }
            "-rlt+" => {
                if !o.detrend {
                    return Err("cannot use both -rlt+ and -no_detrend".into());
                }
                o.rlt = Rlt::Mean;
            }
            "-no_detrend" => {
                if o.rlt != Rlt::Trend {
                    return Err("cannot use both -rlt and -no_detrend".into());
                }
                if o.method == Method::Fourier {
                    return Err("found -no_detrend, changing default to -heptic".into());
                }
                o.detrend = false;
                o.rlt = Rlt::Mean;
            }
            "-tpattern" => o.tpattern = Some(value(&mut nopt, "-tpattern")?),
            _ => return Err(format!("Unknown option: {arg}").into()),
        }
        // `-rlt`, `-rlt+` and `-no_detrend` take no value; the others advanced `nopt` already.
        nopt += 1;
    }
    if !o.detrend && o.method == Method::Fourier {
        io.warn("-no_detrend with Fourier interpolation is dangerous");
    }
    o.dataset = args
        .get(nopt)
        .cloned()
        .ok_or(Fail::Fatal("Need a dataset input?!".into()))?;
    Ok(o)
}

/// The output must be a NIfTI file that does not exist yet (larmorx's conventions).
fn output_path(prefix: &str) -> Result<&Path, Fail> {
    let lower = prefix.to_ascii_lowercase();
    if !(lower.ends_with(".nii") || lower.ends_with(".nii.gz")) {
        return Err(format!(
            "larmorx-gpl writes NIfTI only: the -prefix '{prefix}' must end in .nii or .nii.gz"
        )
        .into());
    }
    Ok(Path::new(prefix))
}

fn check_absent(path: &Path) -> Result<(), Fail> {
    if path.exists() {
        return Err(format!(
            "output dataset name '{}' conflicts with existing file",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn write(path: &Path, ds: &Dataset, n_threads: usize) -> Result<(), Fail> {
    check_absent(path)?;
    dataset::write(path, ds, n_threads)
        .map_err(|e| Fail::Fatal(format!("can't write output dataset: {e}")))
}

/// `TS_copy_input_to_output()`: the dataset as read, written unchanged.
fn copy_input_to_output(
    ds: &Dataset,
    path: &Path,
    o: &Options,
    n_threads: usize,
    io: &mut Io<'_>,
) -> Result<(), Fail> {
    io.warn("==>> output dataset is just a copy of input dataset");
    write(path, ds, n_threads)?;
    if o.verbose > 0 {
        io.info(format!("Wrote output: {}", path.display()));
    }
    Ok(())
}

fn run(args: &[String], io: &mut Io<'_>) -> Result<(), Fail> {
    let mut o = parse(args, io)?;
    let path = output_path(&o.prefix)?;
    let n_threads = threads();

    // Open the dataset; extract values, check for errors.
    if o.verbose > 0 {
        io.print("++ opening input dataset header");
    }
    let mut warnings = Vec::new();
    let read = dataset::read(&o.dataset, n_threads, &mut warnings);
    for w in &warnings {
        if w.starts_with("reading ") {
            io.ininfo(w);
        } else {
            io.warn(w);
        }
    }
    let mut ds = read.map_err(|e| {
        Fail::Fatal(format!(
            "Can't open input dataset '{}': {e}",
            o.dataset.as_str()
        ))
    })?;
    let [_, _, nzz] = ds.nxyz;
    let ntt = ds.nvals;

    if ntt < 2 {
        io.warn("Input dataset has only 1 value per voxel!");
        return copy_input_to_output(&ds, path, &o, n_threads, io);
    }
    if o.slice >= nzz as i64 {
        return Err(format!("-slice value is too large ({} >= {nzz})", o.slice).into());
    }
    if o.ignore > ntt as i64 - 5 {
        return Err(format!("-ignore value {} is too large", o.ignore).into());
    }
    if o.tr <= 0.0 {
        // The TR from the dataset.
        if let Some(t) = &ds.taxis {
            o.tr = t.ttdel;
            o.tunits = t.units;
        }
        if o.tr <= 0.0 {
            o.tr = 1.0;
            o.tunits = TimeUnits::Sec;
        }
        if o.verbose > 0 {
            io.print(format!(
                "++ using dataset TR = {} {}",
                fmt_g(f64::from(o.tr)),
                o.tunits.label()
            ));
        }
    }

    let header_offsets = ds.taxis.as_ref().and_then(|t| t.toff_sl.clone());
    if ds.taxis.is_none() {
        if o.tr == 0.0 || o.tpattern.is_none() {
            io.warn("dataset has no time axis!");
            return copy_input_to_output(&ds, path, &o, n_threads, io);
        }
    } else if o.tpattern.is_none() && header_offsets.is_none() {
        io.warn("dataset is already aligned in time!");
        return copy_input_to_output(&ds, path, &o, n_threads, io);
    }
    let tpat = match &o.tpattern {
        Some(p) => parse_tpattern(nzz, o.tr, p).map_err(|e| match e {
            TpatternError::Unknown(_) => Fail::Error(e.to_string()),
            _ => Fail::Fatal(e.to_string()),
        })?,
        None => {
            let offsets = header_offsets.expect("checked above");
            if offsets.len() != nzz {
                io.warn("dataset temporal pattern is malformed!");
                return copy_input_to_output(&ds, path, &o, n_threads, io);
            }
            offsets
        }
    };
    if o.verbose > 0 {
        let times: String = tpat
            .iter()
            .map(|&t| format!("{} ", fmt_g(f64::from(t))))
            .collect();
        io.print(format!("++ using tpattern = {times}{}", o.tunits.label()));
    }

    // Check the pattern (WAY_BIG = 1e10, stored as float).
    let (mut tomin, mut tomax) = (1.0e10f32, -1.0e10f32);
    for &t in &tpat {
        if t > tomax {
            tomax = t;
        }
        if t < tomin {
            tomin = t;
        }
    }
    if tomin < 0.0 || tomax > o.tr {
        io.warn(format!(
            "some value in tpattern is outside range 0..TR={}",
            fmt_g(f64::from(o.tr))
        ));
        return copy_input_to_output(&ds, path, &o, n_threads, io);
    } else if tomin == tomax {
        io.warn(format!(
            "input has only 1 time offset, {}",
            fmt_g(f64::from(tomin))
        ));
    } else if tomin > tomax {
        io.warn(format!(
            "bad min/max toffset {}/{}, not shifting",
            fmt_g(f64::from(tomin)),
            fmt_g(f64::from(tomax))
        ));
        return copy_input_to_output(&ds, path, &o, n_threads, io);
    }

    // The common time point.
    if o.slice >= 0 && o.slice < nzz as i64 {
        o.tzero = tpat[o.slice as usize];
    } else if o.tzero < 0.0 {
        let mut sum = 0.0f32;
        for &t in &tpat {
            sum += t;
        }
        o.tzero = sum / nzz as f32;
    }
    if o.verbose > 0 {
        io.print(format!(
            "++ common time point set to {}",
            fmt_g(f64::from(o.tzero))
        ));
        io.print("++ copying input dataset bricks");
    }
    check_absent(path)?;

    // Reconfigure the time axis.
    io.print(format!(
        "++ updating time offset to {}",
        fmt_g(f64::from(o.tzero))
    ));
    ds.taxis = Some(TimeAxis {
        ttdel: o.tr,
        ttorg: o.tzero,
        units: o.tunits,
        toff_sl: None,
    });

    let params = TshiftParams {
        tr: o.tr,
        tpat,
        tzero: o.tzero,
        ignore: o.ignore as usize,
        method: o.method,
        rlt: o.rlt,
        detrend: o.detrend,
        bad: o.bad,
        n_threads,
    };
    if o.verbose > 0 {
        if o.method == Method::Fourier {
            io.print(format!(
                "++ Time series length = {ntt}; FFT length set to {}",
                fft_length(ntt)
            ));
        }
        for kk in 0..nzz {
            let s = slice_shift(&params, kk);
            io.print(format!(
                "++ slice {kk}: fractional shift = {}",
                fmt_g(f64::from(s))
            ));
        }
    }
    tshift(&mut ds, &params).map_err(|e| Fail::Fatal(e.to_string()))?;
    write(path, &ds, n_threads)?;
    if o.verbose > 0 {
        let _ = writeln!(io.err, "++ Wrote output: {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Options, String> {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut io = Io {
            out: &mut out,
            err: &mut err,
        };
        parse(&args, &mut io).map_err(|e| match e {
            Fail::Fatal(m) | Fail::Error(m) => m,
        })
    }

    #[test]
    fn method_names() {
        for (arg, method) in [
            ("-Fourier", Method::Fourier),
            ("-fou", Method::Fourier),
            ("-Lin", Method::Linear),
            ("-cub", Method::Cubic),
            ("-quin", Method::Quintic),
            ("-Hept", Method::Heptic),
            ("-WSINC5", Method::Wsinc5),
            ("-wsinc9x", Method::Wsinc9),
        ] {
            assert_eq!(method_option(arg), Some(method), "{arg}");
        }
        for arg in ["-FOURIER", "-CUB", "-li", "-wsinc", "-wsinc7"] {
            assert_eq!(method_option(arg), None, "{arg}");
        }
    }

    #[test]
    fn option_rules() {
        let o = parse_args(&["-verb", "-TR", "2000ms", "-cubic", "-no_detrend", "in.nii"]).unwrap();
        assert_eq!(o.verbose, 1);
        assert!(!o.detrend);
        assert_eq!(o.rlt, Rlt::Mean);
        assert_eq!((o.tr, o.tunits), (2000.0, TimeUnits::Msec));
        assert_eq!(o.method, Method::Cubic);
        assert!(parse_args(&["-no_detrend", "-heptic", "in.nii"]).is_err());
        assert!(parse_args(&["-rlt", "-linear", "-no_detrend", "in.nii"]).is_err());
        assert!(parse_args(&["-linear", "-no_detrend", "-rlt+", "in.nii"]).is_err());
        assert_eq!(parse_args(&["-rlt", "-rlt+", "x"]).unwrap().rlt, Rlt::Mean);
        assert_eq!(parse_args(&["-ignore", "1.7", "x"]).unwrap().ignore, 1);
        assert_eq!(parse_args(&["-tzero", "abc", "x"]).unwrap().tzero, 0.0);
        assert!(parse_args(&["-tzero", "-1", "x"]).is_err());
        assert!(parse_args(&["-slice", "-1", "x"]).is_err());
        assert!(parse_args(&["-ignore", "-1", "x"]).is_err());
        assert!(parse_args(&["-TR", "0", "x"]).is_err());
        assert!(parse_args(&["-TR", "abc", "x"]).is_err());
        assert!(parse_args(&["-prefix", "a b.nii", "x"]).is_err());
        assert!(parse_args(&["-bogus", "x"]).is_err());
        assert!(parse_args(&["-TR"]).is_err());
        assert!(parse_args(&["-linear"]).is_err());
        assert_eq!(
            parse_args(&["x", "-linear"]).unwrap().method,
            Method::Fourier
        );
    }
}
