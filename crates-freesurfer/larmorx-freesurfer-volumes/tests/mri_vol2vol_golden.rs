// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The golden cases of `mri_vol2vol` (rule 10 of larmorx's CLAUDE.md): the v8 scripts' four forms
//! (`--regheader` onto a target, `--reg <lta>` onto the transform's destination, a segmentation
//! by nearest neighbour, `--lta` with a target), fMRIPrep's (`--lta` and `--interp nearest` of a
//! segmentation into a NIfTI), the voxel-to-RAS rewrite with `--save-reg` from a register.dat
//! (FS-1183: the official program stops, the default applies it), and a moving volume with NaN
//! voxels by trilinear and cubic B-spline resampling, on volumes up to 32^3
//! (`tests/fs_vol2vol/cases.rs`' `golden_cases`). Each case's line (exit status, both streams and
//! every file the run wrote, masked as the probe masks them) is hashed; the official program's
//! value was recorded by `validation/probes/fs-vol2vol/check.py --golden` on Linux x64, and every
//! platform must give it with `reproduce` and the recorded default.

#[path = "fs_vol2vol/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-23 by `check.py --golden` from the official program on Linux x64: 8 cases.
const OFFICIAL_SHA256: &str = "be32774dd791db4700732abbcb4b8342d7299d59ef1a4e9b18b376be9221b49d";
/// The port's default on Linux x64 (FS-1183 applied, FS-01's NUL bytes removed, the noise seed
/// fixed: FS-29).
const FIX_SHA256: &str = "c84fa0a21ea4aed240c6ce886a0988d75a779f21d91d2e505f0e96f3ee35318d";

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

/// Prints both values (run on Linux x64 to record them: `cargo test --test mri_vol2vol_golden --
/// --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
