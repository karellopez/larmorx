// SPDX-License-Identifier: Apache-2.0
//! The original AFNI command lines (`larmorx afni <program> ...`).

pub mod automask;
pub(crate) mod common;
pub mod tshift;
pub mod unifize;
pub mod volreg;

use std::io::Write;

use larmorx_core::cli::Overrides;
use larmorx_core::defects::EXIT_BAD_MODE;

use crate::settings::AfniSettings;

/// The AFNI programs on the command line.
pub const TOOLS: &[&str] = &["3dTshift", "3dAutomask", "3dUnifize", "3dvolreg"];

/// Runs AFNI program `tool` with `args` (the arguments after the program name). `None` if
/// there is no such program.
pub fn run(tool: &str, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<u8> {
    match tool {
        "3dTshift" => Some(tshift::main(args, out, err)),
        "3dAutomask" => Some(automask::main(args, out, err)),
        "3dUnifize" => Some(unifize::main(args, out, err)),
        "3dvolreg" => Some(volreg::main(args, out, err)),
        _ => None,
    }
}

/// [`run`] with larmorx's own options (`larmorx --n-threads N --upstream-defects MODE afni
/// ...`): each one given wins over the variable it replaces (`OMP_NUM_THREADS`,
/// `LARMORX_UPSTREAM_DEFECTS`); AFNI's settings from the process environment. Without options,
/// [`run`].
pub fn run_given(
    tool: &str,
    args: &[String],
    given: &Overrides,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<u8> {
    if given.is_empty() || !TOOLS.contains(&tool) {
        return run(tool, args, out, err);
    }
    let mode = match given.upstream_defects() {
        Ok(mode) => mode,
        Err(e) => {
            let _ = writeln!(err, "** FATAL ERROR: {e}");
            return Some(EXIT_BAD_MODE);
        }
    };
    let settings = AfniSettings::from_env();
    let n = given.n_threads_or(threads);
    Some(match tool {
        "3dTshift" => tshift::main_with_threads(args, mode, settings, n, out, err),
        "3dAutomask" => automask::main_with_settings(args, mode, settings, n, out, err),
        "3dUnifize" => unifize::main_with_settings(args, mode, settings, n, out, err),
        _ => volreg::main_with_settings(args, mode, settings, n, out, err),
    })
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
