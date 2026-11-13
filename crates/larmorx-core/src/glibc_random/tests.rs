// SPDX-License-Identifier: Apache-2.0
//! The clean room of `specs/glibc-random.md` against the spec's numbers (§2, §4, §6, from runs of
//! the C library's own functions, `validation/investigations/glibc-random-probe/`), and its golden
//! checksums (§10 item 8), each recorded from the probe's output and checked on every platform.

use super::{GlibcRandom, RAND_MAX, initial_words};
use crate::ranlux::sha256::Sha256;

/// FNV-1a 64-bit over values as 32-bit integers, as the probe's `hash` and `long` commands (§6).
fn fnv(h: u64, v: u32) -> u64 {
    (h ^ u64::from(v)).wrapping_mul(1_099_511_628_211)
}
const FNV0: u64 = 14_695_981_039_346_656_037;

/// The probe's `long <seed> <count>`.
fn long_hash(seed: u32, count: usize) -> u64 {
    let mut g = GlibcRandom::seeded(seed);
    (0..count).fold(FNV0, |h, _| fnv(h, g.next_u31()))
}

/// The probe's `hash <seed0> <nseeds> <values>` (seeds modulo 2³²).
fn block_hash(seed0: u32, nseeds: u32, values: usize) -> u64 {
    let mut h = FNV0;
    let mut g = GlibcRandom::new();
    for k in 0..nseeds {
        g.seed(seed0.wrapping_add(k));
        for _ in 0..values {
            h = fnv(h, g.next_u31());
        }
    }
    h
}

fn first(seed: u32, n: usize) -> Vec<u32> {
    let mut g = GlibcRandom::seeded(seed);
    (0..n).map(|_| g.next_u31()).collect()
}

#[test]
fn seeding_words() {
    // §2: seed 1's words, and the non-negative residue of a negative product.
    let w = initial_words(1);
    assert_eq!(&w[..4], &[1, 16_807, 282_475_249, 1_622_650_073]);
    assert_eq!(w[30], 1_505_795_335);
    let w = initial_words(u32::MAX);
    assert_eq!(w[0], u32::MAX); // r₀ = −1 as a word
    assert_eq!(w[1], 2_147_466_840);
    // 2³¹ − 1: every word after the first is zero.
    let w = initial_words(2_147_483_647);
    assert!(w[1..].iter().all(|&v| v == 0));
    // the zero seed is seed 1.
    assert_eq!(initial_words(0), initial_words(1));
}

#[test]
fn reference_numbers() {
    // §6's table: o₀ … o₄ of each seed.
    let table: &[(u32, [u32; 5])] = &[
        (
            1,
            [1804289383, 846930886, 1681692777, 1714636915, 1957747793],
        ),
        (
            53,
            [2099602078, 1877112656, 1115506446, 957877911, 2107065092],
        ),
        (
            1234,
            [479142414, 465566339, 961126155, 1057886067, 1222702060],
        ),
        (
            2147483647,
            [1065668062, 2142264300, 1066566375, 1064012770, 2141034222],
        ),
        (
            2147483648,
            [1336741213, 1210407648, 1447044896, 337392383, 82502902],
        ),
        (
            4294967295,
            [254925627, 1205188300, 366127624, 1401405153, 76053476],
        ),
    ];
    for (seed, want) in table {
        assert_eq!(first(*seed, 5), want, "seed {seed}");
    }
    assert_eq!(first(0, 5), first(1, 5));
}

#[test]
fn unseeded_is_seed_one() {
    // §4: the state before any seeding is seed 1's (100,000 values, as the probe's `default`).
    let mut a = GlibcRandom::new();
    let mut b = GlibcRandom::seeded(1);
    let mut c = GlibcRandom::default();
    for _ in 0..100_000 {
        let v = a.next_u31();
        assert_eq!(v, b.next_u31());
        assert_eq!(v, c.next_u31());
    }
}

#[test]
fn seed_conversion() {
    // §1: C's conversion of a wider signed seed, modulo 2³².
    let cases: &[(i64, u32)] = &[
        (-1, 4_294_967_295),
        (-2, 4_294_967_294),
        (-2_147_483_648, 2_147_483_648),
        (-2_147_483_649, 2_147_483_647),
        (1 << 32, 0),
        ((1 << 32) + 5, 5),
        ((1 << 40) + 7, 7),
        (-(1 << 40) - 7, 4_294_967_289),
        (i64::MAX, 4_294_967_295),
        (i64::MIN, 0),
    ];
    for &(wide, narrow) in cases {
        let mut g = GlibcRandom::seeded(12345);
        g.seed_c_long(wide);
        assert_eq!(g, GlibcRandom::seeded(narrow), "seed {wide}");
    }
}

#[test]
fn one_shared_stream() {
    // §3: `rand` and `random` consume one stream in call order; seeding again restarts it (the
    // probe's `shared`: srand, then srand and srandom, then srandom alone).
    let want = first(53, 1000);
    let mut g = GlibcRandom::seeded(53);
    let mixed: Vec<u32> = (0..1000)
        .map(|i| {
            if i % 2 == 1 {
                g.random() as u32
            } else {
                g.rand() as u32
            }
        })
        .collect();
    assert_eq!(mixed, want);
    g.seed(53);
    g.seed(53);
    let mut again = vec![0; 1000];
    g.fill(&mut again);
    assert_eq!(again, want);
    assert!(want.iter().all(|&v| v <= RAND_MAX));
}

#[test]
fn long_streams() {
    // §6's hashes of 10⁶ values.
    assert_eq!(long_hash(1, 1_000_000), 0x8b12_006d_0a30_178c);
    assert_eq!(long_hash(53, 1_000_000), 0x8f2b_b11c_5dd4_9223);
    assert_eq!(long_hash(1234, 1_000_000), 0x54ec_7c5a_d4bf_1bfe);
    assert_eq!(long_hash(2_147_483_648, 1_000_000), 0x9846_4c3c_54ef_732f);
}

#[test]
fn block_hashes() {
    // §6: seeds 2³¹ − 512 to 2³¹ + 511, 64 values each (the change of sign of r₀), and seeds 0 to
    // 2²⁰ − 1, 8 values each (a debug build takes a few seconds for the second).
    assert_eq!(block_hash(2_147_483_136, 1024, 64), 0xd3ac_c423_130f_2161);
    assert_eq!(block_hash(0, 1 << 20, 8), 0x5d03_1cec_3a78_334a);
}

/// One golden case: the first 4 values and the 1,000th, one decimal line each, as the probe's
/// `rand <seed> 1000` prints them.
fn golden_text(mut g: GlibcRandom) -> String {
    let mut text = String::new();
    for k in 1..=1000 {
        let v = g.rand();
        if k <= 4 || k == 1000 {
            text.push_str(&format!("{v}\n"));
        }
    }
    text
}

/// The golden checksums (§10 item 8), the SHA-256 of `glibc_random_probe rand <seed> 1000`'s lines
/// 1 to 4 and 1,000 (and `default 1000`'s), recorded from the C library on Linux x86-64.
const GOLDEN: &[(&str, &str)] = &[
    (
        "seed 0",
        "6a8e90e6a65a515cc06ea490fae333ac9a920ab2e2f1f12895ced926808c3001",
    ),
    (
        "seed 1",
        "6a8e90e6a65a515cc06ea490fae333ac9a920ab2e2f1f12895ced926808c3001",
    ),
    (
        "seed 2^31",
        "6c50320b27bba9c9ecf9f6a5f12ac62d9e4d9951ebde2f39172c97bf3cbc809d",
    ),
    (
        "seed 2^32 - 1",
        "446ad9986b67b875477332269f6c6e2804bad5e5e7c0f9cefe94c01c65f9b294",
    ),
    (
        "seed -1 (a C long)",
        "446ad9986b67b875477332269f6c6e2804bad5e5e7c0f9cefe94c01c65f9b294",
    ),
    (
        "unseeded",
        "6a8e90e6a65a515cc06ea490fae333ac9a920ab2e2f1f12895ced926808c3001",
    ),
];

#[test]
fn golden() {
    let mut long = GlibcRandom::seeded(7);
    long.seed_c_long(-1);
    let cases = [
        ("seed 0", GlibcRandom::seeded(0)),
        ("seed 1", GlibcRandom::seeded(1)),
        ("seed 2^31", GlibcRandom::seeded(1 << 31)),
        ("seed 2^32 - 1", GlibcRandom::seeded(u32::MAX)),
        ("seed -1 (a C long)", long),
        ("unseeded", GlibcRandom::new()),
    ];
    let n = cases.len();
    let mut bad = Vec::new();
    for (name, g) in cases {
        let mut h = Sha256::new();
        h.update(golden_text(g).as_bytes());
        let got = h.hex();
        match GOLDEN.iter().find(|(n, _)| *n == name) {
            Some((_, want)) if *want == got => {}
            _ => bad.push(format!("    (\"{name}\", \"{got}\"),")),
        }
    }
    assert_eq!(GOLDEN.len(), n, "cases:\n{}", bad.join("\n"));
    assert!(bad.is_empty(), "changed:\n{}", bad.join("\n"));
}
