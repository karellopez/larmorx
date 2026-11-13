// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surfio against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (450 cases, thirty of each of
//! the 15 commands), recorded by `validation/probes/fs-surfio/check.py --seed 1 --n 450` on Linux
//! x64 from the unit oracle's build (2026-11-13: 450 lines identical), against the port's output
//! with `reproduce`.

#[path = "fs_surfio/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "0cbc85857870611ee541f61cfe41040af6675957c02a27b57eb9825576f93358";

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    for line in cases::generate(1, 450) {
        let (out, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        h.update(out.as_bytes());
        h.update(b"\n");
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}
