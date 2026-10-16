// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Filters of a dataset's pipeline, in the read direction (`H5Z_pipeline` with
//! `H5Z_FLAG_REVERSE`, HDF5 1.12.1 `src/H5Z.c`): deflate (`src/H5Zdeflate.c`), shuffle
//! (`src/H5Zshuffle.c`) and Fletcher-32 (`src/H5Zfletcher32.c`), the filters an HDF5 built
//! without szip (ITK's) applies; and deflate in the write direction for the writer.

use crate::checksum;
use crate::error::{Error, Result};
use crate::message::Filter;

pub const DEFLATE: u16 = 1;
pub const SHUFFLE: u16 = 2;
pub const FLETCHER32: u16 = 3;
pub const SZIP: u16 = 4;
pub const NBIT: u16 = 5;
pub const SCALEOFFSET: u16 = 6;

fn failure() -> Error {
    Error::Filter("filter returned failure during read".into())
}

/// Undoes the filters of `pipeline` on one stored chunk (or one compressed block), skipping
/// those whose bit is set in `mask`, last filter first.
pub fn unfilter(pipeline: &[Filter], mask: u32, mut data: Vec<u8>) -> Result<Vec<u8>> {
    for (idx, f) in pipeline.iter().enumerate().rev() {
        if idx < 32 && mask & (1 << idx) != 0 {
            continue;
        }
        data = match f.id {
            DEFLATE => inflate(f, &data)?,
            SHUFFLE => unshuffle(f, data)?,
            FLETCHER32 => fletcher32_check(&data)?,
            // Built into HDF5 but not implemented here.
            NBIT | SCALEOFFSET => {
                return Err(Error::Unsupported(format!(
                    "the {} filter",
                    if f.id == NBIT { "nbit" } else { "scaleoffset" }
                )));
            }
            _ => {
                // Not registered in ITK's HDF5 (szip is not built; plugins are not loaded).
                return Err(Error::Filter(match &f.name {
                    Some(name) if f.id != SZIP => {
                        format!("required filter '{name}' is not registered")
                    }
                    _ if f.id == SZIP => "required filter 'szip' is not registered".into(),
                    _ => "required filter (name unavailable) is not registered".into(),
                }));
            }
        };
    }
    Ok(data)
}

/// `H5Z__filter_deflate`, reading: a zlib stream inflated to its end; trailing bytes after the
/// end are ignored.
fn inflate(f: &Filter, data: &[u8]) -> Result<Vec<u8>> {
    if f.cd_values.len() != 1 || f.cd_values[0] > 9 {
        return Err(failure());
    }
    let mut d = flate2::Decompress::new(true);
    let mut out = Vec::with_capacity(data.len().max(64) * 4);
    loop {
        let consumed = d.total_in() as usize;
        if out.len() == out.capacity() {
            out.reserve(out.capacity());
        }
        let status = d
            .decompress_vec(&data[consumed..], &mut out, flate2::FlushDecompress::Sync)
            .map_err(|_| failure())?;
        match status {
            flate2::Status::StreamEnd => return Ok(out),
            flate2::Status::Ok => {}
            // No progress possible: the stream is truncated.
            flate2::Status::BufError => {
                if out.len() < out.capacity() {
                    return Err(failure());
                }
            }
        }
        if d.total_in() as usize == data.len() && out.len() < out.capacity() {
            // All input consumed, room left, and no end of stream: truncated.
            let before = d.total_out();
            let st = d
                .decompress_vec(&[], &mut out, flate2::FlushDecompress::Sync)
                .map_err(|_| failure())?;
            if st == flate2::Status::StreamEnd {
                return Ok(out);
            }
            if d.total_out() == before {
                return Err(failure());
            }
        }
    }
}

/// `H5Z__filter_shuffle`, reading: bytes of each significance back into elements; a tail
/// shorter than an element stays as it is.
fn unshuffle(f: &Filter, data: Vec<u8>) -> Result<Vec<u8>> {
    if f.cd_values.len() != 1 {
        return Err(failure());
    }
    let size = f.cd_values[0] as usize;
    if size <= 1 || data.len() / size <= 1 {
        return Ok(data);
    }
    let n = data.len() / size;
    let mut out = data.clone();
    for byte in 0..size {
        for e in 0..n {
            out[e * size + byte] = data[byte * n + e];
        }
    }
    Ok(out)
}

/// `H5Z__filter_fletcher32`, reading: checks and strips the 4-byte checksum (little-endian);
/// the byte-swapped value written by HDF5 1.6.0 to 1.6.2 is accepted too.
fn fletcher32_check(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 4 {
        return Err(failure());
    }
    let (body, stored) = data.split_at(data.len() - 4);
    let stored = u32::from_le_bytes(stored.try_into().unwrap());
    let computed = checksum::fletcher32(body);
    let b = computed.to_le_bytes();
    let reversed = u32::from_le_bytes([b[1], b[0], b[3], b[2]]);
    if stored != computed && stored != reversed {
        return Err(Error::Filter(
            "data error detected by fletcher32 checksum".into(),
        ));
    }
    Ok(body.to_vec())
}
