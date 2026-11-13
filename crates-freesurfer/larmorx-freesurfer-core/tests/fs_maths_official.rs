// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.maths against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (750 cases, ten of each of the
//! 75 commands), recorded by `validation/probes/fs-maths/check.py --seed 1 --n 750` on Linux x64
//! from the unit oracle's build (2026-11-13: 723 lines identical, 27 hashed as `SKIP`: where
//! FreeSurfer reads memory it does not own or never wrote, FS-22, FS-16, FS-42, FS-49, ITK-106,
//! ITK-110, ITK-111, and the non-finite matrices the port refuses, FS-47, FS-48), against the
//! port's output with `reproduce`.

#[path = "fs_maths/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "b6452da0610342e6ff5272ef67167f555327114b533cbaba53ac23a809fd270f";

/// `check.py`'s `SKIP`.
const SKIP: [&str; 9] = [
    "FS-22", "FS-16", "ITK-106", "FS-42", "ITK-110", "ITK-111", "FS-47", "FS-48", "FS-49",
];

#[test]
fn reproduce_gives_the_official_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().join("plot.tmp");
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, 750) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce, &tmp);
        if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(skipped, 27);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}
