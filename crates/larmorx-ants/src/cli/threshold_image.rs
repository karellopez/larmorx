// SPDX-License-Identifier: Apache-2.0
//! `ThresholdImage` with its original arguments (ANTs v2.6.5, `Examples/ThresholdImage.cxx`):
//!
//! ```text
//! ThresholdImage ImageDimension ImageIn.ext outImage.ext threshlo threshhi <insideValue> <outsideValue>
//! ThresholdImage ImageDimension ImageIn.ext outImage.ext Otsu NumberofThresholds <maskImage.ext>
//! ThresholdImage ImageDimension ImageIn.ext outImage.ext Kmeans NumberofThresholds <maskImage.ext>
//! ```
//!
//! As in ANTs:
//! - fewer than two arguments print the usage (exit 0 for `--help`/`-h`, else 1); a
//!   dimension other than 2, 3 or 4 prints "Unsupported dimension" and exits 1;
//! - the sixth argument is always read as a mask image first, even for plain thresholds
//!   (so `... 0.5 1 255 0` prints " file 255 does not exist . "); names shorter than 3
//!   characters fail silently;
//! - any mode other than exactly `Otsu` or `Kmeans` is a range threshold, with the bounds
//!   read by `atof` and stored as float; inside defaults to 1, outside to 0;
//! - a lower bound above the upper one aborts ANTs (ITK throws); larmorx exits 1.
//!
//! `Kmeans` is not supported yet.

use std::io::Write;

use super::cstd::{atof, stoi};
use crate::image::{AntsImage, FileStore, ImageStore, read_with, write_with};
use crate::threshold_image::{ThresholdMode, threshold_image};

const USAGE: &str = "\
Usage: ThresholdImage   ImageDimension ImageIn.ext outImage.ext  threshlo threshhi <insideValue> <outsideValue>
   ImageDimension ImageIn.ext outImage.ext  Otsu NumberofThresholds <maskImage.ext>
   ImageDimension ImageIn.ext outImage.ext  Kmeans NumberofThresholds <maskImage.ext> (not supported by larmorx yet)
 Inclusive thresholds
";

/// Runs `ThresholdImage` on files; returns the exit code.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    run(args, &mut FileStore, super::itk_threads(), out, err)
}

/// Runs `ThresholdImage` with images from `store`.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut argv = vec!["ThresholdImage".to_owned()];
    argv.extend(args.iter().cloned());
    if argv.len() < 3 {
        let _ = out.write_all(USAGE.as_bytes());
        let help = matches!(argv.get(1).map(String::as_str), Some("--help" | "-h"));
        return if help { 0 } else { 1 };
    }
    let Some(dim) = stoi(&argv[1]) else {
        let _ = writeln!(
            err,
            "larmorx: ThresholdImage: the dimension '{}' is not a number (ANTs aborts: std::stoi throws)",
            argv[1]
        );
        return 1;
    };
    if !(2..=4).contains(&dim) {
        let _ = writeln!(out, "Unsupported dimension");
        return 1;
    }
    match threshold(&argv, dim as usize, store, n_threads, err) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

fn threshold(
    argv: &[String],
    dim: usize,
    store: &mut dyn ImageStore,
    n_threads: usize,
    err: &mut dyn Write,
) -> Result<(), String> {
    let mut read = |name: &str, err: &mut dyn Write| -> Result<Option<AntsImage<f32>>, String> {
        match read_with::<f32, _>(store, name, dim, n_threads) {
            Ok(i) => Ok(Some(i)),
            Err(crate::image::ReadError::Unsupported { name, message }) => {
                Err(format!("larmorx: {name}: {message}"))
            }
            Err(e) => {
                if let Some(m) = e.ants_message() {
                    let _ = writeln!(err, "{m}");
                }
                Ok(None)
            }
        }
    };
    let fixed = read(&argv[2], err)?;
    let mut mask: Option<AntsImage<i32>> = None;
    if argv.len() > 6 {
        match read_with::<i32, _>(store, &argv[6], dim, n_threads) {
            Ok(m) => mask = Some(m),
            Err(crate::image::ReadError::Unsupported { name, message }) => {
                return Err(format!("larmorx: {name}: {message}"));
            }
            Err(e) => {
                if let Some(m) = e.ants_message() {
                    let _ = writeln!(err, "{m}");
                }
            }
        }
    }
    let crash = |what: &str| format!("larmorx: ThresholdImage: {what} (ANTs crashes here)");
    let mode_name = argv
        .get(4)
        .ok_or_else(|| crash("missing threshold arguments"))?;
    let Some(fixed) = fixed else {
        return Err(crash(&format!("cannot read image '{}'", argv[2])));
    };
    let count = |s: Option<&String>| -> Result<usize, String> {
        let s = s.ok_or_else(|| crash("missing the number of thresholds"))?;
        let n =
            stoi(s).ok_or_else(|| crash(&format!("'{s}' is not a number (std::stoi throws)")))?;
        usize::try_from(n).map_err(|_| crash(&format!("{n} thresholds")))
    };
    let output_image: AntsImage<f32> = match mode_name.as_str() {
        "Otsu" => {
            let n = count(argv.get(5))?;
            match &mask {
                Some(m) => {
                    if m.size() != fixed.size() {
                        return Err(crash(&format!(
                            "the mask size {:?} differs from the image size {:?}",
                            m.size(),
                            fixed.size()
                        )));
                    }
                    let r = threshold_image(
                        &fixed.data,
                        ThresholdMode::Otsu {
                            thresholds: n,
                            mask: Some(&m.data),
                        },
                        n_threads,
                    )
                    .map_err(|e| crash(&e.to_string()))?;
                    // The output takes the mask's geometry (`CopyInformation(maskImage)`).
                    m.derived(r.data)
                }
                None => {
                    let r = threshold_image(
                        &fixed.data,
                        ThresholdMode::Otsu {
                            thresholds: n,
                            mask: None,
                        },
                        n_threads,
                    )
                    .map_err(|e| crash(&e.to_string()))?;
                    fixed.derived(r.data)
                }
            }
        }
        "Kmeans" => {
            return Err(
                "larmorx: ThresholdImage Kmeans is not supported yet (Otsu and range thresholds are)"
                    .into(),
            );
        }
        _ => {
            let upper = argv
                .get(5)
                .ok_or_else(|| crash("missing the upper threshold"))?;
            let inside = argv.get(6).map_or(1.0, |s| atof(s) as f32);
            let outside = argv.get(7).map_or(0.0, |s| atof(s) as f32);
            let r = threshold_image(
                &fixed.data,
                ThresholdMode::Range {
                    lower: atof(mode_name) as f32,
                    upper: atof(upper) as f32,
                    inside,
                    outside,
                },
                n_threads,
            )
            .map_err(|e| format!("larmorx: ThresholdImage: {e} (ANTs aborts: ITK throws)"))?;
            fixed.derived(r.data)
        }
    };
    let name = argv
        .get(3)
        .ok_or_else(|| crash("missing the output name"))?;
    write_with(store, name, output_image.into(), n_threads)
        .map_err(|e| format!("larmorx: cannot write '{name}': {e}"))?;
    Ok(())
}
