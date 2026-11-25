// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_coreg` against FreeSurfer 8.2.0's official program: the fs-coreg probe's prelude
//! (`fs_coreg/cases.rs`' `prelude`: the command line's paths and failures, the stand-alone options,
//! every registration option on the base pair, a subject's folder and FreeSurfer's environment),
//! each case's line with `reproduce` hashed in order, the cases where FreeSurfer's result is
//! undefined and the one not ported as `SKIP`, against the hash `validation/probes/fs-coreg/check.py
//! --seed 1 --n 0` recorded from the official program (`official_sha256_skip`). A debug build
//! compares the first 66 cases (the command line and the stand-alone options) and hashes the
//! registrations as `SLOW` (`official_sha256_skip_debug`); the default (`fix`) may differ only with
//! a notice.

#[path = "fs_coreg/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-25 (relay 52, from the clean commit 32e612e0) by `check.py --seed 1 --n 0`
/// (the official program, Linux x64, one thread): 125 cases, 117 identical, 8 undefined.
const OFFICIAL_SHA256_SKIP: &str =
    "d21a4202a0023562f3853f666b251b91cbd53c7b61e1a18c715013cfe72df793";
/// The same with the registrations as `SLOW` (a debug build's).
const OFFICIAL_SHA256_SKIP_DEBUG: &str =
    "7ba19f2cdca1bed5b4bbe5eed1da769359445acba4a4ece62d37741eac28790e";
/// The cases a debug build runs.
const DEBUG_CASES: usize = 66;

/// FreeSurfer's result is undefined (`check.py`'s `SKIP`): a case with one of these notices, but
/// FS-1323's division by 0, which `reproduce` gives as FreeSurfer does (`CRASH 8`).
const SKIP: &[&str] = &[
    "FS-1313", "FS-1314", "FS-1316", "FS-1318", "FS-1319", "FS-1320", "FS-1321", "FS-1322",
    "FS-1323", "FS-1326", "FS-167", "FS-58", "FS-288", "FS-290", "FS-294", "FS-441", "FS-302",
    "FS-303", "FS-304", "FS-305", "FS-306", "FS-307", "FS-308", "FS-309", "FS-310",
];

fn skipped(line: &str, ids: &[&str]) -> bool {
    line.contains("is not ported yet")
        || ids
            .iter()
            .any(|x| SKIP.contains(x) && !(*x == "FS-1323" && line.starts_with("CRASH 8")))
}

/// A case's `reproduce` line and notices, its `fix` line and notices.
type Row = (String, Vec<&'static str>, String, Vec<&'static str>);

/// The lines (`limit`: the first ones, the rest `SLOW`) side by side, hashed in order; and the
/// cases where the default differs from `reproduce` without a notice.
fn digest(limit: usize) -> (String, Vec<usize>) {
    let lines = cases::generate(1, 0);
    let rows: Vec<Option<Row>> = std::thread::scope(|sc| {
        let hs: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(k, line)| {
                sc.spawn(move || {
                    (k < limit).then(|| {
                        let (r, ids) = cases::run_line(line, UpstreamDefects::Reproduce);
                        let (f, fids) = cases::run_line(line, UpstreamDefects::Fix);
                        (r, ids, f, fids)
                    })
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("a case")).collect()
    });
    let mut h = Sha256::new();
    let mut bad = Vec::new();
    for (k, row) in rows.into_iter().enumerate() {
        let Some((r, ids, f, fids)) = row else {
            h.update(b"SLOW\n");
            continue;
        };
        if skipped(&r, &ids) {
            h.update(b"SKIP\n");
            continue;
        }
        h.update(r.as_bytes());
        h.update(b"\n");
        if f != r && fids.is_empty() {
            bad.push(k);
        }
    }
    (
        h.finalize().iter().map(|b| format!("{b:02x}")).collect(),
        bad,
    )
}

#[test]
fn reproduce_gives_the_official_outputs_and_fix_differs_only_with_a_notice() {
    let (limit, want) = if cfg!(debug_assertions) {
        (DEBUG_CASES, OFFICIAL_SHA256_SKIP_DEBUG)
    } else {
        (usize::MAX, OFFICIAL_SHA256_SKIP)
    };
    let (got, bad) = digest(limit);
    assert!(
        bad.is_empty(),
        "the default differs without a notice: {bad:?}"
    );
    assert_eq!(got, want);
}
