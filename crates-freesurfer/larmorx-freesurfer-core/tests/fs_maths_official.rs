// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.maths against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (470 cases, ten of each
//! command), recorded by `validation/probes/fs-maths/check.py` on Linux x64 from the unit
//! oracle's build (2026-11-12: 461 lines identical, 9 where FreeSurfer reads outside its arrays,
//! FS-22, hashed as `SKIP`), against the port's output with `reproduce`.

#[path = "fs_maths/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "658b0ffaf12d685e225df01da0b23bf7e959d9d050b6ffc8457ae29764aebadb";

#[test]
fn reproduce_gives_the_official_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().join("plot.tmp");
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, 470) {
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
