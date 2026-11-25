// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `seg2cc`, `mergeseg` and `fscalc` against FreeSurfer 8.2.0's official scripts: the SHA-256 of
//! the official scripts' canonical texts (`fs_scripts/cases.rs`: status, both streams, every file
//! the case's folder then holds, what varies from run to run masked) for every case but the heavy
//! ones, recorded by `validation/probes/fs-scripts/check.py` on Linux x64 (`/bin/tcsh` 6.21.00, GNU
//! coreutils 8.32, the C locale, `TZ=UTC`), against the port's texts with `reproduce` on Unix (a
//! case holding a form the port does not reproduce hashed as `UNMODELLED`); the default's texts
//! against their Linux x64 value everywhere, and differing from `reproduce` only with a notice.

#[path = "fs_scripts/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-25 by `check.py` from the official scripts (relay 59: the unreadable volumes a
/// 13-byte text again): every case but the heavy ones (`full`), and the quick ones a debug build
/// runs; their counts and those not reproduced.
const OFFICIAL_SHA256: &str = "ff0eabd6989763aaa9c6c3e06212adbec7fde91fa136fa6be27d4b031b51e493";
const N: usize = 147;
const UNMODELLED: usize = 8;
const OFFICIAL_SHA256_QUICK: &str =
    "0dcdac2c37fd966dcf806d6778ef18b0f2545d74cab27f0566cb4e6d4d801a10";
const N_QUICK: usize = 64;
const UNMODELLED_QUICK: usize = 0;
/// The default's texts on Linux x64 (`check.py`'s `fix_sha256`, `fix_sha256_quick`).
const FIX_SHA256: &str = "9054423f6a2112998a8cb3598e8ad70a3bc50a60403fe235a21be6e98e915640";
const FIX_SHA256_QUICK: &str = "82c2bc82ac859c7780a969592e7200413a7da0963d81b13fd9ef49a22b1501c4";
/// The default's texts of the cases without a backslash (the same on every system; Windows checks
/// these).
const FIX_SHA256_PORTABLE: &str =
    "43b18637834734711f1a41fba4f2ead482d33eed7ebc180e4e4e89aad97bb9d3";

fn hex(h: Sha256) -> String {
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The cases: every one but the heavy ones, or the quick ones.
fn set(quick: bool) -> Vec<usize> {
    cases::cases()
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.heavy && (!quick || c.quick))
        .map(|(k, _)| k)
        .collect()
}

/// The full set runs in an optimized build (some 200 s in a debug one), or with
/// `FS_SCRIPTS_FULL` set.
fn full_set_runs() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("FS_SCRIPTS_FULL").is_some()
}

fn reproduce_digest(quick: bool) -> (usize, usize, String) {
    let (mut h, mut n, mut unmodelled) = (Sha256::new(), 0, 0);
    for k in set(quick) {
        let (text, ids) = cases::run_case(k, UpstreamDefects::Reproduce);
        n += 1;
        if ids.iter().any(|i| i == "UNMODELLED") {
            unmodelled += 1;
            h.update(b"UNMODELLED\n");
        } else {
            h.update(text.as_bytes());
            h.update(b"\n");
        }
    }
    (n, unmodelled, hex(h))
}

/// Whether a case means the same on every system: no backslash in its arguments (on Windows a
/// folder separator, elsewhere a character of a name).
fn portable(k: usize) -> bool {
    !cases::cases()[k].args.iter().any(|a| a.contains('\\'))
}

fn fix_digest(quick: bool) -> String {
    fix_digest_of(quick, false)
}

/// The default's digest of the set; `portable_only`: only the cases that mean the same everywhere.
fn fix_digest_of(quick: bool, portable_only: bool) -> String {
    let mut h = Sha256::new();
    for k in set(quick)
        .into_iter()
        .filter(|&k| !portable_only || portable(k))
    {
        let (f, ids) = cases::run_case(k, UpstreamDefects::Fix);
        if cfg!(unix) {
            let (r, _) = cases::run_case(k, UpstreamDefects::Reproduce);
            assert!(r == f || !ids.is_empty(), "case {k}");
        }
        h.update(f.as_bytes());
        h.update(b"\n");
    }
    hex(h)
}

#[test]
fn quick_cases_give_the_official_outputs() {
    if cfg!(unix) {
        let (n, u, h) = reproduce_digest(true);
        assert_eq!(
            (n, u, h.as_str()),
            (N_QUICK, UNMODELLED_QUICK, OFFICIAL_SHA256_QUICK)
        );
    }
    assert_eq!(fix_digest(true), FIX_SHA256_QUICK);
}

#[test]
fn every_case_gives_the_official_outputs() {
    if !full_set_runs() {
        eprintln!("skipped in a debug build (set FS_SCRIPTS_FULL to run it)");
        return;
    }
    if cfg!(unix) {
        let (n, u, h) = reproduce_digest(false);
        assert_eq!((n, u, h.as_str()), (N, UNMODELLED, OFFICIAL_SHA256));
        assert_eq!(fix_digest(false), FIX_SHA256);
    } else {
        assert_eq!(fix_digest_of(false, true), FIX_SHA256_PORTABLE);
    }
}

/// Prints both values (on Linux x64: `cargo test --release --test fs_scripts_official --
/// --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    for quick in [true, false] {
        println!("quick {quick} reproduce {:?}", reproduce_digest(quick));
        println!("quick {quick} fix {}", fix_digest(quick));
    }
    println!("portable fix {}", fix_digest_of(false, true));
}

/// Prints each case's default text's digest (to compare two systems: `cargo test --release --test
/// fs_scripts_official -- --ignored --nocapture print_case_digests`).
#[test]
#[ignore]
fn print_case_digests() {
    for k in set(false) {
        let (f, _) = cases::run_case(k, UpstreamDefects::Fix);
        let mut h = Sha256::new();
        h.update(f.as_bytes());
        println!("case {k} {}", hex(h));
    }
}
