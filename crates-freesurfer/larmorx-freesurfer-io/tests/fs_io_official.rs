// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.io against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the official
//! build's output lines for the probe's portable cases of seed 1 (3,600 cases, a hundred of each of
//! the 36 commands that behave alike on every platform: piece 1's ten, piece 2's twelve, piece 3's
//! six and piece 4's seven, the format probe's file cases made portable), recorded by
//! `validation/probes/fs-io/check.py --seed 1 --n 3600 --portable` on Linux x64 from the unit
//! oracle's build (2026-11-13, track A relay 14: 3,243 lines identical, 98 of them with memory
//! digits; 357 hashed as `SKIP`: lines where FreeSurfer crashes, reads for ever, depends on the
//! machine or on memory contents), against the port's output with `reproduce`. A `?` digit (a
//! byte FreeSurfer reads from memory contents) is hashed as `?`; the lines the port marks `SKIP` or
//! `VARY` are hashed as `SKIP`.

#[path = "fs_io/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "1caecb5ae7b6148fddfc840e86be0172444eaae4f93fc8fc787b01cc5ded511a";
const SKIPPED: usize = 357;

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
    for line in cases::generate(1, 3600, true) {
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
