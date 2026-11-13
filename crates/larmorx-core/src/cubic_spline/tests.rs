// SPDX-License-Identifier: Apache-2.0
//! Tests of the cubic spline's clean room: values the probe `fs_spline_probe` gave for the same
//! calls (black-box runs of FreeSurfer's functions), FreeSurfer's messages, and golden checksums
//! of the clean room's own outputs, one per path (`specs/burkardt-spline.md` §9.5).

use super::*;
use crate::ranlux::sha256::Sha256;

const FIX: UpstreamDefects = UpstreamDefects::Fix;
const REPRODUCE: UpstreamDefects = UpstreamDefects::Reproduce;

fn f(bits: u32) -> f32 {
    f32::from_bits(bits)
}

fn set(
    t: &[f32],
    y: &[f32],
    ibc: (i32, i32),
    ends: (f32, f32),
    mode: UpstreamDefects,
) -> Result<Vec<f32>, SplineError> {
    second_derivatives(t, y, ibc.0, ends.0, ibc.1, ends.1, &mut Notices::new(mode))
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// The probe's `set1` and `cases` results.
#[test]
fn the_probes_values() {
    let t = [0.0, 1.0, 2.0];
    let y = [5.0, 6.0, 7.0];
    let m = set(&t, &y, (1, 1), (0.0, 0.0), FIX).unwrap();
    assert_eq!(bits(&m), [0x4040_0000, 0, 0xc040_0000]);
    let m0 = set(&t, &y, (0, 0), (0.0, 0.0), FIX).unwrap();
    assert_eq!(bits(&m0), [0, 0, 0]);
    // cases: n = 3, conditions 1 and 1, x = 0.5
    let e = evaluate(&t, &y, &m, 0.5);
    assert_eq!(e.value.to_bits(), 0x40aa_0000);
    assert_eq!(e.first.map(f32::to_bits), Some(0x3f90_0000));
    assert_eq!(e.second.map(f32::to_bits), Some(0x3fc0_0000));
    // cases: n = 2, t = (1, 5), y = (6, 0), conditions 1 and 1
    let (t2, y2) = ([1.0, 5.0], [6.0, 0.0]);
    let m2 = set(&t2, &y2, (1, 1), (0.0, 0.0), FIX).unwrap();
    assert_eq!(bits(&m2), [0xc010_0000, 0x4010_0000]);
    let below = evaluate(&t2, &y2, &m2, 0.0);
    assert_eq!(
        (below.value.to_bits(), below.first, below.second),
        (0x40c0_0000, None, None)
    );
    let inside = evaluate(&t2, &y2, &m2, 2.0);
    assert_eq!(inside.value.to_bits(), 0x40a2_0000);
    assert_eq!(inside.first.map(f32::to_bits), Some(0xbfd8_0000));
    assert_eq!(inside.second.map(f32::to_bits), Some(0xbf90_0000));
    let above = evaluate(&t2, &y2, &m2, 6.0);
    assert_eq!(
        (
            above.value.to_bits(),
            above.first.map(f32::to_bits),
            above.second
        ),
        (0, Some(0), None)
    );
    // the wrappers take the end values as first derivatives: conditions 2 and 2 with (1, 2)
    let m22 = set(&t2, &y2, (2, 2), (1.0, 2.0), FIX).unwrap();
    assert_eq!(bits(&m22), [0x3f80_0000, 0x4000_0000]);
    let w = open_spline(&t2, &y2, 1.0, 2.0, &mut Notices::new(FIX)).unwrap();
    assert_eq!(bits(&w), [0xc088_0000, 0x4098_0000]);
    assert_eq!(open_splint(&t2, &y2, &m2, 2.0).to_bits(), 0x40a2_0000);
}

/// The probe's `d3`: band systems, a zero diagonal refused, one that becomes 0, NaNs.
#[test]
fn the_probes_systems() {
    let x = solve_tridiagonal(&[0.0, 3.0], &[2.0, 5.0], &[4.0, 0.0], &[7.0, 8.0]).unwrap();
    assert_eq!(bits(&x), [0xbfc0_0000, 0x4020_0000]);
    let x = solve_tridiagonal(
        &[0.0, 3.0, 6.0],
        &[2.0, 5.0, 8.0],
        &[4.0, 7.0, 0.0],
        &[10.0, 11.0, 12.0],
    )
    .unwrap();
    assert_eq!(bits(&x), [0x3eb8_51e0, 0x4014_7ae2, 0xbe75_c28f]);
    let x = solve_tridiagonal(&[1.0, 1.0], &[1.0, 1.0], &[1.0, 1.0], &[2.0, 3.0]).unwrap();
    assert_eq!(bits(&x), [0xff80_0000, 0x7f80_0000]);
    assert!(solve_tridiagonal(&[0.0, 3.0], &[0.0, 5.0], &[4.0, 0.0], &[7.0, 8.0]).is_none());
    assert!(solve_tridiagonal(&[0.0, 3.0], &[2.0, -0.0], &[4.0, 0.0], &[7.0, 8.0]).is_none());
    let x = solve_tridiagonal(
        &[0.0, 3.0],
        &[f(0x7fc0_0001), f(0x7fc0_0002)],
        &[4.0, 0.0],
        &[7.0, 8.0],
    )
    .unwrap();
    assert_eq!(bits(&x), [0x7fc0_0001, 0x7fc0_0001]);
    // §6's example: U₀ and x₁ are NaNs of different payloads; the back substitution's product
    // is x₁ · U₀, so x₀ carries x₁'s payload (`2 00000000 40000000 40400000 7fc00111 40a00000
    // 00000000 40e00000 7fc00222` gives `x 7fc00222 7fc00222`)
    let x = solve_tridiagonal(
        &[0.0, 3.0],
        &[2.0, 5.0],
        &[f(0x7fc0_0111), 0.0],
        &[7.0, f(0x7fc0_0222)],
    )
    .unwrap();
    assert_eq!(bits(&x), [0x7fc0_0222, 0x7fc0_0222]);
    let x = solve_tridiagonal(&[0.0], &[2.0], &[3.0], &[7.0]).unwrap();
    assert_eq!(bits(&x), [0x4060_0000]);
}

/// FreeSurfer's five messages (the probe's `set1`, standard output byte for byte).
#[test]
fn messages() {
    let head = "\nSPLINE_CUBIC_SET - Fatal error!\n";
    let e1 = set(&[0.0], &[5.0], (0, 0), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e1.message(),
        format!(
            "{head}  The number of data points N must be at least 2.\n  The input value is 1.\n"
        )
    );
    let e2 = set(&[0.0, 1.0, 1.0], &[5.0, 6.0, 7.0], (1, 1), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e2.message(),
        format!("{head}  The knots must be strictly increasing, but\n  T(1) = 1\n  T(2) = 1\n")
    );
    let e3 = set(&[0.0, 1.0, 2.0], &[5.0, 6.0, 7.0], (3, 0), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e3.message(),
        format!("{head}  IBCBEG must be 0, 1 or 2.\n  The input value is 3.\n")
    );
    // E3 before E4
    let e34 = set(&[0.0, 1.0, 2.0], &[5.0, 6.0, 7.0], (-1, 7), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(e34, SplineError::BadBegin { ibcbeg: -1 });
    let e4 = set(&[0.0, 1.0, 2.0], &[5.0, 6.0, 7.0], (0, 5), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e4.message(),
        format!("{head}  IBCEND must be 0, 1 or 2.\n  The input value is 5.\n")
    );
    let e5 = set(&[0.0, f(1)], &[5.0, 6.0], (1, 1), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e5.message(),
        format!("{head}  The linear system could not be solved.\n")
    );
    assert_eq!(EXIT_STATUS, 1);
    // the knots' numbers as %g
    let e = set(&[f(1), -f(1)], &[0.0, 0.0], (1, 1), (0.0, 0.0), FIX).unwrap_err();
    assert!(
        e.message()
            .ends_with("T(0) = 1.4013e-45\n  T(1) = -1.4013e-45\n"),
        "{e}"
    );
    let e = set(&[f32::INFINITY, 0.0], &[0.0, 0.0], (1, 1), (0.0, 0.0), FIX).unwrap_err();
    assert!(e.message().ends_with("T(0) = inf\n  T(1) = 0\n"), "{e}");
}

/// FS-103: a NaN knot is E2 by default, FreeSurfer's acceptance with `reproduce`.
#[test]
fn fs103_nan_knot() {
    let t = [0.0, f32::NAN, 2.0];
    let y = [5.0, 6.0, 7.0];
    let e = set(&t, &y, (1, 1), (0.0, 0.0), FIX).unwrap_err();
    assert_eq!(
        e.message(),
        "\nSPLINE_CUBIC_SET - Fatal error!\n  The knots must be strictly increasing, but\n  T(0) = 0\n  T(1) = nan\n"
    );
    let mut notices = Notices::new(REPRODUCE);
    let m = second_derivatives(&t, &y, 1, 0.0, 1, 0.0, &mut notices).unwrap();
    assert_eq!(bits(&m), [0x7fc0_0000; 3]);
    assert_eq!(notices.as_slice()[0].defect.id, "FS-103");
    // a NaN in the last place (the probe: the loop's last comparison)
    let e = set(&[0.0, 1.0, -f32::NAN], &y, (0, 0), (0.0, 0.0), FIX).unwrap_err();
    assert!(e.message().ends_with("T(1) = 1\n  T(2) = -nan\n"), "{e}");
    let mut notices = Notices::new(FIX);
    let _ = set(&[0.0, 1.0, 2.0], &y, (1, 1), (0.0, 0.0), FIX).unwrap();
    assert!(second_derivatives(&[0.0, 1.0], &[0.0, 1.0], 1, 0.0, 1, 0.0, &mut notices).is_ok());
    assert!(notices.is_empty());
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

/// One golden case's text: second derivatives, then each evaluation (value and derivatives, `-`
/// where none is written), as hex bits.
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
                let ev = evaluate(&t, &y, &m, x);
                let opt =
                    |v: Option<f32>| v.map_or("-".to_string(), |v| format!("{:08x}", v.to_bits()));
                s.push_str(&format!(
                    "{:08x} {} {}\n",
                    ev.value.to_bits(),
                    opt(ev.first),
                    opt(ev.second)
                ));
            }
        }
        Err(err) => s.push_str(&err.message()),
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

/// The golden checksums, recorded on Linux x86-64 (`cargo test -p larmorx-core cubic_spline`
/// prints a changed or missing one).
const GOLDEN: &[(&str, &str)] = &[
    (
        "n=12 ibc=0,0",
        "d30ebb4763f96551043ee44ae505b0c695fc59187fb326e12d2f6999ff71955f",
    ),
    (
        "n=12 ibc=0,1",
        "43b7b42ba593fafea4095c0087456bd931531a0c1d35514d19fffe8f1b4f4013",
    ),
    (
        "n=12 ibc=0,2",
        "3452b6c381dbb965d61a4d2e08599ce421b70058296cb9c8aa2e6d275d31c3bf",
    ),
    (
        "n=12 ibc=1,0",
        "e67bd5429899a339ee78f51d239df9b65f9ac25979997a54c8dd352cbf8f5d0c",
    ),
    (
        "n=12 ibc=1,1",
        "5990565e73128e55446098ec8225aed1b51a805d304cf5485e3c7c72f92bffae",
    ),
    (
        "n=12 ibc=1,2",
        "1d5b4e0e55b46d698ba717bed35ba865d899531abf35547ee878dad906a99ebe",
    ),
    (
        "n=12 ibc=2,0",
        "d2cd3c8198e5c51a95eaf839f4f26e02dfe6c93b091ece728afd222547be31df",
    ),
    (
        "n=12 ibc=2,1",
        "e01c5aa60465181d43131acd833a59a34e4f24069e809b0ff8512db6ea023f18",
    ),
    (
        "n=12 ibc=2,2",
        "088d7c2658061f246a57c876c157d55510a69df54f1b42eeaaf575bb3dd01768",
    ),
    (
        "n=2 ibc=0,0",
        "07c5a2ed9d81c3331655b57736f7393c8832817c611cd1394f9e49890888e188",
    ),
    (
        "n=2 ibc=1,1",
        "65a881d2d1d79f6a24ef9fc625d2b0ee97227e2939c65370e10847b8dcc3b0f9",
    ),
    (
        "n=80 ibc=1,1",
        "41b94224283bbdfe864f75814d93de8a57d80039a3b7c75a26ecce8dede23630",
    ),
];

#[test]
fn golden() {
    let mut bad = Vec::new();
    for (name, text) in golden_cases() {
        let mut h = Sha256::new();
        h.update(text.as_bytes());
        let got = h.hex();
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
