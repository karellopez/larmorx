// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl pairreg`: FSL's `pairreg` script (`specs/flirt.md` §15.8), a brain-and-skull
//! registration: three runs of the original's `flirt` with FSL's `pairreg1.sch` to
//! `pairreg3.sch` schedules (read from `$FSLDIR/etc/flirtsch`, as the script reads them) and one
//! `convert_xfm -fixscaleskew`, in this process.

use std::io::Write;

use super::Settings;
use super::common::refuse;

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
    let Some(fsldir) = settings.get("FSLDIR").filter(|d| !d.is_empty()) else {
        return refuse(
            err,
            "pairreg",
            "FSLDIR is not set: pairreg's schedules (pairreg1.sch to pairreg3.sch) are read from $FSLDIR/etc/flirtsch, as FSL's script reads them",
        );
    };
    let (b1, b2, s1, s2, omat) = (&args[0], &args[1], &args[2], &args[3], &args[4]);
    let extra: Vec<String> = args[5..]
        .iter()
        .flat_map(|a| a.split([' ', '\t', '\n']).filter(|w| !w.is_empty()))
        .map(str::to_owned)
        .collect();
    let sch = |k: u8| format!("{fsldir}/etc/flirtsch/pairreg{k}.sch");
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
    let flirt = |a: Vec<String>, out: &mut dyn Write, err: &mut dyn Write| {
        super::flirt::main(&a, settings, out, err);
    };
    flirt(
        words(&["-in", b2, "-ref", b1, "-schedule", &sch1, "-omat", &t1]),
        out,
        err,
    );
    flirt(
        words(&[
            "-in", s2, "-ref", s1, "-init", &t1, "-schedule", &sch2, "-omat", &t2,
        ]),
        out,
        err,
    );
    let fix: Vec<String> = ["-fixscaleskew", &t2, &t1, "-omat", &t3]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    super::convert_xfm::main(&fix, settings, out, err);
    flirt(
        words(&[
            "-in", b2, "-ref", b1, "-init", &t3, "-schedule", &sch3, "-omat", omat,
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
        let (c, _, err) = run(&["a", "b", "c", "d", "o.mat"]);
        assert_eq!(c, 1);
        assert!(err.contains("FSLDIR is not set"), "{err}");
        let (c, _, err) = run(&["a*", "b", "c", "d", "o.mat"]);
        assert_eq!(c, 1);
        assert!(err.contains("shell pattern"), "{err}");
    }
}
