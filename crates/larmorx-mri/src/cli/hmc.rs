// SPDX-License-Identifier: Apache-2.0
//! `larmorx mri hmc`: head-motion correction accepting mcflirt-style options
//! (`specs/mcflirt.md` §10–§12).
//!
//! Deliberate differences from mcflirt (documented in `docs/api/mri-hmc.md`): without
//! `FSLOUTPUTTYPE` the output is `.nii.gz` (mcflirt fails); impossible inputs (a reference
//! index out of range, a reference grid with another number of voxels, `-stages 0` with a
//! reference file) are errors reported before any work, with exit code 1 (mcflirt aborts);
//! `-gdt` is not supported.
//!
//! mcflirt's defects FSL-01, -03, -05 to -10, -13, -14, -59 and -60 are corrected unless the
//! environment variable `LARMORX_UPSTREAM_DEFECTS` is `reproduce` (D13,
//! `docs/api/upstream-defects.md`); a line on standard error names each one that changed (or
//! would have changed) the outputs.
//!
//! Files follow FSL 6.0.7.23 (`specs/mcflirt.md` §4.1, §10.1): read by their content (gzip,
//! zstd, bzip2 or uncompressed), names without an image extension completed in FSL's order,
//! and every `FSLOUTPUTTYPE` written, the zstd and bzip2 types included; an unknown one is
//! reported in FSL's words before any work.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notice, UpstreamDefects};
use larmorx_core::element::DataType;

use super::{atof, atoi};
use crate::hmc::estimate::{EstimateParams, InPlane};
use crate::hmc::files::{self as image_files, ImageFiles};
use crate::hmc::image::{
    self, Series, output_name, output_type, read_reference_files, read_series_files,
};
use crate::hmc::pipeline::{self, HmcOptions, HmcOutput, ReferenceChoice};
use crate::hmc::report::{self, mat_text, par_text, rms_text};
use crate::hmc::resample::{self, Interpolation};
use crate::hmc::rigid::IDENTITY;
use crate::hmc::volume::Volume;
use crate::hmc::{CostFunction, stats};

const USAGE: &str = "Usage: larmorx mri hmc -in <infile> [options]
  Head-motion correction (larmorx, clean-room; accepts mcflirt-style options)

  Available options are:
        -out, -o <outfile>               (default is infile_mcf)
        -cost {mutualinfo,woods,corratio,normcorr,normmi,leastsquares}        (default is normcorr)
        -bins <number of histogram bins>   (default is 256)
        -dof  <number of transform dofs>   (default is 6)
        -refvol <number of reference volume> (default is no_vols/2)- registers to (n+1)th volume in series
        -reffile, -r <filename>            use a separate 3d image file as the target for registration (overrides refvol option)
        -scaling <num>                             (accepted and ignored)
        -smooth <num>                      (1.0 is default - controls smoothing in cost function)
        -rotation <num>                    specify scaling factor for rotation optimization tolerances
        -verbose <num>                     (0 is least and default)
        -stages <number of search levels>  (default is 3 - specify 4 for final sinc interpolation)
        -fov <num>                         (default is 20mm - specify size of field of view when padding 2d volume)
        -2d                                Force padding of volume
        -sinc_final                        (applies final transformations using sinc interpolation)
        -spline_final                      (applies final transformations using spline interpolation)
        -nn_final                          (applies final transformations using Nearest Neighbour interpolation)
        -init <filename>                   (initial transform matrix to apply to all vols)
        -meanvol                           register timeseries to mean volume (overrides refvol and reffile options)
        -stats                             produce variance and std. dev. images
        -mats                              save transformation matricies in subdirectory outfilename.mat
        -plots                             save transformation parameters in file outputfilename.par
        -report                            report progress to screen
        -help

  Output images use the extension of FSLOUTPUTTYPE (default NIFTI_GZ). Threads: OMP_NUM_THREADS
  (default: all logical CPUs); the results do not depend on it.
";

/// Options without a value.
const FLAGS: &[&str] = &[
    "-mats",
    "-plots",
    "-rmsrel",
    "-rmsabs",
    "-stats",
    "-meanvol",
    "-sinc_final",
    "-spline_final",
    "-nn_final",
    "-2d",
    "-gdt",
    "-fudge",
    "-report",
    "-v",
    "-hist",
    "-help",
];

/// Options with a value.
const VALUED: &[&str] = &[
    "-in",
    "-out",
    "-o",
    "-reffile",
    "-r",
    "-refvol",
    "-stages",
    "-dof",
    "-cost",
    "-bins",
    "-smooth",
    "-rotation",
    "-scaling",
    "-fov",
    "-init",
    "-verbose",
];

#[derive(Debug)]
struct Options {
    input: Option<String>,
    out: Option<String>,
    reffile: Option<String>,
    refvol: i64,
    mats: bool,
    plots: bool,
    rmsrel: bool,
    rmsabs: bool,
    stats: bool,
    meanvol: bool,
    stages: i64,
    interpolation: Interpolation,
    sinc: bool,
    nn: bool,
    spline: bool,
    dof: i64,
    cost: CostFunction,
    bins: i64,
    smooth: f64,
    rotation: f64,
    fov: i64,
    two_d: bool,
    init: Option<String>,
    gdt: bool,
    fudge: bool,
    report: bool,
    /// The upstream defects the command line itself met (FSL-05, -06, -07).
    notices: Vec<Notice>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input: None,
            out: None,
            reffile: None,
            refvol: -1,
            mats: false,
            plots: false,
            rmsrel: false,
            rmsabs: false,
            stats: false,
            meanvol: false,
            stages: 3,
            interpolation: Interpolation::Trilinear,
            sinc: false,
            nn: false,
            spline: false,
            dof: 6,
            cost: CostFunction::NormCorr,
            bins: 256,
            smooth: 1.0,
            rotation: 1.0,
            fov: 20,
            two_d: false,
            init: None,
            gdt: false,
            fudge: false,
            report: false,
            notices: Vec::new(),
        }
    }
}

/// C's `atoi` reading of an integer option (FSL-07): mcflirt's (reproduced, with a notice
/// when the text is not exactly an integer), or a strict reading (corrected: anything but an
/// integer is an error).
fn int_value(
    option: &str,
    v: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<i64, String> {
    let strict = v.trim().parse::<i64>().ok();
    match (mode, strict) {
        (_, Some(n)) if n == atoi(v) => Ok(n),
        (UpstreamDefects::Fix, _) => {
            notices.push(Notice::new(
                &defects::FSL_07,
                mode,
                format!(
                    "'{option} {v}' is not an integer: an error; mcflirt reads it as {}",
                    atoi(v)
                ),
            ));
            Err(format!(
                "{option} needs an integer, not '{v}' (mcflirt reads it with C's atoi as {}; \
                 {}=reproduce does too)",
                atoi(v),
                defects::ENV
            ))
        }
        (UpstreamDefects::Reproduce, _) => {
            notices.push(Notice::new(
                &defects::FSL_07,
                mode,
                format!(
                    "'{option} {v}' is read as {}, as C's atoi reads it",
                    atoi(v)
                ),
            ));
            Ok(atoi(v))
        }
    }
}

/// C's `atof` reading of a number option, strict when corrected ([`int_value`]).
fn float_value(
    option: &str,
    v: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<f64, String> {
    let strict = v.trim().parse::<f64>().ok().filter(|x| x.is_finite());
    match (mode, strict) {
        (_, Some(x)) if x.to_bits() == atof(v).to_bits() => Ok(x),
        (UpstreamDefects::Fix, _) => {
            notices.push(Notice::new(
                &defects::FSL_07,
                mode,
                format!(
                    "'{option} {v}' is not a number: an error; mcflirt reads it as {}",
                    atof(v)
                ),
            ));
            Err(format!(
                "{option} needs a number, not '{v}' (mcflirt reads it with C's atof as {}; \
                 {}=reproduce does too)",
                atof(v),
                defects::ENV
            ))
        }
        (UpstreamDefects::Reproduce, _) => {
            notices.push(Notice::new(
                &defects::FSL_07,
                mode,
                format!(
                    "'{option} {v}' is read as {}, as C's atof reads it",
                    atof(v)
                ),
            ));
            Ok(atof(v))
        }
    }
}

/// What parsing decided.
enum Parsed {
    Run(Box<Options>),
    Exit(u8),
}

fn parse(
    args: &[String],
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Parsed {
    let mut o = Options::default();
    let mut i = 0;
    // A value error of the corrected command line: exit 255, as mcflirt's option errors.
    let bad = |err: &mut dyn Write, message: String| {
        let _ = writeln!(err, "Error: {message}");
        Parsed::Exit(255)
    };
    while i < args.len() {
        let a = args[i].as_str();
        if !a.starts_with('-') {
            let _ = writeln!(
                err,
                "WARNING: change in option usage\n\nTo specify the input volume the option -in \
                 should be used\nAccepting the filename for now, but please update to new syntax \
                 in future.\n"
            );
            o.input = Some(a.to_owned());
            i += 1;
            continue;
        }
        if FLAGS.contains(&a) {
            match a {
                "-mats" => o.mats = true,
                "-plots" => o.plots = true,
                "-rmsrel" => o.rmsrel = true,
                "-rmsabs" => o.rmsabs = true,
                "-stats" => o.stats = true,
                "-meanvol" => o.meanvol = true,
                "-sinc_final" => o.sinc = true,
                "-spline_final" => o.spline = true,
                "-nn_final" => o.nn = true,
                "-2d" => o.two_d = true,
                "-gdt" => o.gdt = true,
                "-fudge" => o.fudge = true,
                "-report" => o.report = true,
                "-help" => {
                    let _ = out.write_all(USAGE.as_bytes());
                    return Parsed::Exit(0);
                }
                "-hist" => o.notices.push(ignored_option("-hist", mode)),
                _ => {} // -v: progress messages only
            }
            i += 1;
            continue;
        }
        if i + 1 >= args.len() {
            let _ = writeln!(err, "Lacking argument to option {a}");
            return Parsed::Exit(255);
        }
        if !VALUED.contains(&a) {
            let _ = writeln!(err, "Unrecognised option {a}");
            if a == "-edge" && mode.fix() {
                let _ = writeln!(
                    err,
                    "(-edge is not an mcflirt option: nipype's MCFLIRT interface emits it for \
                     use_contour)"
                );
            }
            return Parsed::Exit(255);
        }
        let v = args[i + 1].as_str();
        let notices = &mut o.notices;
        let parsed = match a {
            "-in" => {
                o.input = Some(v.to_owned());
                Ok(())
            }
            "-out" | "-o" => {
                o.out = Some(v.to_owned());
                Ok(())
            }
            "-reffile" | "-r" => {
                o.reffile = Some(v.to_owned());
                Ok(())
            }
            "-refvol" => int_value(a, v, mode, notices).map(|n| o.refvol = n),
            "-stages" => int_value(a, v, mode, notices).map(|n| o.stages = n),
            "-dof" => int_value(a, v, mode, notices).map(|n| o.dof = n),
            "-cost" => match CostFunction::from_name(v) {
                Some(c) => {
                    o.cost = c;
                    Ok(())
                }
                None if mode.fix() => {
                    notices.push(Notice::new(
                        &defects::FSL_06,
                        mode,
                        format!("the unknown cost '{v}' is an error; mcflirt uses normcorr"),
                    ));
                    Err(format!(
                        "Unrecognised cost function type: {v} (use mutualinfo, woods, corratio, \
                     normcorr, normmi or leastsquares; mcflirt silently uses normcorr, and so \
                     does {}=reproduce)",
                        defects::ENV
                    ))
                }
                None => {
                    if o.report {
                        let _ = writeln!(
                            err,
                            "Unrecognised cost function type: {v}\nUsing the default (NormCorr)"
                        );
                    }
                    notices.push(Notice::new(
                        &defects::FSL_06,
                        mode,
                        format!("the unknown cost '{v}' is replaced by normcorr, as in mcflirt"),
                    ));
                    o.cost = CostFunction::NormCorr;
                    Ok(())
                }
            },
            "-bins" => int_value(a, v, mode, notices).map(|n| o.bins = n),
            "-smooth" => float_value(a, v, mode, notices).map(|x| o.smooth = x),
            "-rotation" => float_value(a, v, mode, notices).map(|x| o.rotation = x),
            "-fov" => int_value(a, v, mode, notices).map(|n| o.fov = n),
            "-init" => {
                o.init = Some(v.to_owned());
                Ok(())
            }
            "-scaling" => {
                notices.push(ignored_option("-scaling", mode));
                Ok(())
            }
            _ => Ok(()), // -verbose: progress messages only
        };
        if let Err(message) = parsed {
            for n in &o.notices {
                print_notice(err, n);
            }
            return bad(err, message);
        }
        i += 2;
    }
    o.interpolation = if o.sinc {
        Interpolation::Sinc
    } else if o.nn {
        Interpolation::Nearest
    } else if o.spline {
        Interpolation::Spline
    } else {
        Interpolation::Trilinear
    };
    Parsed::Run(Box::new(o))
}

/// FSL-05: an option mcflirt documents (or accepts) and ignores.
fn ignored_option(option: &str, mode: UpstreamDefects) -> Notice {
    let detail = match mode {
        UpstreamDefects::Fix => format!("{option} has no effect, here as in mcflirt"),
        UpstreamDefects::Reproduce => format!("{option} is accepted and ignored, as in mcflirt"),
    };
    Notice::new(&defects::FSL_05, mode, detail)
}

/// Runs `larmorx mri hmc` with `args` (mcflirt-style options); returns the exit code. The D13
/// mode comes from `LARMORX_UPSTREAM_DEFECTS`.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match UpstreamDefects::from_env() {
        Ok(mode) => main_with(args, mode, out, err),
        Err(e) => {
            let _ = writeln!(err, "Error: {e}");
            EXIT_BAD_MODE
        }
    }
}

/// Prints an upstream-defect notice on standard error (`Notice::line`).
fn print_notice(err: &mut dyn Write, notice: &Notice) {
    let _ = writeln!(err, "larmorx: {}", notice.line());
}

/// [`main`] with the D13 mode given.
pub fn main_with(
    args: &[String],
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if args.len() < 2 {
        let _ = out.write_all(USAGE.as_bytes());
        // FSL-07: `mcflirt -help` alone exits 1.
        if args.len() == 1 && args[0] == "-help" {
            let detail = match mode {
                UpstreamDefects::Fix => "-help alone prints the usage and exits 0; mcflirt exits 1",
                UpstreamDefects::Reproduce => "-help alone exits 1, as in mcflirt",
            };
            print_notice(err, &Notice::new(&defects::FSL_07, mode, detail));
            return if mode.fix() { 0 } else { 1 };
        }
        return 1;
    }
    let mut o = match parse(args, mode, out, err) {
        Parsed::Run(o) => o,
        Parsed::Exit(code) => return code,
    };
    for n in std::mem::take(&mut o.notices) {
        print_notice(err, &n);
    }
    if o.input.is_none() {
        let _ = writeln!(err, "Input filename not found\n");
        let _ = out.write_all(USAGE.as_bytes());
        return 2;
    }
    match execute(&o, mode, out, err) {
        Ok(()) => 0,
        Err(Failure::Error(message)) => {
            let _ = writeln!(err, "Error: {message}");
            1
        }
        Err(Failure::Printed) => 1,
    }
}

/// Threads: `OMP_NUM_THREADS` if set, else all logical CPUs.
fn threads() -> usize {
    std::env::var("OMP_NUM_THREADS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// `name.mat`, or the first of `name.mat+`, `name.mat++`, ... that does not exist yet.
fn mat_directory(out: &str) -> Result<PathBuf, String> {
    let mut name = format!("{out}.mat");
    for _ in 0..1000 {
        match fs::create_dir(&name) {
            Ok(()) => return Ok(PathBuf::from(name)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => name.push('+'),
            Err(e) => return Err(format!("cannot create {name}: {e}")),
        }
    }
    Err(format!("cannot create a directory for {out}.mat"))
}

fn write_text(path: impl AsRef<Path>, text: &str) -> Result<(), String> {
    let path = path.as_ref();
    fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Why [`execute`] stopped: an error to print as `Error: <message>`, or a message already
/// printed in the original's words.
enum Failure {
    Error(String),
    Printed,
}

impl From<String> for Failure {
    fn from(message: String) -> Failure {
        Failure::Error(message)
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Failure {
        Failure::Error(message.to_owned())
    }
}

/// FSL-60: the files of the image file name `name` (with its extension) as opened: by default
/// the name given; with `reproduce`, the header (or single) file under FSL 6.0.7.23's name, the
/// last `.img` of `name` made `.hdr` (§4.1). Named in both modes where the two differ.
fn opened_files(
    name: &str,
    files: ImageFiles,
    mode: UpstreamDefects,
    err: &mut dyn Write,
) -> ImageFiles {
    let Some(renamed) = image_files::fsl60_header(name) else {
        return files;
    };
    let detail = match mode {
        UpstreamDefects::Fix => {
            format!("{name} opened as given; FSL 6.0.7.23 opens {renamed} instead")
        }
        UpstreamDefects::Reproduce => {
            format!("{name} opened as {renamed}, as FSL 6.0.7.23 opens it")
        }
    };
    print_notice(err, &Notice::new(&defects::FSL_60, mode, detail));
    if mode.fix() {
        return files;
    }
    ImageFiles {
        header: PathBuf::from(renamed),
        data: files.data,
    }
}

/// The files of the input image `name` (§4.1): completed when it has no image extension (FSL's
/// errors), then [`opened_files`].
fn input_files(
    name: &str,
    mode: UpstreamDefects,
    err: &mut dyn Write,
) -> Result<ImageFiles, String> {
    let resolved = image_files::resolve_input(name)?;
    let files = ImageFiles::of(&resolved).ok_or_else(|| format!("{resolved}: not an image"))?;
    Ok(opened_files(&resolved, files, mode, err))
}

fn execute(
    o: &Options,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), Failure> {
    let n_threads = threads();
    let input = o.input.as_deref().unwrap_or_default();
    let out_name = o
        .out
        .clone()
        .unwrap_or_else(|| format!("{}_mcf", image::strip_image_extension(input)));
    let output_type = std::env::var("FSLOUTPUTTYPE").unwrap_or_else(|_| "NIFTI_GZ".into());
    // An unknown FSLOUTPUTTYPE: FSL 6.0.7.23's message, before any work (mcflirt prints it after
    // the estimation and the .mat and .par files, and exits 1).
    let Some((image_path, version)) = output_name(&out_name, &output_type) else {
        let _ = writeln!(err, "{}", image::unknown_output_type_message(&output_type));
        return Err(Failure::Printed);
    };
    let ext = image_path[image::strip_image_extension(&image_path).len()..].to_owned();
    if o.gdt {
        return Err("-gdt (registration of gradient images) is not supported".into());
    }
    note(
        o,
        err,
        &format!("Processed data will be saved as {out_name}\n"),
    );
    note(o, err, "larmorx mri hmc - head-motion correction\n");
    note(o, err, "Reading time series... ");
    let files = input_files(input, mode, err)?;
    let series = read_series_files(&files, n_threads).map_err(|e| e.to_string())?;
    let n = series.volumes.len();
    let reference_volume;
    let choice = if o.meanvol && o.reffile.is_none() {
        ReferenceChoice::Mean(Some(reference_index(o.refvol, n)?))
    } else if let Some(path) = &o.reffile {
        if o.stages <= 0 {
            return Err("-stages 0 with a reference file: nothing would be registered".into());
        }
        let files = input_files(path, mode, err)?;
        let (vol, s, multiple) =
            read_reference_files(&files, n_threads).map_err(|e| e.to_string())?;
        if multiple {
            let _ = writeln!(
                err,
                "Warning: An input intended to be a single 3D volume has multiple timepoints. \
                 Input will be truncated to first volume, but this functionality is deprecated \
                 and will be removed in a future release."
            );
        }
        let geometry = pipeline::series_geometry(&Series {
            volumes: vec![vol.clone()],
            ..s
        });
        reference_volume = vol;
        if o.meanvol {
            ReferenceChoice::MeanFrom(&reference_volume, geometry)
        } else {
            ReferenceChoice::External(&reference_volume, geometry)
        }
    } else {
        ReferenceChoice::Index(Some(reference_index(o.refvol, n)?))
    };
    let dof = o.dof.clamp(6, 12) as usize;
    let stages = o.stages.clamp(0, 4) as usize;
    if o.dof != dof as i64 {
        let registered = match choice {
            ReferenceChoice::Index(_) => n - 1,
            _ => n,
        };
        let line = format!("Erroneous dof {} : using {dof} instead\n", o.dof);
        let _ = err.write_all(line.repeat(registered * stages).as_bytes());
    }
    let init = match &o.init {
        Some(path) => Some(
            fs::read_to_string(path)
                .ok()
                .and_then(|t| report::parse_mat(&t))
                .ok_or_else(|| format!("cannot read the matrix in {path}"))?,
        ),
        None => None,
    };
    let options = HmcOptions {
        estimate: EstimateParams {
            stages,
            dof,
            cost: o.cost,
            smooth: o.smooth as f32,
            rotation: o.rotation,
            bins: o.bins.max(1) as usize,
            fudge: o.fudge,
            in_plane: if o.two_d {
                InPlane::Force
            } else {
                InPlane::Auto { fov: o.fov }
            },
            n_threads,
            upstream_defects: mode,
            normcorr_count: Default::default(),
        },
        interpolation: o.interpolation,
        init,
        resample: true,
    };
    note(o, err, "Registering volumes ...");
    let result = pipeline::run(&series, &choice, &options).map_err(|e| e.to_string())?;
    for n in &result.estimate.notices {
        print_notice(err, n);
    }
    note(o, err, "Saving motion corrected time series... ");
    if let Some(r) = result.estimate.reference_index
        && o.report
        && (o.mats || o.plots || o.rmsrel || o.rmsabs)
    {
        let _ = writeln!(out, "refnum = {r}\nOriginal_refvol = {}", o.refvol);
    }
    for n in write_outputs(
        o,
        mode,
        &out_name,
        &ext,
        version,
        &series,
        &result,
        options.init.as_ref(),
        n_threads,
        err,
    )? {
        print_notice(err, &n);
    }
    Ok(())
}

/// A progress message on stderr with `-report`.
fn note(o: &Options, err: &mut dyn Write, msg: &str) {
    if o.report {
        let _ = writeln!(err, "{msg}");
    }
}

/// The reference index of `-refvol` (`-1`: the default `N / 2`).
fn reference_index(refvol: i64, n: usize) -> Result<usize, String> {
    match refvol {
        -1 => Ok(n / 2),
        r if r >= 0 && (r as usize) < n => Ok(r as usize),
        r => Err(format!(
            "-refvol {r}: invalid volume index for a series of {n} volumes"
        )),
    }
}

/// Writes the files of §10: the `.mat` directory, `.par`, `.rms`, the corrected series, the
/// `-stats` images and the mean of `-meanvol`. Returns the upstream defects that changed them
/// (FSL-08, -09, -10, -13), or would have; FSL-60's notices (an image name holding `.img`) are
/// printed on `err` as each image is opened.
#[allow(clippy::too_many_arguments)]
fn write_outputs(
    o: &Options,
    mode: UpstreamDefects,
    out_name: &str,
    ext: &str,
    version: larmorx_io::nifti::NiftiVersion,
    series: &Series,
    result: &HmcOutput,
    init: Option<&crate::hmc::rigid::Mat4>,
    n_threads: usize,
    err: &mut dyn Write,
) -> Result<Vec<Notice>, String> {
    let est = &result.estimate;
    let mut notices = Vec::new();
    // FSL-13: -rmsrel and -rmsabs make mcflirt create the .mat directory.
    if (o.rmsrel || o.rmsabs) && !o.mats {
        let detail = match mode {
            UpstreamDefects::Fix => {
                "without -mats no .mat directory is created; mcflirt creates one for -rmsrel and \
                 -rmsabs"
            }
            UpstreamDefects::Reproduce => {
                "-rmsrel and -rmsabs create the .mat directory without -mats, as in mcflirt"
            }
        };
        notices.push(Notice::new(&defects::FSL_13, mode, detail));
    }
    if o.mats || ((o.rmsrel || o.rmsabs) && mode.reproduce()) {
        let dir = mat_directory(out_name)?;
        if o.mats {
            for (t, m) in est.matrices.iter().enumerate() {
                write_text(dir.join(format!("MAT_{t:04}")), &mat_text(m))?;
            }
        } else if let Some(r) = est.reference_index {
            write_text(dir.join(format!("MAT_{r:04}")), &mat_text(&IDENTITY))?;
        }
    }
    if o.plots {
        write_text(format!("{out_name}.par"), &par_text(&result.params))?;
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    if o.rmsabs {
        write_text(format!("{out_name}_abs.rms"), &rms_text(&result.rms_abs))?;
        write_text(
            format!("{out_name}_abs_mean.rms"),
            &rms_text(&[mean(&result.rms_abs)]),
        )?;
    }
    if o.rmsrel {
        write_text(format!("{out_name}_rel.rms"), &rms_text(&result.rms_rel))?;
        write_text(
            format!("{out_name}_rel_mean.rms"),
            &rms_text(&[mean(&result.rms_rel)]),
        )?;
    }
    // Corrected data on another grid of the same size are stored under the input's header.
    let shape = series.shape();
    let voxel = series.voxel_size();
    let corrected: Vec<Volume> = result
        .corrected
        .iter()
        .flatten()
        .map(|v| Volume::new(shape, voxel, v.data.clone()))
        .collect();
    // Only the corrected series gets its own cal_min/cal_max (§10.1); the others keep the
    // input's (observed: -stats and -meanvol images carry the input's values).
    let mut write = |path: &str, vols: &[Volume], dt: DataType, cal: bool| {
        let files = ImageFiles::of(path).ok_or_else(|| format!("{path}: not an image"))?;
        let files = opened_files(path, files, mode, err);
        let written = image::write_like_files(
            &files,
            &series.header,
            version,
            series.flipped,
            vols,
            dt,
            DESCRIP,
            cal,
            n_threads,
            mode,
        )
        .map_err(|e| e.to_string())?;
        for n in written {
            defects::push_once(&mut notices, n);
        }
        Ok::<(), String>(())
    };
    let stem = image::strip_image_extension(out_name);
    let data_type = output_type(series.stored, &series.header, mode);
    write(&format!("{stem}{ext}"), &corrected, data_type, true)?;
    // FSL-08: mcflirt names the -stats and -meanvol images after the -out string with its
    // extension inside (`x.nii.gz_meanvol.nii.gz`), and leaves the -stats images' first and
    // last slices at 0.
    let derived = |suffix: &str| match mode {
        UpstreamDefects::Fix => format!("{stem}_{suffix}{ext}"),
        UpstreamDefects::Reproduce => format!("{out_name}_{suffix}{ext}"),
    };
    if o.stats {
        let [m, variance, sigma] = stats::temporal(&corrected, mode.reproduce());
        for (suffix, vol) in [("meanvol", m), ("variance", variance), ("sigma", sigma)] {
            write(
                &derived(suffix),
                std::slice::from_ref(&vol),
                DataType::F32,
                false,
            )?;
        }
    }
    if let Some(m) = &est.mean {
        write(
            &derived("mean_reg"),
            std::slice::from_ref(m),
            DataType::F32,
            false,
        )?;
    }
    if o.stats || (est.mean.is_some() && stem != out_name) {
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "the -stats and -meanvol images are named after '{stem}' and the -stats images \
                 cover every slice; mcflirt names them after '{out_name}' and leaves the first \
                 and last slices at 0"
            ),
            UpstreamDefects::Reproduce => format!(
                "the -stats and -meanvol images are named after '{out_name}' and the -stats \
                 images leave the first and last slices at 0, as in mcflirt"
            ),
        };
        notices.push(Notice::new(&defects::FSL_08, mode, detail));
    }
    // FSL-59 corrected: the series written is the interpolation asked for, mcflirt 6.0.7.23's
    // the trilinear one. FSL-09 (mcflirt's truncation to an integer type) is named as it
    // applies to mcflirt's series, so in both modes alike, also where the series written (a
    // nearest-neighbour one) holds only integers.
    let fsl59 = est.notices.iter().any(|n| n.defect.id == "FSL-59");
    let named09 = notices.iter().any(|n| n.defect.id == "FSL-09");
    if mode.fix()
        && fsl59
        && !named09
        && let (Some((lo, hi)), Some(first)) = (image::integer_range(data_type), corrected.first())
    {
        let c = resample::fold_resampled(
            &series.volumes,
            &est.matrices,
            init,
            first.shape,
            first.voxel_size,
            Interpolation::Trilinear,
            n_threads,
            |v| image::conversion_counts(&v.data, lo, hi),
        )
        .map_err(|e| e.to_string())?;
        if c != image::Conversion::default() {
            let path = format!("{stem}{ext}");
            notices.push(image::fsl09_notice(Path::new(&path), c, data_type, mode));
        }
    }
    Ok(notices)
}

/// The `descrip` field of output images.
const DESCRIP: &str = "larmorx mri hmc";

#[cfg(test)]
mod tests {
    use super::*;

    fn run_mode(args: &[&str], mode: UpstreamDefects) -> (u8, String, String) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = main_with(&args, mode, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn run(args: &[&str]) -> (u8, String, String) {
        run_mode(args, UpstreamDefects::Reproduce)
    }

    fn parsed(args: &[&str], mode: UpstreamDefects) -> Result<Box<Options>, (u8, String)> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        match parse(&args, mode, &mut out, &mut err) {
            Parsed::Run(o) => Ok(o),
            Parsed::Exit(code) => Err((code, String::from_utf8(err).unwrap())),
        }
    }

    const FIX: UpstreamDefects = UpstreamDefects::Fix;
    const REPRODUCE: UpstreamDefects = UpstreamDefects::Reproduce;

    #[test]
    fn usage_and_errors() {
        let (code, out, _) = run(&[]);
        assert_eq!(code, 1);
        assert!(out.starts_with("Usage: larmorx mri hmc"));
        // FSL-07: -help alone exits 1 in mcflirt, 0 corrected; both print the usage.
        let (code, out, err) = run(&["-help"]);
        assert_eq!(code, 1);
        assert!(out.starts_with("Usage") && err.contains("FSL-07"), "{err}");
        let (code, out, err) = run_mode(&["-help"], FIX);
        assert_eq!(code, 0);
        assert!(out.starts_with("Usage") && err.contains("FSL-07 (mcflirt) corrected"));
        let (code, out, _) = run(&["-in", "x.nii", "-help"]);
        assert_eq!(code, 0);
        assert!(out.starts_with("Usage"));
        let (code, out, err) = run(&["-out", "o", "-mats"]);
        assert_eq!(code, 2);
        assert!(err.starts_with("Input filename not found"));
        assert!(out.starts_with("Usage"));
        let (code, _, err) = run(&["-in", "x.nii", "-mats", "-edge"]);
        assert_eq!(code, 255);
        assert_eq!(err, "Lacking argument to option -edge\n");
        let (code, _, err) = run(&["-in", "x.nii", "-edge", "-out", "y"]);
        assert_eq!(code, 255);
        assert_eq!(err, "Unrecognised option -edge\n");
        let (code, _, err) = run(&["-in", "x.nii", "-mats", "-refvol"]);
        assert_eq!(code, 255);
        assert_eq!(err, "Lacking argument to option -refvol\n");
        // Corrected, -edge says where it comes from; the exit code is mcflirt's.
        let (code, _, err) = run_mode(&["-in", "x.nii", "-edge", "-out", "y"], FIX);
        assert_eq!(code, 255);
        assert!(
            err.starts_with("Unrecognised option -edge\n(-edge is not"),
            "{err}"
        );
    }

    /// FSL-06: an unknown cost is an error when corrected, normcorr (with a notice) as mcflirt.
    #[test]
    fn unknown_costs_both_modes() {
        let args = ["-in", "x.nii", "-cost", "bogus"];
        let o = parsed(&args, REPRODUCE).unwrap();
        assert_eq!(o.cost, CostFunction::NormCorr);
        assert_eq!(o.notices[0].defect.id, "FSL-06");
        let (code, err) = parsed(&args, FIX).map(|_| ()).unwrap_err();
        assert_eq!(code, 255);
        assert!(
            err.contains("Unrecognised cost function type: bogus"),
            "{err}"
        );
        assert_eq!(
            parsed(&["-in", "x", "-cost", "woods"], FIX).unwrap().cost,
            CostFunction::Woods
        );
    }

    /// FSL-07: numbers are read with atoi/atof as mcflirt (with a notice when the text is not
    /// a plain number), strictly when corrected.
    #[test]
    fn numbers_both_modes() {
        for (args, expect) in [
            (["-in", "x", "-refvol", "abc"], 0.0),
            (["-in", "x", "-stages", "1.5"], 1.0),
            (["-in", "x", "-smooth", "2mm"], 2.0),
        ] {
            let o = parsed(&args, REPRODUCE).unwrap();
            let got = [o.refvol as f64, o.stages as f64, o.smooth].contains(&expect);
            assert!(got, "{args:?}");
            assert_eq!(o.notices[0].defect.id, "FSL-07", "{args:?}");
            let (code, err) = parsed(&args, FIX).map(|_| ()).unwrap_err();
            assert_eq!(code, 255, "{args:?}");
            assert!(err.contains("needs"), "{err}");
        }
        // Plain numbers: the same in both modes, and nothing said.
        for mode in [FIX, REPRODUCE] {
            let o = parsed(
                &["-in", "x", "-refvol", "+3", "-smooth", "0.5", "-dof", " 7"],
                mode,
            )
            .unwrap();
            assert_eq!((o.refvol, o.smooth, o.dof), (3, 0.5, 7));
            assert!(o.notices.is_empty());
        }
    }

    /// FSL-05: -scaling and -hist have no effect; a notice says so in both modes.
    #[test]
    fn ignored_options_both_modes() {
        for mode in [FIX, REPRODUCE] {
            let o = parsed(&["-in", "x", "-scaling", "6", "-hist"], mode).unwrap();
            assert_eq!(
                o.notices
                    .iter()
                    .map(|n| (n.defect.id, n.mode))
                    .collect::<Vec<_>>(),
                [("FSL-05", mode), ("FSL-05", mode)]
            );
        }
    }

    #[test]
    fn reference_indices() {
        assert_eq!(reference_index(-1, 12), Ok(6));
        assert_eq!(reference_index(0, 12), Ok(0));
        assert!(reference_index(12, 12).is_err());
        assert!(reference_index(-2, 12).is_err());
    }
}
