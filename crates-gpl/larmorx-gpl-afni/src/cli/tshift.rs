// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09 src/3dTshift.c (main, TS_copy_input_to_output),
// src/thd_filestuff.c (THD_filename_ok), src/thd_writedset.c (the AFNI_DECONFLICT part of
// THD_write_3dim_dataset), src/thd_opendset.c (THD_deconflict_prefix, THD_deconflict_nifti),
// src/afni_environ.c (THD_deathcon, THD_ok_overwrite), src/afni_logger.c (the
// AFNI_ECHO_COMMANDLINE part of AFNI_logger), src/thd_notes.c (tross_commandline),
// src/thd_mastery.c (the blank and "~/" cases of THD_open_dataset) and src/thd_opentcat.c (the
// failure path of THD_open_tcat).
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released under the GNU GPL version 2 or any later version. afni_logger.c and
// thd_opentcat.c carry no such notice: they were written at the NIH and are in the public
// domain.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! `3dTshift` with its original arguments: the option scan, checks, messages and output of
//! AFNI's `main()`, step by step.
//!
//! Messages go where AFNI sends them: `printf` lines (`++ updating time offset to ...` and
//! most `-verbose` lines) to standard output; warnings, errors and `++ Wrote output` to
//! standard error, with AFNI's labels ([`crate::message`]).
//!
//! **AFNI's global options and environment variables** ([`crate::environ`]). The process
//! environment is read once into a settings table; AFNI's global options are taken out of the
//! command line wherever they stand and applied to it: `-overwrite`, `-Dname=value`,
//! `-ok_1D_text` and `-skip_afnirc`. The variables then decide what AFNI's code decides with
//! them: an existing output (`AFNI_DECONFLICT`), the output names AFNI accepts
//! (`AFNI_ALLOW_ARBITRARY_FILENAMES`), an input name with a blank (`AFNI_PATH_SPACES_OK`), the
//! messages (`AFNI_MESSAGE_PREFIX`, `AFNI_MESSAGE_COLORIZE`, `AFNI_ECHO_COMMANDLINE`), and
//! through [`dataset`] and [`crate::oned`] the reading and writing. The process environment is
//! never changed.
//!
//! **Writing.** The output is written to a temporary file in its directory and renamed over its
//! name ([`super::output`]), so an existing file is replaced whole (`-overwrite`) or not at all,
//! on every platform.
//!
//! Differences from AFNI, shared with `larmorx afni 3dTshift` (the larmorx conventions):
//! - input and output are NIfTI only, and `-prefix` must end in `.nii` or `.nii.gz` (AFNI
//!   writes its own format otherwise, and by default, with the prefix `tshift`);
//! - `-voxshift`, sub-brick selectors, datasets joined from a list and AFNI's own formats are
//!   not supported;
//! - AFNI's history extension is not written, and neither the version banner nor the compile
//!   date AFNI prints after a fatal error;
//! - with `-verbose`, a first line on standard error names the implementation that runs (the
//!   replica), as `larmorx afni 3dTshift` names the clean-room original.
//!
//! **Upstream defects (D13).** AFNI's defects are corrected unless the environment variable
//! `LARMORX_UPSTREAM_DEFECTS` is `reproduce`, which gives AFNI's result
//! (`docs/findings/upstream-defects.md`); a warning (`*+ WARNING: upstream defect ...`, the
//! line of `larmorx_core::defects::Notice::line`) names each one that changed the output, or
//! would have. Besides those of the dataset ([`dataset`]) and the shift ([`tshift`]), the
//! command line's are:
//! - an existing output that `AFNI_DECONFLICT` does not allow to overwrite or rename is an
//!   error before the work (AFNI-11; AFNI prints "dataset NOT written to disk!" after the work
//!   and exits with status 0);
//! - any `-prefix` path is accepted, so paths with blanks or non-ASCII characters work on every
//!   platform (AFNI-12; unless `AFNI_ALLOW_ARBITRARY_FILENAMES` is YES, AFNI refuses names that
//!   `THD_filename_ok` rejects: blanks, shell metacharacters, non-ASCII bytes; at the option
//!   scan, and again when it writes the file, where a name that passed the scan through
//!   `THD_filename_ok`'s `3dcalc` exception, or a directory starting with `-`, is not written
//!   and the exit status is 0);
//! - an input name with a blank is one file (AFNI-18; unless `AFNI_PATH_SPACES_OK` is YES,
//!   AFNI reads it as a list of datasets, so an input in a directory with a blank cannot be
//!   opened);
//! - `-Dname=value` takes the value as written (AFNI-22; AFNI reads `-DX=a b` as `X=a=b`);
//! - with `-rlt` and `-ignore`, data whose intercept `AFNI_NIFTI_SCALE=NO` dropped are read as
//!   without the variable (AFNI-23, [`rlt_dropped_intercept`]; otherwise the dataset's writer
//!   puts the intercept back).
//!
//! In `reproduce` mode the labels of warnings and errors carry AFNI's terminal escape codes
//! unless `AFNI_MESSAGE_COLORIZE` is NO; by default they are plain text (AFNI-21).

use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notice, Notices, UpstreamDefects};

use crate::cnum::{fmt_g, is_space, strtod};
use crate::dataset::{self, Dataset, ReadError, TimeAxis, TimeUnits};
use crate::environ::{self, Prefiltered, Settings};
use crate::message::{Message, Style};
use crate::oned::decode_string_list;
use crate::shift::Method;
use crate::tpattern::{TpatternError, parse_tpattern};
use crate::tshift::{Rlt, TshiftParams, fft_length, slice_shift, tshift};

use super::output::{openable, replace_atomically};
use super::threads;

/// The first `-verbose` line: which implementation runs (larmorx's addition, not AFNI's).
const IMPLEMENTATION: &str = concat!(
    "implementation: replica (larmorx-gpl ",
    env!("CARGO_PKG_VERSION"),
    ", GPL-3.0-or-later), bit-exact translation of AFNI 25.2.09's 3dTshift"
);

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
  -prefix ppp     output file name, ending in .nii or .nii.gz; it must not exist (see
                  -overwrite)
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

AFNI's global options (anywhere on the command line):
  -overwrite      replace an existing output (AFNI_DECONFLICT=OVERWRITE)
  -Dname=value    set AFNI's environment variable 'name' for this run
  -ok_1D_text     read text in a -tpattern file as 0 (AFNI_1D_ZERO_TEXT=YES)
  -skip_afnirc    accepted (no .afnirc file is read)
";

/// Runs `3dTshift` with `args`; returns the exit status. The D13 mode comes from
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
    // `3dTshift` alone, or with -help first, prints the help before anything else happens.
    if args.is_empty() || args[0] == "-help" {
        let _ = out.write_all(USAGE.as_bytes());
        return 0;
    }
    let pre = environ::prefilter(args, env, mode);
    let mut io = Io::new(out, err, &pre.settings, mode);
    echo_command_line(&pre, &mut io);
    for w in &pre.warnings {
        io.warn(w);
    }
    match run(&pre, mode, &mut io) {
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
    style: Style,
}

impl<'a> Io<'a> {
    /// AFNI's message style: `AFNI_MESSAGE_PREFIX`, and the coloured labels of `reproduce`
    /// mode unless `AFNI_MESSAGE_COLORIZE` starts with N.
    fn new(
        out: &'a mut dyn Write,
        err: &'a mut dyn Write,
        settings: &Settings,
        mode: UpstreamDefects,
    ) -> Io<'a> {
        Io {
            out,
            err,
            style: Style {
                prefix: settings.get("AFNI_MESSAGE_PREFIX").map(str::to_owned),
                colorize: mode.reproduce() && !settings.no("AFNI_MESSAGE_COLORIZE"),
            },
        }
    }

    /// `printf(...)`.
    fn print(&mut self, s: impl std::fmt::Display) {
        let _ = writeln!(self.out, "{s}");
    }

    /// A message on standard error, as AFNI's routine of its kind prints it.
    fn message(&mut self, m: Message) {
        let _ = self.err.write_all(self.style.format(&m).as_bytes());
    }

    /// `WARNING_message(...)`.
    fn warn(&mut self, s: impl std::fmt::Display) {
        self.message(Message::Warning(s.to_string()));
    }

    /// `ERROR_message(...)` (the run goes on).
    fn error(&mut self, s: impl std::fmt::Display) {
        self.message(Message::Error(s.to_string()));
    }

    /// `INFO_message(...)`.
    fn info(&mut self, s: impl std::fmt::Display) {
        self.message(Message::Info(s.to_string()));
    }

    /// `fprintf(stderr, ...)`: the text as it is.
    fn raw(&mut self, s: impl std::fmt::Display) {
        self.message(Message::Raw(s.to_string()));
    }

    /// An upstream-defect notice, as a warning line (`Notice::line`), which the Python
    /// wrappers turn into `larmorx.UpstreamDefectWarning`s.
    fn notice(&mut self, n: &Notice) {
        self.warn(n.line());
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
    /// Whether AFNI would accept `prefix` ([`filename_ok`]); larmorx-gpl accepts any path
    /// unless AFNI-12 is reproduced.
    prefix_afni_ok: bool,
}

/// `THD_filename_ok`: no control characters, blanks, shell metacharacters or non-ASCII bytes,
/// or any name when `AFNI_ALLOW_ARBITRARY_FILENAMES` is YES; a name of more than 6 characters
/// that begins with `3dcalc` always passes. larmorx-gpl accepts any `-prefix`; AFNI-12
/// reproduced refuses the names this rejects (`docs/findings/afni-tshift.md`, "Output names").
fn filename_ok(name: &str, settings: &Settings) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.len() > 6 && name.starts_with("3dcalc") {
        return true;
    }
    if settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES") {
        return true;
    }
    !name.bytes().any(|c| {
        c.is_ascii_control() || is_space(c) || b";*?&|\"><'[](){}!".contains(&c) || c & 128 != 0
    })
}

/// Whether `prefix` names a directory: AFNI looks for a `/`; on Windows a `\` or a drive
/// letter does too.
fn has_directory(prefix: &str) -> bool {
    prefix.contains('/')
        || (cfg!(windows) && (prefix.contains('\\') || prefix.as_bytes().get(1) == Some(&b':')))
}

/// The name AFNI gives the output dataset (`DSET_BRIKNAME`, `DSET_HEADNAME`) and hands
/// `THD_write_nifti` for `prefix`: the dataset's directory (`./` when the prefix names none)
/// and its prefix.
fn afni_output_name(prefix: &str) -> String {
    if has_directory(prefix) {
        prefix.to_owned()
    } else {
        format!("./{prefix}")
    }
}

/// `THD_write_nifti`'s own check of the name it writes ([`afni_output_name`]): what
/// [`filename_ok`] accepts, not starting with `-`. A `-prefix` that passes the option scan only
/// through `filename_ok`'s `3dcalc` exception (`3dcalc a b.nii`, written as `./3dcalc a b.nii`),
/// or whose directory starts with `-`, fails here: AFNI then writes nothing, prints "Illegal
/// filename for NIfTI output", and 3dTshift exits with status 0 (AFNI-12).
fn output_name_ok(prefix: &str, settings: &Settings) -> bool {
    let name = afni_output_name(prefix);
    filename_ok(&name, settings) && !name.starts_with('-')
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

fn parse(
    args: &[String],
    mode: UpstreamDefects,
    settings: &Settings,
    io: &mut Io<'_>,
) -> Result<Options, Fail> {
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
        prefix_afni_ok: true,
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
                // AFNI: "illegal value '...' after -prefix" (larmorx accepts any path).
                o.prefix_afni_ok = filename_ok(&v, settings);
                if !o.prefix_afni_ok && mode.reproduce() {
                    io.notice(&Notice::new(
                        &defects::AFNI_12,
                        mode,
                        format!(
                            "the -prefix '{v}' has blanks, shell metacharacters or non-ASCII \
                             characters: refused, as AFNI does"
                        ),
                    ));
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

/// The output must be a NIfTI file (larmorx's conventions).
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

/// `AFNI_logger`: when `AFNI_ECHO_COMMANDLINE` starts with Y, AFNI echoes the command line
/// (`+++++===== 3dTshift ...`, after the global options were taken out; arguments
/// [`filename_ok`] rejects are quoted, their blanks, control and non-ASCII bytes made blanks)
/// once it has added it to `~/.afni.log`, which it does unless `AFNI_DONT_LOGFILE` is YES or
/// `HOME` is not set. larmorx-gpl writes no log; it echoes when `HOME` is a directory.
fn echo_command_line(pre: &Prefiltered, io: &mut Io<'_>) {
    let s = &pre.settings;
    if !s.yes("AFNI_ECHO_COMMANDLINE") || s.yes("AFNI_DONT_LOGFILE") {
        return;
    }
    if !s.get("HOME").is_some_and(|h| Path::new(h).is_dir()) {
        return;
    }
    let mut line = String::from("3dTshift");
    for arg in pre.args.iter().filter(|a| !a.is_empty()) {
        if filename_ok(arg, s) {
            line.push(' ');
            line.push_str(arg);
        } else {
            let cleaned: String = arg
                .bytes()
                .map(|b| {
                    if b.is_ascii_control() || is_space(b) || b >= 128 {
                        ' '
                    } else {
                        char::from(b)
                    }
                })
                .collect();
            line.push_str(&format!(" '{cleaned}'"));
        }
    }
    io.raw(format!("+++++===== {line}\n"));
}

/// What `AFNI_DECONFLICT` lets AFNI do with an output name that is already a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Deconflict {
    /// `OVERWRITE` exactly (what `-overwrite` sets): overwrite, with the warning "Over-writing
    /// dataset" unless `-overwrite` was given.
    Overwrite,
    /// Any other value starting with `O` or `o`: AFNI checks nothing and overwrites silently.
    OverwriteSilently,
    /// A value starting with `Y` or `y`: write `<name>_AA1.nii` (the first free of `_AA1` ...
    /// `_ZZ9`) instead, with a warning.
    Rename,
    /// Unset or anything else: the conflict is an error (AFNI-11).
    Refuse,
}

/// The output dataset: its name and what to do when it exists.
struct Output {
    /// The `-prefix` as given.
    prefix: String,
    /// The file to write.
    path: PathBuf,
    rule: Deconflict,
    /// `-overwrite` was given: no "Over-writing dataset" warning.
    quiet: bool,
    /// `THD_deathcon`: `AFNI_DECONFLICT` starts with an upper-case `N`, which stops a shift
    /// before the work when the output exists.
    deathcon: bool,
}

impl Output {
    fn new(prefix: &str, settings: &Settings, quiet: bool) -> Output {
        let value = settings.get("AFNI_DECONFLICT");
        let first = value.and_then(|v| v.bytes().next());
        let rule = if value == Some("OVERWRITE") {
            Deconflict::Overwrite
        } else {
            match first.map(|c| c.to_ascii_uppercase()) {
                Some(b'O') => Deconflict::OverwriteSilently,
                Some(b'Y') => Deconflict::Rename,
                _ => Deconflict::Refuse,
            }
        };
        Output {
            prefix: prefix.to_owned(),
            path: PathBuf::from(prefix),
            rule,
            quiet,
            deathcon: first == Some(b'N'),
        }
    }

    /// `THD_is_file(path)`: a file of the output's name exists.
    fn exists(&self) -> bool {
        self.path.is_file()
    }

    /// The output's name without its directory (`DSET_PREFIX`).
    fn file_name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(|| self.prefix.clone(), |n| n.to_string_lossy().into_owned())
    }

    /// The `-prefix` with its file name replaced by that of `path`.
    fn prefix_of(&self, path: &Path) -> String {
        let old = self.file_name();
        let new = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        match self.prefix.strip_suffix(&old) {
            Some(dir) => format!("{dir}{new}"),
            None => new,
        }
    }

    /// `THD_deconflict_nifti`: the first of `<name>_AA1<suffix>`, `_AA2`, ..., `_AB1`, ...,
    /// `_ZZ9` that is not a file (the suffix `.nii` or `.nii.gz`); `None` when all are (AFNI
    /// then appends a random identifier).
    fn deconflicted(&self) -> Option<PathBuf> {
        let name = self.file_name();
        let lower = name.to_ascii_lowercase();
        let cut = if lower.ends_with(".nii") {
            name.len() - 4
        } else if lower.ends_with(".nii.gz") {
            name.len() - 7
        } else {
            name.len()
        };
        let (stem, suffix) = name.split_at(cut);
        for a in b'A'..=b'Z' {
            for b in b'A'..=b'Z' {
                for c in b'1'..=b'9' {
                    let candidate = self.path.with_file_name(format!(
                        "{stem}_{}{}{}{suffix}",
                        char::from(a),
                        char::from(b),
                        char::from(c)
                    ));
                    if !candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    /// The checks before the work. `THD_deathcon` (a shift only, in both modes): with
    /// `AFNI_DECONFLICT` starting with `N`, an existing output stops the run ("output dataset
    /// already exists", exit 1). By default, an existing output that `AFNI_DECONFLICT` does
    /// not allow to overwrite or rename is an error (AFNI-11), where AFNI finds out after the
    /// work, writes nothing and exits with status 0.
    fn check(&self, shift: bool, notices: &Notices, io: &mut Io<'_>) -> Result<(), Fail> {
        if shift && self.deathcon && self.exists() {
            return Err(format!(
                "output dataset already exists '{}'",
                afni_output_name(&self.prefix)
            )
            .into());
        }
        if notices.fix() && self.rule == Deconflict::Refuse && self.path.exists() {
            io.notice(&Notice::new(
                &defects::AFNI_11,
                notices.mode(),
                "the output exists: an error (exit status 1); AFNI writes nothing and exits \
                 with status 0",
            ));
            return Err(format!(
                "output dataset name '{}' conflicts with existing file",
                self.path.display()
            )
            .into());
        }
        Ok(())
    }
}

/// Writes the output (`DSET_write`, `THD_write_3dim_dataset`), then names the upstream defects
/// collected in `notices`. Returns the name AFNI then gives the dataset (`DSET_BRIKNAME`),
/// which `-verbose` prints, even when nothing was written.
///
/// An existing output: overwritten, renamed or refused as `AFNI_DECONFLICT` says
/// ([`Deconflict`]); refused, AFNI writes nothing, prints two errors and the run ends with
/// status 0 (AFNI-11 reproduced), and the dataset keeps the free name AFNI found for it. With
/// AFNI-12 reproduced, a name that `THD_write_nifti` refuses ([`output_name_ok`]) is not
/// written either. A file that cannot be written is an error by default; AFNI ignores the
/// write's result and exits with status 0 (AFNI-26), which `reproduce` gives where AFNI's
/// writer cannot open the file ([`openable`]): its message, nothing written.
fn write(
    out: &Output,
    ds: &Dataset,
    n_threads: usize,
    settings: &Settings,
    notices: &mut Notices,
    io: &mut Io<'_>,
) -> Result<String, Fail> {
    let mut target = out.path.clone();
    let mut written = true;
    if out.exists() {
        match out.rule {
            Deconflict::Overwrite => {
                if !out.quiet {
                    io.warn(format!(
                        "Over-writing dataset {}",
                        afni_output_name(&out.prefix)
                    ));
                }
            }
            Deconflict::OverwriteSilently => {}
            Deconflict::Rename => {
                target = out.deconflicted().ok_or_else(|| {
                    Fail::Fatal(format!(
                        "no free name for the output beside '{}' (_AA1 to _ZZ9 all exist)",
                        out.prefix
                    ))
                })?;
                io.warn(format!(
                    "changed output dataset name from '{}' to '{}'",
                    out.file_name(),
                    target.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            Deconflict::Refuse => {
                if notices.fix() {
                    // Created during the work; checked before it otherwise.
                    out.check(false, notices, io)?;
                }
                if let Some(free) = out.deconflicted() {
                    target = free;
                }
                io.error(format!(
                    "output dataset name '{}' conflicts with existing file",
                    out.file_name()
                ));
                io.error("dataset NOT written to disk!");
                notices.add(
                    &defects::AFNI_11,
                    "the output exists: nothing is written and the exit status is 0, as AFNI \
                     does",
                );
                written = false;
            }
        }
    }
    let shown = afni_output_name(&out.prefix_of(&target));
    if written && notices.reproduce() && !output_name_ok(&out.prefix_of(&target), settings) {
        io.error(format!("Illegal filename for NIfTI output: {shown}"));
        notices.add(
            &defects::AFNI_12,
            format!(
                "the output name '{shown}' has blanks, shell metacharacters or non-ASCII \
                 characters, or starts with '-': nothing is written and the exit status is 0, \
                 as AFNI does"
            ),
        );
        written = false;
    }
    if written
        && notices.reproduce()
        && let Err(e) = openable(&target)
    {
        // `nifti_image_write_engine` (`nifti2_io.c:7908-7912`, `LNI_FERR`, a plain `fprintf`);
        // `THD_write_nifti` returns the failure and 3dTshift ignores it (`3dTshift.c:696`,
        // `:709`).
        io.raw(format!(
            "** ERROR (nifti_image_write_engine): cannot open output file '{shown}'\n"
        ));
        notices.add(
            &defects::AFNI_26,
            format!(
                "the output '{shown}' cannot be written ({e}): nothing is written and the exit \
                 status is 0, as AFNI does"
            ),
        );
        written = false;
    }
    if written {
        let mut messages = Vec::new();
        let result = replace_atomically(&target, |tmp| {
            dataset::write(tmp, ds, n_threads, settings, &mut messages, notices)
        });
        for m in messages {
            io.message(m);
        }
        if let Err(e) = result {
            if notices.fix() {
                io.notice(&Notice::new(
                    &defects::AFNI_26,
                    notices.mode(),
                    "the output cannot be written: an error (exit status 1); AFNI writes \
                     nothing and exits with status 0",
                ));
            }
            return Err(Fail::Fatal(format!("can't write output dataset: {e}")));
        }
    }
    for n in notices.as_slice() {
        io.notice(n);
    }
    Ok(shown)
}

/// AFNI-23 with `-rlt`, which removes the trend, intercept included, from every volume after
/// the first `ignore`: those keep the intercept `AFNI_NIFTI_SCALE=NO` dropped
/// ([`dataset::DroppedIntercept`]), the others no longer need it.
///
/// - Without `-ignore`, AFNI's output is right as it is: nothing changes and nothing is named.
/// - With `-ignore n`, one `scl_inter` cannot describe both kinds of volume. The default reads
///   the data as AFNI does without the variable (float32, `slope * x + inter`), which gives
///   AFNI's output without it; `reproduce` keeps AFNI's integers, whose `n` first volumes are
///   off by the intercept. Named in both modes.
fn rlt_dropped_intercept(ds: &mut Dataset, ignore: usize, notices: &mut Notices) {
    let Some(d) = ds.dropped_inter.as_mut() else {
        return;
    };
    if ignore == 0 {
        d.carried = false;
        return;
    }
    let d = *d;
    ds.dropped_inter = None;
    let (ty, slope, inter) = (d.type_name(), fmt_g(d.slope), fmt_g(d.inter));
    if notices.fix() {
        notices.add(
            &defects::AFNI_23,
            format!(
                "AFNI_NIFTI_SCALE=NO with -rlt and -ignore {ignore}: the ignored volumes keep \
                 scl_inter {inter} and the detrended ones lose it, which one scl_inter cannot \
                 describe, so the {ty} data are read as float32 with scl_slope {slope} and \
                 scl_inter {inter} applied, as without the variable; AFNI keeps them as {ty} \
                 and drops the intercept, so its {ignore} ignored volumes are off by {inter}"
            ),
        );
        ds.data = dataset::BrickData::Float(d.scaled(&ds.data));
        ds.factors.iter_mut().for_each(|f| *f = 0.0);
    } else {
        notices.add(
            &defects::AFNI_23,
            format!(
                "AFNI_NIFTI_SCALE=NO with -rlt and -ignore {ignore}: the {ty} data are kept as \
                 stored and scl_inter {inter} is dropped, as AFNI does: the {ignore} ignored \
                 volumes are off by {inter}"
            ),
        );
    }
}

/// `TS_copy_input_to_output()`: the dataset as read, written unchanged.
fn copy_input_to_output(
    ds: &Dataset,
    out: &Output,
    o: &Options,
    n_threads: usize,
    settings: &Settings,
    notices: &mut Notices,
    io: &mut Io<'_>,
) -> Result<(), Fail> {
    io.warn("==>> output dataset is just a copy of input dataset");
    out.check(false, notices, io)?;
    let shown = write(out, ds, n_threads, settings, notices, io)?;
    if o.verbose > 0 {
        io.info(format!("Wrote output: {shown}"));
    }
    Ok(())
}

/// `THD_open_dataset` for 3dTshift's input, with the messages AFNI prints while opening it.
/// A name with a blank is a list of datasets to join in time unless `AFNI_PATH_SPACES_OK` is
/// YES (AFNI-18): reproduced, each is opened ([`open_list`]); by default the name is one file.
/// A name starting with `~/` starts in `HOME`.
fn open_dataset(
    arg: &str,
    n_threads: usize,
    settings: &Settings,
    messages: &mut Vec<Message>,
    notices: &mut Notices,
) -> Result<Dataset, ReadError> {
    let listed = arg.contains(' ')
        && !settings.yes("AFNI_PATH_SPACES_OK")
        && !["[count ", "[count_afni ", "[1dcat "]
            .iter()
            .any(|s| arg.contains(s));
    if listed {
        if notices.fix() {
            notices.add(
                &defects::AFNI_18,
                format!(
                    "the input name '{arg}' has a blank: read as one file; AFNI reads it as a \
                     list of datasets separated by blanks (unless AFNI_PATH_SPACES_OK is YES)"
                ),
            );
        } else {
            notices.add(
                &defects::AFNI_18,
                format!(
                    "the input name '{arg}' has a blank: read as a list of datasets separated \
                     by blanks, as AFNI does"
                ),
            );
            return open_list(arg, n_threads, settings, messages, notices);
        }
    }
    let name = match (arg.strip_prefix("~/"), settings.get("HOME")) {
        (Some(rest), Some(home)) => format!("{home}/{rest}"),
        _ => arg.to_owned(),
    };
    dataset::read(&name, n_threads, settings, messages, notices)
}

/// `THD_open_tcat(list)`: the datasets named in `list` (separated by blanks or `~`), each
/// opened as an input; one that cannot be opened is reported, and then the list cannot be
/// opened. A list of one dataset is that dataset; joining several in time is not supported.
fn open_list(
    list: &str,
    n_threads: usize,
    settings: &Settings,
    messages: &mut Vec<Message>,
    notices: &mut Notices,
) -> Result<Dataset, ReadError> {
    let names = decode_string_list(list, b"~");
    let mut opened = Vec::new();
    let mut failed = false;
    for name in &names {
        match open_dataset(name, n_threads, settings, messages, notices) {
            Ok(ds) => opened.push(ds),
            Err(_) => {
                messages.push(Message::Warning(format!(
                    "THD_open_tcat: can't open dataset {name}\n"
                )));
                failed = true;
            }
        }
    }
    if failed || opened.is_empty() {
        return Err(ReadError::Afni(format!(
            "{list}: cannot open every dataset of the list"
        )));
    }
    if opened.len() == 1 {
        return Ok(opened.pop().expect("one dataset"));
    }
    Err(ReadError::Unsupported(format!(
        "{list}: a list of datasets to join in time"
    )))
}

fn run(pre: &Prefiltered, mode: UpstreamDefects, io: &mut Io<'_>) -> Result<(), Fail> {
    let settings = &pre.settings;
    let mut o = parse(&pre.args, mode, settings, io)?;
    let path = output_path(&o.prefix)?;
    let out = Output::new(&o.prefix, settings, pre.quiet_overwrite);
    debug_assert_eq!(out.path, path);
    let n_threads = threads();
    let mut notices = Notices::new(mode);
    notices.extend(pre.notices.iter().cloned());
    if !o.prefix_afni_ok {
        notices.add(
            &defects::AFNI_12,
            format!(
                "the -prefix '{}' has blanks, shell metacharacters or non-ASCII characters: \
                 accepted; AFNI refuses it",
                o.prefix
            ),
        );
    } else if mode.fix() && !output_name_ok(&o.prefix, settings) {
        notices.add(
            &defects::AFNI_12,
            format!(
                "the output name '{}' has blanks, shell metacharacters or non-ASCII characters, \
                 or starts with '-': written; AFNI accepts the -prefix but writes nothing (exit \
                 status 0)",
                afni_output_name(&o.prefix)
            ),
        );
    }

    // Open the dataset; extract values, check for errors.
    if o.verbose > 0 {
        io.info(IMPLEMENTATION);
        io.info(format!("upstream defects: {mode} ({})", defects::ENV));
        io.print("++ opening input dataset header");
    }
    let mut messages = Vec::new();
    let read = open_dataset(&o.dataset, n_threads, settings, &mut messages, &mut notices);
    for m in messages {
        io.message(m);
    }
    let mut ds = read.map_err(|e| {
        let arg = o.dataset.as_str();
        match (mode.fix(), &e) {
            // AFNI says only this; larmorx's reasons are its own.
            (false, ReadError::Nifti(_) | ReadError::Afni(_)) => {
                Fail::Fatal(format!("Can't open input dataset '{arg}'"))
            }
            _ => Fail::Fatal(format!("Can't open input dataset '{arg}': {e}")),
        }
    })?;
    let [_, _, nzz] = ds.nxyz;
    let ntt = ds.nvals;

    if ntt < 2 {
        io.warn("Input dataset has only 1 value per voxel!");
        return copy_input_to_output(&ds, &out, &o, n_threads, settings, &mut notices, io);
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
            return copy_input_to_output(&ds, &out, &o, n_threads, settings, &mut notices, io);
        }
    } else if o.tpattern.is_none() && header_offsets.is_none() {
        io.warn("dataset is already aligned in time!");
        return copy_input_to_output(&ds, &out, &o, n_threads, settings, &mut notices, io);
    }
    let tpat = match &o.tpattern {
        Some(p) => {
            let mut messages = Vec::new();
            let parsed = parse_tpattern(nzz, o.tr, p, settings, &mut messages);
            for m in messages {
                io.message(m);
            }
            parsed.map_err(|e| match e {
                TpatternError::Unknown(_) => Fail::Error(e.to_string()),
                _ => Fail::Fatal(e.to_string()),
            })?
        }
        None => {
            let offsets = header_offsets.expect("checked above");
            if offsets.len() != nzz {
                io.warn("dataset temporal pattern is malformed!");
                return copy_input_to_output(&ds, &out, &o, n_threads, settings, &mut notices, io);
            }
            if let Some((start, end)) = ds.seq_dec {
                // Corrected, the times can exceed the TR: then the output is a copy (below).
                let copy = offsets.iter().any(|&t| t < 0.0 || t > o.tr);
                notices.add(
                    &defects::AFNI_03,
                    match (mode.fix(), copy) {
                        (true, false) => format!(
                            "header slice code 2 (sequential decreasing) orders slices \
                             {end}..{start} from the top; AFNI gives every slice time 0 and \
                             shifts nothing"
                        ),
                        (true, true) => format!(
                            "header slice code 2 (sequential decreasing) orders slices \
                             {end}..{start} from the top, and their times exceed the TR: the \
                             output is a copy of the input that keeps them; AFNI gives every \
                             slice time 0, shifts nothing and writes no slice timing"
                        ),
                        (false, _) => "header slice code 2 (sequential decreasing) gives every \
                                       slice time 0, so nothing is shifted, as AFNI does"
                            .to_owned(),
                    },
                );
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
        return copy_input_to_output(&ds, &out, &o, n_threads, settings, &mut notices, io);
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
        return copy_input_to_output(&ds, &out, &o, n_threads, settings, &mut notices, io);
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
    out.check(true, &notices, io)?;

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
        upstream_defects: mode,
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
    if o.rlt == Rlt::Nothing {
        rlt_dropped_intercept(&mut ds, params.ignore, &mut notices);
    }
    let shift_notices = tshift(&mut ds, &params).map_err(|e| Fail::Fatal(e.to_string()))?;
    notices.extend(shift_notices);
    let shown = write(&out, &ds, n_threads, settings, &mut notices, io)?;
    if o.verbose > 0 {
        io.raw(format!("++ Wrote output: {shown}\n"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Options, String> {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let settings = Settings::empty();
        let mut io = Io::new(&mut out, &mut err, &settings, UpstreamDefects::Fix);
        parse(&args, UpstreamDefects::Fix, &settings, &mut io).map_err(|e| match e {
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
        // AFNI refuses this -prefix; larmorx accepts any path, and notes what AFNI would do.
        let o = parse_args(&["-prefix", "a b.nii", "x"]).unwrap();
        assert!(!o.prefix_afni_ok && o.prefix == "a b.nii");
        assert!(
            parse_args(&["-prefix", "a.nii", "x"])
                .unwrap()
                .prefix_afni_ok
        );
        let none = Settings::empty();
        // Names that pass the option scan only through the `3dcalc` exception, or with a
        // directory starting with '-', are refused when AFNI writes them.
        assert!(filename_ok("3dcalc a b.nii", &none) && !output_name_ok("3dcalc a b.nii", &none));
        assert!(output_name_ok("3dcalc/x y.nii", &none) && output_name_ok("3dcalcok.nii", &none));
        assert!(filename_ok("-d/out.nii", &none) && !output_name_ok("-d/out.nii", &none));
        assert_eq!(afni_output_name("a.nii"), "./a.nii");
        assert!(output_name_ok("a.nii", &none) && output_name_ok("/tmp/x/a.nii", &none));
        // AFNI_ALLOW_ARBITRARY_FILENAMES accepts any name, but not one starting with '-'.
        let mut any = Settings::empty();
        any.set("AFNI_ALLOW_ARBITRARY_FILENAMES", "Yes");
        assert!(filename_ok("a b.nii", &any) && output_name_ok("3dcalc a b.nii", &any));
        assert!(!output_name_ok("-d/out.nii", &any));
        assert!(parse_args(&["-bogus", "x"]).is_err());
        assert!(parse_args(&["-TR"]).is_err());
        assert!(parse_args(&["-linear"]).is_err());
        assert_eq!(
            parse_args(&["x", "-linear"]).unwrap().method,
            Method::Fourier
        );
    }

    #[test]
    fn deconflict_rules() {
        let rule = |v: Option<&str>| {
            let mut s = Settings::empty();
            if let Some(v) = v {
                s.set("AFNI_DECONFLICT", v);
            }
            let o = Output::new("out.nii", &s, false);
            (o.rule, o.deathcon)
        };
        assert_eq!(rule(None), (Deconflict::Refuse, false));
        assert_eq!(rule(Some("OVERWRITE")), (Deconflict::Overwrite, false));
        assert_eq!(
            rule(Some("overwrite")),
            (Deconflict::OverwriteSilently, false)
        );
        assert_eq!(rule(Some("O")), (Deconflict::OverwriteSilently, false));
        assert_eq!(rule(Some("yes")), (Deconflict::Rename, false));
        assert_eq!(rule(Some("NO")), (Deconflict::Refuse, true));
        assert_eq!(rule(Some("no")), (Deconflict::Refuse, false));
        assert_eq!(rule(Some("")), (Deconflict::Refuse, false));
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("été.nii.gz").display().to_string();
        let o = Output::new(&prefix, &Settings::empty(), false);
        std::fs::write(&o.path, "x").unwrap();
        let free = o.deconflicted().unwrap();
        assert_eq!(free.file_name().unwrap(), "été_AA1.nii.gz");
        std::fs::write(&free, "x").unwrap();
        assert_eq!(
            o.deconflicted().unwrap().file_name().unwrap(),
            "été_AA2.nii.gz"
        );
        assert!(o.prefix_of(&free).ends_with("été_AA1.nii.gz"));
    }
}
