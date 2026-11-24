// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The golden cases of `seg2cc`, `mergeseg` and `fscalc` on every platform
//! (`fs_scripts/cases.rs`'s `golden_indices`: seg2cc's conformed path to mri_cc, a missing
//! segmentation and its usage; mergeseg with a segid and with a float merge volume holding NaNs
//! and infinities; fscalc's sum, and its sum with that volume; 32^3 volumes): the SHA-256 of their
//! canonical texts, against the value FreeSurfer 8.2.0's official scripts give
//! (`validation/probes/fs-scripts/check.py`, Linux x64) with `reproduce`, and against the port's
//! own Linux x64 value with the default (mri_cc's and mri_binarize's error lines without
//! FreeSurfer's NUL byte, FS-01; FS-841).

#[path = "fs_scripts/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-24 by `check.py` from the official scripts on Linux x64: 7 cases.
const OFFICIAL_SHA256: &str = "3bb3c52c3edc783925c16d73ff2f6eddaa692f151a551a51650c03abe10950aa";
/// The default's value on Linux x64 (`print_digests`).
const FIX_SHA256: &str = "b951eee608f0ac5bd2e9eca9de4d5e8707a2da8c2a9625766debece0346a4834";

fn digest(mode: UpstreamDefects) -> String {
    let mut h = Sha256::new();
    for k in cases::golden_indices() {
        let (text, _) = cases::run_case(k, mode);
        h.update(text.as_bytes());
        h.update(b"\n");
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn reproduce_gives_the_official_outputs() {
    assert_eq!(digest(UpstreamDefects::Reproduce), OFFICIAL_SHA256);
}

#[test]
fn fix_gives_the_recorded_outputs() {
    assert_eq!(digest(UpstreamDefects::Fix), FIX_SHA256);
}

/// Prints both values (on Linux x64: `cargo test --test scripts_golden -- --ignored
/// --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", digest(UpstreamDefects::Reproduce));
    println!("fix {}", digest(UpstreamDefects::Fix));
}
