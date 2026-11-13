// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Tests of `fs.spline`: the values the probe `fs_spline_probe` gave for the same calls (FreeSurfer's
//! compiled functions), FreeSurfer's messages and exit status, FS-103, and golden checksums, one per
//! path. The golden cases are larmorx's clean room's (`larmorx_core::cubic_spline`'s tests, the same
//! texts), so their checksums are the clean room's: every platform checks that the replica and the
//! clean room give the same bits there.

use sha2::{Digest, Sha256};

use super::*;
use larmorx_core::UpstreamDefects;

const FIX: UpstreamDefects = UpstreamDefects::Fix;
const REPRODUCE: UpstreamDefects = UpstreamDefects::Reproduce;
/// The value the probe puts in the derivative outputs before each evaluation.
const SENTINEL: u32 = 0x7fc0_0bad;

fn f(bits: u32) -> f32 {
    f32::from_bits(bits)
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// `SplineCubicSet` on `t` and `y`: the second derivatives, or the text it printed.
fn set(
    t: &[f32],
    y: &[f32],
    ibc: (i32, i32),
    ends: (f32, f32),
    mode: UpstreamDefects,
) -> Result<Vec<f32>, String> {
    let mut g = Globals::new(mode);
    let mut out = Vec::new();
    spline_cubic_set(
        &mut g,
        &mut out,
        t.len() as i32,
        t,
        y,
        ibc.0,
        ends.0,
        ibc.1,
        ends.1,
    )
    .map_err(|e| {
        assert_eq!(e, Fatal::Exit(1));
        String::from_utf8(out).unwrap()
    })
}

/// `SplineCubicValue` at `x`: the value and the two derivative outputs (`None` where unwritten).
fn value(t: &[f32], y: &[f32], m: &[f32], x: f32) -> (f32, Option<f32>, Option<f32>) {
    let (mut d1, mut d2) = (f(SENTINEL), f(SENTINEL));
    let v = spline_cubic_value(t.len() as i32, t, x, y, m, &mut d1, &mut d2);
    let w = |d: f32| (d.to_bits() != SENTINEL).then_some(d);
    (v, w(d1), w(d2))
}

/// The probe's `set1` and `cases` results.
#[test]
fn the_probes_values() {
    let t = [0.0, 1.0, 2.0];
    let y = [5.0, 6.0, 7.0];
    let m = set(&t, &y, (1, 1), (0.0, 0.0), FIX).unwrap();
    assert_eq!(bits(&m), [0x4040_0000, 0, 0xc040_0000]);
    assert_eq!(
        bits(&set(&t, &y, (0, 0), (0.0, 0.0), FIX).unwrap()),
        [0, 0, 0]
    );
    let (v, d1, d2) = value(&t, &y, &m, 0.5);
    assert_eq!(
        (v.to_bits(), d1.map(f32::to_bits), d2.map(f32::to_bits)),
        (0x40aa_0000, Some(0x3f90_0000), Some(0x3fc0_0000))
    );
    let (t2, y2) = ([1.0, 5.0], [6.0, 0.0]);
    let m2 = set(&t2, &y2, (1, 1), (0.0, 0.0), FIX).unwrap();
    assert_eq!(bits(&m2), [0xc010_0000, 0x4010_0000]);
    assert_eq!(value(&t2, &y2, &m2, 0.0), (6.0, None, None));
    let (v, d1, d2) = value(&t2, &y2, &m2, 2.0);
    assert_eq!(
        (v.to_bits(), d1.map(f32::to_bits), d2.map(f32::to_bits)),
        (0x40a2_0000, Some(0xbfd8_0000), Some(0xbf90_0000))
    );
    let (v, d1, d2) = value(&t2, &y2, &m2, 6.0);
    assert_eq!((v.to_bits(), d1.map(f32::to_bits), d2), (0, Some(0), None));
    let m22 = set(&t2, &y2, (2, 2), (1.0, 2.0), FIX).unwrap();
    assert_eq!(bits(&m22), [0x3f80_0000, 0x4000_0000]);
    // the wrappers take the end values as first derivatives
    let mut g = Globals::new(FIX);
    let mut w = [0.0f32; 2];
    open_spline(&mut g, &mut Vec::new(), &t2, &y2, 2, 1.0, 2.0, &mut w).unwrap();
    assert_eq!(bits(&w), [0xc088_0000, 0x4098_0000]);
    let mut o = 0.0;
    open_splint(&t2, &y2, &m2, 2, 2.0, &mut o);
    assert_eq!(o.to_bits(), 0x40a2_0000);
    // one knot and a NaN point: the point, quieted (FreeSurfer reads before its arrays)
    let (v, d1, d2) = value(&[1.0], &[2.0], &[0.0], f(0x7f80_0001));
    assert_eq!((v.to_bits(), d1, d2), (0x7fc0_0001, None, None));
}

/// The probe's `d3`: band systems, a zero diagonal refused, one that becomes 0, NaNs.
#[test]
fn the_probes_systems() {
    // band storage: column j holds U[j - 1], D[j], L[j + 1]
    let d3 = |l: &[f32], d: &[f32], u: &[f32], b: &[f32]| {
        let n = d.len();
        let mut a = vec![0.0f32; 3 * n];
        for j in 0..n {
            a[3 * j] = if j > 0 { u[j - 1] } else { 0.0 };
            a[3 * j + 1] = d[j];
            a[3 * j + 2] = if j + 1 < n { l[j + 1] } else { 0.0 };
        }
        d3_np_fs(&mut a, b)
    };
    let x = d3(&[0.0, 3.0], &[2.0, 5.0], &[4.0, 0.0], &[7.0, 8.0]).unwrap();
    assert_eq!(bits(&x), [0xbfc0_0000, 0x4020_0000]);
    let x = d3(
        &[0.0, 3.0, 6.0],
        &[2.0, 5.0, 8.0],
        &[4.0, 7.0, 0.0],
        &[10.0, 11.0, 12.0],
    )
    .unwrap();
    assert_eq!(bits(&x), [0x3eb8_51e0, 0x4014_7ae2, 0xbe75_c28f]);
    let x = d3(&[1.0, 1.0], &[1.0, 1.0], &[1.0, 1.0], &[2.0, 3.0]).unwrap();
    assert_eq!(bits(&x), [0xff80_0000, 0x7f80_0000]);
    assert!(d3(&[0.0, 3.0], &[0.0, 5.0], &[4.0, 0.0], &[7.0, 8.0]).is_none());
    assert!(d3(&[0.0, 3.0], &[2.0, -0.0], &[4.0, 0.0], &[7.0, 8.0]).is_none());
    let x = d3(
        &[0.0, 3.0],
        &[f(0x7fc0_0001), f(0x7fc0_0002)],
        &[4.0, 0.0],
        &[7.0, 8.0],
    )
    .unwrap();
    assert_eq!(bits(&x), [0x7fc0_0001, 0x7fc0_0001]);
    let x = d3(&[0.0], &[2.0], &[3.0], &[7.0]).unwrap();
    assert_eq!(bits(&x), [0x4060_0000]);
    assert_eq!(d3_np_fs(&mut [], &[]), Some(Vec::new()));
}

/// FreeSurfer's five messages (the probe's `set1`: standard output byte for byte, exit status 1).
#[test]
fn messages() {
    let head = "\nSPLINE_CUBIC_SET - Fatal error!\n";
    let e1 = set(&[0.0], &[5.0], (0, 0), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e1,
        format!(
            "{head}  The number of data points N must be at least 2.\n  The input value is 1.\n"
        )
    );
    let mut out = Vec::new();
    let neg = spline_cubic_set(
        &mut Globals::new(FIX),
        &mut out,
        -3,
        &[],
        &[],
        1,
        0.0,
        1,
        0.0,
    );
    assert_eq!(neg, Err(Fatal::Exit(1)));
    assert!(
        String::from_utf8(out)
            .unwrap()
            .ends_with("The input value is -3.\n")
    );
    let e2 = set(&[0.0, 1.0, 1.0], &[5.0, 6.0, 7.0], (1, 1), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e2,
        format!("{head}  The knots must be strictly increasing, but\n  T(1) = 1\n  T(2) = 1\n")
    );
    let e3 = set(&[0.0, 1.0, 2.0], &[5.0, 6.0, 7.0], (3, 0), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e3,
        format!("{head}  IBCBEG must be 0, 1 or 2.\n  The input value is 3.\n")
    );
    let e34 = set(&[0.0, 1.0, 2.0], &[5.0, 6.0, 7.0], (-1, 7), (0.0, 0.0), FIX).unwrap_err();
    assert!(e34.contains("IBCBEG") && e34.contains("is -1."), "{e34}");
    let e4 = set(&[0.0, 1.0, 2.0], &[5.0, 6.0, 7.0], (0, 5), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e4,
        format!("{head}  IBCEND must be 0, 1 or 2.\n  The input value is 5.\n")
    );
    let e5 = set(&[0.0, f(1)], &[5.0, 6.0], (1, 1), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e5,
        format!("{head}  The linear system could not be solved.\n")
    );
    let e = set(&[f(1), -f(1)], &[0.0, 0.0], (1, 1), (0.0, 0.0), FIX).unwrap_err();
    assert!(
        e.ends_with("T(0) = 1.4013e-45\n  T(1) = -1.4013e-45\n"),
        "{e}"
    );
    let e = set(&[f32::INFINITY, 0.0], &[0.0, 0.0], (1, 1), (0.0, 0.0), FIX).unwrap_err();
    assert!(e.ends_with("T(0) = inf\n  T(1) = 0\n"), "{e}");
}

/// FS-103: a NaN knot is the non-increasing knots' error by default, FreeSurfer's acceptance with
/// `reproduce`.
#[test]
fn fs103_nan_knot() {
    let t = [0.0, f32::NAN, 2.0];
    let y = [5.0, 6.0, 7.0];
    let e = set(&t, &y, (1, 1), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e,
        "\nSPLINE_CUBIC_SET - Fatal error!\n  The knots must be strictly increasing, but\n  T(0) = 0\n  T(1) = nan\n"
    );
    let mut g = Globals::new(REPRODUCE);
    let m = spline_cubic_set(&mut g, &mut Vec::new(), 3, &t, &y, 1, 0.0, 1, 0.0).unwrap();
    assert_eq!(bits(&m), [0x7fc0_0000; 3]);
    assert_eq!(g.notices.as_slice()[0].defect.id, "FS-103");
    let e = set(&[0.0, 1.0, -f32::NAN], &y, (0, 0), (0.0, 0.0), FIX).unwrap_err();
    assert!(e.ends_with("T(1) = 1\n  T(2) = -nan\n"), "{e}");
    let mut g = Globals::new(FIX);
    assert!(
        spline_cubic_set(
            &mut g,
            &mut Vec::new(),
            2,
            &[0.0, 1.0],
            &[0.0, 1.0],
            1,
            0.0,
            1,
            0.0
        )
        .is_ok()
    );
    assert!(g.notices.is_empty());
}

/// A seeded generator of plain arithmetic for the golden inputs (no transcendental function).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    /// A value in [-50, 50) on a grid of 1/64.
    fn value(&mut self) -> f32 {
        (self.next() % 6400) as f32 / 64.0 - 50.0
    }
}

/// One golden case's text, as the clean room's: second derivatives, then each evaluation (value
/// and derivatives, `-` where none is written), as hex bits.
fn case_text(n: usize, ibc: (i32, i32), seed: u64) -> String {
    let mut g = Lcg(seed);
    let mut t = Vec::with_capacity(n);
    let mut acc = g.value();
    for _ in 0..n {
        t.push(acc);
        acc += 0.25 + (g.next() % 400) as f32 / 16.0;
    }
    let y: Vec<f32> = (0..n).map(|_| g.value()).collect();
    let (b, e) = (g.value() / 8.0, g.value() / 8.0);
    let mut s = String::new();
    match set(&t, &y, ibc, (b, e), FIX) {
        Ok(m) => {
            for v in &m {
                s.push_str(&format!("{:08x} ", v.to_bits()));
            }
            s.push('\n');
            let mut xs = vec![t[0] - 1.0, t[0], t[n - 1], t[n - 1] + 3.0, f32::NAN];
            for w in t.windows(2) {
                xs.push(w[0] + (w[1] - w[0]) * 0.375);
            }
            for x in xs {
                let (v, d1, d2) = value(&t, &y, &m, x);
                let opt =
                    |v: Option<f32>| v.map_or("-".to_string(), |v| format!("{:08x}", v.to_bits()));
                s.push_str(&format!("{:08x} {} {}\n", v.to_bits(), opt(d1), opt(d2)));
            }
        }
        Err(err) => s.push_str(&err),
    }
    s
}

fn golden_cases() -> Vec<(String, String)> {
    let mut cases = Vec::new();
    for ibcbeg in 0..3 {
        for ibcend in 0..3 {
            cases.push((
                format!("n=12 ibc={ibcbeg},{ibcend}"),
                case_text(
                    12,
                    (ibcbeg, ibcend),
                    100 + 3 * ibcbeg as u64 + ibcend as u64,
                ),
            ));
        }
    }
    cases.push(("n=2 ibc=0,0".into(), case_text(2, (0, 0), 7)));
    cases.push(("n=2 ibc=1,1".into(), case_text(2, (1, 1), 8)));
    cases.push(("n=80 ibc=1,1".into(), case_text(80, (1, 1), 9)));
    cases
}

/// The golden checksums: the clean room's, recorded on Linux x86-64
/// (`crates/larmorx-core/src/cubic_spline/tests.rs`).
#[rustfmt::skip]
const GOLDEN: &[(&str, &str)] = &[
    ("n=12 ibc=0,0", "d30ebb4763f96551043ee44ae505b0c695fc59187fb326e12d2f6999ff71955f"),
    ("n=12 ibc=0,1", "43b7b42ba593fafea4095c0087456bd931531a0c1d35514d19fffe8f1b4f4013"),
    ("n=12 ibc=0,2", "3452b6c381dbb965d61a4d2e08599ce421b70058296cb9c8aa2e6d275d31c3bf"),
    ("n=12 ibc=1,0", "e67bd5429899a339ee78f51d239df9b65f9ac25979997a54c8dd352cbf8f5d0c"),
    ("n=12 ibc=1,1", "5990565e73128e55446098ec8225aed1b51a805d304cf5485e3c7c72f92bffae"),
    ("n=12 ibc=1,2", "1d5b4e0e55b46d698ba717bed35ba865d899531abf35547ee878dad906a99ebe"),
    ("n=12 ibc=2,0", "d2cd3c8198e5c51a95eaf839f4f26e02dfe6c93b091ece728afd222547be31df"),
    ("n=12 ibc=2,1", "e01c5aa60465181d43131acd833a59a34e4f24069e809b0ff8512db6ea023f18"),
    ("n=12 ibc=2,2", "088d7c2658061f246a57c876c157d55510a69df54f1b42eeaaf575bb3dd01768"),
    ("n=2 ibc=0,0", "07c5a2ed9d81c3331655b57736f7393c8832817c611cd1394f9e49890888e188"),
    ("n=2 ibc=1,1", "65a881d2d1d79f6a24ef9fc625d2b0ee97227e2939c65370e10847b8dcc3b0f9"),
    ("n=80 ibc=1,1", "41b94224283bbdfe864f75814d93de8a57d80039a3b7c75a26ecce8dede23630"),
];

#[test]
fn golden() {
    let mut bad = Vec::new();
    for (name, text) in golden_cases() {
        let got: String = Sha256::digest(text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        match GOLDEN.iter().find(|(n, _)| *n == name) {
            Some((_, want)) if *want == got => {}
            _ => bad.push(format!("    (\"{name}\", \"{got}\"),")),
        }
    }
    assert_eq!(
        GOLDEN.len(),
        golden_cases().len(),
        "cases:\n{}",
        bad.join("\n")
    );
    assert!(bad.is_empty(), "changed:\n{}", bad.join("\n"));
}
