// SPDX-License-Identifier: Apache-2.0
//! The older programs' command-line helpers (spec §7).

/// `CMDisFlag`: an argument of at least two bytes starting with `--`.
pub fn is_flag(arg: &[u8]) -> bool {
    arg.len() >= 2 && arg.starts_with(b"--")
}

/// `CMDsingleDash`: an argument of at least two bytes starting with one `-` not followed by
/// another.
pub fn is_single_dash(arg: &[u8]) -> bool {
    arg.len() >= 2 && arg[0] == b'-' && arg[1] != b'-'
}

/// `CMDnthIsArg(count, args, nth)`: whether the `nth` argument (from 0) is a value: below the
/// count and not a flag. A negative `nth` is not defined by FreeSurfer: here it is no value.
pub fn nth_is_arg(args: &[&[u8]], nth: i64) -> bool {
    usize::try_from(nth)
        .ok()
        .and_then(|i| args.get(i))
        .is_some_and(|a| !is_flag(a))
}

/// `CMDstringMatch`: both strings given and equal; exactly one absent: no match (both absent
/// is not defined by FreeSurfer: here no match).
pub fn string_match(a: Option<&[u8]>, b: Option<&[u8]>) -> bool {
    matches!((a, b), (Some(x), Some(y)) if x == y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags() {
        assert!(is_flag(b"--"));
        assert!(!is_flag(b"-"));
        assert!(is_single_dash(b"-a"));
        assert!(!is_single_dash(b"--a"));
        assert!(!is_single_dash(b"-"));
        assert!(nth_is_arg(&[b"a", b"--b"], 0));
        assert!(!nth_is_arg(&[b"a", b"--b"], 1));
        assert!(!nth_is_arg(&[b"a"], 1));
        assert!(string_match(Some(b""), Some(b"")));
        assert!(!string_match(Some(b"a"), None));
    }
}
