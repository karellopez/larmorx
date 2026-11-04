// SPDX-License-Identifier: GPL-3.0-or-later AND LGPL-2.1-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! The spec writer's model of `specs/glibc-dbl.md`: glibc 2.35's double `sin`, `cos` and `sincos`
//! (x86-64), written step by step from the spec's text (§3 to §8), to prove that the text alone
//! gives glibc's bits. Its author had read glibc's source (IBM's Accurate Mathematical Library,
//! LGPL-2.1-or-later), so the model lives here, in the GPL workspace, never in the main package;
//! a clean-room implementer must not open it (the spec, "Whoever implements it must not open").
//!
//! - `check --record <json> --table <R file> [--glibc-table <file>] [--sse2] [--threads N]`: the
//!   block hashes of every family of `sin`, `cos` and `sincos` (`families.rs`) and the special
//!   arguments' results, against the black-box record; with `--glibc-table`, the model takes
//!   glibc's own table R instead of the definition's (read at run time from glibc's source);
//!   with `--sse2`, `sin` and `cos` read `FMA` as two roundings, against a record of glibc's
//!   build for CPUs without FMA (`GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA`).
//! - `tables --table <R file> --glibc-table <file> [--log2n K] [--threads N]`: the model with
//!   the definition's table R against the model with glibc's (the 18 numbers of §5.3), on the
//!   record's arguments and on `2^K` arguments near each row where the two tables differ.
//!
//! The R file is `tables.py --emit`'s output (from the definitions alone).

// The model writes the spec's operations literally: `x / x` is its NaN result (§4.1), and
// `bb = (t - b) + bb` keeps the spec's order of operands.
#![allow(clippy::eq_op, clippy::assign_op_pattern)]

#[allow(dead_code)] // the families' names are the black-box tool's
#[path = "../../../../validation/investigations/glibc-dbl/families.rs"]
mod families;

use rayon::prelude::*;
use std::fmt::Write as _;

// ---------------------------------------------------------------------------------------------
// §3: the constants (bits from the spec's tables)

const H0: f64 = f64::from_bits(0x3ff9_21fb_5444_2d18);
const H1: f64 = f64::from_bits(0x3c91_a626_3314_5c07);
const G: f64 = f64::from_bits(0x3fe4_5f30_6dc9_c883);
const M1: f64 = f64::from_bits(0x3ff9_21fb_5800_0000);
const M2: f64 = f64::from_bits(0xbe4d_de97_3c00_0000);
const M3: f64 = f64::from_bits(0xbc8c_b3b3_9800_0000);
const M4: f64 = f64::from_bits(0xbacd_747f_23e3_2ed7);
const M2L: f64 = f64::from_bits(0xbe4d_de97_4000_0000);
const K52: f64 = 6_755_399_441_055_744.0; // 1.5 * 2^52
const K54: f64 = 27_021_597_764_222_976.0; // 1.5 * 2^54
const C27: f64 = 134_217_729.0; // 2^27 + 1
const T3: f64 = f64::from_bits(0xbfc5_5555_5555_5555);
const T5: f64 = f64::from_bits(0x3f81_1111_1111_0ece);
const T7: f64 = f64::from_bits(0xbf2a_01a0_19db_08b8);
const T9: f64 = f64::from_bits(0x3ec7_1de2_7b9a_7ed9);
const T11: f64 = f64::from_bits(0xbe5a_ddff_c2fc_df59);
const U3: f64 = f64::from_bits(0xbfc5_5555_5555_5515);
const U5: f64 = f64::from_bits(0x3f81_1110_e829_872f);
const V2: f64 = 0.5;
const V4: f64 = f64::from_bits(0xbfa5_5555_5555_5535);
const V6: f64 = f64::from_bits(0x3f56_c16b_edd9_e239);
const TAU: f64 = f64::from_bits(0x3fc0_20c4_9ba5_e354);
// §4: the limits
const TWO_M27: f64 = f64::from_bits(0x3e40_0000_0000_0000);
const TWO_M26: f64 = f64::from_bits(0x3e50_0000_0000_0000);
const L0: f64 = 0.855_468_75;
const L1: f64 = f64::from_bits(0x4003_68fd_0000_0000);
const L2: f64 = f64::from_bits(0x4199_21fb_0000_0000);

/// §1: the two readings of `FMA`.
#[derive(Clone, Copy, PartialEq)]
enum Reading {
    /// `sin`, `cos`: one rounding of `a b + c`.
    Fused,
    /// `sincos`: the product rounded, then the sum.
    Two,
}

#[inline]
fn fma(m: Reading, a: f64, b: f64, c: f64) -> f64 {
    match m {
        Reading::Fused => a.mul_add(b, c),
        Reading::Two => a * b + c,
    }
}

/// The model: table R (§5) and the digits of 2/pi (§3).
struct Model {
    r: Vec<[f64; 4]>,
    d: Vec<f64>,
    /// The reading of `FMA` in `sin` and `cos`: fused (glibc's FMA build, the record's), or two
    /// roundings (its build for CPUs without FMA, §2).
    sin_cos: Reading,
}

impl Model {
    // ---------------------------------------------------------------------------------------
    // §6: the kernels

    /// §6.1, the sine of a small number.
    fn s0(&self, m: Reading, x: f64, d: f64) -> f64 {
        let z = x * x;
        let p = fma(m, z, fma(m, z, fma(m, z, fma(m, z, T11, T9), T7), T5), T3);
        let h = 0.5 * d;
        let e = fma(m, z, fma(m, p, x, -h), d);
        x + e
    }

    fn row(&self, y: f64) -> (f64, [f64; 4]) {
        let n = (128.0 * y).round_ties_even();
        (y - n / 128.0, self.r[n as usize])
    }

    /// §6.2, the table-driven sine.
    fn s(&self, m: Reading, x: f64, d: f64) -> f64 {
        if x.abs() < TAU {
            return self.s0(m, x, d);
        }
        let d1 = if x > 0.0 { d } else { -d };
        let (xx, [sg, sg1, kp, kp1]) = self.row(x.abs());
        let z = xx * xx;
        let p = fma(m, z, U5, U3);
        let s = xx + fma(m, xx * z, p, d1);
        let q = fma(m, z, fma(m, z, V6, V4), V2);
        let c = fma(m, xx, d1, z * q);
        let rho = fma(m, s, kp, fma(m, -c, sg, fma(m, s, kp1, sg1)));
        (sg + rho).copysign(x)
    }

    /// §6.3, the table-driven cosine.
    fn c(&self, m: Reading, x: f64, d: f64) -> f64 {
        let d1 = if x < 0.0 { -d } else { d };
        let (diff, [sg, sg1, kp, kp1]) = self.row(x.abs());
        let xx = diff + d1;
        let z = xx * xx;
        let p = fma(m, z, U5, U3);
        let s = fma(m, xx * z, p, xx);
        let q = fma(m, z, fma(m, z, V6, V4), V2);
        let c = z * q;
        let rho = fma(m, -s, sg, fma(m, -c, kp, fma(m, -s, sg1, kp1)));
        kp + rho
    }

    // ---------------------------------------------------------------------------------------
    // §8: the reductions

    /// §8.1, below L2.
    fn reduce_small(&self, m: Reading, x: f64) -> (u32, f64, f64) {
        let t = fma(m, x, G, K52);
        let k = t - K52;
        let n = (k as i64).rem_euclid(4) as u32;
        let mut y = fma(m, -k, M1, x);
        y = fma(m, -k, M2, y);
        let t2 = fma(m, -k, M3, y);
        let e = fma(m, -k, M3, y - t2);
        let a = fma(m, -k, M4, t2);
        let da = e + fma(m, -k, M4, t2 - a);
        (n, a, da)
    }

    /// §8.2, from L2 (one rounding per operation, in every build).
    fn reduce_large(&self, x: f64) -> (u32, f64, f64) {
        let xp = x * f64::from_bits(0x1a70_0000_0000_0000); // 2^-600
        let w = xp * C27;
        let x1 = w - (w - xp);
        let x2 = xp - x1;
        let part = |p: f64| -> (f64, f64, f64) {
            let e = ((p.to_bits() >> 52) & 0x7ff) as i64;
            let j0 = if e >= 450 { (e - 450) / 24 } else { 0 } as usize;
            let mut r = [0.0f64; 6];
            for (i, ri) in r.iter_mut().enumerate() {
                let scale = f64::from_bits(((1023 + 576 - 24 * (j0 + i) as i64) as u64) << 52);
                *ri = (p * self.d[j0 + i]) * scale;
            }
            let mut sum = 0.0;
            for ri in r.iter_mut().take(3) {
                let u = (*ri + K52) - K52;
                sum += u;
                *ri -= u;
            }
            let mut t = 0.0;
            for i in (0..6).rev() {
                t += r[i];
            }
            let mut bb = (((((r[0] - t) + r[1]) + r[2]) + r[3]) + r[4]) + r[5];
            let u = (t + K52) - K52;
            sum += u;
            t -= u;
            let b = t + bb;
            bb = (t - b) + bb;
            let u = (sum + K54) - K54;
            sum -= u;
            (b, bb, sum)
        };
        let (b1, bb1, s1) = part(x1);
        let (b2, bb2, s2) = part(x2);
        let mut sum = s1 + s2;
        let mut b = b1 + b2;
        let bb = if b1.abs() > b2.abs() {
            (b1 - b) + b2
        } else {
            (b2 - b) + b1
        };
        if b > 0.5 {
            b -= 1.0;
            sum += 1.0;
        } else if b < -0.5 {
            b += 1.0;
            sum -= 1.0;
        }
        let s = b + ((bb + bb1) + bb2);
        let t = ((b - s) + bb) + (bb1 + bb2);
        let w = s * C27;
        let sa = w - (w - s);
        let sb = s - sa;
        let big = s * H0;
        let bbig =
            ((((sa * M1) - big) + (sa * M2L)) + (sb * M1)) + (((sb * M2L) + (s * H1)) + (t * H0));
        let a = big + bbig;
        let da = (big - a) + bbig;
        ((sum as i64).rem_euclid(4) as u32, a, da)
    }

    fn reduce(&self, m: Reading, x: f64) -> (u32, f64, f64) {
        if x.abs() < L2 {
            self.reduce_small(m, x)
        } else {
            self.reduce_large(x)
        }
    }

    // ---------------------------------------------------------------------------------------
    // §4, §7: the functions

    fn quadrant(&self, m: Reading, n: u32, a: f64, da: f64) -> f64 {
        let v = if n & 1 == 1 {
            self.c(m, a, da)
        } else {
            self.s(m, a, da)
        };
        if n >= 2 { -v } else { v }
    }

    fn sin(&self, x: f64) -> f64 {
        let m = self.sin_cos;
        let ax = x.abs();
        if !x.is_finite() {
            x / x
        } else if ax < TWO_M26 {
            x
        } else if ax < L0 {
            self.s(m, x, 0.0)
        } else if ax < L1 {
            let t = H0 - ax;
            self.c(m, t, H1).copysign(x)
        } else {
            let (n, a, da) = self.reduce(m, x);
            self.quadrant(m, n, a, da)
        }
    }

    fn cos(&self, x: f64) -> f64 {
        let m = self.sin_cos;
        let ax = x.abs();
        if !x.is_finite() {
            x / x
        } else if ax < TWO_M27 {
            1.0
        } else if ax < L0 {
            self.c(m, x, 0.0)
        } else if ax < L1 {
            let y = H0 - ax;
            let a = y + H1;
            let da = (y - a) + H1;
            self.s(m, a, da)
        } else {
            let (n, a, da) = self.reduce(m, x);
            self.quadrant(m, (n + 1) % 4, a, da)
        }
    }

    fn sincos(&self, x: f64) -> (f64, f64) {
        let m = Reading::Two;
        let ax = x.abs();
        if !x.is_finite() {
            (x / x, x / x)
        } else if ax < TWO_M27 {
            (x, 1.0)
        } else if ax < L0 {
            (self.s(m, x, 0.0), self.c(m, x, 0.0))
        } else if ax < L1 {
            let y = H0 - ax;
            let a = y + H1;
            let da = (y - a) + H1;
            (self.c(m, a, da).copysign(x), self.s(m, a, da))
        } else {
            let (n, mut a, mut da) = self.reduce(m, x);
            if n == 1 || n == 2 {
                a = -a;
                da = -da;
            }
            let (s, c) = (self.s(m, a, da), self.c(m, a, da));
            match n {
                0 => (s, c),
                1 => (c, s),
                2 => (s, -c),
                _ => (-c, s),
            }
        }
    }

    fn eval(&self, f: &str, x: f64) -> ([u64; 2], usize) {
        match f {
            "sin" => ([self.sin(x).to_bits(), 0], 1),
            "cos" => ([self.cos(x).to_bits(), 0], 1),
            "sincos" => {
                let (s, c) = self.sincos(x);
                ([s.to_bits(), c.to_bits()], 2)
            }
            _ => unreachable!(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Input: the R file, glibc's table (read at run time), the record

fn load_r(path: &str) -> (Vec<[f64; 4]>, Vec<f64>) {
    let text = std::fs::read_to_string(path).expect("read the R file");
    let (mut r, mut d) = (vec![[0.0; 4]; 110], vec![0.0; 75]);
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let w: Vec<&str> = line.split_whitespace().collect();
        let hx = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).expect("hex"));
        match w[0] {
            "R" => {
                let n: usize = w[1].parse().unwrap();
                r[n] = [hx(w[2]), hx(w[3]), hx(w[4]), hx(w[5])];
            }
            "D" => d[w[1].parse::<usize>().unwrap()] = hx(w[2]),
            _ => {}
        }
    }
    (r, d)
}

/// glibc's table R from its source text (the big-endian block: high word first).
fn load_glibc_table(path: &str) -> Vec<[f64; 4]> {
    let text = std::fs::read_to_string(path).expect("read glibc's table");
    let i = text.find("#ifdef BIG_ENDI").expect("big-endian block");
    let j = i + text[i..]
        .find("#ifdef LITTLE_ENDI")
        .expect("little-endian block");
    let words: Vec<u32> = text[i..j]
        .split("0x")
        .skip(1)
        .map(|s| u32::from_str_radix(&s[..8], 16).expect("hex word"))
        .collect();
    let v: Vec<f64> = words
        .chunks(2)
        .map(|w| f64::from_bits((u64::from(w[0]) << 32) | u64::from(w[1])))
        .collect();
    assert_eq!(v.len(), 440);
    v.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()
}

struct RecordFn {
    blocks: Vec<Vec<u64>>,
    special: Vec<Vec<u64>>,
}

fn hexes(s: &str) -> Vec<u64> {
    s.split('"')
        .filter(|t| t.len() == 16)
        .filter_map(|t| u64::from_str_radix(t, 16).ok())
        .collect()
}

fn load_record(path: &str, f: &str) -> RecordFn {
    let text = std::fs::read_to_string(path).expect("read the record");
    let start = text
        .find(&format!("\"{f}\": {{"))
        .expect("function in the record");
    let sp = start + text[start..].find("\"special\": [").expect("special");
    let mut blocks = Vec::new();
    let mut pos = start;
    while let Some(k) = text[pos..sp].find("\"blocks\": [") {
        let b = pos + k;
        let e = b + text[b..].find(']').unwrap();
        blocks.push(hexes(&text[b..e]));
        pos = e;
    }
    let end = sp + text[sp..].find("]\n").unwrap();
    let words = if f == "sincos" { 3 } else { 2 };
    let special = hexes(&text[sp..end])
        .chunks(words)
        .map(<[u64]>::to_vec)
        .collect();
    RecordFn { blocks, special }
}

const FNV0: u64 = 14_695_981_039_346_656_037;
const FNVP: u64 = 1_099_511_628_211;

fn block_hash(model: &Model, f: &str, g: u64, fam: &families::Family, b: u64) -> u64 {
    let mut h = FNV0;
    for i in b * families::BLOCK..(b + 1) * families::BLOCK {
        let x = (fam.arg)(families::random(g, fam.id, i));
        let (r, n) = model.eval(f, x);
        for w in &r[..n] {
            h = (h ^ w).wrapping_mul(FNVP);
        }
    }
    h
}

fn check(model: &Model, record: &str, label: &str, fns: &[&str]) -> bool {
    let mut all = true;
    for &f in fns {
        let g = families::FUNCTIONS.iter().find(|(n, _)| *n == f).unwrap().1;
        let rec = load_record(record, f);
        let fams = families::families(f);
        let per = (families::COUNT / families::BLOCK) as usize;
        let work: Vec<(usize, u64)> = (0..fams.len())
            .flat_map(|k| (0..per as u64).map(move |b| (k, b)))
            .collect();
        let got: Vec<u64> = work
            .par_iter()
            .map(|&(k, b)| block_hash(model, f, g, &fams[k], b))
            .collect();
        let mut line = String::new();
        let mut bad_total = 0;
        for (k, fam) in fams.iter().enumerate() {
            let bad = (0..per)
                .filter(|&b| got[k * per + b] != rec.blocks[k][b])
                .count();
            bad_total += bad;
            let _ = write!(line, " family {}: {}/{per}", fam.id, per - bad);
        }
        let specials = families::special();
        let sp_bad = specials
            .iter()
            .zip(&rec.special)
            .filter(|(x, want)| {
                let (r, n) = model.eval(f, **x);
                want[0] != x.to_bits() || r[..n] != want[1..]
            })
            .count();
        all &= bad_total == 0 && sp_bad == 0 && rec.special.len() == specials.len();
        println!(
            "{label} {f:6}: blocks agreeing{line}; special {}/{}",
            specials.len() - sp_bad,
            rec.special.len()
        );
    }
    all
}

/// The model with the definition's table against the model with glibc's table.
fn tables(def: &Model, glibc: &Model, log2n: u32) {
    for f in ["sin", "cos", "sincos"] {
        let g = families::FUNCTIONS.iter().find(|(n, _)| *n == f).unwrap().1;
        let fams = families::families(f);
        let differ: u64 = fams
            .par_iter()
            .map(|fam| {
                (0..families::COUNT)
                    .into_par_iter()
                    .filter(|&i| {
                        let x = (fam.arg)(families::random(g, fam.id, i));
                        def.eval(f, x).0 != glibc.eval(f, x).0
                    })
                    .count() as u64
            })
            .sum();
        println!(
            "tables {f:6}: the record's {} arguments: {differ} results differ",
            families::COUNT * fams.len() as u64
        );
    }
    // arguments near the rows whose numbers differ: the direct paths (|x| = N/128 + a uniform
    // offset in [-1/256, 1/256)) of sin, cos and sincos
    let rows: Vec<usize> = (0..110).filter(|&n| def.r[n] != glibc.r[n]).collect();
    let n = 1u64 << log2n;
    for f in ["sin", "cos", "sincos"] {
        let differ: u64 = rows
            .par_iter()
            .map(|&row| {
                (0..n)
                    .into_par_iter()
                    .filter(|&i| {
                        let r = families::splitmix64((0xd0u64 << 56) | ((row as u64) << 40) | i);
                        let off = ((r >> 11) as f64 * f64::from_bits(0x3ca0_0000_0000_0000) - 0.5)
                            / 128.0;
                        let x = (row as f64 / 128.0 + off).copysign(if r & 1 == 0 {
                            1.0
                        } else {
                            -1.0
                        });
                        def.eval(f, x).0 != glibc.eval(f, x).0
                    })
                    .count() as u64
            })
            .sum();
        println!(
            "tables {f:6}: {} rows differ; {n} arguments near each: {differ} results differ",
            rows.len()
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    if let Some(t) = get("--threads") {
        rayon::ThreadPoolBuilder::new()
            .num_threads(t.parse().expect("--threads N"))
            .build_global()
            .unwrap();
    }
    let (r, d) = load_r(&get("--table").expect("--table <R file>"));
    let sse2 = args.iter().any(|a| a == "--sse2");
    let sin_cos = if sse2 { Reading::Two } else { Reading::Fused };
    let def = Model {
        r,
        d: d.clone(),
        sin_cos,
    };
    match args.first().map(String::as_str) {
        Some("check") => {
            let record = get("--record").expect("--record <json>");
            let model = match get("--glibc-table") {
                Some(p) => Model {
                    r: load_glibc_table(&p),
                    d,
                    sin_cos,
                },
                None => def,
            };
            let fns: &[&str] = if sse2 {
                &["sin", "cos"]
            } else {
                &["sin", "cos", "sincos"]
            };
            let ok = check(&model, &record, "model", fns);
            println!("{}", if ok { "MODEL-PASS" } else { "MODEL-FAIL" });
            std::process::exit(if ok { 0 } else { 1 });
        }
        Some("tables") => {
            let glibc = Model {
                r: load_glibc_table(&get("--glibc-table").expect("--glibc-table")),
                d,
                sin_cos,
            };
            tables(
                &def,
                &glibc,
                get("--log2n").map_or(24, |v| v.parse().unwrap()),
            );
        }
        _ => {
            eprintln!(
                "usage: glibc-dbl-model check --record <json> --table <R file> [--glibc-table <file>] [--sse2] [--threads N]\n       glibc-dbl-model tables --table <R file> --glibc-table <file> [--log2n K] [--threads N]"
            );
            std::process::exit(2);
        }
    }
}
