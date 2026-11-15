// SPDX-License-Identifier: Apache-2.0
//! Generated JPEG files for the unit tests and the differential probe (`validation/probes/fs-jpeg`):
//! a small test encoder (not IJG's: any valid stream serves) for baseline files with any number of
//! components and sampling factors, interleaved or one scan per component, with restart intervals,
//! and progressive files (DC first and refinement scans, AC first scans), then seeded damage
//! (changed, inserted and removed bytes, truncation). Everything is integer arithmetic on a seed,
//! so the bytes are the same on every platform.

use super::encode::{
    BITS_AC_LUMINANCE, BITS_DC_LUMINANCE, NATURAL_ORDER, STD_LUMINANCE_QUANT, VAL_AC_LUMINANCE,
    VAL_DC_LUMINANCE, fdct_islow,
};

/// SplitMix64.
#[derive(Clone)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
}

/// One generated file.
#[derive(Clone, Debug)]
pub struct Recipe {
    pub seed: u64,
    pub width: u32,
    pub height: u32,
    /// (identifier, h, v, quantization table) per component.
    pub comps: Vec<(u8, u8, u8, u8)>,
    pub progressive: bool,
    pub interleaved: bool,
    pub restart: u16,
    pub quality: u32,
    pub jfif: bool,
    pub adobe: Option<u8>,
    /// How many random edits damage the file (0: none).
    pub damage: u32,
}

impl Recipe {
    /// A random recipe from `seed`, sizes up to `max_side` samples.
    pub fn random(seed: u64, max_side: u32) -> Recipe {
        let mut r = Rng(seed ^ 0x6a70_6567);
        let width = 1 + r.below(max_side as u64) as u32;
        let height = 1 + r.below(max_side as u64) as u32;
        let ncomp = [1usize, 3, 3, 3, 3, 1, 2, 4][r.below(8) as usize];
        let mut comps = Vec::new();
        let ids: [u8; 4] = match r.below(4) {
            0 => [82, 71, 66, 4],
            1 => [0, 1, 2, 3],
            _ => [1, 2, 3, 4],
        };
        for (c, &id) in ids.iter().enumerate().take(ncomp) {
            let (h, v) = if r.below(3) == 0 {
                (1 + r.below(4) as u8, 1 + r.below(4) as u8)
            } else if c == 0 {
                [(1, 1), (2, 2), (2, 1), (1, 2)][r.below(4) as usize]
            } else {
                (1, 1)
            };
            comps.push((id, h, v, (c.min(1) as u8) * (r.below(2) as u8)));
        }
        let progressive = r.below(3) == 0;
        let interleaved = !progressive && (ncomp == 1 || r.below(4) != 0);
        Recipe {
            seed,
            width,
            height,
            comps,
            progressive,
            interleaved,
            restart: if r.below(4) == 0 {
                1 + r.below(7) as u16
            } else {
                0
            },
            quality: 5 + r.below(96) as u32,
            jfif: r.below(3) != 0,
            adobe: if r.below(4) == 0 {
                Some(r.below(3) as u8)
            } else {
                None
            },
            damage: if r.below(3) == 0 {
                1 + r.below(4) as u32
            } else {
                0
            },
        }
    }
}

/// A sample of component `c` at full resolution.
fn sample(seed: u64, c: usize, x: u32, y: u32) -> u8 {
    let s = seed as u32;
    let a = (x.wrapping_mul(3 + c as u32 * 5)).wrapping_add(y.wrapping_mul(7 + (s & 7)));
    let n = (x ^ y.rotate_left(3))
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add(s)
        >> 26;
    (a.wrapping_add(n).wrapping_add((c as u32) * 40) & 0xFF) as u8
}

struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, code: u32, size: u32) {
        if size == 0 {
            return;
        }
        self.acc = (self.acc << size) | (code as u64 & ((1u64 << size) - 1));
        self.n += size;
        while self.n >= 8 {
            let b = ((self.acc >> (self.n - 8)) & 0xFF) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0);
            }
            self.n -= 8;
        }
    }
    fn flush(&mut self) {
        if self.n > 0 {
            let pad = 8 - self.n;
            self.put((1 << pad) - 1, pad);
        }
        self.acc = 0;
    }
}

/// The standard tables' codes (`(code, size)` per symbol).
fn codes(bits: &[u8; 17], val: &[u8]) -> Vec<(u32, u32)> {
    let mut t = vec![(0u32, 0u32); 256];
    let mut code = 0u32;
    let mut k = 0;
    for (l, &n) in bits.iter().enumerate().skip(1) {
        for _ in 0..n {
            t[val[k] as usize] = (code, l as u32);
            code += 1;
            k += 1;
        }
        code <<= 1;
    }
    t
}

fn magnitude(v: i32) -> (u32, u32) {
    let mut t = v.unsigned_abs();
    let mut n = 0;
    while t != 0 {
        n += 1;
        t >>= 1;
    }
    let bits = if v < 0 { (v - 1) as u32 } else { v as u32 };
    (n, bits)
}

/// The file of `r`.
pub fn make(r: &Recipe) -> Vec<u8> {
    let max_h = r.comps.iter().map(|c| c.1 as u32).max().unwrap_or(1);
    let max_v = r.comps.iter().map(|c| c.2 as u32).max().unwrap_or(1);
    let scale = if r.quality < 50 {
        5000 / r.quality
    } else {
        200 - r.quality * 2
    };
    let mut q = [[1i32; 64]; 2];
    for (t, qt) in q.iter_mut().enumerate() {
        for i in 0..64 {
            let b = STD_LUMINANCE_QUANT[i] as i64 + t as i64 * 3;
            qt[i] = ((b * scale as i64 + 50) / 100).clamp(1, 255) as i32;
        }
    }
    // each component's quantized blocks, rounded up to whole MCUs
    let mcux = r.width.div_ceil(8 * max_h);
    let mcuy = r.height.div_ceil(8 * max_v);
    let mut blocks: Vec<(u32, u32, Vec<[i32; 64]>)> = Vec::new();
    for (ci, &(_, h, v, tq)) in r.comps.iter().enumerate() {
        let (h, v) = (h as u32, v as u32);
        let dw = (r.width * h).div_ceil(max_h);
        let dh = (r.height * v).div_ceil(max_v);
        let bw = mcux * h;
        let bh = mcuy * v;
        let mut list = Vec::with_capacity((bw * bh) as usize);
        for by in 0..bh {
            for bx in 0..bw {
                let mut ws = [0i32; 64];
                for yy in 0..8 {
                    for xx in 0..8 {
                        let x = (bx * 8 + xx).min(dw - 1);
                        let y = (by * 8 + yy).min(dh - 1);
                        let fx = (x * max_h / h).min(r.width - 1);
                        let fy = (y * max_v / v).min(r.height - 1);
                        ws[(yy * 8 + xx) as usize] = sample(r.seed, ci, fx, fy) as i32 - 128;
                    }
                }
                fdct_islow(&mut ws);
                let mut coef = [0i32; 64];
                for i in 0..64 {
                    let d = q[tq as usize][i] << 3;
                    let t = ws[i];
                    coef[i] = if t < 0 {
                        -((-t + (d >> 1)) / d)
                    } else {
                        (t + (d >> 1)) / d
                    };
                }
                list.push(coef);
            }
        }
        blocks.push((bw, bh, list));
    }
    let dc = codes(&BITS_DC_LUMINANCE, &VAL_DC_LUMINANCE);
    let ac = codes(&BITS_AC_LUMINANCE, &VAL_AC_LUMINANCE);
    let mut f = vec![0xFF, 0xD8];
    let seg = |f: &mut Vec<u8>, m: u8, body: &[u8]| {
        f.extend_from_slice(&[0xFF, m]);
        let len = body.len() as u16 + 2;
        f.extend_from_slice(&len.to_be_bytes());
        f.extend_from_slice(body);
    };
    if r.jfif {
        seg(&mut f, 0xE0, b"JFIF\0\x01\x01\x00\x00\x01\x00\x01\x00\x00");
    }
    if let Some(t) = r.adobe {
        seg(
            &mut f,
            0xEE,
            &[b'A', b'd', b'o', b'b', b'e', 0, 100, 0, 0, 0, 0, t],
        );
    }
    for (t, qt) in q.iter().enumerate() {
        let mut body = vec![t as u8];
        body.extend(NATURAL_ORDER.iter().map(|&p| qt[p] as u8));
        seg(&mut f, 0xDB, &body);
    }
    let mut sof = vec![8];
    sof.extend_from_slice(&(r.height as u16).to_be_bytes());
    sof.extend_from_slice(&(r.width as u16).to_be_bytes());
    sof.push(r.comps.len() as u8);
    for &(id, h, v, tq) in &r.comps {
        sof.extend_from_slice(&[id, (h << 4) | v, tq]);
    }
    seg(&mut f, if r.progressive { 0xC2 } else { 0xC0 }, &sof);
    for (idx, bits, val) in [
        (0u8, &BITS_DC_LUMINANCE, &VAL_DC_LUMINANCE[..]),
        (0x10, &BITS_AC_LUMINANCE, &VAL_AC_LUMINANCE[..]),
    ] {
        let mut body = vec![idx];
        body.extend_from_slice(&bits[1..]);
        body.extend_from_slice(val);
        seg(&mut f, 0xC4, &body);
    }
    if r.restart != 0 {
        seg(&mut f, 0xDD, &r.restart.to_be_bytes());
    }
    // the scans: (components, Ss, Se, Ah, Al)
    let n = r.comps.len();
    let all: Vec<usize> = (0..n).collect();
    let mut scans: Vec<(Vec<usize>, u8, u8, u8, u8)> = Vec::new();
    if r.progressive {
        for chunk in all.chunks(4) {
            scans.push((chunk.to_vec(), 0, 0, 0, 1));
        }
        for c in 0..n {
            scans.push((vec![c], 1, 5, 0, 0));
            scans.push((vec![c], 6, 63, 0, 0));
        }
        for chunk in all.chunks(4) {
            scans.push((chunk.to_vec(), 0, 0, 1, 0));
        }
    } else if r.interleaved && n <= 4 {
        scans.push((all.clone(), 0, 63, 0, 0));
    } else {
        for c in 0..n {
            scans.push((vec![c], 0, 63, 0, 0));
        }
    }
    for (sc, ss, se, ah, al) in scans {
        let mut body = vec![sc.len() as u8];
        for &c in &sc {
            body.extend_from_slice(&[r.comps[c].0, 0x00]);
        }
        body.extend_from_slice(&[ss, se, (ah << 4) | al]);
        seg(&mut f, 0xDA, &body);
        let mut w = BitWriter {
            out: Vec::new(),
            acc: 0,
            n: 0,
        };
        let mut last = vec![0i32; n];
        // the MCUs of this scan: lists of (component, block index)
        let mut mcus: Vec<Vec<(usize, usize)>> = Vec::new();
        if sc.len() > 1 {
            for my in 0..mcuy {
                for mx in 0..mcux {
                    let mut m = Vec::new();
                    for &c in &sc {
                        let (h, v) = (r.comps[c].1 as u32, r.comps[c].2 as u32);
                        let bw = blocks[c].0;
                        for yy in 0..v {
                            for xx in 0..h {
                                m.push((c, ((my * v + yy) * bw + mx * h + xx) as usize));
                            }
                        }
                    }
                    mcus.push(m);
                }
            }
        } else {
            let c = sc[0];
            let (h, v) = (r.comps[c].1 as u32, r.comps[c].2 as u32);
            let wib = (r.width * h).div_ceil(max_h).div_ceil(8);
            let hib = (r.height * v).div_ceil(max_v).div_ceil(8);
            for by in 0..hib {
                for bx in 0..wib {
                    mcus.push(vec![(c, (by * blocks[c].0 + bx) as usize)]);
                }
            }
        }
        for (k, m) in mcus.iter().enumerate() {
            if r.restart != 0 && k > 0 && k % r.restart as usize == 0 {
                w.flush();
                let rst = 0xD0 + ((k / r.restart as usize - 1) % 8) as u8;
                w.out.extend_from_slice(&[0xFF, rst]);
                last.iter_mut().for_each(|x| *x = 0);
            }
            for &(c, b) in m {
                let blk = &blocks[c].2[b];
                if ss == 0 && ah == 0 {
                    let v = blk[0] >> al;
                    let (nb, bits) = magnitude(v - last[c]);
                    last[c] = v;
                    w.put(dc[nb as usize].0, dc[nb as usize].1);
                    w.put(bits, nb);
                    if !r.progressive {
                        let mut run = 0;
                        for &p in NATURAL_ORDER.iter().skip(1) {
                            if blk[p] == 0 {
                                run += 1;
                                continue;
                            }
                            while run > 15 {
                                w.put(ac[0xF0].0, ac[0xF0].1);
                                run -= 16;
                            }
                            let (nb, bits) = magnitude(blk[p]);
                            let s = (run << 4) + nb as usize;
                            w.put(ac[s].0, ac[s].1);
                            w.put(bits, nb);
                            run = 0;
                        }
                        if run > 0 {
                            w.put(ac[0].0, ac[0].1);
                        }
                    }
                } else if ss == 0 {
                    w.put((blk[0] >> al) as u32 & 1, 1);
                } else {
                    let mut run = 0;
                    for &p in NATURAL_ORDER.iter().take(se as usize + 1).skip(ss as usize) {
                        if blk[p] == 0 {
                            run += 1;
                            continue;
                        }
                        while run > 15 {
                            w.put(ac[0xF0].0, ac[0xF0].1);
                            run -= 16;
                        }
                        let (nb, bits) = magnitude(blk[p]);
                        let s = (run << 4) + nb as usize;
                        w.put(ac[s].0, ac[s].1);
                        w.put(bits, nb);
                        run = 0;
                    }
                    if run > 0 {
                        w.put(ac[0].0, ac[0].1);
                    }
                }
            }
        }
        w.flush();
        f.extend_from_slice(&w.out);
    }
    f.extend_from_slice(&[0xFF, 0xD9]);
    damage(&mut f, r.seed, r.damage);
    f
}

/// `n` seeded edits of `f`: a changed byte, inserted or removed bytes, or a cut.
pub fn damage(f: &mut Vec<u8>, seed: u64, n: u32) {
    let mut g = Rng(seed ^ 0x646d_6721);
    for _ in 0..n {
        if f.is_empty() {
            return;
        }
        let at = g.below(f.len() as u64) as usize;
        match g.below(6) {
            0 | 1 => f[at] = g.next() as u8,
            2 => f[at] ^= 1 << g.below(8),
            3 => {
                let k = 1 + g.below(4) as usize;
                for _ in 0..k {
                    f.insert(at, g.next() as u8);
                }
            }
            4 => {
                let k = (1 + g.below(8) as usize).min(f.len() - at);
                f.drain(at..at + k);
            }
            _ => f.truncate(at),
        }
    }
}

/// A grey image of `w` x `h` samples for the encoder (rows top first).
pub fn gray_image(seed: u64, w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h) as usize);
    let mut r = Rng(seed);
    let kind = r.below(4);
    for y in 0..h {
        for x in 0..w {
            v.push(match kind {
                0 => sample(seed, 0, x, y),
                1 => r.next() as u8,
                2 => (((x / 3) ^ (y / 5)) & 1) as u8 * 255,
                _ => ((x * 255) / w.max(1)) as u8,
            });
        }
    }
    v
}
