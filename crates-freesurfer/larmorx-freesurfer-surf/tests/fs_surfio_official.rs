// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surfio against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (870 cases, thirty of each of
//! the 29 commands; `metric`'s through the probe built with a stub metric-properties step),
//! recorded by `validation/probes/fs-surfio/check.py --seed 1 --n 870` on Linux x64 from the unit
//! oracle's build (2026-11-13: 865 lines identical, 5 `mprop` cases counted, not compared, where
//! FreeSurfer reads memory it never wrote: `SKIP`), against the port's output with `reproduce`
//! (a case the port marks with one of those defects hashed as `SKIP`, as `check.py` does).

#[path = "fs_surfio/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// `check.py`'s `SKIP` and `SKIP_WHEN_REFUSED`.
const SKIP: [&str; 3] = ["FS-121", "FS-123", "FS-06"];
const SKIP_WHEN_REFUSED: [&str; 1] = ["FS-114"];

const OFFICIAL_SHA256: &str = "16d6b5a0f4b53fe3a13a05708f5cdcd842a026a4ec1737698d1b7f9008125e3b";

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    for line in cases::generate(1, 870) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let refused = out.starts_with("EXIT ");
        let skip = ids.iter().any(|x| SKIP.contains(x))
            || (refused && ids.iter().any(|x| SKIP_WHEN_REFUSED.contains(x)));
        h.update(if skip {
            "SKIP".as_bytes()
        } else {
            out.as_bytes()
        });
        h.update(b"\n");
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}
