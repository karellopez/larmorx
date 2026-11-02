// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl flirt`: FSL's `flirt` command line (`specs/flirt.md` §2).

use std::io::Write;

use super::Settings;
use crate::flirt::engine::{self, Ctx};
use crate::flirt::mat::{atof, atoi};
use crate::flirt::{Cost, Interp, Options, OutType, SincWindow};

/// The usage text (§2.4), with the current `-bins` and `-dof`.
pub fn usage(o: &Options) -> String {
    format!(
        "FLIRT version 6.0

Usage: flirt [options] -in <inputvol> -ref <refvol> -out <outputvol>
       flirt [options] -in <inputvol> -ref <refvol> -omat <outputmatrix>
       flirt [options] -in <inputvol> -ref <refvol> -applyxfm -init <matrix> -out <outputvol>

  Available options are:
        -in  <inputvol>                    (no default)
        -ref <refvol>                      (no default)
        -init <matrix-filname>             (input 4x4 affine matrix)
        -omat <matrix-filename>            (output in 4x4 ascii format)
        -out, -o <outputvol>               (default is none)
        -datatype {{char,short,int,float,double}}                    (force output data type)
        -cost {{mutualinfo,corratio,normcorr,normmi,leastsq,labeldiff,bbr}}        (default is corratio)
        -searchcost {{mutualinfo,corratio,normcorr,normmi,leastsq,labeldiff,bbr}}  (default is corratio)
        -usesqform                         (initialise using appropriate sform or qform)
        -displayinit                       (display initial matrix)
        -anglerep {{quaternion,euler}}       (default is euler)
        -interp {{trilinear,nearestneighbour,sinc,spline}}  (final interpolation: def - trilinear)
        -sincwidth <full-width in voxels>  (default is 7)
        -sincwindow {{rectangular,hanning,blackman}}
        -bins <number of histogram bins>   (default is {bins})
        -dof  <number of transform dofs>   (default is {dof})
        -noresample                        (do not change input sampling)
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
",
        bins = o.bins,
        dof = o.dof
    )
}

/// What the scan of the command line decided.
pub enum Parsed {
    Run(Box<Options>),
    /// Exit with this status (the messages printed).
    Exit(u8),
}

fn degrees_f32(w: &str) -> f32 {
    (atof(w) * std::f64::consts::PI / 180.0) as f32
}

/// The scan of the options (§2, §2.1).
pub fn parse(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Parsed {
    let mut o = Options::default();
    if args.is_empty() {
        let _ = write!(out, "{}", usage(&o));
        return Parsed::Exit(1);
    }
    let no_arg = [
        "-help",
        "-version",
        "-applyxfm",
        "-applynonisoxfm",
        "-i",
        "-nosearch",
        "-nosave",
        "-noresample",
        "-forcescaling",
        "-2D",
        "-noclamp",
        "-noresampblur",
        "-usesqform",
        "-printinit",
        "-displayinit",
        "-debugsave",
        "-debug",
        "-v",
    ];
    let one_arg = [
        "-o",
        "-out",
        "-ref",
        "-in",
        "-init",
        "-schedule",
        "-refweight",
        "-inweight",
        "-wmseg",
        "-fieldmap",
        "-fieldmapmask",
        "-wmcoords",
        "-wmnorms",
        "-omat",
        "-bins",
        "-dof",
        "-sincwidth",
        "-paddingsize",
        "-applyisoxfm",
        "-basescale",
        "-minsampling",
        "-setbackground",
        "-coarsesearch",
        "-finesearch",
        "-echospacing",
        "-pedir",
        "-bbrtype",
        "-bbrslope",
        "-verbose",
        "-datatype",
        "-cost",
        "-searchcost",
        "-interp",
        "-sincwindow",
        "-anglerep",
    ];
    let two_arg = ["-searchrx", "-searchry", "-searchrz"];
    let mut i = 0;
    while i < args.len() {
        let w = args[i].as_str();
        if w.is_empty() {
            i += 1;
            continue;
        }
        if !w.starts_with('-') {
            let _ = writeln!(
                err,
                "Unrecognised option {}",
                w.chars().next().unwrap_or(' ')
            );
            return Parsed::Exit(255);
        }
        let rest = args.len() - i - 1;
        if no_arg.contains(&w) {
            match w {
                "-help" => {
                    let _ = write!(out, "{}", usage(&o));
                    return Parsed::Exit(0);
                }
                "-version" => {
                    let _ = writeln!(out, "FLIRT version 6.0");
                    return Parsed::Exit(0);
                }
                "-applyxfm" | "-applynonisoxfm" => o.apply_xfm = true,
                "-nosearch" => o.search = [[0.0, 0.0]; 3],
                "-noresample" => o.no_resample = true,
                "-forcescaling" => o.force_scaling = true,
                "-2D" => o.two_d = true,
                "-noclamp" => o.no_clamp = true,
                "-noresampblur" => o.no_resample_blur = true,
                "-usesqform" => o.use_sqform = true,
                "-printinit" | "-displayinit" => o.print_init = true,
                "-debugsave" => o.debug_save = true,
                "-debug" => o.debug = true,
                "-v" => o.verbose = 1,
                _ => {}
            }
            i += 1;
            continue;
        }
        if two_arg.contains(&w) {
            if rest < 2 {
                let _ = writeln!(err, "Lacking argument to option {w}");
                return Parsed::Exit(255);
            }
            let (a, b) = (atof(&args[i + 1]), atof(&args[i + 2]));
            let r = [a.min(b), a.max(b)].map(|d| d * std::f64::consts::PI / 180.0);
            let k = match w {
                "-searchrx" => 0,
                "-searchry" => 1,
                _ => 2,
            };
            o.search[k] = r;
            i += 3;
            continue;
        }
        if !one_arg.contains(&w) {
            if rest < 2 {
                let _ = writeln!(err, "Lacking argument to option {w}");
            } else {
                let _ = writeln!(err, "Unrecognised option {w}");
            }
            return Parsed::Exit(255);
        }
        if rest < 1 {
            let _ = writeln!(err, "Lacking argument to option {w}");
            return Parsed::Exit(255);
        }
        let v = args[i + 1].clone();
        let bad = |what: &str, err: &mut dyn Write| {
            let _ = writeln!(err, "{what}: {v}");
            Parsed::Exit(255)
        };
        match w {
            "-o" | "-out" => o.out = Some(v.clone()),
            "-ref" => o.reference = Some(v.clone()),
            "-in" => o.input = Some(v.clone()),
            "-init" => {
                o.init = Some(v.clone());
                o.use_sqform = false;
            }
            "-schedule" => o.schedule = Some(v.clone()),
            "-refweight" => o.ref_weight = Some(v.clone()),
            "-inweight" => o.in_weight = Some(v.clone()),
            "-wmseg" => o.wmseg = Some(v.clone()),
            "-fieldmap" => o.fieldmap = Some(v.clone()),
            "-fieldmapmask" => o.fieldmap_mask = Some(v.clone()),
            "-wmcoords" => o.wmcoords = Some(v.clone()),
            "-wmnorms" => o.wmnorms = Some(v.clone()),
            "-omat" => o.omat = Some(v.clone()),
            "-bins" => o.bins = atoi(&v),
            "-dof" => o.dof = atoi(&v),
            "-sincwidth" => o.sinc_width = atoi(&v) as f32,
            "-paddingsize" => o.padding = atof(&v) as f32,
            "-applyisoxfm" => {
                o.apply_iso = Some(atof(&v) as f32);
                o.apply_xfm = true;
            }
            "-basescale" => o.base_scale = Some(atof(&v) as f32),
            "-minsampling" => o.min_sampling = atof(&v) as f32,
            "-setbackground" => o.background = Some(atof(&v) as f32),
            "-coarsesearch" => o.coarse = degrees_f32(&v),
            "-finesearch" => o.fine = degrees_f32(&v),
            "-echospacing" => o.echo_spacing = atof(&v) as f32,
            "-pedir" => {
                let d = atoi(&v);
                if d == 0 {
                    let _ = writeln!(
                        err,
                        "Unrecognised argument to pedir ({v}) - it should be a number between -3 and 3 (not 0)"
                    );
                    return Parsed::Exit(255);
                }
                o.pedir = d;
            }
            "-bbrtype" => o.bbr_type = v.clone(),
            "-bbrslope" => o.bbr_slope = atof(&v) as f32,
            "-verbose" => o.verbose = atoi(&v),
            "-datatype" => match OutType::from_name(&v) {
                Some(t) => o.datatype = Some(t),
                None => return bad("Unrecognised data type", err),
            },
            "-cost" | "-searchcost" => match Cost::from_name(&v) {
                Some(c) => {
                    if w == "-cost" {
                        o.cost = c;
                    } else {
                        o.search_cost = c;
                    }
                }
                None => return bad("Unrecognised cost function type", err),
            },
            "-interp" => match Interp::from_name(&v) {
                Some(m) => o.interp = m,
                None => return bad("Unrecognised interpolation method", err),
            },
            "-sincwindow" => match SincWindow::from_name(&v) {
                Some(m) => o.sinc_window = m,
                None => return bad("Unrecognised sinc window", err),
            },
            "-anglerep" => match v.as_str() {
                "quaternion" => o.quaternion = true,
                "euler" => o.quaternion = false,
                _ => return bad("Unrecognised angle representation", err),
            },
            _ => {}
        }
        i += 2;
    }
    if o.input.is_none() {
        let _ = writeln!(err, "ERROR:: Input volume filename not found\n");
        let _ = write!(out, "{}", usage(&o));
        return Parsed::Exit(2);
    }
    if o.reference.is_none() {
        let _ = writeln!(err, "ERROR:: Reference volume filename not found\n");
        let _ = write!(out, "{}", usage(&o));
        return Parsed::Exit(2);
    }
    Parsed::Run(Box::new(o))
}

/// `flirt` with FSL's arguments; returns the exit status.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl flirt: {e}");
            return 2;
        }
    };
    let o = match parse(args, out, err) {
        Parsed::Run(o) => o,
        Parsed::Exit(c) => return c,
    };
    let ctx = Ctx {
        settings,
        mode,
        n_threads: settings.threads(),
    };
    engine::run(&o, &ctx, out, err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(words: &[&str]) -> (Option<Box<Options>>, u8, String, String) {
        let args: Vec<String> = words.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let r = parse(&args, &mut out, &mut err);
        let (o, code) = match r {
            Parsed::Run(o) => (Some(o), 0),
            Parsed::Exit(c) => (None, c),
        };
        (
            o,
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn the_scans_messages() {
        let (_, c, out, _) = scan(&[]);
        assert_eq!(c, 1);
        assert!(out.starts_with("FLIRT version 6.0\n\nUsage: flirt"));
        let (_, c, out, _) = scan(&["-bins", "64", "-help"]);
        assert_eq!(c, 0);
        assert!(out.contains("(default is 64)"));
        let (_, c, _, err) = scan(&["epi", "-ref", "t1"]);
        assert_eq!((c, err.as_str()), (255, "Unrecognised option e\n"));
        let (_, c, _, err) = scan(&["-in", "a", "-ref", "b", "-frob", "3"]);
        assert_eq!(
            (c, err.as_str()),
            (255, "Lacking argument to option -frob\n")
        );
        let (_, c, _, err) = scan(&["-in", "a", "-ref", "b", "-frob", "3", "4"]);
        assert_eq!((c, err.as_str()), (255, "Unrecognised option -frob\n"));
        let (_, c, _, err) = scan(&["-in", "a", "-ref", "b", "-searchrx", "3"]);
        assert_eq!(
            (c, err.as_str()),
            (255, "Lacking argument to option -searchrx\n")
        );
        let (_, c, _, err) = scan(&["-in", "a", "-ref", "b", "-pedir", "0"]);
        assert_eq!(c, 255);
        assert!(err.starts_with("Unrecognised argument to pedir (0)"));
        let (_, c, _, err) = scan(&["-in", "a", "-ref", "b", "-cost", "nope"]);
        assert_eq!(
            (c, err.as_str()),
            (255, "Unrecognised cost function type: nope\n")
        );
        let (_, c, out, err) = scan(&["-in", "a"]);
        assert_eq!(c, 2);
        assert_eq!(err, "ERROR:: Reference volume filename not found\n\n");
        assert!(out.starts_with("FLIRT version 6.0"));
    }

    #[test]
    fn options_are_read() {
        let (o, _, _, _) = scan(&[
            "-in",
            "a",
            "-ref",
            "b",
            "-searchrx",
            "20",
            "-10",
            "-nosearch",
            "-searchry",
            "-5",
            "5",
            "-dof",
            "7",
            "-init",
            "m.mat",
            "-coarsesearch",
            "30",
            "",
        ]);
        let o = o.unwrap();
        assert_eq!(o.search[0], [0.0, 0.0]);
        assert!((o.search[1][1] - 5f64.to_radians()).abs() < 1e-15);
        assert_eq!(o.dof, 7);
        assert!(!o.use_sqform && o.init.as_deref() == Some("m.mat"));
        assert_eq!(o.coarse, (30.0 * std::f64::consts::PI / 180.0) as f32);
    }
}
