//! The original AFNI command lines (`larmorx afni <program> ...`).

pub mod tshift;

use std::io::Write;

/// The AFNI programs on the command line.
pub const TOOLS: &[&str] = &["3dTshift"];

/// Runs AFNI program `tool` with `args` (the arguments after the program name). `None` if
/// there is no such program.
pub fn run(tool: &str, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<u8> {
    match tool {
        "3dTshift" => Some(tshift::main(args, out, err)),
        _ => None,
    }
}

/// Threads for the command line: `OMP_NUM_THREADS` if set (AFNI's variable), else all logical
/// CPUs. Results do not depend on it.
pub(crate) fn threads() -> usize {
    std::env::var("OMP_NUM_THREADS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// C's `strtod` on the start of `s`: the longest leading decimal number (no hexadecimal,
/// infinities or NaN) and the rest of the text.
pub(crate) fn leading_number(s: &str) -> Option<(f64, &str)> {
    let t = s.trim_start();
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let digits_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - digits_start;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits += i - frac;
    }
    if digits == 0 {
        return None;
    }
    // An exponent only counts if it has digits.
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            i = j;
        }
    }
    let value: f64 = t[..i].parse().ok()?;
    Some((value, &t[i..]))
}

/// C's `atof`: the leading number, or 0.
pub(crate) fn atof(s: &str) -> f64 {
    leading_number(s).map_or(0.0, |(v, _)| v)
}

/// C's `atoi`: the leading integer, or 0.
pub(crate) fn atoi(s: &str) -> i64 {
    let t = s.trim_start();
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    t[..i].parse::<i64>().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_number_parsing() {
        assert_eq!(leading_number("2s"), Some((2.0, "s")));
        assert_eq!(leading_number("2000ms"), Some((2000.0, "ms")));
        assert_eq!(leading_number("1.5e0s"), Some((1.5, "s")));
        assert_eq!(leading_number("2e"), Some((2.0, "e")));
        assert_eq!(leading_number(".5"), Some((0.5, "")));
        assert_eq!(leading_number("abc"), None);
        assert_eq!(leading_number("-"), None);
        assert_eq!(atof("abc"), 0.0);
        assert_eq!(atof("1e0x"), 1.0);
        assert_eq!(atoi("1.7"), 1);
        assert_eq!(atoi("abc"), 0);
        assert_eq!(atoi("-3"), -3);
    }
}
