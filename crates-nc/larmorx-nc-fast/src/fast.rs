// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-fast4 2111.3 (fast_two.cc). FAST4 - FMRIB's
// Automated Segmentation Tool v4, John Vickers, Mark Jenkinson, Matthew Webster and Steve Smith,
// FMRIB Image Analysis Group; Copyright (C) 2005-2012 University of Oxford. Rust translation
// Copyright 2026 Karel Lopez Vilaret; modified from the originals. Distributed under the FSL
// Licence (see LICENSE).
//! `fast`'s `main`: its options (fsl-utils' parser, FSL's usage text and messages), the input
//! and its negative values, the single-channel segmentation ([`crate::mriseg`]) and the files
//! it writes.

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::exception;
use larmorx_nc_core::options::{ArgFlag, Opt, OptionParser, Parsed, Value};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{self, Elem, FslError, SaveError, SaveOptions, Saved, Volume};

use crate::mriseg::{Io, Params, Segmentation};
use crate::multi_mriseg::{MAX_CHANNELS, MultiParams, MultiSegmentation};

const TITLE: &str = "FAST \nCopyright(c) 2004-2012, University of Oxford";
const EXAMPLES: &str = "fast [options] file(s)";

/// How the replica runs: the settings read from the environment, D13's mode, the threads.
pub struct Run<'a> {
    pub fsl: &'a Settings,
    pub mode: UpstreamDefects,
    /// Threads for the bias field's convolutions (the results do not depend on them).
    pub threads: usize,
}

fn opt(key: &'static str, default: Value, help: &'static str, flag: ArgFlag) -> Opt {
    Opt {
        key,
        default,
        help,
        compulsory: false,
        flag,
        visible: true,
    }
}

/// The options of `fast`, in the order its `main` adds them (the order of the usage text).
/// `--pvestep` is declared in FAST's source but never added: it is no option.
pub fn parser() -> OptionParser {
    use ArgFlag::{NoArgument as N, Requires as R};
    use Value::{Bool as B, Float as F, Int as I, Str as S};
    let s = String::new;
    OptionParser::new(
        TITLE,
        EXAMPLES,
        vec![
            opt(
                "-n,--class",
                I(3),
                "number of tissue-type classes; default=3",
                R(1),
            ),
            opt(
                "-I,--iter",
                I(4),
                "number of main-loop iterations during bias-field removal; default=4",
                R(1),
            ),
            opt(
                "-l,--lowpass",
                F(20.0),
                "bias field smoothing extent (FWHM) in mm; default=20",
                R(1),
            ),
            opt(
                "-t,--type",
                I(1),
                "type of image 1=T1, 2=T2, 3=PD; default=T1",
                R(1),
            ),
            opt(
                "-f,--fHard",
                F(0.02),
                "initial segmentation spatial smoothness (during bias field estimation); default=0.02",
                R(1),
            ),
            opt(
                "-g,--segments",
                B(false),
                "outputs a separate binary image for each tissue type",
                N,
            ),
            opt(
                "-a",
                S(s()),
                "~<standard2input.mat> initialise using priors; you must supply a FLIRT transform",
                R(1),
            ),
            opt(
                "-A",
                S(s()),
                "~<prior1> <prior2> <prior3>    alternative prior images",
                R(3),
            ),
            opt(
                "--nopve",
                B(false),
                "turn off PVE (partial volume estimation)",
                N,
            ),
            opt("-b", B(false), "\toutput estimated bias field", N),
            opt("-B", B(false), "\toutput bias-corrected image", N),
            opt("-N,--nobias", B(false), "do not remove bias field", N),
            opt(
                "-S,--channels",
                I(1),
                "number of input images (channels); default 1",
                R(1),
            ),
            opt("-o,--out", S(s()), "output basename", R(1)),
            opt(
                "-P,--Prior",
                B(false),
                "use priors throughout; you must also set the -a option",
                N,
            ),
            opt(
                "-W,--init",
                I(15),
                "number of segmentation-initialisation iterations; default=15",
                R(1),
            ),
            opt(
                "-R,--mixel",
                F(0.3),
                "spatial smoothness for mixeltype; default=0.3",
                R(1),
            ),
            opt(
                "-O,--fixed",
                I(4),
                "number of main-loop iterations after bias-field removal; default=4",
                R(1),
            ),
            opt(
                "-H,--Hyper",
                F(0.1),
                "segmentation spatial smoothness; default=0.1",
                R(1),
            ),
            opt("-v,--verbose", B(false), "switch on diagnostic messages", N),
            opt("-h,--help", B(false), "display this message", N),
            opt(
                "-s,--manualseg",
                S(s()),
                "~<filename> Filename containing intensities",
                R(1),
            ),
            opt("-p", B(false), "\toutputs individual probability maps", N),
        ],
    )
}

/// The parsed options.
struct Args {
    nclass: i32,
    nbiter: i32,
    nblowpass: f32,
    typeofimage: i32,
    fbeta: f32,
    segments: bool,
    bapriori: String,
    alternate_priors: Option<[String; 3]>,
    pve: i32,
    output_bias: bool,
    output_corrected: bool,
    remove_bias: bool,
    nchannel: i32,
    outname: Option<String>,
    prior_throughout: bool,
    inititer: i32,
    mixel: f32,
    initfixity: i32,
    hyper: f32,
    verbose: bool,
    manualseg: String,
    probabilities: bool,
}

fn args_of(parser: &OptionParser, p: &Parsed) -> Args {
    let get = |k: &str| p.get(parser, k);
    let a = get("-A");
    let str_at = |n: usize| match a.values.get(n) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    };
    let o = get("-o,--out");
    Args {
        nclass: get("-n,--class").int(),
        nbiter: get("-I,--iter").int(),
        nblowpass: get("-l,--lowpass").float(),
        typeofimage: get("-t,--type").int(),
        fbeta: get("-f,--fHard").float(),
        segments: get("-g,--segments").bool(),
        bapriori: get("-a").str().to_owned(),
        alternate_priors: a.set.then(|| [str_at(0), str_at(1), str_at(2)]),
        pve: if get("--nopve").bool() { 0 } else { 100 },
        output_bias: get("-b").bool(),
        output_corrected: get("-B").bool(),
        remove_bias: get("-N,--nobias").bool(),
        nchannel: get("-S,--channels").int(),
        outname: o.set.then(|| o.str().to_owned()),
        prior_throughout: get("-P,--Prior").bool(),
        inititer: get("-W,--init").int(),
        mixel: get("-R,--mixel").float(),
        initfixity: get("-O,--fixed").int(),
        hyper: get("-H,--Hyper").float(),
        verbose: get("-v,--verbose").bool(),
        manualseg: get("-s,--manualseg").str().to_owned(),
        probabilities: get("-p").bool(),
    }
}

/// `fast`'s `main`: `args[0]` is the program name. Returns the exit status.
pub fn main(args: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let parser = parser();
    let parsed = match parser.parse_command_line(args, 0, true) {
        Ok(p) => p,
        Err((_, what)) => {
            let _ = err.write_all(parser.usage().as_bytes());
            let _ = writeln!(err, "\n{what}");
            return 1;
        }
    };
    let help = parsed.get(&parser, "-h,--help").bool();
    if args.len() < 2 || help || parser.check_compulsory(&parsed).is_some() {
        let _ = err.write_all(parser.usage().as_bytes());
        return 1;
    }
    let a = args_of(&parser, &parsed);
    if a.hyper < 0.0 {
        let _ = writeln!(
            err,
            "ERROR: Segmentation smoothness must be positive. Exiting"
        );
        return 1;
    }
    let mut io = Io { out, err };
    if a.nchannel == 1 {
        return segment_single_channel(args, &a, run, &mut io);
    }
    segment_multi_channel(args, &a, run, &mut io)
}

/// `make_basename` of the output name (the input's name without `-o`).
fn output_name(a: &Args, input: &str) -> String {
    volume::make_basename(a.outname.as_deref().unwrap_or(input))
}

/// What `prior_registration` returns: the use of priors (0, 1, 2) and the priors.
struct Priors {
    bapused: i32,
    volumes: Option<[Volume; 3]>,
}

/// `prior_registration(inname, main_prior_vol, pCSF, pGM, pWM)`.
fn prior_registration(a: &Args, run: &Run, io: &mut Io) -> Result<Priors, FslError> {
    let _names = match &a.alternate_priors {
        Some(n) => n.clone(),
        None => {
            // string(getenv("FSLDIR")): a null pointer when FSLDIR is unset
            let Some(dir) = run.fsl.get("FSLDIR") else {
                return Err(FslError {
                    printed: Vec::new(),
                    type_name: "std::logic_error",
                    what: "basic_string: construction from null is not valid".to_owned(),
                });
            };
            let root = format!("{dir}/data/standard/tissuepriors/avg152T1_");
            [
                format!("{root}csf"),
                format!("{root}gray"),
                format!("{root}white"),
            ]
        }
    };
    let mut bapused = i32::from(!a.bapriori.is_empty());
    if a.nclass != 2 && a.nclass != 3 && bapused != 0 {
        bapused = 0;
        let _ = writeln!(
            io.err,
            "Apriori can only be used for 2 or 3-class segmentation"
        );
    }
    if bapused > 0 {
        // The priors are registered with FLIRT ($FSLDIR/bin/flirt -applyxfm), used to
        // initialise (bapused 1) or throughout with -P (bapused 2); the replica of that path is
        // not written yet.
        let _ = writeln!(
            io.err,
            "larmorx-nc: fast -a{} (tissue priors registered with FLIRT) is not replicated yet",
            if a.prior_throughout { " -P" } else { "" }
        );
        return Err(FslError {
            printed: Vec::new(),
            type_name: "",
            what: String::new(),
        });
    }
    Ok(Priors {
        bapused,
        volumes: None,
    })
}

/// Saves one output: newimage's `descrip`; `Some(status)` when FAST stops.
fn save<T: Elem>(v: &Volume<T>, name: &str, run: &Run, io: &mut Io) -> Option<u8> {
    let opts =
        SaveOptions::new(volume::BUILDSTRING.as_bytes()).reproducing_offset(run.mode.reproduce());
    match v.save_volume(name, run.fsl, &opts) {
        Ok(Saved::Written) => None,
        // save_volume's -1 is ignored by FAST
        Ok(Saved::Nothing) => None,
        Err(SaveError::OutputType(o)) => {
            for l in &o.printed {
                let _ = writeln!(io.err, "{l}");
            }
            Some(1)
        }
        Err(SaveError::Exception(e)) => exception::throw(e),
    }
}

/// A 3D volume of the image's grid and properties holding `data`.
fn grid<T: Elem>(props: &Volume, data: Vec<T>) -> Volume<T> {
    let mut v: Volume<T> = Volume::new(props.xsize(), props.ysize(), props.zsize(), 1);
    v.copy_properties(props);
    v.data = data;
    v
}

/// `segmentSingleChannel(argc, argv)`.
fn segment_single_channel(args: &[String], a: &Args, run: &Run, io: &mut Io) -> u8 {
    if a.verbose {
        let _ = writeln!(io.out, "Starting Single Image Segmentation");
    }
    let input_name = args[args.len() - 1].clone();
    let outname = output_name(a, &input_name);
    let mut input: Volume = match volume::read_volume::<f32>(&input_name, run.fsl) {
        Ok((v, warning)) => {
            if let Some(w) = warning {
                let _ = writeln!(io.err, "{w}");
            }
            v
        }
        Err(e) => exception::throw(e),
    };
    clear_negative(&mut input);
    if a.verbose {
        let kind = match a.typeofimage {
            2 => "T2-weighted image",
            3 => "PD-weighted image",
            _ => "T1-weighted image",
        };
        let _ = writeln!(io.out, "{kind}");
        let s = |v: f32| larmorx_nc_core::cnum::stream(f64::from(v));
        let _ = write!(
            io.out,
            "Imagesize : {} x {} x {}\nPixelsize : {} x {} x {}\n\n",
            input.xsize(),
            input.ysize(),
            input.zsize(),
            s(input.xdim()),
            s(input.ydim()),
            s(input.zdim())
        );
    }
    let priors = match prior_registration(a, run, io) {
        Ok(p) => p,
        Err(e) if e.type_name.is_empty() => return 1,
        Err(e) => exception::throw(e),
    };
    input.display_max = 0.0;
    input.display_min = 0.0;
    let params = Params {
        fbeta: a.fbeta,
        nclasses: a.nclass,
        nblowpass: a.nblowpass,
        bias: !a.remove_bias,
        iterationspve: a.pve,
        mixeltype_mrf: a.mixel,
        nbiter: a.nbiter,
        initfixed: a.initfixity,
        inititerations: a.inititer,
        bapused: priors.bapused,
        hyper: a.hyper,
        verbose: a.verbose,
        mansegfile: a.manualseg.clone(),
        imagetype: a.typeofimage,
        threads: run.threads,
    };
    let mut mri = Segmentation::create(&input, params);
    let pr = priors.volumes.as_ref().map(|[c, g, w]| [c, g, w]);
    match mri.main(pr, io) {
        Ok(0) => {}
        // TanakaMain's 1: main returns -1
        Ok(_) => return 255,
        Err(e) => exception::throw(e),
    }
    let props = mri.properties().clone();
    let nc = a.nclass.max(0) as usize;
    let seg: Volume<i32> = grid(&props, mri.segment.clone());
    if let Some(code) = save(&seg, &format!("{outname}_seg"), run, io) {
        return code;
    }
    if a.segments {
        for i in 1..=nc as i32 {
            let ind: Vec<i32> = mri.segment.iter().map(|&s| i32::from(s == i)).collect();
            let v: Volume<i32> = grid(&props, ind);
            if let Some(code) = save(&v, &format!("{outname}_seg_{}", i - 1), run, io) {
                return code;
            }
        }
    }
    if a.probabilities {
        for i in 0..nc {
            let v: Volume = grid(&props, mri.member_volume(i));
            if let Some(code) = save(&v, &format!("{outname}_prob_{i}"), run, io) {
                return code;
            }
        }
    }
    if a.pve != 0 {
        for i in 1..=nc {
            let v: Volume = grid(&props, mri.pve_volume(i));
            if let Some(code) = save(&v, &format!("{outname}_pve_{}", i - 1), run, io) {
                return code;
            }
        }
        let v: Volume<i32> = grid(&props, mri.pve_segment.clone());
        if let Some(code) = save(&v, &format!("{outname}_pveseg"), run, io) {
            return code;
        }
        let v: Volume<i32> = grid(&props, mri.hard_pv.clone());
        if let Some(code) = save(&v, &format!("{outname}_mixeltype"), run, io) {
            return code;
        }
    }
    if a.output_corrected {
        for (v, &b) in input.data.iter_mut().zip(&mri.bias_field) {
            *v *= b;
        }
        if let Some(code) = save(&input, &format!("{outname}_restore"), run, io) {
            return code;
        }
    }
    if a.output_bias {
        let field: Vec<f32> = mri.bias_field.iter().map(|&b| 1.0 / b).collect();
        let v: Volume = grid(&props, field);
        if let Some(code) = save(&v, &format!("{outname}_bias"), run, io) {
            return code;
        }
    }
    0
}

/// The negative values of an input, as `segment*Channel` treats them: when the 2nd percentile
/// is negative the minimum is subtracted, otherwise the negative values are set to 0.
fn clear_negative(input: &mut Volume) {
    if input.min() < 0.0 {
        let (p02, _) = input
            .percentile_masked(0.02, None)
            .unwrap_or_else(|e| exception::throw(e));
        if p02 < 0.0 {
            let m = input.min();
            for v in &mut input.data {
                *v -= m;
            }
        } else {
            let mx = input.max();
            input.threshold_inclusive(0.0, mx);
        }
    }
}

/// `segmentMultiChannel(argc, argv)`: `-S n` reads the last `n` words of the command line,
/// channel 1 the last one (which also names the outputs and orders the classes).
fn segment_multi_channel(args: &[String], a: &Args, run: &Run, io: &mut Io) -> u8 {
    if a.verbose {
        let _ = writeln!(io.out, "Starting Multi Image Segmentation");
    }
    if a.nchannel < 2 {
        let _ = write!(
            io.err,
            "At least 2 channels required for Multi Channel Segmentation"
        );
        return 1;
    }
    let nch = a.nchannel as usize;
    // argv[argc - c - 1] for c < nchannel: argv[0] (the program) or before it when fewer
    // words follow the program than channels
    if nch >= args.len() {
        let _ = writeln!(
            io.err,
            "larmorx-nc: fast -S {nch}: fewer images than channels on the command line (FAST \
             reads the program's own path or memory before the arguments)"
        );
        return 1;
    }
    if nch > MAX_CHANNELS {
        let _ = writeln!(
            io.err,
            "larmorx-nc: fast -S {nch} (more than {MAX_CHANNELS} channels) is not replicated yet"
        );
        return 1;
    }
    let input_name = args[args.len() - 1].clone();
    let outname = output_name(a, &input_name);
    let mut images: Vec<Volume> = Vec::with_capacity(nch);
    for c in 0..nch {
        let name = &args[args.len() - c - 1];
        let mut image: Volume = match volume::read_volume::<f32>(name, run.fsl) {
            Ok((v, warning)) => {
                if let Some(w) = warning {
                    let _ = writeln!(io.err, "{w}");
                }
                v
            }
            Err(e) => exception::throw(e),
        };
        clear_negative(&mut image);
        image.display_max = 0.0;
        image.display_min = 0.0;
        images.push(image);
    }
    let priors = match prior_registration(a, run, io) {
        Ok(p) => p,
        Err(e) if e.type_name.is_empty() => return 1,
        Err(e) => exception::throw(e),
    };
    let (nx, ny, nz) = (images[0].xsize(), images[0].ysize(), images[0].zsize());
    if images
        .iter()
        .any(|v| (v.xsize(), v.ysize(), v.zsize()) != (nx, ny, nz))
    {
        let _ = writeln!(
            io.err,
            "larmorx-nc: fast -S {nch}: the channels' images differ in size (FAST reads and \
             writes past the smaller ones)"
        );
        return 1;
    }
    let params = MultiParams {
        nclasses: a.nclass,
        nbiter: a.nbiter,
        nblowpass: a.nblowpass,
        fbeta: a.fbeta,
        bapused: priors.bapused,
        bias: !a.remove_bias,
        initfixed: a.initfixity,
        verbose: a.verbose,
        iterationspve: a.pve,
        inititerations: a.inititer,
        mixeltype_mrf: a.mixel,
        hyper: a.hyper,
        mansegfile: a.manualseg.clone(),
        imagetype: a.typeofimage,
        threads: run.threads,
    };
    let mut mri = MultiSegmentation::create(&images, params);
    let pr = priors.volumes.as_ref().map(|[c, g, w]| [c, g, w]);
    match mri.main(pr, io) {
        Ok(0) => {}
        // TanakaMain's -1: main returns -1
        Ok(_) => return 255,
        Err(e) => exception::throw(e),
    }
    let mut props = images[0].clone();
    props.data = Vec::new();
    let nc = a.nclass.max(0) as usize;
    let seg: Volume<i32> = grid(&props, mri.segment.clone());
    if let Some(code) = save(&seg, &format!("{outname}_seg"), run, io) {
        return code;
    }
    if a.segments {
        for i in 1..=nc as i32 {
            let ind: Vec<i32> = mri.segment.iter().map(|&s| i32::from(s == i)).collect();
            let v: Volume<i32> = grid(&props, ind);
            if let Some(code) = save(&v, &format!("{outname}_seg_{}", i - 1), run, io) {
                return code;
            }
        }
    }
    if a.probabilities {
        for i in 0..nc {
            let v: Volume = grid(&props, mri.member_volume(i));
            if let Some(code) = save(&v, &format!("{outname}_prob_{i}"), run, io) {
                return code;
            }
        }
    }
    if a.pve != 0 {
        for i in 1..=nc {
            let v: Volume = grid(&props, mri.pve_volume(i));
            if let Some(code) = save(&v, &format!("{outname}_pve_{}", i - 1), run, io) {
                return code;
            }
        }
        let v: Volume<i32> = grid(&props, mri.pve_segment.clone());
        if let Some(code) = save(&v, &format!("{outname}_pveseg"), run, io) {
            return code;
        }
        let v: Volume<i32> = grid(&props, mri.hard_pv.clone());
        if let Some(code) = save(&v, &format!("{outname}_mixeltype"), run, io) {
            return code;
        }
    }
    if a.output_corrected {
        for (ch, image) in images.iter_mut().enumerate() {
            for (v, &b) in image.data.iter_mut().zip(mri.final_bias(ch)) {
                *v *= b;
            }
            if let Some(code) = save(image, &format!("{outname}_restore_{}", ch + 1), run, io) {
                return code;
            }
        }
    }
    if a.output_bias {
        for ch in 0..nch {
            let field: Vec<f32> = mri.final_bias(ch).iter().map(|&b| 1.0 / b).collect();
            let v: Volume = grid(&props, field);
            if let Some(code) = save(&v, &format!("{outname}_bias_{}", ch + 1), run, io) {
                return code;
            }
        }
    }
    0
}
