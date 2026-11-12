// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The C library's `glob(pattern, 0, NULL, &g)` as FreeSurfer's `FileNameFromWildcard` uses it:
//! larmorx's own code, written from POSIX's description of `glob` and `fnmatch` (pattern
//! matching notation, XCU 2.13), no C library source.
//!
//! - The pattern is matched one `/`-separated component at a time; a component with `*`, `?` or
//!   `[` is matched against the directory's entries, any other is taken as it is (its `\`
//!   escapes removed).
//! - `*` matches any bytes, `?` one, `[...]` one of a set (`!` or `^` first negates; ranges;
//!   a `]` first is literal; `[:alpha:]` and the other POSIX classes of the C locale); `\`
//!   quotes the next byte (no `GLOB_NOESCAPE`).
//! - A leading `.` of a name is matched only by a literal `.` (no `GLOB_PERIOD`).
//! - Only paths that exist are returned, sorted in byte order (`strcoll` in the C locale);
//!   no match gives an empty list (`GLOB_NOMATCH`).

use std::path::Path;

use crate::utils::resolve;

fn has_magic(comp: &[u8]) -> bool {
    let mut i = 0;
    while i < comp.len() {
        match comp[i] {
            b'\\' => i += 1,
            b'*' | b'?' | b'[' => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

fn unescape(comp: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(comp.len());
    let mut i = 0;
    while i < comp.len() {
        if comp[i] == b'\\' && i + 1 < comp.len() {
            i += 1;
        }
        out.push(comp[i]);
        i += 1;
    }
    out
}

fn in_class(name: &[u8], c: u8) -> Option<bool> {
    Some(match name {
        b"alpha" => c.is_ascii_alphabetic(),
        b"digit" => c.is_ascii_digit(),
        b"alnum" => c.is_ascii_alphanumeric(),
        b"upper" => c.is_ascii_uppercase(),
        b"lower" => c.is_ascii_lowercase(),
        b"space" => crate::cstd::is_space(c),
        b"blank" => c == b' ' || c == b'\t',
        b"punct" => c.is_ascii_punctuation(),
        b"print" => (0x20..0x7f).contains(&c),
        b"graph" => (0x21..0x7f).contains(&c),
        b"cntrl" => c < 0x20 || c == 0x7f,
        b"xdigit" => c.is_ascii_hexdigit(),
        _ => return None,
    })
}

/// A bracket expression at `p[0] == b'['`: whether `c` is in it and the bytes it takes; `None`
/// when it is not closed (then `[` is an ordinary byte).
fn bracket(p: &[u8], c: u8) -> Option<(bool, usize)> {
    let mut i = 1;
    let negate = i < p.len() && (p[i] == b'!' || p[i] == b'^');
    if negate {
        i += 1;
    }
    let mut matched = false;
    let mut first = true;
    loop {
        if i >= p.len() {
            return None;
        }
        if p[i] == b']' && !first {
            return Some((matched != negate, i + 1));
        }
        first = false;
        let lo;
        if p[i] == b'[' && p.get(i + 1) == Some(&b':') {
            let end = p[i + 2..].windows(2).position(|w| w == b":]")?;
            let name = &p[i + 2..i + 2 + end];
            matched |= in_class(name, c)?;
            i += end + 4;
            continue;
        } else if p[i] == b'\\' && i + 1 < p.len() {
            lo = p[i + 1];
            i += 2;
        } else {
            lo = p[i];
            i += 1;
        }
        if i + 1 < p.len() && p[i] == b'-' && p[i + 1] != b']' {
            let mut hi = p[i + 1];
            i += 2;
            if hi == b'\\' && i < p.len() {
                hi = p[i];
                i += 1;
            }
            matched |= lo <= c && c <= hi;
        } else {
            matched |= c == lo;
        }
    }
}

/// `fnmatch(p, s, FNM_PERIOD)` on one path component.
pub fn fnmatch(p: &[u8], s: &[u8]) -> bool {
    if s.first() == Some(&b'.') && !matches!(p.first(), Some(&b'.')) {
        return false;
    }
    matches(p, s)
}

fn matches(p: &[u8], s: &[u8]) -> bool {
    if p.is_empty() {
        return s.is_empty();
    }
    match p[0] {
        b'*' => (0..=s.len()).any(|k| matches(&p[1..], &s[k..])),
        b'?' => !s.is_empty() && matches(&p[1..], &s[1..]),
        b'[' => match bracket(p, s.first().copied().unwrap_or(0)) {
            Some((hit, used)) => !s.is_empty() && hit && matches(&p[used..], &s[1..]),
            None => s.first() == Some(&b'[') && matches(&p[1..], &s[1..]),
        },
        b'\\' if p.len() > 1 => s.first() == Some(&p[1]) && matches(&p[2..], &s[1..]),
        c => s.first() == Some(&c) && matches(&p[1..], &s[1..]),
    }
}

fn join(base: &str, name: &str) -> String {
    match base {
        "" => name.to_owned(),
        "/" => format!("/{name}"),
        b => format!("{b}/{name}"),
    }
}

/// `glob(pattern, 0)`: the existing paths that match, sorted; relative ones start in `dir`.
pub fn glob(dir: Option<&Path>, pattern: &str) -> Vec<String> {
    let (root, rest) = match pattern.strip_prefix('/') {
        Some(r) => ("/", r),
        None => ("", pattern),
    };
    let mut paths = vec![root.to_owned()];
    for comp in rest
        .as_bytes()
        .split(|&b| b == b'/')
        .filter(|c| !c.is_empty())
    {
        let mut next = Vec::new();
        for base in &paths {
            if has_magic(comp) {
                let at = resolve(dir, if base.is_empty() { "." } else { base });
                let Ok(entries) = std::fs::read_dir(at) else {
                    continue;
                };
                let mut names: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect();
                // readdir also lists "." and "..", which a pattern starting with "." can match.
                names.push(".".to_owned());
                names.push("..".to_owned());
                for name in names {
                    if fnmatch(comp, name.as_bytes()) {
                        next.push(join(base, &name));
                    }
                }
            } else {
                let name = String::from_utf8_lossy(&unescape(comp)).into_owned();
                next.push(join(base, &name));
            }
        }
        paths = next;
    }
    let mut found: Vec<String> = paths
        .into_iter()
        .filter(|p| !p.is_empty() && std::fs::symlink_metadata(resolve(dir, p)).is_ok())
        .collect();
    found.sort();
    found.dedup();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnmatch_notation() {
        assert!(fnmatch(b"*.mgz", b"orig.mgz"));
        assert!(!fnmatch(b"*.mgz", b".hidden.mgz"));
        assert!(fnmatch(b".*", b".hidden"));
        assert!(fnmatch(b"a?c", b"abc"));
        assert!(fnmatch(b"[a-c]x", b"bx"));
        assert!(!fnmatch(b"[!a-c]x", b"bx"));
        assert!(fnmatch(b"[]]", b"]"));
        assert!(fnmatch(b"[[:digit:]]*", b"7up"));
        assert!(fnmatch(b"a\\*", b"a*"));
        assert!(!fnmatch(b"a\\*", b"ab"));
        assert!(fnmatch(b"[ab", b"[ab"));
    }

    #[test]
    fn glob_sorted_existing() {
        let d = tempfile::tempdir().unwrap();
        for n in ["b.mgz", "a.mgz", ".c.mgz", "x.nii"] {
            std::fs::write(d.path().join(n), b"").unwrap();
        }
        std::fs::create_dir(d.path().join("s1")).unwrap();
        std::fs::write(d.path().join("s1").join("t.mgz"), b"").unwrap();
        assert_eq!(glob(Some(d.path()), "*.mgz"), ["a.mgz", "b.mgz"]);
        assert_eq!(glob(Some(d.path()), "s*/t.mgz"), ["s1/t.mgz"]);
        assert!(glob(Some(d.path()), "*.none").is_empty());
        let abs = d.path().to_string_lossy().replace('\\', "/");
        if abs.starts_with('/') {
            assert_eq!(glob(None, &format!("{abs}/x*")), [format!("{abs}/x.nii")]);
        }
    }
}
