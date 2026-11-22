// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `fs_temp_file` against FreeSurfer 8.2.0's official program on every platform: the SHA-256 of
//! the official program's lines (exit status, standard output and error, every file and folder
//! after the run, a file made in `/tmp`; the case's folder as `<DIR>`, the name's six random
//! letters as `XXXXXX`) for every case of the probe, recorded by
//! `validation/probes/fs-temp-file/check.py` on Linux x64, against the port's lines with
//! `reproduce`: Unix's lines with the modes, and elsewhere the lines without modes, the cases only
//! Unix gives the same way (`/tmp`, read-only folders, links) hashed as `UNIX-ONLY`. An account
//! that can write into read-only folders (an administrator's) skips the test.

#[path = "fs_tempfile/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 from the official program (`check.py`): 73 cases, 8 only Unix's.
const N: usize = 73;
const OFFICIAL_SHA256: &str = "9b48c19f7925c9e8759e1213076f4a3528b04c96a91b9d37f730f1c668ea86b9";
const OFFICIAL_SHA256_NO_MODES: &str =
    "c6a52f31e869154fe8547db1c92031e04b835c080808653878105d5e01f2884f";

fn digest(mode: UpstreamDefects) -> (usize, String) {
    let mut h = Sha256::new();
    let lines = cases::all_cases();
    for line in &lines {
        if !cfg!(unix) && cases::unix_only(line) {
            h.update(b"UNIX-ONLY\n");
            continue;
        }
        let (out, _) = cases::run_line(line, mode);
        h.update(out.as_bytes());
        h.update(b"\n");
    }
    (
        lines.len(),
        h.finalize().iter().map(|b| format!("{b:02x}")).collect(),
    )
}

#[test]
fn reproduce_gives_the_official_outputs() {
    if cfg!(unix) && cases::writes_read_only_folders() {
        eprintln!("skipped: this account writes into read-only folders");
        return;
    }
    let want = if cfg!(unix) {
        OFFICIAL_SHA256
    } else {
        OFFICIAL_SHA256_NO_MODES
    };
    let (n, got) = digest(UpstreamDefects::Reproduce);
    assert_eq!((n, got.as_str()), (N, want));
}

/// The default differs from `reproduce` only where a defect it corrects is noted.
#[test]
fn fix_differs_only_with_a_notice() {
    for line in cases::all_cases() {
        if !cfg!(unix) && cases::unix_only(&line) {
            continue;
        }
        let (r, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (f, ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(r == f || !ids.is_empty(), "{line}");
    }
}
