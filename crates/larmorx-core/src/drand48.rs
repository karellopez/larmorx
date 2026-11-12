// SPDX-License-Identifier: Apache-2.0
//! POSIX's 48-bit random numbers: `drand48`, `erand48`, `lrand48`, `nrand48`, `mrand48`,
//! `jrand48`, `srand48`, `seed48` and `lcong48`, as the C library gives them (FreeSurfer's
//! `AntsN4BiasFieldCorrectionFs` seeds them with `srand48(53)`; its noise options and
//! `MRIfixTkReg` draw from them).
//!
//! Written from POSIX's definition (IEEE Std 1003.1, *drand48*), which fixes every bit: the linear
//! congruential generator `X(n+1) = (a·X(n) + c) mod 2^48` with `a = 0x5DEECE66D` and `c = 0xB`
//! unless `lcong48` changes them; `drand48` and `erand48` return `X / 2^48` (all 48 bits; a
//! `double` holds the value exactly), `lrand48` and `nrand48` the high 31 bits, `mrand48` and
//! `jrand48` the high 32 bits as a signed number; `srand48` sets the high 32 bits of `X` to its
//! argument's low 32 bits and the low 16 bits to `0x330E`; `seed48` and `lcong48` take 16-bit
//! words, the least significant first; `srand48` and `seed48` restore `a` and `c`. The C
//! library's own code is not used (glibc is LGPL; CLAUDE.md rule 2): the behaviour is checked
//! against the machine's glibc 2.35 as a black box (`validation/probes/fs-random`), including what
//! POSIX leaves open: before any seeding `X` is 0 in glibc, as here.

const MASK48: u64 = (1 << 48) - 1;
/// The default multiplier, `0x5DEECE66D`.
pub const DEFAULT_A: u64 = 0x5_deec_e66d;
/// The default addend, `0xB`.
pub const DEFAULT_C: u16 = 0xb;

/// The C library's shared state: `X`, the multiplier `a` and the addend `c` (the functions with
/// a caller's array use this state's `a` and `c`, as POSIX says), and `seed48`'s buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drand48 {
    x: u64,
    a: u64,
    c: u16,
    old: [u16; 3],
}

impl Default for Drand48 {
    /// The state of a process that has not seeded: `X = 0`, the default `a` and `c`.
    fn default() -> Drand48 {
        Drand48 {
            x: 0,
            a: DEFAULT_A,
            c: DEFAULT_C,
            old: [0; 3],
        }
    }
}

fn to_words(x: u64) -> [u16; 3] {
    [x as u16, (x >> 16) as u16, (x >> 32) as u16]
}

fn from_words(w: &[u16; 3]) -> u64 {
    u64::from(w[0]) | u64::from(w[1]) << 16 | u64::from(w[2]) << 32
}

impl Drand48 {
    /// The state after `srand48(seed)`.
    pub fn seeded(seed: i64) -> Drand48 {
        let mut s = Drand48::default();
        s.srand48(seed);
        s
    }

    fn step(&self, x: u64) -> u64 {
        self.a.wrapping_mul(x).wrapping_add(u64::from(self.c)) & MASK48
    }

    fn next(&mut self) -> u64 {
        self.x = self.step(self.x);
        self.x
    }

    fn next_in(&self, xsubi: &mut [u16; 3]) -> u64 {
        let x = self.step(from_words(xsubi));
        *xsubi = to_words(x);
        x
    }

    /// `srand48(seedval)`: `X` = the low 32 bits of `seedval` above `0x330E`; `a` and `c`
    /// restored.
    pub fn srand48(&mut self, seedval: i64) {
        self.x = ((seedval as u64 & 0xffff_ffff) << 16) | 0x330e;
        self.a = DEFAULT_A;
        self.c = DEFAULT_C;
    }

    /// `seed48(seed16v)`: `X` from three 16-bit words (least significant first); `a` and `c`
    /// restored; returns the previous `X` as `seed48`'s buffer holds it.
    pub fn seed48(&mut self, seed16v: [u16; 3]) -> [u16; 3] {
        self.old = to_words(self.x);
        self.x = from_words(&seed16v);
        self.a = DEFAULT_A;
        self.c = DEFAULT_C;
        self.old
    }

    /// `lcong48(param)`: `X` from `param[0..3]`, `a` from `param[3..6]`, `c` = `param[6]`.
    pub fn lcong48(&mut self, param: [u16; 7]) {
        self.x = from_words(&[param[0], param[1], param[2]]);
        self.a = from_words(&[param[3], param[4], param[5]]);
        self.c = param[6];
    }

    /// `drand48()`: the next `X / 2^48`, in `[0, 1)`.
    pub fn drand48(&mut self) -> f64 {
        to_unit(self.next())
    }

    /// `lrand48()`: the next `X`'s high 31 bits, in `[0, 2^31)`.
    pub fn lrand48(&mut self) -> i64 {
        (self.next() >> 17) as i64
    }

    /// `mrand48()`: the next `X`'s high 32 bits as a signed number, in `[-2^31, 2^31)`.
    pub fn mrand48(&mut self) -> i64 {
        i64::from((self.next() >> 16) as u32 as i32)
    }

    /// `erand48(xsubi)`: as [`Drand48::drand48`] on the caller's `X` (updated in place), with this
    /// state's `a` and `c`.
    pub fn erand48(&self, xsubi: &mut [u16; 3]) -> f64 {
        to_unit(self.next_in(xsubi))
    }

    /// `nrand48(xsubi)`: as [`Drand48::lrand48`] on the caller's `X`.
    pub fn nrand48(&self, xsubi: &mut [u16; 3]) -> i64 {
        (self.next_in(xsubi) >> 17) as i64
    }

    /// `jrand48(xsubi)`: as [`Drand48::mrand48`] on the caller's `X`.
    pub fn jrand48(&self, xsubi: &mut [u16; 3]) -> i64 {
        i64::from((self.next_in(xsubi) >> 16) as u32 as i32)
    }

    /// The current `X` as three 16-bit words (least significant first).
    pub fn state(&self) -> [u16; 3] {
        to_words(self.x)
    }
}

/// `X / 2^48` (exact).
fn to_unit(x: u64) -> f64 {
    x as f64 / (1u64 << 48) as f64
}

#[cfg(test)]
mod tests {
    use super::Drand48;

    fn fnv(h: &mut u64, w: u64) {
        for b in w.to_le_bytes() {
            *h = (*h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    /// The machine's C library (glibc 2.35): the hashes of `sequence(seed)`'s outputs, recorded by
    /// `validation/probes/fs-random` (`fs-random --record`).
    const ORIGINAL: [(i64, u64); 5] = [
        (53, 0x671c_b207_86d4_07d5),
        (0, 0xab18_4747_4392_a62f),
        (-1, 0x159b_d279_a942_bfd9),
        (1234, 0x5c40_7162_edbc_f332),
        (0x1234_5678_9abc, 0x6280_1cf5_ff56_0c48),
    ];

    /// The probe's fixed sequence (`drand48_sequence` in `validation/probes/fs-random`).
    fn sequence(seed: i64) -> u64 {
        let mut s = Drand48::default();
        let mut h = 0xcbf2_9ce4_8422_2325;
        s.srand48(seed);
        for _ in 0..300 {
            fnv(&mut h, s.drand48().to_bits());
        }
        for _ in 0..300 {
            fnv(&mut h, s.lrand48() as u64);
        }
        for _ in 0..300 {
            fnv(&mut h, s.mrand48() as u64);
        }
        let mut a = [1u16, 2, 3];
        for _ in 0..100 {
            fnv(&mut h, s.erand48(&mut a).to_bits());
        }
        for _ in 0..100 {
            fnv(&mut h, s.nrand48(&mut a) as u64);
        }
        for _ in 0..100 {
            fnv(&mut h, s.jrand48(&mut a) as u64);
        }
        let old = s.seed48([0x1234, 0x5678, 0x9abc]);
        fnv(
            &mut h,
            u64::from(old[0]) | u64::from(old[1]) << 16 | u64::from(old[2]) << 32,
        );
        for _ in 0..100 {
            fnv(&mut h, s.drand48().to_bits());
        }
        s.lcong48([5, 6, 7, 0x1111, 0x2222, 0x3333, 0x44]);
        for _ in 0..100 {
            fnv(&mut h, s.drand48().to_bits());
        }
        for _ in 0..100 {
            fnv(&mut h, s.erand48(&mut a).to_bits());
        }
        h
    }

    #[test]
    fn the_c_librarys_sequences() {
        for (seed, hash) in ORIGINAL {
            assert_eq!(sequence(seed), hash, "seed {seed}");
        }
    }

    #[test]
    fn known_values() {
        // srand48(0): X = 0x330E; the next X = 0x5DEECE66D · 0x330E + 0xB mod 2^48.
        let mut s = Drand48::seeded(0);
        let x1 = (0x5_deec_e66du64 * 0x330e + 0xb) & ((1 << 48) - 1);
        assert_eq!(s.drand48(), x1 as f64 / 2f64.powi(48));
        assert_eq!(s.state(), [x1 as u16, (x1 >> 16) as u16, (x1 >> 32) as u16]);
        // lcong48's multiplier and addend, then seed48 restores the defaults.
        let mut t = Drand48::default();
        t.lcong48([1, 2, 3, 5, 0, 0, 7]);
        assert_eq!(
            t.lrand48(),
            (((5u64 * 0x3_0002_0001 + 7) & ((1 << 48) - 1)) >> 17) as i64
        );
        let before = t.state();
        assert_eq!(t.seed48([9, 0, 0]), before);
        assert_eq!(t.a, super::DEFAULT_A);
        assert_eq!(t.c, super::DEFAULT_C);
    }
}
