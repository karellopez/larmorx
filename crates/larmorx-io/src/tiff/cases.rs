// SPDX-License-Identifier: Apache-2.0
//! Generated TIFF files for the unit tests and the differential probe (`validation/probes/
//! fs-tiff`): seeded, of every byte order, size, sample layout, compression this port reads
//! (and others), tiles and strips, several directories, tags of any type and count, in order
//! or not, and damaged in a third of the files. Not a TIFF writer: the files are made to
//! drive the reader's paths, valid or not. Plain arithmetic only (no platform maths).

/// xorshift64* (the generator of these cases).
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0xd1b5_4a32_d192_ed03 | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }
    pub fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
    pub fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u64) as usize]
    }
}

/// One tag of a directory: tag, type, count, the value's bytes (machine order of the file).
struct Tag {
    tag: u16,
    ty: u16,
    count: u32,
    data: Vec<u8>,
}

struct W {
    big: bool,
    out: Vec<u8>,
}

impl W {
    fn u16(&self, v: u16) -> [u8; 2] {
        if self.big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    }
    fn u32(&self, v: u32) -> [u8; 4] {
        if self.big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    }
    fn shorts(&self, v: &[u16]) -> Vec<u8> {
        v.iter().flat_map(|&x| self.u16(x)).collect()
    }
    fn longs(&self, v: &[u32]) -> Vec<u8> {
        v.iter().flat_map(|&x| self.u32(x)).collect()
    }
    fn pad(&mut self) {
        if self.out.len() % 2 == 1 {
            self.out.push(0);
        }
    }
}

/// TIFF's LZW (MSB-first codes, the code width growing one code early), or the old-style
/// variant (LSB-first codes, growing on time).
pub fn lzw_encode(data: &[u8], old_style: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc: u64 = 0;
    let mut nacc = 0u32;
    let put = |out: &mut Vec<u8>, code: u32, nbits: u32, acc: &mut u64, nacc: &mut u32| {
        if old_style {
            *acc |= u64::from(code) << *nacc;
            *nacc += nbits;
            while *nacc >= 8 {
                out.push(*acc as u8);
                *acc >>= 8;
                *nacc -= 8;
            }
        } else {
            *acc = (*acc << nbits) | u64::from(code);
            *nacc += nbits;
            while *nacc >= 8 {
                out.push((*acc >> (*nacc - 8)) as u8);
                *nacc -= 8;
            }
        }
    };
    let mut dict: std::collections::HashMap<(u32, u8), u32> = std::collections::HashMap::new();
    let mut next = 258u32;
    let mut nbits = 9u32;
    put(&mut out, 256, nbits, &mut acc, &mut nacc);
    let mut cur: Option<u32> = None;
    let early = if old_style { 0 } else { 1 };
    for &b in data {
        match cur {
            None => cur = Some(u32::from(b)),
            Some(c) => {
                if let Some(&n) = dict.get(&(c, b)) {
                    cur = Some(n);
                } else {
                    put(&mut out, c, nbits, &mut acc, &mut nacc);
                    dict.insert((c, b), next);
                    next += 1;
                    if next + early > (1 << nbits) && nbits < 12 {
                        nbits += 1;
                    }
                    if next >= 4094 {
                        put(&mut out, 256, nbits, &mut acc, &mut nacc);
                        dict.clear();
                        next = 258;
                        nbits = 9;
                    }
                    cur = Some(u32::from(b));
                }
            }
        }
    }
    if let Some(c) = cur {
        put(&mut out, c, nbits, &mut acc, &mut nacc);
        next += 1;
        if next + early > (1 << nbits) && nbits < 12 {
            nbits += 1;
        }
    }
    put(&mut out, 257, nbits, &mut acc, &mut nacc);
    if nacc > 0 {
        if old_style {
            out.push(acc as u8);
        } else {
            out.push((acc << (8 - nacc)) as u8);
        }
    }
    out
}

/// A PackBits encoding (runs and literals of up to 128 bytes).
pub fn packbits_encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let mut run = 1;
        while i + run < data.len() && run < 128 && data[i + run] == data[i] {
            run += 1;
        }
        if run > 1 {
            out.push((1i32 - run as i32) as i8 as u8);
            out.push(data[i]);
            i += run;
        } else {
            let start = i;
            let mut n = 0;
            while i < data.len() && n < 128 {
                if i + 1 < data.len() && data[i + 1] == data[i] && n > 0 {
                    break;
                }
                i += 1;
                n += 1;
            }
            out.push((n - 1) as u8);
            out.extend_from_slice(&data[start..start + n]);
        }
    }
    out
}

/// The pixel bytes of one strip or tile: smooth values with noise.
fn pixels(r: &mut Rng, n: usize) -> Vec<u8> {
    let kind = r.below(4);
    let mut v = Vec::with_capacity(n);
    let mut x = r.next_u64() as u8;
    for i in 0..n {
        v.push(match kind {
            0 => r.next_u64() as u8,
            1 => {
                if r.chance(9) {
                    x = r.next_u64() as u8;
                }
                x
            }
            2 => (i as u8).wrapping_mul(3) ^ (i >> 5) as u8,
            _ => 0,
        });
    }
    v
}

/// Horizontal differencing (the encoder side of predictor 2) on rows of `row` bytes.
fn difference(v: &mut [u8], row: usize, stride: usize, bps: u16, big: bool) {
    if row == 0 {
        return;
    }
    for r in v.chunks_mut(row) {
        if bps == 16 {
            let n = r.len() / 2;
            let get = |r: &[u8], i: usize| {
                if big {
                    u16::from_be_bytes([r[2 * i], r[2 * i + 1]])
                } else {
                    u16::from_le_bytes([r[2 * i], r[2 * i + 1]])
                }
            };
            for i in (stride..n).rev() {
                let d = get(r, i).wrapping_sub(get(r, i - stride));
                let b = if big {
                    d.to_be_bytes()
                } else {
                    d.to_le_bytes()
                };
                r[2 * i..2 * i + 2].copy_from_slice(&b);
            }
        } else {
            for i in (stride..r.len()).rev() {
                r[i] = r[i].wrapping_sub(r[i - stride]);
            }
        }
    }
}

/// The file of case `i` of `seed`.
#[allow(clippy::too_many_lines)]
pub fn make(seed: u64, i: u64) -> Vec<u8> {
    let mut r = Rng::new(seed.wrapping_mul(1_000_003).wrapping_add(i));
    let big = r.chance(3);
    let mut w = W {
        big,
        out: Vec::new(),
    };
    w.out.extend_from_slice(if big { b"MM" } else { b"II" });
    let ver = if r.chance(60) { 43 } else { 42 };
    let b = w.u16(ver);
    w.out.extend_from_slice(&b);
    w.out.extend_from_slice(&[0; 4]);
    let ndir = 1 + r.below(3) as usize;
    let mut link_at = 4usize;
    let mut dir_offsets = Vec::new();
    for _ in 0..ndir {
        let wmax = if r.chance(8) { 300 } else { 40 };
        let width = 1 + r.below(wmax) as u32;
        let lmax = if r.chance(8) { 300 } else { 40 };
        let length = 1 + r.below(lmax) as u32;
        let spp: u16 = r.pick(&[1, 1, 1, 3, 3, 4, 2, 5]);
        let bps: u16 = r.pick(&[8, 8, 8, 16, 16, 1, 4, 32, 64, 12, 2]);
        let planar: u16 = if r.chance(5) { 2 } else { 1 };
        let comp: u16 = r.pick(&[
            1, 1, 1, 32773, 32773, 32946, 32946, 8, 5, 5, 5, 99, 7, 6, 34661, 32809, 32766, 2, 3,
            3, 3, 4, 4, 4, 32771, 32809, 32766, 32909, 32909, 32909, 34676, 34676, 34677, 34677,
        ]);
        let fax = matches!(comp, 2 | 3 | 4 | 32771);
        // the codecs' own sample layouts, now and then another
        let (bps, spp, planar) = match comp {
            _ if r.chance(10) => (bps, spp, planar),
            2 | 3 | 4 | 32771 => (1, if r.chance(10) { spp } else { 1 }, planar),
            32809 => (4, 1, 1),
            32766 => (2, 1, 1),
            32909 => (r.pick(&[8, 8, 16, 16, 32, 11, 12]), spp, planar),
            _ => (bps, spp, planar),
        };
        let g3: u32 = match comp {
            3 => r.pick(&[0, 1, 1, 4, 5, 5]),
            4 => r.pick(&[0, 0, 0, 1, 2]),
            _ => 0,
        };
        let predictor = if matches!(comp, 5 | 8 | 32946) {
            r.pick(&[0u16, 1, 2, 2, 2, 3])
        } else if r.chance(20) {
            2
        } else {
            0
        };
        let tiled = r.chance(4);
        let (tw, th) = if tiled {
            (
                r.pick(&[16u32, 16, 32, 8, 0]),
                r.pick(&[16u32, 16, 32, 24, 0]),
            )
        } else {
            (0, 0)
        };
        let rps: Option<u32> = if tiled || r.chance(4) {
            None
        } else {
            Some(1 + r.below(u64::from(length) + 4) as u32)
        };
        let fillorder: Option<u16> = if r.chance(3) {
            Some(r.pick(&[1, 2, 2, 3]))
        } else {
            None
        };
        let orientation: Option<u16> = if r.chance(2) {
            Some(r.pick(&[1, 1, 4, 4, 2, 5, 9, 0]))
        } else {
            None
        };
        let photometric: Option<u16> = if r.chance(8) {
            None
        } else {
            Some(r.pick(&[1, 1, 0, 2, 2, 3, 6, 5, 4]))
        };
        let sampleformat: Option<u16> = if comp == 32909 && !r.chance(8) {
            match bps {
                32 => Some(3),
                12 => Some(r.pick(&[2, 4])),
                _ => Some(r.pick(&[1, 4])),
            }
        } else if r.chance(2) {
            Some(r.pick(&[1, 1, 2, 3, 4, 7, 0]))
        } else {
            None
        };
        // SGILog: LogL or LogLuv with the sample layouts its decoders know, now and then not
        let (photometric, bps, spp, planar, sampleformat) =
            if matches!(comp, 34676 | 34677) && !r.chance(10) {
                let logl = r.chance(3);
                let (b, sp, f) = if logl {
                    r.pick(&[
                        (16u16, 1u16, 2u16),
                        (16, 1, 1),
                        (32, 1, 3),
                        (8, 1, 1),
                        (8, 1, 4),
                    ])
                } else {
                    r.pick(&[
                        (32u16, 3u16, 3u16),
                        (16, 3, 2),
                        (8, 3, 1),
                        (32, 1, 1),
                        (32, 1, 4),
                    ])
                };
                let pl = if r.chance(15) { 2 } else { 1 };
                (Some(if logl { 32844 } else { 32845 }), b, sp, pl, Some(f))
            } else {
                (photometric, bps, spp, planar, sampleformat)
            };
        // The geometry of the data.
        let samples_per_unit = if planar == 1 { u32::from(spp) } else { 1 };
        let row_bits = |cols: u32| u64::from(cols) * u64::from(bps) * u64::from(samples_per_unit);
        let row_bytes = |cols: u32| row_bits(cols).div_ceil(8) as usize;
        let mut blocks: Vec<(Vec<u8>, u32, u32)> = Vec::new();
        let nplanes = if planar == 2 { usize::from(spp) } else { 1 };
        if tiled && tw > 0 && th > 0 {
            let across = width.div_ceil(tw);
            let down = length.div_ceil(th);
            for _ in 0..nplanes {
                for _ in 0..across * down {
                    let row = row_bytes(tw);
                    let mut p = pixels(&mut r, row * th as usize);
                    if predictor == 2 && (bps == 8 || bps == 16) {
                        difference(&mut p, row, samples_per_unit as usize, bps, big);
                    }
                    blocks.push((p, tw, th));
                }
            }
        } else {
            let per = rps.unwrap_or(u32::MAX).min(length).max(1);
            let nstrips = length.div_ceil(per);
            for _ in 0..nplanes {
                for k in 0..nstrips {
                    let rows = per.min(length - k * per);
                    let row = row_bytes(width);
                    let mut p = pixels(&mut r, row * rows as usize);
                    if predictor == 2 && (bps == 8 || bps == 16) {
                        difference(&mut p, row, samples_per_unit as usize, bps, big);
                    }
                    blocks.push((p, width, rows));
                }
            }
        }
        let old_lzw = r.chance(6);
        let mut offsets = Vec::new();
        let mut counts = Vec::new();
        for (p, cols, nrows) in &blocks {
            let (cols, nrows) = (*cols, *nrows);
            let enc = match comp {
                2 | 3 | 4 | 32771 => {
                    let img = super::codecgen::bilevel(&mut r, cols, nrows);
                    super::codecgen::fax_strip(&mut r, comp, g3, &img)
                }
                32766 => super::codecgen::next_strip(&mut r, nrows, cols, row_bytes(cols)),
                32809 => super::codecgen::thunder_strip(&mut r, nrows, cols),
                34676 | 34677 => super::codecgen::sgilog_strip(
                    &mut r,
                    nrows,
                    cols,
                    match (photometric, comp) {
                        (Some(32844), _) => 2,
                        (_, 34677) => 0,
                        _ => 4,
                    },
                ),
                32909 => super::codecgen::pixarlog_strip(
                    &mut r,
                    u64::from(nrows) * u64::from(cols) * u64::from(samples_per_unit),
                    big,
                ),
                1 => p.clone(),
                32773 => packbits_encode(p),
                32946 | 8 => {
                    larmorx_core::zlib::compress_vec(p, r.pick(&[1, 6, 9])).unwrap_or_default()
                }
                5 => lzw_encode(p, old_lzw),
                _ => {
                    let n = r.below(p.len() as u64 + 8) as usize;
                    (0..n).map(|_| r.next_u64() as u8).collect()
                }
            };
            w.pad();
            offsets.push(w.out.len() as u32);
            counts.push(enc.len() as u32);
            w.out.extend_from_slice(&enc);
        }
        // The tags.
        let mut tags: Vec<Tag> = Vec::new();
        let short_or_long = |w: &W, r: &mut Rng, t: u16, v: u32| -> Tag {
            if v <= 0xffff && r.chance(2) {
                let mut d = w.u16(v as u16).to_vec();
                d.extend_from_slice(&[0, 0]);
                Tag {
                    tag: t,
                    ty: 3,
                    count: 1,
                    data: d,
                }
            } else {
                Tag {
                    tag: t,
                    ty: 4,
                    count: 1,
                    data: w.u32(v).to_vec(),
                }
            }
        };
        if !r.chance(40) {
            tags.push(short_or_long(&w, &mut r, 256, width));
        }
        if !r.chance(40) {
            tags.push(short_or_long(&w, &mut r, 257, length));
        }
        {
            let n = if r.chance(4) { 1 } else { u32::from(spp) };
            let vals: Vec<u16> = (0..n)
                .map(|k| if k > 0 && r.chance(30) { bps + 1 } else { bps })
                .collect();
            if r.chance(25) {
                tags.push(Tag {
                    tag: 258,
                    ty: 4,
                    count: n,
                    data: w.longs(&vals.iter().map(|&x| u32::from(x)).collect::<Vec<_>>()),
                });
            } else {
                tags.push(Tag {
                    tag: 258,
                    ty: 3,
                    count: n,
                    data: w.shorts(&vals),
                });
            }
        }
        if comp != 1 || r.chance(2) {
            tags.push(Tag {
                tag: 259,
                ty: 3,
                count: 1,
                data: w.shorts(&[comp, 0]),
            });
        }
        if let Some(p) = photometric {
            tags.push(Tag {
                tag: 262,
                ty: 3,
                count: 1,
                data: w.shorts(&[p, 0]),
            });
        }
        if let Some(f) = fillorder {
            tags.push(Tag {
                tag: 266,
                ty: 3,
                count: 1,
                data: w.shorts(&[f, 0]),
            });
        }
        if r.chance(3) {
            let text: Vec<u8> = (0..r.below(30))
                .map(|_| b'a' + r.below(26) as u8)
                .chain([0])
                .collect();
            tags.push(Tag {
                tag: 270,
                ty: 2,
                count: text.len() as u32,
                data: text,
            });
        }
        let (otag, ctag) = if tiled {
            (324u16, 325u16)
        } else {
            (273u16, 279u16)
        };
        let offs_short = offsets.iter().all(|&o| o <= 0xffff) && r.chance(4);
        if offs_short && !tiled {
            let v: Vec<u16> = offsets.iter().map(|&o| o as u16).collect();
            tags.push(Tag {
                tag: otag,
                ty: 3,
                count: v.len() as u32,
                data: w.shorts(&v),
            });
        } else {
            tags.push(Tag {
                tag: otag,
                ty: 4,
                count: offsets.len() as u32,
                data: w.longs(&offsets),
            });
        }
        if let Some(o) = orientation {
            tags.push(Tag {
                tag: 274,
                ty: 3,
                count: 1,
                data: w.shorts(&[o, 0]),
            });
        }
        if spp != 1 || r.chance(3) {
            tags.push(Tag {
                tag: 277,
                ty: 3,
                count: 1,
                data: w.shorts(&[spp, 0]),
            });
        }
        if let Some(v) = rps {
            tags.push(short_or_long(&w, &mut r, 278, v));
        }
        match r.below(12) {
            0 => {}
            1 => {
                let bad: Vec<u32> = counts.iter().map(|_| 0).collect();
                tags.push(Tag {
                    tag: ctag,
                    ty: 4,
                    count: bad.len() as u32,
                    data: w.longs(&bad),
                });
            }
            _ => tags.push(Tag {
                tag: ctag,
                ty: 4,
                count: counts.len() as u32,
                data: w.longs(&counts),
            }),
        }
        if r.chance(3) {
            let num = 72 + r.below(300) as u32;
            let den = if r.chance(20) {
                0
            } else {
                1 + r.below(3) as u32
            };
            tags.push(Tag {
                tag: 282,
                ty: 5,
                count: 1,
                data: w.longs(&[num, den]),
            });
            tags.push(Tag {
                tag: 283,
                ty: 5,
                count: 1,
                data: w.longs(&[num + 1, den]),
            });
        }
        if planar != 1 || r.chance(3) {
            tags.push(Tag {
                tag: 284,
                ty: 3,
                count: 1,
                data: w.shorts(&[planar, 0]),
            });
        }
        if r.chance(3) {
            tags.push(Tag {
                tag: 296,
                ty: 3,
                count: 1,
                data: w.shorts(&[r.pick(&[1, 2, 3, 4]), 0]),
            });
        }
        if (comp == 3 && !r.chance(6)) || (fax && r.chance(12)) {
            tags.push(Tag {
                tag: 292,
                ty: 4,
                count: 1,
                data: w.longs(&[g3]),
            });
        }
        if comp == 4 && (g3 != 0 || r.chance(3)) {
            tags.push(Tag {
                tag: 293,
                ty: 4,
                count: 1,
                data: w.longs(&[g3]),
            });
        }
        if fax && r.chance(4) {
            let t = r.pick(&[326u16, 327, 328, 34908, 34909, 34910]);
            if t == 34909 {
                let text: Vec<u8> = (0..1 + r.below(12))
                    .map(|_| b'0' + r.below(10) as u8)
                    .chain([0])
                    .collect();
                tags.push(Tag {
                    tag: t,
                    ty: 2,
                    count: text.len() as u32,
                    data: text,
                });
            } else if r.chance(2) && t != 34908 && t != 34910 {
                tags.push(Tag {
                    tag: t,
                    ty: 3,
                    count: 1,
                    data: w.shorts(&[r.below(100) as u16, 0]),
                });
            } else {
                tags.push(Tag {
                    tag: t,
                    ty: 4,
                    count: 1,
                    data: w.longs(&[r.below(1000) as u32]),
                });
            }
        }
        if predictor != 0 {
            tags.push(Tag {
                tag: 317,
                ty: 3,
                count: 1,
                data: w.shorts(&[predictor, 0]),
            });
        }
        if photometric == Some(3) && bps <= 8 && r.chance(2) {
            let n = 3u32 << bps;
            let v: Vec<u16> = (0..n).map(|k| (k * 257) as u16).collect();
            tags.push(Tag {
                tag: 320,
                ty: 3,
                count: n,
                data: w.shorts(&v),
            });
        }
        if tiled {
            if tw > 0 || r.chance(2) {
                tags.push(short_or_long(&w, &mut r, 322, tw));
            }
            tags.push(short_or_long(&w, &mut r, 323, th));
        }
        if spp > 1 && r.chance(3) {
            let n = if r.chance(4) {
                u32::from(spp)
            } else {
                u32::from(spp) - 1
            };
            let v: Vec<u16> = (0..n).map(|_| r.pick(&[0, 1, 2, 3])).collect();
            if r.chance(3) {
                let mut d: Vec<u8> = v.iter().map(|&x| x as u8).collect();
                while d.len() < 4 {
                    d.push(0);
                }
                tags.push(Tag {
                    tag: 338,
                    ty: 1,
                    count: n,
                    data: d,
                });
            } else {
                tags.push(Tag {
                    tag: 338,
                    ty: 3,
                    count: n,
                    data: w.shorts(&v),
                });
            }
        }
        if let Some(f) = sampleformat {
            let n = if r.chance(3) { 1 } else { u32::from(spp) };
            tags.push(Tag {
                tag: 339,
                ty: 3,
                count: n,
                data: w.shorts(&vec![f; n as usize]),
            });
        }
        // Unknown and odd tags.
        for _ in 0..r.below(4) {
            let t = r.pick(&[
                40000u16, 50000, 65000, 33432, 305, 306, 340, 341, 297, 530, 0, 700, 34675, 330,
                32995, 37439,
            ]);
            let ty = r.pick(&[1u16, 2, 3, 4, 5, 7, 11, 12, 6, 8, 9, 10, 13, 0, 14]);
            let count = r.pick(&[0u32, 1, 1, 2, 3, 4, 5, 9, 20]);
            let wdt = match ty {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 | 13 => 4,
                5 | 10 | 12 => 8,
                _ => 1,
            };
            let n = (wdt * count as usize).max(4);
            let data: Vec<u8> = (0..n).map(|_| r.next_u64() as u8 % 97).collect();
            tags.push(Tag {
                tag: t,
                ty,
                count,
                data,
            });
        }
        tags.sort_by_key(|t| t.tag);
        if r.chance(10) && tags.len() > 2 {
            let a = r.below(tags.len() as u64) as usize;
            let b = r.below(tags.len() as u64) as usize;
            tags.swap(a, b);
        }
        // Values longer than 4 bytes go before the directory.
        let mut entries: Vec<(u16, u16, u32, [u8; 4])> = Vec::new();
        for t in &tags {
            let mut v = [0u8; 4];
            if t.data.len() <= 4 {
                v[..t.data.len()].copy_from_slice(&t.data);
            } else {
                w.pad();
                let at = w.out.len() as u32;
                w.out.extend_from_slice(&t.data);
                v = w.u32(at);
            }
            entries.push((t.tag, t.ty, t.count, v));
        }
        w.pad();
        let at = w.out.len() as u32;
        dir_offsets.push(at);
        let b = w.u32(at);
        w.out[link_at..link_at + 4].copy_from_slice(&b);
        let b = w.u16(entries.len() as u16);
        w.out.extend_from_slice(&b);
        for (t, ty, c, v) in entries {
            let b = w.u16(t);
            w.out.extend_from_slice(&b);
            let b = w.u16(ty);
            w.out.extend_from_slice(&b);
            let b = w.u32(c);
            w.out.extend_from_slice(&b);
            w.out.extend_from_slice(&v);
        }
        link_at = w.out.len();
        w.out.extend_from_slice(&[0; 4]);
    }
    // A loop in the chain, now and then.
    if r.chance(25) && !dir_offsets.is_empty() {
        let to = dir_offsets[r.below(dir_offsets.len() as u64) as usize];
        let b = w.u32(to);
        w.out[link_at..link_at + 4].copy_from_slice(&b);
    }
    let mut out = w.out;
    // Damage.
    if r.chance(3) && out.len() > 8 {
        match r.below(3) {
            0 => {
                for _ in 0..1 + r.below(6) {
                    let p = 8 + r.below(out.len() as u64 - 8) as usize;
                    out[p] ^= 1 << r.below(8);
                }
            }
            1 => {
                let n = 8 + r.below(out.len() as u64 - 8) as usize;
                out.truncate(n);
            }
            _ => {
                let p = 8 + r.below(out.len() as u64 - 8) as usize;
                out[p] = r.next_u64() as u8;
            }
        }
    }
    out
}

/// The parameters of a writing case: columns, rows, samples per pixel, bits per sample, the
/// first SampleFormat, the resolutions (0: not set), the compression, and the rows' bytes.
pub struct WriteCase {
    pub cols: u32,
    pub rows: u32,
    pub spp: u16,
    pub bps: u16,
    pub sample_format: u16,
    pub xres: f64,
    pub yres: f64,
    pub compression: u16,
    pub data: Vec<u8>,
}

/// Writing case `i` of `seed`.
pub fn write_case(seed: u64, i: u64) -> WriteCase {
    let mut r = Rng::new(seed.wrapping_mul(7_919).wrapping_add(i) ^ 0x5157);
    let cmax = if r.chance(5) { 900 } else { 60 };
    let cols = 1 + r.below(cmax) as u32;
    let rmax = if r.chance(5) { 400 } else { 60 };
    let rows = 1 + r.below(rmax) as u32;
    let spp = r.pick(&[1u16, 1, 3]);
    let bps = r.pick(&[8u16, 8, 16, 32, 1]);
    let sample_format = r.pick(&[1u16, 2, 3, 3]);
    let res = |r: &mut Rng| -> f64 {
        match r.below(6) {
            0 => 0.0,
            1 => -((1 + r.below(30)) as f64),
            _ => (1 + r.below(300)) as f64,
        }
    };
    let xres = res(&mut r);
    let yres = res(&mut r);
    let compression = if r.chance(8) {
        r.pick(&[1u16, 32773, 8, 5])
    } else {
        32946
    };
    let row = ((u64::from(cols) * u64::from(bps) * u64::from(spp)).div_ceil(8)) as usize;
    let data = pixels(&mut r, row * rows as usize);
    WriteCase {
        cols,
        rows,
        spp,
        bps,
        sample_format,
        xres,
        yres,
        compression,
        data,
    }
}

use super::{Options, SetValue, Tiff, Value, tag};
use larmorx_core::cfmt::{self, Captured, Stream};

/// The tags the read transcript asks `TIFFGetFieldDefaulted` for, with the C type it writes.
pub const QUERIED: &[(u32, char)] = &[
    (256, 'L'),
    (257, 'L'),
    (277, 'S'),
    (258, 'S'),
    (262, 'S'),
    (266, 'S'),
    (259, 'S'),
    (296, 'S'),
    (282, 'F'),
    (283, 'F'),
    (274, 'S'),
    (284, 'S'),
    (278, 'L'),
    (339, 'S'),
    (280, 'S'),
    (281, 'S'),
    (254, 'L'),
    (263, 'S'),
    (32997, 'L'),
    (32998, 'L'),
    (32995, 'S'),
    (32996, 'S'),
    (530, 'P'),
    (297, 'P'),
    (338, 'X'),
];

/// FNV-1a, 64 bits.
pub fn fnv(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &x in b {
        h = (h ^ u64::from(x)).wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn value_text(v: &Value) -> String {
    match v {
        Value::U16(x) => x.to_string(),
        Value::U32(x) => x.to_string(),
        Value::I32(x) => x.to_string(),
        Value::F32(x) => cfmt::g(f64::from(*x), 9),
        Value::F64(x) => cfmt::g(*x, 9),
        Value::U16Pair(a, b) => format!("{a} {b}"),
        Value::ExtraSamples(n, v) => {
            let mut s = n.to_string();
            for x in v.iter().take(usize::from(*n)) {
                s.push_str(&format!(" {x}"));
            }
            s
        }
        Value::U32Array(v) => format!("[{}]", v.len()),
        Value::Present => "present".into(),
    }
}

/// The fields of the current directory as the read transcript prints them.
fn fields_text(t: &mut Tiff, out: &mut String, cap: &mut Captured) {
    for &(tg, _) in QUERIED {
        match t.get_field_defaulted(tg, cap) {
            Some(v) => out.push_str(&format!(" f {tg} 1 {}\n", value_text(&v))),
            None => out.push_str(&format!(" f {tg} 0\n")),
        }
    }
    for tg in [tag::TILEWIDTH, tag::TILELENGTH] {
        match t.get_field(tg, cap) {
            Some(v) => out.push_str(&format!(" g {tg} 1 {}\n", value_text(&v))),
            None => out.push_str(&format!(" g {tg} 0\n")),
        }
    }
}

fn u32_of(v: Option<Value>) -> u32 {
    match v {
        Some(Value::U32(x)) => x,
        Some(Value::U16(x)) => u32::from(x),
        _ => 0,
    }
}

/// The data of the current directory as the read transcript reads it: every scanline (or
/// tile) of up to five samples, each into a buffer filled with 0xab first.
fn data_text(t: &mut Tiff, out: &mut String, cap: &mut Captured) -> Result<(), super::Unported> {
    let tiled = t.tiled();
    let sls = t.scanline_size();
    let ts = t.tile_size();
    out.push_str(&format!(" tiled {} sls {sls} ts {ts}\n", i32::from(tiled)));
    let size = if tiled { ts } else { sls };
    if size <= 0 || size > 16 << 20 {
        out.push_str(" skip\n");
        return Ok(());
    }
    let length = u32_of(t.get_field_defaulted(tag::IMAGELENGTH, cap));
    let width = u32_of(t.get_field_defaulted(tag::IMAGEWIDTH, cap));
    let spp = u32_of(t.get_field_defaulted(tag::SAMPLESPERPIXEL, cap));
    let planar = u32_of(t.get_field_defaulted(tag::PLANARCONFIG, cap));
    let nsamp = if planar == 2 { spp.clamp(1, 5) } else { 1 };
    let mut buf = vec![0xabu8; size as usize];
    let mut reads = 0u32;
    if !tiled {
        for sample in 0..nsamp {
            for row in 0..length.min(400) {
                buf.fill(0xab);
                let r = t.read_scanline(&mut buf, row, sample as u16, cap)?;
                out.push_str(&format!(" r {row} {sample} {r} {:016x}\n", fnv(&buf)));
            }
        }
    } else {
        let tw = u32_of(t.get_field(tag::TILEWIDTH, cap)).max(1);
        let th = u32_of(t.get_field(tag::TILELENGTH, cap)).max(1);
        'all: for sample in 0..nsamp {
            let mut y = 0u32;
            while y < length {
                let mut x = 0u32;
                while x < width {
                    buf.fill(0xab);
                    let r = t.read_tile(&mut buf, x, y, 0, sample as u16, cap)?;
                    out.push_str(&format!(" t {x} {y} {sample} {r} {:016x}\n", fnv(&buf)));
                    reads += 1;
                    if reads >= 200 {
                        break 'all;
                    }
                    x = x.saturating_add(tw);
                }
                y = y.saturating_add(th);
            }
        }
    }
    Ok(())
}

/// The library-wide state of a new process (the cache of `_TIFFFindFieldInfo`): each probe
/// case starts from it, as each original runs in a fresh child.
pub fn reset_thread_state() {
    super::fields::reset_last();
    super::take_closed_notices();
}

/// The read transcript of the port on the file `path` (the probe's `tf_read` in C): its
/// standard output and standard error, the notices' IDs, or the codec not ported yet.
pub fn port_read(
    path: &str,
    opts: Options,
) -> (String, String, Vec<&'static str>, Option<super::Unported>) {
    let mut cap = Captured::default();
    let mut out = String::new();
    let mut ids: Vec<&'static str> = Vec::new();
    let r = (|| -> Result<(), super::Unported> {
        let Some(mut t) = Tiff::open(path, "r", opts, &mut cap)? else {
            out.push_str("open 0\n");
            return Ok(());
        };
        out.push_str("open 1\n");
        let n = t.number_of_directories(&mut cap);
        out.push_str(&format!("ndirs {n}\n"));
        let res = (|| -> Result<(), super::Unported> {
            for d in 0..n.min(6) {
                let r = t.set_directory(d, &mut cap)?;
                out.push_str(&format!("setdir {d} {}\n", i32::from(r)));
                fields_text(&mut t, &mut out, &mut cap);
                data_text(&mut t, &mut out, &mut cap)?;
            }
            let r = t.set_directory(0, &mut cap)?;
            out.push_str(&format!("setdir 0 {}\n", i32::from(r)));
            for _ in 0..8 {
                let r = t.read_directory(&mut cap)?;
                out.push_str(&format!("readdir {}\n", i32::from(r)));
                if !r {
                    break;
                }
            }
            Ok(())
        })();
        t.close(&mut cap);
        out.push_str("close\n");
        res
    })();
    for n in super::take_closed_notices() {
        ids.push(n.defect.id);
    }
    (out, cap.text(Stream::Err), ids, r.err())
}

/// The write transcript of the port (the probe's `tf_write` in C: TIFFOpen "w", the fields,
/// every row, TIFFWriteDirectory, TIFFClose) into `path`: standard output, standard error, the
/// notices' IDs.
pub fn port_write(path: &str, c: &WriteCase, opts: Options) -> (String, String, Vec<&'static str>) {
    let mut cap = Captured::default();
    let mut out = String::new();
    let mut ids = Vec::new();
    let Ok(Some(mut t)) = Tiff::open(path, "w", opts, &mut cap) else {
        out.push_str("open 0\n");
        for n in super::take_closed_notices() {
            ids.push(n.defect.id);
        }
        return (out, cap.text(Stream::Err), ids);
    };
    out.push_str("open 1\n");
    let set = |t: &mut Tiff, tg: u32, v: SetValue, out: &mut String, cap: &mut Captured| {
        let r = t.set_field(tg, v, cap).unwrap_or(false);
        out.push_str(&format!("set {tg} {}\n", i32::from(r)));
    };
    set(
        &mut t,
        tag::SAMPLEFORMAT,
        SetValue::Int(i64::from(c.sample_format)),
        &mut out,
        &mut cap,
    );
    set(
        &mut t,
        tag::IMAGEWIDTH,
        SetValue::Int(i64::from(c.cols)),
        &mut out,
        &mut cap,
    );
    set(
        &mut t,
        tag::IMAGELENGTH,
        SetValue::Int(i64::from(c.rows)),
        &mut out,
        &mut cap,
    );
    set(
        &mut t,
        tag::ORIENTATION,
        SetValue::Int(4),
        &mut out,
        &mut cap,
    );
    set(
        &mut t,
        tag::SAMPLESPERPIXEL,
        SetValue::Int(i64::from(c.spp)),
        &mut out,
        &mut cap,
    );
    set(
        &mut t,
        tag::SAMPLEFORMAT,
        SetValue::Int(2),
        &mut out,
        &mut cap,
    );
    set(
        &mut t,
        tag::BITSPERSAMPLE,
        SetValue::Int(i64::from(c.bps)),
        &mut out,
        &mut cap,
    );
    set(
        &mut t,
        tag::PLANARCONFIG,
        SetValue::Int(1),
        &mut out,
        &mut cap,
    );
    set(
        &mut t,
        tag::PHOTOMETRIC,
        SetValue::Int(1),
        &mut out,
        &mut cap,
    );
    set(
        &mut t,
        tag::RESOLUTIONUNIT,
        SetValue::Int(2),
        &mut out,
        &mut cap,
    );
    if c.xres != 0.0 {
        set(
            &mut t,
            tag::XRESOLUTION,
            SetValue::Double(c.xres),
            &mut out,
            &mut cap,
        );
    }
    if c.yres != 0.0 {
        set(
            &mut t,
            tag::YRESOLUTION,
            SetValue::Double(c.yres),
            &mut out,
            &mut cap,
        );
    }
    set(
        &mut t,
        tag::COMPRESSION,
        SetValue::Int(i64::from(c.compression)),
        &mut out,
        &mut cap,
    );
    let row = ((u64::from(c.cols) * u64::from(c.bps) * u64::from(c.spp)).div_ceil(8)) as usize;
    let mut bad = 0u32;
    for k in 0..c.rows as usize {
        let mut b = c.data[k * row..(k + 1) * row].to_vec();
        b.resize(row + 64, 0);
        let r = t
            .write_scanline(&mut b, k as u32, 0, &mut cap)
            .unwrap_or(-1);
        if r != 1 {
            bad += 1;
            if bad <= 3 {
                out.push_str(&format!("row {k} {r}\n"));
            }
        }
    }
    out.push_str(&format!("rows {} bad {bad}\n", c.rows));
    let r = t.write_directory(&mut cap).unwrap_or(false);
    out.push_str(&format!("writedir {}\n", i32::from(r)));
    t.close(&mut cap);
    out.push_str("close\n");
    for n in super::take_closed_notices() {
        ids.push(n.defect.id);
    }
    (out, cap.text(Stream::Err), ids)
}
