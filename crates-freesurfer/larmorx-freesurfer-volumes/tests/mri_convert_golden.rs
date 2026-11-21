// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_convert`'s golden cases on every platform: the program on a 32^3 input made with integer
//! arithmetic and the files written beside it (`mri_convert_golden/common.rs`), the SHA-256 of
//! each case's exit status, standard output (FS-1053's validity flag masked) and output's digest,
//! against the value FreeSurfer 8.2.0's official program gives on the same inputs
//! (`validation/probes/fs-convert/golden.py`, Linux x64) with `reproduce`, and against the port's
//! own Linux x64 value with the default.

#[path = "mri_convert_golden/common.rs"]
mod common;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-21 by `golden.py` from the official program on Linux x64 (the port's
/// `reproduce` gave the same): six cases.
const OFFICIAL_SHA256: &str = "23e3bed1089a014580c8354ee2e47c312a15493fd1b8870525864efc0e51ec8d";
/// The port's default on Linux x64 (`cases_digest(Fix)`, recorded with the official value: no
/// defect of these cases changes a result, so it is the official value too).
const FIX_SHA256: &str = "23e3bed1089a014580c8354ee2e47c312a15493fd1b8870525864efc0e51ec8d";

fn cases_digest(mode: UpstreamDefects) -> String {
    let dir = tempfile::tempdir().expect("a folder");
    common::write_inputs(dir.path());
    let mut h = Sha256::new();
    for k in 0..common::CASES.len() {
        let (status, text) = common::run_case(dir.path(), k, mode);
        h.update(format!("case {k} {status}\n{text}\n"));
        let name = common::OUTPUTS[k];
        h.update(format!(
            "{name} {}\n",
            common::digest(&dir.path().join(name))
        ));
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

/// Prints both values (run on Linux x64 to record them: `cargo test --test mri_convert_golden
/// -- --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
