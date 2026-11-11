// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl bet2`: FSL's `bet2` command line (`specs/bet.md` §4.1, §4.7, §4.9, §7).

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_mri::hmc::files::{self, ImageFiles};

use super::Settings;
use super::fmt::{f as fmt_f, g};
use super::options::{self, Given, Spec};
use crate::bet::image::{self, ImageError, Input, Output, Values};
use crate::bet::{self, Deformation, Found, mesh};

/// bet2's usage, exactly (standard error).
pub const USAGE: &str = "\nPart of FSL (ID: 2412.6-dirty)\nBET (Brain Extraction Tool) v2.1 - FMRIB Analysis Group, Oxford\n\nUsage: \nbet2 <input_fileroot> <output_fileroot> [options]\n\nOptional arguments (You may optionally specify one or more of):\n\t-o,--outline\tgenerate brain surface outline overlaid onto original image\n\t-m,--mask\tgenerate binary brain mask\n\t-s,--skull\tgenerate approximate skull image\n\t-n,--nooutput\tdon't generate segmented brain image output\n\t-f <f>\t\tfractional intensity threshold (0->1); default=0.5; smaller values give larger brain outline estimates\n\t-g <g>\t\tvertical gradient in fractional intensity threshold (-1->1); default=0; positive values give larger brain outline at bottom, smaller at top\n\t-r,--radius <r>\thead radius (mm not voxels); initial surface sphere is set to half of this\n\t-w,--smooth <r>\tsmoothness factor; default=1; values smaller than 1 produce more detailed brain surface, values larger than one produce smoother, less detailed surface\n\t-c <x y z>\tcentre-of-gravity (voxels not mm) of initial mesh surface.\n\t-t,--threshold\t-apply thresholding to segmented brain image and mask\n\t--ct\t-treat input as CT image (default is MRI)\n\t--binaryMask <image>\tGenerate mesh from existing mask\n\t-e,--mesh\tgenerates brain surface as mesh in vtk format\n\t-v,--verbose\tswitch on diagnostic messages\n\t-h,--help\tdisplays this help, then exits\n\n\n";

const fn flag(short: char, long: &'static str) -> Spec {
    Spec {
        short: Some(short),
        long: Some(long),
        values: 0,
        numeric: false,
        integer: false,
    }
}

const fn number(short: Option<char>, long: Option<&'static str>, values: usize) -> Spec {
    Spec {
        short,
        long,
        values,
        numeric: true,
        integer: false,
    }
}

/// bet2's options, in [`SPECS`]'s order.
const OUTLINE: usize = 0;
const MASK: usize = 1;
const SKULL: usize = 2;
const NO_OUTPUT: usize = 3;
const FRAC: usize = 4;
const GRAD: usize = 5;
const RADIUS: usize = 6;
const SMOOTH: usize = 7;
const CENTRE: usize = 8;
const THRESHOLD: usize = 9;
const CT: usize = 10;
const BINARY_MASK: usize = 11;
const MESH: usize = 12;
const VERBOSE: usize = 13;
const HELP: usize = 14;
const D1: usize = 15;
const D2: usize = 16;
const TANH: usize = 17;

const SPECS: [Spec; 18] = [
    flag('o', "outline"),
    flag('m', "mask"),
    flag('s', "skull"),
    flag('n', "nooutput"),
    number(Some('f'), None, 1),
    number(Some('g'), None, 1),
    number(Some('r'), Some("radius"), 1),
    number(Some('w'), Some("smooth"), 1),
    number(Some('c'), None, 3),
    flag('t', "threshold"),
    Spec {
        short: None,
        long: Some("ct"),
        values: 0,
        numeric: false,
        integer: false,
    },
    Spec {
        short: None,
        long: Some("binaryMask"),
        values: 1,
        numeric: false,
        integer: false,
    },
    flag('e', "mesh"),
    flag('v', "verbose"),
    flag('h', "help"),
    number(None, Some("parameter_d1"), 1),
    number(None, Some("parameter_d2"), 1),
    // larmorx's own (GLIBC-06, the user's decision of 2026-10-31): --tanh=correctly-rounded
    Spec {
        short: None,
        long: Some("tanh"),
        values: 1,
        numeric: false,
        integer: false,
    },
];

/// FSL's image layer's message when `FSLOUTPUTTYPE` is not set (§3.1), printed when the first
/// output is written (with `reproduce`: FSL-11).
pub use larmorx_mri::hmc::image::FSLOUTPUTTYPE_UNSET;

/// bet2's options as given.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub input: String,
    pub output: String,
    pub outline: bool,
    pub mask: bool,
    pub skull: bool,
    pub brain: bool,
    pub frac: f64,
    pub gradient: f64,
    /// `-r` (0: estimated).
    pub radius: f64,
    pub smoothness: f64,
    /// `-c`, voxels of the file as stored.
    pub centre: Option<[f64; 3]>,
    pub threshold: bool,
    pub ct: bool,
    pub binary_mask: Option<String>,
    pub mesh: bool,
    pub verbose: bool,
    pub d1: f64,
    pub d2: f64,
    /// The smoothness weight's `tanh` (larmorx's `--tanh`, GLIBC-06).
    pub tanh: bet::surface::Tanh,
}

/// What [`parse`] decided: run, or stop with an exit status (its messages written).
pub enum Parsed {
    Run(Box<Options>),
    Exit(u8),
}

/// Parses bet2's command line (§4.1), printing FSL's messages.
pub fn parse(args: &[String], err: &mut dyn Write) -> Parsed {
    if args.is_empty() {
        let _ = err.write_all(USAGE.as_bytes());
        return Parsed::Exit(1);
    }
    if args.len() == 1 {
        if args[0].contains("-h") || args[0].contains("--help") {
            let _ = err.write_all(USAGE.as_bytes());
            return Parsed::Exit(0);
        }
        let _ = writeln!(err, "error: not enough arguments, use bet -h for help");
        return Parsed::Exit(255);
    }
    if args[0].starts_with('-') || args[1].starts_with('-') {
        let _ = writeln!(
            err,
            "error : two first arguments should be input and output names, see bet -h for help"
        );
        return Parsed::Exit(255);
    }
    // larmorx's own `--tanh <value>` is also taken in two words (FSL's long options take
    // `--name=value`)
    let words = join_tanh(&args[2..]);
    let (given, _) = match options::parse(&words, &SPECS, false) {
        Ok(g) => g,
        Err(e) => {
            let _ = write!(err, "{USAGE}\n{}\n", e.0);
            return Parsed::Exit(1);
        }
    };
    let tanh = match &given[TANH] {
        Some(Given::Text(t)) => match bet::surface::Tanh::parse(t) {
            Some(v) => v,
            None => {
                let _ = writeln!(
                    err,
                    "larmorx fsl bet2: --tanh is glibc (the default, FSL's bits) or correctly-rounded, not {t:?} (larmorx)"
                );
                return Parsed::Exit(1);
            }
        },
        _ => bet::surface::Tanh::Glibc,
    };
    let flag = |k: usize| given[k].is_some();
    let num = |k: usize, default: f64| match &given[k] {
        Some(Given::Numbers(v)) => v[0],
        _ => default,
    };
    let o = Options {
        input: args[0].clone(),
        output: args[1].clone(),
        outline: flag(OUTLINE),
        mask: flag(MASK),
        skull: flag(SKULL),
        brain: !flag(NO_OUTPUT),
        frac: num(FRAC, 0.5),
        gradient: num(GRAD, 0.0),
        radius: num(RADIUS, 0.0),
        smoothness: num(SMOOTH, 1.0),
        // a centre of three zeros in FSL's radiological frame is "not given" (`centre_mm`)
        centre: match &given[CENTRE] {
            Some(Given::Numbers(v)) => Some([v[0], v[1], v[2]]),
            _ => None,
        },
        threshold: flag(THRESHOLD),
        ct: flag(CT),
        binary_mask: match &given[BINARY_MASK] {
            Some(Given::Text(t)) => Some(t.clone()),
            _ => None,
        },
        mesh: flag(MESH),
        verbose: flag(VERBOSE),
        d1: num(D1, 7.0),
        d2: num(D2, 3.0),
        tanh,
    };
    if o.frac < 0.0 || o.frac > 1.0 {
        let _ = write!(
            err,
            "\nFractional intensity threshold must be between 0 and 1!\n"
        );
        return Parsed::Exit(1);
    }
    if o.gradient < -1.0 || o.gradient > 1.0 {
        let _ = write!(err, "\nVertical gradient must be between -1 and 1!\n");
        return Parsed::Exit(1);
    }
    if flag(HELP) {
        let _ = err.write_all(USAGE.as_bytes());
        return Parsed::Exit(0);
    }
    Parsed::Run(Box::new(o))
}

/// FSL-187's notice where the correction moved the robust range (`fsl`: FSL's), once.
fn note_187(notices: &mut Vec<Notice>, (t2, t98): (f64, f64), fsl: Option<(f64, f64)>) {
    if let Some((f2, f98)) = fsl {
        defects::push_once(
            notices,
            Notice::new(
                &defects::FSL_187,
                UpstreamDefects::Fix,
                format!(
                    "the robust range counts a value far above a histogram pass's range in its last bin: thresh2 {} thresh98 {} (FSL counts it in its first: {} {})",
                    g(t2),
                    g(t98),
                    g(f2),
                    g(f98)
                ),
            ),
        );
    }
}

/// A refusal of larmorx's own (its line ends with `(larmorx)`), after the notices.
fn refuse(err: &mut dyn Write, notices: &[Notice], message: &str) -> u8 {
    print_notices(err, notices);
    let _ = writeln!(err, "larmorx fsl bet2: {message} (larmorx)");
    1
}

pub(crate) fn print_notices(err: &mut dyn Write, notices: &[Notice]) {
    for n in notices {
        let _ = writeln!(err, "larmorx: {}", n.line());
    }
}

/// FSL-42's checks: the numeric options without meaning (§9).
fn meaningless(o: &Options) -> Option<String> {
    if o.frac.is_nan() {
        return Some("-f is not a number".into());
    }
    if o.gradient.is_nan() {
        return Some("-g is not a number".into());
    }
    if !(o.radius >= 0.0 && o.radius.is_finite()) {
        return Some(format!("-r {} is not a radius", g(o.radius)));
    }
    if !(o.smoothness > 0.0 && o.smoothness.is_finite()) {
        return Some(format!(
            "-w {} is not a smoothness (it must be above 0)",
            g(o.smoothness)
        ));
    }
    if let Some(c) = o.centre
        && c.iter().any(|v| !v.is_finite())
    {
        return Some("-c holds a value that is not finite".into());
    }
    if !o.d1.is_finite() || !o.d2.is_finite() {
        return Some("a search depth (--parameter_d1, --parameter_d2) is not finite".into());
    }
    None
}

/// `-c x y z` (voxels of the file as stored) in the algorithm's millimetres (§3.2: `x` becomes
/// `nx − 1 − x` on a mirrored image), or `None` where FSL takes it as not given: a centre of
/// three zeros **in its radiological frame**, after that conversion (observed, FSL 6.0.7.23,
/// black box: random testing 2026-10-31 case 1608, `-c 0 0 0` on a radiologically stored image;
/// 2026-11-05, the permanent cases `original-bet-R-centre-zero` and
/// `original-bet-R-negative-image-bet2-crash`: `bet -R` passes `fslstats -C`'s `nx − 1 0 0` for
/// an empty brain of a neurologically stored image, and FSL's bet2 uses the centre of gravity).
pub fn centre_mm(c: [f64; 3], flipped: bool, v: &bet::Vol) -> Option<[f64; 3]> {
    let nx = v.shape[0] as f64;
    let x = if flipped { nx - 1.0 - c[0] } else { c[0] };
    if x == 0.0 && c[1] == 0.0 && c[2] == 0.0 {
        return None;
    }
    Some([x * v.voxel[0], c[1] * v.voxel[1], c[2] * v.voxel[2]])
}

/// FSL-40: bet2's names (the first `.hdr` removed, again when `.img` remains; `None`: FSL
/// aborts).
pub fn fsl_name(name: &str) -> Option<String> {
    let remove = |s: &str| match s.find(".hdr") {
        Some(k) => format!("{}{}", &s[..k], &s[k + 4..]),
        None => s.to_owned(),
    };
    let n = remove(name);
    if n.contains(".img") {
        if n.contains(".hdr") {
            Some(remove(&n))
        } else {
            None
        }
    } else {
        Some(n)
    }
}

/// The words with larmorx's `--tanh <value>` joined into `--tanh=<value>`.
pub(crate) fn join_tanh(words: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        if words[i] == "--tanh" && i + 1 < words.len() {
            out.push(format!("--tanh={}", words[i + 1]));
            i += 2;
        } else {
            out.push(words[i].clone());
            i += 1;
        }
    }
    out
}

/// The image layer's messages for an input that cannot be read, and larmorx's line.
fn read_failure(err: &mut dyn Write, name: &str, e: &ImageError) -> u8 {
    read_failure_of("bet2", err, name, e)
}

/// [`read_failure`] for `program`'s line.
pub(crate) fn read_failure_of(
    program: &str,
    err: &mut dyn Write,
    name: &str,
    e: &ImageError,
) -> u8 {
    let text = e.to_string();
    let missing = text.starts_with("No image files match") || text.contains("No such file");
    if missing || text.starts_with("Multiple possible") {
        // FSL's words name the image without its extension, then as given (observed)
        let stem = files::strip_image_extension(name);
        let what = if missing {
            format!("No image files match: {stem}")
        } else {
            format!("Multiple possible filenames detected for basename: {stem}")
        };
        let _ = writeln!(err, "Image Exception : #63 :: {what}");
        let _ = writeln!(err, "Image Exception : #22 :: Failed to read volume {name}");
        let _ = writeln!(err, "Error : {what}");
        let _ = writeln!(
            err,
            "larmorx fsl {program}: {name} cannot be read; FSL aborts here (larmorx)"
        );
    } else {
        let _ = writeln!(err, "larmorx fsl {program}: {name}: {text} (larmorx)");
    }
    1
}

/// Reads the image FSL's layer opens for `name` (completed when it has no extension).
pub(crate) fn read_image(
    settings: &Settings,
    name: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
    n_threads: usize,
) -> Result<Input, ImageError> {
    let f = resolve_image(settings, name, mode, notices)?;
    image::read_first(&f, n_threads)
}

/// The files FSL's layer opens for the image `name` (completed when it has no extension;
/// FSL-60's renaming with `reproduce`).
pub(crate) fn resolve_image(
    settings: &Settings,
    name: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<ImageFiles, ImageError> {
    let full = settings.name(name);
    let resolved = image::resolve_input(&full).map_err(ImageError::Invalid)?;
    let mut f = ImageFiles::of(&resolved)
        .ok_or_else(|| ImageError::Invalid(format!("{resolved}: not an image file name")))?;
    if let Some(other) = files::fsl60_header(&resolved) {
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "{resolved} is read as named; FSL 6.0.7.23 opens {other} (the name's last '.img' made '.hdr')"
            ),
            UpstreamDefects::Reproduce => {
                f.header = other.clone().into();
                format!(
                    "{resolved} is opened as {other}, the name's last '.img' made '.hdr', as FSL 6.0.7.23 does"
                )
            }
        };
        defects::push_once(notices, Notice::new(&defects::FSL_60, mode, detail));
    }
    Ok(f)
}

/// The output names of a run (§4.7), from the output root.
struct Names {
    root: String,
}

impl Names {
    fn stem(&self, suffix: &str) -> String {
        format!("{}{suffix}", self.root)
    }
}

/// Runs bet2.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let o = match parse(args, err) {
        Parsed::Run(o) => o,
        Parsed::Exit(code) => return code,
    };
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl bet2: {e} (larmorx)");
            return 1;
        }
    };
    run(&o, settings, mode, out, err)
}

/// Runs bet2 with parsed options.
pub fn run(
    o: &Options,
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let n_threads = settings.threads();
    let mut notices: Vec<Notice> = Vec::new();
    if let Some(why) = meaningless(o) {
        let n = Notice::new(
            &defects::FSL_42,
            mode,
            match mode {
                UpstreamDefects::Fix => format!("{why}: refused; FSL uses it as given"),
                UpstreamDefects::Reproduce => format!("{why}: used as given, as FSL does"),
            },
        );
        if mode.fix() {
            return refuse(err, &[n], &format!("{why} (FSL-42)"));
        }
        notices.push(n);
    }
    // FSL-40: the names
    let (input, output) = {
        let fsl_in = fsl_name(&o.input);
        let fsl_out = fsl_name(&o.output);
        match mode {
            UpstreamDefects::Reproduce => match (fsl_in, fsl_out) {
                (Some(i), Some(out_name)) => {
                    if i != o.input || out_name != o.output {
                        notices.push(Notice::new(
                            &defects::FSL_40,
                            mode,
                            format!(
                                "the names lose their first '.hdr' as bet2 does: {} and {}",
                                i, out_name
                            ),
                        ));
                    }
                    (i, out_name)
                }
                _ => {
                    let n = Notice::new(
                        &defects::FSL_40,
                        mode,
                        "a name holds '.img' and no '.hdr': bet2 aborts (std::out_of_range)",
                    );
                    return refuse(
                        err,
                        &[n],
                        "a name holds '.img' and no '.hdr', on which FSL aborts (FSL-40)",
                    );
                }
            },
            UpstreamDefects::Fix => {
                let i = o.input.clone();
                let out_name = files::strip_image_extension(&o.output).to_owned();
                let theirs = fsl_out.as_deref().map(files::strip_image_extension);
                if fsl_in.as_deref() != Some(i.as_str())
                    && fsl_in.as_deref().map(files::strip_image_extension)
                        != Some(files::strip_image_extension(&i))
                    || theirs != Some(out_name.as_str())
                {
                    notices.push(Notice::new(
                        &defects::FSL_40,
                        mode,
                        format!(
                            "the names are used as given (only a trailing image extension removed); bet2 removes their first '.hdr' wherever it stands{}",
                            if fsl_in.is_none() || fsl_out.is_none() {
                                " and aborts on a name with '.img' and no '.hdr'"
                            } else {
                                ""
                            }
                        ),
                    ));
                }
                (i, out_name)
            }
        }
    };
    let input_root = files::strip_image_extension(&input).to_owned();
    let root = if output == "default__default" {
        format!("{input_root}_brain")
    } else {
        files::strip_image_extension(&output).to_owned()
    };
    let names = Names {
        root: settings.name(&root),
    };
    let defo = Deformation {
        frac: o.frac,
        gradient: o.gradient,
        smoothness: o.smoothness,
        d1: o.d1,
        d2: o.d2,
        exact: o.mesh,
        // GLIBC-06 (the user's decision, 2026-10-31): glibc's bits (FSL's) by default in both
        // modes, the correctly rounded tanh with `--tanh=correctly-rounded` (asked for, so no
        // notice)
        tanh: o.tanh,
    };
    // the image the surface is found on: the mask (--binaryMask) or the input
    let surface_source = o.binary_mask.as_deref();
    let source = match read_image(
        settings,
        surface_source.unwrap_or(&input),
        mode,
        &mut notices,
        n_threads,
    ) {
        Ok(r) => r,
        Err(e) => {
            print_notices(err, &notices);
            return read_failure(err, surface_source.unwrap_or(&input), &e);
        }
    };
    if source.volumes > 1 {
        let _ = writeln!(
            err,
            "Warning: An input intended to be a single 3D volume has multiple timepoints. Input will be truncated to first volume, but this functionality is deprecated and will be removed in a future release."
        );
    }
    // the image the surface is found on: the source, converted for --ct (a copy only then)
    let cormack = |v: &bet::Vol| v.with_data(v.data.iter().map(|&x| bet::cormack(x)).collect());
    let work_owned = o.ct.then(|| cormack(&source.vol));
    let centre_mm = o
        .centre
        .and_then(|c| centre_mm(c, source.flipped, &source.vol));
    // -r 0 (the default) asks for the estimate; any other value is used (FSL-42 reproduced)
    let radius = (o.radius != 0.0).then_some(o.radius);
    let found = bet::find_surface(
        work_owned.as_ref().unwrap_or(&source.vol),
        radius,
        centre_mm,
        &defo,
        mode,
        n_threads,
    );
    note_187(
        &mut notices,
        (found.init.t2, found.init.t98),
        found.init.fsl_187,
    );
    if found.init.one_d {
        let _ = writeln!(err, "Warning: Image appears to be 1D");
    }
    if o.verbose {
        verbose(out, &found);
    }
    // the image the outputs are made from, and the one -t and -s measure (converted for --ct)
    let (input_img, work_out): (Input, Option<bet::Vol>) = if surface_source.is_some() {
        let inp = match read_image(settings, &input, mode, &mut notices, n_threads) {
            Ok(r) => r,
            Err(e) => {
                print_notices(err, &notices);
                return read_failure(err, &input, &e);
            }
        };
        if inp.volumes > 1 {
            let _ = writeln!(
                err,
                "Warning: An input intended to be a single 3D volume has multiple timepoints. Input will be truncated to first volume, but this functionality is deprecated and will be removed in a future release."
            );
        }
        if inp.vol.shape != source.vol.shape {
            let n = Notice::new(
                &defects::FSL_61,
                mode,
                "the mask and the input have other dimensions: FSL applies the brain mask over the input's dimensions, reading and writing outside it; refused",
            );
            return refuse(
                err,
                &[n],
                "--binaryMask names an image on another grid than the input (FSL-61)",
            );
        }
        if !o.ct {
            (inp, None)
        } else if mode.reproduce() {
            // FSL-62: FSL writes the mask, masked, as the brain image (observed)
            notices.push(Notice::new(
                &defects::FSL_62,
                mode,
                "--ct with --binaryMask: the brain image is the mask, masked, as FSL writes it",
            ));
            (
                Input {
                    vol: source.vol,
                    ..inp
                },
                work_owned,
            )
        } else {
            notices.push(Notice::new(
                &defects::FSL_62,
                mode,
                "--ct with --binaryMask: the brain image is the CT input, masked; FSL writes the mask instead",
            ));
            let w = cormack(&inp.vol);
            (inp, Some(w))
        }
    } else {
        (source, work_owned)
    };
    let input_vol_work: &bet::Vol = work_out.as_ref().unwrap_or(&input_img.vol);
    let (t2, t98, t) = if surface_source.is_some() {
        let r = bet::pooled(n_threads, || {
            bet::params::robust_range_in(input_vol_work, mode)
        });
        note_187(&mut notices, (r.t2, r.t98), r.fsl);
        (r.t2, r.t98, r.t2 + 0.1 * (r.t98 - r.t2))
    } else {
        (found.init.t2, found.init.t98, found.init.t)
    };
    let shape = input_img.vol.shape;
    let voxel = input_img.vol.voxel;
    // the outputs
    let needs_images = o.brain || o.mask || o.outline || o.skull;
    let mask: Option<Vec<u8>>;
    // the mask is made whatever the outputs (observed: FSL crashes in its fill at a centroid
    // that is not a number also with -n and no other output, FSL-43)
    {
        let made = bet::pooled(n_threads, || {
            let mut n = Vec::new();
            let m = bet::brain_mask(&found.mesh, &found.points, shape, voxel, mode, &mut n);
            (m, n)
        });
        notices.extend(made.1);
        match made.0 {
            Ok(mut m) => {
                if o.threshold {
                    for (mv, &v) in m.iter_mut().zip(&input_vol_work.data) {
                        if f64::from(v) < t {
                            *mv = 0;
                        }
                    }
                }
                mask = Some(m);
            }
            Err(_) => {
                let n = Notice::new(
                    &defects::FSL_43,
                    mode,
                    "the surface's centroid is not a number (the surface diverged): refused; FSL writes far outside the volume there and crashes",
                );
                return refuse(
                    err,
                    &[n],
                    "the surface diverged (its centroid is not a number), on which FSL crashes (FSL-43)",
                );
            }
        }
    }
    let ext = settings
        .get("FSL_LOAD_NIFTI_EXTENSIONS")
        .is_some_and(|v| options::atof(v) != 0.0);
    let output = |cal: Option<(f64, f64)>| Output {
        template: &input_img.header,
        flipped: input_img.flipped,
        shape,
        volumes: 1,
        cal,
        extensions: ext,
        defects: mode,
        n_threads,
        descrip: image::DESCRIP,
        rewrite_quaternion: true,
    };
    if needs_images {
        let ot = match settings.output_type() {
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
        let mut write = |stem: String, cal: Option<(f64, f64)>, values: Values| -> bool {
            let Some((name, version)) = image::output_file(&stem, &ot) else {
                return false;
            };
            match image::write(&name, version, &output(cal), values) {
                Ok(n) => {
                    for x in n {
                        defects::push_once(&mut notices, x);
                    }
                    true
                }
                Err(e) => {
                    print_notices(err, &notices);
                    let _ = writeln!(
                        err,
                        "larmorx fsl bet2: {name} cannot be written ({e}); FSL aborts here (larmorx)"
                    );
                    false
                }
            }
        };
        if o.brain {
            let m = mask.as_deref().unwrap_or(&[]);
            if !write(
                names.stem(""),
                None,
                Values::F32(bet::pooled(n_threads, || {
                    bet::masked(&input_img.vol.data, m)
                })),
            ) {
                return 1;
            }
        }
        if o.mask {
            let m = mask.as_deref().unwrap_or(&[]);
            if !write(
                names.stem("_mask"),
                Some((0.0, 1.0)),
                Values::I16(m.iter().map(|&x| i16::from(x)).collect()),
            ) {
                return 1;
            }
        }
        if o.outline {
            let v = bet::pooled(n_threads, || {
                // on the image BET ran on: --ct's Cormack units (observed)
                bet::outline(&found.mesh, &found.points, input_vol_work)
            });
            if !write(names.stem("_overlay"), None, Values::F32(v)) {
                return 1;
            }
        }
        if o.skull {
            let s = bet::pooled(n_threads, || {
                bet::skull::skull(&found.mesh, &found.points, input_vol_work, t, t2, t98)
            });
            let v = s.iter().map(|&b| if b { 100i16 } else { 0 }).collect();
            if !write(names.stem("_skull"), None, Values::I16(v)) {
                return 1;
            }
        }
    }
    if o.mesh {
        let name = names.stem("_mesh.vtk");
        if let Err(e) = write_vtk(&name, &found.mesh, &found.points) {
            let _ = e;
            let _ = writeln!(err, "error opening file {name}");
        }
    }
    print_notices(err, &notices);
    0
}

fn verbose(out: &mut dyn Write, found: &Found) {
    let i = &found.init;
    let _ = writeln!(
        out,
        "min {} thresh2 {} thresh {} thresh98 {} max {}",
        g(f64::from(i.min)),
        g(i.t2),
        g(i.t),
        g(i.t98),
        g(f64::from(i.max))
    );
    let _ = writeln!(
        out,
        "c-of-g {} {} {} mm",
        g(i.centre[0]),
        g(i.centre[1]),
        g(i.centre[2])
    );
    let _ = writeln!(out, "radius {} mm", g(i.radius));
    let _ = writeln!(out, "median within-brain intensity {}", g(i.median));
    let last = found.passes.len().saturating_sub(1);
    for (k, p) in found.passes.iter().enumerate() {
        match p.total {
            Some(t) => {
                let _ = writeln!(out, "self-intersection total {} (threshold=4000.0) ", g(t));
            }
            None => {
                if p.intersecting {
                    if k == 0 {
                        let _ = writeln!(out, "the mesh is self-intersecting ");
                    } else {
                        let _ = writeln!(out, "the mesh is self-intersecting");
                    }
                }
            }
        }
        if k < last {
            let _ = writeln!(out, "thus will rerun with higher smoothness constraint");
        }
    }
}

/// The mesh as VTK ASCII polydata (§4.7).
pub fn write_vtk(name: &str, m: &mesh::Mesh, points: &[mesh::V3]) -> std::io::Result<()> {
    std::fs::write(name, vtk_text(m, points))
}

/// The text of [`write_vtk`]'s file.
fn vtk_text(m: &mesh::Mesh, points: &[mesh::V3]) -> String {
    let mut s = String::with_capacity(points.len() * 40 + m.triangles.len() * 24 + 128);
    s.push_str("# vtk DataFile Version 3.0\nsurface file\nASCII\nDATASET POLYDATA\n");
    s.push_str(&format!("POINTS {}  float\n", points.len()));
    for p in points {
        s.push_str(&format!(
            "{} {} {}\n",
            fmt_f(p[0]),
            fmt_f(p[1]),
            fmt_f(p[2])
        ));
    }
    s.push_str(&format!(
        "POLYGONS {} {}\n",
        m.triangles.len(),
        4 * m.triangles.len()
    ));
    for t in &m.triangles {
        s.push_str(&format!("3 {} {} {} \n", t[0], t[1], t[2]));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn centre(words: &[&str]) -> Option<[f64; 3]> {
        let args: Vec<String> = ["in", "out"]
            .iter()
            .chain(words)
            .map(|s| s.to_string())
            .collect();
        match parse(&args, &mut Vec::new()) {
            Parsed::Run(o) => o.centre,
            Parsed::Exit(s) => panic!("exit {s}"),
        }
    }

    #[test]
    fn a_centre_of_zeros_is_not_given() {
        // observed: FSL's bet2 takes a centre of zeros in its radiological frame as no centre
        // (the centre of gravity): `-c 0 0 0` on a radiologically stored image, `-c nx-1 0 0`
        // on a neurologically stored one
        assert_eq!(centre(&[]), None);
        assert_eq!(centre(&["-c", "0", "0", "1"]), Some([0.0, 0.0, 1.0]));
        let v = bet::Vol::new([8, 12, 15], [2.0, 1.0, 0.5], vec![0.0; 8 * 12 * 15]);
        let zeros = centre(&["-c", "0", "0", "0"]).unwrap();
        assert_eq!(centre_mm(zeros, false, &v), None);
        let minus = centre(&["-c", "-0.000000", "0", "0"]).unwrap();
        assert_eq!(centre_mm(minus, false, &v), None);
        assert_eq!(centre_mm(zeros, true, &v), Some([14.0, 0.0, 0.0]));
        assert_eq!(centre_mm([7.0, 0.0, 0.0], true, &v), None);
        assert_eq!(
            centre_mm([7.0, 0.0, 0.0], false, &v),
            Some([14.0, 0.0, 0.0])
        );
        assert_eq!(centre_mm([0.0, 0.0, 1.0], false, &v), Some([0.0, 0.0, 0.5]));
    }

    /// FSL-187 through bet2's notice: an image of values on [0, 1) with one at 10^12; the
    /// default prints the corrected thresholds and names FSL's, `reproduce` prints FSL's.
    #[test]
    fn fsl_187_notice() {
        let d = tempfile::tempdir().unwrap();
        let shape = [5usize, 4, 5];
        let mut data: Vec<f32> = (0..99).map(|i| i as f32 / 99.0).collect();
        data.push(1e12);
        let h = larmorx_io::nifti::NiftiHeader::new(
            larmorx_io::nifti::NiftiVersion::V1,
            &shape,
            larmorx_core::element::DataType::F32,
        )
        .unwrap();
        let o = Output {
            template: &h,
            flipped: false,
            shape,
            volumes: 1,
            cal: None,
            extensions: false,
            defects: UpstreamDefects::Fix,
            n_threads: 1,
            descrip: "test",
            rewrite_quaternion: false,
        };
        let path = d.path().join("outl.nii");
        image::write(
            path.to_str().unwrap(),
            larmorx_io::nifti::NiftiVersion::V1,
            &o,
            Values::F32(data),
        )
        .unwrap();
        let call = |mode: &str| {
            let settings = Settings {
                dir: Some(d.path().to_path_buf()),
                vars: [
                    ("FSLOUTPUTTYPE", "NIFTI_GZ"),
                    (defects::ENV, mode),
                    ("OMP_NUM_THREADS", "2"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
                ..Settings::default()
            };
            let args: Vec<String> = ["outl.nii", "b_out", "-v", "-n"]
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = main(&args, &settings, &mut out, &mut err);
            assert_eq!(code, 0);
            let out = String::from_utf8(out).unwrap();
            let thresholds = out.lines().next().unwrap().to_owned();
            (thresholds, String::from_utf8(err).unwrap())
        };
        let (fix, err) = call("fix");
        let line = err
            .lines()
            .find(|l| l.contains("upstream defect FSL-187"))
            .unwrap_or_else(|| panic!("FSL-187 named: {fix} / {err}"));
        let (rep, rep_err) = call("reproduce");
        assert!(!rep_err.contains("FSL-187"), "{rep_err}");
        assert_ne!(fix, rep);
        // the notice names FSL's thresholds: those reproduce prints
        let words: Vec<&str> = rep.split_whitespace().collect();
        let fsl = format!("its first: {} {})", words[3], words[7]);
        assert!(line.contains(&fsl), "{line} / {rep}");
    }
}
