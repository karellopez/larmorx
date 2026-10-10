// SPDX-License-Identifier: Apache-2.0
//! `ResampleImage` with its original arguments (ANTs v2.6.5, `Examples/ResampleImage.cxx`):
//!
//! ```text
//! ResampleImage imageDimension inputImage outputImage MxNxO [size=1,spacing=0] [interpolate type] [pixeltype]
//! ```
//!
//! As in ANTs:
//! - fewer than four arguments print the usage (exit 0 for `--help`/`-h`, else 1);
//! - **the seventh argument is read twice**: as the pixel type (`std::stoi`: 0 `char`,
//!   1 `unsigned char`, 2 `short`, 3 `unsigned short`, 4 `int`, 5 `unsigned int`, 6 `float`,
//!   7 `double`; anything else prints "Unsupported pixel type") and as the interpolator's
//!   parameter: the Gaussian's sigmas (`1x1x1`, so a Gaussian with sigmas starting with 1
//!   resamples an `unsigned char` image), the windowed sinc's window (its first character,
//!   which `std::stoi` cannot read, so only the default Hamming window is reachable) or the
//!   B-spline order (`3` gives order 3 and `unsigned short` pixels; 6 or 7 give order 3);
//! - the interpolation type is 0 linear, 1 nearest neighbour, 2 Gaussian (sigmas, then alpha
//!   in the eighth argument), 3 windowed sinc, 4 B-spline; other numbers are linear;
//! - a dimension other than 2, 3 or 4 prints "Unsupported dimension" and exits 1;
//! - a spacing or size list whose length is neither 1 nor the dimension prints "Invalid
//!   spacing." or "Invalid size." and leaves the grid uninitialised (larmorx stops).
//!
//! Gaussian, windowed-sinc and B-spline interpolation are supported in 3D; 2D and 4D images
//! are resampled with linear or nearest-neighbour interpolation.

use std::io::Write;

use larmorx_core::RealElement;
use larmorx_interp::{Interpolation, Window};

use super::cstd::{atof, convert_vector_f64, stoi};
use crate::image::{AntsImage, FileStore, ImageStore, OutputImage, Pixel, ReadError, read_with};
use crate::resample_image::{ResampleTarget, resample_image};

const USAGE: &str = "Usage: ResampleImage imageDimension inputImage outputImage MxNxO [size=1,spacing=0] [interpolate type] [pixeltype]
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
";

/// Runs `ResampleImage` on files; returns the exit code.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    run(args, &mut FileStore, super::itk_threads(), out, err)
}

/// Runs `ResampleImage` with images from `store`.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
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
    let result = (|| -> Result<(), String> {
        // `unsigned int typeoption = std::stoi(argv[7])`.
        let type_option = match argv.get(7) {
            None => 6,
            Some(s) => stoi_arg(s)? as u32,
        };
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
    match result {
        Ok(()) => 0,
        Err(message) => {
            if !message.is_empty() {
                let _ = writeln!(err, "{message}");
            }
            1
        }
    }
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
        let image: AntsImage<T> = match read_with::<T, _>(self.store, &argv[2], dim, self.n_threads)
        {
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
        // `static_cast<unsigned int>(double)` (GCC: a 64-bit truncation, low 32 bits).
        let sizes: Vec<usize> = sp
            .iter()
            .map(|&v| crate::image::x86_to_i64(v) as u32 as usize)
            .collect();
        let target = if by_spacing {
            ResampleTarget::Spacing(&sp)
        } else {
            ResampleTarget::Size(&sizes)
        };
        let interpolation = self.interpolation(&image)?;
        let resampled = resample_image(&image, target, &interpolation, self.n_threads)
            .map_err(|e| format!("larmorx: ResampleImage: {e}"))?;
        crate::image::write_with(self.store, &argv[3], resampled.into(), self.n_threads)
            .map_err(|e| format!("larmorx: cannot write '{}': {e}", argv[3]))?;
        Ok(())
    }

    /// The interpolator ANTs chooses from the sixth to eighth arguments.
    fn interpolation<T>(&self, image: &AntsImage<T>) -> Result<Interpolation, String> {
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
                        return Err(crash(&format!(
                            "{} Gaussian sigmas for {dim} dimensions (ANTs reads past them)",
                            sg.len()
                        )));
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
                    // 'b' gives Lanczos too: ANTs' Blackman interpolator is a second Lanczos.
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
    fn usage_types_and_dimensions() {
        let (code, out, _) = run_capture(&["--help"]);
        assert_eq!(code, 0);
        assert!(out.contains("pixeltype"));
        assert_eq!(run_capture(&["3", "a.nii", "b.nii"]).0, 1);
        let (code, out, _) = run_capture(&["3", "a.nii", "b.nii", "1", "0", "0", "9"]);
        assert_eq!((code, out.as_str()), (1, "Unsupported pixel type\n"));
        let (code, out, _) = run_capture(&["7", "a.nii", "b.nii", "1"]);
        assert_eq!((code, out.as_str()), (1, "Unsupported dimension\n"));
        let (code, _, err) = run_capture(&["3", "a.nii", "b.nii", "1", "0", "3", "l"]);
        assert_eq!(code, 1);
        assert!(err.contains("std::stoi"), "{err}");
    }
}
