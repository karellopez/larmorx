// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_register`'s golden cases on every platform: the program on 32^3 images made with
//! integer arithmetic (`fs_robreg/cases.rs`' `golden_cases`: fMRIPrep's call with every output, an
//! affine registration with its half-way outputs, a short source onto a float target of
//! anisotropic voxels, the entropy images, the Powell registration), the SHA-256 of each case's line (exit status, standard output and error,
//! every file written; the time taken masked), against the value FreeSurfer 8.2.0's official
//! program gives on the same inputs (`validation/probes/fs-robust-register/check.py --golden`,
//! Linux x64) with `reproduce`, and against the port's own Linux x64 value with the default.

#[path = "fs_robreg/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 by `check.py --golden` from the official program on Linux x64: 3 cases;
/// re-recorded 2026-11-23 (relay 45): 4 cases, the entropy images added; again (relay 46): 5
/// cases, the Powell registration (`--cost NCC`) added (the same value with the decided `cabs` and
/// `csqrt` preloaded).
const OFFICIAL_SHA256: &str = "e965a37c96c37a7ea74241651d24a2d09d23e2adf26faa2bc9268107e91c24a8";
/// The port's default on Linux x64.
const FIX_SHA256: &str = "e965a37c96c37a7ea74241651d24a2d09d23e2adf26faa2bc9268107e91c24a8";

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

/// Prints both values (run on Linux x64 to record them: `cargo test --test
/// mri_robust_register_golden -- --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
