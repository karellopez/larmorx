// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05 src/afni_environ.c (AFNI_prefilter_args: the -Dname=val,
// -overwrite, -ok_1D_text and -skip_afnirc options; AFNI_setenv; AFNI_yesenv, AFNI_noenv,
// AFNI_numenv) and src/afni_environ.h (YESSISH, NOISH). Major portions Copyright (C) 1994-2000
// Medical College of Wisconsin (AFNI, Robert W. Cox et al.), released by AFNI 26.3.05 under the
// Creative Commons Attribution 4.0 International licence (CC BY 4.0).
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05 (first
// translated from AFNI_25.2.09).
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! AFNI's environment as 3dTshift reads it: a settings table, and AFNI's global options.
//!
//! AFNI programs read many environment variables (`AFNI_DECONFLICT`, `AFNI_NIFTI_PRIORITY`,
//! ...), and take *global options* out of the command line before their own option scan,
//! wherever they stand: `-Dname=value` sets a variable for the run, `-overwrite` sets
//! `AFNI_DECONFLICT=OVERWRITE`. AFNI does it by changing its process environment (`putenv`).
//! larmorx never changes its process environment (`std::env::set_var` is unsafe in a
//! multithreaded program); it builds a [`Settings`] table instead and passes it to every
//! routine that reads a variable:
//!
//! 1. the process environment is read into the table once, at the start ([`Settings::from_env`]);
//! 2. the global options ([`prefilter`]) are applied to it in command-line order, so a
//!    `-Dname=value` wins over the environment and a later one over an earlier one, as in AFNI;
//! 3. the routines look variables up in the table with AFNI's tests ([`Settings::yes`],
//!    [`Settings::no`], [`Settings::num`]).
//!
//! Names are case-sensitive, as on Unix, except that on Windows, where the system itself does
//! not distinguish `afni_deconflict` from `AFNI_DECONFLICT`, a variable of the process
//! environment is found whatever the case of its name; `-D` names are always taken as given.
//!
//! AFNI also reads `~/.afnirc` (and `AFNI_SYSTEM_AFNIRC`) for variables not set otherwise;
//! larmorx reads no such file, so `reproduce` is AFNI without one (`-skip_afnirc`).
//!
//! [`CONSULTED`] lists every variable AFNI 26.3.05's 3dTshift consults on the paths larmorx
//! supports (NIfTI input and output), with what larmorx does with it; a `-D` of any other
//! variable is accepted with a notice that it has no effect.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use larmorx_core::defects::{self, Notice, UpstreamDefects};

use crate::cnum::{is_space, strtod};

/// What larmorx does with a variable AFNI's 3dTshift consults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// Its effect on the output, the exit status or the messages is reproduced.
    Applied,
    /// It changes nothing larmorx writes or prints, for the reason given.
    NoEffect(&'static str),
}

const EXTENSION: &str = "it concerns AFNI's header extension, which larmorx never writes";
const DEBUG: &str = "it turns on AFNI's own debugging output, which larmorx does not reproduce";
const AFNIRC: &str = "it concerns AFNI's .afnirc files, which larmorx does not read";
const AFNI_FORMAT: &str =
    "it concerns AFNI's own dataset format (a -prefix without .nii), which larmorx does not write";
const ATLAS: &str = "it tells AFNI where to find AFNI_atlas_spaces.niml; larmorx uses the table of \
                     template spaces of AFNI 26.3.05's own file";

/// Every environment variable AFNI 26.3.05's 3dTshift consults on the paths larmorx
/// supports: the program, and the library routines it calls to read NIfTI files, 1D slice-time
/// files and `-tpattern` strings, to check output names, to write NIfTI files and to print
/// messages. Found by reading the source and confirmed by tracing the oracle's `getenv` calls
/// (`docs/findings/afni-tshift.md`, "Environment variables").
pub const CONSULTED: &[(&str, Use)] = &[
    // Output names and existing outputs.
    ("AFNI_DECONFLICT", Use::Applied),
    ("AFNI_ALLOW_ARBITRARY_FILENAMES", Use::Applied),
    // Reading the input.
    ("AFNI_PATH_SPACES_OK", Use::Applied),
    ("AFNI_NOREALPATH", Use::Applied),
    ("HOME", Use::Applied),
    ("AFNI_NIFTI_PRIORITY", Use::Applied),
    ("AFNI_NIFTI_SCALE", Use::Applied),
    ("AFNI_NIFTI_TYPE_WARN", Use::Applied),
    ("AFNI_NIFTI_VIEW", Use::Applied),
    ("AFNI_DEFAULT_STD_SPACE", Use::Applied),
    ("AFNI_NO_OBLIQUE_WARNING", Use::Applied),
    // Slice-time files.
    ("AFNI_1D_ZERO_TEXT", Use::Applied),
    ("AFNI_USE_FGETS", Use::Applied),
    // Writing.
    ("AFNI_NIFTI_WRITE_TYPE", Use::Applied),
    ("AFNI_ALLOW_MILLISECONDS", Use::Applied),
    (
        "AFNI_WRITE_1D_AS_PREFIX",
        Use::NoEffect(
            "larmorx always writes NIfTI, as AFNI does with it set (AFNI writes a dataset of \
             one row as a 1D text file otherwise, AFNI-20)",
        ),
    ),
    // Messages.
    ("AFNI_MESSAGE_PREFIX", Use::Applied),
    ("AFNI_MESSAGE_COLORIZE", Use::Applied),
    ("AFNI_ECHO_COMMANDLINE", Use::Applied),
    (
        "AFNI_DONT_LOGFILE",
        Use::Applied, // it decides AFNI_ECHO_COMMANDLINE; larmorx writes no ~/.afni.log
    ),
    (
        "AFNI_QUIET_STARTUP",
        Use::NoEffect("it hides AFNI's version banner, which larmorx does not print"),
    ),
    // No effect on what larmorx writes or prints.
    ("AFNI_NIFTI_NOEXT", Use::NoEffect(EXTENSION)),
    ("AFNI_HISTDB_SCRIPT", Use::NoEffect(EXTENSION)),
    ("AFNI_HISTORY_NAME", Use::NoEffect(EXTENSION)),
    ("AFNI_HISTORY_USERNAME", Use::NoEffect(EXTENSION)),
    ("AFNI_INCLUDE_HISTORY", Use::NoEffect(EXTENSION)),
    ("IDCODE_PREFIX", Use::NoEffect(EXTENSION)),
    ("AFNI_INDEX_PREFIX", Use::NoEffect(EXTENSION)),
    ("AFNI_TRACE", Use::NoEffect(DEBUG)),
    ("AFNI_TRACE_FILE", Use::NoEffect(DEBUG)),
    ("AFNI_NIFTI_DEBUG", Use::NoEffect(DEBUG)),
    ("NIML_DEBUG", Use::NoEffect(DEBUG)),
    ("AFNI_WAMI_DEBUG", Use::NoEffect(DEBUG)),
    // Read only when a -D sets it (AFNI_setenv turns tracing on for `y` or `Y`).
    ("AFNI_DEBUG", Use::NoEffect(DEBUG)),
    ("AFNI_SYSTEM_AFNIRC", Use::NoEffect(AFNIRC)),
    ("AFNI_ENVIRON_RESET", Use::NoEffect(AFNIRC)),
    ("AFNI_ENVIRON_WARNINGS", Use::NoEffect(AFNIRC)),
    (
        "AFNI_RANDOM_SEEDVAL",
        Use::NoEffect("it seeds AFNI's random dataset identifiers, which larmorx does not write"),
    ),
    (
        "AFNI_NOMMAP",
        Use::NoEffect("it changes how AFNI's own format is loaded, not the values"),
    ),
    (
        "AFNI_FLOATSCAN",
        Use::NoEffect("AFNI scans NIfTI float data for non-finite values whatever its value"),
    ),
    (
        "AFNI_BYTEORDER",
        Use::NoEffect("it applies to AFNI's own format; NIfTI files are written in native order"),
    ),
    (
        "AFNI_NIML_BUFSIZE",
        Use::NoEffect("a buffer size of AFNI's NIML reader"),
    ),
    (
        "NIML_BUFSIZE",
        Use::NoEffect("a buffer size of AFNI's NIML reader"),
    ),
    (
        "BUFSIZE",
        Use::NoEffect("a buffer size of AFNI's NIML reader"),
    ),
    ("AFNI_ATLAS_PATH", Use::NoEffect(ATLAS)),
    ("AFNI_PLUGINPATH", Use::NoEffect(ATLAS)),
    ("AFNI_PLUGIN_PATH", Use::NoEffect(ATLAS)),
    ("AFNI_SUPP_ATLAS", Use::NoEffect(ATLAS)),
    ("AFNI_SUPP_ATLAS_DIR", Use::NoEffect(ATLAS)),
    ("AFNI_LOCAL_ATLAS", Use::NoEffect(ATLAS)),
    ("PATH", Use::NoEffect(ATLAS)),
    (
        "AFNI_USE_THD_open_dataset",
        Use::NoEffect("it applies to sub-brick selectors, which larmorx does not support"),
    ),
    (
        "AFNI_TCAT_SELECTORS",
        Use::NoEffect(
            "it applies to datasets concatenated from a list, which larmorx does not support",
        ),
    ),
    (
        "AFNI_TCAT_SELECTORS_VERB",
        Use::NoEffect(
            "it applies to datasets concatenated from a list, which larmorx does not support",
        ),
    ),
    (
        "AFNI_1D_TRANOUT",
        Use::NoEffect("it applies to 1D text output, which larmorx does not write"),
    ),
    (
        "AFNI_3D_BINARY",
        Use::NoEffect("it applies to 1D text output, which larmorx does not write"),
    ),
    ("AFNI_AUTOGZIP", Use::NoEffect(AFNI_FORMAT)),
    ("AFNI_COMPRESSOR", Use::NoEffect(AFNI_FORMAT)),
    ("AFNI_DONT_USE_PIGZ", Use::NoEffect(AFNI_FORMAT)),
    ("AFNI_WRITE_NIML", Use::NoEffect(AFNI_FORMAT)),
    ("AFNI_GLOB_DEBUG", Use::NoEffect(AFNI_FORMAT)),
    ("AFNI_GLOB_SELECTORS", Use::NoEffect(AFNI_FORMAT)),
    ("AFNI_SHELL_GLOB", Use::NoEffect(AFNI_FORMAT)),
    ("SKIP_COMPRESS_SETUP", Use::NoEffect(AFNI_FORMAT)),
];

/// What larmorx does with `name`, if AFNI's 3dTshift consults it.
pub fn consulted(name: &str) -> Option<Use> {
    TSHIFT.consulted(name)
}

/// An AFNI program's environment: the variables it consults, as differences from 3dTshift's
/// [`CONSULTED`] (the dataset reading and writing, the output names and the messages are the
/// same library code). Found by reading each program's source and confirmed by tracing the
/// oracle's `getenv` calls (`docs/findings/afni-automask.md`, `afni-unifize.md`, "Environment
/// variables").
#[derive(Debug)]
pub struct Program {
    /// The program's name, for messages.
    pub name: &'static str,
    /// Variables it consults that 3dTshift does not, or that do something else for it; they
    /// win over [`CONSULTED`].
    pub extra: &'static [(&'static str, Use)],
    /// Variables of [`CONSULTED`] it does not consult.
    pub without: &'static [&'static str],
}

impl Program {
    /// What larmorx does with `name`, if the program consults it.
    pub fn consulted(&self, name: &str) -> Option<Use> {
        if let Some(&(_, u)) = self.extra.iter().find(|(n, _)| *n == name) {
            return Some(u);
        }
        if self.without.contains(&name) {
            return None;
        }
        CONSULTED.iter().find(|(n, _)| *n == name).map(|&(_, u)| u)
    }

    /// Every variable it consults.
    pub fn variables(&self) -> Vec<(&'static str, Use)> {
        let mut all: Vec<(&'static str, Use)> = CONSULTED
            .iter()
            .filter(|(n, _)| !self.without.contains(n) && !self.extra.iter().any(|(e, _)| e == n))
            .copied()
            .collect();
        all.extend_from_slice(self.extra);
        all
    }
}

/// 3dTshift: [`CONSULTED`].
pub const TSHIFT: Program = Program {
    name: "3dTshift",
    extra: &[],
    without: &[],
};

const NO_TEXT_FILES: &str =
    "it changes how AFNI reads text files, which this program reads none of";
const QUIET: Use = Use::Applied; // AFNI_QUIET_STARTUP hides "Authored by" and "Output dataset"

/// 3dAutomask: 3dTshift's variables, without the slice-time files'; `AFNI_QUIET_STARTUP` hides
/// its author line and its "Output dataset" messages (`AUTHOR`, `WROTE_DSET`).
pub const AUTOMASK: Program = Program {
    name: "3dAutomask",
    extra: &[
        ("AFNI_QUIET_STARTUP", QUIET),
        ("AFNI_USE_FGETS", Use::NoEffect(NO_TEXT_FILES)),
    ],
    without: &["AFNI_1D_ZERO_TEXT"],
};

const OPENMP: &str = "a setting of the OpenMP runtime: larmorx's threads come from \
                      OMP_NUM_THREADS, and its results do not depend on them";

/// 3dUnifize: 3dAutomask's variables, and the OpenMP runtime's, which its voxel loop runs
/// under (`OMP_NUM_THREADS`, read by `AFNI_SETUP_OMP` before the global options, so a `-D`
/// cannot set it).
pub const UNIFIZE: Program = Program {
    name: "3dUnifize",
    extra: &[
        ("AFNI_QUIET_STARTUP", QUIET),
        ("AFNI_USE_FGETS", Use::NoEffect(NO_TEXT_FILES)),
        (
            "OMP_NUM_THREADS",
            Use::NoEffect(
                "it sets the number of threads, which changes nothing in larmorx's results \
                 (it is read from the process environment, before the global options)",
            ),
        ),
        ("OMP_DYNAMIC", Use::NoEffect(OPENMP)),
        ("OMP_NESTED", Use::NoEffect(OPENMP)),
        ("OMP_SCHEDULE", Use::NoEffect(OPENMP)),
        ("OMP_STACKSIZE", Use::NoEffect(OPENMP)),
        ("OMP_WAIT_POLICY", Use::NoEffect(OPENMP)),
        ("OMP_THREAD_LIMIT", Use::NoEffect(OPENMP)),
        ("OMP_MAX_ACTIVE_LEVELS", Use::NoEffect(OPENMP)),
        ("OMP_CANCELLATION", Use::NoEffect(OPENMP)),
        ("OMP_DEFAULT_DEVICE", Use::NoEffect(OPENMP)),
        ("OMP_MAX_TASK_PRIORITY", Use::NoEffect(OPENMP)),
        ("OMP_PROC_BIND", Use::NoEffect(OPENMP)),
        ("OMP_PLACES", Use::NoEffect(OPENMP)),
        ("OMP_DISPLAY_ENV", Use::NoEffect(OPENMP)),
        ("OMP_DISPLAY_AFFINITY", Use::NoEffect(OPENMP)),
        ("OMP_AFFINITY_FORMAT", Use::NoEffect(OPENMP)),
        ("OMP_ALLOCATOR", Use::NoEffect(OPENMP)),
        ("OMP_TARGET_OFFLOAD", Use::NoEffect(OPENMP)),
        ("OMP_NUM_TEAMS", Use::NoEffect(OPENMP)),
        ("OMP_TEAMS_THREAD_LIMIT", Use::NoEffect(OPENMP)),
        ("GOMP_CPU_AFFINITY", Use::NoEffect(OPENMP)),
        ("GOMP_SPINCOUNT", Use::NoEffect(OPENMP)),
        ("GOMP_STACKSIZE", Use::NoEffect(OPENMP)),
        ("GOMP_OPENACC_DIM", Use::NoEffect(OPENMP)),
        ("ACC_DEVICE_NUM", Use::NoEffect(OPENMP)),
        ("ACC_DEVICE_TYPE", Use::NoEffect(OPENMP)),
    ],
    without: &["AFNI_1D_ZERO_TEXT"],
};

/// `YESSISH(s)`: the value starts with `Y` or `y`.
pub fn yessish(value: Option<&str>) -> bool {
    value.is_some_and(|v| matches!(v.as_bytes().first(), Some(b'Y' | b'y')))
}

/// `NOISH(s)`: the value starts with `N` or `n`.
pub fn noish(value: Option<&str>) -> bool {
    value.is_some_and(|v| matches!(v.as_bytes().first(), Some(b'N' | b'n')))
}

/// The environment 3dTshift sees: the process environment, with the command line's
/// `-Dname=value` settings on top. Never written back to the process.
///
/// It also holds the run's working directory ([`Settings::with_dir`]), where relative file
/// names start: the process's own unless one is given (a run inside the caller's process,
/// `cli::run_in`, which must not change the process's directory).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// The process environment, by name as the system gives it.
    env: BTreeMap<String, String>,
    /// The command line's settings (`-Dname=value`, `-overwrite`, `-ok_1D_text`), in order:
    /// later ones replace earlier ones.
    defined: BTreeMap<String, String>,
    /// The directory relative file names start in; `None`: the process's.
    dir: Option<PathBuf>,
}

impl Settings {
    /// An empty table (no environment).
    pub fn empty() -> Settings {
        Settings::default()
    }

    /// The process environment, read once. Names and values that are not valid Unicode are
    /// read with their invalid bytes replaced (no variable 3dTshift reads needs them).
    pub fn from_env() -> Settings {
        Settings::from_vars(std::env::vars_os())
    }

    /// A table from `(name, value)` pairs, as the process environment would give them.
    pub fn from_vars(vars: impl IntoIterator<Item = (OsString, OsString)>) -> Settings {
        let env = vars
            .into_iter()
            .map(|(n, v)| {
                (
                    n.to_string_lossy().into_owned(),
                    v.to_string_lossy().into_owned(),
                )
            })
            .collect();
        Settings {
            env,
            defined: BTreeMap::new(),
            dir: None,
        }
    }

    /// The same table, with relative file names starting in `dir` (a run's own working
    /// directory) instead of the process's.
    pub fn with_dir(mut self, dir: impl Into<PathBuf>) -> Settings {
        self.dir = Some(dir.into());
        self
    }

    /// The file a name from the command line refers to: `name` itself when it is absolute or
    /// no directory was given ([`Settings::with_dir`]), else `name` in that directory.
    pub fn path(&self, name: &str) -> PathBuf {
        match &self.dir {
            Some(dir) if Path::new(name).is_relative() => dir.join(name),
            _ => PathBuf::from(name),
        }
    }

    /// The process environment's value of `name`, without the command line's settings: what
    /// a program reads before AFNI's global options are applied (`OMP_NUM_THREADS`).
    pub fn environment(&self, name: &str) -> Option<&str> {
        self.env.get(name).map(String::as_str)
    }

    /// Sets `name` for this run, as `-Dname=value` does (`putenv`).
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.defined.insert(name.into(), value.into());
    }

    /// The value of `name` (`getenv`): the command line's, else the environment's. On Windows
    /// the environment's names are compared without regard to case, as the system does.
    pub fn get(&self, name: &str) -> Option<&str> {
        if let Some(v) = self.defined.get(name) {
            return Some(v);
        }
        if let Some(v) = self.env.get(name) {
            return Some(v);
        }
        if cfg!(windows) {
            return self
                .env
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str());
        }
        None
    }

    /// `AFNI_yesenv(name)`: the value starts with `Y` or `y`.
    pub fn yes(&self, name: &str) -> bool {
        yessish(self.get(name))
    }

    /// `AFNI_noenv(name)`: the value starts with `N` or `n`.
    pub fn no(&self, name: &str) -> bool {
        noish(self.get(name))
    }

    /// `AFNI_numenv(name)`: the value read with `strtod`, times 1024, 1024² or 1024³ when the
    /// number is followed by `k`, `m` or `g` (any case); 0 when unset or not a number.
    pub fn num(&self, name: &str) -> f64 {
        let Some(v) = self.get(name) else {
            return 0.0;
        };
        let (val, used) = strtod(v);
        match v.as_bytes().get(used) {
            Some(b'k' | b'K') => val * 1024.0,
            Some(b'm' | b'M') => val * 1024.0 * 1024.0,
            Some(b'g' | b'G') => val * 1024.0 * 1024.0 * 1024.0,
            _ => val,
        }
    }

    /// The names set on the command line, in name order.
    pub fn defined(&self) -> impl Iterator<Item = (&str, &str)> {
        self.defined.iter().map(|(n, v)| (n.as_str(), v.as_str()))
    }
}

/// `sscanf(s, "%Ns")`: skips white space, then reads at most `max` other characters. Returns
/// the token and where reading stopped; `None` if there is no token.
fn scan_word(s: &[u8], from: usize, max: usize) -> Option<(Vec<u8>, usize)> {
    let mut i = from;
    while i < s.len() && is_space(s[i]) {
        i += 1;
    }
    let start = i;
    while i < s.len() && !is_space(s[i]) && i - start < max {
        i += 1;
    }
    (i > start).then(|| (s[start..i].to_vec(), i))
}

/// `sscanf(cmd, "%255s %1023s", nam, val)`, with `nam` and `val` starting empty: the name and
/// the value it reads (either may stay empty).
fn scan_name_value(cmd: &[u8]) -> (Vec<u8>, Vec<u8>) {
    match scan_word(cmd, 0, 255) {
        None => (Vec::new(), Vec::new()),
        Some((nam, next)) => {
            let val = scan_word(cmd, next, 1023).map_or_else(Vec::new, |(v, _)| v);
            (nam, val)
        }
    }
}

/// `AFNI_setenv(cmd)` for `-D<cmd>`: the variable it sets and its value, or `None` when it sets
/// nothing. AFNI reads `cmd` as two blank-separated words (`name value`), and only if one of
/// them is missing replaces the first `=` by a blank and reads again; it then sets the
/// variable `name=value` with `putenv`. So `-DX=a b` sets `X` to `a=b`, `-DX= Y` sets `X` to
/// `=Y`, a name is cut at 255 characters and a value at 1023, and `-DX=`, `-D=Y` and a
/// command shorter than 3 characters set nothing.
pub fn afni_setenv(cmd: &str) -> Option<(String, String)> {
    let b = cmd.as_bytes();
    if b.len() < 3 {
        return None;
    }
    let (mut nam, mut val) = scan_name_value(b);
    if (nam.is_empty() || val.is_empty()) && b.contains(&b'=') {
        let mut ccc = b.to_vec();
        let eq = ccc.iter().position(|&c| c == b'=').expect("checked above");
        ccc[eq] = b' ';
        // sscanf assigns only what it reads: a value it does not find stays as it was ("").
        let (n2, v2) = scan_name_value(&ccc);
        nam = n2;
        if !v2.is_empty() {
            val = v2;
        }
    }
    if nam.is_empty() || val.is_empty() {
        return None;
    }
    // putenv("nam=val"): the name runs to the first '='.
    let mut eqn = nam;
    eqn.push(b'=');
    eqn.extend_from_slice(&val);
    let eqn = String::from_utf8_lossy(&eqn).into_owned();
    let (name, value) = eqn.split_once('=').expect("an '=' was added");
    (!name.is_empty()).then(|| (name.to_owned(), value.to_owned()))
}

/// What larmorx reads from `-D<cmd>` by default: the name up to the first `=` and the value
/// after it, as written (blanks included). `None` when the name or the value is empty.
pub fn plain_setenv(cmd: &str) -> Option<(String, String)> {
    let (name, value) = cmd.split_once('=')?;
    (!name.is_empty() && !value.is_empty()).then(|| (name.to_owned(), value.to_owned()))
}

/// The command line after AFNI's global options were taken out ([`prefilter`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Prefiltered {
    /// The remaining arguments, in order: what 3dTshift's own option scan reads.
    pub args: Vec<String>,
    /// The environment with the global options applied.
    pub settings: Settings,
    /// `-overwrite` was given: AFNI then overwrites without its "Over-writing dataset" warning.
    pub quiet_overwrite: bool,
    /// Upstream-defect notices about the global options (AFNI-22).
    pub notices: Vec<Notice>,
    /// larmorx's own warnings about them (a `-D` that sets nothing, or a variable AFNI does
    /// not read).
    pub warnings: Vec<String>,
}

/// `AFNI_prefilter_args`: takes AFNI's global options out of `args` (the arguments after the
/// program's name), wherever they stand, even as the value of another option, and applies them
/// to `settings` in order:
///
/// - `-Dname=value` (any argument that starts with `-D` and contains `=`): sets `name`. In
///   `reproduce` mode as AFNI parses it ([`afni_setenv`]); by default the value is the text
///   after the first `=`, as written ([`plain_setenv`]), and AFNI-22 is named when AFNI's
///   reading would have changed what a variable 3dTshift reads does ([`same_effect`]). One that
///   sets nothing is dropped, as AFNI drops it (with a warning by default);
/// - `-overwrite`: `AFNI_DECONFLICT=OVERWRITE`, and no "Over-writing dataset" warning;
/// - `-ok_1D_text`: `AFNI_1D_ZERO_TEXT=YES`;
/// - `-skip_afnirc`: nothing (larmorx reads no `.afnirc`).
///
/// A `-D` of a variable 3dTshift does not consult is applied too, with a warning that it has no
/// effect. AFNI's other global options (`-Vname=`, `-echo_edu`, `-pif`, `-np`, `-all_opts`,
/// `-h_view`, `<<` inclusions, ...) are not taken out: 3dTshift's scan reports them as unknown.
pub fn prefilter(args: &[String], settings: Settings, mode: UpstreamDefects) -> Prefiltered {
    prefilter_for(&TSHIFT, args, settings, mode)
}

/// [`prefilter`] for another AFNI program: its own variables decide which `-D` options get the
/// warning that they have no effect, and which variables can name AFNI-22.
pub fn prefilter_for(
    program: &Program,
    args: &[String],
    settings: Settings,
    mode: UpstreamDefects,
) -> Prefiltered {
    let mut kept = Vec::with_capacity(args.len());
    let mut quiet_overwrite = false;
    let mut warnings = Vec::new();
    // Both readings of the -D options, to tell whether AFNI's would change anything.
    let (mut plain_table, mut afni_table) = (settings.clone(), settings);
    let mut mangled = Vec::new();
    for arg in args {
        if let Some(cmd) = arg.strip_prefix("-D").filter(|c| c.contains('=')) {
            let afni = afni_setenv(cmd);
            let plain = plain_setenv(cmd);
            if afni != plain {
                mangled.push((arg.clone(), plain.clone(), afni.clone()));
            }
            if let Some((name, value)) = &afni {
                afni_table.set(name.clone(), value.clone());
            }
            if let Some((name, value)) = &plain {
                plain_table.set(name.clone(), value.clone());
            }
            match if mode.fix() { &plain } else { &afni } {
                Some((name, _)) if program.consulted(name).is_none() => warnings.push(format!(
                    "{arg} has no effect: {} does not read {name} (larmorx)",
                    program.name
                )),
                None if mode.fix() => warnings.push(format!(
                    "{arg} sets nothing (a name and a value are needed), as in AFNI (larmorx)"
                )),
                _ => {}
            }
            continue;
        }
        match arg.as_str() {
            "-overwrite" => {
                for table in [&mut plain_table, &mut afni_table] {
                    table.set("AFNI_DECONFLICT", "OVERWRITE");
                }
                quiet_overwrite = true;
            }
            "-ok_1D_text" => {
                for table in [&mut plain_table, &mut afni_table] {
                    table.set("AFNI_1D_ZERO_TEXT", "YES");
                }
            }
            "-skip_afnirc" => {}
            _ => kept.push(arg.clone()),
        }
    }
    let mut notices = Vec::new();
    let variables = program.variables();
    let differing: Vec<&str> = variables
        .iter()
        .filter(|(name, u)| {
            *u == Use::Applied && !same_effect(name, plain_table.get(name), afni_table.get(name))
        })
        .map(|(name, _)| *name)
        .collect();
    if !differing.is_empty() {
        let show = |s: &Option<(String, String)>| match s {
            Some((n, v)) => format!("sets {n} to '{v}'"),
            None => "sets nothing".to_owned(),
        };
        let shown: Vec<String> = mangled
            .iter()
            .map(|(arg, plain, afni)| {
                if mode.fix() {
                    format!("'{arg}' {}; AFNI's reading {}", show(plain), show(afni))
                } else {
                    format!("'{arg}' {}, as AFNI reads it", show(afni))
                }
            })
            .collect();
        notices.push(Notice::new(&defects::AFNI_22, mode, shown.join(", ")));
    }
    Prefiltered {
        args: kept,
        settings: if mode.fix() { plain_table } else { afni_table },
        quiet_overwrite,
        notices,
        warnings,
    }
}

/// Whether two values of the variable `name` make 3dTshift do the same: for the variables it
/// tests with `YESSISH`/`NOISH` (`AFNI_yesenv`, `AFNI_noenv`), the same answers to both tests;
/// for the others, the same value.
pub fn same_effect(name: &str, a: Option<&str>, b: Option<&str>) -> bool {
    const FLAGS: &[&str] = &[
        "AFNI_ALLOW_ARBITRARY_FILENAMES",
        "AFNI_PATH_SPACES_OK",
        "AFNI_NOREALPATH",
        "AFNI_NIFTI_SCALE",
        "AFNI_NIFTI_TYPE_WARN",
        "AFNI_NO_OBLIQUE_WARNING",
        "AFNI_1D_ZERO_TEXT",
        "AFNI_USE_FGETS",
        "AFNI_ALLOW_MILLISECONDS",
        "AFNI_MESSAGE_COLORIZE",
        "AFNI_ECHO_COMMANDLINE",
        "AFNI_DONT_LOGFILE",
    ];
    if FLAGS.contains(&name) {
        (yessish(a), noish(a)) == (yessish(b), noish(b))
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    fn pair(n: &str, v: &str) -> Option<(String, String)> {
        Some((n.to_owned(), v.to_owned()))
    }

    #[test]
    fn afni_parses_d_values_as_two_words() {
        assert_eq!(afni_setenv("X=YES"), pair("X", "YES"));
        assert_eq!(afni_setenv("X=a b"), pair("X", "a=b"));
        assert_eq!(afni_setenv("X= YES"), pair("X", "=YES"));
        assert_eq!(afni_setenv("X =YES"), pair("X", "=YES"));
        assert_eq!(afni_setenv("X=YES=1"), pair("X", "YES=1"));
        assert_eq!(afni_setenv("X=="), pair("X", "="));
        assert_eq!(afni_setenv("X="), None);
        assert_eq!(afni_setenv("=YES"), None);
        assert_eq!(afni_setenv("A="), None);
        assert_eq!(afni_setenv("=a b"), None);
        let long = format!("{}=1", "N".repeat(300));
        let (name, value) = afni_setenv(&long).unwrap();
        assert_eq!(name.len(), 255);
        assert_eq!(value, format!("{}=1", "N".repeat(45)));
        // larmorx's default: the value as written.
        assert_eq!(plain_setenv("X=a b"), pair("X", "a b"));
        assert_eq!(plain_setenv("X= YES"), pair("X", " YES"));
        assert_eq!(plain_setenv("X="), None);
        assert_eq!(plain_setenv("=Y"), None);
    }

    #[test]
    fn global_options_are_taken_out_anywhere() {
        let args = s(&[
            "-DAFNI_A=1",
            "-tpattern",
            "-overwrite",
            "alt+z",
            "-prefix",
            "o.nii",
            "in.nii",
            "-DAFNI_A=2",
            "-ok_1D_text",
            "-skip_afnirc",
            "-D",
            "-DNOEQ",
        ]);
        let p = prefilter(&args, Settings::empty(), UpstreamDefects::Reproduce);
        assert_eq!(
            p.args,
            s(&[
                "-tpattern",
                "alt+z",
                "-prefix",
                "o.nii",
                "in.nii",
                "-D",
                "-DNOEQ"
            ])
        );
        assert_eq!(p.settings.get("AFNI_A"), Some("2"));
        assert_eq!(p.settings.get("AFNI_DECONFLICT"), Some("OVERWRITE"));
        assert!(p.settings.yes("AFNI_1D_ZERO_TEXT"));
        assert!(p.quiet_overwrite);
        // AFNI_A is not a variable 3dTshift reads.
        assert_eq!(p.warnings.len(), 2, "{:?}", p.warnings);
    }

    #[test]
    fn the_command_line_wins_over_the_environment() {
        let env = Settings::from_vars([
            ("AFNI_DECONFLICT".into(), "YES".into()),
            ("AFNI_NIFTI_PRIORITY".into(), "Q".into()),
        ]);
        let p = prefilter(
            &s(&["-DAFNI_DECONFLICT=NO", "x"]),
            env,
            UpstreamDefects::Fix,
        );
        assert_eq!(p.settings.get("AFNI_DECONFLICT"), Some("NO"));
        assert_eq!(p.settings.get("AFNI_NIFTI_PRIORITY"), Some("Q"));
        assert!(p.settings.no("AFNI_DECONFLICT") && !p.settings.yes("AFNI_DECONFLICT"));
        // -overwrite then -D: the later one wins, as AFNI's putenv calls do.
        let p = prefilter(
            &s(&["-overwrite", "-DAFNI_DECONFLICT=YES"]),
            Settings::empty(),
            UpstreamDefects::Fix,
        );
        assert_eq!(p.settings.get("AFNI_DECONFLICT"), Some("YES"));
        assert!(p.quiet_overwrite);
    }

    #[test]
    fn mangled_values_are_named() {
        let args = s(&["-DAFNI_MESSAGE_PREFIX=a b"]);
        let fix = prefilter(&args, Settings::empty(), UpstreamDefects::Fix);
        assert_eq!(fix.settings.get("AFNI_MESSAGE_PREFIX"), Some("a b"));
        assert_eq!(fix.notices.len(), 1);
        assert_eq!(fix.notices[0].defect.id, "AFNI-22");
        let rep = prefilter(&args, Settings::empty(), UpstreamDefects::Reproduce);
        assert_eq!(rep.settings.get("AFNI_MESSAGE_PREFIX"), Some("a=b"));
        assert_eq!(rep.notices.len(), 1);
        // A well-formed -D names nothing.
        let ok = prefilter(
            &s(&["-DAFNI_NIFTI_VIEW=tlrc"]),
            Settings::empty(),
            UpstreamDefects::Fix,
        );
        assert!(ok.notices.is_empty() && ok.warnings.is_empty());
        // Nor one AFNI reads otherwise to the same effect: 'Y ES' and 'Y=ES' are both YES.
        let same = prefilter(
            &s(&["-DAFNI_ALLOW_ARBITRARY_FILENAMES=Y ES"]),
            Settings::empty(),
            UpstreamDefects::Fix,
        );
        assert!(same.notices.is_empty());
        assert_eq!(
            same.settings.get("AFNI_ALLOW_ARBITRARY_FILENAMES"),
            Some("Y ES")
        );
    }

    #[test]
    fn afni_tests_of_values() {
        let mut t = Settings::empty();
        for (n, v) in [
            ("A", "yes"),
            ("B", "No"),
            ("C", ""),
            ("D", "2k"),
            ("E", "1.5e0x"),
        ] {
            t.set(n, v);
        }
        assert!(t.yes("A") && !t.no("A"));
        assert!(t.no("B") && !t.yes("B"));
        assert!(!t.yes("C") && !t.no("C") && t.get("C") == Some(""));
        assert!(!t.yes("Z") && t.get("Z").is_none());
        assert_eq!(t.num("D"), 2048.0);
        assert_eq!(t.num("E"), 1.5);
        assert_eq!(t.num("Z"), 0.0);
    }

    #[test]
    fn every_consulted_variable_is_listed_once() {
        for (i, (a, _)) in CONSULTED.iter().enumerate() {
            assert!(CONSULTED[..i].iter().all(|(b, _)| a != b), "{a}");
        }
        assert_eq!(consulted("AFNI_DECONFLICT"), Some(Use::Applied));
        assert!(matches!(
            consulted("AFNI_NIFTI_NOEXT"),
            Some(Use::NoEffect(_))
        ));
        assert_eq!(consulted("AFNI_NOTHING"), None);
    }
}
