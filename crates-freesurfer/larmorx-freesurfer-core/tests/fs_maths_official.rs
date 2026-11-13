// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.maths against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (770 cases, ten of each of the
//! 77 commands), recorded by `validation/probes/fs-maths/check.py --seed 1 --n 770` on Linux x64
//! from the unit oracle's build (2026-11-13, with `fnvh` of relay 14 (track A): 747 lines
//! identical, 23 hashed as `SKIP`: where
//! FreeSurfer reads memory it does not own or never wrote, FS-22, FS-16, FS-42, FS-49, ITK-106,
//! ITK-110, ITK-111, and the non-finite matrices the port refuses, FS-47, FS-48), against the
//! port's output with `reproduce`.

#[path = "fs_maths/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "ae473ef0c96b709d7b92307e24d7ad8e9779211f1e913e4f1876d0a85185e1d2";

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
    for line in cases::generate(1, 770) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce, &tmp);
        if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(skipped, 23);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}
