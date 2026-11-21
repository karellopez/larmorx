// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_normalize`'s own functions and options against FreeSurfer 8.2.0's official code on every
//! platform: the SHA-256 of the official build's output lines for the probe's cases of seed 1,
//! recorded by `validation/probes/fs-normalize/check.py --seed 1 --n <N>` on Linux x64 from the
//! unit oracle's build, against the port's output with `reproduce` (lines where FreeSurfer
//! touches memory outside a volume or crashes, `check.py`'s `SKIP`, hashed as `SKIP`).

#[path = "fs_normalize/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-18 from the clean commit 42773b91 (`mri_normalize`'s own functions: the bias of
/// `-w`, the white-matter outlier removals, the white matter's interior, the off-white-matter
/// removal, the T2 outlier walk, the thresholds; its option loop and help): 450 cases (50 rounds
/// of 9 commands); re-recorded 2026-11-21 from the clean commit 2f7297d8 (relay 48 of track D: the
/// four cases of a label that cannot be opened compared, fs.core modelling the C library's first
/// `perror` on a write-only standard error): 433 lines identical, 17 hashed as `SKIP`.
const N: usize = 450;
const OFFICIAL_SHA256: &str = "7a01bc1b632705893ce8ea8a621355eb7bbca4b114a6234293cc8c9af82b8f9f";
const SKIPPED: usize = 17;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 6] = ["FS-04", "FS-53", "FS-58", "FS-540", "FS-541", "FS-1016"];

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
