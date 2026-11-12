// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.maths against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (490 cases, ten of each of the
//! 49 commands), recorded by `validation/probes/fs-maths/check.py --seed 1 --n 490` on Linux x64
//! from the unit oracle's build (2026-11-12: 481 lines identical, 9 where FreeSurfer reads outside
//! its arrays, FS-22, hashed as `SKIP`), against the port's output with `reproduce`.

#[path = "fs_maths/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "394127d7bcefb0a2849cdac19b2465fb05c1ace98069d89983a22fb95e224054";

#[test]
fn reproduce_gives_the_official_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().join("plot.tmp");
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, 490) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce, &tmp);
        if ids.contains(&"FS-22") {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(skipped, 9);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}
