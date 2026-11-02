// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl aff2rigid`: FSL's `aff2rigid` script (`specs/flirt.md` §15.8), the rigid
//! approximation of an input-to-standard affine matrix (FSL-81: FSL 6.0.7.23's script stops
//! before writing it; the original computes it by default).

use std::io::Write;

use larmorx_core::defects::{self, Notice};

use super::Settings;
use super::common::{print_notices, refuse};
use crate::flirt::fits;
use crate::flirt::mat::Mat4;

/// The usage text (observed; the script names itself as it was called).
pub const USAGE: &str = "Usage: aff2rigid <input2standard mat> <output mat>\n \n       First argument is the FLIRT transform (12 DOF) from the input image to standard\n       Second argument is the output matrix which will go from the input image to standard space (6 DOF)\n          aligning the AC, the AC-PC line and the mid-sagittal plane (in order of decreasing accuracy)\n";

/// FSL 6.0.7.23's traceback (FSL-81: NumPy 2 has no `mat`).
pub const TRACEBACK: &str = "Traceback (most recent call last):\n  File \"aff2rigid\", line 105, in <module>\n    v41=mat(zeros((4,1)))\n        ^^^\nNameError: name 'mat' is not defined\n";

/// A text file of numbers as NumPy's `loadtxt` reads it: blank-separated values (Python's
/// float words: `nan` and `inf` included), `#` comments, blank lines passed over, every row of
/// the same length.
fn loadtxt(text: &str) -> Result<Vec<Vec<f64>>, String> {
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for (k, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("");
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }
        let row = words
            .iter()
            .map(|w| {
                w.parse::<f64>()
                    .map_err(|_| format!("could not convert '{w}' to a number (line {})", k + 1))
            })
            .collect::<Result<Vec<f64>, String>>()?;
        if let Some(first) = rows.first()
            && first.len() != row.len()
        {
            return Err(format!(
                "the number of columns changed from {} to {} at line {}",
                first.len(),
                row.len(),
                k + 1
            ));
        }
        rows.push(row);
    }
    Ok(rows)
}

/// A matrix as the script writes it: `%14.10f`, separated by a blank.
fn written(m: &Mat4) -> String {
    m.iter()
        .map(|row| {
            let mut s = row
                .iter()
                .map(|v| format!("{v:14.10}"))
                .collect::<Vec<_>>()
                .join(" ");
            s.push('\n');
            s
        })
        .collect()
}

/// `aff2rigid` with FSL's arguments; returns the exit status.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl aff2rigid: {e}");
            return 2;
        }
    };
    let Some(input) = args.first() else {
        let _ = write!(out, "{USAGE}");
        return 1;
    };
    let rows = match std::fs::read_to_string(settings.path(input)) {
        Ok(t) => loadtxt(&t),
        Err(e) => Err(format!("cannot be read: {e}")),
    };
    let rows = match rows {
        Ok(r) => r,
        Err(why) => {
            return refuse(
                err,
                "aff2rigid",
                &format!("{input}: {why} (FSL's script stops with NumPy's traceback)"),
            );
        }
    };
    let square = !rows.is_empty() && rows.iter().all(|r| r.len() == rows.len());
    let m = (rows.len() == 4 && square).then(|| {
        let mut m = [[0.0; 4]; 4];
        for i in 0..4 {
            m[i].copy_from_slice(&rows[i]);
        }
        m
    });
    let invertible = m.as_ref().and_then(crate::flirt::mat::inverse).is_some();
    let mut notices = Vec::new();
    if mode.reproduce() {
        if square && (m.is_none() || invertible) {
            // the script inverts the matrix, then stops at NumPy 2's missing `mat`
            defects::push_once(
                &mut notices,
                Notice::new(
                    &defects::FSL_81,
                    mode,
                    "aff2rigid stops at NumPy 2's missing `mat` before writing, as FSL 6.0.7.23's script does".to_owned(),
                ),
            );
            let _ = write!(err, "{TRACEBACK}");
            print_notices(err, &notices);
            return 1;
        }
        return refuse(
            err,
            "aff2rigid",
            &format!(
                "{input}: not an invertible square matrix (FSL's script stops with NumPy's linear-algebra traceback)"
            ),
        );
    }
    let Some(m) = m else {
        return refuse(
            err,
            "aff2rigid",
            &format!("{input}: not a 4 x 4 matrix (FSL's script stops with NumPy's traceback)"),
        );
    };
    if m.iter().flatten().any(|v| !v.is_finite()) || !invertible {
        return refuse(
            err,
            "aff2rigid",
            &format!(
                "{input}: not an invertible matrix of finite numbers (FSL's script stops with NumPy's traceback or computes NaN)"
            ),
        );
    }
    let Some(name) = args.get(1) else {
        return refuse(
            err,
            "aff2rigid",
            "no output matrix named (FSL's script stops before it needs one)",
        );
    };
    let Some(r) = fits::aff2rigid(&m) else {
        return refuse(err, "aff2rigid", &format!("{input}: a singular matrix"));
    };
    if let Err(e) = std::fs::write(settings.path(name), written(&r)) {
        return refuse(err, "aff2rigid", &format!("{name} cannot be written: {e}"));
    }
    defects::push_once(
        &mut notices,
        Notice::new(
            &defects::FSL_81,
            mode,
            "aff2rigid's matrix computed and written as the script defines it (FSL 6.0.7.23's script stops with a NameError before writing it)".to_owned(),
        ),
    );
    print_notices(err, &notices);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_and_reproduces() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("aff.mat"),
            "1.05 0.02 -0.03 2\n-0.01 0.97 0.04 -3\n0.02 -0.05 1.1 1.5\n0 0 0 1\n",
        )
        .unwrap();
        let mut settings = Settings {
            dir: Some(dir.path().to_path_buf()),
            ..Settings::default()
        };
        let run = |words: &[&str], settings: &Settings| {
            let args: Vec<String> = words.iter().map(|s| (*s).to_owned()).collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let c = main(&args, settings, &mut out, &mut err);
            (
                c,
                String::from_utf8(out).unwrap(),
                String::from_utf8(err).unwrap(),
            )
        };
        let (c, out, _) = run(&[], &settings);
        assert_eq!((c, out.as_str()), (1, USAGE));
        let (c, _, err) = run(&["aff.mat", "r.mat"], &settings);
        assert_eq!(c, 0, "{err}");
        assert!(err.contains("FSL-81"));
        let text = std::fs::read_to_string(dir.path().join("r.mat")).unwrap();
        assert_eq!(text.lines().count(), 4);
        assert_eq!(text.lines().last(), Some("  0.0000000000   0.0000000000   0.0000000000   1.0000000000"));
        settings
            .vars
            .insert(defects::ENV.to_owned(), "reproduce".to_owned());
        let (c, _, err) = run(&["aff.mat", "r2.mat"], &settings);
        assert_eq!(c, 1);
        assert!(err.starts_with(TRACEBACK), "{err}");
        assert!(!dir.path().join("r2.mat").exists());
    }
}
