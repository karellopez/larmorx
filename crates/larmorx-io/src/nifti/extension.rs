//! Header extensions (NIfTI-1 §"extensions"): `esize`, `ecode`, then `esize - 8` bytes.

use std::io::{self, Read, Write};

use super::header::ByteOrder;

/// A header extension. The content is kept exactly as stored, including any trailing NUL
/// padding, so writing it back reproduces the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extension {
    /// `NIFTI_ECODE_*` (e.g. 4 = AFNI, 6 = comment, 2 = DICOM, 32 = CIFTI).
    pub code: i32,
    pub content: Vec<u8>,
}

/// Why the extensions could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ExtensionError {
    #[error("extension size {0} is smaller than its 8-byte header")]
    BadSize(i32),
    #[error("the file ends inside an extension")]
    Truncated,
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl Extension {
    pub fn new(code: i32, content: Vec<u8>) -> Self {
        Extension { code, content }
    }

    /// The content without trailing NULs (what nibabel reports as the extension content).
    pub fn trimmed_content(&self) -> &[u8] {
        let end = self
            .content
            .iter()
            .rposition(|&b| b != 0)
            .map_or(0, |i| i + 1);
        &self.content[..end]
    }

    /// Bytes on disk: the 8-byte header plus the content, padded to a multiple of 16.
    pub fn size_on_disk(&self) -> usize {
        (8 + self.content.len()).div_ceil(16) * 16
    }

    /// Reads extensions from just after the 4-byte extender. `limit` is the number of bytes
    /// available (up to the voxel data of a single file); `None` reads to the end of the
    /// stream (a separate header file). Like nibabel, reading stops when fewer than 16 bytes
    /// remain.
    pub fn read_all<R: Read + ?Sized>(
        reader: &mut R,
        order: ByteOrder,
        limit: Option<usize>,
    ) -> Result<Vec<Self>, ExtensionError> {
        let mut out = Vec::new();
        let mut remaining = limit;
        while remaining.is_none_or(|r| r >= 16) {
            let mut head = [0u8; 8];
            let got = read_up_to(reader, &mut head)?;
            if got == 0 && remaining.is_none() {
                break;
            }
            if got < 8 {
                return Err(ExtensionError::Truncated);
            }
            let field = |b: [u8; 4]| match order {
                ByteOrder::Little => i32::from_le_bytes(b),
                ByteOrder::Big => i32::from_be_bytes(b),
            };
            let esize = field(head[..4].try_into().unwrap());
            let code = field(head[4..].try_into().unwrap());
            let size = usize::try_from(esize)
                .ok()
                .filter(|&s| s >= 8)
                .ok_or(ExtensionError::BadSize(esize))?;
            let mut content = vec![0u8; size - 8];
            if read_up_to(reader, &mut content)? < content.len() {
                return Err(ExtensionError::Truncated);
            }
            out.push(Extension { code, content });
            remaining = remaining.map(|r| r.saturating_sub(size));
        }
        Ok(out)
    }

    /// Writes this extension (header, content and padding).
    pub fn write(&self, writer: &mut impl Write, order: ByteOrder) -> io::Result<()> {
        let size = self.size_on_disk();
        let esize = i32::try_from(size)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "extension too large"))?;
        let field = |v: i32| match order {
            ByteOrder::Little => v.to_le_bytes(),
            ByteOrder::Big => v.to_be_bytes(),
        };
        writer.write_all(&field(esize))?;
        writer.write_all(&field(self.code))?;
        writer.write_all(&self.content)?;
        writer.write_all(&vec![0u8; size - 8 - self.content.len()])
    }
}

/// Reads until `buf` is full or the stream ends; returns the number of bytes read.
pub(crate) fn read_up_to<R: Read + ?Sized>(reader: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_read() {
        let exts = vec![
            Extension::new(6, b"a comment".to_vec()),
            Extension::new(4, vec![1; 24]),
        ];
        for order in [ByteOrder::Little, ByteOrder::Big] {
            let mut buf = Vec::new();
            for e in &exts {
                e.write(&mut buf, order).unwrap();
            }
            assert_eq!(buf.len(), 32 + 32);
            let back = Extension::read_all(&mut buf.as_slice(), order, Some(buf.len())).unwrap();
            assert_eq!(back[0].trimmed_content(), b"a comment");
            assert_eq!(back[0].content.len(), 24); // padded
            assert_eq!(back[1], exts[1]);
            let to_eof = Extension::read_all(&mut buf.as_slice(), order, None).unwrap();
            assert_eq!(to_eof, back);
        }
    }

    #[test]
    fn malformed_extensions_are_errors() {
        let mut bad = Vec::new();
        bad.extend_from_slice(&4i32.to_le_bytes());
        bad.extend_from_slice(&6i32.to_le_bytes());
        bad.extend_from_slice(&[0; 8]);
        assert!(matches!(
            Extension::read_all(&mut bad.as_slice(), ByteOrder::Little, Some(16)),
            Err(ExtensionError::BadSize(4))
        ));
        let mut short = Vec::new();
        short.extend_from_slice(&32i32.to_le_bytes());
        short.extend_from_slice(&6i32.to_le_bytes());
        assert!(matches!(
            Extension::read_all(&mut short.as_slice(), ByteOrder::Little, None),
            Err(ExtensionError::Truncated)
        ));
    }
}
