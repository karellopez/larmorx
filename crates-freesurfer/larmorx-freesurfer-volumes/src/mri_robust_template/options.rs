// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_template`'s command line (`mri_robust_template.cpp` 82-133 `Parameters` and their
//! defaults, 406-410 `printUsage`, 421-874 `parseNextCommand`, 884-954 `parseCommandLine`).
//!
//! No argument at all: the help, status 1. The options are read while the next argument starts
//! with `-`: one dash or two are dropped and the rest compared in upper case (`StrUpper`, which
//! also changes the argument the unknown-option error prints). Each option prints its line on
//! standard output (`--threads` none); an option that takes one argument takes the next one
//! whatever it is; an option that takes a list (`--mov`, `--lta`, `--masks`, `--iscaleout`,
//! `--iscalein`, `--ixforms`, `--weights`, `--warp`/`--mapmov`, `--mapmovhdr`) takes the
//! arguments up to the next one that starts with `-` (none at all leaves the list as it is: the
//! official build's assertion is compiled out). The list options add to their lists when given
//! twice. Then the checks, each on standard error with status 1 and no help: at least one
//! `--mov`, `--template`, each output list as long as `--mov`'s (`--iscalein` as long as
//! `--ixforms`'), and `--satit` or `--sat` above 0 (or `--noit` with `--ixforms`).
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-robust-template.md`):
//! - **FS-1080** (crash): an option given last without its argument: FreeSurfer reads past its
//!   arguments: a list option or a number (`atoi`, `atof`, the list's first element) dies of a
//!   segmentation fault (status 139), `--template` aborts (`std::string` of `NULL`:
//!   libstdc++'s `terminate called after throwing an instance of 'std::logic_error'`, status 134).
//!   By default an error naming the option (status 1); `reproduce` ends as FreeSurfer does.

use larmorx_core::cfmt::ostream;
use larmorx_core::defects::FS_1080;
use larmorx_freesurfer_core::cstd::{atof, atoi};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_robust::registration::{Io, RegRobust};

pub use crate::mri_robust_register::options::Run;

/// `mri_robust_template.help.xml`, the text `printUsage` renders (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("mri_robust_template.help.xml");

/// `SAMPLE_NEAREST` (`include/mri.h`): `--finalnearest`.
pub const SAMPLE_NEAREST: i32 = 0;
/// `SAMPLE_TRILINEAR`.
pub const SAMPLE_TRILINEAR: i32 = 1;
/// `SAMPLE_CUBIC_BSPLINE`: the template's default resampling.
pub const SAMPLE_CUBIC_BSPLINE: i32 = 5;

/// `Parameters` with `P`'s defaults.
#[derive(Clone, Debug)]
pub struct Options {
    pub mov: Vec<String>,
    pub masks: Vec<String>,
    pub mean: String,
    pub iltas: Vec<String>,
    pub nltas: Vec<String>,
    pub nweights: Vec<String>,
    pub mapmovhdr: Vec<String>,
    pub fixvoxel: bool,
    pub floattype: bool,
    pub lta_vox2vox: bool,
    pub affine: bool,
    pub iscale: bool,
    pub iscaleonly: bool,
    pub transonly: bool,
    pub leastsquares: bool,
    pub nomulti: bool,
    pub iterate: i32,
    pub epsit: f64,
    pub sat: f64,
    pub nwarps: Vec<String>,
    pub debug: i32,
    pub average: i32,
    pub inittp: i32,
    pub noit: bool,
    pub quick: bool,
    pub subsamplesize: i32,
    pub fixtp: bool,
    pub satit: bool,
    pub conform: String,
    pub doubleprec: bool,
    pub oneminusweights: bool,
    pub iscalein: Vec<String>,
    pub iscaleout: Vec<String>,
    pub finalinterp: i32,
    pub highit: i32,
    /// `unsigned int seed` (0: none given).
    pub seed: u32,
    pub crascenter: bool,
    pub pairiterate: i32,
    pub pairepsit: f64,
    pub resthresh: f32,
    pub frobnormthresh: f64,
    pub allow_diff_vox_size: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mov: Vec::new(),
            masks: Vec::new(),
            mean: String::new(),
            iltas: Vec::new(),
            nltas: Vec::new(),
            nweights: Vec::new(),
            mapmovhdr: Vec::new(),
            fixvoxel: false,
            floattype: false,
            lta_vox2vox: false,
            affine: false,
            iscale: false,
            iscaleonly: false,
            transonly: false,
            leastsquares: false,
            nomulti: false,
            iterate: 5,
            epsit: -1.0,
            sat: -1.0,
            nwarps: Vec::new(),
            debug: 0,
            average: 1,
            inittp: -1,
            noit: false,
            quick: false,
            subsamplesize: -1,
            fixtp: false,
            satit: false,
            conform: String::new(),
            doubleprec: false,
            oneminusweights: true,
            iscalein: Vec::new(),
            iscaleout: Vec::new(),
            finalinterp: SAMPLE_CUBIC_BSPLINE,
            highit: -1,
            seed: 0,
            crascenter: false,
            pairiterate: 5,
            pairepsit: 0.01,
            resthresh: 0.01,
            frobnormthresh: 0.0001,
            allow_diff_vox_size: false,
        }
    }
}

/// `printUsage()`: the help of `mri_robust_template.help.xml`.
pub fn print_usage(g: &mut Globals, r: &mut Run) {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
}

/// libstdc++'s lines when `std::string(NULL)` throws and nobody catches it (GCC 8.4's
/// `basic_string`, inlined into the official build).
const NULL_STRING_ABORT: &str = "terminate called after throwing an instance of 'std::logic_error'\n  what():  basic_string::_M_construct null not valid\n";

/// FS-1080: an option's argument past the last one. `string` tells whether FreeSurfer builds a
/// `std::string` of it (an abort) or reads a number or a list's first element (a segmentation
/// fault).
fn missing(g: &mut Globals, r: &mut Run, arg: &str, string: bool) -> Fatal {
    g.notices.add(
        &FS_1080,
        format!(
            "{arg}: no argument after the option (FreeSurfer reads past its arguments and crashes)"
        ),
    );
    if g.notices.reproduce() {
        let _ = r.out.flush();
        if string {
            let _ = r.err.write_all(NULL_STRING_ABORT.as_bytes());
            return Fatal::Exit(134);
        }
        return Fatal::Exit(139);
    }
    let _ = writeln!(
        r.err,
        "mri_robust_template: option {arg} needs an argument (FreeSurfer crashes here)"
    );
    Fatal::Exit(1)
}

/// The list after a list option (`do { option = argv[nargs + 1]; ... } while (nargs + 1 < argc &&
/// option[0] != '-')`): the arguments up to the next one that starts with `-`. `Err` (FS-1080)
/// when the option is the last argument (FreeSurfer reads `argv[argc]`).
fn list(
    g: &mut Globals,
    r: &mut Run,
    args: &[String],
    into: &mut Vec<String>,
    mut each: impl FnMut(&mut Run, &str),
) -> Result<usize, Fatal> {
    if args.len() < 2 {
        return Err(missing(g, r, &args[0], false));
    }
    let mut nargs = 0;
    loop {
        let option = &args[nargs + 1];
        let dash = option.as_bytes().first() == Some(&b'-');
        if !dash {
            nargs += 1;
            into.push(option.clone());
            each(r, option);
        }
        if !(nargs + 1 < args.len() && !dash) {
            break;
        }
    }
    Ok(nargs)
}

/// `parseNextCommand(argc, argv, P)` on `args[0]` (the option) and what follows it: the arguments
/// it takes after its own. `Err` ends the program (the help, an unknown option, the test mode
/// `--test`, which runs `RegRobust::testRobust` and ends with status 0).
fn parse_next_command(
    g: &mut Globals,
    r: &mut Run,
    p: &mut Options,
    args: &[String],
) -> Result<usize, Fatal> {
    let arg = &args[0];
    let bytes = arg.as_bytes();
    // `option = argv[0] + 1`, and once more for a second dash; then `StrUpper(option)` in place
    let skip = if bytes.get(1) == Some(&b'-') { 2 } else { 1 };
    let option = arg[skip.min(arg.len())..].to_ascii_uppercase();
    let shown = format!("{}{option}", &arg[..skip.min(arg.len())]);
    let next = args.get(1);
    let s = |g: &mut Globals, r: &mut Run| -> Result<String, Fatal> {
        next.cloned().ok_or_else(|| missing(g, r, arg, true))
    };
    let i = |g: &mut Globals, r: &mut Run| -> Result<i32, Fatal> {
        next.map(|v| atoi(v.as_bytes()))
            .ok_or_else(|| missing(g, r, arg, false))
    };
    let f = |g: &mut Globals, r: &mut Run| -> Result<f64, Fatal> {
        next.map(|v| atof(v.as_bytes()))
            .ok_or_else(|| missing(g, r, arg, false))
    };
    let mut nargs = 0;
    macro_rules! say {
        ($($t:tt)*) => {{ let _ = writeln!(r.out, $($t)*); }};
    }
    match option.as_str() {
        "MOV" => {
            nargs = list(g, r, args, &mut p.mov, |r, m| {
                let _ = writeln!(r.out, "--mov: Using {m} as movable/source volume.");
            })?;
            say!("    Total: {nargs} input volumes");
        }
        "TEMPLATE" => {
            p.mean = s(g, r)?;
            nargs = 1;
            say!("--template: Using {} as template output volume.", p.mean);
        }
        "LTA" => {
            nargs = list(g, r, args, &mut p.nltas, |_, _| {})?;
            say!("--lta: Will output LTA transforms");
        }
        "MASKS" => {
            nargs = list(g, r, args, &mut p.masks, |_, _| {})?;
            say!("--masks: Will use masks on inputs.");
        }
        "ISCALEOUT" => {
            nargs = list(g, r, args, &mut p.iscaleout, |_, _| {})?;
            p.iscale = true;
            say!("--iscaleout: Will perform intensity scaling and output results");
        }
        "ISCALEIN" => {
            nargs = list(g, r, args, &mut p.iscalein, |_, _| {})?;
            say!("--iscalein: Will use init intensity scales");
        }
        "IXFORMS" => {
            nargs = list(g, r, args, &mut p.iltas, |_, _| {})?;
            say!("--ixforms: Will use init XFORMS.");
        }
        "AVERAGE" => {
            p.average = i(g, r)?;
            nargs = 1;
            say!(
                "--average: Using method {} for template computation.",
                p.average
            );
        }
        "VOX2VOX" => {
            p.lta_vox2vox = true;
            say!("--vox2vox: Output transforms as VOX2VOX. ");
        }
        "AFFINE" | "A" => {
            p.affine = true;
            say!("--affine: Enabling affine transform!");
        }
        "ISCALE" | "I" => {
            p.iscale = true;
            say!("--iscale: Enabling intensity scaling!");
        }
        "TRANSONLY" => {
            p.transonly = true;
            say!("--transonly: Using only translation!");
        }
        "ISCALEONLY" => {
            p.iscale = true;
            p.iscaleonly = true;
            p.transonly = false;
            p.affine = false;
            say!("--iscaleonly: Computing only global scaling!");
        }
        "NOMULTI" => {
            p.nomulti = true;
            say!("--nomulti: process highest resolution only!");
        }
        "LEASTSQUARES" | "L" => {
            p.leastsquares = true;
            say!("--leastsquares: Using standard least squares (non-robust)!");
        }
        "MAXIT" => {
            p.iterate = i(g, r)?;
            nargs = 1;
            say!(
                "--maxit: Performing maximal {} iterations on each resolution.",
                p.iterate
            );
        }
        "HIGHIT" => {
            p.highit = i(g, r)?;
            nargs = 1;
            say!(
                "--highit: Performing maximal {} iterations on highest resolution",
                p.highit
            );
        }
        "EPSIT" => {
            p.epsit = f(g, r)?;
            nargs = 1;
            say!(
                "--epsit: Stop iterations when change is less than {} . ",
                ostream(p.epsit)
            );
        }
        "PAIRMAXIT" => {
            p.pairiterate = i(g, r)?;
            nargs = 1;
            say!(
                "--pairmaxit: Performing maximal {} iterations on each resolution for individual pairwise registrations.",
                p.pairiterate
            );
        }
        "PAIREPSIT" => {
            p.pairepsit = f(g, r)?;
            nargs = 1;
            say!(
                "--pairepsit: Stop individual pairwise iterations when change is less than {} . ",
                ostream(p.pairepsit)
            );
        }
        "SAT" => {
            p.sat = f(g, r)?;
            nargs = 1;
            say!("--sat: Using saturation {} in M-estimator!", ostream(p.sat));
        }
        "SUBSAMPLE" => {
            p.subsamplesize = i(g, r)?;
            nargs = 1;
            if p.subsamplesize >= 0 {
                say!(
                    "--subsample: Will subsample if size is larger than {} on all axes!",
                    p.subsamplesize
                );
            } else {
                say!("--subsample -1: Will not subsample on any scale!");
            }
        }
        "DEBUG" => {
            p.debug = 1;
            say!("--debug: Will output debug info and files!");
        }
        "NOIT" => {
            p.noit = true;
            say!("--noit: Will output only first template (no iterations)!");
        }
        "FIXTP" => {
            p.fixtp = true;
            say!("--fixtp: Will map everything to init TP!");
        }
        "ALLOW-DIFF-VOX-SIZE" => {
            p.allow_diff_vox_size = true;
            say!("--allow-diff-vox-size ");
        }
        "SATIT" => {
            p.satit = true;
            say!("--satit: Will estimate SAT iteratively!");
        }
        "DOUBLEPREC" => {
            p.doubleprec = true;
            say!("--doubleprec: Will perform algorithm with double precision (higher mem usage)!");
        }
        "WEIGHTS" => {
            nargs = list(g, r, args, &mut p.nweights, |_, _| {})?;
            say!("--weights: Will output weights in target space");
        }
        "WARP" | "MAPMOV" => {
            nargs = list(g, r, args, &mut p.nwarps, |_, _| {})?;
            say!("--mapmov: Will save mapped movables/sources!");
        }
        "MAPMOVHDR" => {
            nargs = list(g, r, args, &mut p.mapmovhdr, |_, _| {})?;
            say!("--mapmovhdr: Will save header-adjusted movables!");
        }
        "TEST" => {
            // `RegRobust R; R.testRobust(argv[2], atoi(argv[1])); exit(0)`: the number first
            // (FreeSurfer's `atoi(NULL)` crashes when there is none), then the image's name
            say!("--test: TEST-MODE ");
            let testno = i(g, r)?;
            let Some(fname) = args.get(2).cloned() else {
                return Err(missing(g, r, arg, true));
            };
            let mut reg = RegRobust::new();
            let mut io = Io {
                out: &mut *r.out,
                err: &mut *r.err,
                env: r.env,
                dir: r.dir,
                input: None,
            };
            reg.test_robust(g, &mut io, &fname, testno)?;
            return Err(Fatal::Exit(0));
        }
        "CONFORM" => {
            // `P.conform = argv[1]`: the assignment of NULL reads it (a segmentation fault)
            p.conform = next.cloned().ok_or_else(|| missing(g, r, arg, false))?;
            nargs = 1;
            say!("--conform: Will output conform template (256^3 and 1mm voxels)!");
        }
        "FLOATTYPE" => {
            p.floattype = true;
            say!("--floattype: Use float images internally (independent of input)!");
        }
        "FINALNEAREST" => {
            p.finalinterp = SAMPLE_NEAREST;
            say!("--finalnearest: Use nearest neighbor interpolation for final average!");
        }
        "INITTP" => {
            p.inittp = i(g, r)?;
            nargs = 1;
            if p.inittp == 0 {
                say!("--inittp 0: No initialization, construct first mean from original TPs");
            } else {
                say!(
                    "--inittp: Using TP {} as target for initialization",
                    p.inittp
                );
            }
        }
        "ONEMINUSW" => {
            p.oneminusweights = false;
            say!("--oneminusw: Will output 1-weights (zero=outlier), as in earlier versions!");
        }
        "SEED" => {
            p.seed = i(g, r)? as u32;
            nargs = 1;
            say!("--seed: Will use random seed {}", p.seed);
        }
        "CRAS" => {
            p.crascenter = true;
            say!("--cras: Will center template at avgerage CRAS!");
        }
        "HELP" | "USAGE" | "H" | "U" => {
            print_usage(g, r);
            return Err(Fatal::Exit(0));
        }
        "RES-THRESH" => {
            p.resthresh = f(g, r)? as f32;
            nargs = 1;
            say!(
                "--res-thresh: Voxsize threshold {}",
                ostream(f64::from(p.resthresh))
            );
        }
        "FROBNORM-THRESH" => {
            p.frobnormthresh = f(g, r)?;
            nargs = 1;
            say!(
                "--frobnorm-thresh: Matrix frobenius norm threshold {}",
                ostream(p.frobnormthresh)
            );
        }
        "THREADS" => {
            // omp_set_num_threads(atoi(argv[1])): the port's timepoints run one after the other
            i(g, r)?;
            nargs = 1;
        }
        _ => {
            let _ = r.out.flush();
            let _ = write!(r.err, "\n\nERROR: Option {shown} unknown !!! \n\n");
            print_usage(g, r);
            return Err(Fatal::Exit(1));
        }
    }
    let _ = r.out.flush();
    Ok(nargs)
}

/// One of the checks: its message on standard error and status 1.
fn check_fails(r: &mut Run, msg: &str) -> Fatal {
    let _ = r.out.flush();
    let _ = writeln!(r.err, "{msg}");
    Fatal::Exit(1)
}

/// `parseCommandLine(argc, argv, P)` on the arguments after the program's name.
pub fn parse_command_line(
    g: &mut Globals,
    r: &mut Run,
    p: &mut Options,
    args: &[String],
) -> Result<(), Fatal> {
    if args.is_empty() {
        print_usage(g, r);
        return Err(Fatal::Exit(1));
    }
    let mut at = 0usize;
    while at < args.len() && args[at].starts_with('-') {
        let nargs = parse_next_command(g, r, p, &args[at..])?;
        at += nargs + 1;
    }
    if p.mov.is_empty() {
        return Err(check_fails(
            r,
            "ERROR: Specify at least 2 inputs with --mov (or one 4D input)!",
        ));
    }
    if p.mean.is_empty() {
        return Err(check_fails(
            r,
            "ERROR: Specify output image with --template !",
        ));
    }
    if !p.nwarps.is_empty() && p.mov.len() != p.nwarps.len() {
        return Err(check_fails(
            r,
            "ERROR: Number of filenames for --warp should equal number of inputs!",
        ));
    }
    if !p.mapmovhdr.is_empty() && p.mov.len() != p.mapmovhdr.len() {
        return Err(check_fails(
            r,
            "ERROR: Number of filenames for --mapmovhdr should equal number of inputs!",
        ));
    }
    if !p.nltas.is_empty() && p.mov.len() != p.nltas.len() {
        return Err(check_fails(
            r,
            "ERROR: Number of filenames for --lta should equal number of inputs!",
        ));
    }
    if !p.iltas.is_empty() && p.mov.len() != p.iltas.len() {
        return Err(check_fails(
            r,
            "ERROR: Number of filenames for --ixforms should equal number of inputs!",
        ));
    }
    if !p.nweights.is_empty() && p.mov.len() != p.nweights.len() {
        return Err(check_fails(
            r,
            "ERROR: Number of filenames for --weights should equal number of inputs!",
        ));
    }
    if !p.iscalein.is_empty() && p.iltas.len() != p.iscalein.len() {
        return Err(check_fails(
            r,
            "ERROR: Number of filenames for --iscalein should agree with number of init LTAs (--ixforms)!",
        ));
    }
    if !p.iscaleout.is_empty() && p.mov.len() != p.iscaleout.len() {
        return Err(check_fails(
            r,
            "ERROR: Number of filenames for --iscaleout should equal number of inputs!",
        ));
    }
    if !p.satit && p.sat <= 0.0 && !(p.noit && !p.iltas.is_empty()) {
        return Err(check_fails(
            r,
            "ERROR: Specify either --satit or set sensitivity manually with --sat <real> !",
        ));
    }
    Ok(())
}
