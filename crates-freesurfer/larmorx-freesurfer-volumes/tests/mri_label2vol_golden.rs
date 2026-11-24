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
//! vertices (`tests/fs_label2vol/cases.rs`' `golden_cases`). Each case's line (exit status, both
//! streams and every file the run wrote, masked as the probe masks them) is hashed; the official
//! program's value was recorded by `validation/probes/fs-label2vol/check.py --golden` on Linux
//! x64, and every platform must give it with `reproduce` and the recorded default.

#[path = "fs_label2vol/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-24 by `check.py --golden` from the official program on Linux x64: 9 cases.
const OFFICIAL_SHA256: &str = "aeabe265b849ca1002d79d2e07cc640fc12d97ca20468cc7dd9be3fe21d7e717";
/// The port's default on Linux x64 (FS-01's NUL bytes removed from the written volumes).
const FIX_SHA256: &str = "9cb289bfca40e71fb04cc2f6ecd364930c72794d207a8fd168347cccf27caa2c";

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
