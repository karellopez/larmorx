// SPDX-License-Identifier: Apache-2.0
//! The x87's 80-bit extended format, `long double` on x86-64 Linux, in software.
//!
//! Rust has no 80-bit floating-point type, but FreeSurfer (and other C and C++ programs built for
//! x86-64 Linux) computes some sums, products and lengths in `long double`, which GCC evaluates
//! with the x87 floating-point unit. A replica that must give the same bits needs the x87's
//! arithmetic: [`F80`] is that arithmetic, written from Intel's description of the x87 (*Intel 64
//! and IA-32 Architectures Software Developer's Manual*, volume 1, chapters 4 and 8: the extended
//! format, the special encodings, the NaN rules, the masked responses) and IEEE 754's
//! definition of correctly rounded operations. No upstream code is translated.
//!
//! **The mode.** Every operation rounds as the x87 does under the control word Linux gives every
//! new process (`0x037f`): precision control 64 bits (the full significand), rounding to nearest
//! with ties to even, every exception masked. So:
//!
//! - addition, subtraction, multiplication, division and square root give the correctly rounded
//!   result of the exact operation, with the extended format's 15-bit exponent range: results
//!   below the smallest normal number are rounded once to the denormal grid (`2^-16445`), results
//!   above the largest finite number become infinities;
//! - invalid operations (`∞ - ∞`, `0 × ∞`, `0 / 0`, `∞ / ∞`, the square root of a negative number,
//!   any operand in an encoding the x87 no longer supports: pseudo-NaN, pseudo-infinity, unnormal)
//!   give the *real indefinite*, the quiet NaN with the sign set and significand `0xC000…0`
//!   ([`F80::INDEFINITE`]); a finite number divided by zero gives an infinity;
//! - NaN operands propagate by the x87's rules: a signalling NaN is made quiet (bit 62 set); of a
//!   quiet and a signalling NaN the quiet one is the result; of two NaNs of the same kind the one
//!   with the larger significand, and with equal significands the positive one;
//! - results are always in the canonical encoding (the x87 reads pseudo-denormals, exponent field 0
//!   with the integer bit set, as numbers of exponent 1, and never writes them).
//!
//! **Conversions** are those GCC emits on x86-64: from `float`, `double` and the integer types
//! exact (`fld`, `fild`; a signalling NaN is made quiet, a NaN keeps its payload in the high bits);
//! to `float` and `double` correctly rounded with the target's range, subnormals and overflow (`fst`;
//! a NaN keeps the high bits of its payload and is made quiet; an unsupported encoding gives the
//! target's indefinite); to the integer types by truncation toward zero (`fistp` under a rounding
//! control set to *chop*), with the x87's *integer indefinite* (the most negative value of the
//! stored width) for NaNs, infinities and values out of range, and GCC's own sequences for the
//! unsigned types ([`F80::to_u64_trunc`], [`F80::to_u32_trunc`]).
//!
//! **Comparisons** ([`PartialEq`], [`PartialOrd`]) are IEEE 754's (`+0 == -0`; a NaN or an
//! unsupported encoding is unordered with everything), as `fucomi`/`fcomi` with GCC's tests give C's
//! `==`, `!=`, `<`, `<=`, `>`, `>=`.
//!
//! **Checked against the hardware** (`validation/probes/x87-extended`): every operation and
//! conversion on seeded operands of many families (random bit patterns, normals of every exponent,
//! cancellations, exact results and ties, denormals and the underflow and overflow boundaries,
//! integers, special values and every unsupported encoding) and the arithmetic patterns of the
//! FreeSurfer functions that compute in `long double`, compiled by GCC 8.4.0 with FreeSurfer's
//! flags; the unit tests check the hardware's recorded hashes on every platform.

pub mod cases;

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// The exponent bias of the extended format.
const BIAS: i32 = 16383;
/// The exponent field of infinities and NaNs.
const EXP_SPECIAL: u16 = 0x7fff;
/// The explicit integer bit of the significand.
const INT_BIT: u64 = 1 << 63;
/// The quiet bit of a NaN's significand.
const QUIET_BIT: u64 = 1 << 62;
/// The binary exponent of the significand's last bit for exponent field 0 and 1: `1 - 16383 - 63`.
const DENORMAL_LSB: i32 = 1 - BIAS - 63;

/// A number in the x87's 80-bit extended format: a sign, a 15-bit biased exponent and a 64-bit
/// significand with an explicit integer bit, as `long double` is stored on x86-64 (the first ten
/// bytes of its sixteen, little-endian).
///
/// Equality and order are IEEE 754's (`==` compares values, not bits): use [`F80::to_bits`] to
/// compare encodings.
#[derive(Clone, Copy, Default)]
pub struct F80 {
    /// The significand, its integer bit included (bit 63).
    m: u64,
    /// The sign (bit 15) and the biased exponent (bits 0 to 14).
    se: u16,
}

/// What an encoding means to the x87.
#[derive(Clone, Copy)]
enum Kind {
    Zero,
    /// A nonzero finite number `sig · 2^exp`, `sig` normalised (bit 63 set): normals, denormals
    /// and pseudo-denormals alike.
    Finite {
        sig: u64,
        exp: i32,
    },
    Inf,
    QNan,
    SNan,
    /// Pseudo-NaN, pseudo-infinity and unnormal: the x87 refuses them since the 80387 (an
    /// invalid operation).
    Unsupported,
}

impl F80 {
    /// `+0`.
    pub const ZERO: F80 = F80 { m: 0, se: 0 };
    /// `-0`.
    pub const NEG_ZERO: F80 = F80 { m: 0, se: 0x8000 };
    /// `1`.
    pub const ONE: F80 = F80 {
        m: INT_BIT,
        se: BIAS as u16,
    };
    /// `+∞`.
    pub const INFINITY: F80 = F80 {
        m: INT_BIT,
        se: EXP_SPECIAL,
    };
    /// `-∞`.
    pub const NEG_INFINITY: F80 = F80 {
        m: INT_BIT,
        se: 0x8000 | EXP_SPECIAL,
    };
    /// The x87's real indefinite: the quiet NaN every invalid operation returns (sign set,
    /// significand `0xC000_0000_0000_0000`).
    pub const INDEFINITE: F80 = F80 {
        m: INT_BIT | QUIET_BIT,
        se: 0x8000 | EXP_SPECIAL,
    };
    /// The largest finite number, `(2 - 2^-63) · 2^16383`.
    pub const MAX: F80 = F80 {
        m: u64::MAX,
        se: EXP_SPECIAL - 1,
    };
    /// The smallest positive normal number, `2^-16382`.
    pub const MIN_POSITIVE: F80 = F80 { m: INT_BIT, se: 1 };
    /// The smallest positive denormal number, `2^-16445`.
    pub const MIN_POSITIVE_DENORMAL: F80 = F80 { m: 1, se: 0 };
    /// The distance from 1 to the next number, `2^-63`.
    pub const EPSILON: F80 = F80 {
        m: INT_BIT,
        se: (BIAS - 63) as u16,
    };

    /// The number with this encoding: bits 0 to 63 the significand, 64 to 78 the exponent, 79 the
    /// sign (higher bits ignored).
    pub const fn from_bits(bits: u128) -> F80 {
        F80 {
            m: bits as u64,
            se: (bits >> 64) as u16,
        }
    }

    /// The encoding, as [`F80::from_bits`] reads it.
    pub const fn to_bits(self) -> u128 {
        ((self.se as u128) << 64) | self.m as u128
    }

    /// The number from its ten bytes as stored in memory (little-endian).
    pub fn from_le_bytes(b: [u8; 10]) -> F80 {
        let mut m = [0u8; 8];
        m.copy_from_slice(&b[..8]);
        F80 {
            m: u64::from_le_bytes(m),
            se: u16::from_le_bytes([b[8], b[9]]),
        }
    }

    /// The ten bytes stored in memory (little-endian).
    pub fn to_le_bytes(self) -> [u8; 10] {
        let mut b = [0u8; 10];
        b[..8].copy_from_slice(&self.m.to_le_bytes());
        b[8..].copy_from_slice(&self.se.to_le_bytes());
        b
    }

    /// The sign bit is set (also for NaNs and zeros).
    pub const fn is_sign_negative(self) -> bool {
        self.se & 0x8000 != 0
    }

    /// A NaN, or an encoding the x87 does not support: what C's `isnan` reports (both are
    /// unordered with themselves).
    pub fn is_nan(self) -> bool {
        matches!(self.kind(), Kind::QNan | Kind::SNan | Kind::Unsupported)
    }

    /// `+∞` or `-∞`.
    pub fn is_infinite(self) -> bool {
        matches!(self.kind(), Kind::Inf)
    }

    /// Zero or a finite number (denormals and pseudo-denormals included).
    pub fn is_finite(self) -> bool {
        matches!(self.kind(), Kind::Zero | Kind::Finite { .. })
    }

    /// The absolute value: the sign bit cleared (`fabs`; NaNs keep their payload).
    pub const fn abs(self) -> F80 {
        F80 {
            m: self.m,
            se: self.se & 0x7fff,
        }
    }

    fn kind(self) -> Kind {
        let e = self.se & EXP_SPECIAL;
        let m = self.m;
        if e == 0 {
            if m == 0 {
                Kind::Zero
            } else {
                let lz = m.leading_zeros();
                Kind::Finite {
                    sig: m << lz,
                    exp: DENORMAL_LSB - lz as i32,
                }
            }
        } else if m & INT_BIT == 0 {
            Kind::Unsupported
        } else if e == EXP_SPECIAL {
            if m == INT_BIT {
                Kind::Inf
            } else if m & QUIET_BIT != 0 {
                Kind::QNan
            } else {
                Kind::SNan
            }
        } else {
            Kind::Finite {
                sig: m,
                exp: e as i32 - BIAS - 63,
            }
        }
    }

    const fn signed_zero(negative: bool) -> F80 {
        F80 {
            m: 0,
            se: if negative { 0x8000 } else { 0 },
        }
    }

    const fn signed_inf(negative: bool) -> F80 {
        F80 {
            m: INT_BIT,
            se: if negative { 0x8000 } else { 0 } | EXP_SPECIAL,
        }
    }

    const fn quieted(self) -> F80 {
        F80 {
            m: self.m | QUIET_BIT,
            se: self.se,
        }
    }

    /// The x87's result when an operand of a two-operand instruction is a NaN or unsupported,
    /// `None` otherwise.
    fn special_operands(a: F80, ka: Kind, b: F80, kb: Kind) -> Option<F80> {
        let nan = |k: Kind| matches!(k, Kind::QNan | Kind::SNan);
        if matches!(ka, Kind::Unsupported) || matches!(kb, Kind::Unsupported) {
            return Some(F80::INDEFINITE);
        }
        match (nan(ka), nan(kb)) {
            (false, false) => None,
            (true, false) => Some(a.quieted()),
            (false, true) => Some(b.quieted()),
            (true, true) => Some(match (ka, kb) {
                (Kind::QNan, Kind::SNan) => a,
                (Kind::SNan, Kind::QNan) => b,
                _ => match a.m.cmp(&b.m) {
                    Ordering::Greater => a.quieted(),
                    Ordering::Less => b.quieted(),
                    Ordering::Equal => {
                        if a.is_sign_negative() {
                            b.quieted()
                        } else {
                            a.quieted()
                        }
                    }
                },
            }),
        }
    }

    /// `a + b` (`negate_b` false) or `a - b` (true), as `fadd` and `fsub` give them.
    fn add_sub(a: F80, b: F80, negate_b: bool) -> F80 {
        let (ka, kb) = (a.kind(), b.kind());
        if let Some(r) = F80::special_operands(a, ka, b, kb) {
            return r;
        }
        let sa = a.is_sign_negative();
        let sb = b.is_sign_negative() ^ negate_b;
        match (ka, kb) {
            (Kind::Inf, Kind::Inf) => {
                if sa == sb {
                    F80::signed_inf(sa)
                } else {
                    F80::INDEFINITE
                }
            }
            (Kind::Inf, _) => F80::signed_inf(sa),
            (_, Kind::Inf) => F80::signed_inf(sb),
            (Kind::Zero, Kind::Zero) => F80::signed_zero(sa && sb),
            (Kind::Zero, Kind::Finite { sig, exp }) => round_pack(sb, sig as u128, exp, false),
            (Kind::Finite { sig, exp }, Kind::Zero) => round_pack(sa, sig as u128, exp, false),
            (Kind::Finite { sig: ma, exp: ea }, Kind::Finite { sig: mb, exp: eb }) => {
                // The larger magnitude first (normalised significands: exponent, then significand).
                let ((sa, ma, ea), (sb, mb, eb)) = if (ea, ma) >= (eb, mb) {
                    ((sa, ma, ea), (sb, mb, eb))
                } else {
                    ((sb, mb, eb), (sa, ma, ea))
                };
                // 63 guard bits below each significand; the bits of the smaller operand shifted
                // out of the 128-bit window (only when the exponents differ by 64 or more) are
                // kept as a sticky bit in bit 0, far below the rounding position.
                let x = (ma as u128) << 63;
                let (mut y, sticky) = shr_sticky((mb as u128) << 63, (ea - eb) as u32);
                if sticky {
                    y |= 1;
                }
                if sa == sb {
                    round_pack(sa, x + y, ea - 63, false)
                } else if x == y {
                    F80::ZERO
                } else {
                    round_pack(sa, x - y, ea - 63, false)
                }
            }
            _ => unreachable!("NaNs and unsupported encodings handled above"),
        }
    }

    /// `self · rhs`, as `fmul` gives it.
    fn mul_impl(self, rhs: F80) -> F80 {
        let (ka, kb) = (self.kind(), rhs.kind());
        if let Some(r) = F80::special_operands(self, ka, rhs, kb) {
            return r;
        }
        let s = self.is_sign_negative() ^ rhs.is_sign_negative();
        match (ka, kb) {
            (Kind::Zero, Kind::Inf) | (Kind::Inf, Kind::Zero) => F80::INDEFINITE,
            (Kind::Inf, _) | (_, Kind::Inf) => F80::signed_inf(s),
            (Kind::Zero, _) | (_, Kind::Zero) => F80::signed_zero(s),
            (Kind::Finite { sig: ma, exp: ea }, Kind::Finite { sig: mb, exp: eb }) => {
                round_pack(s, ma as u128 * mb as u128, ea + eb, false)
            }
            _ => unreachable!("NaNs and unsupported encodings handled above"),
        }
    }

    /// `self / rhs`, as `fdiv` gives it.
    fn div_impl(self, rhs: F80) -> F80 {
        let (ka, kb) = (self.kind(), rhs.kind());
        if let Some(r) = F80::special_operands(self, ka, rhs, kb) {
            return r;
        }
        let s = self.is_sign_negative() ^ rhs.is_sign_negative();
        match (ka, kb) {
            (Kind::Zero, Kind::Zero) | (Kind::Inf, Kind::Inf) => F80::INDEFINITE,
            (Kind::Inf, _) => F80::signed_inf(s),
            (_, Kind::Inf) | (Kind::Zero, _) => F80::signed_zero(s),
            (_, Kind::Zero) => F80::signed_inf(s),
            (Kind::Finite { sig: ma, exp: ea }, Kind::Finite { sig: mb, exp: eb }) => {
                // ma / mb lies in (1/2, 2): the first quotient has 64 or 65 bits.
                let num = (ma as u128) << 64;
                let den = mb as u128;
                let q1 = num / den;
                let r1 = num % den;
                if q1 >> 64 != 0 {
                    // 65 bits: one guard bit, the remainder as sticky.
                    round_pack(s, q1, ea - eb - 64, r1 != 0)
                } else {
                    // 64 bits: 64 more quotient bits, the remainder as sticky.
                    let num2 = r1 << 64;
                    let q2 = num2 / den;
                    let r2 = num2 % den;
                    round_pack(s, (q1 << 64) | q2, ea - eb - 128, r2 != 0)
                }
            }
            _ => unreachable!("NaNs and unsupported encodings handled above"),
        }
    }

    /// The square root, correctly rounded (`fsqrt`; glibc's `sqrtl` and GCC's `__builtin_sqrtl`
    /// give the same bits): `sqrt(-0) = -0`, the square root of a negative number (`-∞` included)
    /// the real indefinite.
    pub fn sqrt(self) -> F80 {
        match self.kind() {
            Kind::Zero => self,
            Kind::Inf => {
                if self.is_sign_negative() {
                    F80::INDEFINITE
                } else {
                    self
                }
            }
            Kind::QNan => self,
            Kind::SNan => self.quieted(),
            Kind::Unsupported => F80::INDEFINITE,
            Kind::Finite { sig, exp } => {
                if self.is_sign_negative() {
                    return F80::INDEFINITE;
                }
                // N = sig · 2^s with exp - s even and N in [2^126, 2^128): sqrt(N) in [2^63, 2^64).
                let s: i32 = if exp & 1 != 0 { 63 } else { 64 };
                let n = (sig as u128) << s;
                let q = n.isqrt();
                let r = n - q * q;
                // sqrt(N) > q + 1/2 exactly when r > q; a tie is impossible (a midpoint's square
                // has more bits than N).
                let m = q + u128::from(r > q);
                round_pack(false, m, (exp - s) / 2, false)
            }
        }
    }

    /// The exact value of a `double` (`fldl`): a signalling NaN is made quiet, a NaN's payload
    /// goes to the significand's high bits.
    pub fn from_f64(x: f64) -> F80 {
        let bits = x.to_bits();
        let negative = bits >> 63 != 0;
        let e = ((bits >> 52) & 0x7ff) as i32;
        let f = bits & ((1 << 52) - 1);
        let sign = if negative { 0x8000 } else { 0 };
        if e == 0x7ff {
            if f == 0 {
                F80::signed_inf(negative)
            } else {
                F80 {
                    m: INT_BIT | QUIET_BIT | (f << 11),
                    se: sign | EXP_SPECIAL,
                }
            }
        } else if e == 0 {
            if f == 0 {
                F80::signed_zero(negative)
            } else {
                round_pack(negative, f as u128, -1074, false)
            }
        } else {
            F80 {
                m: (f | (1 << 52)) << 11,
                se: sign | (e - 1023 + BIAS) as u16,
            }
        }
    }

    /// The exact value of a `float` (`flds`), as [`F80::from_f64`].
    pub fn from_f32(x: f32) -> F80 {
        let bits = x.to_bits();
        let negative = bits >> 31 != 0;
        let e = ((bits >> 23) & 0xff) as i32;
        let f = (bits & ((1 << 23) - 1)) as u64;
        let sign = if negative { 0x8000 } else { 0 };
        if e == 0xff {
            if f == 0 {
                F80::signed_inf(negative)
            } else {
                F80 {
                    m: INT_BIT | QUIET_BIT | (f << 40),
                    se: sign | EXP_SPECIAL,
                }
            }
        } else if e == 0 {
            if f == 0 {
                F80::signed_zero(negative)
            } else {
                round_pack(negative, f as u128, -149, false)
            }
        } else {
            F80 {
                m: (f | (1 << 23)) << 40,
                se: sign | (e - 127 + BIAS) as u16,
            }
        }
    }

    /// The exact value of a signed integer (`fild`; zero is `+0`).
    pub fn from_i64(x: i64) -> F80 {
        if x == 0 {
            F80::ZERO
        } else {
            round_pack(x < 0, x.unsigned_abs() as u128, 0, false)
        }
    }

    /// The exact value of an unsigned integer (GCC: `fildq`, then `+ 2^64` when bit 63 is set).
    pub fn from_u64(x: u64) -> F80 {
        if x == 0 {
            F80::ZERO
        } else {
            round_pack(false, x as u128, 0, false)
        }
    }

    /// The nearest `double` (`fstpl`): ties to even, `double`'s subnormals, overflow to infinity; a
    /// NaN keeps its payload's high 51 bits and is made quiet; an unsupported encoding gives
    /// `double`'s indefinite (`-NaN`, `0xfff8_0000_0000_0000`).
    pub fn to_f64(self) -> f64 {
        let negative = self.is_sign_negative();
        let sign = (negative as u64) << 63;
        match self.kind() {
            Kind::Zero => f64::from_bits(sign),
            Kind::Inf => f64::from_bits(sign | (0x7ff << 52)),
            Kind::QNan | Kind::SNan => f64::from_bits(
                sign | (0x7ff << 52) | (1 << 51) | ((self.m >> 11) & ((1 << 52) - 1)),
            ),
            Kind::Unsupported => f64::from_bits(0xfff8_0000_0000_0000),
            Kind::Finite { sig, exp } => {
                match round_format(sig as u128, exp, false, 53, 1023, 2047) {
                    None => f64::from_bits(sign | (0x7ff << 52)),
                    Some((be, q)) => {
                        f64::from_bits(sign | ((be as u64) << 52) | (q as u64 & ((1 << 52) - 1)))
                    }
                }
            }
        }
    }

    /// The nearest `float` (`fstps`), as [`F80::to_f64`] (`float`'s indefinite `0xffc0_0000`).
    pub fn to_f32(self) -> f32 {
        let negative = self.is_sign_negative();
        let sign = (negative as u32) << 31;
        match self.kind() {
            Kind::Zero => f32::from_bits(sign),
            Kind::Inf => f32::from_bits(sign | (0xff << 23)),
            Kind::QNan | Kind::SNan => f32::from_bits(
                sign | (0xff << 23) | (1 << 22) | ((self.m >> 40) as u32 & ((1 << 23) - 1)),
            ),
            Kind::Unsupported => f32::from_bits(0xffc0_0000),
            Kind::Finite { sig, exp } => {
                match round_format(sig as u128, exp, false, 24, 127, 255) {
                    None => f32::from_bits(sign | (0xff << 23)),
                    Some((be, q)) => {
                        f32::from_bits(sign | ((be as u32) << 23) | (q as u32 & ((1 << 23) - 1)))
                    }
                }
            }
        }
    }

    /// The value truncated toward zero, if it is finite and its integer part lies in
    /// `[-2^63, 2^63)`; `None` otherwise.
    pub fn trunc_to_i64_checked(self) -> Option<i64> {
        match self.kind() {
            Kind::Zero => Some(0),
            Kind::Finite { sig, exp } => {
                let negative = self.is_sign_negative();
                if exp >= 0 {
                    // |value| >= 2^63: only -2^63 itself fits.
                    (negative && sig == INT_BIT && exp == 0).then_some(i64::MIN)
                } else if exp <= -64 {
                    Some(0)
                } else {
                    let mag = sig >> (-exp) as u32;
                    Some(if negative { -(mag as i64) } else { mag as i64 })
                }
            }
            _ => None,
        }
    }

    /// C's `(long)x` as GCC compiles it (`fistpq` under truncation): the x87's integer indefinite
    /// `i64::MIN` for NaNs, infinities and values out of range.
    pub fn to_i64_trunc(self) -> i64 {
        self.trunc_to_i64_checked().unwrap_or(i64::MIN)
    }

    /// C's `(int)x` (`fistpl` under truncation): `i32::MIN` for NaNs, infinities and values out of
    /// range.
    pub fn to_i32_trunc(self) -> i32 {
        self.trunc_to_i64_checked()
            .and_then(|v| i32::try_from(v).ok())
            .unwrap_or(i32::MIN)
    }

    /// C's `(short)x` (`fistps` under truncation): `i16::MIN` for NaNs, infinities and values out
    /// of range. C's `(signed char)x` and `(unsigned char)x` are its low 8 bits (GCC stores 16).
    pub fn to_i16_trunc(self) -> i16 {
        self.trunc_to_i64_checked()
            .and_then(|v| i16::try_from(v).ok())
            .unwrap_or(i16::MIN)
    }

    /// C's `(unsigned int)x` as GCC compiles it: `fistpq` under truncation, then the low 32 bits
    /// (so `0` for NaNs, infinities and values out of `i64`'s range).
    pub fn to_u32_trunc(self) -> u32 {
        self.to_i64_trunc() as u32
    }

    /// C's `(unsigned long)x` as GCC compiles it: below `2^63` (or unordered) `fistpq` under
    /// truncation, read as unsigned; from `2^63` the same of `x - 2^63` with bit 63 flipped
    /// (so `0` from `2^64` and for `+∞`, `2^63` for NaNs and values below `-2^63`).
    pub fn to_u64_trunc(self) -> u64 {
        let two63 = F80 {
            m: INT_BIT,
            se: (BIAS + 63) as u16,
        };
        if self >= two63 {
            ((self - two63).to_i64_trunc() as u64) ^ (1 << 63)
        } else {
            self.to_i64_trunc() as u64
        }
    }

    /// The value rounded to the nearest integer, ties to even, as `fistpq` under the default
    /// rounding control gives it (glibc's `llrintl`): `i64::MIN` for NaNs, infinities and values
    /// out of range.
    pub fn round_ties_even_to_i64(self) -> i64 {
        match self.kind() {
            Kind::Zero => 0,
            Kind::Finite { sig, exp } => {
                let negative = self.is_sign_negative();
                if exp >= 0 {
                    return i64::MIN;
                }
                let mag = rne_shift(sig as u128, false, (-exp) as u32);
                if negative {
                    if mag <= 1 << 63 {
                        (mag as u64).wrapping_neg() as i64
                    } else {
                        i64::MIN
                    }
                } else if mag < 1 << 63 {
                    mag as i64
                } else {
                    i64::MIN
                }
            }
            _ => i64::MIN,
        }
    }
}

/// `(x >> n, whether nonzero bits were shifted out)`, for any `n`.
fn shr_sticky(x: u128, n: u32) -> (u128, bool) {
    if n == 0 {
        (x, false)
    } else if n >= 128 {
        (0, x != 0)
    } else {
        (x >> n, x & ((1u128 << n) - 1) != 0)
    }
}

/// `(sig + δ) / 2^n` rounded to an integer, ties to even, where `0 < δ < 1` when `sticky` (the
/// bits below `sig`) and `δ = 0` otherwise.
fn rne_shift(sig: u128, sticky: bool, n: u32) -> u128 {
    if n == 0 {
        return sig;
    }
    if n > 128 {
        // sig / 2^n < 1/2.
        return 0;
    }
    let (q, rem, half) = if n == 128 {
        (0, sig, 1u128 << 127)
    } else {
        (sig >> n, sig & ((1u128 << n) - 1), 1u128 << (n - 1))
    };
    let up = rem > half || (rem == half && (sticky || q & 1 == 1));
    q + u128::from(up)
}

/// Rounds `(sig + δ) · 2^e` (`sig ≠ 0`, `δ` as in [`rne_shift`]) to a binary format of `p`
/// significand bits (the leading bit included), exponent bias `bias` and exponent field `emax`
/// for infinities: `Some((exponent field, significand))`, the significand with its leading bit
/// (`p` bits for normal numbers; fewer, exponent field 0, for subnormals), or `None` on overflow.
fn round_format(
    sig: u128,
    e: i32,
    sticky: bool,
    p: u32,
    bias: i32,
    emax: i32,
) -> Option<(i32, u128)> {
    let lz = sig.leading_zeros();
    let sig = sig << lz;
    // value = sig · 2^(e - lz), the leading bit at 127.
    let be = e - lz as i32 + 127 + bias;
    if be >= 1 {
        let mut q = rne_shift(sig, sticky, 128 - p);
        let mut be = be;
        if q >> p != 0 {
            q >>= 1;
            be += 1;
        }
        (be < emax).then_some((be, q))
    } else {
        // Below the smallest normal: rounded once to the subnormal grid.
        let extra = (1 - be).min(256) as u32;
        let q = rne_shift(sig, sticky, 128 - p + extra);
        // Rounding up to the smallest normal gives its encoding (exponent field 1).
        Some((i32::from(q >> (p - 1) != 0), q))
    }
}

/// Rounds `(sig + δ) · 2^e` to the extended format (`sig ≠ 0`).
fn round_pack(negative: bool, sig: u128, e: i32, sticky: bool) -> F80 {
    match round_format(sig, e, sticky, 64, BIAS, EXP_SPECIAL as i32) {
        None => F80::signed_inf(negative),
        Some((be, q)) => F80 {
            m: q as u64,
            se: if negative { 0x8000 } else { 0 } | be as u16,
        },
    }
}

impl From<f64> for F80 {
    fn from(x: f64) -> F80 {
        F80::from_f64(x)
    }
}

impl From<f32> for F80 {
    fn from(x: f32) -> F80 {
        F80::from_f32(x)
    }
}

impl From<i64> for F80 {
    fn from(x: i64) -> F80 {
        F80::from_i64(x)
    }
}

impl From<i32> for F80 {
    fn from(x: i32) -> F80 {
        F80::from_i64(x.into())
    }
}

impl From<u32> for F80 {
    fn from(x: u32) -> F80 {
        F80::from_u64(x.into())
    }
}

impl From<u64> for F80 {
    fn from(x: u64) -> F80 {
        F80::from_u64(x)
    }
}

impl Add for F80 {
    type Output = F80;
    #[inline]
    fn add(self, rhs: F80) -> F80 {
        F80::add_sub(self, rhs, false)
    }
}

impl Sub for F80 {
    type Output = F80;
    #[inline]
    fn sub(self, rhs: F80) -> F80 {
        F80::add_sub(self, rhs, true)
    }
}

impl Mul for F80 {
    type Output = F80;
    #[inline]
    fn mul(self, rhs: F80) -> F80 {
        self.mul_impl(rhs)
    }
}

impl Div for F80 {
    type Output = F80;
    #[inline]
    fn div(self, rhs: F80) -> F80 {
        self.div_impl(rhs)
    }
}

/// `-x`: the sign bit flipped (`fchs`; NaNs keep their payload).
impl Neg for F80 {
    type Output = F80;
    #[inline]
    fn neg(self) -> F80 {
        F80 {
            m: self.m,
            se: self.se ^ 0x8000,
        }
    }
}

impl AddAssign for F80 {
    #[inline]
    fn add_assign(&mut self, rhs: F80) {
        *self = *self + rhs;
    }
}

impl SubAssign for F80 {
    #[inline]
    fn sub_assign(&mut self, rhs: F80) {
        *self = *self - rhs;
    }
}

impl MulAssign for F80 {
    #[inline]
    fn mul_assign(&mut self, rhs: F80) {
        *self = *self * rhs;
    }
}

impl DivAssign for F80 {
    #[inline]
    fn div_assign(&mut self, rhs: F80) {
        *self = *self / rhs;
    }
}

impl PartialEq for F80 {
    fn eq(&self, other: &F80) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl PartialOrd for F80 {
    /// IEEE 754's order: `None` when either is a NaN or an unsupported encoding.
    fn partial_cmp(&self, other: &F80) -> Option<Ordering> {
        // Magnitude keys: zero < finite (exponent, significand) < infinity.
        let key = |k: Kind| -> Option<(u8, i32, u64)> {
            match k {
                Kind::Zero => Some((0, 0, 0)),
                Kind::Finite { sig, exp } => Some((1, exp, sig)),
                Kind::Inf => Some((2, 0, 0)),
                _ => None,
            }
        };
        let (a, b) = (key(self.kind())?, key(other.kind())?);
        if a.0 == 0 && b.0 == 0 {
            return Some(Ordering::Equal);
        }
        let (na, nb) = (self.is_sign_negative(), other.is_sign_negative());
        // A zero's sign does not count.
        let na = na && a.0 != 0;
        let nb = nb && b.0 != 0;
        Some(match (na, nb) {
            (false, false) => a.cmp(&b),
            (true, true) => b.cmp(&a),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
        })
    }
}

impl fmt::Debug for F80 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "F80({:#06x}_{:016x} ≈ {:e})",
            self.se,
            self.m,
            self.to_f64()
        )
    }
}

#[cfg(test)]
mod hardware;
#[cfg(test)]
mod tests;
