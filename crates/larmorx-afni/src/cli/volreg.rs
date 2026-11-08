// SPDX-License-Identifier: Apache-2.0
//! `3dvolreg` with its original arguments, the clean-room original (`specs/3dvolreg.md` §9,
//! §10), AFNI's global options and environment variables included.
//!
//! Differences from AFNI, all deliberate (the larmorx AFNI tools' rules): the outputs must be
//! NIfTI files (AFNI's default prefix `volreg`, in AFNI's own format, is refused); no history
//! extension or `VOLREG_*` attributes are written; AFNI's version banner and compile date are
//! not printed; the CPU times are printed as 0. AFNI's defects are corrected unless
//! `LARMORX_UPSTREAM_DEFECTS` is `reproduce` (D13).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notice, UpstreamDefects};

use crate::dataset::{self, AfniImage, BrickData, OutputTiming};
use crate::msg::{Log, Message};
use crate::output::refused;
use crate::settings::{self, AfniSettings, Program, take_global_options_of};
use crate::timing;
use crate::volreg::grid::Grid;
use crate::volreg::{self, Base, Edging, Method, VolregParams, notices, report};

use super::common::{self, Failure, Target, check_nifti, strtod, strtol_int};
use super::threads;

/// The program, for the warnings about `-D` options.
pub const PROGRAM: Program = Program {
    name: "3dvolreg",
    consults: consulted,
};

/// The variables 3dvolreg reads: the masks' (the dataset reading and writing) and its own.
fn consulted(name: &str) -> bool {
    settings::is_consulted_by_masks(name)
        || matches!(
            name,
            "AFNI_VOLREG_EDGING"
                | "AFNI_BLUR_FIRFAC"
                | "AFNI_BLUR_FIROLD"
                | "AFNI_BLUR_INTS_AS_OLD"
                | "AFNI_ROTA_ZPAD"
                | "AFNI_QUIET_STARTUP"
        )
}

const USAGE: &str = "\
Usage: 3dvolreg [options] dataset   (larmorx: clean-room original of AFNI 26.3.05's 3dvolreg)

Registers each 3D sub-brick of the input dataset to the base brick (rigid body, 6
parameters), by iterated linearised weighted least squares, as AFNI's 3dvolreg does. The
dataset and the outputs are NIfTI files.

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
  -savedisp sss        save the displacements of every voxel (sss_DX, sss_DY, sss_DZ)
  -maxite m  -x_thresh x  -rot_thresh r  -delta d  -params m x r d
  -weight 'wset[n]'    the weight brick
  -wtinp               sub-brick 0 of the input as the weight
  -edging ee | ee%     the weight's zeroed edge (default 5%, or AFNI_VOLREG_EDGING)
  -twopass  -twoblur bb  -sinit  -coarse del num  -coarserot  -nocoarserot
                       two passes: a coarse search on blurred bricks first
  -tshift ii           slice-time correct the input first (ignoring ii time points)
  -rotparent rset  -gridparent gset  -twodup
  -input dset / -source dset   the dataset, given as an option

AFNI's global options (anywhere on the command line):
  -overwrite      replace an existing output (AFNI_DECONFLICT=OVERWRITE)
  -Dname=value    set AFNI's environment variable 'name' for this run
  -skip_afnirc    accepted (no .afnirc file is read)
";

/// The first `-verbose` line.
const IMPLEMENTATION: &str = concat!(
    "implementation: original (larmorx ",
    env!("CARGO_PKG_VERSION"),
    ", Apache-2.0), clean-room 3dvolreg"
);

/// Runs `3dvolreg` with `args`; returns the exit code.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match UpstreamDefects::from_env() {
        Ok(mode) => main_with_settings(args, mode, AfniSettings::from_env(), threads(), out, err),
        Err(e) => {
            let _ = writeln!(err, "** FATAL ERROR: {e}");
            EXIT_BAD_MODE
        }
    }
}

/// A failure printed as it is.
enum Stop {
    /// `** FATAL ERROR: ...`
    Fatal(String),
    /// A line printed as it is (AFNI's `** ...` messages without a label).
    Raw(String),
}

impl From<Failure> for Stop {
    fn from(f: Failure) -> Stop {
        match f {
            Failure::Fatal(m) => Stop::Fatal(m),
        }
    }
}

/// The parsed command line.
struct Options {
    /// Notices of the option scan printed at the end (AFNI-90's).
    late: Vec<Notice>,
    dataset: Option<String>,
    input_image: Option<(AfniImage, String)>,
    by_input: bool,
    input_option: String,
    maxdisp: bool,
    maxdisp1d: Option<String>,
    savedisp: Option<(String, bool)>,
    matrix_save: Option<String>,
    sinit: bool,
    max_iter: i32,
    x_thresh: f32,
    rot_thresh: f32,
    delta: f32,
    final_method: Option<Method>,
    rotcom: bool,
    twopass: bool,
    twoblur: f32,
    verbose: u8,
    clip: bool,
    zpad: Option<usize>,
    method: Method,
    prefix: String,
    dfile: Option<String>,
    oned: Option<String>,
    base: Option<BaseOpt>,
    /// AFNI-95: a second `-base` after `-base 0`.
    base_twice: bool,
    coarse: (i32, i32),
    coarse_rotation: bool,
    /// AFNI-86: `-nocoarserot` given.
    nocoarserot: bool,
    wtinp: bool,
    edging: Option<Edging>,
    weight: Option<(String, usize, AfniImage)>,
    twodup: bool,
    tshift: Option<usize>,
    /// `-rotparent`: the dataset's name and its VOLREG attributes.
    rotparent: Option<(String, RotAttrs)>,
    /// `-gridparent`: the dataset's name and the dataset.
    gridparent: Option<(String, Box<AfniImage>)>,
    float: bool,
    rest: Vec<String>,
    /// The arguments after the global options (for `-maxdisp1D`'s first line).
    command: Vec<String>,
}

enum BaseOpt {
    Index(usize),
    Dataset(Box<AfniImage>, String),
}

/// `-edging`'s value (or `AFNI_VOLREG_EDGING`'s): `e` voxels or `e%`.
fn edging_value(v: &str) -> Result<Edging, &'static str> {
    let Some((e, rest)) = super::leading_number(v) else {
        return Ok(Edging::Voxels(0.0));
    };
    if e < 0.0 {
        return Err("value");
    }
    if rest.starts_with('%') {
        if e > 25.0 {
            return Err("percentage");
        }
        Ok(Edging::Percent(e))
    } else {
        Ok(Edging::Voxels(e))
    }
}

thread_local! {
    /// Whether this run already printed the obliquity report's long warning (§9.3: for the run's
    /// first oblique dataset only); reset at the start of every run.
    static OBLIQUE_SEEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The NIfTI type conversion notice was printed (the run's first converted dataset only,
    /// unless `AFNI_NIFTI_TYPE_WARN` is yes: §9.3).
    static TYPE_NOTICE_SEEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The notice that a NIfTI type is converted to float32, muted after its first time (the
/// muting text is in it unless `AFNI_NIFTI_TYPE_WARN` is yes, when every one is printed).
fn muted_type_notice(m: &Message) -> bool {
    matches!(m, Message::Raw(t) if t.starts_with("** AFNI converts NIFTI_datatype=") && t.contains("will be muted"))
        && TYPE_NOTICE_SEEN.with(|s| s.replace(true))
}

/// The obliquity report's long warning (the paragraph before the one-line report).
fn long_oblique_warning(m: &Message) -> bool {
    matches!(m, Message::Warning(t) if t.starts_with("  If you are performing spatial transformations"))
}

/// Opens a dataset as 3dvolreg reads it (its messages and notices to `log`).
fn open(
    name: &str,
    settings: &AfniSettings,
    mode: UpstreamDefects,
    n_threads: usize,
    log: &mut Log<'_>,
) -> Result<(AfniImage, String), String> {
    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".nii") || lower.ends_with(".nii.gz")) || name.contains('[') {
        return Err(format!(
            "{name}: larmorx reads NIfTI files (.nii, .nii.gz) only: not supported by larmorx"
        ));
    }
    let (mut messages, mut notices) = (Vec::new(), Vec::new());
    let found = crate::input::resolve(name, settings, mode, &mut messages, &mut notices);
    for m in &messages {
        if long_oblique_warning(m) && OBLIQUE_SEEN.with(|s| s.replace(true)) {
            continue;
        }
        log.message(m);
    }
    for n in &notices {
        log.notice(n);
    }
    let found = found.map_err(|e| match std::fs::metadata(name) {
        Err(io) => format!("{name}: {io}"),
        Ok(_) => e.0,
    })?;
    let mut image = dataset::read(&found.path, &found.shown, n_threads, mode, settings)
        .map_err(|e| e.to_string())?;
    for m in std::mem::take(&mut image.messages) {
        if long_oblique_warning(&m) && OBLIQUE_SEEN.with(|s| s.replace(true)) {
            continue;
        }
        if muted_type_notice(&m) {
            continue;
        }
        log.message(&m);
    }
    for n in std::mem::take(&mut image.notices) {
        log.notice(&n);
    }
    Ok((image, found.shown))
}

/// AFNI's message for a dataset that cannot be opened, with larmorx's reason by default, and
/// in both modes where the reason is larmorx's own limit (a format it does not read).
fn reason(afni: String, why: &str, mode: UpstreamDefects) -> String {
    if mode.fix() || why.ends_with("not supported by larmorx") {
        format!("{afni}: {why}")
    } else {
        afni
    }
}

/// The AFNI-88 notice and error for an option given last without its value.
fn needs_value(opt: &str, mode: UpstreamDefects, log: &mut Log<'_>) -> Stop {
    log.notice(&notices::afni_88(opt, mode));
    Stop::Fatal(format!(
        "need argument after {opt} (AFNI-88: AFNI crashes here)"
    ))
}

fn starts(arg: &str, name: &str, n: usize) -> bool {
    arg.len() >= n && arg.as_bytes()[..n] == name.as_bytes()[..n]
}

fn parse(
    args: &[String],
    log: &mut Log<'_>,
    settings: &AfniSettings,
    mode: UpstreamDefects,
    n_threads: usize,
) -> Result<Options, Stop> {
    let mut o = Options {
        dataset: None,
        input_image: None,
        by_input: false,
        input_option: String::new(),
        maxdisp: true,
        maxdisp1d: None,
        savedisp: None,
        late: Vec::new(),
        matrix_save: None,
        sinit: true,
        max_iter: 23,
        x_thresh: 0.01,
        rot_thresh: 0.02,
        delta: 0.7,
        final_method: None,
        rotcom: false,
        twopass: false,
        twoblur: 2.0,
        verbose: 0,
        clip: true,
        zpad: None,
        method: Method::Heptic,
        prefix: "volreg".into(),
        dfile: None,
        oned: None,
        base: None,
        base_twice: false,
        coarse: (10, 2),
        coarse_rotation: true,
        nocoarserot: false,
        wtinp: false,
        edging: None,
        weight: None,
        twodup: false,
        tshift: None,
        rotparent: None,
        gridparent: None,
        float: false,
        rest: Vec::new(),
        command: args.to_vec(),
    };
    let mut i = 0;
    while i < args.len() && args[i].starts_with('-') {
        let arg = args[i].as_str();
        let value = |k: usize| -> Option<&str> { args.get(i + k).map(String::as_str) };
        macro_rules! need {
            ($k:expr) => {
                match value($k) {
                    Some(v) => v,
                    None => return Err(needs_value(arg, mode, log)),
                }
            };
        }
        if arg == "-input" || arg == "-source" {
            let Some(v) = value(1) else {
                return Err(Stop::Fatal("Can't open input dataset '(null)'".into()));
            };
            if o.by_input {
                return Err(Stop::Fatal(format!("Can't have 2 '{arg}' options!")));
            }
            let image = open(v, settings, mode, n_threads, log).map_err(|e| {
                Stop::Fatal(reason(format!("Can't open input dataset '{v}'"), &e, mode))
            })?;
            o.input_image = Some(image);
            o.by_input = true;
            o.input_option = arg.to_owned();
            o.dataset = Some(v.to_owned());
            i += 2;
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
            let v = need!(1);
            o.maxdisp = true;
            o.maxdisp1d = Some(v.to_owned());
            i += 2;
            continue;
        }
        if arg == "-savedisp" || arg == "-SAVEDISP" {
            let v = need!(1);
            let lower = v.to_ascii_lowercase();
            let ok = !refused(v, settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES"))
                && !((lower.ends_with(".nii.gz") && v.len() < 8)
                    || (lower.ends_with(".nii") && v.len() < 5));
            if !ok {
                return Err(Stop::Fatal(format!(
                    "{arg} '{v}' is invalid output dataset name"
                )));
            }
            o.savedisp = Some((v.to_owned(), arg == "-SAVEDISP"));
            i += 2;
            continue;
        }
        if arg == "-1Dmatrix_save" {
            let Some(v) = value(1) else {
                return Err(Stop::Fatal("no argument after '-1Dmatrix_save'!".into()));
            };
            if o.matrix_save.is_some() {
                return Err(Stop::Fatal(
                    "Can't have multiple -1Dmatrix_save options!".into(),
                ));
            }
            if refused(v, settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES")) {
                return Err(Stop::Fatal(format!(
                    "badly formed filename: -1Dmatrix_save '{v}'"
                )));
            }
            o.matrix_save = Some(v.to_owned());
            i += 2;
            continue;
        }
        if arg == "-sinit" {
            o.sinit = false;
            i += 1;
            continue;
        }
        if starts(arg, "-params", 4) {
            let (m, x, r, d) = (need!(1), need!(2), need!(3), need!(4));
            o.max_iter = strtol_int(m);
            o.x_thresh = strtod(x) as f32;
            o.rot_thresh = strtod(r) as f32;
            o.delta = strtod(d) as f32;
            i += 5;
            continue;
        }
        if starts(arg, "-maxite", 4) {
            let m = strtol_int(need!(1));
            if m > 0 {
                o.max_iter = m;
            }
            i += 2;
            continue;
        }
        if starts(arg, "-x_thresh", 4) {
            let x = strtod(need!(1)) as f32;
            if x > 0.0 {
                o.x_thresh = x;
            }
            i += 2;
            continue;
        }
        if starts(arg, "-rot_thresh", 6) {
            let x = strtod(need!(1)) as f32;
            if x > 0.0 {
                o.rot_thresh = x;
            }
            i += 2;
            continue;
        }
        if starts(arg, "-delta", 4) {
            let x = strtod(need!(1)) as f32;
            if x > 0.0 {
                o.delta = x;
            }
            i += 2;
            continue;
        }
        if starts(arg, "-final", 4) {
            let v = need!(1);
            o.final_method = Some(match v {
                "cubic" => Method::Cubic,
                "quintic" => Method::Quintic,
                "heptic" => Method::Heptic,
                "Fourier" => Method::Fourier,
                "NN" => Method::NN,
                "linear" => Method::Linear,
                _ => return Err(Stop::Raw("** Illegal mode after -final".into())),
            });
            i += 2;
            continue;
        }
        if arg == "-rotcom" {
            o.rotcom = true;
            i += 1;
            continue;
        }
        if arg == "-twopass" {
            o.twopass = true;
            if o.max_iter < 10 {
                o.max_iter = 66;
            }
            i += 1;
            continue;
        }
        if arg == "-twoblur" {
            let b = strtod(need!(1)) as f32;
            if b < 2.0 {
                return Err(Stop::Raw("** ERROR: value after -twoblur is < 2.0".into()));
            }
            o.twoblur = b;
            i += 2;
            continue;
        }
        if starts(arg, "-verbose", 5) {
            o.verbose = o.verbose.saturating_add(1);
            i += 1;
            continue;
        }
        if starts(arg, "-clipit", 4) {
            log.info("Notice: -clipit is now the default");
            o.clip = true;
            i += 1;
            continue;
        }
        // AFNI-86: AFNI tests -noclip's four letters first, so -nocoarserot is -noclip.
        if arg == "-nocoarserot" && mode.fix() {
            o.coarse_rotation = false;
            o.nocoarserot = true;
            i += 1;
            continue;
        }
        if starts(arg, "-noclip", 4) {
            if arg == "-nocoarserot" {
                o.nocoarserot = true;
            }
            o.clip = false;
            i += 1;
            continue;
        }
        if starts(arg, "-zpad", 5) {
            let z = strtod(need!(1)).trunc();
            if let Some(prev) = o.zpad
                && prev != 4
            {
                log.info("WARNING: second -zpad option!");
            }
            if z < 0.0 {
                return Err(Stop::Raw(format!("** ERROR: Can't use -zpad {}", z as i64)));
            }
            o.zpad = Some(z as usize);
            i += 2;
            continue;
        }
        let method = [
            ("-Fourier", "-fourier", Method::Fourier),
            ("-linear", "-Linear", Method::Linear),
            ("-cubic", "-Cubic", Method::Cubic),
            ("-quintic", "-Quintic", Method::Quintic),
            ("-heptic", "-Heptic", Method::Heptic),
        ]
        .into_iter()
        .find(|(a, b, _)| starts(arg, a, 4) || starts(arg, b, 4));
        if let Some((_, _, m)) = method {
            o.method = m;
            i += 1;
            continue;
        }
        if starts(arg, "-prefix", 4) {
            let v = need!(1);
            long_name("-prefix name", v, mode, log, &mut o)?;
            o.prefix = v.to_owned();
            i += 2;
            continue;
        }
        if starts(arg, "-dfile", 4) {
            let v = need!(1);
            long_name("-dfile name", v, mode, log, &mut o)?;
            o.dfile = Some(v.to_owned());
            i += 2;
            continue;
        }
        if starts(arg, "-1Dfile", 4) {
            let v = need!(1);
            long_name("-1Dfile name", v, mode, log, &mut o)?;
            o.oned = Some(v.to_owned());
            i += 2;
            continue;
        }
        if starts(arg, "-base", 4) {
            let v = need!(1);
            match &o.base {
                Some(BaseOpt::Index(0)) => {
                    o.base_twice = true;
                    if mode.fix() {
                        log.notice(&afni_95(mode));
                        return Err(Stop::Raw("** Can't have two -base arguments".into()));
                    }
                }
                Some(_) => {
                    print_notices(log, &scan_notices(&o, mode));
                    return Err(Stop::Raw("** Can't have two -base arguments".into()));
                }
                None => {}
            }
            let t = v.trim_start();
            let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
            let integer = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
            if integer {
                let n = strtol_int(v);
                if n < 0 {
                    return Err(Stop::Fatal("Illegal negative number after -base".into()));
                }
                o.base = Some(BaseOpt::Index(n as usize));
            } else {
                if o.verbose > 0 {
                    log.info(&format!("Reading in base dataset {v}"));
                }
                let (image, shown) = open(v, settings, mode, n_threads, log).map_err(|e| {
                    Stop::Fatal(reason(format!("Couldn't open -base dataset {v}"), &e, mode))
                })?;
                if image.bricks.shape[3] > 1 {
                    log.warning(&format!("-base dataset {v} has more than 1 sub-brick"));
                }
                o.base = Some(BaseOpt::Dataset(Box::new(image), shown));
            }
            i += 2;
            continue;
        }
        if arg == "-coarse" {
            let (a, b) = (need!(1), need!(2));
            o.coarse = (strtol_int(a), strtol_int(b));
            i += 3;
            continue;
        }
        if arg == "-coarserot" {
            log.info("-coarserot is now the default");
            o.coarse_rotation = true;
            i += 1;
            continue;
        }
        if arg == "-wtrim" {
            log.info("Notice: -wtrim is now always enabled");
            i += 1;
            continue;
        }
        if arg == "-wtinp" {
            o.wtinp = true;
            i += 1;
            continue;
        }
        if arg == "-edging" {
            let v = need!(1);
            o.edging = Some(
                edging_value(v)
                    .map_err(|what| Stop::Raw(format!("** Illegal {what} after -edging")))?,
            );
            i += 2;
            continue;
        }
        if starts(arg, "-weight", 4) {
            let v = need!(1);
            if o.weight.is_some() {
                return Err(Stop::Raw("** Can't have two -weight options".into()));
            }
            let Some(at) = v.find('[').filter(|&a| a > 0) else {
                return Err(Stop::Raw("** Illegal weight dataset after -weight".into()));
            };
            let name = &v[..at];
            // AFNI-90: the weight dataset is opened here, so the notice comes at once.
            if name.len() >= 256 {
                log.notice(&notices::afni_90("-weight dataset name", name.len(), mode));
                if mode.reproduce() {
                    return Err(Stop::Fatal(notices::afni_90_fatal(
                        "-weight dataset name",
                        name.len(),
                    )));
                }
            }
            let index = crate::cli::atoi(&v[at + 1..]);
            let index = if v[at + 1..]
                .trim_start()
                .starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+')
            {
                index
            } else {
                -1
            };
            if index < 0 {
                return Err(Stop::Raw(
                    "** Illegal sub-brick selector after -weight".into(),
                ));
            }
            let (image, _) = open(name, settings, mode, n_threads, log).map_err(|e| {
                Stop::Raw(reason(
                    format!("** Can't open weight dataset {name}"),
                    &e,
                    mode,
                ))
            })?;
            if index as usize >= image.bricks.shape[3] {
                return Err(Stop::Raw(format!(
                    "** Illegal sub-brick selector for dataset {name}"
                )));
            }
            if o.verbose > 0 {
                log.info(&format!("Reading in weight dataset {name}"));
            }
            o.weight = Some((name.to_owned(), index as usize, image));
            i += 2;
            continue;
        }
        if arg == "-twodup" {
            o.twodup = true;
            i += 1;
            continue;
        }
        if arg == "-tshift" {
            let v = strtod(need!(1)).trunc();
            if v < 0.0 {
                return Err(Stop::Raw("** -tshift parameter is negative!".into()));
            }
            o.tshift = Some(v as usize);
            i += 2;
            continue;
        }
        if starts(arg, "-rotparent", 7) {
            let Some(v) = value(1) else {
                return Err(Stop::Raw("** Can't open -rotparent dataset!\n\n".into()));
            };
            if o.rotparent.is_some() {
                return Err(Stop::Raw("** Can't use -2 -rotparent options!".into()));
            }
            let (image, _) = match open(v, settings, mode, n_threads, log) {
                Ok(found) => found,
                // AFNI's line and a blank line; by default the reason too (as the replica).
                Err(_) if mode.reproduce() => {
                    return Err(Stop::Raw("** Can't open -rotparent dataset!\n\n".into()));
                }
                Err(why) => {
                    return Err(Stop::Raw(format!(
                        "** Can't open -rotparent dataset {v}: {why}"
                    )));
                }
            };
            let attrs = rot_attrs(&image);
            if attrs.matvec.len() < 12 {
                return Err(Stop::Raw(
                    "** -rotparent dataset doesn't have VOLREG attributes!?".into(),
                ));
            }
            // AFNI-96: the centres must be there, three numbers each (an error in both modes).
            for (name, c) in [
                ("VOLREG_CENTER_OLD", &attrs.center_old),
                ("VOLREG_CENTER_BASE", &attrs.center_base),
            ] {
                if c.as_ref().is_none_or(|c| c.len() < 3) {
                    log.notice(&notices::afni_96(v, name, mode));
                    return Err(Stop::Fatal(notices::afni_96_fatal(v, name)));
                }
            }
            o.rotparent = Some((v.to_owned(), attrs));
            i += 2;
            continue;
        }
        if starts(arg, "-gridparent", 7) {
            let Some(v) = value(1) else {
                return Err(Stop::Raw("** Can't open -gridparent dataset!\n\n".into()));
            };
            if o.gridparent.is_some() {
                return Err(Stop::Raw("** Can't use -2 -gridparent options!".into()));
            }
            let (image, _) = match open(v, settings, mode, n_threads, log) {
                Ok(found) => found,
                // AFNI's line and a blank line; by default the reason too (as the replica).
                Err(_) if mode.reproduce() => {
                    return Err(Stop::Raw("** Can't open -gridparent dataset!\n\n".into()));
                }
                Err(why) => {
                    return Err(Stop::Raw(format!(
                        "** Can't open -gridparent dataset {v}: {why}"
                    )));
                }
            };
            o.gridparent = Some((v.to_owned(), Box::new(image)));
            i += 2;
            continue;
        }
        if starts(arg, "-float", 6) {
            o.float = true;
            i += 1;
            continue;
        }
        return Err(Stop::Raw(format!("** Unknown option: {arg}\x07")));
    }
    o.rest = args[i..].to_vec();
    Ok(o)
}

/// The VOLREG attributes `-rotparent` reads from a dataset's AFNI extension (§8.9).
#[derive(Clone, Debug, Default)]
struct RotAttrs {
    matvec: Vec<f32>,
    center_old: Option<Vec<f32>>,
    center_base: Option<Vec<f32>>,
}

/// The numbers of the attribute `name` in AFNI's header extension (code 4) of `image`: the
/// text between the attribute's `>` and `</AFNI_atr>`, read as C's `strtod` then float32.
fn attribute(image: &AfniImage, name: &str) -> Option<Vec<f32>> {
    let key = format!("atr_name=\"{name}\"");
    for ext in image.header.extensions.iter().filter(|e| e.code == 4) {
        let text = String::from_utf8_lossy(ext.trimmed_content());
        let Some(at) = text.find(&key) else {
            continue;
        };
        let rest = &text[at + key.len()..];
        let start = rest.find('>')? + 1;
        let end = rest[start..].find("</AFNI_atr>")? + start;
        let values = rest[start..end]
            .split_whitespace()
            .filter_map(|w| w.trim_matches('"').parse::<f64>().ok())
            .map(|v| v as f32)
            .collect();
        return Some(values);
    }
    None
}

fn rot_attrs(image: &AfniImage) -> RotAttrs {
    RotAttrs {
        matvec: attribute(image, "VOLREG_MATVEC_000000").unwrap_or_default(),
        center_old: attribute(image, "VOLREG_CENTER_OLD"),
        center_base: attribute(image, "VOLREG_CENTER_BASE"),
    }
}

/// The cardinal (plumb) matrix of a grid, as NIfTI holds it (RAS: DICOM's x and y negated).
fn cardinal_matrix(g: &Grid) -> crate::geometry::M34 {
    let mut m: crate::geometry::M34 = [[0.0; 4]; 3];
    for j in 0..3 {
        let r = g.orient[j].dicom_axis();
        let ras = |v: f32| if r < 2 { -v } else { v };
        m[r][j] = ras(g.delta[j]);
        m[r][3] = ras(g.origin[j]);
    }
    m
}

/// AFNI-90: a name of 256 bytes or more aborts AFNI at its option (`reproduce`); kept whole by
/// default, with the notice at the end.
fn long_name(
    what: &str,
    v: &str,
    mode: UpstreamDefects,
    log: &mut Log<'_>,
    o: &mut Options,
) -> Result<(), Stop> {
    if v.len() < 256 {
        return Ok(());
    }
    if mode.reproduce() {
        let mut pending = scan_notices(o, mode);
        pending.push(notices::afni_90(what, v.len(), mode));
        print_notices(log, &pending);
        return Err(Stop::Fatal(notices::afni_90_fatal(what, v.len())));
    }
    o.late.push(notices::afni_90(what, v.len(), mode));
    Ok(())
}

fn afni_95(mode: UpstreamDefects) -> Notice {
    notices::afni_95(mode)
}

fn afni_86(mode: UpstreamDefects) -> Notice {
    notices::afni_86(mode)
}

/// `-maxdisp1D`'s delta file: `_delt` before a known extension, else appended.
fn delta_name(name: &str) -> String {
    const EXT: [&str; 29] = [
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
    ];
    let mut best: Option<&str> = None;
    for e in EXT.iter().chain(std::iter::once(&".heic")) {
        if name.ends_with(e) && best.is_none_or(|b| e.len() > b.len()) {
            best = Some(e);
        }
    }
    match best {
        Some(e) => format!("{}_delt{e}", &name[..name.len() - e.len()]),
        None => format!("{name}_delt"),
    }
}

/// `-maxdisp1D`'s first line: the command line, refused names quoted.
fn command_line(args: &[String], settings: &AfniSettings) -> String {
    let arbitrary = settings.yes("AFNI_ALLOW_ARBITRARY_FILENAMES");
    let mut line = String::from("3dvolreg");
    for a in args.iter().filter(|a| !a.is_empty()) {
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

/// Writes a text output; AFNI's messages for an existing file and for one it cannot open.
fn write_text(path: &str, text: &str, warn: Option<&str>, sad: bool, log: &mut Log<'_>) {
    if Path::new(path).exists()
        && let Some(w) = warn
    {
        log.message(&if w.starts_with("*+") {
            Message::Warning(format!("Overwriting file {path}"))
        } else {
            Message::Raw(format!("** Warning: overwriting file {path}"))
        });
    }
    if std::fs::write(path, text).is_err() {
        log.error(&format!(
            "Cannot open '{path}' for output{}",
            if sad { " :(" } else { "" }
        ));
    }
}

/// Runs `3dvolreg` with the D13 mode and AFNI's settings given.
pub fn main_with_settings(
    args: &[String],
    mode: UpstreamDefects,
    mut settings: AfniSettings,
    n_threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if args.is_empty() || args[0].starts_with("-help") {
        let _ = out.write_all(USAGE.as_bytes());
        return 0;
    }
    OBLIQUE_SEEN.with(|s| s.set(false));
    TYPE_NOTICE_SEEN.with(|s| s.set(false));
    let globals = take_global_options_of(args, &mut settings, mode, &PROGRAM);
    let mut log = Log::new(out, err, &settings, mode);
    if !settings.yes("AFNI_QUIET_STARTUP") {
        log.info("Authored by: RW Cox");
    }
    for w in &globals.warnings {
        log.warning(w);
    }
    for n in &globals.notices {
        log.notice(n);
    }
    let mut rest = globals.rest;
    // `-@` as the last argument: the words of standard input.
    if rest.last().is_some_and(|a| a == "-@") {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text);
        let words: Vec<String> = text
            .split([' ', '\t', '\n', '\r', '\x0c', '\x0b'])
            .filter(|w| !w.is_empty())
            .map(str::to_owned)
            .collect();
        if !words.is_empty() {
            rest.pop();
            rest.extend(words);
        }
    }
    let result = parse(&rest, &mut log, &settings, mode, n_threads)
        .and_then(|o| execute(o, mode, &settings, n_threads, &mut log));
    match result {
        Ok(code) => code,
        Err(Stop::Fatal(m)) => {
            log.fatal(&m);
            1
        }
        Err(Stop::Raw(m)) => {
            log.raw(&m);
            1
        }
    }
}

fn execute(
    mut o: Options,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    n_threads: usize,
    log: &mut Log<'_>,
) -> Result<u8, Stop> {
    log.verbose = o.verbose > 0;
    log.larmorx_info(IMPLEMENTATION);
    log.larmorx_info(&format!("upstream defects: {mode} ({})", defects::ENV));
    let mut late: Vec<Notice> = std::mem::take(&mut o.late);
    if o.nocoarserot {
        late.push(afni_86(mode));
    }
    if o.base_twice && mode.reproduce() {
        late.push(afni_95(mode));
    }
    // The scan's notices come at the end of a run; a run that stops with an error after the scan
    // prints them before its error (as the replica does), unless it printed them itself.
    let result = execute_after_scan(o, mode, settings, n_threads, log, &mut late);
    if result.is_err() {
        print_notices(log, &late);
    }
    result
}

fn execute_after_scan(
    mut o: Options,
    mode: UpstreamDefects,
    settings: &AfniSettings,
    n_threads: usize,
    log: &mut Log<'_>,
    late: &mut Vec<Notice>,
) -> Result<u8, Stop> {
    // §9.2.
    let external = matches!(o.base, Some(BaseOpt::Dataset(..)));
    if !(o.twopass && external) {
        o.twodup = false;
    }
    if o.gridparent.is_some() && o.rotparent.is_none() {
        log.info("WARNING: -gridparent means nothing without -rotparent!");
        o.gridparent = None;
    }
    if o.rotparent.is_some() && o.twopass {
        log.info("WARNING: Combining -rotparent and -twopass isn't recommended!");
    }
    if o.gridparent.is_some() && o.twodup {
        log.info("WARNING: -gridparent overrides -twodup!");
        o.twodup = false;
    }
    let edging = match o.edging {
        Some(e) => e,
        None => match settings.get("AFNI_VOLREG_EDGING") {
            Some(v) => edging_value(v)
                .map_err(|what| Stop::Raw(format!("** Illegal {what} in AFNI_VOLREG_EDGING")))?,
            None => Edging::Percent(5.0),
        },
    };
    let (image, shown, arg_name) = if o.by_input {
        if let Some(first) = o.rest.first() {
            log.warning(&format!(
                "Skipping argument(s) after last option? '{first}'"
            ));
        }
        let (image, shown) = o.input_image.take().expect("opened by -input");
        let name = o.dataset.clone().unwrap_or_default();
        (image, shown, name)
    } else {
        match o.rest.len() {
            0 => {
                return Err(Stop::Fatal(format!(
                    "Too few arguments!?  Last={}",
                    rest_last(&o)
                )));
            }
            1 => {}
            _ => {
                return Err(Stop::Fatal(format!(
                    "Too many arguments?!  Dataset={}?",
                    o.rest[0]
                )));
            }
        }
        let name = o.rest[0].clone();
        let (image, shown) = open(&name, settings, mode, n_threads, log).map_err(|e| {
            if mode.fix() {
                Stop::Fatal(format!("Can't open input dataset '{name}': {e}"))
            } else {
                Stop::Raw(format!("** Can't open dataset {name}"))
            }
        })?;
        (image, shown, name)
    };
    let [nx, ny, nz, nt] = image.bricks.shape;
    let scaled = image.bricks.factors.iter().any(|&f| f != 0.0 && f != 1.0);
    if scaled && !o.float {
        log.warning("Input dataset has scale factors ==> output will be stored as floats!");
    }
    let grid = Grid::from_geometry(&image.geometry, [nx, ny, nz], mode);
    // The base.
    let mut base_geometry = None;
    let (base, base_grid_name) = match o.base.take() {
        None => (Base::Index(0), None),
        Some(BaseOpt::Index(n)) => (Base::Index(n), None),
        Some(BaseOpt::Dataset(b, shown)) => {
            base_geometry = Some(b.geometry.clone());
            let g = Grid::from_geometry(
                &b.geometry,
                [b.bricks.shape[0], b.bricks.shape[1], b.bricks.shape[2]],
                mode,
            );
            let v = volreg::volume_f32(&b.bricks, 0);
            (
                Base::External { volume: v, grid: g },
                Some((shown, b.bricks.shape)),
            )
        }
    };
    if let Base::External { grid: bg, .. } = &base
        && o.verbose > 0
    {
        let (a, b) = (grid.centre(), bg.centre());
        let d = ((a[0] - b[0]) * (a[0] - b[0])
            + (a[1] - b[1]) * (a[1] - b[1])
            + (a[2] - b[2]) * (a[2] - b[2]))
            .sqrt();
        if d > 0.01 {
            log.info(&format!(
                "centers of base and input datasets are {} mm apart",
                crate::cfmt::fixed(f64::from(d), 2)
            ));
        }
    }
    // §9.2 item 8: -tshift's checks.
    if let Some(ii) = o.tshift {
        let offsets = image.slice_timing.as_ref().map_or(0, |h| h.times.len());
        if nt == 1 {
            log.info("WARNING: -tshift used on a 1 brick dataset!");
            o.tshift = None;
        } else if offsets < nz {
            log.info("WARNING: -tshift used on a dataset with no time-offsets!");
            o.tshift = None;
        } else if ii as i64 > nt as i64 - 5 {
            log.info("WARNING: -tshift ignore is too large for this dataset!");
            o.tshift = None;
        } else if let Base::Index(b) = &base
            && *b < ii
        {
            log.info("WARNING: base brick is prior to -tshift ignore point.");
        }
    }
    if let Base::Index(n) = &base
        && *n >= nt
    {
        let shown_arg = afni_98_name(&o, &arg_name, mode, log);
        return Err(Stop::Raw(format!(
            "** Dataset {shown_arg} doesn't have base brick index = {n}"
        )));
    }
    if let (Base::External { grid: bg, .. }, Some((bshown, bshape))) = (&base, &base_grid_name) {
        let mut bad = Vec::new();
        let dot = |s: &str| {
            if s.contains('/') {
                s.to_owned()
            } else {
                format!("./{s}")
            }
        };
        let (ina, bna) = (dot(&shown), dot(bshown));
        if bshape[..3] != [nx, ny, nz] {
            bad.push(format!(
                "** Input {ina} and base {bna} don't have same dimensions!\n   Input: nx={nx}  ny={ny}  nz={nz}\n   Base:  nx={}  ny={}  nz={}",
                bshape[0], bshape[1], bshape[2]
            ));
        }
        let step_differs =
            (0..3).any(|a| (grid.delta[a] - bg.delta[a]).abs() > 0.01 * grid.delta[a].abs());
        if step_differs {
            let f = |d: [f32; 3]| d.map(|x| crate::cfmt::fixed_width(f64::from(x), 6, 3));
            let (a, b) = (f(grid.delta), f(bg.delta));
            bad.push(format!(
                "** Input {ina} and base {bna} don't have same grid spacing!\n   Input: dx={}  dy={}  dz={}\n   Base:  dx={}  dy={}  dz={}",
                a[0], a[1], a[2], b[0], b[1], b[2]
            ));
        }
        if grid.orient != bg.orient {
            let f = |g: &Grid| g.orient.map(|o| o.name()).join(" ");
            bad.push(format!(
                "** Input {ina} and base {bna} don't have same orientation!\n   Input: {}\n   Base:  {} ",
                f(&grid),
                f(bg)
            ));
        }
        if !bad.is_empty() {
            for b in bad {
                log.raw(&b);
            }
            // AFNI's label is not coloured here
            return Err(Stop::Raw(
                "** FATAL ERROR: perhaps you could make your datasets match?".into(),
            ));
        }
    }
    let weight = match o.weight.take() {
        None => None,
        Some((_, index, w)) => {
            let ws = w.bricks.shape;
            if ws[..3] != [nx, ny, nz] {
                let shown_arg = afni_98_name(&o, &arg_name, mode, log);
                return Err(Stop::Raw(format!(
                    "** Dataset {shown_arg} doesn't conform to dimensions of weight brick\n    Weight  has nx = {}  ny = {}  nz = {}\n    Dataset has nx = {nx}  ny = {ny}  nz = {nz}",
                    ws[0], ws[1], ws[2]
                )));
            }
            Some(volreg::volume_f32(&w.bricks, index))
        }
    };
    if matches!(base, Base::Index(_)) && nt == 1 {
        return Err(Stop::Fatal(
            "You can't register a 1 brick dataset to itself!".into(),
        ));
    }
    let mut coarse = o.coarse;
    if o.twopass && coarse.0 > 0 && coarse.1 > 0 {
        let m = ((0.1 * nx.min(ny).min(nz) as f64 + 0.499).trunc() as i32).max(1);
        if coarse.0 > m {
            log.info(&format!("Coarse del was {}, replaced with {m}", coarse.0));
            coarse.0 = m;
        }
    }
    if weight.is_some() && o.wtinp {
        log.info("Input weight file overrides -wtinp option!");
        o.wtinp = false;
    }
    // §8.9: -gridparent's checks and padding, -rotparent's transformation.
    let mut out_grid = grid.clone();
    let mut pad = [0usize; 2];
    let mut gp_grid = None;
    if let Some((_, gp)) = &o.gridparent {
        let gs = gp.bricks.shape;
        let g = Grid::from_geometry(&gp.geometry, [gs[0], gs[1], gs[2]], mode);
        let spacing = (0..3).any(|a| (g.delta[a] - grid.delta[a]).abs() > 0.01 * g.delta[a].abs());
        if spacing || g.orient != grid.orient {
            return Err(Stop::Raw(
                "** Fatal Error:\n** -gridparent dataset and input dataset don't\n** match in grid spacing and/or orientation!".into(),
            ));
        }
        if gs[0] != nx || gs[1] != ny {
            return Err(Stop::Raw(
                "** Fatal Error:\n** -gridparent and input datasets\n** don't match in x,y dimensions!".into(),
            ));
        }
        if gs[2] < nz {
            return Err(Stop::Raw(
                "** Fatal Error:\n** -gridparent has fewer slices than input dataset!".into(),
            ));
        }
        let extra = gs[2] - nz;
        if extra > 0 {
            let (n1, n2) = (extra / 2, extra - extra / 2);
            pad = if grid.delta[2] > 0.0 {
                [n1, n2]
            } else {
                [n2, n1]
            };
            if o.verbose > 0 {
                let (first, second) = grid.orient[2].letters();
                let (lo, hi) = if "RAI".contains(first) {
                    (first, second)
                } else {
                    (second, first)
                };
                log.info(&format!(
                    "Zero padding to match -gridparent: -{lo} {n1}  -{hi} {n2}"
                ));
            }
        }
        out_grid.n[2] = gs[2];
        out_grid.origin = g.origin;
        gp_grid = Some(g);
    }
    let mut rotparent = None;
    let mut ndz = 0i64;
    if let Some((_, attrs)) = &o.rotparent {
        // The centres (checked at the scan, AFNI-96).
        let centre = |c: &Option<Vec<f32>>| -> [f64; 3] {
            let v = c.as_deref().unwrap_or(&[0.0; 3]);
            [0, 1, 2].map(|k| f64::from(v[k]))
        };
        let s2 = centre(&attrs.center_old);
        let s1 = centre(&attrs.center_base);
        let mv: Vec<f64> = attrs.matvec.iter().map(|&x| f64::from(x)).collect();
        let r: volreg::grid::M3 = [
            [mv[0], mv[1], mv[2]],
            [mv[4], mv[5], mv[6]],
            [mv[8], mv[9], mv[10]],
        ];
        let mut t = [mv[3], mv[7], mv[11]];
        let rtr = volreg::rotate::mul(&volreg::rotate::transpose(&r), &r);
        let mut off = 0.0f64;
        for (i, row) in rtr.iter().enumerate() {
            for (j, &v) in row.iter().enumerate() {
                off += (v - if i == j { 1.0 } else { 0.0 }).abs();
            }
        }
        if off as f32 > 0.01 {
            return Err(Stop::Raw("** -rotparent matrix not orthogonal!".into()));
        }
        // E2: the centre of the padded input grid (the input's origin moved by the low padding),
        // before the origin becomes the gridparent's (observed).
        let mut padded_grid = grid.clone();
        padded_grid.n[2] = out_grid.n[2];
        padded_grid.origin[2] = grid.origin[2] - pad[0] as f32 * grid.delta[2];
        let e2 = padded_grid.centre().map(f64::from);
        let e1 = gp_grid.as_ref().map_or(e2, |g| g.centre().map(f64::from));
        let d2 = [0, 1, 2].map(|k| e2[k] - s2[k]);
        let rd = volreg::rotate::mul_vec(&r, d2);
        for k in 0..3 {
            t[k] += rd[k] - (e1[k] - s1[k]);
        }
        let p = out_grid.p(true);
        let pt = volreg::rotate::transpose(&p);
        let rd = volreg::rotate::mul(&pt, &volreg::rotate::mul(&r, &p));
        let td = volreg::rotate::mul_vec(&pt, t);
        ndz = (td[2] / f64::from(grid.delta[2].abs())).round_ties_even() as i64;
        // Only a dataset with slice offsets has them adjusted (observed: no line without).
        if o.verbose > 0 && ndz != 0 && image.slice_timing.is_some() {
            log.info(&format!("adjusting time-offsets by {ndz} slices"));
        }
        rotparent = Some(volreg::RotParent { r: rd, t: td, pad });
    }
    // §8.1: the output.
    let output = o.prefix != "NULL";
    if output {
        check_nifti("-prefix", &o.prefix)?;
    }
    if let Some((s, _)) = &o.savedisp {
        for axis in ["DX", "DY", "DZ"] {
            check_nifti("-prefix", &savedisp_name(s, axis))?;
        }
    }
    if !output {
        log.warning("No output dataset will be calculated");
    } else {
        if settings
            .get("AFNI_DECONFLICT")
            .is_some_and(|v| v.starts_with('N'))
            && crate::output::exists(&o.prefix)
        {
            return Err(Stop::Fatal(format!(
                "Output file {} already exists -- cannot continue!",
                crate::output::afni_name(&o.prefix)
            )));
        }
        common::refuse_existing(&o.prefix, mode, settings, log)?;
    }
    if o.verbose > 0 {
        if o.tshift.is_some() {
            log.info(&format!("Time shifting input dataset {shown}"));
        } else {
            log.info(&format!("Reading input dataset {shown}"));
        }
    }
    // §8.2: -tshift, in place, before anything reads the series.
    let mut image = image;
    let mut shifted = false;
    if let Some(ii) = o.tshift {
        let times = image
            .slice_timing
            .as_ref()
            .map(|h| h.times.clone())
            .unwrap_or_default();
        let tr = image.tr;
        let usable = tr > 0.0
            && times.iter().all(|&t| (0.0..=tr).contains(&t))
            && times.iter().any(|&t| t != times[0]);
        let done = usable && {
            let params = crate::tshift::TshiftParams {
                tr,
                slice_times: times.clone(),
                tzero: timing::mean_time(&times),
                ignore: ii,
                method: crate::tshift::Method::Fourier,
                restore: crate::tshift::Restore::Trend,
                detrend: true,
                n_threads,
                upstream_defects: mode,
            };
            match crate::tshift::tshift(&mut image.bricks, &params) {
                Ok(ns) => {
                    for n in ns {
                        log.notice(&n);
                    }
                    true
                }
                Err(_) => false,
            }
        };
        if done {
            shifted = true;
        } else {
            log.info("WARNING: some error during -tshift operation!");
        }
    }
    let float_output = o.float || scaled;
    let final_method = o.final_method.unwrap_or(o.method);
    let params = VolregParams {
        base,
        method: o.method,
        final_method,
        clip: o.clip,
        zpad: o.zpad.unwrap_or(4),
        max_iter: o.max_iter,
        x_thresh: o.x_thresh,
        rot_thresh: o.rot_thresh,
        delta: o.delta,
        weight,
        wtinp: o.wtinp,
        edging,
        twopass: o.twopass,
        twoblur: o.twoblur,
        coarse,
        coarse_rotation: o.coarse_rotation,
        sinit: o.sinit,
        maxdisp: o.maxdisp,
        maxdisp_file: o.maxdisp1d.is_some(),
        coarse_clfrac: if o.maxdisp || mode.fix() { 0.333 } else { 0.5 },
        output,
        float_output,
        savedisp: o.savedisp.is_some(),
        verbose: o.verbose,
        rotparent,
        firfac: settings
            .get("AFNI_BLUR_FIRFAC")
            .and_then(|v| super::leading_number(v).map(|(x, _)| x)),
        firold: settings.yes("AFNI_BLUR_FIROLD"),
        n_threads,
        mode,
    };
    if !o.maxdisp && o.twopass && params.coarse_rotation && coarse.0 > 0 && coarse.1 > 0 {
        late.push(notices::afni_92(mode));
    }
    let header = image.header.clone();
    let mut geometry = image.geometry.clone();
    // After -tshift the output has no slice offsets and a time origin of 0.
    let (tr, toffset, mut slice_timing) = if shifted {
        (image.tr, 0.0, None)
    } else {
        (image.tr, image.toffset, image.slice_timing.clone())
    };
    // §8.9: -gridparent's origin (and its padded grid written plumb); the slice offsets padded
    // and moved by -rotparent's slices.
    if o.gridparent.is_some() {
        if pad != [0, 0] {
            geometry.m = cardinal_matrix(&out_grid);
        } else {
            let g = cardinal_matrix(&out_grid);
            for (row, c) in geometry.m.iter_mut().zip(&g) {
                row[3] = c[3];
            }
        }
        geometry.qform_mm = None;
    }
    if params.rotparent.is_some()
        && let Some(h) = slice_timing.as_mut()
    {
        let nz2 = out_grid.n[2] as i64;
        let old = h.times.clone();
        h.times = (0..nz2)
            .map(|k| {
                let src = k - ndz - pad[0] as i64;
                if (0..old.len() as i64).contains(&src) {
                    old[src as usize]
                } else {
                    0.0
                }
            })
            .collect();
    }
    let result = volreg::run(image.bricks, &grid, &params);
    let r = match result {
        Ok(r) => r,
        Err(e) => {
            for m in &e.messages {
                log.message(m);
            }
            // the option scan's notices too (AFNI-86, -90, -95), before the error
            print_notices(log, std::mem::take(late).iter().chain(&e.notices));
            return Err(if e.fatal_error {
                Stop::Fatal(e.fatal)
            } else {
                Stop::Raw(format!("** {}", e.fatal))
            });
        }
    };
    for m in &r.messages {
        log.message(m);
    }
    if volreg::rotate::NO_SHEARS.load(std::sync::atomic::Ordering::Relaxed) {
        log.raw("*** THD_rota_vol: can't compute shear transformation!");
    }
    // §10.2: the outputs, in AFNI's order.
    if let Some(ff) = &o.matrix_save {
        let name = if ff.ends_with(".1D") {
            ff.clone()
        } else {
            format!("{ff}.aff12.1D")
        };
        let mut text = String::from("# 3dvolreg matrices (DICOM-to-DICOM, row-by-row):\n");
        for m in &r.matrices {
            text.push_str(&report::matrix_text(m));
            text.push('\n');
        }
        write_text(&name, &text, None, true, log);
    }
    for m in &r.maxdisp_messages {
        log.message(m);
    }
    if let (Some(mm), Some(md)) = (&o.maxdisp1d, &r.maxdisp) {
        let cmd = command_line(&o.command, settings);
        let mut a = format!("# {cmd}\n# max displacement (mm) for each volume\n");
        let mut b = format!("# {cmd}\n# max delta displ (mm) for each volume\n");
        for (d, e) in &md.per_volume {
            a.push_str(&format!(" {}\n", crate::cfmt::fixed(f64::from(*d), 3)));
            b.push_str(&format!(" {}\n", crate::cfmt::fixed(f64::from(*e), 3)));
        }
        if mm == "-" {
            log.stdout(a.trim_end_matches('\n'));
        } else {
            write_text(mm, &a, Some("*+"), true, log);
        }
        write_text(&delta_name(mm), &b, Some("*+"), true, log);
    }
    // §8.10: -twodup moves the written outputs' origin to the base dataset's.
    if o.twodup
        && let Some(bg) = &base_geometry
    {
        for i in 0..3 {
            geometry.m[i][3] = bg.m[i][3];
        }
        if let (Some(q), Some(bq)) = (geometry.qform_mm.as_mut(), bg.qform_mm.as_ref()) {
            for i in 0..3 {
                q[i][3] = bq[i][3];
            }
        }
    }
    let mut notices = r.notices.clone();
    let mut r = r;
    if let Some(mut bricks) = r.registered.take() {
        bricks.factors = vec![0.0; nt];
        let slices = slice_timing
            .as_ref()
            .map(|h| timing::copy_slice_fields(&h.times, mode).0);
        let timing = OutputTiming {
            tr,
            toffset,
            slices,
        };
        let (hdr, hnotices, hmessages) =
            dataset::output_header(&geometry, &bricks, &timing, None, mode, settings)
                .map_err(Stop::Fatal)?;
        for m in &hmessages {
            log.message(m);
        }
        for n in hnotices {
            defects::push_once(&mut notices, n);
        }
        let _ = header;
        let target = Target {
            option: "-prefix",
            prefix: &o.prefix,
            // 3dvolreg does not check -prefix at the scan (§10.1): refused names are named
            // (AFNI-12) when the output is written
            refused_at_scan: false,
        };
        // AFNI-90: by default a -prefix of 256 bytes or more is kept whole; a file name the
        // system cannot hold fails as the replica's does: AFNI-26 and the notices, then the error.
        if let Some(e) = unwritable_name(&o.prefix) {
            let unwritable = notices::afni_26_unwritable(mode);
            let pending: Vec<Notice> = std::mem::take(late);
            print_notices(log, pending.iter().chain(&notices).chain([&unwritable]));
            return Err(Stop::Fatal(format!(
                "can't write output dataset: cannot replace '{}': {e}; nothing was written",
                o.prefix
            )));
        }
        let mut wnotices = Vec::new();
        let written = common::write_dataset(
            &target,
            &hdr,
            &bricks,
            n_threads,
            mode,
            settings,
            log,
            &mut wnotices,
        )?;
        for n in wnotices {
            defects::push_once(&mut notices, n);
        }
        if o.verbose > 0
            && let Some((name, path)) = &written
            && Path::new(path).is_file()
        {
            log.info(&format!("Wrote dataset to disk in {name}"));
        }
    }
    if let (Some((s, all)), Some(disp)) = (&o.savedisp, r.displacements.as_mut()) {
        let total = nx * ny * nz;
        if *all {
            // -SAVEDISP: the three normalised, then their sixteen products.
            for d in disp.iter_mut() {
                report::normalise_displacements(d, total);
            }
        }
        // Each dataset made just before it is written (one series at a time beyond the three).
        let mut sets: Vec<(&str, Option<[usize; 3]>)> =
            vec![("DX", None), ("DY", None), ("DZ", None)];
        if *all {
            sets.extend(report::SAVEDISP_PRODUCTS.iter().map(|&(n, c)| (n, Some(c))));
        }
        for (q, (axis, counts)) in sets.into_iter().enumerate() {
            let name = savedisp_name(s, axis);
            let data = match counts {
                None => disp[q].clone(),
                Some(c) => report::savedisp_product(disp, c, total),
            };
            let bricks = crate::dataset::Bricks {
                shape: [nx, ny, nz, nt],
                data: BrickData::Float(data),
                factors: vec![0.0; nt],
            };
            // the output's header (§10.3), its slice fields included (observed)
            let timing = OutputTiming {
                tr,
                toffset,
                slices: slice_timing
                    .as_ref()
                    .map(|h| timing::copy_slice_fields(&h.times, mode).0),
            };
            let (hdr, _, _) =
                dataset::output_header(&geometry, &bricks, &timing, None, mode, settings)
                    .map_err(Stop::Fatal)?;
            let target = Target {
                option: "-savedisp",
                prefix: &name,
                refused_at_scan: false,
            };
            let mut wn = Vec::new();
            let written = common::write_dataset(
                &target, &hdr, &bricks, n_threads, mode, settings, log, &mut wn,
            )?;
            if o.verbose > 0
                && let Some((nm, path)) = &written
                && Path::new(path).is_file()
            {
                log.info(&format!("Wrote dataset to disk in {nm}"));
            }
        }
    }
    if let Some(f) = &o.dfile {
        let mut text = String::new();
        for (k, (p, rms)) in r.reported.iter().zip(&r.rms).enumerate() {
            text.push_str(&report::dfile_line(k, p, *rms));
            text.push('\n');
        }
        write_text(f, &text, Some("**"), false, log);
    }
    if let Some(f) = &o.oned {
        let mut text = String::new();
        for p in &r.reported {
            text.push_str(&report::oned_line(p));
            text.push('\n');
        }
        write_text(f, &text, Some("**"), false, log);
    }
    if o.rotcom {
        let s = if nt > 1 { "s" } else { "" };
        let mut text = format!("\n3drotate fragment{s}:\n\n");
        for p in &r.reported {
            text.push_str(&report::rotcom_line(
                final_method.rotate_option(),
                o.clip,
                params.zpad,
                p,
            ));
            text.push('\n');
        }
        // then a blank line (§10.2)
        text.push('\n');
        log.stdout(&text);
    }
    if scaled && output && !o.float {
        log.warning("Input dataset has scale factors ==> output is stored as floats!");
    }
    print_notices(log, late.iter().chain(&notices));
    Ok(0)
}

/// The dataset's name in AFNI-98's two messages: with `-input` AFNI names the argument after the
/// options (`(null)` when there is none); by default the dataset; AFNI-98 named where they differ.
fn afni_98_name(o: &Options, arg_name: &str, mode: UpstreamDefects, log: &mut Log<'_>) -> String {
    if !o.by_input {
        return arg_name.to_owned();
    }
    let afni = o.rest.first().cloned().unwrap_or_else(|| "(null)".into());
    if afni != arg_name {
        log.notice(&notices::afni_98(mode));
    }
    if mode.reproduce() {
        afni
    } else {
        arg_name.to_owned()
    }
}

/// Prints notices as the replica orders them: by the defect's ID (a stable sort: one defect's
/// several notices in their order), each notice once.
fn print_notices<'a>(log: &mut Log<'_>, notices: impl IntoIterator<Item = &'a Notice>) {
    let mut all: Vec<&Notice> = Vec::new();
    for n in notices {
        if !all.contains(&n) {
            all.push(n);
        }
    }
    all.sort_by_key(|n| n.defect.id);
    for n in all {
        log.notice(n);
    }
}

/// The notices the option scan has met so far, printed before a scan's error.
fn scan_notices(o: &Options, mode: UpstreamDefects) -> Vec<Notice> {
    let mut out = o.late.clone();
    if o.nocoarserot {
        out.push(afni_86(mode));
    }
    if o.base_twice && mode.reproduce() {
        out.push(afni_95(mode));
    }
    out
}

/// The system's refusal of an output whose file name is longer than 255 bytes (AFNI-90's names
/// kept whole by default), or `None`.
fn unwritable_name(prefix: &str) -> Option<std::io::Error> {
    let long = Path::new(prefix)
        .file_name()
        .is_some_and(|n| n.as_encoded_bytes().len() > 255);
    if !long {
        return None;
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(prefix)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(prefix);
            None
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => None,
        Err(e) => Some(e),
    }
}

/// The last argument of the command line (AFNI's "Too few arguments" message).
fn rest_last(o: &Options) -> String {
    o.command.last().cloned().unwrap_or_default()
}

/// `-savedisp`'s output for one axis: `<stem>_<axis><suffix>`.
fn savedisp_name(s: &str, axis: &str) -> String {
    let lower = s.to_ascii_lowercase();
    for suffix in [".nii.gz", ".nii"] {
        if lower.ends_with(suffix) {
            let (stem, suf) = s.split_at(s.len() - suffix.len());
            return format!("{stem}_{axis}{suf}");
        }
    }
    format!("{s}_{axis}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_names() {
        assert_eq!(delta_name("md.1D"), "md_delt.1D");
        assert_eq!(delta_name("md"), "md_delt");
        assert_eq!(delta_name("a.nii.gz"), "a_delt.nii.gz");
        assert_eq!(delta_name("-"), "-_delt");
        assert_eq!(savedisp_name("sd.nii", "DX"), "sd_DX.nii");
        assert_eq!(savedisp_name("sd", "DY"), "sd_DY");
    }

    #[test]
    fn edging_values() {
        assert_eq!(edging_value("5%"), Ok(Edging::Percent(5.0)));
        assert_eq!(edging_value("3"), Ok(Edging::Voxels(3.0)));
        assert_eq!(edging_value("26%"), Err("percentage"));
        assert_eq!(edging_value("-1"), Err("value"));
    }
}
