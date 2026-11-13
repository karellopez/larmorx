// SPDX-License-Identifier: Apache-2.0
//! Deterministic deflate streams for the checks of [`super`] (larmorx's own writer, from RFC
//! 1951 and RFC 1950; not miniz's): stored, fixed and dynamic blocks from tokens, random
//! complete Huffman codes, the code-length code with its repeat symbols, and symbols deflate
//! does not define (FS-282) or patterns a code does not cover (FS-283) on request. Seeded,
//! integer arithmetic only: the same bytes on every platform.

/// splitmix64.
#[derive(Clone, Debug)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// A number in `0..n` (`n > 0`).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// An LSB-first bit writer.
#[derive(Clone, Debug, Default)]
pub struct Bits {
    pub out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    pub fn put(&mut self, v: u32, nbits: u32) {
        self.acc |= ((v as u64) & ((1u64 << nbits) - 1)) << self.n;
        self.n += nbits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    /// A Huffman code, its first bit the code's most significant.
    pub fn code(&mut self, code: u32, len: u32) {
        for i in (0..len).rev() {
            self.put((code >> i) & 1, 1);
        }
    }
    pub fn align(&mut self) {
        if self.n > 0 {
            self.put(0, 8 - self.n);
        }
    }
    pub fn finish(mut self) -> Vec<u8> {
        self.align();
        self.out
    }
}

/// One token of a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Lit(u8),
    /// A match (length 3..=258, distance 1..=32768).
    Match(u32, u32),
    /// A length symbol (257..=287) with its extra bits' value, then a distance symbol
    /// (0..=31) with its extra bits' value, or none: undefined symbols on purpose.
    Raw(u32, u32, Option<(u32, u32)>),
    /// Bits as they are (`value`, `count`): patterns a code does not cover.
    Bits(u32, u32),
}

const LEN_BASE: [u32; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u32; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// (symbol, extra bits, their value) of a length.
pub fn length_symbol(len: u32) -> (u32, u32, u32) {
    let i = (0..29).rev().find(|&i| LEN_BASE[i] <= len).unwrap();
    let i = if len == 258 { 28 } else { i.min(27) };
    (257 + i as u32, LEN_EXTRA[i], len - LEN_BASE[i])
}

/// (symbol, extra bits, their value) of a distance.
pub fn dist_symbol(dist: u32) -> (u32, u32, u32) {
    let i = (0..30).rev().find(|&i| DIST_BASE[i] <= dist).unwrap();
    (i as u32, DIST_EXTRA[i], dist - DIST_BASE[i])
}

/// Canonical codes of the lengths (RFC 1951 §3.2.2).
pub fn canonical(lens: &[u32]) -> Vec<u32> {
    let mut count = [0u32; 16];
    for &l in lens {
        count[l as usize] += 1;
    }
    count[0] = 0;
    let mut next = [0u32; 16];
    let mut code = 0;
    for b in 1..16 {
        code = (code + count[b - 1]) << 1;
        next[b] = code;
    }
    lens.iter()
        .map(|&l| {
            if l == 0 {
                0
            } else {
                let c = next[l as usize];
                next[l as usize] += 1;
                c
            }
        })
        .collect()
}

/// Random code lengths over `n` symbols: a complete code (Kraft sum 1) of at most `max_len`
/// bits (`n >= 2`), a single code of length 1 for `n == 1`.
pub fn random_lengths(rng: &mut Rng, n: usize, max_len: u32) -> Vec<u32> {
    if n == 1 {
        return vec![1];
    }
    let mut leaves = vec![0u32];
    while leaves.len() < n {
        let open: Vec<usize> = (0..leaves.len()).filter(|&i| leaves[i] < max_len).collect();
        let i = open[rng.below(open.len() as u64) as usize];
        let d = leaves[i] + 1;
        leaves[i] = d;
        leaves.push(d);
    }
    for i in (1..leaves.len()).rev() {
        let j = rng.below(i as u64 + 1) as usize;
        leaves.swap(i, j);
    }
    leaves
}

/// A stored block.
pub fn stored(b: &mut Bits, data: &[u8], last: bool) {
    b.put(last as u32, 1);
    b.put(0, 2);
    b.align();
    let n = data.len() as u32;
    b.put(n & 0xffff, 16);
    b.put(!n & 0xffff, 16);
    for &x in data {
        b.put(x as u32, 8);
    }
}

fn tokens(b: &mut Bits, toks: &[Tok], ll: &[u32], lc: &[u32], dl: &[u32], dc: &[u32]) {
    for &t in toks {
        match t {
            Tok::Lit(x) => b.code(lc[x as usize], ll[x as usize]),
            Tok::Match(len, dist) => {
                let (s, e, v) = length_symbol(len);
                b.code(lc[s as usize], ll[s as usize]);
                b.put(v, e);
                let (s, e, v) = dist_symbol(dist);
                b.code(dc[s as usize], dl[s as usize]);
                b.put(v, e);
            }
            Tok::Raw(ls, lv, d) => {
                b.code(lc[ls as usize], ll[ls as usize]);
                let e = if (257..285).contains(&ls) {
                    LEN_EXTRA[(ls - 257) as usize]
                } else {
                    0
                };
                b.put(lv, e);
                if let Some((ds, dv)) = d {
                    b.code(dc[ds as usize], dl[ds as usize]);
                    b.put(dv, if ds < 30 { DIST_EXTRA[ds as usize] } else { 0 });
                }
            }
            Tok::Bits(v, n) => b.put(v, n),
        }
    }
    b.code(lc[256], ll[256]);
}

/// The fixed code's lengths (288 and 32 symbols).
pub fn fixed_lengths() -> (Vec<u32>, Vec<u32>) {
    let mut ll = vec![8u32; 288];
    ll[144..256].fill(9);
    ll[256..280].fill(7);
    (ll, vec![5u32; 32])
}

/// A block with the fixed code.
pub fn fixed(b: &mut Bits, toks: &[Tok], last: bool) {
    b.put(last as u32, 1);
    b.put(1, 2);
    let (ll, dl) = fixed_lengths();
    let (lc, dc) = (canonical(&ll), canonical(&dl));
    tokens(b, toks, &ll, &lc, &dl, &dc);
}

/// A block with its own code: the literal/length lengths `ll` (257..=288 of them) and the
/// distance lengths `dl` (1..=32), sent through a random code-length code that uses the
/// repeat symbols where `rle`.
pub fn dynamic(
    b: &mut Bits,
    rng: &mut Rng,
    toks: &[Tok],
    ll: &[u32],
    dl: &[u32],
    rle: bool,
    last: bool,
) {
    let all: Vec<u32> = ll.iter().chain(dl.iter()).copied().collect();
    // The code-length symbols, with their extra bits.
    let mut syms: Vec<(u32, u32, u32)> = Vec::new();
    let mut i = 0;
    while i < all.len() {
        let l = all[i];
        let mut run = 1;
        while i + run < all.len() && all[i + run] == l {
            run += 1;
        }
        if rle && l == 0 && run >= 3 {
            let r = run.min(138);
            if r >= 11 {
                syms.push((18, 7, (r - 11) as u32));
            } else {
                syms.push((17, 3, (r - 3) as u32));
            }
            i += r;
        } else if rle && i > 0 && all[i - 1] == l && run >= 3 {
            let r = run.min(6);
            syms.push((16, 2, (r - 3) as u32));
            i += r;
        } else {
            syms.push((l, 0, 0));
            i += 1;
        }
    }
    let mut used: Vec<u32> = syms.iter().map(|s| s.0).collect();
    used.sort();
    used.dedup();
    let mut cl = [0u32; 19];
    let lens = random_lengths(rng, used.len().max(2), 7);
    for (k, &s) in used.iter().enumerate() {
        cl[s as usize] = lens[k];
    }
    if used.len() == 1 {
        // a complete code needs two: give an unused symbol the other length-1 code
        let other = if used[0] == 0 { 1 } else { 0 };
        cl[other] = 1;
    }
    let order = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut hclen = 19;
    while hclen > 4 && cl[order[hclen - 1]] == 0 {
        hclen -= 1;
    }
    b.put(last as u32, 1);
    b.put(2, 2);
    b.put((ll.len() - 257) as u32, 5);
    b.put((dl.len() - 1) as u32, 5);
    b.put((hclen - 4) as u32, 4);
    for &o in &order[..hclen] {
        b.put(cl[o], 3);
    }
    let cc = canonical(&cl);
    for (s, e, v) in syms {
        b.code(cc[s as usize], cl[s as usize]);
        b.put(v, e);
    }
    let (lc, dc) = (canonical(ll), canonical(dl));
    tokens(b, toks, ll, &lc, dl, &dc);
}

/// Random tokens making `n` bytes of output after `history` (matches only within it and the
/// new bytes, `max_dist` back at most), with the bytes they make.
pub fn random_tokens(
    rng: &mut Rng,
    history: &[u8],
    n: usize,
    alphabet: u32,
    max_dist: u32,
) -> (Vec<Tok>, Vec<u8>) {
    let mut out = history.to_vec();
    let mut toks = Vec::new();
    let end = history.len() + n;
    while out.len() < end {
        let room = end - out.len();
        let avail = (out.len() as u32).min(max_dist);
        if avail > 0 && room >= 3 && rng.below(3) != 0 {
            let len = 3 + rng.below((room.min(258) - 2) as u64) as u32;
            let dist = 1 + rng.below(avail as u64) as u32;
            for _ in 0..len {
                let x = out[out.len() - dist as usize];
                out.push(x);
            }
            toks.push(Tok::Match(len, dist));
        } else {
            let x = rng.below(alphabet as u64) as u8;
            out.push(x);
            toks.push(Tok::Lit(x));
        }
    }
    (toks, out[history.len()..].to_vec())
}

/// Random code lengths for a block that uses `toks`: every symbol they use gets a code.
pub fn lengths_for(rng: &mut Rng, toks: &[Tok]) -> (Vec<u32>, Vec<u32>) {
    let nl = 257 + rng.below(30) as usize;
    let nd = 1 + rng.below(30) as usize;
    let mut need_l = 257usize;
    let mut need_d = 1usize;
    for &t in toks {
        match t {
            Tok::Match(len, dist) => {
                need_l = need_l.max(length_symbol(len).0 as usize + 1);
                need_d = need_d.max(dist_symbol(dist).0 as usize + 1);
            }
            Tok::Raw(ls, _, d) => {
                need_l = need_l.max(ls as usize + 1);
                if let Some((ds, _)) = d {
                    need_d = need_d.max(ds as usize + 1);
                }
            }
            Tok::Lit(_) | Tok::Bits(..) => {}
        }
    }
    let (nl, nd) = (nl.max(need_l), nd.max(need_d));
    let ll = random_lengths(rng, nl, 15);
    let dl = if nd == 1 {
        vec![1]
    } else {
        random_lengths(rng, nd, 15)
    };
    (ll, dl)
}

/// The zlib wrapping (RFC 1950) of a raw stream for `data`: `78 9C`, the stream, adler-32.
pub fn zlib(raw: &[u8], data: &[u8]) -> Vec<u8> {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    let mut v = vec![0x78, 0x9c];
    v.extend_from_slice(raw);
    v.extend_from_slice(&((b << 16) | a).to_be_bytes());
    v
}
