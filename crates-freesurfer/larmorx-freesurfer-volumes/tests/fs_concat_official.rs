// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_concat` against FreeSurfer 8.2.0's official program on every platform: the SHA-256 of
//! the official program's lines (exit status, standard output and error, every file the run wrote
//! or changed and every folder it made; the case's folder as `<DIR>`, `--debug`'s and `--rusage`'s
//! resource usage masked) for the probe's cases of seed 1, recorded by
//! `validation/probes/fs-concat/check.py --seed 1 --n <N>` on Linux x64, against the port's lines
//! with `reproduce` (cases whose result in FreeSurfer is undefined: a crash or memory contents,
//! `check.py`'s `SKIP`, hashed as `SKIP`).

#[path = "fs_concat/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-23 from the official program (`check.py --seed 1 --n 600`, relay 55): 600
/// cases, 569 lines identical, 31 hashed as `SKIP`.
const N: usize = 600;
const OFFICIAL_SHA256: &str = "87569463d95ee48ca81bcc39c541160113f1aa16890b66daf94d8875cbe34560";
const SKIPPED: usize = 31;

/// `check.py`'s `SKIP` (the port's pseudo-notices).
const SKIP: [&str; 2] = ["CRASH", "MEMORY"];

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
