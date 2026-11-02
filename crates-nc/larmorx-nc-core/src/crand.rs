// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret. Part of larmorx-nc (FSL Licence terms; see LICENSE).
//! The C library's `srand` and `rand` as FSL's Linux binaries call them (glibc's `random`, its
//! default `TYPE_3` generator), written from the algorithm's published descriptions, not from
//! glibc's code: the additive lagged-Fibonacci generator of BSD's `random(3)` (degree 31, lag 3:
//! `r[i] = r[i-31] + r[i-3]` modulo 2^32, the output `r[i] >> 1`), seeded with the
//! Park-Miller "minimal standard" sequence `r[i] = 16807 r[i-1] mod (2^31 - 1)` in Schrage's
//! form on 32-bit integers (Park and Miller, Comm. ACM 31, 1988), the first 310 outputs
//! discarded (P. Selinger, "The GLIBC pseudo-random number generator", 2007). The seed's
//! conversion to a 32-bit signed integer (FAST's `srand(-1)` seeds with `-1`) and a seed of 0
//! read as 1 are the C library's documented behaviour; the sequences are checked against the C
//! library in the tests (values from glibc 2.35).
//!
//! FAST draws its random initial posteriors from it (`srand(-1)`, then `rand() / (float)
//! RAND_MAX`).

/// `RAND_MAX` (glibc).
pub const RAND_MAX: i32 = 2_147_483_647;

/// The state of `random` after `srand(seed)`: 31 words, the front and rear positions.
#[derive(Clone, Debug)]
pub struct Rand {
    state: [i32; 31],
    front: usize,
    rear: usize,
}

impl Rand {
    /// `srand(seed)`.
    #[must_use]
    pub fn new(seed: u32) -> Rand {
        let seed = if seed == 0 { 1 } else { seed };
        let mut state = [0i32; 31];
        state[0] = seed as i32;
        // the seed as a 32-bit signed word, then 16807 * word mod (2^31 - 1) in Schrage's form
        let mut word = i64::from(seed as i32);
        for s in state.iter_mut().skip(1) {
            let hi = word / 127_773;
            let lo = word % 127_773;
            word = 16807 * lo - 2836 * hi;
            if word < 0 {
                word += 2_147_483_647;
            }
            *s = word as i32;
        }
        let mut r = Rand {
            state,
            front: 3,
            rear: 0,
        };
        for _ in 0..310 {
            r.rand();
        }
        r
    }

    /// `rand()`: a number in `[0, RAND_MAX]`.
    pub fn rand(&mut self) -> i32 {
        let val = (self.state[self.front] as u32).wrapping_add(self.state[self.rear] as u32);
        self.state[self.front] = val as i32;
        self.front += 1;
        if self.front >= 31 {
            self.front = 0;
            self.rear += 1;
        } else {
            self.rear += 1;
            if self.rear >= 31 {
                self.rear = 0;
            }
        }
        (val >> 1) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The C library's sequences (glibc 2.35, x86-64 Linux).
    #[test]
    fn the_c_library_sequences() {
        let mut r = Rand::new(u32::MAX);
        let v: Vec<i32> = (0..5).map(|_| r.rand()).collect();
        assert_eq!(v, [254925627, 1205188300, 366127624, 1401405153, 76053476]);
        for seed in [0, 1] {
            let mut r = Rand::new(seed);
            let v: Vec<i32> = (0..3).map(|_| r.rand()).collect();
            assert_eq!(v, [1804289383, 846930886, 1681692777]);
        }
        let mut r = Rand::new(u32::MAX);
        let mut sum = 0i64;
        let mut last = 0;
        for _ in 0..1_000_000 {
            last = r.rand();
            sum = (sum + i64::from(last)) % 1_000_000_007;
        }
        assert_eq!((last, sum), (949151631, 623352516));
    }
}
