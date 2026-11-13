// SPDX-License-Identifier: Apache-2.0
//! The FNV-1a hash (Glenn Fowler, Landon Curt Noll, Kiem-Phong Vo; public domain): for each
//! byte, the hash is XORed with the byte, then multiplied by the prime, modulo 2^32 (FNV-1a
//! 32: offset basis 2166136261, prime 16777619) or 2^64 (FNV-1a 64: offset basis
//! 14695981039346656037, prime 1099511628211). Written from the algorithm's public
//! description (Noll's FNV page, the IETF draft `draft-eastlake-fnv`).
//!
//! [`fnv1a_32_wide`]: the 32-bit offset basis and prime with a 64-bit accumulator, as C code
//! that keeps the hash in an `unsigned long` computes it on LP64 systems (Linux and macOS on
//! 64 bits; Windows' `unsigned long` has 32 bits): the multiplication wraps modulo 2^64, so the
//! low 32 bits are FNV-1a 32's and the high bits carry on.

/// FNV-1a 32's offset basis.
pub const FNV32_OFFSET: u32 = 2_166_136_261;
/// FNV-1a 32's prime.
pub const FNV32_PRIME: u32 = 16_777_619;
/// FNV-1a 64's offset basis.
pub const FNV64_OFFSET: u64 = 14_695_981_039_346_656_037;
/// FNV-1a 64's prime.
pub const FNV64_PRIME: u64 = 1_099_511_628_211;

/// FNV-1a 32 of `bytes`, continuing from `hash` ([`FNV32_OFFSET`] to start).
pub fn fnv1a_32(mut hash: u32, bytes: &[u8]) -> u32 {
    for &b in bytes {
        hash ^= b as u32;
        hash = hash.wrapping_mul(FNV32_PRIME);
    }
    hash
}

/// FNV-1a 64 of `bytes`, continuing from `hash` ([`FNV64_OFFSET`] to start).
pub fn fnv1a_64(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV64_PRIME);
    }
    hash
}

/// FNV-1a 32's offset basis and prime with a 64-bit accumulator (module docs), continuing from
/// `hash` (`FNV32_OFFSET as u64` to start).
pub fn fnv1a_32_wide(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV32_PRIME as u64);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_vectors() {
        // Noll's test vectors (FNV-1a).
        assert_eq!(fnv1a_32(FNV32_OFFSET, b""), 0x811c9dc5);
        assert_eq!(fnv1a_32(FNV32_OFFSET, b"a"), 0xe40c292c);
        assert_eq!(fnv1a_32(FNV32_OFFSET, b"foobar"), 0xbf9cf968);
        assert_eq!(fnv1a_64(FNV64_OFFSET, b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a_64(FNV64_OFFSET, b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a_64(FNV64_OFFSET, b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn wide_low_bits_are_fnv1a_32() {
        let mut bytes = Vec::new();
        let mut x = 1u64;
        for n in 0..2000 {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            bytes.push((x >> 56) as u8);
            let wide = fnv1a_32_wide(FNV32_OFFSET as u64, &bytes[..n]);
            assert_eq!(wide as u32, fnv1a_32(FNV32_OFFSET, &bytes[..n]));
        }
        // in pieces as at once
        let (a, b) = bytes.split_at(777);
        let whole = fnv1a_32_wide(FNV32_OFFSET as u64, &bytes);
        assert_eq!(
            fnv1a_32_wide(fnv1a_32_wide(FNV32_OFFSET as u64, a), b),
            whole
        );
        // the 64-bit value of 2,000 bytes, as the probe (validation/probes/fs-misc) found LP64
        // C code computing it
        assert_eq!(whole, WIDE_2000);
    }

    const WIDE_2000: u64 = 0xfee1_0e5a_0c77_fb5b;
}
