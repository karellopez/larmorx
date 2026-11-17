// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The folder separator of file names (larmorx's own code, not a translation; track A relay 44).
//!
//! FreeSurfer runs only where `/` separates folders (Linux, macOS), and its name functions split
//! names at `/` alone. The port runs on Windows too, where `\` separates folders as well
//! (`C:\data\sub\mri\orig.mgz`): split at `/` only, such a name would be taken as a bare name
//! (its folder lost, or the working directory or the subject's folder put before it). These
//! helpers take `/` as a separator everywhere and `\` as well **on Windows only**, so Linux and
//! macOS keep FreeSurfer's results byte for byte (a `\` there is an ordinary byte of a name).
//!
//! Every function that decides by a name's folders uses them: where the last folder ends, whether a
//! name has a folder, its trailing separators, whether it is absolute. Names the port only prints
//! or stores (a GIFTI array's `Name`, a header's field) are data, the same on every platform, and
//! stay split at `/` as FreeSurfer splits them.

use std::path::Path;

/// Whether `c` separates folders with Windows' rule (`windows`) or FreeSurfer's (`/` only).
pub const fn is_sep_on(c: u8, windows: bool) -> bool {
    c == b'/' || (windows && c == b'\\')
}

/// Whether `c` separates folders here: `/` everywhere, `\` too on Windows.
pub const fn is_sep(c: u8) -> bool {
    is_sep_on(c, cfg!(windows))
}

/// The index of the last separator in `name` ([`is_sep_on`]).
pub fn rfind_sep_on(name: &[u8], windows: bool) -> Option<usize> {
    name.iter().rposition(|&c| is_sep_on(c, windows))
}

/// The index of the last separator in `name` (`strrchr(name, '/')`, with `\` on Windows).
pub fn rfind_sep(name: &[u8]) -> Option<usize> {
    rfind_sep_on(name, cfg!(windows))
}

/// [`rfind_sep`] of a `&str` (a byte index; both separators are ASCII, so it is a character
/// boundary).
pub fn rfind_sep_str(name: &str) -> Option<usize> {
    rfind_sep(name.as_bytes())
}

/// Whether `name` holds a separator (`strchr(name, '/')`, with `\` on Windows).
pub fn has_sep(name: &[u8]) -> bool {
    name.iter().any(|&c| is_sep(c))
}

/// [`has_sep`] of a `&str`.
pub fn has_sep_str(name: &str) -> bool {
    has_sep(name.as_bytes())
}

/// Whether `name` ends with a separator.
pub fn ends_with_sep(name: &[u8]) -> bool {
    name.last().is_some_and(|&c| is_sep(c))
}

/// `name` without its trailing separators ([`is_sep_on`]).
pub fn trim_end_seps_on(name: &[u8], windows: bool) -> &[u8] {
    let n = name
        .iter()
        .rposition(|&c| !is_sep_on(c, windows))
        .map_or(0, |i| i + 1);
    &name[..n]
}

/// `name` without its trailing separators.
pub fn trim_end_seps(name: &[u8]) -> &[u8] {
    trim_end_seps_on(name, cfg!(windows))
}

/// [`trim_end_seps`] of a `&str`.
pub fn trim_end_seps_str(name: &str) -> &str {
    let n = trim_end_seps(name.as_bytes()).len();
    &name[..n]
}

/// Whether FreeSurfer's test "the name starts with `/`" holds here: on Linux and macOS exactly
/// that; on Windows also a name that starts with `\` (the drive's root) or is absolute for
/// Windows (`C:\x`, `C:/x`, `\\server\share\x`).
pub fn is_absolute_name(name: &[u8]) -> bool {
    match name.first() {
        Some(b'/') => true,
        Some(b'\\') => cfg!(windows),
        Some(_) if cfg!(windows) => Path::new(&*String::from_utf8_lossy(name)).is_absolute(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separators_by_platform() {
        assert!(is_sep_on(b'/', false) && is_sep_on(b'/', true));
        assert!(!is_sep_on(b'\\', false) && is_sep_on(b'\\', true));
        assert_eq!(is_sep(b'\\'), cfg!(windows));
        let n = b"C:\\data\\sub/mri\\orig.mgz";
        assert_eq!(rfind_sep_on(n, false), Some(11));
        assert_eq!(rfind_sep_on(n, true), Some(15));
        assert_eq!(rfind_sep_on(b"orig.mgz", true), None);
        assert_eq!(trim_end_seps_on(b"a/b\\//\\", true), b"a/b");
        assert_eq!(trim_end_seps_on(b"a/b\\//\\", false), b"a/b\\//\\");
        assert_eq!(trim_end_seps_on(b"a/b//", false), b"a/b");
        assert_eq!(trim_end_seps_on(b"///", true), b"");
        assert_eq!(trim_end_seps_str("x/"), "x");
    }

    #[test]
    fn here() {
        let windows = cfg!(windows);
        assert_eq!(has_sep_str("a\\b"), windows);
        assert!(has_sep_str("a/b") && !has_sep_str("ab"));
        assert_eq!(rfind_sep_str("a\\b"), windows.then_some(1));
        assert_eq!(ends_with_sep(b"a\\"), windows);
        assert!(ends_with_sep(b"a/") && !ends_with_sep(b""));
        assert!(is_absolute_name(b"/x") && !is_absolute_name(b"x") && !is_absolute_name(b""));
        assert_eq!(is_absolute_name(b"\\x"), windows);
        assert_eq!(is_absolute_name(b"C:\\x"), windows);
        assert_eq!(is_absolute_name(b"C:/x"), windows);
        assert!(!is_absolute_name(b"C:x"));
    }
}
