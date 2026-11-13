// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.io against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the official
//! build's output lines for the probe's recorded cases (`cases::recorded_cases`: seed 1's 4,800
//! portable cases, about 87 of each of the 55 commands that behave alike on every platform: piece
//! 1's ten, piece 2's twelve, piece 3's six, piece 4's seven (the format probe's file cases made
//! portable), piece 5's seven (mosaics, MATLAB files), piece 6's five (reading volumes and
//! registration files) and piece 7's seven (Bruker studies), without those that take seconds in a
//! debug build, `cases::HEAVY_SEED1`, which the probe's runs keep), recorded by
//! `validation/probes/fs-io/check.py --seed 1 --n 4800 --portable --light` on Linux x64 from the
//! unit oracle's build (2026-11-13, track A relay 17, with piece 7: 4,799 cases, 4,192 lines
//! identical, 81 of them with memory digits; 607 hashed as `SKIP`), against the port's output with
//! `reproduce`. A `?` digit (a byte FreeSurfer reads from memory contents) is hashed as `?`; the
//! lines the port marks `SKIP` or `VARY` are hashed as `SKIP` (lines where FreeSurfer crashes,
//! reads for ever, depends on the machine or on memory contents, and formats not ported yet).

#[path = "fs_io/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "93576fdf78cc9d86e46e5d625dc9a240155dead5e835f7bce4b1faeb8552e6d8";
const SKIPPED: usize = 607;

#[test]
fn reproduce_gives_the_official_outputs() {
    let dir = tempfile::Builder::new()
        .prefix("fs io ~")
        .tempdir()
        .expect("a temporary directory");
    let w = cases::Work {
        fix: dir.path().join("fix"),
        dat: dir.path().join("dat"),
    };
    cases::make_fixture(&w.fix);
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::recorded_cases() {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce, &w);
        if ids.iter().any(|x| *x == "SKIP" || *x == "VARY") {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    cases::remove_fixture(&w.fix);
    assert_eq!(skipped, SKIPPED);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}
