// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (globaloptions.{h,cc}). Copyright (C)
// 1999-2023 University of Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the original. Distributed under the FSL Licence (see LICENSE).
//! FLIRT's command line (`globaloptions`): the defaults, the option scan with FSL's messages and
//! exit statuses, and the usage text.

use larmorx_nc_core::cnum;
use larmorx_nc_core::costfns::CostType;
use larmorx_nc_core::nifti::dt;

/// `interps`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interp {
    TriLinear,
    NearestNeighbour,
    Sinc,
    Spline,
}

/// `windowtype`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    Rect,
    Hanning,
    Blackman,
}

/// `anglereps`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AngleRep {
    Euler,
    Quaternion,
}

/// FLIRT's version string (`const string version = "6.0"`).
pub const VERSION: &str = "6.0";

/// `globaloptions`: what the command line sets (the schedule's matrices and the image pair live
/// in the run).
#[derive(Clone, Debug)]
pub struct Options {
    pub inputfname: String,
    pub outputfname: String,
    pub reffname: String,
    pub outputmatascii: String,
    pub initmatfname: String,
    pub refweightfname: String,
    pub testweightfname: String,
    pub wmsegfname: String,
    pub wmcoordsfname: String,
    pub wmnormsfname: String,
    pub fmapfname: String,
    pub fmapmaskfname: String,
    pub initmatsqform: bool,
    pub printinit: bool,
    pub schedulefname: String,
    pub no_params: i32,
    pub dof: i32,
    pub no_bins: i32,
    pub maincostfn: CostType,
    pub searchcostfn: CostType,
    pub anglerep: AngleRep,
    pub isoscale: f32,
    pub min_sampling: f32,
    pub force_basescale: bool,
    pub basescale: f32,
    pub force_scaling: bool,
    /// `searchrx`, `searchry`, `searchrz`: (min, max) in radians.
    pub searchr: [[f64; 2]; 3],
    pub coarsedelta: f32,
    pub finedelta: f32,
    pub datatype: i16,
    pub forcedatatype: bool,
    pub verbose: i32,
    pub debug: bool,
    pub interactive: bool,
    pub do_optimise: bool,
    pub nosave: bool,
    pub iso: bool,
    pub resample: bool,
    pub useweights: bool,
    pub useseg: bool,
    pub usecoords: bool,
    pub mode2d: bool,
    pub clamping: bool,
    pub forcebackgnd: bool,
    pub backgndval: f32,
    pub interpblur: bool,
    pub interpmethod: Interp,
    pub sincwidth: f32,
    pub sincwindow: Window,
    pub paddingsize: f32,
    pub pe_dir: i32,
    pub echo_spacing: f32,
    pub bbr_type: String,
    pub bbr_slope: f32,
}

impl Default for Options {
    fn default() -> Options {
        let half_pi = std::f64::consts::PI / 2.0;
        Options {
            inputfname: String::new(),
            outputfname: String::new(),
            reffname: String::new(),
            outputmatascii: String::new(),
            initmatfname: String::new(),
            refweightfname: String::new(),
            testweightfname: String::new(),
            wmsegfname: String::new(),
            wmcoordsfname: String::new(),
            wmnormsfname: String::new(),
            fmapfname: String::new(),
            fmapmaskfname: String::new(),
            initmatsqform: false,
            printinit: false,
            schedulefname: String::new(),
            no_params: 12,
            dof: 12,
            no_bins: 256,
            maincostfn: CostType::CorrRatio,
            searchcostfn: CostType::CorrRatio,
            anglerep: AngleRep::Euler,
            isoscale: 1.0,
            min_sampling: 1.0,
            force_basescale: false,
            basescale: 1.0,
            force_scaling: false,
            searchr: [[-half_pi, half_pi]; 3],
            coarsedelta: (60.0 * std::f64::consts::PI / 180.0) as f32,
            finedelta: (18.0 * std::f64::consts::PI / 180.0) as f32,
            datatype: -1,
            forcedatatype: false,
            verbose: 0,
            debug: false,
            interactive: false,
            do_optimise: true,
            nosave: true,
            iso: true,
            resample: true,
            useweights: false,
            useseg: false,
            usecoords: false,
            mode2d: false,
            clamping: true,
            forcebackgnd: false,
            backgndval: 0.0,
            interpblur: true,
            interpmethod: Interp::TriLinear,
            sincwidth: 7.0,
            sincwindow: Window::Hanning,
            paddingsize: 0.0,
            pe_dir: 0,
            echo_spacing: 5e-4,
            bbr_type: "signed".to_owned(),
            bbr_slope: -0.5,
        }
    }
}

/// How the scan ends: a run, or an exit with what it printed on standard output and error.
#[derive(Debug)]
pub enum Parsed {
    Run(Box<Options>),
    Exit {
        code: u8,
        stdout: String,
        stderr: String,
    },
}

/// `exit(-1)`'s status.
pub const EXIT_MINUS_ONE: u8 = 255;

/// `MISCMATHS::Min`/`Max` of the two `atof` values.
fn min_max(a: f64, b: f64) -> (f64, f64) {
    (if a < b { a } else { b }, if a > b { a } else { b })
}

/// `globaloptions::parse_command_line(argc, argv)`; `args` without the program name, which the
/// usage text names `flirt`.
pub fn parse(args: &[String]) -> Parsed {
    let prog = "flirt";
    let mut o = Options::default();
    let exit = |code: u8, stdout: String, stderr: String| Parsed::Exit {
        code,
        stdout,
        stderr,
    };
    if args.is_empty() {
        return exit(1, usage(prog, &o), String::new());
    }
    let argc = args.len() + 1;
    let mut n = 1usize;
    let arg_at = |k: usize| args[k - 1].as_str();
    while n < argc {
        let arg = arg_at(n);
        if arg.is_empty() {
            n += 1;
            continue;
        }
        let first = arg.chars().next().unwrap_or('\0');
        if first != '-' {
            // cerr << "Unrecognised option " << first: the first character only.
            let b = arg.as_bytes()[0];
            let shown = String::from_utf8_lossy(&[b]).into_owned();
            return exit(
                EXIT_MINUS_ONE,
                String::new(),
                format!("Unrecognised option {shown}\n"),
            );
        }
        // Options without arguments.
        match arg {
            "-help" => return exit(0, usage(prog, &o), String::new()),
            "-version" => return exit(0, format!("FLIRT version {VERSION}\n"), String::new()),
            "-applyxfm" | "-applynonisoxfm" => {
                o.do_optimise = false;
                o.iso = false;
                o.nosave = false;
                n += 1;
                continue;
            }
            "-i" => {
                o.interactive = true;
                n += 1;
                continue;
            }
            "-nosearch" => {
                o.searchr = [[0.0, 0.0]; 3];
                n += 1;
                continue;
            }
            "-nosave" => {
                o.nosave = true;
                n += 1;
                continue;
            }
            "-noresample" => {
                o.resample = false;
                n += 1;
                continue;
            }
            "-forcescaling" => {
                o.force_scaling = true;
                n += 1;
                continue;
            }
            "-2D" => {
                o.mode2d = true;
                n += 1;
                continue;
            }
            "-noclamp" => {
                o.clamping = false;
                n += 1;
                continue;
            }
            "-noresampblur" => {
                o.interpblur = false;
                n += 1;
                continue;
            }
            "-usesqform" => {
                o.initmatsqform = true;
                n += 1;
                continue;
            }
            "-printinit" => {
                o.printinit = true;
                n += 1;
                continue;
            }
            "-debugsave" => {
                o.nosave = false;
                n += 1;
                continue;
            }
            "-debug" => {
                o.debug = true;
                n += 1;
                continue;
            }
            "-v" => {
                o.verbose = 1;
                n += 1;
                continue;
            }
            _ => {}
        }
        if n + 1 >= argc {
            return exit(
                EXIT_MINUS_ONE,
                String::new(),
                format!("Lacking argument to option {arg}\n"),
            );
        }
        let v = arg_at(n + 1);
        let mut one = true;
        match arg {
            "-o" | "-out" => o.outputfname = v.to_owned(),
            "-ref" => o.reffname = v.to_owned(),
            "-in" => o.inputfname = v.to_owned(),
            "-init" => {
                o.initmatfname = v.to_owned();
                o.initmatsqform = false;
            }
            "-schedule" => o.schedulefname = v.to_owned(),
            "-refweight" => {
                o.refweightfname = v.to_owned();
                o.useweights = true;
            }
            "-inweight" => {
                o.testweightfname = v.to_owned();
                o.useweights = true;
            }
            "-wmseg" => {
                o.wmsegfname = v.to_owned();
                o.useseg = true;
            }
            "-fieldmap" => o.fmapfname = v.to_owned(),
            "-fieldmapmask" => o.fmapmaskfname = v.to_owned(),
            "-wmcoords" => {
                o.wmcoordsfname = v.to_owned();
                o.useseg = true;
                o.usecoords = true;
            }
            "-wmnorms" => {
                o.wmnormsfname = v.to_owned();
                o.useseg = true;
                o.usecoords = true;
            }
            "-omat" => o.outputmatascii = v.to_owned(),
            "-bins" => o.no_bins = cnum::atoi(v),
            "-dof" => {
                o.no_params = cnum::atoi(v);
                o.dof = cnum::atoi(v);
            }
            "-sincwidth" => o.sincwidth = cnum::atoi(v) as f32,
            "-paddingsize" => o.paddingsize = cnum::atof(v) as f32,
            "-applyisoxfm" => {
                o.isoscale = cnum::atof(v) as f32;
                o.do_optimise = false;
                o.iso = true;
                o.nosave = false;
            }
            "-basescale" => {
                o.force_basescale = true;
                o.basescale = cnum::atof(v) as f32;
            }
            "-minsampling" => o.min_sampling = cnum::atof(v) as f32,
            "-setbackground" => {
                o.backgndval = cnum::atof(v) as f32;
                o.forcebackgnd = true;
            }
            "-coarsesearch" => {
                o.coarsedelta = (cnum::atof(v) * std::f64::consts::PI / 180.0) as f32;
            }
            "-finesearch" => {
                o.finedelta = (cnum::atof(v) * std::f64::consts::PI / 180.0) as f32;
            }
            "-echospacing" => o.echo_spacing = cnum::atof(v) as f32,
            "-pedir" => {
                o.pe_dir = cnum::atoi(v);
                if o.pe_dir == 0 {
                    return exit(
                        EXIT_MINUS_ONE,
                        String::new(),
                        format!(
                            "Unrecognised argument to pedir ({v}) - it should be a number between -3 and 3 (not 0)\n"
                        ),
                    );
                }
            }
            "-bbrtype" => o.bbr_type = v.to_owned(),
            "-bbrslope" => o.bbr_slope = cnum::atof(v) as f32,
            "-verbose" => o.verbose = cnum::atoi(v),
            "-datatype" => {
                o.forcedatatype = true;
                o.datatype = match v {
                    "double" => dt::DOUBLE,
                    "float" => dt::FLOAT,
                    "int" => dt::SIGNED_INT,
                    "short" => dt::SIGNED_SHORT,
                    "char" => dt::UNSIGNED_CHAR,
                    _ => {
                        return exit(
                            EXIT_MINUS_ONE,
                            String::new(),
                            format!("Unrecognised data type: {v}\n"),
                        );
                    }
                };
            }
            "-cost" | "-searchcost" => match CostType::from_name(v) {
                Some(c) => {
                    if arg == "-cost" {
                        o.maincostfn = c;
                    } else {
                        o.searchcostfn = c;
                    }
                }
                None => {
                    return exit(
                        EXIT_MINUS_ONE,
                        String::new(),
                        format!("Unrecognised cost function type: {v}\n"),
                    );
                }
            },
            "-interp" => {
                o.interpmethod = match v {
                    "trilinear" => Interp::TriLinear,
                    "nearestneighbour" => Interp::NearestNeighbour,
                    "sinc" => Interp::Sinc,
                    "spline" => Interp::Spline,
                    _ => {
                        return exit(
                            EXIT_MINUS_ONE,
                            String::new(),
                            format!("Unrecognised interpolation method: {v}\n"),
                        );
                    }
                };
            }
            "-sincwindow" => {
                o.sincwindow = match v {
                    "rectangular" => Window::Rect,
                    "hanning" => Window::Hanning,
                    "blackman" => Window::Blackman,
                    _ => {
                        return exit(
                            EXIT_MINUS_ONE,
                            String::new(),
                            format!("Unrecognised sinc window: {v}\n"),
                        );
                    }
                };
            }
            "-anglerep" => {
                o.anglerep = match v {
                    "quaternion" => AngleRep::Quaternion,
                    "euler" => AngleRep::Euler,
                    _ => {
                        return exit(
                            EXIT_MINUS_ONE,
                            String::new(),
                            format!("Unrecognised angle representation: {v}\n"),
                        );
                    }
                };
            }
            _ => one = false,
        }
        if one {
            n += 2;
            continue;
        }
        if n + 2 >= argc {
            return exit(
                EXIT_MINUS_ONE,
                String::new(),
                format!("Lacking argument to option {arg}\n"),
            );
        }
        let axis = match arg {
            "-searchrx" => 0,
            "-searchry" => 1,
            "-searchrz" => 2,
            _ => {
                return exit(
                    EXIT_MINUS_ONE,
                    String::new(),
                    format!("Unrecognised option {arg}\n"),
                );
            }
        };
        let (lo, hi) = min_max(cnum::atof(arg_at(n + 1)), cnum::atof(arg_at(n + 2)));
        o.searchr[axis] = [
            lo * std::f64::consts::PI / 180.0,
            hi * std::f64::consts::PI / 180.0,
        ];
        n += 3;
    }
    if o.inputfname.is_empty() {
        return exit(
            2,
            usage(prog, &o),
            "ERROR:: Input volume filename not found\n\n".to_owned(),
        );
    }
    if o.reffname.is_empty() {
        return exit(
            2,
            usage(prog, &o),
            "ERROR:: Reference volume filename not found\n\n".to_owned(),
        );
    }
    Parsed::Run(Box::new(o))
}

/// `print_usage(argc, argv)` (with `print_version` first and a blank line); the bins and dof
/// defaults are the options' values when it is printed.
pub fn usage(prog: &str, o: &Options) -> String {
    let mut s = format!("FLIRT version {VERSION}\n\n");
    s.push_str(&format!(
        "Usage: {prog} [options] -in <inputvol> -ref <refvol> -out <outputvol>\n"
    ));
    s.push_str(&format!(
        "       {prog} [options] -in <inputvol> -ref <refvol> -omat <outputmatrix>\n"
    ));
    s.push_str(&format!(
        "       {prog} [options] -in <inputvol> -ref <refvol> -applyxfm -init <matrix> -out <outputvol>\n\n"
    ));
    s.push_str(USAGE_HEAD);
    s.push_str(&format!(
        "        -bins <number of histogram bins>   (default is {})\n",
        o.no_bins
    ));
    s.push_str(&format!(
        "        -dof  <number of transform dofs>   (default is {})\n",
        o.dof
    ));
    s.push_str(USAGE_TAIL);
    s
}

const USAGE_HEAD: &str = "  Available options are:
        -in  <inputvol>                    (no default)
        -ref <refvol>                      (no default)
        -init <matrix-filname>             (input 4x4 affine matrix)
        -omat <matrix-filename>            (output in 4x4 ascii format)
        -out, -o <outputvol>               (default is none)
        -datatype {char,short,int,float,double}                    (force output data type)
        -cost {mutualinfo,corratio,normcorr,normmi,leastsq,labeldiff,bbr}        (default is corratio)
        -searchcost {mutualinfo,corratio,normcorr,normmi,leastsq,labeldiff,bbr}  (default is corratio)
        -usesqform                         (initialise using appropriate sform or qform)
        -displayinit                       (display initial matrix)
        -anglerep {quaternion,euler}       (default is euler)
        -interp {trilinear,nearestneighbour,sinc,spline}  (final interpolation: def - trilinear)
        -sincwidth <full-width in voxels>  (default is 7)
        -sincwindow {rectangular,hanning,blackman}
";

const USAGE_TAIL: &str = "        -noresample                        (do not change input sampling)
        -forcescaling                      (force rescaling even for low-res images)
        -minsampling <vox_dim>             (set minimum voxel dimension for sampling (in mm))
        -applyxfm                          (applies transform (no optimisation) - requires -init)
        -applyisoxfm <scale>               (as applyxfm but forces isotropic resampling)
        -paddingsize <number of voxels>    (for applyxfm: interpolates outside image by size)
        -searchrx <min_angle> <max_angle>  (angles in degrees: default is -90 90)
        -searchry <min_angle> <max_angle>  (angles in degrees: default is -90 90)
        -searchrz <min_angle> <max_angle>  (angles in degrees: default is -90 90)
        -nosearch                          (sets all angular search ranges to 0 0)
        -coarsesearch <delta_angle>        (angle in degrees: default is 60)
        -finesearch <delta_angle>          (angle in degrees: default is 18)
        -schedule <schedule-file>          (replaces default schedule)
        -refweight <volume>                (use weights for reference volume)
        -inweight <volume>                 (use weights for input volume)
        -wmseg <volume>                    (white matter segmentation volume needed by BBR cost function)
        -wmcoords <text matrix>            (white matter boundary coordinates for BBR cost function)
        -wmnorms <text matrix>             (white matter boundary normals for BBR cost function)
        -fieldmap <volume>                 (fieldmap image in rads/s - must be already registered to the reference image)
        -fieldmapmask <volume>             (mask for fieldmap image)
        -pedir <index>                     (phase encode direction of EPI - 1/2/3=x/y/z & -1/-2/-3=-x/-y/-z)
        -echospacing <value>               (value of EPI echo spacing - units of seconds)
        -bbrtype <value>                   (type of bbr cost function: signed [default], global_abs, local_abs)
        -bbrslope <value>                  (value of bbr slope)
        -setbackground <value>             (use specified background value for points outside FOV)
        -noclamp                           (do not use intensity clamping)
        -noresampblur                      (do not use blurring on downsampling)
        -2D                                (use 2D rigid body mode - ignores dof)
        -verbose <num>                     (0 is least and default)
        -v                                 (same as -verbose 1)
        -i                                 (pauses at each stage: default is off)
        -version                           (prints version number)
        -help
";
