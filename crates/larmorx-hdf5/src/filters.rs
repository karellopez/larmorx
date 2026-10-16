// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Filters of a dataset's pipeline, in the read direction (`H5Z_pipeline` with
//! `H5Z_FLAG_REVERSE`, HDF5 1.12.1 `src/H5Z.c`): deflate (`src/H5Zdeflate.c`), shuffle
//! (`src/H5Zshuffle.c`), Fletcher-32 (`src/H5Zfletcher32.c`), n-bit (`src/H5Znbit.c`) and
//! scale-offset (`src/H5Zscaleoffset.c`): the filters an HDF5 built without szip (ITK's)
//! applies.

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
            NBIT => nbit(f, data)?,
            SCALEOFFSET => scaleoffset(f, data)?,
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

// ------------------------------------------------------------------------------------------------
// N-bit (`src/H5Znbit.c`)

/// `H5Z_NBIT_ATOMIC`, `H5Z_NBIT_ARRAY`, `H5Z_NBIT_COMPOUND`, `H5Z_NBIT_NOOPTYPE`.
const NBIT_ATOMIC: u32 = 1;
const NBIT_ARRAY: u32 = 2;
const NBIT_COMPOUND: u32 = 3;
const NBIT_NOOPTYPE: u32 = 4;

/// One atomic type's n-bit parameters (`parms_atomic`).
#[derive(Clone, Copy)]
struct NbitAtomic {
    size: usize,
    big_endian: bool,
    precision: usize,
    offset: usize,
}

/// The packed bits being read: the byte index and the bits left in it (`j`, `buf_len`).
struct NbitReader<'a> {
    buffer: &'a [u8],
    j: usize,
    buf_len: usize,
}

impl NbitReader<'_> {
    fn byte(&self) -> Result<u32> {
        self.buffer
            .get(self.j)
            .map(|&b| u32::from(b))
            .ok_or_else(|| Error::Filter("nbit: compressed data shorter than its elements".into()))
    }

    fn next_byte(&mut self) {
        self.j += 1;
        self.buf_len = 8;
    }
}

/// `~(~0 << n)` for `n` up to 8 bits, in `unsigned` arithmetic.
fn low_bits(n: usize) -> u32 {
    if n >= 32 { u32::MAX } else { !(u32::MAX << n) }
}

/// `H5Z__nbit_decompress_one_byte` (its arguments, as in HDF5).
#[allow(clippy::too_many_arguments)]
fn nbit_one_byte(
    data: &mut [u8],
    at: usize,
    k: usize,
    begin_i: usize,
    end_i: usize,
    r: &mut NbitReader<'_>,
    p: &NbitAtomic,
    datatype_len: usize,
) -> Result<()> {
    let mut val = r.byte()?;
    let mut dat_offset = 0usize;
    let mut dat_len;
    if begin_i != end_i {
        if k == begin_i {
            dat_len = 8 - (datatype_len - p.precision - p.offset) % 8;
        } else if k == end_i {
            dat_len = 8 - p.offset % 8;
            dat_offset = 8 - dat_len;
        } else {
            dat_len = 8;
        }
    } else {
        dat_offset = p.offset % 8;
        dat_len = p.precision;
    }
    if r.buf_len > dat_len {
        data[at + k] = (((val >> (r.buf_len - dat_len)) & low_bits(dat_len)) << dat_offset) as u8;
        r.buf_len -= dat_len;
    } else {
        data[at + k] = (((val & low_bits(r.buf_len)) << (dat_len - r.buf_len)) << dat_offset) as u8;
        dat_len -= r.buf_len;
        r.next_byte();
        if dat_len == 0 {
            return Ok(());
        }
        val = r.byte()?;
        data[at + k] |= (((val >> (r.buf_len - dat_len)) & low_bits(dat_len)) << dat_offset) as u8;
        r.buf_len -= dat_len;
    }
    Ok(())
}

/// `H5Z__nbit_decompress_one_nooptype`: whole bytes.
fn nbit_nooptype(data: &mut [u8], at: usize, r: &mut NbitReader<'_>, size: usize) -> Result<()> {
    for i in 0..size {
        let val = r.byte()?;
        let mut dat_len = 8usize;
        data[at + i] = ((val & low_bits(r.buf_len)) << (dat_len - r.buf_len)) as u8;
        dat_len -= r.buf_len;
        r.next_byte();
        if dat_len == 0 {
            continue;
        }
        let val = r.byte()?;
        data[at + i] |= ((val >> (r.buf_len - dat_len)) & low_bits(dat_len)) as u8;
        r.buf_len -= dat_len;
    }
    Ok(())
}

/// `H5Z__nbit_decompress_one_atomic`: the significant bytes, most significant first.
fn nbit_atomic(data: &mut [u8], at: usize, r: &mut NbitReader<'_>, p: &NbitAtomic) -> Result<()> {
    let datatype_len = p.size * 8;
    if at + p.size > data.len() {
        return Err(Error::Filter("nbit: an element beyond the chunk".into()));
    }
    if p.big_endian {
        let begin_i = (datatype_len - p.precision - p.offset) / 8;
        let end_i = if !p.offset.is_multiple_of(8) {
            (datatype_len - p.offset) / 8
        } else {
            (datatype_len - p.offset) / 8 - 1
        };
        for k in begin_i..=end_i {
            nbit_one_byte(data, at, k, begin_i, end_i, r, p, datatype_len)?;
        }
    } else {
        let begin_i = if !(p.precision + p.offset).is_multiple_of(8) {
            (p.precision + p.offset) / 8
        } else {
            (p.precision + p.offset) / 8 - 1
        };
        let end_i = p.offset / 8;
        for k in (end_i..=begin_i).rev() {
            nbit_one_byte(data, at, k, begin_i, end_i, r, p, datatype_len)?;
        }
    }
    Ok(())
}

/// The parameters, read in turn (`parms[(*parms_index)++]`).
struct NbitParms<'a> {
    parms: &'a [u32],
    index: usize,
}

impl NbitParms<'_> {
    fn next(&mut self) -> Result<u32> {
        let v = self.peek()?;
        self.index += 1;
        Ok(v)
    }

    fn peek(&self) -> Result<u32> {
        self.parms
            .get(self.index)
            .copied()
            .ok_or_else(|| Error::Filter("nbit: too few filter parameters".into()))
    }

    fn atomic(&mut self, size: usize) -> Result<NbitAtomic> {
        let p = NbitAtomic {
            size,
            big_endian: self.next()? == 1,
            precision: self.next()? as usize,
            offset: self.next()? as usize,
        };
        if p.precision > p.size * 8 || p.precision + p.offset > p.size * 8 || p.precision == 0 {
            return Err(Error::Filter("invalid datatype precision/offset".into()));
        }
        Ok(p)
    }
}

/// `H5Z__nbit_decompress_one_array`.
fn nbit_array(
    data: &mut [u8],
    at: usize,
    r: &mut NbitReader<'_>,
    q: &mut NbitParms<'_>,
) -> Result<()> {
    let total_size = q.next()? as usize;
    match q.next()? {
        NBIT_ATOMIC => {
            let size = q.next()? as usize;
            let p = q.atomic(size)?;
            for i in 0..total_size / p.size.max(1) {
                nbit_atomic(data, at + i * p.size, r, &p)?;
            }
        }
        NBIT_ARRAY => {
            let base_size = (q.peek()? as usize).max(1);
            let begin = q.index;
            for i in 0..total_size / base_size {
                nbit_array(data, at + i * base_size, r, q)?;
                q.index = begin;
            }
        }
        NBIT_COMPOUND => {
            let base_size = (q.peek()? as usize).max(1);
            let begin = q.index;
            for i in 0..total_size / base_size {
                nbit_compound(data, at + i * base_size, r, q)?;
                q.index = begin;
            }
        }
        NBIT_NOOPTYPE => {
            q.next()?;
            nbit_nooptype(data, at, r, total_size)?;
        }
        _ => return Err(Error::Filter("nbit: unknown datatype class".into())),
    }
    Ok(())
}

/// `H5Z__nbit_decompress_one_compound`.
fn nbit_compound(
    data: &mut [u8],
    at: usize,
    r: &mut NbitReader<'_>,
    q: &mut NbitParms<'_>,
) -> Result<()> {
    let size = q.next()? as usize;
    let nmembers = q.next()?;
    let mut used = 0usize;
    for _ in 0..nmembers {
        let member_offset = q.next()? as usize;
        let member_class = q.next()?;
        let member_size = q.peek()? as usize;
        used += member_size;
        if used > size {
            return Err(Error::Filter(
                "compound member offset overflowed compound size".into(),
            ));
        }
        match member_class {
            NBIT_ATOMIC => {
                q.next()?;
                let p = q.atomic(member_size)?;
                nbit_atomic(data, at + member_offset, r, &p)?;
            }
            NBIT_ARRAY => nbit_array(data, at + member_offset, r, q)?,
            NBIT_COMPOUND => nbit_compound(data, at + member_offset, r, q)?,
            NBIT_NOOPTYPE => {
                q.next()?;
                nbit_nooptype(data, at + member_offset, r, member_size)?;
            }
            _ => return Err(Error::Filter("nbit: unknown datatype class".into())),
        }
    }
    Ok(())
}

/// `H5Z__filter_nbit`, reading (`H5Z__nbit_decompress`).
fn nbit(f: &Filter, data: Vec<u8>) -> Result<Vec<u8>> {
    let parms: Vec<u32> = f.cd_values.clone();
    if parms.is_empty() || parms[0] as usize != parms.len() {
        return Err(Error::Filter("invalid nbit aggression level".into()));
    }
    if parms.get(1).copied().unwrap_or(0) != 0 {
        // Nothing to undo: the type was not packed.
        return Ok(data);
    }
    if parms.len() < 5 {
        return Err(Error::Filter("nbit: too few filter parameters".into()));
    }
    let d_nelmts = parms[2] as usize;
    let size = parms[4] as usize;
    let mut out = vec![0u8; d_nelmts * size];
    let mut r = NbitReader {
        buffer: &data,
        j: 0,
        buf_len: 8,
    };
    let mut q = NbitParms {
        parms: &parms,
        index: 4,
    };
    match parms[3] {
        NBIT_ATOMIC => {
            q.index = 5;
            let p = q.atomic(size)?;
            for i in 0..d_nelmts {
                nbit_atomic(&mut out, i * size, &mut r, &p)?;
            }
        }
        NBIT_ARRAY => {
            for i in 0..d_nelmts {
                q.index = 4;
                nbit_array(&mut out, i * size, &mut r, &mut q)?;
            }
        }
        NBIT_COMPOUND => {
            for i in 0..d_nelmts {
                q.index = 4;
                nbit_compound(&mut out, i * size, &mut r, &mut q)?;
            }
        }
        _ => return Err(Error::Filter("nbit: unknown datatype class".into())),
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------------
// Scale-offset (`src/H5Zscaleoffset.c`)

/// `H5Z_SCALEOFFSET_TOTAL_NPARMS`, the classes, the scale types.
const SO_NPARMS: usize = 20;
const SO_CLS_INTEGER: u32 = 0;
const SO_CLS_FLOAT: u32 = 1;
const SO_FLOAT_DSCALE: u32 = 0;
const SO_FLOAT_ESCALE: u32 = 1;
const SO_INT: u32 = 2;

/// `H5Z__scaleoffset_decompress_one_byte` (memory little-endian).
fn so_one_byte(
    data: &mut [u8],
    at: usize,
    k: usize,
    begin_i: usize,
    r: &mut NbitReader<'_>,
    minbits: usize,
    dtype_len: usize,
) -> Result<()> {
    let mut val = r.byte()?;
    let mut dat_len = if k == begin_i {
        8 - (dtype_len - minbits) % 8
    } else {
        8
    };
    if r.buf_len > dat_len {
        data[at + k] = ((val >> (r.buf_len - dat_len)) & low_bits(dat_len)) as u8;
        r.buf_len -= dat_len;
    } else {
        data[at + k] = ((val & low_bits(r.buf_len)) << (dat_len - r.buf_len)) as u8;
        dat_len -= r.buf_len;
        r.next_byte();
        if dat_len == 0 {
            return Ok(());
        }
        val = r.byte()?;
        data[at + k] |= ((val >> (r.buf_len - dat_len)) & low_bits(dat_len)) as u8;
        r.buf_len -= dat_len;
    }
    Ok(())
}

/// The value of `size` little-endian bytes as a signed integer of that width.
fn so_signed(bytes: &[u8]) -> i64 {
    let mut b = [0u8; 8];
    b[..bytes.len()].copy_from_slice(bytes);
    let v = u64::from_le_bytes(b);
    let shift = 64 - 8 * bytes.len() as u32;
    ((v << shift) as i64) >> shift
}

/// `H5Z__filter_scaleoffset`, reading: the packed integers unpacked (`minbits` bits each,
/// most significant first), then the minimum added back (integers) or the decimal scaling
/// undone (`H5Z_SO_FLOAT_DSCALE`: `x / 10^D + min`), the fill value restored where the
/// all-ones pattern marks it. On a little-endian machine, as HDF5 decodes there.
fn scaleoffset(f: &Filter, data: Vec<u8>) -> Result<Vec<u8>> {
    let cd = &f.cd_values;
    if cd.len() != SO_NPARMS {
        return Err(Error::Filter(
            "invalid scaleoffset number of parameters".into(),
        ));
    }
    let scale_type = cd[0];
    let mut scale_factor = cd[1] as i32;
    let d_nelmts = cd[2] as usize;
    let class = cd[3];
    let size = cd[4] as usize;
    let signed = cd[5] == 1;
    let big_endian = cd[6] == 1;
    let filavail = cd[7] == 1;
    if class == SO_CLS_FLOAT && scale_type != SO_FLOAT_DSCALE && scale_type != SO_FLOAT_ESCALE {
        return Err(Error::Filter("invalid scale type".into()));
    }
    if class == SO_CLS_INTEGER {
        if scale_type != SO_INT {
            return Err(Error::Filter("invalid scale type".into()));
        }
        scale_factor = scale_factor.max(0);
    }
    if scale_type == SO_FLOAT_ESCALE {
        return Err(Error::Filter("E-scaling method not supported".into()));
    }
    let valid_size = match class {
        SO_CLS_INTEGER => matches!(size, 1 | 2 | 4 | 8),
        SO_CLS_FLOAT => matches!(size, 4 | 8),
        _ => false,
    };
    if !valid_size {
        return Err(Error::Filter("cannot find matched memory dataype".into()));
    }
    if scale_type != SO_FLOAT_DSCALE {
        if scale_factor as usize > size * 8 {
            return Err(Error::Filter(
                "minimum number of bits exceeds maximum".into(),
            ));
        }
        if scale_factor as usize == size * 8 {
            return Ok(data);
        }
    }
    if data.len() < 21 {
        return Err(Error::Filter(
            "scaleoffset: compressed data too short".into(),
        ));
    }
    let minbits = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let minval_size = usize::from(data[4]).min(8);
    let mut mb = [0u8; 8];
    mb[..minval_size].copy_from_slice(&data[5..5 + minval_size]);
    let minval = u64::from_le_bytes(mb);
    if minbits > size * 8 {
        return Err(Error::Filter(
            "scaleoffset: minimum bits beyond the type".into(),
        ));
    }
    let out_len = d_nelmts * size;
    let swap = |out: &mut Vec<u8>| {
        if big_endian {
            for e in out.chunks_exact_mut(size) {
                e.reverse();
            }
        }
    };
    if minbits == size * 8 {
        let mut out = data
            .get(21..21 + out_len)
            .ok_or_else(|| Error::Filter("scaleoffset: compressed data too short".into()))?
            .to_vec();
        swap(&mut out);
        return Ok(out);
    }
    let mut out = vec![0u8; out_len];
    if minbits != 0 {
        let mut r = NbitReader {
            buffer: &data[21..],
            j: 0,
            buf_len: 8,
        };
        let dtype_len = size * 8;
        let begin_i = size - 1 - (dtype_len - minbits) / 8;
        for i in 0..d_nelmts {
            for k in (0..=begin_i).rev() {
                so_one_byte(&mut out, i * size, k, begin_i, &mut r, minbits, dtype_len)?;
            }
        }
    }
    // The fill value, little-endian, in the parameters from the ninth on.
    let mut fill = [0u8; 8];
    for (k, b) in fill.iter_mut().take(size).enumerate() {
        *b = cd[8 + k / 4].to_le_bytes()[k % 4];
    }
    let all_ones = |e: &[u8]| -> bool {
        // `buf[i] == ((type)1 << minbits) - 1` in the element's own width.
        let mut b = [0u8; 8];
        b[..size].copy_from_slice(e);
        let v = u64::from_le_bytes(b);
        let mask = if size == 8 {
            u64::MAX
        } else {
            (1u64 << (8 * size)) - 1
        };
        let ones = if minbits >= 64 {
            u64::MAX
        } else {
            (1u64 << minbits).wrapping_sub(1)
        } & mask;
        v == ones
    };
    if class == SO_CLS_INTEGER {
        for e in out.chunks_exact_mut(size) {
            let value: u64 = if filavail && all_ones(e) {
                let mut b = [0u8; 8];
                b[..size].copy_from_slice(&fill[..size]);
                u64::from_le_bytes(b)
            } else {
                let mut b = [0u8; 8];
                b[..size].copy_from_slice(e);
                let v = u64::from_le_bytes(b);
                if signed {
                    (so_signed(e).wrapping_add(minval as i64)) as u64
                } else {
                    v.wrapping_add(minval)
                }
            };
            e.copy_from_slice(&value.to_le_bytes()[..size]);
        }
    } else if size == 4 {
        // H5Z_scaleoffset_postdecompress_3(float, HDpowf, ...).
        let min = f32::from_le_bytes([mb[0], mb[1], mb[2], mb[3]]);
        let fillv = f32::from_le_bytes([fill[0], fill[1], fill[2], fill[3]]);
        let scale = larmorx_core::math::powf(10.0, scale_factor as f32);
        for e in out.as_chunks_mut::<4>().0 {
            let v = if filavail && all_ones(e) {
                fillv
            } else {
                let int = i32::from_le_bytes([e[0], e[1], e[2], e[3]]);
                int as f32 / scale + min
            };
            e.copy_from_slice(&v.to_le_bytes());
        }
    } else {
        // H5Z_scaleoffset_postdecompress_3(double, HDpow, ...).
        let min = f64::from_le_bytes(mb);
        let fillv = f64::from_le_bytes(fill);
        let scale = pow10(scale_factor);
        for e in out.as_chunks_mut::<8>().0 {
            let v = if filavail && all_ones(e) {
                fillv
            } else {
                let int = i64::from_le_bytes(*e);
                int as f64 / scale + min
            };
            e.copy_from_slice(&v.to_le_bytes());
        }
    }
    swap(&mut out);
    Ok(out)
}

/// `pow(10.0, d)` for an integer `d`, correctly rounded where |d| ≤ 22 (10^|d| is then exact,
/// and so is its reciprocal's single rounding), as glibc's `pow` gives it.
fn pow10(d: i32) -> f64 {
    let k = d.unsigned_abs();
    let p = larmorx_core::math::powi(10.0, k);
    if d >= 0 { p } else { 1.0 / p }
}
