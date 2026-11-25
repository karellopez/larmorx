// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_register`'s golden cases on every platform: the program on 16^3 images made with
//! integer arithmetic (`fs_robreg/cases.rs`' `golden_cases`: fMRIPrep's call with every output, an
//! affine registration with its half-way outputs, a short source onto a float target of
//! anisotropic voxels, the entropy images, the Powell registration, the mutual information and the
//! SB cost; the test mode, `--test`, on images of 32^3 and 16^3; the debug output, `--debug`), the
//! SHA-256 of each case's line (exit status, standard output and error, every file written; the
//! time taken masked), against the value FreeSurfer 8.2.0's official program gives on the same
//! inputs (`validation/probes/fs-robust-register/check.py --golden --preload`, Linux x64, the
//! decided `cabs`, `csqrt`, `acos` and `clog`) with `reproduce`, and against the port's own Linux
//! x64 value with the default. The cases run side by side, so the debug build's test takes a few
//! seconds.

#[path = "fs_robreg/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-22 by `check.py --golden` from the official program on Linux x64: 3 cases;
/// re-recorded 2026-11-23 (relay 45): 4 cases, the entropy images added; again (relay 46): 5
/// cases, the Powell registration (`--cost NCC`) added (the same value with the decided `cabs` and
/// `csqrt` preloaded); again (relay 47, with that preload): 7 cases, `--cost MI` and `--cost SB` on
/// 16^3 images added; again (relay 48, with that preload): 11 cases, the test mode (`--test 4`,
/// `--test 5` on a float image, `--test 30`, `--test 0` on a 16^3 image) added; again (relay 49,
/// with that preload): 13 cases, the debug output (a robust registration with `--satit` and a
/// half-way image, `--cost MI`) added; again (relay 52, from the clean commit e77cb321, with the
/// decided `cabs`, `csqrt`, `acos` and `clog` preloaded: the same value as with `cabs` and `csqrt`
/// alone): the registrations' images 16^3, the test mode's 32^3 byte image drawn after them.
const OFFICIAL_SHA256: &str = "7d92cb63b3f2c861b35eb3716a97968c00df9baa3453c0643d1f45c215e7ea3e";
/// The port's default on Linux x64 (FS-1078's pattern as voxel values, FS-1076's error).
const FIX_SHA256: &str = "fb1a7c0ecfc70a70837198a58343391dcd0ea7490d501cedc167063fd28a3f77";

/// The lines' outputs run side by side (each case in its own folder, with its own state), hashed
/// in order.
fn digest(lines: &[String], mode: UpstreamDefects) -> String {
    let outs: Vec<String> = std::thread::scope(|sc| {
        let hs: Vec<_> = lines
            .iter()
            .map(|line| sc.spawn(move || cases::run_line(line, mode).0))
            .collect();
        hs.into_iter().map(|h| h.join().expect("a case")).collect()
    });
    let mut h = Sha256::new();
    for out in outs {
        h.update(out.as_bytes());
        h.update(b"\n");
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn cases_digest(mode: UpstreamDefects) -> String {
    digest(&cases::golden_cases(), mode)
}

#[test]
fn reproduce_gives_the_official_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Reproduce), OFFICIAL_SHA256);
}

#[test]
fn fix_gives_the_recorded_outputs() {
    assert_eq!(cases_digest(UpstreamDefects::Fix), FIX_SHA256);
}

/// The Powell registration's half-way outputs (relay 48, `cases::golden_halfway_cases`): FreeSurfer
/// reads their geometry from memory it has freed (FS-1075), so the value is the port's on Linux x64,
/// the same in both modes.
const HALFWAY_SHA256: &str = "1633129d30d8fc51a906e516333ea48631d4dbc1c07c659d35f873e829fbee3c";

fn halfway_digest(mode: UpstreamDefects) -> String {
    digest(&cases::golden_halfway_cases(), mode)
}

#[test]
fn powell_half_way_outputs_give_the_recorded_outputs() {
    let (rep, fix) = std::thread::scope(|sc| {
        let r = sc.spawn(|| halfway_digest(UpstreamDefects::Reproduce));
        let f = sc.spawn(|| halfway_digest(UpstreamDefects::Fix));
        (r.join().expect("reproduce"), f.join().expect("fix"))
    });
    assert_eq!(rep, HALFWAY_SHA256);
    assert_eq!(fix, HALFWAY_SHA256);
}

/// Prints the values (run on Linux x64 to record them: `cargo test --test
/// mri_robust_register_golden -- --ignored --nocapture`).
#[test]
#[ignore]
fn print_digests() {
    println!("reproduce {}", cases_digest(UpstreamDefects::Reproduce));
    println!("fix {}", cases_digest(UpstreamDefects::Fix));
    println!(
        "half-way reproduce {}",
        halfway_digest(UpstreamDefects::Reproduce)
    );
    println!("half-way fix {}", halfway_digest(UpstreamDefects::Fix));
}
