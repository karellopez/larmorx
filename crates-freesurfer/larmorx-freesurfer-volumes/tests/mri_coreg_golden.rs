// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_coreg`'s golden cases on every platform: the program on small volumes made with integer
//! arithmetic (`fs_coreg/cases.rs`' `golden_cases`: fMRIPrep's call with a reference mask and a
//! narrowed brute-force search, the 12-parameter schema with every output and an initial
//! registration, the 2-D schema with the centroids and RAS-to-RAS output, `--mat2par` and
//! `--lrrev`), the SHA-256 of each case's line (exit status, standard output and error, every file
//! written; the times and the machine's names masked), against the value FreeSurfer 8.2.0's
//! official program gives on the same inputs (`validation/probes/fs-coreg/check.py --golden`,
//! Linux x64) with `reproduce`, and against the port's own Linux x64 value with the default.
//! The cases run side by side, so the debug build's test takes a few seconds.

#[path = "fs_coreg/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-25 (relay 52, from the clean commit b5e8d6be) by `check.py --golden` from the
/// official program on Linux x64: 5 cases, each identical with the port's `reproduce`.
const OFFICIAL_SHA256: &str = "1e9c03d944ef5c07ca876b55df624c0d824540dcbb07674000e799438dbdf5e8";
/// The port's default on Linux x64 (one case differs from `reproduce`, with its notice).
const FIX_SHA256: &str = "dda58a68640b7dd568f61ff26de1f797d0ac5880c05ac815d77225a2cd260a0f";

/// The cases run side by side (each in its own folder, with its own state), hashed in order.
fn cases_digest(mode: UpstreamDefects) -> String {
    let lines = cases::golden_cases();
    let outs: Vec<String> = std::thread::scope(|sc| {
        let hs: Vec<_> = lines
            .iter()
            .map(|line| sc.spawn(move || cases::run_line(line, mode).0))
            .collect();
        hs.into_iter().map(|h| h.join().expect("a case")).collect()
    });
    let mut h = Sha256::new();
    for out in outs {
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

/// Prints the values (run on Linux x64 to record them: `cargo test --test mri_coreg_golden --
/// --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
