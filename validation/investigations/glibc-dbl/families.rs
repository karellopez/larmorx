// SPDX-License-Identifier: Apache-2.0
//! The arguments of `specs/glibc-dbl.md`'s black-box record: families of seeded arguments of the
//! double `sin`, `cos`, `sincos`, `tan`, `asin`, `acos` and `atan`, and their special values.
//! Plain integer and IEEE arithmetic only (no function of the C library), so every program that
//! includes this file makes the same arguments on every platform; the spec defines each family
//! in words (§ "The black-box record"), and this file is that definition as code.
//!
//! Included verbatim by the black-box tool (`blackbox/`) and by any implementation's test that
//! checks its function against the record.

/// The functions of the record, with their stream numbers (`g` of [`random`]).
pub const FUNCTIONS: [(&str, u64); 7] = [
    ("sin", 1),
    ("cos", 2),
    ("sincos", 3),
    ("tan", 4),
    ("asin", 5),
    ("acos", 6),
    ("atan", 7),
];

/// Arguments per family, and per hashed block.
pub const COUNT: u64 = 1 << 27;
pub const BLOCK: u64 = 1 << 22;

/// SplitMix64's output for the state `s` (one step: the state advanced by the golden gamma, then
/// mixed).
#[inline]
pub fn splitmix64(s: u64) -> u64 {
    let mut z = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The `i`-th random number of family `f` of function `g`.
#[inline]
pub fn random(g: u64, f: u64, i: u64) -> u64 {
    splitmix64((g << 56) | (f << 44) | i)
}

const SIGN: u64 = 0x8000_0000_0000_0000;
const MANT: u64 = 0x000f_ffff_ffff_ffff;

/// `(r >> 11) * 2^-53`: uniform on [0, 1), exact.
#[inline]
fn unit(r: u64) -> f64 {
    (r >> 11) as f64 * f64::from_bits(0x3ca0_0000_0000_0000)
}

/// `a + w * unit(r)`: the product rounded, then the sum (two IEEE operations, no fused one).
#[inline]
fn uniform(r: u64, a: f64, w: f64) -> f64 {
    let p = w * unit(r);
    a + p
}

/// A double with `r`'s sign bit and 52 low bits, and the exponent `e0 + (r >> 52 & 0x7ff) mod n`.
#[inline]
fn log_uniform(r: u64, e0: i64, n: u64) -> f64 {
    let e = e0 + (((r >> 52) & 0x7ff) % n) as i64;
    f64::from_bits((r & SIGN) | (((1023 + e) as u64) << 52) | (r & MANT))
}

/// A float with `r`'s bit 31 and 23 low bits and the exponent `e0 + (r >> 23 & 0xff) mod n`, as
/// a double (exact).
#[inline]
fn float_values(r: u64, e0: i64, n: u64) -> f64 {
    let e = e0 + (((r >> 23) & 0xff) % n) as i64;
    let b = ((r as u32) & 0x8000_0000) | (((127 + e) as u32) << 23) | ((r as u32) & 0x007f_ffff);
    f64::from(f32::from_bits(b))
}

/// The bit pattern of `limits[r mod len]` moved by `(r >> 8 mod 2^25) - 2^24` units in the last
/// place (an integer added to the pattern), with `r`'s sign bit.
#[inline]
fn near_limits(r: u64, limits: &[u64]) -> f64 {
    let l = limits[(r % limits.len() as u64) as usize];
    let d = ((r >> 8) % (1 << 25)) as i64 - (1 << 24);
    f64::from_bits(((l as i64 + d) as u64) | (r & SIGN))
}

/// The positive limits of the paths of `sin`, `cos` and `sincos` (bit patterns): 2^-27, 2^-26,
/// 0.126, 0.85546875, the doubles whose high 32 bits are 0x400368fd and 0x419921fb, infinity.
pub const SINCOS_LIMITS: [u64; 7] = [
    0x3e40_0000_0000_0000,
    0x3e50_0000_0000_0000,
    0x3fc0_20c4_9ba5_e354,
    0x3feb_6000_0000_0000,
    0x4003_68fd_0000_0000,
    0x4199_21fb_0000_0000,
    0x7ff0_0000_0000_0000,
];

/// The positive limits of the paths of `tan` (bit patterns).
pub const TAN_LIMITS: [u64; 6] = [
    0x3e4b_096c_0000_0000,
    0x3faf_212d_0000_0000,
    0x3fe9_2f1a_0000_0000,
    0x4039_0000_0000_0000,
    0x4197_d784_0000_0000,
    0x7ff0_0000_0000_0000,
];

/// The positive limits of the paths of `asin` and `acos` (bit patterns).
pub const ASIN_LIMITS: [u64; 10] = [
    0x3c88_0000_0000_0000,
    0x3e50_0000_0000_0000,
    0x3fc0_0000_0000_0000,
    0x3fd0_0000_0000_0000,
    0x3fe0_0000_0000_0000,
    0x3fe8_0000_0000_0000,
    0x3fed_8000_0000_0000,
    0x3fee_8000_0000_0000,
    0x3fef_0000_0000_0000,
    0x3ff0_0000_0000_0000,
];

/// The positive limits of the paths of `atan` (bit patterns).
pub const ATAN_LIMITS: [u64; 6] = [
    0x3e4b_b67a_0000_0000,
    0x3fb0_0000_0000_0000,
    0x3ff0_0000_0000_0000,
    0x4030_0000_0000_0000,
    0x4334_9ff2_0000_0000,
    0x7ff0_0000_0000_0000,
];

/// One family: its number (`f` of [`random`]), its name, the argument made from a random number.
pub struct Family {
    pub id: u64,
    pub name: &'static str,
    pub arg: fn(u64) -> f64,
}

/// The families of `sin`, `cos`, `sincos` and `tan` (`tan` with its own limits).
pub fn periodic(tan: bool) -> Vec<Family> {
    let limits: fn(u64) -> f64 = if tan {
        |r| near_limits(r, &TAN_LIMITS)
    } else {
        |r| near_limits(r, &SINCOS_LIMITS)
    };
    vec![
        Family {
            id: 1,
            name: "bit patterns",
            arg: f64::from_bits,
        },
        Family {
            id: 2,
            name: "magnitudes 2^-40 to 2^40, log-uniform, both signs",
            arg: |r| log_uniform(r, -40, 80),
        },
        Family {
            id: 3,
            name: "uniform on [-2 pi, 2 pi)",
            arg: |r| {
                uniform(
                    r,
                    -f64::from_bits(0x4019_21fb_5444_2d18),
                    f64::from_bits(0x4029_21fb_5444_2d18),
                )
            },
        },
        Family {
            id: 4,
            name: "uniform on [-2^27, 2^27)",
            arg: |r| uniform(r, -134_217_728.0, 268_435_456.0),
        },
        Family {
            id: 5,
            name: "near the multiples k pi/2, k = 1 to 2^28, within 2^16 units in the last place, both signs",
            arg: |r| {
                let k = (1 + (r % (1 << 28))) as f64;
                let x0 = k * f64::from_bits(0x3ff9_21fb_5444_2d18);
                let d = ((r >> 28) % (1 << 17)) as i64 - (1 << 16);
                f64::from_bits(((x0.to_bits() as i64 + d) as u64) | (r & SIGN))
            },
        },
        Family {
            id: 6,
            name: "within 2^24 units in the last place of the paths' limits, both signs",
            arg: limits,
        },
        Family {
            id: 7,
            name: "float values with magnitudes 2^-10 to 2^17, both signs (image intensities)",
            arg: |r| float_values(r, -10, 27),
        },
        Family {
            id: 8,
            name: "magnitudes 2^26 to 2^1023, log-uniform, both signs (the large-argument reduction)",
            arg: |r| log_uniform(r, 26, 998),
        },
    ]
}

/// The families of `asin` and `acos`.
pub fn inverse_sine() -> Vec<Family> {
    vec![
        Family {
            id: 1,
            name: "bit patterns",
            arg: f64::from_bits,
        },
        Family {
            id: 2,
            name: "magnitudes 2^-60 to 2^-1, log-uniform, both signs",
            arg: |r| log_uniform(r, -60, 60),
        },
        Family {
            id: 3,
            name: "uniform on [-1, 1)",
            arg: |r| uniform(r, -1.0, 2.0),
        },
        Family {
            id: 4,
            name: "near 1 and -1: 1 - m 2^e, e = -53 to -3, m in [1, 2), both signs",
            arg: |r| {
                let e = -53 + (((r >> 52) & 0x7ff) % 51) as i64;
                let d = f64::from_bits((((1023 + e) as u64) << 52) | (r & MANT));
                f64::from_bits((1.0 - d).to_bits() | (r & SIGN))
            },
        },
        Family {
            id: 5,
            name: "within 2^24 units in the last place of the paths' limits, both signs",
            arg: |r| near_limits(r, &ASIN_LIMITS),
        },
        Family {
            id: 6,
            name: "float values with magnitudes 2^-24 to 2^-1, both signs (correlations)",
            arg: |r| float_values(r, -24, 24),
        },
    ]
}

/// The families of `atan`.
pub fn arc_tangent() -> Vec<Family> {
    vec![
        Family {
            id: 1,
            name: "bit patterns",
            arg: f64::from_bits,
        },
        Family {
            id: 2,
            name: "magnitudes 2^-60 to 2^60, log-uniform, both signs",
            arg: |r| log_uniform(r, -60, 120),
        },
        Family {
            id: 3,
            name: "uniform on [-1, 1)",
            arg: |r| uniform(r, -1.0, 2.0),
        },
        Family {
            id: 4,
            name: "uniform on [-16, 16)",
            arg: |r| uniform(r, -16.0, 32.0),
        },
        Family {
            id: 5,
            name: "within 2^24 units in the last place of the paths' limits, both signs",
            arg: |r| near_limits(r, &ATAN_LIMITS),
        },
        Family {
            id: 6,
            name: "float values with magnitudes 2^-10 to 2^17, both signs (image intensities)",
            arg: |r| float_values(r, -10, 27),
        },
    ]
}

/// The families of a function of [`FUNCTIONS`].
pub fn families(function: &str) -> Vec<Family> {
    match function {
        "sin" | "cos" | "sincos" => periodic(false),
        "tan" => periodic(true),
        "asin" | "acos" => inverse_sine(),
        "atan" => arc_tangent(),
        _ => panic!("unknown function {function}"),
    }
}

/// The special arguments (each with both signs): zeros, infinities, NaNs (quiet and signalling,
/// with payloads), the extremes of the subnormal and normal ranges, every path limit of the seven
/// functions and its two neighbours, and a few classic hard cases of the argument reduction.
pub fn special() -> Vec<f64> {
    let mut base: Vec<u64> = vec![
        0,
        0x7ff0_0000_0000_0000,
        0x7ff8_0000_0000_0000,
        0x7ff8_0000_0000_1234,
        0x7ff0_0000_0000_0001,
        0x7ff4_0000_0000_5678,
        1,
        0x000f_ffff_ffff_ffff,
        0x0010_0000_0000_0000,
        0x7fef_ffff_ffff_ffff,
        0x3ff0_0000_0000_0000, // 1
        0x3fe0_0000_0000_0000, // 0.5
        0x3ff9_21fb_5444_2d18, // RN(pi/2)
        0x4009_21fb_5444_2d18, // RN(pi)
        0x3fe9_21fb_5444_2d18, // RN(pi/4)
        0x4012_d97c_7f33_21d2, // fl(3 RN(pi) / 2)
        0x4019_21fb_5444_2d18, // RN(2 pi)
        0x4480_f0cf_064d_d592, // 1e22
        0x7506_ac5b_262c_a1ff, // 6381956970095103 * 2^797 (Kahan and McDonald's)
        0x4330_0000_0000_0000, // 2^52
        0x4340_0000_0000_0000, // 2^53
        0x7fe0_0000_0000_0000, // 2^1023
    ];
    for l in SINCOS_LIMITS
        .iter()
        .chain(&TAN_LIMITS)
        .chain(&ASIN_LIMITS)
        .chain(&ATAN_LIMITS)
    {
        base.extend([*l, l.wrapping_sub(1), l + 1]);
    }
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for b in base {
        for w in [b, b | SIGN] {
            if seen.insert(w) {
                out.push(f64::from_bits(w));
            }
        }
    }
    out
}
