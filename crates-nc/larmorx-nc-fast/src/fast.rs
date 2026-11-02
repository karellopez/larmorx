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

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_nc_core::cnum;
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

/// `fast`'s `main`: `args[0]` is the program name. Returns the exit status. The notices of
/// FSL's defects are printed last, as every `larmorx-nc` tool prints them.
pub fn main(args: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut notices = Notices::new(run.mode);
    let code = exception::catch(|| {
        let mut io = Io {
            out: &mut *out,
            err: &mut *err,
        };
        main_inner(args, run, &mut io, &mut notices)
    });
    for n in notices.as_slice() {
        let _ = writeln!(err, "larmorx-nc: {}", n.line());
    }
    // an exception nothing in FAST catches goes on to the program's entry (std::terminate)
    code.unwrap_or_else(|e| exception::throw(e))
}

fn main_inner(args: &[String], run: &Run, io: &mut Io, notices: &mut Notices) -> u8 {
    let parser = parser();
    let parsed = match parser.parse_command_line(args, 0, true) {
        Ok(p) => p,
        Err((_, what)) => {
            let _ = io.err.write_all(parser.usage().as_bytes());
            let _ = writeln!(io.err, "\n{what}");
            return 1;
        }
    };
    let help = parsed.get(&parser, "-h,--help").bool();
    if args.len() < 2 || help || parser.check_compulsory(&parsed).is_some() {
        let _ = io.err.write_all(parser.usage().as_bytes());
        return 1;
    }
    let a = args_of(&parser, &parsed);
    if a.hyper < 0.0 {
        let _ = writeln!(
            io.err,
            "ERROR: Segmentation smoothness must be positive. Exiting"
        );
        return 1;
    }
    if let Some(code) = meaningless_options(&a, io, notices) {
        return code;
    }
    if a.nchannel == 1 {
        return segment_single_channel(args, &a, run, io, notices);
    }
    segment_multi_channel(args, &a, run, io, notices)
}

/// A refusal of larmorx's (D13): `larmorx-nc: error: fast: ...`, exit status 1.
fn refuse(io: &mut Io, what: &str) -> u8 {
    let _ = writeln!(io.err, "larmorx-nc: error: fast: {what}");
    1
}

/// FSL-99 (rule B): the numeric options FAST uses as given although they have no meaning: a
/// smoothness (`-H`, `-f`, `-R`) that is not a finite number (a NaN `-H` passes FAST's check
/// for a negative one and estimates the smoothness against a table it never filled), a bias
/// field smoothing extent (`-l`) that is not a positive finite number, fewer than one class
/// (`-n`). Refused by default; FAST's run with `reproduce` (which aborts for `-n` below 1).
fn meaningless_options(a: &Args, io: &mut Io, notices: &mut Notices) -> Option<u8> {
    let mut bad: Vec<String> = Vec::new();
    for (opt, v) in [("-H", a.hyper), ("-f", a.fbeta), ("-R", a.mixel)] {
        if !v.is_finite() {
            bad.push(format!("{opt} {}", cnum::stream(f64::from(v))));
        }
    }
    if !(a.nblowpass.is_finite() && a.nblowpass > 0.0) {
        bad.push(format!("-l {}", cnum::stream(f64::from(a.nblowpass))));
    }
    if a.nclass < 1 {
        bad.push(format!("-n {}", a.nclass));
    }
    if bad.is_empty() {
        return None;
    }
    let which = bad.join(", ");
    if notices.fix() {
        notices.add(&defects::FSL_99, format!("{which} refused"));
        return Some(refuse(
            io,
            &format!(
                "{which}: a smoothness must be a finite number, the bias field's smoothing \
                 extent a positive one and the classes at least 1; FAST uses them as given \
                 (FSL-99)"
            ),
        ));
    }
    notices.add(
        &defects::FSL_99,
        format!("{which} used as FAST uses them, although they have no meaning"),
    );
    None
}

/// `make_basename` of the output name (the input's name without `-o`).
fn output_name(a: &Args, input: &str) -> String {
    volume::make_basename(a.outname.as_deref().unwrap_or(input))
}

/// What `prior_registration` returns: the use of priors (0: none, 1: to initialise, 2:
/// throughout with `-P`; -1 when FLIRT's outputs are missing) and the priors.
struct Priors {
    bapused: i32,
    volumes: Option<[Volume; 3]>,
}

/// How `prior_registration` stops FAST: an exit status, or an exception nothing catches.
enum Stop {
    Exit(u8),
    Throw(FslError),
}

/// `FslFileExists(name)`: an image of that name (any of newimage's extensions) exists.
fn file_exists(name: &str) -> bool {
    volume::return_validimagefilename(name).is_ok()
}

/// `read_volume(v, name)`: newimage's 4D warning printed; its exceptions unwind.
fn read_image(name: &str, run: &Run, io: &mut Io) -> Volume {
    match volume::read_volume::<f32>(name, run.fsl) {
        Ok((v, warning)) => {
            if let Some(w) = warning {
                let _ = writeln!(io.err, "{w}");
            }
            v
        }
        Err(e) => exception::throw(e),
    }
}

/// The blanks the shell splits words at, and its special characters (`/bin/sh` reads FAST's
/// FLIRT commands, the names unquoted).
fn shell_special(word: &str) -> bool {
    word.chars().any(|c| "|&;<>()$`\\\"'*?[".contains(c))
        || word.starts_with('#')
        || word.starts_with('~')
}

fn has_blank(word: &str) -> bool {
    word.chars().any(|c| matches!(c, ' ' | '\t' | '\n'))
}

/// One of `prior_registration`'s FLIRT runs, `$FSLDIR/bin/flirt -ref <input> -in <prior> -out
/// <out> -applyxfm -init <matrix>`, which FAST runs with `system()` (`/bin/sh -c`, the names
/// unquoted): the FLIRT replica, in this process. FSL-102: with `reproduce` the command's words
/// are the shell's (a name with a blank split into words; a name with the shell's special
/// characters refused, their expansion not reproduced; FSLDIR unset: the text `(null)`, a
/// syntax error of the shell's); by default each name is one argument and FLIRT needs no FSLDIR.
#[allow(clippy::too_many_arguments)]
fn run_flirt(
    fsldir: Option<&str>,
    names: [&str; 4],
    a: &Args,
    run: &Run,
    io: &mut Io,
    notices: &mut Notices,
) -> Result<(), Stop> {
    let [reference, prior, output, matrix] = names;
    let program = format!("{}/bin/flirt", fsldir.unwrap_or("(null)"));
    let cmd =
        format!("{program} -ref {reference} -in {prior} -out {output} -applyxfm -init {matrix}");
    if a.verbose {
        let _ = writeln!(io.out, "{cmd}");
        let _ = io.out.flush();
    }
    let odd: Vec<&str> = names
        .iter()
        .copied()
        .filter(|w| has_blank(w) || shell_special(w) || w.is_empty())
        .collect();
    let words: Vec<String> = if notices.reproduce() {
        if fsldir.is_none() {
            // sprintf's "(null)": "(" starts a subshell the shell cannot parse
            let _ = writeln!(io.err, "sh: 1: Syntax error: word unexpected");
            return Ok(());
        }
        if let Some(w) = odd.iter().find(|w| shell_special(w)) {
            let _ = writeln!(
                io.err,
                "larmorx-nc: error: fast: -a runs FLIRT through the shell, which would \
                 interpret the special characters of {w:?}; that is not reproduced (FSL-102)"
            );
            return Err(Stop::Exit(1));
        }
        if !odd.is_empty() {
            notices.add(
                &defects::FSL_102,
                format!("FLIRT's command split at the blanks of {odd:?}, as the shell splits it"),
            );
        }
        cmd.split([' ', '\t', '\n'])
            .filter(|w| !w.is_empty())
            .skip(1)
            .map(str::to_owned)
            .collect()
    } else {
        if fsldir.is_none() {
            notices.add(
                &defects::FSL_102,
                "FSLDIR unset: FAST's FLIRT command starts with (null), which the shell cannot \
                 run (FAST then crashes); FLIRT run in this process",
            );
        }
        if !odd.is_empty() {
            notices.add(
                &defects::FSL_102,
                format!(
                    "{odd:?} passed to FLIRT whole: FAST's shell would split or interpret them"
                ),
            );
        }
        [
            "-ref",
            reference,
            "-in",
            prior,
            "-out",
            output,
            "-applyxfm",
            "-init",
            matrix,
        ]
        .iter()
        .map(|w| (*w).to_owned())
        .collect()
    };
    // FLIRT's own process: its exceptions end it (std::terminate), not FAST
    let r = exception::catch(|| {
        larmorx_nc_flirt::flirt(&words, run.fsl, run.mode, run.threads, io.out, io.err)
    });
    if let Err(e) = r {
        let _ = write!(
            io.err,
            "{}terminate called after throwing an instance of '{}'\n  what():  {}\n",
            e.printed_text(),
            e.type_name,
            e.what
        );
    }
    let _ = io.out.flush();
    Ok(())
}

/// `prior_registration(inname, main_prior_vol, pCSF, pGM, pWM)`: the tissue priors (FSL's
/// standard ones in `$FSLDIR/data/standard/tissuepriors`, or `-A`'s), registered to the input
/// with FLIRT and `-a`'s matrix (FLIRT's outputs `<out>_{csf,gm,wm}_stdspace` stay).
fn prior_registration(
    a: &Args,
    inname: &str,
    main_prior_vol: &str,
    run: &Run,
    io: &mut Io,
    notices: &mut Notices,
) -> Result<Priors, Stop> {
    let fsldir = run.fsl.get("FSLDIR");
    let names: [String; 3] = match (&a.alternate_priors, fsldir) {
        (Some(n), _) => n.clone(),
        (None, Some(dir)) => {
            let root = format!("{dir}/data/standard/tissuepriors/avg152T1_");
            [
                format!("{root}csf"),
                format!("{root}gray"),
                format!("{root}white"),
            ]
        }
        // string(getenv("FSLDIR")): a null pointer when FSLDIR is unset (FSL-100)
        (None, None) if notices.reproduce() || !a.bapriori.is_empty() => {
            if notices.fix() {
                notices.add(
                    &defects::FSL_100,
                    "FSLDIR unset with -a: FAST aborts; refused",
                );
                let _ = refuse(
                    io,
                    "-a: FSL's standard tissue priors are read from \
                     $FSLDIR/data/standard/tissuepriors and FSLDIR is unset; name the priors \
                     with -A (FSL-100)",
                );
                return Err(Stop::Exit(1));
            }
            notices.add(
                &defects::FSL_100,
                "FSLDIR unset: FAST aborts before it segments, priors asked for or not",
            );
            return Err(Stop::Throw(FslError {
                printed: Vec::new(),
                type_name: "std::logic_error",
                what: "basic_string: construction from null is not valid".to_owned(),
            }));
        }
        (None, None) => {
            notices.add(
                &defects::FSL_100,
                "FSLDIR unset: FAST aborts before it segments, although no priors are asked \
                 for; segmented",
            );
            [String::new(), String::new(), String::new()]
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
    let mut vols: [Option<Volume>; 3] = [None, None, None];
    if bapused > 0 {
        for (v, name) in vols.iter_mut().zip(&names) {
            if file_exists(name) {
                *v = Some(read_image(name, run, io));
            } else {
                let _ = writeln!(
                    io.err,
                    "prior image {name} is not found! priors are not used!"
                );
                bapused = 0;
            }
        }
    }
    let tissues = [
        ("csf", "csf"),
        ("gm", "grey matter"),
        ("wm", "white matter"),
    ];
    if bapused > 0 {
        for (i, (t, _)) in tissues.iter().enumerate() {
            let output = format!("{main_prior_vol}_{t}_stdspace");
            run_flirt(
                fsldir,
                [inname, &names[i], &output, &a.bapriori],
                a,
                run,
                io,
                notices,
            )?;
        }
        for (i, (t, what)) in tissues.iter().enumerate() {
            let name = format!("{main_prior_vol}_{t}_stdspace");
            if file_exists(&name) {
                vols[i] = Some(read_image(&name, run, io));
            } else {
                let _ = writeln!(
                    io.err,
                    "{what} prior image not transformed correctly! priors are not used!"
                );
                return Ok(Priors {
                    bapused: -1,
                    volumes: None,
                });
            }
        }
        if a.prior_throughout {
            bapused = 2;
        }
        let [c, g, w] = vols;
        return Ok(Priors {
            bapused,
            volumes: Some([c.unwrap(), g.unwrap(), w.unwrap()]),
        });
    }
    Ok(Priors {
        bapused,
        volumes: None,
    })
}

/// FSL-101: FLIRT's outputs missing (a matrix it cannot read, a prior it cannot register):
/// FAST says the priors are not used, then initialises from priors it never allocated and
/// crashes. By default the segmentation runs without them, as the message says; `reproduce`
/// gives what FAST defines before it crashes: `-v`'s "Beginning prior-based initialisation"
/// (its first act there), and with several channels of one size whose first voxel lies in the
/// mask (every channel above 0), the exception FAST throws at that voxel (its unallocated
/// priors' bounds-checked time index: "Out of Bounds (time index)", then an abort); elsewhere
/// FAST writes through a null pointer (or, with an empty mask, runs on): refused.
/// `multi_first_masked`: with several channels, whether that exception is FAST's next act.
fn unregistered_priors(
    priors: &mut Priors,
    io: &mut Io,
    notices: &mut Notices,
    verbose: bool,
    multi_first_masked: bool,
) -> Option<u8> {
    if priors.bapused != -1 {
        return None;
    }
    if notices.fix() {
        notices.add(
            &defects::FSL_101,
            "FLIRT's registered priors are missing: segmented without priors, as FAST's message \
             says, where FAST crashes",
        );
        priors.bapused = 0;
        return None;
    }
    notices.add(
        &defects::FSL_101,
        "FLIRT's registered priors are missing: FAST crashes here (memory it never allocated)",
    );
    if verbose {
        let _ = writeln!(io.out, "Beginning prior-based initialisation");
    }
    if multi_first_masked {
        let _ = io.out.flush();
        exception::throw(FslError::imthrow("Out of Bounds (time index)", 5));
    }
    Some(refuse(
        io,
        "the registered priors are missing and FAST initialises from priors it never \
         allocated: it crashes here (FSL-101; the default segments without priors)",
    ))
}

/// Resolves `prior_registration`'s outcome: the priors, or FAST's exit.
fn priors_or_exit(
    r: Result<Priors, Stop>,
    io: &mut Io,
    notices: &mut Notices,
    verbose: bool,
    multi_first_masked: bool,
) -> Result<Priors, u8> {
    match r {
        Ok(mut p) => match unregistered_priors(&mut p, io, notices, verbose, multi_first_masked) {
            Some(code) => Err(code),
            None => Ok(p),
        },
        Err(Stop::Exit(code)) => Err(code),
        Err(Stop::Throw(e)) => exception::throw(e),
    }
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
fn segment_single_channel(
    args: &[String],
    a: &Args,
    run: &Run,
    io: &mut Io,
    notices: &mut Notices,
) -> u8 {
    if a.verbose {
        let _ = writeln!(io.out, "Starting Single Image Segmentation");
    }
    let input_name = args[args.len() - 1].clone();
    let outname = output_name(a, &input_name);
    let mut input: Volume = read_image(&input_name, run, io);
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
    let r = prior_registration(a, &input_name, &outname, run, io, notices);
    // FSL-101 with `reproduce`: what FAST does next depends on its mask (decided below)
    let unregistered = !notices.fix() && matches!(&r, Ok(p) if p.bapused == -1);
    let priors = if unregistered {
        match r {
            Ok(p) => p,
            Err(_) => unreachable!("matched above"),
        }
    } else {
        match priors_or_exit(r, io, notices, a.verbose, false) {
            Ok(p) => p,
            Err(code) => return code,
        }
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
        mode: run.mode,
    };
    let mut mri = Segmentation::create(&input, params);
    if mri.max_left_out > 0 {
        notices.add(
            &defects::FSL_103,
            format!(
                "{} voxel(s) of the image's maximum {} {} the mask",
                mri.max_left_out,
                cnum::stream(f64::from(input.max())),
                if notices.fix() {
                    "kept in"
                } else {
                    "left out of"
                }
            ),
        );
    }
    // FSL-101 (`reproduce`): FAST initialises from priors it never allocated; with an empty
    // mask no voxel reads them and FAST runs on (every statistic of an empty mask), otherwise it
    // writes through a null pointer at the first voxel of the mask: refused there
    let unallocated: [Volume; 3];
    let pr = if unregistered {
        if !mri.mask_is_empty() {
            let mut p = priors;
            return unregistered_priors(&mut p, io, notices, a.verbose, false).unwrap_or(1);
        }
        notices.add(
            &defects::FSL_101,
            "FLIRT's registered priors are missing: FAST initialises from priors it never \
             allocated, which no voxel of its empty mask reads",
        );
        let mut z = Volume::<f32>::new(input.xsize(), input.ysize(), input.zsize(), 1);
        z.copy_properties(&input);
        unallocated = [z.clone(), z.clone(), z];
        Some([&unallocated[0], &unallocated[1], &unallocated[2]])
    } else {
        priors.volumes.as_ref().map(|[c, g, w]| [c, g, w])
    };
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
        if mri.members_written {
            unwritten_notice(mri.outside_mask(), notices);
        } else {
            notices.add(
                &defects::FSL_96,
                "-p's probability maps, which FAST never writes with these options (priors, or \
                 no k-means iteration, with --nopve: it saves what its memory held): written as 0",
            );
        }
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

/// FSL-96: `-p`'s probability maps outside the mask, where FAST never writes them (it saves
/// whatever its memory held): 0 in both modes, named when the mask leaves voxels out.
fn unwritten_notice(outside: usize, notices: &mut Notices) {
    if outside > 0 {
        notices.add(
            &defects::FSL_96,
            format!(
                "-p's probability maps outside the mask ({outside} voxels), which FAST never \
                 writes (it saves what its memory held): written as 0"
            ),
        );
    }
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
fn segment_multi_channel(
    args: &[String],
    a: &Args,
    run: &Run,
    io: &mut Io,
    notices: &mut Notices,
) -> u8 {
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
    let input_name = args[args.len() - 1].clone();
    let outname = output_name(a, &input_name);
    let mut images: Vec<Volume> = Vec::with_capacity(nch);
    for c in 0..nch {
        // argv[argc - c - 1]: the words before the options and argv[0] (the program's own path,
        // which never reads as an image: FAST aborts there); before argv[0] is memory
        let Some(name) = args.len().checked_sub(c + 1).map(|i| &args[i]) else {
            return refuse(
                io,
                &format!("-S {nch}: fewer words than channels before them on the command line"),
            );
        };
        let mut image: Volume = read_image(name, run, io);
        clear_negative(&mut image);
        image.display_max = 0.0;
        image.display_min = 0.0;
        images.push(image);
    }
    // (after the images are read: FAST's errors in reading them come first). More than eight
    // channels are refused in both modes (the orchestrator, 2026-11-02): FSL's results there
    // follow OpenBLAS's kernels for larger matrices (iamax's aligned path, the 8-way dot
    // products, getrf's blocked path), which are not ported.
    if nch > MAX_CHANNELS {
        return refuse(
            io,
            &format!(
                "-S {nch}: more than {MAX_CHANNELS} channels are not replicated (FSL's results \
                 there follow OpenBLAS's kernels for larger matrices, which larmorx does not \
                 reproduce)"
            ),
        );
    }
    let r = prior_registration(a, &input_name, &outname, run, io, notices);
    // FSL-101's exception (reproduce): FAST's first voxel in the mask of every channel, the
    // channels of one size (otherwise FAST reads past the smaller ones first: FSL-104)
    let first_masked = images.iter().all(|v| {
        (v.xsize(), v.ysize(), v.zsize())
            == (images[0].xsize(), images[0].ysize(), images[0].zsize())
            && v.data.first().is_some_and(|&x| x > 0.0)
    });
    let priors = match priors_or_exit(r, io, notices, a.verbose, first_masked) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let (nx, ny, nz) = (images[0].xsize(), images[0].ysize(), images[0].zsize());
    let mut size_error = false;
    if images
        .iter()
        .any(|v| (v.xsize(), v.ysize(), v.zsize()) != (nx, ny, nz))
    {
        // FSL-104: FAST works over channel 1's grid, reading every channel at channel 1's voxel
        // indices. When channel 1 fits inside every other channel those reads stay inside them
        // (each channel's corner of channel 1's size) and FAST aborts at its first percentile of
        // another channel (unless -s gives every mean); otherwise it reads past the smaller ones
        let fits = images
            .iter()
            .all(|v| v.xsize() >= nx && v.ysize() >= ny && v.zsize() >= nz);
        let nc = a.nclass.max(0) as usize;
        let means_given =
            !a.manualseg.is_empty() && crate::mriseg::read_floats(&a.manualseg).len() == nc * nch;
        if notices.reproduce() && fits && !means_given {
            notices.add(
                &defects::FSL_104,
                "-S with images of different sizes: FAST reads each channel's corner of channel \
                 1's size and aborts at its first percentile of another channel",
            );
            for v in images.iter_mut().skip(1) {
                let (vx, vy) = (v.xsize() as usize, v.ysize() as usize);
                let mut corner = Vec::with_capacity((nx * ny * nz) as usize);
                for z in 0..nz as usize {
                    for y in 0..ny as usize {
                        let row = (z * vy + y) * vx;
                        corner.extend_from_slice(&v.data[row..row + nx as usize]);
                    }
                }
                v.dims[0] = nx;
                v.dims[1] = ny;
                v.dims[2] = nz;
                v.dims[3] = 1;
                v.data = corner;
            }
            size_error = true;
        } else {
            notices.add(
                &defects::FSL_104,
                format!("-S {nch} with images of different sizes refused"),
            );
            return refuse(
                io,
                &format!(
                    "-S {nch}: the channels' images differ in size (FAST reads every channel over \
                     channel 1's grid: past the smaller ones, or a corner of the larger ones \
                     before it aborts; FSL-104)"
                ),
            );
        }
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
        mode: run.mode,
        size_error,
    };
    if a.nclass < 0 {
        // TanakaCreate's first matrix, m_mean.ReSize(nclasses, numberofchannels)
        exception::throw(FslError {
            printed: Vec::new(),
            type_name: "std::logic_error",
            what: "Mat::init(): requested size is too large".to_owned(),
        });
    }
    let mut mri = MultiSegmentation::create(&images, params);
    let pr = priors.volumes.as_ref().map(|[c, g, w]| [c, g, w]);
    match mri.main(pr, io) {
        Ok(0) => {}
        // TanakaMain's -1: main returns -1
        Ok(_) => return 255,
        Err(e) => exception::throw(e),
    }
    if mri.sort_not_involution {
        notices.add(
            &defects::FSL_98,
            if notices.fix() {
                "the classes sorted by channel 1's means with their own covariances, \
                 posteriors and other channels' means (FAST gives each sorted class another \
                 class's)"
            } else {
                "the classes sorted as FAST sorts them: each sorted class gets another class's \
                 covariance, posteriors and other channels' means"
            },
        );
    }
    if a.remove_bias && (a.output_bias || a.output_corrected) {
        notices.add(
            &defects::FSL_97,
            "-N with -b or -B: FAST saves a bias field it never wrote (memory); written as no \
             bias field (1, and each channel's image)",
        );
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
        unwritten_notice(mri.outside_mask(), notices);
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
