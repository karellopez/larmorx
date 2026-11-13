// SPDX-License-Identifier: Apache-2.0
//! Strings and file names (spec §10). Strings are bytes (FreeSurfer's end at the first NUL; the
//! callers here pass the bytes before it); white space is the C locale's; case changes touch
//! ASCII letters only.

use std::io::BufRead;

use larmorx_core::defects::FS_11;

use crate::clib::is_c_space;
use crate::process::Process;

/// The bytes before the first NUL.
pub fn c_str(s: &[u8]) -> &[u8] {
    s.iter().position(|&b| b == 0).map_or(s, |i| &s[..i])
}

/// `StrUpper`, `str_toupper` (in place).
pub fn str_upper(s: &mut [u8]) {
    s.make_ascii_uppercase();
}

/// `StrLower` (in place).
pub fn str_lower(s: &mut [u8]) {
    s.make_ascii_lowercase();
}

fn last(s: &[u8], c: u8) -> Option<usize> {
    s.iter().rposition(|&b| b == c)
}

/// The part after the last `/` (all of it without one).
fn after_slash(s: &[u8]) -> &[u8] {
    last(s, b'/').map_or(s, |i| &s[i + 1..])
}

/// Cut at the last `#`, then at the last `@`.
fn cut_hash_at(mut s: &[u8]) -> &[u8] {
    if let Some(i) = last(s, b'#') {
        s = &s[..i];
    }
    if let Some(i) = last(s, b'@') {
        s = &s[..i];
    }
    s
}

/// `FileNameOnly`: the part after the last `/`, cut at its last `#`, then at its last `@`.
pub fn file_name_only(s: &[u8]) -> Vec<u8> {
    cut_hash_at(after_slash(s)).to_vec()
}

/// `FileName` (in place): the part after the last `/`, after skipping one leading `@`, cut at
/// its last `#`, then at its last `@`; the caller's string ends where the result ends. Returns
/// the result's start in the (shortened) string.
pub fn file_name_in_place(s: &mut Vec<u8>) -> usize {
    let mut start = last(s, b'/').map_or(0, |i| i + 1);
    if s.get(start) == Some(&b'@') {
        start += 1;
    }
    let len = cut_hash_at(&s[start..]).len();
    s.truncate(start + len);
    start
}

/// `FileNamePath`: everything before the last `/` (empty for `/name`); `.` without one.
pub fn file_name_path(s: &[u8]) -> Vec<u8> {
    match last(s, b'/') {
        Some(i) => s[..i].to_vec(),
        None => b".".to_vec(),
    }
}

/// `FileNameRemoveExtension`: cut at the last `.` (even inside a directory name).
pub fn file_name_remove_extension(s: &[u8]) -> Vec<u8> {
    last(s, b'.').map_or(s, |i| &s[..i]).to_vec()
}

impl Process {
    /// `FileNameExtension`: after the last `.`, empty without one. FS-11: a string of 1,024
    /// bytes or more overflows FreeSurfer's buffer; both modes take the whole string, noted.
    pub fn file_name_extension(&mut self, s: &[u8]) -> Vec<u8> {
        if s.len() >= 1024 {
            self.note(
                &FS_11,
                "a file name of 1,024 bytes or more read whole (FreeSurfer's buffer overflows)",
            );
        }
        last(s, b'.').map_or(&[][..], |i| &s[i + 1..]).to_vec()
    }

    /// `stricmp`: the difference of the first differing bytes after upper-casing, as unsigned
    /// bytes; 0 when equal. FS-11 as [`Process::file_name_extension`].
    pub fn stricmp(&mut self, a: &[u8], b: &[u8]) -> i32 {
        if a.len() >= 1024 || b.len() >= 1024 {
            self.note(
                &FS_11,
                "a string of 1,024 bytes or more compared whole (FreeSurfer's buffer overflows)",
            );
        }
        stricmp(a, b)
    }

    /// `GetNthItemFromString(s, n)`: the n-th white-space separated item (from 0; a negative n:
    /// the last); else the error line and `None`.
    pub fn nth_item(&mut self, s: &[u8], n: i32) -> Option<Vec<u8>> {
        let items: Vec<&[u8]> = s
            .split(|&c| is_c_space(c))
            .filter(|w| !w.is_empty())
            .collect();
        let count = items.len() as i32;
        let k = if n < 0 { count - 1 } else { n };
        if k >= 0 && k < count {
            return Some(items[k as usize].to_vec());
        }
        let shown = if n < 0 { -1 } else { n };
        self.put_err(
            format!("error: asking for item {shown}, but {count} items in string\n").as_bytes(),
        );
        None
    }
}

/// The comparison of [`Process::stricmp`] (without its notice).
pub fn stricmp(a: &[u8], b: &[u8]) -> i32 {
    let up = |c: u8| i32::from(c.to_ascii_uppercase());
    for i in 0.. {
        let x = a.get(i).copied().map_or(0, up);
        let y = b.get(i).copied().map_or(0, up);
        if x != y || x == 0 {
            return x - y;
        }
    }
    0
}

/// `ItemsInString`: runs of bytes other than a space; -1 for an empty string.
pub fn items_in_string(s: &[u8]) -> i32 {
    if s.is_empty() {
        return -1;
    }
    s.split(|&c| c == b' ').filter(|w| !w.is_empty()).count() as i32
}

/// `CountItemsInString`: runs of bytes other than space and tab; the count stops where a run
/// would start with `\n` or `\r`.
pub fn count_items_in_string(s: &[u8]) -> i32 {
    let mut n = 0;
    let mut i = 0;
    loop {
        while i < s.len() && (s[i] == b' ' || s[i] == b'\t') {
            i += 1;
        }
        if i >= s.len() || s[i] == b'\n' || s[i] == b'\r' {
            return n;
        }
        n += 1;
        while i < s.len() && s[i] != b' ' && s[i] != b'\t' {
            i += 1;
        }
    }
}

/// `deblank`: the string without its white space.
pub fn deblank(s: &[u8]) -> Vec<u8> {
    s.iter().copied().filter(|&c| !is_c_space(c)).collect()
}

/// `FindFirstMatch`: the first index of `value`, or -1.
pub fn find_first_match(list: &[i32], value: i32) -> i32 {
    list.iter()
        .position(|&v| v == value)
        .map_or(-1, |i| i as i32)
}

/// `stringEndsWith`: whether `s` ends with `end` (the empty string ends every string).
pub fn string_ends_with(s: &[u8], end: &[u8]) -> bool {
    s.ends_with(end)
}

/// `getEnvironVar`: the variable's value, empty when unset.
pub fn environ_var(p: &Process, name: &str) -> Vec<u8> {
    p.env.get(name).unwrap_or(b"").to_vec()
}

/// `argv2cmdline`: each argument followed by one space.
pub fn argv_to_cmdline(args: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for a in args {
        out.extend_from_slice(a);
        out.push(b' ');
    }
    out
}

/// One piece of the C library's `fgets` with a buffer of `n` bytes: at most `n - 1` bytes,
/// through a newline; `None` at the end of the input. A read error ends it too.
fn fgets(r: &mut dyn BufRead, n: usize) -> Option<Vec<u8>> {
    let max = n.saturating_sub(1);
    let mut out = Vec::new();
    while out.len() < max {
        let buf = r.fill_buf().ok()?;
        if buf.is_empty() {
            break;
        }
        let room = max - out.len();
        let take = buf
            .iter()
            .take(room)
            .position(|&c| c == b'\n')
            .map_or(room.min(buf.len()), |i| i + 1);
        out.extend_from_slice(&buf[..take]);
        r.consume(take);
        if out.last() == Some(&b'\n') {
            break;
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// `fgetl(buffer of n bytes, file)`: the next piece `fgets` gives that is not empty, blank or
/// a comment after its leading white space; without that white space, cut at its first `#`,
/// without its final newline. `None` at the end of the file.
pub fn fgetl(r: &mut dyn BufRead, n: usize) -> Option<Vec<u8>> {
    loop {
        let piece = fgets(r, n)?;
        let piece = c_str(&piece);
        let start = piece
            .iter()
            .position(|&c| !is_c_space(c))
            .unwrap_or(piece.len());
        let rest = &piece[start..];
        if rest.is_empty() || rest[0] == b'#' {
            continue;
        }
        let mut line = rest.to_vec();
        if let Some(i) = line.iter().position(|&c| c == b'#') {
            line.truncate(i);
        }
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        return Some(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names() {
        let mut s = b"/a/b/@c.mgz#3@x".to_vec();
        let at = file_name_in_place(&mut s);
        assert_eq!(&s[at..], b"c.mgz");
        assert_eq!(s, b"/a/b/@c.mgz");
        assert_eq!(file_name_only(b"/a/b/@c.mgz#3@x"), b"");
        assert_eq!(file_name_path(b"/name"), b"");
        assert_eq!(file_name_path(b"name"), b".");
        assert_eq!(file_name_remove_extension(b"a.b/c"), b"a");
        assert_eq!(stricmp(b"\xff", b"a"), 190);
        assert_eq!(stricmp(b"aB", b"Ab"), 0);
    }
}
