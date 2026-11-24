// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The golden cases of `mri_label2vol` (rule 10 of larmorx's CLAUDE.md): `defect2seg`'s two calls
//! (the left hemisphere's defects painted into a template with a cortex label, then merged into an
//! `int` template with its own colour table), defect numbers with NaN and infinities (from their
//! bits), `seg2cc`'s segmentation mapped onto another grid by the headers (also upsampled, with
//! its hits), two volume labels with their hits, a label with NaN, infinite and huge coordinates
//! with its stat volume through a `register.dat`, a surface label projected along the normals by
//! a fraction of the thickness, and an annotation, on volumes up to 12^3 and a surface of 66
//! vertices; `--new-aseg2vol` with no sample per voxel (FS-1266), with the tissue types' vote and
//! through a `register.dat`; `--fill-ribbon` with a label of NaN and infinite stats (from the
//! label's text) and its stat volume, through an `.lta` whose destination is the ribbon, and with
//! an annotation (`tests/fs_label2vol/cases.rs`' `golden_cases`). Each case's line (exit status, both
//! streams and every file the run wrote, masked as the probe masks them) is hashed; the official
//! program's value was recorded by `validation/probes/fs-label2vol/check.py --golden` on Linux
//! x64, and every platform must give it with `reproduce` and the recorded default.

#[path = "fs_label2vol/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-24 by `check.py --golden` from the official program on Linux x64: 15 cases
/// (relay 57: part 2's six).
const OFFICIAL_SHA256: &str = "f50d82be5500fd538efeaa58f05453303144525721334ce59d38d3afe5c0b139";
/// The port's default on Linux x64 (FS-01's NUL bytes removed from the written volumes).
const FIX_SHA256: &str = "17475818fc453f38cbb8061dba1f5e8260aaf3544a1da5844d9c0b008b101953";

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

/// Prints both values (run on Linux x64 to record them: `cargo test --test mri_label2vol_golden --
/// --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
