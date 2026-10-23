// SPDX-License-Identifier: Apache-2.0
//! `3dAutomask` with its original arguments (the behaviour of AFNI 25.2.09,
//! `specs/3dAutomask.md` §8), AFNI's global options and environment variables included.
//!
//! Differences from AFNI, all deliberate: the outputs must be NIfTI files; AFNI's default
//! output (`automask` in AFNI's own format) is refused; AFNI's version banner and compile date
//! are not printed; where AFNI crashes (an option's value missing, AFNI-40) larmorx refuses
//! with one line. AFNI's defects are corrected unless `LARMORX_UPSTREAM_DEFECTS` is
//! `reproduce` (D13); a warning names each one that changed (or would have changed) a result.

use std::io::Write;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notice, UpstreamDefects};

use crate::automask::morph::Neighbourhood;
use crate::automask::{self, AutomaskOptions};
use crate::cfmt;
use crate::dataset;
use crate::msg::{Log, Message};
use crate::output::refused;
use crate::settings::{self, AfniSettings, Program, take_global_options_of};

use super::common::{self, Failure, Target, check_nifti, echo_line, echoes, strtod, strtol_int};
use super::threads;

/// The program, for the warnings about `-D` options.
pub const PROGRAM: Program = Program {
    name: "3dAutomask",
    consults: settings::is_consulted_by_masks,
};

const USAGE: &str = "\
Usage: 3dAutomask [options] dataset   (larmorx, clean-room re-implementation of AFNI 3dAutomask)

Makes a brain mask (1 inside, 0 outside) from an EPI time series or a skull-stripped anatomical
NIfTI dataset: a clip level that varies gradually across the image, the largest connected
cluster, peeled and restored, its holes filled.

Options (they must come before the dataset):
  -prefix ppp        the mask (NIfTI: .nii or .nii.gz)
  -apply_prefix ppp  the input with every voxel outside the mask 0
  -depth ppp         how many erosions remove each mask voxel
  -clfrac cc         clip level fraction, 0.1 to 0.9 [0.5] (also -mfrac)
  -nograd            one clip level for the whole image
  -peels pp          peel and restore the mask pp times [1]; 0 turns it off
  -nbhrs nn          neighbours (of 18) a voxel needs not to be peeled, 6 to 26 [17]
  -NN1 -NN2 -NN3     neighbourhood of -dilate, -erode and -depth [NN2]; also sets -nbhrs
  -eclip             remove exterior voxels below the clip level
  -dilate nd         dilate the mask nd times
  -erode ne          erode the mask ne times
  -SI hh             clear everything more than hh mm inferior to the most superior voxel
  -q                 no progress messages

AFNI's global options may stand anywhere: -overwrite, -Dname=value, -ok_1D_text, -skip_afnirc.
";

/// larmorx's `-verbose`-like first line is not printed by 3dAutomask (it has no verbose
/// option); the implementation is named in the Python results.
pub const IMPLEMENTATION: &str = concat!(
    "implementation: original (larmorx ",
    env!("CARGO_PKG_VERSION"),
    ", Apache-2.0), clean-room 3dAutomask"
);

/// Runs `3dAutomask` with `args`; returns the exit code. The D13 mode comes from
/// `LARMORX_UPSTREAM_DEFECTS`, AFNI's settings from the process environment.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match UpstreamDefects::from_env() {
        Ok(mode) => main_with_settings(args, mode, AfniSettings::from_env(), threads(), out, err),
        Err(e) => {
            let _ = writeln!(err, "** FATAL ERROR: {e}");
            EXIT_BAD_MODE
        }
    }
}

/// The parsed command line.
#[derive(Debug)]
struct Options {
    prefix: Option<String>,
    apply_prefix: Option<String>,
    depth: Option<String>,
    /// The names AFNI refuses at the option scan (AFNI-12).
    refused: Vec<String>,
    options: AutomaskOptions,
    quiet: bool,
    dataset: String,
}

/// The option values AFNI reads with C's conversions.
fn missing_value(option: &str) -> Failure {
    Failure::Fatal(format!(
        "{option} needs a value: AFNI crashes (Fatal Signal 11) when an option's value is \
         missing (AFNI-40)"
    ))
}

fn parse(
    args: &[String],
    log: &mut Log<'_>,
    settings: &AfniSettings,
    mode: UpstreamDefects,
) -> Result<Options, Failure> {
    let mut o = Options {
        prefix: None,
        apply_prefix: None,
        depth: None,
        refused: Vec::new(),
        options: AutomaskOptions::default(),
        quiet: false,
        dataset: String::new(),
    };
    let mut nbhrs: Option<(f64, String)> = None;
    let mut nn_threshold: Option<u32> = None;
    let (mut dilate_given, mut erode_given) = (false, false);
    let arbitrary = settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES");
    let mut i = 0;
    while i < args.len() && args[i].starts_with('-') {
        let arg = args[i].as_str();
        let value = |i: usize| args.get(i + 1).map(String::as_str);
        let name_option = |i: usize, o: &mut Options, log: &mut Log<'_>| {
            let Some(v) = value(i) else {
                return Err(Failure::Fatal(format!("{arg} (null) is illegal!")));
            };
            if refused(v, arbitrary) {
                o.refused.push(v.to_owned());
                log.notice(&common::afni_12(arg, v, true, mode));
                if mode.reproduce() {
                    return Err(Failure::Fatal(format!("{arg} {v} is illegal!")));
                }
            }
            Ok(v.to_owned())
        };
        let b = arg.as_bytes();
        match arg {
            "-prefix" => o.prefix = Some(name_option(i, &mut o, log)?),
            "-apply_prefix" => o.apply_prefix = Some(name_option(i, &mut o, log)?),
            "-depth" => o.depth = Some(name_option(i, &mut o, log)?),
            "-clfrac" | "-mfrac" => {
                let v = value(i).ok_or_else(|| missing_value(arg))?;
                let c = strtod(v) as f32;
                if !(0.1..=0.9).contains(&c) {
                    return Err(Failure::Fatal(format!(
                        "-clfrac value {} is illegal!",
                        cfmt::f(f64::from(c))
                    )));
                }
                o.options.mask.clfrac = c;
            }
            "-NN1" | "-NN2" | "-NN3" => {
                let level = b[3] - b'0';
                o.options.nn = Neighbourhood::from_level(level).expect("1, 2 or 3");
                // AFNI-42 (kept): -NN1/2/3 also set the peel threshold.
                nn_threshold = Some([6, 18, 26][usize::from(level - 1)]);
                i += 1;
                continue;
            }
            "-eclip" => {
                o.options.mask.exterior_clip = true;
                i += 1;
                continue;
            }
            "-dilate" | "-erode" => {
                let v = value(i).ok_or_else(|| missing_value(arg))?;
                let n = strtol_int(v);
                if n < 0 {
                    return Err(Failure::Fatal(format!("{arg} {v} is illegal!")));
                }
                if arg == "-dilate" {
                    o.options.dilate = n as usize;
                    dilate_given = true;
                } else {
                    o.options.erode = n as usize;
                    erode_given = true;
                }
            }
            "-SI" => {
                let v = value(i).ok_or_else(|| missing_value(arg))?;
                let h = strtod(v) as f32;
                if h <= 0.0 || h.is_nan() {
                    return Err(Failure::Fatal(format!(
                        "-SI value {} is illegal!",
                        cfmt::f(f64::from(h))
                    )));
                }
                o.options.si = Some(h);
            }
            "-q" => {
                o.quiet = true;
                i += 1;
                continue;
            }
            _ if b.len() >= 5 && &b[..5] == b"-nogr" => {
                o.options.mask.gradual = false;
                i += 1;
                continue;
            }
            _ if b.len() >= 5 && &b[..5] == b"-peel" => {
                let v = value(i).ok_or_else(|| missing_value(arg))?;
                o.options.requested_peels = strtod(v).trunc() as i64;
            }
            _ if b.len() >= 5 && &b[..5] == b"-nbhr" => {
                let v = value(i).ok_or_else(|| missing_value(arg))?;
                nbhrs = Some((strtod(v).trunc(), v.to_owned()));
            }
            _ => return Err(Failure::Fatal(format!("ILLEGAL option: {arg}"))),
        }
        i += 2;
    }
    // -nbhrs (AFNI-41) and -NN1/2/3 (AFNI-42).
    let mut threshold = 17u32;
    if let Some((t, text)) = &nbhrs {
        if (6.0..=26.0).contains(t) {
            threshold = *t as u32;
        } else {
            let detail = match mode {
                UpstreamDefects::Fix => format!(
                    "-nbhrs {text} is outside 6..26: an error (exit status 1); AFNI silently uses 17"
                ),
                UpstreamDefects::Reproduce => format!(
                    "-nbhrs {text} is outside 6..26: 17 is used, as AFNI does"
                ),
            };
            log.notice(&Notice::new(&defects::AFNI_41, mode, detail));
            if mode.fix() {
                return Err(Failure::Fatal(format!(
                    "-nbhrs {text} is illegal: the number of neighbours must be 6 to 26 (AFNI \
                     silently uses 17, AFNI-41)"
                )));
            }
        }
    }
    if let Some(t) = nn_threshold {
        threshold = t;
    }
    o.options.mask.threshold = threshold;
    o.dataset = args.get(i).cloned().unwrap_or_default();
    if dilate_given && erode_given {
        log.warning("Combining dilate,erode options is like peels option");
    }
    if args.get(i).is_none() {
        return Err(Failure::Fatal("Can't open dataset '(null)'".into()));
    }
    Ok(o)
}

/// [`main`] with the D13 mode, AFNI's settings and the thread count given.
pub fn main_with_settings(
    args: &[String],
    mode: UpstreamDefects,
    mut settings: AfniSettings,
    n_threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if args.is_empty() || matches!(args[0].as_str(), "-help" | "-h") {
        let _ = out.write_all(USAGE.as_bytes());
        return 0;
    }
    let globals = take_global_options_of(args, &mut settings, mode, &PROGRAM);
    let mut log = Log::new(out, err, &settings, mode);
    if echoes(&settings) {
        log.raw(&echo_line(PROGRAM.name, &globals.rest, &settings));
    }
    for w in &globals.warnings {
        log.warning(w);
    }
    for n in &globals.notices {
        log.notice(n);
    }
    let quiet_startup = settings.yes("AFNI_QUIET_STARTUP");
    if !quiet_startup {
        log.info("Authored by: Emperor Zhark");
    }
    let result = parse(&globals.rest, &mut log, &settings, mode).and_then(|o| {
        let progress = !o.quiet;
        execute(&o, mode, &settings, n_threads, progress, &mut log)
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

fn execute<'a>(
    o: &'a Options,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    n_threads: usize,
    progress: bool,
    log: &mut Log<'_>,
) -> Result<u8, Failure> {
    let quiet_startup = settings.yes("AFNI_QUIET_STARTUP");
    let outputs: Vec<(&str, &str)> = [
        ("-prefix", &o.prefix),
        ("-apply_prefix", &o.apply_prefix),
        ("-depth", &o.depth),
    ]
    .into_iter()
    .filter_map(|(opt, p)| p.as_deref().map(|p| (opt, p)))
    .collect();
    if o.prefix.is_none() && o.apply_prefix.is_none() {
        return Err(Failure::Fatal(
            "larmorx writes NIfTI only: give -prefix name.nii (AFNI's default 'automask' is its \
             own format)"
                .into(),
        ));
    }
    for &(opt, p) in &outputs {
        check_nifti(opt, p)?;
    }
    for &(_, p) in &outputs {
        common::refuse_existing(p, mode, settings, log)?;
    }
    // Open the dataset.
    let (mut messages, mut notices) = (Vec::new(), Vec::new());
    let found = crate::input::resolve(&o.dataset, settings, mode, &mut messages, &mut notices);
    for m in &messages {
        log.message(m);
    }
    for n in &notices {
        log.notice(n);
    }
    let found = found.map_err(|e| {
        if e.0.starts_with("larmorx:") {
            Failure::Fatal(e.0)
        } else {
            Failure::Fatal(format!("Can't open dataset '{}'", o.dataset))
        }
    })?;
    let mut image = dataset::read_as(&found.path, &found.shown, n_threads, mode, settings, true)
        .map_err(|e| Failure::Fatal(format!("Can't open dataset '{}': {e}", o.dataset)))?;
    for m in std::mem::take(&mut image.messages) {
        log.message(&m);
    }
    let mut late: Vec<Notice> = std::mem::take(&mut image.notices);
    let say = |log: &mut Log<'_>, m: Message| {
        if progress {
            log.message(&m);
        }
    };
    say(log, Message::Info(format!("Loading dataset {}", o.dataset)));
    let mut options = o.options;
    options.apply = o.apply_prefix.is_some();
    options.depth = o.depth.is_some();
    let run =
        automask::run(&mut image, &options, mode, settings, n_threads).map_err(Failure::Fatal)?;
    for m in run.messages {
        say(log, m);
    }
    late.extend(run.notices);
    if let Some(e) = run.failure {
        for n in &late {
            log.notice(n);
        }
        return Err(Failure::Fatal(e));
    }
    // The outputs.
    let mut written_notices = Vec::new();
    let output_line = |log: &mut Log<'_>, w: &Option<(String, String)>| {
        if let Some(line) = common::output_line(w)
            && progress
            && !quiet_startup
        {
            log.info(&line);
        }
    };
    let names = |opt: &'static str, prefix: &'a str| Target {
        option: opt,
        prefix,
        refused_at_scan: o.refused.iter().any(|r| r == prefix),
    };
    if let (Some(p), Some((header, bricks))) = (&o.prefix, &run.mask) {
        let w = common::write_dataset(
            &names("-prefix", p),
            header,
            bricks,
            n_threads,
            mode,
            settings,
            log,
            &mut written_notices,
        )?;
        output_line(log, &w);
    }
    if let Some(p) = &o.apply_prefix {
        say(log, Message::Info("applying mask to original data".into()));
        say(log, Message::Info("Writing masked data".into()));
        let (header, bricks) = run.masked.as_ref().expect("the masked input");
        let w = common::write_dataset(
            &names("-apply_prefix", p),
            header,
            bricks,
            n_threads,
            mode,
            settings,
            log,
            &mut written_notices,
        )?;
        output_line(log, &w);
    }
    if let (Some(p), Some((header, bricks))) = (&o.depth, &run.depth) {
        say(log, Message::Info("Writing depth dataset".into()));
        let w = common::write_dataset(
            &names("-depth", p),
            header,
            bricks,
            n_threads,
            mode,
            settings,
            log,
            &mut written_notices,
        )?;
        output_line(log, &w);
    }
    late.extend(written_notices);
    let mut seen = Vec::new();
    for n in &late {
        if !seen.contains(&n.defect.id) {
            seen.push(n.defect.id);
            log.notice(n);
        }
    }
    say(log, Message::Info("CPU time = 0.000000 sec".into()));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str], mode: UpstreamDefects) -> Result<Options, String> {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let settings = AfniSettings::new();
        let mut log = Log::new(&mut out, &mut err, &settings, mode);
        parse(&args, &mut log, &settings, mode).map_err(|f| match f {
            Failure::Fatal(m) | Failure::Error(m) => m,
        })
    }

    const FIX: UpstreamDefects = UpstreamDefects::Fix;
    const REPRODUCE: UpstreamDefects = UpstreamDefects::Reproduce;

    #[test]
    fn options() {
        let o = parse_args(
            &["-nogrxyz", "-peelx", "2.9", "-nbhr", "12", "-dilate", " +3", "in.nii", "-q"],
            FIX,
        )
        .unwrap();
        assert!(!o.options.mask.gradual && !o.quiet);
        assert_eq!(o.options.requested_peels, 2);
        assert_eq!(o.options.mask.threshold, 12);
        assert_eq!(o.options.dilate, 3);
        assert_eq!(o.dataset, "in.nii");
        // -NN2 sets 18 whatever -nbhrs says (AFNI-42).
        let o = parse_args(&["-nbhrs", "12", "-NN2", "in.nii"], FIX).unwrap();
        assert_eq!(o.options.mask.threshold, 18);
        assert_eq!(o.options.nn, Neighbourhood::Edges);
        // AFNI-41: refused by default, 17 reproduced.
        assert!(parse_args(&["-nbhrs", "40", "in.nii"], FIX).is_err());
        let o = parse_args(&["-nbhrs", "40", "in.nii"], REPRODUCE).unwrap();
        assert_eq!(o.options.mask.threshold, 17);
        // Errors.
        assert_eq!(
            parse_args(&["-clfrac", "0.05", "in.nii"], FIX).unwrap_err(),
            "-clfrac value 0.050000 is illegal!"
        );
        assert_eq!(
            parse_args(&["-dilate", "-1", "in.nii"], FIX).unwrap_err(),
            "-dilate -1 is illegal!"
        );
        assert_eq!(
            parse_args(&["-SI", "-3", "in.nii"], FIX).unwrap_err(),
            "-SI value -3.000000 is illegal!"
        );
        assert_eq!(
            parse_args(&["-prefix"], FIX).unwrap_err(),
            "-prefix (null) is illegal!"
        );
        assert!(parse_args(&["-peels"], FIX).unwrap_err().contains("AFNI-40"));
        assert_eq!(
            parse_args(&["-bogus", "in.nii"], FIX).unwrap_err(),
            "ILLEGAL option: -bogus"
        );
        assert_eq!(
            parse_args(&["-q"], FIX).unwrap_err(),
            "Can't open dataset '(null)'"
        );
        // AFNI-12 at the option scan: accepted by default, refused reproduced.
        assert!(parse_args(&["-prefix", "a b.nii", "in.nii"], FIX).is_ok());
        assert_eq!(
            parse_args(&["-prefix", "a b.nii", "in.nii"], REPRODUCE).unwrap_err(),
            "-prefix a b.nii is illegal!"
        );
    }
}
