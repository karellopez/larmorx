// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! vnl's random numbers (`vnl_random`): Marsaglia and Zaman's subtract-with-borrow generator on 37
//! words of 32 bits, seeded by a linear congruential generator, and the uniform and normal
//! numbers drawn from it.
//!
//! **The order of `drand64`'s two draws.** `drand64` adds `lrand32() / 0xffffffff` and
//! `lrand32() / 0xffffffff²`; C++ leaves the order of the two calls unspecified. GCC 8.4.0 at
//! `-O3` divides the *first* call's result by `0xffffffff` (ITK 4.13.2's file compiled and
//! compared in `validation/probes/fs-random`): this port draws left to right.
//!
//! **The logarithm** in [`VnlRandom::normal`] and [`VnlRandom::normal64`] is the C library's in
//! the original (glibc 2.35's on the reference machine): the caller passes it, `math::log_glibc`
//! for the original's bits, `math::log` (correctly rounded, CLAUDE.md rule 5) otherwise.
//!
//! **Differences from the C++**, none on a defined call:
//! - the time-seeded constructor and `reseed()` are left out (LARMORX seeds explicitly: the caller
//!   passes the time if it must reproduce a clock seed);
//! - a copy ([`Clone`]) keeps the cached second normal number; the C++ copy constructor and
//!   assignment copy its flag but not its value, so a copy's next `normal()` returns
//!   uninitialised memory (both ITK 4.13.2 and 5.4.5);
//! - [`VnlRandom::lrand32_range`] computes the range in 64 bits and refuses `lower > upper` (the
//!   C++ computes `upper - lower + 1` in `int`, which overflows when the range exceeds `2^31 - 1`:
//!   a range of `2^32` divides by zero, one from `2^31` to `2^32 - 1` never returns; with
//!   `lower > upper` it divides
//!   by zero or never returns under `NDEBUG`).
//!
//! Ported from VXL as bundled with ITK 4.13.2, the version FreeSurfer 8.2.0's official binaries
//! link (`core/vnl/vnl_random.h`, `vnl_random.cxx`; identical in ITK 5.4.5 apart from the default
//! constructor's initialisers).

// Original copyright and licence (BSD-3-Clause) notice of VXL
// (Modules/ThirdParty/VNL/src/vxl/core/vxl_copyright.h in ITK 4.13.2):
//
//   Copyright 2000-2013 VXL Contributors
//   All rights reserved.
//
//   Redistribution and use in source and binary forms, with or without
//   modification, are permitted provided that the following conditions
//   are met:
//
//   * Redistributions of source code must retain the above copyright
//     notice, this list of conditions and the following disclaimer.
//
//   * Redistributions in binary form must reproduce the above copyright
//     notice, this list of conditions and the following disclaimer in the
//     documentation and/or other materials provided with the distribution.
//
//   * Neither the names of the copyright holders nor the names of their
//     contributors may be used to endorse or promote products derived
//     from this software without specific prior written permission.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
//   "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
//   LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
//   FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
//   COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
//   INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
//   (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//   SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
//   HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
//   STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
//   OF THE POSSIBILITY OF SUCH DAMAGE.

/// The number of words of the subtract-with-borrow state (`vnl_random_array_size`).
pub const ARRAY_SIZE: usize = 37;
/// The multiplier of the seeding generator, `x · 1664525 + 1 mod 2^32`.
const LC_MULTIPLIER: u64 = 1_664_525;
/// The lag of the subtract-with-borrow generator.
const MZ_PREVIOUS1: usize = 24;
/// `0xffffffff` as `double`.
const MAX32: f64 = 4_294_967_295.0;

/// `vnl_random`'s state. `unsigned long` is 64 bits on x86-64 Linux, but every word the
/// generator stores is below `2^32`.
#[derive(Clone, Debug)]
pub struct VnlRandom {
    linear_congruential_previous: u64,
    mz_seed_array: [u64; ARRAY_SIZE],
    mz_array: [u64; ARRAY_SIZE],
    mz_array_position: usize,
    mz_borrow: u64,
    mz_previous_normal: f64,
    mz_previous_normal_flag: bool,
}

impl VnlRandom {
    /// `vnl_random(seed)`: [`VnlRandom::reseed`] with `seed` (a C++ `long` seed converts to
    /// `unsigned long`: `seed as u64`).
    pub fn new(seed: u64) -> VnlRandom {
        let mut r = VnlRandom {
            linear_congruential_previous: seed,
            mz_seed_array: [0; ARRAY_SIZE],
            mz_array: [0; ARRAY_SIZE],
            mz_array_position: 0,
            mz_borrow: 0,
            mz_previous_normal: 0.0,
            mz_previous_normal_flag: false,
        };
        r.reseed(seed);
        r
    }

    /// `vnl_random(seed[37])`: the state from 37 words (each used modulo `2^32`, as the
    /// generator's arithmetic masks every result to 32 bits).
    pub fn from_array(seed: &[u64; ARRAY_SIZE]) -> VnlRandom {
        let mut r = VnlRandom::new(0);
        r.reseed_array(seed);
        r
    }

    fn linear_congruential_lrand32(&mut self) -> u64 {
        self.linear_congruential_previous = self
            .linear_congruential_previous
            .wrapping_mul(LC_MULTIPLIER)
            .wrapping_add(1)
            & 0xffff_ffff;
        self.linear_congruential_previous
    }

    /// `reseed(seed)`: the 37 words from the linear congruential generator started at `seed`,
    /// then 1,000 draws to warm up. The cached normal number is kept, as in the C++.
    pub fn reseed(&mut self, seed: u64) {
        self.mz_array_position = 0;
        self.mz_borrow = 0;
        self.linear_congruential_previous = seed;
        for i in 0..ARRAY_SIZE {
            self.mz_seed_array[i] = self.linear_congruential_lrand32();
            self.mz_array[i] = self.mz_seed_array[i];
        }
        for _ in 0..1000 {
            self.lrand32();
        }
    }

    /// `reseed(seed[37])`: the words as given, without warming up.
    pub fn reseed_array(&mut self, seed: &[u64; ARRAY_SIZE]) {
        self.mz_array_position = 0;
        self.mz_borrow = 0;
        self.mz_array = *seed;
        self.mz_seed_array = *seed;
    }

    /// `restart()`: back to the words of the last reseed (the borrow is kept, as in the C++).
    pub fn restart(&mut self) {
        self.mz_array_position = 0;
        self.mz_array = self.mz_seed_array;
    }

    /// `lrand32()`: the next 32-bit number.
    pub fn lrand32(&mut self) -> u32 {
        let p1 = self.mz_array[(ARRAY_SIZE + self.mz_array_position - MZ_PREVIOUS1) % ARRAY_SIZE];
        let p2 = p1
            .wrapping_sub(self.mz_array[self.mz_array_position])
            .wrapping_sub(self.mz_borrow)
            & 0xffff_ffff;
        if p2 < p1 {
            self.mz_borrow = 0;
        }
        if p2 > p1 {
            self.mz_borrow = 1;
        }
        self.mz_array[self.mz_array_position] = p2;
        self.mz_array_position = (self.mz_array_position + 1) % ARRAY_SIZE;
        p2 as u32
    }

    /// `lrand32(lower, upper)`: uniform in `lower..=upper`, rejecting the draws that would bias
    /// it. Panics when `lower > upper` (the C++ asserts it).
    pub fn lrand32_range(&mut self, lower: i32, upper: i32) -> i32 {
        self.lrand32_range_count(lower, upper).0
    }

    /// `lrand32(lower, upper, count)`: [`VnlRandom::lrand32_range`] and the number of draws it
    /// took.
    pub fn lrand32_range_count(&mut self, lower: i32, upper: i32) -> (i32, i32) {
        assert!(lower <= upper, "vnl_random::lrand32: lower > upper");
        let range = (i64::from(upper) - i64::from(lower) + 1) as u64;
        // A range of 2^32 (every int) accepts every draw; the C++ divides by zero there.
        let denom = (0xffff_ffff / range).max(1);
        let mut count = 1;
        let ran = loop {
            let ran = u64::from(self.lrand32());
            if ran < denom.saturating_mul(range) {
                break ran;
            }
            count += 1;
        };
        ((i64::from(lower) + (ran / denom) as i64) as i32, count)
    }

    /// `lrand32(b)`: uniform in `0..=b`.
    pub fn lrand32_upto(&mut self, b: i32) -> i32 {
        self.lrand32_range(0, b)
    }

    /// `operator()(n)`: uniform in `0..n` (`n >= 1`).
    pub fn pick(&mut self, n: u32) -> u64 {
        assert!(n >= 1, "vnl_random::operator(): n must be at least 1");
        self.lrand32_range(0, (n - 1) as i32) as u64
    }

    /// `drand32(lower, upper)`: `lrand32() / 0xffffffff · (upper - lower) + lower`.
    pub fn drand32(&mut self, lower: f64, upper: f64) -> f64 {
        (f64::from(self.lrand32()) / MAX32) * (upper - lower) + lower
    }

    /// `drand64(lower, upper)`: two draws, the first divided by `0xffffffff`, the second by its
    /// square (`double`'s product, `0x43ef_ffff_ffc0_0000`), summed, scaled and shifted.
    pub fn drand64(&mut self, lower: f64, upper: f64) -> f64 {
        let hi = f64::from(self.lrand32()) / MAX32;
        let lo = f64::from(self.lrand32()) / (MAX32 * MAX32);
        (hi + lo) * (upper - lower) + lower
    }

    /// `normal()`: a standard normal number by Marsaglia's polar method on `drand32(-1, 1)`
    /// pairs, the second of each pair cached for the next call. `log` is the C library's
    /// logarithm (module documentation).
    pub fn normal(&mut self, log: fn(f64) -> f64) -> f64 {
        self.polar(log, VnlRandom::drand32)
    }

    /// `normal64()`: the same on `drand64(-1, 1)` pairs.
    pub fn normal64(&mut self, log: fn(f64) -> f64) -> f64 {
        self.polar(log, VnlRandom::drand64)
    }

    fn polar(&mut self, log: fn(f64) -> f64, draw: fn(&mut VnlRandom, f64, f64) -> f64) -> f64 {
        if self.mz_previous_normal_flag {
            self.mz_previous_normal_flag = false;
            return self.mz_previous_normal;
        }
        let (x, y, r2) = loop {
            let x = draw(self, -1.0, 1.0);
            let y = draw(self, -1.0, 1.0);
            let r2 = x * x + y * y;
            if !(r2 >= 1.0 || r2 == 0.0) {
                break (x, y, r2);
            }
        };
        let fac = (-2.0 * log(r2) / r2).sqrt();
        self.mz_previous_normal = x * fac;
        self.mz_previous_normal_flag = true;
        y * fac
    }
}

#[cfg(test)]
mod tests {
    use super::VnlRandom;
    use crate::math;

    fn fnv(h: &mut u64, w: u64) {
        for b in w.to_le_bytes() {
            *h = (*h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    /// ITK 4.13.2's vnl_random (its own vnl_random.cxx compiled by GCC 8.4.0 at `-O3`, with
    /// glibc 2.35's logarithm): the hashes of `sequence(seed)`'s outputs,
    /// recorded by `validation/probes/fs-random` (`fs-random --record`).
    const ORIGINAL: [(u64, u64); 5] = [
        (0x4d2, 0xd541_c4ad_b5bd_0300),
        (0x35, 0xccaf_d16a_1421_a6be),
        (0x0, 0x11a8_44e5_3cc2_c5ba),
        (0x1, 0x99cc_2a15_916d_41ae),
        (0xffff_ffff_ffff_ffff, 0x1213_7c15_b6f2_5623),
    ];

    /// The probe's fixed sequence (`vnl_sequence` in `validation/probes/fs-random`): 1,000
    /// `lrand32`, 500 `drand64(0, 1)`, 200 `normal64` and 200 `normal`, 300
    /// `(float) drand64(0, 1)`, 200 `lrand32(-5, 1000)`, 200 `drand32(-1, 1)`.
    fn sequence(seed: u64) -> u64 {
        let mut r = VnlRandom::new(seed);
        let mut h = 0xcbf2_9ce4_8422_2325;
        for _ in 0..1000 {
            fnv(&mut h, u64::from(r.lrand32()));
        }
        for _ in 0..500 {
            fnv(&mut h, r.drand64(0.0, 1.0).to_bits());
        }
        for _ in 0..200 {
            fnv(&mut h, r.normal64(math::log_glibc).to_bits());
        }
        for _ in 0..200 {
            fnv(&mut h, r.normal(math::log_glibc).to_bits());
        }
        for _ in 0..300 {
            fnv(&mut h, u64::from((r.drand64(0.0, 1.0) as f32).to_bits()));
        }
        for _ in 0..200 {
            fnv(&mut h, r.lrand32_range(-5, 1000) as u64);
        }
        for _ in 0..200 {
            fnv(&mut h, r.drand32(-1.0, 1.0).to_bits());
        }
        h
    }

    #[test]
    fn the_originals_sequences() {
        for (seed, hash) in ORIGINAL {
            assert_eq!(sequence(seed), hash, "seed {seed:#x}");
        }
    }

    #[test]
    fn seeding_restart_and_ranges() {
        // The linear congruential seeding of seed 1: 1·1664525 + 1, then on.
        let mut r = VnlRandom::new(1);
        assert_eq!(r.mz_seed_array[0], 1_664_526);
        assert_eq!(
            r.mz_seed_array[1],
            (1_664_526u64 * 1_664_525 + 1) & 0xffff_ffff
        );
        // Only the seed's low 32 bits matter.
        let mut a = VnlRandom::new(7);
        let mut b = VnlRandom::new(7 + (1 << 32));
        assert!((0..100).all(|_| a.lrand32() == b.lrand32()));
        // A reseed equals a new generator (the borrow and the position reset).
        a.reseed(99);
        let mut c = VnlRandom::new(99);
        assert!((0..100).all(|_| a.lrand32() == c.lrand32()));
        for _ in 0..1000 {
            assert!((-3..=5).contains(&r.lrand32_range(-3, 5)));
            let d = r.drand64(0.0, 1.0);
            assert!((0.0..=1.0 + 2.4e-10).contains(&d));
        }
        // The full range of int, where the C++ divides by zero: one draw, every value.
        assert_eq!(r.lrand32_range_count(i32::MIN, i32::MAX).1, 1);
        // A copy keeps the cached normal number (the C++ copy loses it).
        let mut n = VnlRandom::new(5);
        let _ = n.normal(math::log);
        let mut m = n.clone();
        assert_eq!(n.normal(math::log).to_bits(), m.normal(math::log).to_bits());
    }
}
