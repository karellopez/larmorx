// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05 src/3dvolreg.c (main and VL_command_line: the option scan,
// the checks, the messages, the parameter files and the -rotcom fragments), src/thd_notes.c
// (tross_commandline), src/thd_opendset.c (modify_afni_prefix with a suffix),
// src/thd_mismatch.c (THD_dataset_mismatch's dimension, spacing and orientation tests),
// src/thd_tshift.c (THD_dataset_tshift), 3dvolreg.c's VL_normalize_timeseries and
// VL_create_disprod, src/thd_detrend.c (THD_normmax), src/thd_zeropad.c (THD_zeropad with
// ZPAD_EMPTY), src/cs_addto_args.c (addto_args) and src/thd_coords.c (THD_report_obliquity's
// first_oblique). Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI,
// Robert W. Cox et al.), released by AFNI 26.3.05 under the Creative Commons Attribution 4.0
// International licence (CC BY 4.0). thd_mismatch.c and thd_zeropad.c carry no such notice:
// they were written at the NIH and are in the public domain.
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
//! history), so a `-rotparent` dataset must come from AFNI's own 3dvolreg; no version banner;
//! the CPU times of `-verbose` are not measured (printed as 0); `-savedisp`'s names must end
//! in `.nii` or `.nii.gz` too; sub-brick selectors (`dset[...]`) are refused, as by every
//! larmorx AFNI tool (`-weight`'s own `wset[n]` is 3dvolreg's, not a selector, and works).
//! `-@` (the arguments from standard input) works as AFNI's.
//!
//! The datasets of `-input`, `-base`, `-weight`, `-rotparent` and `-gridparent` are opened
//! during the option scan, as AFNI opens them, so their messages come where AFNI prints them.

use std::io::{Read, Write};
use std::path::Path;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notices, UpstreamDefects};

use crate::axes::Axes;
use crate::cnum::{fmt_g, strtod};
use crate::dataset::{BrickData, Dataset, ReadError};
use crate::environ::{self, Prefiltered, Settings, VOLREG};
use crate::message::Message;
use crate::shift::Method;
use crate::tshift::{Rlt, TshiftParams};
use crate::vecmat::{self, DVec3, FVec3};
use crate::volreg::{self, Base, Edging, VolregError, VolregParams, VolregResult, mode_name};

use super::common::{
    Io, Output, afni_output_name, echo_command_line, filename_ok, open_dataset, output_name_ok,
    output_path, write_dataset,
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
    let mut pre = environ::prefilter_for(&VOLREG, args, env, mode);
    let mut io = Io::new(out, err, &pre.settings, mode);
    echo_command_line("3dvolreg", &pre, &mut io);
    for w in &pre.warnings {
        io.warn(w);
    }
    if !pre.settings.yes("AFNI_QUIET_STARTUP") {
        io.info("Authored by: RW Cox");
    }
    // addto_args: a last argument `-@` is replaced by the words of the standard input.
    if pre.args.last().map(String::as_str) == Some("-@") {
        let mut text = Vec::new();
        let _ = std::io::stdin().lock().read_to_end(&mut text);
        addto_args(&mut pre.args, &text);
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

/// `addto_args`: `-@`, the last argument, replaced by the words of `text` (split at blanks,
/// tabs, newlines, carriage returns, form feeds and vertical tabs). Nothing read: the `-@`
/// stays, and the option scan reports it.
fn addto_args(args: &mut Vec<String>, text: &[u8]) {
    let words: Vec<String> = text
        .split(|&c| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0c | 0x0b))
        .filter(|w| !w.is_empty())
        .map(|w| String::from_utf8_lossy(w).into_owned())
        .collect();
    if text.is_empty() || words.is_empty() {
        return;
    }
    args.pop();
    args.extend(words);
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
    /// `-weight`: the dataset's name, and its sub-brick as floats with the grid's size.
    weight: Option<(String, Vec<f32>, [usize; 3])>,
    twodup: bool,
    floatize: bool,
    dataset: String,
    /// `-input`'s dataset, opened during the scan as AFNI opens it.
    input_ds: Option<Dataset>,
    /// With `-input`: the first argument after the options, if any (AFNI-98).
    rest: Option<String>,
    /// `-base dset`: what 3dvolreg keeps of it after the scan.
    bset: Option<BaseSet>,
    /// `-tshift ii`: the number of points to ignore.
    tshift: Option<i32>,
    /// `-savedisp` / `-SAVEDISP`: the names' first and last parts, and whether the 19
    /// normalised datasets are wanted.
    savedisp: Option<(String, String, bool)>,
    rotpar: Option<RotPar>,
    gridpar: Option<GridPar>,
}

/// A `-base` dataset as 3dvolreg keeps it after the scan (AFNI unloads the dataset): its
/// sub-brick 0 as floats, its grid and its centre.
struct BaseSet {
    name: String,
    vol: Vec<f32>,
    nxyz: [usize; 3],
    axes: Axes,
    center: FVec3,
}

/// A `-rotparent` dataset's `VOLREG_*` attributes.
struct RotPar {
    name: String,
    matvec: Vec<f32>,
    center_old: Option<Vec<f32>>,
    center_base: Option<Vec<f32>>,
}

/// A `-gridparent` dataset's grid.
struct GridPar {
    nxyz: [usize; 3],
    axes: Axes,
    center: FVec3,
}

/// `-base`'s value as given: none, an index, or a dataset.
enum BaseArg {
    None,
    Index(usize),
    Dataset,
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
/// What a run prints and writes of `-maxdisp`'s values, as it prints them: the two messages
/// (two decimals and the sub-brick), `-verbose`'s list (two decimals) and the lines of the
/// `-maxdisp1D` files (three). AFNI-99's notice compares the two modes' forms, so it is given
/// exactly where the correction changes what the run shows.
fn printed_maxdisp(md: &volreg::MaxDisp, o: &Options) -> Vec<String> {
    let mut shown = vec![
        format!("{:.2} {}", md.top.0, md.top.1),
        format!("{:.2} {}", md.top_delta.0, md.top_delta.1),
    ];
    if o.verbose > 0 {
        shown.extend(
            md.dmax
                .iter()
                .zip(&md.emax)
                .map(|(d, e)| format!("{d:.2}({e:.2})")),
        );
    }
    if o.dmaxfile.is_some() {
        shown.extend(md.dmax.iter().map(|d| format!("{d:.3}")));
    }
    if o.emaxfile.is_some() {
        shown.extend(md.emax.iter().map(|e| format!("{e:.3}")));
    }
    shown
}

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
    n_threads: usize,
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
        input_ds: None,
        rest: None,
        bset: None,
        tshift: None,
        savedisp: None,
        rotpar: None,
        gridpar: None,
    };
    let mut i = 0;
    while i < args.len() && args[i].starts_with('-') {
        let arg = args[i].as_str();
        if arg == "-input" || arg == "-source" {
            if o.input.is_some() {
                return Err(format!("Can't have 2 '{arg}' options!").into());
            }
            i += 1;
            // THD_open_dataset(NULL) fails: AFNI names the dataset "(null)".
            let Some(name) = args.get(i).cloned() else {
                return Err(Stop::Fatal("Can't open input dataset '(null)'".to_owned()));
            };
            let ds = open(&name, n_threads, settings, notices, io)
                .map_err(|e| input_error(&name, &e, mode, true))?;
            o.input = Some(name);
            o.input_ds = Some(ds);
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
            // AFNI checks `++Iarg > Argc` (off by one) and then copies a NULL: AFNI-88.
            let v = value(args, &mut i, arg, notices)?;
            let bad = || -> Stop { format!("{arg} '{v}' is invalid output dataset name").into() };
            if !filename_ok(&v, settings) {
                return Err(bad());
            }
            let (pra, prb) = if let Some(stem) = v.strip_suffix(".nii") {
                if v.len() < 5 {
                    return Err(bad());
                }
                (stem.to_owned(), ".nii")
            } else if let Some(stem) = v.strip_suffix(".nii.gz") {
                if v.len() < 8 {
                    return Err(bad());
                }
                (stem.to_owned(), ".nii.gz")
            } else {
                (v.clone(), "")
            };
            o.savedisp = Some((pra, prb.to_owned(), arg == "-SAVEDISP"));
            i += 1;
            continue;
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
            o.prefix = long_name(value(args, &mut i, arg, notices)?, arg, mode, notices)?;
            i += 1;
            continue;
        }
        if starts(arg, "-dfile", 4) {
            o.dfile = Some(long_name(
                value(args, &mut i, arg, notices)?,
                arg,
                mode,
                notices,
            )?);
            i += 1;
            continue;
        }
        if starts(arg, "-1Dfile", 4) {
            o.oned_file = Some(long_name(
                value(args, &mut i, arg, notices)?,
                arg,
                mode,
                notices,
            )?);
            i += 1;
            continue;
        }
        if starts(arg, "-base", 4) {
            let twice = match o.base {
                BaseArg::Dataset => true,
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
            if used == v.len() {
                o.base = BaseArg::Index(bb as usize);
                o.bset = None;
            } else {
                // The base dataset is opened, read and unloaded during the scan.
                let ds = open(&v, n_threads, settings, notices, io).map_err(|e| {
                    Stop::Fatal(match (mode.fix(), &e) {
                        (false, ReadError::Nifti(_) | ReadError::Afni(_)) => {
                            format!("Couldn't open -base dataset {v}")
                        }
                        _ => format!("Couldn't open -base dataset {v}: {e}"),
                    })
                })?;
                if o.verbose > 0 {
                    io.raw(format!(
                        "++ Reading in base dataset {}\n",
                        crate::dataset::brikname(&settings.path(&v).to_string_lossy(), settings)
                    ));
                }
                if ds.nvals > 1 {
                    io.warn(format!("-base dataset {v} has more than 1 sub-brick"));
                }
                let nvox = ds.nxyz.iter().product();
                o.bset = Some(BaseSet {
                    name: v.clone(),
                    vol: crate::automask::scale_to_float(&ds.data, nvox, 0, ds.factors[0]),
                    nxyz: ds.nxyz,
                    center: volreg::dataset_center(&ds).map_err(stop)?,
                    axes: ds.geometry.axes.clone(),
                });
                o.base = BaseArg::Dataset;
            }
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
            // `char dname[256]`: a longer name overflows it (AFNI-90).
            if pos > 255 {
                long_name_notice(&name, "-weight dataset", mode, notices);
                if mode.reproduce() {
                    return Err(long_name_stop("-weight dataset", pos));
                }
            }
            // sscanf(cpt+1, "%d", &bb) with bb = -1 first.
            let bb = crate::cnum::scan_int(&v[pos + 1..]).map_or(-1, |(b, _)| b);
            if bb < 0 {
                return Err(Stop::Raw(
                    "** Illegal sub-brick selector after -weight\n".to_owned(),
                ));
            }
            let bb = bb as usize;
            let ds = open(&name, n_threads, settings, notices, io).map_err(|e| {
                Stop::Raw(match (mode.fix(), &e) {
                    (false, ReadError::Nifti(_) | ReadError::Afni(_)) => {
                        format!("** Can't open weight dataset {name}\n")
                    }
                    _ => format!("** Can't open weight dataset {name}: {e}\n"),
                })
            })?;
            if bb >= ds.nvals {
                return Err(Stop::Raw(format!(
                    "** Illegal sub-brick selector for dataset {name}\n"
                )));
            }
            if o.verbose > 0 {
                io.raw(format!(
                    "++ Reading in weight dataset {}\n",
                    crate::dataset::brikname(&settings.path(&name).to_string_lossy(), settings)
                ));
            }
            let nvox = ds.nxyz.iter().product();
            let w = crate::automask::scale_to_float(&ds.data, nvox, bb, ds.factors[bb]);
            o.weight = Some((name, w, ds.nxyz));
            i += 1;
            continue;
        }
        if arg == "-twodup" {
            o.twodup = true;
            i += 1;
            continue;
        }
        if arg == "-tshift" {
            // (int)strtod(...): out-of-range values clamp (C leaves them undefined).
            let ig = strtod(&value(args, &mut i, arg, notices)?).0 as i32;
            if ig < 0 {
                return Err(Stop::Raw("** -tshift parameter is negative!\n".to_owned()));
            }
            o.tshift = Some(ig);
            i += 1;
            continue;
        }
        if starts(arg, "-rotpar", 7) {
            if o.rotpar.is_some() {
                return Err(Stop::Raw(
                    "** Can't use -2 -rotparent options!\n".to_owned(),
                ));
            }
            i += 1;
            // THD_open_one_dataset(NULL) fails, as a dataset that cannot be opened.
            let cant = "** Can't open -rotparent dataset!\n\n";
            let Some(name) = args.get(i).cloned() else {
                return Err(Stop::Raw(cant.to_owned()));
            };
            let _ds = open(&name, n_threads, settings, notices, io).map_err(|e| {
                Stop::Raw(if mode.fix() {
                    format!("** Can't open -rotparent dataset {name}: {e}\n")
                } else {
                    cant.to_owned()
                })
            })?;
            let attrs = crate::dataset::afni_attributes(&settings.path(&name).to_string_lossy());
            let floats = |n: &str| {
                attrs
                    .as_ref()
                    .and_then(|a| a.floats(n))
                    .map(<[f32]>::to_vec)
            };
            let Some(matvec) = floats("VOLREG_MATVEC_000000").filter(|m| m.len() >= 12) else {
                return Err(Stop::Raw(
                    "** -rotparent dataset doesn't have VOLREG attributes!?\n".to_owned(),
                ));
            };
            o.rotpar = Some(RotPar {
                name,
                matvec,
                center_old: floats("VOLREG_CENTER_OLD"),
                center_base: floats("VOLREG_CENTER_BASE"),
            });
            i += 1;
            continue;
        }
        if starts(arg, "-gridpar", 7) {
            if o.gridpar.is_some() {
                return Err(Stop::Raw(
                    "** Can't use -2 -gridparent options!\n".to_owned(),
                ));
            }
            i += 1;
            let cant = "** Can't open -gridparent dataset!\n\n";
            let Some(name) = args.get(i).cloned() else {
                return Err(Stop::Raw(cant.to_owned()));
            };
            let ds = open(&name, n_threads, settings, notices, io).map_err(|e| {
                Stop::Raw(if mode.fix() {
                    format!("** Can't open -gridparent dataset {name}: {e}\n")
                } else {
                    cant.to_owned()
                })
            })?;
            o.gridpar = Some(GridPar {
                nxyz: ds.nxyz,
                center: volreg::dataset_center(&ds).map_err(stop)?,
                axes: ds.geometry.axes.clone(),
            });
            i += 1;
            continue;
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
        o.rest = args.get(i).cloned();
        o.dataset = o.input.clone().unwrap_or_default();
    }
    Ok(o)
}

/// The message when the dataset to register cannot be opened: AFNI's, and by default the
/// reason too where AFNI gives none.
fn input_error(arg: &str, e: &ReadError, mode: UpstreamDefects, option: bool) -> Stop {
    match (mode.fix(), e, option) {
        (false, ReadError::Nifti(_) | ReadError::Afni(_), true) => {
            Stop::Fatal(format!("Can't open input dataset '{arg}'"))
        }
        (false, ReadError::Nifti(_) | ReadError::Afni(_), false) => {
            Stop::Raw(format!("** Can't open dataset {arg}\n"))
        }
        _ => Stop::Fatal(format!("Can't open input dataset '{arg}': {e}")),
    }
}

/// AFNI-90: `strcpy` into a 256-byte buffer (`-prefix`, `-dfile`, `-1Dfile`). A name of 256
/// bytes or more makes AFNI abort (`*** buffer overflow detected ***`); by default it is kept
/// whole, with the notice; `reproduce` refuses it.
fn long_name(
    v: String,
    arg: &str,
    mode: UpstreamDefects,
    notices: &mut Notices,
) -> Result<String, Stop> {
    if v.len() > 255 {
        long_name_notice(&v, arg, mode, notices);
        if mode.reproduce() {
            return Err(long_name_stop(arg, v.len()));
        }
    }
    Ok(v)
}

fn long_name_notice(v: &str, what: &str, mode: UpstreamDefects, notices: &mut Notices) {
    let n = v.len();
    notices.add(
        &defects::AFNI_90,
        if mode.fix() {
            format!(
                "the {what} name has {n} bytes: kept whole; AFNI copies it into a 256-byte \
                 buffer and aborts (buffer overflow detected)"
            )
        } else {
            format!(
                "the {what} name has {n} bytes, more than AFNI's 256-byte buffer holds: AFNI \
                 aborts (buffer overflow detected)"
            )
        },
    );
}

fn long_name_stop(what: &str, n: usize) -> Stop {
    Stop::Fatal(format!(
        "the {what} name has {n} bytes; AFNI's buffer holds 255 and AFNI aborts here \
         (*** buffer overflow detected ***, AFNI-90)"
    ))
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
        // THD_report_obliquity: its long warning only for the run's first oblique dataset.
        if let Message::Warning(text) = &m
            && text.starts_with(OBLIQUE_WARNING)
        {
            if io.oblique_warned {
                continue;
            }
            io.oblique_warned = true;
        }
        // THD_open_nifti's conversion to float: announced for the run's first such dataset
        // only, unless AFNI_NIFTI_TYPE_WARN is YES (its static n_xform_warn).
        if let Message::Raw(text) = &m
            && text.starts_with(TYPE_WARNING)
        {
            let first = !io.type_warned;
            io.type_warned = true;
            if !first && !settings.yes("AFNI_NIFTI_TYPE_WARN") {
                continue;
            }
        }
        io.message(m);
    }
    read
}

/// The start of `THD_open_nifti`'s announcement of a conversion to float.
const TYPE_WARNING: &str = "** AFNI converts NIFTI_datatype=";

/// The start of `THD_report_obliquity`'s warning, which AFNI prints once per run.
const OBLIQUE_WARNING: &str = "  If you are performing spatial transformations on an oblique dset";

/// `"%.4f"` of a float, as C prints it.
fn f4(x: f32) -> String {
    format!("{x:.4}")
}

#[allow(clippy::too_many_lines)]
fn run(pre: &Prefiltered, mode: UpstreamDefects, io: &mut Io<'_>) -> Result<(), Stop> {
    let mut notices = Notices::new(mode);
    notices.extend(pre.notices.iter().cloned());
    let r = run_with(pre, mode, io, &mut notices);
    if r.is_err() {
        // A run that stops names the defects met so far: the default's refusals (AFNI-94,
        // AFNI-95, ...) with their reason. A run that ends prints them itself, last.
        for n in notices.as_slice() {
            io.notice(n);
        }
    }
    r
}

fn run_with(
    pre: &Prefiltered,
    mode: UpstreamDefects,
    io: &mut Io<'_>,
    notices: &mut Notices,
) -> Result<(), Stop> {
    let settings = &pre.settings;
    let n_threads = threads(settings);
    let mut o = parse(&pre.args, mode, settings, n_threads, notices, io)?;
    let intern = !matches!(o.base, BaseArg::Dataset);
    let nbase_given = match o.base {
        BaseArg::Index(k) => k,
        _ => 0,
    };

    // After the scan: -twodup needs -twopass and an external base; -gridparent needs
    // -rotparent.
    let mut twodup = o.twodup && o.twopass && !intern;
    if o.gridpar.is_some() && o.rotpar.is_none() {
        io.raw("++ WARNING: -gridparent means nothing without -rotparent!\n");
        o.gridpar = None;
    }
    if o.rotpar.is_some() && o.twopass {
        io.raw("++ WARNING: Combining -rotparent and -twopass isn't recommended!\n");
    }
    if o.gridpar.is_some() && twodup {
        io.raw("++ WARNING: -gridparent overrides -twodup!\n");
        twodup = false;
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
    let mut ds = match o.input_ds.take() {
        Some(ds) => ds,
        None => open(&o.dataset, n_threads, settings, notices, io)
            .map_err(|e| input_error(&o.dataset, &e, mode, false))?,
    };
    let mut floatize = o.floatize;
    let mut floatize_forced = false;
    if !floatize && ds.need_brick_factor() {
        floatize = true;
        floatize_forced = true;
        io.warn("Input dataset has scale factors ==> output will be stored as floats!");
    }
    let cen_inp = volreg::dataset_center(&ds).map_err(stop)?;
    if let Some(b) = &o.bset {
        let cen_bas = b.center;
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
    let [nx, ny, nz] = ds.nxyz;
    let mut tshift = o.tshift;
    if let Some(ig) = tshift {
        let nvals = ds.nvals as i64;
        let nsl = ds
            .taxis
            .as_ref()
            .and_then(|t| t.toff_sl.as_ref())
            .map_or(0, Vec::len);
        if ds.nvals < 2 {
            io.raw("++ WARNING: -tshift used on a 1 brick dataset!\n");
            tshift = None;
        } else if ds.taxis.is_none() || nsl < nz {
            io.raw("++ WARNING: -tshift used on a dataset with no time-offsets!\n");
            tshift = None;
        } else if i64::from(ig) > nvals - 5 {
            io.raw("++ WARNING: -tshift ignore is too large for this dataset!\n");
            tshift = None;
        } else if intern && (nbase_given as i64) < i64::from(ig) {
            io.raw("++ WARNING: base brick is prior to -tshift ignore point.\n");
        }
    }
    // AFNI-98: these two messages print the argument after the options; with -input that is
    // a stray argument or none ("(null)").
    let shown = match (&o.input, mode.reproduce()) {
        (Some(_), true) => o.rest.clone().unwrap_or_else(|| "(null)".to_owned()),
        _ => o.dataset.clone(),
    };
    let note98 = |notices: &mut Notices| {
        let afni = o.rest.clone().unwrap_or_else(|| "(null)".to_owned());
        if o.input.is_some() && afni != o.dataset {
            notices.add(
                &defects::AFNI_98,
                if mode.fix() {
                    format!(
                        "the message names the dataset given by -input; AFNI prints '{afni}' (the \
                         argument after the options) instead"
                    )
                } else {
                    format!(
                        "the message names '{afni}', the argument after the options, as AFNI does, \
                         not the dataset given by -input"
                    )
                },
            );
        }
    };
    if intern && nbase_given >= ds.nvals {
        note98(notices);
        return Err(Stop::Raw(format!(
            "** Dataset {shown} doesn't have base brick index = {nbase_given}\n"
        )));
    }
    if let Some(b) = &o.bset {
        mismatch(&ds, b.nxyz, &b.axes, &o.dataset, &b.name)?;
    }
    if let Some((_, _, wd)) = &o.weight
        && *wd != ds.nxyz
    {
        note98(notices);
        return Err(Stop::Raw(format!(
            "** Dataset {} doesn't conform to dimensions of weight brick\n    Weight  has nx = {}  ny = {}  nz = {}\n    Dataset has nx = {}  ny = {}  nz = {}\n",
            shown, wd[0], wd[1], wd[2], nx, ny, nz
        )));
    }
    if intern && ds.nvals == 1 {
        return Err("You can't register a 1 brick dataset to itself!"
            .to_owned()
            .into());
    }
    let mut coarse_del = o.coarse_del;
    if o.twopass && o.coarse_del > 0 && o.coarse_num > 0 {
        let mm = nx.min(ny).min(nz);
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
    if o.weight.is_some() && wtinp {
        io.raw("++ Input weight file overrides -wtinp option!\n");
        wtinp = false;
    }

    // main(): the output dataset's grid (new_dset), zero-padded to match -gridparent.
    let mut out_axes = ds.geometry.axes.clone();
    let mut out_taxis = ds.taxis.clone();
    let mut out_nz = nz;
    let mut npad = [0usize; 2];
    let orient = ds.geometry.axes.orient.ok_or_else(|| {
        Stop::Fatal(
            "the dataset's voxel-to-world matrix has no orientation (a zero or repeated \
             column): not supported by larmorx"
                .to_owned(),
        )
    })?;
    if let Some(gp) = &o.gridpar {
        let (a, g) = (&ds.geometry.axes, &gp.axes);
        let far = |x: f32, y: f32| f64::from(x - y).abs() > 0.01 * f64::from(x).abs();
        let delta = (0..3).any(|k| far(g.del[k], a.del[k]));
        if delta || g.orient != a.orient {
            return Err(Stop::Raw(
                "** Fatal Error:\n** -gridparent dataset and input dataset don't\n** match in grid spacing and/or orientation!\n".to_owned(),
            ));
        }
        if gp.nxyz[0] != nx || gp.nxyz[1] != ny {
            return Err(Stop::Raw(
                "** Fatal Error:\n** -gridparent and input datasets\n** don't match in x,y dimensions!\n".to_owned(),
            ));
        }
        let nz_gp = gp.nxyz[2];
        if nz_gp < nz {
            return Err(Stop::Raw(
                "** Fatal Error:\n** -gridparent has fewer slices than input dataset!\n".to_owned(),
            ));
        }
        if nz_gp > nz {
            let npad1 = (nz_gp - nz) / 2;
            let npad2 = (nz_gp - nz) - npad1;
            let zo = orient[2];
            let (sp1, sp2) = match crate::axes::coordinate(zo) {
                0 => ("R", "L"),
                1 => ("A", "P"),
                _ => ("I", "S"),
            };
            // npad_neg slices before the first (index 0), npad_pos after the last.
            npad = if crate::axes::positive(zo) {
                [npad1, npad2]
            } else {
                [npad2, npad1]
            };
            if o.verbose > 0 {
                io.raw(format!(
                    "++ Zero padding to match -gridparent: -{sp1} {npad1}  -{sp2} {npad2}\n"
                ));
            }
            // THD_zeropad(..., ZPAD_EMPTY): the new grid, its matrix the cardinal one, the
            // slice times padded with zeros.
            out_nz = nz + npad1 + npad2;
            out_axes.org[2] -= npad[0] as f32 * out_axes.del[2];
            if let Some(m) = crate::axes::cardinal(&out_axes) {
                out_axes.real = m;
            }
            if let Some(t) = out_taxis.as_mut()
                && let Some(old) = t.toff_sl.take()
            {
                let mut new = vec![0.0f32; out_nz];
                new[npad[0]..npad[0] + old.len()].copy_from_slice(&old);
                t.toff_sl = Some(new);
            }
        }
    }

    // The output's name.
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
        output.check(false, notices, io)?;
        name_notice(&o.prefix, mode, settings, notices);
        out = Some(output);
    }
    // -savedisp's datasets (written with larmorx's NIfTI writer only).
    let disp_names: Vec<(&str, String)> = match &o.savedisp {
        None => Vec::new(),
        Some((pra, prb, s19)) => {
            let labels: &[&str] = if *s19 {
                &DISP_LABELS
            } else {
                &DISP_LABELS[..3]
            };
            labels
                .iter()
                .map(|l| (*l, format!("{pra}{l}{prb}")))
                .collect()
        }
    };
    let mut disp_out = Vec::new();
    for (_, name) in &disp_names {
        output_path(name)?;
        let output = Output::new(name, settings, pre.quiet_overwrite);
        output.check(false, notices, io)?;
        name_notice(name, mode, settings, notices);
        disp_out.push(output);
    }
    let cmdline = commandline(&pre.args, settings);

    // -rotparent: its transformation, moved for the centres of the datasets and converted to
    // the dataset's axis order.
    let mut matvec = None;
    if let Some(rp) = &o.rotpar {
        let m: Vec<f64> = rp.matvec.iter().map(|&x| f64::from(x)).collect();
        let mut rmat = [[m[0], m[1], m[2]], [m[4], m[5], m[6]], [m[8], m[9], m[10]]];
        let mut tvec = [m[3], m[7], m[11]];
        let pp = vecmat::mul(&vecmat::transpose(&rmat), &rmat);
        let sum = ((pp[0][0] - 1.0).abs()
            + pp[1][0].abs()
            + pp[2][0].abs()
            + pp[0][1].abs()
            + (pp[1][1] - 1.0).abs()
            + pp[2][1].abs()
            + pp[0][2].abs()
            + pp[1][2].abs()
            + (pp[2][2] - 1.0).abs()) as f32;
        if f64::from(sum) > 0.01 {
            return Err(Stop::Raw(
                "** -rotparent matrix not orthogonal!\n".to_owned(),
            ));
        }
        let out_grid = grid_center(&out_axes, [nx, ny, out_nz]);
        let cv_e2 = out_grid.map(f64::from);
        let cv_e1 = o
            .gridpar
            .as_ref()
            .map_or(cv_e2, |g| g.center.map(f64::from));
        let mut three = |v: &Option<Vec<f32>>, what: &str| -> Result<DVec3, Stop> {
            match v.as_deref() {
                Some(&[a, b, c, ..]) => Ok([f64::from(a), f64::from(b), f64::from(c)]),
                _ => {
                    notices.add(
                        &defects::AFNI_96,
                        format!(
                            "the -rotparent dataset {} has VOLREG_MATVEC_000000 but no {what} \
                             attribute of 3 numbers: refused; AFNI reads through a NULL pointer \
                             (a crash) or past the attribute",
                            rp.name
                        ),
                    );
                    Err(Stop::Fatal(format!(
                        "-rotparent dataset {} has no {what} attribute (AFNI-96: AFNI crashes)",
                        rp.name
                    )))
                }
            }
        };
        let cv_s2 = three(&rp.center_old, "VOLREG_CENTER_OLD")?;
        let cv_s1 = three(&rp.center_base, "VOLREG_CENTER_BASE")?;
        let dv = vecmat::matvec(&rmat, &vecmat::sub(&cv_e2, &cv_s2));
        let ev = vecmat::sub(&cv_e1, &cv_s1);
        let qv = vecmat::sub(&dv, &ev);
        tvec = vecmat::add(&tvec, &qv);
        let pp = volreg::mat_to_dicomm(orient);
        let ppt = vecmat::transpose(&pp);
        rmat = vecmat::mul(&vecmat::mul(&ppt, &rmat), &pp);
        tvec = vecmat::matvec(&ppt, &tvec);
        if let Some(gp) = &o.gridpar {
            out_axes.org = gp.axes.org;
        }
        // The time offsets, moved by the shift along the slices.
        if let Some(t) = out_taxis.as_mut()
            && let Some(old) = t.toff_sl.as_mut()
        {
            let ndz = (tvec[2] / f64::from(out_axes.del[2]).abs()).round_ties_even() as i64;
            if ndz != 0 {
                let nsl = old.len() as i64;
                let shifted: Vec<f32> = (0..nsl)
                    .map(|kk| {
                        let jj = kk - ndz;
                        if jj < 0 || jj >= nsl {
                            0.0
                        } else {
                            old[jj as usize]
                        }
                    })
                    .collect();
                *old = shifted;
                if o.verbose > 0 {
                    io.raw(format!("++ adjusting time-offsets by {ndz} slices\n"));
                }
            }
        }
        matvec = Some(volreg::Matvec { rmat, tvec, npad });
    }

    // Reading (and time-shifting) the input.
    if o.verbose > 0 {
        let shown =
            crate::dataset::brikname(&settings.path(&o.dataset).to_string_lossy(), settings);
        if tshift.is_some() {
            io.raw(format!("++ Time shifting input dataset {shown}\n"));
        } else {
            io.raw(format!("++ Reading input dataset {shown}\n"));
        }
    }
    if let Some(ig) = tshift {
        match tshift_dataset(&mut ds, ig as usize, n_threads, mode) {
            Ok(found) => {
                notices.extend(found);
                if let Some(t) = out_taxis.as_mut() {
                    t.toff_sl = None;
                    t.ttorg = 0.0;
                }
            }
            Err(TshiftFail::Afni) => {
                io.raw("++ WARNING: some error during -tshift operation!\n");
            }
            Err(TshiftFail::Larmorx(m)) => return Err(Stop::Fatal(m)),
        }
    }

    let base = match &o.bset {
        Some(b) => Base::External {
            vol: b.vol.clone(),
            center: b.center,
            origin: b.axes.org,
        },
        None => Base::Index(nbase_given),
    };
    let p = VolregParams {
        base,
        weight: o.weight.as_ref().map(|(_, w, _)| w.clone()),
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
        matvec,
        whole_point_mirrors: false,
        mode,
    };
    drop(o.weight.take());
    let mut log = Vec::new();
    let result = volreg::volreg(&mut ds, &p, null_output, &mut log, notices);
    // Where AFNI crashes, its banner starts a new line: a progress line left open ("0..1")
    // ends before larmorx's error.
    let open_line = matches!(log.last(), Some(Message::Raw(t)) if !t.ends_with('\n'));
    let crashed = matches!(result, Err(VolregError::Crash(_)));
    for m in log {
        io.message(m);
    }
    if crashed && open_line {
        io.raw("\n");
    }
    let mut res = result.map_err(stop)?;
    drop(p);
    let final_mode = o.final_mode.unwrap_or(o.resam);
    let imcount = ds.nvals;
    let out_n = [nx, ny, out_nz];

    // AFNI-99: named where a maximum displacement the run prints or writes differs between the
    // two modes (the other mode's values come from the registration).
    if let Some(md) = &res.maxdisp
        && md
            .other
            .as_deref()
            .is_some_and(|m2| printed_maxdisp(md, &o) != printed_maxdisp(m2, &o))
    {
        notices.add(
            &defects::AFNI_99,
            if mode.fix() {
                "the maximum displacements are those of the voxels themselves; AFNI converts the \
                 matrix with the signs of the axes stored L2R, P2A or S2I but not the voxel \
                 positions, and so reports the displacements of the mirrored positions"
            } else {
                "the maximum displacements are those of the positions mirrored along the axes \
                 stored L2R, P2A or S2I, as AFNI computes them"
            },
        );
    }

    // The matrices, the displacements and the maximum displacements.
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
    // -savedisp: each voxel's displacement along x, y and z, sub-brick by sub-brick.
    let mut disp: Vec<Dataset> = Vec::new();
    if !disp_names.is_empty() {
        let mut parts = [Vec::new(), Vec::new(), Vec::new()];
        // AFNI-99: the other mode's displacements too, to name it where a value changes.
        let other_mode = if mode.reproduce() {
            UpstreamDefects::Fix
        } else {
            UpstreamDefects::Reproduce
        };
        let check = out_axes.orient.is_some_and(volreg::has_negative_axis);
        let mut differs = false;
        for kim in 0..imcount {
            let m = &res.matvec[kim];
            let tvec = [f64::from(m[3]), f64::from(m[7]), f64::from(m[11])];
            let (a, r) = (&ds.geometry.axes, &res.rmat[kim]);
            let d = volreg::displacements(a, &out_axes, out_n, r, &tvec, mode).map_err(stop)?;
            if check && !differs {
                let e = volreg::displacements(a, &out_axes, out_n, r, &tvec, other_mode)
                    .map_err(stop)?;
                differs = d != e;
            }
            for (part, v) in parts.iter_mut().zip(d) {
                part.extend_from_slice(&v);
            }
        }
        if differs {
            notices.add(
                &defects::AFNI_99,
                if mode.fix() {
                    "-savedisp's displacements are those of the voxels themselves; AFNI converts \
                     the matrix with the signs of the axes stored L2R, P2A or S2I but not the \
                     voxel positions, and so writes the displacements of the mirrored positions"
                } else {
                    "-savedisp's displacements are those of the positions mirrored along the axes \
                     stored L2R, P2A or S2I, as AFNI computes them"
                },
            );
        }
        for part in parts {
            disp.push(float_dataset(&ds, out_n, &out_axes, &out_taxis, part));
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

    // The registered dataset: the output grid, -twodup's origin (the base's).
    if let Some(ds_out) = res.output.as_mut() {
        ds_out.geometry.axes = out_axes.clone();
        ds_out.taxis = out_taxis.clone();
        if twodup
            && o.rotpar.is_none()
            && let Some(b) = &o.bset
        {
            ds_out.geometry.axes.org = b.axes.org;
        }
    }
    if let (Some(output), Some(ds_out)) = (&out, &res.output) {
        let written =
            write_dataset(output, ds_out, n_threads, settings, notices, io).map_err(Stop::from)?;
        if o.verbose > 0 {
            io.info(format!("Wrote dataset to disk in {}", written.shown));
        }
    }
    res.output = None;
    if let Some((_, _, s19)) = &o.savedisp {
        for (k, d) in disp.iter_mut().enumerate() {
            if *s19 {
                normalize_timeseries(d);
            }
            let written = write_dataset(&disp_out[k], d, n_threads, settings, notices, io)
                .map_err(Stop::from)?;
            if o.verbose > 0 {
                io.info(format!("Wrote dataset to disk in {}", written.shown));
            }
        }
        if *s19 {
            for (k, powers) in DISP_PRODUCTS.iter().enumerate() {
                let mut d = disprod(&disp, *powers);
                normalize_timeseries(&mut d);
                let written = write_dataset(&disp_out[3 + k], &d, n_threads, settings, notices, io)
                    .map_err(Stop::from)?;
                if o.verbose > 0 {
                    io.info(format!("Wrote dataset to disk in {}", written.shown));
                }
            }
        }
    }
    drop(disp);

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

/// AFNI-12 by default: an output name `THD_filename_ok` rejects (blanks, shell metacharacters,
/// non-ASCII characters, a leading `-`) is written; AFNI's 3dvolreg writes nothing then, with
/// exit status 0 (`reproduce`: the writer says so and writes nothing).
fn name_notice(prefix: &str, mode: UpstreamDefects, settings: &Settings, notices: &mut Notices) {
    if mode.fix() && !output_name_ok(prefix, settings) {
        notices.add(
            &defects::AFNI_12,
            format!(
                "the output name '{}' has blanks, shell metacharacters or non-ASCII characters, \
                 or starts with '-': written; AFNI writes nothing (exit status 0)",
                afni_output_name(prefix)
            ),
        );
    }
}

/// The suffixes of `-savedisp`'s 3 and `-SAVEDISP`'s 19 datasets, in the order AFNI writes
/// them.
const DISP_LABELS: [&str; 19] = [
    "_DX", "_DY", "_DZ", "_DXX", "_DYY", "_DZZ", "_DXY", "_DXZ", "_DYZ", "_DXXX", "_DYYY", "_DZZZ",
    "_DXXY", "_DXXZ", "_DXYY", "_DXZZ", "_DYYZ", "_DYZZ", "_DXYZ",
];

/// The Legendre degrees of x, y and z of `-SAVEDISP`'s 16 products (`VL_create_disprod`).
const DISP_PRODUCTS: [[u8; 3]; 16] = [
    [2, 0, 0],
    [0, 2, 0],
    [0, 0, 2],
    [1, 1, 0],
    [1, 0, 1],
    [0, 1, 1],
    [3, 0, 0],
    [0, 3, 0],
    [0, 0, 3],
    [2, 1, 0],
    [2, 0, 1],
    [1, 2, 0],
    [1, 0, 2],
    [0, 2, 1],
    [0, 1, 2],
    [1, 1, 1],
];

/// `THD_dataset_center` of a grid given by its axes and size.
fn grid_center(axes: &Axes, n: [usize; 3]) -> FVec3 {
    let Some(o) = axes.orient else {
        return [0.0; 3];
    };
    let dicom = |v: [f32; 3]| {
        let mut d = [0.0f32; 3];
        for (axis, &code) in o.iter().enumerate() {
            d[crate::axes::coordinate(code)] = v[axis];
        }
        d
    };
    let fv1 = dicom(axes.org);
    let fv2 = dicom([
        axes.org[0] + (n[0] as f32 - 1.0) * axes.del[0],
        axes.org[1] + (n[1] as f32 - 1.0) * axes.del[1],
        axes.org[2] + (n[2] as f32 - 1.0) * axes.del[2],
    ]);
    [
        0.5f32 * (fv1[0] + fv2[0]),
        0.5f32 * (fv1[1] + fv2[1]),
        0.5f32 * (fv1[2] + fv2[2]),
    ]
}

/// A float dataset on the output's grid (`EDIT_empty_copy(new_dset)` with float sub-bricks).
fn float_dataset(
    like: &Dataset,
    n: [usize; 3],
    axes: &Axes,
    taxis: &Option<crate::dataset::TimeAxis>,
    data: Vec<f32>,
) -> Dataset {
    let mut d = header_of(like);
    d.nxyz = n;
    d.geometry.axes = axes.clone();
    d.taxis = taxis.clone();
    d.factors = vec![0.0; like.nvals];
    d.data = BrickData::Float(data);
    d.dropped_inter = None;
    d
}

/// A dataset's header without its data.
fn header_of(ds: &Dataset) -> Dataset {
    Dataset {
        nxyz: ds.nxyz,
        nvals: ds.nvals,
        data: BrickData::Float(Vec::new()),
        factors: ds.factors.clone(),
        taxis: ds.taxis.clone(),
        geometry: ds.geometry.clone(),
        dropped_inter: None,
    }
}

/// `VL_normalize_timeseries`: each voxel's series detrended (`THD_linear_detrend`) and scaled
/// to a largest magnitude of 1 (`THD_normmax`).
fn normalize_timeseries(d: &mut Dataset) {
    let nvox: usize = d.nxyz.iter().product();
    let nvals = d.nvals;
    let BrickData::Float(v) = &mut d.data else {
        return;
    };
    let mut tar = vec![0.0f32; nvals];
    for ii in 0..nvox {
        for (t, x) in tar.iter_mut().enumerate() {
            *x = v[ii + t * nvox];
        }
        crate::detrend::linear_detrend(&mut tar);
        let mut fac = 0.0f32;
        for &x in &tar {
            let val = x.abs();
            fac = if fac > val { fac } else { val };
        }
        if fac != 0.0 {
            let fac = 1.0f32 / fac;
            for x in &mut tar {
                *x *= fac;
            }
        }
        for (t, &x) in tar.iter().enumerate() {
            v[ii + t * nvox] = x;
        }
    }
}

/// `VL_create_disprod`: the product of Legendre polynomials of the normalised x, y and z
/// displacements (`P0 = 1`, `P1 = x`, `P2 = 1.5x² - 0.5`, `P3 = (2.5x² - 1.5)x`, in float).
fn disprod(disp: &[Dataset], powers: [u8; 3]) -> Dataset {
    let leg = |n: u8, x: f32| -> f32 {
        match n {
            0 => 1.0,
            1 => x,
            2 => 1.5f32 * x * x - 0.5f32,
            _ => (2.5f32 * x * x - 1.5f32) * x,
        }
    };
    let get = |k: usize| match &disp[k].data {
        BrickData::Float(v) => v.as_slice(),
        _ => &[],
    };
    let (xd, yd, zd) = (get(0), get(1), get(2));
    let data: Vec<f32> = (0..xd.len())
        .map(|i| leg(powers[0], xd[i]) * leg(powers[1], yd[i]) * leg(powers[2], zd[i]))
        .collect();
    let mut d = header_of(&disp[0]);
    d.data = BrickData::Float(data);
    d
}

/// Why `-tshift` did not shift: one of `THD_dataset_tshift`'s own failures (AFNI then warns
/// and goes on), or one of larmorx's limits.
enum TshiftFail {
    Afni,
    Larmorx(String),
}

/// `THD_dataset_tshift(dset, ignore)`: 3dTshift's default (Fourier interpolation, the linear
/// trend removed and restored, the slices aligned to their mean time offset) on the dataset in
/// place. Returns the upstream defects met (AFNI-01).
fn tshift_dataset(
    ds: &mut Dataset,
    ignore: usize,
    n_threads: usize,
    mode: UpstreamDefects,
) -> Result<Vec<defects::Notice>, TshiftFail> {
    let nzz = ds.nxyz[2];
    let ntt = ds.nvals;
    if ignore + 4 > ntt || ntt < 2 {
        return Err(TshiftFail::Afni);
    }
    let Some(t) = ds.taxis.as_ref() else {
        return Err(TshiftFail::Afni);
    };
    let Some(tpat) = t.toff_sl.as_ref().filter(|v| v.len() >= nzz) else {
        return Err(TshiftFail::Afni);
    };
    let tpat: Vec<f32> = tpat[..nzz].to_vec();
    let tr = t.ttdel;
    // WAY_BIG = 1e10, as float.
    let (mut tomin, mut tomax) = (1.0e10f32, -1.0e10f32);
    for &x in &tpat {
        if x > tomax {
            tomax = x;
        }
        if x < tomin {
            tomin = x;
        }
    }
    if tomin < 0.0 || tomax > tr || tomin >= tomax {
        return Err(TshiftFail::Afni);
    }
    let mut tzero = 0.0f32;
    for &x in &tpat {
        tzero += x;
    }
    tzero /= nzz as f32;
    let params = TshiftParams {
        tr,
        tpat,
        tzero,
        ignore,
        method: Method::Fourier,
        rlt: Rlt::Trend,
        detrend: true,
        bad: false,
        n_threads,
        upstream_defects: mode,
    };
    crate::tshift::tshift(ds, &params).map_err(|e| TshiftFail::Larmorx(e.to_string()))
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
fn mismatch(
    ds: &Dataset,
    b_nxyz: [usize; 3],
    c: &Axes,
    name: &str,
    base_name: &str,
) -> Result<(), Stop> {
    let mut text = String::new();
    let a = &ds.geometry.axes;
    let shown_in = afni_output_name(name);
    let shown_b = afni_output_name(base_name);
    if ds.nxyz != b_nxyz {
        text.push_str(&format!(
            "** Input {shown_in} and base {shown_b} don't have same dimensions!\n   Input: nx={}  ny={}  nz={}\n   Base:  nx={}  ny={}  nz={}\n",
            ds.nxyz[0], ds.nxyz[1], ds.nxyz[2], b_nxyz[0], b_nxyz[1], b_nxyz[2]
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
            format!("{}-{}", t[0], t[1])
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
        VolregError::Fatal(m) | VolregError::Crash(m) => Stop::Fatal(m),
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
