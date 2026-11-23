// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_vol2vol` against FreeSurfer 8.2.0's official program on every platform: the SHA-256 of
//! the official program's lines (exit status, standard output and error, every file the run wrote
//! or changed and every folder it made; the case's folder as `<DIR>`, the noise seed, `VmPeak`
//! and the transforms' `# created by` line masked) for the probe's cases of seed 1, recorded by
//! `validation/probes/fs-vol2vol/check.py --seed 1 --n <N>` on Linux x64, against the port's
//! lines with `reproduce` (cases whose result in FreeSurfer is undefined or whose path relay 53
//! leaves for later, `check.py`'s `SKIP`, hashed as `SKIP`).

#[path = "fs_vol2vol/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-23 from the official program (`check.py --seed 1 --n 600`): 600 cases, 587
/// lines identical, 13 hashed as `SKIP`.
const N: usize = 600;
const OFFICIAL_SHA256: &str = "27d880534dc80af57c524bf565bbc1c2bf19a12a695b4a66c3c80977d0926407";
const SKIPPED: usize = 13;

/// `check.py`'s `SKIP` (the port's pseudo-notices).
const SKIP: [&str; 2] = ["CRASH", "NOTPORTED"];

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
