// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09 src/3dAutomask.c (main: the option scan, checks,
// messages and outputs) and the AUTHOR, WROTE_DSET and CHECK_OPEN_ERROR macros of
// src/mrilib.h.
// 3dAutomask.c carries no Medical College of Wisconsin notice: it was written at the NIH and is
// in the public domain. mrilib.h: Major portions Copyright (C) 1994-2000 Medical College of
// Wisconsin (AFNI, Robert W. Cox et al.), released under the GNU GPL version 2 or any later
// version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! `3dAutomask` with its original arguments: AFNI's `main()`, step by step (the work is in
//! [`crate::automask`] and [`crate::mask`]).
//!
//! Messages go where AFNI sends them: everything to standard error, with AFNI's labels
//! ([`crate::message`]): `++ Authored by`, `++ Loading dataset`, the automask's ` + ` progress
//! lines, the counts, the empty planes and `++ Output dataset` for each output, unless `-q`.
//!
//! **AFNI's global options and environment variables** as for 3dTshift ([`crate::environ`],
//! [`crate::environ::AUTOMASK`]): `-overwrite`, `-Dname=value`, `-ok_1D_text`, `-skip_afnirc`;
//! `AFNI_QUIET_STARTUP` hides the author line and the `Output dataset` lines.
//!
//! Differences from AFNI, shared with the 3dTshift replica (the larmorx conventions):
//! - input and outputs are NIfTI only: `-prefix`, `-apply_prefix` and `-depth` must end in
//!   `.nii` or `.nii.gz` (AFNI writes its own format otherwise, and for its default prefix
//!   `automask`, which larmorx-gpl therefore refuses);
//! - AFNI's history extension is not written, and neither the version banner nor the compile
//!   date AFNI prints after a fatal error. `CPU time = 0.000000 sec` is printed as AFNI 25.2.09
//!   prints it on Linux (its CPU clock reads 0 there).
//!
//! **Upstream defects (D13)**, corrected unless `LARMORX_UPSTREAM_DEFECTS=reproduce`
//! (`docs/findings/afni-automask.md`; the notices `*+ WARNING: upstream defect ...`): those of
//! the shared dataset and output code (AFNI-02, -11, -12, -16, -18, -20, -22, -23, -26), AFNI-01
//! in the average of several sub-bricks ([`crate::automask::aveabs`]), `-peels 0` (AFNI-43),
//! `-nbhrs` outside 6..26 refused (AFNI-41, rule B: AFNI's range check never runs and the
//! value becomes 17), and an option given without its value as the last argument, where AFNI
//! crashes (AFNI-40): a refusal, in both modes.

use std::io::Write;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notice, Notices, UpstreamDefects};

use crate::automask::{self as am, aveabs, meets_afni_01};
use crate::cnum::strtod;
use crate::dataset::{BrickData, Dataset, ReadError};
use crate::environ::{self, AUTOMASK, Prefiltered, Settings};
use crate::mask::{self, Grid, MaskSettings};
use crate::message::Message;

use super::common::{
    Fail, Io, Output, echo_command_line, filename_ok, open_dataset, output_name_ok, output_path,
    write_dataset,
};
use super::threads;

const USAGE: &str = "\
Usage: 3dAutomask [options] dataset   (larmorx-gpl: replica of AFNI 25.2.09's 3dAutomask)

The input dataset is EPI 3D+time, or a skull-stripped anatomical; the output is a brain mask,
1 inside and 0 outside, as AFNI's 3dAutomask computes it, bit for bit. The dataset and the
outputs are NIfTI files.

Method: the clip level of 3dClipLevel (gradual across the octants unless -nograd), the
largest connected cluster, peeled and restored, holes filled.

Options (before the dataset):
  -prefix ppp       the mask, ending in .nii or .nii.gz (AFNI's default 'automask' is its
                    own format, not written by larmorx-gpl)
  -apply_prefix ppp the input with every voxel outside the mask set to 0 (alone, the only
                    output)
  -clfrac cc        the clip level fraction, 0.1 to 0.9 (default 0.5); -mfrac is the same
  -nograd           a fixed clip level instead of a gradual one
  -peels pp         peel (erode) the mask pp times, then unpeel (default 1)
  -NN1 -NN2 -NN3    neighbours for -erode, -dilate and -depth: faces, edges (default),
                    corners
  -nbhrs nn         neighbours (of 18) a voxel needs not to be peeled (default 17; 6 to 26,
                    another value is an error, AFNI-41)
  -q                no progress messages
  -eclip            remove exterior voxels below the clip level
  -dilate nd        dilate the mask outwards nd times
  -erode ne         erode the mask inwards ne times
  -SI hh            remove everything more than hh mm inferior to the most superior voxel
  -depth DEP        a dataset of how many peels it takes to reach each mask voxel

AFNI's global options (anywhere on the command line):
  -overwrite      replace existing outputs (AFNI_DECONFLICT=OVERWRITE)
  -Dname=value    set AFNI's environment variable 'name' for this run
  -ok_1D_text     accepted (AFNI_1D_ZERO_TEXT=YES; no effect here)
  -skip_afnirc    accepted (no .afnirc file is read)
";

/// Runs `3dAutomask` with `args`; returns the exit status. The D13 mode comes from
/// `LARMORX_UPSTREAM_DEFECTS`, AFNI's environment from the process environment.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match UpstreamDefects::from_env() {
        Ok(mode) => main_with(args, mode, Settings::from_env(), out, err),
        Err(e) => {
            let _ = writeln!(err, "** FATAL ERROR: {e}");
            EXIT_BAD_MODE
        }
    }
}

/// [`main`] with the D13 mode and AFNI's environment (`env`) given.
pub fn main_with(
    args: &[String],
    mode: UpstreamDefects,
    env: Settings,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    // `3dAutomask` alone, or with -help first, prints the help before anything else happens.
    if args.is_empty() || args[0] == "-help" {
        let _ = out.write_all(USAGE.as_bytes());
        return 0;
    }
    let pre = environ::prefilter_for(&AUTOMASK, args, env, mode);
    let mut io = Io::new(out, err, &pre.settings, mode);
    echo_command_line("3dAutomask", &pre, &mut io);
    for w in &pre.warnings {
        io.warn(w);
    }
    let quiet = pre.settings.yes("AFNI_QUIET_STARTUP");
    if !quiet {
        io.info("Authored by: Emperor Zhark");
    }
    match run(&pre, mode, quiet, &mut io) {
        Ok(()) => 0,
        Err(Fail::Fatal(message)) => {
            io.message(Message::Fatal(message));
            1
        }
        Err(Fail::Error(message)) => {
            io.message(Message::Error(message));
            1
        }
    }
}

/// The command line after the option scan.
#[derive(Debug)]
struct Options {
    prefix: Option<String>,
    apply_prefix: Option<String>,
    depth_prefix: Option<String>,
    dilate: i32,
    erode: i32,
    dilate_flag: bool,
    erode_flag: bool,
    si: f32,
    verbose: bool,
    peels: i32,
    nbhrs: i32,
    nn: u8,
    settings: MaskSettings,
    dataset: Option<String>,
    /// The names AFNI would refuse (`THD_filename_ok`), accepted by larmorx-gpl (AFNI-12).
    refused_names: Vec<String>,
    /// The last `-nbhrs` value given (its text and `(int) strtod`), whatever `-NNx` does.
    given_nbhrs: Option<(String, i32)>,
}

/// `(int) strtod(s, NULL)`: truncation toward zero (saturating where C is undefined).
fn strtod_int(s: &str) -> i32 {
    strtod(s).0 as i32
}

/// `strtol(s, NULL, 10)` assigned to an `int`: leading blanks, a sign, decimal digits (0 when
/// there are none); the `long` saturates, then the conversion keeps the low 32 bits, as GCC
/// does.
fn strtol_int(s: &str) -> i32 {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && crate::cnum::is_space(b[i]) {
        i += 1;
    }
    let mut neg = false;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        neg = b[i] == b'-';
        i += 1;
    }
    let mut v: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        v = v.saturating_mul(10).saturating_add(i64::from(b[i] - b'0'));
        i += 1;
    }
    let v = if neg { v.saturating_neg() } else { v };
    v as i32
}

fn parse(
    args: &[String],
    mode: UpstreamDefects,
    settings: &Settings,
    io: &mut Io<'_>,
) -> Result<Options, Fail> {
    let mut o = Options {
        prefix: None,
        apply_prefix: None,
        depth_prefix: None,
        dilate: 0,
        erode: 0,
        dilate_flag: false,
        erode_flag: false,
        si: 0.0,
        verbose: true,
        peels: 1,
        nbhrs: 17,
        nn: 2,
        settings: MaskSettings::default(),
        dataset: None,
        refused_names: Vec::new(),
        given_nbhrs: None,
    };
    let mut new_nn = 0u8;
    let mut iarg = 0;
    // The value after an option. AFNI reads `argv[++iarg]` without a check: past the last
    // argument it crashes (AFNI-40), except where it hands the NULL to THD_filename_ok, which
    // refuses it ("-prefix (null) is illegal!").
    let value = |iarg: &mut usize, name: &str, nullable: bool| -> Result<String, Fail> {
        *iarg += 1;
        match args.get(*iarg) {
            Some(v) => Ok(v.clone()),
            None if nullable => Err(Fail::Fatal(format!("{name} (null) is illegal!\n"))),
            None => {
                // AFNI's crash banner is not reproduced: a refusal, in both modes.
                Err(Fail::Fatal(format!(
                    "{name} needs a value: AFNI crashes (Fatal Signal 11) when an option's value \
                     is missing (AFNI-40)"
                )))
            }
        }
    };
    let mut check_name = |v: &str, name: &str, o: &mut Options| -> Result<(), Fail> {
        if filename_ok(v, settings) {
            return Ok(());
        }
        if mode.reproduce() {
            io.notice(&Notice::new(
                &defects::AFNI_12,
                mode,
                format!(
                    "the {name} '{v}' has blanks, shell metacharacters or non-ASCII characters: \
                     refused, as AFNI does"
                ),
            ));
            return Err(Fail::Fatal(format!("{name} {v} is illegal!\n")));
        }
        o.refused_names.push(format!("{name} '{v}'"));
        Ok(())
    };
    while iarg < args.len() && args[iarg].starts_with('-') {
        let arg = args[iarg].as_str();
        let b = arg.as_bytes();
        let n5 = |name: &str| b.len() >= 5 && b[..5] == name.as_bytes()[..5];
        if n5("-peel") {
            o.peels = strtod_int(&value(&mut iarg, "-peels", false)?);
            iarg += 1;
            continue;
        }
        match arg {
            "-NN1" => new_nn = 1,
            "-NN2" => new_nn = 2,
            "-NN3" => new_nn = 3,
            _ => {}
        }
        if matches!(arg, "-NN1" | "-NN2" | "-NN3") {
            iarg += 1;
            continue;
        }
        if n5("-nbhr") {
            let text = value(&mut iarg, "-nbhrs", false)?;
            o.nbhrs = strtod_int(&text);
            o.given_nbhrs = Some((text, o.nbhrs));
            iarg += 1;
            continue;
        }
        // `strncmp(argv[iarg],"-nograd",5)`: "-nogr" and anything after it.
        if n5("-nogr") {
            o.settings.gradualize = false;
            iarg += 1;
            continue;
        }
        match arg {
            "-clfrac" | "-mfrac" => {
                let clfrac = strtod(&value(&mut iarg, arg, false)?).0 as f32;
                if !(0.1..=0.9).contains(&clfrac) {
                    return Err(
                        format!("-clfrac value {:.6} is illegal!", f64::from(clfrac)).into(),
                    );
                }
                o.settings.set_clipfrac(clfrac);
            }
            "-SI" => {
                o.si = strtod(&value(&mut iarg, "-SI", false)?).0 as f32;
                if o.si <= 0.0 {
                    return Err(format!("-SI value {:.6} is illegal!\n", f64::from(o.si)).into());
                }
            }
            "-eclip" => o.settings.exterior_clip = true,
            "-depth" => {
                let v = value(&mut iarg, "-depth", true)?;
                check_name(&v, "-depth", &mut o)?;
                o.depth_prefix = Some(v);
            }
            "-q" => o.verbose = false,
            "-prefix" => {
                let v = value(&mut iarg, "-prefix", true)?;
                check_name(&v, "-prefix", &mut o)?;
                o.prefix = Some(v);
            }
            "-dilate" => {
                let v = value(&mut iarg, "-dilate", false)?;
                o.dilate = strtol_int(&v);
                o.dilate_flag = true;
                if o.dilate < 0 {
                    return Err(format!("-dilate {v} is illegal!\n").into());
                }
            }
            "-erode" => {
                let v = value(&mut iarg, "-erode", false)?;
                o.erode = strtol_int(&v);
                o.erode_flag = true;
                if o.erode < 0 {
                    return Err(format!("-erode {v} is illegal!\n").into());
                }
            }
            "-apply_prefix" => {
                let v = value(&mut iarg, "-apply_prefix", true)?;
                check_name(&v, "-apply_prefix", &mut o)?;
                o.apply_prefix = Some(v);
            }
            _ => return Err(format!("ILLEGAL option: {arg}\n").into()),
        }
        iarg += 1;
    }
    // Nearest-neighbour modes: -NNx sets the neighbourhood and the peeling count (AFNI-42).
    if new_nn != 0 {
        o.nn = new_nn;
    }
    let max_nbhrs = match o.nn {
        1 => 6,
        3 => 26,
        _ => 18,
    };
    if new_nn != 0 {
        o.nbhrs = max_nbhrs;
    }
    // AFNI-41: AFNI's check of -nbhrs reads `new_nbhrs`, which nothing sets, so it never runs;
    // `THD_automask_set_peelcounts` then replaces a value outside 6..26 (the help's range) by
    // 17. Corrected (rule B, decided 2026-10-23): a value outside 6..26 is an error, whether
    // or not -NN1/2/3 override it; `reproduce` keeps AFNI's behaviour, named in `run`.
    if let Some((text, v)) = &o.given_nbhrs
        && !(6..=26).contains(v)
        && mode.fix()
    {
        let used = if o.nbhrs == *v { 17 } else { o.nbhrs.min(18) };
        io.notice(&Notice::new(
            &defects::AFNI_41,
            mode,
            format!(
                "-nbhrs {text} is outside 6..26 (the range the help gives): an error (exit status \
                 1); AFNI silently peels as with {used}"
            ),
        ));
        return Err(Fail::Fatal(format!(
            "-nbhrs {text} is illegal: choose a value between 6 and 26 (AFNI-41; \
             LARMORX_UPSTREAM_DEFECTS=reproduce gives AFNI's result)"
        )));
    }
    o.settings.set_peelcounts(o.peels, o.nbhrs);
    if o.dilate_flag && o.erode_flag {
        io.warn("Combining dilate,erode options is like peels option");
    }
    o.dataset = args.get(iarg).cloned();
    Ok(o)
}

/// Writes one output and prints `WROTE_DSET`'s line when asked (and the file exists).
#[allow(clippy::too_many_arguments)]
fn write_one(
    out: &Output,
    ds: &Dataset,
    announce: bool,
    quiet: bool,
    n_threads: usize,
    settings: &Settings,
    notices: &mut Notices,
    io: &mut Io<'_>,
) -> Result<(), Fail> {
    let w = write_dataset(out, ds, n_threads, settings, notices, io)?;
    if announce && !quiet && w.path.is_file() {
        io.info(format!("Output dataset {}", w.shown));
    }
    Ok(())
}

/// A dataset of one sub-brick on `ds`'s grid, without a time axis (`EDIT_empty_copy` with
/// `ADN_nvals 1`, `ADN_ntt 0`, a new datum).
pub(crate) fn one_brick(ds: &Dataset, data: BrickData) -> Dataset {
    Dataset {
        nxyz: ds.nxyz,
        nvals: 1,
        data,
        factors: vec![0.0],
        taxis: None,
        geometry: ds.geometry.clone(),
        seq_dec: None,
        dropped_inter: None,
    }
}

/// AFNI-23 (`AFNI_NIFTI_SCALE=NO`): by default the data are read as without the variable
/// (float32, `slope * x + inter`), so the mask, the masked copy and the unifized image see the
/// right values; `reproduce` keeps AFNI's integers without the intercept. Named in both modes.
pub(crate) fn dropped_intercept(ds: &mut Dataset, program: &str, notices: &mut Notices) {
    let Some(d) = ds.dropped_inter else {
        return;
    };
    let (ty, slope, inter) = (
        d.type_name(),
        crate::cnum::fmt_g(d.slope),
        crate::cnum::fmt_g(d.inter),
    );
    if notices.fix() {
        notices.add(
            &defects::AFNI_23,
            format!(
                "AFNI_NIFTI_SCALE=NO: the {ty} data are read as float32 with scl_slope {slope} \
                 and scl_inter {inter} applied, as without the variable; AFNI keeps them as {ty} \
                 and drops the intercept, so {program} works on values off by {inter}"
            ),
        );
        ds.data = BrickData::Float(d.scaled(&ds.data));
        ds.factors.iter_mut().for_each(|f| *f = 0.0);
        ds.dropped_inter = None;
    } else {
        notices.add(
            &defects::AFNI_23,
            format!(
                "AFNI_NIFTI_SCALE=NO: the {ty} data are kept as stored and scl_inter {inter} is \
                 dropped, as AFNI does: {program} works on values off by {inter}"
            ),
        );
    }
}

/// `THD_open_dataset` and `CHECK_OPEN_ERROR` for the input (`'(null)'` when there is none),
/// with the messages AFNI prints while opening it.
pub(crate) fn open_input(
    arg: Option<&str>,
    n_threads: usize,
    settings: &Settings,
    notices: &mut Notices,
    io: &mut Io<'_>,
) -> Result<Dataset, Fail> {
    let Some(arg) = arg else {
        return Err("Can't open dataset '(null)'".into());
    };
    let mut messages = Vec::new();
    let read = open_dataset(arg, n_threads, settings, &mut messages, notices, true);
    for m in messages {
        io.message(m);
    }
    read.map_err(|e| match (notices.fix(), &e) {
        (false, ReadError::Nifti(_) | ReadError::Afni(_)) => {
            Fail::Fatal(format!("Can't open dataset '{arg}'"))
        }
        _ => Fail::Fatal(format!("Can't open dataset '{arg}': {e}")),
    })
}

/// The outputs named on the command line (larmorx's NIfTI-only convention, then AFNI-11's and
/// AFNI-12's checks before the work).
pub(crate) fn outputs(
    names: &[Option<&str>],
    settings: &Settings,
    quiet_overwrite: bool,
    notices: &mut Notices,
    io: &mut Io<'_>,
) -> Result<Vec<Option<Output>>, Fail> {
    let mut outs = Vec::new();
    for name in names {
        let Some(name) = name else {
            outs.push(None);
            continue;
        };
        output_path(name)?;
        if notices.fix() && filename_ok(name, settings) && !output_name_ok(name, settings) {
            notices.add(
                &defects::AFNI_12,
                format!(
                    "the output name '{}' has blanks, shell metacharacters or non-ASCII \
                     characters, or starts with '-': written; AFNI accepts it but writes nothing \
                     (exit status 0)",
                    super::common::afni_output_name(name)
                ),
            );
        }
        let out = Output::new(name, settings, quiet_overwrite);
        out.check(false, notices, io)?;
        outs.push(Some(out));
    }
    Ok(outs)
}

fn run(pre: &Prefiltered, mode: UpstreamDefects, quiet: bool, io: &mut Io<'_>) -> Result<(), Fail> {
    let settings = &pre.settings;
    let mut o = parse(&pre.args, mode, settings, io)?;
    let n_threads = threads();
    let mut notices = Notices::new(mode);
    notices.extend(pre.notices.iter().cloned());
    for name in &o.refused_names {
        notices.add(
            &defects::AFNI_12,
            format!(
                "the {name} has blanks, shell metacharacters or non-ASCII characters: accepted; \
                 AFNI refuses it"
            ),
        );
    }
    if let Some((text, v)) = &o.given_nbhrs
        && !(6..=26).contains(v)
    {
        // Only `reproduce` gets here (the default refused it in `parse`): AFNI's peel count
        // is 17 for the value itself, min(18, n) for -NNx's.
        let used = if o.nbhrs == *v { 17 } else { o.nbhrs.min(18) };
        notices.add(
            &defects::AFNI_41,
            format!(
                "-nbhrs {text} is outside 6..26 (the range the help gives): the peeling acts as \
                 with {used}, as in AFNI; by default it is an error"
            ),
        );
    }
    // The outputs: the mask unless only -apply_prefix was given (AFNI's default 'automask' is
    // its own format: larmorx-gpl refuses it).
    let prefix = match (&o.prefix, &o.apply_prefix) {
        (Some(p), _) => Some(p.clone()),
        (None, None) => Some("automask".to_owned()),
        (None, Some(_)) => None,
    };
    let outs = outputs(
        &[
            prefix.as_deref(),
            o.apply_prefix.as_deref(),
            o.depth_prefix.as_deref(),
        ],
        settings,
        pre.quiet_overwrite,
        &mut notices,
        io,
    )?;

    let mut ds = open_input(o.dataset.as_deref(), n_threads, settings, &mut notices, io)?;
    if o.verbose {
        io.info(format!(
            "Loading dataset {}\n",
            o.dataset.as_deref().unwrap_or("(null)")
        ));
    }
    dropped_intercept(&mut ds, "3dAutomask", &mut notices);
    let [nx, ny, nz] = ds.nxyz;
    let g = Grid::new(nx, ny, nz);

    // The real work.
    if o.verbose {
        io.info("Forming automask\n");
    }
    o.settings.verbose = o.verbose;
    // AFNI-43: -peels 0 peels once in AFNI (THD_automask_set_peelcounts makes it 1); its help
    // says 0 turns the peeling off, which the default does.
    let peels0 = o.peels == 0;
    if peels0 && mode.fix() {
        o.settings.peelcount = 0;
    }
    let mut log = Vec::new();
    let mask = am::automask(&ds, &o.settings, mode, &mut log);
    let printed = log.clone();
    for m in log {
        io.message(m);
    }
    let mut mask = mask.ok_or(Fail::Fatal(
        "Mask creation fails for unknown reasons!\n".into(),
    ))?;
    if peels0 {
        let s = MaskSettings {
            peelcount: if mode.fix() { 1 } else { 0 },
            ..o.settings.clone()
        };
        let mut other_log = Vec::new();
        let other = am::automask(&ds, &s, mode, &mut other_log);
        // The notice where the mask differs, or only AFNI's messages (the clusters it counts
        // before and after peeling; random testing, 2026-10-25).
        if other.as_deref() != Some(&mask[..]) {
            notices.add(
                &defects::AFNI_43,
                if mode.fix() {
                    "-peels 0: the mask is not peeled, as the help says; AFNI peels it once"
                } else {
                    "-peels 0: the mask is peeled once, as AFNI does (its help says 0 turns the \
                     peeling off)"
                },
            );
        } else if other_log != printed {
            notices.add(
                &defects::AFNI_43,
                if mode.fix() {
                    "-peels 0: the mask is not peeled, as the help says (AFNI peels it once): the \
                     same mask, but the clustering messages count other voxels"
                } else {
                    "-peels 0: the mask is peeled once, as AFNI does: the same mask as without \
                     peeling, but the clustering messages count other voxels"
                },
            );
        }
    }
    if meets_afni_01(&ds) {
        let other = if mode.fix() {
            UpstreamDefects::Reproduce
        } else {
            UpstreamDefects::Fix
        };
        let s = MaskSettings {
            verbose: false,
            ..o.settings.clone()
        };
        // The averaged values differ by the factor's magnitude, so the clip levels AFNI
        // prints do; the mask, which scales with the data, seldom does.
        let medim = aveabs(&ds, other);
        let other_mask = mask::automask_image(&medim, g, &s, &mut Vec::new());
        let n = other_mask.map_or(mask.len(), |m| {
            m.iter().zip(&mask).filter(|(a, b)| a != b).count()
        });
        notices.add(
            &defects::AFNI_01,
            if mode.fix() {
                format!(
                    "a negative scale factor is applied when the sub-bricks are averaged, where \
                     AFNI drops it (and its magnitude): the mask differs from AFNI's in {n} \
                     voxels"
                )
            } else {
                format!(
                    "a negative scale factor is dropped when the sub-bricks are averaged, as AFNI \
                     does: the mask differs in {n} voxels from the one with the factor"
                )
            },
        );
    }

    let depth = if o.depth_prefix.is_some() {
        let mut log = Vec::new();
        let (d, stuck) = mask::depth(g, &mask, o.nn, &o.settings, &mut log);
        for m in log {
            io.message(m);
        }
        if let Some(mask::DepthNote::Stuck {
            iterations,
            remaining,
        }) = stuck
        {
            io.warn(format!(
                "Nothing left to peel, after {iterations} iterations.\n however {remaining} \
                 voxels remain in cpmask!\n Jumping ship.\n"
            ));
        }
        Some(d)
    } else {
        None
    };

    let mut log = Vec::new();
    am::dilate_erode(
        g,
        &mut mask,
        o.dilate,
        o.erode,
        o.nn,
        o.verbose,
        &o.settings,
        &mut log,
    );
    for m in log {
        io.message(m);
    }

    let nvox = g.nvox();
    let nmask = mask::count(&mask);
    if o.verbose {
        io.info(format!(
            "{nmask} voxels in the mask [out of {nvox}: {:.2}%]\n",
            (100.0 * f64::from(nmask)) / nvox as f64
        ));
    }
    if nmask == 0 {
        return Err("No voxels? Quitting without saving mask\n".into());
    }
    let mut log = Vec::new();
    am::planes_and_si(&ds, &mut mask, o.si, o.verbose, &mut log);
    for m in log {
        io.message(m);
    }

    // The outputs.
    if let Some(out) = &outs[0] {
        let mset = one_brick(&ds, BrickData::Byte(mask.clone()));
        write_one(
            out,
            &mset,
            o.verbose,
            quiet,
            n_threads,
            settings,
            &mut notices,
            io,
        )?;
    }
    if let Some(out) = &outs[1] {
        if o.verbose {
            io.info("applying mask to original data\n");
        }
        let masked = am::apply_mask(&ds, &mask);
        if o.verbose {
            io.info("Writing masked data\n");
        }
        write_one(
            out,
            &masked,
            o.verbose,
            quiet,
            n_threads,
            settings,
            &mut notices,
            io,
        )?;
    }
    if let (Some(out), Some(d)) = (&outs[2], depth) {
        if o.verbose {
            io.info("Writing depth dataset\n");
        }
        let dset = one_brick(&ds, BrickData::Short(d));
        write_one(
            out,
            &dset,
            o.verbose,
            quiet,
            n_threads,
            settings,
            &mut notices,
            io,
        )?;
    }
    for n in notices.as_slice() {
        io.notice(n);
    }
    if o.verbose {
        io.info("CPU time = 0.000000 sec\n");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Options, String> {
        parse_in(args, UpstreamDefects::Fix).0
    }

    /// `parse` in `mode`, with what it printed on standard error.
    fn parse_in(args: &[&str], mode: UpstreamDefects) -> (Result<Options, String>, String) {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let settings = Settings::empty();
        let mut io = Io::new(&mut out, &mut err, &settings, mode);
        let r = parse(&args, mode, &settings, &mut io).map_err(|e| match e {
            Fail::Fatal(m) | Fail::Error(m) => m,
        });
        drop(io);
        (r, String::from_utf8(err).unwrap())
    }

    #[test]
    fn option_rules() {
        let o = parse_args(&["-q", "-peels", "2.9", "-nbhrs", "15", "x.nii"]).unwrap();
        assert!(!o.verbose);
        assert_eq!((o.settings.peelcount, o.settings.peelthr), (2, 15));
        assert_eq!(o.dataset.as_deref(), Some("x.nii"));
        // -NNx sets the peeling threshold to its neighbour count.
        let o = parse_args(&["-nbhrs", "15", "-NN1", "x.nii"]).unwrap();
        assert_eq!((o.nn, o.settings.peelthr), (1, 6));
        let o = parse_args(&["-NN2", "x.nii"]).unwrap();
        assert_eq!(o.settings.peelthr, 18);
        // AFNI-41: out of range, AFNI's 17 silently; corrected, an error naming the entry.
        let reproduce = UpstreamDefects::Reproduce;
        for v in ["40", "5", "0", "abc", "27"] {
            let (o, _) = parse_in(&["-nbhrs", v, "x"], reproduce);
            assert_eq!(o.unwrap().settings.peelthr, 17, "{v}");
            let (e, err) = parse_in(&["-nbhrs", v, "x"], UpstreamDefects::Fix);
            assert!(e.unwrap_err().contains("AFNI-41"), "{v}");
            assert!(
                err.contains("upstream defect AFNI-41 (3dAutomask)"),
                "{err}"
            );
        }
        // The help's range is accepted (19 to 26 peel as 18), and -NN1 does not excuse 40.
        for v in ["6", "18", "26", "6.9"] {
            assert!(parse_args(&["-nbhrs", v, "x"]).is_ok(), "{v}");
        }
        assert!(parse_args(&["-nbhrs", "40", "-NN1", "x"]).is_err());
        let (o, _) = parse_in(&["-nbhrs", "40", "-NN1", "x"], reproduce);
        assert_eq!(o.unwrap().settings.peelthr, 6);
        assert!(
            !parse_args(&["-nogradual", "x"])
                .unwrap()
                .settings
                .gradualize
        );
        assert!(
            parse_args(&["-clfrac", "0.95", "x"])
                .unwrap_err()
                .contains("0.950000 is illegal")
        );
        assert!(parse_args(&["-SI", "0", "x"]).is_err());
        assert_eq!(parse_args(&["-dilate", "2x", "x"]).unwrap().dilate, 2);
        assert!(
            parse_args(&["-dilate", "-1", "x"])
                .unwrap_err()
                .contains("-dilate -1 is illegal")
        );
        assert!(
            parse_args(&["-prefix"])
                .unwrap_err()
                .contains("-prefix (null) is illegal!")
        );
        assert!(parse_args(&["-peels"]).unwrap_err().contains("AFNI-40"));
        assert!(
            parse_args(&["-bogus", "x"])
                .unwrap_err()
                .contains("ILLEGAL option: -bogus")
        );
        assert_eq!(parse_args(&["-q"]).unwrap().dataset, None);
        assert_eq!(strtol_int(" -12abc"), -12);
        assert_eq!(strtol_int("abc"), 0);
    }
}
