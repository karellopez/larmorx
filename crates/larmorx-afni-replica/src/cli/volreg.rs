// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05 src/3dvolreg.c (main and VL_command_line: the option scan,
// the checks, the messages, the parameter files and the -rotcom fragments), src/thd_notes.c
// (tross_commandline), src/thd_opendset.c (modify_afni_prefix with a suffix) and
// src/thd_mismatch.c (THD_dataset_mismatch's dimension, spacing and orientation tests). Major
// portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released by AFNI 26.3.05 under the Creative Commons Attribution 4.0 International licence (CC BY
// 4.0). thd_mismatch.c carries no such notice: it was written at the NIH and is in the public
// domain.
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05.
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! `3dvolreg` with its original arguments: the option scan, checks, messages and outputs of
//! AFNI's `main()` and `VL_command_line()`, step by step, around the registration of
//! [`crate::volreg`].
//!
//! Options are matched as AFNI matches them: some exactly, others by a prefix of 4 to 7
//! letters (`-maxite` by `-max`, `-final` by `-fin`, `-noclip` by `-noc`, ...), in AFNI's order,
//! so `-nocoarserot` is `-noclip` (AFNI-86, reproduced with `reproduce`; by default it does
//! what its help says). Messages go where AFNI sends them: the `-rotcom` fragments to standard
//! output, everything else to standard error.
//!
//! Differences from AFNI, shared with every larmorx AFNI tool: input and output are NIfTI
//! only, and `-prefix` must end in `.nii` or `.nii.gz` (AFNI's default prefix `volreg` writes
//! its own format); no AFNI header extension is written (so no `VOLREG_*` attributes or
//! history); no version banner; the CPU times of `-verbose` are not measured (printed as 0).
//! Not yet supported (relay 1): `-tshift`, `-savedisp`/`-SAVEDISP`, `-rotparent`,
//! `-gridparent`, `-twodup` with an external base, sub-brick selectors, `-@`.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notices, UpstreamDefects};

use crate::cnum::{fmt_g, strtod};
use crate::dataset::{Dataset, ReadError};
use crate::environ::{self, Prefiltered, Settings, VOLREG};
use crate::message::Message;
use crate::shift::Method;
use crate::volreg::{self, Base, Edging, VolregError, VolregParams, VolregResult, mode_name};

use super::common::{
    Io, Output, afni_output_name, echo_command_line, filename_ok, open_dataset, output_path,
    write_dataset,
};
use super::threads;

/// The first `-verbose` line: which implementation runs (larmorx's addition, not AFNI's).
const IMPLEMENTATION: &str = concat!(
    "implementation: replica (larmorx ",
    env!("CARGO_PKG_VERSION"),
    ", Apache-2.0 AND CC-BY-4.0), bit-exact translation of AFNI 26.3.05's 3dvolreg"
);

const USAGE: &str = "\
Usage: 3dvolreg [options] dataset   (larmorx: replica of AFNI 26.3.05's 3dvolreg)

Registers each 3D sub-brick of the input dataset to the base brick (rigid body, 6
parameters), with AFNI's iterated linearised weighted least squares. The results are those
of AFNI's 3dvolreg, bit for bit. The dataset and the output are NIfTI files.

Options (before the dataset):
  -verbose             print progress reports (twice: lots of output)
  -Fourier -heptic -quintic -cubic -linear
                       interpolation for the estimation (default -heptic)
  -final mode          final interpolation: NN cubic quintic heptic Fourier linear
  -clipit / -noclip    clip each output to its input's range (default on)
  -zpad n              zero padding around the edges during rotations (default 4)
  -prefix fname        output (.nii or .nii.gz); NULL writes no dataset
  -float               write the output as float
  -base n | -base dset the n'th sub-brick of the input, or sub-brick 0 of another dataset
  -dfile f             save n, roll, pitch, yaw, dS, dL, dP, rmsold, rmsnew
  -1Dfile f            save roll, pitch, yaw, dS, dL, dP
  -1Dmatrix_save ff    save the base-to-input matrices (ff, or ff.aff12.1D)
  -rotcom              print the 3drotate fragments
  -maxdisp / -nomaxdisp  report the maximum displacement in the automask (default on)
  -maxdisp1D mm        also write it per sub-brick to mm (and its change to mm_delt)
  -maxite m  -x_thresh x  -rot_thresh r  -delta d  -params m x r d
  -weight 'wset[n]'    the weight brick
  -wtinp               sub-brick 0 of the input as the weight
  -edging ee | ee%     the weight's zeroed edge (default 5%, or AFNI_VOLREG_EDGING)
  -twopass  -twoblur bb  -sinit  -coarse del num  -coarserot  -nocoarserot
  -input dset / -source dset   the dataset, given as an option

AFNI's global options (anywhere on the command line):
  -overwrite      replace an existing output (AFNI_DECONFLICT=OVERWRITE)
  -Dname=value    set AFNI's environment variable 'name' for this run
  -skip_afnirc    accepted (no .afnirc file is read)
";

/// How the run stops: `ERROR_exit` (`** FATAL ERROR: ...`), or AFNI's own `fprintf(stderr, ...)`
/// and `exit(1)` (the text as it is, newline included).
enum Stop {
    Fatal(String),
    Raw(String),
}

impl From<String> for Stop {
    fn from(m: String) -> Stop {
        Stop::Fatal(m)
    }
}

impl From<super::common::Fail> for Stop {
    fn from(f: super::common::Fail) -> Stop {
        match f {
            super::common::Fail::Fatal(m) => Stop::Fatal(m),
            super::common::Fail::Error(m) => Stop::Raw(format!("** ERROR: {m}\n")),
        }
    }
}

/// Runs `3dvolreg` with `args`; returns the exit status. The D13 mode comes from
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
    // `3dvolreg` alone, or an argument starting with -help first, prints the help.
    if args.is_empty() || args[0].starts_with("-help") {
        let _ = out.write_all(USAGE.as_bytes());
        return 0;
    }
    let pre = environ::prefilter_for(&VOLREG, args, env, mode);
    let mut io = Io::new(out, err, &pre.settings, mode);
    echo_command_line("3dvolreg", &pre, &mut io);
    for w in &pre.warnings {
        io.warn(w);
    }
    if !pre.settings.yes("AFNI_QUIET_STARTUP") {
        io.info("Authored by: RW Cox");
    }
    match run(&pre, mode, &mut io) {
        Ok(()) => 0,
        Err(Stop::Fatal(message)) => {
            io.message(Message::Fatal(message));
            1
        }
        Err(Stop::Raw(text)) => {
            io.raw(text);
            1
        }
    }
}

/// The command line after the option scan (3dvolreg's `VL_*` globals).
struct Options {
    input: Option<String>,
    maxdisp: bool,
    dmaxfile: Option<String>,
    emaxfile: Option<String>,
    matrix_save: Option<String>,
    sinit: bool,
    maxite: i32,
    dxy: f32,
    dph: f32,
    del: f32,
    final_mode: Option<Method>,
    rotcom: bool,
    twopass: bool,
    twoblur: f32,
    verbose: u32,
    clipit: bool,
    zpad: i32,
    resam: Method,
    prefix: String,
    dfile: Option<String>,
    oned_file: Option<String>,
    base: BaseArg,
    coarse_del: i32,
    coarse_num: i32,
    coarse_rot: bool,
    wtinp: bool,
    edging: Option<Edging>,
    weight: Option<(String, usize)>,
    twodup: bool,
    floatize: bool,
    dataset: String,
}

/// `-base`'s value as given: none, an index, or a dataset.
enum BaseArg {
    None,
    Index(usize),
    Dataset(String),
}

/// `strtol(s, &end, 10)` stored in an `int`: leading blanks, a sign and digits; returns the
/// value (the low 32 bits of the saturated `long`, as GCC converts) and the bytes used (0 when
/// there is no number).
fn strtol(s: &str) -> (i32, usize) {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && crate::cnum::is_space(b[i]) {
        i += 1;
    }
    let start = i;
    let neg = i < b.len() && b[i] == b'-';
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let digits = i;
    let mut v: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        v = v.saturating_mul(10).saturating_add(i64::from(b[i] - b'0'));
        i += 1;
    }
    if i == digits {
        return (0, 0);
    }
    let _ = start;
    let v = if neg { v.saturating_neg() } else { v };
    (v as i32, i)
}

/// A prefix match of `len` letters (`strncmp(arg, name, len) == 0`).
fn starts(arg: &str, name: &str, len: usize) -> bool {
    let (a, n) = (arg.as_bytes(), name.as_bytes());
    let len = len.min(n.len());
    a.len() >= len && a[..len] == n[..len]
}

/// `modify_afni_prefix(name, NULL, "_delt")`: the suffix inserted before a known extension, or
/// appended.
fn delt_name(name: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }
    const EXT: &[&str] = &[
        ".HEAD",
        ".BRIK",
        ".BRIK.gz",
        ".mnc",
        ".mri",
        ".svl",
        ".1D",
        ".1D.dset",
        ".1D.do",
        ".txt",
        ".3D",
        ".nii",
        ".nii.gz",
        ".nia",
        ".hdr",
        ".img",
        ".mpg",
        ".mpeg",
        ".MPG",
        ".MPEG",
        ".niml",
        ".niml.dset",
        ".niml.do",
        ".gii",
        ".gii.dset",
        ".niml.tract",
        ".jpg",
        ".jpeg",
        ".png",
        ".heic",
    ];
    for e in EXT {
        if let Some(stem) = name.strip_suffix(e) {
            return Some(format!("{stem}_delt{e}"));
        }
    }
    Some(format!("{name}_delt"))
}

/// The value after an option (AFNI reads `Argv[++Iarg]` without a check: AFNI-88).
fn value(
    args: &[String],
    i: &mut usize,
    name: &str,
    notices: &mut Notices,
) -> Result<String, Stop> {
    *i += 1;
    match args.get(*i) {
        Some(v) => Ok(v.clone()),
        None => {
            notices.add(
                &defects::AFNI_88,
                format!("{name} is the last argument and needs a value: AFNI reads past the end of its arguments (a crash)"),
            );
            Err(Stop::Fatal(format!(
                "need argument after {name} (AFNI-88: AFNI crashes here)"
            )))
        }
    }
}

#[allow(clippy::too_many_lines)]
fn parse(
    args: &[String],
    mode: UpstreamDefects,
    settings: &Settings,
    notices: &mut Notices,
    io: &mut Io<'_>,
) -> Result<Options, Stop> {
    let mut o = Options {
        input: None,
        maxdisp: true,
        dmaxfile: None,
        emaxfile: None,
        matrix_save: None,
        sinit: true,
        maxite: 23,
        dxy: 0.01,
        dph: 0.02,
        del: 0.70,
        final_mode: None,
        rotcom: false,
        twopass: false,
        twoblur: 2.0,
        verbose: 0,
        clipit: true,
        zpad: 4,
        resam: Method::Heptic,
        prefix: "volreg".to_owned(),
        dfile: None,
        oned_file: None,
        base: BaseArg::None,
        coarse_del: 10,
        coarse_num: 2,
        coarse_rot: true,
        wtinp: false,
        edging: None,
        weight: None,
        twodup: false,
        floatize: false,
        dataset: String::new(),
    };
    let mut i = 0;
    while i < args.len() && args[i].starts_with('-') {
        let arg = args[i].as_str();
        let unsupported = |what: &str| -> Result<Options, Stop> {
            Err(Stop::Fatal(format!(
                "{what} is not supported by larmorx yet"
            )))
        };
        if arg == "-input" || arg == "-source" {
            if o.input.is_some() {
                return Err(format!("Can't have 2 '{arg}' options!").into());
            }
            o.input = Some(value(args, &mut i, arg, notices)?);
            i += 1;
            continue;
        }
        if arg == "-maxdisp" {
            o.maxdisp = true;
            i += 1;
            continue;
        }
        if arg == "-nomaxdisp" {
            o.maxdisp = false;
            i += 1;
            continue;
        }
        if arg == "-maxdisp1D" {
            o.maxdisp = true;
            let v = value(args, &mut i, arg, notices)?;
            // MCW_strncpy(VL_dmaxfile, ..., 998): AFNI-90 by default keeps the whole name.
            let v = if mode.reproduce() && v.len() > 997 {
                v[..997].to_owned()
            } else {
                v
            };
            o.emaxfile = delt_name(&v);
            o.dmaxfile = Some(v);
            i += 1;
            continue;
        }
        if arg == "-savedisp" || arg == "-SAVEDISP" {
            return unsupported(arg);
        }
        if arg == "-1Dmatrix_save" {
            if o.matrix_save.is_some() {
                return Err(format!("Can't have multiple {arg} options!").into());
            }
            i += 1;
            let Some(v) = args.get(i) else {
                return Err(format!("no argument after '{arg}'!").into());
            };
            if !filename_ok(v, settings) {
                return Err(format!("badly formed filename: {arg} '{v}'").into());
            }
            o.matrix_save = Some(if v.ends_with(".1D") {
                v.clone()
            } else {
                format!("{v}.aff12.1D")
            });
            i += 1;
            continue;
        }
        if arg == "-sinit" {
            o.sinit = false;
            i += 1;
            continue;
        }
        if starts(arg, "-params", 4) {
            let a = value(args, &mut i, arg, notices)?;
            let b = value(args, &mut i, arg, notices)?;
            let c = value(args, &mut i, arg, notices)?;
            let d = value(args, &mut i, arg, notices)?;
            o.maxite = strtol(&a).0;
            o.dxy = strtod(&b).0 as f32;
            o.dph = strtod(&c).0 as f32;
            o.del = strtod(&d).0 as f32;
            i += 1;
            continue;
        }
        if starts(arg, "-maxite", 4) {
            let mit = strtol(&value(args, &mut i, arg, notices)?).0;
            if mit > 0 {
                o.maxite = mit;
            }
            i += 1;
            continue;
        }
        if starts(arg, "-x_thresh", 4) {
            let dxy = strtod(&value(args, &mut i, arg, notices)?).0 as f32;
            if dxy > 0.0 {
                o.dxy = dxy;
            }
            i += 1;
            continue;
        }
        if starts(arg, "-rot_thresh", 6) {
            let dph = strtod(&value(args, &mut i, arg, notices)?).0 as f32;
            if dph > 0.0 {
                o.dph = dph;
            }
            i += 1;
            continue;
        }
        if starts(arg, "-delta", 4) {
            let del = strtod(&value(args, &mut i, arg, notices)?).0 as f32;
            if del > 0.0 {
                o.del = del;
            }
            i += 1;
            continue;
        }
        if starts(arg, "-final", 4) {
            let v = value(args, &mut i, arg, notices)?;
            o.final_mode = Some(match v.as_str() {
                "cubic" => Method::Cubic,
                "quintic" => Method::Quintic,
                "heptic" => Method::Heptic,
                "Fourier" => Method::Fourier,
                "NN" => Method::Nn,
                "linear" => Method::Linear,
                _ => return Err(Stop::Raw("** Illegal mode after -final\n".to_owned())),
            });
            i += 1;
            continue;
        }
        if arg == "-rotcom" {
            o.rotcom = true;
            i += 1;
            continue;
        }
        if arg == "-twopass" {
            o.twopass = true;
            if o.maxite < 10 {
                o.maxite = 66;
            }
            i += 1;
            continue;
        }
        if arg == "-twoblur" {
            o.twoblur = strtod(&value(args, &mut i, arg, notices)?).0 as f32;
            if o.twoblur < 2.0 {
                return Err(Stop::Raw(
                    "** ERROR: value after -twoblur is < 2.0\n".to_owned(),
                ));
            }
            i += 1;
            continue;
        }
        if starts(arg, "-verbose", 5) {
            o.verbose += 1;
            i += 1;
            continue;
        }
        if starts(arg, "-clipit", 4) {
            io.raw("++ Notice: -clipit is now the default\n");
            o.clipit = true;
            i += 1;
            continue;
        }
        // AFNI-86: AFNI tests `-noc` before `-nocoarserot`.
        if starts(arg, "-noclip", 4) && !(mode.fix() && arg == "-nocoarserot") {
            if arg == "-nocoarserot" {
                notices.add(
                    &defects::AFNI_86,
                    "-nocoarserot is read as -noclip (its first four letters): the clipping is \
                     turned off and the coarse rotation search stays on, as AFNI does",
                );
            }
            o.clipit = false;
            i += 1;
            continue;
        }
        if starts(arg, "-zpad", 5) {
            if o.zpad != 4 {
                io.raw("++ WARNING: second -zpad option!\n");
            }
            o.zpad = strtod(&value(args, &mut i, arg, notices)?).0 as i32;
            if o.zpad < 0 {
                return Err(Stop::Raw(format!("** ERROR: Can't use -zpad {}\n", o.zpad)));
            }
            i += 1;
            continue;
        }
        if starts(arg, "-Fourier", 4) || starts(arg, "-fourier", 4) {
            o.resam = Method::Fourier;
            i += 1;
            continue;
        }
        if starts(arg, "-linear", 4) || starts(arg, "-Linear", 4) {
            o.resam = Method::Linear;
            i += 1;
            continue;
        }
        if starts(arg, "-cubic", 4) || starts(arg, "-Cubic", 4) {
            o.resam = Method::Cubic;
            i += 1;
            continue;
        }
        if starts(arg, "-quintic", 4) || starts(arg, "-Quintic", 4) {
            o.resam = Method::Quintic;
            i += 1;
            continue;
        }
        if starts(arg, "-heptic", 4) || starts(arg, "-Heptic", 4) {
            o.resam = Method::Heptic;
            i += 1;
            continue;
        }
        if starts(arg, "-prefix", 4) {
            o.prefix = value(args, &mut i, arg, notices)?;
            i += 1;
            continue;
        }
        if starts(arg, "-dfile", 4) {
            o.dfile = Some(value(args, &mut i, arg, notices)?);
            i += 1;
            continue;
        }
        if starts(arg, "-1Dfile", 4) {
            o.oned_file = Some(value(args, &mut i, arg, notices)?);
            i += 1;
            continue;
        }
        if starts(arg, "-base", 4) {
            let twice = match o.base {
                BaseArg::Dataset(_) => true,
                BaseArg::Index(k) => k > 0 || mode.fix(),
                BaseArg::None => false,
            };
            if twice {
                if let BaseArg::Index(0) = o.base {
                    notices.add(
                        &defects::AFNI_95,
                        "a second -base after -base 0: refused; AFNI accepts it (the last -base \
                         wins)",
                    );
                }
                return Err(Stop::Raw("** Can't have two -base arguments\n".to_owned()));
            }
            if let BaseArg::Index(0) = o.base {
                notices.add(
                    &defects::AFNI_95,
                    "a second -base after -base 0 replaces it, as AFNI does",
                );
            }
            let v = value(args, &mut i, arg, notices)?;
            let (bb, used) = strtol(&v);
            if bb < 0 {
                return Err("Illegal negative number after -base".to_owned().into());
            }
            o.base = if used == v.len() {
                BaseArg::Index(bb as usize)
            } else {
                BaseArg::Dataset(v)
            };
            i += 1;
            continue;
        }
        if arg == "-coarse" {
            o.coarse_del = strtol(&value(args, &mut i, arg, notices)?).0;
            o.coarse_num = strtol(&value(args, &mut i, arg, notices)?).0;
            i += 1;
            continue;
        }
        if arg == "-coarserot" {
            io.info("-coarserot is now the default");
            o.coarse_rot = true;
            i += 1;
            continue;
        }
        if arg == "-nocoarserot" {
            notices.add(
                &defects::AFNI_86,
                "-nocoarserot turns the coarse rotation search off, as its help says; AFNI reads \
                 it as -noclip (its first four letters), which turns the clipping off instead",
            );
            o.coarse_rot = false;
            i += 1;
            continue;
        }
        if arg == "-wtrim" {
            io.raw("++ Notice: -wtrim is now always enabled\n");
            i += 1;
            continue;
        }
        if arg == "-wtinp" {
            o.wtinp = true;
            i += 1;
            continue;
        }
        if arg == "-edging" {
            let v = value(args, &mut i, arg, notices)?;
            let (ee, used) = strtod(&v);
            let ee = ee as f32;
            if ee < 0.0 {
                return Err(Stop::Raw("** Illegal value after -edging\n".to_owned()));
            }
            if v.as_bytes().get(used) == Some(&b'%') {
                if ee > 25.0 {
                    return Err(Stop::Raw(
                        "** Illegal percentage after -edging\n".to_owned(),
                    ));
                }
                o.edging = Some(Edging::Percent(ee));
            } else {
                o.edging = Some(Edging::Voxels(ee));
            }
            i += 1;
            continue;
        }
        if starts(arg, "-weight", 4) {
            if o.weight.is_some() {
                return Err(Stop::Raw("** Can't have two -weight options\n".to_owned()));
            }
            let v = value(args, &mut i, arg, notices)?;
            let Some(pos) = v.find('[').filter(|&p| p > 0) else {
                return Err(Stop::Raw(
                    "** Illegal weight dataset after -weight\n".to_owned(),
                ));
            };
            let name = v[..pos].to_owned();
            // sscanf(cpt+1, "%d", &bb) with bb = -1 first.
            let bb = crate::cnum::scan_int(&v[pos + 1..]).map_or(-1, |(b, _)| b);
            if bb < 0 {
                return Err(Stop::Raw(
                    "** Illegal sub-brick selector after -weight\n".to_owned(),
                ));
            }
            o.weight = Some((name, bb as usize));
            i += 1;
            continue;
        }
        if arg == "-twodup" {
            o.twodup = true;
            i += 1;
            continue;
        }
        if arg == "-tshift" {
            return unsupported(arg);
        }
        if starts(arg, "-rotpar", 7) || starts(arg, "-gridpar", 8) {
            return unsupported(arg);
        }
        if starts(arg, "-float", 6) {
            o.floatize = true;
            i += 1;
            continue;
        }
        return Err(Stop::Raw(format!("** Unknown option: {arg}\x07\n")));
    }
    // The dataset.
    if o.input.is_none() {
        if i >= args.len() {
            let last = args.last().map_or("(null)", String::as_str);
            return Err(format!("Too few arguments!?  Last={last}").into());
        } else if i < args.len() - 1 {
            return Err(format!("Too many arguments?!  Dataset={}?", args[i]).into());
        }
        o.dataset = args[i].clone();
    } else {
        if i < args.len() {
            io.warn(format!(
                "Skipping argument(s) after last option? '{}'",
                args[i]
            ));
        }
        o.dataset = o.input.clone().unwrap_or_default();
    }
    Ok(o)
}

/// `tross_commandline("3dvolreg", argc, argv)`: the program and its arguments (after the
/// global options were taken out), the ones `THD_filename_ok` rejects quoted with their blanks,
/// control and non-ASCII bytes made blanks.
fn commandline(args: &[String], settings: &Settings) -> String {
    let mut line = String::from("3dvolreg");
    for a in args.iter().filter(|a| !a.is_empty()) {
        if filename_ok(a, settings) {
            line.push(' ');
            line.push_str(a);
        } else {
            let cleaned: String = a
                .bytes()
                .map(|b| {
                    if b.is_ascii_control() || crate::cnum::is_space(b) || b >= 128 {
                        ' '
                    } else {
                        char::from(b)
                    }
                })
                .collect();
            line.push_str(&format!(" '{cleaned}'"));
        }
    }
    line
}

/// Opens a dataset as `THD_open_dataset` + `DSET_load` (an input, a base, a weight), with the
/// messages AFNI prints while reading it; `None` with AFNI's reasons on failure.
fn open(
    name: &str,
    n_threads: usize,
    settings: &Settings,
    notices: &mut Notices,
    io: &mut Io<'_>,
) -> Result<Dataset, ReadError> {
    if name.contains('[') && !Path::new(&settings.path(name)).exists() {
        return Err(ReadError::Unsupported(format!(
            "{name}: sub-brick selectors"
        )));
    }
    let mut messages = Vec::new();
    let read = open_dataset(name, n_threads, settings, &mut messages, notices, true);
    for m in messages {
        io.message(m);
    }
    read
}

/// `"%.4f"` of a float, as C prints it.
fn f4(x: f32) -> String {
    format!("{x:.4}")
}

#[allow(clippy::too_many_lines)]
fn run(pre: &Prefiltered, mode: UpstreamDefects, io: &mut Io<'_>) -> Result<(), Stop> {
    let settings = &pre.settings;
    let n_threads = threads(settings);
    let mut notices = Notices::new(mode);
    notices.extend(pre.notices.iter().cloned());
    let o = parse(&pre.args, mode, settings, &mut notices, io)?;

    // The base dataset, opened during the option scan in AFNI (its messages come there).
    let mut base = Base::Index(0);
    let mut nbase_given = 0usize;
    let mut bset: Option<Dataset> = None;
    match &o.base {
        BaseArg::Index(k) => nbase_given = *k,
        BaseArg::Dataset(name) => {
            let ds = open(name, n_threads, settings, &mut notices, io).map_err(|e| {
                Stop::Fatal(match (mode.fix(), &e) {
                    (false, ReadError::Nifti(_) | ReadError::Afni(_)) => {
                        format!("Couldn't open -base dataset {name}")
                    }
                    _ => format!("Couldn't open -base dataset {name}: {e}"),
                })
            })?;
            if o.verbose > 0 {
                io.raw(format!(
                    "++ Reading in base dataset {}\n",
                    afni_output_name(name)
                ));
            }
            if ds.nvals > 1 {
                io.warn(format!("-base dataset {name} has more than 1 sub-brick"));
            }
            bset = Some(ds);
        }
        BaseArg::None => {}
    }
    let intern = !matches!(o.base, BaseArg::Dataset(_));
    // The weight dataset, also opened during the scan.
    let mut weight: Option<Vec<f32>> = None;
    let mut weight_dims = None;
    if let Some((name, bb)) = &o.weight {
        let ds = open(name, n_threads, settings, &mut notices, io).map_err(|e| {
            Stop::Raw(match (mode.fix(), &e) {
                (false, ReadError::Nifti(_) | ReadError::Afni(_)) => {
                    format!("** Can't open weight dataset {name}\n")
                }
                _ => format!("** Can't open weight dataset {name}: {e}\n"),
            })
        })?;
        if *bb >= ds.nvals {
            return Err(Stop::Raw(format!(
                "** Illegal sub-brick selector for dataset {name}\n"
            )));
        }
        if o.verbose > 0 {
            io.raw(format!(
                "++ Reading in weight dataset {}\n",
                afni_output_name(name)
            ));
        }
        let nvox = ds.nxyz.iter().product();
        weight = Some(crate::automask::scale_to_float(
            &ds.data,
            nvox,
            *bb,
            ds.factors[*bb],
        ));
        weight_dims = Some(ds.nxyz);
    }

    // After the scan.
    let twodup = o.twodup && o.twopass && !intern;
    if twodup {
        return Err(Stop::Fatal(
            "-twodup with an external base is not supported by larmorx yet".to_owned(),
        ));
    }
    let edging = match o.edging {
        Some(e) => e,
        None => match settings.get("AFNI_VOLREG_EDGING") {
            None => Edging::Percent(5.0),
            Some(ef) => {
                let (ee, used) = strtod(ef);
                let ee = ee as f32;
                if ee < 0.0 {
                    return Err(Stop::Raw(
                        "** Illegal value in AFNI_VOLREG_EDGING\n".to_owned(),
                    ));
                }
                if ef.as_bytes().get(used) == Some(&b'%') {
                    if ee > 25.0 {
                        return Err(Stop::Raw(
                            "** Illegal percentage in AFNI_VOLREG_EDGING\n".to_owned(),
                        ));
                    }
                    Edging::Percent(ee)
                } else {
                    Edging::Voxels(ee)
                }
            }
        },
    };

    // The dataset to register.
    if o.verbose > 0 {
        io.info(IMPLEMENTATION);
        io.info(format!("upstream defects: {mode} ({})", defects::ENV));
    }
    let ds = open(&o.dataset, n_threads, settings, &mut notices, io).map_err(|e| {
        let arg = o.dataset.as_str();
        match (mode.fix(), &e, o.input.is_some()) {
            (false, ReadError::Nifti(_) | ReadError::Afni(_), true) => {
                Stop::Fatal(format!("Can't open input dataset '{arg}'"))
            }
            (false, ReadError::Nifti(_) | ReadError::Afni(_), false) => {
                Stop::Raw(format!("** Can't open dataset {arg}\n"))
            }
            _ => Stop::Fatal(format!("Can't open input dataset '{arg}': {e}")),
        }
    })?;
    let mut floatize = o.floatize;
    let mut floatize_forced = false;
    if !floatize && ds.need_brick_factor() {
        floatize = true;
        floatize_forced = true;
        io.warn("Input dataset has scale factors ==> output will be stored as floats!");
    }
    let cen_inp = volreg::dataset_center(&ds).map_err(stop)?;
    if let Some(b) = &bset {
        let cen_bas = volreg::dataset_center(b).map_err(stop)?;
        let dv = [
            cen_bas[0] - cen_inp[0],
            cen_bas[1] - cen_inp[1],
            cen_bas[2] - cen_inp[2],
        ];
        let dist = (f64::from(dv[0] * dv[0] + dv[1] * dv[1] + dv[2] * dv[2])).sqrt() as f32;
        if dist > 0.01 && o.verbose > 0 {
            io.info(format!(
                "centers of base and input datasets are {dist:.2} mm apart"
            ));
        }
    }
    if intern && nbase_given >= ds.nvals {
        return Err(Stop::Raw(format!(
            "** Dataset {} doesn't have base brick index = {nbase_given}\n",
            o.dataset
        )));
    }
    if let Some(b) = &bset {
        mismatch(&ds, b, &o.dataset)?;
    }
    if let Some(wd) = weight_dims
        && wd != ds.nxyz
    {
        return Err(Stop::Raw(format!(
            "** Dataset {} doesn't conform to dimensions of weight brick\n    Weight  has nx = {}  ny = {}  nz = {}\n    Dataset has nx = {}  ny = {}  nz = {}\n",
            o.dataset, wd[0], wd[1], wd[2], ds.nxyz[0], ds.nxyz[1], ds.nxyz[2]
        )));
    }
    if intern && ds.nvals == 1 {
        return Err("You can't register a 1 brick dataset to itself!"
            .to_owned()
            .into());
    }
    let mut coarse_del = o.coarse_del;
    if o.twopass && o.coarse_del > 0 && o.coarse_num > 0 {
        let mm = ds.nxyz[0].min(ds.nxyz[1]).min(ds.nxyz[2]);
        let mut mm = (0.1 * mm as f64 + 0.499) as i32;
        if mm < 1 {
            mm = 1;
        }
        if coarse_del > mm {
            io.raw(format!(
                "++ Coarse del was {coarse_del}, replaced with {mm}\n"
            ));
            coarse_del = mm;
        }
    }
    let mut wtinp = o.wtinp;
    if weight.is_some() && wtinp {
        io.raw("++ Input weight file overrides -wtinp option!\n");
        wtinp = false;
    }

    // main(): the output.
    let null_output = o.prefix == "NULL";
    let mut out = None;
    if null_output {
        io.warn("No output dataset will be calculated");
    } else {
        output_path(&o.prefix)?;
        let output = Output::new(&o.prefix, settings, pre.quiet_overwrite);
        if output.deathcon && output.exists() {
            return Err(Stop::Fatal(format!(
                "Output file {} already exists -- cannot continue!\n",
                afni_output_name(&o.prefix)
            )));
        }
        output.check(false, &notices, io)?;
        out = Some(output);
    }
    let cmdline = commandline(&pre.args, settings);
    if o.verbose > 0 {
        io.raw(format!("++ Reading input dataset {}\n", o.dataset));
    }

    if let Some(b) = &bset {
        let nvox = b.nxyz.iter().product();
        let vol = crate::automask::scale_to_float(&b.data, nvox, 0, b.factors[0]);
        base = Base::External {
            vol,
            center: volreg::dataset_center(b).map_err(stop)?,
            origin: b.geometry.axes.org,
        };
    } else {
        base = match base {
            Base::Index(_) => Base::Index(nbase_given),
            other => other,
        };
    }
    let p = VolregParams {
        base,
        weight,
        wtinp,
        resam: o.resam,
        final_mode: o.final_mode,
        clipit: o.clipit,
        zpad: o.zpad.max(0) as usize,
        twopass: o.twopass,
        twoblur: o.twoblur,
        sinit: o.sinit,
        coarse_del,
        coarse_num: o.coarse_num,
        coarse_rot: o.coarse_rot,
        edging,
        maxite: o.maxite,
        dxy: o.dxy,
        dph: o.dph,
        del: o.del,
        maxdisp: o.maxdisp,
        verbose: o.verbose,
        floatize,
        firfac: settings.num("AFNI_BLUR_FIRFAC"),
        firold: settings.yes("AFNI_BLUR_FIROLD"),
        mode,
    };
    let mut log = Vec::new();
    let result = volreg::volreg(&ds, &p, null_output, &mut log, &mut notices);
    for m in log {
        io.message(m);
    }
    let res = result.map_err(stop)?;
    let final_mode = o.final_mode.unwrap_or(o.resam);
    let imcount = ds.nvals;

    // The matrices and the maximum displacements.
    if res.maxdisp.is_some() && o.verbose > 0 {
        io.info("Max displacements (mm) for each sub-brick:");
    }
    if let Some(name) = &o.matrix_save {
        let mut text = String::from("# 3dvolreg matrices (DICOM-to-DICOM, row-by-row):\n");
        for row in &res.matrix_save {
            let cells: Vec<String> = row.iter().map(|&v| format!("{:>13}", fmt_g(v))).collect();
            text.push_str(&cells.join(" "));
            text.push('\n');
        }
        if std::fs::write(settings.path(name), text).is_err() {
            io.error(format!("Cannot open '{name}' for output :("));
        }
    }
    if let Some(md) = &res.maxdisp {
        if o.verbose > 0 {
            for kim in 0..imcount {
                io.raw(format!(" {:.2}({:.2})", md.dmax[kim], md.emax[kim]));
            }
            io.raw("\n");
        }
        io.info(format!(
            "Max displacement in automask = {:.2} (mm) at sub-brick {}",
            md.top.0, md.top.1
        ));
        io.info(format!(
            "Max delta displ  in automask = {:.2} (mm) at sub-brick {}",
            md.top_delta.0, md.top_delta.1
        ));
        let files = [
            (&o.dmaxfile, "max displacement", &md.dmax),
            (&o.emaxfile, "max delta displ", &md.emax),
        ];
        for (file, what, vals) in files {
            let Some(name) = file else { continue };
            let mut text = format!("# {cmdline}\n# {what} (mm) for each volume\n");
            for v in vals.iter() {
                text.push_str(&format!(" {v:.3}\n"));
            }
            if name == "-" {
                io.print(text.trim_end_matches('\n'));
                continue;
            }
            let path = settings.path(name);
            if path.is_file() {
                io.warn(format!("Overwriting file {name}"));
            }
            if std::fs::write(&path, text).is_err() {
                io.error(format!("Cannot open '{name}' for output :("));
            }
        }
    }

    // The dataset.
    if let (Some(output), Some(ds_out)) = (&out, &res.output) {
        let written = write_dataset(output, ds_out, n_threads, settings, &mut notices, io)
            .map_err(Stop::from)?;
        if o.verbose > 0 {
            io.info(format!("Wrote dataset to disk in {}", written.shown));
        }
    }

    // The parameter files.
    write_params(&o, &res, imcount, settings, io);
    if o.rotcom {
        io.print(format!(
            "\n3drotate fragment{}:\n",
            if imcount > 1 { "s" } else { "" }
        ));
        for kim in 0..imcount {
            let mut line = format!("3drotate {}", mode_name(final_mode));
            line.push_str(if o.clipit { " -clipit" } else { " -noclip" });
            if o.zpad != 0 {
                line.push_str(&format!(" -zpad {}", o.zpad));
            }
            line.push_str(&format!(
                " -rotate {}I {}R {}A -ashift {}S {}L {}P",
                f4(res.roll[kim]),
                f4(res.pitch[kim]),
                f4(res.yaw[kim]),
                f4(res.dx[kim]),
                f4(res.dy[kim]),
                f4(res.dz[kim])
            ));
            io.print(line);
        }
        io.print("");
    }
    if floatize_forced && !null_output {
        io.warn("Input dataset has scale factors ==> output is stored as floats!");
    }
    for n in notices.as_slice() {
        io.notice(n);
    }
    Ok(())
}

/// `-dfile` and `-1Dfile`.
fn write_params(
    o: &Options,
    res: &VolregResult,
    imcount: usize,
    settings: &Settings,
    io: &mut Io<'_>,
) {
    let f8 = |x: f32| format!("{x:8.4}");
    let g11 = |x: f32| format!("{:>11}", gp(f64::from(x), 4));
    if let Some(name) = &o.dfile {
        let path = settings.path(name);
        if path.is_file() {
            io.raw(format!("** Warning: overwriting file {name}\n"));
        }
        let mut text = String::new();
        for kim in 0..imcount {
            text.push_str(&format!(
                "{kim:4} {} {} {} {} {} {}  {} {}\n",
                f8(res.roll[kim]),
                f8(res.pitch[kim]),
                f8(res.yaw[kim]),
                f8(res.dx[kim]),
                f8(res.dy[kim]),
                f8(res.dz[kim]),
                g11(res.rmsold[kim]),
                g11(res.rmsnew[kim])
            ));
        }
        if std::fs::write(&path, text).is_err() {
            io.error(format!("Cannot open '{name}' for output"));
        }
    }
    if let Some(name) = &o.oned_file {
        let path = settings.path(name);
        if path.is_file() {
            io.raw(format!("** Warning: overwriting file {name}\n"));
        }
        let mut text = String::new();
        for kim in 0..imcount {
            text.push_str(&format!(
                "{} {} {} {} {} {}\n",
                f8(res.roll[kim]),
                f8(res.pitch[kim]),
                f8(res.yaw[kim]),
                f8(res.dx[kim]),
                f8(res.dy[kim]),
                f8(res.dz[kim])
            ));
        }
        if std::fs::write(&path, text).is_err() {
            io.error(format!("Cannot open '{name}' for output"));
        }
    }
}

/// `printf("%.<prec>g", x)`.
fn gp(x: f64, prec: usize) -> String {
    if x.is_nan() {
        return if x.is_sign_negative() { "-nan" } else { "nan" }.to_owned();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf" } else { "inf" }.to_owned();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    let p = prec.max(1);
    let sci = format!("{x:.*e}", p - 1);
    let (mantissa, exp) = sci.split_once('e').expect("exponent form");
    let exp: i32 = exp.parse().expect("an exponent");
    let trim = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s.to_owned()
        }
    };
    if exp < -4 || exp >= p as i32 {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", trim(mantissa), exp.abs())
    } else {
        let decimals = (p as i32 - 1 - exp) as usize;
        trim(&format!("{x:.decimals$}"))
    }
}

/// `THD_dataset_mismatch`'s dimension, spacing and orientation tests between the input and the
/// base, with 3dvolreg's messages.
fn mismatch(ds: &Dataset, b: &Dataset, name: &str) -> Result<(), Stop> {
    let mut text = String::new();
    let (a, c) = (&ds.geometry.axes, &b.geometry.axes);
    let shown_in = afni_output_name(name);
    let shown_b = "(base)";
    if ds.nxyz != b.nxyz {
        text.push_str(&format!(
            "** Input {shown_in} and base {shown_b} don't have same dimensions!\n   Input: nx={}  ny={}  nz={}\n   Base:  nx={}  ny={}  nz={}\n",
            ds.nxyz[0], ds.nxyz[1], ds.nxyz[2], b.nxyz[0], b.nxyz[1], b.nxyz[2]
        ));
    }
    let far = |x: f32, y: f32| f64::from(x - y).abs() > 0.01 * f64::from(x).abs();
    if far(a.del[0], c.del[0]) || far(a.del[1], c.del[1]) || far(a.del[2], c.del[2]) {
        text.push_str(&format!(
            "** Input {shown_in} and base {shown_b} don't have same grid spacing!\n   Input: dx={:6.3}  dy={:6.3}  dz={:6.3}\n   Base:  dx={:6.3}  dy={:6.3}  dz={:6.3}\n",
            a.del[0], a.del[1], a.del[2], c.del[0], c.del[1], c.del[2]
        ));
    }
    if a.orient != c.orient {
        let s = |o: Option<[u8; 3]>, k: usize| {
            let t = crate::automask::tinystr(o.map(|v| v[k]));
            format!("{}2{}", t[0], t[1])
        };
        text.push_str(&format!(
            "** Input {shown_in} and base {shown_b} don't have same orientation!\n   Input: {} {} {}\n   Base:  {} {} {} \n",
            s(a.orient, 0), s(a.orient, 1), s(a.orient, 2), s(c.orient, 0), s(c.orient, 1), s(c.orient, 2)
        ));
    }
    if text.is_empty() {
        return Ok(());
    }
    text.push_str("** FATAL ERROR: perhaps you could make your datasets match?\n");
    Err(Stop::Raw(text))
}

/// A registration error, as AFNI prints it.
fn stop(e: VolregError) -> Stop {
    match e {
        VolregError::Fatal(m) => Stop::Fatal(m),
        VolregError::Plain(m) => Stop::Raw(m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_names() {
        assert_eq!(strtol("12"), (12, 2));
        assert_eq!(strtol(" -3x"), (-3, 3));
        assert_eq!(strtol("abc"), (0, 0));
        assert!(starts("-maxdispX", "-maxite", 4));
        assert!(starts("-nocoarserot", "-noclip", 4));
        assert!(!starts("-no", "-noclip", 4));
        assert_eq!(delt_name("md.1D").as_deref(), Some("md_delt.1D"));
        assert_eq!(delt_name("-").as_deref(), Some("-_delt"));
        assert_eq!(delt_name("a.b").as_deref(), Some("a.b_delt"));
        assert_eq!(gp(0.123456, 4), "0.1235");
        assert_eq!(gp(123456.0, 4), "1.235e+05");
        assert_eq!(gp(12.0, 4), "12");
        assert_eq!(gp(0.00001234, 4), "1.234e-05");
    }
}
