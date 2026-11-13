// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer 8.2.0's FNV hashes (`include/fnv_hash.h`, Bevin Brett; `include/fnvhash.h`), on
//! larmorx-core's FNV-1a ([`larmorx_core::fnv`]): FreeSurfer keeps the hash in an `unsigned long`,
//! so it is FNV-1a 32's offset basis and prime with a 64-bit accumulator
//! ([`larmorx_core::fnv::fnv1a_32_wide`], never reduced to 32 bits).
//!
//! - `fnv_init`, `fnv_add`, `fnv_hash` (the random moves of the surface library's metric
//!   properties: a hash in place of a random number);
//! - `FnvHash` (an object's hash: `MRI::hash`, the surfaces' checks), its `write`s and its printed
//!   form.

use std::fmt;
use std::path::Path;

use larmorx_core::fnv::{FNV32_OFFSET, fnv1a_32_wide};

use crate::utils;

/// `fnv_init()`: FNV-1a 32's offset basis, 0x811c9dc5.
pub const fn fnv_init() -> u64 {
    FNV32_OFFSET as u64
}

/// `fnv_add(hash, p, size)`: for each byte, `hash ^= byte; hash *= 16777619` modulo 2^64.
pub fn fnv_add(hash: u64, bytes: &[u8]) -> u64 {
    fnv1a_32_wide(hash, bytes)
}

/// `fnv_hash(i, k, &random_counter, lo, hi)`: the hash ([`fnv_add`] from [`fnv_init`]) of the
/// three `unsigned int`s `i`, `k` and the counter (their bytes little-endian, as x86-64's memory
/// holds them); the counter is then incremented (wrapping at the `int`'s end, as the official
/// build does); the hash's low 32 bits divided by 0xffffffff in `double`, mapped to
/// `(hi - lo) f + lo` in `double` and rounded to `float`.
pub fn fnv_hash(i: i32, k: i32, random_counter: &mut i32, lo: f64, hi: f64) -> f32 {
    let mut b = [0u8; 12];
    b[0..4].copy_from_slice(&(i as u32).to_le_bytes());
    b[4..8].copy_from_slice(&(k as u32).to_le_bytes());
    b[8..12].copy_from_slice(&(*random_counter as u32).to_le_bytes());
    *random_counter = random_counter.wrapping_add(1);
    let hash = fnv_add(fnv_init(), &b);
    let f = (hash & 0xffff_ffff) as f64 / f64::from(0xffff_ffffu32);
    ((hi - lo) * f + lo) as f32
}

/// `FnvHash`: a running hash, from 2166136261, of the bytes added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FnvHash {
    pub value: u64,
}

impl Default for FnvHash {
    fn default() -> Self {
        FnvHash::new()
    }
}

impl FnvHash {
    /// A hash of nothing yet (2166136261).
    pub const fn new() -> FnvHash {
        FnvHash {
            value: FNV32_OFFSET as u64,
        }
    }

    /// `add(str, size)`: the bytes added.
    pub fn add_bytes(&mut self, bytes: &[u8]) {
        self.value = fnv_add(self.value, bytes);
    }

    /// `add(&x)` of an `int`, `float`, `double`, ...: its bytes as x86-64's memory holds them.
    pub fn add_i32(&mut self, x: i32) {
        self.add_bytes(&x.to_le_bytes());
    }
    pub fn add_u32(&mut self, x: u32) {
        self.add_bytes(&x.to_le_bytes());
    }
    pub fn add_i64(&mut self, x: i64) {
        self.add_bytes(&x.to_le_bytes());
    }
    pub fn add_u64(&mut self, x: u64) {
        self.add_bytes(&x.to_le_bytes());
    }
    pub fn add_f32(&mut self, x: f32) {
        self.add_bytes(&x.to_le_bytes());
    }
    pub fn add_f64(&mut self, x: f64) {
        self.add_bytes(&x.to_le_bytes());
    }

    /// `write(filename)`: the file holds the value in decimal and a newline (created or
    /// emptied; nothing happens when it cannot be opened). A relative name starts in `dir`.
    pub fn write(&self, dir: Option<&Path>, filename: &str) {
        let _ = std::fs::write(utils::resolve(dir, filename), format!("{}\n", self.value));
    }

    /// `write(filename, suffix)`: `<value> | <suffix>` and a newline.
    pub fn write_with_suffix(&self, dir: Option<&Path>, filename: &str, suffix: &str) {
        let _ = std::fs::write(
            utils::resolve(dir, filename),
            format!("{} | {suffix}\n", self.value),
        );
    }
}

/// `os << hash`: the value in decimal.
impl fmt::Display for FnvHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_accumulator() {
        assert_eq!(fnv_init(), 0x811c_9dc5);
        // one byte: (0x811c9dc5 ^ 0x61) * 16777619, kept in 64 bits
        assert_eq!(
            fnv_add(fnv_init(), b"a"),
            (0x811c_9dc5u64 ^ 0x61) * 16_777_619
        );
        let mut h = FnvHash::new();
        h.add_bytes(b"a");
        assert_eq!(h.value, fnv_add(fnv_init(), b"a"));
        assert_eq!(h.to_string(), h.value.to_string());
        let mut c = 7;
        let v = fnv_hash(1, 2, &mut c, -0.001, 0.001);
        assert_eq!(c, 8);
        assert!((-0.001..=0.001).contains(&f64::from(v)));
        let mut m = i32::MAX;
        fnv_hash(0, 0, &mut m, 0.0, 1.0);
        assert_eq!(m, i32::MIN);
    }
}
