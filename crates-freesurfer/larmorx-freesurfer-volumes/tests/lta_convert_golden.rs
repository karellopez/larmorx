// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The golden cases of `lta_convert` (rule 10 of larmorx's CLAUDE.md): recon-all's step 4,
//! talairach_avi's call, an ITK registration (as fMRIPrep's) to an `.lta`, an FSL matrix and a
//! `register.dat`, an `.lta` to ITK, and an inverse to ITK, voxel space, NiftyReg and MINC, on
//! volumes up to 32^3 (`tests/fs_ltaconv/cases.rs`' `golden_cases`). Each case's line (exit
//! status, both streams and every file after the run, the folder as `<DIR>`) is hashed; the
//! official program's value was recorded by `validation/probes/fs-ltaconv/check.py --golden` on
//! Linux x64, and every platform must give it with `reproduce` and the recorded default.

#[path = "fs_ltaconv/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 by `check.py --golden` from the official program on Linux x64: 5 cases.
const OFFICIAL_SHA256: &str = "22d2636b6f0feee5166d8eac6fcd19d0cb15d4a7c4ec7703b37c15a6e038636c";
/// The port's default on Linux x64 (the same: no defect is reached).
const FIX_SHA256: &str = "22d2636b6f0feee5166d8eac6fcd19d0cb15d4a7c4ec7703b37c15a6e038636c";

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

/// Prints both values (run on Linux x64 to record them: `cargo test --test lta_convert_golden --
/// --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
