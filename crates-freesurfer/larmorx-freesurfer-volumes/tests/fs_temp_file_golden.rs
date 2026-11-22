// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `fs_temp_file`'s golden cases on every platform (`fs_tempfile/cases.rs`'s `golden_cases`: the
//! scripts' two calls, without an option and with `--suffix .txt`, a base folder with a suffix, a
//! variable whose folder does not exist, the help): the SHA-256 of each case's line (exit status,
//! standard output and error, every file after the run; the name's six random letters as
//! `XXXXXX`), against the value FreeSurfer 8.2.0's official program gives
//! (`validation/probes/fs-temp-file/check.py --golden`, Linux x64) with `reproduce` and with the
//! default (no golden case meets a defect); without Unix's modes elsewhere.

#[path = "fs_tempfile/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 by `check.py --golden` from the official program on Linux x64: 5 cases.
const OFFICIAL_SHA256: &str = "86cabb9413e2e4ea4a6e8e71d0b3deb60d765c146f3094b09e755c91f7d15930";
const OFFICIAL_SHA256_NO_MODES: &str =
    "82aae397170e37b780c2291927bd50c8cb0acdbdcfa9e549b499fbfe4fc5a8d2";

fn cases_digest(mode: UpstreamDefects) -> String {
    let mut h = Sha256::new();
    for line in cases::golden_cases() {
        let (out, _) = cases::run_line(&line, mode);
        h.update(out.as_bytes());
        h.update(b"\n");
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn want() -> &'static str {
    if cfg!(unix) {
        OFFICIAL_SHA256
    } else {
        OFFICIAL_SHA256_NO_MODES
    }
}

#[test]
fn reproduce_gives_the_official_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Reproduce), want());
}

#[test]
fn fix_gives_the_official_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Fix), want());
}
