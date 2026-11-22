// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_add_xform_to_header`'s golden cases on every platform: the program on a 32^3 byte volume
//! made with integer arithmetic, as an MGZ volume and as a COR folder (`fs_xform/cases.rs`'s
//! `golden_cases`: recon-all's call, the transform read, an MGH output, a COR folder's info), the
//! SHA-256 of each case's line (exit status, standard output and error, every file after the
//! run), against the value FreeSurfer 8.2.0's official program gives on the same inputs
//! (`validation/probes/fs-add-xform/check.py --golden`, Linux x64) with `reproduce`, and against
//! the port's own Linux x64 value with the default.

#[path = "fs_xform/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 by `check.py --golden` from the official program on Linux x64: 5 cases.
const OFFICIAL_SHA256: &str = "92ff34ba55079c9cccf3b68a3f86b6ebbf3927a435c384d802153acd51d847cb";
/// The port's default on Linux x64 (FS-1158 writes the COR folder's transform name with `-c`).
const FIX_SHA256: &str = "8c733367fad9a2d1e0e584d3960dbe8280b527b0df4f00e7b1590a8a37ebcdbf";

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
/// mri_add_xform_to_header_golden -- --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
