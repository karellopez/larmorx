// SPDX-License-Identifier: Apache-2.0
//! Seeded operands and arithmetic patterns for checking [`F80`] against the x87.
//!
//! Shared by this module's unit tests and the hardware probe
//! (`validation/probes/x87-extended`): the probe runs the same cases through the machine's x87
//! (C and C++ compiled by GCC 8.4.0) and records the hashes of the hardware's results that the
//! unit tests check on every platform. Inputs come from a SplitMix64 stream and integer
//! arithmetic only, so they are the same everywhere.

use super::F80;

/// SplitMix64 (Steele, Lea and Flood 2014): a small seeded generator.
#[derive(Clone)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> SplitMix64 {
        SplitMix64(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (`n > 0`; the small bias of the modulo does not matter here).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
}

/// FNV-1a over 128-bit words (16 little-endian bytes each): the hash the unit tests compare
/// with the hardware's.
pub fn fnv(words: impl IntoIterator<Item = u128>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for w in words {
        for b in w.to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// An extended number from its sign, exponent field and significand.
pub fn f80(negative: bool, exp_field: u16, m: u64) -> F80 {
    F80::from_bits(((u128::from(exp_field) | if negative { 0x8000 } else { 0 }) << 64) | u128::from(m))
}

/// Special values: zeros, the extremes of every range, infinities, NaNs of both kinds with
/// several payloads and signs, pseudo-denormals and every unsupported encoding.
pub fn specials() -> Vec<F80> {
    const I: u64 = 1 << 63;
    const Q: u64 = 1 << 62;
    let mut v = Vec::new();
    for neg in [false, true] {
        v.extend([
            f80(neg, 0, 0),                     // zero
            f80(neg, 0, 1),                     // smallest denormal
            f80(neg, 0, I - 1),                 // largest denormal
            f80(neg, 0, I),                     // pseudo-denormal (= smallest normal)
            f80(neg, 0, I | 12345),             // pseudo-denormal
            f80(neg, 1, I),                     // smallest normal
            f80(neg, 0x3fff, I),                // 1
            f80(neg, 0x3fff, I | 1),            // 1 + 2^-63
            f80(neg, 0x4000, I | (1 << 62)),    // 3
            f80(neg, 0x3ffe, u64::MAX),         // 1 - 2^-64
            f80(neg, 0x7ffe, u64::MAX),         // largest finite
            f80(neg, 0x403e, I),                // 2^63
            f80(neg, 0x7fff, I),                // infinity
            f80(neg, 0x7fff, I | Q),            // quiet NaN (indefinite when negative)
            f80(neg, 0x7fff, I | Q | 0x1234),   // quiet NaN with a payload
            f80(neg, 0x7fff, I | 1),            // signalling NaN
            f80(neg, 0x7fff, I | 0x00ff_0000),  // signalling NaN with another payload
            f80(neg, 0x7fff, 0),                // pseudo-infinity
            f80(neg, 0x7fff, Q | 5),            // pseudo-NaN
            f80(neg, 0x3fff, Q),                // unnormal
            f80(neg, 0x1234, 1),                // unnormal
        ]);
    }
    v
}

/// The operand families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Random 80-bit patterns (every encoding, unsupported ones included).
    RawBits,
    /// Normal numbers of every exponent.
    Normal,
    /// Normal numbers whose exponents differ by at most 70 (additions with every alignment).
    Near,
    /// Significands with only their top 1 to 64 bits random, exponents differing by at most 70:
    /// exact results and exact ties.
    Sparse,
    /// The second operand within a few units in the last place of the first, either sign:
    /// cancellation in additions and subtractions.
    Cancel,
    /// Both operands near `2^-8191`: products in the denormal range and its boundaries.
    UnderMul,
    /// Quotients in the denormal range: `2^-8191` over `2^8191`.
    UnderDiv,
    /// Both near `2^8191`: products near overflow.
    OverMul,
    /// Quotients near overflow.
    OverDiv,
    /// Denormals, pseudo-denormals and the smallest normals.
    Denormal,
    /// Exact `double` values (the operands of FreeSurfer's `long double` sums of `double`s).
    Doubles,
    /// Exact `float` values.
    Floats,
    /// Integers of up to 40 bits.
    Integers,
    /// Pairs of special values.
    Special,
    /// Values near `double`'s subnormal, normal and overflow boundaries and its rounding ties.
    DoubleEdges,
    /// The same for `float`.
    FloatEdges,
    /// Values near integers, halves and the integer types' limits.
    IntEdges,
}

impl Family {
    pub const ALL: [Family; 17] = [
        Family::RawBits,
        Family::Normal,
        Family::Near,
        Family::Sparse,
        Family::Cancel,
        Family::UnderMul,
        Family::UnderDiv,
        Family::OverMul,
        Family::OverDiv,
        Family::Denormal,
        Family::Doubles,
        Family::Floats,
        Family::Integers,
        Family::Special,
        Family::DoubleEdges,
        Family::FloatEdges,
        Family::IntEdges,
    ];

    /// A pair of operands of this family.
    pub fn pair(self, rng: &mut SplitMix64) -> (F80, F80) {
        const I: u64 = 1 << 63;
        let sign = |rng: &mut SplitMix64| rng.below(2) == 1;
        let normal = |rng: &mut SplitMix64, unbiased: i64| {
            let e = (unbiased + 16383).clamp(1, 0x7ffe) as u16;
            f80(sign(rng), e, I | rng.next_u64())
        };
        match self {
            Family::RawBits => (
                F80::from_bits(u128::from(rng.next_u64()) | (u128::from(rng.next_u64() as u16) << 64)),
                F80::from_bits(u128::from(rng.next_u64()) | (u128::from(rng.next_u64() as u16) << 64)),
            ),
            Family::Normal => {
                let ea = rng.range(-16382, 16383);
                let eb = rng.range(-16382, 16383);
                (normal(rng, ea), normal(rng, eb))
            }
            Family::Near => {
                let ea = rng.range(-16000, 16000);
                let eb = ea + rng.range(-70, 70);
                (normal(rng, ea), normal(rng, eb))
            }
            Family::Sparse => {
                let ea = rng.range(-200, 200);
                let eb = ea + rng.range(-70, 70);
                let sparse = |rng: &mut SplitMix64, e: i64| {
                    let k = rng.range(1, 64) as u32;
                    let top = if k == 64 {
                        rng.next_u64()
                    } else {
                        rng.next_u64() >> (64 - k) << (64 - k)
                    };
                    f80(sign(rng), (e + 16383) as u16, I | top)
                };
                (sparse(rng, ea), sparse(rng, eb))
            }
            Family::Cancel => {
                let ea = rng.range(-16000, 16000);
                let a = normal(rng, ea);
                let bits = a.to_bits();
                let width = rng.range(0, 64) as u32;
                let delta = if width == 0 { 0 } else { rng.next_u64() >> (64 - width) };
                let m = (bits as u64).wrapping_add(delta) | I;
                let se = ((bits >> 64) as u16) ^ if sign(rng) { 0x8000 } else { 0 };
                let se = match rng.below(4) {
                    0 => se + 1,
                    1 => se - 1,
                    _ => se,
                };
                (a, F80::from_bits((u128::from(se) << 64) | u128::from(m)))
            }
            Family::UnderMul => {
                let ea = -8191 + rng.range(-40, 40);
                let eb = -8191 + rng.range(-40, 40);
                (normal(rng, ea), normal(rng, eb))
            }
            Family::UnderDiv => {
                let ea = -8191 + rng.range(-40, 40);
                let eb = 8191 + rng.range(-40, 40);
                (normal(rng, ea), normal(rng, eb))
            }
            Family::OverMul => {
                let ea = 8191 + rng.range(-3, 3);
                let eb = 8191 + rng.range(-3, 3);
                (normal(rng, ea), normal(rng, eb))
            }
            Family::OverDiv => {
                let ea = 8191 + rng.range(-3, 3);
                let eb = -8191 + rng.range(-3, 3);
                (normal(rng, ea), normal(rng, eb))
            }
            Family::Denormal => {
                let one = |rng: &mut SplitMix64| match rng.below(4) {
                    0 => f80(sign(rng), 0, rng.next_u64() >> rng.range(1, 63)),
                    1 => f80(sign(rng), 0, rng.next_u64() | I),
                    2 => f80(sign(rng), 1, rng.next_u64() | I),
                    _ => f80(sign(rng), rng.range(1, 80) as u16, rng.next_u64() | I),
                };
                (one(rng), one(rng))
            }
            Family::Doubles => {
                let d = |rng: &mut SplitMix64| {
                    let e = rng.range(1, 2046) as u64;
                    let bits = (rng.below(2) << 63) | (e << 52) | (rng.next_u64() >> 12);
                    F80::from_f64(f64::from_bits(bits))
                };
                (d(rng), d(rng))
            }
            Family::Floats => {
                let f = |rng: &mut SplitMix64| {
                    let e = rng.range(1, 254) as u32;
                    let bits = ((rng.below(2) as u32) << 31) | (e << 23) | (rng.next_u64() as u32 >> 9);
                    F80::from_f32(f32::from_bits(bits))
                };
                (f(rng), f(rng))
            }
            Family::Integers => {
                let i = |rng: &mut SplitMix64| {
                    let bits = rng.range(0, 40) as u32;
                    let v = if bits == 0 { 0 } else { (rng.next_u64() >> (64 - bits)) as i64 };
                    F80::from_i64(if sign(rng) { -v } else { v })
                };
                (i(rng), i(rng))
            }
            Family::Special => {
                let s = specials();
                let n = s.len() as u64;
                (s[rng.below(n) as usize], s[rng.below(n) as usize])
            }
            Family::DoubleEdges => (edge(rng, 53, 1023), edge(rng, 53, 1023)),
            Family::FloatEdges => (edge(rng, 24, 127), edge(rng, 24, 127)),
            Family::IntEdges => {
                let one = |rng: &mut SplitMix64| {
                    let e = rng.range(-2, 66);
                    let m = I | rng.next_u64();
                    // Keep the bits from just below the binary point up, then a tie, a value
                    // just off it, or an integer.
                    let frac_bits = (63 - e).clamp(0, 63) as u32;
                    let m = match rng.below(4) {
                        0 if frac_bits > 0 => (m >> frac_bits << frac_bits) | (1 << (frac_bits - 1)),
                        1 if frac_bits > 0 => m >> frac_bits << frac_bits,
                        _ => m,
                    };
                    f80(sign(rng), (e + 16383) as u16, m | I)
                };
                (one(rng), one(rng))
            }
        }
    }
}

/// A value near the boundaries of a narrower format (`p` significand bits, exponent bias
/// `bias`): its overflow, its subnormal range and its rounding ties.
fn edge(rng: &mut SplitMix64, p: u32, bias: i64) -> F80 {
    const I: u64 = 1 << 63;
    let e = match rng.below(4) {
        0 => bias + rng.range(-3, 1),                 // near overflow
        1 => 1 - bias + rng.range(-(p as i64) - 3, 3), // subnormal and smallest normal
        2 => 1 - bias - p as i64 + rng.range(-3, 1),  // below the smallest subnormal
        _ => rng.range(1 - bias, bias),
    };
    let drop = 64 - p;
    let mut m = I | rng.next_u64();
    let low = match rng.below(6) {
        0 => 1u64 << (drop - 1),       // a tie
        1 => (1u64 << (drop - 1)) + 1, // just above
        2 => (1u64 << (drop - 1)) - 1, // just below
        3 => 0,                        // exact
        _ => m & ((1u64 << drop) - 1),
    };
    if rng.below(8) == 0 {
        m |= !0u64 << drop; // all ones above: rounding carries
    }
    m = (m >> drop << drop) | low;
    f80(rng.below(2) == 1, (e + 16383).clamp(1, 0x7ffe) as u16, m)
}

/// The operations the probe checks, in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Sqrt,
    /// The six comparisons as bits: `<`, `<=`, `==`, `!=`, `>`, `>=`.
    Cmp,
    ToF64,
    ToF32,
    ToI64,
    ToI32,
    ToI16,
    ToU64,
    ToU32,
    /// `llrintl`.
    RoundToI64,
}

impl Op {
    pub const ALL: [Op; 14] = [
        Op::Add,
        Op::Sub,
        Op::Mul,
        Op::Div,
        Op::Sqrt,
        Op::Cmp,
        Op::ToF64,
        Op::ToF32,
        Op::ToI64,
        Op::ToI32,
        Op::ToI16,
        Op::ToU64,
        Op::ToU32,
        Op::RoundToI64,
    ];

    /// This operation in software, as one word: an extended result's 80 bits ([`F80::to_bits`]),
    /// a narrower result's bits zero-extended.
    pub fn apply(self, a: F80, b: F80) -> u128 {
        match self {
            Op::Add => (a + b).to_bits(),
            Op::Sub => (a - b).to_bits(),
            Op::Mul => (a * b).to_bits(),
            Op::Div => (a / b).to_bits(),
            Op::Sqrt => a.sqrt().to_bits(),
            Op::Cmp => u128::from(
                u8::from(a < b)
                    | u8::from(a <= b) << 1
                    | u8::from(a == b) << 2
                    | u8::from(a != b) << 3
                    | u8::from(a > b) << 4
                    | u8::from(a >= b) << 5,
            ),
            Op::ToF64 => a.to_f64().to_bits().into(),
            Op::ToF32 => a.to_f32().to_bits().into(),
            Op::ToI64 => (a.to_i64_trunc() as u64).into(),
            Op::ToI32 => (a.to_i32_trunc() as u32).into(),
            Op::ToI16 => (a.to_i16_trunc() as u16).into(),
            Op::ToU64 => a.to_u64_trunc().into(),
            Op::ToU32 => a.to_u32_trunc().into(),
            Op::RoundToI64 => (a.round_ties_even_to_i64() as u64).into(),
        }
    }

    /// Whether the operation reads its second operand.
    pub fn binary(self) -> bool {
        matches!(self, Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Cmp)
    }
}

/// The seed of an operation's cases in a family.
pub fn seed(op: usize, family: usize) -> u64 {
    0x0087_0000_0000_0000 ^ ((op as u64) << 16) ^ family as u64
}

/// The conversions into the extended format the probe checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conv {
    /// `double` bit patterns (every encoding).
    F64,
    /// `float` bit patterns.
    F32,
    I64,
    I32,
    U64,
    U32,
}

impl Conv {
    pub const ALL: [Conv; 6] = [Conv::F64, Conv::F32, Conv::I64, Conv::I32, Conv::U64, Conv::U32];

    /// A random input word of this conversion (the low bits for the narrower types).
    pub fn input(self, rng: &mut SplitMix64) -> u64 {
        let x = rng.next_u64();
        // A quarter of the inputs small or special: exponent fields 0 and all ones, small integers.
        match (self, rng.below(4)) {
            (Conv::F64, 0) => x & 0x800f_ffff_ffff_ffff | if rng.below(2) == 1 { 0x7ff0 << 48 } else { 0 },
            (Conv::F32, 0) => (x as u32 & 0x807f_ffff | if rng.below(2) == 1 { 0x7f80_0000 } else { 0 }) as u64,
            (Conv::F32, _) => x as u32 as u64,
            (Conv::I32 | Conv::U32, 0) => (x >> rng.range(32, 63)) as u32 as u64,
            (Conv::I32 | Conv::U32, _) => x as u32 as u64,
            (Conv::I64 | Conv::U64, 0) => x >> rng.range(1, 63),
            _ => x,
        }
    }

    /// The conversion in software, as [`F80::to_bits`].
    pub fn apply(self, x: u64) -> u128 {
        let v = match self {
            Conv::F64 => F80::from_f64(f64::from_bits(x)),
            Conv::F32 => F80::from_f32(f32::from_bits(x as u32)),
            Conv::I64 => F80::from_i64(x as i64),
            Conv::I32 => F80::from(x as u32 as i32),
            Conv::U64 => F80::from_u64(x),
            Conv::U32 => F80::from(x as u32),
        };
        v.to_bits()
    }
}

/// The seed of a conversion's cases.
pub fn from_seed(from: usize) -> u64 {
    0x0087_f000_0000_0000 ^ from as u64
}

/// Arithmetic patterns of the FreeSurfer functions that compute in `long double`
/// (`docs/freesurfer/variation.md` §1), written as plain loops: what the probe's C++ computes
/// with GCC 8.4.0 and FreeSurfer's flags, here with [`F80`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// `s += (long double)a[i] * b[i]` over `double`s, returned as `double` (DVectorDot,
    /// DMatrixMultiply).
    Dot,
    /// The sum and the sum of squares of `double`s in `long double`, their mean, variance and
    /// standard deviation (DListStats-like).
    Stats,
    /// Edge vectors from `float` coordinates in `long double`, their cross product and its length
    /// (MRISfaceNormalFace-like).
    Cross,
    /// An edge vector from `double`s, its length, unit vector and gradient `w wᵀ / L³ - I / L`
    /// (SurfGradUnitVector-like).
    Unit,
    /// `float` values summed and their `float` squares summed in `long double`, each value divided
    /// by the sum and stored as `float` (MRIgaussianSmoothNI-like).
    Smooth,
    /// `double` differences squared and summed in `long double`, a `2 dL / len` factor
    /// (MRISedgeLengthCost-like).
    EdgeCost,
}

impl Pattern {
    pub const ALL: [Pattern; 6] = [
        Pattern::Dot,
        Pattern::Stats,
        Pattern::Cross,
        Pattern::Unit,
        Pattern::Smooth,
        Pattern::EdgeCost,
    ];

    /// One case's inputs: `doubles` and `floats` (the probe passes them to the C++), and `n`.
    pub fn inputs(self, rng: &mut SplitMix64) -> PatternInput {
        let d = |rng: &mut SplitMix64, scale: i64| {
            // A double of moderate size: random significand, exponent within ±scale.
            let e = (1023 + rng.range(-scale, scale)) as u64;
            f64::from_bits((rng.below(2) << 63) | (e << 52) | (rng.next_u64() >> 12))
        };
        let f = |rng: &mut SplitMix64| {
            // Coordinates in millimetres: |x| < 128, 2^-10 granularity or full float.
            let v = (rng.next_u64() >> 46) as i64 - (1 << 17);
            let x = v as f32 / 1024.0;
            if rng.below(2) == 0 {
                x
            } else {
                f32::from_bits(x.to_bits() ^ (rng.next_u64() as u32 & 0x3ff))
            }
        };
        match self {
            Pattern::Dot => {
                let n = rng.range(1, 64) as usize;
                PatternInput {
                    n,
                    doubles: (0..2 * n).map(|_| d(rng, 30)).collect(),
                    floats: vec![],
                }
            }
            Pattern::Stats => {
                let n = rng.range(2, 64) as usize;
                let base = d(rng, 10);
                PatternInput {
                    n,
                    doubles: (0..n).map(|_| base + d(rng, 3)).collect(),
                    floats: vec![],
                }
            }
            Pattern::Cross => PatternInput {
                n: 1,
                doubles: vec![],
                floats: (0..9).map(|_| f(rng)).collect(),
            },
            Pattern::Unit => PatternInput {
                n: 1,
                doubles: (0..6).map(|_| d(rng, 8)).collect(),
                floats: vec![],
            },
            Pattern::Smooth => {
                // Kernel weights: positive floats from 2^-40 to 1, full significands, so that
                // their sums need more than double's 53 bits.
                let n = rng.range(1, 48) as usize;
                PatternInput {
                    n,
                    doubles: vec![],
                    floats: (0..n)
                        .map(|_| {
                            let e = (127 - rng.range(1, 40)) as u32;
                            f32::from_bits((e << 23) | (rng.next_u64() as u32 >> 9))
                        })
                        .collect(),
                }
            }
            Pattern::EdgeCost => {
                let n = rng.range(1, 64) as usize;
                let l0 = d(rng, 2).abs();
                let mut doubles = vec![l0];
                doubles.extend((0..n).map(|_| l0 + d(rng, 4)));
                PatternInput {
                    n,
                    doubles,
                    floats: vec![],
                }
            }
        }
    }

    /// The pattern in software: its outputs as words (`double` and `float` bits).
    pub fn apply(self, x: &PatternInput) -> Vec<u64> {
        let ld = F80::from_f64;
        let lf = F80::from_f32;
        let n = x.n;
        match self {
            Pattern::Dot => {
                let mut s = F80::ZERO;
                for i in 0..n {
                    s += ld(x.doubles[i]) * ld(x.doubles[n + i]);
                }
                vec![s.to_f64().to_bits()]
            }
            Pattern::Stats => {
                let (mut sum, mut sum2) = (F80::ZERO, F80::ZERO);
                for &v in &x.doubles[..n] {
                    sum += ld(v);
                    sum2 += ld(v) * ld(v);
                }
                let nn = F80::from(n as i32);
                let mean = sum / nn;
                let var = (sum2 - nn * mean * mean) / F80::from(n as i32 - 1);
                let std = var.sqrt();
                vec![mean.to_f64().to_bits(), var.to_f64().to_bits(), std.to_f64().to_bits()]
            }
            Pattern::Cross => {
                let v = &x.floats;
                let a = [lf(v[3]) - lf(v[0]), lf(v[4]) - lf(v[1]), lf(v[5]) - lf(v[2])];
                let b = [lf(v[6]) - lf(v[3]), lf(v[7]) - lf(v[4]), lf(v[8]) - lf(v[5])];
                let cx = a[1] * b[2] - a[2] * b[1];
                let cy = a[2] * b[0] - a[0] * b[2];
                let cz = a[0] * b[1] - a[1] * b[0];
                let cl = (cx * cx + cy * cy + cz * cz).sqrt();
                [cx, cy, cz, cl].iter().map(|c| c.to_f64().to_bits()).collect()
            }
            Pattern::Unit => {
                let v = &x.doubles;
                let mut w = [F80::ZERO; 3];
                let mut l = F80::ZERO;
                for k in 0..3 {
                    w[k] = ld(v[3 + k]) - ld(v[k]);
                    l += w[k] * w[k];
                }
                let l = l.sqrt();
                let mut out = vec![l.to_f64().to_bits()];
                for wk in w {
                    out.push((wk / l).to_f64().to_bits());
                }
                let il3 = ld(1.0) / (l * l * l);
                let il = ld(1.0) / l;
                for k in 0..3 {
                    for m in 0..3 {
                        let mut a = (w[k] * w[m]) * il3;
                        if k == m {
                            a -= il;
                        }
                        out.push(a.to_f64().to_bits());
                    }
                }
                out
            }
            Pattern::Smooth => {
                let (mut scale, mut vmf) = (F80::ZERO, F80::ZERO);
                for &v in &x.floats[..n] {
                    scale += lf(v);
                    vmf += lf(v * v);
                }
                let mut out: Vec<u64> = x.floats[..n]
                    .iter()
                    .map(|&v| u64::from((lf(v) / scale).to_f32().to_bits()))
                    .collect();
                out.push(vmf.to_f64().to_bits());
                out.push((ld(1.0) / vmf).to_f64().to_bits());
                out
            }
            Pattern::EdgeCost => {
                let l0 = x.doubles[0];
                let mut total = F80::ZERO;
                let mut out = Vec::with_capacity(n + 1);
                for &len in &x.doubles[1..=n] {
                    let dl = ld(len - l0);
                    total += dl * dl;
                    let a = ld(2.0) * dl / ld(len);
                    out.push(a.to_f64().to_bits());
                }
                out.push(total.to_f64().to_bits());
                out
            }
        }
    }
}

/// One pattern case's inputs.
#[derive(Clone, Debug)]
pub struct PatternInput {
    pub n: usize,
    pub doubles: Vec<f64>,
    pub floats: Vec<f32>,
}

/// The seed of a pattern's cases.
pub fn pattern_seed(pattern: usize) -> u64 {
    0x0087_c000_0000_0000 ^ pattern as u64
}
