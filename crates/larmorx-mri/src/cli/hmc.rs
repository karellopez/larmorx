// SPDX-License-Identifier: Apache-2.0
//! `larmorx mri hmc`: head-motion correction accepting mcflirt-style options
//! (`specs/mcflirt.md` §10–§12).
//!
//! Deliberate differences from mcflirt (documented in `docs/api/mri-hmc.md`): without
//! `FSLOUTPUTTYPE` the output is `.nii.gz` (mcflirt fails); impossible inputs (a reference
//! index out of range, a reference grid with another number of voxels, `-stages 0` with a
//! reference file) are errors reported before any work, with exit code 1 (mcflirt aborts);
//! `-gdt` is not supported.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::element::DataType;

use super::{atof, atoi};
use crate::hmc::estimate::{EstimateParams, Reference, estimate};
use crate::hmc::image::{self, Series, output_name, output_type, read_reference, read_series};
use crate::hmc::report::{self, mat_text, par_text, rms_text};
use crate::hmc::resample::{Interpolation, resample_series};
use crate::hmc::rigid::{IDENTITY, Mat4};
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
        }
    }
}

/// What parsing decided.
enum Parsed {
    Run(Box<Options>),
    Exit(u8),
}

fn parse(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Parsed {
    let mut o = Options::default();
    let mut i = 0;
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
                _ => {} // -v, -hist: no effect on the results
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
            return Parsed::Exit(255);
        }
        let v = args[i + 1].as_str();
        match a {
            "-in" => o.input = Some(v.to_owned()),
            "-out" | "-o" => o.out = Some(v.to_owned()),
            "-reffile" | "-r" => o.reffile = Some(v.to_owned()),
            "-refvol" => o.refvol = atoi(v),
            "-stages" => o.stages = atoi(v),
            "-dof" => o.dof = atoi(v),
            "-cost" => match CostFunction::from_name(v).filter(|c| *c != CostFunction::Pearson) {
                Some(c) => o.cost = c,
                None => {
                    if o.report {
                        let _ = writeln!(
                            err,
                            "Unrecognised cost function type: {v}\nUsing the default (NormCorr)"
                        );
                    }
                    o.cost = CostFunction::NormCorr;
                }
            },
            "-bins" => o.bins = atoi(v),
            "-smooth" => o.smooth = atof(v),
            "-rotation" => o.rotation = atof(v),
            "-fov" => o.fov = atoi(v),
            "-init" => o.init = Some(v.to_owned()),
            _ => {} // -scaling, -verbose: no effect on the results
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

/// Runs `larmorx mri hmc` with `args` (mcflirt-style options); returns the exit code.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if args.len() < 2 {
        let _ = out.write_all(USAGE.as_bytes());
        return 1;
    }
    let o = match parse(args, out, err) {
        Parsed::Run(o) => o,
        Parsed::Exit(code) => return code,
    };
    if o.input.is_none() {
        let _ = writeln!(err, "Input filename not found\n");
        let _ = out.write_all(USAGE.as_bytes());
        return 2;
    }
    match execute(&o, out, err) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(err, "Error: {message}");
            1
        }
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

fn execute(o: &Options, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), String> {
    let n_threads = threads();
    let input = o.input.as_deref().unwrap_or_default();
    let out_name = o
        .out
        .clone()
        .unwrap_or_else(|| format!("{}_mcf", image::strip_image_extension(input)));
    let output_type = std::env::var("FSLOUTPUTTYPE").unwrap_or_else(|_| "NIFTI_GZ".into());
    let (image_path, version) = output_name(&out_name, &output_type)
        .ok_or_else(|| format!("unknown FSLOUTPUTTYPE {output_type:?}"))?;
    let ext = &image_path[image::strip_image_extension(&image_path).len()..];
    if o.gdt {
        return Err("-gdt (registration of gradient images) is not supported".into());
    }
    note(o, err, &format!("Processed data will be saved as {out_name}\n"));
    note(o, err, "larmorx mri hmc - head-motion correction\n");
    note(o, err, "Reading time series... ");
    let series = read_series(Path::new(input), n_threads).map_err(|e| e.to_string())?;
    let n = series.volumes.len();
    let shape = series.shape();
    let (reference, ref_grid, ref_index) = if o.meanvol {
        let r = default_index(o.refvol, n)?;
        (Reference::Mean(r), None, None)
    } else if let Some(path) = &o.reffile {
        if o.stages <= 0 {
            return Err("-stages 0 with a reference file: nothing would be registered".into());
        }
        let (vol, s, multiple) =
            read_reference(Path::new(path), n_threads).map_err(|e| e.to_string())?;
        if multiple {
            let _ = writeln!(
                err,
                "Warning: An input intended to be a single 3D volume has multiple timepoints. \
                 Input will be truncated to first volume, but this functionality is deprecated \
                 and will be removed in a future release."
            );
        }
        if vol.len() != series.volumes[0].len() {
            return Err(format!(
                "the reference has {} voxels, the volumes of the series {}: the corrected \
                 series cannot be written",
                vol.len(),
                series.volumes[0].len()
            ));
        }
        (Reference::Volume(vol), Some(s), None)
    } else {
        let r = default_index(o.refvol, n)?;
        (Reference::Index(r), None, Some(r))
    };
    let dof = o.dof.clamp(6, 12) as usize;
    if o.dof != dof as i64 {
        let registered = if ref_index.is_some() { n - 1 } else { n };
        let line = format!("Erroneous dof {} : using {dof} instead\n", o.dof);
        let _ = err.write_all(line.repeat(registered * o.stages.clamp(0, 4) as usize).as_bytes());
    }
    let params = EstimateParams {
        stages: o.stages.clamp(0, 4) as usize,
        dof,
        cost: o.cost,
        smooth: o.smooth as f32,
        rotation: o.rotation,
        bins: o.bins.max(1) as usize,
        fudge: o.fudge,
        n_threads,
    };
    note(o, err, "Registering volumes ...");
    let est = estimate(&series.volumes, &reference, &params).map_err(|e| e.to_string())?;
    let init = match &o.init {
        Some(path) => Some(
            fs::read_to_string(path)
                .ok()
                .and_then(|t| report::parse_mat(&t))
                .ok_or_else(|| format!("cannot read the matrix in {path}"))?,
        ),
        None => None,
    };
    // The output grid: the reference's with a reference file, else the series'.
    let (grid_shape, grid_voxel) = match &reference {
        Reference::Volume(v) => (v.shape, v.voxel_size),
        _ => (shape, series.volumes[0].voxel_size),
    };
    note(o, err, "Saving motion corrected time series... ");
    let corrected = resample_series(
        &series.volumes,
        &est.matrices,
        init.as_ref(),
        grid_shape,
        grid_voxel,
        o.interpolation,
        n_threads,
    )
    .map_err(|e| e.to_string())?;
    // Corrected data on another grid of the same size are stored under the input's header.
    let corrected: Vec<Volume> = corrected
        .into_iter()
        .map(|v| Volume::new(shape, series.volumes[0].voxel_size, v.data))
        .collect();
    if o.report && (o.mats || o.plots || o.rmsrel || o.rmsabs) {
        if let Some(r) = ref_index {
            let _ = writeln!(out, "refnum = {r}\nOriginal_refvol = {}", o.refvol);
        }
    }
    write_outputs(o, &out_name, ext, version, &series, &est, ref_grid.as_ref(), &corrected, n_threads)
}

/// A progress message on stderr with `-report`.
fn note(o: &Options, err: &mut dyn Write, msg: &str) {
    if o.report {
        let _ = writeln!(err, "{msg}");
    }
}

/// The reference index of `-refvol` (`-1`: the default `N / 2`).
fn default_index(refvol: i64, n: usize) -> Result<usize, String> {
    match refvol {
        -1 => Ok(n / 2),
        r if r >= 0 && (r as usize) < n => Ok(r as usize),
        r => Err(format!(
            "-refvol {r}: invalid volume index for a series of {n} volumes"
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn write_outputs(
    o: &Options,
    out_name: &str,
    ext: &str,
    version: larmorx_io::nifti::NiftiVersion,
    series: &Series,
    est: &crate::hmc::Estimate,
    _ref_series: Option<&Series>,
    corrected: &[Volume],
    n_threads: usize,
) -> Result<(), String> {
    let matrices: &[Mat4] = &est.matrices;
    if o.mats || o.rmsrel || o.rmsabs {
        let dir = mat_directory(out_name)?;
        if o.mats {
            for (t, m) in matrices.iter().enumerate() {
                write_text(dir.join(format!("MAT_{t:04}")), &mat_text(m))?;
            }
        } else if let Some(r) = est.reference_index {
            write_text(dir.join(format!("MAT_{r:04}")), &mat_text(&IDENTITY))?;
        }
    }
    if o.plots {
        let params = report::motion_parameters(matrices, &est.reference);
        write_text(format!("{out_name}.par"), &par_text(&params))?;
    }
    if o.rmsabs || o.rmsrel {
        let (abs, rel) = report::rms_series(matrices, &est.reference);
        if o.rmsabs {
            let mean = abs.iter().sum::<f64>() / abs.len() as f64;
            write_text(format!("{out_name}_abs.rms"), &rms_text(&abs))?;
            write_text(format!("{out_name}_abs_mean.rms"), &rms_text(&[mean]))?;
        }
        if o.rmsrel {
            let mean = rel.iter().sum::<f64>() / rel.len() as f64;
            write_text(format!("{out_name}_rel.rms"), &rms_text(&rel))?;
            write_text(format!("{out_name}_rel_mean.rms"), &rms_text(&[mean]))?;
        }
    }
    let data_type = output_type(series.stored, &series.header);
    let stem = image::strip_image_extension(out_name);
    let write = |path: &str, vols: &[Volume], dt: DataType| {
        image::write_like(
            Path::new(path),
            &series.header,
            version,
            series.flipped,
            vols,
            dt,
            DESCRIP,
            n_threads,
        )
        .map_err(|e| e.to_string())
    };
    write(&format!("{stem}{ext}"), corrected, data_type)?;
    if o.stats {
        let [mean, variance, sigma] = stats::temporal(corrected);
        write(&format!("{out_name}_meanvol{ext}"), &[mean], DataType::F32)?;
        write(&format!("{out_name}_variance{ext}"), &[variance], DataType::F32)?;
        write(&format!("{out_name}_sigma{ext}"), &[sigma], DataType::F32)?;
    }
    if let Some(mean) = &est.mean {
        write(&format!("{out_name}_mean_reg{ext}"), std::slice::from_ref(mean), DataType::F32)?;
    }
    Ok(())
}

/// The `descrip` field of output images.
const DESCRIP: &str = "larmorx mri hmc";

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> (u8, String, String) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = main(&args, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn usage_and_errors() {
        let (code, out, _) = run(&[]);
        assert_eq!(code, 1);
        assert!(out.starts_with("Usage: larmorx mri hmc"));
        let (code, _, _) = run(&["-help"]);
        assert_eq!(code, 1);
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
    }

    #[test]
    fn reference_indices() {
        assert_eq!(default_index(-1, 12), Ok(6));
        assert_eq!(default_index(0, 12), Ok(0));
        assert!(default_index(12, 12).is_err());
        assert!(default_index(-2, 12).is_err());
    }
}
