// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl fast`: FAST's command line (`specs/fast.md` §2, §3, §7): the options, the usage
//! and its errors, the images (the last words), the messages and exit statuses, and every
//! output of §3.3, on the clean-room segmentation of [`crate::fast`].

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::math::Libm;
use larmorx_mri::hmc::files;

use super::Settings;
use super::bet2::{FSLOUTPUTTYPE_UNSET, read_image, resolve_image};
use super::common::{TRUNCATED, abort, image_exception, print_notices};
use super::fmt::g;
use super::options::{self, Given, Spec};
use crate::bet::image::{self, Output, Values};
use crate::fast::multi::Multi;
use crate::fast::{self, Params, Single, Stop};

/// FAST's usage text (§2.3), printed on standard error.
pub const USAGE: &str = "\nPart of FSL (ID: 2412.6-dirty)\nFAST \nCopyright(c) 2004-2012, University of Oxford\n\nUsage: \nfast [options] file(s)\n\nOptional arguments (You may optionally specify one or more of):\n\t-n,--class\tnumber of tissue-type classes; default=3\n\t-I,--iter\tnumber of main-loop iterations during bias-field removal; default=4\n\t-l,--lowpass\tbias field smoothing extent (FWHM) in mm; default=20\n\t-t,--type\ttype of image 1=T1, 2=T2, 3=PD; default=T1\n\t-f,--fHard\tinitial segmentation spatial smoothness (during bias field estimation); default=0.02\n\t-g,--segments\toutputs a separate binary image for each tissue type\n\t-a <standard2input.mat> initialise using priors; you must supply a FLIRT transform\n\t-A <prior1> <prior2> <prior3>    alternative prior images\n\t--nopve\tturn off PVE (partial volume estimation)\n\t-b\t\toutput estimated bias field\n\t-B\t\toutput bias-corrected image\n\t-N,--nobias\tdo not remove bias field\n\t-S,--channels\tnumber of input images (channels); default 1\n\t-o,--out\toutput basename\n\t-P,--Prior\tuse priors throughout; you must also set the -a option\n\t-W,--init\tnumber of segmentation-initialisation iterations; default=15\n\t-R,--mixel\tspatial smoothness for mixeltype; default=0.3\n\t-O,--fixed\tnumber of main-loop iterations after bias-field removal; default=4\n\t-H,--Hyper\tsegmentation spatial smoothness; default=0.1\n\t-v,--verbose\tswitch on diagnostic messages\n\t-h,--help\tdisplay this message\n\t-s,--manualseg <filename> Filename containing intensities\n\t-p\t\toutputs individual probability maps\n\n\n";

/// The `descrip` of the images larmorx's FAST writes.
pub const DESCRIP: &str = "larmorx fsl fast";

const fn spec(short: Option<char>, long: Option<&'static str>, values: usize, kind: u8) -> Spec {
    Spec {
        short,
        long,
        values,
        numeric: kind > 0,
        integer: kind == 2,
    }
}

const CLASS: usize = 0;
const ITER: usize = 1;
const LOWPASS: usize = 2;
const TYPE: usize = 3;
const FHARD: usize = 4;
const SEGMENTS: usize = 5;
const PRIORS: usize = 6;
const ALT_PRIORS: usize = 7;
const NOPVE: usize = 8;
const BIAS_OUT: usize = 9;
const RESTORE: usize = 10;
const NOBIAS: usize = 11;
const CHANNELS: usize = 12;
const OUT: usize = 13;
const PRIOR_ALL: usize = 14;
const INIT: usize = 15;
const MIXEL: usize = 16;
const FIXED: usize = 17;
const HYPER: usize = 18;
const VERBOSE: usize = 19;
const HELP: usize = 20;
const MANUAL: usize = 21;
const PROBS: usize = 22;

/// The options in the usage's order (0 a flag or text, 1 a number, 2 an integer).
const SPECS: [Spec; 23] = [
    spec(Some('n'), Some("class"), 1, 2),
    spec(Some('I'), Some("iter"), 1, 2),
    spec(Some('l'), Some("lowpass"), 1, 1),
    spec(Some('t'), Some("type"), 1, 2),
    spec(Some('f'), Some("fHard"), 1, 1),
    spec(Some('g'), Some("segments"), 0, 0),
    spec(Some('a'), None, 1, 0),
    spec(Some('A'), None, 3, 0),
    spec(None, Some("nopve"), 0, 0),
    spec(Some('b'), None, 0, 0),
    spec(Some('B'), None, 0, 0),
    spec(Some('N'), Some("nobias"), 0, 0),
    spec(Some('S'), Some("channels"), 1, 2),
    spec(Some('o'), Some("out"), 1, 0),
    spec(Some('P'), Some("Prior"), 0, 0),
    spec(Some('W'), Some("init"), 1, 2),
    spec(Some('R'), Some("mixel"), 1, 1),
    spec(Some('O'), Some("fixed"), 1, 2),
    spec(Some('H'), Some("Hyper"), 1, 1),
    spec(Some('v'), Some("verbose"), 0, 0),
    spec(Some('h'), Some("help"), 0, 0),
    spec(Some('s'), Some("manualseg"), 1, 0),
    spec(Some('p'), None, 0, 0),
];

/// The command line, read.
#[derive(Clone, Debug)]
pub struct Options {
    pub n_classes: i64,
    pub bias_iterations: i64,
    pub lowpass: f32,
    pub image_type: i64,
    pub f_hard: f32,
    pub segments: bool,
    pub priors_matrix: Option<String>,
    pub priors: Option<Vec<String>>,
    pub pve: bool,
    pub bias_out: bool,
    pub restore: bool,
    pub no_bias: bool,
    pub channels: i64,
    pub out: Option<String>,
    pub priors_throughout: bool,
    pub init_iterations: i64,
    pub mixel: f32,
    pub fixed_iterations: i64,
    pub hyper: f32,
    pub verbose: bool,
    pub manual: Option<String>,
    pub probabilities: bool,
    /// The images, channel 1 first: the command line's last `-S` words (channel 1 the last; at
    /// most as many as there are words).
    pub images: Vec<String>,
}

/// What reading the command line gave.
pub enum Parsed {
    Run(Box<Options>),
    Exit(u8),
}

/// Reads the command line (§2.1, §2.4).
pub fn parse(args: &[String], err: &mut dyn Write) -> Parsed {
    if args.is_empty() {
        let _ = err.write_all(USAGE.as_bytes());
        return Parsed::Exit(1);
    }
    let given = match options::parse(args, &SPECS, true) {
        Ok((g, _)) => g,
        Err(e) => {
            let _ = err.write_all(USAGE.as_bytes());
            let _ = writeln!(err, "\n{}", e.0);
            return Parsed::Exit(1);
        }
    };
    if given[HELP].is_some() {
        let _ = err.write_all(USAGE.as_bytes());
        return Parsed::Exit(1);
    }
    let int = |k: usize, d: i64| match &given[k] {
        Some(Given::Numbers(v)) => v[0] as i64,
        _ => d,
    };
    let num = |k: usize, d: f32| match &given[k] {
        Some(Given::Numbers(v)) => v[0] as f32,
        _ => d,
    };
    let text = |k: usize| match &given[k] {
        Some(Given::Text(t)) => Some(t.clone()),
        _ => None,
    };
    let flag = |k: usize| given[k].is_some();
    let o = Options {
        n_classes: int(CLASS, 3),
        bias_iterations: int(ITER, 4),
        lowpass: num(LOWPASS, 20.0),
        image_type: int(TYPE, 1),
        f_hard: num(FHARD, 0.02),
        segments: flag(SEGMENTS),
        priors_matrix: text(PRIORS),
        priors: match &given[ALT_PRIORS] {
            Some(Given::Words(w)) => Some(w.clone()),
            _ => None,
        },
        pve: !flag(NOPVE),
        bias_out: flag(BIAS_OUT),
        restore: flag(RESTORE),
        no_bias: flag(NOBIAS),
        channels: int(CHANNELS, 1),
        out: text(OUT),
        priors_throughout: flag(PRIOR_ALL),
        init_iterations: int(INIT, 15),
        mixel: num(MIXEL, 0.3),
        fixed_iterations: int(FIXED, 4),
        hyper: num(HYPER, 0.1),
        verbose: flag(VERBOSE),
        manual: text(MANUAL),
        probabilities: flag(PROBS),
        images: Vec::new(),
    };
    let take = o.channels.clamp(1, args.len() as i64) as usize;
    let o = Options {
        images: args.iter().rev().take(take).cloned().collect(),
        ..o
    };
    Parsed::Run(Box::new(o))
}

/// `-s`'s numbers as C++'s stream reads floats: decimal numbers separated by white space, up to
/// the first word that is not one (a number followed by other characters ends the list after
/// it).
pub fn stream_floats(text: &str) -> Vec<f32> {
    let mut out = Vec::new();
    for word in text.split_ascii_whitespace() {
        let b = word.as_bytes();
        let mut i = 0;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let d0 = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let mut digits = i - d0;
        if i < b.len() && b[i] == b'.' {
            i += 1;
            let f0 = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            digits += i - f0;
        }
        if digits == 0 {
            break;
        }
        if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            let mut j = i + 1;
            if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                j += 1;
            }
            let e0 = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > e0 {
                i = j;
            }
        }
        match word[..i].parse::<f32>() {
            Ok(v) => out.push(v),
            Err(_) => break,
        }
        if i < b.len() {
            break;
        }
    }
    out
}

/// Runs FAST.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let o = match parse(args, err) {
        Parsed::Run(o) => o,
        Parsed::Exit(code) => return code,
    };
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl fast: {e} (larmorx)");
            return 1;
        }
    };
    run(&o, settings, mode, out, err)
}

/// A refusal of larmorx's (§7): `larmorx: error: fast: <why>`, exit status 1.
fn refuse(err: &mut dyn Write, notices: &[Notice], why: &str) -> u8 {
    print_notices(err, notices);
    let _ = writeln!(err, "larmorx: error: fast: {why}");
    1
}

/// FSL-99's options without meaning, as the refusal names them.
fn meaningless(o: &Options) -> Option<String> {
    let mut bad = Vec::new();
    for (name, v) in [("-H", o.hyper), ("-f", o.f_hard), ("-R", o.mixel)] {
        if !v.is_finite() {
            bad.push(format!("{name} {}", g(f64::from(v))));
        }
    }
    if !(o.lowpass > 0.0 && o.lowpass.is_finite()) {
        bad.push(format!("-l {}", g(f64::from(o.lowpass))));
    }
    if o.n_classes < 1 {
        bad.push(format!("-n {}", o.n_classes));
    }
    (!bad.is_empty()).then(|| bad.join(", "))
}

/// One channel, read: the image after §4.1's negative-value rule (on channel 1's grid).
struct Channel {
    name: String,
    input: crate::bet::image::Input,
}

/// The names of FSL's standard priors, or `-A`'s (§6.1).
fn prior_names(o: &Options, settings: &Settings) -> Option<[String; 3]> {
    if let Some(w) = &o.priors {
        return Some([w[0].clone(), w[1].clone(), w[2].clone()]);
    }
    let dir = settings.get("FSLDIR")?;
    let base = format!("{dir}/data/standard/tissuepriors/avg152T1_");
    Some(["csf", "gray", "white"].map(|t| format!("{base}{t}")))
}

/// The shell's special characters: `reproduce` cannot reproduce their expansion in FAST's
/// FLIRT commands (FSL-102).
const SHELL_SPECIAL: &[char] = &[
    '|', '&', ';', '<', '>', '(', ')', '$', '`', '\\', '"', '\'', '*', '?', '[', ']', '#', '~',
    '{', '}', '!', '\n',
];

/// What the priors gave (§6).
enum Priors {
    /// Not used (no `-a`, a class count they do not serve, a prior missing).
    None,
    /// Registered: CSF, grey and white matter over channel 1's grid.
    Images([Vec<f32>; 3]),
    /// FSL-101: FLIRT's registered priors are missing.
    Missing,
    /// Stop with this exit status (the messages printed).
    Exit(u8),
}

/// `-a`: the checks, FLIRT's three runs in this process and the registered priors read back
/// (§6.1 to §6.5; FSL-100, FSL-101, FSL-102).
#[allow(clippy::too_many_arguments)]
fn register_priors(
    o: &Options,
    settings: &Settings,
    mode: UpstreamDefects,
    input_name: &str,
    shape: [usize; 3],
    notices: &mut Vec<Notice>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Priors {
    let Some(matrix) = &o.priors_matrix else {
        return Priors::None;
    };
    if !(o.n_classes == 2 || o.n_classes == 3) {
        let _ = writeln!(
            err,
            "Apriori can only be used for 2 or 3-class segmentation"
        );
        return Priors::None;
    }
    let Some(names) = prior_names(o, settings) else {
        // FSLDIR unset without -A: with reproduce FAST aborted before (FSL-100)
        return Priors::Exit(refuse(
            err,
            notices,
            "-a without -A needs FSLDIR for FSL's standard priors: name the priors with -A (FSLDIR is unset; FAST aborts: FSL-100)",
        ));
    };
    let mut scratch = Vec::new();
    let mut missing = false;
    for n in &names {
        if resolve_image(settings, n, mode, &mut scratch).is_err() {
            let _ = writeln!(err, "prior image {n} is not found! priors are not used!");
            missing = true;
        }
    }
    if missing {
        return Priors::None;
    }
    let root = files::strip_image_extension(o.out.as_deref().unwrap_or(input_name)).to_owned();
    let fsldir = settings.get("FSLDIR");
    let tissues = ["csf", "gm", "wm"];
    let words = [input_name, matrix.as_str(), root.as_str()]
        .into_iter()
        .chain(names.iter().map(String::as_str));
    let blank = words
        .clone()
        .any(|w| w.contains(|c: char| c.is_whitespace()));
    let special = words.clone().any(|w| w.contains(SHELL_SPECIAL));
    if blank || special || fsldir.is_none() {
        let why = if fsldir.is_none() {
            "FSLDIR is unset"
        } else {
            "a name holds a blank or a character the shell interprets"
        };
        if mode.fix() {
            notices.push(Notice::new(
                &defects::FSL_102,
                mode,
                format!(
                    "-a: {why}; FLIRT ran with each name one argument; FAST runs it through the shell, which splits or interprets the names"
                ),
            ));
        } else if special {
            return Priors::Exit(refuse(
                err,
                notices,
                "-a: a name holds a character the shell interprets; FAST's FLIRT commands go through the shell, whose expansion larmorx does not reproduce (FSL-102)",
            ));
        } else {
            notices.push(Notice::new(
                &defects::FSL_102,
                mode,
                format!(
                    "-a: {why}; FLIRT's commands are run as the shell splits them, as FAST does"
                ),
            ));
        }
    }
    for (t, prior) in tissues.iter().zip(&names) {
        let target = format!("{root}_{t}_stdspace");
        let line = format!(
            "{}/bin/flirt -ref {input_name} -in {prior} -out {target} -applyxfm -init {matrix}",
            fsldir.unwrap_or("(null)")
        );
        if o.verbose {
            let _ = writeln!(out, "{line}");
        }
        let args: Vec<String> = if mode.fix() {
            [
                "-ref",
                input_name,
                "-in",
                prior,
                "-out",
                &target,
                "-applyxfm",
                "-init",
                matrix,
            ]
            .iter()
            .map(|w| (*w).to_owned())
            .collect()
        } else if fsldir.is_none() {
            // FAST's command starts with "(null)": the shell's syntax error, FLIRT never runs
            let _ = writeln!(err, "sh: 1: Syntax error: word unexpected");
            continue;
        } else {
            line.split_whitespace().skip(1).map(str::to_owned).collect()
        };
        let _ = super::flirt::main(&args, settings, out, err);
    }
    let labels = ["csf", "grey matter", "white matter"];
    let mut data: Vec<Vec<f32>> = Vec::new();
    for (t, label) in tissues.iter().zip(labels) {
        let name = format!("{root}_{t}_stdspace");
        let mut scratch = Vec::new();
        match read_image(settings, &name, mode, &mut scratch, settings.threads()) {
            Ok(i) if i.vol.shape == shape => data.push(i.vol.data),
            _ => {
                let _ = writeln!(
                    err,
                    "{label} prior image not transformed correctly! priors are not used!"
                );
                return Priors::Missing;
            }
        }
    }
    let wm = data.pop().unwrap_or_default();
    let gm = data.pop().unwrap_or_default();
    let csf = data.pop().unwrap_or_default();
    Priors::Images([csf, gm, wm])
}

/// The channels' images differ in size: FSL-104's text.
fn sizes_text(o: &Options, ch: &[Channel]) -> String {
    let s = |c: &Channel| {
        let [x, y, z] = c.input.vol.shape;
        format!("{x} x {y} x {z}")
    };
    let other = ch
        .iter()
        .skip(1)
        .find(|c| c.input.vol.shape != ch[0].input.vol.shape)
        .map_or(String::new(), |c| format!("{} is {}", c.name, s(c)));
    format!(
        "-S {}: the channels' images differ in size (channel 1, {}, is {}; {other}): FAST reads every channel over channel 1's grid, past the end of smaller images (FSL-104)",
        o.channels,
        ch[0].name,
        s(&ch[0])
    )
}

/// Runs FAST with the options read.
pub fn run(
    o: &Options,
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut notices: Vec<Notice> = Vec::new();
    if o.hyper < 0.0 {
        let _ = writeln!(
            err,
            "ERROR: Segmentation smoothness must be positive. Exiting"
        );
        return 1;
    }
    if o.channels < 1 {
        let _ = write!(
            err,
            "At least 2 channels required for Multi Channel Segmentation"
        );
        return 1;
    }
    let multi = o.channels > 1;
    if o.images.len() < o.channels as usize {
        return refuse(
            err,
            &notices,
            &format!(
                "-S {}: the command line holds {} words; FAST would read its own name and memory beyond its arguments as images",
                o.channels,
                o.images.len()
            ),
        );
    }
    if let Some(why) = meaningless(o) {
        let n = Notice::new(
            &defects::FSL_99,
            mode,
            match mode {
                UpstreamDefects::Fix => format!("{why}: refused; FAST uses them as given"),
                UpstreamDefects::Reproduce => format!("{why}: used as given, as FAST does"),
            },
        );
        if mode.fix() {
            return refuse(
                err,
                &[n],
                &format!(
                    "{why}: a smoothness must be a finite number, the bias field's smoothing extent a positive one and the classes at least 1; FAST uses them as given (FSL-99)"
                ),
            );
        }
        notices.push(n);
        if o.n_classes < 0 {
            print_notices(err, &notices);
            if multi {
                return abort(
                    err,
                    "fast",
                    "std::logic_error",
                    "Mat::init(): requested size is too large",
                );
            }
            let what = if o.n_classes == -1 {
                "vector::_M_fill_insert"
            } else {
                "vector::_M_default_append"
            };
            return abort(err, "fast", "std::length_error", what);
        }
    }
    // FSL-100: FAST reads FSLDIR (for its standard priors) whether or not -a is given, unless
    // -A names the priors (observed: `-A a b c` without FSLDIR segments)
    if settings.get("FSLDIR").is_none() && o.priors.is_none() {
        match mode {
            UpstreamDefects::Reproduce => {
                print_notices(
                    err,
                    &[Notice::new(
                        &defects::FSL_100,
                        mode,
                        "FSLDIR is unset: FAST aborts before it segments",
                    )],
                );
                return abort(
                    err,
                    "fast",
                    "std::logic_error",
                    "basic_string: construction from null is not valid",
                );
            }
            UpstreamDefects::Fix => notices.push(Notice::new(
                &defects::FSL_100,
                mode,
                "FSLDIR is unset: segmented without it (no standard priors used); FAST aborts there",
            )),
        }
    }
    let n_threads = settings.threads();
    // the channels, channel 1 first (the last word)
    let mut channels: Vec<Channel> = Vec::new();
    for name in o.images.iter().take(o.channels as usize) {
        match read_image(settings, name, mode, &mut notices, n_threads) {
            Ok(i) => {
                if i.volumes > 1 {
                    let _ = writeln!(err, "{TRUNCATED}");
                }
                channels.push(Channel {
                    name: name.clone(),
                    input: i,
                });
            }
            Err(e) => {
                print_notices(err, &notices);
                let (lines, what) = image_exception(name, &e);
                let _ = err.write_all(lines.as_bytes());
                return abort(err, "fast", "std::runtime_error", &what);
            }
        }
    }
    let shape = channels[0].input.vol.shape;
    let voxel = channels[0].input.vol.voxel.map(|d| (d as f32).abs());
    // FSL-104: channels of different sizes
    let mut sizes_differ = false;
    if channels.iter().any(|c| c.input.vol.shape != shape) {
        let fits = channels.iter().all(|c| {
            let t = c.input.vol.shape;
            t[0] >= shape[0] && t[1] >= shape[1] && t[2] >= shape[2]
        });
        let text = sizes_text(o, &channels);
        if mode.fix() || !fits {
            let n = Notice::new(
                &defects::FSL_104,
                mode,
                "the channels' images differ in size: refused",
            );
            return refuse(err, &[n], &text);
        }
        notices.push(Notice::new(
            &defects::FSL_104,
            mode,
            "the channels' images differ in size: each is read over channel 1's grid (its corner), as FAST does",
        ));
        let [nx, ny, nz] = shape;
        for c in channels.iter_mut().skip(1) {
            let [cx, cy, _] = c.input.vol.shape;
            let src = &c.input.vol.data;
            let mut v = vec![0f32; nx * ny * nz];
            for z in 0..nz {
                for y in 0..ny {
                    for x in 0..nx {
                        v[(z * ny + y) * nx + x] = src[(z * cy + y) * cx + x];
                    }
                }
            }
            c.input.vol.data = v;
            c.input.vol.shape = shape;
        }
        sizes_differ = true;
    }
    if o.verbose {
        if multi {
            let _ = writeln!(out, "Starting Multi Image Segmentation");
        } else {
            let _ = writeln!(out, "Starting Single Image Segmentation");
            let _ = writeln!(
                out,
                "{}",
                match o.image_type {
                    2 => "T2-weighted image",
                    3 => "PD-weighted image",
                    _ => "T1-weighted image",
                }
            );
            let _ = writeln!(
                out,
                "Imagesize : {} x {} x {}",
                shape[0], shape[1], shape[2]
            );
            let _ = writeln!(
                out,
                "Pixelsize : {} x {} x {}",
                g(f64::from(voxel[0])),
                g(f64::from(voxel[1])),
                g(f64::from(voxel[2]))
            );
            let _ = writeln!(out);
        }
    }
    let manual = o
        .manual
        .as_ref()
        .and_then(|f| std::fs::read_to_string(settings.path(f)).ok())
        .map(|t| stream_floats(&t));
    // §4.1: the negative values, the mask
    for c in channels.iter_mut() {
        // on the call's pool (its parallel parts would otherwise use rayon's global pool)
        let data = &mut c.input.vol.data;
        if let Err(e) =
            larmorx_core::parallel::with_threads(n_threads, || fast::single::negative_rule(data))
        {
            return refuse(err, &notices, &e.to_string());
        }
    }
    let bound = fast::single::maximum(&channels[0].input.vol.data) + 1.0;
    if !multi {
        let data = &channels[0].input.vol.data;
        let differ = data
            .iter()
            .filter(|&&v| fast::single::in_mask(v) != fast::single::in_fsl_mask(v, bound))
            .count();
        if differ > 0 {
            notices.push(Notice::new(
                &defects::FSL_103,
                mode,
                match mode {
                    UpstreamDefects::Fix => format!(
                        "the mask keeps the {differ} voxels equal to the image's maximum ({}); FAST's leaves them out (its bound, the maximum + 1, rounds to the maximum in single precision)",
                        g(f64::from(bound))
                    ),
                    UpstreamDefects::Reproduce => format!(
                        "the mask leaves out the {differ} voxels equal to the image's maximum ({}), as FAST's does",
                        g(f64::from(bound))
                    ),
                },
            ));
        }
    }
    if o.n_classes == 0 {
        // FSL-99 with reproduce: no classes
        if o.pve && !multi {
            print_notices(err, &notices);
            return abort(
                err,
                "fast",
                "std::invalid_argument",
                "Mixeltype exceeds allowed types",
            );
        }
        let zeros = vec![0i32; shape.iter().product()];
        return write_outputs(
            o,
            settings,
            mode,
            &channels,
            &Outputs::Empty(zeros, o.pve && multi),
            notices,
            &Libm::quiet(mode),
            err,
        );
    }
    let fix = mode.fix();
    let in_mask = |i: usize| -> bool {
        if multi {
            channels.iter().all(|c| c.input.vol.data[i] > 0.0)
        } else {
            let v = channels[0].input.vol.data[i];
            if fix {
                fast::single::in_mask(v)
            } else {
                fast::single::in_fsl_mask(v, bound)
            }
        }
    };
    // §6: the priors
    let mut priors = match register_priors(
        o,
        settings,
        mode,
        &channels[0].name,
        shape,
        &mut notices,
        out,
        err,
    ) {
        Priors::Exit(code) => return code,
        Priors::Images(p) => Some(p),
        Priors::None => None,
        Priors::Missing => {
            // FSL-101
            let empty = !(0..shape.iter().product::<usize>()).any(in_mask);
            if mode.fix() {
                notices.push(Notice::new(
                    &defects::FSL_101,
                    mode,
                    "-a: FLIRT's registered priors are missing: segmented without priors, as FAST's message says; FAST then initialises from priors it never allocated",
                ));
                None
            } else if !multi && empty {
                notices.push(Notice::new(
                    &defects::FSL_101,
                    mode,
                    "-a: FLIRT's registered priors are missing and the mask is empty: FAST initialises from them, reading none",
                ));
                Some([Vec::new(), Vec::new(), Vec::new()])
            } else if multi
                && channels
                    .iter()
                    .all(|c| c.input.vol.data.first().is_some_and(|&v| v > 0.0))
            {
                if o.verbose {
                    // FAST starts the prior-based initialisation before it fails (observed)
                    let _ = writeln!(out, "Beginning prior-based initialisation");
                }
                print_notices(err, &notices);
                let _ = writeln!(err, "Image Exception : #5 :: Out of Bounds (time index)");
                return abort(
                    err,
                    "fast",
                    "std::runtime_error",
                    "Out of Bounds (time index)",
                );
            } else {
                if o.verbose {
                    let _ = writeln!(out, "Beginning prior-based initialisation");
                }
                return refuse(
                    err,
                    &notices,
                    "-a: FLIRT's registered priors are missing and FAST initialises from priors it never allocated: it crashes here (FSL-101)",
                );
            }
        }
    };
    let params = Params {
        n_classes: o.n_classes as usize,
        bias_iterations: o.bias_iterations,
        lowpass: o.lowpass,
        image_type: o.image_type,
        f_hard: o.f_hard,
        no_bias: o.no_bias,
        init_iterations: o.init_iterations,
        mixel: o.mixel,
        fixed_iterations: o.fixed_iterations,
        hyper: o.hyper,
        pve: o.pve,
        manual,
        verbose: o.verbose,
        probabilities: o.probabilities,
        priors: priors.take(),
        priors_throughout: o.priors_throughout,
        fsl98_pairing: !fix,
        // quiet: FAST names no GLIBC-07 (its iterations cannot tell whether a last bit of its
        // functions moved the segmentation; the user's decision of 2026-11-04)
        libm: std::sync::Arc::new(larmorx_core::math::Libm::quiet(mode)),
    };
    let result = larmorx_core::parallel::with_threads(n_threads, || {
        let mut text: Vec<u8> = Vec::new();
        let mut etext: Vec<u8> = Vec::new();
        let r = if multi {
            let data: Vec<&[f32]> = channels.iter().map(|c| &c.input.vol.data[..]).collect();
            fast::multi::segment(
                &data,
                shape,
                voxel,
                in_mask,
                &params,
                sizes_differ,
                &mut text,
            )
            .map(|m| Outputs::Multi(Box::new(m)))
        } else {
            fast::segment(
                &channels[0].input.vol.data,
                shape,
                voxel,
                in_mask,
                &params,
                &mut text,
                &mut etext,
            )
            .map(|s| Outputs::Single(Box::new(s)))
        };
        (r, text, etext)
    });
    let (r, text, etext) = match result {
        Ok(x) => x,
        Err(e) => return refuse(err, &notices, &e.to_string()),
    };
    let _ = out.write_all(&text);
    let _ = err.write_all(&etext);
    let what = match r {
        Ok(w) => w,
        Err(Stop::KMeans) => {
            let _ = writeln!(out, "Exception: Not enough classes detected to init KMeans");
            print_notices(err, &notices);
            return 255;
        }
        Err(Stop::Singular) => {
            print_notices(err, &notices);
            return abort(
                err,
                "fast",
                "std::runtime_error",
                "inv(): matrix is singular",
            );
        }
        Err(Stop::Sizes) => {
            print_notices(err, &notices);
            let what = "mask and vol have different sizes in calc_percentiles";
            let _ = writeln!(err, "Image Exception : #3 :: {what}");
            return abort(err, "fast", "std::runtime_error", what);
        }
        Err(Stop::TooLarge) => {
            return refuse(
                err,
                &notices,
                "the image has too many voxels for larmorx's FAST",
            );
        }
    };
    if let Outputs::Single(s) = &what {
        for _ in 0..s.empty_sums {
            let _ = writeln!(err, "ERROR:: Empty mask image");
        }
    }
    if let Outputs::Multi(m) = &what {
        if m.cycle {
            notices.push(Notice::new(
                &defects::FSL_98,
                mode,
                match mode {
                    UpstreamDefects::Fix => "the class order is a cycle: each sorted class keeps its own statistics and posteriors; FAST gives it another class's",
                    UpstreamDefects::Reproduce => "the class order is a cycle: each sorted class takes another class's statistics and posteriors, as FAST does",
                },
            ));
        }
        if m.unwritten_beta {
            notices.push(Notice::new(
                &defects::FSL_99,
                mode,
                "-H nan with several channels: FAST estimates β against a table it never writes; larmorx takes the table as zeros (β = 0)",
            ));
        }
    }
    write_outputs(
        o,
        settings,
        mode,
        &channels,
        &what,
        notices,
        &params.libm,
        err,
    )
}

/// What a run writes.
enum Outputs {
    /// `-n 0` (`reproduce`): the segmentation, all 0 (several channels with partial volumes:
    /// also `_pveseg` and `_mixeltype`, all 0).
    Empty(Vec<i32>, bool),
    Single(Box<Single>),
    Multi(Box<Multi>),
}

/// The per-class and per-channel outputs both kinds share.
struct Parts<'a> {
    grid: &'a fast::grid::Grid,
    n: usize,
    seg: &'a [u8],
    members: Option<&'a [f32]>,
    pve: Option<&'a fast::pve::Pve>,
    /// Each channel's bias field per mask voxel (`None`: no field).
    bias: Option<Vec<&'a [f32]>>,
    /// The field outside the mask (one channel: NaN when its mean is not finite, else ±0).
    bias_outside: f32,
}

/// Writes the outputs (§3.2, §3.3) in FAST's order, then the notices.
#[allow(clippy::too_many_arguments)]
fn write_outputs(
    o: &Options,
    settings: &Settings,
    mode: UpstreamDefects,
    channels: &[Channel],
    what: &Outputs,
    mut notices: Vec<Notice>,
    libm: &Libm,
    err: &mut dyn Write,
) -> u8 {
    let ot = match settings.get("FSLOUTPUTTYPE") {
        None => {
            print_notices(err, &notices);
            let _ = err.write_all(FSLOUTPUTTYPE_UNSET.as_bytes());
            return 1;
        }
        Some(t) if !image::OUTPUT_TYPES.contains(&t) => {
            print_notices(err, &notices);
            let _ = writeln!(err, "{}", image::unknown_output_type_message(t));
            return 1;
        }
        Some(t) => t.to_owned(),
    };
    let input = &channels[0].input;
    let root_name = o.out.clone().unwrap_or_else(|| channels[0].name.clone());
    let root = settings.name(files::strip_image_extension(&root_name));
    let n_threads = settings.threads();
    let ext = settings
        .get("FSL_LOAD_NIFTI_EXTENSIONS")
        .is_some_and(|v| options::atof(v) != 0.0);
    fn output_of<'a>(
        c: &'a Channel,
        shape: [usize; 3],
        ext: bool,
        mode: UpstreamDefects,
        n_threads: usize,
    ) -> Output<'a> {
        Output {
            template: &c.input.header,
            flipped: c.input.flipped,
            shape,
            volumes: 1,
            cal: Some((0.0, 0.0)),
            extensions: ext,
            defects: mode,
            n_threads,
            descrip: DESCRIP,
            rewrite_quaternion: true,
        }
    }
    let output = output_of(&channels[0], input.vol.shape, ext, mode, n_threads);
    let mut write = |suffix: String,
                     values: Values,
                     notices: &mut Vec<Notice>,
                     output: &Output<'_>|
     -> bool {
        let stem = format!("{root}{suffix}");
        let Some((name, version)) = image::output_file(&stem, &ot) else {
            return false;
        };
        match image::write(&name, version, output, values) {
            Ok(ns) => {
                for x in ns {
                    defects::push_once(notices, x);
                }
                true
            }
            Err(e) => {
                print_notices(err, notices);
                let _ = writeln!(
                    err,
                    "larmorx fsl fast: {name} cannot be written ({e}); FSL aborts here (larmorx)"
                );
                false
            }
        }
    };
    let parts = match what {
        Outputs::Empty(seg, pve) => {
            let names: &[&str] = if *pve {
                &["_seg", "_pveseg", "_mixeltype"]
            } else {
                &["_seg"]
            };
            for nm in names {
                if !write(
                    (*nm).into(),
                    Values::I32(seg.clone()),
                    &mut notices,
                    &output,
                ) {
                    return 1;
                }
            }
            print_notices(err, &notices);
            return 0;
        }
        Outputs::Single(s) => Parts {
            grid: &s.grid,
            n: s.n,
            seg: &s.seg,
            members: s.members.as_deref(),
            pve: s.pve.as_ref(),
            bias: s.bias.as_ref().map(|b| vec![&b[..]]),
            bias_outside: if s.bias.is_some() {
                s.bias_outside
            } else {
                0.0
            },
        },
        Outputs::Multi(m) => Parts {
            grid: &m.grid,
            n: m.n,
            seg: &m.seg,
            members: Some(&m.members[..]),
            pve: m.pve.as_ref(),
            bias: m.bias.as_ref().map(|b| b.iter().map(|v| &v[..]).collect()),
            bias_outside: 0.0,
        },
    };
    let grid = parts.grid;
    let n = parts.n;
    let multi = channels.len() > 1;
    let label = |v: &[u8]| -> Vec<i32> { grid.expand_on(n_threads, |i| i32::from(v[i]), 0) };
    if !write(
        "_seg".into(),
        Values::I32(label(parts.seg)),
        &mut notices,
        &output,
    ) {
        return 1;
    }
    if o.segments {
        for c in 0..n {
            let target = (c + 1) as u8;
            let v = grid.expand_on(n_threads, |i| i32::from(parts.seg[i] == target), 0);
            if !write(format!("_seg_{c}"), Values::I32(v), &mut notices, &output) {
                return 1;
            }
        }
    }
    if o.probabilities {
        let outside = grid.len() < grid.voxels();
        match parts.members {
            Some(q) => {
                if outside {
                    defects::push_once(
                        &mut notices,
                        Notice::new(
                            &defects::FSL_96,
                            mode,
                            format!(
                                "-p's probability maps are 0 at the {} voxels outside the mask, where FAST never writes them",
                                grid.voxels() - grid.len()
                            ),
                        ),
                    );
                }
                for c in 0..n {
                    let v = grid.expand_on(n_threads, |i| q[i * n + c], 0.0);
                    if !write(format!("_prob_{c}"), Values::F32(v), &mut notices, &output) {
                        return 1;
                    }
                }
            }
            None => {
                defects::push_once(
                    &mut notices,
                    Notice::new(
                        &defects::FSL_96,
                        mode,
                        "-p's probability maps are 0 everywhere: FAST never computes them here (--nopve with the priors or without an initialisation iteration)",
                    ),
                );
                for c in 0..n {
                    let v = vec![0f32; grid.voxels()];
                    if !write(format!("_prob_{c}"), Values::F32(v), &mut notices, &output) {
                        return 1;
                    }
                }
            }
        }
    }
    if let Some(p) = parts.pve {
        for c in 0..n {
            let v = grid.expand_on(n_threads, |i| p.frac[i * n + c], 0.0);
            if !write(format!("_pve_{c}"), Values::F32(v), &mut notices, &output) {
                return 1;
            }
        }
        if !write(
            "_pveseg".into(),
            Values::I32(label(&p.seg)),
            &mut notices,
            &output,
        ) {
            return 1;
        }
        if !write(
            "_mixeltype".into(),
            Values::I32(label(&p.mixel)),
            &mut notices,
            &output,
        ) {
            return 1;
        }
    }
    if multi && parts.bias.is_none() && (o.restore || o.bias_out) {
        notices.push(Notice::new(
            &defects::FSL_97,
            mode,
            "-N with several channels: no bias field, so _bias_<c> is 1 and _restore_<c> the channel's image; FAST never computes the field it writes (memory never written)",
        ));
    }
    // the correction exp(-b) of channel c: 1 without a bias field; outside the mask exp(-b) of
    // b's value there (1, or NaN when the field's mean is not finite: FSL's NaN then reaches the
    // whole image, §4.5's b · M)
    let outside = libm.expf(-parts.bias_outside);
    let correction = |c: usize, i: usize| -> f32 {
        match &parts.bias {
            Some(b) => libm.expf(-b[c][i]),
            None => 1.0,
        }
    };
    let suffix = |base: &str, c: usize| {
        if multi {
            format!("{base}_{}", c + 1)
        } else {
            base.to_owned()
        }
    };
    if o.restore {
        for (c, ch) in channels.iter().enumerate() {
            // NaN voxels are written as 0, the others outside the mask as they are (observed: a
            // voxel equal to a maximum of 2^25, outside FSL-103's mask, keeps its value)
            let mut v: Vec<f32> = ch
                .input
                .vol
                .data
                .iter()
                .map(|&x| if x.is_nan() { 0.0 } else { x * outside })
                .collect();
            for (i, &idx) in grid.index.iter().enumerate() {
                let x = ch.input.vol.data[idx as usize];
                v[idx as usize] = if x.is_nan() { 0.0 } else { x } * correction(c, i);
            }
            let own = output_of(ch, input.vol.shape, ext, mode, n_threads);
            if !write(suffix("_restore", c), Values::F32(v), &mut notices, &own) {
                return 1;
            }
        }
    }
    if o.bias_out {
        for c in 0..channels.len() {
            let v = grid.expand_on(n_threads, |i| 1.0 / correction(c, i), 1.0 / outside);
            if !write(suffix("_bias", c), Values::F32(v), &mut notices, &output) {
                return 1;
            }
        }
    }
    print_notices(err, &notices);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn command_line() {
        let mut err = Vec::new();
        assert!(matches!(parse(&[], &mut err), Parsed::Exit(1)));
        assert!(String::from_utf8_lossy(&err).ends_with("probability maps\n\n\n"));
        let mut err = Vec::new();
        assert!(matches!(
            parse(&w(&["--bogus", "x"]), &mut err),
            Parsed::Exit(1)
        ));
        assert!(
            String::from_utf8_lossy(&err).ends_with("maps\n\n\n\n--bogus: Option doesn't exist!\n")
        );
        let mut err = Vec::new();
        let Parsed::Run(o) = parse(
            &w(&["-gp", "-n", "0x2", "-I", "010", "--out=s", "in.nii"]),
            &mut err,
        ) else {
            panic!("parsed")
        };
        assert!(o.segments && o.probabilities);
        assert_eq!((o.n_classes, o.bias_iterations), (2, 8));
        assert_eq!(o.out.as_deref(), Some("s"));
        assert_eq!(o.images, ["in.nii"]);
        let mut err = Vec::new();
        assert!(matches!(
            parse(&w(&["-n", "2.5", "x"]), &mut err),
            Parsed::Exit(1)
        ));
        assert!(
            String::from_utf8_lossy(&err).ends_with("-n: Couldn't set_value! valstr=\"2.5 x\"!\n")
        );
        assert_eq!(stream_floats("1 2.5\n3e1 x 4"), vec![1.0, 2.5, 30.0]);
        assert_eq!(stream_floats("1 2abc 3"), vec![1.0, 2.0]);
    }
}
