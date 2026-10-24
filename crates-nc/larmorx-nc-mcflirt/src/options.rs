// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17: fsl-mcflirt 2111.0 (Globaloptions.{h,cc}). Copyright (C)
// 1999-2001 University of Oxford (Peter Bannister). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the original. Distributed under the FSL Licence (see LICENSE).
//! mcflirt's command line (`Globaloptions::parse_command_line`): options, messages and exit
//! statuses, with larmorx's corrections of FSL-05, -06 and -07 (D13).

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_nc_core::costfns::CostType;
use larmorx_nc_core::settings::atoi;
use larmorx_nc_core::volume::make_basename;

/// mcflirt's usage text (`print_usage`, on standard output).
pub fn usage() -> String {
    let mut s = String::new();
    s.push_str("Usage: mcflirt -in <infile> [options]\n\n");
    s.push_str("  Available options are:\n");
    s.push_str("        -out, -o <outfile>               (default is infile_mcf)\n");
    s.push_str("        -cost {mutualinfo,woods,corratio,normcorr,normmi,leastsquares}        (default is normcorr)\n");
    s.push_str("        -bins <number of histogram bins>   (default is 256)\n");
    s.push_str("        -dof  <number of transform dofs>   (default is 6)\n");
    s.push_str("        -refvol <number of reference volume> (default is no_vols/2)- registers to (n+1)th volume in series\n");
    s.push_str("        -reffile, -r <filename>            use a separate 3d image file as the target for registration (overrides refvol option)\n");
    s.push_str("        -scaling <num>                             (6.0 is default)\n");
    s.push_str("        -smooth <num>                      (1.0 is default - controls smoothing in cost function)\n");
    s.push_str("        -rotation <num>                    specify scaling factor for rotation optimization tolerances\n");
    s.push_str("        -verbose <num>                     (0 is least and default)\n");
    s.push_str("        -stages <number of search levels>  (default is 3 - specify 4 for final sinc interpolation)\n");
    s.push_str("        -fov <num>                         (default is 20mm - specify size of field of view when padding 2d volume)\n");
    s.push_str("        -2d                                Force padding of volume\n");
    s.push_str("        -sinc_final                        (applies final transformations using sinc interpolation)\n");
    s.push_str("        -spline_final                      (applies final transformations using spline interpolation)\n");
    s.push_str("        -nn_final                          (applies final transformations using Nearest Neighbour interpolation)\n");
    s.push_str("        -init <filename>                   (initial transform matrix to apply to all vols)\n");
    s.push_str("        -gdt                               (run search on gradient images)\n");
    s.push_str("        -meanvol                           register timeseries to mean volume (overrides refvol and reffile options)\n");
    s.push_str("        -stats                             produce variance and std. dev. images\n");
    s.push_str("        -mats                              save transformation matricies in subdirectory outfilename.mat\n");
    s.push_str("        -plots                             save transformation parameters in file outputfilename.par\n");
    s.push_str("        -report                            report progress to screen\n");
    s.push_str("        -help\n");
    s
}

/// `Globaloptions`: what the command line sets.
#[derive(Clone, Debug)]
pub struct Options {
    pub inputfname: String,
    pub outputfname: String,
    pub verbose: i32,
    pub no_params: i32,
    pub dof: i32,
    pub no_bins: i32,
    pub refnum: i32,
    pub gdtflag: bool,
    pub histflag: bool,
    pub matflag: bool,
    pub statflag: bool,
    pub plotflag: bool,
    pub tmpmatflag: bool,
    pub no_reporting: bool,
    pub meanvol: bool,
    pub sinc_final: bool,
    pub spline_final: bool,
    pub nn_final: bool,
    pub rmsrelflag: bool,
    pub rmsabsflag: bool,
    pub scaling: f32,
    pub smoothsize: f32,
    pub rot_param: f32,
    pub no_stages: i32,
    pub fudgeflag: bool,
    pub fov: f32,
    pub twodcorrect: bool,
    pub reffileflag: bool,
    pub reffilename: String,
    pub init_transform: String,
    pub maincostfn: CostType,
    /// Whether no `-out` was given: the output name is the input's basename plus `_mcf`.
    pub default_output: bool,
}

impl Default for Options {
    /// `Globaloptions::Globaloptions()`.
    fn default() -> Self {
        Options {
            inputfname: String::new(),
            outputfname: String::new(),
            verbose: 0,
            no_params: 6,
            dof: 6,
            no_bins: 256,
            refnum: -1,
            gdtflag: false,
            histflag: false,
            matflag: false,
            statflag: false,
            plotflag: false,
            tmpmatflag: false,
            no_reporting: true,
            meanvol: false,
            sinc_final: false,
            spline_final: false,
            nn_final: false,
            rmsrelflag: false,
            rmsabsflag: false,
            scaling: 6.0,
            smoothsize: 1.0,
            rot_param: 1.0,
            no_stages: 3,
            fudgeflag: false,
            fov: 20.0,
            twodcorrect: false,
            reffileflag: false,
            reffilename: String::new(),
            init_transform: String::new(),
            maincostfn: CostType::NormCorr,
            default_output: false,
        }
    }
}

/// What parsing produced: options to run, or an exit with its outputs.
#[derive(Debug)]
pub enum Parsed {
    Run(Box<Options>),
    Exit(Exit),
}

/// A program end: the exit status and what it wrote.
#[derive(Debug, Default)]
pub struct Exit {
    pub code: u8,
    pub stdout: String,
    pub stderr: String,
}

/// glibc's `atof` (`strtod` with no end pointer): the longest prefix that is a number, after
/// leading white space; 0 when there is none. Rust's parser is correctly rounded, as glibc's
/// `strtod` is.
pub fn atof(s: &str) -> f64 {
    let t = s.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    // Longest prefix accepted by Rust's float parser among decimal forms; inf/nan as glibc.
    let lower = t.to_ascii_lowercase();
    let (sign, rest_lower, rest) = match lower.as_bytes().first() {
        Some(b'-') => (-1.0, &lower[1..], &t[1..]),
        Some(b'+') => (1.0, &lower[1..], &t[1..]),
        _ => (1.0, &lower[..], t),
    };
    if rest_lower.starts_with("inf") {
        return sign * f64::INFINITY;
    }
    if rest_lower.starts_with("nan") {
        return if sign < 0.0 { -f64::NAN } else { f64::NAN };
    }
    if rest_lower.starts_with("0x") {
        return sign * hex_prefix(&rest[2..]);
    }
    let b = rest.as_bytes();
    let mut j = 0;
    let mut digits = 0;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
        digits += 1;
    }
    if j < b.len() && b[j] == b'.' {
        j += 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return 0.0;
    }
    if j < b.len() && (b[j] == b'e' || b[j] == b'E') {
        let mut k = j + 1;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }
        let ds = k;
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        if k > ds {
            j = k;
        }
    }
    sign * rest[..j].parse::<f64>().unwrap_or(0.0)
}

/// The value of a hexadecimal significand prefix (`strtod`'s `0x` form), exponent included.
fn hex_prefix(s: &str) -> f64 {
    let b = s.as_bytes();
    let mut v = 0.0f64;
    let mut j = 0;
    let mut scale = 0i32;
    let mut any = false;
    while j < b.len() && b[j].is_ascii_hexdigit() {
        v = v * 16.0 + f64::from((b[j] as char).to_digit(16).unwrap_or(0));
        j += 1;
        any = true;
    }
    if j < b.len() && b[j] == b'.' {
        j += 1;
        while j < b.len() && b[j].is_ascii_hexdigit() {
            v = v * 16.0 + f64::from((b[j] as char).to_digit(16).unwrap_or(0));
            scale -= 4;
            j += 1;
            any = true;
        }
    }
    if !any {
        return 0.0;
    }
    if j < b.len() && (b[j] == b'p' || b[j] == b'P') {
        let rest = &s[j + 1..];
        let e = atoi(rest);
        if rest.trim_start_matches(['+', '-']).starts_with(|c: char| c.is_ascii_digit()) {
            scale += e;
        }
    }
    v * 2f64.powi(scale)
}

/// Whether `v` is exactly an integer as written (what larmorx's correction of FSL-07 accepts).
fn strict_int(v: &str) -> Option<i64> {
    v.trim().parse::<i64>().ok()
}

/// Whether `v` is exactly a finite number as written.
fn strict_float(v: &str) -> Option<f64> {
    v.trim().parse::<f64>().ok().filter(|x| x.is_finite())
}

/// An integer option (`atoi`), with FSL-07: corrected, a value that is not exactly an integer is
/// an error (exit 255); reproduced, `atoi`'s reading with a notice.
fn int_option(
    option: &str,
    v: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<i32, String> {
    let c = atoi(v);
    match strict_int(v) {
        Some(n) if n == i64::from(c) => Ok(c),
        _ => {
            if mode.fix() {
                notices.push(Notice::new(
                    &defects::FSL_07,
                    mode,
                    format!("'{option} {v}' is not an integer: an error; mcflirt reads it as {c}"),
                ));
                Err(format!(
                    "{option} needs an integer, not '{v}' (mcflirt reads it with C's atoi as {c}; \
                     {}=reproduce does too)",
                    defects::ENV
                ))
            } else {
                notices.push(Notice::new(
                    &defects::FSL_07,
                    mode,
                    format!("'{option} {v}' is read as {c}, as C's atoi reads it"),
                ));
                Ok(c)
            }
        }
    }
}

/// A float option (`atof`, stored in a `float`), with FSL-07 as [`int_option`].
fn float_option(
    option: &str,
    v: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<f32, String> {
    let c = atof(v);
    match strict_float(v) {
        Some(x) if x.to_bits() == c.to_bits() => Ok(c as f32),
        _ => {
            if mode.fix() {
                notices.push(Notice::new(
                    &defects::FSL_07,
                    mode,
                    format!(
                        "'{option} {v}' is not a number: an error; mcflirt reads it as {}",
                        larmorx_nc_core::cnum::stream(c)
                    ),
                ));
                Err(format!(
                    "{option} needs a number, not '{v}' (mcflirt reads it with C's atof as {}; \
                     {}=reproduce does too)",
                    larmorx_nc_core::cnum::stream(c),
                    defects::ENV
                ))
            } else {
                notices.push(Notice::new(
                    &defects::FSL_07,
                    mode,
                    format!(
                        "'{option} {v}' is read as {}, as C's atof reads it",
                        larmorx_nc_core::cnum::stream(c)
                    ),
                ));
                Ok(c as f32)
            }
        }
    }
}

/// `parse_command_line(argc, argv)` (`args` without the program name), in `mode`. Notices of
/// the defects met are collected in `notices`; messages go into the returned outputs.
pub fn parse(args: &[String], mode: UpstreamDefects, notices: &mut Vec<Notice>) -> (Parsed, String) {
    let mut err = String::new();
    // argc < 3: the usage and exit 1 (so `mcflirt -help` alone exits 1: FSL-07).
    if args.len() < 2 {
        let mut code = 1;
        if args.len() == 1 && args[0] == "-help" {
            if mode.fix() {
                code = 0;
                notices.push(Notice::new(
                    &defects::FSL_07,
                    mode,
                    "-help alone prints the usage and exits 0; mcflirt exits 1",
                ));
            } else {
                notices.push(Notice::new(
                    &defects::FSL_07,
                    mode,
                    "-help alone exits 1, as in mcflirt",
                ));
            }
        }
        return (
            Parsed::Exit(Exit {
                code,
                stdout: usage(),
                stderr: err,
            }),
            String::new(),
        );
    }
    let mut o = Options::default();
    let mut n = 0;
    while n < args.len() {
        let arg = args[n].as_str();
        if arg.is_empty() {
            n += 1;
            continue;
        }
        if !arg.starts_with('-') {
            o.inputfname = arg.to_owned();
            n += 1;
            err.push_str("WARNING: change in option usage\n\n");
            err.push_str("To specify the input volume the option -in should be used\n");
            err.push_str(
                "Accepting the filename for now, but please update to new syntax in future.\n\n",
            );
            continue;
        }
        let flag = match arg {
            "-help" => {
                return (
                    Parsed::Exit(Exit {
                        code: 0,
                        stdout: usage(),
                        stderr: err,
                    }),
                    String::new(),
                );
            }
            "-v" => {
                o.verbose = 5;
                true
            }
            "-gdt" => {
                o.gdtflag = true;
                true
            }
            "-hist" => {
                o.histflag = true;
                notices.push(ignored(mode, "-hist"));
                true
            }
            "-stats" => {
                o.statflag = true;
                true
            }
            "-mats" => {
                o.tmpmatflag = true;
                o.matflag = true;
                true
            }
            "-plots" => {
                o.tmpmatflag = true;
                o.plotflag = true;
                true
            }
            "-meanvol" => {
                o.meanvol = true;
                true
            }
            "-report" => {
                o.no_reporting = false;
                true
            }
            "-fudge" => {
                o.fudgeflag = true;
                true
            }
            "-sinc_final" => {
                o.sinc_final = true;
                true
            }
            "-nn_final" => {
                o.nn_final = true;
                true
            }
            "-spline_final" => {
                o.spline_final = true;
                true
            }
            "-rmsrel" => {
                o.tmpmatflag = true;
                o.rmsrelflag = true;
                true
            }
            "-rmsabs" => {
                o.tmpmatflag = true;
                o.rmsabsflag = true;
                true
            }
            "-2d" => {
                o.twodcorrect = true;
                true
            }
            _ => false,
        };
        if flag {
            n += 1;
            continue;
        }
        if n + 1 >= args.len() {
            err.push_str(&format!("Lacking argument to option {arg}\n"));
            return (
                Parsed::Exit(Exit {
                    code: 255,
                    stdout: String::new(),
                    stderr: err,
                }),
                String::new(),
            );
        }
        let v = args[n + 1].as_str();
        let result: Result<(), String> = match arg {
            "-in" => {
                o.inputfname = v.to_owned();
                Ok(())
            }
            "-o" | "-out" => {
                o.outputfname = v.to_owned();
                Ok(())
            }
            "-r" | "-reffile" => {
                o.reffilename = v.to_owned();
                o.reffileflag = true;
                Ok(())
            }
            "-init" => {
                o.init_transform = v.to_owned();
                Ok(())
            }
            "-dof" => int_option(arg, v, mode, notices).map(|c| {
                o.no_params = c;
                o.dof = c;
            }),
            "-verbose" => int_option(arg, v, mode, notices).map(|c| o.verbose = c),
            "-bins" => int_option(arg, v, mode, notices).map(|c| o.no_bins = c),
            "-refvol" => int_option(arg, v, mode, notices).map(|c| o.refnum = c),
            "-scaling" => {
                // Ignored (FSL-05): its value is read with atof and never used.
                notices.push(ignored(mode, "-scaling"));
                o.scaling = atof(v) as f32;
                Ok(())
            }
            "-smooth" => float_option(arg, v, mode, notices).map(|x| o.smoothsize = x),
            "-rotation" => float_option(arg, v, mode, notices).map(|x| o.rot_param = x),
            "-stages" => int_option(arg, v, mode, notices).map(|c| o.no_stages = c),
            "-fov" => int_option(arg, v, mode, notices).map(|c| o.fov = c as f32),
            "-cost" => {
                let c = match v {
                    "mutualinfo" => Some(CostType::MutualInfo),
                    "corratio" => Some(CostType::CorrRatio),
                    "woods" => Some(CostType::Woods),
                    "normcorr" => Some(CostType::NormCorr),
                    "normmi" => Some(CostType::NormMI),
                    "leastsquares" => Some(CostType::LeastSq),
                    _ => None,
                };
                match c {
                    Some(c) => {
                        o.maincostfn = c;
                        Ok(())
                    }
                    None if mode.fix() => {
                        notices.push(Notice::new(
                            &defects::FSL_06,
                            mode,
                            format!("the unknown cost '{v}' is an error; mcflirt uses normcorr"),
                        ));
                        Err(format!(
                            "Unrecognised cost function type: {v} (use mutualinfo, woods, \
                             corratio, normcorr, normmi or leastsquares; mcflirt silently uses \
                             normcorr, and so does {}=reproduce)",
                            defects::ENV
                        ))
                    }
                    None => {
                        if !o.no_reporting {
                            err.push_str(&format!("Unrecognised cost function type: {v}\n"));
                            err.push_str("Using the default (NormCorr)\n");
                        }
                        notices.push(Notice::new(
                            &defects::FSL_06,
                            mode,
                            format!("the unknown cost '{v}' is replaced by normcorr, as in mcflirt"),
                        ));
                        Ok(())
                    }
                }
            }
            _ => {
                err.push_str(&format!("Unrecognised option {arg}\n"));
                return (
                    Parsed::Exit(Exit {
                        code: 255,
                        stdout: String::new(),
                        stderr: err,
                    }),
                    String::new(),
                );
            }
        };
        if let Err(message) = result {
            err.push_str(&format!("Error: {message}\n"));
            return (
                Parsed::Exit(Exit {
                    code: 255,
                    stdout: String::new(),
                    stderr: err,
                }),
                String::new(),
            );
        }
        n += 2;
    }
    if o.inputfname.is_empty() {
        err.push_str("Input filename not found\n\n");
        return (
            Parsed::Exit(Exit {
                code: 2,
                stdout: usage(),
                stderr: err,
            }),
            String::new(),
        );
    }
    if o.outputfname.is_empty() {
        // make_basename(inputfname) modifies the input name too: it is then read by FSL's name
        // search.
        o.inputfname = make_basename(&o.inputfname);
        o.outputfname = format!("{}_mcf", o.inputfname);
        o.default_output = true;
        if !o.no_reporting {
            err.push_str("No output name specified. ");
        }
    }
    if !o.no_reporting {
        err.push_str(&format!("Processed data will be saved as {}\n", o.outputfname));
    }
    (Parsed::Run(Box::new(o)), err)
}

/// FSL-05: an option mcflirt accepts and ignores.
fn ignored(mode: UpstreamDefects, option: &str) -> Notice {
    let detail = match mode {
        UpstreamDefects::Fix => format!("{option} has no effect, here as in mcflirt"),
        UpstreamDefects::Reproduce => format!("{option} is accepted and ignored, as in mcflirt"),
    };
    Notice::new(&defects::FSL_05, mode, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str], mode: UpstreamDefects) -> (Parsed, String, Vec<Notice>) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let mut notices = Vec::new();
        let (p, err) = parse(&args, mode, &mut notices);
        (p, err, notices)
    }

    const FIX: UpstreamDefects = UpstreamDefects::Fix;
    const REPRODUCE: UpstreamDefects = UpstreamDefects::Reproduce;

    #[test]
    fn exits() {
        let (p, _, _) = parse_args(&[], REPRODUCE);
        assert!(matches!(p, Parsed::Exit(Exit { code: 1, .. })));
        let (p, _, n) = parse_args(&["-help"], REPRODUCE);
        assert!(matches!(p, Parsed::Exit(Exit { code: 1, .. })));
        assert_eq!(n[0].defect.id, "FSL-07");
        let (p, _, _) = parse_args(&["-help"], FIX);
        assert!(matches!(p, Parsed::Exit(Exit { code: 0, .. })));
        let (p, _, _) = parse_args(&["-in", "x", "-help"], REPRODUCE);
        assert!(matches!(p, Parsed::Exit(Exit { code: 0, .. })));
        let (p, _, _) = parse_args(&["-in", "x", "-mats", "-edge"], REPRODUCE);
        match p {
            Parsed::Exit(e) => {
                assert_eq!(e.code, 255);
                assert_eq!(e.stderr, "Lacking argument to option -edge\n");
            }
            _ => panic!(),
        }
        let (p, _, _) = parse_args(&["-in", "x", "-edge", "y", "-mats"], REPRODUCE);
        match p {
            Parsed::Exit(e) => assert_eq!(e.stderr, "Unrecognised option -edge\n"),
            _ => panic!(),
        }
        let (p, _, _) = parse_args(&["-out", "y", "-mats"], REPRODUCE);
        match p {
            Parsed::Exit(e) => {
                assert_eq!(e.code, 2);
                assert_eq!(e.stderr, "Input filename not found\n\n");
                assert!(e.stdout.starts_with("Usage: mcflirt"));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn values_and_names() {
        let (p, _, n) = parse_args(&["-in", "a/b.nii.gz", "-refvol", "abc"], REPRODUCE);
        match p {
            Parsed::Run(o) => {
                assert_eq!(o.refnum, 0);
                assert_eq!(o.inputfname, "a/b");
                assert_eq!(o.outputfname, "a/b_mcf");
            }
            _ => panic!(),
        }
        assert_eq!(n[0].defect.id, "FSL-07");
        let (p, _, _) = parse_args(&["-in", "x", "-refvol", "abc"], FIX);
        assert!(matches!(p, Parsed::Exit(Exit { code: 255, .. })));
        let (p, _, _) = parse_args(&["-in", "x", "-cost", "bogus"], FIX);
        assert!(matches!(p, Parsed::Exit(Exit { code: 255, .. })));
        let (p, _, _) = parse_args(&["-in", "x", "-cost", "bogus"], REPRODUCE);
        match p {
            Parsed::Run(o) => assert_eq!(o.maincostfn, CostType::NormCorr),
            _ => panic!(),
        }
        assert_eq!(atof("2mm"), 2.0);
        assert_eq!(atof(" -1.5e1x"), -15.0);
        assert_eq!(atof("x"), 0.0);
        assert_eq!(atof("1e"), 1.0);
    }
}
