// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-avwutils 2209.6 (fslroi.cc). Copyright (C) 1999-2023
// University of Oxford. Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the
// original. Distributed under the FSL Licence (see LICENSE).
//! `fslroi`: a sub-volume or a range of volumes of an image, in the input's type
//! (`closestTemplatedType`), its header's transforms moved to the region's corner. The
//! region's arithmetic, the type dispatch and the saving are the shared core's
//! ([`larmorx_nc_core::avwutils::fslroi`], which the `bet` and `standard_space_roi` scripts run
//! too); this is the program: its usage, its argument counts and its exit statuses.
//!
//! fslroi's `main` catches every exception as `std::exception`, prints its `what()` after
//! newimage's `Image Exception` line, and then falls off its end: the exit status is 0 (an
//! image that does not read, `Invalid ROI dimensions`, a time index outside the series), FSL-215;
//! corrected by default (status 1, the messages unchanged), reproduced with `reproduce`. The
//! usage (1), the output type's refusal (1) and the unsupported data types (255) give their own
//! status in both modes.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_nc_core::avwutils;
use larmorx_nc_core::settings::Settings;

const USAGE: &str = "
Usage: fslroi <input> <output> <xmin> <xsize> <ymin> <ysize> <zmin> <zsize>
       fslroi <input> <output> <tmin> <tsize>

       fslroi <input> <output> <xmin> <xsize> <ymin> <ysize> <zmin> <zsize> <tmin> <tsize>
Note: indexing (in both time and space) starts with 0 not 1! Inputting -1 for a size will set it to the full image extent for that dimension.
";

/// fslroi's `main` (`argv[0]` is the program name).
pub fn main(
    argv: &[String],
    fsl: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let reproduce = mode.reproduce();
    let argc = argv.len();
    if argc != 5 && argc != 9 && argc != 11 {
        let _ = out.write_all(USAGE.as_bytes());
        return 1;
    }
    let numbers: Vec<&str> = argv[3..].iter().map(String::as_str).collect();
    match avwutils::fslroi(&argv[1], &argv[2], &numbers, fsl, reproduce) {
        Ok(()) => 0,
        Err(f) => {
            let _ = err.write_all(f.err.as_bytes());
            if !f.exception {
                return f.code;
            }
            let detail = if reproduce {
                "exit status 0 after the error, as FSL's fslroi"
            } else {
                "exit status 1 after the error (FSL's fslroi exits 0)"
            };
            let n = Notice::new(&defects::FSL_215, mode, detail);
            let _ = writeln!(err, "larmorx-nc: {}", n.line());
            if reproduce { 0 } else { 1 }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> (u8, String, String) {
        let argv: Vec<String> = std::iter::once("fslroi")
            .chain(args.iter().copied())
            .map(str::to_owned)
            .collect();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let code = main(
            &argv,
            &Settings::new(),
            UpstreamDefects::Reproduce,
            &mut o,
            &mut e,
        );
        (
            code,
            String::from_utf8(o).unwrap(),
            String::from_utf8(e).unwrap(),
        )
    }

    #[test]
    fn usage_for_other_argument_counts() {
        for args in [
            &["a", "b"][..],
            &["a", "b", "0"],
            &["a", "b", "0", "1", "2"],
        ] {
            let (code, out, err) = run(args);
            assert_eq!((code, err.as_str()), (1, ""));
            assert!(out.starts_with("\nUsage: fslroi <input>"), "{out}");
            assert!(out.ends_with("for that dimension.\n"), "{out}");
        }
    }

    #[test]
    fn a_missing_image_exits_zero_after_its_message_in_reproduce() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing").display().to_string();
        let (code, out, err) = run(&[&missing, "o", "0", "1"]);
        assert_eq!((code, out.as_str()), (0, ""));
        assert!(
            err.starts_with("Image Exception : #63 :: No image files match:"),
            "{err}"
        );
    }
}
