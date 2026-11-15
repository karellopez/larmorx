// SPDX-License-Identifier: Apache-2.0 AND Zlib
//! zlib 1.2.11's compressor (Jean-loup Gailly and Mark Adler; zlib's licence), translated so that
//! it gives zlib's own bytes: `deflate.c`, `trees.c`, `compress.c` and the checksums
//! (`adler32.c`, `crc32.c`), as Ubuntu 22.04 builds them (`zlib1g` 1:1.2.11.dfsg-2ubuntu9.2,
//! the library FreeSurfer 8.2.0's official programs load: its patches for CVE-2018-25032, the
//! `deflateParams` change and the hooks of IBM Z's hardware compressor, which an x86-64 build
//! compiles to nothing).
//!
//! The decompressor is not here: larmorx reads deflate data through zlib-rs (flate2) and
//! [`crate::miniz`]; their output is fixed by the format. A compressor chooses its own matches
//! and blocks, so only a translation of zlib's gives zlib's bytes (an MGZ or a GIFTI file
//! identical to FreeSurfer's).
//!
//! The interface follows zlib's `z_stream` with Rust's slices in place of its pointers: each
//! call takes the input still available (`next_in`/`avail_in`) and the output space
//! (`next_out`/`avail_out`) and says how much of each it used. Null pointers cannot be passed,
//! so zlib's checks of them have no counterpart. `gzip` headers write `OS_CODE` 3 (Unix) on
//! every platform, as the Linux build does.

#[doc(hidden)]
pub mod cases;
mod deflate;
mod trees;

#[cfg(test)]
mod tests;

pub use deflate::{Deflate, GzHeader};

// zlib.h's constants.
pub const Z_NO_FLUSH: i32 = 0;
pub const Z_PARTIAL_FLUSH: i32 = 1;
pub const Z_SYNC_FLUSH: i32 = 2;
pub const Z_FULL_FLUSH: i32 = 3;
pub const Z_FINISH: i32 = 4;
pub const Z_BLOCK: i32 = 5;
pub const Z_TREES: i32 = 6;

pub const Z_OK: i32 = 0;
pub const Z_STREAM_END: i32 = 1;
pub const Z_NEED_DICT: i32 = 2;
pub const Z_ERRNO: i32 = -1;
pub const Z_STREAM_ERROR: i32 = -2;
pub const Z_DATA_ERROR: i32 = -3;
pub const Z_MEM_ERROR: i32 = -4;
pub const Z_BUF_ERROR: i32 = -5;
pub const Z_VERSION_ERROR: i32 = -6;

pub const Z_NO_COMPRESSION: i32 = 0;
pub const Z_BEST_SPEED: i32 = 1;
pub const Z_BEST_COMPRESSION: i32 = 9;
pub const Z_DEFAULT_COMPRESSION: i32 = -1;

pub const Z_FILTERED: i32 = 1;
pub const Z_HUFFMAN_ONLY: i32 = 2;
pub const Z_RLE: i32 = 3;
pub const Z_FIXED: i32 = 4;
pub const Z_DEFAULT_STRATEGY: i32 = 0;

pub const Z_BINARY: i32 = 0;
pub const Z_TEXT: i32 = 1;
pub const Z_UNKNOWN: i32 = 2;

pub const Z_DEFLATED: i32 = 8;

/// `MAX_WBITS`, `DEF_MEM_LEVEL` and `MAX_MEM_LEVEL` (zconf.h, zutil.h).
pub const MAX_WBITS: i32 = 15;
pub const DEF_MEM_LEVEL: i32 = 8;
pub const MAX_MEM_LEVEL: i32 = 9;

/// `ZLIB_VERSION`.
pub const ZLIB_VERSION: &str = "1.2.11";

/// zlib's messages for its return codes (`z_errmsg`, as `ERR_MSG` indexes it).
pub fn err_msg(err: i32) -> &'static str {
    match err {
        Z_NEED_DICT => "need dictionary",
        Z_STREAM_END => "stream end",
        Z_OK => "",
        Z_ERRNO => "file error",
        Z_STREAM_ERROR => "stream error",
        Z_DATA_ERROR => "data error",
        Z_MEM_ERROR => "insufficient memory",
        Z_BUF_ERROR => "buffer error",
        Z_VERSION_ERROR => "incompatible version",
        _ => "",
    }
}

const BASE: u64 = 65521;
const NMAX: usize = 5552;

/// `adler32_z(adler, buf, len)`; with no buffer (`None`) the initial value 1.
pub fn adler32(adler: u64, buf: Option<&[u8]>) -> u64 {
    let mut sum2 = (adler >> 16) & 0xffff;
    let mut adler = adler & 0xffff;
    let Some(buf) = buf else {
        // zlib returns 1 for a null buffer, after its one-byte shortcut (which needs a byte).
        return 1;
    };
    if buf.len() == 1 {
        adler += u64::from(buf[0]);
        if adler >= BASE {
            adler -= BASE;
        }
        sum2 += adler;
        if sum2 >= BASE {
            sum2 -= BASE;
        }
        return adler | (sum2 << 16);
    }
    if buf.len() < 16 {
        for &b in buf {
            adler += u64::from(b);
            sum2 += adler;
        }
        if adler >= BASE {
            adler -= BASE;
        }
        sum2 %= BASE;
        return adler | (sum2 << 16);
    }
    let mut rest = buf;
    while rest.len() >= NMAX {
        for &b in &rest[..NMAX] {
            adler += u64::from(b);
            sum2 += adler;
        }
        adler %= BASE;
        sum2 %= BASE;
        rest = &rest[NMAX..];
    }
    if !rest.is_empty() {
        for &b in rest {
            adler += u64::from(b);
            sum2 += adler;
        }
        adler %= BASE;
        sum2 %= BASE;
    }
    adler | (sum2 << 16)
}

/// The CRC-32 table of the polynomial 0xedb88320 (zlib's `crc_table[0]`).
const CRC_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[n] = c;
        n += 1;
    }
    t
};

/// `crc32_z(crc, buf, len)`; with no buffer (`None`) the initial value 0.
pub fn crc32(crc: u64, buf: Option<&[u8]>) -> u64 {
    let Some(buf) = buf else {
        return 0;
    };
    let mut c = (crc as u32) ^ 0xffff_ffff;
    for &b in buf {
        c = CRC_TABLE[((c ^ u32::from(b)) & 0xff) as usize] ^ (c >> 8);
    }
    u64::from(c ^ 0xffff_ffff)
}

/// `compressBound(sourceLen)`.
pub fn compress_bound(source_len: u64) -> u64 {
    source_len
        .wrapping_add(source_len >> 12)
        .wrapping_add(source_len >> 14)
        .wrapping_add(source_len >> 25)
        .wrapping_add(13)
}

/// `compress2(dest, &destLen, source, sourceLen, level)`: the zlib stream of `source` into
/// `dest` (its length is `destLen` on entry); the status and the bytes written (`destLen` on
/// return).
pub fn compress2(dest: &mut [u8], source: &[u8], level: i32) -> (i32, usize) {
    const MAX: u64 = u32::MAX as u64;
    let mut left = dest.len() as u64;
    let mut stream = match Deflate::init(level) {
        Ok(s) => s,
        Err(e) => return (e, 0),
    };
    let (mut out_at, mut out_end) = (0usize, 0usize);
    let (mut in_at, mut in_end) = (0usize, 0usize);
    let mut source_len = source.len() as u64;
    let mut err;
    loop {
        if out_at == out_end {
            let n = left.min(MAX);
            out_end = out_at + n as usize;
            left -= n;
        }
        if in_at == in_end {
            let n = source_len.min(MAX);
            in_end = in_at + n as usize;
            source_len -= n;
        }
        let flush = if source_len != 0 {
            Z_NO_FLUSH
        } else {
            Z_FINISH
        };
        let (r, used, made) =
            stream.deflate(&source[in_at..in_end], &mut dest[out_at..out_end], flush);
        in_at += used;
        out_at += made;
        err = r;
        if err != Z_OK {
            break;
        }
    }
    let n = stream.total_out as usize;
    stream.end();
    (if err == Z_STREAM_END { Z_OK } else { err }, n)
}

/// `compress2` into a buffer of `compressBound` bytes, the bytes written returned.
pub fn compress_vec(source: &[u8], level: i32) -> Result<Vec<u8>, i32> {
    let mut out = vec![0u8; compress_bound(source.len() as u64) as usize];
    let (r, n) = compress2(&mut out, source, level);
    if r != Z_OK {
        return Err(r);
    }
    out.truncate(n);
    Ok(out)
}
