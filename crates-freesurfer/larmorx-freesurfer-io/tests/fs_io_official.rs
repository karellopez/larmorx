// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.io against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the official
//! build's output lines for the probe's recorded cases (`cases::recorded_cases`: seed 1's 4,800
//! portable cases, about 87 of each of the 55 commands that behave alike on every platform: piece
//! 1's ten, piece 2's twelve, piece 3's six, piece 4's seven (the format probe's file cases made
//! portable), piece 5's seven (mosaics, MATLAB files), piece 6's five (reading volumes and
//! registration files) and piece 7's seven (Bruker studies), without those that take seconds in a
//! debug build, `cases::HEAVY_SEED1`, which the probe's runs keep), recorded by
//! `validation/probes/fs-io/check.py --seed 1 --n 4800 --portable --light` on Linux x64 from the
//! unit oracle's build (2026-11-13, track A relay 17, with piece 7: 4,799 cases, 4,192 lines
//! identical, 81 of them with memory digits; 607 hashed as `SKIP`), against the port's output with
//! `reproduce`. A `?` digit (a byte FreeSurfer reads from memory contents) is hashed as `?`; the
//! lines the port marks `SKIP` or `VARY` are hashed as `SKIP` (lines where FreeSurfer crashes,
//! reads for ever, depends on the machine or on memory contents, and formats not ported yet).
//!
//! When the hash differs, the assertion names the first case whose output differs from the
//! recorded per-case digests (`fs_io/official_digests.txt`: the first 48 bits of the SHA-256 of
//! each case line and of its hashed output; CI shows only the message): its index, its line, the
//! port's output and notices here, and how many cases differ. The case's official output is the
//! Linux port's (`fs_io_probe run` of that line, or `fsio`). The digests are recorded with the
//! ignored test below, on Linux x64, where the outputs give `OFFICIAL_SHA256`:
//!
//!     cargo test -p larmorx-freesurfer-io --test fs_io_official -- --ignored record_official_digests

#[path = "fs_io/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "93576fdf78cc9d86e46e5d625dc9a240155dead5e835f7bce4b1faeb8552e6d8";
const SKIPPED: usize = 607;

/// The recorded per-case digests: `<case line> <output or SKIP>`, 12 hex digits each.
const DIGESTS: &str = include_str!("fs_io/official_digests.txt");

fn d48(b: &[u8]) -> String {
    Sha256::digest(b)[..6]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Each recorded case: its line, the bytes hashed for it (`SKIP\n` or its output and a newline),
/// its output and its notices.
fn outputs() -> Vec<(String, Vec<u8>, String, Vec<&'static str>)> {
    let dir = tempfile::Builder::new()
        .prefix("fs io ~")
        .tempdir()
        .expect("a temporary directory");
    let w = cases::Work {
        fix: dir.path().join("fix"),
        dat: dir.path().join("dat"),
    };
    cases::make_fixture(&w.fix);
    let mut all = Vec::new();
    for line in cases::recorded_cases() {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce, &w);
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
        "fs.io's recorded cases: SHA-256 {hex} where {OFFICIAL_SHA256} is recorded; {skipped} SKIP or VARY where {SKIPPED}; {} cases, {} recorded digests.",
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
            let same_case = want.split(' ').next() == mine.split(' ').next();
            message.push_str(&format!(
                " First differing case {i}{}: {} | recorded {want}, here {mine} | here: {} | notices: {}.",
                if same_case { "" } else { " (the digests list another case: regenerate them)" },
                cut(line, 300),
                cut(out, 1500),
                ids.join(",")
            ));
        }
        differ.push(i);
    }
    let first: Vec<String> = differ
        .iter()
        .take(12)
        .map(|&i| format!("{i} {}", all[i].0.split(' ').next().unwrap_or("")))
        .collect();
    message.push_str(&format!(
        " {} cases differ; the first: {}.",
        differ.len(),
        first.join(", ")
    ));
    panic!("{message}");
}

/// Records `fs_io/official_digests.txt` from this platform's outputs, after checking that they give
/// `OFFICIAL_SHA256` (Linux x64, where the port's outputs are the official build's).
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
        "# SPDX-License-Identifier: LicenseRef-FreeSurfer\n# fs_io_official.rs's per-case digests: the first 48 bits of the SHA-256 of each recorded case line and of its hashed output (SKIP: hashed as SKIP); recorded by record_official_digests on Linux x64.\n",
    );
    for (line, hashed, _, _) in &all {
        text.push_str(&entry(line, hashed));
        text.push('\n');
    }
    let p =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fs_io/official_digests.txt");
    std::fs::write(p, text).expect("the digests written");
}

/// The fixture's path is removed in each form the port prints it on Windows (CI's temporary
/// folders, `C:\Users\RUNNER~1\...`): joined by the platform, by a `/` (`mri_read`'s working
/// directory, MINC's file names), or with every `\` turned into `/`.
#[test]
fn the_fixture_path_is_removed_in_every_form() {
    let fix = std::path::Path::new(r"C:\Users\RUNNER~1\AppData\Local\Temp\fs io ~aB3\fix");
    for (raw, want) in [
        (
            r"ERROR: C:\Users\RUNNER~1\AppData\Local\Temp\fs io ~aB3\fix/f is not a directory.",
            "ERROR: f is not a directory.",
        ),
        (
            r"open C:\Users\RUNNER~1\AppData\Local\Temp\fs io ~aB3\fix\w/a.mgz",
            "open w/a.mgz",
        ),
        (
            "glob C:/Users/RUNNER~1/AppData/Local/Temp/fs io ~aB3/fix/w/x",
            "glob w/x",
        ),
    ] {
        assert_eq!(cases::strip_fix(raw.as_bytes(), fix), want.as_bytes());
    }
}
