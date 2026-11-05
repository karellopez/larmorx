// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl pairreg`: FSL's `pairreg` script (`specs/flirt.md` §15.8), a brain-and-skull
//! registration: three runs of the original's `flirt` with FSL's `pairreg1.sch` to
//! `pairreg3.sch` schedules (carried by the original, `flirt::schedule::PAIRREG_1` to `_3`: no
//! `$FSLDIR` needed) and one `convert_xfm -fixscaleskew`, in this process.

use std::io::Write;

use super::Settings;
use super::common::refuse;
use crate::flirt::schedule;

/// The usage text (observed: standard output; the script names itself as it was called).
pub const USAGE: &str = "Usage: pairreg brain1 brain2 skull1 skull2 outputmatrix [extra flirt args]\n\n  NB: brain1 and skull1 are used as -ref arguments internally\n";

/// `pairreg` with FSL's arguments; returns the exit status.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if args.len() < 5 {
        let _ = write!(out, "{USAGE}");
        return 1;
    }
    if args.iter().any(|a| a.contains(['*', '?', '['])) {
        return refuse(
            err,
            "pairreg",
            "an argument holds a shell pattern character (*, ? or [): the script passes its arguments unquoted, so the shell would expand it",
        );
    }
    let (b1, b2, s1, s2, omat) = (&args[0], &args[1], &args[2], &args[3], &args[4]);
    let extra: Vec<String> = args[5..]
        .iter()
        .flat_map(|a| a.split([' ', '\t', '\n']).filter(|w| !w.is_empty()))
        .map(str::to_owned)
        .collect();
    // the schedules' names as the script gives them (an extra -schedule given after them wins,
    // as FLIRT's parser takes the last); each name runs the text the original carries
    let sch = |k: u8| format!("$FSLDIR/etc/flirtsch/pairreg{k}.sch");
    let pid = std::process::id();
    let tmp = |k: u8| format!("{omat}.{k}.{pid}");
    let words = |v: &[&str]| -> Vec<String> {
        v.iter()
            .map(|s| (*s).to_owned())
            .chain(extra.iter().cloned())
            .collect()
    };
    let (t1, t2, t3) = (tmp(1), tmp(2), tmp(3));
    let (sch1, sch2, sch3) = (sch(1), sch(2), sch(3));
    let carried = [
        (sch1.as_str(), schedule::PAIRREG_1),
        (sch2.as_str(), schedule::PAIRREG_2),
        (sch3.as_str(), schedule::PAIRREG_3),
    ];
    // a step FSL aborts (signal 6) is followed by the shell's `Aborted` line (observed,
    // 2026-11-05: convert_xfm -fixscaleskew of FLIRT's NaN matrices)
    let step = |err: &mut dyn Write, run: &mut dyn FnMut(&mut dyn Write)| {
        let mut buf = Vec::new();
        run(&mut buf);
        let _ = err.write_all(&buf);
        if String::from_utf8_lossy(&buf).contains(super::common::ABORTED) {
            let _ = writeln!(err, "Aborted");
        }
    };
    let flirt = |a: Vec<String>, out: &mut dyn Write, err: &mut dyn Write| {
        step(err, &mut |e: &mut dyn Write| {
            super::flirt::main_carrying(&a, &carried, settings, out, e);
        });
    };
    flirt(
        words(&["-in", b2, "-ref", b1, "-schedule", &sch1, "-omat", &t1]),
        out,
        err,
    );
    flirt(
        words(&[
            "-in",
            s2,
            "-ref",
            s1,
            "-init",
            &t1,
            "-schedule",
            &sch2,
            "-omat",
            &t2,
        ]),
        out,
        err,
    );
    let fix: Vec<String> = ["-fixscaleskew", &t2, &t1, "-omat", &t3]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    step(err, &mut |e: &mut dyn Write| {
        super::convert_xfm::main(&fix, settings, out, e);
    });
    flirt(
        words(&[
            "-in",
            b2,
            "-ref",
            b1,
            "-init",
            &t3,
            "-schedule",
            &sch3,
            "-omat",
            omat,
        ]),
        out,
        err,
    );
    for t in [&t1, &t2, &t3] {
        let _ = std::fs::remove_file(settings.path(t));
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_and_refusals() {
        let settings = Settings::default();
        let run = |words: &[&str]| {
            let args: Vec<String> = words.iter().map(|s| (*s).to_owned()).collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let c = main(&args, &settings, &mut out, &mut err);
            (
                c,
                String::from_utf8(out).unwrap(),
                String::from_utf8(err).unwrap(),
            )
        };
        assert_eq!(run(&["a", "b", "c", "d"]), (1, USAGE.into(), String::new()));
        let (c, _, err) = run(&["a*", "b", "c", "d", "o.mat"]);
        assert_eq!(c, 1);
        assert!(err.contains("shell pattern"), "{err}");
    }

    /// The carried schedules (§15.8; FSL's lines as `flirt -v` echoes them, observed): the
    /// default's steps 1 to 3, the 2 mm stage from the identity, the rigid subset at 2 and 1 mm.
    #[test]
    fn the_carried_schedules() {
        let lines = |t: &str| t.split('\n').map(str::to_owned).collect::<Vec<_>>();
        let (a, b, c) = (
            lines(schedule::PAIRREG_1),
            lines(schedule::PAIRREG_2),
            lines(schedule::PAIRREG_3),
        );
        assert_eq!((a.len(), b.len(), c.len()), (73, 22, 32));
        // pairreg1.sch: the 3D default's lines up to its 2 mm stage's sort (no copy into UF,
        // no 1 mm stage), then an empty line
        let default: Vec<&str> = schedule::DEFAULT_3D.lines().collect();
        let n = default.iter().position(|l| *l == "copy U UF").unwrap();
        assert_eq!(a[..n], default[..n]);
        assert_eq!(a[n], "");
        assert!(b[0] == "# 2mm scale" && b[21].is_empty() && b[20] == "sort U");
        assert!(c[3].starts_with("setoption paramsubset 6  1 0") && c[3].ends_with(" 0 "));
        assert!(c[30].is_empty() && c[31].is_empty());
    }
}
