// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surfio against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (720 cases, thirty of each of
//! the 24 commands; `metric`'s through the probe built with a stub metric-properties step),
//! recorded by `validation/probes/fs-surfio/check.py --seed 1 --n 720` on Linux x64 from the unit
//! oracle's build (2026-11-13: 720 lines identical), against the port's output with `reproduce`.

#[path = "fs_surfio/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

const OFFICIAL_SHA256: &str = "15533dfff68f232f97a985423d16c9fc84d53dcc5b71b99be897d3e66856a825";

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    for line in cases::generate(1, 720) {
        let (out, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        h.update(out.as_bytes());
        h.update(b"\n");
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}
