// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09 src/3dUnifize.c (main: the option scan, checks,
// messages and outputs), src/thd_dsetto3D.c (THD_extract_float_brick), src/thd_median.c
// (THD_median_brick), src/thd_dsetto1D.c (THD_extract_array) and the WROTE_DSET macro of
// src/mrilib.h.
// 3dUnifize.c, thd_dsetto3D.c and thd_median.c carry no Medical College of Wisconsin notice:
// they were written at the NIH and are in the public domain. thd_dsetto1D.c and mrilib.h:
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et
// al.), released under the GNU GPL version 2 or any later version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! `3dUnifize` with its original arguments: AFNI's `main()`, step by step (the work is in
//! [`crate::unifize`] and [`crate::mask`]).
//!
//! Messages go where AFNI sends them, all to standard error: the progress letters
//! (` + Pre-processing: IAVUWIm`, unless `-quiet`), AFNI's labelled messages, `++ Output dataset`
//! (also with `-quiet`, unless `AFNI_QUIET_STARTUP`) and `++ ===== Elapsed = ... sec`, with the
//! elapsed time of this run (AFNI 25.2.09 on Linux prints the elapsed time only: its CPU clock
//! reads 0 there).
//!
//! **AFNI's global options and environment variables** as for 3dTshift ([`crate::environ`],
//! [`crate::environ::UNIFIZE`]). The thread count comes from `OMP_NUM_THREADS`; the results do
//! not depend on it, as AFNI's do not.
//!
//! Differences from AFNI, shared with the 3dTshift replica (the larmorx conventions): input and
//! outputs are NIfTI only (`-prefix`, `-ssave` and `-amsave` must end in `.nii` or `.nii.gz`;
//! AFNI's default prefix `Unifized` is its own format, which larmorx-gpl refuses); AFNI's history
//! extension is not written, and neither the version banner nor the compile date AFNI prints
//! after a fatal error.
//!
//! **Upstream defects (D13)**, corrected unless `LARMORX_UPSTREAM_DEFECTS=reproduce`
//! (`docs/findings/afni-unifize.md`): those of the shared dataset and output code (AFNI-02, -11,
//! -12, -16, -18, -20, -22, -23, -26), AFNI-01 in `-EPI`'s median volume, `-T2up` at or near 100
//! (AFNI-45), the half-size grid of a volume with an axis of 2 or 3 voxels (AFNI-47); and two
//! crashes refused in both modes: `-T2up` without its value (AFNI-40), `-T2` on a 2D image
//! smaller than 16 x 16 (AFNI-46).

use std::io::Write;
use std::time::Instant;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notice, Notices, UpstreamDefects};

use crate::automask::{meets_afni_01, need_brick_factor, scale_to_float};
use crate::cnum::strtod;
use crate::dataset::{self, BrickData, Dataset};
use crate::environ::{self, Prefiltered, Settings, UNIFIZE};
use crate::glibc_floatmath::cbrtf;
use crate::mask::{Grid, MaskSettings, automask_image};
use crate::message::Message;
use crate::unifize::{
    Inversion, PKVAL, clip_edges, duplo_changes_grid, gm_unifize, invert_contrast, local_percmean,
    masked_input, percmean_args_ok, qmed, wm_unifize,
};

use super::automask::{dropped_intercept, one_brick, open_input, outputs};
use super::common::{Fail, Io, Output, echo_command_line, filename_ok, write_dataset};
use super::threads;

/// The first `-verb` line: which implementation runs (larmorx's addition, not AFNI's).
const IMPLEMENTATION: &str = concat!(
    "implementation: replica (larmorx-gpl ",
    env!("CARGO_PKG_VERSION"),
    ", GPL-3.0-or-later), bit-exact translation of AFNI 25.2.09's 3dUnifize"
);

const USAGE: &str = "\
Usage: 3dUnifize [options] inputdataset   (larmorx-gpl: replica of AFNI 25.2.09's 3dUnifize)

The input is a T1-weighted volume (or, with -T2, a T2-weighted one); the output has its white
matter intensity uniformized across space, scaled to about 1000, as AFNI's 3dUnifize computes
it, bit for bit. The output is float32. Only sub-brick #0 of the input is processed (except
with -EPI). The dataset and the outputs are NIfTI files.

Options:
  -prefix pp   the output, ending in .nii or .nii.gz (AFNI's default 'Unifized' is its own
               format, not written by larmorx-gpl)
  -input dd    the input dataset (instead of the last argument); -inset is the same
  -T2          treat the input as T2-weighted: invert its contrast, unifize, invert back;
               twice: do not invert back, clip bright edges instead
  -T2up uu     the percentile (90 to 100, default 98.5) the contrast is inverted about
  -GM          also scale the grey matter (lower intensities) to a standard contrast
  -Urad rr     the radius in voxels of the ball for the local white-matter intensity
               (5 to 99, default 18.3)
  -rbt R b t   the radius, and the bottom and top percentiles of the local histogram that
               are averaged (defaults 18.3, 70, 80); -param is the same
  -clfrac cc   the automask's clip level fraction, 0.1 to 0.9 (default 0.2); -mfrac is the
               same
  -ssave ss    save the white-matter scale factor of each voxel
  -amsave aa   save the automasked input
  -noduplo     do not compute on a half-size copy (only used above a million voxels);
               -nodouble is the same
  -EPI         a T2* EPI time series: the scaling of its median volume applied to every
               volume (implies -noduplo and -T2, turns off -GM)
  -nosquash    do not squash values above 1300; -oink is the same
  -quiet       no progress letters
  -verb        more messages
  -useall      accepted (disabled in AFNI 25.2.09)

AFNI's global options (anywhere on the command line):
  -overwrite      replace existing outputs (AFNI_DECONFLICT=OVERWRITE)
  -Dname=value    set AFNI's environment variable 'name' for this run
  -ok_1D_text     accepted (AFNI_1D_ZERO_TEXT=YES; no effect here)
  -skip_afnirc    accepted (no .afnirc file is read)
";

/// Runs `3dUnifize` with `args`; returns the exit status. The D13 mode comes from
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
    if args.is_empty() || args[0] == "-help" {
        let _ = out.write_all(USAGE.as_bytes());
        return 0;
    }
    let start = Instant::now();
    let pre = environ::prefilter_for(&UNIFIZE, args, env, mode);
    let mut io = Io::new(out, err, &pre.settings, mode);
    echo_command_line("3dUnifize", &pre, &mut io);
    for w in &pre.warnings {
        io.warn(w);
    }
    match run(&pre, mode, start, &mut io) {
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
    prefix: String,
    sspref: Option<String>,
    ampref: Option<String>,
    input: Option<Dataset>,
    input_name: Option<String>,
    do_double: bool,
    do_epi: bool,
    do_squash: bool,
    do_gm: u32,
    do_t2: u32,
    t2_uperc: f32,
    clfrac: f32,
    uprad: f32,
    upbot: f32,
    uptop: f32,
    verb: u32,
    refused_names: Vec<String>,
}

#[allow(clippy::too_many_lines)]
fn parse(
    args: &[String],
    mode: UpstreamDefects,
    settings: &Settings,
    notices: &mut Notices,
    io: &mut Io<'_>,
) -> Result<(Options, usize), Fail> {
    let mut o = Options {
        prefix: "Unifized".to_owned(),
        sspref: None,
        ampref: None,
        input: None,
        input_name: None,
        do_double: true,
        do_epi: false,
        do_squash: true,
        do_gm: 0,
        do_t2: 0,
        t2_uperc: 98.5,
        clfrac: 0.2,
        uprad: 18.3,
        upbot: 70.0,
        uptop: 80.0,
        verb: 1,
        refused_names: Vec::new(),
    };
    let n_threads = threads();
    let mut iarg = 0;
    // `if( ++iarg >= argc ) ERROR_exit("Need argument after '%s'",argv[iarg-1])`.
    let need = |iarg: &mut usize| -> Result<String, Fail> {
        *iarg += 1;
        args.get(*iarg)
            .cloned()
            .ok_or_else(|| Fail::Fatal(format!("Need argument after '{}'", args[*iarg - 1])))
    };
    let name_ok = |v: &str, option: &str, o: &mut Options, io: &mut Io<'_>| {
        if filename_ok(v, settings) {
            return Ok(());
        }
        if mode.reproduce() {
            io.notice(&Notice::new(
                &defects::AFNI_12,
                mode,
                format!(
                    "the {option} '{v}' has blanks, shell metacharacters or non-ASCII \
                     characters: refused, as AFNI does"
                ),
            ));
            return Err(Fail::Fatal(format!("Illegal value after {option}!")));
        }
        o.refused_names.push(format!("{option} '{v}'"));
        Ok(())
    };
    let ieq = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    while iarg < args.len() && args[iarg].starts_with('-') {
        let arg = args[iarg].as_str();
        if arg == "-nodouble" || arg == "-noduplo" {
            o.do_double = false;
        } else if ieq(arg, "-EPI") {
            o.do_epi = true;
        } else if arg == "-clfrac" || arg == "-mfrac" {
            o.clfrac = strtod(&need(&mut iarg)?).0 as f32;
            if !(0.1..=0.9).contains(&o.clfrac) {
                return Err(format!("-clfrac value {:.6} is illegal!", f64::from(o.clfrac)).into());
            }
        } else if arg == "-prefix" {
            let v = need(&mut iarg)?;
            name_ok(&v, "-prefix", &mut o, io)?;
            o.prefix = v;
        } else if arg == "-ssave" {
            let v = need(&mut iarg)?;
            name_ok(&v, "-ssave", &mut o, io)?;
            o.sspref = Some(v);
        } else if arg == "-amsave" {
            let v = need(&mut iarg)?;
            name_ok(&v, "-amsave", &mut o, io)?;
            o.ampref = Some(v);
        } else if arg == "-input" || arg == "-inset" {
            let v = need(&mut iarg)?;
            if o.input.is_some() {
                return Err(format!("Can't use '{arg}' twice").into());
            }
            o.input = Some(open_input(Some(&v), n_threads, settings, notices, io)?);
            o.input_name = Some(v);
        } else if arg == "-Urad" {
            o.uprad = strtod(&need(&mut iarg)?).0 as f32;
            if !(5.0..=99.0).contains(&o.uprad) {
                return Err(
                    format!("Illegal value {:.6} after option -Urad", f64::from(o.uprad)).into(),
                );
            }
        } else if arg == "-useall" {
            io.warn("-useall option is disabled in this version");
        } else if arg == "-param" || arg == "-rbt" {
            // `if( ++iarg >= argc-2 )` (argv counts the program): three values must follow.
            if iarg + 3 >= args.len() {
                return Err(format!("Need 3 arguments (R pb pt) after '{arg}'").into());
            }
            let v = |k: usize| strtod(&args[iarg + k]).0 as f32;
            o.uprad = v(1);
            o.upbot = v(2);
            o.uptop = v(3);
            iarg += 4;
            if o.uprad < 5.0
                || o.uprad > 99.0
                || o.upbot < 30.0
                || o.upbot > 80.0
                || o.uptop <= o.upbot
                || o.uptop > 90.0
            {
                return Err(format!("Illegal values (R pb pt) after '{arg}'").into());
            }
            continue;
        } else if ieq(arg, "-GM") {
            o.do_gm += 1;
        } else if ieq(arg, "-nosquash") || ieq(arg, "-oink") {
            o.do_squash = false;
        } else if ieq(arg, "-T2") {
            o.do_t2 += 1;
        } else if ieq(arg, "-T2up") {
            iarg += 1;
            let Some(v) = args.get(iarg) else {
                // AFNI reads argv[argc] (NULL) as a number and crashes.
                return Err(Fail::Fatal(
                    "-T2up needs a value: AFNI crashes (Fatal Signal 11) when an option's \
                     value is missing (AFNI-40)"
                        .into(),
                ));
            };
            o.t2_uperc = strtod(v).0 as f32;
            if !(90.0..=100.0).contains(&o.t2_uperc) {
                return Err("-T2up value is out of range 90..100 :-(".into());
            }
        } else if ieq(arg, "-quiet") {
            o.verb = 0;
        } else if ieq(arg, "-verb") {
            o.verb += 1;
        } else {
            return Err(format!("Unknown option: {arg}\n").into());
        }
        iarg += 1;
    }
    Ok((o, iarg))
}

/// `THD_extract_float_brick(iv, dset)`: the values of sub-brick `iv` as floats, non-finite
/// values 0, then times the brick factor (any factor but 0 and 1, negative included).
fn extract_float_brick(ds: &Dataset, iv: usize) -> Vec<f32> {
    let nvox = ds.nxyz.iter().product::<usize>();
    let mut v = scale_to_float(&ds.data, nvox, iv, 0.0);
    let fac = if ds.factors[iv] == 0.0 {
        1.0
    } else {
        ds.factors[iv]
    };
    if fac != 1.0 {
        for x in &mut v {
            *x *= fac;
        }
    }
    v
}

/// `THD_median_brick` of a dataset of several sub-bricks: at each voxel, the series read as
/// `THD_extract_array` reads it (non-finite values 0, then each positive factor applied: AFNI-01
/// reproduced; with `Fix`, any factor but 0) and its median (`qmed_float`).
fn median_brick(ds: &Dataset, mode: UpstreamDefects) -> Vec<f32> {
    let nvox = ds.nxyz.iter().product::<usize>();
    let scale = need_brick_factor(ds);
    let mut series = vec![0.0f32; ds.nvals];
    (0..nvox)
        .map(|ii| {
            for (iv, s) in series.iter_mut().enumerate() {
                let k = iv * nvox + ii;
                *s = match &ds.data {
                    BrickData::Byte(v) => f32::from(v[k]),
                    BrickData::Short(v) => f32::from(v[k]),
                    BrickData::Float(v) => v[k],
                };
            }
            dataset::floatscan(&mut series);
            if scale {
                for (s, &f) in series.iter_mut().zip(&ds.factors) {
                    if f > 0.0 || (mode.fix() && f != 0.0) {
                        *s *= f;
                    }
                }
            }
            qmed(&mut series)
        })
        .collect()
}

/// Prints the messages and letters collected so far.
fn flush(log: &mut Vec<Message>, io: &mut Io<'_>) {
    for m in log.drain(..) {
        io.message(m);
    }
}

/// `mri_invertcontrast_inplace`, with AFNI-45 (the percentile one past the sorted values):
/// corrected, the largest value; reproduced, 0 (what AFNI's memory usually holds there).
fn invert(im: &mut [f32], ny: usize, uperc: f32, mask: &[u8], notices: &mut Notices) {
    let fix = notices.fix();
    let past = |hist: &[f32]| if fix { hist[hist.len() - 1] } else { 0.0 };
    if let Inversion::PastTheEnd { ucut, nhist } = invert_contrast(im, ny, uperc, mask, &past) {
        let p = crate::cnum::fmt_g(f64::from(uperc));
        notices.add(
            &defects::AFNI_45,
            if fix {
                format!(
                    "-T2up {p} points one past the {nhist} sorted mask values: the contrast is \
                     inverted about the largest, {}; AFNI reads past them (usually 0, which \
                     zeroes the output)",
                    crate::cnum::fmt_g(f64::from(ucut))
                )
            } else {
                format!(
                    "-T2up {p} points one past the {nhist} sorted mask values: inverted about 0, \
                     where AFNI reads past them"
                )
            },
        );
    }
}

/// Writes one output; `WROTE_DSET`'s line when `announce` (and the file exists).
#[allow(clippy::too_many_arguments)]
fn write_out(
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

#[allow(clippy::too_many_lines)]
fn run(
    pre: &Prefiltered,
    mode: UpstreamDefects,
    start: Instant,
    io: &mut Io<'_>,
) -> Result<(), Fail> {
    let settings = &pre.settings;
    let quiet = settings.yes("AFNI_QUIET_STARTUP");
    let n_threads = threads();
    let mut notices = Notices::new(mode);
    notices.extend(pre.notices.iter().cloned());
    let (mut o, iarg) = parse(&pre.args, mode, settings, &mut notices, io)?;
    let verbose = o.verb > 0;
    if o.verb > 1 {
        io.info(IMPLEMENTATION);
        io.info(format!("upstream defects: {mode} ({})", defects::ENV));
    }
    for name in &o.refused_names {
        notices.add(
            &defects::AFNI_12,
            format!(
                "the {name} has blanks, shell metacharacters or non-ASCII characters: accepted; \
                 AFNI refuses it"
            ),
        );
    }
    let mut ds = match o.input.take() {
        Some(ds) => ds,
        None => {
            let Some(arg) = pre.args.get(iarg) else {
                return Err("No dataset name on command line?\n".into());
            };
            o.input_name = Some(arg.clone());
            open_input(Some(arg), n_threads, settings, &mut notices, io)?
        }
    };
    let outs = outputs(
        &[
            Some(o.prefix.as_str()),
            o.sspref.as_deref(),
            o.ampref.as_deref(),
        ],
        settings,
        pre.quiet_overwrite,
        &mut notices,
        io,
    )?;
    dropped_intercept(&mut ds, "3dUnifize", &mut notices);

    if o.do_epi {
        o.do_t2 = 1;
        o.do_double = false;
        if o.do_gm > 0 {
            io.info("-EPI turns off -GM");
            o.do_gm = 0;
        }
        let d = ds.geometry.axes.del;
        let uu = cbrtf((d[0] * d[1] * d[2]).abs());
        let mut uu = (18.3 / f64::from(uu)) as f32;
        if uu < 4.14 {
            uu = 4.14;
        }
        o.uprad = uu;
        io.info(format!(
            "-EPI changes -Urad to {:.1} voxels",
            f64::from(o.uprad)
        ));
    }
    let mut log: Vec<Message> = Vec::new();
    if verbose {
        io.raw(" + Pre-processing: ");
    }
    if !o.do_epi && ds.nvals > 1 {
        io.warn(format!(
            "Only processing sub-brick #0 (out of {})",
            ds.nvals
        ));
    } else if o.do_epi && ds.nvals == 1 {
        io.warn("-EPI was used, but only 1 volume in the input dataset");
        o.do_epi = false;
    }
    let [nx, ny, nz] = ds.nxyz;
    let g = Grid::new(nx, ny, nz);
    let mut imin = if !o.do_epi {
        extract_float_brick(&ds, 0)
    } else {
        if verbose {
            io.raw("M");
        }
        if meets_afni_01(&ds) {
            notices.add(
                &defects::AFNI_01,
                if mode.fix() {
                    "a negative scale factor is applied to the series whose median -EPI scales \
                     by; AFNI drops it there (and applies it to each volume), so its median has \
                     the wrong sign"
                } else {
                    "a negative scale factor is dropped from the series whose median -EPI \
                     scales by, as AFNI does: the median has the wrong sign"
                },
            );
        }
        median_brick(&ds, mode)
    };
    let mut ms = MaskSettings {
        exterior_clip: true,
        ..MaskSettings::default()
    };
    ms.set_clipfrac(o.clfrac);
    ms.verbose = o.verb > 1;

    // Invert a T2 contrast.
    let mut t2_mask: Option<Vec<u8>> = None;
    if o.do_t2 > 0 {
        if verbose {
            io.raw("I");
        }
        let m = automask_image(&imin, g, &ms, &mut log);
        flush(&mut log, io);
        let Some(m) = m else {
            // AFNI's crash banner starts on a new line; so does the refusal.
            if verbose {
                io.raw("\n");
            }
            return Err(
                "the automask of a 2D image smaller than 16 x 16 is empty (NULL): AFNI crashes \
                 inverting the contrast with it (AFNI-46)"
                    .into(),
            );
        };
        invert(&mut imin, ny, o.t2_uperc, &m, &mut notices);
        t2_mask = Some(m);
    }

    // The local white-matter scaling (mri_WMunifize).
    let do_double = o.do_double && imin.len() > 1_000_000;
    let expand_to = (do_double && duplo_changes_grid(g)).then(|| {
        notices.add(
            &defects::AFNI_47,
            if mode.fix() {
                format!(
                    "the {nx} x {ny} x {nz} grid, halved and doubled back, keeps an axis of 1: \
                     the scale image is expanded to the input's grid; AFNI reads its smaller one \
                     as if it were the input's, past its end"
                )
            } else {
                format!(
                    "the {nx} x {ny} x {nz} grid, halved and doubled back, keeps an axis of 1: \
                     its scale image is read as AFNI reads it, the values past its end 0"
                )
            },
        );
        g
    });
    let expand_to = if mode.fix() { expand_to } else { None };
    if !percmean_args_ok(o.uprad, o.upbot, o.uptop) {
        flush(&mut log, io);
        if verbose {
            io.raw("\n");
        }
        return Err("Can't compute Unifize-d dataset for some reason :-(".into());
    }
    let masked = masked_input(&imin, g, &ms, verbose, &mut log);
    let Some(aar) = masked else {
        // mri_WMunifize returned NULL: no scale image (-ssave writes nothing).
        flush(&mut log, io);
        if o.do_epi {
            return Err("Can't process EPI data: scale image missing :(".into());
        }
        if verbose {
            io.raw("\n");
        }
        return Err("Can't compute Unifize-d dataset for some reason :-(".into());
    };
    flush(&mut log, io);
    if let Some(out) = &outs[2] {
        let am = one_brick(&ds, BrickData::Float(aar.clone()));
        write_out(
            out,
            &am,
            false,
            quiet,
            n_threads,
            settings,
            &mut notices,
            io,
        )?;
    }
    let par = local_percmean(
        aar, g, o.uprad, o.upbot, o.uptop, do_double, expand_to, verbose, &mut log, n_threads,
    );
    let wm = wm_unifize(&imin, par, o.do_squash, verbose, &mut log);
    flush(&mut log, io);
    if let Some(out) = &outs[1] {
        let ss = one_brick(&ds, BrickData::Float(wm.scale.clone()));
        write_out(
            out,
            &ss,
            false,
            quiet,
            n_threads,
            settings,
            &mut notices,
            io,
        )?;
    }

    // -EPI: every volume scaled by the median volume's factors.
    if o.do_epi {
        let mask = t2_mask.as_deref().expect("-EPI implies -T2");
        let nvox = g.nvox();
        if verbose {
            io.raw("E");
        }
        let mut all = Vec::with_capacity(nvox * ds.nvals);
        for iv in 0..ds.nvals {
            if iv % 100 == 47 {
                io.raw(".");
            }
            let mut v = extract_float_brick(&ds, iv);
            invert(&mut v, ny, o.t2_uperc, mask, &mut notices);
            for (x, &s) in v.iter_mut().zip(&wm.scale) {
                *x *= s;
            }
            invert(&mut v, ny, o.t2_uperc, mask, &mut notices);
            all.extend(v);
        }
        if verbose {
            io.raw("\n");
        }
        let epi = Dataset {
            data: BrickData::Float(all),
            factors: vec![0.0; ds.nvals],
            dropped_inter: None,
            ..ds.clone()
        };
        if let Some(out) = &outs[0] {
            write_out(
                out,
                &epi,
                true,
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
        return Ok(());
    }

    let mut imout = wm.image;
    if o.do_gm > 0 {
        gm_unifize(&mut imout, verbose, &mut log);
    }
    if o.do_t2 == 1 {
        if verbose {
            log.push(Message::Raw("I".into()));
        }
        flush(&mut log, io);
        invert(
            &mut imout,
            ny,
            o.t2_uperc,
            t2_mask.as_deref().expect("-T2"),
            &mut notices,
        );
    } else if o.do_t2 == 2 {
        clip_edges(
            &mut imout,
            g,
            PKVAL * 1.111f32,
            PKVAL * 1.055f32,
            verbose,
            &mut log,
        );
    }
    // The final automask (08 Aug 2018).
    let mmm = automask_image(&imout, g, &ms, &mut log);
    flush(&mut log, io);
    let Some(mmm) = mmm else {
        return Err(
            "the automask of the unifized 2D image is empty (NULL): AFNI crashes applying it \
             (AFNI-46)"
                .into(),
        );
    };
    if verbose {
        io.raw("m");
    }
    for (v, &m) in imout.iter_mut().zip(&mmm) {
        if m == 0 {
            *v = 0.0;
        }
    }
    if verbose {
        io.raw("\n");
    }
    if let Some(out) = &outs[0] {
        let result = one_brick(&ds, BrickData::Float(imout));
        write_out(
            out,
            &result,
            true,
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
    if verbose {
        io.info(format!(
            "===== Elapsed = {:.1} sec\n",
            start.elapsed().as_secs_f64()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<(Options, usize), String> {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let settings = Settings::empty();
        let mut io = Io::new(&mut out, &mut err, &settings, UpstreamDefects::Fix);
        let mut notices = Notices::new(UpstreamDefects::Fix);
        parse(
            &args,
            UpstreamDefects::Fix,
            &settings,
            &mut notices,
            &mut io,
        )
        .map_err(|e| match e {
            Fail::Fatal(m) | Fail::Error(m) => m,
        })
    }

    #[test]
    fn option_rules() {
        let (o, i) =
            parse_args(&["-t2", "-T2", "-gm", "-EPI", "-OINK", "-quiet", "x.nii"]).unwrap();
        assert_eq!(
            (o.do_t2, o.do_gm, o.do_epi, o.do_squash, o.verb, i),
            (2, 1, true, false, 0, 6)
        );
        let (o, _) = parse_args(&["-rbt", "18.3", "65", "90", "x"]).unwrap();
        assert_eq!((o.uprad, o.upbot, o.uptop), (18.3, 65.0, 90.0));
        assert!(
            parse_args(&["-rbt", "18", "60"])
                .unwrap_err()
                .contains("Need 3 arguments")
        );
        assert!(
            parse_args(&["-rbt", "18", "60", "50", "x"])
                .unwrap_err()
                .contains("Illegal values")
        );
        assert!(
            parse_args(&["-Urad", "3", "x"])
                .unwrap_err()
                .contains("Illegal value 3.000000")
        );
        assert!(
            parse_args(&["-prefix"])
                .unwrap_err()
                .contains("Need argument after '-prefix'")
        );
        assert!(parse_args(&["-T2up"]).unwrap_err().contains("AFNI-40"));
        assert!(
            parse_args(&["-T2up", "89", "x"])
                .unwrap_err()
                .contains("out of range")
        );
        assert!(
            parse_args(&["-bogus"])
                .unwrap_err()
                .contains("Unknown option: -bogus")
        );
        let (o, _) = parse_args(&["-noduplo", "-verb", "-verb", "x"]).unwrap();
        assert!(!o.do_double && o.verb == 3);
    }
}
