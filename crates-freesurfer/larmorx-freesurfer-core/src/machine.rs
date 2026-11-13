// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's byte-order swaps (`fs.core`, queue item 3.1-22), translated from FreeSurfer
//! 8.2.0's `utils/machine.cpp`: `swapDouble`, `swapLong32`, `swapFloat`, `swapShort`, `swapInt`,
//! and (with `fs.io`'s 64-bit readers and writers, queue item 3.1-25) `swapLong64`. Each reverses
//! the bytes of its value (`swapDouble` by four pair exchanges, the 32-bit ones by swapping the
//! bytes of each 16-bit half and then the halves): the result is the value's bytes in reverse
//! order, bit patterns kept (a float's NaN payload included); `swapLong64` then passes its result
//! through a `double` (FS-176). With `fs.io`'s readers (track A relay 12): `swapUShort`, and
//! `ByteSwapBuf` with `ByteSwap2`, `ByteSwap4`, `ByteSwap8` (FS-261).

use std::io::Write;

use larmorx_core::defects::{FS_176, FS_261};

use crate::globals::Globals;

/// `swapDouble`.
pub fn swap_double(d: f64) -> f64 {
    f64::from_bits(d.to_bits().swap_bytes())
}

/// `swapLong32` (a 32-bit `long32`).
pub fn swap_long32(l: i32) -> i32 {
    l.swap_bytes()
}

/// `swapFloat`.
pub fn swap_float(f: f32) -> f32 {
    f32::from_bits(f.to_bits().swap_bytes())
}

/// `swapShort`.
pub fn swap_short(s: i16) -> i16 {
    s.swap_bytes()
}

/// `swapInt`.
pub fn swap_int(i: i32) -> i32 {
    i.swap_bytes()
}

/// `swapUShort`.
pub fn swap_ushort(s: u16) -> u16 {
    s.swap_bytes()
}

/// `ByteSwap2`, `ByteSwap4`, `ByteSwap8` (`k` 2, 4, 8): the byte order of each `k`-byte item of
/// `buf` reversed, for `nitems` items; 0. FreeSurfer's loop counts `nitems` in steps of `k` while
/// it moves one item per step, so it reverses only the first `ceil(nitems / k)` items (FS-261): by
/// default all `nitems`, with `reproduce` FreeSurfer's, noted where more than one item is asked
/// for. Items beyond the end of `buf` are not touched (FreeSurfer writes past the buffer there).
fn byte_swap_k(g: &mut Globals, buf: &mut [u8], nitems: i64, k: usize) -> i32 {
    if nitems <= 0 {
        return 0;
    }
    let n = nitems as u64;
    let freesurfer = n.div_ceil(k as u64);
    if freesurfer != n {
        g.notices.add(
            &FS_261,
            format!("ByteSwap{k}: {n} items asked for, FreeSurfer reverses the first {freesurfer}"),
        );
    }
    let todo = if g.notices.reproduce() { freesurfer } else { n };
    let fit = (buf.len() / k) as u64;
    for item in buf.chunks_exact_mut(k).take(todo.min(fit) as usize) {
        item.reverse();
    }
    0
}

/// `ByteSwap2(buf, nitems)` (FS-261).
pub fn byte_swap2(g: &mut Globals, buf: &mut [u8], nitems: i64) -> i32 {
    byte_swap_k(g, buf, nitems, 2)
}

/// `ByteSwap4(buf, nitems)` (FS-261).
pub fn byte_swap4(g: &mut Globals, buf: &mut [u8], nitems: i64) -> i32 {
    byte_swap_k(g, buf, nitems, 4)
}

/// `ByteSwap8(buf, nitems)` (FS-261).
pub fn byte_swap8(g: &mut Globals, buf: &mut [u8], nitems: i64) -> i32 {
    byte_swap_k(g, buf, nitems, 8)
}

/// `ByteSwapBuf(buf, nitems, nbytes)`: [`byte_swap2`], [`byte_swap4`] or [`byte_swap8`] for items
/// of 2, 4 or 8 bytes, nothing for 1; 0. Another size: `ERROR: ByteSwapBuf: nBytesPerItem = <k> not
/// supported` on standard output and 1, the buffer unchanged.
pub fn byte_swap_buf(
    g: &mut Globals,
    out: &mut dyn Write,
    buf: &mut [u8],
    nitems: i64,
    nbytes: i32,
) -> i32 {
    match nbytes {
        1 => 0,
        2 => byte_swap2(g, buf, nitems),
        4 => byte_swap4(g, buf, nitems),
        8 => byte_swap8(g, buf, nitems),
        k => {
            let _ = writeln!(out, "ERROR: ByteSwapBuf: nBytesPerItem = {k} not supported");
            1
        }
    }
}

/// `swapLong64(l)`: the bytes of `l` reversed. FreeSurfer stores the result in a `double` before
/// returning it as a `long long` (FS-176): rounded to 53 significant bits (to nearest, ties to
/// even), and -2^63 where the rounding reaches 2^63 (x86-64's conversion). By default the exact
/// result; `reproduce` gives FreeSurfer's. A notice where the two differ.
pub fn swap_long64(g: &mut Globals, l: i64) -> i64 {
    let s = l.swap_bytes();
    let via = s as f64;
    // 2^63, the first double beyond the long long range
    let back = if via >= 2f64.powi(63) {
        i64::MIN
    } else {
        via as i64
    };
    if back != s {
        if g.notices.reproduce() {
            g.notices.add(
                &FS_176,
                format!("swapLong64: {s:#018x} became {back:#018x} through a double"),
            );
            return back;
        }
        g.notices.add(
            &FS_176,
            format!(
                "swapLong64: {s:#018x} kept exact (FreeSurfer's {back:#018x} through a double)"
            ),
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_reverse_the_bytes() {
        assert_eq!(swap_short(0x1234), 0x3412);
        assert_eq!(swap_int(0x0102_0304), 0x0403_0201);
        assert_eq!(swap_long32(-2), 0xfeff_ffffu32 as i32);
        assert_eq!(swap_float(1.0).to_bits(), 0x0000_803f);
        let nan = f32::from_bits(0x7fc0_0001);
        assert_eq!(swap_float(nan).to_bits(), 0x0100_c07f);
        assert_eq!(swap_double(1.0).to_bits(), 0x0000_0000_0000_f03f);
        assert_eq!(swap_double(swap_double(-3.25)), -3.25);
        assert_eq!(swap_ushort(0x1234), 0x3412);
    }

    fn hexb(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn fs261_a_buffer_swap() {
        use larmorx_core::defects::UpstreamDefects;
        // FreeSurfer's results (the unit oracle's fs_core_probe batch, 2026-11-13) with reproduce
        let cases = [
            (
                2,
                4,
                "0102030405060708",
                "0201040305060708",
                "0201040306050807",
            ),
            (
                4,
                3,
                "0102030405060708090a0b0c",
                "0403020105060708090a0b0c",
                "04030201080706050c0b0a09",
            ),
            (
                8,
                2,
                "0102030405060708090a0b0c0d0e0f10",
                "0807060504030201090a0b0c0d0e0f10",
                "0807060504030201100f0e0d0c0b0a09",
            ),
            (1, 4, "01020304", "01020304", "01020304"),
            (2, 0, "0102", "0102", "0102"),
            (2, -1, "0102", "0102", "0102"),
            (
                2,
                5,
                "0102030405060708090a",
                "0201040306050708090a",
                "02010403060508070a09",
            ),
        ];
        for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
            for (k, n, input, reproduced, fixed) in cases {
                let mut g = Globals::new(mode);
                let mut out = Vec::new();
                let mut b = hexb(input);
                assert_eq!(byte_swap_buf(&mut g, &mut out, &mut b, n, k), 0);
                let want = if mode == UpstreamDefects::Reproduce {
                    reproduced
                } else {
                    fixed
                };
                assert_eq!(b, hexb(want), "k {k} n {n}");
                assert_eq!(g.notices.contains(&FS_261), reproduced != fixed);
            }
        }
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut out = Vec::new();
        let mut b = hexb("010203040506");
        assert_eq!(byte_swap_buf(&mut g, &mut out, &mut b, 2, 3), 1);
        assert_eq!(b, hexb("010203040506"));
        assert_eq!(
            out,
            b"ERROR: ByteSwapBuf: nBytesPerItem = 3 not supported\n"
        );
    }
}
