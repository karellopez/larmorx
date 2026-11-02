// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl rmsdiff`: FSL's `rmsdiff` (`specs/flirt.md` §15.3), the RMS deviation between
//! two matrices in millimetres.

use std::io::Write;

use super::Settings;
use super::common::{self, abort, print_notices};
use crate::flirt::mat::{self, Mat4};
use crate::flirt::programs;

const USAGE: &str = "Usage: rmsdiff matrixfile1 matrixfile2 refvol [mask]\n        Outputs rms deviation between matrices (in mm)\n";

/// `rmsdiff` with FSL's arguments; returns the exit status.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl rmsdiff: {e}");
            return 2;
        }
    };
    if args.len() < 3 {
        let _ = write!(err, "{USAGE}");
        return 255;
    }
    let mut mats: Vec<Mat4> = Vec::new();
    for (k, name) in args[..2].iter().enumerate() {
        let rows = match common::read_rows(settings, name, err) {
            Ok(r) => r,
            Err(m) => return abort(err, "rmsdiff", "std::runtime_error", &m),
        };
        if rows.len() < 4 {
            let _ = writeln!(err, "Could not read matrix {name}");
            return 254;
        }
        let Some(m) = common::exact4(&rows) else {
            let (r, c) = common::size(&rows);
            return common::refuse(
                err,
                "rmsdiff",
                &format!(
                    "{name} holds a {r} x {c} matrix; rmsdiff needs a 4 x 4 one (FSL aborts at its determinant)"
                ),
            );
        };
        if programs::determinant(&m).abs() < 0.1 {
            let _ = writeln!(
                err,
                "WARNING:: matrix {} has low determinant\n{}",
                k + 1,
                mat::print_text(&m)
            );
        }
        mats.push(m);
    }
    let mut notices = Vec::new();
    let code = run(&mats, args, settings, mode, &mut notices, out, err);
    print_notices(err, &notices);
    code
}

fn run(
    mats: &[Mat4],
    args: &[String],
    settings: &Settings,
    mode: larmorx_core::defects::UpstreamDefects,
    notices: &mut Vec<larmorx_core::defects::Notice>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let read = |name: &str, notices: &mut Vec<_>, err: &mut dyn Write| {
        common::read_volume(settings, name, mode, notices, err).map_err(|e| {
            let (lines, what) = common::image_exception(name, &e);
            let _ = write!(err, "{lines}");
            abort(err, "rmsdiff", "std::runtime_error", &what)
        })
    };
    let reference = match read(&args[2], notices, err) {
        Ok(v) => v,
        Err(code) => return code,
    };
    match args.get(3) {
        Some(name) => {
            let mask = match read(name, notices, err) {
                Ok(v) => v,
                Err(code) => return code,
            };
            let (largest, rms) = programs::mask_deviation(
                &mats[0],
                &mats[1],
                &mask.data,
                mask.shape,
                reference.voxel_size,
            );
            let _ = writeln!(out, "{}\n{}", mat::g(largest, 6), mat::g(rms, 6));
        }
        None => {
            let c = common::centre(&reference, "rmsdiff", mode, notices, err);
            match programs::rms_deviation(&mats[0], &mats[1], c, 80.0) {
                Some(r) => {
                    let _ = writeln!(out, "{}", mat::g(f64::from(r as f32), 6));
                }
                None => {
                    let _ = writeln!(err, "RMS_DEVIATION ERROR:: Could not invert matrix");
                    return 251;
                }
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_and_matrices() {
        let d = tempfile::tempdir().unwrap();
        let settings = Settings {
            dir: Some(d.path().to_path_buf()),
            ..Settings::default()
        };
        let call = |words: &[&str]| {
            let args: Vec<String> = words.iter().map(|s| (*s).to_owned()).collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = main(&args, &settings, &mut out, &mut err);
            (
                code,
                String::from_utf8(out).unwrap(),
                String::from_utf8(err).unwrap(),
            )
        };
        assert_eq!(call(&[]), (255, String::new(), USAGE.to_owned()));
        assert_eq!(
            call(&["none.mat", "b", "c"]),
            (
                254,
                String::new(),
                "Could not open matrix file none.mat\nCould not read matrix none.mat\n".to_owned()
            )
        );
        std::fs::write(
            d.path().join("s.mat"),
            "1 0 0 0\n0 0 0 0\n0 0 1 0\n0 0 0 1\n",
        )
        .unwrap();
        std::fs::write(
            d.path().join("i.mat"),
            "1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n",
        )
        .unwrap();
        let (c, _, err) = call(&["i.mat", "s.mat", "none.nii"]);
        assert_eq!(c, 1);
        assert!(err.starts_with("WARNING:: matrix 2 has low determinant\n1.000000 0.000000 0.000000 0.000000 \n0.000000 0.000000 0.000000 0.000000 \n"), "{err}");
    }
}
