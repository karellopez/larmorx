// SPDX-License-Identifier: Apache-2.0
//! `SmoothImage` with its original arguments (ANTs v2.6.5, `Examples/SmoothImage.cxx`):
//!
//! ```text
//! SmoothImage ImageDimension image.ext smoothingsigma outimage.ext {sigma-is-in-spacing-units-(0)/1} {medianfilter-(0)/1}
//! ```
//!
//! As in ANTs:
//! - fewer than three arguments print the usage (exit 0 for `--help`/`-h`, else 1); a
//!   dimension other than 2, 3 or 4 prints "Unsupported dimension" and exits 1;
//! - the sigma is `ConvertVector<float>` (`1.5` or `1x1x2`); in voxels (multiplied by the
//!   spacing, in float) unless the fifth argument is non-zero (`std::stoi`);
//! - a sigma list whose length is neither 1 nor the dimension prints "Incorrect sigma vector
//!   size" and smooths with the filter's default, 1 mm, exit 0 (ANTS-15); with the median
//!   filter it leaves the radius uninitialised (ANTS-28);
//! - with a non-zero sixth argument, the sigma is the median filter's radius in voxels
//!   (truncated to an integer).
//!
//! Where ANTs crashes (a missing input, no output name: `std::string(nullptr)` throws), larmorx
//! prints a message and exits 1.
//!
//! **Upstream defects (D13).** By default a sigma list of the wrong length is an error (exit
//! 1). With `LARMORX_UPSTREAM_DEFECTS=reproduce` the Gaussian smooths with 1 mm as ANTs does,
//! and the median, whose radius ANTs leaves uninitialised (its result depends on memory),
//! uses a radius of 0 (the image unchanged), with a notice that says so.

use std::io::Write;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notices, UpstreamDefects};

use super::cstd::{convert_vector_f32, stoi};
use crate::image::{AntsImage, FileStore, ImageStore, ReadError, read_with, write_with};
use crate::smooth_image::{Smoothing, smooth_image};

const USAGE: &str = "Usage:
SmoothImage ImageDimension image.ext smoothingsigma outimage.ext {sigma-is-in-spacing-units-(0)/1} {medianfilter-(0)/1}
 If using median filter, sigma is the radius of filtering, in voxels
 A separate sigma can be specified for each dimension, e.g., 1.5x1x2
";

/// Runs `SmoothImage` on files, in the D13 mode of `LARMORX_UPSTREAM_DEFECTS`; returns the
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

/// Runs `SmoothImage` with images from `store` in the D13 mode `mode`.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec!["SmoothImage".to_owned()];
    argv.extend(args.iter().cloned());
    if argv.len() < 4 {
        let _ = out.write_all(USAGE.as_bytes());
        let help = matches!(argv.get(1).map(String::as_str), Some("--help" | "-h"));
        return if help { 0 } else { 1 };
    }
    let Some(dim) = stoi(&argv[1]) else {
        let _ = writeln!(
            err,
            "larmorx: SmoothImage: the dimension '{}' is not a number (ANTs aborts: std::stoi throws)",
            argv[1]
        );
        return 1;
    };
    if !(2..=4).contains(&dim) {
        let _ = writeln!(out, "Unsupported dimension");
        return 1;
    }
    let mut notices = Notices::new(mode);
    let code = match smooth(&argv, dim as usize, store, n_threads, &mut notices, err) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    };
    notices.print(err);
    code
}

fn crash(what: &str) -> String {
    format!("larmorx: SmoothImage: {what} (ANTs crashes here)")
}

fn smooth(
    argv: &[String],
    dim: usize,
    store: &mut dyn ImageStore,
    n_threads: usize,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<(), String> {
    let sigma = convert_vector_f32(&argv[3]).ok_or_else(|| {
        crash(&format!(
            "the sigma '{}' starts with an empty element",
            argv[3]
        ))
    })?;
    let image: AntsImage<f32> = match read_with::<f32, _>(store, &argv[2], dim, n_threads, notices)
    {
        Ok(i) => i,
        Err(ReadError::Unsupported { name, message }) => {
            return Err(format!("larmorx: {name}: {message}"));
        }
        Err(e) => {
            if let Some(m) = e.ants_message() {
                let _ = writeln!(err, "{m}");
            }
            return Err(crash(&format!("cannot read image '{}'", argv[2])));
        }
    };
    let flag = |i: usize| -> Result<bool, String> {
        match argv.get(i) {
            None => Ok(false),
            Some(s) => stoi(s).map(|v| v != 0).ok_or_else(|| {
                format!(
                    "larmorx: SmoothImage: '{s}' is not a number (ANTs aborts: std::stoi throws)"
                )
            }),
        }
    };
    let physical = flag(5)?;
    let use_median = flag(6)?;
    let incorrect = sigma.len() != 1 && sigma.len() != dim;
    if incorrect {
        let _ = writeln!(
            err,
            "Incorrect sigma vector size.  Must either be of size 1 or ImageDimension."
        );
        // ANTS-15: ANTs goes on (1 mm, or an uninitialised median radius) and exits 0.
        let what = format!("{} sigma(s) for a {dim}-dimensional image", sigma.len());
        if notices.fix() {
            notices.add(
                &defects::ANTS_15,
                format!(
                    "{what}: an error; ANTs {} and exits 0",
                    if use_median {
                        "filters with a median radius it leaves uninitialised"
                    } else {
                        "smooths with 1 mm"
                    }
                ),
            );
            return Err(format!(
                "larmorx: SmoothImage needs 1 or {dim} sigmas, not {} ({}=reproduce goes on as \
                 ANTs does)",
                sigma.len(),
                defects::ENV
            ));
        }
        notices.add(
            &defects::ANTS_15,
            if use_median {
                format!("{what}: the median filter runs, as in ANTs")
            } else {
                format!("{what}: the image is smoothed with 1 mm, as in ANTs")
            },
        );
    }
    let data = if use_median {
        let radius = if incorrect {
            // ANTS-28: ANTs leaves the radius uninitialised; its result depends on memory.
            notices.add(
                &defects::ANTS_28,
                "the median radius ANTs leaves uninitialised (its result depends on memory) is \
                 0 here: the image is copied unchanged",
            );
            vec![0usize; dim]
        } else {
            // `static_cast<unsigned long>(float)`: truncation; negative or NaN radii are
            // undefined (ANTs then cannot allocate the neighbourhood): an error.
            sigma
                .iter()
                .map(|&s| {
                    if (0.0..1.0e9).contains(&s) {
                        Ok(s as usize)
                    } else {
                        Err(crash(&format!("a median radius of {s} voxels")))
                    }
                })
                .collect::<Result<Vec<usize>, String>>()?
        };
        smooth_image(
            image.view(),
            Smoothing::Median { radius: &radius },
            n_threads,
        )
    } else {
        let sigma = (!incorrect).then_some(sigma.as_slice());
        smooth_image(
            image.view(),
            Smoothing::Gaussian { sigma, physical },
            n_threads,
        )
    }
    .map_err(|e| format!("larmorx: SmoothImage: {e} (ANTs aborts: ITK throws)"))?;
    let Some(name) = argv.get(4) else {
        return Err(crash("no output name (std::string(nullptr) throws)"));
    };
    write_with(store, name, image.derived(data).into(), n_threads, notices)
        .map_err(|e| format!("larmorx: cannot write '{name}': {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_and_dimension() {
        let run_capture = |args: &[&str]| {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = main(&args, &mut out, &mut err);
            (code, String::from_utf8(out).unwrap())
        };
        let (code, out) = run_capture(&["--help"]);
        assert_eq!(code, 0);
        assert!(out.contains("smoothingsigma"));
        assert_eq!(run_capture(&["3", "a.nii"]).0, 1);
        let (code, out) = run_capture(&["5", "a.nii", "1", "b.nii"]);
        assert_eq!((code, out.as_str()), (1, "Unsupported dimension\n"));
    }
}
