// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.gca's `MRIStransform` against FreeSurfer 8.2.0's official code on every platform: the SHA-256
//! of the official build's output lines for the fs-gcam probe's cases of seed 1 (600 `mst` cases:
//! surfaces through linear transform arrays and GCA morphs, the volumes given in the five ways),
//! recorded by `validation/probes/fs-gcam/check.py --seed 1 --n 600` on Linux x64 from the unit
//! oracle's build (track A's relay 45, 2026-11-17), against the port's output with `reproduce`. The
//! lines where FreeSurfer crashes or takes memory contents are hashed as `SKIP` (FS-858, FS-940,
//! FS-942).

#[path = "fs_gca/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "a4da083b243f9d8b49451d2aaae2c68705ac4d62fab69f319a6090f22adefca7";

/// `check.py`'s `SKIP`.
const SKIP: [&str; 3] = ["FS-858", "FS-940", "FS-942"];

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, 600) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(skipped, 125);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}
