// SPDX-License-Identifier: Apache-2.0
//! The sampled arguments of the glibc-maths proofs: families of seeded arguments in the ranges the
//! tools meet, and the special values whose every pair is compared. Plain arithmetic only (no
//! function of the C library), so every program that includes this file gets the same arguments.
//!
//! Included verbatim by the FSL replicas' `glibc-float-verify copy`
//! (`crates-nc/larmorx-nc-core/tools/glibc-float-verify`), which compares the same arguments with
//! the replica core's translations.

// The limits of `exp` as decimal literals, digit for digit.
#![allow(clippy::excessive_precision)]

/// One family: its name, how many arguments, and the argument made from a 64-bit random number.
pub struct Family<T> {
    pub name: &'static str,
    pub count: u64,
    pub arg: fn(u64) -> T,
}

/// SplitMix64: the `i`-th random number.
#[inline]
pub fn splitmix64(i: u64) -> u64 {
    let mut z = i.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The `i`-th random number of family `f` of function `g` (disjoint streams).
#[inline]
pub fn random(g: u64, f: u64, i: u64) -> u64 {
    splitmix64((g << 56) | (f << 44) | i)
}

/// A double uniform in [0, 1) from the top 53 bits.
#[inline]
fn unit(r: u64) -> f64 {
    (r >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// A float uniform in [0, 1) from 24 bits.
#[inline]
fn unit32(r: u32) -> f32 {
    (r >> 8) as f32 * (1.0 / (1u32 << 24) as f32)
}

/// `2^e` as a double, `e` in the normal range.
#[inline]
fn pow2(e: i64) -> f64 {
    f64::from_bits(((1023 + e) as u64) << 52)
}

/// `2^e` as a float, `e` in the normal range.
#[inline]
fn pow2f(e: i32) -> f32 {
    f32::from_bits(((127 + e) as u32) << 23)
}

/// `log2(x)` for `x` in [1, 2] by plain arithmetic, to about 1e-12 (only to place arguments near
/// the overflow and underflow limits; the exact value does not matter).
fn log2_approx(x: f64) -> f64 {
    // atanh series: log2(x) = 2 / ln 2 * sum t^(2k+1) / (2k+1), t = (x - 1) / (x + 1).
    let t = (x - 1.0) / (x + 1.0);
    let t2 = t * t;
    let (mut term, mut sum) = (t, 0.0);
    for k in 0..20 {
        sum += term / f64::from(2 * k + 1);
        term *= t2;
    }
    2.0 * sum * std::f64::consts::LOG2_E
}

const N28: u64 = 1 << 28;
const N27: u64 = 1 << 27;

/// `exp`: 6 × 2^28 arguments.
pub const EXP: [Family<f64>; 6] = [
    Family {
        name: "random bit patterns",
        count: N28,
        arg: f64::from_bits,
    },
    Family {
        name: "uniform on [-746, 710] (every finite result, subnormal and overflow edges)",
        count: N28,
        arg: |r| -746.0 + 1456.0 * unit(r),
    },
    Family {
        name: "uniform on [-40, 0] (Gaussian kernels, FAST's energies)",
        count: N28,
        arg: |r| -40.0 * unit(r),
    },
    Family {
        name: "uniform on [-1, 1]",
        count: N28,
        arg: |r| 2.0 * unit(r) - 1.0,
    },
    Family {
        name: "tiny: magnitudes 2^-70 to 2^-20, both signs (the |x| < 2^-54 path)",
        count: N28,
        arg: |r| {
            let e = -70 + (r % 50) as i64;
            let m = f64::from_bits(0x3ff0_0000_0000_0000 | (r >> 12));
            let v = m * pow2(e);
            if r & (1 << 6) != 0 { -v } else { v }
        },
    },
    Family {
        name: "the special-case ranges [-745.2, -708] and [709, 709.79]",
        count: N28,
        arg: |r| {
            if r & 1 == 0 {
                -745.2 + 37.2 * unit(r)
            } else {
                709.0 + 0.79 * unit(r)
            }
        },
    },
];

/// `log`: 6 × 2^28 arguments.
pub const LOG: [Family<f64>; 6] = [
    Family {
        name: "random bit patterns",
        count: N28,
        arg: f64::from_bits,
    },
    Family {
        name: "uniform on (0, 1] (probabilities)",
        count: N28,
        arg: |r| 1.0 - unit(r),
    },
    Family {
        name: "around 1: [0.9336, 1.0742] (the path near 1 and its limits)",
        count: N28,
        arg: |r| 0.9336 + 0.1406 * unit(r),
    },
    Family {
        name: "positive bit patterns (every exponent, log-uniform)",
        count: N28,
        arg: |r| f64::from_bits(r & 0x7fff_ffff_ffff_ffff),
    },
    Family {
        name: "image intensities: integers 1 to 2^24 and reals on [0, 65536]",
        count: N28,
        arg: |r| {
            if r & 1 == 0 {
                ((r >> 1) % (1 << 24) + 1) as f64
            } else {
                65536.0 * unit(r)
            }
        },
    },
    Family {
        name: "subnormals, both signs",
        count: N28,
        arg: |r| f64::from_bits(r & 0x800f_ffff_ffff_ffff),
    },
];

/// `pow`: 8 × 2^27 pairs.
pub const POW: [Family<(f64, f64)>; 8] = [
    Family {
        name: "random bit patterns",
        count: N27,
        arg: |r| (f64::from_bits(r), f64::from_bits(splitmix64(r))),
    },
    Family {
        name: "x uniform on [0, 1e6), y on [-4, 4)",
        count: N27,
        arg: |r| (1e6 * unit(r), 8.0 * unit(splitmix64(r)) - 4.0),
    },
    Family {
        name: "BET: x on [0, 1e7), y one of 1/3, 1/2, 0.275",
        count: N27,
        arg: |r| {
            let y = [1.0 / 3.0, 0.5, 0.275][(splitmix64(r) % 3) as usize];
            (1e7 * unit(r), y)
        },
    },
    Family {
        name: "x within 2^-20 of 1, y on [-1e6, 1e6)",
        count: N27,
        arg: |r| {
            (
                1.0 + (unit(r) - 0.5) * pow2(-19),
                (unit(splitmix64(r)) - 0.5) * 2e6,
            )
        },
    },
    Family {
        name: "negative x on [-100, 0), y an integer in [-60, 60] or on [-4, 4)",
        count: N27,
        arg: |r| {
            let r2 = splitmix64(r);
            let y = if r2 & 1 == 0 {
                ((r2 >> 1) % 121) as f64 - 60.0
            } else {
                8.0 * unit(r2) - 4.0
            };
            (-100.0 * (1.0 - unit(r)), y)
        },
    },
    Family {
        name: "results near overflow and underflow: x on [1.5, 2), y log2 x on [1020, 1026) or [-1080, -1068)",
        count: N27,
        arg: |r| {
            let r2 = splitmix64(r);
            let x = 1.5 + 0.5 * unit(r);
            let t = if r2 & 1 == 0 {
                1020.0 + 6.0 * unit(r2)
            } else {
                -1080.0 + 12.0 * unit(r2)
            };
            let x = if r2 & 2 == 0 { x } else { 1.0 / x };
            let t = if r2 & 2 == 0 { t } else { -t };
            (x, t / log2_approx(if r2 & 2 == 0 { x } else { 1.0 / x }))
        },
    },
    Family {
        name: "subnormal x, y on [-2, 2)",
        count: N27,
        arg: |r| {
            (
                f64::from_bits(r & 0x000f_ffff_ffff_ffff),
                4.0 * unit(splitmix64(r)) - 2.0,
            )
        },
    },
    Family {
        name: "x on [0, 1000), |y| below 2^-65 or 2^63 to 2^70",
        count: N27,
        arg: |r| {
            let r2 = splitmix64(r);
            let m = 1.0 + unit(r2);
            let y = if r2 & 1 == 0 {
                m * pow2(-66 - (r2 >> 60) as i64)
            } else {
                m * pow2(63 + ((r2 >> 61) & 7) as i64)
            };
            let y = if r2 & 2 == 0 { y } else { -y };
            (1000.0 * unit(r), y)
        },
    },
];

/// `powf`: 6 × 2^28 pairs.
pub const POWF: [Family<(f32, f32)>; 6] = [
    Family {
        name: "random bit patterns",
        count: N28,
        arg: |r| (f32::from_bits(r as u32), f32::from_bits((r >> 32) as u32)),
    },
    Family {
        name: "TFCE: heights on (0, 1000] to H on [0.5, 3), cluster sizes 1 to 2^20 to E on [0.5, 3)",
        count: N28,
        arg: |r| {
            let e = 0.5 + 2.5 * unit32((r >> 32) as u32);
            if r & 1 == 0 {
                (1000.0 * (1.0 - unit32(r as u32)), e)
            } else {
                (((r >> 1) % (1 << 20) + 1) as f32, e)
            }
        },
    },
    Family {
        name: "x on (0, 100], y on [-10, 10)",
        count: N28,
        arg: |r| {
            (
                100.0 * (1.0 - unit32(r as u32)),
                20.0 * unit32((r >> 32) as u32) - 10.0,
            )
        },
    },
    Family {
        name: "negative x on [-100, 0), y an integer in [-40, 40] or on [-4, 4)",
        count: N28,
        arg: |r| {
            let x = -100.0 * (1.0 - unit32(r as u32));
            let y = if r & (1 << 63) == 0 {
                ((r >> 32) % 81) as f32 - 40.0
            } else {
                8.0 * unit32((r >> 32) as u32) - 4.0
            };
            (x, y)
        },
    },
    Family {
        name: "results near overflow and underflow: y log2 x on [120, 130) or [-155, -145)",
        count: N28,
        arg: |r| {
            let x = 1.5 + 0.5 * unit(r);
            let r2 = splitmix64(r);
            let t = if r2 & 1 == 0 {
                120.0 + 10.0 * unit(r2)
            } else {
                -155.0 + 10.0 * unit(r2)
            };
            let y = t / log2_approx(x);
            if r2 & 2 == 0 {
                (x as f32, y as f32)
            } else {
                ((1.0 / x) as f32, -y as f32)
            }
        },
    },
    Family {
        name: "subnormal x, both signs, y on [-3, 3)",
        count: N28,
        arg: |r| {
            (
                f32::from_bits(r as u32 & 0x807f_ffff),
                6.0 * unit32((r >> 32) as u32) - 3.0,
            )
        },
    },
];

/// `atan2f(y, x)`: 5 × 2^28 pairs `(y, x)`.
pub const ATAN2F: [Family<(f32, f32)>; 5] = [
    Family {
        name: "random bit patterns",
        count: N28,
        arg: |r| (f32::from_bits(r as u32), f32::from_bits((r >> 32) as u32)),
    },
    Family {
        name: "rotation-like: |y| <= 1, x on [0.5, 1] (atan2(sy, cy))",
        count: N28,
        arg: |r| {
            (
                2.0 * unit32(r as u32) - 1.0,
                0.5 + 0.5 * unit32((r >> 32) as u32),
            )
        },
    },
    Family {
        name: "y and x on [-1, 1] (atan2(sx, cx), atan2(sz, cz))",
        count: N28,
        arg: |r| {
            (
                2.0 * unit32(r as u32) - 1.0,
                2.0 * unit32((r >> 32) as u32) - 1.0,
            )
        },
    },
    Family {
        name: "magnitudes 2^-80 to 2^80 each, both signs (exponent differences beyond 60)",
        count: N28,
        arg: |r| {
            let r2 = splitmix64(r);
            let y = (1.0 + unit32(r as u32)) * pow2f(-80 + ((r >> 32) % 160) as i32);
            let x = (1.0 + unit32(r2 as u32)) * pow2f(-80 + ((r2 >> 32) % 160) as i32);
            (
                if r2 & 1 == 0 { y } else { -y },
                if r2 & 2 == 0 { x } else { -x },
            )
        },
    },
    Family {
        name: "x = 1 or -1, y random bit patterns",
        count: N28,
        arg: |r| {
            (
                f32::from_bits(r as u32),
                if r & (1 << 63) == 0 { 1.0 } else { -1.0 },
            )
        },
    },
];

/// Special doubles: zeros, infinities, NaNs (quiet and signalling, both signs, payloads), limits
/// of the functions' paths and their neighbours.
pub fn special_f64() -> Vec<f64> {
    let base = [
        0.0,
        1.0,
        2.0,
        0.5,
        0.275,
        1.0 / 3.0,
        3.0,
        -3.0,
        1e-300,
        1e300,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        f64::MAX,
        f64::INFINITY,
        709.782712893384,
        708.39641853226408,
        745.1332191019411,
        745.1332191019412,
        512.0,
        1024.0,
        pow2(-54),
        pow2(-65),
        pow2(63),
        0.9375,
        1.064_697_265_625,
        1e6,
        2.5,
        0.75,
        f64::from_bits(0x7ff8_0000_0000_0000),
        f64::from_bits(0x7ff8_0000_0000_1234),
        f64::from_bits(0x7ff0_0000_0000_0001),
        f64::from_bits(0x7ff4_0000_0000_5678),
    ];
    let mut out = Vec::new();
    for v in base {
        for w in [v, -v] {
            out.push(w);
            if w.is_finite() && w != 0.0 {
                out.push(f64::from_bits(w.to_bits() + 1));
                out.push(f64::from_bits(w.to_bits() - 1));
            }
        }
    }
    out
}

/// Special floats, as [`special_f64`].
pub fn special_f32() -> Vec<f32> {
    let base = [
        0.0f32,
        1.0,
        2.0,
        0.5,
        1.5,
        3.0,
        0.4375,
        0.6875,
        1.1875,
        2.4375,
        1e-30,
        1e-20,
        1e10,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::from_bits(0x007f_ffff),
        f32::MAX,
        f32::INFINITY,
        88.0,
        88.72283,
        103.27893,
        103.97208,
        120.0,
        std::f32::consts::FRAC_PI_4,
        pow2f(-12),
        pow2f(-29),
        pow2f(25),
        126.0,
        150.0,
        f32::from_bits(0x7fc0_0000),
        f32::from_bits(0x7fc0_1234),
        f32::from_bits(0x7f80_0001),
        f32::from_bits(0x7fa0_5678),
    ];
    let mut out = Vec::new();
    for v in base {
        for w in [v, -v] {
            out.push(w);
            if w.is_finite() && w != 0.0 {
                out.push(f32::from_bits(w.to_bits() + 1));
                out.push(f32::from_bits(w.to_bits() - 1));
            }
        }
    }
    out
}
