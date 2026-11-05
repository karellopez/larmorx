// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl pointflirt`: FSL's `pointflirt` (`specs/flirt.md` §15.6), the affine matrix
//! that maps input points onto reference points in the least-squares sense.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};

use super::Settings;
use super::common::{self, abort, print_notices};
use super::options::{self, Given, Spec};
use crate::flirt::fits;
use crate::flirt::mat;

/// The usage text (observed).
pub const USAGE: &str = "\nPart of FSL (ID: 2412.6-dirty)\npointflirt \nCopyright(c) 2001, University of Oxford (Mark Jenkinson)\n\nUsage: \npointflirt -i <invol coords>  -r <refvol coords> -o <output matrix>\npointflirt -i <invol coords>  -r <refvol coords> -o <output matrix> --vox --invol=<input vol> --refvol=<ref vol>\n\nCompulsory arguments (You MUST set one or more of):\n\t-i\tfilename of input volume coordinates\n\t-r\tfilename of reference volume coordinates\n\nOptional arguments (You may optionally specify one or more of):\n\t-o,--out\tfilename of affine matrix output\n\t--vox\tuse voxel coordinates, not mm, for input\n\t--invol\tfilename of input volume (needed for --vox option)\n\t--refvol\tfilename of reference volume (needed for --vox option)\n\t-v,--verbose\tswitch on diagnostic messages\n\t-h,--help\tdisplay this message\n\n\n";

const STARS: &str = "***************************************************";

const fn spec(short: Option<char>, long: Option<&'static str>, values: usize) -> Spec {
    Spec {
        short,
        long,
        values,
        numeric: false,
        integer: false,
    }
}

const SPECS: [Spec; 8] = [
    spec(Some('i'), None, 1),
    spec(Some('r'), None, 1),
    spec(Some('o'), Some("out"), 1),
    spec(None, Some("vox"), 0),
    spec(None, Some("invol"), 1),
    spec(None, Some("refvol"), 1),
    spec(Some('v'), Some("verbose"), 0),
    spec(Some('h'), Some("help"), 0),
];

fn text(g: &Option<Given>) -> Option<&str> {
    match g {
        Some(Given::Text(t)) => Some(t.as_str()),
        _ => None,
    }
}

/// A matrix's rows printed (§15's layout), then a blank line.
fn rows_text(rows: &[Vec<f64>]) -> String {
    let mut s = String::new();
    for row in rows {
        for &v in row {
            s.push_str(&mat::f6(v));
            s.push(' ');
        }
        s.push('\n');
    }
    s
}

fn transpose(rows: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let c = rows.first().map_or(0, Vec::len);
    (0..c)
        .map(|j| rows.iter().map(|r| r[j]).collect())
        .collect()
}

/// `pointflirt` with FSL's arguments; returns the exit status.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl pointflirt: {e}");
            return 2;
        }
    };
    let (given, _) = match options::parse(args, &SPECS, false) {
        Ok(r) => r,
        Err(e) => {
            let _ = write!(err, "{USAGE}\n{}\n", e.0);
            return 1;
        }
    };
    if given[7].is_some() {
        let _ = write!(err, "{USAGE}");
        return 1;
    }
    let (inname, refname) = (text(&given[0]), text(&given[1]));
    if inname.is_none() || refname.is_none() {
        let mut missing = String::new();
        if inname.is_none() {
            missing += "\t-i\tfilename of input volume coordinates\n";
        }
        if refname.is_none() {
            missing += "\t-r\tfilename of reference volume coordinates\n";
        }
        let _ = write!(
            err,
            "{STARS}\nThe following COMPULSORY options have not been set:\n{missing}{STARS}\n{USAGE}"
        );
        return 1;
    }
    run(
        inname.unwrap_or_default(),
        refname.unwrap_or_default(),
        &given,
        settings,
        mode,
        out,
        err,
    )
}

fn run(
    inname: &str,
    refname: &str,
    given: &[Option<Given>],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut notices = Vec::new();
    let read = |name: &str, err: &mut dyn Write| common::read_rows(settings, name, err);
    let mut x = match read(inname, err) {
        Ok(r) => r,
        Err(m) => return abort(err, "pointflirt", "std::runtime_error", &m),
    };
    let mut y = match read(refname, err) {
        Ok(r) => r,
        Err(m) => return abort(err, "pointflirt", "std::runtime_error", &m),
    };
    // points as columns: both transposed when the input has more rows than columns (§15.6)
    let (r, c) = common::size(&x);
    if r > c {
        x = transpose(&x);
        y = transpose(&y);
    }
    if common::size(&x).1 != common::size(&y).1 {
        let _ = writeln!(
            err,
            "ERROR: Different number of coordinates in the two files"
        );
        return 255;
    }
    let verbose = given[6].is_some();
    if given[3].is_some() {
        let (Some(iv), Some(rv)) = (text(&given[4]), text(&given[5])) else {
            let _ = writeln!(
                err,
                "ERROR: need to specify --invol and --refvol to use --vox"
            );
            return 2;
        };
        for (pts, name) in [(&mut x, iv), (&mut y, rv)] {
            match common::read_header(settings, name, mode, &mut notices) {
                Ok(h) => {
                    for (k, row) in pts.iter_mut().enumerate().take(3) {
                        for v in row.iter_mut() {
                            *v *= f64::from(h.voxel[k]);
                        }
                    }
                }
                Err(e) => {
                    let (lines, what) = common::image_exception(name, &e);
                    let _ = write!(err, "{lines}");
                    print_notices(err, &notices);
                    return abort(err, "pointflirt", "std::runtime_error", &what);
                }
            }
        }
    }
    if verbose {
        let _ = write!(
            out,
            "Input volume coordinates are (in mm)\n{}\nReference volume coordinates are (in mm)\n{}\n",
            rows_text(&x),
            rows_text(&y)
        );
    }
    if x.is_empty() && y.is_empty() {
        // FSL-204 (§15.6): no points at all; FSL crashes here (a segmentation fault, after the
        // lines above), refused in both modes (rule B)
        let detail = "both coordinate files are empty or missing (no points): FSL crashes (a segmentation fault); refused";
        defects::push_once(
            &mut notices,
            Notice::new(&defects::FSL_204, mode, detail.to_owned()),
        );
        print_notices(err, &notices);
        return common::refuse(
            err,
            "pointflirt",
            "no points in the coordinate files (FSL-204)",
        );
    }
    for pts in [&x, &y] {
        if pts.len() != 3 {
            print_notices(err, &notices);
            return abort(
                err,
                "pointflirt",
                "std::logic_error",
                &format!(
                    "copy into submatrix: incompatible matrix dimensions: 3x1 and {}x1",
                    pts.len()
                ),
            );
        }
    }
    let as3 = |p: &[Vec<f64>]| [p[0].clone(), p[1].clone(), p[2].clone()];
    let Some(fit) = fits::pointflirt(&as3(&x), &as3(&y)) else {
        // a coordinate that is not finite (an overflow: a value read as the largest double
        // times a voxel size above 1, or products that overflow): FSL's SVD fails (§15.6)
        print_notices(err, &notices);
        return abort(
            err,
            "pointflirt",
            "std::out_of_range",
            "Mat::operator(): index out of bounds",
        );
    };
    if fit.nullspace > 0 {
        let _ = writeln!(
            out,
            "WARNING: nullspace of dimension {}\n  ... lacks sufficient independent coordinates\nFinding non-unique solution",
            fit.nullspace
        );
    }
    match text(&given[2]) {
        Some(name) => {
            if std::fs::write(settings.path(name), mat::file_text(&fit.matrix)).is_err() {
                let _ = writeln!(err, "Could not open file {name} for writing");
            }
        }
        None => {
            let _ = writeln!(out, "{}", mat::print_text(&fit.matrix));
        }
    }
    print_notices(err, &notices);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(words: &[&str], files: &[(&str, &str)]) -> (u8, String, String) {
        let dir = tempfile::tempdir().unwrap();
        for (n, t) in files {
            std::fs::write(dir.path().join(n), t).unwrap();
        }
        let args: Vec<String> = words.iter().map(|s| (*s).to_owned()).collect();
        let settings = Settings {
            dir: Some(dir.path().to_path_buf()),
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

    const IN: &str =
        "10 20 30\n-12 5 40\n33 -7 12\n0 0 0\n25 25 -10\n-8 -30 22\n14 9 -3\n-20 18 6\n";
    const REF: &str = "13.31 18.62 35.25\n-9.33 7.41 44.05\n38.47 -8.32 13.9\n2.5 -1 1.5\n29.29 23.95 -9.68\n-5.24 -31.87 26.59\n17.35 7.55 -1.71\n-17.75 18.24 7.86\n";

    #[test]
    fn as_fsl_prints() {
        // FSL 6.0.7.23 (observed)
        let files = [("in.txt", IN), ("ref.txt", REF)];
        let (c, out, err) = run(&["-i", "in.txt", "-r", "ref.txt"], &files);
        assert_eq!((c, err.as_str()), (0, ""));
        assert_eq!(
            out,
            "1.057696 -0.004719 0.008826 2.938679 \n-0.031168 1.033040 0.034489 -1.097244 \n-0.003568 -0.010023 1.088950 1.585333 \n0.000000 0.000000 0.000000 1.000000 \n\n"
        );
        let three: String = IN.lines().take(3).map(|l| format!("{l}\n")).collect();
        let three_ref: String = REF.lines().take(3).map(|l| format!("{l}\n")).collect();
        let files = [("i3.txt", three.as_str()), ("r3.txt", three_ref.as_str())];
        let (_, out, _) = run(&["-i", "i3.txt", "-r", "r3.txt"], &files);
        assert!(
            out.starts_with("WARNING: nullspace of dimension 3\n  ... lacks sufficient independent coordinates\nFinding non-unique solution\n0.953344 0.118176 0.155811 0.052923 \n"),
            "{out}"
        );
    }

    #[test]
    fn messages() {
        let (c, _, err) = run(&[], &[]);
        assert_eq!(c, 1);
        assert!(err.starts_with(STARS), "{err}");
        let files = [("in.txt", IN), ("r.txt", "1 2 3\n")];
        let (c, _, err) = run(&["-i", "in.txt", "-r", "r.txt"], &files);
        assert_eq!(
            (c, err.as_str()),
            (
                255,
                "ERROR: Different number of coordinates in the two files\n"
            )
        );
        let files = [("in.txt", IN), ("ref.txt", REF)];
        let (c, _, err) = run(&["-i", "in.txt", "-r", "ref.txt", "--vox"], &files);
        assert_eq!(
            (c, err.as_str()),
            (
                2,
                "ERROR: need to specify --invol and --refvol to use --vox\n"
            )
        );
        let files = [("o.txt", "1 2 3\n4 5 6\n"), ("p.txt", "1 2 4\n4 5 7\n")];
        let (c, out, err) = run(&["-v", "-i", "o.txt", "-r", "p.txt"], &files);
        assert_eq!(c, 1);
        assert!(
            out.starts_with("Input volume coordinates are (in mm)\n1.000000 2.000000 3.000000 \n")
        );
        assert!(err.contains("3x1 and 2x1"), "{err}");
    }

    #[test]
    fn no_points_fsl_204() {
        // FSL 6.0.7.23 (observed): its lines, the verbose headings, then a segmentation fault;
        // refused in both modes (the test runs the default)
        let (c, out, err) = run(&["-v", "-i", "none1.txt", "-r", "none2.txt"], &[]);
        assert_eq!(c, 1);
        assert_eq!(
            out,
            "Input volume coordinates are (in mm)\n\nReference volume coordinates are (in mm)\n\n"
        );
        assert!(
            err.starts_with("Could not open matrix file none1.txt\nCould not open matrix file none2.txt\nlarmorx: upstream defect FSL-204"),
            "{err}"
        );
        assert!(err.ends_with("no points in the coordinate files (FSL-204) (larmorx)\n"));
        // files of words only read no points either; --vox's own error comes first
        let files = [("w.txt", "abc def\n"), ("e.txt", "")];
        let (c, _, err) = run(&["-i", "w.txt", "-r", "e.txt"], &files);
        assert!(c == 1 && err.contains("FSL-204"), "{err}");
        let (c, _, err) = run(&["-i", "w.txt", "-r", "e.txt", "--vox"], &files);
        assert_eq!(
            (c, err.as_str()),
            (
                2,
                "ERROR: need to specify --invol and --refvol to use --vox\n"
            )
        );
    }

    #[test]
    fn words_and_overflows() {
        // a `nan` word reads 0 (§2.2), so the fit of the points as read (observed: the same
        // bytes as with 0)
        let with = |w: &str| IN.replacen("40\n", &format!("{w}\n"), 1);
        let (zero, nan) = (with("0"), with("nan"));
        let (c0, out0, _) = run(
            &["-i", "a.txt", "-r", "r.txt"],
            &[("a.txt", &zero), ("r.txt", REF)],
        );
        let (c1, out1, _) = run(
            &["-i", "a.txt", "-r", "r.txt"],
            &[("a.txt", &nan), ("r.txt", REF)],
        );
        assert_eq!((c0, c1), (0, 0));
        assert_eq!(out0, out1);
        // an overflow (the largest double squared): FSL's SVD fails (observed with --vox)
        let big = with("1e200");
        let (c, _, err) = run(
            &["-i", "a.txt", "-r", "r.txt"],
            &[("a.txt", &big), ("r.txt", REF)],
        );
        assert_eq!(c, 1);
        assert!(
            err.starts_with("terminate called after throwing an instance of 'std::out_of_range'\n  what():  Mat::operator(): index out of bounds\n"),
            "{err}"
        );
    }
}
