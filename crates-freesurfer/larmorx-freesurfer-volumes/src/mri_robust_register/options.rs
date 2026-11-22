// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_register`'s command line (`mri_robust_register.cpp` 71-142 `Parameters` and their
//! defaults, 1424-1427 `printUsage`, 1838-2287 `parseNextCommand`, 2298-2370 `parseCommandLine`).
//!
//! The options are read while the next argument starts with `-`: one dash or two are dropped and
//! the rest compared in upper case (`StrUpper`, which also changes the argument printed by the
//! unknown-option error). Each option prints its line on standard output; an option that takes an
//! argument takes the next one whatever it is. Then the checks, each after the help (but the
//! last): `--mov` and `--dst`, `--lta` (unless `--iscaleonly`), `--satit` or `--sat` (or another
//! cost, or `--leastsquares`), `--iscale` with `--iscaleout` (which sets it: never failing), a
//! `--mapmov` name that is not the `--weights` name, `--maxit` above 0. No argument at all: the
//! help, status 1. `--leastsquares` changes nothing but the check (FreeSurfer's `setRobust` is
//! commented out); `--satest` and `--conform` set fields nothing reads.
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-robust-register.md`):
//! - **FS-1070** (crash): an option that takes an argument given last: FreeSurfer builds its
//!   string from `argv[argc]` (`NULL`: libstdc++ throws `std::logic_error`, the program aborts
//!   with `terminate called after throwing an instance of 'std::logic_error'` and status 134) or
//!   passes it to `atoi` or `atof` (a segmentation fault). By default an error naming the option
//!   (status 1); `reproduce` prints libstdc++'s two lines and ends with 134 where FreeSurfer
//!   aborts (the same error where it crashes).

use std::io::Write;

use larmorx_core::cfmt::ostream;
use larmorx_core::defects::FS_1070;
use larmorx_freesurfer_core::cstd::{atof, atoi};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_robust::regstep::Cost;

/// `mri_robust_register.help.xml`, the text `printUsage` renders (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("mri_robust_register.help.xml");

/// `SAMPLE_CUBIC_BSPLINE` (`include/mri.h`): the outputs' resampling.
pub const SAMPLE_CUBIC_BSPLINE: i32 = 5;

/// The program's streams, environment and working directory.
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'b Environment,
    pub dir: Option<&'b std::path::Path>,
}

/// The cost functions `--cost` names: `ROB` (also `ROBENT`, with entropy images) runs the robust
/// registration; the others FreeSurfer's Powell registration (`RegPowell`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CostName {
    Ls,
    Rob,
    Mi,
    Nmi,
    Ecc,
    Ncc,
    Scr,
    Tb,
    Lncc,
    Sad,
    Sb,
}

/// `Parameters` with `P`'s defaults (the images are kept by [`super::main`]).
#[derive(Clone, Debug)]
pub struct Options {
    pub mov: String,
    pub dst: String,
    pub lta: String,
    pub maskmov: String,
    pub maskdst: String,
    pub halfmov: String,
    pub halfdst: String,
    pub halfweights: String,
    pub halfmovlta: String,
    pub halfdstlta: String,
    pub weightsout: String,
    pub satit: bool,
    pub nomulti: bool,
    pub conform: bool,
    pub floattype: bool,
    pub lta_vox2vox: bool,
    pub affine: bool,
    pub iscale: bool,
    pub iscaleonly: bool,
    pub transonly: bool,
    pub isoscale: bool,
    pub transform: String,
    pub leastsquares: bool,
    pub iterate: i32,
    pub epsit: f64,
    pub sat: f64,
    pub warpout: String,
    pub norlout: String,
    pub subsamplesize: i32,
    pub debug: i32,
    pub dosatest: bool,
    pub initorient: bool,
    pub inittrans: bool,
    pub initscaling: bool,
    pub verbose: i32,
    pub highit: i32,
    pub doubleprec: bool,
    pub wlimit: f64,
    pub oneminusweights: bool,
    pub symmetry: bool,
    pub iscaleout: String,
    pub iscalein: String,
    pub minsize: i32,
    pub maxsize: i32,
    pub cost: CostName,
    pub finalsampletype: i32,
    pub entropy: bool,
    pub entroradius: i32,
    pub entmov: String,
    pub entdst: String,
    pub entball: bool,
    pub entcorrection: bool,
    pub sobel: bool,
    pub powelltol: f64,
    pub whitebgmov: bool,
    pub whitebgdst: bool,
    pub uchartype: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mov: String::new(),
            dst: String::new(),
            lta: String::new(),
            maskmov: String::new(),
            maskdst: String::new(),
            halfmov: String::new(),
            halfdst: String::new(),
            halfweights: String::new(),
            halfmovlta: String::new(),
            halfdstlta: String::new(),
            weightsout: String::new(),
            satit: false,
            nomulti: false,
            conform: false,
            floattype: false,
            lta_vox2vox: false,
            affine: false,
            iscale: false,
            iscaleonly: false,
            transonly: false,
            isoscale: false,
            transform: String::new(),
            leastsquares: false,
            iterate: 5,
            epsit: 0.01,
            sat: -1.0,
            warpout: String::new(),
            norlout: String::new(),
            subsamplesize: -1,
            debug: 0,
            dosatest: false,
            initorient: false,
            inittrans: true,
            initscaling: false,
            verbose: 1,
            highit: -1,
            doubleprec: false,
            wlimit: 0.16,
            oneminusweights: true,
            symmetry: true,
            iscaleout: String::new(),
            iscalein: String::new(),
            minsize: -1,
            maxsize: -1,
            cost: CostName::Rob,
            finalsampletype: SAMPLE_CUBIC_BSPLINE,
            entropy: false,
            entroradius: 5,
            entmov: String::new(),
            entdst: String::new(),
            entball: false,
            entcorrection: false,
            sobel: false,
            powelltol: 1e-5,
            whitebgmov: false,
            whitebgdst: false,
            uchartype: false,
        }
    }
}

impl Options {
    /// `P.cost == Registration::ROB`.
    pub fn robust(&self) -> bool {
        self.cost == CostName::Rob
    }

    /// The library's cost of the robust registration.
    pub fn library_cost(&self) -> Cost {
        Cost::Rob
    }
}

/// `printUsage()`: the help of `mri_robust_register.help.xml`.
pub fn print_usage(g: &mut Globals, r: &mut Run) {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
}

/// libstdc++'s lines when `std::string(NULL)` throws and nobody catches it (GCC 8.4's
/// `basic_string`, inlined into the official build).
const NULL_STRING_ABORT: &str = "terminate called after throwing an instance of 'std::logic_error'\n  what():  basic_string::_M_construct null not valid\n";

/// FS-1070: an option's argument past the last one. `string` tells whether FreeSurfer builds a
/// `std::string` of it (an abort) or reads a number (a segmentation fault).
fn missing(g: &mut Globals, r: &mut Run, arg: &str, string: bool) -> Fatal {
    g.notices.add(
        &FS_1070,
        format!(
            "{arg}: no argument after the option (FreeSurfer reads past its arguments and crashes)"
        ),
    );
    if string && g.notices.reproduce() {
        let _ = r.out.flush();
        let _ = r.err.write_all(NULL_STRING_ABORT.as_bytes());
        return Fatal::Exit(134);
    }
    let _ = writeln!(
        r.err,
        "mri_robust_register: option {arg} needs an argument (FreeSurfer crashes here)"
    );
    Fatal::Exit(1)
}

/// `parseNextCommand(argc, argv, P)` on `args[0]` (the option) and what follows it: the arguments
/// it takes after its own. `Err` ends the program (the help, an unknown option or cost, `--test`).
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
    // the option's argument as a string or a number (FS-1070 when there is none)
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
        "MOV" | "M" => {
            p.mov = s(g, r)?;
            nargs = 1;
            say!("--mov: Using {} as movable/source volume.", p.mov);
        }
        "DST" | "D" => {
            p.dst = s(g, r)?;
            nargs = 1;
            say!("--dst: Using {} as target volume.", p.dst);
        }
        "LTA" => {
            p.lta = s(g, r)?;
            nargs = 1;
            say!("--lta: Output transform as {} . ", p.lta);
        }
        "VOX2VOX" => {
            p.lta_vox2vox = true;
            say!("--vox2vox: Output transform as VOX2VOX. ");
        }
        "AFFINE" | "A" => {
            p.affine = true;
            say!("--affine: Enabling affine transform!");
        }
        "ISOSCALE" => {
            p.isoscale = true;
            say!("--isoscale: Enabling isotropic scaling!");
        }
        "INITSCALING" => {
            p.initscaling = true;
            say!("--initscaling: Enabling initial scale adjustment based on image dimensions!");
        }
        "ISCALE" | "I" => {
            p.iscale = true;
            say!("--iscale: Enabling intensity scaling!");
        }
        "ISCALEONLY" => {
            p.iscaleonly = true;
            p.iscale = true;
            p.affine = false;
            p.isoscale = false;
            p.initscaling = false;
            p.transonly = false;
            p.initorient = false;
            p.inittrans = false;
            say!("--iscaleonly: No geometric transform, only iscale!");
        }
        "TRANSONLY" => {
            p.transonly = true;
            say!("--transonly: Using only translation!");
        }
        "TRANSFORM" | "IXFORM" => {
            p.transform = s(g, r)?;
            nargs = 1;
            say!(
                "--ixform: Using previously computed initial transform: {}",
                p.transform
            );
        }
        "INITORIENT" => {
            p.initorient = true;
            say!("--initorient: Using moments for initial orientation!");
        }
        "NOINIT" => {
            p.inittrans = false;
            say!("--noinit: Skipping init of transform !");
        }
        "LEASTSQUARES" => {
            p.leastsquares = true;
            say!("--leastsquares: Using standard least squares (non-robust)!");
        }
        "COST" => {
            let cost = s(g, r)?.to_ascii_uppercase();
            nargs = 1;
            p.cost = match cost.as_str() {
                "LS" => CostName::Ls,
                "ROB" => CostName::Rob,
                "ROBENT" => {
                    p.entropy = true;
                    CostName::Rob
                }
                "MI" => CostName::Mi,
                "NMI" => CostName::Nmi,
                "ECC" => CostName::Ecc,
                "NCC" => CostName::Ncc,
                "SCR" => CostName::Scr,
                "TB" => CostName::Tb,
                "LNCC" => CostName::Lncc,
                "SAD" => CostName::Sad,
                "SB" => CostName::Sb,
                _ => {
                    say!("ERROR: cost function {cost} unknown! ");
                    return Err(Fatal::Exit(1));
                }
            };
            say!("--cost: Using cost function: {cost} !");
        }
        "ENTRADIUS" | "RADIUS" => {
            p.entroradius = i(g, r)?;
            nargs = 1;
            say!(
                "--entradius: Using local boxes with radius {} = {} sides. ",
                p.entroradius,
                p.entroradius.wrapping_mul(2).wrapping_add(1)
            );
        }
        "ENTDST" => {
            p.entdst = s(g, r)?;
            nargs = 1;
            say!("--entdst: Output entropy dst image as {}", p.entdst);
        }
        "ENTBALL" => {
            p.entball = true;
            say!("--entball: Using ball instead of local boxes. ");
        }
        "ENTCORRECTION" => {
            p.entcorrection = true;
            say!("--entcorrection: Using 'correction' mode for entropy images. ");
        }
        "ENTMOV" => {
            p.entmov = s(g, r)?;
            nargs = 1;
            say!("--entmov: Output entropy mov image as {}", p.entmov);
        }
        "SOBEL" => {
            p.sobel = true;
            say!("--sobel: Using Sobel magnitude images.");
        }
        "MAXIT" => {
            p.iterate = i(g, r)?;
            nargs = 1;
            say!(
                "--maxit: Performing maximal {} iterations on each resolution",
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
        "NOMULTI" => {
            p.nomulti = true;
            say!("--nomulti: Will work on highest resolution only (nomulti)!");
        }
        "MAXSIZE" => {
            p.maxsize = i(g, r)?;
            nargs = 1;
            say!("--maxsize: Largest dimension < {} . ", p.maxsize);
        }
        "MINSIZE" => {
            p.minsize = i(g, r)?;
            nargs = 1;
            say!("--minsize: Smallest dimension > {} . ", p.minsize);
        }
        "SAT" => {
            p.sat = f(g, r)?;
            nargs = 1;
            say!("--sat: Using saturation {} in M-estimator!", ostream(p.sat));
        }
        "WLIMIT" => {
            p.wlimit = f(g, r)?;
            nargs = 1;
            say!("--wlimit: Using wlimit in satit {}", ostream(p.wlimit));
        }
        "SUBSAMPLE" => {
            let Some(v) = next else {
                log::error_exit(
                    g,
                    r.err,
                    ERROR_BADPARM,
                    "\nERROR: specify min side lenght to start subsampling, e.g. --subsample 200.\n",
                )?;
                return Ok(0);
            };
            p.subsamplesize = atoi(v.as_bytes());
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
        "SATIT" => {
            p.satit = true;
            say!("--satit: Will iterate with different SAT to ensure outliers below wlimit!");
        }
        "SATEST" => {
            p.dosatest = true;
            say!("--satest: Trying to estimate SAT value!");
        }
        "POWELLTOL" => {
            p.powelltol = f(g, r)?;
            nargs = 1;
            say!(
                "--powelltol: Using tolerance {} in Powell optimizier!",
                ostream(p.powelltol)
            );
        }
        "DOUBLEPREC" => {
            p.doubleprec = true;
            say!("--doubleprec: Will perform algorithm with double precision (higher mem usage)!");
        }
        "DEBUG" => {
            p.debug = 1;
            say!("--debug: Will output debug info and files!");
        }
        "VERBOSE" => {
            p.verbose = i(g, r)?;
            nargs = 1;
            say!("--verbose: Will use verbose level : {}", p.verbose);
        }
        "WEIGHTS" => {
            p.weightsout = s(g, r)?;
            nargs = 1;
            say!(
                "--weights: Will output weights (in target space) as {} !",
                p.weightsout
            );
        }
        "WARP" | "MAPMOV" => {
            p.warpout = s(g, r)?;
            nargs = 1;
            say!("--mapmov: Will save resampled movable as {} !", p.warpout);
        }
        "MAPMOVHDR" => {
            p.norlout = s(g, r)?;
            nargs = 1;
            say!(
                "--mapmovhdr: Will save header adjusted movable as {} !",
                p.norlout
            );
        }
        "HALFMOV" => {
            p.halfmov = s(g, r)?;
            nargs = 1;
            say!("--halfmov: Will output final half way MOV !");
        }
        "HALFDST" => {
            p.halfdst = s(g, r)?;
            nargs = 1;
            say!("--halfdst: Will output final half way DST !");
        }
        "HALFWEIGHTS" => {
            p.halfweights = s(g, r)?;
            nargs = 1;
            say!(
                "--halfweights: Will output half way WEIGHTS from last step to {} !",
                p.halfweights
            );
        }
        "HALFMOVLTA" => {
            p.halfmovlta = s(g, r)?;
            nargs = 1;
            say!(
                "--halfmovlta: Will output half way transform (mov) {} !",
                p.halfmovlta
            );
        }
        "HALFDSTLTA" => {
            p.halfdstlta = s(g, r)?;
            nargs = 1;
            say!(
                "--halfdstlta: Will output half way transform (dst) {} !",
                p.halfdstlta
            );
        }
        "MASKMOV" => {
            p.maskmov = s(g, r)?;
            nargs = 1;
            say!("--maskmov: Will apply {} to mask mov/src !", p.maskmov);
        }
        "MASKDST" => {
            p.maskdst = s(g, r)?;
            nargs = 1;
            say!("--maskdst: Will apply {} to mask dst/target !", p.maskdst);
        }
        "TEST" => {
            say!("--test: TEST-MODE ");
            let _ = writeln!(
                r.err,
                "mri_robust_register: the test mode (--test, RegRobust::testRobust) is not ported yet"
            );
            return Err(Fatal::Exit(1));
        }
        "CONFORM" => {
            p.conform = true;
            say!("--conform: Will conform images to 256^3 and voxels to 1mm!");
        }
        "FLOATTYPE" => {
            p.floattype = true;
            say!("--floattype: Use float images internally (independent of input)!");
        }
        "ONEMINUSW" => {
            p.oneminusweights = false;
            say!("--oneminusw: Will output 1-weights (zero=outlier), as in earlier versions!");
        }
        "NOSYM" => {
            p.symmetry = false;
            say!("--nosym: Will resample source to target (no half-way space)!");
        }
        "WHITEBGMOV" => {
            p.whitebgmov = true;
            say!("--whitebgmov: Will assume white background in MOV!");
        }
        "WHITEBGDST" => {
            p.whitebgdst = true;
            say!("--whitebgdst: Will assume white background in DST!");
        }
        "UCHAR" => {
            p.uchartype = true;
            say!("--uchar: Will convert images to uchar (with re-scaling and histogram cropping)!");
        }
        "ISCALEOUT" => {
            p.iscaleout = s(g, r)?;
            nargs = 1;
            p.iscale = true;
            say!(
                "--iscaleout: Will do --iscale and ouput intensity scale to {}",
                p.iscaleout
            );
        }
        "ISCALEIN" => {
            nargs = 1;
            p.iscalein = s(g, r)?;
            say!("--iscalein: Will use init intensity scale");
        }
        "HELP" | "H" => {
            print_usage(g, r);
            return Err(Fatal::Exit(1));
        }
        _ => {
            let _ = r.out.flush();
            let _ = write!(r.err, "\n\nERROR: Option: {shown} unknown !! \n\n");
            return Err(Fatal::Exit(1));
        }
    }
    let _ = r.out.flush();
    Ok(nargs)
}

/// The check `parseCommandLine` fails: the help (but for `--maxit`), then the error between two
/// empty lines on standard error, status 1.
fn check_fails(g: &mut Globals, r: &mut Run, help: bool, msg: &str) -> Fatal {
    if help {
        print_usage(g, r);
    }
    let _ = r.out.flush();
    let _ = write!(r.err, "\n\nERROR: {msg}\n\n");
    Fatal::Exit(1)
}

/// `parseCommandLine(argc, argv, P)` on the arguments after the program's name.
pub fn parse_command_line(
    g: &mut Globals,
    r: &mut Run,
    p: &mut Options,
    args: &[String],
) -> Result<(), Fatal> {
    let mut at = 0usize;
    while at < args.len() && args[at].starts_with('-') {
        let nargs = parse_next_command(g, r, p, &args[at..])?;
        at += nargs + 1;
    }
    if args.is_empty() {
        print_usage(g, r);
        return Err(Fatal::Exit(1));
    }
    if p.mov.is_empty() || p.dst.is_empty() {
        return Err(check_fails(
            g,
            r,
            true,
            "Please specify --mov and --dst !  ",
        ));
    }
    if p.lta.is_empty() && !p.iscaleonly {
        return Err(check_fails(g, r, true, "Please specify --lta !  "));
    }
    if !(p.satit || p.sat > 0.0 || !p.robust() || p.leastsquares) {
        return Err(check_fails(
            g,
            r,
            true,
            "Please specify either --satit or --sat <float> !  ",
        ));
    }
    if !(p.iscaleout.is_empty() || p.iscale) {
        return Err(check_fails(
            g,
            r,
            true,
            "Please specify --iscale together with --iscaleout to compute and output global intensity scaling! ",
        ));
    }
    if !(p.warpout.is_empty() || p.warpout != p.weightsout) {
        return Err(check_fails(
            g,
            r,
            true,
            "Resampled input name (--mapmov) cannot be same as --weights output!",
        ));
    }
    if p.iterate <= 0 {
        return Err(check_fails(
            g,
            r,
            false,
            "--maxit must be larger than 0 (max iterations on each resolution)",
        ));
    }
    Ok(())
}
