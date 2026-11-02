// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl avscale`: FSL's `avscale` (`specs/flirt.md` §15.1), an affine matrix
//! decomposed into rotations, translations, scales and skews, with its half transforms.

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;

use super::Settings;
use super::common::{self, abort, print_notices};
use crate::flirt::mat::{self, Stream};
use crate::flirt::programs;

const USAGE: &str =
    "Usage: avscale [--allparams/--inverteddies] matrixfile [non-reference-volume]\n";

fn row(stream: &Stream, v: &[f64]) -> String {
    v.iter().map(|&x| format!("{} ", stream.num(x))).collect()
}

/// `avscale` with FSL's arguments; returns the exit status.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl avscale: {e}");
            return 2;
        }
    };
    if args.is_empty() {
        let _ = write!(err, "{USAGE}");
        return 255;
    }
    let (option, first) = if args[0].starts_with('-') {
        (args[0].as_str(), 1)
    } else {
        ("", 0)
    };
    let (all_params, eddies) = (option == "--allparams", option == "--inverteddies");
    let Some(matrix) = args.get(first) else {
        let _ = writeln!(err, "Image error");
        return abort(
            err,
            "avscale",
            "std::logic_error",
            "basic_string: construction from null is not valid",
        );
    };
    let mut notices = Vec::new();
    let code = run(
        matrix,
        args.get(first + 1).map(String::as_str),
        (all_params, eddies),
        settings,
        mode,
        &mut notices,
        out,
        err,
    );
    print_notices(err, &notices);
    code
}

#[allow(clippy::too_many_arguments)]
fn run(
    matrix: &str,
    volume: Option<&str>,
    (all_params, eddies): (bool, bool),
    settings: &Settings,
    mode: UpstreamDefects,
    notices: &mut Vec<larmorx_core::defects::Notice>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    // the volume first (an empty name is not read), then the matrix; FSL catches what they
    // throw, prints its message and throws it again
    let vol = match volume {
        Some(name) if !name.is_empty() => {
            match common::read_volume(settings, name, mode, notices, err) {
                Ok(v) => Some(v),
                Err(e) => {
                    let (lines, what) = common::image_exception(name, &e);
                    let _ = writeln!(err, "{lines}{what}");
                    return abort(err, "avscale", "std::runtime_error", &what);
                }
            }
        }
        _ => None,
    };
    let rows = match common::read_rows(settings, matrix, err) {
        Ok(r) => r,
        Err(m) => {
            let _ = writeln!(err, "{m}");
            return abort(err, "avscale", "std::runtime_error", &m);
        }
    };
    if rows.len() < 4 {
        return 254;
    }
    let Some(m) = common::exact4(&rows) else {
        let (r, c) = common::size(&rows);
        return common::refuse(
            err,
            "avscale",
            &format!(
                "{matrix} holds a {r} x {c} matrix; avscale needs a 4 x 4 one (FSL prints part of its output, then aborts at its determinant or a product)"
            ),
        );
    };
    let centre = match (&vol, volume) {
        (Some(v), _) => common::centre(v, "avscale", mode, notices, err),
        (None, Some(_)) => {
            let _ = writeln!(err, "WARNING::in calculating COG, total = 0.0");
            [0.0; 3]
        }
        (None, None) => [0.0; 3],
    };
    let a = match programs::avscale(&m, centre) {
        Ok(a) => a,
        Err(what) => {
            let _ = writeln!(err, "{what}");
            return abort(err, "avscale", "std::runtime_error", what);
        }
    };
    let mut s = Stream::default();
    let p = &a.params;
    let mut text = format!(
        "Rotation & Translation Matrix:\n{}\n",
        s.matrix(&a.rotation_translation)
    );
    if all_params {
        text += &format!(
            "Rotation Angles (x,y,z) [rads] = {}\n\nTranslations (x,y,z) [mm] = {}\n\n",
            row(&s, &p[0..3]),
            row(&s, &p[3..6])
        );
    }
    text += &format!(
        "Scales (x,y,z) = {}\n\nSkews (xy,xz,yz) = {}\n\nAverage scaling = {}\n\nDeterminant = {}\nLeft-Right orientation: {}\n\n",
        row(&s, &p[6..9]),
        row(&s, &p[9..12]),
        s.num(f64::from(a.average_scale)),
        s.num(a.determinant),
        if a.determinant > 0.0 {
            "preserved"
        } else {
            "swapped"
        }
    );
    let _ = write!(out, "{text}");
    let (forward, backward) = match programs::half_transforms(&m, a.determinant) {
        Ok(h) => h,
        Err(what) => {
            let _ = writeln!(err, "{what}");
            return abort(err, "avscale", "std::runtime_error", what);
        }
    };
    let mut text = format!(
        "Forward half transform =\n{}\nBackward half transform =\n{}\n",
        s.matrix(&forward),
        s.matrix(&backward)
    );
    if eddies {
        let Some((check, eddy)) = programs::inverted_eddies(&m, &a) else {
            let _ = write!(out, "{text}");
            let _ = writeln!(err, "{}", mat::SINGULAR);
            return abort(err, "avscale", "std::runtime_error", mat::SINGULAR);
        };
        text += &format!(
            "\nMatrix check: mat^-1 * rotmat * skew * scale =\n{}\nInverted eddy matrix:\n{}\n",
            s.matrix(&check),
            s.matrix(&eddy)
        );
    }
    let _ = write!(out, "{text}");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_in(dir: &std::path::Path, words: &[&str]) -> (u8, String, String) {
        let args: Vec<String> = words.iter().map(|s| (*s).to_owned()).collect();
        let settings = Settings {
            dir: Some(dir.to_path_buf()),
            ..Settings::default()
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = main(&args, &settings, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn as_fsl_prints() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("f.mat"),
            "-1.1 0.1 0 3\n0.05 0.9 0.2 -1\n0 -0.1 1.2 2\n0 0 0 1\n",
        )
        .unwrap();
        let (c, out, err) = run_in(d.path(), &["--inverteddies", "f.mat"]);
        assert_eq!((c, err.as_str()), (0, ""));
        // FSL 6.0.7.23 (observed)
        let want = concat!(
            "Rotation & Translation Matrix:\n",
            "-0.998969 0.045132 0.004995 3.000000 \n",
            "-0.044311 -0.992956 0.109882 -1.000000 \n",
            "0.009919 0.109547 0.993932 2.000000 \n",
            "0.000000 0.000000 0.000000 1.000000 \n",
            "\n",
            "Scales (x,y,z) = 1.101136 0.909129 1.214695 \n",
            "\n",
            "Skews (xy,xz,yz) = -0.064930 0.007476 0.054818 \n",
            "\n",
            "Average scaling = 1.074986\n",
            "\n",
            "Determinant = -1.216000\n",
            "Left-Right orientation: swapped\n",
            "\n",
            "Forward half transform =\n",
            "1.049079 0.052534 -0.001368 1.477323 \n",
            "-0.023830 0.951087 0.090924 -0.542074 \n",
            "-0.000657 -0.052538 1.098797 0.942989 \n",
            "0.000000 0.000000 0.000000 1.000000 \n",
            "\n",
            "Backward half transform =\n",
            "-0.946416 0.160431 -0.027878 4.532759 \n",
            "0.068860 1.038304 -0.097281 0.484213 \n",
            "0.002520 0.042301 0.908614 -0.839498 \n",
            "0.000000 0.000000 0.000000 1.000000 \n",
            "\n",
            "\n",
            "Matrix check: mat^-1 * rotmat * skew * scale =\n",
            "0.990071 -0.180916 -0.013372 -0.000000 \n",
            "-0.109218 -0.990071 -0.147087 -0.000000 \n",
            "0.000000 0.000000 1.000000 0.000000 \n",
            "0.000000 0.000000 0.000000 1.000000 \n",
            "\n",
            "Inverted eddy matrix:\n",
            "-1.100000 -0.021703 -0.006010 3.000000 \n",
            "-0.048793 -1.095371 0.199189 -1.000000 \n",
            "0.010922 0.121205 1.200120 2.000000 \n",
            "0.000000 0.000000 0.000000 1.000000 \n",
            "\n",
        );
        // FSL's last check row is `0.000000 0.000000 1.000000 0.000000`: residuals of 1e-10
        // whose signs follow FSL's own last bits (the findings, "FLIRT's other programs")
        let ours: Vec<&str> = out.lines().collect();
        let theirs: Vec<&str> = want.lines().collect();
        assert_eq!(ours.len(), theirs.len());
        for (k, (a, b)) in ours.iter().zip(&theirs).enumerate() {
            if k == 31 {
                assert_eq!(a, &"-0.000000 0.000000 1.000000 0.000000 ");
            } else {
                assert_eq!(a, b, "line {k}");
            }
        }
        std::fs::write(
            d.path().join("k.mat"),
            "2 0 0 0\n0 1 0 0\n0 0 1 0\n0.5 0 0 1\n",
        )
        .unwrap();
        let (_, out, _) = run_in(d.path(), &["--allparams", "k.mat"]);
        assert!(out.contains("Rotation Angles (x,y,z) [rads] = 0.000000 -0.000000 0.000000 \n"));
        assert!(
            out.contains("\n-0.250000 0.000000 0.000000 1.000000 \n"),
            "{out}"
        );
    }

    #[test]
    fn failures() {
        let d = tempfile::tempdir().unwrap();
        let (c, _, err) = run_in(d.path(), &[]);
        assert_eq!((c, err.as_str()), (255, USAGE));
        let (c, _, err) = run_in(d.path(), &["none.mat"]);
        assert_eq!(
            (c, err.as_str()),
            (254, "Could not open matrix file none.mat\n")
        );
        let (c, _, err) = run_in(d.path(), &["--allparams"]);
        assert_eq!(c, 1);
        assert!(err.starts_with("Image error\nterminate called after throwing an instance of 'std::logic_error'\n  what():  basic_string: construction from null is not valid\n"));
        std::fs::write(
            d.path().join("z.mat"),
            "0 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n",
        )
        .unwrap();
        let (c, out, err) = run_in(d.path(), &["z.mat"]);
        assert_eq!((c, out.as_str()), (1, ""));
        assert!(err.starts_with(&format!("{}\nterminate called", programs::INVERSE_PROBLEM)));
        std::fs::write(
            d.path().join("s.mat"),
            "1 0.1 0 2\n0 1 0 3\n0 0 1 4\n0 0 0 0\n",
        )
        .unwrap();
        let (c, out, err) = run_in(d.path(), &["s.mat"]);
        assert_eq!(c, 1);
        assert!(
            out.ends_with("Determinant = 0.000000\nLeft-Right orientation: swapped\n\n"),
            "{out}"
        );
        assert!(err.starts_with("inv(): matrix is singular\nterminate called"));
    }
}
