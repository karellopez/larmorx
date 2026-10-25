// SPDX-License-Identifier: Apache-2.0
//! `ResampleImage` with its original arguments (ANTs v2.6.5, `Examples/ResampleImage.cxx`):
//!
//! ```text
//! ResampleImage imageDimension inputImage outputImage MxNxO [size=1,spacing=0] [interpolate type] [pixeltype]
//! ```
//!
//! As in ANTs:
//! - fewer than four arguments print the usage (exit 0 for `--help`/`-h`, else 1);
//! - the interpolation type is 0 linear, 1 nearest neighbour, 2 Gaussian (sigmas, then alpha
//!   in the eighth argument), 3 windowed sinc, 4 B-spline; other numbers are linear;
//! - a dimension other than 2, 3 or 4 prints "Unsupported dimension" and exits 1;
//! - a spacing or size list whose length is neither 1 nor the dimension prints "Invalid
//!   spacing." or "Invalid size." and leaves the grid uninitialised (larmorx stops).
//!
//! **The seventh argument (ANTS-30).** ANTs reads it twice: as the pixel type (`std::stoi`:
//! 0 `char`, 1 `unsigned char`, 2 `short`, 3 `unsigned short`, 4 `int`, 5 `unsigned int`,
//! 6 `float`, 7 `double`; anything else prints "Unsupported pixel type") and as the
//! interpolator's parameter: the Gaussian's sigmas (`1x1x1`, so a Gaussian with sigmas
//! starting with 1 resamples an `unsigned char` image), the windowed sinc's window (its first
//! character, which `std::stoi` cannot read, so only the default Hamming window is reachable)
//! or the B-spline order (`3` gives order 3 and `unsigned short` pixels; 6 or 7 give order 3).
//! larmorx reads the command line as the usage groups it (`docs/api/ants-resample-image.md`,
//! "The seventh argument"):
//!
//! ```text
//! ResampleImage d in out MxNxO [size=1,spacing=0] [interpolate type [its parameters]] [pixeltype]
//! ```
//!
//! An interpolation type's parameters follow it, where ANTs reads them: Gaussian `sigma` and
//! `alpha` (arguments 7 and 8), the windowed sinc's window letter (7: `c`osine, `w`elch,
//! `b`lackman, `l`anczos, `h`amming), the B-spline order (7). The pixel type comes after them:
//! argument 7 for linear and nearest-neighbour interpolation (as in ANTs), 8 for windowed sinc
//! and B-spline, 9 for Gaussian. With `LARMORX_UPSTREAM_DEFECTS=reproduce` the arguments are
//! read as ANTs reads them. A notice names ANTS-30 whenever the two readings differ.
//!
//! Also corrected by default: a size of 1 (ANTS-29, an error; reproduced: ANTs' image of
//! zeros with an infinite spacing) and sizes out of `unsigned int`'s range (ANTS-06: clamped;
//! reproduced: wrapped as x86-64 converts them).
//!
//! Gaussian, windowed-sinc and B-spline interpolation are supported in 3D; 2D and 4D images
//! are resampled with linear or nearest-neighbour interpolation.

use std::io::Write;

use larmorx_core::RealElement;
use larmorx_core::defects::{self, EXIT_BAD_MODE, Notices, UpstreamDefects};
use larmorx_interp::{Interpolation, Window};

use super::cstd::{atof, convert_vector_f64, stoi};
use crate::image::{AntsImage, FileStore, ImageStore, OutputImage, Pixel, ReadError, read_with};
use crate::resample_image::{ResampleTarget, resample_image};

const USAGE: &str = "Usage: ResampleImage imageDimension inputImage outputImage MxNxO [size=1,spacing=0] [interpolate type [its parameters]] [pixeltype]
  Interpolation type:
    0. linear (default)
    1. nn
    2. gaussian [sigma=imageSpacing] [alpha=1.0]
    3. windowedSinc [type = 'c'osine, 'w'elch, 'b'lackman, 'l'anczos, 'h'amming]
    4. B-Spline [order=3]
 pixeltype  :  TYPE
  0  :  char
  1  :  unsigned char
  2  :  short
  3  :  unsigned short
  4  :  int
  5  :  unsigned int
  6  :  float (default)
  7  :  double
 larmorx reads an interpolation type's parameters right after it, and the pixel type after
 them (LARMORX_UPSTREAM_DEFECTS=reproduce reads the seventh argument as both, as ANTs does).
";

/// Runs `ResampleImage` on files, in the D13 mode of `LARMORX_UPSTREAM_DEFECTS`; returns the
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

/// How many parameters interpolation `kind` (`argv[6]`) takes after it in larmorx's reading
/// (ANTS-30): Gaussian 2, windowed sinc and B-spline 1, the others none.
fn interpolation_parameters(kind: i32) -> usize {
    match kind {
        2 => 2,
        3 | 4 => 1,
        _ => 0,
    }
}

/// A pixel-type argument: its value, or why it is not one (`std::stoi` throws).
fn pixel_type(s: Option<&str>) -> Result<u32, String> {
    match s {
        None => Ok(6),
        // `unsigned int typeoption = std::stoi(...)`: a negative number wraps.
        Some(s) => stoi_arg(s).map(|v| v as u32),
    }
}

fn describe_type(t: &Result<u32, String>) -> String {
    const NAMES: [&str; 8] = [
        "char",
        "unsigned char",
        "short",
        "unsigned short",
        "int",
        "unsigned int",
        "float",
        "double",
    ];
    match t {
        Ok(t) => NAMES
            .get(*t as usize)
            .map_or_else(|| format!("unsupported ({t})"), |n| (*n).to_owned()),
        Err(_) => "an abort (std::stoi throws)".to_owned(),
    }
}

/// Runs `ResampleImage` with images from `store` in the D13 mode `mode`.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec!["ResampleImage".to_owned()];
    argv.extend(args.iter().cloned());
    if argv.len() < 5 {
        let _ = out.write_all(USAGE.as_bytes());
        let help = matches!(argv.get(1).map(String::as_str), Some("--help" | "-h"));
        return if help { 0 } else { 1 };
    }
    let mut notices = Notices::new(mode);
    let result = (|| -> Result<(), String> {
        // ANTS-30: the pixel type is argv[7] in ANTs; in larmorx's reading it follows the
        // interpolation type's parameters.
        let ants_type = pixel_type(argv.get(7).map(String::as_str));
        let kind = match argv.get(6) {
            Some(s) => stoi(s).unwrap_or(0),
            None => 0,
        };
        let own_index = 7 + interpolation_parameters(kind);
        let own_type = pixel_type(argv.get(own_index).map(String::as_str));
        if ants_type != own_type {
            let detail = if notices.fix() {
                format!(
                    "the pixel type is {} (argument {own_index}, after the interpolator's \\
                     parameters); ANTs reads argument 7 as both and gives {}",
                    describe_type(&own_type),
                    describe_type(&ants_type)
                )
            } else {
                format!(
                    "argument 7 is read as the pixel type too, as in ANTs: {}",
                    describe_type(&ants_type)
                )
            };
            notices.add(&defects::ANTS_30, detail);
        }
        let type_option = if notices.fix() { own_type } else { ants_type }?;
        if type_option > 7 {
            let _ = writeln!(out, "Unsupported pixel type");
            return Err(String::new());
        }
        let dim = stoi_arg(&argv[1])?;
        if !(2..=4).contains(&dim) {
            let _ = writeln!(out, "Unsupported dimension");
            return Err(String::new());
        }
        let dim = dim as usize;
        let mut job = Job {
            argv: &argv,
            dim,
            store,
            n_threads,
            notices: &mut notices,
            out,
            err,
        };
        match type_option {
            0 => job.resample::<i8>(),
            1 => job.resample::<u8>(),
            2 => job.resample::<i16>(),
            3 => job.resample::<u16>(),
            4 => job.resample::<i32>(),
            5 => job.resample::<u32>(),
            6 => job.resample::<f32>(),
            _ => job.resample::<f64>(),
        }
    })();
    let code = match result {
        Ok(()) => 0,
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

fn crash(what: &str) -> String {
    format!("larmorx: ResampleImage: {what} (ANTs crashes here)")
}

/// `std::stoi`, or the abort ANTs has when it throws.
fn stoi_arg(s: &str) -> Result<i32, String> {
    stoi(s).ok_or_else(|| {
        format!("larmorx: ResampleImage: '{s}' is not a number (ANTs aborts: std::stoi throws)")
    })
}

struct Job<'a> {
    argv: &'a [String],
    dim: usize,
    store: &'a mut dyn ImageStore,
    n_threads: usize,
    notices: &'a mut Notices,
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
}

impl Job<'_> {
    /// `ResampleImage<dim, T>`.
    fn resample<T>(&mut self) -> Result<(), String>
    where
        T: Pixel + RealElement,
        AntsImage<T>: Into<OutputImage>,
    {
        let (argv, dim) = (self.argv, self.dim);
        let argc = argv.len();
        let image: AntsImage<T> =
            match read_with::<T, _>(self.store, &argv[2], dim, self.n_threads, self.notices) {
                Ok(i) => i,
                Err(ReadError::Unsupported { name, message }) => {
                    return Err(format!("larmorx: {name}: {message}"));
                }
                Err(e) => {
                    if let Some(m) = e.ants_message() {
                        let _ = writeln!(self.err, "{m}");
                    }
                    return Err(crash(&format!("cannot read image '{}'", argv[2])));
                }
            };
        let sp = convert_vector_f64(&argv[4])
            .ok_or_else(|| crash(&format!("'{}' starts with an empty element", argv[4])))?;
        let by_spacing = argc <= 5 || stoi_arg(&argv[5])? == 0;
        if sp.len() != 1 && sp.len() != dim {
            let what = if by_spacing { "spacing" } else { "size" };
            let _ = writeln!(self.out, "Invalid {what}.");
            return Err(crash(&format!("the output {what} is left uninitialised")));
        }
        // `static_cast<unsigned int>(double)`: GCC truncates to 64 bits and keeps the low 32
        // (out of range, ANTS-06: reproduced as x86-64 converts; clamped by default).
        let undefined = |v: f64| !(v > -1.0 && v < 4_294_967_296.0);
        let wrapped = if by_spacing {
            0
        } else {
            sp.iter().filter(|&&v| undefined(v)).count()
        };
        if wrapped > 0 {
            self.notices.add(
                &defects::ANTS_06,
                if self.notices.fix() {
                    format!(
                        "{wrapped} size(s) outside unsigned int's range are clamped; ANTs on \\
                         x86-64 wraps them around"
                    )
                } else {
                    format!(
                        "{wrapped} size(s) outside unsigned int's range wrap around as ANTs on \\
                         x86-64 converts them"
                    )
                },
            );
        }
        let sizes: Vec<usize> = sp
            .iter()
            .map(|&v| {
                if self.notices.fix() {
                    v as u32 as usize
                } else {
                    crate::image::x86_to_i64(v) as u32 as usize
                }
            })
            .collect();
        let target = if by_spacing {
            ResampleTarget::Spacing(&sp)
        } else {
            ResampleTarget::Size(&sizes)
        };
        if !by_spacing && sizes.contains(&1) && self.notices.fix() {
            self.notices.add(
                &defects::ANTS_29,
                "a size of 1: an error; ANTs divides by size − 1 and writes an image of zeros \\
                 with an infinite spacing",
            );
        }
        let interpolation = self.interpolation(&image)?;
        let (resampled, notices) = resample_image(
            &image,
            target,
            &interpolation,
            self.n_threads,
            self.notices.mode(),
        )
        .map_err(|e| format!("larmorx: ResampleImage: {e}"))?;
        self.notices.extend(notices);
        crate::image::write_with(
            self.store,
            &argv[3],
            resampled.into(),
            self.n_threads,
            self.notices,
        )
        .map_err(|e| format!("larmorx: cannot write '{}': {e}", argv[3]))?;
        Ok(())
    }

    /// The interpolator ANTs chooses from the sixth to eighth arguments.
    fn interpolation<T>(&mut self, image: &AntsImage<T>) -> Result<Interpolation, String> {
        let (argv, dim) = (self.argv, self.dim);
        let argc = argv.len();
        let kind = match argv.get(6) {
            Some(s) => stoi_arg(s)?,
            None => 0,
        };
        Ok(match kind {
            1 => Interpolation::NearestNeighbor,
            2 => {
                let mut sigma: Vec<f64> = image.geometry.spacing.clone();
                if argc > 7 {
                    let sg = convert_vector_f64(&argv[7]).ok_or_else(|| {
                        crash(&format!("'{}' starts with an empty element", argv[7]))
                    })?;
                    if sg.len() < dim {
                        // ANTS-160 (found by differential random testing, 2026-10-25): ANTs
                        // reads the missing sigmas past the end of its vector
                        // (`ResampleImage.cxx:178-182`) and exits 0 with a result that depends
                        // on the heap (usually NaN: an image of zeros in an integer type, and
                        // vnl's convergence messages). Class (a): an error in both modes, never
                        // reproduced (as ANTS-77).
                        let detail = format!(
                            "{} Gaussian sigmas for {dim} dimensions: ANTs reads the missing ones past the end of its list (its result depends on memory it never wrote)",
                            sg.len()
                        );
                        if self.notices.fix() {
                            self.notices
                                .add(&defects::ANTS_160, format!("{detail}: an error"));
                        }
                        return Err(format!(
                            "larmorx: ResampleImage: {detail}: an error (ANTS-160)"
                        ));
                    }
                    sigma = sg[..dim].to_vec();
                }
                let alpha = argv.get(8).map_or(1.0, |s| atof(s));
                if dim != 3 {
                    return Err(unsupported("Gaussian", dim));
                }
                Interpolation::Gaussian {
                    sigma: [sigma[0], sigma[1], sigma[2]],
                    alpha,
                }
            }
            3 => {
                if dim != 3 {
                    return Err(unsupported("windowed-sinc", dim));
                }
                let window = match argv.get(7).and_then(|s| s.bytes().next()) {
                    Some(b'c') => Window::Cosine,
                    // 'b' is documented as Blackman; ANTs' code builds a second Lanczos
                    // (unreachable there: `std::stoi('b')` aborts first; ANTS-30).
                    Some(b'b') if self.notices.fix() => Window::Blackman,
                    Some(b'l' | b'b') => Window::Lanczos,
                    Some(b'w') => Window::Welch,
                    _ => Window::Hamming,
                };
                Interpolation::WindowedSincEdge(window)
            }
            4 => {
                if dim != 3 {
                    return Err(unsupported("B-spline", dim));
                }
                let order = match argv.get(7) {
                    Some(s) => {
                        let n = stoi_arg(s)?;
                        if (0..=5).contains(&n) { n as u32 } else { 3 }
                    }
                    None => 3,
                };
                Interpolation::BSpline { order }
            }
            _ => Interpolation::Linear,
        })
    }
}

fn unsupported(what: &str, dim: usize) -> String {
    format!(
        "larmorx: ResampleImage: {what} interpolation is supported for 3D images only, not yet in {dim}D"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_capture(args: &[&str], mode: UpstreamDefects) -> (u8, String, String) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args, &mut FileStore, 1, mode, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn usage_types_and_dimensions() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let run_capture = |args: &[&str]| run_capture(args, mode);
            let (code, out, _) = run_capture(&["--help"]);
            assert_eq!(code, 0);
            assert!(out.contains("pixeltype"));
            assert_eq!(run_capture(&["3", "a.nii", "b.nii"]).0, 1);
            let (code, out, _) = run_capture(&["3", "a.nii", "b.nii", "1", "0", "0", "9"]);
            assert_eq!((code, out.as_str()), (1, "Unsupported pixel type\n"));
            let (code, out, _) = run_capture(&["7", "a.nii", "b.nii", "1"]);
            assert_eq!((code, out.as_str()), (1, "Unsupported dimension\n"));
        }
        // ANTS-30: windowed sinc 'l' aborts ANTs (std::stoi of the pixel type); larmorx reads
        // the window, and the pixel type after it.
        let (code, _, err) = run_capture(
            &["3", "a.nii", "b.nii", "1", "0", "3", "l"],
            UpstreamDefects::Reproduce,
        );
        assert_eq!(code, 1);
        assert!(
            err.contains("std::stoi") && err.contains("ANTS-30"),
            "{err}"
        );
        let (code, _, err) = run_capture(
            &["3", "missing.nii", "b.nii", "1", "0", "3", "l"],
            UpstreamDefects::Fix,
        );
        assert_eq!(code, 1);
        assert!(
            err.contains("does not exist") && err.contains("ANTS-30"),
            "{err}"
        );
        // B-spline order 5: pixel type 5 in ANTs, float (or argument 8) in larmorx.
        let (_, _, err) = run_capture(
            &["3", "missing.nii", "b.nii", "1", "0", "4", "5"],
            UpstreamDefects::Fix,
        );
        assert!(
            err.contains("the pixel type is float") && err.contains("unsigned int"),
            "{err}"
        );
        let (_, out, err) = run_capture(
            &["3", "missing.nii", "b.nii", "1", "0", "4", "5", "9"],
            UpstreamDefects::Fix,
        );
        assert_eq!(out, "Unsupported pixel type\n", "{err}");
        // Linear and nearest neighbour: the same reading, no notice.
        let (_, _, err) = run_capture(
            &["3", "missing.nii", "b.nii", "1", "0", "1", "3"],
            UpstreamDefects::Fix,
        );
        assert!(!err.contains("ANTS-30"), "{err}");
    }

    /// ANTS-160 (differential random testing, 2026-10-25): one Gaussian sigma for three
    /// dimensions. ANTs reads the other two past the end of its vector and exits 0 with a
    /// result that depends on the heap; larmorx refuses in both modes (class (a)).
    #[test]
    fn missing_gaussian_sigmas_are_refused() {
        use larmorx_core::element::DataType;
        use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, WriteOptions};
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.nii");
        let h = NiftiHeader::new(NiftiVersion::V1, &[2, 2, 2], DataType::F32).unwrap();
        let data = larmorx_core::ndarray::ArrayD::from_elem(
            larmorx_core::ndarray::IxDyn(&[2, 2, 2]),
            3.0f32,
        );
        nifti::write(&a, &h, data.view(), &WriteOptions::default()).unwrap();
        let (a, b) = (a.to_str().unwrap(), dir.path().join("b.nii"));
        let b = b.to_str().unwrap();
        let args = ["3", a, b, "3x3x3", "1", "2", "1", "1"];
        let (code, _, err) = run_capture(&args, UpstreamDefects::Fix);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("an error (ANTS-160)")
                && err.contains("ANTS-160 (ResampleImage) corrected"),
            "{err}"
        );
        let (code, _, err) = run_capture(&args, UpstreamDefects::Reproduce);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("an error (ANTS-160)") && !err.contains("ANTS-160 (ResampleImage)"),
            "{err}"
        );
        assert!(!dir.path().join("b.nii").exists());
        // Three sigmas: no notice.
        let args = ["3", a, b, "3x3x3", "1", "2", "1x1x1", "1"];
        let (code, _, err) = run_capture(&args, UpstreamDefects::Fix);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("ANTS-160"), "{err}");
    }
}
