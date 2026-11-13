// SPDX-License-Identifier: Apache-2.0
//! Matrix files (spec §18): the text, ASCII and binary readers and writers, on the C library's
//! `%f` and `%d` as §18.1 describes glibc 2.35's (FS-49 for a binary file that ends early).

use larmorx_core::cscan::{self, is_c_space};
use larmorx_core::defects::FS_49;

use super::Returned;
use super::matrix::{MATRIX_COMPLEX, Matrix};
use crate::clib::printf::sprintf_f64;
use crate::process::{Process, Stop, codes};
use crate::sse;

/// `ERANGE`.
const ERANGE: i32 = 34;

/// A file opened for reading (`FILE *`): its bytes and the position (`ftell`).
#[derive(Clone, Debug, Default)]
pub struct CStream {
    pub bytes: Vec<u8>,
    pub pos: usize,
}

impl CStream {
    /// A stream over `bytes` at its start.
    pub fn new(bytes: Vec<u8>) -> CStream {
        CStream { bytes, pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    /// `fgets(buffer of n bytes)`: up to `n - 1` bytes or through the newline; `None` at the end.
    pub fn fgets(&mut self, n: usize) -> Option<Vec<u8>> {
        if self.pos >= self.bytes.len() || n < 2 {
            return None;
        }
        let mut out = Vec::new();
        while out.len() < n - 1 {
            let Some(c) = self.peek() else { break };
            self.pos += 1;
            out.push(c);
            if c == b'\n' {
                break;
            }
        }
        Some(out)
    }

    /// Reads `n` bytes (fewer at the end).
    fn read(&mut self, n: usize) -> Vec<u8> {
        let end = (self.pos + n).min(self.bytes.len());
        let s = self.bytes[self.pos..end].to_vec();
        self.pos = end;
        s
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(is_c_space) {
            self.pos += 1;
        }
    }
}

/// How one `%f` item ended.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Scan {
    /// A value.
    Value(f32),
    /// A matching failure.
    Matching,
}

/// The item's look at the next character; `met` set when it looks past the end of the input
/// (§18.1: the item meets the end).
fn look(s: &CStream, met: &mut bool) -> Option<u8> {
    let c = s.peek();
    if c.is_none() {
        *met = true;
    }
    c
}

/// A word matched letter by letter, any case: a letter that does not match is read.
fn word(s: &mut CStream, w: &[u8], met: &mut bool) -> bool {
    for &x in w {
        match look(s, met) {
            Some(c) if c.to_ascii_lowercase() == x => s.pos += 1,
            Some(_) => {
                s.pos += 1;
                return false;
            }
            None => return false,
        }
    }
    true
}

/// A run of digits (hexadecimal or decimal): how many were read.
fn digits(s: &mut CStream, hex: bool, met: &mut bool) -> usize {
    let mut n = 0;
    while look(s, met).is_some_and(|c| {
        if hex {
            c.is_ascii_hexdigit()
        } else {
            c.is_ascii_digit()
        }
    }) {
        s.pos += 1;
        n += 1;
    }
    n
}

/// The item of the C library's `%f` (§18.1), its white space already skipped: the outcome and
/// the `errno` it sets (`Some`); `met` when it looked past the end of the input.
fn scan_f(s: &mut CStream, met: &mut bool) -> (Scan, Option<i32>) {
    let start = s.pos;
    let mut neg = false;
    if let Some(c @ (b'+' | b'-')) = look(s, met) {
        neg = c == b'-';
        s.pos += 1;
    }
    match look(s, met) {
        Some(b'i' | b'I') => {
            if !word(s, b"inf", met) {
                return (Scan::Matching, None);
            }
            if let Some(b'i' | b'I') = look(s, met)
                && !word(s, b"inity", met)
            {
                return (Scan::Matching, None);
            }
            let v = if neg {
                f32::NEG_INFINITY
            } else {
                f32::INFINITY
            };
            return (Scan::Value(v), None);
        }
        Some(b'n' | b'N') => {
            if !word(s, b"nan", met) {
                return (Scan::Matching, None);
            }
            let v = f32::from_bits(if neg { 0xffc0_0000 } else { 0x7fc0_0000 });
            return (Scan::Value(v), None);
        }
        _ => {}
    }
    let hex = s.peek() == Some(b'0') && matches!(s.bytes.get(s.pos + 1), Some(b'x' | b'X'));
    if hex {
        s.pos += 2;
        let mut n = digits(s, true, met);
        let mut point = false;
        if look(s, met) == Some(b'.') {
            s.pos += 1;
            point = true;
            n += digits(s, true, met);
        }
        if n == 0 {
            if !point {
                return (Scan::Matching, None);
            }
            let v = if neg { -0.0 } else { 0.0 };
            return (Scan::Value(v), None);
        }
        if let Some(b'p' | b'P') = look(s, met) {
            s.pos += 1;
            if let Some(b'+' | b'-') = look(s, met) {
                s.pos += 1;
            }
            digits(s, false, met);
        }
    } else {
        let mut n = digits(s, false, met);
        if look(s, met) == Some(b'.') {
            s.pos += 1;
            n += digits(s, false, met);
        }
        if n == 0 {
            return (Scan::Matching, None);
        }
        if let Some(b'e' | b'E') = look(s, met) {
            s.pos += 1;
            if let Some(b'+' | b'-') = look(s, met) {
                s.pos += 1;
            }
            digits(s, false, met);
        }
    }
    let text = &s.bytes[start..s.pos];
    let (v, _) = cscan::strtof(text);
    let (d, _) = cscan::strtod(text);
    let mut errno = None;
    if v.is_infinite() {
        errno = Some(ERANGE);
    } else if d != 0.0 && d.abs() < f64::from(f32::MIN_POSITIVE) {
        // inexact and below the smallest normal float (a decimal number there never is exact)
        let exact = f64::from(v) == d && hex_exact(text);
        if !exact {
            errno = Some(ERANGE);
        }
    }
    (Scan::Value(v), errno)
}

/// Whether a hexadecimal number's reading is exact (53 significant bits at most); a decimal one
/// is never taken as exact.
fn hex_exact(text: &[u8]) -> bool {
    let t: Vec<u8> = text.iter().map(u8::to_ascii_lowercase).collect();
    let Some(x) = t.windows(2).position(|w| w == b"0x") else {
        return false;
    };
    let mant: Vec<u8> = t[x + 2..]
        .iter()
        .copied()
        .take_while(|&c| c != b'p')
        .filter(u8::is_ascii_hexdigit)
        .collect();
    let first = mant.iter().position(|&c| c != b'0');
    let last = mant.iter().rposition(|&c| c != b'0');
    match (first, last) {
        (Some(a), Some(b)) => (b - a + 1) * 4 <= 53,
        _ => true,
    }
}

/// One `scanf` call's reading (§18.1): the first time the call meets the end of its input it
/// keeps `errno`'s value (0 when that meeting is a conversion's skip of white space), and every
/// later meeting sets `errno` back to it; each call starts afresh.
struct Call {
    kept: Option<i32>,
}

impl Call {
    fn new() -> Call {
        Call { kept: None }
    }

    /// The call meets the end of its input; `skip`: in a conversion's skip of white space.
    fn meet(&mut self, p: &mut Process, skip: bool) {
        let errno = p.errno;
        p.errno = *self.kept.get_or_insert(if skip { 0 } else { errno });
    }

    /// A conversion's skip of white space; false at the end of the input (an input failure).
    fn skip(&mut self, p: &mut Process, s: &mut CStream) -> bool {
        s.skip_space();
        if s.peek().is_none() {
            self.meet(p, true);
            return false;
        }
        true
    }

    /// White space in the format that reads (not just before a conversion).
    fn ws(&mut self, p: &mut Process, s: &mut CStream) {
        s.skip_space();
        if s.peek().is_none() {
            self.meet(p, false);
        }
    }

    /// A `%f` conversion: its value, or none (an input or matching failure).
    fn f(&mut self, p: &mut Process, s: &mut CStream) -> Option<f32> {
        if !self.skip(p, s) {
            return None;
        }
        let mut met = false;
        let (r, e) = scan_f(s, &mut met);
        if met {
            self.meet(p, false);
        }
        if let Some(e) = e {
            p.errno = e;
        }
        match r {
            Scan::Value(v) => Some(v),
            Scan::Matching => None,
        }
    }

    /// A `%d` conversion (a 64-bit `long` kept to its low 32 bits): its value, or none.
    fn d(&mut self, p: &mut Process, s: &mut CStream) -> Option<i32> {
        if !self.skip(p, s) {
            return None;
        }
        let mut met = false;
        let start = s.pos;
        if let Some(b'+' | b'-') = look(s, &mut met) {
            s.pos += 1;
        }
        let n = digits(s, false, &mut met);
        if met {
            self.meet(p, false);
        }
        if n == 0 {
            return None;
        }
        let item = &s.bytes[start..s.pos];
        let (v, _) = cscan::strtol10(item);
        if v == i64::MAX || v == i64::MIN {
            // beyond the long range: its limit and ERANGE
            let digits: String = item
                .iter()
                .filter(|c| c.is_ascii_digit())
                .map(|&c| c as char)
                .collect();
            let d = digits.trim_start_matches('0');
            let limit = if v == i64::MAX {
                "9223372036854775807"
            } else {
                "9223372036854775808"
            };
            if d.len() > 19 || (d.len() == 19 && d > limit) {
                p.errno = ERANGE;
            }
        }
        Some(v as i32)
    }
}

/// `sscanf(line, "%d %d %d")`'s values (as many as it assigned), `errno` updated (§18.1).
fn scan_ddd(p: &mut Process, line: &[u8]) -> Vec<i32> {
    let mut s = CStream::new(line.to_vec());
    let mut call = Call::new();
    let mut out = Vec::new();
    for _ in 0..3 {
        match call.d(p, &mut s) {
            Some(v) => out.push(v),
            None => break,
        }
    }
    out
}

impl Process {
    /// `MatrixReadTxt(name)` (§18.2).
    pub fn matrix_read_txt(&mut self, name: &[u8]) -> Result<Option<Matrix>, Stop> {
        let bytes = match std::fs::read(crate::files::path_of(name)) {
            Ok(b) => b,
            Err(e) => {
                self.errno = e.raw_os_error().unwrap_or(2);
                let mut m = b"MatrixReadTxt(".to_vec();
                m.extend_from_slice(name);
                m.extend_from_slice(b") - file open failed\n");
                self.error_printf(codes::NO_FILE, &m);
                return Ok(None);
            }
        };
        let mut s = CStream::new(bytes);
        let Some(first) = s.fgets(999) else {
            let mut m = b"MatrixReadTxt: could not read 1st line from ".to_vec();
            m.extend_from_slice(name);
            m.push(b'\n');
            self.error_printf(codes::BAD_PARM, &m);
            return Ok(None);
        };
        let cols = crate::strings::items_in_string(&first);
        if cols < 1 {
            return Ok(None);
        }
        let mut rows = 1;
        while s.fgets(999).is_some() {
            rows += 1;
        }
        let Some(mut m) = self.matrix_alloc(rows, cols, 1)? else {
            return Ok(None);
        };
        s.pos = 0;
        for r in 1..=rows {
            for c in 1..=cols {
                match Call::new().f(self, &mut s) {
                    Some(v) => m.set(r, c, v),
                    None => {
                        let msg = format!("MatrixReadTxT: could not scan value [{r}][{c}]\n");
                        self.error_printf(codes::BAD_PARM, msg.as_bytes());
                        return Ok(None);
                    }
                }
            }
        }
        Ok(Some(m))
    }

    /// `MatrixAsciiWriteInto(fp, m)`: the text it writes (§18.2).
    pub fn matrix_ascii_write(&mut self, m: &Matrix) -> Vec<u8> {
        let mut b = format!("{} {} {}\n", m.kind, m.rows, m.cols).into_bytes();
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                if m.kind == MATRIX_COMPLEX {
                    let (re, im) = m.pair(r, c);
                    b.extend_from_slice(&sprintf_f64(b"%+f", sse::widen(re)));
                    b.push(b' ');
                    b.extend_from_slice(&sprintf_f64(b"%+f", sse::widen(im)));
                    b.extend_from_slice(b"   ");
                } else {
                    b.extend_from_slice(&sprintf_f64(b"%+f", sse::widen(m.at(r, c))));
                    b.extend_from_slice(b"  ");
                }
            }
            b.push(b'\n');
        }
        b
    }

    /// The matrix a reader fills: a new one (`Ok(Some)`), the given one when it matches its
    /// header (`Ok(None)`), or what the reader returns after an error (`Err`).
    fn reader_matrix(
        &mut self,
        name: &str,
        kind: i32,
        rows: i32,
        cols: i32,
        given: Option<&Matrix>,
    ) -> Result<Result<Option<Matrix>, Returned<Matrix>>, Stop> {
        let Some(g) = given else {
            return match self.matrix_alloc(rows, cols, kind)? {
                Some(x) => Ok(Ok(Some(x))),
                None => {
                    let msg = format!("{name}: could not allocate matrix");
                    self.error_printf(codes::BAD_FILE, msg.as_bytes());
                    Ok(Err(Returned::Nothing))
                }
            };
        };
        if g.kind != kind || g.rows != rows || g.cols != cols {
            let msg = format!("{name}: specified matrix does not match file");
            self.error_printf(codes::BAD_FILE, msg.as_bytes());
            return Ok(Err(Returned::Output));
        }
        Ok(Ok(None))
    }

    /// `MatrixAsciiReadFrom(fp, m)` (§18.2).
    pub fn matrix_ascii_read_from(
        &mut self,
        s: &mut CStream,
        m: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let header = {
            let mut cur = std::io::Cursor::new(&s.bytes[s.pos..]);
            let line = crate::strings::fgetl(&mut cur, 200);
            s.pos += cur.position() as usize;
            line
        };
        let v = match header.as_deref() {
            Some(l) => scan_ddd(self, l),
            None => Vec::new(),
        };
        if v.len() < 3 {
            self.error_printf(
                codes::BAD_FILE,
                b"MatrixAsciiReadFrom: could not scanf parms",
            );
            return Ok(Returned::Nothing);
        }
        let (kind, rows, cols) = (v[0], v[1], v[2]);
        let mut fresh =
            match self.reader_matrix("MatrixAsciiReadFrom", kind, rows, cols, m.as_deref())? {
                Ok(f) => f,
                Err(r) => return Ok(r),
            };
        let target: &mut Matrix = match (fresh.as_mut(), m) {
            (Some(f), _) => f,
            (None, Some(g)) => g,
            (None, None) => return Ok(Returned::Nothing),
        };
        for r in 1..=rows {
            for c in 1..=cols {
                // One call: `%f` and white space (complex: `%f`, white space, `%f`, white
                // space); the white space before a conversion reads nothing itself.
                let mut call = Call::new();
                if kind == MATRIX_COMPLEX {
                    let Some(re) = call.f(self, s) else {
                        return Ok(self.scan_element_failed(r, c));
                    };
                    let Some(im) = call.f(self, s) else {
                        let p = target.complex_place(r, c);
                        target.write(p, re);
                        return Ok(self.scan_element_failed(r, c));
                    };
                    call.ws(self, s);
                    target.set_pair(r, c, re, im);
                } else {
                    let Some(x) = call.f(self, s) else {
                        return Ok(self.scan_element_failed(r, c));
                    };
                    call.ws(self, s);
                    target.set(r, c, x);
                }
            }
            // White space after each row: a call of its own.
            Call::new().ws(self, s);
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    fn scan_element_failed(&mut self, r: i32, c: i32) -> Returned<Matrix> {
        let msg = format!("MatrixAsciiReadFrom: could not scan element ({r}, {c})");
        self.error_printf(codes::BAD_FILE, msg.as_bytes());
        Returned::Nothing
    }

    /// `MatrixReadFrom(fp, m)`: the binary reader (§18.2; FS-49).
    pub fn matrix_read_from(
        &mut self,
        s: &mut CStream,
        m: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let reproduce = self.mode().reproduce();
        // FreeSurfer reads each number into one stack slot that it then byte-swaps in place: a
        // short read leaves the slot's other bytes as the previous number left them, swapped
        // (measured: `mfiles`); the slots start as zeros here (memory never written, §1)
        let mut short = false;
        let mut header = [0i32; 3];
        let mut slot4 = [0u8; 4];
        for h in &mut header {
            let b = s.read(4);
            short |= b.len() < 4;
            slot4[..b.len()].copy_from_slice(&b);
            *h = i32::from_be_bytes(slot4);
            slot4.reverse();
        }
        let [kind, rows, cols] = header;
        if short {
            if !reproduce {
                return Ok(self.ends_early());
            }
            self.note(
                &FS_49,
                "a binary matrix file that ends early: the missing bytes are the previous number's, swapped",
            );
        }
        let mut fresh =
            match self.reader_matrix("MatrixReadFrom", kind, rows, cols, m.as_deref())? {
                Ok(f) => f,
                Err(r) => return Ok(r),
            };
        let target: &mut Matrix = match (fresh.as_mut(), m) {
            (Some(f), _) => f,
            (None, Some(g)) => g,
            (None, None) => return Ok(Returned::Nothing),
        };
        let per = if kind == MATRIX_COMPLEX { 2 } else { 1 };
        let mut vals = Vec::new();
        let mut slot8 = [0u8; 8];
        for _ in 0..(i64::from(rows.max(0)) * i64::from(cols.max(0)) * per) {
            let b = s.read(8);
            if b.len() < 8 {
                self.error_printf(codes::BAD_FILE, b"freadDouble: fread failed");
                if !reproduce {
                    return Ok(self.ends_early());
                }
                self.note(
                    &FS_49,
                    "a binary matrix file that ends early: the missing bytes are the previous number's, swapped",
                );
            }
            slot8[..b.len()].copy_from_slice(&b);
            vals.push(sse::narrow(f64::from_be_bytes(slot8)));
            slot8.reverse();
        }
        let mut k = 0;
        for r in 1..=rows {
            for c in 1..=cols {
                if kind == MATRIX_COMPLEX {
                    target.set_pair(r, c, vals[k], vals[k + 1]);
                    k += 2;
                } else {
                    target.set(r, c, vals[k]);
                    k += 1;
                }
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    fn ends_early(&mut self) -> Returned<Matrix> {
        self.note(&FS_49, "a binary matrix file that ends early refused");
        self.error_printf(
            codes::BAD_FILE,
            b"MatrixReadFrom: the file ends before the matrix does",
        );
        Returned::Nothing
    }

    /// `MatrixWriteInto(fp, m)`: the bytes it writes (§18.2).
    pub fn matrix_write_into(&mut self, m: &Matrix) -> Vec<u8> {
        let mut b = Vec::new();
        for v in [m.kind, m.rows, m.cols] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                if m.kind == MATRIX_COMPLEX {
                    let (re, im) = m.pair(r, c);
                    b.extend_from_slice(&sse::widen(re).to_be_bytes());
                    b.extend_from_slice(&sse::widen(im).to_be_bytes());
                } else {
                    b.extend_from_slice(&sse::widen(m.at(r, c)).to_be_bytes());
                }
            }
        }
        b
    }

    /// `MatrixWriteTxt(name, m)`: 0, or -1 after its error when the file cannot be opened.
    pub fn matrix_write_txt(&mut self, name: &[u8], m: &Matrix) -> i32 {
        if !self.real_only("MatrixWriteTxt", m) {
            return codes::BAD_PARM;
        }
        let mut b = Vec::new();
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                b.extend_from_slice(&sprintf_f64(b"%+4.5f", sse::widen(m.at(r, c))));
                b.push(b' ');
            }
            b.push(b'\n');
        }
        match std::fs::write(crate::files::path_of(name), &b) {
            Ok(()) => 0,
            Err(e) => {
                self.errno = e.raw_os_error().unwrap_or(2);
                let mut msg = b"MatrixWriteTxt(".to_vec();
                msg.extend_from_slice(name);
                msg.extend_from_slice(b") - file open failed\n");
                self.error_printf(codes::NO_FILE, &msg);
                codes::NO_FILE
            }
        }
    }
}
