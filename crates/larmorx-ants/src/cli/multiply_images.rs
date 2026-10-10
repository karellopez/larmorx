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
//!   `atof`; **a missing file therefore multiplies by 0**, silently;
//! - the product is computed at the first image's voxel indices, in float, and written with
//!   the first image's geometry (the second image's is ignored);
//! - no output name (`MultiplyImages 3 a.nii b.nii`) aborts ANTs ("missing output filename",
//!   then `throw;`); larmorx exits 1.
//!
//! Only scalar images are supported (ANTs also multiplies vector and tensor images component
//! by component), in 2, 3 or 4 dimensions (ANTs also accepts 1).

use std::io::Write;

use super::cstd::{atof, stoi};
use crate::image::{AntsImage, FileStore, ImageStore, ReadError, from_itk, read_with};
use crate::image_math::Operand;
use crate::multiply_images::multiply_images;

const USAGE: &str = "\
Usage:
MultiplyImages ImageDimension img1.nii img2.nii product.nii {smoothing}
 2nd image file may also be floating point numerical value, and program will act accordingly -- i.e. read as a number.
 Program handles vector and tensor images as well (larmorx: scalar images only)
";

/// Runs `MultiplyImages` on files; returns the exit code.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    run(args, &mut FileStore, super::itk_threads(), out, err)
}

/// Runs `MultiplyImages` with images from `store`.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
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
    match multiply(&argv, store, n_threads, out) {
        Ok(()) => 0,
        Err(message) => {
            if !message.is_empty() {
                let _ = writeln!(err, "{message}");
            }
            1
        }
    }
}

fn multiply(
    argv: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
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
    let raw = match store.load(&argv[2], n_threads) {
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
    let first: AntsImage<f32> = from_itk(raw, &argv[2], dim).map_err(|e| e.to_string())?;
    // The second operand: an image if it reads as one, else a number (`atof`).
    let second: Option<AntsImage<f32>> = match read_with(store, &argv[3], dim, n_threads) {
        Ok(i) => Some(i),
        Err(ReadError::Unsupported { name, message }) => {
            return Err(format!("larmorx: {name}: {message}"));
        }
        Err(_) => None,
    };
    let operand = match &second {
        Some(i) => Operand::Image {
            data: &i.data,
            size: i.size(),
        },
        None => Operand::Scalar(atof(&argv[3]) as f32),
    };
    let product = multiply_images(&first.data, first.size(), operand, n_threads)
        .map_err(|e| format!("larmorx: MultiplyImages: {e} (ANTs reads outside the image)"))?;
    let output = first.derived(product);
    store
        .save(&argv[4], &output.into(), n_threads)
        .map_err(|e| format!("larmorx: cannot write '{}': {e}", argv[4]))
}
