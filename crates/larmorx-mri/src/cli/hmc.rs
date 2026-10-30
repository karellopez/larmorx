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
use crate::hmc::estimate::{EstimateError, EstimateParams, InPlane, refused_bins};
use crate::hmc::files::{self as image_files, ImageFiles};
use crate::hmc::image::{
    self, Series, output_name, output_type, read_reference_files, read_series_files,
};
use crate::hmc::pipeline::{self, HmcError, HmcOptions, HmcOutput, ReferenceChoice};
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
        Err(Failure::Exit(code)) => code,
    }
}

/// Threads: `OMP_NUM_THREADS` if set, else all logical CPUs.
fn threads() -> usize {
    std::env::var("OMP_NUM_THREADS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// Why the `.mat` directory (§10.2) could not be made: its name as tried, and whether its
/// parent refused the permission (mcflirt's message and run go on) or something else failed
/// (FSL-20: a parent directory that does not exist or is a file).
struct MatDirError {
    name: String,
    permission: bool,
    error: std::io::Error,
}

/// `name.mat`, or the first of `name.mat+`, `name.mat++`, ... that does not exist yet.
fn mat_directory(out: &str) -> Result<PathBuf, MatDirError> {
    let mut name = format!("{out}.mat");
    loop {
        match fs::create_dir(&name) {
            Ok(()) => return Ok(PathBuf::from(name)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && name.len() < 4096 => {
                name.push('+');
            }
            Err(error) => {
                return Err(MatDirError {
                    permission: error.kind() == std::io::ErrorKind::PermissionDenied,
                    name,
                    error,
                });
            }
        }
    }
}

/// FSL-20 corrected without `-mats` (FSL-13 corrected: no `.mat` directory is made): whether
/// mcflirt could make `<out>.mat`, i.e. its parent is a directory. Its permission is not
/// checked (mcflirt then only prints a message).
fn mat_parent_exists(out: &str) -> Result<(), MatDirError> {
    let name = format!("{out}.mat");
    match Path::new(&name).parent() {
        Some(p) if !p.as_os_str().is_empty() && !p.is_dir() => Err(MatDirError {
            name: name.clone(),
            permission: false,
            error: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                if p.exists() {
                    format!("{} is not a directory", p.display())
                } else {
                    format!("{} does not exist", p.display())
                },
            ),
        }),
        _ => Ok(()),
    }
}

fn write_text(path: impl AsRef<Path>, text: &str) -> Result<(), String> {
    let path = path.as_ref();
    fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Why [`execute`] stopped: an error to print as `Error: <message>`, a message already
/// printed in the original's words (exit status 1), or mcflirt's own end of the run
/// reproduced, its messages printed (FSL-20, FSL-65: exit status 0).
enum Failure {
    Error(String),
    Printed,
    Exit(u8),
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
    // FSL-21 and FSL-66 corrected (rule B, specs/mcflirt.md §12.1): bins no stage can use are
    // refused before any work, whatever the series.
    let stages = o.stages.clamp(0, 4) as usize;
    if mode.fix()
        && let Some(r) = refused_bins(o.bins, stages, o.cost)
    {
        print_notice(err, &r.notice());
        return Err(Failure::Error(r.message));
    }
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
            bins: o.bins,
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
    let result = match pipeline::run(&series, &choice, &options) {
        Ok(r) => r,
        // FSL-21 and FSL-66 reproduced: where mcflirt crashes, its messages, then an error
        // (a crash is never reproduced, D13).
        Err(HmcError::Estimate(EstimateError::McflirtEnds(end))) => {
            for line in end.mcflirt_lines() {
                let _ = writeln!(err, "{line}");
            }
            print_notice(err, &end.notice());
            return Err(Failure::Error(end.error()));
        }
        Err(e) => return Err(e.to_string().into()),
    };
    for n in &result.estimate.notices {
        print_notice(err, n);
    }
    let mut notices = Vec::new();
    let written = write_outputs(
        o,
        mode,
        &out_name,
        &ext,
        version,
        &series,
        &result,
        options.init.as_ref(),
        n_threads,
        out,
        err,
        &mut notices,
    );
    for n in &notices {
        print_notice(err, n);
    }
    written
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

/// A text output of §10.2–§10.4 that could not be opened (FSL-65): its name and why.
struct Unopened {
    name: String,
    error: std::io::Error,
}

/// Opens (creates) the text file `name`.
fn open_text(name: &str) -> Result<fs::File, Unopened> {
    fs::File::create(name).map_err(|error| Unopened {
        name: name.to_owned(),
        error,
    })
}

/// The `.par` and `.rms` files (§10.3, §10.4), as `(name, text)` in mcflirt's order of writing:
/// `_rel.rms`, `_rel_mean.rms`, `_abs.rms`, `_abs_mean.rms`.
fn rms_files(o: &Options, out_name: &str, result: &HmcOutput) -> Vec<(String, String)> {
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let mut files = Vec::new();
    if o.rmsrel {
        files.push((format!("{out_name}_rel.rms"), rms_text(&result.rms_rel)));
        files.push((
            format!("{out_name}_rel_mean.rms"),
            rms_text(&[mean(&result.rms_rel)]),
        ));
    }
    if o.rmsabs {
        files.push((format!("{out_name}_abs.rms"), rms_text(&result.rms_abs)));
        files.push((
            format!("{out_name}_abs_mean.rms"),
            rms_text(&[mean(&result.rms_abs)]),
        ));
    }
    files
}

/// The prefix mcflirt puts before the `.mat` directory's name in its permission message: the
/// `-in` string up to and including its last `/`, or, without one, its first character
/// (`specs/mcflirt.md` §10.6).
fn input_prefix(input: &str) -> &str {
    match input.rfind('/') {
        Some(i) => &input[..=i],
        None => input
            .char_indices()
            .nth(1)
            .map_or(input, |(i, _)| &input[..i]),
    }
}

/// FSL-20 corrected: the error of a `.mat` directory that cannot be made (`specs/mcflirt.md`
/// §10.6), with its notice where it is FSL-20's case (not the permission).
fn mat_dir_failure(e: &MatDirError, notices: &mut Vec<Notice>) -> Failure {
    if e.permission {
        return Failure::Error(format!(
            "cannot create the directory {} for the matrices: {}",
            e.name, e.error
        ));
    }
    notices.push(Notice::new(
        &defects::FSL_20,
        UpstreamDefects::Fix,
        format!(
            "the directory {} cannot be created ({}): an error (exit status 1); mcflirt prints \
             an error and exits with status 0, writing nothing more",
            e.name, e.error
        ),
    ));
    Failure::Error(format!(
        "cannot create the directory {} for the matrices (FSL-20: mcflirt prints \"Error!: \
         Unknown Log directory return value\" and exits with status 0)",
        e.name
    ))
}

/// FSL-65 corrected: the error of a `.par` or `.rms` file that cannot be opened, with its
/// notice.
fn unopened_failure(u: &Unopened, par: bool, notices: &mut Vec<Notice>) -> Failure {
    let mcflirt = if par {
        "mcflirt prints \"error: unable to open output file!\" at the first volume and exits with \
         status 0, after the first matrix and the .rms files"
    } else {
        "mcflirt silently does not write it and exits with status 0"
    };
    notices.push(Notice::new(
        &defects::FSL_65,
        UpstreamDefects::Fix,
        format!(
            "{} cannot be opened ({}): an error (exit status 1); {mcflirt}",
            u.name, u.error
        ),
    ));
    Failure::Error(format!(
        "cannot open {} for writing: {} (FSL-65: {mcflirt})",
        u.name, u.error
    ))
}

/// Writes the files of §10 in mcflirt's order (§10.6): the mean of `-meanvol`, the `-stats`
/// images, the `refnum` lines (`-report`), the text outputs (the `.par` file opened, the `.mat`
/// directory made, the `.rms` files, then volume by volume its `MAT_` file and `.par` line), the
/// corrected series last. Adds to `notices` the upstream defects that changed them (FSL-08, -09,
/// -10, -13, -20, -65), or would have; FSL-60's notices (an image name holding `.img`) are
/// printed on `err` as each image is opened.
///
/// Outputs that cannot be written (§10.6): by default every text output is opened (the `.mat`
/// directory made) before anything is written, and one that cannot be is an error (FSL-20, FSL-65;
/// what this step made is removed); with `reproduce`, mcflirt's run: an uncreatable `.mat`
/// directory ends it with mcflirt's message and exit status 0 (FSL-20), an unopenable `.rms` file
/// is not written and an unopenable `.par` file ends it at volume 0 with status 0 (FSL-65). An
/// image that cannot be written is an error in both modes (mcflirt aborts; with `reproduce` its two
/// lines are printed first).
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
    out: &mut dyn Write,
    err: &mut dyn Write,
    notices: &mut Vec<Notice>,
) -> Result<(), Failure> {
    let est = &result.estimate;
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
    let text_outputs = o.mats || o.plots || o.rmsrel || o.rmsabs;
    // mcflirt makes the .mat directory for -mats, -rmsrel and -rmsabs (FSL-13).
    let mcflirt_mat_dir = o.mats || o.rmsrel || o.rmsabs;
    let rms = rms_files(o, out_name, result);
    let par_name = format!("{out_name}.par");

    // The default: every text output opened before anything is written (FSL-20, FSL-65).
    let mut prepared: Option<(Option<PathBuf>, Option<fs::File>, Vec<fs::File>)> = None;
    if mode.fix() && text_outputs {
        let mut made: Vec<PathBuf> = Vec::new();
        let undo = |made: &[PathBuf]| {
            for p in made.iter().rev() {
                let _ = if p.is_dir() {
                    fs::remove_dir(p)
                } else {
                    fs::remove_file(p)
                };
            }
        };
        if mcflirt_mat_dir && !o.mats {
            mat_parent_exists(out_name).map_err(|e| mat_dir_failure(&e, notices))?;
        }
        let par = if o.plots {
            match open_text(&par_name) {
                Ok(f) => {
                    made.push(PathBuf::from(&par_name));
                    Some(f)
                }
                Err(u) => {
                    // FSL-20 first where mcflirt would meet it (its .mat directory's parent is the
                    // .par file's).
                    if mcflirt_mat_dir && let Err(e) = mat_parent_exists(out_name) {
                        return Err(mat_dir_failure(&e, notices));
                    }
                    return Err(unopened_failure(&u, true, notices));
                }
            }
        } else {
            None
        };
        let mut rms_handles = Vec::new();
        for (name, _) in &rms {
            match open_text(name) {
                Ok(f) => {
                    made.push(PathBuf::from(name));
                    rms_handles.push(f);
                }
                Err(u) => {
                    undo(&made);
                    if mcflirt_mat_dir && let Err(e) = mat_parent_exists(out_name) {
                        return Err(mat_dir_failure(&e, notices));
                    }
                    return Err(unopened_failure(&u, false, notices));
                }
            }
        }
        let dir = if o.mats {
            match mat_directory(out_name) {
                Ok(d) => Some(d),
                Err(e) => {
                    undo(&made);
                    return Err(mat_dir_failure(&e, notices));
                }
            }
        } else {
            None
        };
        prepared = Some((dir, par, rms_handles));
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
    // input's (observed: -stats and -meanvol images carry the input's values). An image that
    // cannot be written is an error; with `reproduce`, after mcflirt's two lines (it aborts).
    let write = |path: &str,
                     vols: &[Volume],
                     dt: DataType,
                     cal: bool,
                     err: &mut dyn Write,
                     notices: &mut Vec<Notice>|
     -> Result<(), Failure> {
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
        );
        match written {
            Ok(w) => {
                for n in w {
                    defects::push_once(notices, n);
                }
                Ok(())
            }
            Err(e) => {
                if mode.reproduce() {
                    let _ = writeln!(
                        err,
                        "terminate called after throwing an instance of \
                         'NiftiIO::NiftiException'\n  what():  Error: cant open file {path}"
                    );
                }
                Err(Failure::Error(format!("cannot write {path}: {e}")))
            }
        }
    };
    let stem = image::strip_image_extension(out_name);
    let data_type = output_type(series.stored, &series.header, mode);
    // FSL-08: mcflirt names the -stats and -meanvol images after the -out string with its
    // extension inside (`x.nii.gz_meanvol.nii.gz`), and leaves the -stats images' first and
    // last slices at 0.
    let derived = |suffix: &str| match mode {
        UpstreamDefects::Fix => format!("{stem}_{suffix}{ext}"),
        UpstreamDefects::Reproduce => format!("{out_name}_{suffix}{ext}"),
    };
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
    if let Some(m) = &est.mean {
        write(
            &derived("mean_reg"),
            std::slice::from_ref(m),
            DataType::F32,
            false,
            err,
            notices,
        )?;
    }
    if o.stats {
        let [m, variance, sigma] = stats::temporal(&corrected, mode.reproduce());
        for (suffix, vol) in [("meanvol", m), ("variance", variance), ("sigma", sigma)] {
            write(
                &derived(suffix),
                std::slice::from_ref(&vol),
                DataType::F32,
                false,
                err,
                notices,
            )?;
        }
    }
    if let Some(r) = est.reference_index
        && o.report
        && text_outputs
    {
        let _ = writeln!(out, "refnum = {r}\nOriginal_refvol = {}", o.refvol);
    }
    let n = est.matrices.len();
    let mat_text_of = |t: usize| -> Option<String> {
        if o.mats {
            Some(mat_text(&est.matrices[t]))
        } else {
            // FSL-13 reproduced: only the reference volume's identity.
            (est.reference_index == Some(t)).then(|| mat_text(&IDENTITY))
        }
    };
    let write_handle = |f: &mut fs::File, name: &str, text: &str| -> Result<(), Failure> {
        f.write_all(text.as_bytes())
            .map_err(|e| Failure::Error(format!("cannot write {name}: {e}")))
    };
    if let Some((dir, mut par, mut rms_handles)) = prepared {
        for ((name, text), f) in rms.iter().zip(rms_handles.iter_mut()) {
            write_handle(f, name, text)?;
        }
        for t in 0..n {
            if let Some(d) = &dir {
                write_text(d.join(format!("MAT_{t:04}")), &mat_text(&est.matrices[t]))?;
            }
            if let Some(f) = par.as_mut() {
                write_handle(f, &par_name, &par_text(&result.params[t..=t]))?;
            }
        }
    } else if text_outputs {
        // mcflirt's run (reproduce), §10.6.
        let mut par = o.plots.then(|| open_text(&par_name));
        let mut dir = None;
        if mcflirt_mat_dir {
            match mat_directory(out_name) {
                Ok(d) => dir = Some(d),
                Err(e) if e.permission => {
                    let _ = writeln!(
                        err,
                        "Error! Could not create directory: {}{}. No write permission",
                        input_prefix(o.input.as_deref().unwrap_or_default()),
                        e.name
                    );
                }
                Err(e) => {
                    let _ = writeln!(err, "Error!: Unknown Log directory return value");
                    notices.push(Notice::new(
                        &defects::FSL_20,
                        mode,
                        format!(
                            "the directory {} cannot be created ({}): mcflirt's message and exit \
                             status 0, nothing more written",
                            e.name, e.error
                        ),
                    ));
                    return Err(Failure::Exit(0));
                }
            }
        }
        for (name, text) in &rms {
            match open_text(name) {
                Ok(mut f) => write_handle(&mut f, name, text)?,
                Err(u) => notices.push(Notice::new(
                    &defects::FSL_65,
                    mode,
                    format!(
                        "{} cannot be opened ({}): not written, as in mcflirt (which says nothing \
                         and exits with status 0)",
                        u.name, u.error
                    ),
                )),
            }
        }
        for t in 0..n {
            if let (Some(d), Some(text)) = (&dir, mat_text_of(t)) {
                write_text(d.join(format!("MAT_{t:04}")), &text)?;
            }
            match par.as_mut() {
                Some(Ok(f)) => write_handle(f, &par_name, &par_text(&result.params[t..=t]))?,
                Some(Err(u)) => {
                    let _ = writeln!(err, "error: unable to open output file!");
                    notices.push(Notice::new(
                        &defects::FSL_65,
                        mode,
                        format!(
                            "{} cannot be opened ({}): mcflirt's message at volume 0 and exit \
                             status 0, nothing more written",
                            u.name, u.error
                        ),
                    ));
                    return Err(Failure::Exit(0));
                }
                None => {}
            }
        }
    }
    note(o, err, "Saving motion corrected time series... ");
    write(
        &format!("{stem}{ext}"),
        &corrected,
        data_type,
        true,
        err,
        notices,
    )?;
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
    Ok(())
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
