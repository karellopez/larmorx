// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! HDF5's checksums: Bob Jenkins' lookup3 hash (`H5_checksum_lookup3`, used for every
//! version-2 metadata structure and for link-name hashes) and Fletcher-32
//! (`H5_checksum_fletcher32`, the `fletcher32` filter).
//!
//! Translated from HDF5 1.12.1 `src/H5checksum.c` (BSD-style licence, see NOTICE); lookup3 is
//! Bob Jenkins' public-domain code.

/// `H5_checksum_lookup3(key, len, initval)`.
pub fn lookup3(key: &[u8], initval: u32) -> u32 {
    fn rot(x: u32, k: u32) -> u32 {
        x.rotate_left(k)
    }
    let mut length = key.len();
    let mut a = 0xdead_beef_u32
        .wrapping_add(length as u32)
        .wrapping_add(initval);
    let mut b = a;
    let mut c = a;
    let mut k = key;
    let word = |k: &[u8], i: usize| -> u32 {
        u32::from(k[i])
            | (u32::from(k[i + 1]) << 8)
            | (u32::from(k[i + 2]) << 16)
            | (u32::from(k[i + 3]) << 24)
    };
    while length > 12 {
        a = a.wrapping_add(word(k, 0));
        b = b.wrapping_add(word(k, 4));
        c = c.wrapping_add(word(k, 8));
        // mix(a, b, c)
        a = a.wrapping_sub(c);
        a ^= rot(c, 4);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= rot(a, 6);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= rot(b, 8);
        b = b.wrapping_add(a);
        a = a.wrapping_sub(c);
        a ^= rot(c, 16);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= rot(a, 19);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= rot(b, 4);
        b = b.wrapping_add(a);
        length -= 12;
        k = &k[12..];
    }
    if length == 0 {
        return c;
    }
    // The last block: bytes beyond `length` count as zero.
    let mut tail = [0u8; 12];
    tail[..length].copy_from_slice(&k[..length]);
    a = a.wrapping_add(word(&tail, 0));
    if length > 4 {
        b = b.wrapping_add(word(&tail, 4));
    }
    if length > 8 {
        c = c.wrapping_add(word(&tail, 8));
    }
    // final(a, b, c)
    c ^= b;
    c = c.wrapping_sub(rot(b, 14));
    a ^= c;
    a = a.wrapping_sub(rot(c, 11));
    b ^= a;
    b = b.wrapping_sub(rot(a, 25));
    c ^= b;
    c = c.wrapping_sub(rot(b, 16));
    a ^= c;
    a = a.wrapping_sub(rot(c, 4));
    b ^= a;
    b = b.wrapping_sub(rot(a, 14));
    c ^= b;
    c = c.wrapping_sub(rot(b, 24));
    c
}

/// `H5_checksum_metadata`: lookup3 with an initial value of 0.
pub fn metadata(data: &[u8]) -> u32 {
    lookup3(data, 0)
}

/// `H5_checksum_fletcher32`: 16-bit big-endian words, sums reduced every 360 words.
pub fn fletcher32(data: &[u8]) -> u32 {
    let (mut sum1, mut sum2) = (0u32, 0u32);
    for block in data.chunks(720) {
        let words = block.len() / 2;
        for w in 0..words {
            sum1 += (u32::from(block[2 * w]) << 8) | u32::from(block[2 * w + 1]);
            sum2 += sum1;
        }
        if words > 0 {
            sum1 = (sum1 & 0xffff) + (sum1 >> 16);
            sum2 = (sum2 & 0xffff) + (sum2 >> 16);
        }
    }
    if data.len() % 2 == 1 {
        sum1 += u32::from(data[data.len() - 1]) << 8;
        sum2 += sum1;
        sum1 = (sum1 & 0xffff) + (sum1 >> 16);
        sum2 = (sum2 & 0xffff) + (sum2 >> 16);
    }
    sum1 = (sum1 & 0xffff) + (sum1 >> 16);
    sum2 = (sum2 & 0xffff) + (sum2 >> 16);
    (sum2 << 16) | sum1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup3_reference_values() {
        // Bob Jenkins' published test values for hashlittle.
        assert_eq!(lookup3(b"", 0), 0xdead_beef);
        assert_eq!(lookup3(b"", 0xdead_beef), 0xbd5b_7dde);
        assert_eq!(lookup3(b"Four score and seven years ago", 0), 0x1777_0551);
        assert_eq!(lookup3(b"Four score and seven years ago", 1), 0xcd62_8161);
    }

    #[test]
    fn fletcher32_matches_definition() {
        // Odd lengths and blocks longer than 360 words.
        let data: Vec<u8> = (0..2001u32).map(|i| (i * 7 + 3) as u8).collect();
        for len in [0, 1, 2, 3, 719, 720, 721, 2001] {
            let d = &data[..len];
            // Reference: the same sums without the periodic reduction, in 64 bits.
            let (mut s1, mut s2) = (0u64, 0u64);
            let mut i = 0;
            while i + 1 < d.len() {
                s1 = (s1 + ((u64::from(d[i]) << 8) | u64::from(d[i + 1]))) % 65535;
                s2 = (s2 + s1) % 65535;
                i += 2;
            }
            if d.len() % 2 == 1 {
                s1 = (s1 + (u64::from(d[d.len() - 1]) << 8)) % 65535;
                s2 = (s2 + s1) % 65535;
            }
            let got = fletcher32(d);
            // HDF5's ones'-complement reduction may leave 0xffff where the modulus gives 0.
            let norm = |x: u32| if x == 0xffff { 0 } else { u64::from(x) };
            assert_eq!(norm(got & 0xffff), s1, "len {len}");
            assert_eq!(norm(got >> 16), s2, "len {len}");
        }
    }
}
