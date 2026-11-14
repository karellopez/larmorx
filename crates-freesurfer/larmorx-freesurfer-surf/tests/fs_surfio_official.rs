// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surfio against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (1,350 cases, thirty of each of
//! the 45 commands; `metric`'s through the probe built with a stub metric-properties step),
//! recorded by `validation/probes/fs-surfio/check.py --seed 1 --n 1350` on Linux x64 from the unit
//! oracle's build (2026-11-14, relay 24: 1,309 lines identical, 41 counted, not compared, where
//! FreeSurfer reads memory it never wrote or does not own, writes past a buffer or loops for ever:
//! `SKIP`; an `mpg` line whose only differences are glibc's `acos` last bits would be hashed as the
//! port's, none in this set), against the port's output with `reproduce` (a case the port marks
//! with one of those defects hashed as `SKIP`, as `check.py` does). The `sfile`, `cfile`, `sread`,
//! `stl`, `tri`, `fnam`, `swr`, `srd`, `mpb`, `mpc`, `mpd`, `mpe`, `gii`, `gwr`, `mpf` and `mpg`
//! cases run in a temporary folder of their own.
//!
//! When the hash differs, the assertion names the first case whose output differs from the
//! recorded per-case digests (`fs_surfio/official_digests.txt`), how many differ and which; the
//! digests are recorded on Linux x64 by the ignored `record_official_digests`.

#[path = "fs_surfio/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use sha2::{Digest, Sha256};

/// `check.py`'s `SKIP` and `SKIP_WHEN_REFUSED`.
const SKIP: [&str; 44] = [
    "FS-121", "FS-123", "FS-06", "FS-167", "FS-49", "FS-140", "FS-144", "FS-151", "FS-154",
    "FS-157", "FS-158", "FS-160", "FS-346", "FS-352", "FS-354", "FS-356", "FS-357", "FS-358",
    "FS-359", "FS-360", "FS-361", "FS-362", "FS-367", "FS-370", "FS-371", "FS-372", "FS-373",
    "FS-76", "FS-376", "FS-379", "FS-380", "FS-48h", "FS-382", "FS-383", "FS-384", "FS-386",
    "FS-388", "FS-389", "FS-391", "FS-392", "FS-53", "FS-521", "FS-533", "FS-534",
];
const SKIP_WHEN_REFUSED: [&str; 1] = ["FS-114"];

const OFFICIAL_SHA256: &str = "6a44bcf748d45fc2be9fbcfb07335b04a355090e528bf8db73b40258f0e84b1c";

/// The recorded per-case digests: `<case line> <output or SKIP>`, 12 hex digits each.
const DIGESTS: &str = include_str!("fs_surfio/official_digests.txt");

fn d48(b: &[u8]) -> String {
    Sha256::digest(b)[..6]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Each case of seed 1: its line, the bytes hashed for it (`SKIP\n` or its output and a
/// newline), its output and its notices.
fn outputs() -> Vec<(String, Vec<u8>, String, Vec<&'static str>)> {
    cases::generate(1, 1350)
        .into_iter()
        .map(|line| {
            let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
            let refused = out.starts_with("EXIT ");
            let skip = ids.iter().any(|x| SKIP.contains(x))
                || (refused && ids.iter().any(|x| SKIP_WHEN_REFUSED.contains(x)));
            let hashed = if skip {
                b"SKIP\n".to_vec()
            } else {
                format!("{out}\n").into_bytes()
            };
            (line, hashed, out, ids)
        })
        .collect()
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

fn sha256_of(all: &[(String, Vec<u8>, String, Vec<&'static str>)]) -> String {
    let mut h = Sha256::new();
    for (_, hashed, _, _) in all {
        h.update(hashed);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// When the hash differs, the message names the first case whose output differs from the
/// recorded per-case digests (`fs_surfio/official_digests.txt`: the first 48 bits of the SHA-256
/// of each case line and of its hashed output; CI shows only the message): its index, its line,
/// the port's output and notices here, and how many cases differ. The case's official output is
/// the Linux port's (`fs_surfio_probe run` of that line, or `fssurfio`).
#[test]
fn reproduce_gives_the_official_outputs() {
    let all = outputs();
    let hex = sha256_of(&all);
    if hex == OFFICIAL_SHA256 {
        return;
    }
    let recorded: Vec<&str> = DIGESTS
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .collect();
    let mut differ = Vec::new();
    let mut message = format!(
        "fs.surfio's seed-1 cases: SHA-256 {hex} where {OFFICIAL_SHA256} is recorded; {} cases, {} recorded digests.",
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
                if same_case {
                    ""
                } else {
                    " (the digests list another case: regenerate them)"
                },
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

/// Records `fs_surfio/official_digests.txt` from this platform's outputs, after checking that
/// they give `OFFICIAL_SHA256` (Linux x64, where the port's outputs are the official build's):
///
///     cargo test -p larmorx-freesurfer-surf --test fs_surfio_official -- --ignored record_official_digests
#[test]
#[ignore = "records the per-case digests (Linux x64)"]
fn record_official_digests() {
    let all = outputs();
    assert_eq!(
        sha256_of(&all),
        OFFICIAL_SHA256,
        "the outputs are not the recorded ones"
    );
    let mut text = String::from(
        "# SPDX-License-Identifier: LicenseRef-FreeSurfer\n# fs_surfio_official.rs's per-case digests: the first 48 bits of the SHA-256 of each seed-1 case line and of its hashed output (SKIP: hashed as SKIP); recorded by record_official_digests on Linux x64.\n",
    );
    for (line, hashed, _, _) in &all {
        text.push_str(&entry(line, hashed));
        text.push('\n');
    }
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fs_surfio/official_digests.txt");
    std::fs::write(p, text).expect("the digests written");
}

/// FS-120 is kept by default (rule D, until measured): the default mode never notes FS-120 on
/// an `mprop` case; the correction on request (`fs120_write_face_angles` with `fix`) changes
/// only cases it notes (some of them in what they print: a later step can recompute the
/// angles); with `reproduce` the request changes nothing.
#[test]
fn fs120_the_face_angles_are_written_back_only_on_request() {
    let on = || SurfGlobals {
        fs120_write_face_angles: true,
        ..SurfGlobals::default()
    };
    let (mut noted, mut changed, mut cases) = (0, 0, 0);
    for line in cases::generate(1, 103 * 45) {
        if !line.starts_with("mprop ") {
            continue;
        }
        cases += 1;
        let (fix, fix_ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(!fix_ids.contains(&"FS-120"), "{line}");
        let (rep, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (rep_on, _) = cases::run_line_with(&line, UpstreamDefects::Reproduce, on());
        assert_eq!(rep, rep_on, "{line}");
        let (fix_on, on_ids) = cases::run_line_with(&line, UpstreamDefects::Fix, on());
        if on_ids.contains(&"FS-120") {
            noted += 1;
            changed += usize::from(fix != fix_on);
        } else {
            assert_eq!(fix, fix_on, "{line}");
        }
    }
    assert!(
        cases >= 100 && noted > 0 && changed > 0,
        "{cases} cases, {noted} noted, {changed} changed"
    );
    eprintln!("FS-120 on request: {cases} mprop cases, {noted} noted, {changed} changed");
}

/// The `FS-48h` key (`cases::fs48_hangs`) on a matrix on which LINPACK's `ssvdc` never returns
/// (`larmorx_core::vnl_svd_f32::vnl_svd_f32_full` names it):
/// `OpenSvdcmp` in both modes notes FS-48 with ITK-116 and the key finds the hang; on a finite
/// matrix it does not. A change of fs.core's notices that loses ITK-116 fails here, by name.
#[test]
fn fs48_hang_key_holds() {
    use larmorx_core::vnl_svd_f32::vnl_svd_f32_full;
    use larmorx_freesurfer_core::globals::Globals;
    use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc};
    use larmorx_freesurfer_core::numerics::open_svdcmp;
    // a 7 x 6 matrix with one infinite entry on which ssvdc never returns (found by a seeded
    // search over small integer matrices with infinite or NaN entries, relay 21)
    let inf = f32::INFINITY;
    #[rustfmt::skip]
    let hanging: (usize, usize, Vec<f32>) = (7, 6, vec![
        4.0, 2.0, 3.0, -3.0, -4.0, 1.0, -3.0, 2.0, 2.0, inf, -2.0, 2.0, -4.0, -1.0, 2.0, 1.0,
        -4.0, -3.0, 4.0, -2.0, 3.0, 4.0, -1.0, -2.0, 1.0, -2.0, 1.0, -2.0, 1.0, -2.0, 2.0, -3.0,
        3.0, -1.0, 0.0, -1.0, -4.0, -2.0, -2.0, -2.0, 3.0, 2.0,
    ]);
    assert!(vnl_svd_f32_full(&hanging.2, 7, 6).never_returns());
    let run = |rows: usize, cols: usize, a: &[f32], mode: UpstreamDefects| {
        let mut g = Globals::new(mode);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut alloc = |r: usize, c: usize| {
            matrix_alloc(&mut g, &mut out, &mut err, r as i32, c as i32, MATRIX_REAL)
                .unwrap()
                .unwrap()
        };
        let (mut m, mut w, mut v) = (alloc(rows, cols), alloc(cols, 1), alloc(cols, cols));
        m.data_mut()[..rows * cols].copy_from_slice(a);
        let _ = open_svdcmp(&mut g, &mut err, &mut m, &mut w, &mut v);
        cases::fs48_hangs(g.notices.as_slice())
    };
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        assert!(run(hanging.0, hanging.1, &hanging.2, mode), "{mode:?}");
        assert!(!run(2, 2, &[1.0, 2.0, 3.0, 4.0], mode), "{mode:?}");
    }
}
