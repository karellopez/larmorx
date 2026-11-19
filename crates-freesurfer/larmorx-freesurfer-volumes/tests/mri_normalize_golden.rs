// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_normalize`'s golden cases on every platform: the program on 32^3 inputs made with integer
//! arithmetic (`mri_normalize_golden/common.rs`), the SHA-256 of each case's exit status, standard
//! output (the timing line's numbers masked) and output volumes' digests, against the value FreeSurfer
//! 8.2.0's official program gives on the same inputs (`validation/probes/fs-normalize/prog.py
//! --golden`, Linux x64, the unit oracle's `bin/mri_normalize`), in both modes (the outputs reach
//! no defect the default corrects).

#[path = "mri_normalize_golden/common.rs"]
mod common;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-19 by `prog.py --golden` from the official program on Linux x64 (the port gave
/// the same): three cases, 32^3 inputs; the tissue levels FreeSurfer read from below a histogram
/// (FS-597, memory contents) masked in the text.
const OFFICIAL_SHA256: &str = "c94b1d57f871763de5acd8cdb4995f56d2925b6879c1aaf62f302c882dc982c2";

fn cases_digest(mode: UpstreamDefects) -> String {
    let dir = tempfile::tempdir().expect("a folder");
    common::write_inputs(dir.path());
    let mut h = Sha256::new();
    for k in 0..common::CASES.len() {
        let (status, text) = common::run_case(dir.path(), k, mode);
        h.update(format!("case {k} {status}\n{text}\n"));
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
fn fix_gives_the_same_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Fix), OFFICIAL_SHA256);
}
