// SPDX-License-Identifier: Apache-2.0
//! The generated files' data for the codecs whose encoders the port does not have (part of
//! [`super::cases`], for the unit tests and the probe): CCITT streams (Modified Huffman rows,
//! Group 3 one- and two-dimensional rows with their end-of-line codes, Group 4 rows, RLE/W
//! alignment) from bilevel images with run structure, NeXT and ThunderScan streams, valid or
//! now and then not. Plain arithmetic only.

use super::cases::Rng;
use super::fax3::code_list;

/// A bit writer, most significant bit first.
pub struct BitW {
    pub out: Vec<u8>,
    acc: u8,
    n: u32,
}

impl Default for BitW {
    fn default() -> BitW {
        BitW::new()
    }
}

impl BitW {
    pub fn new() -> BitW {
        BitW {
            out: Vec::new(),
            acc: 0,
            n: 0,
        }
    }
    pub fn bit(&mut self, b: bool) {
        self.acc = (self.acc << 1) | u8::from(b);
        self.n += 1;
        if self.n == 8 {
            self.out.push(self.acc);
            self.acc = 0;
            self.n = 0;
        }
    }
    /// A code given least significant bit first (mkg3states' lists).
    pub fn lsb(&mut self, code: u16, width: u8) {
        for k in 0..width {
            self.bit((code >> k) & 1 != 0);
        }
    }
    /// A code given most significant bit first.
    pub fn msb(&mut self, code: u32, width: u32) {
        for k in (0..width).rev() {
            self.bit((code >> k) & 1 != 0);
        }
    }
    /// Zero bits to the next multiple of `bits` bits.
    pub fn align(&mut self, bits: u32) {
        while (self.out.len() as u32 * 8 + self.n) % bits != 0 {
            self.bit(false);
        }
    }
    pub fn bits(&self) -> u64 {
        self.out.len() as u64 * 8 + u64::from(self.n)
    }
    pub fn finish(mut self) -> Vec<u8> {
        self.align(8);
        self.out
    }
}

/// The run codes of one colour: `(run, code, width)`, terminating and make-up codes.
struct Codes {
    term: Vec<(u16, u8)>,
    makeup: Vec<(u32, u16, u8)>,
}

fn codes(black: bool) -> Codes {
    let term = code_list(if black { "termb" } else { "termw" })
        .into_iter()
        .map(|(c, w, _)| (c, w))
        .collect();
    let mut makeup: Vec<(u32, u16, u8)> = code_list(if black { "makeupb" } else { "makeupw" })
        .into_iter()
        .chain(code_list("makeup"))
        .map(|(c, w, r)| (r, c, w))
        .collect();
    makeup.sort_by_key(|m| m.0);
    Codes { term, makeup }
}

/// libtiff's `putspan`: make-up codes of 2560 while the span is 2624 or more, one make-up
/// code, the terminating code.
pub fn put_span(w: &mut BitW, mut span: u32, black: bool) {
    let c = codes(black);
    let find = |r: u32| *c.makeup.iter().find(|m| m.0 == r).expect("make-up code");
    while span >= 2624 {
        let (r, code, wd) = find(2560);
        w.lsb(code, wd);
        span -= r;
    }
    if span >= 64 {
        let (r, code, wd) = find(span >> 6 << 6);
        w.lsb(code, wd);
        span -= r;
    }
    let (code, wd) = c.term[span as usize];
    w.lsb(code, wd);
}

/// The end-of-line code, 000000000001.
pub fn eol(w: &mut BitW) {
    w.msb(1, 12);
}

/// A bilevel image of `rows` rows of `width` pixels (true: black), rows like their neighbours
/// (edges moved a little, now and then new runs), sometimes noise.
pub fn bilevel(r: &mut Rng, width: u32, rows: u32) -> Vec<Vec<bool>> {
    let w = width as usize;
    let mut out: Vec<Vec<bool>> = Vec::new();
    let noise = r.chance(6);
    let mut prev = vec![false; w];
    for _ in 0..rows {
        let mut row = vec![false; w];
        if noise {
            for p in &mut row {
                *p = r.chance(2);
            }
        } else if out.is_empty() || r.chance(5) {
            let mut x = 0usize;
            let mut black = r.chance(3);
            while x < w {
                let span = if r.chance(4) { 200 } else { 12 };
                let len = 1 + r.below(span) as usize;
                for p in row.iter_mut().skip(x).take(len) {
                    *p = black;
                }
                x += len;
                black = !black;
            }
        } else {
            row.clone_from(&prev);
            for _ in 0..r.below(4) {
                let x = r.below(w as u64) as usize;
                let len = 1 + r.below(4) as usize;
                let v = r.chance(2);
                for p in row.iter_mut().skip(x).take(len) {
                    *p = v;
                }
            }
        }
        prev.clone_from(&row);
        out.push(row);
    }
    out
}

/// The row's bytes (1 = black, most significant bit first), `rowbytes` long.
pub fn pack(row: &[bool], rowbytes: usize) -> Vec<u8> {
    let mut v = vec![0u8; rowbytes];
    for (x, &b) in row.iter().enumerate() {
        if b && x / 8 < rowbytes {
            v[x / 8] |= 0x80 >> (x % 8);
        }
    }
    v
}

fn pixel(row: &[bool], x: u32) -> bool {
    row.get(x as usize).copied().unwrap_or(false)
}

/// `finddiff`: the first position from `bs` whose pixel is not `color` (or `be`).
fn finddiff(row: &[bool], bs: u32, be: u32, color: bool) -> u32 {
    let mut x = bs;
    while x < be && pixel(row, x) == color {
        x += 1;
    }
    x
}

fn finddiff2(row: &[bool], bs: u32, be: u32, color: bool) -> u32 {
    if bs < be {
        finddiff(row, bs, be, color)
    } else {
        be
    }
}

/// A one-dimensional (Modified Huffman) row.
pub fn row_1d(w: &mut BitW, row: &[bool]) {
    let bits = row.len() as u32;
    let mut bs = 0;
    loop {
        let span = finddiff(row, bs, bits, false) - bs;
        put_span(w, span, false);
        bs += span;
        if bs >= bits {
            break;
        }
        let span = finddiff(row, bs, bits, true) - bs;
        put_span(w, span, true);
        bs += span;
        if bs >= bits {
            break;
        }
    }
}

/// A two-dimensional row against the reference row `rp` (libtiff's `Fax3Encode2DRow`).
pub fn row_2d(w: &mut BitW, bp: &[bool], rp: &[bool]) {
    const VCODES: [(u32, u32); 7] = [
        (0x03, 7),
        (0x03, 6),
        (0x03, 3),
        (0x1, 1),
        (0x2, 3),
        (0x02, 6),
        (0x02, 7),
    ];
    let bits = bp.len() as u32;
    let mut a0: u32 = 0;
    let mut a1 = if pixel(bp, 0) {
        0
    } else {
        finddiff(bp, 0, bits, false)
    };
    let mut b1 = if pixel(rp, 0) {
        0
    } else {
        finddiff(rp, 0, bits, false)
    };
    loop {
        let b2 = finddiff2(rp, b1, bits, pixel(rp, b1));
        if b2 >= a1 {
            let d = i64::from(b1) - i64::from(a1);
            if !(-3..=3).contains(&d) {
                let a2 = finddiff2(bp, a1, bits, pixel(bp, a1));
                w.msb(1, 3);
                if a0 + a1 == 0 || !pixel(bp, a0) {
                    put_span(w, a1 - a0, false);
                    put_span(w, a2 - a1, true);
                } else {
                    put_span(w, a1 - a0, true);
                    put_span(w, a2 - a1, false);
                }
                a0 = a2;
            } else {
                let (c, n) = VCODES[(d + 3) as usize];
                w.msb(c, n);
                a0 = a1;
            }
        } else {
            w.msb(1, 4);
            a0 = b2;
        }
        if a0 >= bits {
            break;
        }
        let c = pixel(bp, a0);
        a1 = finddiff(bp, a0, bits, c);
        b1 = finddiff(rp, a0, bits, !c);
        b1 = finddiff(rp, b1, bits, c);
    }
}

/// One strip or tile of CCITT data for `compression` (2, 3, 4, 32771): the rows of
/// `rowpixels` pixels, Group 3 options `g3` (bit 0: two-dimensional, bit 2: fill bits before
/// each end of line), now and then a stray code.
pub fn fax_strip(r: &mut Rng, compression: u16, g3: u32, rows: &[Vec<bool>]) -> Vec<u8> {
    let k = 1 + r.below(4);
    let stray = if r.chance(20) {
        Some(r.below(rows.len() as u64 + 1) as usize)
    } else {
        None
    };
    let code = r.below(128) as u32;
    let end = r.chance(2);
    fax_encode(compression, g3, k, rows, end, stray.map(|at| (at, code)))
}

/// The CCITT stream of `rows`: every `k`th row one-dimensional (Group 3 two-dimensional), the
/// end codes (Group 3's RTC, Group 4's EOFB) when `end`, a 7-bit code `stray.1` after row
/// `stray.0`.
pub fn fax_encode(
    compression: u16,
    g3: u32,
    k: u64,
    rows: &[Vec<bool>],
    end: bool,
    stray: Option<(usize, u32)>,
) -> Vec<u8> {
    let mut w = BitW::new();
    let width = rows.first().map_or(0, Vec::len);
    let mut refrow = vec![false; width];
    for (i, row) in rows.iter().enumerate() {
        match compression {
            2 => {
                row_1d(&mut w, row);
                w.align(8);
            }
            32771 => {
                row_1d(&mut w, row);
                w.align(16);
            }
            3 => {
                if g3 & 4 != 0 {
                    // the end of line ends on a byte boundary
                    while (w.bits() + 12) % 8 != 0 {
                        w.bit(false);
                    }
                }
                eol(&mut w);
                if g3 & 1 != 0 {
                    let one = i as u64 % k.max(1) == 0;
                    w.bit(one);
                    if one {
                        row_1d(&mut w, row);
                    } else {
                        row_2d(&mut w, row, &refrow);
                    }
                } else {
                    row_1d(&mut w, row);
                }
            }
            _ => row_2d(&mut w, row, &refrow),
        }
        refrow.clone_from(row);
        if let Some((at, code)) = stray
            && at == i
        {
            w.msb(code, 7);
        }
    }
    if end {
        match compression {
            3 => {
                for _ in 0..6 {
                    eol(&mut w);
                    if g3 & 1 != 0 {
                        w.bit(true);
                    }
                }
            }
            4 => {
                eol(&mut w);
                eol(&mut w);
            }
            _ => {}
        }
    }
    w.finish()
}

/// One strip of NeXT data: rows of `width` 2-bit pixels, `scanline` bytes each.
pub fn next_strip(r: &mut Rng, rows: u32, width: u32, scanline: usize) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..rows {
        match r.below(5) {
            0 => {
                v.push(0x00);
                for _ in 0..scanline {
                    v.push(r.next_u64() as u8);
                }
            }
            1 => {
                let off = r.below(scanline as u64 + 1) as usize;
                let n = if r.chance(8) {
                    r.below(scanline as u64 + 4) as usize
                } else {
                    r.below((scanline - off.min(scanline)) as u64 + 1) as usize
                };
                v.push(0x40);
                v.extend_from_slice(&[(off >> 8) as u8, off as u8, (n >> 8) as u8, n as u8]);
                for _ in 0..n {
                    v.push(r.next_u64() as u8);
                }
            }
            _ => {
                let mut left = i64::from(width);
                let mut first = true;
                while left > 0 {
                    let mut n = 1 + r.below(63) as i64;
                    if !r.chance(6) {
                        n = n.min(left);
                    }
                    let grey = r.below(4) as u8;
                    let mut b = (grey << 6) | n as u8;
                    if first && (b == 0x00 || b == 0x40) {
                        b |= 1;
                    }
                    first = false;
                    v.push(b);
                    left -= i64::from(b & 0x3f);
                }
            }
        }
    }
    if r.chance(6) && !v.is_empty() {
        let n = r.below(v.len() as u64) as usize;
        v.truncate(n);
    }
    v
}

/// One strip of ThunderScan data: rows of `width` 4-bit pixels.
pub fn thunder_strip(r: &mut Rng, rows: u32, width: u32) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..rows {
        let mut n: i64 = 0;
        let target = i64::from(width)
            + if r.chance(8) {
                r.below(5) as i64 - 2
            } else {
                0
            };
        while n < target {
            match r.below(5) {
                0 => {
                    let span = if r.chance(5) { 63 } else { 8 };
                    let c = 1 + r.below(span) as i64;
                    let c = if r.chance(5) { c } else { c.min(target - n) };
                    v.push(c as u8);
                    n += c;
                }
                1 => {
                    let d = r.below(64) as u8;
                    v.push(0x40 | d);
                    n += [(d >> 4) & 3, (d >> 2) & 3, d & 3]
                        .iter()
                        .filter(|&&x| x != 2)
                        .count() as i64;
                }
                2 => {
                    let d = r.below(64) as u8;
                    v.push(0x80 | d);
                    n += [(d >> 3) & 7, d & 7].iter().filter(|&&x| x != 4).count() as i64;
                }
                _ => {
                    v.push(0xc0 | r.below(64) as u8);
                    n += 1;
                }
            }
        }
    }
    if r.chance(8) && !v.is_empty() {
        let n = r.below(v.len() as u64) as usize;
        v.truncate(n);
    }
    v
}

/// A little-endian TIFF of one strip: `width` x `rows`, `bps` bits, one sample, `compression`,
/// photometric `photometric`, the extra `(tag, type, value)` SHORT or LONG entries (replacing
/// the defaults of their tags), the strip's bytes.
pub fn one_strip_file(
    width: u32,
    rows: u32,
    bps: u16,
    compression: u16,
    photometric: u16,
    extra: &[(u16, u16, u32)],
    data: &[u8],
) -> Vec<u8> {
    let mut out = b"II*\0".to_vec();
    let dir_at = 8 + data.len() as u32 + (data.len() as u32 & 1);
    out.extend_from_slice(&dir_at.to_le_bytes());
    out.extend_from_slice(data);
    if data.len() & 1 == 1 {
        out.push(0);
    }
    let mut tags: Vec<(u16, u16, u32)> = vec![
        (256, 4, width),
        (257, 4, rows),
        (258, 3, u32::from(bps)),
        (259, 3, u32::from(compression)),
        (262, 3, u32::from(photometric)),
        (273, 4, 8),
        (277, 3, 1),
        (278, 4, rows),
        (279, 4, data.len() as u32),
    ];
    tags.retain(|t| extra.iter().all(|e| e.0 != t.0));
    tags.extend_from_slice(extra);
    tags.sort_by_key(|t| t.0);
    // BitsPerSample and SampleFormat give one value per sample
    let spp = tags.iter().find(|t| t.0 == 277).map_or(1, |t| t.2.max(1));
    let entries_end = out.len() + 2 + 12 * tags.len() + 4;
    let mut far: Vec<u8> = Vec::new();
    out.extend_from_slice(&(tags.len() as u16).to_le_bytes());
    for (t, ty, v) in tags {
        let count = if t == 258 || t == 339 { spp } else { 1 };
        out.extend_from_slice(&t.to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        let mut val: Vec<u8> = Vec::new();
        for _ in 0..count {
            if ty == 3 {
                val.extend_from_slice(&(v as u16).to_le_bytes());
            } else {
                val.extend_from_slice(&v.to_le_bytes());
            }
        }
        if val.len() <= 4 {
            val.resize(4, 0);
            out.extend_from_slice(&val);
        } else {
            out.extend_from_slice(&((entries_end + far.len()) as u32).to_le_bytes());
            far.extend_from_slice(&val);
        }
    }
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&far);
    out
}

/// One strip of PixarLog data: `n` 11-bit codes (horizontally differenced: a first code, then
/// small steps; now and then a first code past 2047, or the count off), in the file's byte
/// order, deflated at level 1, 6 or 9.
pub fn pixarlog_strip(r: &mut Rng, n: u64, big: bool) -> Vec<u8> {
    let n = if r.chance(8) { r.below(n + 8) } else { n };
    let mut v = Vec::with_capacity(2 * n as usize);
    let mut k = 0u64;
    while k < n {
        let row = 1 + r.below(64);
        for j in 0..row.min(n - k) {
            let c: u16 = if j == 0 {
                if r.chance(20) {
                    2048 + r.below(60000) as u16
                } else {
                    r.below(2048) as u16
                }
            } else {
                (r.below(9) as i32 - 4) as u16
            };
            v.extend_from_slice(&if big {
                c.to_be_bytes()
            } else {
                c.to_le_bytes()
            });
        }
        k += row;
    }
    let level = r.pick(&[1, 6, 9]);
    larmorx_core::zlib::compress_vec(&v, level).unwrap_or_default()
}
