// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_segment`'s golden cases on every platform: the program on 32^3 volumes made with integer
//! arithmetic (`mri_segment_golden/common.rs`; one of them `float` with NaNs and infinities), the
//! SHA-256 of each case's exit status, standard output (its time left out), standard error
//! (larmorx's notices left out) and outputs' digests, against the value FreeSurfer 8.2.0's
//! official program gives on the same inputs (`validation/probes/fs-segment/prog.py --golden`,
//! Linux x64) with `reproduce`, and against the port's own Linux x64 value with the default.

#[path = "mri_segment_golden/common.rs"]
mod common;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-24 by `prog.py --golden` from the official program on Linux x64 (the port's
/// `reproduce` gave the same): eleven cases.
const OFFICIAL_SHA256: &str = "d4ca21c0af962e596f0b091da61a58fa0b866ac78327b361ced6e8d3b139b34c";
/// The port's default on Linux x64 (`cases_digest(Fix)`): `expf` correctly rounded (GLIBC-07),
/// `-b`'s value (FS-1220), status 1 for the unwritten output (FS-1226), no crash without
/// ventricles (FS-1227) or for an option without its value (FS-1219).
const FIX_SHA256: &str = "735f6cf680c9bb93d3c0fb7c50c2b9e19c3722cb827add8e8d116017254665a3";

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

/// Prints both values (run on Linux x64 to record them: `cargo test --test mri_segment_golden --
/// --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
