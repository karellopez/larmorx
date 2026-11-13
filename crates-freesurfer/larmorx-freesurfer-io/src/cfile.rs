// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! C's `FILE` as FreeSurfer's readers and writers use it, larmorx's own code from the C standard's
//! description of the stream functions (ISO C 7.21) and glibc 2.35's behaviour as measured by the
//! fs.io probe (`validation/probes/fs-io/`): `fopen`'s modes and `errno`, `fread` and `fwrite` of
//! members (a short read keeps the bytes it got), `getc`, `fgets`, the end-of-file and error flags
//! (`feof` is set by a read that finds no more bytes, never by one that ends exactly at the end),
//! `fseek`, `rewind` and `ftell` (measured by `validation/investigations/glibc-stdio-probe/`), and
//! `scanf`'s input ([`Stream`], one byte of lookahead).
//!
//! A stream is read or written, not both (the `+` modes open the file for both, and the stream
//! flushes its pending output before it reads); files or memory (for tests and in-process
//! programs).

use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

use larmorx_freesurfer_core::cscan::Stream;
use larmorx_freesurfer_core::log;

/// The size of the stream's buffer (glibc's `BUFSIZ`).
const BUFSIZ: usize = 8192;

enum Backing {
    File(std::fs::File),
    Mem(Cursor<Vec<u8>>),
}

impl Backing {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Backing::File(f) => f.read(b),
            Backing::Mem(c) => c.read(b),
        }
    }
    fn write_all(&mut self, b: &[u8]) -> std::io::Result<()> {
        match self {
            Backing::File(f) => f.write_all(b),
            Backing::Mem(c) => c.write_all(b),
        }
    }
    fn seek(&mut self, s: SeekFrom) -> std::io::Result<u64> {
        match self {
            Backing::File(f) => f.seek(s),
            Backing::Mem(c) => c.seek(s),
        }
    }
}

/// A C stream (`FILE *`).
pub struct CFile {
    f: Backing,
    readable: bool,
    writable: bool,
    /// Bytes read from the file and not yet given to the program: `rbuf[rpos..]`.
    rbuf: Vec<u8>,
    rpos: usize,
    /// Bytes written by the program and not yet to the file.
    wbuf: Vec<u8>,
    eof: bool,
    error: bool,
    /// The bytes given to the program so far (reading).
    taken: u64,
}

/// `fopen`'s mode: (read, write, append, truncate, exclusive), or `None` (`EINVAL`).
fn parse_mode(mode: &str) -> Option<(bool, bool, bool, bool, bool)> {
    let b = mode.as_bytes();
    let (&first, rest) = b.split_first()?;
    let (mut r, mut w, a) = match first {
        b'r' => (true, false, false),
        b'w' => (false, true, false),
        b'a' => (false, true, true),
        _ => return None,
    };
    let mut excl = false;
    // glibc reads at most six more characters of the mode.
    for &c in rest.iter().take(6) {
        match c {
            b'+' => {
                r = true;
                w = true;
            }
            b'x' => excl = true,
            _ => {}
        }
    }
    Some((r, w, a, first == b'w', excl))
}

impl CFile {
    /// `fopen(name, mode)`: the stream, or `errno` (`EINVAL` 22 for a mode that is not one,
    /// the operating system's error otherwise, as Linux numbers it). A relative name starts in
    /// `dir` (the program's working directory) when given. A directory opens for reading on
    /// every platform, as on Linux (its reads fail with `EISDIR`).
    pub fn open(dir: Option<&Path>, name: &str, mode: &str) -> Result<CFile, i32> {
        let Some((r, w, a, trunc, excl)) = parse_mode(mode) else {
            return Err(22);
        };
        if name.is_empty() {
            return Err(2);
        }
        let path = larmorx_freesurfer_core::utils::resolve(dir, name);
        let mut o = std::fs::OpenOptions::new();
        o.read(r).write(w && !a).append(a);
        if w {
            if excl {
                o.create_new(true);
            } else {
                o.create(true);
            }
        }
        if trunc {
            o.truncate(true);
        }
        match o.open(&path) {
            Ok(file) => Ok(CFile::new(Backing::File(file), r, w)),
            Err(e) => {
                #[cfg(not(unix))]
                if r && !w && path.is_dir() {
                    return Ok(CFile::from_bytes(Vec::new()));
                }
                Err(log::errno_of(&e).unwrap_or(5))
            }
        }
    }

    fn new(f: Backing, readable: bool, writable: bool) -> CFile {
        CFile {
            f,
            readable,
            writable,
            rbuf: Vec::new(),
            rpos: 0,
            wbuf: Vec::new(),
            eof: false,
            error: false,
            taken: 0,
        }
    }

    /// A stream that reads `bytes` (as `fmemopen(..., "r")`).
    pub fn from_bytes(bytes: Vec<u8>) -> CFile {
        CFile::new(Backing::Mem(Cursor::new(bytes)), true, false)
    }

    /// A stream that writes into memory ([`CFile::into_bytes`] gives what it holds).
    pub fn memory_writer() -> CFile {
        CFile::new(Backing::Mem(Cursor::new(Vec::new())), false, true)
    }

    /// What a memory stream holds (its pending output included); a file stream's pending output
    /// is written and nothing is returned.
    pub fn into_bytes(mut self) -> Vec<u8> {
        self.flush_output();
        let f = std::mem::replace(&mut self.f, Backing::Mem(Cursor::new(Vec::new())));
        match f {
            Backing::Mem(c) => c.into_inner(),
            Backing::File(_) => Vec::new(),
        }
    }

    fn flush_output(&mut self) -> bool {
        if self.wbuf.is_empty() {
            return true;
        }
        let pending = std::mem::take(&mut self.wbuf);
        if self.f.write_all(&pending).is_err() {
            self.error = true;
            return false;
        }
        true
    }

    /// Fills the read buffer when it is empty; `false` when the file has no more bytes (the
    /// end-of-file flag is then set) or a read failed (the error flag).
    fn fill(&mut self) -> bool {
        if self.rpos < self.rbuf.len() {
            return true;
        }
        if !self.readable {
            self.error = true;
            return false;
        }
        if !self.flush_output() {
            return false;
        }
        self.rbuf.resize(BUFSIZ, 0);
        self.rpos = 0;
        loop {
            match self.f.read(&mut self.rbuf) {
                Ok(0) => {
                    self.rbuf.clear();
                    self.eof = true;
                    return false;
                }
                Ok(n) => {
                    self.rbuf.truncate(n);
                    return true;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => {
                    self.rbuf.clear();
                    self.error = true;
                    return false;
                }
            }
        }
    }

    /// Reads up to `buf.len()` bytes into `buf`, as `fread` copies them: the number read (fewer at
    /// the end of the file, whose flag is then set; the bytes after them are left as they were).
    pub fn read_bytes(&mut self, buf: &mut [u8]) -> usize {
        let mut got = 0;
        while got < buf.len() {
            if !self.fill() {
                break;
            }
            let n = (self.rbuf.len() - self.rpos).min(buf.len() - got);
            buf[got..got + n].copy_from_slice(&self.rbuf[self.rpos..self.rpos + n]);
            self.rpos += n;
            self.taken += n as u64;
            got += n;
        }
        got
    }

    /// `fread(buf, size, nmemb, fp)`: the complete members read (`size * nmemb` bytes asked for,
    /// at most `buf.len()`).
    pub fn fread(&mut self, buf: &mut [u8], size: usize, nmemb: usize) -> usize {
        if size == 0 || nmemb == 0 {
            return 0;
        }
        let total = size.saturating_mul(nmemb).min(buf.len());
        self.read_bytes(&mut buf[..total]) / size
    }

    /// `fwrite(buf, size, nmemb, fp)`: the members written (0 on a stream not open for writing,
    /// whose error flag is then set).
    pub fn fwrite(&mut self, buf: &[u8], size: usize, nmemb: usize) -> usize {
        if size == 0 || nmemb == 0 {
            return 0;
        }
        if !self.writable {
            self.error = true;
            return 0;
        }
        let total = size.saturating_mul(nmemb).min(buf.len());
        // Unread input is dropped (the file position is where the program has read to).
        if self.rpos < self.rbuf.len() {
            let back = (self.rbuf.len() - self.rpos) as i64;
            let _ = self.f.seek(SeekFrom::Current(-back));
            self.rbuf.clear();
            self.rpos = 0;
        }
        self.wbuf.extend_from_slice(&buf[..total]);
        if self.wbuf.len() >= BUFSIZ && !self.flush_output() {
            return 0;
        }
        total / size
    }

    /// Writes all of `s` (`fputs`, `fprintf`); whether it was taken.
    pub fn put_bytes(&mut self, s: &[u8]) -> bool {
        s.is_empty() || self.fwrite(s, 1, s.len()) == s.len()
    }

    /// `getc(fp)`: the next byte, or -1 (`EOF`).
    pub fn getc(&mut self) -> i32 {
        if !self.fill() {
            return -1;
        }
        let c = self.rbuf[self.rpos];
        self.rpos += 1;
        self.taken += 1;
        i32::from(c)
    }

    /// `fgets(s, n, fp)`: a line with its newline, of at most `n - 1` bytes, or `None` at the end
    /// of the file before any byte (or for `n` < 1).
    pub fn fgets(&mut self, n: usize) -> Option<Vec<u8>> {
        if n == 0 {
            return None;
        }
        let mut out = Vec::new();
        while out.len() + 1 < n {
            let c = self.getc();
            if c < 0 {
                break;
            }
            out.push(c as u8);
            if c == i32::from(b'\n') {
                break;
            }
        }
        if out.is_empty() && n > 1 {
            None
        } else {
            Some(out)
        }
    }

    /// `ftell(fp)`: the program's position in the file (the bytes read but not yet given to the
    /// program and those written but not yet in the file counted as glibc does); -1 when the
    /// position cannot be had.
    pub fn tell(&mut self) -> i64 {
        match self.f.seek(SeekFrom::Current(0)) {
            Ok(p) => p as i64 - (self.rbuf.len() - self.rpos) as i64 + self.wbuf.len() as i64,
            Err(_) => -1,
        }
    }

    /// `fseek(fp, offset, whence)`: 0, or -1 with `errno` set (glibc 2.35's behaviour measured as a
    /// black box: `validation/investigations/glibc-stdio-probe/`). `whence` is 0 (`SEEK_SET`), 1
    /// (`SEEK_CUR`: from the program's position, the bytes read but not yet given to it excluded)
    /// or 2 (`SEEK_END`); any other value fails with `EINVAL` (22) before anything else, the
    /// pending output kept. Otherwise the pending output is written first; a position before the
    /// start fails with `EINVAL`, the position and the end-of-file flag kept. A position past the
    /// end is allowed (a read there finds the end; a write fills the gap with zero bytes). A
    /// successful seek drops the bytes read ahead and clears the end-of-file flag; it keeps the
    /// error flag and `errno`. Not modelled: the `errno` 21 a successful seek on a directory can
    /// leave (glibc reads a block there; every later read of a directory fails anyway).
    pub fn fseek(&mut self, errno: &mut i32, offset: i64, whence: i32) -> i32 {
        if !(0..=2).contains(&whence) {
            *errno = 22;
            return -1;
        }
        if !self.flush_output() {
            *errno = 5;
            return -1;
        }
        // after the output is written: an append stream's writes went to the end
        let here = if whence == 1 { self.tell() } else { 0 };
        let to = match whence {
            0 => (offset >= 0).then_some(SeekFrom::Start(offset as u64)),
            1 => here
                .checked_add(offset)
                .filter(|&p| here >= 0 && p >= 0)
                .map(|p| SeekFrom::Start(p as u64)),
            _ => Some(SeekFrom::End(offset)),
        };
        let Some(to) = to else {
            *errno = 22;
            return -1;
        };
        match self.f.seek(to) {
            Ok(_) => {
                self.rbuf.clear();
                self.rpos = 0;
                self.eof = false;
                0
            }
            Err(e) => {
                *errno = log::errno_of(&e).unwrap_or(22);
                -1
            }
        }
    }

    /// `rewind(fp)`: `fseek(fp, 0, SEEK_SET)` with its result dropped, then the error flag cleared
    /// (`errno` kept).
    pub fn rewind(&mut self) {
        let mut errno = 0;
        self.fseek(&mut errno, 0, 0);
        self.error = false;
    }

    /// How many bytes the program has read from the stream.
    pub fn taken(&self) -> u64 {
        self.taken
    }

    /// `feof(fp)`.
    pub fn feof(&self) -> bool {
        self.eof
    }

    /// `ferror(fp)`.
    pub fn ferror(&self) -> bool {
        self.error
    }

    /// `fclose(fp)`: 0, or -1 (`EOF`) when the pending output could not be written.
    pub fn close(mut self) -> i32 {
        let ok = self.flush_output();
        if let Backing::File(f) = &mut self.f {
            let _ = f.flush();
        }
        if ok { 0 } else { -1 }
    }
}

impl Stream for CFile {
    fn hit_end(&mut self) -> bool {
        self.eof && self.rpos >= self.rbuf.len()
    }
    fn peek(&mut self) -> Option<u8> {
        if !self.fill() {
            return None;
        }
        Some(self.rbuf[self.rpos])
    }
    fn bump(&mut self) {
        if self.rpos < self.rbuf.len() {
            self.rpos += 1;
            self.taken += 1;
        }
    }
}

impl Write for CFile {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if self.put_bytes(b) {
            Ok(b.len())
        } else {
            Err(std::io::Error::other("the stream is not writable"))
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if self.flush_output() {
            Ok(())
        } else {
            Err(std::io::Error::other(
                "the stream's output could not be written",
            ))
        }
    }
}

impl Drop for CFile {
    fn drop(&mut self) {
        self.flush_output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_of_file_is_set_by_a_read_past_the_end() {
        let mut f = CFile::from_bytes(vec![1, 2, 3, 4]);
        let mut b = [0u8; 4];
        assert_eq!(f.fread(&mut b, 4, 1), 1);
        assert!(!f.feof());
        let mut c = [9u8; 4];
        assert_eq!(f.fread(&mut c, 4, 1), 0);
        assert!(f.feof());
        assert_eq!(c, [9; 4]);
        let mut f = CFile::from_bytes(vec![1, 2]);
        let mut c = [9u8; 4];
        assert_eq!(f.fread(&mut c, 4, 1), 0);
        assert_eq!(c, [1, 2, 9, 9]);
    }

    #[test]
    fn lines() {
        let mut f = CFile::from_bytes(b"ab\ncdef".to_vec());
        assert_eq!(f.fgets(10), Some(b"ab\n".to_vec()));
        assert_eq!(f.fgets(3), Some(b"cd".to_vec()));
        assert_eq!(f.fgets(10), Some(b"ef".to_vec()));
        assert!(f.feof());
        assert_eq!(f.fgets(10), None);
    }

    /// `glibc-stdio-probe/fseek_probe.c`'s measurements (glibc 2.35; `errno` 5 before each call).
    #[test]
    fn seeks_as_glibc() {
        let mut f = CFile::from_bytes(b"abcdef".to_vec());
        let mut b = [0u8; 3];
        f.fread(&mut b, 1, 3);
        let mut e = 5;
        assert_eq!((f.fseek(&mut e, 2, 0), e, f.tell()), (0, 5, 2));
        assert_eq!(f.getc(), i32::from(b'c'));
        assert_eq!((f.fseek(&mut e, 1, 1), f.tell()), (0, 4));
        assert_eq!(f.getc(), i32::from(b'e'));
        assert_eq!((f.fseek(&mut e, -2, 2), f.tell()), (0, 4));
        assert_eq!(f.getc(), i32::from(b'e'));
        assert_eq!((f.fseek(&mut e, 100, 0), f.tell()), (0, 100));
        assert_eq!((f.getc(), f.tell(), f.feof()), (-1, 100, true));
        assert_eq!((f.fseek(&mut e, 0, 0), f.feof()), (0, false));
        assert_eq!(f.getc(), i32::from(b'a'));
        while f.getc() >= 0 {}
        for (off, wh) in [(-1, 0), (-100, 1), (0, 3), (-7, 2)] {
            let mut e = 5;
            assert_eq!(
                (f.fseek(&mut e, off, wh), e, f.tell(), f.feof()),
                (-1, 22, 6, true),
                "{off} {wh}"
            );
        }
        assert_eq!((f.fseek(&mut e, 0, 2), f.tell(), f.feof()), (0, 6, false));
        assert_eq!((f.getc(), f.feof()), (-1, true));
        assert_eq!((f.fseek(&mut e, 3, 2), f.tell()), (0, 9));
        f.error = true;
        assert_eq!((f.fseek(&mut e, 1, 0), f.ferror()), (0, true));
        assert_eq!(f.getc(), i32::from(b'b'));
        f.rewind();
        assert_eq!((f.tell(), f.feof(), f.ferror(), e), (0, false, false, 5));
        assert_eq!(f.getc(), i32::from(b'a'));
        let mut f = CFile::from_bytes(Vec::new());
        assert_eq!((f.fseek(&mut e, 5, 0), f.getc(), f.feof()), (0, -1, true));
        assert_eq!((f.fseek(&mut e, -5, 1), f.tell(), f.getc()), (0, 0, -1));
    }

    #[test]
    fn seeks_while_writing() {
        let mut e = 5;
        let mut w = CFile::memory_writer();
        w.fwrite(b"abc", 1, 3);
        assert_eq!((w.fseek(&mut e, 5, 0), w.tell()), (0, 5));
        w.fwrite(b"x", 1, 1);
        assert_eq!(w.into_bytes(), b"abc\0\0x".to_vec());
        let mut w = CFile::memory_writer();
        w.fwrite(b"abcdef", 1, 6);
        w.fseek(&mut e, 2, 0);
        w.fwrite(b"X", 1, 1);
        assert_eq!((w.fseek(&mut e, 0, 1), w.tell()), (0, 3));
        assert_eq!(w.into_bytes(), b"abXdef".to_vec());
        // an invalid `whence` fails before the pending output is written; a position before the
        // start after it
        let mut w = CFile::memory_writer();
        w.fwrite(b"abc", 1, 3);
        assert_eq!((w.fseek(&mut e, 0, 7), e), (-1, 22));
        assert!(w.wbuf == b"abc");
        assert_eq!(w.fseek(&mut e, -1, 0), -1);
        assert!(w.wbuf.is_empty());
        assert_eq!(w.tell(), 3);
    }

    /// The file cases of `fseek_probe.c`: a gap after the end, a read-write stream and an append.
    #[test]
    fn seeks_on_files() {
        let dir = tempfile::Builder::new()
            .prefix("cfile ~")
            .tempdir()
            .unwrap();
        let d = Some(dir.path());
        let mut e = 5;
        let mut w = CFile::open(d, "w", "w").unwrap();
        w.fwrite(b"abc", 1, 3);
        w.fseek(&mut e, 5, 0);
        w.fwrite(b"x", 1, 1);
        assert_eq!(std::fs::metadata(dir.path().join("w")).unwrap().len(), 3);
        w.close();
        assert_eq!(std::fs::read(dir.path().join("w")).unwrap(), b"abc\0\0x");
        std::fs::write(dir.path().join("rw"), b"abcdef").unwrap();
        let mut f = CFile::open(d, "rw", "r+").unwrap();
        let mut b = [0u8; 2];
        f.fread(&mut b, 1, 2);
        assert_eq!((f.fseek(&mut e, 0, 1), f.tell()), (0, 2));
        f.fwrite(b"Z", 1, 1);
        assert_eq!((f.fseek(&mut e, 0, 1), f.tell()), (0, 3));
        assert_eq!(f.getc(), i32::from(b'd'));
        f.fseek(&mut e, 0, 0);
        let mut all = [0u8; 10];
        assert_eq!(f.fread(&mut all, 1, 10), 6);
        assert_eq!(&all[..6], b"abZdef");
        assert!(f.feof());
        f.close();
        std::fs::write(dir.path().join("ap"), b"abc").unwrap();
        let mut a = CFile::open(d, "ap", "a").unwrap();
        assert_eq!((a.fseek(&mut e, 0, 0), a.tell()), (0, 0));
        a.fwrite(b"Q", 1, 1);
        assert_eq!((a.fseek(&mut e, 0, 1), a.tell()), (0, 4));
        a.close();
        assert_eq!(std::fs::read(dir.path().join("ap")).unwrap(), b"abcQ");
        assert_eq!(e, 5);
    }

    #[test]
    fn modes_and_writes() {
        assert_eq!(CFile::open(None, "x", "q").err(), Some(22));
        assert_eq!(CFile::open(None, "", "r").err(), Some(2));
        let mut w = CFile::memory_writer();
        assert_eq!(w.fwrite(b"abcd", 2, 2), 2);
        assert_eq!(w.getc(), -1);
        assert!(w.ferror());
        assert_eq!(w.into_bytes(), b"abcd".to_vec());
    }
}
