// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_convert`'s whole conversions against FreeSurfer 8.2.0's official code on every platform:
//! the SHA-256 of the official build's output lines for the probe's `cnv` cases of seed 1 (small
//! MGH inputs, options with arguments they accept, every written file through `gzread`; the
//! command line the files keep with the time, user, machine and kernel version as `X`), recorded by `validation/probes/fs-convert/check.py --kind
//! cnv --seed 1 --n <N>` on Linux x64 from the unit oracle's build, against the port's output with
//! `reproduce` (lines where FreeSurfer reads memory it does not own or crashes, `check.py`'s
//! `SKIP_CNV`, hashed as `SKIP`; FS-1053's `valid` line masked on both sides).

#[path = "fs_convert/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-19 from the clean commit ca89ec6a (`mri_convert`'s `main` after its option
/// loop: 48 fixed conversions, then seeded ones; the files' command lines canonical, the time,
/// user, machine and kernel version as `X`, on both sides): 400 cases, 382 lines identical, 18
/// hashed as `SKIP`.
const N: usize = 400;
const OFFICIAL_SHA256: &str = "2218d1e4ccceecf5c84860f25e2418bb48c1988110dbe5c6ff72ea3177cf2f76";
const SKIPPED: usize = 18;

/// `check.py`'s `SKIP_CNV`.
const SKIP: [&str; 23] = [
    "FS-170", "FS-171", "FS-1031", "FS-1032", "FS-1034", "FS-1035", "FS-1036", "FS-1037",
    "FS-1038", "FS-1039", "FS-1040", "FS-1041", "FS-1042", "FS-53", "FS-163", "FS-1043", "FS-1044",
    "FS-1046", "FS-1047", "FS-1049", "FS-1050", "FS-1051", "FS-1052",
];

/// `check.py`'s `MASK`: FS-1053's `valid   : <n>` line as `valid   : ?`.
fn masked(line: &str, ids: &[&str]) -> String {
    if !ids.contains(&"FS-1053") {
        return line.to_owned();
    }
    let key = "valid   : ";
    let mut out = String::new();
    let mut rest = line;
    while let Some(p) = rest.find(key) {
        out.push_str(&rest[..p + key.len()]);
        rest = &rest[p + key.len()..];
        let digits = rest
            .char_indices()
            .take_while(|&(i, c)| c.is_ascii_digit() || (i == 0 && c == '-'))
            .count();
        if digits > 0 && !(digits == 1 && rest.starts_with('-')) {
            out.push('?');
            rest = &rest[digits..];
        }
    }
    out.push_str(rest);
    out
}

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate_cnv(1, N) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(masked(&out, &ids).as_bytes());
            h.update(b"\n");
        }
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!((skipped, hex.as_str()), (SKIPPED, OFFICIAL_SHA256));
}

/// The default differs from `reproduce` only where a defect it corrects is noted.
#[test]
fn fix_differs_only_with_a_notice() {
    for line in cases::generate_cnv(1, N) {
        let (r, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (f, ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(r == f || !ids.is_empty(), "{line}");
    }
}

#[test]
fn the_mask_of_fs_1053() {
    assert_eq!(
        masked("a\\x0avalid   : 1\\x0ab valid   : -12 c", &["FS-1053"]),
        "a\\x0avalid   : ?\\x0ab valid   : ? c"
    );
    assert_eq!(masked("valid   : 1", &[]), "valid   : 1");
}
