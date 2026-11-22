// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `getfullpath` against FreeSurfer 8.2.0's official script: the SHA-256 of the official script's
//! lines (exit status, standard output and error; the case's folder as `<DIR>`) for every fixed
//! case of the probe, recorded by `validation/probes/fs-getfullpath/check.py` on Linux x64
//! (`/bin/tcsh` 6.21.00, GNU coreutils 8.32, the C locale), against the port's lines with
//! `reproduce` on Unix (a case whose tcsh form the port does not reproduce hashed as
//! `UNMODELLED`), and the plain cases' lines (no character tcsh reads again) against the port's in
//! both modes on every system. An account that can enter folders without permission (an
//! administrator's) skips the Unix part.

#[path = "fs_getfullpath/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 from the official script (`check.py`): 134 cases, 2 not reproduced, 40
/// plain.
const N: usize = 134;
const OFFICIAL_SHA256: &str = "a2e80735ba60c2798c6284cbaca86f49c0ebb337e6b2ab9c79b71c8176b1a03f";
const PLAIN: usize = 40;
const OFFICIAL_SHA256_PLAIN: &str =
    "3bf059766084e723866c3019e85c2437c00247c6e833bca2bf62ed812f000921";
const UNMODELLED: usize = 2;

fn hex(h: Sha256) -> String {
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether this account enters a folder of mode 000 (the permission case then does not hold).
fn enters_closed_folders() -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().expect("a temporary folder");
        let p = d.path().join("closed");
        std::fs::create_dir(&p).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o000)).unwrap();
        let entered = std::fs::metadata(p.join(".")).is_ok();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        entered
    }
    #[cfg(not(unix))]
    true
}

#[test]
fn reproduce_gives_the_official_outputs_on_unix() {
    if !cfg!(unix) || enters_closed_folders() {
        eprintln!("skipped: not Unix, or this account enters closed folders");
        return;
    }
    let (mut h, mut unmodelled) = (Sha256::new(), 0);
    let lines = cases::all_cases();
    for line in &lines {
        let (out, ids) = cases::run_line(line, UpstreamDefects::Reproduce);
        if ids.contains(&"UNMODELLED") {
            unmodelled += 1;
            h.update(b"UNMODELLED\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(
        (lines.len(), unmodelled, hex(h).as_str()),
        (N, UNMODELLED, OFFICIAL_SHA256)
    );
}

#[test]
fn plain_names_give_the_official_outputs_everywhere() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut h = Sha256::new();
        let mut n = 0;
        for line in cases::all_cases().iter().filter(|l| cases::plain(l)) {
            let (out, ids) = cases::run_line(line, mode);
            if mode == UpstreamDefects::Fix {
                assert!(ids.is_empty(), "{line}: {ids:?}");
            }
            n += 1;
            h.update(out.as_bytes());
            h.update(b"\n");
        }
        // reproduce reads the case's folder again: only Unix's folder names hold no backslash
        if mode == UpstreamDefects::Reproduce && !cfg!(unix) {
            continue;
        }
        assert_eq!((n, hex(h).as_str()), (PLAIN, OFFICIAL_SHA256_PLAIN), "{mode:?}");
    }
}

/// The default differs from `reproduce` only where a defect it corrects is noted.
#[test]
fn fix_differs_only_with_a_notice() {
    for line in cases::all_cases() {
        if !cfg!(unix) && !cases::plain(&line) {
            continue;
        }
        let (r, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (f, ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(r == f || !ids.is_empty(), "{line}");
    }
}
