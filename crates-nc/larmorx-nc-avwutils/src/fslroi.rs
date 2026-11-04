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
//! status in both modes. An int8 input is written as int8 with its values by default, as FSL's
//! uint8 with the same bytes in `reproduce` (FSL-217, named).

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
        Ok(None) => 0,
        Ok(Some(int8)) => {
            // FSL-217: the corrected output's type always differs from FSL's uint8; in
            // `reproduce` named where FSL's bytes change a value
            if int8.kept || int8.negative > 0 {
                let n = Notice::new(&defects::FSL_217, mode, int8.detail("fslroi"));
                let _ = writeln!(err, "larmorx-nc: {}", n.line());
            }
            0
        }
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

    /// A 4 x 1 x 1 int8 `.nii` of `values`.
    fn int8_nii(path: &std::path::Path, values: &[i8]) {
        let mut h = vec![0u8; 352];
        h[0..4].copy_from_slice(&348i32.to_le_bytes());
        let dims: [i16; 8] = [3, values.len() as i16, 1, 1, 1, 1, 1, 1];
        for (k, d) in dims.iter().enumerate() {
            h[40 + 2 * k..42 + 2 * k].copy_from_slice(&d.to_le_bytes());
        }
        h[70..72].copy_from_slice(&256i16.to_le_bytes()); // DT_INT8
        h[72..74].copy_from_slice(&8i16.to_le_bytes());
        for (k, p) in [1.0f32, 1.0, 1.0, 1.0].iter().enumerate() {
            h[76 + 4 * k..80 + 4 * k].copy_from_slice(&p.to_le_bytes());
        }
        h[108..112].copy_from_slice(&352f32.to_le_bytes());
        h[112..116].copy_from_slice(&1f32.to_le_bytes());
        h[344..348].copy_from_slice(b"n+1\0");
        h.extend(values.iter().map(|&v| v as u8));
        std::fs::write(path, h).unwrap();
    }

    /// FSL-217: an int8 input written as int8 with its values by default, as FSL's uint8 with
    /// the same bytes in `reproduce`; named in both modes when a value is negative.
    #[test]
    fn an_int8_input_keeps_its_type_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("i8.nii");
        int8_nii(&input, &[-64, 0, 5, 127]);
        let fsl = Settings::new().with("FSLOUTPUTTYPE", "NIFTI");
        for (mode, code, named) in [
            (UpstreamDefects::Fix, 256i16, "int8 image written as int8"),
            (
                UpstreamDefects::Reproduce,
                2i16,
                "1 negative values became 128 to 255",
            ),
        ] {
            let out = dir.path().join(format!("o{}", code));
            let argv: Vec<String> = [
                "fslroi",
                &input.display().to_string(),
                &out.display().to_string(),
                "0",
                "3",
                "0",
                "1",
                "0",
                "1",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect();
            let (mut o, mut e) = (Vec::new(), Vec::new());
            assert_eq!(main(&argv, &fsl, mode, &mut o, &mut e), 0);
            let e = String::from_utf8(e).unwrap();
            assert!(e.contains("FSL-217") && e.contains(named), "{e}");
            let bytes = std::fs::read(out.with_extension("nii")).unwrap();
            assert_eq!(i16::from_le_bytes([bytes[70], bytes[71]]), code);
            let off = f32::from_le_bytes(bytes[108..112].try_into().unwrap()) as usize;
            // x 0..2 of [-64, 0, 5, 127]: the same bytes in both modes, -64 or (uint8) 192
            assert_eq!(&bytes[off..off + 3], &[0xC0u8, 0, 5]);
        }
        // a region without negative values: named by default only (the type differs)
        for (mode, named) in [
            (UpstreamDefects::Fix, true),
            (UpstreamDefects::Reproduce, false),
        ] {
            let out = dir.path().join("p");
            let argv: Vec<String> = [
                "fslroi",
                &input.display().to_string(),
                &out.display().to_string(),
                "2",
                "2",
                "0",
                "1",
                "0",
                "1",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect();
            let (mut o, mut e) = (Vec::new(), Vec::new());
            assert_eq!(main(&argv, &fsl, mode, &mut o, &mut e), 0);
            let e = String::from_utf8(e).unwrap();
            assert_eq!(e.contains("FSL-217"), named, "{e}");
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
