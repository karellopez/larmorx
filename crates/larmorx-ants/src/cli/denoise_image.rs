// SPDX-License-Identifier: Apache-2.0
//! `DenoiseImage` with its original arguments (ANTs v2.6.5, `Examples/DenoiseImage.cxx`),
//! through ANTs' command-line parser.
//!
//! ```text
//! DenoiseImage [-d 2/3/4] -i input [-n Rician/(Gaussian)] [-x mask] [-s (1)/2/3/...]
//!     [-p 1|1x1x1] [-r 2|2x2x2] [-o corrected | [corrected,<noise>]] [-v (0)/1]
//! ```
//!
//! As in ANTs:
//! - values are read with the parser's strict `Convert<T>`: a value that is not entirely a
//!   number (`-s 1.5`, `-d 3d`, `-p 1x1.5`) makes ANTs throw and abort; larmorx prints a
//!   message and exits 1;
//! - `-n` is case-insensitive; another model exits 1 (the message only with `-v`);
//! - `-p` and `-r` take one value for every axis or one per axis, else exit 1 (the message
//!   only with `-v`);
//! - the mask is read as `unsigned char` (a C++ cast of each value) and must be on the grid
//!   the filter runs on: with `-s 2` a full-resolution mask makes ITK throw, and ANTs exits 1
//!   (printing the exception only with `-v`); a mask that cannot be read is no mask, and
//!   the whole image is denoised (ANTS-47);
//! - without `-d`, the dimension is the input file's (ANTs: 3 whatever the file, ANTS-99);
//! - `-o [denoised,noise]` writes the noise first, then the denoised image; without `-o`
//!   nothing is written.
//!
//! `-v` prints ANTs' progress characters and the elapsed time; ITK's filter dump (`Print`) is
//! replaced by the filter's parameters. ANTs uses `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`
//! threads and its result depends on it; larmorx uses the same number of threads and gives
//! ANTs' single-thread result (`docs/findings/ants-denoise.md`).
//!
//! **Upstream defects (D13).** By default larmorx refuses a mask it cannot read (ANTS-47),
//! clamps mask values outside `unsigned char`'s range (ANTS-06), corrects the filter's
//! Rician correction factor (ANTS-43) and gives the voxels outside a mask the smoothed and
//! corrected Rician bias (ANTS-44); `LARMORX_UPSTREAM_DEFECTS=reproduce` gives ANTs'
//! behaviour.

use std::io::Write;
use std::time::Instant;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notices, UpstreamDefects};

use super::cstd::{
    format_g, parser_convert_bool, parser_convert_i32, parser_convert_u32,
    parser_convert_vector_u32,
};
use super::parser::{self, OptionSpec, OptionValue, Parsed};
use crate::denoise_image::{DenoiseError, DenoiseOptions, NoiseModel, denoise_image};
use crate::image::{AntsImage, FileStore, ImageStore, Pixel, ReadError, read_with, write_with};

const fn opt(long: &'static str, short: Option<char>) -> OptionSpec {
    OptionSpec { long, short }
}

const SPECS: &[OptionSpec] = &[
    opt("image-dimensionality", Some('d')),
    opt("input-image", Some('i')),
    opt("noise-model", Some('n')),
    opt("mask-image", Some('x')),
    opt("shrink-factor", Some('s')),
    opt("patch-radius", Some('p')),
    opt("search-radius", Some('r')),
    opt("output", Some('o')),
    opt("version", None),
    opt("verbose", Some('v')),
    opt("h", Some('h')),
    opt("help", None),
];

const USAGE: &str = "\
COMMAND:
     DenoiseImage (larmorx port of ANTs v2.6.5)
          Denoise an image using a spatially adaptive filter originally described in J. V.
          Manjon, P. Coupe, Luis Marti-Bonmati, D. L. Collins, and M. Robles. Adaptive
          Non-Local Means Denoising of MR Images With Spatially Varying Noise Levels, Journal
          of Magnetic Resonance Imaging, 31:192-203, June 2010.

OPTIONS:
     -d, --image-dimensionality 2/3/4
     -i, --input-image inputImageFilename
     -n, --noise-model Rician/(Gaussian)
     -x, --mask-image maskImageFilename
     -s, --shrink-factor (1)/2/3/...
     -p, --patch-radius 1
                        1x1x1
     -r, --search-radius 2
                         2x2x2
     -o, --output correctedImage
                  [correctedImage,<noiseImage>]
     --version
     -v, --verbose (0)/1
     -h
     --help
";

/// Runs `DenoiseImage` on files, in the D13 mode of `LARMORX_UPSTREAM_DEFECTS`; returns the
/// exit code.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match UpstreamDefects::from_env() {
        Ok(mode) => run(args, &mut FileStore, super::itk_threads(), mode, out, err),
        Err(e) => {
            let _ = writeln!(err, "larmorx: {e}");
            EXIT_BAD_MODE
        }
    }
}

/// Runs `DenoiseImage` with images from `store` in the D13 mode `mode`.
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
        "larmorx: DenoiseImage: ERROR: Unable to convert '{text}' for --{option} \
         (ANTs aborts: the parser throws)"
    )
}

/// `-p` / `-r`: `ConvertVector<unsigned int>` of the option's name.
fn radius_option(p: &Parsed, long: &str) -> Result<Option<Vec<u32>>, String> {
    p.last(long)
        .map(|v| parser_convert_vector_u32(&v.name).ok_or_else(|| throws(long, &v.name)))
        .transpose()
}

/// The output names: `-o name` or `-o [denoised,noise]`.
fn outputs(o: &OptionValue) -> (String, Option<String>) {
    if o.parameters.is_empty() {
        (o.name.clone(), None)
    } else {
        (o.parameters[0].clone(), o.parameters.get(1).cloned())
    }
}

/// ANTs' `ReadImage<itk::Image<T, dim>>`: `None` (after ANTs' message) where it reads
/// nothing.
fn read<T: Pixel>(
    store: &mut dyn ImageStore,
    name: &str,
    dim: usize,
    n_threads: usize,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<Option<AntsImage<T>>, String> {
    match read_with::<T, _>(store, name, dim, n_threads, notices) {
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

/// ANTs' progress observer: up to 99 characters, the tens as numbers, the rest as `*`.
fn progress_line() -> String {
    (1..100)
        .map(|i| {
            if i % 10 == 0 {
                i.to_string()
            } else {
                "*".to_owned()
            }
        })
        .collect()
}

fn radius_text(r: &[u32], dim: usize, default: u32) -> String {
    let values: Vec<String> = match r.len() {
        0 => vec![default.to_string(); dim],
        1 => vec![r[0].to_string(); dim],
        _ => r.iter().map(ToString::to_string).collect(),
    };
    format!("[{}]", values.join(", "))
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
                        "larmorx: DenoiseImage: cannot read '{name}' (ANTs crashes here)"
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
    let verbose = match p.last("verbose") {
        Some(v) => parser_convert_bool(&v.name).ok_or_else(|| throws("verbose", &v.name))?,
        None => false,
    };
    if verbose {
        let _ = write!(out, "\nRunning for {dim}-dimensional images.\n\n");
    }

    // The input.
    let Some(input_option) = p.last("input-image") else {
        if verbose {
            let _ = writeln!(err, "Input image not specified.");
        }
        return Ok(1);
    };
    let Some(input) = read::<f32>(store, &input_option.name, dim, n_threads, notices, err)? else {
        return Err(
            "larmorx: DenoiseImage: cannot read the input image (ANTs crashes here)".into(),
        );
    };

    let mut options = DenoiseOptions {
        upstream_defects: notices.mode(),
        ..DenoiseOptions::default()
    };
    // -s, read before the noise model.
    if let Some(s) = p.last("shrink-factor") {
        options.shrink_factor =
            parser_convert_i32(&s.name).ok_or_else(|| throws("shrink-factor", &s.name))?;
    }
    // -n
    let model = p
        .last("noise-model")
        .map_or_else(|| "gaussian".to_owned(), |n| n.name.to_lowercase());
    options.noise_model = match model.as_str() {
        "rician" => NoiseModel::Rician,
        "gaussian" => NoiseModel::Gaussian,
        _ => {
            if verbose {
                let _ = writeln!(err, "Unrecognized noise model:  {model}.  See help menu.");
            }
            return Ok(1);
        }
    };
    // -x, read as unsigned char; ANTs takes a mask that cannot be read as no mask
    // (ANTS-47).
    let mask = match p.last("mask-image") {
        Some(m) => {
            let mask = read::<u8>(store, &m.name, dim, n_threads, notices, err)?;
            if mask.is_none() {
                if notices.fix() {
                    notices.add(
                        &defects::ANTS_47,
                        format!(
                            "the mask '{}' cannot be read: an error; ANTs denoises the whole \
                             image",
                            m.name
                        ),
                    );
                    return Err(format!(
                        "larmorx: DenoiseImage: cannot read the mask '{}' ({}=reproduce \
                         denoises the whole image, as ANTs does)",
                        m.name,
                        defects::ENV
                    ));
                }
                notices.add(
                    &defects::ANTS_47,
                    format!(
                        "the mask '{}' cannot be read: the whole image is denoised, as in ANTs",
                        m.name
                    ),
                );
            }
            mask
        }
        None => None,
    };
    // -r, then -p.
    if let Some(r) = radius_option(p, "search-radius")? {
        if r.len() != 1 && r.len() != dim {
            if verbose {
                let _ = writeln!(
                    err,
                    "Search radius specified incorrectly.  Please see usage options."
                );
            }
            return Ok(1);
        }
        options.search_radius = r;
    }
    if let Some(r) = radius_option(p, "patch-radius")? {
        if r.len() != 1 && r.len() != dim {
            if verbose {
                let _ = writeln!(
                    err,
                    "Patch radius specified incorrectly.  Please see usage options."
                );
            }
            return Ok(1);
        }
        options.patch_radius = r;
    }

    let start = Instant::now();
    let result = match denoise_image(&input, mask.as_ref(), &options, n_threads) {
        Ok(r) => r,
        Err(
            e @ (DenoiseError::Radius { .. }
            | DenoiseError::Geometry
            | DenoiseError::MaskRegion { .. }
            | DenoiseError::Filter(_)
            | DenoiseError::Resample(_)),
        ) => {
            if verbose {
                let _ = writeln!(err, "Exception caught: {e}");
            }
            return Ok(1);
        }
    };
    if verbose {
        let _ = write!(out, "{}\n\n", progress_line());
        let _ = writeln!(
            out,
            "   AdaptiveNonLocalMeansDenoisingImageFilter (larmorx)\n   \
             Using mean squares to measure the patch similarity.\n   \
             Neighborhood search radius = {}\n   \
             Neighborhood patch radius = {}\n   \
             Using {} noise model.\n   \
             Epsilon = 1e-05\n   Mean threshold = 0.95\n   Variance threshold = 0.5\n   \
             Smoothing variance = 2\n   \
             Neighborhood radius for local mean and variance = {}",
            radius_text(&options.search_radius, dim, 2),
            radius_text(&options.patch_radius, dim, 1),
            if options.noise_model == NoiseModel::Rician {
                "Rician"
            } else {
                "Gaussian"
            },
            radius_text(&[], dim, 1),
        );
        let _ = writeln!(
            out,
            "Elapsed time: {}",
            format_g(start.elapsed().as_secs_f64())
        );
    }

    notices.extend(result.notices);
    if let Some(o) = p.last("output") {
        let (denoised_name, noise_name) = outputs(o);
        if let Some(name) = noise_name {
            write_with(store, &name, result.noise.into(), n_threads, notices)
                .map_err(|e| format!("larmorx: cannot write '{name}': {e}"))?;
        }
        write_with(
            store,
            &denoised_name,
            result.denoised.into(),
            n_threads,
            notices,
        )
        .map_err(|e| format!("larmorx: cannot write '{denoised_name}': {e}"))?;
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
        assert!(err.contains("--noise-model"));
        let (code, out, _) = run_capture(&["--help"]);
        assert_eq!((code, out.contains("--search-radius")), (0, true));
        let (code, out, _) = run_capture(&["-h"]);
        assert_eq!((code, out.contains("--patch-radius")), (0, true));
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

    /// ANTS-99 (differential random testing, 2026-10-25, `ants-denoise:20261025:188`): without
    /// `-d`, ANTs takes a 2D file as 3D (its ImageIO answers 3 before reading the header) and
    /// writes a 3D image; larmorx denoises it in 2D by default, as ANTs in `reproduce`, and
    /// names ANTS-99 in both modes.
    #[test]
    fn without_d_a_2d_image_is_3d_in_ants() {
        use larmorx_core::element::DataType;
        use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, WriteOptions};
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.nii");
        let h = NiftiHeader::new(NiftiVersion::V1, &[5, 4], DataType::F32).unwrap();
        let data = larmorx_core::ndarray::ArrayD::from_shape_vec(
            larmorx_core::ndarray::IxDyn(&[5, 4]),
            (0..20).map(|k| 10.0 + (k * k % 7) as f32).collect(),
        )
        .unwrap();
        nifti::write(&a, &h, data.view(), &WriteOptions::default()).unwrap();
        for (mode, dims) in [(UpstreamDefects::Fix, 2), (UpstreamDefects::Reproduce, 3)] {
            let b = dir.path().join(format!("{}.nii", mode.name()));
            let args: Vec<String> = ["-i", a.to_str().unwrap(), "-o", b.to_str().unwrap()]
                .iter()
                .map(|s| s.to_string())
                .collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = run(&args, &mut FileStore, 1, mode, &mut out, &mut err);
            let err = String::from_utf8(err).unwrap();
            assert_eq!(code, 0, "{err}");
            assert!(err.contains("ANTS-99"), "{err}");
            let header = nifti::read_header(&b).unwrap();
            assert_eq!(header.dim[0] as usize, dims, "{mode:?}");
        }
    }

    #[test]
    fn progress_has_99_characters() {
        let line = progress_line();
        assert!(line.starts_with("*********10*********20"));
        assert!(line.ends_with("90*********"));
    }
}
