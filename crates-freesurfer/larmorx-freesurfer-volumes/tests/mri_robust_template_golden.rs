// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_template`'s golden cases on every platform: the program on 16^3 images made with
//! integer arithmetic (`fs_robtmpl/cases.rs`' `golden_cases`: fMRIPrep's call on three inputs,
//! recon-all's motion correction on two, the rigid mean space of two, one global iteration of three
//! with weights, an input with NaN voxels built from their bits, and a 4-D input's one `--mapmov`
//! image), the
//! SHA-256 of each case's line (exit status, standard output and error, every file written; the time
//! taken masked), against the value FreeSurfer 8.2.0's official program gives on the same inputs
//! (`validation/probes/fs-robust-template/check.py --golden --preload`, Linux x64, the decided `cabs`,
//! `csqrt` and `acos`) with `reproduce`, and against the port's own Linux x64 value with the default.
//! The cases run side by side, so the debug build's test takes a few seconds.

#[path = "fs_robtmpl/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-24 (relay 51: the cases on 16^3 images) by `check.py --golden --preload` from
/// the official program on Linux x64: 6 cases.
const OFFICIAL_SHA256: &str = "c22789e352318a1b5c26ba6b669100ebe2251c901b2eba56ccd44350e4203533";
/// The port's default on Linux x64 (FS-1090: the 4-D `--mapmov` with the template's header).
const FIX_SHA256: &str = "9b573b7c92e2a894e1e1f9ddc4eb1e84e02372482b29621814d1b487a51afbe3";

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

/// Prints the values (run on Linux x64 to record them: `cargo test --test
/// mri_robust_template_golden -- --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
