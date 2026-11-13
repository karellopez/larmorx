// SPDX-License-Identifier: Apache-2.0
//! The C library's readings FreeSurfer's services rely on (spec §0, §2, §4, §9, §10, §14, §15),
//! as glibc 2.35 gives them in the C locale: `strerror`'s texts, `atoi`, `atof`, `sscanf`'s
//! `0x%lx` and `%d`, C++'s `std::stol`, `glob` (in [`glob`]) and the local time (in [`time`]).
//! Written from the C and C++ standards and POSIX (the spec's §21), with glibc's choices read as
//! a black box (the probe's `clib`, `files` and `time` groups).

mod strerror_table;

pub mod glob;

use larmorx_core::cscan;

pub use larmorx_core::cscan::is_c_space;

/// `strerror(n)` in the C locale: glibc 2.35's text, `Unknown error <n>` for a code it has none
/// for.
pub fn strerror(n: i32) -> String {
    usize::try_from(n)
        .ok()
        .and_then(|i| strerror_table::TEXTS.get(i).copied().flatten())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Unknown error {n}"))
}

/// `strtol(s, NULL, 10)` with a 64-bit `long`: white space, a sign, decimal digits, clamped to
/// the `long` range; 0 without digits.
pub fn strtol(s: &[u8]) -> i64 {
    cscan::strtol10(s).0
}

/// `atoi(s)`: [`strtol`]'s value, its low 32 bits.
pub fn atoi(s: &[u8]) -> i32 {
    strtol(s) as i32
}

/// `atof(s)`: `strtod`'s reading (decimal and hexadecimal numbers, `inf`, `nan` with a
/// payload); 0 without a number.
pub fn atof(s: &[u8]) -> f64 {
    cscan::strtod(s).0
}

/// What C++'s `std::stol` does with a string (base 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stol {
    Value(i64),
    /// No digits: `std::invalid_argument`.
    InvalidArgument,
    /// Beyond the 64-bit range: `std::out_of_range`.
    OutOfRange,
}

/// `std::stol(s)`: white space, a sign, decimal digits (the rest ignored).
pub fn stol(s: &[u8]) -> Stol {
    let (v, used) = cscan::strtol10(s);
    if used == 0 {
        return Stol::InvalidArgument;
    }
    // the digits' exact value decides the range (strtol10 clamps)
    let mut i = 0;
    while i < s.len() && is_c_space(s[i]) {
        i += 1;
    }
    let neg = s.get(i) == Some(&b'-');
    if matches!(s.get(i), Some(b'+' | b'-')) {
        i += 1;
    }
    let mut exact: i128 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        exact = (exact * 10 + i128::from(s[i] - b'0')).min(1 << 70);
        i += 1;
    }
    let exact = if neg { -exact } else { exact };
    if exact < i128::from(i64::MIN) || exact > i128::from(i64::MAX) {
        Stol::OutOfRange
    } else {
        Stol::Value(v)
    }
}

/// `sscanf(s, "%d", &v)`: `Some` value (the `long` reading's low 32 bits), `None` for a matching
/// failure, and the count it returns (1, 0, or -1 at the end of the input before a conversion).
pub fn scan_d(s: &[u8]) -> (i32, Option<i32>) {
    let mut i = 0;
    while i < s.len() && is_c_space(s[i]) {
        i += 1;
    }
    if i >= s.len() {
        return (-1, None);
    }
    match cscan::sscanf_i32(s) {
        Some(v) => (1, Some(v)),
        None => (0, None),
    }
}

/// `sscanf(s, "0x%lx", &v)` (the diagnostic bits' reading, §4): the literal `0x` (the first two
/// bytes exactly), then `%lx`: white space, a sign, an optional `0x`/`0X`, hexadecimal digits
/// (beyond 64 bits: all ones), a minus negating modulo 2^64. The count it returns (1, 0, or -1
/// at the end of the input before the conversion) and the value.
pub fn scan_0x_lx(s: &[u8]) -> (i32, Option<u64>) {
    for (i, want) in [b'0', b'x'].into_iter().enumerate() {
        match s.get(i) {
            None => return (-1, None),
            Some(&c) if c == want => {}
            Some(_) => return (0, None),
        }
    }
    let mut i = 2;
    while i < s.len() && is_c_space(s[i]) {
        i += 1;
    }
    if i >= s.len() {
        return (-1, None);
    }
    let mut neg = false;
    if s[i] == b'+' || s[i] == b'-' {
        neg = s[i] == b'-';
        i += 1;
    }
    if i + 1 < s.len()
        && s[i] == b'0'
        && (s[i + 1] == b'x' || s[i + 1] == b'X')
        && s.get(i + 2).is_some_and(u8::is_ascii_hexdigit)
    {
        i += 2;
    }
    let first = i;
    let mut v: u64 = 0;
    let mut over = false;
    while let Some(d) = s.get(i).and_then(|c| (*c as char).to_digit(16)) {
        match v.checked_mul(16).and_then(|x| x.checked_add(u64::from(d))) {
            Some(x) => v = x,
            None => over = true,
        }
        i += 1;
    }
    if i == first {
        return (0, None);
    }
    if over {
        v = u64::MAX;
    } else if neg {
        v = v.wrapping_neg();
    }
    (1, Some(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strerror_texts() {
        assert_eq!(strerror(0), "Success");
        assert_eq!(strerror(2), "No such file or directory");
        assert_eq!(strerror(41), "Unknown error 41");
        assert_eq!(strerror(134), "Unknown error 134");
        assert_eq!(strerror(-2), "Unknown error -2");
        // the spec's §2: the SHA-256 of the lines "<n>\t<text>\n" for 0 to 133
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        for n in 0..134 {
            h.update(format!("{n}\t{}\n", strerror(n)).as_bytes());
        }
        let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "f986d0f7191c5b16f925e7bbdffddb87b71196a3058f577eecfb4fc54ecece94"
        );
    }

    #[test]
    fn readings() {
        assert_eq!(atoi(b"99999999999999999999"), -1);
        assert_eq!(atoi(b"-2147483649"), 2147483647);
        assert_eq!(atoi(b"-99999999999999999999"), 0);
        assert_eq!(stol(b" 12abc"), Stol::Value(12));
        assert_eq!(stol(b"-0x10"), Stol::Value(0));
        assert_eq!(stol(b"+"), Stol::InvalidArgument);
        assert_eq!(stol(b"9223372036854775808"), Stol::OutOfRange);
        assert_eq!(stol(b"-9223372036854775808"), Stol::Value(i64::MIN));
        assert_eq!(scan_0x_lx(b"0x0x14000"), (1, Some(0x14000)));
        assert_eq!(scan_0x_lx(b"0x-1"), (1, Some(u64::MAX)));
        assert_eq!(scan_0x_lx(b"0X4000"), (0, None));
        assert_eq!(scan_0x_lx(b"0x"), (-1, None));
        assert_eq!(scan_0x_lx(b"0"), (-1, None));
        assert_eq!(scan_0x_lx(b"0xg"), (0, None));
        assert_eq!(scan_0x_lx(b"0x 10"), (1, Some(16)));
        assert_eq!(scan_d(b""), (-1, None));
        assert_eq!(scan_d(b"4294967297"), (1, Some(1)));
        assert_eq!(scan_d(b".5"), (0, None));
    }
}
