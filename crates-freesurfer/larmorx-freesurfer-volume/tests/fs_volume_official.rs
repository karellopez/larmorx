// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.volume against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1, recorded by
//! `validation/probes/fs-volume/check.py --seed 1 --n <N>` on Linux x64 from the unit oracle's build,
//! against the port's output with `reproduce` (lines where FreeSurfer reads or writes memory it does
//! not own or crashes, FS-53, FS-58, FS-288, FS-290, FS-294, and the eigen-systems it never returns from, FS-47, FS-48,
//! ITK-112, ITK-115, ITK-116, hashed as `SKIP`).

#[path = "fs_volume/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-13 (46 commands: transform.cpp's VOL_GEOM functions, 3.1-35's voxel
/// arithmetic, coordinates and geometry): 920 cases, 862 lines identical, 58 hashed as `SKIP`.
const N: usize = 920;
const OFFICIAL_SHA256: &str = "772e44de139a4686eb3a943a5964efe8d8031a860511d70ddfaeae65b9bd0f93";
const SKIPPED: usize = 58;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 10] = [
    "FS-288", "FS-53", "FS-58", "FS-290", "FS-294", "FS-47", "FS-48", "ITK-112", "ITK-115", "ITK-116",
];

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, N) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(skipped, SKIPPED);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}

/// The default differs from `reproduce` only where a defect it corrects is noted.
#[test]
fn fix_differs_only_with_a_notice() {
    for line in cases::generate(1, N) {
        let (r, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (f, ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(r == f || !ids.is_empty(), "{line}");
    }
}
