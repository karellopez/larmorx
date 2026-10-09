// SPDX-License-Identifier: Apache-2.0
//! `3dTshift` with its original arguments (the behaviour of AFNI 25.2.09,
//! `specs/3dTshift.md` §3 and §7).
//!
//! Differences from AFNI, all deliberate: the output must be a NIfTI file (`-prefix` ending in
//! `.nii` or `.nii.gz`); an existing output file is an error (AFNI warns and exits 0 without
//! writing); `-voxshift`, sub-brick selectors and AFNI's own formats are not supported; no
//! history extension is written.

use std::io::Write;
use std::path::Path;

use crate::dataset::{self, AfniImage, OutputTiming};
use crate::oned;
use crate::timing::{HeaderTiming, Pattern, mean_time};
use crate::tshift::{Method, Restore, TshiftParams, slice_shifts, tshift};

use super::{atof, atoi, leading_number, threads};

const USAGE: &str = "\
Usage: 3dTshift [options] dataset   (larmorx, clean-room re-implementation of AFNI 3dTshift)

Shifts voxel time series so that all slices are aligned to the same temporal origin:
detrend -> interpolate -> retrend. The dataset is a 4D NIfTI file.

Options (they must come before the dataset):
  -verbose        print progress messages
  -TR ddd         use ddd as the TR (suffix s or ms; ms switches every time to milliseconds)
  -tzero zzz      align each slice to time offset zzz (default: the mean slice offset)
  -slice nnn      align each slice to the offset of slice nnn (wins over -tzero)
  -prefix ppp     output file name: must end in .nii or .nii.gz, and must not exist
  -ignore ii      leave the first ii time points out of the detrending and shifting
  -rlt            do not add the trend back after shifting
  -rlt+           add back only the intercept of the trend
  -no_detrend     remove (and restore) only the mean; needs a non-Fourier method before it
  -Fourier -linear -cubic -quintic -heptic -wsinc5 -wsinc9
                  interpolation method (default -Fourier)
  -tpattern ttt   slice timing: alt+z altplus alt+z2 alt-z altminus alt-z2 seq+z seqplus
                  seq-z seqminus zero simult, or @file / '@1D: ...' with one offset per slice
                  (default: the slice timing in the NIfTI header)
";

/// The warning for `-no_detrend`, whose AFNI behaviour larmorx reproduces.
const NO_DETREND_PAIRS: &str = "-no_detrend: as AFNI 25.2.09 does, only the first voxel of each pair \
(in index order within a slice) has its mean removed and restored; the second is shifted as raw values";

/// Runs `3dTshift` with `args`; returns the exit code.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if args.is_empty() || matches!(args[0].as_str(), "-help" | "-h") {
        let _ = out.write_all(USAGE.as_bytes());
        return 0;
    }
    let mut log = Log {
        err,
        verbose: false,
    };
    let result = parse(args, &mut log).and_then(|o| {
        log.verbose = o.verbose;
        execute(&o, &mut log)
    });
    match result {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(log.err, "** FATAL ERROR: {message}");
            1
        }
    }
}

struct Log<'a> {
    err: &'a mut dyn Write,
    verbose: bool,
}

impl Log<'_> {
    fn info(&mut self, message: impl std::fmt::Display) {
        if self.verbose {
            let _ = writeln!(self.err, "++ {message}");
        }
    }

    fn warn(&mut self, message: impl std::fmt::Display) {
        let _ = writeln!(self.err, "*+ WARNING: {message}");
    }
}

/// The parsed command line.
#[derive(Debug, Default)]
struct Options {
    verbose: bool,
    /// `-TR`: the value and whether it was given in milliseconds.
    tr: Option<(f64, bool)>,
    tzero: Option<f32>,
    slice: Option<usize>,
    ignore: usize,
    method: Method,
    restore: Restore,
    /// The `-rlt` option given last, if any.
    rlt: Option<&'static str>,
    no_detrend: bool,
    tpattern: Option<String>,
    prefix: Option<String>,
    dataset: String,
}

/// A method option: the first 4 characters are compared (`-cub` works), case-sensitively
/// against the two spellings; `-wsinc5`/`-wsinc9` compare 7 characters, ignoring case.
fn method_option(arg: &str) -> Option<Method> {
    let b = arg.as_bytes();
    let prefix4 =
        |names: [&str; 2]| b.len() >= 4 && names.iter().any(|n| b[..4] == n.as_bytes()[..4]);
    let wsinc = |name: &str| b.len() >= 7 && b[..7].eq_ignore_ascii_case(name.as_bytes());
    if prefix4(["-Fourier", "-fourier"]) {
        Some(Method::Fourier)
    } else if prefix4(["-linear", "-Linear"]) {
        Some(Method::Linear)
    } else if prefix4(["-cubic", "-Cubic"]) {
        Some(Method::Cubic)
    } else if prefix4(["-quintic", "-Quintic"]) {
        Some(Method::Quintic)
    } else if prefix4(["-heptic", "-Heptic"]) {
        Some(Method::Heptic)
    } else if wsinc("-wsinc5") {
        Some(Method::Wsinc5)
    } else if wsinc("-wsinc9") {
        Some(Method::Wsinc9)
    } else {
        None
    }
}

/// `-TR ddd[s|ms]`: a leading number and an optional unit; `ms` or `msec` (any case) means
/// milliseconds, anything else seconds.
fn parse_tr(value: &str) -> Result<(f64, bool), String> {
    let illegal = || format!("illegal value '{value}' after -TR!");
    let (tr, unit) = leading_number(value).ok_or_else(illegal)?;
    if tr.is_nan() || tr <= 0.0 || tr.is_infinite() {
        return Err(illegal());
    }
    let ms = unit.eq_ignore_ascii_case("ms") || unit.eq_ignore_ascii_case("msec");
    Ok((tr, ms))
}

fn parse(args: &[String], log: &mut Log<'_>) -> Result<Options, String> {
    let mut o = Options::default();
    let mut i = 0;
    while i < args.len() && args[i].starts_with('-') {
        let arg = args[i].as_str();
        let value = |i: usize| -> Result<&str, String> {
            args.get(i + 1)
                .map(String::as_str)
                .ok_or_else(|| format!("need an argument after {arg}!"))
        };
        if arg.len() >= 5 && arg.as_bytes()[..5] == *b"-verb" {
            o.verbose = true;
            i += 1;
            continue;
        }
        match arg {
            "-TR" => o.tr = Some(parse_tr(value(i)?)?),
            "-tzero" => {
                let v = value(i)?;
                let tzero = atof(v);
                if tzero < 0.0 {
                    return Err(format!("illegal value '{v}' after -tzero!"));
                }
                o.tzero = Some(tzero as f32);
            }
            "-slice" => {
                let v = value(i)?;
                let k = atoi(v);
                if k < 0 {
                    return Err(format!("illegal value '{v}' after -slice!"));
                }
                o.slice = Some(k as usize);
            }
            "-ignore" => {
                let k = atoi(value(i)?);
                if k < 0 {
                    return Err(format!("-ignore value {k} is negative!"));
                }
                o.ignore = k as usize;
            }
            "-rlt" => {
                o.restore = Restore::None;
                o.rlt = Some("-rlt");
                i += 1;
                continue;
            }
            "-rlt+" => {
                o.restore = Restore::Intercept;
                o.rlt = Some("-rlt+");
                i += 1;
                continue;
            }
            "-no_detrend" => {
                if o.method == Method::Fourier {
                    return Err(
                        "-no_detrend needs a non-Fourier method given before it (e.g. -heptic -no_detrend)"
                            .into(),
                    );
                }
                o.no_detrend = true;
                log.warn(NO_DETREND_PAIRS);
                i += 1;
                continue;
            }
            "-tpattern" => o.tpattern = Some(value(i)?.to_owned()),
            "-prefix" => o.prefix = Some(value(i)?.to_owned()),
            "-voxshift" => return Err("larmorx: -voxshift is not supported".into()),
            _ => {
                let Some(method) = method_option(arg) else {
                    return Err(format!("Unknown option: {arg}"));
                };
                if method == Method::Fourier && o.no_detrend {
                    log.warn("-no_detrend with Fourier interpolation is dangerous");
                }
                o.method = method;
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    o.dataset = args.get(i).ok_or("Need a dataset input?!")?.clone();
    if let (true, Some(rlt)) = (o.no_detrend, o.rlt) {
        return Err(format!("cannot use both {rlt} and -no_detrend"));
    }
    Ok(o)
}

/// The slice times named by `-tpattern` for `nz` slices over `tr`.
fn tpattern_times(spec: &str, nz: usize, tr: f32) -> Result<Vec<f32>, String> {
    if let Some(file) = spec.strip_prefix('@') {
        let values =
            oned::read(file).map_err(|e| format!("Can't read tpattern file {file}: {e}"))?;
        if values.len() < nz {
            return Err(format!(
                "tpattern file {file} has {} values but the dataset has {nz} slices",
                values.len()
            ));
        }
        let times = values[..nz].to_vec();
        if let Some(bad) = times.iter().find(|&&t| t < 0.0 || t > tr) {
            return Err(format!("Illegal value {bad} in tpattern file {file}"));
        }
        return Ok(times);
    }
    let pattern = Pattern::from_name(spec).ok_or_else(|| format!("Unknown tpattern = {spec}"))?;
    Ok(pattern.times(nz, tr))
}

fn output_path(o: &Options) -> Result<&str, String> {
    let prefix = o
        .prefix
        .as_deref()
        .ok_or("larmorx writes NIfTI only: give -prefix name.nii or name.nii.gz")?;
    let lower = prefix.to_ascii_lowercase();
    if !(lower.ends_with(".nii") || lower.ends_with(".nii.gz")) {
        return Err(format!(
            "larmorx writes NIfTI only: the -prefix '{prefix}' must end in .nii or .nii.gz"
        ));
    }
    if Path::new(prefix).exists() {
        return Err(format!(
            "output dataset name '{prefix}' conflicts with existing file"
        ));
    }
    Ok(prefix)
}

/// Writes the input unchanged (AFNI's "output dataset is just a copy of input dataset").
fn write_copy(
    image: &AfniImage,
    path: &str,
    slices: Option<HeaderTiming>,
    n_threads: usize,
    log: &mut Log<'_>,
) -> Result<(), String> {
    log.warn("==>> output dataset is just a copy of input dataset");
    let timing = OutputTiming {
        tr: image.tr,
        toffset: image.toffset,
        slices,
    };
    let header = dataset::output_header(&image.header, &image.bricks, &timing)?;
    dataset::write(path, &header, &image.bricks, n_threads).map_err(|e| e.to_string())?;
    log.info(format!("Wrote output: {path}"));
    Ok(())
}

fn execute(o: &Options, log: &mut Log<'_>) -> Result<(), String> {
    let path = output_path(o)?;
    let n_threads = threads();
    log.info("opening input dataset header");
    let mut image = dataset::read(&o.dataset, n_threads)
        .map_err(|e| format!("Can't open input dataset '{}': {e}", o.dataset))?;
    for w in std::mem::take(&mut image.warnings) {
        log.warn(w);
    }
    let [_, _, nz, nt] = image.bricks.shape;
    if !image.has_time_axis || nt < 2 {
        log.warn("Input dataset has only 1 value per voxel!");
        return write_copy(&image, path, None, n_threads, log);
    }

    // The TR, in milliseconds if -TR says so: then every time is taken in milliseconds.
    let (tr, ms) = match o.tr {
        Some((tr, ms)) => {
            if ms {
                log.warn("TR expressed in milliseconds is deprecated [not wanted].");
            }
            (tr as f32, ms)
        }
        None => {
            log.info(format!("using dataset TR = {} s", image.tr));
            (image.tr, false)
        }
    };
    let unit = if ms { "ms" } else { "s" };

    let times = match &o.tpattern {
        Some(spec) => tpattern_times(spec, nz, tr)?,
        None => match &image.slice_timing {
            Some(h) => {
                if h.times.iter().any(|&t| t < 0.0 || t > tr) {
                    log.warn(format!(
                        "some value in tpattern is outside range 0..TR={tr}"
                    ));
                    let kept = image.slice_timing.clone();
                    return write_copy(&image, path, kept, n_threads, log);
                }
                h.times.clone()
            }
            None => {
                log.warn("dataset is already aligned in time!");
                return write_copy(&image, path, None, n_threads, log);
            }
        },
    };
    log.info(format!(
        "using tpattern = {} {unit}",
        times
            .iter()
            .map(|t| format!("{t}"))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    if times.iter().all(|&t| t == times[0]) {
        log.warn(format!("input has only 1 time offset, {}", times[0]));
    }

    let tzero = match (o.slice, o.tzero) {
        (Some(k), _) => {
            if k >= nz {
                return Err(format!("-slice value is too large ({k} >= {nz})"));
            }
            times[k]
        }
        (None, Some(z)) => z,
        (None, None) => mean_time(&times),
    };
    log.info(format!("common time point set to {tzero}"));
    if o.ignore + 5 > nt {
        return Err(format!("-ignore value {} is too large", o.ignore));
    }

    let params = TshiftParams {
        tr,
        slice_times: times,
        tzero,
        ignore: o.ignore,
        method: o.method,
        restore: o.restore,
        detrend: !o.no_detrend,
        n_threads,
    };
    if log.verbose {
        if o.method == Method::Fourier {
            log.info(format!(
                "Time series length = {nt}; FFT length set to {}",
                crate::tshift::series::fourier_length(nt)
            ));
        }
        for (k, s) in slice_shifts(&params).iter().enumerate() {
            log.info(format!("slice {k}: fractional shift = {s}"));
        }
    }
    tshift(&mut image.bricks, &params).map_err(|e| e.to_string())?;

    let seconds = |t: f32| {
        if ms {
            (f64::from(t) / 1000.0) as f32
        } else {
            t
        }
    };
    let timing = OutputTiming {
        tr: seconds(tr),
        toffset: seconds(tzero),
        slices: None,
    };
    let header = dataset::output_header(&image.header, &image.bricks, &timing)?;
    dataset::write(path, &header, &image.bricks, n_threads).map_err(|e| e.to_string())?;
    log.info(format!("Wrote output: {path}"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Options, String> {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let mut sink = Vec::new();
        let mut log = Log {
            err: &mut sink,
            verbose: false,
        };
        parse(&args, &mut log)
    }

    #[test]
    fn method_names_compare_four_or_seven_characters() {
        for (arg, method) in [
            ("-Fourier", Method::Fourier),
            ("-fou", Method::Fourier),
            ("-Lin", Method::Linear),
            ("-cub", Method::Cubic),
            ("-Cubi", Method::Cubic),
            ("-quin", Method::Quintic),
            ("-Hept", Method::Heptic),
            ("-WSINC5", Method::Wsinc5),
            ("-wsinc5x", Method::Wsinc5),
            ("-Wsinc9", Method::Wsinc9),
        ] {
            assert_eq!(method_option(arg), Some(method), "{arg}");
        }
        for arg in ["-FOURIER", "-CUB", "-li", "-wsinc", "-wsinc7", "-HEPTIC"] {
            assert_eq!(method_option(arg), None, "{arg}");
        }
    }

    #[test]
    fn tr_units() {
        assert_eq!(parse_tr("2").unwrap(), (2.0, false));
        assert_eq!(parse_tr("2s").unwrap(), (2.0, false));
        assert_eq!(parse_tr("2000ms").unwrap(), (2000.0, true));
        assert_eq!(parse_tr("2Msec").unwrap(), (2.0, true));
        assert_eq!(parse_tr("0.5Hz").unwrap(), (0.5, false));
        assert_eq!(parse_tr("2000m").unwrap(), (2000.0, false));
        for bad in ["0", "-1", "abc"] {
            assert!(parse_tr(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn option_rules() {
        let o = parse_args(&["-verb", "-TR", "2s", "-cubic", "-no_detrend", "in.nii"]).unwrap();
        assert!(o.verbose && o.no_detrend);
        assert_eq!(o.method, Method::Cubic);
        assert_eq!(o.dataset, "in.nii");
        // -no_detrend while the method is still Fourier.
        assert!(parse_args(&["-no_detrend", "-heptic", "in.nii"]).is_err());
        // -rlt and -no_detrend, in either order.
        assert!(parse_args(&["-rlt", "-linear", "-no_detrend", "in.nii"]).is_err());
        assert!(parse_args(&["-linear", "-no_detrend", "-rlt+", "in.nii"]).is_err());
        // The last -rlt wins.
        assert_eq!(
            parse_args(&["-rlt", "-rlt+", "in.nii"]).unwrap().restore,
            Restore::Intercept
        );
        assert_eq!(
            parse_args(&["-rlt+", "-rlt", "in.nii"]).unwrap().restore,
            Restore::None
        );
        // atof/atoi values; negative values are errors.
        assert_eq!(
            parse_args(&["-tzero", "abc", "in.nii"]).unwrap().tzero,
            Some(0.0)
        );
        assert_eq!(parse_args(&["-ignore", "1.7", "in.nii"]).unwrap().ignore, 1);
        assert!(parse_args(&["-tzero", "-1", "in.nii"]).is_err());
        assert!(parse_args(&["-slice", "-1", "in.nii"]).is_err());
        assert!(parse_args(&["-ignore", "-1", "in.nii"]).is_err());
        // Options are matched exactly, apart from the methods and -verb.
        for bad in ["-tz", "-tpat", "-pref", "-rltx", "-help", "-voxshift"] {
            assert!(parse_args(&[bad, "x", "in.nii"]).is_err(), "{bad}");
        }
        assert!(parse_args(&["-TR"]).is_err());
        assert!(parse_args(&["-linear"]).is_err());
        // Arguments after the dataset are ignored.
        assert_eq!(
            parse_args(&["in.nii", "-linear"]).unwrap().method,
            Method::Fourier
        );
    }

    #[test]
    fn tpattern_files_are_checked_against_the_tr() {
        assert_eq!(
            tpattern_times("@1D: 0 1 0.5", 3, 2.0).unwrap(),
            [0.0, 1.0, 0.5]
        );
        assert_eq!(
            tpattern_times("@1D: 0 1 0.5 1.5", 3, 2.0).unwrap(),
            [0.0, 1.0, 0.5]
        );
        assert!(tpattern_times("@1D: 0 1", 3, 2.0).is_err());
        assert!(tpattern_times("@1D: 0 2.0001 1", 3, 2.0).is_err());
        assert!(tpattern_times("@1D: 0 -0.1 1", 3, 2.0).is_err());
        assert_eq!(
            tpattern_times("@1D: 0 2 1", 3, 2.0).unwrap(),
            [0.0, 2.0, 1.0]
        );
        assert!(tpattern_times("ALT+Z", 3, 2.0).is_err());
    }
}
