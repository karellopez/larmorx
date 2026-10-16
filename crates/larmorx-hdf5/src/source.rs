// SPDX-License-Identifier: Apache-2.0
//! Where file bytes come from (a file read on demand, or bytes in memory), and a cursor that
//! decodes HDF5's little-endian fields.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::{Error, Result, corrupt};

/// The bytes of an HDF5 file, read at absolute offsets.
pub(crate) enum Source {
    Memory(Vec<u8>),
    File {
        path: PathBuf,
        file: Mutex<std::fs::File>,
        len: u64,
    },
}

impl Source {
    pub(crate) fn open(path: &Path) -> Result<Source> {
        let io = |source| Error::Io {
            path: path.to_path_buf(),
            source,
        };
        let file = std::fs::File::open(path).map_err(io)?;
        let len = file.metadata().map_err(io)?.len();
        Ok(Source::File {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            len,
        })
    }

    /// The size of the file in bytes (its end of file, as the operating system reports it).
    pub(crate) fn len(&self) -> u64 {
        match self {
            Source::Memory(b) => b.len() as u64,
            Source::File { len, .. } => *len,
        }
    }

    /// `len` bytes at absolute offset `offset`; an error if they are not all in the file.
    pub(crate) fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let end = offset
            .checked_add(len as u64)
            .ok_or_else(|| corrupt("address overflow"))?;
        if end > self.len() {
            return Err(corrupt(format!(
                "read of {len} bytes at address {offset} past the end of the file ({})",
                self.len()
            )));
        }
        match self {
            Source::Memory(b) => Ok(b[offset as usize..end as usize].to_vec()),
            Source::File { path, file, .. } => {
                let mut buf = vec![0u8; len];
                let mut f = file.lock().unwrap_or_else(|e| e.into_inner());
                let io = |source| Error::Io {
                    path: path.clone(),
                    source,
                };
                f.seek(SeekFrom::Start(offset)).map_err(io)?;
                f.read_exact(&mut buf).map_err(io)?;
                Ok(buf)
            }
        }
    }

    /// Up to `len` bytes at `offset` (fewer at the end of the file).
    pub(crate) fn read_up_to(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let avail = self.len().saturating_sub(offset);
        self.read_at(offset, len.min(avail as usize))
    }
}

/// An address value meaning "not allocated" (all bits set).
pub const UNDEF: u64 = u64::MAX;

/// Decodes little-endian fields of a structure, with the file's sizes of offsets and lengths.
#[derive(Clone)]
pub(crate) struct Cursor<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) pos: usize,
    pub(crate) sizeof_addr: u8,
    pub(crate) sizeof_size: u8,
    /// What is being decoded, for error messages.
    pub(crate) what: &'static str,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(buf: &'a [u8], sizeof_addr: u8, sizeof_size: u8, what: &'static str) -> Self {
        Cursor {
            buf,
            pos: 0,
            sizeof_addr,
            sizeof_size,
            what,
        }
    }

    fn short(&self) -> Error {
        corrupt(format!("{} is truncated", self.what))
    }

    pub(crate) fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or_else(|| self.short())?;
        let s = self.buf.get(self.pos..end).ok_or_else(|| self.short())?;
        self.pos = end;
        Ok(s)
    }

    pub(crate) fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    pub(crate) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub(crate) fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }

    /// An unsigned integer of `n` bytes (0 to 8), little-endian.
    pub(crate) fn uvar(&mut self, n: usize) -> Result<u64> {
        if n > 8 {
            return Err(corrupt(format!("{}: field of {n} bytes", self.what)));
        }
        let b = self.bytes(n)?;
        Ok(b.iter()
            .rev()
            .fold(0u64, |acc, &x| (acc << 8) | u64::from(x)))
    }

    /// An address (`sizeof_addr` bytes); all bits set gives [`UNDEF`].
    pub(crate) fn addr(&mut self) -> Result<u64> {
        let n = usize::from(self.sizeof_addr);
        let b = self.bytes(n)?;
        if b.iter().all(|&x| x == 0xff) {
            return Ok(UNDEF);
        }
        Ok(b.iter()
            .rev()
            .fold(0u64, |acc, &x| (acc << 8) | u64::from(x)))
    }

    /// A length (`sizeof_size` bytes).
    pub(crate) fn length(&mut self) -> Result<u64> {
        let n = usize::from(self.sizeof_size);
        self.uvar(n)
    }

    /// Checks a signature of 4 bytes.
    pub(crate) fn magic(&mut self, magic: &[u8; 4]) -> Result<()> {
        let got = self.bytes(4)?;
        if got != magic {
            return Err(corrupt(format!(
                "{}: wrong signature {:?}, expected {:?}",
                self.what,
                String::from_utf8_lossy(got),
                String::from_utf8_lossy(magic)
            )));
        }
        Ok(())
    }
}

/// `floor(log2(value))`, 0 for 0: `H5VM_log2_gen`.
pub(crate) fn log2_gen(value: u64) -> u32 {
    if value == 0 {
        0
    } else {
        63 - value.leading_zeros()
    }
}

/// The number of bytes needed to encode `value` (at least 1): `H5VM_limit_enc_size`.
pub(crate) fn limit_enc_size(value: u64) -> usize {
    (log2_gen(value) / 8) as usize + 1
}

/// Verifies the lookup3 checksum stored in the last 4 bytes of `image`.
pub(crate) fn verify_checksum(image: &[u8], what: &str) -> Result<()> {
    if image.len() < 4 {
        return Err(corrupt(format!("{what} is truncated")));
    }
    let (data, stored) = image.split_at(image.len() - 4);
    let stored = u32::from_le_bytes(stored.try_into().unwrap());
    let computed = crate::checksum::metadata(data);
    if stored != computed {
        return Err(Error::Checksum(what.to_string()));
    }
    Ok(())
}
