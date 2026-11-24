// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_register --debug` against FreeSurfer 8.2.0's official program on every platform: the
//! SHA-256 of the official program's lines (exit status, standard output and error, every file
//! written, the debug output's intermediate images among them; the case's folder as `<DIR>`, the
//! time taken masked) for the probe's debug family (`cases::debug_cases`), recorded by
//! `validation/probes/fs-robust-register/check.py --family debug --preload <decided_complex.sh's
//! library>` on Linux x64 (the official program's standard input `/dev/null`, the port's an empty
//! one), against the port's lines with `reproduce` (cases whose result in FreeSurfer is undefined,
//! `check.py`'s `SKIP`, hashed as `SKIP`).

#[path = "fs_robreg/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-24 (relay 49) from the official program with the decided `cabs` and `csqrt`
/// preloaded (the user's D4 decision): 48 cases, 44 lines identical, 4 hashed as `SKIP` (FS-1063,
/// FS-1067, and two of FS-1075 with FS-1079).
const OFFICIAL_SHA256: &str = "85b2ae762dd06f9171fbb638808129b8c77280cd8a58ce32f7fbf6e1feb1bd2e";
/// In a debug build (CI's) the nine cases of a joint histogram's cost or SB are hashed as `SLOW`,
/// not run (`cases::histogram_or_sb_cost`; `check.py`'s `official_sha256_skip_debug`).
const OFFICIAL_SHA256_DEBUG: &str =
    "f6630330d951412c9178ee99459f7b8a4fd0f5a0c3996205354e610a1f393123";
const SLOW_IN_DEBUG: usize = 9;
const SKIPPED: usize = 4;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 24] = [
    "FS-1070", "FS-1071", "FS-1072", "FS-1063", "FS-1067", "FS-1068", "FS-1069", "FS-1075",
    "FS-1077", "FS-167", "FS-58", "FS-288", "FS-290", "FS-294", "FS-441", "FS-302", "FS-303",
    "FS-304", "FS-305", "FS-306", "FS-307", "FS-308", "FS-309", "FS-310",
];

/// Both modes of every case, on four threads (the cases are independent; the lines are joined in
/// order).
fn both_modes() -> Vec<(String, Vec<&'static str>, String, Vec<&'static str>)> {
    let cases = cases::debug_cases();
    let chunks: Vec<&[String]> = cases.chunks(cases.len().div_ceil(4)).collect();
    std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                s.spawn(move || {
                    chunk
                        .iter()
                        .map(|line| {
                            if cfg!(debug_assertions) && cases::histogram_or_sb_cost(line) {
                                return ("SLOW".to_owned(), vec![], "SLOW".to_owned(), vec![]);
                            }
                            let (r, rids) = cases::run_line(line, UpstreamDefects::Reproduce);
                            let (f, fids) = cases::run_line(line, UpstreamDefects::Fix);
                            (r, rids, f, fids)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a chunk of cases"))
            .collect()
    })
}

/// `reproduce` gives the official lines; the default differs from `reproduce` only where a defect
/// it corrects is noted.
#[test]
fn debug_output_reproduce_gives_the_official_outputs_and_fix_differs_only_with_a_notice() {
    let mut h = Sha256::new();
    let (mut skipped, mut slow) = (0, 0);
    for (out, ids, f, fids) in both_modes() {
        assert!(
            !out.contains("not ported yet"),
            "{}",
            &out[..out.len().min(300)]
        );
        if out == "SLOW" {
            slow += 1;
            h.update(b"SLOW\n");
        } else if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
        assert!(
            out == f || !fids.is_empty(),
            "{}",
            &out[..out.len().min(300)]
        );
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    let (want_slow, want) = if cfg!(debug_assertions) {
        (SLOW_IN_DEBUG, OFFICIAL_SHA256_DEBUG)
    } else {
        (0, OFFICIAL_SHA256)
    };
    assert_eq!((slow, hex.as_str()), (want_slow, want));
    if !cfg!(debug_assertions) {
        assert_eq!(skipped, SKIPPED);
    }
}
