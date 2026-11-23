// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_cc`'s golden cases on every platform: the program on subjects of 32^3 volumes made with
//! integer arithmetic (`mri_cc_golden/common.rs`), the SHA-256 of each case's exit status,
//! standard output (its time and peak memory left out), standard error (larmorx's notices left
//! out) and outputs' digests, against the value FreeSurfer 8.2.0's official program gives on the
//! same inputs (`validation/probes/fs-cc/prog.py --golden`, Linux x64) with `reproduce`, and
//! against the port's own Linux x64 value with the default.

#[path = "mri_cc_golden/common.rs"]
mod common;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-23 by `prog.py --golden` from the official program on Linux x64 (the port's
/// `reproduce` gave the same): twelve cases.
const OFFICIAL_SHA256: &str = "93da69667ef481b6687d4fc483aea5a6998497ae47c0f297f470fcd165e03993";
/// The port's default on Linux x64 (`cases_digest(Fix)`): the seed's voxel where the seed search
/// fails (FS-1210), errors where FreeSurfer crashes or aborts (FS-1205, FS-1211, FS-1212), no NUL
/// byte in the error lines (FS-01).
const FIX_SHA256: &str = "c817c0e13c00a567625bbea9e7d087d604958a26242b2d0b629a949349c34412";

fn cases_digest(mode: UpstreamDefects) -> String {
    let dir = tempfile::tempdir().expect("a folder");
    common::write_inputs(dir.path());
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

/// Prints both values (run on Linux x64 to record them: `cargo test --test mri_cc_golden --
/// --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
