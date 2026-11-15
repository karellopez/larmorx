// SPDX-License-Identifier: Apache-2.0 AND IJG
//! IJG 6b's compressor for one 8-bit grey component after `jpeg_set_defaults` (this software is
//! based in part on the work of the Independent JPEG Group; `LICENSES/IJG.txt`): `jcparam.c`'s
//! defaults (quality 75 with `force_baseline`, the standard Huffman tables, a JFIF 1.01 header),
//! `jcmaster.c`'s checks, `jcprepct.c`'s and `jcsample.c`'s edge expansion, `jcdctmgr.c`'s
//! quantization over `jfdctint.c`'s slow integer forward DCT, `jchuff.c`'s Huffman coding,
//! `jcmarker.c`'s markers and `jdatadst.c`'s 4,096-byte output buffer.

use std::io::Write;

use larmorx_core::cfmt::Streams;

use super::msg::*;

pub(super) const NATURAL_ORDER: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

pub(super) const STD_LUMINANCE_QUANT: [u32; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

pub(super) const BITS_DC_LUMINANCE: [u8; 17] = [0, 0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
pub(super) const VAL_DC_LUMINANCE: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
pub(super) const BITS_AC_LUMINANCE: [u8; 17] =
    [0, 0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
#[rustfmt::skip]
pub(super) const VAL_AC_LUMINANCE: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

const OUTPUT_BUF_SIZE: usize = 4096;

/// `c_derived_tbl`: each symbol's code and length.
struct CDerived {
    ehufco: [u32; 256],
    ehufsi: [u8; 256],
}

/// `jpeg_make_c_derived_tbl` for the standard tables (valid: no check can fail).
fn c_derived(bits: &[u8; 17], val: &[u8]) -> CDerived {
    let mut huffsize = Vec::with_capacity(257);
    for (l, &n) in bits.iter().enumerate().skip(1) {
        for _ in 0..n {
            huffsize.push(l as u32);
        }
    }
    let mut huffcode = vec![0u32; huffsize.len()];
    let mut code = 0u32;
    let mut si = huffsize.first().copied().unwrap_or(0);
    let mut p = 0;
    while p < huffsize.len() {
        while p < huffsize.len() && huffsize[p] == si {
            huffcode[p] = code;
            p += 1;
            code += 1;
        }
        code <<= 1;
        si += 1;
    }
    let mut t = CDerived {
        ehufco: [0; 256],
        ehufsi: [0; 256],
    };
    for p in 0..huffsize.len() {
        t.ehufco[val[p] as usize] = huffcode[p];
        t.ehufsi[val[p] as usize] = huffsize[p] as u8;
    }
    t
}

/// `jpeg_stdio_dest`: bytes go out in blocks of 4,096 (`empty_output_buffer`), the rest at the
/// end (`term_destination`, with `fflush` and the `ferror` check).
struct Dest<'w> {
    out: &'w mut dyn Write,
    buf: Vec<u8>,
    failed: bool,
}

impl Dest<'_> {
    fn byte(&mut self, v: u8) {
        self.buf.push(v);
        if self.buf.len() == OUTPUT_BUF_SIZE {
            if !self.failed && self.out.write_all(&self.buf).is_err() {
                self.failed = true;
            }
            self.buf.clear();
        }
    }
    fn two(&mut self, v: u32) {
        self.byte(((v >> 8) & 0xFF) as u8);
        self.byte((v & 0xFF) as u8);
    }
    fn marker(&mut self, m: u8) {
        self.byte(0xFF);
        self.byte(m);
    }
}

/// The Huffman coder's state (`savable_state`, `put_buffer`, `put_bits`).
struct Bits {
    put_buffer: u64,
    put_bits: i32,
}

/// `emit_bits`.
fn emit_bits(dest: &mut Dest, st: &mut Bits, code: u32, size: i32) {
    let mut put_buffer = (code as u64) & ((1u64 << size) - 1);
    let mut put_bits = st.put_bits + size;
    put_buffer <<= 24 - put_bits;
    put_buffer |= st.put_buffer;
    while put_bits >= 8 {
        let c = ((put_buffer >> 16) & 0xFF) as u8;
        dest.byte(c);
        if c == 0xFF {
            dest.byte(0);
        }
        put_buffer <<= 8;
        put_bits -= 8;
    }
    st.put_buffer = put_buffer;
    st.put_bits = put_bits;
}

/// `jpeg_fdct_islow` (8 x 8 in place, `DCTELEM` = int).
pub(super) fn fdct_islow(data: &mut [i32; 64]) {
    const CONST_BITS: i64 = 13;
    const PASS1_BITS: i64 = 2;
    let descale = |x: i64, n: i64| (x + (1i64 << (n - 1))) >> n;
    const F0298: i64 = 2446;
    const F0390: i64 = 3196;
    const F0541: i64 = 4433;
    const F0765: i64 = 6270;
    const F0899: i64 = 7373;
    const F1175: i64 = 9633;
    const F1501: i64 = 12299;
    const F1847: i64 = 15137;
    const F1961: i64 = 16069;
    const F2053: i64 = 16819;
    const F2562: i64 = 20995;
    const F3072: i64 = 25172;
    for pass in 0..2 {
        for i in 0..8 {
            // pass 0: rows (stride 1, base 8 i); pass 1: columns (stride 8, base i)
            let (base, step) = if pass == 0 { (8 * i, 1) } else { (i, 8) };
            let d = |k: usize| data[base + k * step] as i64;
            let tmp0 = d(0) + d(7);
            let mut tmp7 = d(0) - d(7);
            let tmp1 = d(1) + d(6);
            let mut tmp6 = d(1) - d(6);
            let tmp2 = d(2) + d(5);
            let mut tmp5 = d(2) - d(5);
            let tmp3 = d(3) + d(4);
            let mut tmp4 = d(3) - d(4);
            let tmp10 = tmp0 + tmp3;
            let tmp13 = tmp0 - tmp3;
            let tmp11 = tmp1 + tmp2;
            let tmp12 = tmp1 - tmp2;
            let z1 = (tmp12 + tmp13) * F0541;
            let (o0, o4, o2, o6);
            if pass == 0 {
                o0 = ((tmp10 + tmp11) << PASS1_BITS) as i32;
                o4 = ((tmp10 - tmp11) << PASS1_BITS) as i32;
                o2 = descale(z1 + tmp13 * F0765, CONST_BITS - PASS1_BITS) as i32;
                o6 = descale(z1 + tmp12 * -F1847, CONST_BITS - PASS1_BITS) as i32;
            } else {
                o0 = descale(tmp10 + tmp11, PASS1_BITS) as i32;
                o4 = descale(tmp10 - tmp11, PASS1_BITS) as i32;
                o2 = descale(z1 + tmp13 * F0765, CONST_BITS + PASS1_BITS) as i32;
                o6 = descale(z1 + tmp12 * -F1847, CONST_BITS + PASS1_BITS) as i32;
            }
            let z1 = tmp4 + tmp7;
            let z2 = tmp5 + tmp6;
            let z3 = tmp4 + tmp6;
            let z4 = tmp5 + tmp7;
            let z5 = (z3 + z4) * F1175;
            tmp4 *= F0298;
            tmp5 *= F2053;
            tmp6 *= F3072;
            tmp7 *= F1501;
            let z1 = z1 * -F0899;
            let z2 = z2 * -F2562;
            let z3 = z3 * -F1961 + z5;
            let z4 = z4 * -F0390 + z5;
            let n = if pass == 0 {
                CONST_BITS - PASS1_BITS
            } else {
                CONST_BITS + PASS1_BITS
            };
            let o7 = descale(tmp4 + z1 + z3, n) as i32;
            let o5 = descale(tmp5 + z2 + z4, n) as i32;
            let o3 = descale(tmp6 + z2 + z3, n) as i32;
            let o1 = descale(tmp7 + z1 + z4, n) as i32;
            for (k, v) in [o0, o1, o2, o3, o4, o5, o6, o7].into_iter().enumerate() {
                data[base + k * step] = v;
            }
        }
    }
}

/// The compressor for one 8-bit grey component after `jpeg_set_defaults` (FreeSurfer's
/// `JPEGWriteImage`): `image_width` x `image_height` samples, row `r` (0 first, as
/// `jpeg_write_scanlines` takes them) written into the buffer `row` gives; the file's bytes go to
/// `out`. A write that fails is IJG's `Output file write error --- out of disk space?`.
pub fn encode_gray(
    image_width: u32,
    image_height: u32,
    row: &mut dyn FnMut(u32, &mut [u8]),
    out: &mut dyn Write,
    streams: &mut dyn Streams,
) -> Result<(), Fatal> {
    let mut err = ErrMgr::default();
    // jcmaster.c's initial_setup
    if image_height == 0 || image_width == 0 {
        return Err(err.fatal(streams, JERR_EMPTY_IMAGE, &[]));
    }
    if image_height > 65500 || image_width > 65500 {
        return Err(err.fatal(streams, JERR_IMAGE_TOO_BIG, &[65500]));
    }
    // jpeg_set_quality(75, TRUE): scale factor 50
    let mut quant = [0u32; 64];
    for (q, &b) in quant.iter_mut().zip(STD_LUMINANCE_QUANT.iter()) {
        *q = ((b as i64 * 50 + 50) / 100).clamp(1, 255) as u32;
    }
    let divisors: Vec<i32> = quant.iter().map(|&q| (q as i32) << 3).collect();
    let dc = c_derived(&BITS_DC_LUMINANCE, &VAL_DC_LUMINANCE);
    let ac = c_derived(&BITS_AC_LUMINANCE, &VAL_AC_LUMINANCE);
    let mut dest = Dest {
        out,
        buf: Vec::with_capacity(OUTPUT_BUF_SIZE),
        failed: false,
    };
    // write_file_header: SOI, the JFIF APP0 marker
    dest.marker(0xD8);
    dest.marker(0xE0);
    dest.two(16);
    for &b in b"JFIF\0" {
        dest.byte(b);
    }
    dest.byte(1);
    dest.byte(1);
    dest.byte(0);
    dest.two(1);
    dest.two(1);
    dest.byte(0);
    dest.byte(0);
    // write_frame_header: DQT 0, SOF0
    dest.marker(0xDB);
    dest.two(64 + 1 + 2);
    dest.byte(0);
    for &pos in NATURAL_ORDER.iter() {
        dest.byte((quant[pos] & 0xFF) as u8);
    }
    dest.marker(0xC0);
    dest.two(3 + 2 + 5 + 1);
    dest.byte(8);
    dest.two(image_height);
    dest.two(image_width);
    dest.byte(1);
    dest.byte(1);
    dest.byte(0x11);
    dest.byte(0);
    // write_scan_header: DHT DC 0, DHT AC 0, SOS
    for (index, bits, val) in [
        (0u8, &BITS_DC_LUMINANCE, &VAL_DC_LUMINANCE[..]),
        (0x10, &BITS_AC_LUMINANCE, &VAL_AC_LUMINANCE[..]),
    ] {
        dest.marker(0xC4);
        let length: u32 = bits[1..].iter().map(|&b| b as u32).sum();
        dest.two(length + 2 + 1 + 16);
        dest.byte(index);
        for &b in &bits[1..] {
            dest.byte(b);
        }
        for &v in val.iter().take(length as usize) {
            dest.byte(v);
        }
    }
    dest.marker(0xDA);
    dest.two(2 + 2 + 1 + 3);
    dest.byte(1);
    dest.byte(1);
    dest.byte(0);
    dest.byte(0);
    dest.byte(63);
    dest.byte(0);
    // the data: each iMCU row (8 sample rows, the last ones repeated below the image, each row
    // continued by its last sample) as blocks, quantized and coded
    let w = image_width as usize;
    let wib = w.div_ceil(8);
    let pw = wib * 8;
    let mut rows = vec![0u8; pw * 8];
    let mut st = Bits {
        put_buffer: 0,
        put_bits: 0,
    };
    let mut last_dc = 0i32;
    let total = (image_height as usize).div_ceil(8);
    let mut scratch = vec![0u8; w];
    for imcu in 0..total {
        let mut filled = 0;
        for r in 0..8 {
            let y = imcu * 8 + r;
            if y >= image_height as usize {
                break;
            }
            row(y as u32, &mut scratch);
            let dst = &mut rows[r * pw..(r + 1) * pw];
            dst[..w].copy_from_slice(&scratch[..w]);
            let last = dst[w - 1];
            dst[w..].fill(last);
            filled = r + 1;
        }
        for r in filled..8 {
            rows.copy_within((filled - 1) * pw..filled * pw, r * pw);
        }
        for b in 0..wib {
            let mut ws = [0i32; 64];
            for r in 0..8 {
                for c in 0..8 {
                    ws[r * 8 + c] = rows[r * pw + b * 8 + c] as i32 - 128;
                }
            }
            fdct_islow(&mut ws);
            let mut coef = [0i32; 64];
            for i in 0..64 {
                let qval = divisors[i];
                let mut temp = ws[i];
                if temp < 0 {
                    temp = -temp;
                    temp += qval >> 1;
                    temp = if temp >= qval { temp / qval } else { 0 };
                    temp = -temp;
                } else {
                    temp += qval >> 1;
                    temp = if temp >= qval { temp / qval } else { 0 };
                }
                coef[i] = temp as i16 as i32;
            }
            encode_one_block(
                &mut dest, &mut st, &coef, last_dc, &dc, &ac, &mut err, streams,
            )?;
            last_dc = coef[0];
        }
    }
    // finish_pass_huff's flush_bits, write_file_trailer
    emit_bits(&mut dest, &mut st, 0x7F, 7);
    dest.marker(0xD9);
    // term_destination
    let rest = std::mem::take(&mut dest.buf);
    if !dest.failed && !rest.is_empty() && dest.out.write_all(&rest).is_err() {
        dest.failed = true;
    }
    if !dest.failed && dest.out.flush().is_err() {
        dest.failed = true;
    }
    if dest.failed {
        return Err(err.fatal(streams, JERR_FILE_WRITE, &[]));
    }
    Ok(())
}

/// `encode_one_block`.
#[allow(clippy::too_many_arguments)]
fn encode_one_block(
    dest: &mut Dest,
    st: &mut Bits,
    block: &[i32; 64],
    last_dc_val: i32,
    dctbl: &CDerived,
    actbl: &CDerived,
    err: &mut ErrMgr,
    streams: &mut dyn Streams,
) -> Result<(), Fatal> {
    let mut temp = block[0] - last_dc_val;
    let mut temp2 = temp;
    if temp < 0 {
        temp = -temp;
        temp2 -= 1;
    }
    let mut nbits = 0;
    while temp != 0 {
        nbits += 1;
        temp >>= 1;
    }
    if nbits > 11 {
        return Err(err.fatal(streams, JERR_BAD_DCT_COEF, &[]));
    }
    emit_code(dest, st, dctbl, nbits as usize, err, streams)?;
    if nbits != 0 {
        emit_bits(dest, st, temp2 as u32, nbits);
    }
    let mut r = 0;
    for &pos in NATURAL_ORDER.iter().skip(1) {
        let mut temp = block[pos];
        if temp == 0 {
            r += 1;
            continue;
        }
        while r > 15 {
            emit_code(dest, st, actbl, 0xF0, err, streams)?;
            r -= 16;
        }
        let mut temp2 = temp;
        if temp < 0 {
            temp = -temp;
            temp2 -= 1;
        }
        let mut nbits = 1;
        loop {
            temp >>= 1;
            if temp == 0 {
                break;
            }
            nbits += 1;
        }
        if nbits > 10 {
            return Err(err.fatal(streams, JERR_BAD_DCT_COEF, &[]));
        }
        emit_code(dest, st, actbl, ((r << 4) + nbits) as usize, err, streams)?;
        emit_bits(dest, st, temp2 as u32, nbits);
        r = 0;
    }
    if r > 0 {
        emit_code(dest, st, actbl, 0, err, streams)?;
    }
    Ok(())
}

/// `emit_bits` of a symbol's code (`JERR_HUFF_MISSING_CODE` for a symbol of length 0).
fn emit_code(
    dest: &mut Dest,
    st: &mut Bits,
    tbl: &CDerived,
    sym: usize,
    err: &mut ErrMgr,
    streams: &mut dyn Streams,
) -> Result<(), Fatal> {
    let size = tbl.ehufsi[sym] as i32;
    if size == 0 {
        return Err(err.fatal(streams, JERR_HUFF_MISSING_CODE, &[]));
    }
    emit_bits(dest, st, tbl.ehufco[sym], size);
    Ok(())
}
