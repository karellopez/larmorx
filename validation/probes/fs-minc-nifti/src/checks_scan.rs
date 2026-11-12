// SPDX-License-Identifier: Apache-2.0
//! `larmorx_core::cscan` against the C library's `sscanf` (`%lf`, `%d`) and `strtod`.

use std::ffi::{CString, c_char, c_int};

use larmorx_core::cscan;
use larmorx_core::x87::cases::SplitMix64;

use crate::Tally;

unsafe extern "C" {
    fn sscanf(s: *const c_char, fmt: *const c_char, ...) -> c_int;
    fn strtod(s: *const c_char, end: *mut *mut c_char) -> f64;
}

fn c_lf(s: &[u8]) -> Option<f64> {
    let c = CString::new(s).unwrap();
    let mut d = 0.0f64;
    // SAFETY: a NUL-terminated string and one double for one conversion.
    let n = unsafe { sscanf(c.as_ptr(), c"%lf".as_ptr(), &mut d as *mut f64) };
    (n == 1).then_some(d)
}

fn c_d(s: &[u8]) -> Option<i32> {
    let c = CString::new(s).unwrap();
    let mut d: c_int = 0;
    // SAFETY: a NUL-terminated string and one int for one conversion.
    let n = unsafe { sscanf(c.as_ptr(), c"%d".as_ptr(), &mut d as *mut c_int) };
    (n == 1).then_some(d)
}

fn c_strtod(s: &[u8]) -> (f64, usize) {
    let c = CString::new(s).unwrap();
    let mut end: *mut c_char = std::ptr::null_mut();
    // SAFETY: a NUL-terminated string; end points into it.
    let v = unsafe { strtod(c.as_ptr(), &mut end) };
    (v, end as usize - c.as_ptr() as usize)
}

/// A token in the alphabet that decides `%lf`'s and `strtod`'s paths.
pub fn token(rng: &mut SplitMix64) -> Vec<u8> {
    const WORDS: &[&str] = &[
        "inf", "INF", "infinity", "Infinity", "infin", "infinit", "in", "i", "nan", "NaN", "na",
        "n", "nan()", "nan(123)", "nan(0x1f)", "nan(abc", "nan(a_b)", "0x", "0X", "0x.", "0x.8",
        "0x1p", "0x1p+", "0x1p-3", "0xg", "1e", "1e+", "1e-", "e5", ".e5", ".", "-", "+", "--",
        "+-", "1.5.3", "1e400", "-1e400", "1e-400", "4.9e-324", "2.4703282292062327e-324",
        "2.4703282292062328e-324", "1.7976931348623157e308", "1.7976931348623158e308",
        "0x1.fffffffffffffp1023", "0x1.fffffffffffff8p1023", "0x1p-1074", "0x1p-1075",
        "0x1.8p-1075", "0x0.0000000000001p-1022", "00012", "-0", "+0.0", "1,5", "1;", "x", "2147483647", "2147483648", "-2147483649",
        "4294967297", "99999999999999999999999", "-9223372036854775809", "+12abc", " 7",
    ];
    const ALPHA: &[u8] = b"0123456789.eE+-xXpPabcdfinINaAtyY() \t\x0b";
    match rng.next_u64() % 10 {
        0..=2 => WORDS[(rng.next_u64() % WORDS.len() as u64) as usize].as_bytes().to_vec(),
        3 => {
            let v = f64::from_bits(rng.next_u64());
            format!("{v:e}").into_bytes()
        }
        4 => {
            let v = (rng.next_u64() as i64 as f64) / 2f64.powi((rng.next_u64() % 80) as i32);
            larmorx_core::cfmt::g(v, 1 + (rng.next_u64() % 20) as usize).into_bytes()
        }
        5 => {
            // A long decimal string near a rounding boundary.
            let v = f64::from_bits(rng.next_u64() >> 2);
            let mut s = format!("{v:.40e}");
            if rng.next_u64() % 2 == 0 {
                s = s.replace("e", "000000000001e");
            }
            s.into_bytes()
        }
        6 => {
            let mant = rng.next_u64();
            let e = (rng.next_u64() % 2300) as i64 - 1150;
            format!("0x{:x}.{:x}p{e}", mant >> 40, mant & 0xffffff).into_bytes()
        }
        _ => {
            let n = 1 + rng.next_u64() % 12;
            (0..n).map(|_| ALPHA[(rng.next_u64() % ALPHA.len() as u64) as usize]).collect()
        }
    }
}

pub fn check(scale: u64) -> Vec<Tally> {
    let mut t_lf = Tally::new("scan %lf");
    let mut t_d = Tally::new("scan %d");
    let mut t_s = Tally::new("strtod");
    let mut rng = SplitMix64::new(0x5ca7_0001);
    let n = 400_000 * scale;
    for _ in 0..n {
        let mut s = token(&mut rng);
        if rng.next_u64() % 4 == 0 {
            let mut more = token(&mut rng);
            s.append(&mut more);
        }
        let shown = || format!("{:?}", String::from_utf8_lossy(&s));
        t_lf.cases += 1;
        let ours = cscan::sscanf_f64(&s).map(f64::to_bits);
        let theirs = c_lf(&s).map(f64::to_bits);
        t_lf.eq(&|| format!("sscanf(%lf) {}", shown()), ours, theirs);
        t_s.cases += 1;
        let (a, ua) = cscan::strtod(&s);
        let (b, ub) = c_strtod(&s);
        t_s.eq(&|| format!("strtod {}", shown()), (a.to_bits(), ua), (b.to_bits(), ub));
        t_d.cases += 1;
        t_d.eq(&|| format!("sscanf(%d) {}", shown()), cscan::sscanf_i32(&s), c_d(&s));
    }
    vec![t_lf, t_s, t_d]
}
