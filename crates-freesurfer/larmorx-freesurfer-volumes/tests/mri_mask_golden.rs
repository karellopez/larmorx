// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_mask`'s golden cases on every platform: the program on 32^3 inputs made with integer
//! arithmetic (`mri_mask_golden/common.rs`), the SHA-256 of each case's exit status, standard
//! output, standard error (larmorx's notices left out) and outputs' digests, against the value
//! FreeSurfer 8.2.0's official program gives on the same inputs
//! (`validation/probes/fs-mask/prog.py --golden`, Linux x64) with `reproduce`, and against the
//! port's own Linux x64 value with the default.

#[path = "mri_mask_golden/common.rs"]
mod common;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 by `prog.py --golden` from the official program on Linux x64 (the port's
/// `reproduce` gave the same): thirteen cases, 32^3 volumes and a 16 x 16 x 32 pair.
const OFFICIAL_SHA256: &str = "f3af007dd90c7e0cc0984075bc516ba8c0f1bd83bd3c7ad64c1116ec62389eb7";
/// The port's default on Linux x64 (`cases_digest(Fix)`): no NUL byte in the error lines
/// (FS-01), an error for `-BBEQ` without a box (FS-1133).
const FIX_SHA256: &str = "5606171d737750ae73cc1fb9a7781b8e1e2bc67409408f99c7b1798e793846aa";

fn cases_digest(mode: UpstreamDefects) -> String {
    let dir = tempfile::tempdir().expect("a folder");
    common::write_inputs(dir.path());
    let mut h = Sha256::new();
    for k in 0..common::CASES.len() {
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

/// Prints both values (run on Linux x64 to record them: `cargo test --test mri_mask_golden --
/// --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
