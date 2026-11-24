// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_pretess`'s golden cases on every platform: the program on 32^3 volumes made with integer
//! arithmetic (`mri_pretess_golden/common.rs`; one of them `float` with NaNs and infinities), the
//! SHA-256 of each case's exit status, standard output, standard error (larmorx's notices left
//! out) and outputs' digests, against the value FreeSurfer 8.2.0's official program gives on the
//! same inputs (`validation/probes/fs-pretess/prog.py --golden`, Linux x64) with `reproduce`, and
//! against the port's own Linux x64 value with the default. And `-w` (the diagnostic writes, never
//! given to the official program: its licence check) changes nothing.

#[path = "mri_pretess_golden/common.rs"]
mod common;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-24 by `prog.py --golden` from the official program on Linux x64 (the port's
/// `reproduce` gave the same): eight cases.
const OFFICIAL_SHA256: &str = "7ad16e8175b1a44baf52adf23947e525d00f8961a66151562e33ea0223490d86";
/// The port's default on Linux x64 (`cases_digest(Fix)`): an error for the label above 255
/// (FS-1282), status 1 for the unwritten output (FS-1285), no crash for `-debug_voxel` without its
/// value (FS-1281).
const FIX_SHA256: &str = "7668b7d9521b9f48316229a15a84f9bb094425ad3c3ac865bbef4054f7a64432";

fn cases_digest(mode: UpstreamDefects) -> String {
    let dir = tempfile::tempdir().expect("a folder");
    let mut h = Sha256::new();
    for k in 0..common::cases().len() {
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

/// `-w` (any option starting with `w`: FreeSurfer's diagnostic write flag) gives recon-all's
/// white-matter call the same streams and output, in both modes.
#[test]
fn the_diagnostic_write_option_changes_nothing() {
    let dir = tempfile::tempdir().expect("a folder");
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let plain = common::run_case(dir.path(), 0, mode);
        let a = common::digest(&dir.path().join("out.mgz"));
        let mut args = vec!["-w".to_owned()];
        args.extend(common::cases().swap_remove(0));
        let with_w = common::run_args(dir.path(), &args, &[], mode);
        let b = common::digest(&dir.path().join("out.mgz"));
        assert_eq!(plain, with_w);
        assert_eq!(a, b);
    }
}

/// Prints both values (run on Linux x64 to record them: `cargo test --test mri_pretess_golden --
/// --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
}
