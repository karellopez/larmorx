// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl makerot`: FSL's `makerot` (`specs/flirt.md` §15.4), a rotation matrix about an
//! axis through a centre.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};

use super::Settings;
use super::common::{self, abort, print_notices};
use super::options::{self, Given, Spec};
use crate::flirt::mat;
use crate::flirt::programs;

/// The usage text (observed).
pub const USAGE: &str = "\nPart of FSL (ID: 2412.6-dirty)\nmakerot \nCopyright(c) 2003, University of Oxford (Mark Jenkinson)\n\nUsage: \nmakerot [options] --theta=angle\n\nCompulsory arguments (You MUST set one or more of):\n\t-t,--theta\tangle of rotation (in degrees)\n\nOptional arguments (You may optionally specify one or more of):\n\t-a,--axis <ax,ay,az>\tunnormalised axis vector (comma separated)\n\t--cov\timage filename used for centre of volume\n\t-c,--centre <cx,cy,cz>\tcentre of rotation in mm (comma separated)\n\t-o,--out\toutput filename for matrix\n\t-v,--verbose\tswitch on diagnostic messages\n\t-h,--help\tdisplay this message\n\n\n";

const COMPULSORY: &str = "***************************************************\nThe following COMPULSORY options have not been set:\n\t-t,--theta\tangle of rotation (in degrees)\n***************************************************\n";

const fn spec(short: Option<char>, long: &'static str, values: usize, numeric: bool) -> Spec {
    Spec {
        short,
        long: Some(long),
        values,
        numeric,
        integer: false,
    }
}

const SPECS: [Spec; 7] = [
    spec(Some('t'), "theta", 1, true),
    spec(Some('a'), "axis", 1, false),
    spec(None, "cov", 1, false),
    spec(Some('c'), "centre", 1, false),
    spec(Some('o'), "out", 1, false),
    spec(Some('v'), "verbose", 0, false),
    spec(Some('h'), "help", 0, false),
];

/// A vector option's word split as FSL splits it (§15.4): at `:` when the word holds one, else
/// at `,`; each piece read as C's `atof` and kept in single precision.
fn pieces(word: &str) -> Vec<f64> {
    let sep = if word.contains(':') { ':' } else { ',' };
    word.split(sep)
        .map(|p| f64::from(mat::atof(p) as f32))
        .collect()
}

/// A vector option's three values from its occurrences, as FSL reads them (FSL-77: past the
/// values given, into what an earlier occurrence left), and whether an occurrence held other
/// than three.
fn vector(words: &[&str], default: [f64; 3]) -> ([f64; 3], Option<String>) {
    if words.is_empty() {
        return (default, None);
    }
    let mut buffer = [0.0f64; 3];
    let mut odd = None;
    for w in words {
        let v = pieces(w);
        if v.len() != 3 && odd.is_none() {
            odd = Some(format!(
                "'{w}' ({} value{})",
                v.len(),
                if v.len() == 1 { "" } else { "s" }
            ));
        }
        for (k, x) in v.iter().take(3).enumerate() {
            buffer[k] = *x;
        }
    }
    (buffer, odd)
}

/// `makerot` with FSL's arguments; returns the exit status.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl makerot: {e}");
            return 2;
        }
    };
    let (all, _) = match options::parse_all(args, &SPECS, false) {
        Ok(r) => r,
        Err(e) => {
            let _ = write!(err, "{USAGE}\n{}\n", e.0);
            return 1;
        }
    };
    let text = |k: usize| -> Vec<&str> {
        all.iter()
            .filter(|(i, _)| *i == k)
            .filter_map(|(_, g)| match g {
                Given::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    };
    if all.iter().any(|(k, _)| *k == 6) {
        let _ = write!(err, "{USAGE}");
        return 1;
    }
    let theta = all.iter().rev().find_map(|(k, g)| match (k, g) {
        (0, Given::Numbers(v)) => v.first().copied(),
        _ => None,
    });
    let Some(theta) = theta else {
        let _ = write!(err, "{COMPULSORY}{USAGE}");
        return 1;
    };
    let mut notices = Vec::new();
    let (axis, odd_axis) = vector(&text(1), [0.0, 0.0, 1.0]);
    let (mut centre, odd_centre) = vector(&text(3), [0.0; 3]);
    for (what, odd) in [("axis", &odd_axis), ("centre", &odd_centre)] {
        let Some(odd) = odd else { continue };
        if mode.fix() {
            let n = Notice::new(
                &defects::FSL_77,
                mode,
                format!("the {what} {odd} is refused (FSL reads past the values given)"),
            );
            print_notices(err, &[n]);
            return common::refuse(
                err,
                "makerot",
                &format!(
                    "the {what} {odd} must hold three values (FSL-77: FSL reads past the values given, into its buffer's zeros or an earlier occurrence's values, and ignores those after the third); refused"
                ),
            );
        }
        defects::push_once(
            &mut notices,
            Notice::new(
                &defects::FSL_77,
                mode,
                format!(
                    "the {what} {odd} is read as FSL reads it: {}",
                    if what == "axis" { axis } else { centre }
                        .map(|v| mat::g(v, 6))
                        .join(",")
                ),
            ),
        );
    }
    let len = programs::axis_length(axis);
    if len == 0.0 {
        let _ = writeln!(err, "ERROR:: Axis cannot be zero");
        print_notices(err, &notices);
        return 1;
    }
    // named only where FSL's rotation differs from the corrected one (D13: a zero angle, or one
    // whose two scalings give the same matrix, changes nothing); the rotation decides it, the
    // translation following from it about any centre
    let bits = |m: mat::Mat4| m.map(|row| row.map(f64::to_bits));
    let changes = || {
        let at = |fsl76| bits(programs::makerot(theta as f32, axis, [0.0; 3], fsl76));
        at(true) != at(false)
    };
    if len != 1.0 && changes() {
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "the axis of length {} is divided by its length (FSL divides it by the square root of its length: a rotation by {} degrees, not {})",
                mat::g(len, 6),
                mat::g(theta * len.sqrt(), 6),
                mat::g(theta, 6)
            ),
            UpstreamDefects::Reproduce => format!(
                "the axis of length {} is divided by the square root of its length, as FSL does (a rotation by {} degrees, not {})",
                mat::g(len, 6),
                mat::g(theta * len.sqrt(), 6),
                mat::g(theta, 6)
            ),
        };
        defects::push_once(&mut notices, Notice::new(&defects::FSL_76, mode, detail));
    }
    if let Some(cov) = text(2).last() {
        match common::read_header(settings, cov, mode, &mut notices) {
            Ok(h) => {
                for k in 0..3 {
                    centre[k] = ((h.shape[k] as f64 - 1.0) * f64::from(h.voxel[k])) * 0.5;
                }
            }
            Err(e) => {
                let (lines, what) = common::image_exception(cov, &e);
                let _ = write!(err, "{lines}");
                print_notices(err, &notices);
                return abort(err, "makerot", "std::runtime_error", &what);
            }
        }
    }
    let m = programs::makerot(theta as f32, axis, centre, mode.reproduce());
    match text(4).last() {
        Some(name) => {
            if std::fs::write(settings.path(name), mat::file_text(&m)).is_err() {
                let _ = writeln!(err, "Could not open file {name} for writing");
            }
        }
        None => {
            let _ = writeln!(out, "{}", mat::print_text(&m));
        }
    }
    print_notices(err, &notices);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(words: &[&str], mode: &str) -> (u8, String, String) {
        let args: Vec<String> = words.iter().map(|s| (*s).to_owned()).collect();
        let mut settings = Settings::default();
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

    #[test]
    fn as_fsl_prints() {
        // FSL 6.0.7.23 (observed)
        let (c, out, err) = run(&["-t", "30"], "reproduce");
        assert_eq!((c, err.as_str()), (0, ""));
        assert_eq!(
            out,
            "0.866025 0.500000 0.000000 0.000000 \n-0.500000 0.866025 0.000000 0.000000 \n0.000000 0.000000 1.000000 0.000000 \n0.000000 0.000000 0.000000 1.000000 \n\n"
        );
        let (_, out, err) = run(
            &["--theta=-45.5", "--axis=0,0,4", "--centre=1:2:3"],
            "reproduce",
        );
        assert!(
            out.starts_with("-0.017452 -0.999848 0.000000 3.017148 \n0.999848 -0.017452 0.000000 1.035057 \n0.000000 0.000000 1.000000 -0.000000 \n"),
            "{out}"
        );
        assert!(err.contains("FSL-76 (makerot) reproduced"), "{err}");
        let (_, out, _) = run(&["-t", "30", "-a", "1,2,3", "-a", "7"], "reproduce");
        assert!(
            out.starts_with("0.811577 0.581954 0.051683 0.000000 \n"),
            "{out}"
        );
        let (_, out, _) = run(&["-t", "30", "-a", "5"], "reproduce");
        assert!(
            out.starts_with(
                "1.000000 0.000000 0.000000 0.000000 \n0.000000 0.389413 0.921063 0.000000 \n"
            ),
            "{out}"
        );
        let (_, out, _) = run(&["-t", "30", "-a", "nan,1,1", "-c", "1,2,3"], "reproduce");
        assert_eq!(
            out,
            "nan nan nan nan \nnan nan nan nan \n-nan -nan -nan -nan \n0.000000 0.000000 0.000000 1.000000 \n\n"
        );
    }

    #[test]
    fn messages() {
        let (c, _, err) = run(&[], "fix");
        assert_eq!((c, err), (1, format!("{COMPULSORY}{USAGE}")));
        let (c, _, err) = run(&["-h"], "fix");
        assert_eq!((c, err), (1, USAGE.to_owned()));
        let (c, _, err) = run(&["-t", "abc"], "fix");
        assert_eq!(c, 1);
        assert!(
            err.ends_with("\n\n\n\n-t: Couldn't set_value! valstr=\"abc\"!\n"),
            "{err}"
        );
        let (c, _, err) = run(&["-t", "30", "extra"], "fix");
        assert_eq!(c, 1);
        assert!(err.ends_with("\nextra:  is an unrecognised token!\n"));
        let (c, _, err) = run(&["-t", "10", "-a", "0,0,0"], "fix");
        assert_eq!((c, err.as_str()), (1, "ERROR:: Axis cannot be zero\n"));
        // FSL-77 refused by default
        let (c, out, err) = run(&["-t", "30", "-a", "5"], "fix");
        assert_eq!((c, out.as_str()), (1, ""));
        assert!(err.contains("FSL-77"), "{err}");
        let (c, _, err) = run(&["--theta", "30"], "fix");
        assert_eq!(c, 1);
        assert!(err.ends_with("--theta: Missing non-optional argument!\n"));
    }
}
