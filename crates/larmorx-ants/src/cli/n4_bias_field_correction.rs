// SPDX-License-Identifier: Apache-2.0
//! `N4BiasFieldCorrection` with its original arguments (ANTs v2.6.5,
//! `Examples/N4BiasFieldCorrection.cxx`), through ANTs' command-line parser.
//!
//! ```text
//! N4BiasFieldCorrection [-d 2/3/4] -i input [-x mask] [-w weight] [-r 0/(1)] [-s (4)]
//!     [-c [50x50x50x50,0.0]] [-b [splineDistance|meshResolution,<order=3>]]
//!     [-t [0.15,0.01,200]] [-o corrected | [corrected,biasField]] [-v (0)/1]
//! ```
//!
//! As in ANTs:
//! - option values are read with the parser's strict `Convert<T>`: a value that is not
//!   entirely a number (`-s 2.5`, `-d 3d`, `-r true`) makes ANTs throw and abort; larmorx
//!   prints a message and exits 1;
//! - options that need brackets do nothing without them: `-b 200`, `-c 50x50` and
//!   `-t 0.15` set no parameter. `-c` without parameters leaves ITK's defaults (one level
//!   of 50 iterations, threshold 0.001); without `-c` at all, ANTs' (four levels of 50,
//!   threshold 0) (ANTS-35);
//! - `-c [iterations]` sets the threshold to 0, `-c [iterations,threshold]` to `threshold`;
//! - without `-d`, the dimension is the input file's (ANTs: 3 whatever the file, ANTS-99);
//! - a mask that cannot be read is replaced by an image of ones ("Mask not read"), but
//!   still counts as given: outside-mask restoration and rescaling happen (ANTS-36);
//! - rescaling is on when a mask is given, unless `-r 0`; without a mask `-r 1` does nothing
//!   (ANTS-36).
//!
//! **Upstream defects (D13).** By default larmorx reads an unbracketed value as the option's
//! first parameter (`-b 200` is `-b [200]`, `-c 50x50` is `-c [50x50]`, `-t 0.15` is
//! `-t [0.15]`; ANTS-35), refuses a mask it cannot read, and says that `-r 1` has no effect
//! without a mask (ANTS-36); ITK's filter defects ANTS-38, ANTS-39 and ANTS-40 (the zero and
//! negative voxels N4 uses left out of its log image) are corrected too. With
//! `LARMORX_UPSTREAM_DEFECTS=reproduce` it behaves as ANTs does.
//!
//! `-v` prints ANTs' progress lines; ITK's filter dump (`Print`) is not reproduced. ANTs uses
//! `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` threads and its result depends on it; larmorx uses
//! the same number of threads and gives ANTs' single-thread result
//! (`docs/findings/ants-n4.md`).

use std::io::Write;
use std::time::Instant;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notices, UpstreamDefects};

use super::cstd::{
    format_g, parser_convert_bool, parser_convert_f32, parser_convert_i32, parser_convert_u32,
    parser_convert_vector_f32, parser_convert_vector_u32,
};
use super::parser::{self, OptionSpec, OptionValue, Parsed};
use crate::image::{AntsImage, FileStore, ImageStore, ReadError, read_with, write_with};
use crate::n4_bias_field_correction::{BSplineMesh, N4Error, N4Options, n4_bias_field_correction};

const fn opt(long: &'static str, short: Option<char>) -> OptionSpec {
    OptionSpec { long, short }
}

const SPECS: &[OptionSpec] = &[
    opt("image-dimensionality", Some('d')),
    opt("input-image", Some('i')),
    opt("mask-image", Some('x')),
    opt("rescale-intensities", Some('r')),
    opt("weight-image", Some('w')),
    opt("shrink-factor", Some('s')),
    opt("convergence", Some('c')),
    opt("bspline-fitting", Some('b')),
    opt("histogram-sharpening", Some('t')),
    opt("output", Some('o')),
    opt("version", None),
    opt("verbose", Some('v')),
    opt("h", Some('h')),
    opt("help", None),
];

const USAGE: &str = "\
COMMAND:
     N4BiasFieldCorrection (larmorx port of ANTs v2.6.5)
          N4 is a variant of the popular N3 (nonparameteric nonuniform normalization)
          retrospective bias correction algorithm (N. Tustison et al., N4ITK: Improved N3
          Bias Correction, IEEE Transactions on Medical Imaging, 29(6):1310-1320, June 2010).

OPTIONS:
     -d, --image-dimensionality 2/3/4
     -i, --input-image inputImageFilename
     -x, --mask-image maskImageFilename
     -r, --rescale-intensities 0/(1)
     -w, --weight-image weightImageFilename
     -s, --shrink-factor 1/2/3/(4)/...
     -c, --convergence [<numberOfIterations=50x50x50x50>,<convergenceThreshold=0.0>]
     -b, --bspline-fitting [splineDistance,<splineOrder=3>]
                           [initialMeshResolution,<splineOrder=3>]
     -t, --histogram-sharpening [<FWHM=0.15>,<wienerNoise=0.01>,<numberOfHistogramBins=200>]
     -o, --output correctedImage
                  [correctedImage,<biasField>]
     --version
     -v, --verbose (0)/1
     -h
     --help
";

/// Runs `N4BiasFieldCorrection` on files, in the D13 mode of `LARMORX_UPSTREAM_DEFECTS`;
/// returns the exit code.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match UpstreamDefects::from_env() {
        Ok(mode) => run(args, &mut FileStore, super::itk_threads(), mode, out, err),
        Err(e) => {
            let _ = writeln!(err, "larmorx: {e}");
            EXIT_BAD_MODE
        }
    }
}

/// Runs `N4BiasFieldCorrection` with images from `store` in the D13 mode `mode`.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if args.is_empty() {
        let _ = err.write_all(USAGE.as_bytes());
        return 1;
    }
    let parsed = match parser::parse(args, SPECS) {
        Ok(p) => p,
        Err(message) => {
            let _ = writeln!(err, "ERROR: {message}");
            return 1;
        }
    };
    let mut notices = Notices::new(mode);
    let code = match execute(&parsed, store, n_threads, &mut notices, out, err) {
        Ok(code) => code,
        Err(message) => {
            if !message.is_empty() {
                let _ = writeln!(err, "{message}");
            }
            1
        }
    };
    notices.print(err);
    code
}

/// ANTs aborts here: an uncaught exception from `Convert<T>`.
fn throws(option: &str, text: &str) -> String {
    format!(
        "larmorx: N4BiasFieldCorrection: ERROR: Unable to convert '{text}' for --{option} \
         (ANTs aborts: the parser throws)"
    )
}

fn bool_of(p: &Parsed, long: &str) -> Result<Option<bool>, String> {
    p.last(long)
        .map(|v| parser_convert_bool(&v.name).ok_or_else(|| throws(long, &v.name)))
        .transpose()
}

/// What the options set, as `N4<D>` reads them.
struct Settings {
    options: N4Options,
    verbose: bool,
}

/// The parameters of an option that ANTs reads only in brackets (ANTS-35): its parameters,
/// or, when it has none and a value, with `fix` that value as its first parameter (noted).
fn bracketed(option: &OptionValue, long: &str, notices: &mut Notices) -> Vec<String> {
    if !option.parameters.is_empty() || option.name.is_empty() {
        return option.parameters.clone();
    }
    let given = format!("--{long} {}", option.name);
    if notices.fix() {
        notices.add(
            &defects::ANTS_35,
            format!(
                "'{given}' is read as --{long} [{}]; ANTs sets nothing without brackets",
                option.name
            ),
        );
        vec![option.name.clone()]
    } else {
        notices.add(
            &defects::ANTS_35,
            format!(
                "'{given}' sets nothing, as in ANTs (only --{long} [{}] would)",
                option.name
            ),
        );
        Vec::new()
    }
}

fn settings(
    p: &Parsed,
    dim: usize,
    verbose: bool,
    notices: &mut Notices,
) -> Result<Settings, String> {
    let mut o = N4Options {
        upstream_defects: notices.mode(),
        ..N4Options::default()
    };
    // -c
    if let Some(c) = p.last("convergence") {
        let parameters = bracketed(c, "convergence", notices);
        if let Some(iters) = parameters.first() {
            o.iterations =
                parser_convert_vector_u32(iters).ok_or_else(|| throws("convergence", iters))?;
            o.convergence_threshold = 0.0;
        } else {
            // ITK's own defaults: one level of 50 iterations, threshold 0.001.
            o.iterations = vec![50];
            o.convergence_threshold = 0.001;
        }
        if let Some(t) = parameters.get(1) {
            o.convergence_threshold =
                parser_convert_f32(t).ok_or_else(|| throws("convergence", t))?;
        }
    }
    // -b
    if let Some(b) = p.last("bspline-fitting") {
        let parameters = bracketed(b, "bspline-fitting", notices);
        if let Some(order) = parameters.get(1) {
            o.spline_order =
                parser_convert_u32(order).ok_or_else(|| throws("bspline-fitting", order))?;
        }
        if let Some(mesh) = parameters.first() {
            let values =
                parser_convert_vector_f32(mesh).ok_or_else(|| throws("bspline-fitting", mesh))?;
            o.mesh = if values.len() == 1 {
                BSplineMesh::Distance(values[0])
            } else if values.len() == dim {
                BSplineMesh::Elements(values)
            } else {
                return Err(if verbose {
                    "Incorrect mesh resolution".into()
                } else {
                    String::new()
                });
            };
        }
    }
    // -s
    if let Some(s) = p.last("shrink-factor") {
        o.shrink_factor =
            parser_convert_i32(&s.name).ok_or_else(|| throws("shrink-factor", &s.name))?;
    }
    // -t
    if let Some(t) = p.last("histogram-sharpening") {
        let params = &bracketed(t, "histogram-sharpening", notices);
        if let Some(v) = params.first() {
            o.fwhm = parser_convert_f32(v).ok_or_else(|| throws("histogram-sharpening", v))?;
        }
        if let Some(v) = params.get(1) {
            o.wiener_noise =
                parser_convert_f32(v).ok_or_else(|| throws("histogram-sharpening", v))?;
        }
        if let Some(v) = params.get(2) {
            o.bins = parser_convert_u32(v).ok_or_else(|| throws("histogram-sharpening", v))?;
        }
    }
    Ok(Settings {
        options: o,
        verbose,
    })
}

/// The output names: `-o name` or `-o [corrected,bias]`.
fn outputs(o: &OptionValue) -> (String, Option<String>) {
    if o.parameters.is_empty() {
        (o.name.clone(), None)
    } else {
        (o.parameters[0].clone(), o.parameters.get(1).cloned())
    }
}

/// ANTs' `ReadImage<itk::Image<float, dim>>`: `None` (after ANTs' message) where it reads
/// nothing.
fn read(
    store: &mut dyn ImageStore,
    name: &str,
    dim: usize,
    n_threads: usize,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<Option<AntsImage<f32>>, String> {
    match read_with::<f32, _>(store, name, dim, n_threads, notices) {
        Ok(image) => Ok(Some(image)),
        Err(ReadError::Unsupported { name, message }) => Err(format!("larmorx: {name}: {message}")),
        Err(e) => {
            if let Some(m) = e.ants_message() {
                let _ = writeln!(err, "{m}");
            }
            Ok(None)
        }
    }
}

/// itk::Size printed: `[a, b, c]`.
fn size_text(size: &[usize]) -> String {
    let parts: Vec<String> = size.iter().map(ToString::to_string).collect();
    format!("[{}]", parts.join(", "))
}

fn execute(
    p: &Parsed,
    store: &mut dyn ImageStore,
    n_threads: usize,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, String> {
    if let Some(h) = p.last("help")
        && parser_convert_bool(&h.name).ok_or_else(|| throws("help", &h.name))?
    {
        let _ = out.write_all(USAGE.as_bytes());
        return Ok(0);
    }
    if let Some(h) = p.last("h")
        && parser_convert_bool(&h.name).ok_or_else(|| throws("h", &h.name))?
    {
        let _ = out.write_all(USAGE.as_bytes());
        return Ok(0);
    }
    if let Some(v) = p.last("version") {
        let name = v.name.to_lowercase();
        if name == "1" || name == "true" {
            let _ = writeln!(
                out,
                "larmorx {} (port of ANTs v2.6.5)",
                larmorx_core::VERSION
            );
            return Ok(0);
        }
    }
    // The dimension: -d, or the input file's.
    let dim = match p.last("image-dimensionality") {
        Some(d) => parser_convert_u32(&d.name)
            .ok_or_else(|| throws("image-dimensionality", &d.name))? as usize,
        None => {
            let Some(i) = p.last("input-image") else {
                let _ = writeln!(
                    err,
                    "No input images were specified.  Specify an input image with the -i option"
                );
                return Ok(1);
            };
            let name = i.parameters.first().unwrap_or(&i.name).clone();
            let dims = match store.load(&name, n_threads, notices) {
                Ok(image) => image.geometry.ndim,
                Err(_) => {
                    return Err(format!(
                        "larmorx: N4BiasFieldCorrection: cannot read '{name}' (ANTs crashes here)"
                    ));
                }
            };
            // ANTS-99: ANTs asks a new ImageIO for the dimension before it reads the header,
            // and NIfTI's answers 3 for every file (random testing, 2026-10-25).
            if dims != 3 {
                notices.add(
                    &defects::ANTS_99,
                    format!(
                        "{name} is {dims}-dimensional and processed as such; ANTs processes it \
                         in 3D"
                    ),
                );
            }
            if notices.fix() { dims } else { 3 }
        }
    };
    if !(2..=4).contains(&dim) {
        let _ = writeln!(out, "Unsupported dimension");
        return Ok(1);
    }
    let verbose = bool_of(p, "verbose")?.unwrap_or(false);
    if verbose {
        let _ = write!(out, "\nRunning N4 for {dim}-dimensional images.\n\n");
    }

    // The input.
    let Some(input_option) = p.last("input-image") else {
        if verbose {
            let _ = writeln!(err, "Input image not specified.");
        }
        return Ok(1);
    };
    let Some(input) = read(store, &input_option.name, dim, n_threads, notices, err)? else {
        return Err(
            "larmorx: N4BiasFieldCorrection: cannot read the input image (ANTs crashes here)"
                .into(),
        );
    };

    // The mask (read as float); a mask that cannot be read becomes ones but counts as given.
    let mask_given = p.has("mask-image");
    let mut mask = match p.last("mask-image") {
        Some(m) => read(store, &m.name, dim, n_threads, notices, err)?,
        None => None,
    };
    if mask_given && mask.is_none() {
        // ANTS-36: ANTs goes on with a mask of ones, which still counts as given.
        let name = p
            .last("mask-image")
            .map_or("", |m| m.name.as_str())
            .to_owned();
        if notices.fix() {
            notices.add(
                &defects::ANTS_36,
                format!(
                    "the mask '{name}' cannot be read: an error; ANTs uses a mask of ones and \
                     still restores and rescales as with a mask"
                ),
            );
            return Err(format!(
                "larmorx: N4BiasFieldCorrection: cannot read the mask '{name}' ({}=reproduce \
                 uses a mask of ones, as ANTs does)",
                defects::ENV
            ));
        }
        notices.add(
            &defects::ANTS_36,
            format!(
                "the mask '{name}' cannot be read: a mask of ones is used, and still counts as \
                 given, as in ANTs"
            ),
        );
    }
    if mask.is_none() {
        if verbose {
            let _ = write!(
                out,
                "Mask not read.  Using the entire image as the mask.\n\n"
            );
        }
        if mask_given {
            mask = Some(input.derived(vec![1.0f32; input.len()]));
        }
    }

    let settings = settings(p, dim, verbose, notices)?;
    let mut options = settings.options;

    // The weight image.
    let weight = match p.last("weight-image") {
        Some(w) => read(store, &w.name, dim, n_threads, notices, err)?,
        None => None,
    };

    // Rescaling: with a mask, unless -r 0 (read after the correction in ANTs; an invalid
    // value aborts before anything is written either way).
    let rescale_flag = bool_of(p, "rescale-intensities")?;
    options.rescale_intensities = mask_given && rescale_flag != Some(false);
    if rescale_flag == Some(true) && !mask_given {
        // ANTS-36: ANTs rescales only with a mask.
        notices.add(
            &defects::ANTS_36,
            "-r 1 has no effect without a mask (-x), in larmorx as in ANTs",
        );
    }

    let start = Instant::now();
    let mut lines: Vec<String> = Vec::new();
    let iterations = options.iterations.clone();
    let threshold = options.convergence_threshold;
    let mut observer = |it: &larmorx_image::n4::N4Iteration| {
        if it.iteration == 1 {
            lines.push(format!("Current level = {}", it.level + 1));
        }
        lines.push(format!(
            "  Iteration {} (of {}).   Current convergence value = {} (threshold = {})",
            it.iteration,
            iterations[it.level],
            format_g(f64::from(it.convergence)),
            format_g(f64::from(threshold))
        ));
    };
    let result = n4_bias_field_correction(
        &input,
        mask.as_ref(),
        weight.as_ref(),
        &options,
        n_threads,
        if settings.verbose {
            Some(&mut observer)
        } else {
            None
        },
    );
    let result = match result {
        Ok(r) => r,
        Err(N4Error::MeshResolution) => {
            if settings.verbose {
                let _ = writeln!(err, "Incorrect mesh resolution");
            }
            return Ok(1);
        }
        Err(N4Error::Geometry("mask")) => {
            return Err(
                "larmorx: N4BiasFieldCorrection: the mask is not on the image's grid \
                        (ITK: Inputs do not occupy the same physical space!; ANTs aborts)"
                    .into(),
            );
        }
        Err(N4Error::Memory(e)) => {
            return Err(format!(
                "larmorx: N4BiasFieldCorrection: {e}: the image padded for the B-spline mesh \
                 (ITK throws MemoryAllocationError; ANTs aborts)"
            ));
        }
        Err(N4Error::BeyondMemory(message)) => return Err(format!("larmorx: {message}")),
        Err(e) => {
            if settings.verbose {
                let _ = writeln!(err, "Exception caught: {e}");
            }
            return Ok(1);
        }
    };
    if settings.verbose {
        let (lo, hi) = result.original_range;
        let _ = writeln!(
            out,
            "Original intensity range:  [{}, {}]",
            format_g(f64::from(lo)),
            format_g(f64::from(hi))
        );
        if lo <= 0.0 {
            let _ = write!(
                out,
                "\n***********************************************************\n\
                 Warning:  Your input image contains nonpositive values\n\
                 which could cause failure or problematic results.  A\n\
                 possible workaround would be to:\n   \
                 1. rescale your image to positive values e.g., [10,100].\n   \
                 2. run N4 on your rescaled image.\n   \
                 3. (optional) rescale the N4 output to the original\n      \
                 intensity range.\n\
                 ***********************************************************\n\n"
            );
        }
        if let BSplineMesh::Distance(distance) = options.mesh {
            let _ = write!(
                out,
                "Specified spline distance: {}mm\n  original image size:  {}\n  padded image \
                 size:  {}\n  number of control points:  {}\n\n",
                format_g(f64::from(distance)),
                size_text(input.size()),
                size_text(&result.padded_size),
                size_text(&result.control_points)
            );
        }
        for line in &lines {
            let _ = writeln!(out, "{line}");
        }
        let _ = writeln!(
            out,
            "Elapsed time: {}",
            format_g(start.elapsed().as_secs_f64())
        );
    }

    notices.extend(result.notices);
    if let Some(o) = p.last("output") {
        let (corrected_name, bias_name) = outputs(o);
        write_with(
            store,
            &corrected_name,
            result.corrected.into(),
            n_threads,
            notices,
        )
        .map_err(|e| format!("larmorx: cannot write '{corrected_name}': {e}"))?;
        if let Some(name) = bias_name {
            write_with(store, &name, result.bias_field.into(), n_threads, notices)
                .map_err(|e| format!("larmorx: cannot write '{name}': {e}"))?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_capture(args: &[&str]) -> (u8, String, String) {
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
    fn usage_help_and_errors() {
        let (code, _, err) = run_capture(&[]);
        assert_eq!(code, 1);
        assert!(err.contains("--bspline-fitting"));
        let (code, out, _) = run_capture(&["--help"]);
        assert_eq!((code, out.contains("--convergence")), (0, true));
        let (code, out, _) = run_capture(&["-h"]);
        assert_eq!((code, out.contains("--shrink-factor")), (0, true));
        let (code, _, err) = run_capture(&["-d", "3"]);
        assert_eq!(code, 1);
        assert!(err.is_empty());
        let (code, out, _) = run_capture(&["-d", "5", "-i", "x.nii"]);
        assert_eq!((code, out.as_str()), (1, "Unsupported dimension\n"));
        let (code, _, err) = run_capture(&["-d", "3d", "-i", "x.nii"]);
        assert_eq!(code, 1);
        assert!(err.contains("Unable to convert"));
        let (code, _, err) = run_capture(&["-d", "3", "--nope", "1"]);
        assert_eq!(code, 1);
        assert!(err.contains("Invalid flag"));
    }

    #[test]
    fn settings_follow_ants() {
        let parse = |a: &[&str]| {
            let args: Vec<String> = a.iter().map(|s| s.to_string()).collect();
            let p = parser::parse(&args, SPECS).unwrap();
            settings(&p, 3, false, &mut Notices::new(UpstreamDefects::Reproduce))
        };
        let s = parse(&[
            "-c", "[", "50x40,", "1e-7", "]", "-b", "[", "200", "]", "-s", "2",
        ])
        .unwrap()
        .options;
        assert_eq!(s.iterations, [50, 40]);
        assert_eq!(s.convergence_threshold, 1e-7);
        assert_eq!(s.mesh, BSplineMesh::Distance(200.0));
        assert_eq!(s.shrink_factor, 2);
        // Without brackets, -b and -c set nothing (-c leaves ITK's own defaults).
        let s = parse(&["-b", "200", "-c", "50x50"]).unwrap().options;
        assert_eq!(s.mesh, BSplineMesh::Default);
        assert_eq!((s.iterations, s.convergence_threshold), (vec![50], 0.001));
        let s = parse(&["-b", "[2x2x3,2]", "-t", "[0.2,0.02,128]"])
            .unwrap()
            .options;
        assert_eq!(s.mesh, BSplineMesh::Elements(vec![2.0, 2.0, 3.0]));
        assert_eq!(
            (s.spline_order, s.fwhm, s.wiener_noise, s.bins),
            (2, 0.2, 0.02, 128)
        );
        assert!(parse(&["-s", "2.5"]).is_err());
        assert!(parse(&["-b", "[2x2]"]).is_err());
    }

    #[test]
    fn unbracketed_values_are_read() {
        // ANTS-35 corrected: an unbracketed value is the option's first parameter.
        let args: Vec<String> = ["-b", "200", "-c", "50x50", "-t", "0.2"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let p = parser::parse(&args, SPECS).unwrap();
        let mut notices = Notices::new(UpstreamDefects::Fix);
        let s = settings(&p, 3, false, &mut notices).unwrap().options;
        assert_eq!(s.mesh, BSplineMesh::Distance(200.0));
        assert_eq!((s.iterations, s.convergence_threshold), (vec![50, 50], 0.0));
        assert_eq!(s.fwhm, 0.2);
        assert!(notices.contains(&defects::ANTS_35));
        let mut notices = Notices::new(UpstreamDefects::Reproduce);
        let s = settings(&p, 3, false, &mut notices).unwrap().options;
        assert_eq!(s.mesh, BSplineMesh::Default);
        assert!(notices.contains(&defects::ANTS_35));
        // Brackets: nothing to say.
        let args: Vec<String> = ["-b", "[200]"].iter().map(|s| s.to_string()).collect();
        let p = parser::parse(&args, SPECS).unwrap();
        let mut notices = Notices::new(UpstreamDefects::Fix);
        settings(&p, 3, false, &mut notices).unwrap();
        assert!(notices.is_empty());
    }
}
