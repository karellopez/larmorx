// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.filter against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1, recorded by
//! `validation/probes/fs-filter/check.py --seed 1 --n <N>` on Linux x64 from the unit oracle's build,
//! against the port's output with `reproduce` (lines where FreeSurfer reads memory outside a volume
//! or crashes, FS-53, FS-58, FS-540, FS-541, hashed as `SKIP`).

#[path = "fs_filter/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-14 from the clean commit 66a5e65f (the distance maps `MRIextractDistanceMap`,
/// `MRIdistanceTransform`): 400 cases, 348 lines identical, 52 hashed as `SKIP`.
const N: usize = 400;
const OFFICIAL_SHA256: &str = "aa81ea376da2b51f6f2f0b03df0522542c3ad826072e65d49b82c51f1fc74d13";
const SKIPPED: usize = 52;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 4] = ["FS-540", "FS-541", "FS-58", "FS-53"];

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
