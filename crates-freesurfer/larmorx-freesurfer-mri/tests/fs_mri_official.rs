// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.mri against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the official
//! build's output lines for the probe's cases of seed 1 (400 cases, twenty of each of the 20
//! commands), recorded by `validation/probes/fs-mri/check.py --seed 1 --n 400` on Linux x64 from
//! the unit oracle's build (2026-11-13: 365 lines identical, 35 hashed as `SKIP`: where FreeSurfer
//! reads or writes memory it does not own, FS-53, or crashes, FS-58), against the port's output
//! with `reproduce`.

#[path = "fs_mri/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "723d33a9cde606610cc6ba0245e31e26be01e110a77b49c639c262574d2ecaaa";

/// `check.py`'s `SKIP`.
const SKIP: [&str; 2] = ["FS-53", "FS-58"];

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, 400) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(skipped, 35);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}
