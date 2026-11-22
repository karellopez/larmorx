// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_add_xform_to_header` against FreeSurfer 8.2.0's official program on every platform: the
//! SHA-256 of the official program's lines (exit status, standard output and error, every file
//! and folder after the run; the case's folder as `<DIR>`) for the probe's cases of seed 1,
//! recorded by `validation/probes/fs-add-xform/check.py --seed 1 --n <N>` on Linux x64, against
//! the port's lines with `reproduce` (cases whose result in FreeSurfer is undefined, `check.py`'s
//! `SKIP`, hashed as `SKIP`).

#[path = "fs_xform/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 from the official program (`check.py --seed 1 --n 600`): 600 cases, 564
/// lines identical, 36 hashed as `SKIP`.
const N: usize = 600;
const OFFICIAL_SHA256: &str = "7ae8257416ec8d5007f88953cb3a86992caf9f6d620dcb9e283aa00ae6911eb2";
const SKIPPED: usize = 36;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 5] = ["FS-1154", "FS-1156", "FS-58", "FS-167", "FS-441"];

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, N) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!((skipped, hex.as_str()), (SKIPPED, OFFICIAL_SHA256));
}

/// The default differs from `reproduce` only where a defect it corrects is noted.
#[test]
fn fix_differs_only_with_a_notice() {
    for line in cases::generate(1, N) {
        let (r, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (f, ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(
            r == f || !ids.is_empty(),
            "{}",
            &line[line.len().saturating_sub(200)..]
        );
    }
}
