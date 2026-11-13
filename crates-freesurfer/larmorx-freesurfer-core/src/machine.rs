// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's byte-order swaps (`fs.core`, queue item 3.1-22), translated from FreeSurfer
//! 8.2.0's `utils/machine.cpp`: `swapDouble`, `swapLong32`, `swapFloat`, `swapShort`, `swapInt`.
//! Each reverses the bytes of its value (`swapDouble` by four pair exchanges, the 32-bit ones by
//! swapping the bytes of each 16-bit half and then the halves): the result is the value's bytes
//! in reverse order, bit patterns kept (a float's NaN payload included).
//!
//! Not reached by any default-path program, so not translated: `swapLong64` (it returns the
//! swapped 64-bit integer through a `double`, which loses the bits beyond 53: a defect to
//! register when a program reaches it) and `swapUShort`.

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
    }
}
