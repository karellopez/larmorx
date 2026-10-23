// SPDX-License-Identifier: Apache-2.0
//! `3dUnifize` with its original arguments (the behaviour of AFNI 25.2.09,
//! `specs/3dUnifize.md` §8), AFNI's global options and environment variables included.
//!
//! Differences from AFNI, all deliberate: the outputs must be NIfTI files; AFNI's default
//! output (`Unifized` in AFNI's own format) is refused; AFNI's version banner and compile date
//! are not printed; where AFNI crashes (`-T2up` without its value, AFNI-40; `-T2` on a plane
//! smaller than 16 × 16, AFNI-46) larmorx refuses with one line. AFNI's defects are corrected
//! unless `LARMORX_UPSTREAM_DEFECTS` is `reproduce` (D13).

use std::io::Write;
use std::time::Instant;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notice, UpstreamDefects};

use crate::cfmt;
use crate::dataset::{self, AfniImage};
use crate::msg::{Log, Message};
use crate::output::refused;
use crate::settings::{self, AfniSettings, Program, take_global_options_of};
use crate::unifize::{self, Event, Saved, UnifizeOptions};

use super::common::{self, Failure, Target, check_nifti, echo_line, echoes, strtod};
use super::threads;

/// The program, for the warnings about `-D` options.
pub const PROGRAM: Program = Program {
    name: "3dUnifize",
    consults: settings::is_consulted_by_masks,
};

const USAGE: &str = "\
Usage: 3dUnifize [options] dataset   (larmorx, clean-room re-implementation of AFNI 3dUnifize)

Makes the white-matter intensity of a T1-weighted NIfTI volume uniform (about 1000): at each
voxel, the mean of the automasked values between two percentiles in a ball around it estimates
the local white-matter intensity, which divides the image.

Options:
  -prefix pp      the output (NIfTI: .nii or .nii.gz)
  -input dd       the dataset (also -inset; or the first argument after the options)
  -T2             invert the contrast first and back after; twice: no inversion back
  -T2up uu        the upper percentile of the inversion, 90 to 100 [98.5]
  -GM             also scale the grey matter
  -Urad rr        the ball's radius in voxels, 5 to 99 [18.3]
  -rbt R b t      radius and percentiles [18.3 70 80] (also -param)
  -clfrac cc      the automask's clip level fraction, 0.1 to 0.9 [0.2] (also -mfrac)
  -ssave ss       save the scale factors
  -amsave aa      save the automasked input
  -noduplo        never work on a half-size copy (also -nodouble)
  -EPI            an EPI series: the scaling of its median volume applied to every volume
  -nosquash       do not squash values above 1300 (also -oink)
  -quiet          no progress messages
  -verb           the automask's messages too

AFNI's global options may stand anywhere: -overwrite, -Dname=value, -ok_1D_text, -skip_afnirc.
";

/// Runs `3dUnifize` with `args`; returns the exit code. The D13 mode comes from
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
    ssave: Option<String>,
    amsave: Option<String>,
    /// The names AFNI refuses at the option scan (AFNI-12).
    refused: Vec<String>,
    options: UnifizeOptions,
    quiet: bool,
    /// The dataset as given, and the image when `-input` opened it during the scan.
    dataset: String,
    image: Option<AfniImage>,
}

/// Opens the dataset `name` as AFNI does (its messages and notices to `log`).
fn open(
    name: &str,
    settings: &AfniSettings,
    mode: UpstreamDefects,
    n_threads: usize,
    log: &mut Log<'_>,
) -> Result<AfniImage, Failure> {
    let (mut messages, mut notices) = (Vec::new(), Vec::new());
    let found = crate::input::resolve(name, settings, mode, &mut messages, &mut notices);
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
            Failure::Fatal(format!("Can't open dataset '{name}'"))
        }
    })?;
    let mut image = dataset::read_as(&found.path, &found.shown, n_threads, mode, settings, true)
        .map_err(|e| Failure::Fatal(format!("Can't open dataset '{name}': {e}")))?;
    for m in std::mem::take(&mut image.messages) {
        log.message(&m);
    }
    for n in std::mem::take(&mut image.notices) {
        log.notice(&n);
    }
    Ok(image)
}

fn parse(
    args: &[String],
    log: &mut Log<'_>,
    settings: &AfniSettings,
    mode: UpstreamDefects,
    n_threads: usize,
) -> Result<Options, Failure> {
    let mut o = Options {
        prefix: None,
        ssave: None,
        amsave: None,
        refused: Vec::new(),
        options: UnifizeOptions::default(),
        quiet: false,
        dataset: String::new(),
        image: None,
    };
    let arbitrary = settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES");
    let mut i = 0;
    while i < args.len() && args[i].starts_with('-') {
        let arg = args[i].as_str();
        let value = |i: usize| {
            args.get(i + 1)
                .map(String::as_str)
                .ok_or_else(|| Failure::Fatal(format!("Need argument after '{arg}'")))
        };
        let is = |name: &str| arg.eq_ignore_ascii_case(name);
        match arg {
            "-prefix" | "-ssave" | "-amsave" => {
                let v = value(i)?;
                if refused(v, arbitrary) {
                    o.refused.push(v.to_owned());
                    log.notice(&common::afni_12(arg, v, true, mode));
                    if mode.reproduce() {
                        return Err(Failure::Fatal(format!("Illegal value after {arg}!")));
                    }
                }
                let slot = match arg {
                    "-prefix" => &mut o.prefix,
                    "-ssave" => &mut o.ssave,
                    _ => &mut o.amsave,
                };
                *slot = Some(v.to_owned());
                i += 2;
                continue;
            }
            "-input" | "-inset" => {
                let v = value(i)?;
                if o.image.is_some() {
                    return Err(Failure::Fatal(format!("Can't use '{arg}' twice")));
                }
                o.image = Some(open(v, settings, mode, n_threads, log)?);
                o.dataset = v.to_owned();
                i += 2;
                continue;
            }
            "-Urad" => {
                let r = strtod(value(i)?) as f32;
                if !(5.0..=99.0).contains(&r) {
                    return Err(Failure::Fatal(format!(
                        "Illegal value {} after option -Urad",
                        cfmt::f(f64::from(r))
                    )));
                }
                o.options.radius = r;
                i += 2;
                continue;
            }
            "-rbt" | "-param" => {
                if i + 3 >= args.len() {
                    return Err(Failure::Fatal(format!(
                        "Need 3 arguments (R pb pt) after '{arg}'"
                    )));
                }
                let r = strtod(&args[i + 1]) as f32;
                let b = strtod(&args[i + 2]) as f32;
                let t = strtod(&args[i + 3]) as f32;
                if !(5.0..=99.0).contains(&r)
                    || !(30.0..=80.0).contains(&b)
                    || !(b < t && t <= 90.0)
                {
                    return Err(Failure::Fatal(format!(
                        "Illegal values (R pb pt) after '{arg}'"
                    )));
                }
                (o.options.radius, o.options.bottom, o.options.top) = (r, b, t);
                i += 4;
                continue;
            }
            "-clfrac" | "-mfrac" => {
                let c = strtod(value(i)?) as f32;
                if !(0.1..=0.9).contains(&c) {
                    return Err(Failure::Fatal(format!(
                        "-clfrac value {} is illegal!",
                        cfmt::f(f64::from(c))
                    )));
                }
                o.options.clfrac = c;
                i += 2;
                continue;
            }
            "-useall" => {
                log.warning("-useall option is disabled in this version");
                i += 1;
                continue;
            }
            "-noduplo" | "-nodouble" => {
                o.options.duplo = false;
                i += 1;
                continue;
            }
            _ => {}
        }
        if is("-T2up") {
            let Some(v) = args.get(i + 1) else {
                return Err(Failure::Fatal(
                    "-T2up needs a value: AFNI crashes (Fatal Signal 11) when an option's value \
                     is missing (AFNI-40)"
                        .into(),
                ));
            };
            let u = strtod(v) as f32;
            if !(90.0..=100.0).contains(&u) {
                return Err(Failure::Fatal(
                    "-T2up value is out of range 90..100 :-(".into(),
                ));
            }
            o.options.t2up = u;
            i += 2;
        } else if is("-T2") {
            o.options.t2 += 1;
            i += 1;
        } else if is("-GM") {
            o.options.gm = true;
            i += 1;
        } else if is("-EPI") {
            o.options.epi = true;
            i += 1;
        } else if is("-nosquash") || is("-oink") {
            o.options.squash = false;
            i += 1;
        } else if is("-quiet") {
            o.quiet = true;
            i += 1;
        } else if is("-verb") {
            o.options.verb = true;
            i += 1;
        } else {
            return Err(Failure::Fatal(format!("Unknown option: {arg}")));
        }
    }
    if o.image.is_none() {
        match args.get(i) {
            Some(d) => o.dataset = d.clone(),
            None => return Err(Failure::Fatal("No dataset name on command line?".into())),
        }
    }
    // AFNI-48 (decided 2026-10-23, rule B): -T2 three or more times.
    if o.options.t2 >= 3 && !o.options.epi {
        let n = o.options.t2;
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "-T2 given {n} times: an error (exit status 1); AFNI neither inverts the result \
                 back nor clips its edges, so the output stays inverted"
            ),
            UpstreamDefects::Reproduce => format!(
                "-T2 given {n} times: the result is neither inverted back nor edge-clipped, as \
                 AFNI does (the output stays inverted)"
            ),
        };
        log.notice(&Notice::new(&defects::AFNI_48, mode, detail));
        if mode.fix() {
            return Err(Failure::Fatal(format!(
                "-T2 given {n} times: AFNI's output for more than two stays inverted (AFNI-48). \
                 Give it once (invert back) or twice (no inversion back, the edges clipped)"
            )));
        }
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
    let start = Instant::now();
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
    let result = parse(&globals.rest, &mut log, &settings, mode, n_threads)
        .and_then(|o| execute(o, mode, &settings, n_threads, start, &mut log));
    match result {
        Ok(code) => code,
        Err(Failure::Fatal(message)) => {
            log.fatal(&message);
            1
        }
    }
}

fn execute(
    mut o: Options,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    n_threads: usize,
    start: Instant,
    log: &mut Log<'_>,
) -> Result<u8, Failure> {
    let Some(prefix) = o.prefix.clone() else {
        return Err(Failure::Fatal(
            "larmorx writes NIfTI only: give -prefix name.nii (AFNI's default 'Unifized' is its \
             own format)"
                .into(),
        ));
    };
    let outputs: Vec<(&str, String)> = [
        ("-prefix", Some(prefix.clone())),
        ("-ssave", o.ssave.clone()),
        ("-amsave", o.amsave.clone()),
    ]
    .into_iter()
    .filter_map(|(opt, p)| p.map(|p| (opt, p)))
    .collect();
    for (opt, p) in &outputs {
        check_nifti(opt, p)?;
    }
    let mut image = match o.image.take() {
        Some(image) => image,
        None => open(&o.dataset, settings, mode, n_threads, log)?,
    };
    for (_, p) in &outputs {
        common::refuse_existing(p, mode, settings, log)?;
    }
    let mut options = o.options;
    options.scale = o.ssave.is_some();
    options.automasked = o.amsave.is_some();
    let quiet = o.quiet;
    let run =
        unifize::run(&mut image, &options, mode, settings, n_threads).map_err(Failure::Fatal)?;
    let mut notices: Vec<Notice> = run.notices.clone();
    for event in &run.events {
        match event {
            Event::Progress(m) => {
                if !quiet {
                    log.message(m);
                }
            }
            Event::Always(m) => log.message(m),
            Event::Write(which) => {
                let (opt, name, output) = match which {
                    Saved::Automasked => ("-amsave", &o.amsave, &run.automasked),
                    Saved::Scale => ("-ssave", &o.ssave, &run.scale),
                };
                if let (Some(name), Some((header, bricks))) = (name, output) {
                    common::write_dataset(
                        &target(opt, name, &o.refused),
                        header,
                        bricks,
                        n_threads,
                        mode,
                        settings,
                        log,
                        &mut notices,
                    )?;
                }
            }
        }
    }
    if let Some(why) = &run.failure {
        print_notices(log, &notices);
        return Err(Failure::Fatal(why.clone()));
    }
    let (header, bricks) = run.output.as_ref().expect("an output");
    let written = common::write_dataset(
        &target("-prefix", &prefix, &o.refused),
        header,
        bricks,
        n_threads,
        mode,
        settings,
        log,
        &mut notices,
    )?;
    if let Some(line) = common::output_line(&written)
        && !settings.yes("AFNI_QUIET_STARTUP")
    {
        log.info(&line);
    }
    print_notices(log, &notices);
    if !quiet && !run.epi {
        log.message(&Message::Info(format!(
            "===== Elapsed = {:.1} sec",
            start.elapsed().as_secs_f64()
        )));
    }
    Ok(0)
}

fn print_notices(log: &mut Log<'_>, notices: &[Notice]) {
    let mut seen = Vec::new();
    for n in notices {
        if !seen.contains(&n.defect.id) {
            seen.push(n.defect.id);
            log.notice(n);
        }
    }
}

/// The output named `prefix` by option `opt`.
fn target<'a>(opt: &'static str, prefix: &'a str, refused: &[String]) -> Target<'a> {
    Target {
        option: opt,
        prefix,
        refused_at_scan: refused.iter().any(|r| r == prefix),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str], mode: UpstreamDefects) -> Result<Options, String> {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let settings = AfniSettings::new();
        let mut log = Log::new(&mut out, &mut err, &settings, mode);
        parse(&args, &mut log, &settings, mode, 1).map_err(|f| match f {
            Failure::Fatal(m) => m,
        })
    }

    const FIX: UpstreamDefects = UpstreamDefects::Fix;

    #[test]
    fn options() {
        let o = parse_args(
            &[
                "-t2",
                "-gm",
                "-NOSQUASH",
                "-rbt",
                "18.3",
                "65",
                "90",
                "-prefix",
                "u.nii",
                "in.nii",
            ],
            FIX,
        )
        .unwrap();
        assert_eq!(o.options.t2, 1);
        assert!(o.options.gm && !o.options.squash);
        assert_eq!(
            (o.options.radius, o.options.bottom, o.options.top),
            (18.3, 65.0, 90.0)
        );
        assert_eq!(o.dataset, "in.nii");
        for (args, message) in [
            (
                &["-Urad", "3", "in.nii"][..],
                "Illegal value 3.000000 after option -Urad",
            ),
            (
                &["-rbt", "18", "20", "80", "in.nii"],
                "Illegal values (R pb pt) after '-rbt'",
            ),
            (
                &["-rbt", "18", "60"],
                "Need 3 arguments (R pb pt) after '-rbt'",
            ),
            (
                &["-clfrac", "0.05", "in.nii"],
                "-clfrac value 0.050000 is illegal!",
            ),
            (
                &["-T2up", "89", "in.nii"],
                "-T2up value is out of range 90..100 :-(",
            ),
            (&["-prefix"], "Need argument after '-prefix'"),
            (&["-bogus", "in.nii"], "Unknown option: -bogus"),
            (&["-quiet"], "No dataset name on command line?"),
            (&["-NODUPLO", "in.nii"], "Unknown option: -NODUPLO"),
        ] {
            assert_eq!(
                parse_args(args, FIX).err().as_deref(),
                Some(message),
                "{args:?}"
            );
        }
        assert!(parse_args(&["-T2up"], FIX).unwrap_err().contains("AFNI-40"));
        // AFNI-48: refused by default, AFNI's in reproduce mode.
        assert!(parse_args(&["-T2", "-T2", "-T2", "in.nii"], FIX).is_err());
        let o = parse_args(&["-T2", "-T2", "-T2", "in.nii"], UpstreamDefects::Reproduce).unwrap();
        assert_eq!(o.options.t2, 3);
    }
}
