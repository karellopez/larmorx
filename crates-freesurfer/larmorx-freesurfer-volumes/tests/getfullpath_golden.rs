// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `getfullpath`'s golden cases on every platform (`fs_getfullpath/cases.rs`'s `golden_cases`:
//! seg2cc's call with an absolute name, a relative name, a name in the folder itself, a folder that
//! does not exist, no argument): the SHA-256 of each case's line (exit status, standard output and
//! error; the case's folder as `<DIR>`), against the value FreeSurfer 8.2.0's official script
//! gives (`validation/probes/fs-getfullpath/check.py --golden`, Linux x64), with the default on
//! every platform and with `reproduce` on Unix (elsewhere tcsh's reading of the folder's
//! backslashes changes it).

#[path = "fs_getfullpath/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 by `check.py --golden` from the official script on Linux x64: 5 cases.
const OFFICIAL_SHA256: &str = "59ce37864da8f17cd6bfbaa1d9a0e64fbb46398c480c1d71f4bcbf9352a7bec0";

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
fn fix_gives_the_official_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Fix), OFFICIAL_SHA256);
}

#[test]
fn reproduce_gives_the_official_outputs_on_unix() {
    if cfg!(unix) {
        assert_eq!(cases_digest(UpstreamDefects::Reproduce), OFFICIAL_SHA256);
    }
}
