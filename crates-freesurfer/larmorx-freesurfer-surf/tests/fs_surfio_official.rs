// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surfio against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (1,200 cases, thirty of each of
//! the 40 commands; `metric`'s through the probe built with a stub metric-properties step),
//! recorded by `validation/probes/fs-surfio/check.py --seed 1 --n 1200` on Linux x64 from the unit
//! oracle's build (2026-11-13, relay 19: 1,161 lines identical, 39 counted, not compared, where
//! FreeSurfer reads memory it never wrote or does not own, writes past a buffer or loops for ever:
//! `SKIP`), against the port's output with `reproduce` (a case the port marks with one of those
//! defects hashed as `SKIP`, as `check.py` does). The `sfile`, `cfile`, `sread`, `stl`, `tri`,
//! `fnam`, `swr`, `srd`, `mpb`, `mpc` and `mpd` cases run in a temporary folder of their own.

#[path = "fs_surfio/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use sha2::{Digest, Sha256};

/// `check.py`'s `SKIP` and `SKIP_WHEN_REFUSED`.
const SKIP: [&str; 32] = [
    "FS-121", "FS-123", "FS-06", "FS-167", "FS-49", "FS-140", "FS-144", "FS-151", "FS-154",
    "FS-157", "FS-158", "FS-160", "FS-346", "FS-352", "FS-354", "FS-356", "FS-357", "FS-358",
    "FS-359", "FS-360", "FS-361", "FS-362", "FS-367", "FS-370", "FS-371", "FS-372", "FS-373",
    "FS-76", "FS-376", "FS-379", "FS-48h",
];
const SKIP_WHEN_REFUSED: [&str; 1] = ["FS-114"];

const OFFICIAL_SHA256: &str = "fef1c552a18bbce1b96b8bb346c4d48efdf69a581782beb6f98a9dff0b217614";

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    for line in cases::generate(1, 1200) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let refused = out.starts_with("EXIT ");
        let skip = ids.iter().any(|x| SKIP.contains(x))
            || (refused && ids.iter().any(|x| SKIP_WHEN_REFUSED.contains(x)));
        h.update(if skip {
            "SKIP".as_bytes()
        } else {
            out.as_bytes()
        });
        h.update(b"\n");
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}

/// FS-120 is kept by default (rule D, until measured): the default mode never notes FS-120 on
/// an `mprop` case; the correction on request (`fs120_write_face_angles` with `fix`) changes
/// only cases it notes (some of them in what they print: a later step can recompute the
/// angles); with `reproduce` the request changes nothing.
#[test]
fn fs120_the_face_angles_are_written_back_only_on_request() {
    let on = || SurfGlobals {
        fs120_write_face_angles: true,
        ..SurfGlobals::default()
    };
    let (mut noted, mut changed, mut cases) = (0, 0, 0);
    for line in cases::generate(1, 103 * 40) {
        if !line.starts_with("mprop ") {
            continue;
        }
        cases += 1;
        let (fix, fix_ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(!fix_ids.contains(&"FS-120"), "{line}");
        let (rep, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (rep_on, _) = cases::run_line_with(&line, UpstreamDefects::Reproduce, on());
        assert_eq!(rep, rep_on, "{line}");
        let (fix_on, on_ids) = cases::run_line_with(&line, UpstreamDefects::Fix, on());
        if on_ids.contains(&"FS-120") {
            noted += 1;
            changed += usize::from(fix != fix_on);
        } else {
            assert_eq!(fix, fix_on, "{line}");
        }
    }
    assert!(
        cases >= 100 && noted > 0 && changed > 0,
        "{cases} cases, {noted} noted, {changed} changed"
    );
    eprintln!("FS-120 on request: {cases} mprop cases, {noted} noted, {changed} changed");
}
