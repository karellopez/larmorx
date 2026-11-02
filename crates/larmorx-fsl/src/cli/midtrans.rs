// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl midtrans`: FSL's `midtrans` (`specs/flirt.md` §15.5), the transform from the
//! space several transforms map into to their middle.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};

use super::Settings;
use super::common::{self, print_notices};
use super::options::{self, Given, Spec};
use crate::flirt::fits::{self, MidError, Step};
use crate::flirt::mat::{self, Mat4};

/// The usage text (observed).
pub const USAGE: &str = "\nPart of FSL (ID: 2412.6-dirty)\nmidtrans \nCopyright(c) 2010, University of Oxford (Mark Jenkinson)\n\nUsage: \nmidtrans [options] transform1 transform2 ... transformN\n  e.g. midtrans -o temp2mid.mat A2temp.mat B2temp.mat C2temp.mat\n       midtrans -o C2mid.mat A2C.mat B2C.mat ident.mat\n\nOptional arguments (You may optionally specify one or more of):\n\t-o,--out\toutput filename for matrix\n\t--template\tinput filename for template image (needed for fix origin)\n\t--separate\tbasename for output of separate matrices (final name includes a number; e.g. img2mid0001.mat)\n\t--debug\tswitch on debugging output\n\t-v,--verbose\tswitch on diagnostic messages\n\t-h,--help\tdisplay this message\n\n\n";

const fn spec(short: Option<char>, long: &'static str, values: usize) -> Spec {
    Spec {
        short,
        long: Some(long),
        values,
        numeric: false,
        integer: false,
    }
}

const SPECS: [Spec; 6] = [
    spec(Some('o'), "out", 1),
    spec(None, "template", 1),
    spec(None, "separate", 1),
    spec(None, "debug", 0),
    spec(Some('v'), "verbose", 0),
    spec(Some('h'), "help", 0),
];

fn text(g: &Option<Given>) -> Option<&str> {
    match g {
        Some(Given::Text(t)) => Some(t.as_str()),
        _ => None,
    }
}

fn m3_text(m: &fits::M3) -> String {
    let mut s = String::new();
    for row in m {
        for &v in row {
            s.push_str(&mat::f6(v));
            s.push(' ');
        }
        s.push('\n');
    }
    s
}

fn row_text(v: &[f64]) -> String {
    let mut s: String = v.iter().map(|&x| format!("{} ", mat::f6(x))).collect();
    s.push('\n');
    s
}

/// An exception of the computation (FSL-78): FSL prints its message and exits with status 0;
/// the original exits 1 by default (rule B), 0 with `reproduce`, naming FSL-78 in both modes.
fn exception(
    err: &mut dyn Write,
    what: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> u8 {
    let _ = writeln!(err, "{what}");
    let what = what.lines().next().unwrap_or_default();
    let detail = match mode {
        UpstreamDefects::Fix => format!(
            "midtrans stops at '{what}' and writes nothing: exit status 1 (FSL exits with status 0)"
        ),
        UpstreamDefects::Reproduce => {
            format!("midtrans stops at '{what}' and writes nothing: exit status 0, as FSL")
        }
    };
    defects::push_once(notices, Notice::new(&defects::FSL_78, mode, detail));
    print_notices(err, notices);
    u8::from(mode.fix())
}

/// `midtrans` with FSL's arguments; returns the exit status.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl midtrans: {e}");
            return 2;
        }
    };
    let (given, rest) = match options::parse(args, &SPECS, true) {
        Ok(r) => r,
        Err(e) => {
            let _ = write!(err, "{USAGE}\n{}\n", e.0);
            return 1;
        }
    };
    if given[5].is_some() {
        let _ = write!(err, "{USAGE}");
        return 1;
    }
    let names = &args[rest..];
    if names.len() < 2 {
        let _ = write!(
            err,
            "Must specify at least 2 transforms to find the mid-transform\n{USAGE}"
        );
        return 1;
    }
    let (debug, verbose) = (given[3].is_some(), given[4].is_some());
    let mut notices = Vec::new();
    // the transforms, each checked by its determinant as it is read
    let mut ts: Vec<Vec<Vec<f64>>> = Vec::with_capacity(names.len());
    for name in names {
        let rows = match common::read_rows(settings, name, err) {
            Ok(r) => r,
            Err(m) => return exception(err, &m, mode, &mut notices),
        };
        let (r, c) = common::size(&rows);
        if r != c && !(r == 0 && c == 0) {
            return exception(
                err,
                "det(): given matrix must be square sized",
                mode,
                &mut notices,
            );
        }
        let det = fits::determinant(&rows);
        if !(det.abs() >= 1e-6) || det.is_infinite() {
            let _ = writeln!(err, "Could not read matrix {name}");
            print_notices(err, &notices);
            return 1;
        }
        ts.push(rows);
    }
    if verbose {
        let _ = writeln!(out, "Number of matrices = {}", ts.len());
    }
    // the centre: the template's centre of gravity, else the MNI origin (0 with --debug)
    let centre: Vec<f64> = match text(&given[1]) {
        Some(name) => match common::read_volume(settings, name, mode, &mut notices, err) {
            Ok(v) => common::centre(&v, "midtrans", mode, &mut notices, err).to_vec(),
            Err(e) => {
                // the image layer's lines, then its exception caught (FSL-78)
                let (lines, what) = common::image_exception(name, &e);
                let _ = write!(err, "{lines}");
                return exception(err, &what, mode, &mut notices);
            }
        },
        None => {
            let _ = writeln!(out, "Assuming MNI origin as no template image specified");
            if debug {
                vec![0.0; 4]
            } else {
                vec![90.0, 110.0, 90.0, 0.0]
            }
        }
    };
    if verbose {
        let _ = writeln!(out, "Centre of template = {}", row_text(&centre));
        let _ = writeln!(out, "Initialising averages");
    }
    let o = [centre[0], centre[1], centre[2], 1.0];
    let mut mats: Vec<Mat4> = Vec::with_capacity(ts.len());
    let mut xs = Vec::with_capacity(ts.len());
    for rows in &ts {
        let Some(t) = common::exact4(rows) else {
            let (r, c) = common::size(rows);
            return exception(
                err,
                &format!("matrix multiplication: incompatible matrix dimensions: {r}x{c} and 4x1"),
                mode,
                &mut notices,
            );
        };
        let Some(x) = mat::solve_vec(&t, o) else {
            return exception(err, mat::SINGULAR, mode, &mut notices);
        };
        mats.push(t);
        xs.push([x[0], x[1], x[2]]);
    }
    if verbose {
        let _ = writeln!(out, "Stage 1");
    }
    let mut text_out = String::new();
    let mut step = |s: Step<'_>| match s {
        Step::P(n, p) if debug => text_out += &format!("P_{n} = \n{}\n", m3_text(p)),
        Step::PxxAv(p) if debug => text_out += &format!("Pxx_av = \n{}\n", m3_text(p)),
        Step::A(a) if verbose => text_out += &format!("A = \n{}\n", m3_text(a)),
        Step::S(v) if verbose => text_out += &format!("s = {}\n", row_text(v)),
        Step::Stage2 if verbose => text_out += "Stage 2\n",
        Step::Subject(n, r, u) if verbose => {
            text_out += &format!(
                "Subject number {n}\nR = \n{}\nu = {}\n",
                m3_text(r),
                row_text(u)
            );
        }
        Step::Stage3 if verbose => text_out += "Stage 3\n",
        Step::M(m) if verbose => text_out += &format!("M = \n{}\n", m3_text(m)),
        Step::W(w) if verbose => text_out += &format!("w = {}\n", row_text(w)),
        Step::Final if verbose => text_out += "Final calculations\n",
        _ => {}
    };
    let result = fits::midtrans(&mats, &xs, &mut step);
    let _ = out.write_all(text_out.as_bytes());
    let result = match result {
        Ok(m) => m,
        Err(MidError::Singular) => return exception(err, mat::SINGULAR, mode, &mut notices),
        Err(MidError::NotFinite) => {
            print_notices(err, &notices);
            return common::refuse(
                err,
                "midtrans",
                "the averages hold values that are not finite (a transform too large for double precision): their singular value decomposition is undefined (FSL's fails and it goes on with empty matrices); refused",
            );
        }
    };
    let written = |name: &str, m: &Mat4, err: &mut dyn Write| {
        if std::fs::write(settings.path(name), mat::file_text(m)).is_err() {
            let _ = writeln!(err, "Could not open file {name} for writing");
        }
    };
    let outname = text(&given[0]);
    if let Some(name) = outname {
        written(name, &result, err);
    }
    if verbose || outname.is_none() {
        let _ = writeln!(out, "{}", mat::print_text(&result));
    }
    if let Some(p) = text(&given[2]) {
        for (k, t) in mats.iter().enumerate() {
            written(&format!("{p}{:04}.mat", k + 1), &mat::mul(&result, t), err);
        }
    }
    print_notices(err, &notices);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(words: &[&str], files: &[(&str, &str)], mode: &str) -> (u8, String, String) {
        let dir = tempfile::tempdir().unwrap();
        for (n, t) in files {
            std::fs::write(dir.path().join(n), t).unwrap();
        }
        let args: Vec<String> = words.iter().map(|s| (*s).to_owned()).collect();
        let mut settings = Settings {
            dir: Some(dir.path().to_path_buf()),
            ..Settings::default()
        };
        settings
            .vars
            .insert(defects::ENV.to_owned(), mode.to_owned());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = main(&args, &settings, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    const A: &str = "1.02 0.05 -0.01 3.5\n-0.04 0.98 0.06 -2\n0.02 -0.03 1.05 1\n0 0 0 1\n";
    const B: &str = "0.97 -0.08 0.02 -1.5\n0.07 1.01 -0.02 4\n-0.03 0.02 0.99 -3\n0 0 0 1\n";
    const C: &str = "1.1 0.01 0.03 0.5\n0.02 0.92 -0.05 1\n-0.01 0.04 1.03 2.5\n0 0 0 1\n";

    #[test]
    fn as_fsl_prints() {
        // FSL 6.0.7.23 (observed)
        let files = [("a.mat", A), ("b.mat", B), ("c.mat", C)];
        let (c, out, err) = run(&["a.mat", "b.mat", "c.mat"], &files, "reproduce");
        assert_eq!((c, err.as_str()), (0, ""));
        assert_eq!(
            out,
            "Assuming MNI origin as no template image specified\n0.966599 0.008067 -0.012422 -0.493887 \n-0.015291 1.026823 0.003797 -0.813904 \n0.006625 -0.009776 0.975835 0.126196 \n0.000000 0.000000 0.000000 1.000000 \n\n"
        );
        let (_, out, _) = run(&["-v", "--debug", "a.mat", "b.mat"], &files, "reproduce");
        assert!(
            out.contains("Stage 1\nP_1 = \n1.020000 0.050000 -0.010000 \n"),
            "{out}"
        );
        assert!(
            out.contains("Pxx_av = \n1280.651266 -25.591333 10.210684 \n"),
            "{out}"
        );
        assert!(
            out.contains("w = 0.003354 0.012553 0.017655 \n\nFinal calculations\n1.001915 0.016518 -0.004671 -1.190757 \n"),
            "{out}"
        );
    }

    #[test]
    fn messages() {
        let files = [("a.mat", A), ("r.mat", "1 0 0 1\n0 1 0 2\n0 0 1 3\n")];
        let (c, _, err) = run(&["a.mat"], &files, "fix");
        assert_eq!(c, 1);
        assert!(err.starts_with("Must specify at least 2 transforms"));
        let (c, out, err) = run(&["a.mat", "r.mat"], &files, "reproduce");
        assert_eq!((c, out.as_str()), (0, ""));
        assert!(err.starts_with("det(): given matrix must be square sized\n"));
        let (c, _, err) = run(&["a.mat", "r.mat"], &files, "fix");
        assert_eq!(c, 1);
        assert!(err.contains("FSL-78"), "{err}");
        let (c, _, err) = run(&["a.mat", "b.mat", "-v"], &files, "reproduce");
        assert_eq!(c, 0);
        assert!(
            err.contains("Could not open matrix file b.mat\nCould not open matrix file -v\nmatrix multiplication: incompatible matrix dimensions: 0x0 and 4x1\n"),
            "{err}"
        );
        let sing = [
            ("a.mat", A),
            ("s.mat", "1 0 0 0\n0 0 0 0\n0 0 1 0\n0 0 0 1\n"),
        ];
        let (c, _, err) = run(&["a.mat", "s.mat"], &sing, "fix");
        assert_eq!((c, err.as_str()), (1, "Could not read matrix s.mat\n"));
    }
}
