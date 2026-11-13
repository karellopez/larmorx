// SPDX-License-Identifier: Apache-2.0
//! fs.maths' defects as D13 switches (`specs/freesurfer-maths.md` §24): each one noted in both
//! modes on a case that meets it, and its two modes' results different where the correction
//! changes them (memory reads, refusals and crashes give the same result in both modes, §1,
//! §23.3). The cases are lines of the comparison's own seeded batches (the shortest line on which
//! the clean room notes the defect in both modes), run through the probe's commands
//! (`tests/maths_cmd/`); a few are direct calls.
//!
//! Not covered here: FS-30 (two consecutive `drand48` draws of exactly 0.5 cannot occur: the
//! generator maps the state 2^47 to 2^47 + 11), FS-41 (an eigensystem of more than 1,024 rows),
//! FS-42 and ITK-111 (QL and QR methods that do not converge), FS-39 below only through the
//! driver's evaluation limit.

mod maths_cmd;

use larmorx_core::defects::UpstreamDefects;
use larmorx_fs_core::maths::minimize::LbfgsDriver;
use larmorx_fs_core::{Environment, Process};

/// (defect, the old allocator's environment, the two modes' results differ, the line).
const CASES: &[(&str, bool, bool, &str)] = &[
    ("FS-01", false, true, "halloc -1"),
    (
        "FS-12",
        false,
        true,
        "haddsample 1 1 3f000000 3fc00000 3fc00000 3fc00000 00000000 4 7fc00000 c1100000 41b80000 af378426 42c28b63 c2c3527d",
    ),
    ("FS-13", true, false, "malloc -1 3 1"),
    // FS-13 with `reproduce`: FreeSurfer's matrix of no element; the record under rows -9 to -16
    ("FS-13", false, true, "malloc -2 1 1"),
    ("FS-13", false, true, "malloc2 -9 0 2"),
    ("FS-14", false, false, "mclone 2 1 1 c294b01c c23169fc"),
    (
        "FS-15",
        false,
        true,
        "mmul 1 2 2 d4ce728b f0d9008b c0000000 c0aedacb 2 2 1 35830e26 00000001 7f7fffff 42786ddb 2 2 1 41b00000 421c0000 4270c58f c7af3faa",
    ),
    (
        "FS-16",
        false,
        true,
        "mmul 1 2 2 d4ce728b f0d9008b c0000000 c0aedacb 2 2 1 35830e26 00000001 7f7fffff 42786ddb 1 2 1 421801f7 4de8d852",
    ),
    (
        "FS-17",
        false,
        false,
        "hvalley 1 1 3f800000 ffc00000 3fc00000 3fc00000 ffd00001 7 9 53",
    ),
    (
        "FS-18",
        false,
        true,
        "hclearcounts 2 2 3f800000 40a00000 40c00000 40a00000 40c00000 7fc12345 ffc00000 new",
    ),
    (
        "FS-19",
        false,
        true,
        "haddsample 2 2 40000000 00000000 40000000 00000000 40000000 41a80000 00000000 1 c266d083 c196cbaa 4260bded",
    ),
    (
        "FS-20",
        false,
        true,
        "haddsample 1 1 3f800000 00000000 00000000 00000000 42ce0000 2 42a2d549 41600000 41300000 41980000",
    ),
    (
        "FS-21",
        false,
        false,
        "hcopy 2 2 3f800000 40a00000 40c00000 40a00000 40c00000 7fc12345 ffc00000 1 1 3e800000 00000000 00000000 00000000 428edd94",
    ),
    (
        "FS-22",
        false,
        false,
        "hfill 1 1 40400000 c1200000 c1200000 c1200000 428e0000",
    ),
    (
        "FS-23",
        false,
        true,
        "hpdf 1 1 40400000 c1200000 c1200000 c1200000 428e0000 1 1 40400000 3fc00000 3fc00000 3fc00000 00000000",
    ),
    (
        "FS-24",
        false,
        false,
        "hrobust 1 1 40400000 40a00000 40a00000 40a00000 42840000 3feccccccccccccd",
    ),
    ("FS-25", false, true, "msum 2 1 1 c294b01c c23169fc"),
    (
        "FS-27",
        false,
        true,
        "mregion 2 1 3 42860203 425c0000 42500000 c1000000 41a80000 c1880000 1 1 1 424216b9 1 3 0 0 1 1",
    ),
    ("FS-28", false, true, "midentity 3 1 1 1 c25b6c8a"),
    (
        "FS-31",
        false,
        true,
        "v3angle 1 1 1 42840000 1 1 1 428a0000",
    ),
    ("FS-32", false, true, "pclosest 1 12"),
    ("FS-33", false, true, "mdet 1 0 0"),
    (
        "FS-34",
        false,
        false,
        "powell 1 2 c0000000 00000000 42800000 3f800000 3f000000 bf60a95d 3f45d0f8 3f800000 3f800000 0 3a83126f 0",
    ),
    (
        "FS-37",
        false,
        false,
        "dfpmin 1 2 3f800000 00000000 42800000 40400000 40000000 bd2b3aa0 3face27e bef13273 4012d3c9 3a83126f 49742400 0 0 1",
    ),
    (
        "FS-38",
        false,
        true,
        "dfpmin 1 2 00000000 00000000 40800000 40000000 3f000000 bf7434ff bf3a9928 3fbd48ff 3f23ce3f 3dcccccd bf800000 0 1 1",
    ),
    ("FS-40", false, true, "mcond 1 2 1 9c708f20 42ac0000"),
    (
        "FS-43",
        false,
        true,
        "msvd 1 2 1 9c708f20 42ac0000 1 1 1 42820000 new",
    ),
    (
        "FS-44",
        false,
        true,
        "msvdinverse 1 2 1 9c708f20 42ac0000 new",
    ),
    (
        "FS-45",
        false,
        false,
        "mpinverse 1 1 2 b566e401 c171c20c new",
    ),
    (
        "FS-46",
        false,
        true,
        "msvdpinverse 1 2 2 c2b3feb9 ffc00000 424d8a56 41800000 new",
    ),
    (
        "FS-47",
        false,
        true,
        "oeigen 2 7f800000 c0800000 c0800000 41000000",
    ),
    (
        "FS-48",
        false,
        true,
        "mnscond 1 1 3 42600000 6fa02ee8 428f757b",
    ),
    // FS-48 where VNL never returns: refused in both modes
    (
        "FS-48",
        false,
        false,
        "mcond 1 3 3 80000000 ff800000 3f800000 80000000 7fc00000 7f800000 80000000 7fc00000 7fc00000",
    ),
    ("FS-49", false, true, "mreadbin x00 new"),
    (
        "FS-50",
        false,
        true,
        "habs 3 3 80000000 40a00000 40e00000 40a00000 40c00000 40e00000 7fc12345 40e00000 ffc00000 new",
    ),
    (
        "FS-51",
        false,
        true,
        "hrobust 20 20 7f800000 40a00000 41c00000 40a00000 40c00000 40e00000 41000000 41100000 41200000 41300000 41400000 41500000 41600000 41700000 41800000 41880000 41900000 41980000 41a00000 41a80000 41b00000 41b80000 41c00000 3fa88600 405d3631 40f77076 416bda7a 41bf920b 42049861 421c6984 421d3a51 4206afe8 41c4b9a8 4175b391 41063b78 409555ec 408d9269 4102b6a3 418791b3 41fad5f9 42474267 42873c0c 429cba9e 3feccccccccccccd",
    ),
    ("GLIBC-07", false, false, "mrotation 3 c2bea1b3 3 new"),
    (
        "ITK-105",
        false,
        true,
        "dfpmin 0 3 00000000 00000000 41800000 3f000000 40400000 40000000 bffbc660 3f564f13 bee6b526 403292d6 40226854 3fb1abdf 3dcccccd 49742400 1 1 1",
    ),
    (
        "ITK-106",
        false,
        true,
        "dfpmin 3 3 3f800000 00000000 41800000 40000000 3f000000 3f800000 3fcac77e be953de5 bf8ec246 400466ed bf3df9b0 bf0525b4 3dcccccd 42480000 1 0 1",
    ),
    (
        "ITK-110",
        false,
        false,
        "oeigenns 2 c0e00000 3a035bcf dd02de58 421b0418",
    ),
    (
        "ITK-112",
        false,
        true,
        "oeigenns 4 41d80000 ffd00001 41500000 41c00000 c0800000 41b00000 00000000 41a00000 41e80000 c0400000 40400000 41100000 41600000 41c80000 41980000 41d00000",
    ),
    (
        "FS-26",
        false,
        true,
        "mrealregion 2 1 1 3f800000 40000000 2 1 1 40400000 40800000 1 1 1 1 1 1",
    ),
];

/// The line in `mode`: its result and streams without the notices, and the IDs noted.
fn run(line: &str, old: bool, mode: UpstreamDefects) -> (String, Vec<String>) {
    let mut env = Environment::new();
    if old {
        env.set(b"FREESURFER_MatrixAlloc_old", b"1");
    }
    let toks: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
    let r = maths_cmd::run_line(&toks, mode, &env);
    let (res, streams) = r.rsplit_once(" | ").expect("a result line");
    let (out, err) = streams.split_once(' ').expect("two streams");
    let err: Vec<u8> = (1..err.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&err[i..i + 2], 16).expect("hex"))
        .collect();
    let err = String::from_utf8_lossy(&err).into_owned();
    let mut ids = Vec::new();
    let mut kept = String::new();
    for l in err.split_inclusive('\n') {
        match l.strip_prefix("upstream defect ") {
            Some(rest) => ids.push(rest.split(' ').next().unwrap_or("").to_owned()),
            None => kept.push_str(l),
        }
    }
    (format!("{res} | {out} {kept}"), ids)
}

#[test]
fn each_defect_is_noted_in_both_modes() {
    for &(id, old, differs, line) in CASES {
        let (fix, fix_ids) = run(line, old, UpstreamDefects::Fix);
        let (rep, rep_ids) = run(line, old, UpstreamDefects::Reproduce);
        assert!(
            fix_ids.iter().any(|i| i == id),
            "{id} not noted in fix: {line}"
        );
        assert!(
            rep_ids.iter().any(|i| i == id),
            "{id} not noted in reproduce: {line}"
        );
        assert_eq!(fix != rep, differs, "{id}: the modes' results {line}");
    }
}

/// FS-47 where VNL never returns (`[[1, NaN], [NaN, 2]]`): refused in both modes, nothing written;
/// where it returns (`[[inf, -4], [-4, 8]]`): VNL's values with `reproduce`, refused by default.
#[test]
fn fs47_never_returns_refused_in_both_modes() {
    let never = [1.0, f32::NAN, f32::NAN, 2.0];
    let returns = [f32::INFINITY, -4.0, -4.0, 8.0];
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut p = Process::silent(Environment::new(), mode);
        let e = p.open_eigen_system(&never, 2);
        assert_eq!((e.status, e.values, e.vectors), (-4, None, None));
        assert!(p.notices().contains(&larmorx_core::defects::FS_47));
        let e = p.open_eigen_system(&returns, 2);
        if mode == UpstreamDefects::Fix {
            assert_eq!(e.status, -4);
        } else {
            assert_eq!(e.status, 0);
            assert_eq!(e.values, Some(vec![8.0, f32::INFINITY]));
            assert_eq!(e.vectors, Some(vec![0.0, 1.0, 1.0, 0.0]));
        }
    }
}

/// FS-29: the time-of-day seed is 1 by default, the clock's with `reproduce`; noted either way.
#[test]
fn fs29_time_of_day_seed() {
    let mut p = Process::silent(Environment::new(), UpstreamDefects::Fix);
    assert_eq!(p.pdf_tod_seed(), 1);
    assert!(p.notices().contains(&larmorx_core::defects::FS_29));
    let mut p = Process::silent(Environment::new(), UpstreamDefects::Reproduce);
    assert!(p.pdf_tod_seed() > 1_000_000_000);
    assert!(p.notices().contains(&larmorx_core::defects::FS_29));
}

/// FS-35 and FS-36 are kept in both modes (not noted): Powell's iteration limit gives the same
/// results, and OpenPowell's tolerance changes nothing.
#[test]
fn fs35_fs36_kept() {
    let line = "powell2 0 2 00000000 00000000 41800000 3f000000 3f800000 bf8b3143 bf30f3bc \
                bf93045e bfb9d52e 1 3f800000 00000000 00000000 3f800000 3c23d70a 38d1b717 1 0 0";
    let (fix, fix_ids) = run(line, false, UpstreamDefects::Fix);
    let (rep, rep_ids) = run(line, false, UpstreamDefects::Reproduce);
    assert_eq!(fix, rep);
    assert!(
        fix_ids.is_empty() && rep_ids.is_empty(),
        "{fix_ids:?} {rep_ids:?}"
    );
    assert!(fix.starts_with("0 1 "), "{fix}");
    let a = "powell 0 2 00000000 00000000 41800000 3f000000 3f800000 bf8b3143 bf30f3bc \
             bf93045e bfb9d52e 0";
    let (t1, _) = run(&format!("{a} 3a83126f 0"), false, UpstreamDefects::Fix);
    let (t2, _) = run(&format!("{a} 3f800000 0"), false, UpstreamDefects::Fix);
    assert_eq!(t1, t2);
}

/// FS-39's evaluation limit, at the driver: a pass that reaches it fails with
/// FAILED_USER_REQUEST, its best point kept by default.
#[test]
fn fs39_evaluation_limit_at_the_driver() {
    use larmorx_core::vnl_minimize::ReturnCode;
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut p = Process::silent(Environment::new(), mode);
        let mut d = LbfgsDriver::new(2);
        d.maxfev = 3;
        let mut compute = |x: &[f64], g: &mut [f64]| {
            g[0] = 2.0 * (x[0] - 3.0);
            g[1] = 20.0 * (x[1] + 1.0);
            (x[0] - 3.0) * (x[0] - 3.0) + 10.0 * (x[1] + 1.0) * (x[1] + 1.0)
        };
        let mut seen = 0;
        let mut observer = |_: f64, _: &[f64]| seen += 1;
        let mut x = [0.0, 0.0];
        let pass = d.minimize(&mut p, &mut compute, &mut x, &mut observer);
        assert!(!pass.ok);
        assert_eq!(d.failure_code, ReturnCode::FailedUserRequest);
        assert_eq!(d.num_evaluations, 4);
        assert!(seen > 0);
        assert!(d.end_error < 19.0, "{}", d.end_error);
    }
}
