// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.robust against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1, recorded by
//! `validation/probes/fs-robust/check.py --seed 1 --n <N>` on Linux x64 (FreeSurfer's
//! `mri_robust_register/` files compiled with the official flags, the unit oracle's libraries),
//! against the port's output with `reproduce` (lines where FreeSurfer reads or writes memory it does
//! not own, crashes or hangs, the defects of `check.py`'s `SKIP`, hashed as `SKIP`).

#[path = "fs_robust/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-18 (queue 3.2-02, `fs.robust` 1 of 2: MyMRI's, MyMatrix's and CostFunctions'
/// shared helpers, 23 commands): 2,300 cases, 2,248 lines identical, 52 hashed as `SKIP`, 2 as
/// `NANBITS` (FS-968: identical but for the NaNs' bits).
const N: usize = 2300;
const OFFICIAL_SHA256: &str = "afdca92c06265190e0ff1f7a897995758181c306a471e510b0e91271ca1c934a";
const SKIPPED: usize = 52;

/// `check.py`'s `SKIP`.
const SKIP: &[&str] = &[
    "FS-288", "FS-53", "FS-58", "FS-290", "FS-294", "FS-47", "FS-48", "ITK-112", "ITK-115",
    "ITK-116", "FS-300", "FS-302", "FS-303", "FS-304", "FS-305", "FS-306", "FS-307", "FS-308",
    "FS-309", "FS-310", "FS-314", "FS-315", "FS-322", "FS-323", "FS-324", "FS-325", "FS-327",
    "FS-330", "FS-463", "FS-465", "FS-467", "FS-163", "FS-469", "FS-470", "FS-471", "FS-473",
    "FS-475", "FS-479", "FS-482", "FS-484", "FS-485", "FS-487", "FS-488", "FS-490", "FS-491",
    "FS-493", "FS-496", "FS-497", "FS-498", "FS-499", "FS-501", "FS-502", "FS-504", "FS-508",
    "FS-509", "FS-511", "FS-515", "FS-517", "FS-270", "FS-167", "FS-886", "FS-887", "FS-889",
    "FS-890", "FS-957", "FS-964", "FS-965", "FS-540", "FS-541", "FS-546", "FS-548", "FS-549",
    "FS-550", "FS-551", "FS-552", "FS-555", "FS-559", "FS-560", "FS-561", "FS-562", "FS-564",
    "FS-567", "FS-568", "FS-569", "FS-570", "FS-571", "FS-573", "FS-575", "FS-579", "FS-581",
    "FS-582", "FS-583", "FS-586", "FS-588", "FS-589", "FS-591", "FS-593", "FS-597", "FS-598",
    "FS-600", "FS-601", "FS-615", "FS-618", "FS-966", "FS-967", "FS-1063", "FS-1073", "FS-1064",
    "FS-1065", "FS-1066", "FS-1067", "ITK-113", "FS-1068", "FS-1069",
];

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, N) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if ids.contains(&"FS-968") {
            // check.py's NANBITS: compared there with every NaN read as one, hashed so here
            h.update(b"NANBITS\n");
        } else if ids.iter().any(|x| SKIP.contains(x)) {
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

/// The matrix square root's stream (`check.py --family sqrtm`: `schur`, `sqrtm`), recorded
/// 2026-11-20 from f6dd961a (3.2-02 part 2: `MatrixSqrt`, `SchurComplex`, LAPACK 3.10.0's `ZGEES`
/// with the decided `cabs` and `csqrt` linked into the official code too): 1,200 cases, 1,187 lines
/// identical, 13 hashed as `NOTPORTED` (LAPACK's `ZLAQR0`, where `ZLAHQR` does not converge on a
/// matrix with an infinite or NaN element: FS-969).
const N_SQRTM: usize = 1200;
const OFFICIAL_SQRTM_SHA256: &str =
    "42dd17ef0dda71dd610e75d35615262066e7e2c719fe6e124ca19ad4c12e1d42";
const NOTPORTED_SQRTM: usize = 13;

#[test]
fn reproduce_gives_the_official_square_roots() {
    let mut h = Sha256::new();
    let mut notported = 0;
    for line in cases::generate_sqrtm(1, N_SQRTM) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if out.contains("is not ported yet") {
            notported += 1;
            h.update(b"NOTPORTED\n");
        } else if ids.iter().any(|x| SKIP.contains(x)) {
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(notported, NOTPORTED_SQRTM);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SQRTM_SHA256);
}

/// The square roots' default differs from `reproduce` only where a defect it corrects is noted.
#[test]
fn square_roots_fix_differs_only_with_a_notice() {
    for line in cases::generate_sqrtm(1, N_SQRTM) {
        let (r, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (f, ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(r == f || !ids.is_empty(), "{line}");
    }
}

/// The matrix logarithm's and exponential's stream (`check.py --family logm`: `logm`, `expm`,
/// `geomean`), recorded 2026-11-24 (3.2-15: `MatrixLog`, `MatrixExp`, `GeometricMean`, on the
/// decided `cabs`, `csqrt` and `clog` linked into the official code too; the cases where FreeSurfer's
/// exponential reads uninitialized memory, FS-1311, against `fsrobust-pade`, the official code with
/// those coefficients defined): 600 cases, 598 lines identical, 2 hashed as `NOTPORTED` (`ZLAQR0`,
/// FS-969).
const N_LOGM: usize = 600;
const OFFICIAL_LOGM_SHA256: &str =
    "570ad980b91284dae6b5bea960bd0572357a3a06af78655bc8863b53f7b43bda";
const NOTPORTED_LOGM: usize = 2;

#[test]
fn reproduce_gives_the_official_logarithms() {
    let mut h = Sha256::new();
    let mut notported = 0;
    for line in cases::generate_logm(1, N_LOGM) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if out.contains("is not ported yet") {
            notported += 1;
            h.update(b"NOTPORTED\n");
        } else if ids.iter().any(|x| *x == "FS-968" || *x == "SVDNAN") {
            h.update(b"NANBITS\n");
        } else if ids.iter().any(|x| SKIP.contains(x)) {
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(notported, NOTPORTED_LOGM);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_LOGM_SHA256);
}

/// The logarithm's default differs from `reproduce` only where a defect it corrects is noted.
#[test]
fn logarithms_fix_differs_only_with_a_notice() {
    for line in cases::generate_logm(1, N_LOGM) {
        let (r, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (f, ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(r == f || !ids.is_empty(), "{line}");
    }
}

/// The registration step's stream (`check.py --family regstep`: `rgauss`, `regress`, `trans`,
/// `quat`, `rstep`), recorded 2026-11-22 (3.2-03 part 1: `RobustGaussian`, `Regression`,
/// `Quaternion`, the fifteen transformation models and `RegistrationStep` in float and double):
/// 1,000 cases, 914 lines identical, 80 hashed as `SKIP` (FS-1063..FS-1067, ITK-113, ITK-116,
/// FS-163), 6 as `NOTPORTED` (the least-squares solve of fewer rows than columns under
/// `reproduce`, FS-1067's other path).
const N_REGSTEP: usize = 1000;
const OFFICIAL_REGSTEP_SHA256: &str =
    "401667eb618dc40ebb57e6110aa4f7b25c2a1af0d4d679e840562e11b37f6bb2";
const SKIPPED_REGSTEP: usize = 80;
const NOTPORTED_REGSTEP: usize = 6;

#[test]
fn reproduce_gives_the_official_registration_steps() {
    let mut h = Sha256::new();
    let (mut skipped, mut notported) = (0, 0);
    for line in cases::generate_regstep(1, N_REGSTEP) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if out.contains("is not ported yet") {
            notported += 1;
            h.update(b"NOTPORTED\n");
        } else if ids.iter().any(|x| *x == "FS-968" || *x == "SVDNAN") {
            h.update(b"NANBITS\n");
        } else if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!((skipped, notported), (SKIPPED_REGSTEP, NOTPORTED_REGSTEP));
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_REGSTEP_SHA256);
}

/// The registration step's default differs from `reproduce` only where a defect it corrects is
/// noted.
#[test]
fn registration_steps_fix_differs_only_with_a_notice() {
    for line in cases::generate_regstep(1, N_REGSTEP) {
        let (r, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (f, ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(r == f || !ids.is_empty(), "{line}");
    }
}

/// The registration's stream (`check.py --family reg`: `rreg`), recorded 2026-11-22 (3.2-03 part 2:
/// `RegRobust` with `Registration`'s code: `setSourceAndTarget` with its reordering and isotropic
/// resampling, the pyramids, the initial transforms, `findSaturation`, the multiresolution and
/// iterative registrations, the final and half-way maps, on pairs of structured volumes of 16 to
/// 33 voxels a side, 2-D and 3-D, `float`, `uchar` and `short`, every model, symmetric or not, in
/// `float` and `double`): 40 cases, 36 lines identical, 4 hashed as `SKIP` (FS-1063, `double` with
/// `iscale`). The probe compares more (seeds 7 and 13); 40 keep the debug build's run near 50 s.
const N_REG: usize = 40;
const OFFICIAL_REG_SHA256: &str =
    "5485548405ae9d5c608171b3bf6567e796903b3bf43f6614f114d2f8d9e5ecf3";
const SKIPPED_REG: usize = 4;

#[test]
fn reproduce_gives_the_official_registrations() {
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate_reg(1, N_REG) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
        let (f, fids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(f == out || !fids.is_empty(), "{line}");
    }
    assert_eq!(skipped, SKIPPED_REG);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_REG_SHA256);
}
