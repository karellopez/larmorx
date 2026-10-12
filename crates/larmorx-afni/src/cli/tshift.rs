// SPDX-License-Identifier: Apache-2.0
//! `3dTshift` with its original arguments (the behaviour of AFNI 25.2.09,
//! `specs/3dTshift.md` §3, §6 and §7), AFNI's global options and environment variables
//! included (§7.1).
//!
//! The process environment is read once into a table of settings ([`AfniSettings`]); AFNI's
//! global options (`-overwrite`, `-Dname=value`, `-ok_1D_text`, `-skip_afnirc`) are taken out
//! wherever they stand and applied to it. Messages are AFNI's, in AFNI's order, with larmorx's
//! own lines added (the upstream-defect notices, `-verbose`'s first two lines, warnings about
//! global options ending in `(larmorx)`).
//!
//! Differences from AFNI, all deliberate: the output must be a NIfTI file (`-prefix` ending in
//! `.nii` or `.nii.gz`); `-voxshift`, sub-brick selectors, lists of datasets and AFNI's own
//! formats are not supported; no history extension is written; AFNI's version banner and
//! compile date are not printed.
//!
//! AFNI's defects are corrected unless the environment variable `LARMORX_UPSTREAM_DEFECTS` is
//! `reproduce` (D13, `docs/api/upstream-defects.md`); a warning names each one that changed
//! (or would have changed) the output.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notice, UpstreamDefects};

use crate::cfmt::g32;
use crate::dataset::{self, AfniImage, InterceptPlan, OutputScaling, OutputTiming};
use crate::input;
use crate::msg::Log;
use crate::oned::{self, OneDOptions};
use crate::output::{self, afni_name, file_name, refused, refused_when_written};
use crate::settings::{AfniSettings, Deconflict, take_global_options};
use crate::timing::{self, Pattern, SliceFields, mean_time};
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
  -prefix ppp     output file name: must end in .nii or .nii.gz; an existing file is an error
                  unless -overwrite or AFNI_DECONFLICT says otherwise
  -ignore ii      leave the first ii time points out of the detrending and shifting
  -rlt            do not add the trend back after shifting
  -rlt+           add back only the intercept of the trend
  -no_detrend     remove (and restore) only the mean; needs a non-Fourier method before it
  -Fourier -linear -cubic -quintic -heptic -wsinc5 -wsinc9
                  interpolation method (default -Fourier)
  -tpattern ttt   slice timing: alt+z altplus alt+z2 alt-z altminus alt-z2 seq+z seqplus
                  seq-z seqminus zero simult, or @file / '@1D: ...' with one offset per slice
                  (default: the slice timing in the NIfTI header)

AFNI's global options may stand anywhere: -overwrite, -Dname=value (sets one of AFNI's
environment variables for this run), -ok_1D_text, -skip_afnirc.
";

/// The first `-verbose` line: which implementation runs (`larmorx afni 3dTshift` runs the
/// bit-exact replica instead when the `larmorx-gpl` program is found; `docs/licensing.md`).
const IMPLEMENTATION: &str = concat!(
    "implementation: original (larmorx ",
    env!("CARGO_PKG_VERSION"),
    ", Apache-2.0), clean-room 3dTshift"
);

/// Runs `3dTshift` with `args`; returns the exit code. The D13 mode comes from
/// `LARMORX_UPSTREAM_DEFECTS`, AFNI's settings from the process environment.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match UpstreamDefects::from_env() {
        Ok(defects) => main_with(args, defects, out, err),
        Err(e) => {
            let _ = writeln!(err, "** FATAL ERROR: {e}");
            EXIT_BAD_MODE
        }
    }
}

/// [`main`] with the D13 mode given; AFNI's settings from the process environment.
pub fn main_with(
    args: &[String],
    defects: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    main_with_settings(args, defects, AfniSettings::from_env(), out, err)
}

/// [`main`] with the D13 mode and AFNI's settings given.
pub fn main_with_settings(
    args: &[String],
    mode: UpstreamDefects,
    mut settings: AfniSettings,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    // The help comes before the global options are taken out (observed).
    if args.is_empty() || matches!(args[0].as_str(), "-help" | "-h") {
        let _ = out.write_all(USAGE.as_bytes());
        return 0;
    }
    let globals = take_global_options(args, &mut settings, mode);
    let mut log = Log::new(out, err, &settings, mode);
    if settings.yes("AFNI_ECHO_COMMANDLINE")
        && settings.get("HOME").is_some_and(|h| Path::new(h).is_dir())
        && !settings.yes("AFNI_DONT_LOGFILE")
    {
        log.raw(&echo_line(&globals.rest, &settings));
    }
    for w in &globals.warnings {
        log.warning(w);
    }
    for n in &globals.notices {
        log.notice(n);
    }
    let result = parse(&globals.rest, &mut log, &settings, mode).and_then(|o| {
        log.verbose = o.verbose;
        execute(&o, mode, &settings, &mut log)
    });
    match result {
        Ok(code) => code,
        Err(Failure::Fatal(message)) => {
            log.fatal(&message);
            1
        }
        Err(Failure::Error(message)) => {
            log.error(&message);
            1
        }
    }
}

/// AFNI's echoed command line (`AFNI_ECHO_COMMANDLINE`): the arguments after the global
/// options were taken out, each one the output-name rule refuses quoted, its blanks, control
/// characters and non-ASCII bytes made blanks.
fn echo_line(args: &[String], settings: &AfniSettings) -> String {
    let arbitrary = settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES");
    let mut line = String::from("+++++===== 3dTshift");
    for a in args {
        line.push(' ');
        if refused(a, arbitrary) {
            let cleaned: String = a
                .bytes()
                .map(|b| {
                    if b <= 32 || b >= 127 {
                        ' '
                    } else {
                        char::from(b)
                    }
                })
                .collect();
            line.push('\'');
            line.push_str(&cleaned);
            line.push('\'');
        } else {
            line.push_str(a);
        }
    }
    line
}

/// How a run fails: a fatal error, or an error (AFNI's `** ERROR:`), both with exit status 1.
enum Failure {
    Fatal(String),
    Error(String),
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
    /// AFNI refuses the `-prefix` at the option scan (AFNI-12; named there).
    prefix_refused: bool,
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

/// The AFNI-12 notice: AFNI refuses the output's name at the option scan (`first`) or when it
/// writes the file.
fn afni_12(name: &str, first: bool, mode: UpstreamDefects) -> Notice {
    let detail = match (first, mode) {
        (true, UpstreamDefects::Fix) => format!(
            "the -prefix '{name}' has blanks, shell metacharacters or non-ASCII characters: \
             accepted; AFNI refuses it"
        ),
        (true, UpstreamDefects::Reproduce) => format!(
            "the -prefix '{name}' has blanks, shell metacharacters or non-ASCII characters: \
             refused, as AFNI does"
        ),
        (false, UpstreamDefects::Fix) => format!(
            "the output name '{name}' has blanks, shell metacharacters or non-ASCII characters, \
             or starts with '-': written; AFNI accepts the -prefix but writes nothing (exit \
             status 0)"
        ),
        (false, UpstreamDefects::Reproduce) => format!(
            "the output name '{name}' has blanks, shell metacharacters or non-ASCII characters, \
             or starts with '-': nothing is written and the exit status is 0, as AFNI does"
        ),
    };
    Notice::new(&defects::AFNI_12, mode, detail)
}

/// The AFNI-11 notice: the output exists.
fn afni_11(mode: UpstreamDefects) -> Notice {
    let detail = match mode {
        UpstreamDefects::Fix => {
            "the output exists: an error (exit status 1); AFNI writes nothing and exits with \
             status 0"
        }
        UpstreamDefects::Reproduce => {
            "the output exists: nothing is written and the exit status is 0, as AFNI does"
        }
    };
    Notice::new(&defects::AFNI_11, mode, detail)
}

/// The AFNI-26 notice: AFNI's NIfTI writer cannot open the output.
fn afni_26(mode: UpstreamDefects) -> Notice {
    let detail = match mode {
        UpstreamDefects::Fix => {
            "the output cannot be opened for writing: an error (exit status 1); AFNI writes \
             nothing and exits with status 0"
        }
        UpstreamDefects::Reproduce => {
            "the output cannot be opened for writing: nothing is written and the exit status is \
             0, as AFNI does"
        }
    };
    Notice::new(&defects::AFNI_26, mode, detail)
}

fn parse(
    args: &[String],
    log: &mut Log<'_>,
    settings: &AfniSettings,
    mode: UpstreamDefects,
) -> Result<Options, Failure> {
    let mut o = Options::default();
    let mut i = 0;
    while i < args.len() && args[i].starts_with('-') {
        let arg = args[i].as_str();
        let value = |i: usize| -> Result<&str, Failure> {
            args.get(i + 1)
                .map(String::as_str)
                .ok_or_else(|| Failure::Fatal(format!("{arg} needs an argument!")))
        };
        if arg.len() >= 5 && arg.as_bytes()[..5] == *b"-verb" {
            o.verbose = true;
            i += 1;
            continue;
        }
        match arg {
            "-TR" => {
                let tr = parse_tr(value(i)?).map_err(Failure::Fatal)?;
                if tr.1 {
                    log.warning("TR expressed in milliseconds is deprecated [not wanted].");
                }
                o.tr = Some(tr);
            }
            "-tzero" => {
                let v = value(i)?;
                let tzero = atof(v);
                if tzero < 0.0 {
                    return Err(Failure::Fatal(format!("illegal value '{v}' after -tzero!")));
                }
                o.tzero = Some(tzero as f32);
            }
            "-slice" => {
                let v = value(i)?;
                let k = atoi(v);
                if k < 0 {
                    return Err(Failure::Fatal(format!("illegal value '{v}' after -slice!")));
                }
                o.slice = Some(k as usize);
            }
            "-ignore" => {
                let k = atoi(value(i)?);
                if k < 0 {
                    return Err(Failure::Fatal(format!("-ignore value {k} is negative!")));
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
                    return Err(Failure::Fatal(match mode {
                        UpstreamDefects::Reproduce => {
                            "found -no_detrend, changing default to -heptic".into()
                        }
                        UpstreamDefects::Fix => "-no_detrend needs a non-Fourier method given \
                                                 before it (e.g. -heptic -no_detrend)"
                            .into(),
                    }));
                }
                o.no_detrend = true;
                i += 1;
                continue;
            }
            "-tpattern" => o.tpattern = Some(value(i)?.to_owned()),
            "-prefix" => {
                let v = value(i)?;
                if refused(v, settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES")) {
                    o.prefix_refused = true;
                    log.notice(&afni_12(v, true, mode));
                    if mode.reproduce() {
                        return Err(Failure::Fatal(format!("illegal value '{v}' after -prefix")));
                    }
                }
                o.prefix = Some(v.to_owned());
            }
            "-voxshift" => {
                return Err(Failure::Fatal("larmorx: -voxshift is not supported".into()));
            }
            _ => {
                let Some(method) = method_option(arg) else {
                    return Err(Failure::Fatal(format!("Unknown option: {arg}")));
                };
                if method == Method::Fourier && o.no_detrend {
                    log.warning("-no_detrend with Fourier interpolation is dangerous");
                }
                o.method = method;
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    o.dataset = args
        .get(i)
        .ok_or_else(|| Failure::Fatal("Need a dataset input?!".into()))?
        .clone();
    if let (true, Some(rlt)) = (o.no_detrend, o.rlt) {
        return Err(Failure::Fatal(format!(
            "cannot use both {rlt} and -no_detrend"
        )));
    }
    Ok(o)
}

/// The slice times named by `-tpattern` for `nz` slices over `tr`.
fn tpattern_times(
    spec: &str,
    nz: usize,
    tr: f32,
    settings: &AfniSettings,
    log: &mut Log<'_>,
) -> Result<Vec<f32>, Failure> {
    let Some(file) = spec.strip_prefix('@') else {
        let pattern = Pattern::from_name(spec)
            .ok_or_else(|| Failure::Error(format!("Unknown tpattern = {spec}")))?;
        return Ok(pattern.times(nz, tr));
    };
    let options = OneDOptions {
        zero_text: settings.yes("AFNI_1D_ZERO_TEXT"),
        fgets: settings.yes("AFNI_USE_FGETS"),
    };
    let values = oned::read(file, options).map_err(|messages| {
        for m in &messages {
            log.message(m);
        }
        Failure::Fatal(format!("Can't read tpattern file {file}"))
    })?;
    if values.len() < nz {
        return Err(Failure::Fatal(format!(
            "tpattern file {file} has {} values but have {nz} slices",
            values.len()
        )));
    }
    let times = values[..nz].to_vec();
    if let Some(&bad) = times.iter().find(|&&t| t < 0.0 || t > tr) {
        return Err(Failure::Fatal(format!(
            "Illegal value {} in tpattern file {file}",
            g32(bad)
        )));
    }
    Ok(times)
}

fn list(times: &[f32]) -> String {
    times.iter().map(|&t| g32(t)).collect::<Vec<_>>().join(" ")
}

/// What the run writes, besides the dataset.
struct Target<'a> {
    prefix: &'a str,
    deconflict: Deconflict,
    prefix_refused: bool,
    n_threads: usize,
}

fn execute(
    o: &Options,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    log: &mut Log<'_>,
) -> Result<u8, Failure> {
    let prefix = o.prefix.as_deref().ok_or_else(|| {
        Failure::Fatal("larmorx writes NIfTI only: give -prefix name.nii or name.nii.gz".into())
    })?;
    let lower = prefix.to_ascii_lowercase();
    if !(lower.ends_with(".nii") || lower.ends_with(".nii.gz")) {
        return Err(Failure::Fatal(format!(
            "larmorx writes NIfTI only: the -prefix '{prefix}' must end in .nii or .nii.gz"
        )));
    }
    let deconflict = Deconflict::from_settings(settings);
    // AFNI-11 corrected: an existing output is an error before the work.
    if mode.fix() && output::exists(prefix) && deconflict == Deconflict::Unset {
        log.notice(&afni_11(mode));
        return Err(Failure::Fatal(format!(
            "output dataset name '{}' conflicts with existing file",
            file_name(prefix)
        )));
    }
    let target = Target {
        prefix,
        deconflict,
        prefix_refused: o.prefix_refused,
        n_threads: threads(),
    };
    log.larmorx_info(IMPLEMENTATION);
    log.larmorx_info(&format!(
        "upstream defects: {mode} ({})",
        larmorx_core::defects::ENV
    ));
    log.progress("++ opening input dataset header");
    let (mut messages, mut notices) = (Vec::new(), Vec::new());
    let found = input::resolve(&o.dataset, settings, mode, &mut messages, &mut notices);
    for m in &messages {
        log.message(m);
    }
    for n in &notices {
        log.notice(n);
    }
    let found = found.map_err(|e| Failure::Fatal(e.0))?;
    let mut image = dataset::read(&found.path, &found.shown, target.n_threads, mode, settings)
        .map_err(|e| Failure::Fatal(format!("Can't open input dataset '{}': {e}", o.dataset)))?;
    for m in std::mem::take(&mut image.messages) {
        log.message(&m);
    }
    for n in std::mem::take(&mut image.notices) {
        log.notice(&n);
    }
    let [_, _, nz, nt] = image.bricks.shape;
    if !image.has_time_axis || nt < 2 {
        log.warning("Input dataset has only 1 value per voxel!");
        return write_copy(&mut image, None, &target, mode, settings, log);
    }
    if let Some(k) = o.slice
        && k >= nz
    {
        return Err(Failure::Fatal(format!(
            "-slice value is too large ({k} >= {nz})"
        )));
    }
    if o.ignore + 5 > nt {
        return Err(Failure::Fatal(format!(
            "-ignore value {} is too large",
            o.ignore
        )));
    }

    // The TR, in milliseconds if -TR says so: then every time is taken in milliseconds.
    let (tr, ms) = match o.tr {
        Some((tr, ms)) => (tr as f32, ms),
        None => {
            log.progress(&format!("++ using dataset TR = {} s", g32(image.tr)));
            (image.tr, false)
        }
    };
    let unit = if ms { "ms" } else { "s" };

    let times = match &o.tpattern {
        Some(spec) => {
            let times = tpattern_times(spec, nz, tr, settings, log)?;
            log.progress(&format!("++ using tpattern = {} {unit}", list(&times)));
            times
        }
        None => match image.slice_timing.clone() {
            Some(h) => {
                log.progress(&format!("++ using tpattern = {} {unit}", list(&h.times)));
                // AFNI-03 is named whenever the header's code-2 timing is used (§6.4).
                if let Some(n) = timing::seq_dec_notice(&h, mode) {
                    log.notice(&n);
                }
                if h.times.iter().any(|&t| t < 0.0 || t > tr) {
                    log.warning(&format!(
                        "some value in tpattern is outside range 0..TR={}",
                        g32(tr)
                    ));
                    let (fields, zero) = timing::copy_slice_fields(&h.times, mode);
                    if zero {
                        log.notice(&timing::zero_times_notice(nz, mode));
                    }
                    return write_copy(&mut image, Some(fields), &target, mode, settings, log);
                }
                h.times
            }
            None => {
                log.warning("dataset is already aligned in time!");
                return write_copy(&mut image, None, &target, mode, settings, log);
            }
        },
    };
    if times.iter().all(|&t| t == times[0]) {
        log.warning(&format!("input has only 1 time offset, {}", g32(times[0])));
    }
    let tzero = match (o.slice, o.tzero) {
        (Some(k), _) => times[k],
        (None, Some(z)) => z,
        (None, None) => mean_time(&times),
    };
    log.progress(&format!("++ common time point set to {}", g32(tzero)));
    log.progress("++ copying input dataset bricks");
    if deconflict == Deconflict::Stop && output::exists(prefix) {
        return Err(Failure::Fatal(format!(
            "output dataset already exists '{}'",
            afni_name(prefix)
        )));
    }
    log.stdout(&format!("++ updating time offset to {}", g32(tzero)));

    let params = TshiftParams {
        tr,
        slice_times: times,
        tzero,
        ignore: o.ignore,
        method: o.method,
        restore: o.restore,
        detrend: !o.no_detrend,
        n_threads: target.n_threads,
        upstream_defects: mode,
    };
    if o.method == Method::Fourier {
        log.progress(&format!(
            "++ Time series length = {nt}; FFT length set to {}",
            crate::tshift::series::fourier_length(nt)
        ));
    }
    for (k, s) in slice_shifts(&params).iter().enumerate() {
        log.progress(&format!("++ slice {k}: fractional shift = {}", g32(*s)));
    }
    // AFNI_NIFTI_SCALE=NO (AFNI-23).
    let mut scaling = None;
    if let Some(kept) = image.kept_intercept {
        let factor = image.bricks.factors.first().copied().unwrap_or(0.0);
        let (plan, notice) =
            dataset::intercept_plan(&kept, factor, false, o.restore, o.ignore, mode);
        if let Some(n) = notice {
            log.notice(&n);
        }
        match plan {
            InterceptPlan::ReadScaled => dataset::read_scaled(&mut image.bricks, &kept),
            InterceptPlan::InHeader(s) => scaling = Some(s),
            InterceptPlan::Afni => {}
        }
    }
    for n in tshift(&mut image.bricks, &params).map_err(|e| Failure::Fatal(e.to_string()))? {
        log.notice(&n);
    }

    let seconds = |t: f32| {
        if ms {
            (f64::from(t) / 1000.0) as f32
        } else {
            t
        }
    };
    // AFNI-19: with AFNI_ALLOW_MILLISECONDS, AFNI writes the time offset in milliseconds.
    let mut toffset = seconds(tzero);
    if ms && settings.yes("AFNI_ALLOW_MILLISECONDS") && tzero != 0.0 {
        let (s, m) = (g32(seconds(tzero)), g32(tzero));
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "AFNI_ALLOW_MILLISECONDS is set and the TR is in milliseconds: the time offset \
                 is written in seconds ({s}), with the TR; AFNI writes it in milliseconds ({m}) \
                 and labels it seconds"
            ),
            UpstreamDefects::Reproduce => format!(
                "AFNI_ALLOW_MILLISECONDS is set and the TR is in milliseconds: the time offset \
                 is written in milliseconds ({m}) and labelled seconds, as AFNI does"
            ),
        };
        log.notice(&Notice::new(&defects::AFNI_19, mode, detail));
        if mode.reproduce() {
            toffset = tzero;
        }
    }
    let timing = OutputTiming {
        tr: seconds(tr),
        toffset,
        slices: None,
    };
    write_output(
        &image, &timing, scaling, false, &target, mode, settings, log,
    )
}

/// Writes the input unchanged (AFNI's "output dataset is just a copy of input dataset"), with
/// the slice fields of a copy that keeps the header's slice times.
fn write_copy(
    image: &mut AfniImage,
    slices: Option<SliceFields>,
    target: &Target<'_>,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    log: &mut Log<'_>,
) -> Result<u8, Failure> {
    log.warning("==>> output dataset is just a copy of input dataset");
    let mut scaling = None;
    if let Some(kept) = image.kept_intercept {
        let factor = image.bricks.factors.first().copied().unwrap_or(0.0);
        let (plan, notice) = dataset::intercept_plan(&kept, factor, true, Restore::Trend, 0, mode);
        if let Some(n) = notice {
            log.notice(&n);
        }
        if let InterceptPlan::InHeader(s) = plan {
            scaling = Some(s);
        }
    }
    let timing = OutputTiming {
        tr: image.tr,
        toffset: image.toffset,
        slices,
    };
    write_output(image, &timing, scaling, true, target, mode, settings, log)
}

/// `-verbose`'s last line: a labelled message after a copy, plain text after a shift
/// (observed).
fn wrote(log: &mut Log<'_>, copy: bool, name: &str) {
    if log.verbose {
        if copy {
            log.info(&format!("Wrote output: {name}"));
        } else {
            log.raw(&format!("++ Wrote output: {name}"));
        }
    }
}

/// Writes `image`'s bricks with the output header of `timing`, as AFNI does with an existing
/// output (`AFNI_DECONFLICT`, AFNI-11), a name it refuses when writing (AFNI-12) and an output
/// its writer cannot open (AFNI-26).
#[allow(clippy::too_many_arguments)]
fn write_output(
    image: &AfniImage,
    timing: &OutputTiming,
    scaling: Option<OutputScaling>,
    copy: bool,
    target: &Target<'_>,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    log: &mut Log<'_>,
) -> Result<u8, Failure> {
    let prefix = target.prefix;
    let mut path = prefix.to_owned();
    let mut name = afni_name(prefix);
    if output::exists(prefix) {
        match target.deconflict {
            Deconflict::OverwriteExact => {
                if !settings.overwrite_option() {
                    log.warning(&format!("Over-writing dataset {name}"));
                }
            }
            Deconflict::Overwrite => {}
            Deconflict::Rename => {
                let new = output::renamed(prefix).ok_or_else(|| {
                    Failure::Fatal(format!("no free name to rename the output '{prefix}' to"))
                })?;
                log.warning(&format!(
                    "changed output dataset name from '{}' to '{}'",
                    file_name(prefix),
                    file_name(&new)
                ));
                name = afni_name(&new);
                path = new;
            }
            // A copy with AFNI_DECONFLICT=NO: as unset (a shift stopped before the work).
            Deconflict::Stop | Deconflict::Unset => {
                log.notice(&afni_11(mode));
                let conflict = format!(
                    "output dataset name '{}' conflicts with existing file",
                    file_name(prefix)
                );
                if mode.fix() {
                    return Err(Failure::Fatal(conflict));
                }
                log.error(&conflict);
                log.error("dataset NOT written to disk!");
                let would = output::renamed(prefix).map_or(name, |n| afni_name(&n));
                wrote(log, copy, &would);
                return Ok(0);
            }
        }
    }
    if refused_when_written(&name, settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES")) {
        if !target.prefix_refused {
            log.notice(&afni_12(&name, false, mode));
        }
        if mode.reproduce() {
            log.error(&format!("Illegal filename for NIfTI output: {name}"));
            wrote(log, copy, &name);
            return Ok(0);
        }
    }
    let [nx, ny, nz, _] = image.bricks.shape;
    // AFNI writes a dataset of one row as 1D text (AFNI-20).
    let one_row_text = ny == 1 && nz == 1 && !settings.yes("AFNI_WRITE_1D_AS_PREFIX");
    if one_row_text {
        log.notice(&Notice::new(
            &defects::AFNI_20,
            UpstreamDefects::Fix,
            format!(
                "the dataset has one row ({nx} x 1 x 1 voxels): written as NIfTI, in both \
                 modes; AFNI writes a 1D text file under that name (unless \
                 AFNI_WRITE_1D_AS_PREFIX is YES), with a random identifier"
            ),
        ));
    }
    let (header, notices, messages) = dataset::output_header(
        &image.geometry,
        &image.bricks,
        timing,
        scaling,
        mode,
        settings,
    )
    .map_err(Failure::Fatal)?;
    for m in &messages {
        log.message(m);
    }
    for n in &notices {
        log.notice(n);
    }
    // AFNI-26: AFNI's writer cannot open the output (a missing or read-only directory, a
    // read-only file, a directory of that name). AFNI prints the NIfTI library's message
    // (plain, whatever AFNI_MESSAGE_PREFIX and AFNI_MESSAGE_COLORIZE say), writes nothing and
    // exits with status 0; its 1D text writer (AFNI-20) says nothing (observed). A write that
    // fails later stays an error in both modes.
    if let Some(e) = output::cannot_open(Path::new(&path)) {
        log.notice(&afni_26(mode));
        if mode.fix() {
            return Err(Failure::Fatal(format!(
                "cannot open output file '{name}' ({e}): nothing was written"
            )));
        }
        if !one_row_text {
            log.raw(&format!(
                "** ERROR (nifti_image_write_engine): cannot open output file '{name}'"
            ));
        }
        wrote(log, copy, &name);
        return Ok(0);
    }
    output::write_replacing(Path::new(&path), |tmp| {
        dataset::write(tmp, &header, &image.bricks, target.n_threads).map_err(|e| e.to_string())
    })
    .map_err(|e| Failure::Fatal(format!("can't write the output '{path}': {e}")))?;
    wrote(log, copy, &name);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Options, String> {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let settings = AfniSettings::new();
        let mut log = Log::new(&mut out, &mut err, &settings, UpstreamDefects::Fix);
        parse(&args, &mut log, &settings, UpstreamDefects::Fix).map_err(|f| match f {
            Failure::Fatal(m) | Failure::Error(m) => m,
        })
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
        assert_eq!(parse_args(&["-TR"]).unwrap_err(), "-TR needs an argument!");
        assert_eq!(
            parse_args(&["-ignore"]).unwrap_err(),
            "-ignore needs an argument!"
        );
        assert!(parse_args(&["-linear"]).is_err());
        // Arguments after the dataset are ignored.
        assert_eq!(
            parse_args(&["in.nii", "-linear"]).unwrap().method,
            Method::Fourier
        );
        // Any -prefix is accepted by default (AFNI-12).
        assert!(
            parse_args(&["-prefix", "a b.nii", "in.nii"])
                .unwrap()
                .prefix_refused
        );
    }

    #[test]
    fn tpattern_files_are_checked_against_the_tr() {
        let settings = AfniSettings::new();
        let times = |spec: &str, nz, tr| {
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let mut log = Log::new(&mut out, &mut err, &settings, UpstreamDefects::Fix);
            tpattern_times(spec, nz, tr, &settings, &mut log).map_err(|f| match f {
                Failure::Fatal(m) | Failure::Error(m) => m,
            })
        };
        assert_eq!(times("@1D: 0 1 0.5", 3, 2.0).unwrap(), [0.0, 1.0, 0.5]);
        assert_eq!(times("@1D: 0 1 0.5 1.5", 3, 2.0).unwrap(), [0.0, 1.0, 0.5]);
        assert_eq!(
            times("@1D: 0 1", 3, 2.0).unwrap_err(),
            "tpattern file 1D: 0 1 has 2 values but have 3 slices"
        );
        assert_eq!(
            times("@1D: 0 2.0001 1", 3, 2.0).unwrap_err(),
            "Illegal value 2.0001 in tpattern file 1D: 0 2.0001 1"
        );
        assert!(times("@1D: 0 -0.1 1", 3, 2.0).is_err());
        assert_eq!(times("@1D: 0 2 1", 3, 2.0).unwrap(), [0.0, 2.0, 1.0]);
        assert_eq!(
            times("ALT+Z", 3, 2.0).unwrap_err(),
            "Unknown tpattern = ALT+Z"
        );
    }

    #[test]
    fn echoed_command_lines_quote_refused_names() {
        let args: Vec<String> = ["-prefix", "a b.nii", "é.nii", "3dcalc a b.nii"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(
            echo_line(&args, &AfniSettings::new()),
            "+++++===== 3dTshift -prefix 'a b.nii' '  .nii' 3dcalc a b.nii"
        );
        let arbitrary = AfniSettings::from_pairs([("AFNI_ALLOW_ARBITRARY_FILENAMES", "YES")]);
        assert_eq!(
            echo_line(&args[..2], &arbitrary),
            "+++++===== 3dTshift -prefix a b.nii"
        );
    }
}
