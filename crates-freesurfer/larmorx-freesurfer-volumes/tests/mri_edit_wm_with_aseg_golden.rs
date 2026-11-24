// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_edit_wm_with_aseg`'s golden cases on every platform: the program on 32^3 volumes made with
//! integer arithmetic (`mri_edit_wm_with_aseg_golden/common.rs`; one of them `float` with NaNs and
//! infinities), the SHA-256 of each case's exit status, standard output, standard error (its time
//! and larmorx's notices left out) and outputs' digests, against the value FreeSurfer 8.2.0's
//! official program gives on the same inputs (`validation/probes/fs-editwm/prog.py --golden`,
//! Linux x64) with `reproduce`, and against the port's own Linux x64 value with the default.

#[path = "mri_edit_wm_with_aseg_golden/common.rs"]
mod common;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-24 by `prog.py --golden` from the official program on Linux x64 (the port's
/// `reproduce` gave the same): nine cases.
const OFFICIAL_SHA256: &str = "edc6586833df4f36563a735d576040f1f25b041a4c3132b2cc5cd9a5130d56ef";
/// The port's default on Linux x64 (`cases_digest(Fix)`): `-keep-hailvcp`'s values taken (FS-1238),
/// no crash for an option without its value (FS-1239), status 1 for the unwritten output (FS-1240).
const FIX_SHA256: &str = "2731d00eaae2120fd385068587d8a6d7c01f98bafe63d61face3b2fd73e8d130";

fn cases_digest(mode: UpstreamDefects) -> String {
    let dir = tempfile::tempdir().expect("a folder");
    let mut h = Sha256::new();
    for k in 0..common::cases().len() {
        let (status, text) = common::run_case(dir.path(), k, mode);
        // as prog.py --golden takes the official's text: without its last new lines
        h.update(format!(
            "case {k} {status}\n{}\n",
            text.trim_end_matches('\n')
        ));
        for name in common::OUTPUTS[k] {
            h.update(format!(
                "{name} {}\n",
                common::digest(&dir.path().join(name))
            ));
        }
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

/// Prints both values (run on Linux x64 to record them: `cargo test --test mri_edit_wm_with_aseg_golden --
/// --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
