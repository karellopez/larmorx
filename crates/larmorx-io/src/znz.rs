// SPDX-License-Identifier: Apache-2.0 AND Zlib
//! znzlib (nifticlib 2.0.0, Mark Jenkinson's "zipped or non-zipped" files, public domain): one
//! handle for a plain file (C's stdio) or a gzip file (zlib's `gz*` functions), as FreeSurfer's MGZ
//! and NIfTI code reads and writes through it.
//!
//! The gzip side is zlib 1.2.11's file layer (`gzlib.c`, `gzread.c`, `gzwrite.c`; zlib's licence,
//! the version Ubuntu 22.04 ships and FreeSurfer's official binaries load), translated with its
//! buffers (8 KiB of input, 16 KiB of output) and states (`LOOK`, `COPY`, `GZIP`), so that reads,
//! seeks, `gztell`, `gzeof` and the end of corrupt or truncated data come at the same calls as in
//! zlib; inflate and deflate themselves are zlib-rs's (through flate2). The decompressed bytes are
//! zlib's; the compressed bytes a writer produces are zlib-rs's at the same level (a deflate that
//! gives zlib's own bytes is FreeSurfer's item P-17). The plain side is C's stdio, by its
//! description (ISO C 7.21) and glibc's behaviour.

use std::io::{Read, Seek, SeekFrom, Write};

use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status};
use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{self as d, Notices, UpstreamDefects};

/// `SEEK_SET`, `SEEK_CUR`, `SEEK_END` (C's values).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Whence {
    Set = 0,
    Cur = 1,
    End = 2,
}

/// The size of one call's chunk (`ZNZ_MAX_BLOCK_SIZE`, 2^30 bytes).
const ZNZ_MAX_BLOCK_SIZE: usize = 1 << 30;
/// zlib's `GZBUFSIZE`.
const GZBUFSIZE: usize = 8192;
/// The scratch buffer for inflating into a caller's buffer.
const SCRATCH: usize = 1 << 16;

// zlib's return codes, as `gz_error` keeps them.
const Z_OK: i32 = 0;
const Z_ERRNO: i32 = -1;
const Z_STREAM_ERROR: i32 = -2;
const Z_DATA_ERROR: i32 = -3;
const Z_BUF_ERROR: i32 = -5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum How {
    Look,
    Copy,
    Gzip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GzMode {
    Read,
    Write,
}

/// zlib 1.2.11's `gzFile` (`gz_state`, the parts this layer uses): `gzopen`, `gzread`,
/// `gzwrite`, `gzseek`, `gztell`, `gzeof`, `gzgetc`, `gzputc`, `gzflush`, `gzrewind`, `gzclose`.
pub struct GzFile {
    file: std::fs::File,
    mode: GzMode,
    level: i32,
    direct: bool,
    want: usize,
    size: usize,
    start: u64,
    // reading
    input: Vec<u8>,
    next_in: usize,
    avail_in: usize,
    out: Vec<u8>,
    x_next: usize,
    x_have: usize,
    eof: bool,
    past: bool,
    how: How,
    inflate: Option<Decompress>,
    /// Inflate's room for a caller's buffer (at most [`SCRATCH`] bytes).
    scratch: Vec<u8>,
    // writing
    deflate: Option<Compress>,
    wbuf: Vec<u8>,
    // both
    x_pos: i64,
    seek: bool,
    skip: i64,
    err: i32,
    /// FS-78's notice, when it changed the file.
    notices: Notices,
}

/// A stdio stream on a plain file (`fopen`, `fread`, `fwrite`, `fseek`, `ftell`, `feof`).
pub struct StdioFile {
    file: std::fs::File,
    readable: bool,
    writable: bool,
    append: bool,
    eof: bool,
    error: bool,
}

enum Inner {
    Plain(StdioFile),
    Gz(Box<GzFile>),
}

/// A znzlib file (`znzFile`), with D13's mode for nifticlib's defects (FS-76, FS-77) and the
/// notices they gave.
pub struct ZnzFile {
    inner: Inner,
    defects: Notices,
}

fn to_path(name: &[u8]) -> std::path::PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        std::path::PathBuf::from(std::ffi::OsStr::from_bytes(name))
    }
    #[cfg(not(unix))]
    {
        std::path::PathBuf::from(String::from_utf8_lossy(name).into_owned())
    }
}

/// `fopen`'s mode: read, write, append, update, exclusive (glibc's `x`); `None`: not a mode
/// (`EINVAL`).
fn stdio_mode(mode: &[u8]) -> Option<(bool, bool, bool, bool, bool)> {
    let (&first, rest) = mode.split_first()?;
    let (mut r, mut w, a) = match first {
        b'r' => (true, false, false),
        b'w' => (false, true, false),
        b'a' => (false, true, true),
        _ => return None,
    };
    let trunc = first == b'w';
    let mut excl = false;
    // glibc reads at most six more characters of the mode.
    for &c in rest.iter().take(6) {
        match c {
            b'+' => {
                r = true;
                w = true;
            }
            b'x' => excl = true,
            b'b' | b'm' | b'c' | b'e' => {}
            _ => {}
        }
    }
    Some((r, w, a, trunc, excl))
}

impl ZnzFile {
    /// `znzopen(path, mode, use_compression)`: `gzopen` when compressing, else `fopen`; `None`
    /// where those fail.
    pub fn open(path: &[u8], mode: &[u8], use_compression: bool) -> Option<ZnzFile> {
        if use_compression {
            GzFile::open(path, mode).map(|g| ZnzFile {
                inner: Inner::Gz(Box::new(g)),
                defects: Notices::default(),
            })
        } else {
            let (r, w, a, trunc, excl) = stdio_mode(mode)?;
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
            let mut file = o.open(to_path(path)).ok()?;
            if a {
                // glibc's append stream starts at the end (its ftell says so before any write).
                let _ = file.seek(SeekFrom::End(0));
            }
            Some(ZnzFile {
                inner: Inner::Plain(StdioFile {
                    file,
                    readable: r,
                    writable: w,
                    append: a,
                    eof: false,
                    error: false,
                }),
                defects: Notices::default(),
            })
        }
    }

    /// D13's mode for this file (`Fix` by default).
    pub fn with_upstream_defects(mut self, mode: UpstreamDefects) -> ZnzFile {
        self.defects = Notices::new(mode);
        self
    }

    /// The notices of the defects that changed a result so far (nifticlib's and zlib's).
    pub fn notices(&self) -> Notices {
        let mut n = self.defects.clone();
        if let Inner::Gz(g) = &self.inner {
            n.extend(g.notices.as_slice().iter().cloned());
        }
        n
    }

    /// The gzip file underneath, when compressed.
    pub fn gz(&mut self) -> Option<&mut GzFile> {
        match &mut self.inner {
            Inner::Gz(g) => Some(g),
            Inner::Plain(_) => None,
        }
    }

    /// The stdio stream underneath, when plain.
    pub fn stdio(&mut self) -> Option<&mut StdioFile> {
        match &mut self.inner {
            Inner::Plain(p) => Some(p),
            Inner::Gz(_) => None,
        }
    }

    /// nifticlib's count of the members read or written by a compressed file: `nmemb -
    /// remain / size` counts a member it only partly read as read (FS-77); corrected, the
    /// complete members only.
    fn members(
        &mut self,
        nmemb: usize,
        total: usize,
        remain: usize,
        size: usize,
        what: &str,
    ) -> i64 {
        let original = nmemb - remain / size;
        let complete = (total - remain) / size;
        if original != complete {
            let n = format!(
                "a {what} of {total} bytes ended {remain} bytes short, inside a member of {size} bytes"
            );
            self.defects.add(&d::FS_77, n);
            if self.defects.reproduce() {
                return original as i64;
            }
        }
        complete as i64
    }

    /// Whether the file is read through zlib.
    pub fn compressed(&self) -> bool {
        matches!(self.inner, Inner::Gz(_))
    }

    /// `znzread(buf, size, nmemb)`: the number of members read; for a compressed file, zlib's
    /// `-1` on an error (as C's `size_t` would carry it, here a negative number), and the warning
    /// `** znzread: read short by N bytes` when a member is cut. A member size of 0 on a compressed
    /// file divides by zero in nifticlib (FS-76); larmorx returns 0.
    pub fn read(
        &mut self,
        buf: &mut [u8],
        size: usize,
        nmemb: usize,
        streams: &mut dyn Streams,
    ) -> i64 {
        let total = size.saturating_mul(nmemb).min(buf.len());
        match &mut self.inner {
            Inner::Plain(p) => p.fread(&mut buf[..total], size, nmemb) as i64,
            Inner::Gz(g) => {
                if size == 0 {
                    self.defects.add(
                        &d::FS_76,
                        "a read with a member size of 0 (nifticlib divides by zero)",
                    );
                    return 0;
                }
                let mut remain = total;
                let mut at = 0usize;
                while remain > 0 {
                    let n2read = remain.min(ZNZ_MAX_BLOCK_SIZE);
                    let nread = g.gzread(&mut buf[at..at + n2read]);
                    if nread < 0 {
                        return nread as i64;
                    }
                    let nread = nread as usize;
                    remain -= nread;
                    at += nread;
                    if nread < n2read {
                        break;
                    }
                }
                if remain > 0 && remain < size {
                    streams.err(&format!(
                        "** znzread: read short by {} bytes\n",
                        remain as u32
                    ));
                }
                self.members(nmemb, total, remain, size, "read")
            }
        }
    }

    /// `znzwrite(buf, size, nmemb)`: the number of members written (and, compressed, the warning
    /// `** znzwrite: write short by N bytes` when a member is cut).
    pub fn write(
        &mut self,
        buf: &[u8],
        size: usize,
        nmemb: usize,
        streams: &mut dyn Streams,
    ) -> i64 {
        let total = size.saturating_mul(nmemb).min(buf.len());
        match &mut self.inner {
            Inner::Plain(p) => p.fwrite(&buf[..total], size, nmemb) as i64,
            Inner::Gz(g) => {
                if size == 0 {
                    self.defects.add(
                        &d::FS_76,
                        "a write with a member size of 0 (nifticlib divides by zero)",
                    );
                    return 0;
                }
                let mut remain = total;
                let mut at = 0usize;
                while remain > 0 {
                    let n2write = remain.min(ZNZ_MAX_BLOCK_SIZE);
                    let nwritten = g.gzwrite(&buf[at..at + n2write]);
                    if nwritten < 0 {
                        return nwritten as i64;
                    }
                    let nwritten = nwritten as usize;
                    remain -= nwritten;
                    at += nwritten;
                    if nwritten < n2write {
                        break;
                    }
                }
                if remain > 0 && remain < size {
                    streams.err(&format!(
                        "** znzwrite: write short by {} bytes\n",
                        remain as u32
                    ));
                }
                self.members(nmemb, total, remain, size, "write")
            }
        }
    }

    /// `znzseek`: `fseek`'s 0 or -1 for a plain file, `gzseek`'s new position (or -1) for a
    /// compressed one.
    pub fn seek(&mut self, offset: i64, whence: Whence) -> i64 {
        match &mut self.inner {
            Inner::Plain(p) => p.fseek(offset, whence),
            Inner::Gz(g) => g.gzseek(offset, whence),
        }
    }

    /// `znzrewind`.
    pub fn rewind(&mut self) -> i32 {
        match &mut self.inner {
            Inner::Plain(p) => {
                p.fseek(0, Whence::Set);
                p.error = false;
                0
            }
            Inner::Gz(g) => g.gzseek(0, Whence::Set) as i32,
        }
    }

    /// `znztell`.
    pub fn tell(&mut self) -> i64 {
        match &mut self.inner {
            Inner::Plain(p) => p.file.stream_position().map_or(-1, |v| v as i64),
            Inner::Gz(g) => g.gztell(),
        }
    }

    /// `znzeof`.
    pub fn eof(&self) -> bool {
        match &self.inner {
            Inner::Plain(p) => p.eof,
            Inner::Gz(g) => g.gzeof(),
        }
    }

    /// `znzgetc`: the next byte, or -1.
    pub fn getc(&mut self) -> i32 {
        match &mut self.inner {
            Inner::Plain(p) => {
                let mut b = [0u8];
                if p.fread(&mut b, 1, 1) == 1 {
                    b[0] as i32
                } else {
                    -1
                }
            }
            Inner::Gz(g) => g.gzgetc(),
        }
    }

    /// `znzputc`: the byte written, or -1.
    pub fn putc(&mut self, c: i32) -> i32 {
        let b = [c as u8];
        match &mut self.inner {
            Inner::Plain(p) => {
                if p.fwrite(&b, 1, 1) == 1 {
                    c & 0xff
                } else {
                    -1
                }
            }
            Inner::Gz(g) => g.gzputc(c),
        }
    }

    /// `znzputs`: a non-negative number, or -1 (`fputs` returns 1 in glibc, `gzputs` the
    /// length).
    pub fn puts(&mut self, s: &[u8]) -> i32 {
        match &mut self.inner {
            Inner::Plain(p) => {
                if p.fwrite(s, 1, s.len()) == s.len() {
                    1
                } else {
                    -1
                }
            }
            Inner::Gz(g) => {
                let n = g.gzwrite(s);
                if n < 0 || (n as usize) < s.len() && !s.is_empty() {
                    -1
                } else {
                    n
                }
            }
        }
    }

    /// `znzgets(size)`: a line (with its newline) of at most `size - 1` bytes, or `None`.
    pub fn gets(&mut self, size: usize) -> Option<Vec<u8>> {
        if size == 0 {
            return None;
        }
        let mut out = Vec::new();
        while out.len() + 1 < size {
            let c = self.getc();
            if c < 0 {
                break;
            }
            out.push(c as u8);
            if c == b'\n' as i32 {
                break;
            }
        }
        if out.is_empty() && size > 1 {
            None
        } else {
            Some(out)
        }
    }

    /// `znzflush`: `fflush`, or `gzflush(Z_SYNC_FLUSH)`.
    pub fn flush(&mut self) -> i32 {
        match &mut self.inner {
            Inner::Plain(p) => {
                if p.file.flush().is_ok() {
                    0
                } else {
                    -1
                }
            }
            Inner::Gz(g) => g.gzflush(),
        }
    }

    /// `znzclose` (`Xznzclose`): `fclose`'s or `gzclose`'s value.
    pub fn close(self) -> i32 {
        self.close_with_notices().0
    }

    /// [`ZnzFile::close`], and the notices of the whole file's life (closing a compressed file
    /// writes a pending gap: FS-78).
    pub fn close_with_notices(self) -> (i32, Notices) {
        let mut notices = self.defects;
        match self.inner {
            Inner::Plain(mut p) => (if p.file.flush().is_ok() { 0 } else { -1 }, notices),
            Inner::Gz(g) => {
                let (r, n) = g.gzclose_with_notices();
                notices.extend(n.into_vec());
                (r, notices)
            }
        }
    }
}

impl StdioFile {
    /// `fread`: the members read.
    pub fn fread(&mut self, buf: &mut [u8], size: usize, nmemb: usize) -> usize {
        if size == 0 || nmemb == 0 {
            return 0;
        }
        if !self.readable {
            self.error = true;
            return 0;
        }
        let mut got = 0;
        while got < buf.len() {
            match self.file.read(&mut buf[got..]) {
                Ok(0) => {
                    self.eof = true;
                    break;
                }
                Ok(n) => got += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => {
                    self.error = true;
                    break;
                }
            }
        }
        got / size
    }

    /// `fwrite`: the members written.
    pub fn fwrite(&mut self, buf: &[u8], size: usize, nmemb: usize) -> usize {
        if size == 0 || nmemb == 0 {
            return 0;
        }
        if !self.writable {
            self.error = true;
            return 0;
        }
        let _ = self.append;
        match self.file.write_all(buf) {
            Ok(()) => nmemb,
            Err(_) => {
                self.error = true;
                0
            }
        }
    }

    /// `fseek`: 0 or -1.
    pub fn fseek(&mut self, offset: i64, whence: Whence) -> i64 {
        let target = match whence {
            Whence::Set => SeekFrom::Start(offset.max(0) as u64),
            Whence::Cur => SeekFrom::Current(offset),
            Whence::End => SeekFrom::End(offset),
        };
        if whence == Whence::Set && offset < 0 {
            return -1;
        }
        match self.file.seek(target) {
            Ok(_) => {
                self.eof = false;
                0
            }
            Err(_) => -1,
        }
    }
}

impl GzFile {
    /// `gz_open`: the mode's letters (`r`, `w`, `a`, a level digit, `T`; `+` refused), the file
    /// opened (read only; or created, truncated or appended), the state reset.
    pub fn open(path: &[u8], mode: &[u8]) -> Option<GzFile> {
        let mut m = None;
        let mut level = -1;
        let mut direct = false;
        let mut exclusive = false;
        let mut append = false;
        for &c in mode {
            match c {
                b'0'..=b'9' => level = (c - b'0') as i32,
                b'r' => m = Some(GzMode::Read),
                b'w' => {
                    m = Some(GzMode::Write);
                    append = false;
                }
                b'a' => {
                    m = Some(GzMode::Write);
                    append = true;
                }
                b'+' => return None,
                b'x' => exclusive = true,
                b'T' => direct = true,
                _ => {}
            }
        }
        let mode = m?;
        if mode == GzMode::Read {
            if direct {
                return None;
            }
            direct = true; // for an empty file
        }
        let mut o = std::fs::OpenOptions::new();
        if mode == GzMode::Read {
            o.read(true);
        } else {
            o.write(true);
            if exclusive {
                o.create_new(true);
            } else {
                o.create(true);
            }
            if append {
                o.append(true);
            } else {
                o.truncate(true);
            }
        }
        let mut file = o.open(to_path(path)).ok()?;
        let start = if mode == GzMode::Read {
            file.stream_position().unwrap_or(0)
        } else {
            0
        };
        let mut g = GzFile {
            file,
            mode,
            level,
            direct,
            want: GZBUFSIZE,
            size: 0,
            start,
            input: Vec::new(),
            next_in: 0,
            avail_in: 0,
            out: Vec::new(),
            x_next: 0,
            x_have: 0,
            eof: false,
            past: false,
            how: How::Look,
            inflate: None,
            scratch: Vec::new(),
            deflate: None,
            wbuf: Vec::new(),
            x_pos: 0,
            seek: false,
            skip: 0,
            err: Z_OK,
            notices: Notices::default(),
        };
        g.reset();
        Some(g)
    }

    /// `gz_reset`.
    fn reset(&mut self) {
        self.x_have = 0;
        if self.mode == GzMode::Read {
            self.eof = false;
            self.past = false;
            self.how = How::Look;
        }
        self.seek = false;
        self.error(Z_OK);
        self.x_pos = 0;
        self.avail_in = 0;
    }

    /// `gz_error` (the message is not kept: znzlib never asks for it).
    fn error(&mut self, err: i32) {
        if err != Z_OK && err != Z_BUF_ERROR {
            self.x_have = 0;
        }
        self.err = err;
    }

    /// `gz_load`: `read()` into `buf` until it is full or the file ends (`eof`).
    fn load(&mut self, which: Target, len: usize) -> Result<usize, ()> {
        let mut have = 0;
        let mut last: std::io::Result<usize> = Ok(1);
        while have < len {
            let r = {
                let buf: &mut [u8] = match which {
                    Target::In(at) => &mut self.input[at + have..at + len],
                    Target::Out => &mut self.out[have..len],
                };
                self.file.read(buf)
            };
            match r {
                Ok(0) => {
                    last = Ok(0);
                    break;
                }
                Ok(n) => have += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => {
                    last = Err(e);
                    break;
                }
            }
        }
        match last {
            Err(_) => {
                self.error(Z_ERRNO);
                Err(())
            }
            Ok(0) => {
                self.eof = true;
                Ok(have)
            }
            Ok(_) => Ok(have),
        }
    }

    /// `gz_avail`.
    fn avail(&mut self) -> Result<(), ()> {
        if self.err != Z_OK && self.err != Z_BUF_ERROR {
            return Err(());
        }
        if !self.eof {
            if self.avail_in > 0 {
                self.input
                    .copy_within(self.next_in..self.next_in + self.avail_in, 0);
            }
            let got = self.load(Target::In(self.avail_in), self.size - self.avail_in)?;
            self.avail_in += got;
            self.next_in = 0;
        }
        Ok(())
    }

    /// `gz_look`.
    fn look(&mut self) -> Result<(), ()> {
        if self.size == 0 {
            self.input = vec![0; self.want];
            self.out = vec![0; self.want << 1];
            self.size = self.want;
            self.inflate = Some(Decompress::new_gzip(15));
        }
        if self.avail_in < 2 {
            self.avail()?;
            if self.avail_in == 0 {
                return Ok(());
            }
        }
        if self.avail_in > 1
            && self.input[self.next_in] == 31
            && self.input[self.next_in + 1] == 139
        {
            // inflateReset
            self.inflate = Some(Decompress::new_gzip(15));
            self.how = How::Gzip;
            self.direct = false;
            return Ok(());
        }
        if !self.direct {
            // trailing garbage after gzip data: ignored
            self.avail_in = 0;
            self.eof = true;
            self.x_have = 0;
            return Ok(());
        }
        self.x_next = 0;
        if self.avail_in > 0 {
            let (a, n) = (self.next_in, self.avail_in);
            self.out[..n].copy_from_slice(&self.input[a..a + n]);
            self.x_have = n;
            self.avail_in = 0;
        }
        self.how = How::Copy;
        self.direct = true;
        Ok(())
    }

    /// `gz_decomp` into `dest` (the output buffer, or a caller's): the bytes produced.
    fn decomp(&mut self, dest: Dest, avail_out: usize, user: &mut [u8]) -> Result<usize, ()> {
        let mut produced = 0usize;
        let mut ended = false;
        loop {
            if self.avail_in == 0 {
                self.avail()?;
            }
            if self.avail_in == 0 {
                self.error(Z_BUF_ERROR);
                break;
            }
            let inf = self.inflate.as_mut().expect("inflate state");
            let (ti, to) = (inf.total_in(), inf.total_out());
            let input = &self.input[self.next_in..self.next_in + self.avail_in];
            let r = match dest {
                Dest::Out => inf.decompress(
                    input,
                    &mut self.out[produced..avail_out],
                    FlushDecompress::None,
                ),
                Dest::User => {
                    // zlib writes nothing past what it produces; zlib-rs may use the whole
                    // output slice, so a caller's buffer gets the produced bytes only (through
                    // a bounded scratch buffer: the same bytes as one call with all the room).
                    let room = (avail_out - produced).min(SCRATCH);
                    if self.scratch.len() < room {
                        self.scratch.resize(room, 0);
                    }
                    let r = inf.decompress(input, &mut self.scratch[..room], FlushDecompress::None);
                    let made = (inf.total_out() - to) as usize;
                    user[produced..produced + made].copy_from_slice(&self.scratch[..made]);
                    r
                }
            };
            let used = (inf.total_in() - ti) as usize;
            let made = (inf.total_out() - to) as usize;
            self.next_in += used;
            self.avail_in -= used;
            produced += made;
            match r {
                Err(_) => {
                    self.error(Z_DATA_ERROR);
                    return Err(());
                }
                Ok(Status::StreamEnd) => {
                    ended = true;
                    break;
                }
                Ok(_) => {}
            }
            if produced == avail_out {
                break;
            }
        }
        if ended {
            self.how = How::Look;
        }
        Ok(produced)
    }

    /// `gz_fetch`.
    fn fetch(&mut self) -> Result<(), ()> {
        loop {
            match self.how {
                How::Look => {
                    self.look()?;
                    if self.how == How::Look {
                        return Ok(());
                    }
                }
                How::Copy => {
                    let len = self.size << 1;
                    self.x_have = self.load(Target::Out, len)?;
                    self.x_next = 0;
                    return Ok(());
                }
                How::Gzip => {
                    let n = self.decomp(Dest::Out, self.size << 1, &mut [])?;
                    self.x_have = n;
                    self.x_next = 0;
                }
            }
            if !(self.x_have == 0 && (!self.eof || self.avail_in > 0)) {
                return Ok(());
            }
        }
    }

    /// `gz_skip`.
    fn skip_bytes(&mut self, mut len: i64) -> Result<(), ()> {
        while len > 0 {
            if self.x_have > 0 {
                let n = (self.x_have as i64).min(len) as usize;
                self.x_have -= n;
                self.x_next += n;
                self.x_pos += n as i64;
                len -= n as i64;
            } else if self.eof && self.avail_in == 0 {
                break;
            } else {
                self.fetch()?;
            }
        }
        Ok(())
    }

    /// `gz_read`: the bytes copied (0 at the end or after an error).
    fn read_into(&mut self, buf: &mut [u8]) -> usize {
        let mut len = buf.len();
        if len == 0 {
            return 0;
        }
        if self.seek {
            self.seek = false;
            if self.skip_bytes(self.skip).is_err() {
                return 0;
            }
        }
        let mut got = 0usize;
        loop {
            let mut n = len;
            if self.x_have > 0 {
                n = n.min(self.x_have);
                buf[got..got + n].copy_from_slice(&self.out[self.x_next..self.x_next + n]);
                self.x_next += n;
                self.x_have -= n;
            } else if self.eof && self.avail_in == 0 {
                self.past = true;
                break;
            } else if self.how == How::Look || n < (self.size << 1) {
                if self.fetch().is_err() {
                    return 0;
                }
                continue;
            } else if self.how == How::Copy {
                let mut have = 0;
                // gz_load into the caller's buffer.
                let mut last = Ok(1usize);
                while have < n {
                    match self.file.read(&mut buf[got + have..got + n]) {
                        Ok(0) => {
                            last = Ok(0);
                            break;
                        }
                        Ok(k) => have += k,
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(e) => {
                            last = Err(e);
                            break;
                        }
                    }
                }
                match last {
                    Err(_) => {
                        self.error(Z_ERRNO);
                        return 0;
                    }
                    Ok(0) => self.eof = true,
                    Ok(_) => {}
                }
                n = have;
            } else {
                match self.decomp(Dest::User, n, &mut buf[got..got + n]) {
                    Ok(k) => n = k,
                    Err(()) => return 0,
                }
                self.x_have = 0;
            }
            len -= n;
            got += n;
            self.x_pos += n as i64;
            if len == 0 {
                break;
            }
        }
        got
    }

    /// `gzread`.
    pub fn gzread(&mut self, buf: &mut [u8]) -> i32 {
        if self.mode != GzMode::Read || (self.err != Z_OK && self.err != Z_BUF_ERROR) {
            return -1;
        }
        if buf.len() > i32::MAX as usize {
            self.error(Z_STREAM_ERROR);
            return -1;
        }
        let n = self.read_into(buf);
        if n == 0 && self.err != Z_OK && self.err != Z_BUF_ERROR {
            return -1;
        }
        n as i32
    }

    /// `gzgetc`.
    pub fn gzgetc(&mut self) -> i32 {
        if self.mode != GzMode::Read || (self.err != Z_OK && self.err != Z_BUF_ERROR) {
            return -1;
        }
        if self.x_have > 0 {
            self.x_have -= 1;
            self.x_pos += 1;
            let c = self.out[self.x_next];
            self.x_next += 1;
            return c as i32;
        }
        let mut b = [0u8];
        if self.read_into(&mut b) < 1 {
            -1
        } else {
            b[0] as i32
        }
    }

    /// `gzrewind`.
    /// `gzrewind`.
    pub fn gzrewind(&mut self) -> Result<(), ()> {
        if self.mode != GzMode::Read || (self.err != Z_OK && self.err != Z_BUF_ERROR) {
            return Err(());
        }
        self.file
            .seek(SeekFrom::Start(self.start))
            .map_err(|_| ())?;
        self.reset();
        Ok(())
    }

    /// `gzseek64`.
    pub fn gzseek(&mut self, mut offset: i64, whence: Whence) -> i64 {
        if self.err != Z_OK && self.err != Z_BUF_ERROR {
            return -1;
        }
        if whence == Whence::End {
            return -1;
        }
        if whence == Whence::Set {
            offset -= self.x_pos;
        } else if self.seek {
            offset += self.skip;
        }
        self.seek = false;
        if self.mode == GzMode::Read && self.how == How::Copy && self.x_pos + offset >= 0 {
            if self
                .file
                .seek(SeekFrom::Current(offset - self.x_have as i64))
                .is_err()
            {
                return -1;
            }
            self.x_have = 0;
            self.eof = false;
            self.past = false;
            self.seek = false;
            self.error(Z_OK);
            self.avail_in = 0;
            self.x_pos += offset;
            return self.x_pos;
        }
        if offset < 0 {
            if self.mode != GzMode::Read {
                return -1;
            }
            offset += self.x_pos;
            if offset < 0 {
                return -1;
            }
            if self.gzrewind().is_err() {
                return -1;
            }
        }
        if self.mode == GzMode::Read {
            let n = (self.x_have as i64).min(offset) as usize;
            self.x_have -= n;
            self.x_next += n;
            self.x_pos += n as i64;
            offset -= n as i64;
        }
        if offset != 0 {
            self.seek = true;
            self.skip = offset;
        }
        self.x_pos + offset
    }

    /// `gzeof`: whether a read went past the end.
    pub fn gzeof(&self) -> bool {
        self.mode == GzMode::Read && self.past
    }

    /// `gzputc`: a pending gap is written first (before the buffers exist: FS-78), then the byte.
    pub fn gzputc(&mut self, c: i32) -> i32 {
        if self.mode != GzMode::Write || self.err != Z_OK {
            return -1;
        }
        if self.seek {
            self.seek = false;
            if self.zero(self.skip).is_err() {
                return -1;
            }
        }
        if self.size > 0 && self.wbuf.len() < self.size {
            self.wbuf.push(c as u8);
            self.x_pos += 1;
            return c & 0xff;
        }
        if self.gzwrite(&[c as u8]) == 1 {
            c & 0xff
        } else {
            -1
        }
    }

    /// FS-78's notice, when it changed the file.
    pub fn notices(&self) -> &Notices {
        &self.notices
    }

    /// `gztell64`.
    pub fn gztell(&self) -> i64 {
        self.x_pos + if self.seek { self.skip } else { 0 }
    }

    // ---- writing (gzwrite.c) ----

    /// `gz_init`.
    fn init_write(&mut self) {
        self.wbuf = Vec::with_capacity(self.want);
        if !self.direct {
            let level = if self.level < 0 { 6 } else { self.level as u32 };
            self.deflate = Some(Compress::new_gzip(Compression::new(level), 15));
        }
        self.size = self.want;
    }

    /// `gz_comp(flush)`: compresses the buffered input and writes what deflate gives.
    fn comp(&mut self, flush: FlushCompress) -> Result<(), ()> {
        if self.size == 0 {
            self.init_write();
        }
        let input = std::mem::take(&mut self.wbuf);
        if self.direct {
            if self.file.write_all(&input).is_err() {
                self.error(Z_ERRNO);
                return Err(());
            }
            self.wbuf = input;
            self.wbuf.clear();
            return Ok(());
        }
        let d = self.deflate.as_mut().expect("deflate state");
        let mut at = 0usize;
        let mut out = vec![0u8; self.want];
        loop {
            let (ti, to) = (d.total_in(), d.total_out());
            let st = d.compress(&input[at..], &mut out, flush);
            at += (d.total_in() - ti) as usize;
            let made = (d.total_out() - to) as usize;
            if made > 0 && self.file.write_all(&out[..made]).is_err() {
                self.error(Z_ERRNO);
                return Err(());
            }
            match st {
                Err(_) => {
                    self.error(Z_STREAM_ERROR);
                    return Err(());
                }
                Ok(Status::StreamEnd) => break,
                Ok(_) => {
                    if made == 0 && (at == input.len() || (d.total_in() - ti) == 0) {
                        break;
                    }
                }
            }
        }
        if flush == FlushCompress::Finish {
            d.reset();
        }
        self.wbuf = input;
        self.wbuf.clear();
        Ok(())
    }

    /// `gz_zero`: `len` zeros. zlib 1.2.11 compresses (or writes) memory it never set instead
    /// when the zeros come before the first write (its buffer is allocated after the one `memset`;
    /// FS-78); larmorx writes zeros.
    fn zero(&mut self, mut len: i64) -> Result<(), ()> {
        if self.size == 0 && len > 0 {
            self.notices.add(
                &d::FS_78,
                format!(
                    "{len} zeros before the first write (zlib 1.2.11 writes memory it never set)"
                ),
            );
        }
        if !self.wbuf.is_empty() {
            self.comp(FlushCompress::None)?;
        }
        while len > 0 {
            let n = (self.size as i64).min(len) as usize;
            self.wbuf.resize(n, 0);
            self.x_pos += n as i64;
            self.comp(FlushCompress::None)?;
            len -= n as i64;
        }
        Ok(())
    }

    /// `gzwrite`: the bytes taken, or 0 after an error.
    pub fn gzwrite(&mut self, buf: &[u8]) -> i32 {
        if self.mode != GzMode::Write || self.err != Z_OK {
            return 0;
        }
        if buf.len() > i32::MAX as usize {
            self.error(Z_DATA_ERROR);
            return 0;
        }
        if buf.is_empty() {
            return 0;
        }
        if self.size == 0 {
            self.init_write();
        }
        if self.seek {
            self.seek = false;
            if self.zero(self.skip).is_err() {
                return 0;
            }
        }
        if buf.len() < self.size {
            let mut rest = buf;
            while !rest.is_empty() {
                let copy = (self.size - self.wbuf.len()).min(rest.len());
                self.wbuf.extend_from_slice(&rest[..copy]);
                self.x_pos += copy as i64;
                rest = &rest[copy..];
                if !rest.is_empty() && self.comp(FlushCompress::None).is_err() {
                    return 0;
                }
            }
        } else {
            if !self.wbuf.is_empty() && self.comp(FlushCompress::None).is_err() {
                return 0;
            }
            self.wbuf.extend_from_slice(buf);
            self.x_pos += buf.len() as i64;
            if self.comp(FlushCompress::None).is_err() {
                return 0;
            }
        }
        buf.len() as i32
    }

    /// `gzflush(Z_SYNC_FLUSH)`.
    pub fn gzflush(&mut self) -> i32 {
        if self.mode != GzMode::Write || self.err != Z_OK {
            return Z_STREAM_ERROR;
        }
        if self.seek {
            self.seek = false;
            if self.zero(self.skip).is_err() {
                return self.err;
            }
        }
        let _ = self.comp(FlushCompress::Sync);
        self.err
    }

    /// `gzclose`: `gzclose_r` (`Z_BUF_ERROR` when the data ended early, else `Z_OK`) or
    /// `gzclose_w`.
    pub fn gzclose(self) -> i32 {
        self.gzclose_with_notices().0
    }

    /// [`GzFile::gzclose`] and the notices (FS-78 may come at the close).
    pub fn gzclose_with_notices(mut self) -> (i32, Notices) {
        let r = self.close_inner();
        (r, self.notices)
    }

    fn close_inner(&mut self) -> i32 {
        match self.mode {
            GzMode::Read => {
                if self.err == Z_BUF_ERROR {
                    Z_BUF_ERROR
                } else {
                    Z_OK
                }
            }
            GzMode::Write => {
                let mut ret = Z_OK;
                if self.seek {
                    self.seek = false;
                    if self.zero(self.skip).is_err() {
                        ret = self.err;
                    }
                }
                if self.comp(FlushCompress::Finish).is_err() {
                    ret = self.err;
                }
                if self.file.flush().is_err() {
                    ret = Z_ERRNO;
                }
                ret
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Target {
    In(usize),
    Out,
}

#[derive(Clone, Copy)]
enum Dest {
    Out,
    User,
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::cfmt::{Captured, Stream};

    fn gz(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), Compression::new(6));
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    fn data(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 7 % 251) as u8).collect()
    }

    #[test]
    fn writes_and_reads_back_both_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = Captured::default();
        for comp in [false, true] {
            let p = dir.path().join(if comp { "a.gz" } else { "a.bin" });
            let pb = p.as_os_str().as_encoded_bytes();
            let d = data(70_000);
            let mut f = ZnzFile::open(pb, b"wb", comp).unwrap();
            assert_eq!(f.write(&d, 1, d.len(), &mut c), d.len() as i64);
            assert_eq!(f.tell(), d.len() as i64);
            assert_eq!(f.close(), 0);
            let mut f = ZnzFile::open(pb, b"rb", comp).unwrap();
            let mut back = vec![0u8; d.len() + 10];
            assert_eq!(f.read(&mut back, 1, d.len() + 10, &mut c), d.len() as i64);
            assert_eq!(&back[..d.len()], &d[..]);
            assert!(f.eof());
            f.close();
        }
        assert_eq!(c.text(Stream::Err), "");
    }

    #[test]
    fn gzip_seeks() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.gz");
        let d = data(50_000);
        std::fs::write(&p, gz(&d)).unwrap();
        let mut f = ZnzFile::open(p.as_os_str().as_encoded_bytes(), b"rb", true).unwrap();
        let mut c = Captured::default();
        assert_eq!(f.seek(30_000, Whence::Set), 30_000);
        assert_eq!(f.tell(), 30_000);
        assert_eq!(f.getc(), d[30_000] as i32);
        assert_eq!(f.seek(-20_000, Whence::Cur), 10_001);
        let mut b = [0u8; 4];
        assert_eq!(f.read(&mut b, 1, 4, &mut c), 4);
        assert_eq!(&b, &d[10_001..10_005]);
        assert_eq!(f.seek(0, Whence::End), -1);
        assert_eq!(f.seek(-1, Whence::Set), -1);
        // a seek past the end is accepted; the next read finds the end
        assert_eq!(f.seek(60_000, Whence::Set), 60_000);
        assert_eq!(f.read(&mut b, 1, 4, &mut c), 0);
        assert!(f.eof());
    }

    #[test]
    fn transparent_trailing_truncated_and_two_members() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.gz");
        let pb = p.as_os_str().as_encoded_bytes().to_vec();
        let d = data(30_000);
        let mut c = Captured::default();
        let read_all = |c: &mut Captured| {
            let mut f = ZnzFile::open(&pb, b"rb", true).unwrap();
            let mut back = vec![0u8; 100_000];
            let len = back.len();
            let n = f.read(&mut back, 1, len, c);
            (n, f.close(), back)
        };
        std::fs::write(&p, &d).unwrap();
        assert_eq!(read_all(&mut c).0, 30_000);
        let mut t = gz(&d);
        t.extend_from_slice(b"garbage");
        std::fs::write(&p, &t).unwrap();
        assert_eq!(read_all(&mut c).0, 30_000);
        let mut two = gz(&d[..10_000]);
        two.extend(gz(&d[10_000..]));
        std::fs::write(&p, &two).unwrap();
        let (n, _, back) = read_all(&mut c);
        assert_eq!(n, 30_000);
        assert_eq!(&back[..30_000], &d[..]);
        let mut cut = gz(&d);
        cut.truncate(cut.len() / 2);
        std::fs::write(&p, &cut).unwrap();
        let (n, close, _) = read_all(&mut c);
        assert!(n > 0 && n < 30_000);
        assert_eq!(close, Z_BUF_ERROR);
    }

    #[test]
    fn defects_76_77_78() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("d.gz");
        let pb = p.as_os_str().as_encoded_bytes().to_vec();
        std::fs::write(&p, gz(&data(7))).unwrap();
        let mut c = Captured::default();
        let mut buf = [0u8; 8];
        // 7 bytes read as 4 members of 2: nifticlib says 4 (FS-77), larmorx 3.
        let mut f = ZnzFile::open(&pb, b"rb", true).unwrap();
        assert_eq!(f.read(&mut buf, 2, 4, &mut c), 3);
        assert!(f.notices().contains(&d::FS_77));
        assert_eq!(c.text(Stream::Err), "** znzread: read short by 1 bytes\n");
        let mut f = ZnzFile::open(&pb, b"rb", true)
            .unwrap()
            .with_upstream_defects(UpstreamDefects::Reproduce);
        assert_eq!(f.read(&mut buf, 2, 4, &mut c), 4);
        let mut f = ZnzFile::open(&pb, b"rb", true).unwrap();
        assert_eq!(f.read(&mut buf, 0, 4, &mut c), 0);
        assert!(f.notices().contains(&d::FS_76));
        // a gap before the first write: zeros, and FS-78's notice
        let q = dir.path().join("w.gz");
        let mut f = ZnzFile::open(q.as_os_str().as_encoded_bytes(), b"wb", true).unwrap();
        assert_eq!(f.seek(100, Whence::Set), 100);
        assert_eq!(f.putc(7), 7);
        let (r, n) = f.close_with_notices();
        assert_eq!(r, 0);
        assert!(n.contains(&d::FS_78));
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(&std::fs::read(&q).unwrap()[..])
            .read_to_end(&mut out)
            .unwrap();
        assert_eq!(out.len(), 101);
        assert!(out[..100].iter().all(|&b| b == 0));
        assert_eq!(out[100], 7);
    }

    #[test]
    fn plain_streams() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("p.bin");
        std::fs::write(&p, data(10)).unwrap();
        let mut c = Captured::default();
        let mut f = ZnzFile::open(p.as_os_str().as_encoded_bytes(), b"rb", false).unwrap();
        assert_eq!(f.seek(3, Whence::Set), 0);
        assert_eq!(f.tell(), 3);
        let mut b = [0u8; 20];
        assert_eq!(f.read(&mut b, 4, 5, &mut c), 1);
        assert!(f.eof());
        assert_eq!(f.seek(-2, Whence::End), 0);
        assert!(!f.eof());
        assert_eq!(f.getc(), data(10)[8] as i32);
        assert!(ZnzFile::open(b"/nonexistent/x", b"rb", false).is_none());
        assert!(ZnzFile::open(p.as_os_str().as_encoded_bytes(), b"q", false).is_none());
        assert!(ZnzFile::open(p.as_os_str().as_encoded_bytes(), b"r+", true).is_none());
    }
}
