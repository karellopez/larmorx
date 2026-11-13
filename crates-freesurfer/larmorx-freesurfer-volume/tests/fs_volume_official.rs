// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.volume against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1, recorded by
//! `validation/probes/fs-volume/check.py --seed 1 --n <N>` on Linux x64 from the unit oracle's build,
//! against the port's output with `reproduce` (lines where FreeSurfer reads or writes memory it does
//! not own or crashes, FS-53, FS-58, FS-288, FS-290, FS-294, and the eigen-systems it never returns from, FS-47, FS-48,
//! ITK-112, ITK-115, ITK-116, hashed as `SKIP`).

#[path = "fs_volume/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-13 (47 commands: transform.cpp's VOL_GEOM functions, 3.1-35's voxel
/// arithmetic, coordinates and geometry, `MRIaverage`): 940 cases, 886 lines identical, 54 hashed
/// as `SKIP`.
const N: usize = 940;
const OFFICIAL_SHA256: &str = "2b37ea96a0d3665073c7c7b2fc0563aa984f9826fc4d736a8f2c8a702114b30b";
const SKIPPED: usize = 54;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 10] = [
    "FS-288", "FS-53", "FS-58", "FS-290", "FS-294", "FS-47", "FS-48", "ITK-112", "ITK-115",
    "ITK-116",
];

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
            h.update(out.as_bytes());
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

/// `MRIextractInto` with an integral (negative) voxel size: the default gives FreeSurfer's start and
/// end, the sign of a zero included (found at seed 7: `-0 + 0 * -1` is +0 in FreeSurfer's integer
/// product, -0 in a float one).
#[test]
fn fix_keeps_freesurfers_start_for_integral_voxel_sizes() {
    let line = "extract 1 1 1 3 1 0 182 1 2 4 4 40000000 bf800000 bf800000 1 bf19999a 3dcccccd \
        3f7eb852 3f4ccccd 3f7eb852 bf800000 bf4ccccd bf800000 bf800000 c1a60000 c0e00000 c1f50000 \
        be99999a 37a7c5ac 00000000 - 0 2 c2878000 c2310000 425a0000 c22a0000 80000000 41a00000 \
        00000000 00000000 44fa0000 3a83126f 00000000 3ff8000000000000 3 1 0 0 -1 0 2 1 2 2 -1 1";
    let (r, _) = cases::run_line(line, UpstreamDefects::Reproduce);
    let (f, ids) = cases::run_line(line, UpstreamDefects::Fix);
    assert!(ids.is_empty(), "{ids:?}");
    assert_eq!(r, f);
}
