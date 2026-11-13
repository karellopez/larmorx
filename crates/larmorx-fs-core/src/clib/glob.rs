// SPDX-License-Identifier: Apache-2.0
//! The C library's `glob(pattern, 0, NULL, &g)` in the C locale (POSIX.1-2017's `glob` and
//! `fnmatch`; spec §10's file name from a wildcard): `*`, `?`, bracket expressions (`[...]`,
//! `[!...]`, ranges, classes), `\` escapes; a leading `.` of a name matched only by a literal
//! `.`; the matches sorted in byte order; `.` and `..` among a directory's names.

use std::path::Path;

use crate::files::path_of;

/// `GLOB_NOMATCH`.
pub const NOMATCH: i32 = 3;

fn is_magic(c: u8) -> bool {
    matches!(c, b'*' | b'?' | b'[')
}

/// Whether a pattern holds an unescaped wildcard.
fn has_magic(p: &[u8]) -> bool {
    let mut i = 0;
    while i < p.len() {
        if p[i] == b'\\' {
            i += 2;
            continue;
        }
        if is_magic(p[i]) {
            return true;
        }
        i += 1;
    }
    false
}

/// A pattern without its escapes.
fn unescape(p: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(p.len());
    let mut i = 0;
    while i < p.len() {
        if p[i] == b'\\' && i + 1 < p.len() {
            i += 1;
        }
        out.push(p[i]);
        i += 1;
    }
    out
}

fn class(name: &[u8], c: u8) -> Option<bool> {
    let ch = c as char;
    Some(match name {
        b"alpha" => ch.is_ascii_alphabetic(),
        b"digit" => ch.is_ascii_digit(),
        b"alnum" => ch.is_ascii_alphanumeric(),
        b"upper" => ch.is_ascii_uppercase(),
        b"lower" => ch.is_ascii_lowercase(),
        b"space" => crate::clib::is_c_space(c),
        b"blank" => c == b' ' || c == b'\t',
        b"punct" => ch.is_ascii_punctuation(),
        b"xdigit" => ch.is_ascii_hexdigit(),
        b"cntrl" => ch.is_ascii_control(),
        b"graph" => ch.is_ascii_graphic(),
        b"print" => ch.is_ascii_graphic() || c == b' ',
        _ => return None,
    })
}

/// A bracket expression at `p[i] == '['` against `c`: `Some((matched, end))` with `end` past
/// its `]`, `None` when it is not terminated (then `[` is a literal).
fn bracket(p: &[u8], i: usize, c: u8) -> Option<(bool, usize)> {
    let mut j = i + 1;
    let negate = matches!(p.get(j), Some(b'!' | b'^'));
    if negate {
        j += 1;
    }
    let mut matched = false;
    let mut first = true;
    loop {
        let &b = p.get(j)?;
        if b == b']' && !first {
            return Some((matched != negate, j + 1));
        }
        first = false;
        if b == b'['
            && p.get(j + 1) == Some(&b':')
            && let Some(len) = p[j + 2..].windows(2).position(|w| w == b":]")
            && let Some(m) = class(&p[j + 2..j + 2 + len], c)
        {
            matched |= m;
            j += 2 + len + 2;
            continue;
        }
        let (lo, next) = if b == b'\\' && j + 1 < p.len() {
            (p[j + 1], j + 2)
        } else {
            (b, j + 1)
        };
        if p.get(next) == Some(&b'-') && p.get(next + 1).is_some_and(|&e| e != b']') {
            let (hi, after) = if p[next + 1] == b'\\' && next + 2 < p.len() {
                (p[next + 2], next + 3)
            } else {
                (p[next + 1], next + 2)
            };
            matched |= lo <= c && c <= hi;
            j = after;
        } else {
            matched |= c == lo;
            j = next;
        }
    }
}

/// `fnmatch(pattern, name, FNM_PERIOD)` on one file name (no `/` in it).
pub fn fnmatch(p: &[u8], name: &[u8]) -> bool {
    fn go(p: &[u8], mut i: usize, s: &[u8], mut k: usize, start: bool) -> bool {
        while i < p.len() {
            let leading_dot = start && k == 0 && s.first() == Some(&b'.');
            match p[i] {
                b'*' => {
                    if leading_dot {
                        return false;
                    }
                    while i < p.len() && p[i] == b'*' {
                        i += 1;
                    }
                    if i == p.len() {
                        return true;
                    }
                    for kk in k..=s.len() {
                        if go(p, i, s, kk, false) {
                            return true;
                        }
                    }
                    return false;
                }
                b'?' => {
                    if k >= s.len() || leading_dot {
                        return false;
                    }
                    i += 1;
                    k += 1;
                }
                b'[' => {
                    if k >= s.len() {
                        return false;
                    }
                    match bracket(p, i, s[k]) {
                        Some((m, end)) => {
                            if !m || leading_dot {
                                return false;
                            }
                            i = end;
                            k += 1;
                        }
                        None => {
                            if s[k] != b'[' {
                                return false;
                            }
                            i += 1;
                            k += 1;
                        }
                    }
                }
                c => {
                    let (lit, next) = if c == b'\\' && i + 1 < p.len() {
                        (p[i + 1], i + 2)
                    } else {
                        (c, i + 1)
                    };
                    if k >= s.len() || s[k] != lit {
                        return false;
                    }
                    i = next;
                    k += 1;
                }
            }
        }
        k == s.len()
    }
    go(p, 0, name, 0, true)
}

/// A directory's names, `.` and `..` included (none when it cannot be read).
fn names(dir: &Path) -> Vec<Vec<u8>> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v = vec![b".".to_vec(), b"..".to_vec()];
    v.extend(rd.flatten().map(|e| crate::env::os_bytes(&e.file_name())));
    v
}

fn exists(p: &[u8]) -> bool {
    std::fs::symlink_metadata(path_of(p)).is_ok()
}

fn expand(pattern: &[u8]) -> Vec<Vec<u8>> {
    if !has_magic(pattern) {
        let u = unescape(pattern);
        return if !u.is_empty() && exists(&u) {
            vec![u]
        } else {
            Vec::new()
        };
    }
    let (dirs, base): (Vec<Option<Vec<u8>>>, &[u8]) = match pattern.iter().rposition(|&c| c == b'/')
    {
        None => (vec![None], pattern),
        Some(i) => {
            let d = &pattern[..i];
            let dirs = if d.is_empty() {
                vec![Some(b"/".to_vec())]
            } else if has_magic(d) {
                expand(d).into_iter().map(Some).collect()
            } else {
                vec![Some(unescape(d))]
            };
            (dirs, &pattern[i + 1..])
        }
    };
    let mut out = Vec::new();
    for d in dirs {
        let join = |name: &[u8]| -> Vec<u8> {
            match &d {
                None => name.to_vec(),
                Some(d) if d == b"/" => [b"/", name].concat(),
                Some(d) => [d.as_slice(), b"/", name].concat(),
            }
        };
        if !has_magic(base) {
            let full = join(&unescape(base));
            if exists(&full) {
                out.push(full);
            }
            continue;
        }
        let dir_path = match &d {
            None => path_of(b"."),
            Some(d) => path_of(d),
        };
        for n in names(&dir_path) {
            if fnmatch(base, &n) {
                out.push(join(&n));
            }
        }
    }
    out
}

/// `glob(pattern, 0)`: 0 and the sorted matches, or [`NOMATCH`] and none.
pub fn glob(pattern: &[u8]) -> (i32, Vec<Vec<u8>>) {
    let mut v = expand(pattern);
    if v.is_empty() {
        return (NOMATCH, v);
    }
    v.sort();
    (0, v)
}

/// `FileNameFromWildcard`: a name holding `*` becomes the first path `glob` finds; no match, or
/// a name without `*`, leaves it.
pub fn file_name_from_wildcard(name: &[u8]) -> Vec<u8> {
    if name.contains(&b'*') {
        let (r, v) = glob(name);
        if r == 0 {
            return v[0].clone();
        }
    }
    name.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching() {
        assert!(fnmatch(b"*.mgz", b"a.mgz"));
        assert!(!fnmatch(b"*.mgz", b".hidden.mgz"));
        assert!(fnmatch(b".*", b".."));
        assert!(fnmatch(b"[!a].mgz", b"b.mgz"));
        assert!(!fnmatch(b"[!a].mgz", b"a.mgz"));
        assert!(!fnmatch(b"x[1].txt", b"x[1].txt"));
        assert!(fnmatch(b"x\\[1\\].txt", b"x[1].txt"));
        assert!(fnmatch(b"[[:digit:]]", b"7"));
        assert!(fnmatch(b"[", b"["));
    }
}
