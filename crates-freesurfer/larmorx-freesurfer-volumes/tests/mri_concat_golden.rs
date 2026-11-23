// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The golden cases of `mri_concat` (rule 10 of larmorx's CLAUDE.md): `pctsurfcon`'s percent
//! contrast (`--paired-diff-norm --mul 100` of two surface overlays) and `mri_mcadura_seg`'s sum of
//! two segmentations with a colour table, a concatenation into NIfTI, the frame mean and standard
//! deviation, weighted frames' maximum index, a matrix of the frames, a masked vote, the first
//! frame not zero, NaN made by `--mul +nan` and by NaN weights, an input with NaN and infinite
//! voxels (made 0 when read), the principal components with their matrix files, `--zconcat` and a
//! list file's median, on volumes up to 32^3 (`tests/fs_concat/cases.rs`' `golden_cases`). Each
//! case's line (exit status, both streams and every file the run wrote, masked as the probe masks
//! them) is hashed; the official program's value was recorded by
//! `validation/probes/fs-concat/check.py --golden` on Linux x64, and every platform must give it
//! with `reproduce` and the recorded default.

#[path = "fs_concat/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-23 by `check.py --golden` from the official program on Linux x64: 15 cases.
const OFFICIAL_SHA256: &str = "583298963d943406802b3991ffe4dd07565426f02b873d184939a8764e37e65c";
/// The port's default on Linux x64 (FS-01's NUL bytes removed from the written volumes).
const FIX_SHA256: &str = "e4af9f7a836f154b3800f3f117565048c6c9ba6b9d774c4df05efae4f5628f99";

fn cases_digest(mode: UpstreamDefects) -> String {
    let mut h = Sha256::new();
    for line in cases::golden_cases() {
        let (out, _) = cases::run_line(&line, mode);
        h.update(out.as_bytes());
        h.update(b"\n");
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn reproduce_gives_the_official_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Reproduce), OFFICIAL_SHA256);
}

#[test]
fn fix_gives_the_recorded_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Fix), FIX_SHA256);
}

/// Prints both values (run on Linux x64 to record them: `cargo test --test mri_concat_golden --
/// --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
