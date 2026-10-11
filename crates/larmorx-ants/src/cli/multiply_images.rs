// SPDX-License-Identifier: Apache-2.0
//! `MultiplyImages` with its original arguments (ANTs v2.6.5, `Examples/MultiplyImages.cxx`):
//!
//! ```text
//! MultiplyImages ImageDimension img1.nii img2.nii product.nii
//! MultiplyImages ImageDimension img1.nii 2.5 product.nii
//! ```
//!
//! As in ANTs:
//! - fewer than three arguments print the usage (exit 0 for `--help`/`-h`, else 1);
//! - the second operand is read as an image if it can be, else taken as a number with
//!   `atof`; **a missing file therefore multiplies by 0**, silently (ANTS-27: larmorx refuses
//!   an argument that is neither a readable image nor a number, unless
//!   `LARMORX_UPSTREAM_DEFECTS=reproduce`);
//! - the product is computed at the first image's voxel indices, in float, and written with
//!   the first image's geometry (the second image's is ignored);
//! - no output name (`MultiplyImages 3 a.nii b.nii`) aborts ANTs ("missing output filename",
//!   then `throw;`); larmorx exits 1.
//!
//! Only scalar images are supported (ANTs also multiplies vector and tensor images component
//! by component), in 2, 3 or 4 dimensions (ANTs also accepts 1).

use std::io::Write;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notices, UpstreamDefects};

use super::cstd::{StrictNumber, atof, stoi, strict_f32};
use crate::image::{AntsImage, FileStore, ImageStore, ReadError, from_itk};
use crate::image_math::Operand;
use crate::multiply_images::multiply_images;

const USAGE: &str = "\
Usage:
MultiplyImages ImageDimension img1.nii img2.nii product.nii {smoothing}
 2nd image file may also be floating point numerical value, and program will act accordingly -- i.e. read as a number.
 Program handles vector and tensor images as well (larmorx: scalar images only)
";

/// Runs `MultiplyImages` on files, in the D13 mode of `LARMORX_UPSTREAM_DEFECTS`; returns the
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

/// Runs `MultiplyImages` with images from `store` in the D13 mode `mode`.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec!["MultiplyImages".to_owned()];
    argv.extend(args.iter().cloned());
    if argv.len() < 4 {
        let _ = out.write_all(USAGE.as_bytes());
        let help = matches!(argv.get(1).map(String::as_str), Some("--help" | "-h"));
        return if help { 0 } else { 1 };
    }
    let mut notices = Notices::new(mode);
    let code = match multiply(&argv, store, n_threads, &mut notices, out) {
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

fn multiply(
    argv: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    notices: &mut Notices,
    out: &mut dyn Write,
) -> Result<(), String> {
    let dim = stoi(&argv[1]).ok_or_else(|| {
        format!(
            "larmorx: MultiplyImages: the dimension '{}' is not a number (ANTs aborts: std::stoi throws)",
            argv[1]
        )
    })?;
    // ANTs opens the first image (`CreateImageIO`) before checking the dimension; a missing
    // or unreadable one crashes it.
    let raw = match store.load(&argv[2], n_threads, notices) {
        Ok(i) => i,
        Err(ReadError::Unsupported { name, message }) => {
            return Err(format!("larmorx: {name}: {message}"));
        }
        Err(_) => {
            return Err(format!(
                "larmorx: MultiplyImages: cannot read image '{}' (ANTs crashes here)",
                argv[2]
            ));
        }
    };
    if !(2..=4).contains(&dim) {
        if dim == 1 {
            return Err("larmorx: MultiplyImages: dimension 1 is not supported yet".into());
        }
        let _ = writeln!(out, " not supported {dim}");
        return Err(String::new());
    }
    let dim = dim as usize;
    if argv.len() < 5 {
        let _ = writeln!(out, "missing output filename");
        return Err(
            "larmorx: MultiplyImages: missing output filename (ANTs aborts: throw without an exception)"
                .into(),
        );
    }
    let first: AntsImage<f32> =
        from_itk(raw, &argv[2], dim, n_threads, notices).map_err(|e| e.to_string())?;
    // The second operand: an image if ITK's reader reads it, else a number (`atof`).
    let second: Option<AntsImage<f32>> = match store.load(&argv[3], n_threads, notices) {
        Ok(raw) => {
            Some(from_itk(raw, &argv[3], dim, n_threads, notices).map_err(|e| e.to_string())?)
        }
        Err(ReadError::Unsupported { name, message })
            if strict_f32(&argv[3]) == StrictNumber::NotANumber =>
        {
            return Err(format!("larmorx: {name}: {message}"));
        }
        Err(_) => None,
    };
    if second.is_none() && strict_f32(&argv[3]) == StrictNumber::NotANumber {
        // ANTS-27: neither an image nor a number; ANTs multiplies by atof of the name.
        let value = atof(&argv[3]) as f32;
        if notices.fix() {
            notices.add(
                &defects::ANTS_27,
                format!(
                    "'{}' is neither a readable image nor a number: an error; ANTs multiplies \
                     by {value}",
                    argv[3]
                ),
            );
            return Err(format!(
                "larmorx: MultiplyImages: cannot read '{}' as an image, and it is not a number \
                 ({}=reproduce multiplies by {value}, as ANTs does)",
                argv[3],
                defects::ENV
            ));
        }
        notices.add(
            &defects::ANTS_27,
            format!(
                "'{}' is neither a readable image nor a number: multiplied by {value}, as ANTs \
                 does",
                argv[3]
            ),
        );
    }
    let operand = match &second {
        Some(i) => Operand::Image {
            data: &i.data,
            size: i.size(),
        },
        None => Operand::Scalar(atof(&argv[3]) as f32),
    };
    let (product, product_notices) = multiply_images(
        &first.data,
        first.size(),
        operand,
        n_threads,
        notices.mode(),
    )
    .map_err(|e| format!("larmorx: MultiplyImages: {e}"))?;
    notices.extend(product_notices);
    let output = first.derived(product);
    store
        .save(&argv[4], &output.into(), n_threads)
        .map_err(|e| format!("larmorx: cannot write '{}': {e}", argv[4]))
}
