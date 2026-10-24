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
//!
//! **Upstream defects (D13).** By default larmorx reads `nn` from its own argument in 2D and
//! 4D and refuses other dimensions (ANTS-31), refuses to smooth an axis of fewer than 4
//! voxels (ANTS-28, ITK's message), smooths by `out/in − 1` input voxels and gives the voxels
//! beyond the input 0 (ANTS-32; the sigma printed is the one the filter receives, in
//! millimetres). With `LARMORX_UPSTREAM_DEFECTS=reproduce` it reads the arguments as ANTs
//! does, exits 0 for other dimensions, resamples the smoothing's never-written buffer, filled
//! with 0, after printing "Exception catched !" as ANTs does, takes the sigma in millimetres
//! and gives the voxels beyond the input its value at index `(1, 1, …)`.

use std::io::Write;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notices, UpstreamDefects};

use super::cstd::{atof, format_g, stoi};
use crate::image::{AntsImage, FileStore, ImageStore, ReadError, read_with, write_with};
use crate::resample_image_by_spacing::{
    ResampleBySpacingOptions, plan, resample_image_by_spacing, unsmoothable_axes,
};

const USAGE: &str = "Usage:
ResampleImageBySpacing  ImageDimension inputImageFile  outputImageFile outxspc outyspc {outzspacing}  {dosmooth?}  {addvox} {nn-interp?}
 addvox pads each dimension by addvox

";

/// Runs `ResampleImageBySpacing` on files, in the D13 mode of `LARMORX_UPSTREAM_DEFECTS`;
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

/// Runs `ResampleImageBySpacing` with images from `store` in the D13 mode `mode`.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    mode: UpstreamDefects,
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
    let mut notices = Notices::new(mode);
    if !(2..=4).contains(&dim) {
        // ANTS-31: ANTs has no branch for other dimensions: nothing happens, exit 0.
        if notices.fix() {
            notices.add(
                &defects::ANTS_31,
                format!("dimension {dim}: an error; ANTs does nothing and exits 0"),
            );
            let _ = writeln!(
                err,
                "larmorx: ResampleImageBySpacing: dimension {dim} is not supported (2, 3 or 4)"
            );
            notices.print(err);
            return 1;
        }
        notices.add(
            &defects::ANTS_31,
            format!("dimension {dim}: nothing is done, exit 0, as in ANTs"),
        );
        notices.print(err);
        return 0;
    }
    let code = match resample(
        &argv,
        dim as usize,
        store,
        n_threads,
        &mut notices,
        out,
        err,
    ) {
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
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), String> {
    let argc = argv.len();
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
    // The flags follow the spacings: argv[4 + dim], argv[5 + dim], argv[6 + dim]. ANTS-31:
    // ANTs' 2D reads `nn` from argv[7], the `addvox` argument; its 4D reads it from argv[10]
    // as soon as argc > 9.
    let mut smooth = true;
    if argc > 4 + dim {
        smooth = int(4 + dim)? != 0;
    }
    let mut add_voxels = 0;
    if argc > 5 + dim {
        add_voxels = int(5 + dim)?;
    }
    // larmorx's reading: `nn` from argv[6 + dim].
    let own = if argc > 6 + dim {
        Some(int(6 + dim)? != 0)
    } else {
        None
    };
    let ants = match dim {
        2 if argc > 8 => Some(Some(int(7)? != 0)),
        // `std::stoi(argv[10])` on the null pointer after the last argument: an abort.
        4 if argc == 10 => Some(None),
        4 if argc > 10 => Some(Some(int(10)? != 0)),
        3 if argc > 9 => Some(Some(int(9)? != 0)),
        _ => None,
    };
    let nearest = if notices.fix() {
        let differs = match ants {
            Some(Some(a)) => a != own.unwrap_or(false),
            Some(None) => true,
            None => own.is_some_and(|o| o),
        };
        if differs {
            let ants_reading = match ants {
                Some(Some(true)) => "interpolates by nearest neighbour",
                Some(Some(false)) | None => "interpolates linearly",
                Some(None) => "aborts (it reads an argument that is not there)",
            };
            notices.add(
                &defects::ANTS_31,
                format!(
                    "nn is read from its own argument ({}); ANTs reads it from argument {} and \
                     {ants_reading}",
                    if own.unwrap_or(false) {
                        "nearest neighbour"
                    } else {
                        "linear"
                    },
                    if dim == 2 { 7 } else { 10 }
                ),
            );
        }
        own.unwrap_or(false)
    } else {
        match ants {
            Some(Some(a)) => {
                if a != own.unwrap_or(false) {
                    notices.add(
                        &defects::ANTS_31,
                        format!(
                            "nn is read from argument {}, as in ANTs: {}",
                            if dim == 2 { 7 } else { 10 },
                            if a { "nearest neighbour" } else { "linear" }
                        ),
                    );
                }
                a
            }
            Some(None) => {
                return Err(crash(
                    "no nn argument after addvox (std::stoi(argv[10]) on a null pointer)",
                ));
            }
            None => false,
        }
    };
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
    let thin = unsmoothable_axes(&image.geometry, &planned);
    for k in 0..planned.sigmas.len() {
        // The sigma the smoothing filter receives (millimetres; ANTS-32 corrected by default).
        let _ = writeln!(
            out,
            " smoothing by : {} dir {k}",
            format_g(planned.physical_sigma(k, &image.geometry, notices.mode()))
        );
        if thin.contains(&k) && notices.reproduce() {
            // ANTs catches ITK's exception here, prints it and goes on (ANTS-28). ITK's
            // printout holds two addresses and its build directory, which larmorx cannot
            // know: they are written as 0x0 and `<ITK source>`.
            let _ = writeln!(out, "Exception catched !");
            let _ = write!(
                out,
                "\nitk::ExceptionObject (0x0)\nLocation: \"unknown\" \nFile: <ITK source>/\
                 Modules/Filtering/ImageFilterBase/include/itkRecursiveSeparableImageFilter.hxx\n\
                 Line: 223\nDescription: ITK ERROR: RecursiveGaussianImageFilter(0x0): The \
                 number of pixels along direction {k} is less than 4. This filter requires a \
                 minimum of four pixels along the dimension to be processed.\n\n\n"
            );
        }
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
    if !thin.is_empty() && notices.fix() {
        notices.add(
            &defects::ANTS_28,
            format!(
                "axis {} cannot be smoothed (fewer than 4 voxels): an error; ANTs resamples the \
                 smoothing's never-written buffer",
                thin[0]
            ),
        );
    }
    let (resampled, _, resample_notices) =
        resample_image_by_spacing(&image, &options, n_threads, notices.mode()).map_err(|e| {
            format!(
                "larmorx: ResampleImageBySpacing: {e} (ANTs {})",
                if thin.is_empty() {
                    "aborts: ITK throws"
                } else {
                    "catches ITK's exception and resamples a never-written buffer; \
                     LARMORX_UPSTREAM_DEFECTS=reproduce fills it with 0"
                }
            )
        })?;
    notices.extend(resample_notices);
    write_with(store, &argv[3], resampled.into(), n_threads, notices)
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
        // Dimension 5: ANTs does nothing and exits 0 (ANTS-31); larmorx refuses it.
        let args: Vec<String> = ["5", "a.nii", "b.nii", "1", "1"].map(String::from).to_vec();
        let mut store = FileStore;
        let reproduce = UpstreamDefects::Reproduce;
        let mut err = Vec::new();
        assert_eq!(
            run(&args, &mut store, 1, reproduce, &mut Vec::new(), &mut err),
            0
        );
        assert!(String::from_utf8(err).unwrap().contains("ANTS-31"));
        let mut err = Vec::new();
        let fix = UpstreamDefects::Fix;
        assert_eq!(run(&args, &mut store, 1, fix, &mut Vec::new(), &mut err), 1);
        assert!(String::from_utf8(err).unwrap().contains("ANTS-31"));
    }
}
