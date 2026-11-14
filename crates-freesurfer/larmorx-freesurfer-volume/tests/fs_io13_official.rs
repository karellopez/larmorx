// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece 13 (the label coordinate conversions and `MRIwriteFrame`,
//! larmorx-freesurfer-volume's `label_coords` and `io_frames`) against FreeSurfer 8.2.0's official
//! code on every platform: the SHA-256 of the official build's output lines for seed 1's first
//! 1,200 cases (`fs_io13/piece13.rs`, every case portable), recorded by
//! `validation/probes/fs-io/check.py --package larmorx-freesurfer-volume --example fs_io13_probe
//! --seed 1 --n 1200 --portable --light` on Linux x64 from the unit oracle's build (2026-11-14,
//! track A relay 22: 1,200 cases, 1,145 lines identical; 55 hashed as `SKIP`: writers not ported
//! yet, FS-429), against the port's output with `reproduce` (lines the port marks `SKIP` or `VARY`
//! hashed as `SKIP`). When the hash differs, the assertion names the first case whose output
//! differs from the recorded per-case digests (`fs_io13/official_digests.txt`, recorded on Linux
//! x64 by the ignored test below):
//!
//!     cargo test -p larmorx-freesurfer-volume --test fs_io13_official -- --ignored record_official_digests

#[path = "../../larmorx-freesurfer-io/tests/fs_io/cases.rs"]
mod cases;
#[path = "fs_io13/piece13.rs"]
mod piece13;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "f8f83b35b2bdfbe962ddea7d132141f2a229b5346bc3cc87ae8658e5f410467a";
const SKIPPED: usize = 55;

/// The recorded per-case digests: `<case line> <output or SKIP>`, 12 hex digits each.
const DIGESTS: &str = include_str!("fs_io13/official_digests.txt");

fn d48(b: &[u8]) -> String {
    Sha256::digest(b)[..6]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Each recorded case: its line, the bytes hashed for it, its output and its notices.
fn outputs() -> Vec<(String, Vec<u8>, String, Vec<&'static str>)> {
    let dir = tempfile::Builder::new()
        .prefix("fs io13 ~")
        .tempdir()
        .expect("a temporary directory");
    let w = cases::Work {
        fix: dir.path().join("fix"),
        dat: dir.path().join("dat"),
    };
    cases::make_fixture(&w.fix);
    let mut all = Vec::new();
    for line in piece13::cases(1, piece13::RECORDED) {
        let (out, ids) =
            cases::run_line_with(&line, UpstreamDefects::Reproduce, &w, Some(piece13::run));
        let hashed = if ids.iter().any(|x| *x == "SKIP" || *x == "VARY") {
            b"SKIP\n".to_vec()
        } else {
            format!("{out}\n").into_bytes()
        };
        all.push((line, hashed, out, ids));
    }
    cases::remove_fixture(&w.fix);
    all
}

fn entry(line: &str, hashed: &[u8]) -> String {
    let o = if hashed == b"SKIP\n" {
        "SKIP".to_owned()
    } else {
        d48(hashed)
    };
    format!("{} {o}", d48(line.as_bytes()))
}

fn cut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

#[test]
fn reproduce_gives_the_official_outputs() {
    let all = outputs();
    let mut h = Sha256::new();
    let mut skipped = 0;
    for (_, hashed, _, _) in &all {
        if hashed == b"SKIP\n" {
            skipped += 1;
        }
        h.update(hashed);
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if hex == OFFICIAL_SHA256 && skipped == SKIPPED {
        return;
    }
    let recorded: Vec<&str> = DIGESTS
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .collect();
    let mut differ = Vec::new();
    let mut message = format!(
        "piece 13's recorded cases: SHA-256 {hex} where {OFFICIAL_SHA256} is recorded; {skipped} SKIP or VARY where {SKIPPED}; {} cases, {} recorded digests.",
        all.len(),
        recorded.len()
    );
    for (i, (line, hashed, out, ids)) in all.iter().enumerate() {
        let mine = entry(line, hashed);
        let want = recorded.get(i).copied().unwrap_or("none");
        if mine == want {
            continue;
        }
        if differ.is_empty() {
            message.push_str(&format!(
                " First differing case {i}: {} | recorded {want}, here {mine} | here: {} | notices: {}.",
                cut(line, 300),
                cut(out, 1500),
                ids.join(",")
            ));
        }
        differ.push(i);
    }
    let first: Vec<String> = differ.iter().take(12).map(|i| i.to_string()).collect();
    message.push_str(&format!(
        " {} cases differ; the first: {}.",
        differ.len(),
        first.join(", ")
    ));
    panic!("{message}");
}

/// Records `fs_io13/official_digests.txt` from this platform's outputs, after checking that they
/// give `OFFICIAL_SHA256` (Linux x64).
#[test]
#[ignore = "records the per-case digests (Linux x64)"]
fn record_official_digests() {
    let all = outputs();
    let mut h = Sha256::new();
    for (_, hashed, _, _) in &all {
        h.update(hashed);
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, OFFICIAL_SHA256,
        "the outputs are not the recorded ones"
    );
    let mut text = String::from(
        "# SPDX-License-Identifier: LicenseRef-FreeSurfer\n# fs_io13_official.rs's per-case digests: the first 48 bits of the SHA-256 of each recorded case line and of its hashed output (SKIP: hashed as SKIP); recorded by record_official_digests on Linux x64.\n",
    );
    for (line, hashed, _, _) in &all {
        text.push_str(&entry(line, hashed));
        text.push('\n');
    }
    let p =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fs_io13/official_digests.txt");
    std::fs::write(p, text).expect("the digests written");
}
