// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.io against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the official
//! build's output lines for the probe's portable cases of seed 1 (1,000 cases, a hundred of each
//! of the ten commands that behave alike on every platform), recorded by
//! `validation/probes/fs-io/check.py --seed 1 --n 1000 --portable` on Linux x64 from the unit
//! oracle's build (2026-11-13: 911 lines identical, 106 of them with memory digits; 89 hashed as
//! `SKIP`: 48 byte swaps past their buffer, FS-175, 28 tables FreeSurfer reads for ever or prints
//! with another type's format, FS-172 and FS-173, 13 results that depend on memory, FS-168 and
//! FS-170), against the port's output with `reproduce`. A `?` digit (a byte
//! FreeSurfer reads from memory contents) is hashed as `?`; the lines the port marks `SKIP` or
//! `VARY` are hashed as `SKIP`.

#[path = "fs_io/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "ce3ddcd0e2dee840bb43ec0d0d9ba4fee8f1b4004f0b22a976b19601b573d34f";
const SKIPPED: usize = 89;

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
    for line in cases::generate(1, 1000, true) {
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
