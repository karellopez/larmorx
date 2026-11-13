// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.maths against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (670 cases, ten of each of the
//! 67 commands), recorded by `validation/probes/fs-maths/check.py --seed 1 --n 670` on Linux x64
//! from the unit oracle's build (2026-11-13: 657 lines identical, 13 where FreeSurfer reads memory
//! it does not own, FS-22, FS-16 or ITK-106, hashed as `SKIP`), against the port's output with
//! `reproduce`.

#[path = "fs_maths/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "642c41982a861dcf793433c0d73274d9672611a0e36612a0312a31cd2c71c3b9";

#[test]
fn reproduce_gives_the_official_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().join("plot.tmp");
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, 670) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce, &tmp);
        if ids.contains(&"FS-22") || ids.contains(&"FS-16") || ids.contains(&"ITK-106") {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(skipped, 13);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}
