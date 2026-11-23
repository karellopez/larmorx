// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_register` against FreeSurfer 8.2.0's official program on every platform: the
//! SHA-256 of the official program's lines (exit status, standard output and error, every file
//! written; the case's folder as `<DIR>`, the time taken masked) for the probe's cases of seed 1,
//! recorded by `validation/probes/fs-robust-register/check.py --seed 1 --n <N>` on Linux x64,
//! against the port's lines with `reproduce` (cases whose result in FreeSurfer is undefined,
//! `check.py`'s `SKIP`, hashed as `SKIP`; the paths the port does not run yet, `NOTPORTED`).

#[path = "fs_robreg/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 from the official program (`check.py --seed 1 --n 120`); re-recorded
/// 2026-11-23 (relay 45: the entropy images ported, the 17 entropy cases after the prelude); and
/// again 2026-11-23 (relay 46: the Powell registration's `LS`, `NCC`, `TB`, `LNCC` and `SAD`
/// ported) from the official program with the decided `cabs` and `csqrt` preloaded (`check.py
/// --preload`, `decided_complex.sh`: the user's D4 decision, which the port's matrix square roots
/// follow); and again 2026-11-23 (relay 47: `MI`, `NMI`, `ECC`, `SCR` and `SB` ported, the same
/// preload). 120 cases, 110 lines identical, 8 hashed as `SKIP`, 2 as `NOTPORTED` (`--debug` and
/// `--test`). The Powell families: `fs_robreg_powell_official.rs`, `fs_robreg_histo_official.rs`.
const N: usize = 120;
const OFFICIAL_SHA256: &str = "70ea2e644455db29d5a7e2b5332748ad3bb816bc9d8add6410f59e819678a0d2";
/// In a debug build (CI's) the five cases of a joint histogram's cost or SB are hashed as `SLOW`,
/// not run (about 270 s at one thread in a debug build; `cases::histogram_or_sb_cost`): the same
/// official lines with those five so (`check.py`'s `official_sha256_skip_debug`). A release build
/// compares them too, and `fs_robreg_histo_official.rs` holds 32 more.
const OFFICIAL_SHA256_DEBUG: &str =
    "f9f10b84d937b70438d4c4d5cde4247a5cb6cd98e4ee0c4d03b54766e34bfb4c";
const SLOW_IN_DEBUG: usize = 5;
const SKIPPED: usize = 8;
const NOT_PORTED: usize = 2;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 22] = [
    "FS-1070", "FS-1071", "FS-1072", "FS-1063", "FS-1067", "FS-1068", "FS-1069", "FS-167", "FS-58",
    "FS-288", "FS-290", "FS-294", "FS-441", "FS-302", "FS-303", "FS-304", "FS-305", "FS-306",
    "FS-307", "FS-308", "FS-309", "FS-310",
];

/// Both modes of every case, on four threads (the cases are independent; the lines are joined in
/// order).
fn both_modes() -> Vec<(String, Vec<&'static str>, String, Vec<&'static str>)> {
    let cases = cases::generate(1, N);
    let chunks: Vec<&[String]> = cases.chunks(N.div_ceil(4)).collect();
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
fn reproduce_gives_the_official_outputs_and_fix_differs_only_with_a_notice() {
    let mut h = Sha256::new();
    let (mut skipped, mut not_ported, mut slow) = (0, 0, 0);
    for (out, ids, f, fids) in both_modes() {
        if out == "SLOW" {
            slow += 1;
            h.update(b"SLOW\n");
        } else if out.contains("not ported yet") {
            not_ported += 1;
            h.update(b"NOTPORTED\n");
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
    assert_eq!(
        (skipped, not_ported, slow, hex.as_str()),
        (SKIPPED, NOT_PORTED, want_slow, want)
    );
}
