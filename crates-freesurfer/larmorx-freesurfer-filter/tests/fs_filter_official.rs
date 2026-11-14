// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.filter against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1, recorded by
//! `validation/probes/fs-filter/check.py --seed 1 --n <N>` on Linux x64 from the unit oracle's build,
//! against the port's output with `reproduce` (lines where FreeSurfer touches memory outside a
//! volume, crashes or hangs, `check.py`'s `SKIP`, hashed as `SKIP`).

#[path = "fs_filter/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-14 from the clean commit d4c9fb50 (the distance maps, the convolutions,
/// smoothing, window statistics, reductions, derivatives, border and non-parametric
/// classification, connected components, the frame functions, the volume clusters, the connected
/// segments, flooding, the Voronoi diagram and the soap bubble, the intensity normalisation without
/// histograms): 1,740 cases, 1,602 lines identical, 138 hashed as `SKIP`.
const N: usize = 1740;
const OFFICIAL_SHA256: &str = "e6434cc1a65dcfa28072048588087484aa8ccc5ebc022a4ce8f16f0796a55e03";
const SKIPPED: usize = 138;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 34] = [
    "FS-540", "FS-541", "FS-58", "FS-53", "FS-546", "FS-548", "FS-549", "FS-550", "FS-551",
    "FS-552", "FS-555", "FS-163", "FS-559", "FS-560", "FS-561", "FS-562", "FS-564", "FS-567",
    "FS-568", "FS-569", "FS-570", "FS-571", "FS-573", "FS-575", "FS-579", "FS-581", "FS-582",
    "FS-583", "FS-586", "FS-588", "FS-589", "FS-591", "FS-593", "FS-487",
];

/// FS-596: the number in `MRInormAddFileControlPoints`' message on rejected points is whatever
/// FreeSurfer's register holds; where the port cannot know it (`check.py`'s `MASKED`) both sides'
/// number is replaced by `?`.
fn mask(out: &str, ids: &[&str]) -> String {
    if !ids.contains(&"FS-596") {
        return out.to_owned();
    }
    let (pre, tail) = ("!!!!! ", " control points rejected");
    let mut r = String::new();
    let mut rest = out;
    while let Some(i) = rest.find(pre) {
        let after = &rest[i + pre.len()..];
        let n = after
            .char_indices()
            .take_while(|&(k, c)| c.is_ascii_digit() || (k == 0 && c == '-'))
            .count();
        if n > 0 && after[n..].starts_with(tail) {
            r.push_str(&rest[..i + pre.len()]);
            r.push('?');
            rest = &after[n..];
        } else {
            r.push_str(&rest[..i + pre.len()]);
            rest = after;
        }
    }
    r.push_str(rest);
    r
}

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, N) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(mask(&out, &ids).as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(skipped, SKIPPED);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}

/// The default differs from `reproduce` only where a defect it corrects is noted.
#[test]
fn fix_differs_only_with_a_notice() {
    for line in cases::generate(1, N) {
        let (r, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (f, ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(r == f || !ids.is_empty(), "{line}");
    }
}
