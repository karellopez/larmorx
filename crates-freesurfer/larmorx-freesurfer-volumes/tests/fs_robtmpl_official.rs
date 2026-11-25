// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_template` against FreeSurfer 8.2.0's official program on every platform: the SHA-256
//! of the official program's lines (exit status, standard output and error, every file written; the
//! case's folder as `<DIR>`, the time taken and `-all-info`'s run-time fields masked) for the probe's
//! cases of seed 1, recorded by `validation/probes/fs-robust-template/check.py --seed 1 --n <N>
//! --preload <decided.sh's library> --pade <rebuild.sh's>` on Linux x64 (the decided `acos`, `cabs`,
//! `csqrt` and `clog`), against
//! the port's lines with `reproduce` (cases whose result in FreeSurfer is undefined, `check.py`'s
//! `SKIP`, hashed as `SKIP`; the paths the port does not run yet, `NOTPORTED`).

#[path = "fs_robtmpl/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-24 (track C relay 51) from the official program with the decided functions
/// preloaded (`--pade`: the affine mean space's case from the official sources with the matrix
/// exponential's numbers defined, FS-1311): 150 cases, 144 lines identical, 6 hashed as `SKIP`.
const N: usize = 150;
const OFFICIAL_SHA256: &str = "08addb5daf59426a3bd3943d898466e7afd54a8bcbaf71899f525574563f67c4";
const SKIPPED: usize = 6;
const NOT_PORTED: usize = 0;
/// In a debug build (CI's) only the first `DEBUG_CASES` cases run (the command line's own paths and
/// failures: no registration, which takes seconds each in a debug build), the rest hashed as `SLOW`
/// (`check.py`'s `official_sha256_skip_debug`, `--debug-cases 45`); the golden cases
/// (`mri_robust_template_golden.rs`) hold seven templates on every platform.
const DEBUG_CASES: usize = 45;
const OFFICIAL_SHA256_DEBUG: &str =
    "a162a70211463e072523c90bfcd8ac1319c82abc650712fa2abe83de9f9ad3f2";
const SKIPPED_DEBUG: usize = 2;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 26] = [
    "FS-1065", "FS-1083", "FS-1084", "FS-1088", "FS-1092", "FS-1094", "FS-1095", "FS-1063",
    "FS-1067", "FS-1068", "FS-1069", "FS-167", "FS-58", "FS-288", "FS-290", "FS-294", "FS-441",
    "FS-302", "FS-303", "FS-304", "FS-305", "FS-306", "FS-307", "FS-308", "FS-309", "FS-310",
];

/// Both modes of every case, on four threads (the cases are independent; the lines are joined in
/// order).
fn both_modes() -> Vec<(String, Vec<&'static str>, String, Vec<&'static str>)> {
    let cases: Vec<(usize, String)> = cases::generate(1, N).into_iter().enumerate().collect();
    let chunks: Vec<&[(usize, String)]> = cases.chunks(N.div_ceil(4)).collect();
    std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                s.spawn(move || {
                    chunk
                        .iter()
                        .map(|(k, line)| {
                            if cfg!(debug_assertions) && *k >= DEBUG_CASES {
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
    let (mut skipped, mut not_ported) = (0, 0);
    for (out, ids, f, fids) in both_modes() {
        if out == "SLOW" {
            h.update(b"SLOW\n");
        } else if out.contains("is not ported yet") {
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
    let want = if cfg!(debug_assertions) {
        (SKIPPED_DEBUG, 0, OFFICIAL_SHA256_DEBUG)
    } else {
        (SKIPPED, NOT_PORTED, OFFICIAL_SHA256)
    };
    assert_eq!((skipped, not_ported, hex.as_str()), want);
}
