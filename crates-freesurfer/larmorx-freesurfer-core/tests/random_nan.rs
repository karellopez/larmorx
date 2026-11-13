// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `randomNumber(low, high)` with infinities, NaNs and signed zeros as bounds, on every platform:
//!
//! - against the official code: the table below is the unit oracle's output (`fs-random`:
//!   `setRandomSeed(5)` then three draws, printed with C's `%a`, so a NaN shows its sign), made by
//!   `validation/probes/fs-maths/random_number_nan.py` (2026-11-13, track A relay 17): 64 pairs of
//!   eight bounds (0, -0, 1.5, -2, inf, -inf, nan, -nan);
//! - NaN payloads: the bits of 3 draws for each of 4,096 seeded pairs of bounds drawn from a pool
//!   with NaNs of both signs, quiet and signalling, with payloads, hashed (FNV-1a, 64 bits) with the
//!   messages and checked against the hash recorded on Linux x64 (x86-64's NaN rules, the official
//!   build's operand order: `(hi - low)`, then the draw times that, then plus `low`).
//!
//! Run under AArch64's NaN rules (`NAN_TRAP_ARM=1` with `NAN_TRAP_DIR`, the porting guide's
//! emulation) both must give the same answers.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::random::{random_number, set_random_seed};
use larmorx_freesurfer_core::run::Environment;

const RECORDED: u64 = 0x7aa5_c233_49de_2403;

/// The official outputs: (low, high, three draws after `setRandomSeed(5)`).
const OFFICIAL: [(&str, &str, [&str; 3]); 64] = [
    ("0x0p+0", "0x0p+0", ["0x0p+0", "0x0p+0", "0x0p+0"]),
    ("0x0p+0", "-0x0p+0", ["0x0p+0", "0x0p+0", "0x0p+0"]),
    (
        "0x0p+0",
        "0x1.8p+0",
        ["0x1.1d92fdp+0", "0x1.4f981b8p+0", "0x1.6cb336p+0"],
    ),
    (
        "0x0p+0",
        "-0x1p+1",
        ["-0x1.067808p-1", "-0x1.022a18p-2", "-0x1.9bbb8p-4"],
    ),
    ("0x0p+0", "inf", ["inf", "inf", "inf"]),
    ("0x0p+0", "-inf", ["-nan", "-nan", "-nan"]),
    ("0x0p+0", "nan", ["nan", "nan", "nan"]),
    ("0x0p+0", "-nan", ["-nan", "-nan", "-nan"]),
    ("-0x0p+0", "0x0p+0", ["0x0p+0", "0x0p+0", "0x0p+0"]),
    ("-0x0p+0", "-0x0p+0", ["0x0p+0", "0x0p+0", "0x0p+0"]),
    (
        "-0x0p+0",
        "0x1.8p+0",
        ["0x1.1d92fdp+0", "0x1.4f981b8p+0", "0x1.6cb336p+0"],
    ),
    (
        "-0x0p+0",
        "-0x1p+1",
        ["-0x1.067808p-1", "-0x1.022a18p-2", "-0x1.9bbb8p-4"],
    ),
    ("-0x0p+0", "inf", ["inf", "inf", "inf"]),
    ("-0x0p+0", "-inf", ["-nan", "-nan", "-nan"]),
    ("-0x0p+0", "nan", ["nan", "nan", "nan"]),
    ("-0x0p+0", "-nan", ["-nan", "-nan", "-nan"]),
    (
        "0x1.8p+0",
        "0x0p+0",
        ["0x1.1d92fdp+0", "0x1.4f981b8p+0", "0x1.6cb336p+0"],
    ),
    (
        "0x1.8p+0",
        "-0x0p+0",
        ["0x1.1d92fdp+0", "0x1.4f981b8p+0", "0x1.6cb336p+0"],
    ),
    ("0x1.8p+0", "0x1.8p+0", ["0x1.8p+0", "0x1.8p+0", "0x1.8p+0"]),
    (
        "0x1.8p+0",
        "-0x1p+1",
        ["0x1.34adf2p-1", "0x1.0f0d958p+0", "0x1.52f77ep+0"],
    ),
    ("0x1.8p+0", "inf", ["inf", "inf", "inf"]),
    ("0x1.8p+0", "-inf", ["-nan", "-nan", "-nan"]),
    ("0x1.8p+0", "nan", ["nan", "nan", "nan"]),
    ("0x1.8p+0", "-nan", ["-nan", "-nan", "-nan"]),
    (
        "-0x1p+1",
        "0x0p+0",
        ["-0x1.067808p-1", "-0x1.022a18p-2", "-0x1.9bbb8p-4"],
    ),
    (
        "-0x1p+1",
        "-0x0p+0",
        ["-0x1.067808p-1", "-0x1.022a18p-2", "-0x1.9bbb8p-4"],
    ),
    (
        "-0x1p+1",
        "0x1.8p+0",
        ["0x1.34adf2p-1", "0x1.0f0d958p+0", "0x1.52f77ep+0"],
    ),
    ("-0x1p+1", "-0x1p+1", ["-0x1p+1", "-0x1p+1", "-0x1p+1"]),
    ("-0x1p+1", "inf", ["inf", "inf", "inf"]),
    ("-0x1p+1", "-inf", ["-nan", "-nan", "-nan"]),
    ("-0x1p+1", "nan", ["nan", "nan", "nan"]),
    ("-0x1p+1", "-nan", ["-nan", "-nan", "-nan"]),
    ("inf", "0x0p+0", ["inf", "inf", "inf"]),
    ("inf", "-0x0p+0", ["inf", "inf", "inf"]),
    ("inf", "0x1.8p+0", ["inf", "inf", "inf"]),
    ("inf", "-0x1p+1", ["inf", "inf", "inf"]),
    ("inf", "inf", ["-nan", "-nan", "-nan"]),
    ("inf", "-inf", ["-nan", "-nan", "-nan"]),
    ("inf", "nan", ["nan", "nan", "nan"]),
    ("inf", "-nan", ["-nan", "-nan", "-nan"]),
    ("-inf", "0x0p+0", ["-nan", "-nan", "-nan"]),
    ("-inf", "-0x0p+0", ["-nan", "-nan", "-nan"]),
    ("-inf", "0x1.8p+0", ["-nan", "-nan", "-nan"]),
    ("-inf", "-0x1p+1", ["-nan", "-nan", "-nan"]),
    ("-inf", "inf", ["-nan", "-nan", "-nan"]),
    ("-inf", "-inf", ["-nan", "-nan", "-nan"]),
    ("-inf", "nan", ["nan", "nan", "nan"]),
    ("-inf", "-nan", ["-nan", "-nan", "-nan"]),
    ("nan", "0x0p+0", ["nan", "nan", "nan"]),
    ("nan", "-0x0p+0", ["nan", "nan", "nan"]),
    ("nan", "0x1.8p+0", ["nan", "nan", "nan"]),
    ("nan", "-0x1p+1", ["nan", "nan", "nan"]),
    ("nan", "inf", ["nan", "nan", "nan"]),
    ("nan", "-inf", ["nan", "nan", "nan"]),
    ("nan", "nan", ["nan", "nan", "nan"]),
    ("nan", "-nan", ["-nan", "-nan", "-nan"]),
    ("-nan", "0x0p+0", ["-nan", "-nan", "-nan"]),
    ("-nan", "-0x0p+0", ["-nan", "-nan", "-nan"]),
    ("-nan", "0x1.8p+0", ["-nan", "-nan", "-nan"]),
    ("-nan", "-0x1p+1", ["-nan", "-nan", "-nan"]),
    ("-nan", "inf", ["-nan", "-nan", "-nan"]),
    ("-nan", "-inf", ["-nan", "-nan", "-nan"]),
    ("-nan", "nan", ["nan", "nan", "nan"]),
    ("-nan", "-nan", ["-nan", "-nan", "-nan"]),
];

/// `strtod` of the bounds the table uses.
fn parse(s: &str) -> f64 {
    let (neg, t) = match s.strip_prefix('-') {
        Some(t) => (true, t),
        None => (false, s),
    };
    let v = match t {
        "inf" => f64::INFINITY,
        "nan" => f64::from_bits(0x7ff8_0000_0000_0000),
        "0x0p+0" => 0.0,
        "0x1.8p+0" => 1.5,
        "0x1p+1" => 2.0,
        _ => panic!("bound {s}"),
    };
    if neg { -v } else { v }
}

/// C's `%a` of a double (glibc's form).
fn hexfloat(v: f64) -> String {
    let sign = if v.is_sign_negative() { "-" } else { "" };
    if v.is_nan() {
        return format!("{sign}nan");
    }
    if v.is_infinite() {
        return format!("{sign}inf");
    }
    let bits = v.to_bits();
    let (e, m) = (((bits >> 52) & 0x7ff) as i64, bits & ((1 << 52) - 1));
    if e == 0 && m == 0 {
        return format!("{sign}0x0p+0");
    }
    let (lead, exp) = if e == 0 { (0, -1022) } else { (1, e - 1023) };
    let digits = format!("{m:013x}");
    let digits = digits.trim_end_matches('0');
    let dot = if digits.is_empty() { "" } else { "." };
    let es = if exp < 0 { "-" } else { "+" };
    format!("{sign}0x{lead}{dot}{digits}p{es}{}", exp.abs())
}

#[test]
fn the_official_outputs_with_infinities_and_nans() {
    let env = Environment::from_pairs(Vec::<(String, String)>::new());
    for (low, high, want) in OFFICIAL {
        for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
            let mut g = Globals::new(mode);
            set_random_seed(&mut g, 5);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            for w in want {
                let x = random_number(&mut g, &env, &mut o, &mut e, parse(low), parse(high))
                    .expect("a draw");
                assert_eq!(hexfloat(x), w, "randomNumber({low}, {high})");
            }
        }
    }
}

struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn bound(&mut self) -> f64 {
        const POOL: [u64; 14] = [
            0x0000_0000_0000_0000,
            0x8000_0000_0000_0000,
            0x3ff8_0000_0000_0000,
            0xc000_0000_0000_0000,
            0x7ff0_0000_0000_0000,
            0xfff0_0000_0000_0000,
            0x7ff8_0000_0000_0000,
            0xfff8_0000_0000_0000,
            0x7ff8_0000_0000_0001,
            0xfff8_1234_5678_9abc,
            0x7ff0_0000_0000_0003,
            0xfff0_0000_0000_0004,
            0x7fef_ffff_ffff_ffff,
            0x0000_0000_0000_0001,
        ];
        f64::from_bits(POOL[(self.next() % POOL.len() as u64) as usize])
    }
}

fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= u64::from(*b);
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

#[test]
fn nan_payloads_follow_x86_64() {
    let env = Environment::from_pairs(Vec::<(String, String)>::new());
    let mut rng = Gen(17);
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for i in 0..4096 {
        let (low, high) = (rng.bound(), rng.bound());
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        set_random_seed(&mut g, 1 + i64::from(i % 7));
        let (mut o, mut e) = (Vec::new(), Vec::new());
        for _ in 0..3 {
            let x = random_number(&mut g, &env, &mut o, &mut e, low, high).expect("a draw");
            fnv(&mut h, &x.to_bits().to_le_bytes());
        }
        fnv(&mut h, &o);
        fnv(&mut h, &e);
    }
    assert_eq!(h, RECORDED, "{h:#018x}");
}
