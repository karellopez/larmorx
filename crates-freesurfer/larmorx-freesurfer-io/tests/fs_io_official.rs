// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.io against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the official
//! build's output lines for the probe's recorded cases (`cases::recorded_cases`: seed 1's 4,800
//! portable cases, about 51 of each of the 95 commands that behave alike on every platform: piece
//! 1's ten, piece 2's twelve, piece 3's six, piece 4's seven (the format probe's file cases made
//! portable), piece 5's seven (mosaics, MATLAB files), piece 6's five (reading volumes and
//! registration files), piece 7's seven (Bruker studies), piece 8's twelve (2-D images), piece 9's
//! one (fs.core's ArgumentParser) and piece 10's sixteen (fs.labels' text colour tables and
//! lookups, registration files), without those that take seconds in a debug build,
//! `cases::HEAVY_SEED1`, which the probe's runs keep), recorded by `validation/probes/fs-io/check.py --seed 1 --n 4800 --portable
//! --light` on Linux x64 from the unit oracle's build (2026-11-13, track A relay 19, with piece 10:
//! 4,797 cases, 4,217 lines identical, 49 of them with memory digits; 580 hashed as `SKIP`),
//! against the port's output with `reproduce`. A `?` digit (a byte FreeSurfer reads from memory
//! contents) is hashed as `?`; the lines the port marks `SKIP` or `VARY` are hashed as `SKIP`
//! (lines where FreeSurfer crashes, reads for ever, depends on the machine or on memory contents,
//! and formats not ported yet).

#[path = "fs_io/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "48b6b766e91769872cd3b03268c5e30731d504117c3ebb949a50563106f1c1f2";
const SKIPPED: usize = 506;

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
