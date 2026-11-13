// SPDX-License-Identifier: Apache-2.0
//! Generated DICOM files and other files a format probe meets: part 10 files in every transfer
//! syntax CTN tells apart, bare data sets in implicit and explicit VR, both byte orders, sequences
//! of defined and undefined length nested a few levels, native and encapsulated pixel data, group
//! lengths right and wrong; then mutations (cuts, flipped bytes, odd, huge and undefined lengths,
//! elements out of order, stray delimiters, unknown VR codes) and files that are not DICOM.

use larmorx_io::dicom::dict;

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9e37_79b9_7f4a_7c15)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    pub fn chance(&mut self, p: f64) -> bool {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 <= p
    }
    pub fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len() as u64) as usize]
    }
    pub fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next() as u8).collect()
    }
}

#[derive(Clone, Copy)]
pub struct Enc {
    pub big: bool,
    pub explicit: bool,
}

const LONG_VRS: [&[u8; 2]; 5] = [b"OB", b"OW", b"UN", b"UT", b"SQ"];

fn put16(o: &mut Vec<u8>, v: u16, enc: Enc) {
    o.extend_from_slice(&if enc.big { v.to_be_bytes() } else { v.to_le_bytes() });
}
fn put32(o: &mut Vec<u8>, v: u32, enc: Enc) {
    o.extend_from_slice(&if enc.big { v.to_be_bytes() } else { v.to_le_bytes() });
}

/// One element: tag, (explicit) VR code, length, value.
pub fn element(o: &mut Vec<u8>, tag: u32, vr: [u8; 2], length: u32, value: &[u8], enc: Enc) {
    put16(o, (tag >> 16) as u16, enc);
    put16(o, tag as u16, enc);
    let delimiter = (tag >> 16) == 0xfffe;
    if enc.explicit && !delimiter {
        o.extend_from_slice(&vr);
        if LONG_VRS.iter().any(|v| **v == vr) {
            o.extend_from_slice(&[0, 0]);
            put32(o, length, enc);
        } else {
            put16(o, length as u16, enc);
        }
    } else {
        put32(o, length, enc);
    }
    o.extend_from_slice(value);
}

fn vr_code(vr: u32) -> [u8; 2] {
    const CODES: [&[u8; 2]; 31] = [
        b"AE", b"AS", b"AT", b"CS", b"DA", b"DD", b"DS", b"FD", b"FL", b"IS", b"LO", b"LT", b"OT",
        b"SH", b"SL", b"SQ", b"SS", b"ST", b"TM", b"UI", b"UL", b"UN", b"US", b"RT", b"  ", b"PN",
        b"OB", b"OW", b"DT", b"\0\0", b"UT",
    ];
    *CODES[vr as usize]
}

/// A value for representation `vr` (CTN's number), even length mostly.
fn value(rng: &mut Rng, vr: u32, enc: Enc) -> Vec<u8> {
    let k = 1 + rng.below(4) as usize;
    let mut v = match vr {
        // strings
        0 | 1 | 3 | 4 | 6 | 9 | 10 | 11 | 13 | 17 | 18 | 19 | 25 | 28 | 30 => {
            let n = rng.below(24) as usize;
            let mut s: Vec<u8> = (0..n)
                .map(|_| *rng.pick(b"0123456789.ABCDEFGHIJ abc\\-+^"))
                .collect();
            if vr == 19 && rng.chance(0.5) {
                s = b"1.2.840.10008.5.1.4.1.1.4".to_vec();
            }
            if s.len() % 2 == 1 {
                s.push(if vr == 19 { 0 } else { b' ' });
            }
            s
        }
        // 16-bit
        16 | 22 => {
            let mut s = Vec::new();
            for _ in 0..k {
                put16(&mut s, rng.below(70000) as u16, enc);
            }
            s
        }
        // 32-bit and tags
        2 | 8 | 14 | 20 => {
            let mut s = Vec::new();
            for _ in 0..k {
                put32(&mut s, rng.next() as u32, enc);
            }
            s
        }
        7 => {
            let mut s = Vec::new();
            for _ in 0..k {
                let f = (rng.below(2_000_000) as f64 - 1e6) / 37.0;
                s.extend_from_slice(&if enc.big { f.to_be_bytes() } else { f.to_le_bytes() });
            }
            s
        }
        _ => {
            let n = 2 * rng.below(20) as usize;
            rng.bytes(n)
        }
    };
    if rng.chance(0.02) {
        v.clear();
    }
    v
}

/// The dictionary's tags of the groups a data set holds, in order.
fn dataset_tags() -> Vec<(u32, u32)> {
    let mut t: Vec<(u32, u32)> = dict::entries()
        .filter(|e| (e.tag >> 16) >= 0x0008 && (e.tag >> 16) < 0xfffe)
        .map(|e| (e.tag, e.vr))
        .collect();
    t.sort();
    t.dedup_by_key(|x| x.0);
    t
}

pub struct Gen {
    tags: Vec<(u32, u32)>,
}

impl Default for Gen {
    fn default() -> Self {
        Gen::new()
    }
}

impl Gen {
    pub fn new() -> Gen {
        Gen { tags: dataset_tags() }
    }

    /// A data set: elements in ascending order, some unknown or private, sequences, pixel data.
    pub fn dataset(&self, rng: &mut Rng, enc: Enc, depth: u32, max: usize) -> Vec<u8> {
        let mut picked: Vec<(u32, u32)> = Vec::new();
        let n = rng.below(max as u64 + 1) as usize;
        for _ in 0..n {
            let r = rng.below(100);
            let t = if r < 75 {
                *rng.pick(&self.tags)
            } else if r < 85 {
                // a private or unknown tag
                let g = *rng.pick(&[0x0009u32, 0x0019, 0x0029, 0x0043, 0x2001, 0x0008, 0x0018]);
                ((g << 16) | rng.below(0x1100) as u32, if rng.chance(0.5) { 21 } else { 10 })
            } else if r < 92 {
                // a group length
                let g = *rng.pick(&[0x0008u32, 0x0010, 0x0018, 0x0020, 0x0028, 0x0011]);
                (g << 16, 20)
            } else {
                // the elements CTN reads for itself
                *rng.pick(&[
                    (0x0028_0100u32, 22u32),
                    (0x0028_0103, 22),
                    (0x0028_0106, 24),
                    (0x0028_3006, 24),
                    (0x003a_0103, 3),
                    (0x003a_1000, 24),
                    (0x0008_1140, 15),
                ])
            };
            picked.push(t);
        }
        picked.sort();
        picked.dedup_by_key(|x| x.0);
        let mut o = Vec::new();
        for (tag, vr) in picked {
            if tag == 0x7fe0_0010 {
                continue;
            }
            let mut vr = vr;
            if vr == 23 || vr == 24 || vr == 29 {
                vr = if rng.chance(0.5) { 22 } else { 16 };
            }
            if vr == 15 && depth < 3 {
                self.sequence(rng, &mut o, tag, enc, depth);
                continue;
            } else if vr == 15 {
                vr = 26;
            }
            let v = value(rng, vr, enc);
            let code = if enc.explicit && rng.chance(0.03) {
                *rng.pick(&[*b"OB", *b"UN", *b"XX", *b"ob", *b"\0\0", *b"  ", *b"US", *b"SQ"])
            } else {
                vr_code(vr)
            };
            element(&mut o, tag, code, v.len() as u32, &v, enc);
        }
        if depth == 0 && rng.chance(0.4) {
            self.pixels(rng, &mut o, enc);
        }
        o
    }

    fn sequence(&self, rng: &mut Rng, o: &mut Vec<u8>, tag: u32, enc: Enc, depth: u32) {
        let undefined = rng.chance(0.5);
        let mut body = Vec::new();
        let items = rng.below(4);
        for _ in 0..items {
            let inner = self.dataset(rng, enc, depth + 1, 6);
            if rng.chance(0.5) {
                element(&mut body, 0xfffe_e000, *b"\0\0", 0xffff_ffff, &inner, enc);
                element(&mut body, 0xfffe_e00d, *b"\0\0", 0, &[], enc);
            } else {
                element(&mut body, 0xfffe_e000, *b"\0\0", inner.len() as u32, &inner, enc);
            }
        }
        let code = if rng.chance(0.15) { *b"UN" } else { *b"SQ" };
        if undefined {
            element(&mut body, 0xfffe_e0dd, *b"\0\0", 0, &[], enc);
            element(o, tag, code, 0xffff_ffff, &body, enc);
        } else {
            element(o, tag, code, body.len() as u32, &body, enc);
        }
    }

    fn pixels(&self, rng: &mut Rng, o: &mut Vec<u8>, enc: Enc) {
        if rng.chance(0.5) {
            let n = 2 * rng.below(200) as usize;
            let v = rng.bytes(n);
            element(o, 0x7fe0_0010, if rng.chance(0.5) { *b"OW" } else { *b"OB" }, n as u32, &v, enc);
        } else {
            let mut body = Vec::new();
            for _ in 0..rng.below(4) {
                let mut n = 2 * rng.below(40) as usize;
                if rng.chance(0.05) {
                    n += 1;
                }
                let v = rng.bytes(n);
                element(&mut body, 0xfffe_e000, *b"\0\0", n as u32, &v, enc);
            }
            if rng.chance(0.9) {
                element(&mut body, 0xfffe_e0dd, *b"\0\0", 0, &[], enc);
            }
            element(o, 0x7fe0_0010, *b"OB", 0xffff_ffff, &body, enc);
        }
    }

    /// A part 10 file: preamble, DICM, the file meta group (explicit little endian), the data set.
    pub fn part10(&self, rng: &mut Rng) -> Vec<u8> {
        let ts: &[u8] = rng.pick(&[
            &b"1.2.840.10008.1.2\0"[..],
            b"1.2.840.10008.1.2.1\0",
            b"1.2.840.10008.1.2.2\0",
            b"1.2.840.10008.1.2.4.70",
            b"1.2.840.10008.1.2.1 ",
            b"1.2.840.10008.1.2 ",
            b"1.2.840.10008.1.2.5\0",
        ]);
        let enc = match ts {
            b"1.2.840.10008.1.2\0" => Enc { big: false, explicit: false },
            b"1.2.840.10008.1.2.2\0" => Enc { big: true, explicit: true },
            _ => Enc { big: false, explicit: true },
        };
        let mle = Enc { big: false, explicit: true };
        let mut meta = Vec::new();
        element(&mut meta, 0x0002_0001, *b"OB", 2, &[0, 1], mle);
        let sop = b"1.2.840.10008.5.1.4.1.1.4\0";
        element(&mut meta, 0x0002_0002, *b"UI", sop.len() as u32, sop, mle);
        let inst = b"1.2.3.4.5.6.7\0";
        element(&mut meta, 0x0002_0003, *b"UI", inst.len() as u32, inst, mle);
        if !rng.chance(0.05) {
            element(&mut meta, 0x0002_0010, *b"UI", ts.len() as u32, ts, mle);
        }
        let impl_uid = b"1.2.276.0.7230010\0";
        element(&mut meta, 0x0002_0012, *b"UI", impl_uid.len() as u32, impl_uid, mle);
        let mut o = if rng.chance(0.8) { vec![0u8; 128] } else { rng.bytes(128) };
        o.extend_from_slice(b"DICM");
        let gl = match rng.below(20) {
            0 => meta.len() as u32 + 2,
            1 => meta.len() as u32 - 2,
            2 => rng.next() as u32,
            _ => meta.len() as u32,
        };
        if !rng.chance(0.03) {
            element(&mut o, 0x0002_0000, *b"UL", 4, &gl.to_le_bytes(), mle);
        }
        o.extend_from_slice(&meta);
        o.extend_from_slice(&self.dataset(rng, enc, 0, 30));
        o
    }

    /// A bare data set in `enc`.
    pub fn bare(&self, rng: &mut Rng, enc: Enc) -> Vec<u8> {
        self.dataset(rng, enc, 0, 30)
    }
}

/// A random mutation of `f`.
pub fn mutate(rng: &mut Rng, f: &mut Vec<u8>) {
    if f.is_empty() {
        return;
    }
    match rng.below(7) {
        0 => {
            let n = rng.below(f.len() as u64) as usize;
            f.truncate(n);
        }
        1 => {
            for _ in 0..1 + rng.below(4) {
                let i = rng.below(f.len() as u64) as usize;
                f[i] ^= 1 << rng.below(8);
            }
        }
        2 if f.len() > 8 => {
            let i = rng.below(f.len() as u64 - 4) as usize;
            let v: u32 = *rng.pick(&[0xffff_ffffu32, 0xffff_fffe, 0x7fff_fffe, 3, 1, 0x8000_0000, 0x10]);
            f[i..i + 4].copy_from_slice(&v.to_le_bytes());
        }
        3 if f.len() > 16 => {
            // a stray delimiter
            let i = 2 * rng.below(f.len() as u64 / 2) as usize;
            let d: &[u8] = rng.pick(&[
                &[0xfe, 0xff, 0x0d, 0xe0, 0, 0, 0, 0][..],
                &[0xfe, 0xff, 0xdd, 0xe0, 0, 0, 0, 0],
                &[0xfe, 0xff, 0x00, 0xe0, 0xff, 0xff, 0xff, 0xff],
            ]);
            let mut g = f[..i].to_vec();
            g.extend_from_slice(d);
            g.extend_from_slice(&f[i..]);
            *f = g;
        }
        4 if f.len() > 8 => {
            let i = rng.below(f.len() as u64 - 2) as usize;
            f[i] = rng.next() as u8;
            f[i + 1] = rng.next() as u8;
        }
        5 => {
            let n = rng.below(64) as usize;
            let b = rng.bytes(n);
            f.extend_from_slice(&b);
        }
        _ => {
            let i = rng.below(f.len() as u64) as usize;
            f.drain(i..(i + 1 + rng.below(8) as usize).min(f.len()));
        }
    }
}

/// A file that is not DICOM (or barely).
pub fn other(rng: &mut Rng) -> Vec<u8> {
    match rng.below(8) {
        0 => {
            let n = rng.below(300) as usize;
            rng.bytes(n)
        }
        1 => {
            // a NIfTI-1 header
            let mut h = vec![0u8; 348];
            h[0..4].copy_from_slice(&348i32.to_le_bytes());
            h[344..348].copy_from_slice(b"n+1\0");
            let n = rng.below(64) as usize;
            h.extend(rng.bytes(n));
            h
        }
        2 => {
            // an MGH header
            let mut h = Vec::new();
            for v in [1i32, 4, 5, 6, 1, 3] {
                h.extend_from_slice(&v.to_be_bytes());
            }
            h.resize(284, 0);
            h
        }
        3 => b"# a text file\nnot dicom at all\n".repeat(1 + rng.below(5) as usize),
        4 => vec![0u8; rng.below(400) as usize],
        5 => {
            let mut h = vec![0u8; rng.below(132) as usize];
            if h.len() >= 4 {
                let n = h.len();
                h[n - 4..].copy_from_slice(b"DICM");
            }
            h
        }
        6 => {
            let mut h = vec![0u8; 128];
            h.extend_from_slice(b"DICM");
            let n = rng.below(32) as usize;
            h.extend(rng.bytes(n));
            h
        }
        _ => {
            // a gzip file
            let mut h = vec![0x1f, 0x8b, 8, 0];
            let n = rng.below(200) as usize;
            h.extend(rng.bytes(n));
            h
        }
    }
}
