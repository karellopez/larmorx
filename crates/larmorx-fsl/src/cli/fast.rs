// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl fast`: FAST's command line (`specs/fast.md` §2, §3, §7): the options, the usage
//! and its errors, the images (the last words), the messages and exit statuses, and every
//! output of §3.3, on the clean-room segmentation of [`crate::fast`].

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_mri::hmc::files;

use super::Settings;
use super::bet2::{FSLOUTPUTTYPE_UNSET, read_image};
use super::common::{TRUNCATED, abort, image_exception, print_notices};
use super::fmt::g;
use super::options::{self, Given, Spec};
use crate::bet::image::{self, Output, Values};
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
    /// The images: the command line's last words (channel 1 last).
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
        images: args.last().cloned().into_iter().collect(),
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
    if o.channels != 1 {
        if o.channels < 1 {
            let _ = write!(
                err,
                "At least 2 channels required for Multi Channel Segmentation"
            );
            return 1;
        }
        return refuse(
            err,
            &notices,
            &format!(
                "-S {}: the clean-room original segments one channel so far (the multi-channel segmentation is the next part of its work); use the replica (larmorx-nc)",
                o.channels
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
            let what = if o.n_classes == -1 {
                "vector::_M_fill_insert"
            } else {
                "vector::_M_default_append"
            };
            return abort(err, "fast", "std::length_error", what);
        }
    }
    if o.priors_matrix.is_some() {
        return refuse(
            err,
            &notices,
            "-a: the clean-room original does not register priors yet (the next part of its work); use the replica (larmorx-nc)",
        );
    }
    // FSL-100: FAST reads FSLDIR (for its standard priors) whether or not -a is given
    if settings.get("FSLDIR").is_none() {
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
                "FSLDIR is unset: segmented without it (no priors asked for); FAST aborts there",
            )),
        }
    }
    let n_threads = settings.threads();
    let name = o.images.last().cloned().unwrap_or_default();
    let mut input = match read_image(settings, &name, mode, &mut notices, n_threads) {
        Ok(i) => i,
        Err(e) => {
            print_notices(err, &notices);
            let (lines, what) = image_exception(&name, &e);
            let _ = err.write_all(lines.as_bytes());
            return abort(err, "fast", "std::runtime_error", &what);
        }
    };
    if input.volumes > 1 {
        let _ = writeln!(err, "{TRUNCATED}");
    }
    let shape = input.vol.shape;
    let voxel = input.vol.voxel.map(|d| (d as f32).abs());
    if o.verbose {
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
        let _ = writeln!(out, "Imagesize : {} x {} x {}", shape[0], shape[1], shape[2]);
        let _ = writeln!(
            out,
            "Pixelsize : {} x {} x {}",
            g(f64::from(voxel[0])),
            g(f64::from(voxel[1])),
            g(f64::from(voxel[2]))
        );
        let _ = writeln!(out);
    }
    let manual = o
        .manual
        .as_ref()
        .and_then(|f| std::fs::read_to_string(settings.path(f)).ok())
        .map(|t| stream_floats(&t));
    // §4.1: the negative values, the mask
    fast::single::negative_rule(&mut input.vol.data);
    let data = &input.vol.data;
    let bound = fast::single::maximum(data) + 1.0;
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
    if o.n_classes == 0 {
        // FSL-99 with reproduce: no classes
        if o.pve {
            print_notices(err, &notices);
            return abort(
                err,
                "fast",
                "std::invalid_argument",
                "Mixeltype exceeds allowed types",
            );
        }
        let seg = vec![0i32; data.len()];
        return write_outputs(
            o,
            settings,
            mode,
            &input,
            &Outputs::Empty(seg),
            notices,
            err,
        );
    }
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
    };
    let fix = mode.fix();
    let result = larmorx_core::parallel::with_threads(n_threads, || {
        let mut text: Vec<u8> = Vec::new();
        let r = fast::segment(
            data,
            shape,
            voxel,
            |i| {
                let v = data[i];
                if fix {
                    fast::single::in_mask(v)
                } else {
                    fast::single::in_fsl_mask(v, bound)
                }
            },
            &params,
            &mut text,
        );
        (r, text)
    });
    let (r, text) = match result {
        Ok(x) => x,
        Err(e) => return refuse(err, &notices, &e.to_string()),
    };
    let _ = out.write_all(&text);
    let single = match r {
        Ok(s) => s,
        Err(Stop::KMeans) => {
            let _ = writeln!(out, "Exception: Not enough classes detected to init KMeans");
            print_notices(err, &notices);
            return 255;
        }
        Err(Stop::TooLarge) => {
            return refuse(err, &notices, "the image has too many voxels for larmorx's FAST");
        }
    };
    write_outputs(
        o,
        settings,
        mode,
        &input,
        &Outputs::Single(Box::new(single)),
        notices,
        err,
    )
}

/// What a run writes.
enum Outputs {
    /// `-n 0` (`reproduce`): the segmentation, all 0.
    Empty(Vec<i32>),
    Single(Box<Single>),
}

/// Writes the outputs (§3.2, §3.3) in FAST's order, then the notices.
fn write_outputs(
    o: &Options,
    settings: &Settings,
    mode: UpstreamDefects,
    input: &crate::bet::image::Input,
    what: &Outputs,
    mut notices: Vec<Notice>,
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
    let root_name = o.out.clone().unwrap_or_else(|| o.images.last().cloned().unwrap_or_default());
    let root = settings.name(files::strip_image_extension(&root_name));
    let n_threads = settings.threads();
    let ext = settings
        .get("FSL_LOAD_NIFTI_EXTENSIONS")
        .is_some_and(|v| options::atof(v) != 0.0);
    let output = Output {
        template: &input.header,
        flipped: input.flipped,
        shape: input.vol.shape,
        volumes: 1,
        cal: Some((0.0, 0.0)),
        extensions: ext,
        defects: mode,
        n_threads,
        descrip: DESCRIP,
        rewrite_quaternion: true,
    };
    let mut write = |suffix: String, values: Values, notices: &mut Vec<Notice>| -> bool {
        let stem = format!("{root}{suffix}");
        let Some((name, version)) = image::output_file(&stem, &ot) else {
            return false;
        };
        match image::write(&name, version, &output, values) {
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
    let s = match what {
        Outputs::Empty(seg) => {
            if !write("_seg".into(), Values::I32(seg.clone()), &mut notices) {
                return 1;
            }
            print_notices(err, &notices);
            return 0;
        }
        Outputs::Single(s) => s,
    };
    let grid = &s.grid;
    let n = s.n;
    let label = |v: &[u8]| -> Vec<i32> { grid.expand(|i| i32::from(v[i]), 0) };
    if !write("_seg".into(), Values::I32(label(&s.seg)), &mut notices) {
        return 1;
    }
    if o.segments {
        for c in 0..n {
            let target = (c + 1) as u8;
            let v = grid.expand(|i| i32::from(s.seg[i] == target), 0);
            if !write(format!("_seg_{c}"), Values::I32(v), &mut notices) {
                return 1;
            }
        }
    }
    if o.probabilities {
        let outside = grid.len() < grid.voxels();
        match &s.members {
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
                    let v = grid.expand(|i| q[i * n + c], 0.0);
                    if !write(format!("_prob_{c}"), Values::F32(v), &mut notices) {
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
                        "-p's probability maps are 0 everywhere: FAST never computes them here (--nopve without an initialisation iteration)",
                    ),
                );
                for c in 0..n {
                    let v = vec![0f32; grid.voxels()];
                    if !write(format!("_prob_{c}"), Values::F32(v), &mut notices) {
                        return 1;
                    }
                }
            }
        }
    }
    if let Some(p) = &s.pve {
        for c in 0..n {
            let v = grid.expand(|i| p.frac[i * n + c], 0.0);
            if !write(format!("_pve_{c}"), Values::F32(v), &mut notices) {
                return 1;
            }
        }
        if !write("_pveseg".into(), Values::I32(label(&p.seg)), &mut notices) {
            return 1;
        }
        if !write("_mixeltype".into(), Values::I32(label(&p.mixel)), &mut notices) {
            return 1;
        }
    }
    // the correction exp(-b): 1 outside the mask and without a bias field
    let correction = |i: usize| -> f32 {
        match &s.bias {
            Some(b) => larmorx_core::math::expf(-b[i]),
            None => 1.0,
        }
    };
    if o.restore {
        let mut v = input.vol.data.clone();
        for (i, &idx) in grid.index.iter().enumerate() {
            v[idx as usize] *= correction(i);
        }
        if !write("_restore".into(), Values::F32(v), &mut notices) {
            return 1;
        }
    }
    if o.bias_out {
        let v = grid.expand(|i| 1.0 / correction(i), 1.0);
        if !write("_bias".into(), Values::F32(v), &mut notices) {
            return 1;
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
        assert!(String::from_utf8_lossy(&err).ends_with("maps\n\n\n\n--bogus: Option doesn't exist!\n"));
        let mut err = Vec::new();
        let Parsed::Run(o) = parse(&w(&["-gp", "-n", "0x2", "-I", "010", "--out=s", "in.nii"]), &mut err)
        else {
            panic!("parsed")
        };
        assert!(o.segments && o.probabilities);
        assert_eq!((o.n_classes, o.bias_iterations), (2, 8));
        assert_eq!(o.out.as_deref(), Some("s"));
        assert_eq!(o.images, ["in.nii"]);
        let mut err = Vec::new();
        assert!(matches!(parse(&w(&["-n", "2.5", "x"]), &mut err), Parsed::Exit(1)));
        assert!(String::from_utf8_lossy(&err).ends_with("-n: Couldn't set_value! valstr=\"2.5 x\"!\n"));
        assert_eq!(stream_floats("1 2.5\n3e1 x 4"), vec![1.0, 2.5, 30.0]);
        assert_eq!(stream_floats("1 2abc 3"), vec![1.0, 2.0]);
    }
}
