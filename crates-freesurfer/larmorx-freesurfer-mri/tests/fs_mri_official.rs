// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.mri against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the official
//! build's output lines for the probe's cases of seed 1 (980 cases, twenty of each of the 49
//! commands), recorded by `validation/probes/fs-mri/check.py --seed 1 --n 980` on Linux x64 from
//! the unit oracle's build (2026-11-13, with the bounding box and the linear transform array of
//! relay 12: 922 lines identical, 58 hashed as `SKIP`: where FreeSurfer reads or writes memory it
//! does not own, FS-53, FS-198 and FS-163 where the port refuses the copy, or crashes, FS-58,
//! FS-165), against the port's output with `reproduce`.

#[path = "fs_mri/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "ecce606b6218d98ab5325c5e7234805bf08d4da20179bd9caaad57cb4a4793e5";

/// `check.py`'s `SKIP`, and its `SKIP_WHEN_REFUSED` (skipped where the port's line is `EXIT`).
const SKIP: [&str; 4] = ["FS-53", "FS-58", "FS-165", "FS-198"];
const SKIP_WHEN_REFUSED: [&str; 1] = ["FS-163"];

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, 980) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let refused = out.starts_with("EXIT ");
        if ids.iter().any(|x| SKIP.contains(x))
            || (refused && ids.iter().any(|x| SKIP_WHEN_REFUSED.contains(x)))
        {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(skipped, 58);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}
