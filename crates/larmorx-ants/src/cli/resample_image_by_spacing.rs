// SPDX-License-Identifier: Apache-2.0
//! `ResampleImageBySpacing` with its original arguments (ANTs v2.6.5,
//! `Examples/ResampleImageBySpacing.cxx`):
//!
//! ```text
//! ResampleImageBySpacing ImageDimension inputImageFile outputImageFile outxspc outyspc {outzspacing} {dosmooth?} {addvox} {nn-interp?}
//! ```
//!
//! As in ANTs:
//! - fewer than four arguments print the usage (exit 0 for `--help`/`-h`, else 1);
//! - a dimension other than 2, 3 or 4 does nothing and exits 0;
//! - the spacings are read with `atof`, one per axis (4D: the time spacing too), then
//!   `dosmooth`, `addvox` and `nn` with `std::stoi`. **In 2D the `nn` flag is read from the
//!   `addvox` argument** (`argv[7]`, whenever an eighth argument exists), and **in 4D an
//!   `addvox` argument without an `nn` argument aborts ANTs** (`std::stoi(argv[10])` on a null
//!   pointer);
//! - with smoothing (the default), the sigma of every axis is computed from its spacing
//!   argument, so every spacing must be given (ANTs crashes on `atof(nullptr)` otherwise);
//! - it prints the input spacing, the new spacing, each axis's smoothing sigma, and the output
//!   size, as ITK formats them.

use std::io::Write;

use super::cstd::{atof, format_g, stoi};
use crate::image::{AntsImage, FileStore, ImageStore, ReadError, read_with, write_with};
use crate::resample_image_by_spacing::{ResampleBySpacingOptions, plan, resample_image_by_spacing};

const USAGE: &str = "Usage:
ResampleImageBySpacing  ImageDimension inputImageFile  outputImageFile outxspc outyspc {outzspacing}  {dosmooth?}  {addvox} {nn-interp?}
 addvox pads each dimension by addvox

";

/// Runs `ResampleImageBySpacing` on files; returns the exit code.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    run(args, &mut FileStore, super::itk_threads(), out, err)
}

/// Runs `ResampleImageBySpacing` with images from `store`.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec!["ResampleImageBySpacing".to_owned()];
    argv.extend(args.iter().cloned());
    if argv.len() < 5 {
        let _ = out.write_all(USAGE.as_bytes());
        let help = matches!(argv.get(1).map(String::as_str), Some("--help" | "-h"));
        return if help { 0 } else { 1 };
    }
    let Some(dim) = stoi(&argv[1]) else {
        let _ = writeln!(
            err,
            "larmorx: ResampleImageBySpacing: the dimension '{}' is not a number (ANTs aborts: std::stoi throws)",
            argv[1]
        );
        return 1;
    };
    if !(2..=4).contains(&dim) {
        // ANTs has no branch for other dimensions: nothing happens, exit 0.
        return 0;
    }
    match resample(&argv, dim as usize, store, n_threads, out, err) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

fn crash(what: &str) -> String {
    format!("larmorx: ResampleImageBySpacing: {what} (ANTs crashes here)")
}

/// ITK's `operator<<` for a vector: `[a, b, c]`, each with `std::cout`'s six digits.
fn vector(v: &[f64]) -> String {
    let items: Vec<String> = v.iter().map(|&x| format_g(x)).collect();
    format!("[{}]", items.join(", "))
}

fn resample(
    argv: &[String],
    dim: usize,
    store: &mut dyn ImageStore,
    n_threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), String> {
    let argc = argv.len();
    let image: AntsImage<f32> = match read_with::<f32, _>(store, &argv[2], dim, n_threads) {
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
    let input_spacing = image.geometry.spacing.clone();
    let _ = writeln!(out, " spacing {} dim {dim}", vector(&input_spacing));
    let int = |i: usize| -> Result<i32, String> {
        stoi(&argv[i]).ok_or_else(|| {
            format!(
                "larmorx: ResampleImageBySpacing: '{}' is not a number (ANTs aborts: std::stoi throws)",
                argv[i]
            )
        })
    };
    let mut spacing = input_spacing.clone();
    for (k, s) in spacing.iter_mut().enumerate() {
        if argc > 4 + k {
            *s = atof(&argv[4 + k]);
        }
    }
    // The flags follow the spacings: argv[4 + dim], argv[5 + dim], argv[6 + dim] (2D reads
    // `nn` from argv[7], the `addvox` argument; 4D reads it from argv[10] as soon as argc > 9).
    let mut smooth = true;
    if argc > 4 + dim {
        smooth = int(4 + dim)? != 0;
    }
    let mut add_voxels = 0;
    if argc > 5 + dim {
        add_voxels = int(5 + dim)?;
    }
    let mut nearest = false;
    match dim {
        2 if argc > 8 => nearest = int(7)? != 0,
        4 if argc > 9 => {
            if argc == 10 {
                return Err(crash(
                    "no nn argument after addvox (std::stoi(argv[10]) on a null pointer)",
                ));
            }
            nearest = int(10)? != 0;
        }
        3 if argc > 9 => nearest = int(9)? != 0,
        _ => {}
    }
    let _ = writeln!(out, " spacing2 {}", vector(&spacing));
    if smooth && argc < 4 + dim {
        return Err(crash(&format!(
            "smoothing needs all {dim} spacings (ANTs calls atof on a null pointer)"
        )));
    }
    let options = ResampleBySpacingOptions {
        spacing: spacing.clone(),
        smooth,
        add_voxels,
        nearest,
    };
    let planned = plan(&image.geometry, &options).map_err(|e| crash(&e.to_string()))?;
    for (k, &sigma) in planned.sigmas.iter().enumerate() {
        let _ = writeln!(
            out,
            " smoothing by : {} dir {k}",
            format_g(f64::from(sigma))
        );
    }
    let _ = writeln!(out, " out space {}", vector(&spacing));
    let sizes: Vec<String> = planned
        .geometry
        .size
        .iter()
        .map(ToString::to_string)
        .collect();
    let _ = writeln!(
        out,
        " output size [{}] spc {}",
        sizes.join(", "),
        vector(&spacing)
    );
    let (resampled, _) = resample_image_by_spacing(&image, &options, n_threads)
        .map_err(|e| format!("larmorx: ResampleImageBySpacing: {e} (ANTs aborts: ITK throws)"))?;
    write_with(store, &argv[3], resampled.into(), n_threads)
        .map_err(|e| format!("larmorx: cannot write '{}': {e}", argv[3]))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_and_vectors() {
        let args: Vec<String> = vec!["--help".into()];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(main(&args, &mut out, &mut err), 0);
        assert!(String::from_utf8(out).unwrap().contains("addvox"));
        assert_eq!(vector(&[1.0, 0.5, 2.25]), "[1, 0.5, 2.25]");
        // Dimension 5: ANTs does nothing and exits 0.
        let args: Vec<String> = ["5", "a.nii", "b.nii", "1", "1"].map(String::from).to_vec();
        assert_eq!(main(&args, &mut Vec::new(), &mut Vec::new()), 0);
    }
}
