// SPDX-License-Identifier: Apache-2.0
//! The random numbers of FAST's MRF iterations (`specs/fast.md` §8.1): the C library's `rand`
//! of glibc, as P. Selinger describes it ("The GLIBC pseudo-random number generator", 2007):
//! the BSD `random(3)` additive generator of degree 31 and lag 3, seeded by a multiplicative
//! congruential sequence, its first 310 outputs discarded. Written from that description and
//! checked against glibc's `rand` run as a black box (the test below).

/// FAST's seed: `srand(-1)`, the 32-bit word `0xFFFFFFFF`.
pub const FAST_SEED: u32 = 0xFFFF_FFFF;

/// glibc's `rand` after `srand(seed)`.
#[derive(Clone, Debug)]
pub struct GlibcRand {
    /// The last 32 values of the sequence `r`, `r_i` at `i % 32`.
    hist: [u32; 32],
    /// The index of the next value.
    i: usize,
}

impl GlibcRand {
    pub fn new(seed: u32) -> GlibcRand {
        const M: i64 = 2_147_483_647;
        let mut r = [0u32; 34];
        // a seed of 0 is read as 1; the seed is the signed 32-bit word
        let mut prev = i64::from(if seed == 0 { 1 } else { seed as i32 });
        r[0] = prev as u32;
        for v in r.iter_mut().take(31).skip(1) {
            let mut next = (16_807 * prev) % M;
            if next < 0 {
                next += M;
            }
            *v = next as u32;
            prev = next;
        }
        for i in 31..34 {
            r[i] = r[i - 31];
        }
        let mut hist = [0u32; 32];
        for (i, &v) in r.iter().enumerate().skip(2) {
            hist[i % 32] = v;
        }
        let mut g = GlibcRand { hist, i: 34 };
        // the outputs start at r_344
        for _ in 34..344 {
            g.step();
        }
        g
    }

    #[inline]
    fn step(&mut self) -> u32 {
        let i = self.i;
        let v = self.hist[(i - 31) % 32].wrapping_add(self.hist[(i - 3) % 32]);
        self.hist[i % 32] = v;
        self.i += 1;
        v
    }

    /// The next value of `rand()`, from 0 to 2^31 - 1.
    #[inline]
    pub fn next_value(&mut self) -> u32 {
        self.step() >> 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_glibc() {
        // glibc 2.35's rand after srand(0xFFFFFFFF) and srand(0), run as a black box (ctypes)
        let mut g = GlibcRand::new(FAST_SEED);
        let first: Vec<u32> = (0..6).map(|_| g.next_value()).collect();
        assert_eq!(
            first,
            [
                254_925_627,
                1_205_188_300,
                366_127_624,
                1_401_405_153,
                76_053_476,
                1_604_170_158
            ]
        );
        for _ in 6..1_000_000 {
            g.next_value();
        }
        assert_eq!(g.next_value(), 422_525_085);
        let mut z = GlibcRand::new(0);
        assert_eq!(
            [z.next_value(), z.next_value(), z.next_value()],
            [1_804_289_383, 846_930_886, 1_681_692_777]
        );
    }
}
