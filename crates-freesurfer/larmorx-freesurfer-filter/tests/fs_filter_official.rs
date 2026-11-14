// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.filter against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1, recorded by
//! `validation/probes/fs-filter/check.py --seed 1 --n <N>` on Linux x64 from the unit oracle's build,
//! against the port's output with `reproduce` (lines where FreeSurfer touches memory outside a
//! volume, crashes or hangs, `check.py`'s `SKIP`, hashed as `SKIP`).

#[path = "fs_filter/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-14 from the clean commit 6700b476 (the distance maps, the convolutions,
/// smoothing, window statistics, reductions, derivatives, border and non-parametric
/// classification, connected components, the frame functions and the volume clusters): 1,380
/// cases, 1,247 lines identical, 133 hashed as `SKIP`.
const N: usize = 1380;
const OFFICIAL_SHA256: &str = "91ea7bc20dc852444c665d431a47a23c024a7532f707939f0f7733a88ba27449";
const SKIPPED: usize = 133;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 24] = [
    "FS-540", "FS-541", "FS-58", "FS-53", "FS-546", "FS-548", "FS-549", "FS-550", "FS-551",
    "FS-552", "FS-555", "FS-163", "FS-559", "FS-560", "FS-561", "FS-562", "FS-564", "FS-567",
    "FS-568", "FS-569", "FS-570", "FS-571", "FS-573", "FS-575",
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
