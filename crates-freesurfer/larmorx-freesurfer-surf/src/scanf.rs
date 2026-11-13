// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! One call of C's `sscanf` or `fscanf` as the surface library's text readers make it
//! (`fs.surfio`, queue item 3.1-48): text that must match, white space, `%d`, `%f` (and `%e`),
//! `%s` with a width, and the suppressed forms (`%*d`, `%*f`, `%*s`); the value each conversion
//! assigns comes from fs.core's conversions ([`cscan`]), and the call's return value and `errno`
//! follow glibc 2.35's behaviour **measured as a black box**
//! (`validation/probes/fs-surfio/scanf_errno.c`), no glibc code:
//!
//! - the call returns the number of items it assigned, or `EOF` (-1) when its input ended before
//!   it assigned any (a matching failure returns the count);
//! - a number beyond its type's range sets `errno` to `ERANGE`, suppressed or not;
//! - the first time the call meets the end of its input it remembers `errno` as it is then, and
//!   every later attempt of the same call to read a character (a conversion's item, a text
//!   character, the white space a format ends with) sets `errno` back to that value; an end met
//!   while a conversion skips the white space before its item is remembered as 0 (`sscanf("",
//!   "%d")` leaves `errno` 0, `sscanf("", "x%d")` leaves it as it was, `sscanf("1e99", "%f %d")`
//!   leaves it as it was before the call although the `%f` overflowed, `sscanf("1e99 ", "%f %d")`
//!   leaves it 0).
//!
//! A call that stops (a matching or input failure) reads nothing more, so a format's final white
//! space is skipped only by a call that reached it.

use larmorx_freesurfer_core::cscan::{self, ERANGE, Scan, Stream};
use larmorx_freesurfer_core::cstd::is_space;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stop {
    Matching,
    Input,
}

/// One `scanf` call on a stream.
pub struct Scanf<'a> {
    st: &'a mut dyn Stream,
    /// The call met the end of its input.
    eof: bool,
    /// `errno` as it was then.
    saved: i32,
    /// A white-space directive is pending.
    skip: bool,
    done: i32,
    stop: Option<Stop>,
}

/// The stream as a conversion reads it: the first end it meets is remembered with `errno_now`.
struct Tracked<'b, 'a> {
    sc: &'b mut Scanf<'a>,
    errno_now: i32,
}

impl Stream for Tracked<'_, '_> {
    fn peek(&mut self) -> Option<u8> {
        self.sc.peek(self.errno_now)
    }
    fn bump(&mut self) {
        self.sc.st.bump();
    }
    fn hit_end(&mut self) -> bool {
        self.sc.st.hit_end()
    }
}

impl<'a> Scanf<'a> {
    pub fn new(st: &'a mut dyn Stream) -> Scanf<'a> {
        Scanf {
            st,
            eof: false,
            saved: 0,
            skip: false,
            done: 0,
            stop: None,
        }
    }

    fn peek(&mut self, errno_now: i32) -> Option<u8> {
        let c = self.st.peek();
        if c.is_none() && !self.eof {
            self.eof = true;
            self.saved = errno_now;
        }
        c
    }

    /// Whether the call has stopped.
    pub fn stopped(&self) -> bool {
        self.stop.is_some()
    }

    /// The format's text: white space (any amount, none included) and characters that must
    /// match.
    pub fn text(&mut self, errno: &mut i32, t: &[u8]) {
        for &b in t {
            if self.stop.is_some() {
                return;
            }
            if is_space(b) {
                self.skip = true;
                continue;
            }
            if self.eof {
                *errno = self.saved;
                self.stop = Some(Stop::Input);
                return;
            }
            loop {
                match self.peek(*errno) {
                    None => {
                        self.stop = Some(Stop::Input);
                        return;
                    }
                    Some(c) if self.skip && is_space(c) => self.st.bump(),
                    Some(c) => {
                        self.skip = false;
                        if c == b {
                            self.st.bump();
                        } else {
                            self.stop = Some(Stop::Matching);
                        }
                        break;
                    }
                }
            }
        }
    }

    /// The start of a conversion: the white space before its item (an end met there remembered
    /// as 0), then its first character; `false` when the call stops here.
    fn begin(&mut self, errno: &mut i32) -> bool {
        if self.stop.is_some() {
            return false;
        }
        self.skip = false;
        if !self.eof {
            while let Some(c) = self.peek(0) {
                if !is_space(c) {
                    return true;
                }
                self.st.bump();
            }
        }
        *errno = self.saved;
        self.stop = Some(Stop::Input);
        false
    }

    fn after<T>(&mut self, errno: &mut i32, errno_before: i32, r: Scan<T>, assign: bool) -> Option<T> {
        if !self.eof && self.st.hit_end() {
            self.eof = true;
            self.saved = errno_before;
        }
        match r {
            Scan::Value(v, erange) => {
                if erange {
                    *errno = ERANGE;
                }
                if assign {
                    self.done += 1;
                }
                Some(v)
            }
            Scan::Fail => {
                self.stop = Some(Stop::Matching);
                None
            }
            Scan::Eof => {
                self.stop = Some(Stop::Input);
                None
            }
        }
    }

    /// `%d` (`assign` false: `%*d`): the value read, or `None` when the call stops.
    pub fn d(&mut self, errno: &mut i32, assign: bool) -> Option<i32> {
        if !self.begin(errno) {
            return None;
        }
        let before = *errno;
        let r = cscan::scan_i32(&mut Tracked {
            sc: self,
            errno_now: before,
        });
        self.after(errno, before, r, assign)
    }

    /// `%f` or `%e` into a `float` (`assign` false: suppressed).
    pub fn f(&mut self, errno: &mut i32, assign: bool) -> Option<f32> {
        if !self.begin(errno) {
            return None;
        }
        let before = *errno;
        let r = cscan::scan_f32(&mut Tracked {
            sc: self,
            errno_now: before,
        });
        self.after(errno, before, r, assign)
    }

    /// `%<width>s` (`assign` false: `%*s`): the characters before white space or the end, at most
    /// `width`.
    pub fn s(&mut self, errno: &mut i32, width: usize, assign: bool) -> Option<Vec<u8>> {
        if !self.begin(errno) {
            return None;
        }
        let mut w = Vec::new();
        while w.len() < width {
            match self.peek(*errno) {
                Some(c) if !is_space(c) => {
                    w.push(c);
                    self.st.bump();
                }
                _ => break,
            }
        }
        if assign {
            self.done += 1;
        }
        Some(w)
    }

    /// The end of the format: its final white space skipped (when the call reached it); the
    /// call's return value.
    pub fn finish(&mut self, errno: &mut i32) -> i32 {
        if self.stop.is_none() && self.skip {
            if self.eof {
                *errno = self.saved;
            } else {
                while let Some(c) = self.peek(*errno) {
                    if !is_space(c) {
                        break;
                    }
                    self.st.bump();
                }
            }
        }
        if self.stop == Some(Stop::Input) && self.done == 0 {
            -1
        } else {
            self.done
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_freesurfer_core::cscan::Bytes;

    /// The black-box measurements (`scanf_errno.c`, glibc 2.35, `errno` 5 before each call):
    /// the format as a sequence of directives, the return value and `errno` after.
    #[test]
    fn glibc_measurements() {
        let run = |input: &str, f: &dyn Fn(&mut Scanf, &mut i32)| {
            let mut b = Bytes::new(input.as_bytes());
            let mut sc = Scanf::new(&mut b);
            let mut errno = 5;
            f(&mut sc, &mut errno);
            (sc.finish(&mut errno), errno)
        };
        let points = |sc: &mut Scanf, e: &mut i32| {
            sc.text(e, b"POINTS ");
            sc.d(e, true);
            sc.text(e, b" float\n");
        };
        assert_eq!(run("POINTS", &points), (-1, 0));
        assert_eq!(run("POINTS ", &points), (-1, 0));
        assert_eq!(run("POINT", &points), (-1, 5));
        assert_eq!(run("POINTSX", &points), (0, 5));
        assert_eq!(run("POINTS 5", &points), (1, 5));
        assert_eq!(run("POINTS 99999999999 float", &points), (1, 5));
        let curv = |sc: &mut Scanf, e: &mut i32| {
            sc.d(e, false);
            sc.text(e, b" ");
            sc.f(e, false);
            sc.text(e, b" ");
            sc.f(e, false);
            sc.text(e, b" ");
            sc.f(e, false);
            sc.text(e, b" ");
            sc.f(e, true);
            sc.text(e, b"\n");
        };
        assert_eq!(run("12", &curv), (-1, 5));
        assert_eq!(run("12 1 2 3", &curv), (-1, 5));
        assert_eq!(run("99999999999 1e99 2 3 4", &curv), (1, 34));
        assert_eq!(run("1 2 3 4 1e99", &curv), (1, 5));
        assert_eq!(run("x 2 3 4 5", &curv), (0, 5));
        let field = |sc: &mut Scanf, e: &mut i32| {
            sc.text(e, b"FIELD ");
            sc.s(e, usize::MAX, false);
            sc.text(e, b" ");
            sc.d(e, true);
            sc.text(e, b"\n");
        };
        assert_eq!(run("FIELD", &field), (-1, 0));
        assert_eq!(run("FIELD a", &field), (-1, 5));
        assert_eq!(run("FIELD a 3", &field), (1, 5));
        let named = |sc: &mut Scanf, e: &mut i32| {
            sc.s(e, usize::MAX, true);
            sc.text(e, b" ");
            sc.d(e, true);
            sc.text(e, b" ");
            sc.d(e, true);
            sc.text(e, b" ");
            sc.s(e, usize::MAX, false);
            sc.text(e, b"\n");
        };
        assert_eq!(run("curv", &named), (1, 5));
        assert_eq!(run("curv 1", &named), (2, 5));
        assert_eq!(run("curv 1 2", &named), (3, 5));
        assert_eq!(run("", &|sc, e| {
            sc.d(e, true);
        }), (-1, 0));
        assert_eq!(run("", &|sc, e| {
            sc.text(e, b"x");
            sc.d(e, true);
        }), (-1, 5));
        assert_eq!(run(" ", &|sc, e| {
            sc.s(e, usize::MAX, false);
        }), (-1, 0));
        let fd = |sc: &mut Scanf, e: &mut i32| {
            sc.f(e, true);
            sc.text(e, b" ");
            sc.d(e, true);
        };
        assert_eq!(run("1e99", &fd), (1, 5));
        assert_eq!(run("1e99 ", &fd), (1, 0));
        assert_eq!(run("1e99 x", &fd), (1, 34));
        assert_eq!(run("inf", &fd), (1, 5));
        assert_eq!(run("1e99", &|sc, e| {
            sc.f(e, true);
            sc.text(e, b" x");
        }), (1, 5));
        assert_eq!(run("1e99", &|sc, e| {
            sc.f(e, true);
            sc.text(e, b"\n");
        }), (1, 5));
        let fff = |sc: &mut Scanf, e: &mut i32| {
            for k in 0..3 {
                if k > 0 {
                    sc.text(e, b" ");
                }
                sc.f(e, true);
            }
        };
        assert_eq!(run("1e99", &fff), (1, 5));
        assert_eq!(run("1 2", &fff), (2, 5));
    }
}
