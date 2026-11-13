// SPDX-License-Identifier: Apache-2.0
//! Byte order (spec §6): one value's bytes reversed (bit patterns kept, NaNs included), the
//! 64-bit integer swap (FS-262) and the buffer swap (FS-261).

use larmorx_core::defects::{FS_261, FS_262};

use crate::process::Process;

/// `swapShort`: a signed 16-bit value's bytes reversed.
pub fn swap_i16(v: i16) -> i16 {
    v.swap_bytes()
}

/// `swapUShort`.
pub fn swap_u16(v: u16) -> u16 {
    v.swap_bytes()
}

/// `swapInt`, `swapLong32`.
pub fn swap_i32(v: i32) -> i32 {
    v.swap_bytes()
}

/// `swapFloat`: the bits reversed (a signalling NaN stays one).
pub fn swap_f32_bits(bits: u32) -> u32 {
    bits.swap_bytes()
}

/// `swapDouble`.
pub fn swap_f64_bits(bits: u64) -> u64 {
    bits.swap_bytes()
}

/// `Arch486`: whether this is a little-endian machine, as FreeSurfer's build answers (1).
pub fn arch486() -> i32 {
    1
}

/// FreeSurfer's 64-bit integer swap: the reversal rounded to the nearest double (ties to even)
/// and converted back, truncating (2^63 gives -2^63, as x86-64's conversion does).
fn swap_i64_through_double(v: i64) -> i64 {
    let r = v.swap_bytes();
    let d = r as f64; // round to nearest, ties to even
    if d >= 9223372036854775808.0 {
        i64::MIN
    } else {
        d as i64
    }
}

impl Process {
    /// `swapLong64` (FS-262): `fix` gives the exact reversal, `reproduce` FreeSurfer's rounding
    /// through a double; noted when they differ.
    pub fn swap_i64(&mut self, v: i64) -> i64 {
        let exact = v.swap_bytes();
        let rounded = swap_i64_through_double(v);
        if exact != rounded {
            if self.mode.reproduce() {
                self.note(&FS_262, "a 64-bit integer's swap rounded through a double");
            } else {
                self.note(
                    &FS_262,
                    "a 64-bit integer's swap exact (FreeSurfer rounds it through a double)",
                );
            }
        }
        if self.mode.reproduce() {
            rounded
        } else {
            exact
        }
    }

    /// `ByteSwapBuf(buffer, n, k)`: `n` items of `k` bytes. k = 1 leaves the buffer; k other than
    /// 1, 2, 4 or 8 writes its error on standard output and returns 1. FS-261: `fix` reverses
    /// every item, `reproduce` only the first ⌈n/k⌉; noted when they differ (n ≥ 2).
    pub fn byte_swap_buf(&mut self, buffer: &mut [u8], n: i64, k: i32) -> i32 {
        if k == 1 {
            return 0;
        }
        if !matches!(k, 2 | 4 | 8) {
            self.put_out(
                format!("ERROR: ByteSwapBuf: nBytesPerItem = {k} not supported\n").as_bytes(),
            );
            return 1;
        }
        let k = k as usize;
        let n = n.max(0) as usize;
        let items = if self.mode.reproduce() {
            n.div_ceil(k)
        } else {
            n
        };
        if n.div_ceil(k) != n {
            if self.mode.reproduce() {
                self.note(
                    &FS_261,
                    "only the first ceil(n/k) items of a buffer swapped",
                );
            } else {
                self.note(
                    &FS_261,
                    "every item of a buffer swapped (FreeSurfer swaps the first ceil(n/k))",
                );
            }
        }
        let items = items.min(buffer.len() / k);
        swap_items(&mut buffer[..items * k], k);
        0
    }
}

/// Reverses the bytes of every `k`-byte item of `buffer` (k = 2, 4 or 8; the length a multiple).
pub fn swap_items(buffer: &mut [u8], k: usize) {
    match k {
        2 => buffer
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .for_each(|c| c.swap(0, 1)),
        4 => buffer
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .for_each(|c| *c = u32::from_ne_bytes(*c).swap_bytes().to_ne_bytes()),
        8 => buffer
            .as_chunks_mut::<8>()
            .0
            .iter_mut()
            .for_each(|c| *c = u64::from_ne_bytes(*c).swap_bytes().to_ne_bytes()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long64_rounding() {
        assert_eq!(
            swap_i64_through_double(0x0100000000000001),
            72057594037927936
        );
        assert_eq!(
            swap_i64_through_double(0x0123456789abcdef),
            -1167088121787636992
        );
        assert_eq!(0x0123456789abcdefi64.swap_bytes(), -1167088121787636991);
    }
}
