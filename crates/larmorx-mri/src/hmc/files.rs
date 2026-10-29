// SPDX-License-Identifier: Apache-2.0
//! Image file names and compression as FSL 6.0.7.23's mcflirt opens them (`specs/mcflirt.md`
//! §4.1, §10.1).
//!
//! A file is read by its content (gzip, zstd, bzip2 or uncompressed), whatever its name; a
//! name without an image extension is completed by trying FSL's extensions in its order; a
//! pair is a `.hdr` file and an `.img` file with the same compression suffix. Outputs are
//! written in the compression their extension names, from `FSLOUTPUTTYPE`.
//!
//! FSL 6.0.7.23 opens the header (or single) file of every image under its name with the last
//! `.img` replaced by `.hdr`, also where that `.img` is in a directory or in the file's own stem
//! (FSL-60): [`fsl60_header`] gives that name, which `reproduce` opens; by default larmorx opens
//! the name given.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// How a file's bytes are compressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    None,
    Gzip,
    Zstd,
    Bzip2,
}

impl Compression {
    /// From a file's first bytes: gzip `1f 8b`, zstd `28 b5 2f fd`, bzip2 `42 5a 68`; `None`
    /// when fewer than four bytes decide nothing.
    pub fn of_bytes(head: &[u8]) -> Option<Compression> {
        if head.len() < 4 {
            return None;
        }
        Some(if head[..2] == [0x1f, 0x8b] {
            Compression::Gzip
        } else if head[..4] == [0x28, 0xb5, 0x2f, 0xfd] {
            Compression::Zstd
        } else if head[..3] == *b"BZh" {
            Compression::Bzip2
        } else {
            Compression::None
        })
    }

    /// From a name's suffix (`.gz`, `.zst`, `.bz2`).
    pub fn of_name(name: &str) -> Compression {
        if name.ends_with(".gz") {
            Compression::Gzip
        } else if name.ends_with(".zst") {
            Compression::Zstd
        } else if name.ends_with(".bz2") {
            Compression::Bzip2
        } else {
            Compression::None
        }
    }

    /// The compression of the file at `path`: by its first four bytes, or, for a shorter file,
    /// by its name (§4.1).
    pub fn of_file(path: &Path) -> io::Result<Compression> {
        let mut head = [0u8; 4];
        let mut file = File::open(path)?;
        let mut got = 0;
        while got < 4 {
            match file.read(&mut head[got..])? {
                0 => break,
                n => got += n,
            }
        }
        Ok(Compression::of_bytes(&head[..got])
            .unwrap_or_else(|| Compression::of_name(&path.to_string_lossy())))
    }

    /// The file's bytes, decompressed (every concatenated stream or frame).
    pub fn decompress(self, raw: Vec<u8>) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        match self {
            Compression::None => return Ok(raw),
            Compression::Gzip => {
                flate2::read::MultiGzDecoder::new(raw.as_slice()).read_to_end(&mut out)?;
            }
            Compression::Bzip2 => {
                bzip2::read::MultiBzDecoder::new(raw.as_slice()).read_to_end(&mut out)?;
            }
            Compression::Zstd => zstd_decode(&raw, &mut out)?,
        }
        Ok(out)
    }

    /// `bytes` compressed into `w` ([`Compression::write_pieces`] with one piece).
    pub fn compress(self, bytes: &[u8], w: impl Write) -> io::Result<()> {
        self.write_pieces(&[bytes], w)
    }

    /// The `pieces` of a file, one after the other, compressed into `w`: gzip at level 2 (as
    /// larmorx's NIfTI writer) and bzip2 with blocks of 900 kB as one stream each, zstd (ruzstd's
    /// fastest level) as one frame per piece, as FSL 6.0.7.23 writes the pieces of a file. Only
    /// the decompressed bytes are specified (§10.1).
    pub fn write_pieces(self, pieces: &[&[u8]], w: impl Write) -> io::Result<()> {
        match self {
            Compression::None => {
                let mut w = w;
                for p in pieces {
                    w.write_all(p)?;
                }
                w.flush()
            }
            Compression::Gzip => {
                let mut enc = larmorx_io::gzip::ParallelGzEncoder::new(w, 2)?;
                for p in pieces {
                    enc.write_all(p)?;
                }
                enc.finish()?.flush()
            }
            Compression::Bzip2 => {
                let mut enc = bzip2::write::BzEncoder::new(w, bzip2::Compression::best());
                for p in pieces {
                    enc.write_all(p)?;
                }
                enc.finish()?.flush()
            }
            Compression::Zstd => {
                let mut w = w;
                for p in pieces {
                    ruzstd::encoding::compress(
                        *p,
                        &mut w,
                        ruzstd::encoding::CompressionLevel::Fastest,
                    );
                }
                w.flush()
            }
        }
    }
}

/// Decodes every zstd frame of `raw` into `out` (skippable frames skipped).
fn zstd_decode(raw: &[u8], out: &mut Vec<u8>) -> io::Result<()> {
    use ruzstd::decoding::errors::{FrameDecoderError, ReadFrameHeaderError};
    use ruzstd::decoding::{BlockDecodingStrategy, FrameDecoder};
    let bad = |e: FrameDecoderError| io::Error::new(io::ErrorKind::InvalidData, e.to_string());
    let mut src = raw;
    let mut dec = FrameDecoder::new();
    while !src.is_empty() {
        match dec.init(&mut src) {
            Ok(()) => {}
            Err(FrameDecoderError::ReadFrameHeaderError(ReadFrameHeaderError::SkipFrame {
                length,
                ..
            })) => {
                src = src.get(length as usize..).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::UnexpectedEof, "truncated zstd frame")
                })?;
                continue;
            }
            Err(e) => return Err(bad(e)),
        }
        loop {
            dec.decode_blocks(&mut src, BlockDecodingStrategy::UptoBytes(1 << 20))
                .map_err(bad)?;
            dec.collect_to_writer(&mut *out)?;
            if dec.is_finished() {
                dec.collect_to_writer(&mut *out)?;
                break;
            }
        }
    }
    Ok(())
}

/// The single-file extensions, every compression.
const SINGLE: [&str; 4] = [".nii", ".nii.gz", ".nii.zst", ".nii.bz2"];
/// The extensions of a pair's header file.
const HEADER: [&str; 4] = [".hdr", ".hdr.gz", ".hdr.zst", ".hdr.bz2"];
/// The extensions of a pair's data file.
const DATA: [&str; 4] = [".img", ".img.gz", ".img.zst", ".img.bz2"];

/// The order in which FSL 6.0.7.23 completes a name without an image extension (§4.1).
pub const COMPLETIONS: [&str; 8] = [
    ".nii.gz", ".nii", ".hdr", ".hdr.gz", ".nii.zst", ".hdr.zst", ".nii.bz2", ".hdr.bz2",
];

/// The image extension `name` ends with (`.nii`, `.hdr`, `.img`, each optionally `.gz`,
/// `.zst` or `.bz2`), if any.
pub fn image_extension(name: &str) -> Option<&'static str> {
    SINGLE
        .iter()
        .chain(&HEADER)
        .chain(&DATA)
        .filter(|e| name.ends_with(*e))
        .max_by_key(|e| e.len())
        .copied()
}

/// `name` without its image extension.
pub fn strip_image_extension(name: &str) -> &str {
    match image_extension(name) {
        Some(e) => &name[..name.len() - e.len()],
        None => name,
    }
}

/// The files of an image: the header (or single) file and, for a pair, the data file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageFiles {
    pub header: PathBuf,
    pub data: Option<PathBuf>,
}

impl ImageFiles {
    /// The files a name with an image extension opens: the file itself, or for a pair the
    /// `.hdr` and `.img` names with the given compression suffix (either may be given).
    pub fn of(name: &str) -> Option<ImageFiles> {
        let ext = image_extension(name)?;
        let stem = &name[..name.len() - ext.len()];
        let suffix = &ext[4..];
        if ext.starts_with(".nii") {
            return Some(ImageFiles {
                header: PathBuf::from(name),
                data: None,
            });
        }
        Some(ImageFiles {
            header: PathBuf::from(format!("{stem}.hdr{suffix}")),
            data: Some(PathBuf::from(format!("{stem}.img{suffix}"))),
        })
    }

    /// Whether the files are named as larmorx's NIfTI reader and writer name them (`.nii`,
    /// `.hdr`, `.img`, optionally `.gz`; a pair's files with the same suffix), so that they read
    /// and write them (when the content is gzip or uncompressed).
    pub fn standard_names(&self) -> bool {
        let named = |p: &Path, exts: &[&str]| {
            let s = p.to_string_lossy();
            exts.iter().any(|e| s.ends_with(e))
        };
        match &self.data {
            None => named(&self.header, &SINGLE[..2]),
            Some(d) => {
                let normal = ImageFiles::of(&self.header.to_string_lossy());
                named(&self.header, &HEADER[..2])
                    && normal.is_some_and(|n| n.data.as_deref() == Some(d.as_path()))
            }
        }
    }
}

/// An input name as FSL 6.0.7.23 resolves it (§4.1): a name with an image extension as given;
/// otherwise the first of [`COMPLETIONS`] that exists. More than one existing completion, or
/// none, is an error with FSL's words.
pub fn resolve_input(name: &str) -> Result<String, String> {
    if image_extension(name).is_some() {
        return Ok(name.to_owned());
    }
    let found: Vec<String> = COMPLETIONS
        .iter()
        .map(|e| format!("{name}{e}"))
        .filter(|p| Path::new(p).is_file())
        .collect();
    match found.len() {
        1 => Ok(found.into_iter().next().unwrap_or_default()),
        0 => Err(format!("No image files match: {name}")),
        _ => Err(format!(
            "Multiple possible filenames detected for basename: {name}"
        )),
    }
}

/// FSL-60: the header (or single) file FSL 6.0.7.23 opens for the image file name `name` (with
/// its extension): `name` with its last `.img` replaced by `.hdr`, when that is not the file
/// `name` normally opens (an `.img` in a directory or in the file's stem). The data file of a
/// pair keeps its name.
pub fn fsl60_header(name: &str) -> Option<String> {
    let at = name.rfind(".img")?;
    let fsl = format!("{}.hdr{}", &name[..at], &name[at + 4..]);
    let normal = ImageFiles::of(name).map_or_else(|| PathBuf::from(name), |f| f.header);
    (Path::new(&fsl) != normal).then_some(fsl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_and_pairs() {
        assert_eq!(image_extension("a/b.nii.zst"), Some(".nii.zst"));
        assert_eq!(image_extension("b.hdr.bz2"), Some(".hdr.bz2"));
        assert_eq!(image_extension("b.img"), Some(".img"));
        assert_eq!(image_extension("b.nii.xz"), None);
        assert_eq!(strip_image_extension("x.img.d/out.nii.gz"), "x.img.d/out");
        assert_eq!(strip_image_extension("plain"), "plain");
        let p = ImageFiles::of("x.img.zst").unwrap();
        assert_eq!(p.header, PathBuf::from("x.hdr.zst"));
        assert_eq!(p.data, Some(PathBuf::from("x.img.zst")));
        assert_eq!(ImageFiles::of("y.nii.bz2").unwrap().data, None);
        assert!(ImageFiles::of("y.txt").is_none());
    }

    /// FSL-60 (observed with mcflirt 6.0.7.23): the last `.img` of the name becomes `.hdr`.
    #[test]
    fn fsl60_names() {
        assert_eq!(
            fsl60_header("a.img.d/in.nii.gz").as_deref(),
            Some("a.hdr.d/in.nii.gz")
        );
        assert_eq!(
            fsl60_header("sub.img2.nii.gz").as_deref(),
            Some("sub.hdr2.nii.gz")
        );
        assert_eq!(
            fsl60_header("c.img.d/out.hdr").as_deref(),
            Some("c.hdr.d/out.hdr")
        );
        assert_eq!(
            fsl60_header("x.hdr.y.img.d/o5.hdr").as_deref(),
            Some("x.hdr.y.hdr.d/o5.hdr")
        );
        // A pair's data name opens its header: no defect, also in a directory holding .img.
        assert_eq!(fsl60_header("p.img"), None);
        assert_eq!(fsl60_header("p.img.d/pair.img.gz"), None);
        assert_eq!(fsl60_header("in.nii.gz"), None);
    }

    #[test]
    fn compression_round_trips() {
        let bytes: Vec<u8> = (0..200_000u32).map(|i| (i * 7 % 251) as u8).collect();
        for c in [
            Compression::None,
            Compression::Gzip,
            Compression::Zstd,
            Compression::Bzip2,
        ] {
            let mut out = Vec::new();
            c.compress(&bytes, &mut out).unwrap();
            if c != Compression::None {
                assert_eq!(Compression::of_bytes(&out), Some(c), "{c:?}");
            }
            assert_eq!(c.decompress(out).unwrap(), bytes, "{c:?}");
        }
        // Two zstd frames, as FSL writes the pieces of a file.
        let mut two = Vec::new();
        Compression::Zstd
            .compress(&bytes[..1000], &mut two)
            .unwrap();
        Compression::Zstd
            .compress(&bytes[1000..], &mut two)
            .unwrap();
        assert_eq!(Compression::Zstd.decompress(two).unwrap(), bytes);
        assert_eq!(Compression::of_bytes(b"ab"), None);
        assert_eq!(Compression::of_name("x.nii.bz2"), Compression::Bzip2);
    }
}
