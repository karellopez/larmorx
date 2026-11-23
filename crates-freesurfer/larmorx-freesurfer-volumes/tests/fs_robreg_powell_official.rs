// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_register`'s Powell registration against FreeSurfer 8.2.0's official program on
//! every platform: the SHA-256 of the official program's lines (exit status, standard output and
//! error, every file written; the case's folder as `<DIR>`, the time taken masked) for the probe's
//! Powell family (`cases::powell_cases`), recorded by `validation/probes/fs-robust-register/check.py
//! --family powell --preload <decided_complex.sh's library>` on Linux x64,
//! against the port's lines with `reproduce` (cases whose result in FreeSurfer is undefined,
//! `check.py`'s `SKIP`, hashed as `SKIP`; the paths the port does not run yet, `NOTPORTED`).

#[path = "fs_robreg/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-23 (relay 46) from the official program with the decided `cabs` and `csqrt`
/// preloaded (the user's D4 decision, which the port's matrix square roots follow; without it 4
/// of the 40 cases differ in their last digits through glibc's `hypot`): 40 cases, 36 lines
/// identical, 4 hashed as `SKIP` (FS-1063: `--transonly --iscale`, a transformation set from
/// memory contents).
const OFFICIAL_SHA256: &str = "083c04fdc3fe62beecad8723863434501872cf33825c8b1aac270e676ac4d20b";
const SKIPPED: usize = 4;
const NOT_PORTED: usize = 0;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 24] = [
    "FS-1070", "FS-1071", "FS-1072", "FS-1063", "FS-1067", "FS-1068", "FS-1069", "FS-1075",
    "FS-1077", "FS-167", "FS-58", "FS-288", "FS-290", "FS-294", "FS-441", "FS-302", "FS-303",
    "FS-304", "FS-305", "FS-306", "FS-307", "FS-308", "FS-309", "FS-310",
];

/// Both modes of every case, on four threads (the cases are independent; the lines are joined in
/// order).
fn both_modes() -> Vec<(String, Vec<&'static str>, String, Vec<&'static str>)> {
    let cases = cases::powell_cases();
    let chunks: Vec<&[String]> = cases.chunks(cases.len().div_ceil(4)).collect();
    std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                s.spawn(move || {
                    chunk
                        .iter()
                        .map(|line| {
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
#[cfg_attr(
    debug_assertions,
    ignore = "about 7 minutes in a debug build (an LNCC case of 1,718 iterations): run with --release, or --ignored"
)]
fn powell_reproduce_gives_the_official_outputs_and_fix_differs_only_with_a_notice() {
    let mut h = Sha256::new();
    let (mut skipped, mut not_ported) = (0, 0);
    for (out, ids, f, fids) in both_modes() {
        if out.contains("not ported yet") {
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
    assert_eq!(
        (skipped, not_ported, hex.as_str()),
        (SKIPPED, NOT_PORTED, OFFICIAL_SHA256)
    );
}
