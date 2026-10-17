// SPDX-License-Identifier: Apache-2.0
//! ITK's random numbers (ITK v5.4.5): `MersenneTwisterRandomVariateGenerator`
//! (`Modules/Core/Common/include/itkMersenneTwisterRandomVariateGenerator.h`, MT19937 as in
//! Richard Wagner's `MersenneTwister.h`) and `ImageRandomConstIteratorWithIndex`
//! (`itkImageRandomConstIteratorWithIndex.hxx`), which the registration method uses to
//! sample the virtual domain.
//!
//! The normal variate takes a logarithm and a cosine; they are correctly rounded here
//! (`larmorx_core::math`), as glibc's are in all but rare cases.

const N: usize = 624;
const M: usize = 397;

/// `MersenneTwisterRandomVariateGenerator` seeded with [`MersenneTwister::new`].
#[derive(Clone)]
pub struct MersenneTwister {
    state: [u32; N],
    next: usize,
    left: usize,
}

impl std::fmt::Debug for MersenneTwister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MersenneTwister")
            .field("next", &self.next)
            .field("left", &self.left)
            .finish()
    }
}

#[inline]
fn twist(m: u32, s0: u32, s1: u32) -> u32 {
    let mix = (s0 & 0x8000_0000) | (s1 & 0x7fff_ffff);
    m ^ (mix >> 1) ^ ((s1 & 1).wrapping_neg() & 0x9908_b0df)
}

impl MersenneTwister {
    /// `SetSeed(seed)` (`Initialize` then `reload`). ITK takes the seed as `uint32_t`; an
    /// `int` seed converts modulo 2³².
    pub fn new(seed: u32) -> Self {
        let mut state = [0u32; N];
        state[0] = seed;
        for i in 1..N {
            let r = state[i - 1];
            state[i] = 1_812_433_253u32
                .wrapping_mul(r ^ (r >> 30))
                .wrapping_add(i as u32);
        }
        let mut g = MersenneTwister {
            state,
            next: 0,
            left: 0,
        };
        g.reload();
        g
    }

    /// An `int` seed, as `SetSeed(int)` receives it.
    pub fn from_int(seed: i32) -> Self {
        Self::new(seed as u32)
    }

    fn reload(&mut self) {
        let s = &mut self.state;
        let mut p = 0usize;
        for _ in 0..N - M {
            s[p] = twist(s[p + M], s[p], s[p + 1]);
            p += 1;
        }
        for _ in 0..M - 1 {
            s[p] = twist(s[p + M - N], s[p], s[p + 1]);
            p += 1;
        }
        s[p] = twist(s[p + M - N], s[p], s[0]);
        self.left = N;
        self.next = 0;
    }

    /// `GetIntegerVariate()`.
    #[inline]
    pub fn integer(&mut self) -> u32 {
        if self.left == 0 {
            self.reload();
        }
        self.left -= 1;
        let mut s1 = self.state[self.next];
        self.next += 1;
        s1 ^= s1 >> 11;
        s1 ^= (s1 << 7) & 0x9d2c_5680;
        s1 ^= (s1 << 15) & 0xefc6_0000;
        s1 ^ (s1 >> 18)
    }

    /// `GetVariateWithOpenUpperRange()`: `[0, 1)`.
    #[inline]
    pub fn open_upper(&mut self) -> f64 {
        f64::from(self.integer()) * (1.0 / 4_294_967_296.0)
    }

    /// `GetVariateWithOpenRange()`: `(0, 1)`.
    #[inline]
    pub fn open(&mut self) -> f64 {
        (f64::from(self.integer()) + 0.5) * (1.0 / 4_294_967_296.0)
    }

    /// `GetVariateWithClosedRange()`: `[0, 1]`.
    #[inline]
    pub fn closed(&mut self) -> f64 {
        f64::from(self.integer()) * (1.0 / 4_294_967_295.0)
    }

    /// `GetNormalVariate(0, 1)`: Box–Muller,
    /// `sqrt(−2·log(1 − u₁)·1)·cos(2π·u₂)`.
    #[inline]
    pub fn normal(&mut self) -> f64 {
        self.normal_with(0.0, 1.0)
    }

    /// `GetNormalVariate(mean, variance)`.
    #[inline]
    pub fn normal_with(&mut self, mean: f64, variance: f64) -> f64 {
        let r = (-2.0 * larmorx_core::math::log(1.0 - self.open()) * variance).sqrt();
        let phi = 2.0 * std::f64::consts::PI * self.open_upper();
        mean + r * larmorx_core::math::cos(phi)
    }

    /// `GetUniformVariate(a, b)`.
    #[inline]
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        let u = self.open_upper();
        (1.0 - u) * a + u * b
    }
}

/// `ImageRandomConstIteratorWithIndex` over a region of `size` voxels starting at `start`:
/// each step jumps to a uniformly drawn voxel (`RandomJump`).
#[derive(Clone, Debug)]
pub struct RandomIterator<const D: usize> {
    generator: MersenneTwister,
    size: [usize; D],
    start: [i64; D],
    pixels: usize,
}

impl<const D: usize> RandomIterator<D> {
    /// The iterator, its generator seeded with `seed` (`ReinitializeSeed(seed)`).
    pub fn new(size: [usize; D], start: [i64; D], seed: i32) -> Self {
        RandomIterator {
            generator: MersenneTwister::from_int(seed),
            size,
            start,
            pixels: size.iter().product(),
        }
    }

    /// `RandomJump`: the position `long(GetVariateWithOpenRange(pixels − 0.5))`, decomposed
    /// into an index (x fastest).
    pub fn jump(&mut self) -> [i64; D] {
        let mut position = (self.generator.open() * (self.pixels as f64 - 0.5)) as i64;
        let mut idx = [0i64; D];
        for d in 0..D {
            let n = self.size[d] as i64;
            let residual = position % n;
            idx[d] = residual + self.start[d];
            position -= residual;
            position /= n;
        }
        idx
    }

    /// The `count` indices a loop `GoToBegin(); !IsAtEnd(); ++it` visits.
    pub fn samples(&mut self, count: usize) -> Vec<[i64; D]> {
        (0..count).map(|_| self.jump()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mt19937_reference_values() {
        // MT19937 with the reference initialisation (init_genrand) of seed 5489: the first
        // outputs of the reference implementation.
        let mut g = MersenneTwister::new(5489);
        let first: Vec<u32> = (0..5).map(|_| g.integer()).collect();
        assert_eq!(
            first,
            [
                3_499_211_612,
                581_869_302,
                3_890_346_734,
                3_586_334_585,
                545_404_204
            ]
        );
        // The 10000th output for seed 5489 (the C++11 standard's check for std::mt19937).
        let mut g = MersenneTwister::new(5489);
        let mut v = 0;
        for _ in 0..10_000 {
            v = g.integer();
        }
        assert_eq!(v, 4_123_659_995);
    }
}
