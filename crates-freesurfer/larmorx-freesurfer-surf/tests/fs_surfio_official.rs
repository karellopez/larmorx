// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surfio against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1 (1,230 cases, thirty of each of
//! the 41 commands; `metric`'s through the probe built with a stub metric-properties step),
//! recorded by `validation/probes/fs-surfio/check.py --seed 1 --n 1230` on Linux x64 from the unit
//! oracle's build (2026-11-13, relay 20: 1,206 lines identical, 24 counted, not compared, where
//! FreeSurfer reads memory it never wrote or does not own, writes past a buffer or loops for ever:
//! `SKIP`), against the port's output with `reproduce` (a case the port marks with one of those
//! defects hashed as `SKIP`, as `check.py` does). The `sfile`, `cfile`, `sread`, `stl`, `tri`,
//! `fnam`, `swr`, `srd`, `mpb`, `mpc`, `mpd` and `mpe` cases run in a temporary folder of their own.

#[path = "fs_surfio/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use sha2::{Digest, Sha256};

/// `check.py`'s `SKIP` and `SKIP_WHEN_REFUSED`.
const SKIP: [&str; 41] = [
    "FS-121", "FS-123", "FS-06", "FS-167", "FS-49", "FS-140", "FS-144", "FS-151", "FS-154",
    "FS-157", "FS-158", "FS-160", "FS-346", "FS-352", "FS-354", "FS-356", "FS-357", "FS-358",
    "FS-359", "FS-360", "FS-361", "FS-362", "FS-367", "FS-370", "FS-371", "FS-372", "FS-373",
    "FS-76", "FS-376", "FS-379", "FS-380", "FS-48h", "FS-382", "FS-383", "FS-384", "FS-386",
    "FS-388", "FS-389", "FS-391", "FS-392", "FS-53",
];
const SKIP_WHEN_REFUSED: [&str; 1] = ["FS-114"];

const OFFICIAL_SHA256: &str = "0e25d92be940267c0bd6f7d96a0d5541e2d3d4688caa8e762beccfc541641353";

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    for line in cases::generate(1, 1230) {
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
    for line in cases::generate(1, 103 * 41) {
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
/// `OpenSvdcmp` in both modes notes FS-48 and the key finds the hang; on a finite matrix it does
/// not. A change of fs.core's FS-48 text that loses the marker fails here, by name.
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
