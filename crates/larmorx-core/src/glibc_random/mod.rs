// SPDX-License-Identifier: Apache-2.0
//! The C library's default random numbers, `rand`, `srand`, `random` and `srandom`, as the GNU C
//! Library 2.35 (x86-64) gives them: a **clean-room implementation** of `specs/glibc-random.md`
//! (FreeSurfer's foundations, queue item 3.1-04's clean room), written from that spec, the
//! publications it cites and black-box runs of the C library's functions only.
//!
//! [`GlibcRandom`] is the generator's whole state, owned by its caller (never a process-wide
//! variable: `larmorx-freesurfer`'s replica of FreeSurfer's process state holds one):
//!
//! - **seeding** ([`GlibcRandom::seed`], C's `srand` and `srandom`, which are one function, §2):
//!   Park and Miller's congruence `r ← 16807 · r mod (2³¹ − 1)` makes 31 words from the seed (read
//!   as a signed 32-bit integer; 0 means 1), and the additive lagged-Fibonacci recurrence
//!   `rₙ = rₙ₋₃₁ + rₙ₋₃ mod 2³²` runs 310 terms that are never delivered;
//! - **drawing** ([`GlibcRandom::next_u31`], with C's types [`GlibcRandom::rand`] and
//!   [`GlibcRandom::random`]: `rand` and `random` advance one stream, §3): each call delivers the
//!   top 31 bits of the recurrence's next term, an integer in `[0, RAND_MAX]`;
//! - **the state before any seeding** ([`GlibcRandom::new`], [`Default`]) is seed 1's (§4).
//!
//! Integer arithmetic only: the same values on every platform, no threads, no locks, no
//! allocation. The spec registers no defect (§8): `srand(0)` meaning seed 1 is the C library's
//! documented rule, the same in `fix` and `reproduce`.

#[cfg(test)]
mod tests;

/// The largest value [`GlibcRandom::next_u31`] delivers (C's `RAND_MAX`, 2³¹ − 1).
pub const RAND_MAX: u32 = 2_147_483_647;

/// The number of words of the state (the recurrence's long lag).
const WORDS: usize = 31;
/// The recurrence's short lag.
const SHORT_LAG: usize = 3;
/// The first term the recurrence makes (r₃₁ to r₃₃ are copies, §2 step 5).
const FIRST_TERM: usize = 34;
/// The terms seeding computes and never delivers (ten times the state, §2 step 6).
const DISCARDED: usize = 310;
/// The terms that exist after seeding, r₀ … r₃₄₃: the first value delivered is ⌊r₃₄₄ / 2⌋.
const SEEDED: usize = FIRST_TERM + DISCARDED;
/// Park and Miller's modulus, 2³¹ − 1.
const MODULUS: i64 = 2_147_483_647;
/// Park and Miller's multiplier, 7⁵.
const MULTIPLIER: i64 = 16_807;

/// One state of the C library's default generator (31 words of the recurrence and where the next
/// term goes). Created as the state before any seeding ([`GlibcRandom::new`]); clone it to fork
/// the stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlibcRandom {
    /// The last 31 terms: term `rⱼ` in `ring[j mod 31]`.
    ring: [u32; WORDS],
    /// `n mod 31` for the next term `rₙ`: where `rₙ₋₃₁` is, and where `rₙ` goes.
    long: usize,
    /// `(n − 3) mod 31`: where `rₙ₋₃` is.
    short: usize,
}

impl Default for GlibcRandom {
    /// The state before any seeding: seed 1's (§4).
    fn default() -> Self {
        Self::new()
    }
}

impl GlibcRandom {
    /// The state a process has before any `srand` or `srandom`: the stream of seed 1 (§4; the C
    /// standard's rule for `rand`).
    pub const fn new() -> Self {
        Self::seeded(1)
    }

    /// The state `srand(seed)` or `srandom(seed)` leaves (§2), for a seed of C's `unsigned int`.
    pub const fn seeded(seed: u32) -> Self {
        // r₀ … r₃₃₃ in a flat buffer (no ring arithmetic in the loop: its only dependency is the
        // short lag's); the last 31 go into the ring at their places j mod 31.
        let mut t = [0u32; SEEDED];
        let words = initial_words(seed);
        let mut n = 0;
        while n < WORDS {
            t[n] = words[n];
            n += 1;
        }
        // r₃₁, r₃₂, r₃₃: copies of r₀, r₁, r₂.
        while n < FIRST_TERM {
            t[n] = t[n - WORDS];
            n += 1;
        }
        // The recurrence as three interleaved chains (rₙ needs rₙ₋₃, the chain's previous term),
        // each kept in a register: 310 = 3 · 103 + 1 terms.
        let (mut a, mut b, mut c) = (t[n - 3], t[n - 2], t[n - 1]);
        while n + 3 <= SEEDED {
            a = a.wrapping_add(t[n - WORDS]);
            t[n] = a;
            b = b.wrapping_add(t[n + 1 - WORDS]);
            t[n + 1] = b;
            c = c.wrapping_add(t[n + 2 - WORDS]);
            t[n + 2] = c;
            n += 3;
        }
        while n < SEEDED {
            t[n] = t[n - WORDS].wrapping_add(t[n - SHORT_LAG]);
            n += 1;
        }
        let mut ring = [0u32; WORDS];
        let mut j = SEEDED - WORDS;
        while j < SEEDED {
            ring[j % WORDS] = t[j];
            j += 1;
        }
        GlibcRandom {
            ring,
            long: SEEDED % WORDS,
            short: (SEEDED - SHORT_LAG) % WORDS,
        }
    }

    /// `srand(seed)` and `srandom(seed)` (one function, §2): replaces the state by seed `seed`'s.
    pub fn seed(&mut self, seed: u32) {
        *self = Self::seeded(seed);
    }

    /// The same for a seed of a wider signed C type (FreeSurfer's `setRandomSeed(long)`): C
    /// converts it to `unsigned int` modulo 2³² (§1: −1 is 4294967295, 2⁴⁰ + 7 is 7).
    pub fn seed_c_long(&mut self, seed: i64) {
        self.seed(seed as u32);
    }

    /// The next value of the stream (§3), `oₖ = ⌊r₃₄₄₊ₖ / 2⌋`, in `[0, RAND_MAX]`, advancing the
    /// state by one term. `rand` and `random` both draw it.
    #[inline]
    pub fn next_u31(&mut self) -> u32 {
        self.step() >> 1
    }

    /// C's `rand()`: [`GlibcRandom::next_u31`] as an `int`.
    #[inline]
    pub fn rand(&mut self) -> i32 {
        self.next_u31() as i32
    }

    /// C's `random()`: [`GlibcRandom::next_u31`] as a `long`, the same stream as
    /// [`GlibcRandom::rand`].
    #[inline]
    pub fn random(&mut self) -> i64 {
        i64::from(self.next_u31())
    }

    /// The next `out.len()` values of the stream, in order (as that many calls of
    /// [`GlibcRandom::next_u31`]).
    pub fn fill(&mut self, out: &mut [u32]) {
        for v in out {
            *v = self.next_u31();
        }
    }

    /// The recurrence's next term `rₙ = rₙ₋₃₁ + rₙ₋₃ mod 2³²`, which replaces `rₙ₋₃₁`.
    #[inline]
    fn step(&mut self) -> u32 {
        let term = self.ring[self.long].wrapping_add(self.ring[self.short]);
        self.ring[self.long] = term;
        self.long = if self.long + 1 == WORDS {
            0
        } else {
            self.long + 1
        };
        self.short = if self.short + 1 == WORDS {
            0
        } else {
            self.short + 1
        };
        term
    }
}

/// `16807ⁱ mod (2³¹ − 1)` for `i` = 0 … 30.
const POWERS: [u64; WORDS] = {
    let mut p = [1u64; WORDS];
    let mut i = 1;
    while i < WORDS {
        p[i] = p[i - 1] * MULTIPLIER as u64 % MODULUS as u64;
        i += 1;
    }
    p
};

/// The 31 words Park and Miller's congruence makes from a seed (§2 steps 1 to 4): `r₀` the seed
/// (0 replaced by 1) read as a signed 32-bit integer, `rᵢ = 16807 · rᵢ₋₁ mod (2³¹ − 1)` (the
/// non-negative residue, also of a negative product), every one taken as a 32-bit word. Computed
/// as `rᵢ = (16807ⁱ mod (2³¹ − 1)) · (r₀ mod (2³¹ − 1)) mod (2³¹ − 1)`, the same residue (the
/// congruence is a multiplication modulo 2³¹ − 1), so the 30 words do not wait for each other.
const fn initial_words(seed: u32) -> [u32; WORDS] {
    let seed = if seed == 0 { 1 } else { seed };
    let r0 = (seed as i32 as i64).rem_euclid(MODULUS) as u64;
    let mut words = [0u32; WORDS];
    words[0] = seed;
    let mut i = 1;
    while i < WORDS {
        words[i] = (POWERS[i] * r0 % MODULUS as u64) as u32;
        i += 1;
    }
    words
}
