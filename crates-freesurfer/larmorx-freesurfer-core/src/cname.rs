// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A C `char[STRLEN]` name field (`STRLEN` = 1,024 bytes) with C's string semantics, for the fields
//! whose whole buffer reaches a file (`VOL_GEOM::fname`, written 512 bytes at a time with
//! `memcpy`, and `MRI_FRAME::name`, written whole): the bytes after a name's NUL are whatever an
//! earlier, longer name left in the same buffer, and FreeSurfer writes them (larmorx's own code:
//! ISO C's `strcpy`, `memcpy`, `memset` and `strlen` on a fixed buffer).
//!
//! Every such buffer FreeSurfer's reached code makes starts as 1,024 zero bytes (C++'s member
//! initialiser `= {'\0'}`, or `calloc`), so every byte is deterministic: an earlier name's bytes or
//! zeros. The orchestrator's decision (2026-11-13): both modes keep those bytes and write them (rule
//! C: no result depends on them; the files stay byte-identical).

use std::fmt;

/// `STRLEN` (`const.h`).
pub const STRLEN: usize = 1024;

/// A `char[STRLEN]` buffer.
#[derive(Clone, PartialEq, Eq)]
pub struct CName(Box<[u8; STRLEN]>);

impl Default for CName {
    /// A buffer of zeros (the empty name).
    fn default() -> Self {
        CName(Box::new([0u8; STRLEN]))
    }
}

impl fmt::Debug for CName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(self.c_bytes()))
    }
}

impl CName {
    /// A zeroed buffer holding `s` (C's `strcpy` into a new buffer; `s` up to its first NUL, at
    /// most `STRLEN - 1` bytes: longer names are cut, as no reached caller writes one).
    pub fn new(s: &[u8]) -> CName {
        let mut n = CName::default();
        n.strcpy_bytes(s);
        n
    }

    /// The whole buffer.
    pub fn buffer(&self) -> &[u8; STRLEN] {
        &self.0
    }

    /// The whole buffer, to be written into (`memcpy`/`fread` into it: the caller sets the bytes).
    pub fn buffer_mut(&mut self) -> &mut [u8; STRLEN] {
        &mut self.0
    }

    /// `strlen(name)`: the bytes before the first NUL (all 1,024 when there is none).
    pub fn len(&self) -> usize {
        self.0.iter().position(|&c| c == 0).unwrap_or(STRLEN)
    }

    /// Whether the name is empty (its first byte is NUL).
    pub fn is_empty(&self) -> bool {
        self.0[0] == 0
    }

    /// The name as C reads it: the bytes before the first NUL.
    pub fn c_bytes(&self) -> &[u8] {
        &self.0[..self.len()]
    }

    /// The name as text (bytes that are not UTF-8 replaced): for messages built with `{}`.
    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(self.c_bytes()).into_owned()
    }

    /// `strcpy(this, other)`: `other`'s name and its NUL; the bytes after them stay this buffer's.
    pub fn strcpy(&mut self, other: &CName) {
        let n = other.len().min(STRLEN - 1);
        self.0[..n].copy_from_slice(&other.0[..n]);
        self.0[n] = 0;
    }

    /// `strcpy(this, s)` for a C string `s` (its bytes up to the first NUL, at most `STRLEN - 1`).
    pub fn strcpy_bytes(&mut self, s: &[u8]) {
        let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        let n = end.min(STRLEN - 1);
        self.0[..n].copy_from_slice(&s[..n]);
        self.0[n] = 0;
    }

    /// `memset(this, 0, n)`.
    pub fn memset0(&mut self, n: usize) {
        let n = n.min(STRLEN);
        self.0[..n].fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strcpy_keeps_the_tail() {
        let mut a = CName::new(b"longer.mgz");
        assert_eq!(a.c_bytes(), b"longer.mgz");
        a.strcpy_bytes(b"ab");
        assert_eq!(a.c_bytes(), b"ab");
        assert_eq!(&a.buffer()[..11], b"ab\0ger.mgz\0");
        let mut b = CName::default();
        b.strcpy(&a);
        assert_eq!(&b.buffer()[..4], b"ab\0\0");
        let mut c = CName::new(b"xyz");
        c.memset0(2);
        assert_eq!(&c.buffer()[..4], b"\0\0z\0");
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }
}
