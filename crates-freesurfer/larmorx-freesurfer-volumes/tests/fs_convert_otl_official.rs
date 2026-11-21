// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_convert`'s outline (`.otl`) series against FreeSurfer 8.2.0's official code on every
//! platform: the SHA-256 of the official build's output lines for the probe's `otl` cases of seed
//! 1 (outline files of every kind with a small colour table, the volume's size from a header-only
//! `--in_like`, the read's options; the written volume through `gzread`, its command line's time,
//! user, machine and kernel version as `X`), recorded by `validation/probes/fs-convert/check.py
//! --kind otl --seed 1 --n <N>` on Linux x64 from the unit oracle's build, against the port's output
//! with `reproduce` (lines where FreeSurfer reads memory it does not own or crashes, `check.py`'s
//! `SKIP_OTL`, hashed as `SKIP`).

#[path = "fs_convert/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-21 (the reader's every path once, then seeded series): 120 cases, 104 lines
/// identical, 16 hashed as `SKIP`.
const N: usize = 120;
const OFFICIAL_SHA256: &str = "8277c8101b5d53821d5172bd20572e56b4693ac683cdb29e0097010f661fe56c";
const SKIPPED: usize = 16;

/// `check.py`'s `SKIP_OTL`.
const SKIP: [&str; 30] = [
    "FS-170", "FS-171", "FS-1031", "FS-1032", "FS-1034", "FS-1035", "FS-1036", "FS-1037",
    "FS-1038", "FS-1039", "FS-1040", "FS-1041", "FS-1042", "FS-53", "FS-163", "FS-1043", "FS-1044",
    "FS-1046", "FS-1047", "FS-1049", "FS-1050", "FS-1051", "FS-1052", "FS-1054", "FS-1057",
    "FS-1141", "FS-1142", "FS-1143", "FS-1144", "FS-1145",
];

/// `reproduce` gives the official lines; the default differs from it only where a defect it
/// corrects is noted (one pass over the cases for both: each case run once in each mode).
#[test]
fn reproduce_gives_the_official_outputs_and_fix_differs_only_with_a_notice() {
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate_otl(1, N) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
        let (f, fids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(out == f || !fids.is_empty(), "{line}");
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!((skipped, hex.as_str()), (SKIPPED, OFFICIAL_SHA256));
}
