// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surfio against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (990 cases, thirty of each of
//! the 33 commands; `metric`'s through the probe built with a stub metric-properties step),
//! recorded by `validation/probes/fs-surfio/check.py --seed 1 --n 990` on Linux x64 from the unit
//! oracle's build (2026-11-13, relay 13: 980 lines identical, 3 `mprop` cases, 5 `stl`, 1 `sfile`
//! and 1 `cfile` case counted, not compared, where FreeSurfer reads memory it never wrote or does
//! not own: `SKIP`),
//! against the port's output with `reproduce` (a case the port marks with one of those defects
//! hashed as `SKIP`, as `check.py` does). The `sfile`, `cfile`, `sread` and `stl` cases run in a
//! temporary folder of their own.

#[path = "fs_surfio/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use sha2::{Digest, Sha256};

/// `check.py`'s `SKIP` and `SKIP_WHEN_REFUSED`.
const SKIP: [&str; 12] = [
    "FS-121", "FS-123", "FS-06", "FS-167", "FS-49", "FS-140", "FS-144", "FS-151", "FS-154", "FS-157",
    "FS-158", "FS-160",
];
const SKIP_WHEN_REFUSED: [&str; 1] = ["FS-114"];

const OFFICIAL_SHA256: &str = "2543d2708fc953a9ec60e5dff31e85b7799e1a20285c2514b338e30f7b7abdaf";

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    for line in cases::generate(1, 990) {
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
    for line in cases::generate(1, 900 * 4) {
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
