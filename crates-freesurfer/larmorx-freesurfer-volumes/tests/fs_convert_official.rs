// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_convert`'s stats tables, CSV reader and command line against FreeSurfer 8.2.0's official
//! code on every platform: the SHA-256 of the official build's output lines for the probe's cases
//! of seed 1, recorded by `validation/probes/fs-convert/check.py --seed 1 --n <N>` on Linux x64 from
//! the unit oracle's build, against the port's output with `reproduce` (lines where FreeSurfer
//! reads memory it does not own or crashes, `check.py`'s `SKIP`, hashed as `SKIP`).

#[path = "fs_convert/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-18 from the clean commit de5d1402 (`mri_convert`'s stats tables in its three
/// formats, `csv.h`'s reader, its command line to the parse-only report and the checks before the
/// input is read; 45 fixed cases, then seeded ones): 1,000 cases, 947 lines identical, 53 hashed as
/// `SKIP`.
const N: usize = 1000;
const OFFICIAL_SHA256: &str = "0d0e0ee84677d1dd68be7857618595bbde5b3233625fb734cc060783e3192c5d";
const SKIPPED: usize = 53;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 13] = [
    "FS-170", "FS-171", "FS-1031", "FS-1032", "FS-1034", "FS-1035", "FS-1036", "FS-1037",
    "FS-1038", "FS-1039", "FS-1040", "FS-1041", "FS-1042",
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
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!((skipped, hex.as_str()), (SKIPPED, OFFICIAL_SHA256));
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
