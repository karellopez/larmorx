// SPDX-License-Identifier: Apache-2.0
//! The command lines of the `mri` family (`larmorx mri <tool> ...`).

pub mod hmc;

use std::io::Write;

/// The tools on the command line.
pub const TOOLS: &[&str] = &["hmc"];

/// Runs tool `tool` with `args` (the arguments after the tool name). `None` if there is no
/// such tool.
pub fn run(tool: &str, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<u8> {
    match tool {
        "hmc" => Some(hmc::main(args, out, err)),
        _ => None,
    }
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

/// C's `atof`: the longest leading decimal number, or 0.
pub(crate) fn atof(s: &str) -> f64 {
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
        return 0.0;
    }
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
    t[..i].parse().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_number_parsing() {
        assert_eq!(atoi("abc"), 0);
        assert_eq!(atoi("12x"), 12);
        assert_eq!(atoi("-1"), -1);
        assert_eq!(atof("2.5mm"), 2.5);
        assert_eq!(atof("1e-1"), 0.1);
        assert_eq!(atof("x"), 0.0);
    }
}
