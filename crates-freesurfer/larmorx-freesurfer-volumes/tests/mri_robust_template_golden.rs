// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_template`'s golden cases on every platform: the program on 32^3 images made with
//! integer arithmetic (`fs_robtmpl/cases.rs`' `golden_cases`: fMRIPrep's call on three inputs,
//! recon-all's motion correction on two, the rigid mean space of two, one global iteration of three
//! with weights, an input with NaN voxels built from their bits, and a 4-D input's one `--mapmov`
//! image), the
//! SHA-256 of each case's line (exit status, standard output and error, every file written; the time
//! taken masked), against the value FreeSurfer 8.2.0's official program gives on the same inputs
//! (`validation/probes/fs-robust-template/check.py --golden --preload`, Linux x64, the decided `cabs`
//! and `csqrt`) with `reproduce`, and against the port's own Linux x64 value with the default.

#[path = "fs_robtmpl/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-24 by `check.py --golden --preload` from the official program on Linux x64: 6
/// cases.
const OFFICIAL_SHA256: &str = "5ee82ce663af88dc34df36c4cea69e551aae036f8637e7f6b172daf61873fda0";
/// The port's default on Linux x64 (FS-1090: the 4-D `--mapmov` with the template's header).
const FIX_SHA256: &str = "cfe17db9203387f230880cae98bea9fc4a96dd152d09e2de96ff91c46dc14665";

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
fn reproduce_gives_the_official_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Reproduce), OFFICIAL_SHA256);
}

#[test]
fn fix_gives_the_recorded_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Fix), FIX_SHA256);
}

/// Prints the values (run on Linux x64 to record them: `cargo test --test
/// mri_robust_template_golden -- --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
