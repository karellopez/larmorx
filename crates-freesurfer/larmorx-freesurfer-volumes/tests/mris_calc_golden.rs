// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mris_calc`'s golden cases on every platform: the program on 32^3 inputs made with integer
//! arithmetic (`mris_calc_golden/common.rs`), the SHA-256 of each case's exit status, standard
//! output (the peak memory masked), standard error (larmorx's notices left out) and outputs'
//! digests, against the value
//! FreeSurfer 8.2.0's official program gives on the same inputs (`validation/probes/fs-calc/
//! prog.py --golden`, Linux x64) with `reproduce`, and against the port's own Linux x64 value with
//! the default.

#[path = "mris_calc_golden/common.rs"]
mod common;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 by `prog.py --golden` from the official program on Linux x64 (the port's
/// `reproduce` gave the same): ten cases, 32^3 volumes and 1,000-value curvature files.
const OFFICIAL_SHA256: &str = "b458bcb2273de933e2d984c81896ec11a90b8f92d92d2d17145b0bc010f3c608";
/// The port's default on Linux x64 (`cases_digest(Fix)`, recorded with the official value: the
/// correctly rounded `atan2f`, `pow` and `log10` (GLIBC-07), `andbw` of NaN and infinities
/// (FS-17)).
const FIX_SHA256: &str = "4c2c95918dc4f37bcf540273acd190fac4c1c56c71f6836ee8dea47ae4e6bd7f";

fn cases_digest(mode: UpstreamDefects) -> String {
    let dir = tempfile::tempdir().expect("a folder");
    common::write_inputs(dir.path());
    let mut h = Sha256::new();
    for k in 0..common::CASES.len() {
        let (status, text) = common::run_case(dir.path(), k, mode);
        // as prog.py --golden takes the official's text: without its last new lines
        h.update(format!(
            "case {k} {status}\n{}\n",
            text.trim_end_matches('\n')
        ));
        for name in common::OUTPUTS[k] {
            h.update(format!(
                "{name} {}\n",
                common::digest(&dir.path().join(name))
            ));
        }
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn reproduce_gives_the_official_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Reproduce), OFFICIAL_SHA256);
}

#[test]
fn fix_gives_the_recorded_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Fix), FIX_SHA256);
}

/// Prints both values (run on Linux x64 to record them: `cargo test --test mris_calc_golden
/// -- --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
