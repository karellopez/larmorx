// SPDX-License-Identifier: Apache-2.0
//! The file layer of libtiff 3.6.1's Unix build (`tif_unix.c`, built without `HAVE_MMAP` as
//! FreeSurfer 8.2.0 builds it, so files are never mapped): `open`, `read`, `write`, `lseek` and
//! `fstat` with Linux's results on every platform. A name that is a folder opens for reading
//! (Linux's `open(O_RDONLY)` of a folder succeeds) and every read of it fails, as Linux's
//! `read` does (`EISDIR`); opened for writing it fails. One `read` transfers at most
//! 0x7ffff000 bytes, Linux's `MAX_RW_COUNT`; a negative count fails (`EINVAL`).

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

/// Linux's `MAX_RW_COUNT` (`INT_MAX & PAGE_MASK`).
const MAX_RW_COUNT: u64 = 0x7fff_f000;

/// An open descriptor.
#[derive(Debug)]
pub(crate) enum Fd {
    File(File),
    /// A folder opened for reading: reads fail, seeks succeed, its size is what `fstat` gives
    /// (modelled as 4,096 bytes, ext4's).
    Folder(u64),
}

/// `whence` of `lseek`.
#[derive(Clone, Copy)]
pub(crate) enum Whence {
    Set,
    Cur,
    End,
}

impl Fd {
    /// `open(name, O_RDONLY)` or `open(name, O_RDWR | O_CREAT [| O_TRUNC], 0666)`.
    pub(crate) fn open(name: &str, write: bool, truncate: bool) -> Option<Fd> {
        let path = std::path::Path::new(name);
        if name.is_empty() {
            return None;
        }
        if path.is_dir() {
            return if write { None } else { Some(Fd::Folder(0)) };
        }
        let f = if write {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(truncate)
                .open(path)
        } else {
            File::open(path)
        };
        f.ok().map(Fd::File)
    }

    /// `read(fd, buf, size)` with libtiff's `tsize_t` count: the bytes read, or -1.
    pub(crate) fn read(&mut self, buf: &mut [u8], size: i32) -> i32 {
        if size < 0 {
            return -1;
        }
        match self {
            Fd::Folder(_) => -1,
            Fd::File(f) => {
                let want = (size as u64).min(MAX_RW_COUNT).min(buf.len() as u64) as usize;
                let mut got = 0usize;
                while got < want {
                    match f.read(&mut buf[got..want]) {
                        Ok(0) => break,
                        Ok(n) => got += n,
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(_) => return if got == 0 { -1 } else { got as i32 },
                    }
                }
                got as i32
            }
        }
    }

    /// `write(fd, buf, size)`: the bytes written, or -1.
    pub(crate) fn write(&mut self, buf: &[u8], size: i32) -> i32 {
        if size < 0 {
            return -1;
        }
        match self {
            Fd::Folder(_) => -1,
            Fd::File(f) => {
                let n = (size as u64).min(MAX_RW_COUNT).min(buf.len() as u64) as usize;
                match f.write_all(&buf[..n]) {
                    Ok(()) => n as i32,
                    Err(_) => -1,
                }
            }
        }
    }

    /// `lseek(fd, (off_t) off, whence)` with libtiff's `toff_t` (32-bit) offset, the result
    /// truncated to `toff_t` (-1 as 0xffffffff).
    pub(crate) fn seek(&mut self, off: u32, whence: Whence) -> u32 {
        match self {
            Fd::Folder(pos) => {
                let p = match whence {
                    Whence::Set => u64::from(off),
                    Whence::Cur => *pos + u64::from(off),
                    Whence::End => 4096 + u64::from(off),
                };
                *pos = p;
                p as u32
            }
            Fd::File(f) => {
                let r = match whence {
                    Whence::Set => f.seek(SeekFrom::Start(u64::from(off))),
                    Whence::Cur => f.seek(SeekFrom::Current(i64::from(off))),
                    Whence::End => f.seek(SeekFrom::End(i64::from(off))),
                };
                match r {
                    Ok(p) => p as u32,
                    Err(_) => u32::MAX,
                }
            }
        }
    }

    /// `_tiffSizeProc`: `fstat`'s size as `toff_t` (0 when `fstat` fails).
    pub(crate) fn size(&self) -> u32 {
        match self {
            Fd::Folder(_) => 4096,
            Fd::File(f) => f.metadata().map(|m| m.len() as u32).unwrap_or(0),
        }
    }
}
