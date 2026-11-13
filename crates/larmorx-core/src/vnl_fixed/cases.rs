// SPDX-License-Identifier: Apache-2.0
//! Seeded small matrices with non-finite and extreme entries, for the checks of the closed forms'
//! NaNs (the unit tests' digest, and the differential probe against VNL's own instances): NaNs
//! of both signs with random payloads, quiet and signalling, infinities, signed zeros, numbers
//! whose products overflow or underflow, subnormals, among ordinary numbers. SplitMix64 and
//! integer arithmetic only, so every platform draws the same matrices.

use crate::x87::cases::SplitMix64;

/// How many entries of an `n x n` matrix are special: one, two, three, a row's worth, half or
/// all of them.
fn special_count(rng: &mut SplitMix64, n: usize) -> usize {
    let nn = n * n;
    match rng.below(8) {
        0..=2 => 1,
        3 => 2.min(nn),
        4 => 3.min(nn),
        5 => n,
        6 => nn / 2,
        _ => nn,
    }
}

/// An ordinary number: an integer up to 2^20 over a power of 2 up to 2^20.
fn ordinary(rng: &mut SplitMix64) -> f64 {
    let k = rng.range(-(1 << 20), 1 << 20) as f64;
    let mut d = 1.0;
    for _ in 0..rng.below(21) {
        d *= 2.0;
    }
    k / d
}

/// A special `float`, as bits.
pub fn special_f32(rng: &mut SplitMix64) -> f32 {
    let sign = (rng.below(2) as u32) << 31;
    let bits = match rng.below(12) {
        // quiet NaNs, the default payloads among them
        0..=3 => {
            let payload = if rng.below(4) == 0 {
                0
            } else {
                rng.next_u64() as u32 & 0x003f_ffff
            };
            0x7fc0_0000 | payload
        }
        // signalling NaNs
        4 | 5 => 0x7f80_0000 | (1 + rng.below(0x003f_ffff) as u32),
        6 | 7 => 0x7f80_0000,
        8 => 0,
        // magnitudes from 2^100 to the largest: their products overflow
        9 => ((227 + rng.below(28) as u32) << 23) | (rng.next_u64() as u32 & 0x007f_ffff),
        // from 2^-100 to the subnormals: their products underflow
        10 => ((rng.below(28) as u32) << 23) | (rng.next_u64() as u32 & 0x007f_ffff),
        _ => (ordinary(rng) as f32).to_bits() & 0x7fff_ffff,
    };
    f32::from_bits(sign | bits)
}

/// A special `double`, as bits.
pub fn special_f64(rng: &mut SplitMix64) -> f64 {
    let sign = rng.below(2) << 63;
    let frac = (1u64 << 52) - 1;
    let bits = match rng.below(12) {
        0..=3 => {
            let payload = if rng.below(4) == 0 {
                0
            } else {
                rng.next_u64() & (frac >> 1)
            };
            0x7ff8_0000_0000_0000 | payload
        }
        4 | 5 => 0x7ff0_0000_0000_0000 | (1 + rng.below(frac >> 1)),
        6 | 7 => 0x7ff0_0000_0000_0000,
        8 => 0,
        9 => ((1823 + rng.below(224)) << 52) | (rng.next_u64() & frac),
        10 => (rng.below(224) << 52) | (rng.next_u64() & frac),
        _ => ordinary(rng).to_bits() & !(1 << 63),
    };
    f64::from_bits(sign | bits)
}

/// An `n x n` `float` matrix (row-major) with some special entries.
pub fn matrix_f32(rng: &mut SplitMix64, n: usize) -> Vec<f32> {
    let mut m: Vec<f32> = (0..n * n).map(|_| ordinary(rng) as f32).collect();
    for _ in 0..special_count(rng, n) {
        let i = rng.below((n * n) as u64) as usize;
        m[i] = special_f32(rng);
    }
    m
}

/// An `n x n` `double` matrix (row-major) with some special entries.
pub fn matrix_f64(rng: &mut SplitMix64, n: usize) -> Vec<f64> {
    let mut m: Vec<f64> = (0..n * n).map(|_| ordinary(rng)).collect();
    for _ in 0..special_count(rng, n) {
        let i = rng.below((n * n) as u64) as usize;
        m[i] = special_f64(rng);
    }
    m
}
