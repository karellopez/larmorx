// SPDX-License-Identifier: Apache-2.0 AND Zlib
//! zlib 1.2.11's `trees.c` (Ubuntu's `sym_buf` layout): the Huffman trees, the bit buffer and
//! the block writers. A tree entry keeps zlib's two unions (`Freq`/`Code`, `Dad`/`Len`) as two
//! fields, so that every write that overwrites the other member does so here too.

use std::sync::OnceLock;

pub(super) const LENGTH_CODES: usize = 29;
pub(super) const LITERALS: usize = 256;
pub(super) const L_CODES: usize = LITERALS + 1 + LENGTH_CODES;
pub(super) const D_CODES: usize = 30;
pub(super) const BL_CODES: usize = 19;
pub(super) const HEAP_SIZE: usize = 2 * L_CODES + 1;
const MAX_BITS: usize = 15;
const MAX_BL_BITS: i32 = 7;
const END_BLOCK: usize = 256;
const REP_3_6: usize = 16;
const REPZ_3_10: usize = 17;
const REPZ_11_138: usize = 18;
const BUF_SIZE: i32 = 16;
pub(super) const MIN_MATCH: u32 = 3;
pub(super) const MAX_MATCH: u32 = 258;

const STORED_BLOCK: i32 = 0;
const STATIC_TREES: i32 = 1;
const DYN_TREES: i32 = 2;

const EXTRA_LBITS: [i32; LENGTH_CODES] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const EXTRA_DBITS: [i32; D_CODES] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const EXTRA_BLBITS: [i32; BL_CODES] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7];
const BL_ORDER: [usize; BL_CODES] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// `ct_data`: `fc` is `Freq` or `Code`, `dl` is `Dad` or `Len`.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct Ct {
    pub(super) fc: u16,
    pub(super) dl: u16,
}

/// The static tables `tr_static_init` builds (`trees.h`).
pub(super) struct Tables {
    pub(super) static_ltree: [Ct; L_CODES + 2],
    pub(super) static_dtree: [Ct; D_CODES],
    pub(super) dist_code: [u8; 512],
    pub(super) length_code: [u8; (MAX_MATCH - MIN_MATCH + 1) as usize],
    base_length: [i32; LENGTH_CODES],
    base_dist: [i32; D_CODES],
}

/// `bi_reverse(code, len)`.
fn bi_reverse(mut code: u32, mut len: i32) -> u32 {
    let mut res = 0u32;
    loop {
        res |= code & 1;
        code >>= 1;
        res <<= 1;
        len -= 1;
        if len <= 0 {
            break;
        }
    }
    res >> 1
}

/// `gen_codes(tree, max_code, bl_count)`.
fn gen_codes(tree: &mut [Ct], max_code: i32, bl_count: &[u16; MAX_BITS + 1]) {
    let mut next_code = [0u16; MAX_BITS + 1];
    let mut code: u32 = 0;
    for bits in 1..=MAX_BITS {
        code = (code + u32::from(bl_count[bits - 1])) << 1;
        next_code[bits] = code as u16;
    }
    for n in 0..=max_code {
        let len = tree[n as usize].dl as i32;
        if len == 0 {
            continue;
        }
        let c = next_code[len as usize];
        next_code[len as usize] = c.wrapping_add(1);
        tree[n as usize].fc = bi_reverse(u32::from(c), len) as u16;
    }
}

pub(super) fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Tables {
            static_ltree: [Ct::default(); L_CODES + 2],
            static_dtree: [Ct::default(); D_CODES],
            dist_code: [0; 512],
            length_code: [0; (MAX_MATCH - MIN_MATCH + 1) as usize],
            base_length: [0; LENGTH_CODES],
            base_dist: [0; D_CODES],
        };
        let mut length = 0usize;
        let mut code = 0usize;
        while code < LENGTH_CODES - 1 {
            t.base_length[code] = length as i32;
            for _ in 0..(1 << EXTRA_LBITS[code]) {
                t.length_code[length] = code as u8;
                length += 1;
            }
            code += 1;
        }
        t.length_code[length - 1] = code as u8;
        let mut dist = 0usize;
        code = 0;
        while code < 16 {
            t.base_dist[code] = dist as i32;
            for _ in 0..(1 << EXTRA_DBITS[code]) {
                t.dist_code[dist] = code as u8;
                dist += 1;
            }
            code += 1;
        }
        dist >>= 7;
        while code < D_CODES {
            t.base_dist[code] = (dist << 7) as i32;
            for _ in 0..(1 << (EXTRA_DBITS[code] - 7)) {
                t.dist_code[256 + dist] = code as u8;
                dist += 1;
            }
            code += 1;
        }
        let mut bl_count = [0u16; MAX_BITS + 1];
        let mut n = 0usize;
        while n <= 143 {
            t.static_ltree[n].dl = 8;
            bl_count[8] += 1;
            n += 1;
        }
        while n <= 255 {
            t.static_ltree[n].dl = 9;
            bl_count[9] += 1;
            n += 1;
        }
        while n <= 279 {
            t.static_ltree[n].dl = 7;
            bl_count[7] += 1;
            n += 1;
        }
        while n <= 287 {
            t.static_ltree[n].dl = 8;
            bl_count[8] += 1;
            n += 1;
        }
        gen_codes(&mut t.static_ltree, (L_CODES + 1) as i32, &bl_count);
        for n in 0..D_CODES {
            t.static_dtree[n].dl = 5;
            t.static_dtree[n].fc = bi_reverse(n as u32, 5) as u16;
        }
        t
    })
}

/// `d_code(dist)`.
#[inline]
pub(super) fn d_code(t: &Tables, dist: u32) -> usize {
    if dist < 256 {
        t.dist_code[dist as usize] as usize
    } else {
        t.dist_code[256 + (dist >> 7) as usize] as usize
    }
}

/// Which static description a tree has (`static_l_desc`, `static_d_desc`, `static_bl_desc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Desc {
    L,
    D,
    Bl,
}

impl Desc {
    fn elems(self) -> usize {
        match self {
            Desc::L => L_CODES,
            Desc::D => D_CODES,
            Desc::Bl => BL_CODES,
        }
    }
    fn extra_base(self) -> i32 {
        match self {
            Desc::L => (LITERALS + 1) as i32,
            _ => 0,
        }
    }
    fn max_length(self) -> i32 {
        match self {
            Desc::Bl => MAX_BL_BITS,
            _ => MAX_BITS as i32,
        }
    }
    fn extra(self, i: usize) -> i32 {
        match self {
            Desc::L => EXTRA_LBITS[i],
            Desc::D => EXTRA_DBITS[i],
            Desc::Bl => EXTRA_BLBITS[i],
        }
    }
    fn stree(self, t: &Tables) -> Option<&[Ct]> {
        match self {
            Desc::L => Some(&t.static_ltree),
            Desc::D => Some(&t.static_dtree),
            Desc::Bl => None,
        }
    }
}

/// The heap and the lengths `build_tree` works with (the parts of `deflate_state` it uses).
#[derive(Clone)]
pub(super) struct Heap {
    pub(super) bl_count: [u16; MAX_BITS + 1],
    heap: [i32; 2 * L_CODES + 1],
    heap_len: i32,
    heap_max: i32,
    depth: [u8; 2 * L_CODES + 1],
    pub(super) opt_len: u64,
    pub(super) static_len: u64,
}

impl Heap {
    pub(super) fn new() -> Heap {
        Heap {
            bl_count: [0; MAX_BITS + 1],
            heap: [0; 2 * L_CODES + 1],
            heap_len: 0,
            heap_max: 0,
            depth: [0; 2 * L_CODES + 1],
            opt_len: 0,
            static_len: 0,
        }
    }

    #[inline]
    fn smaller(&self, tree: &[Ct], n: i32, m: i32) -> bool {
        let (tn, tm) = (tree[n as usize].fc, tree[m as usize].fc);
        tn < tm || (tn == tm && self.depth[n as usize] <= self.depth[m as usize])
    }

    /// `pqdownheap(s, tree, k)`.
    fn pqdownheap(&mut self, tree: &[Ct], mut k: i32) {
        let v = self.heap[k as usize];
        let mut j = k << 1;
        while j <= self.heap_len {
            if j < self.heap_len
                && self.smaller(tree, self.heap[(j + 1) as usize], self.heap[j as usize])
            {
                j += 1;
            }
            if self.smaller(tree, v, self.heap[j as usize]) {
                break;
            }
            self.heap[k as usize] = self.heap[j as usize];
            k = j;
            j <<= 1;
        }
        self.heap[k as usize] = v;
    }

    /// `gen_bitlen(s, desc)`.
    fn gen_bitlen(&mut self, tree: &mut [Ct], desc: Desc, max_code: i32, t: &Tables) {
        let stree = desc.stree(t);
        let base = desc.extra_base();
        let max_length = desc.max_length();
        let mut overflow = 0i32;
        self.bl_count = [0; MAX_BITS + 1];
        tree[self.heap[self.heap_max as usize] as usize].dl = 0;
        let mut h = self.heap_max + 1;
        while (h as usize) < HEAP_SIZE {
            let n = self.heap[h as usize];
            let mut bits = i32::from(tree[tree[n as usize].dl as usize].dl) + 1;
            if bits > max_length {
                bits = max_length;
                overflow += 1;
            }
            tree[n as usize].dl = bits as u16;
            h += 1;
            if n > max_code {
                continue;
            }
            self.bl_count[bits as usize] += 1;
            let mut xbits = 0;
            if n >= base {
                xbits = desc.extra((n - base) as usize);
            }
            let f = u64::from(tree[n as usize].fc);
            self.opt_len = self
                .opt_len
                .wrapping_add(f.wrapping_mul((bits + xbits) as u32 as u64));
            if let Some(st) = stree {
                let sl = (i32::from(st[n as usize].dl) + xbits) as u32 as u64;
                self.static_len = self.static_len.wrapping_add(f.wrapping_mul(sl));
            }
        }
        if overflow == 0 {
            return;
        }
        loop {
            let mut bits = (max_length - 1) as usize;
            while self.bl_count[bits] == 0 {
                bits -= 1;
            }
            self.bl_count[bits] -= 1;
            self.bl_count[bits + 1] += 2;
            self.bl_count[max_length as usize] = self.bl_count[max_length as usize].wrapping_sub(1);
            overflow -= 2;
            if overflow <= 0 {
                break;
            }
        }
        let mut bits = max_length;
        while bits != 0 {
            let mut n = self.bl_count[bits as usize];
            while n != 0 {
                h -= 1;
                let m = self.heap[h as usize];
                if m > max_code {
                    continue;
                }
                if u32::from(tree[m as usize].dl) != bits as u32 {
                    let d = (bits as u64).wrapping_sub(u64::from(tree[m as usize].dl));
                    self.opt_len = self
                        .opt_len
                        .wrapping_add(d.wrapping_mul(u64::from(tree[m as usize].fc)));
                    tree[m as usize].dl = bits as u16;
                }
                n -= 1;
            }
            bits -= 1;
        }
    }

    /// `build_tree(s, desc)`: returns the new `max_code`.
    pub(super) fn build_tree(&mut self, tree: &mut [Ct], desc: Desc, t: &Tables) -> i32 {
        let stree = desc.stree(t);
        let elems = desc.elems();
        let mut max_code: i32 = -1;
        self.heap_len = 0;
        self.heap_max = HEAP_SIZE as i32;
        for (n, t) in tree.iter_mut().enumerate().take(elems) {
            if t.fc != 0 {
                self.heap_len += 1;
                self.heap[self.heap_len as usize] = n as i32;
                max_code = n as i32;
                self.depth[n] = 0;
            } else {
                t.dl = 0;
            }
        }
        while self.heap_len < 2 {
            let node = if max_code < 2 {
                max_code += 1;
                max_code
            } else {
                0
            };
            self.heap_len += 1;
            self.heap[self.heap_len as usize] = node;
            tree[node as usize].fc = 1;
            self.depth[node as usize] = 0;
            self.opt_len = self.opt_len.wrapping_sub(1);
            if let Some(st) = stree {
                self.static_len = self
                    .static_len
                    .wrapping_sub(u64::from(st[node as usize].dl));
            }
        }
        let mut n = self.heap_len / 2;
        while n >= 1 {
            self.pqdownheap(tree, n);
            n -= 1;
        }
        let mut node = elems as i32;
        loop {
            // pqremove
            let n = self.heap[1];
            self.heap[1] = self.heap[self.heap_len as usize];
            self.heap_len -= 1;
            self.pqdownheap(tree, 1);
            let m = self.heap[1];
            self.heap_max -= 1;
            self.heap[self.heap_max as usize] = n;
            self.heap_max -= 1;
            self.heap[self.heap_max as usize] = m;
            tree[node as usize].fc = tree[n as usize].fc.wrapping_add(tree[m as usize].fc);
            let (dn, dm) = (self.depth[n as usize], self.depth[m as usize]);
            self.depth[node as usize] = (if dn >= dm { dn } else { dm }).wrapping_add(1);
            tree[n as usize].dl = node as u16;
            tree[m as usize].dl = node as u16;
            self.heap[1] = node;
            node += 1;
            self.pqdownheap(tree, 1);
            if self.heap_len < 2 {
                break;
            }
        }
        self.heap_max -= 1;
        self.heap[self.heap_max as usize] = self.heap[1];
        self.gen_bitlen(tree, desc, max_code, t);
        let bl_count = self.bl_count;
        gen_codes(tree, max_code, &bl_count);
        max_code
    }
}

/// `scan_tree(s, tree, max_code)`.
pub(super) fn scan_tree(bl_tree: &mut [Ct], tree: &mut [Ct], max_code: i32) {
    let mut prevlen: i32 = -1;
    let mut nextlen = i32::from(tree[0].dl);
    let mut count = 0;
    let (mut max_count, mut min_count) = (7, 4);
    if nextlen == 0 {
        max_count = 138;
        min_count = 3;
    }
    tree[(max_code + 1) as usize].dl = 0xffff;
    for n in 0..=max_code {
        let curlen = nextlen;
        nextlen = i32::from(tree[(n + 1) as usize].dl);
        count += 1;
        if count < max_count && curlen == nextlen {
            continue;
        } else if count < min_count {
            let f = &mut bl_tree[curlen as usize].fc;
            *f = f.wrapping_add(count as u16);
        } else if curlen != 0 {
            if curlen != prevlen {
                bl_tree[curlen as usize].fc = bl_tree[curlen as usize].fc.wrapping_add(1);
            }
            bl_tree[REP_3_6].fc = bl_tree[REP_3_6].fc.wrapping_add(1);
        } else if count <= 10 {
            bl_tree[REPZ_3_10].fc = bl_tree[REPZ_3_10].fc.wrapping_add(1);
        } else {
            bl_tree[REPZ_11_138].fc = bl_tree[REPZ_11_138].fc.wrapping_add(1);
        }
        count = 0;
        prevlen = curlen;
        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        } else if curlen == nextlen {
            max_count = 6;
            min_count = 3;
        } else {
            max_count = 7;
            min_count = 4;
        }
    }
}

/// The pending buffer and the bit buffer (`pending_buf`, `pending`, `pending_out`, `bi_buf`,
/// `bi_valid`); `sym_buf` lies in the same buffer, from `lit_bufsize`, as in zlib.
#[derive(Clone)]
pub(super) struct Out {
    pub(super) buf: Vec<u8>,
    pub(super) pending: usize,
    pub(super) pending_out: usize,
    pub(super) bi_buf: u16,
    pub(super) bi_valid: i32,
}

impl Out {
    #[inline]
    pub(super) fn put_byte(&mut self, c: u8) {
        self.buf[self.pending] = c;
        self.pending += 1;
    }

    #[inline]
    fn put_short(&mut self, w: u16) {
        self.put_byte((w & 0xff) as u8);
        self.put_byte((w >> 8) as u8);
    }

    /// `send_bits(s, value, length)`.
    #[inline]
    pub(super) fn send_bits(&mut self, value: i32, length: i32) {
        if self.bi_valid > BUF_SIZE - length {
            let val = value as u16 as u32;
            self.bi_buf |= (val << self.bi_valid) as u16;
            let b = self.bi_buf;
            self.put_short(b);
            self.bi_buf = (val >> (BUF_SIZE - self.bi_valid)) as u16;
            self.bi_valid += length - BUF_SIZE;
        } else {
            self.bi_buf |= ((value as u16 as u32) << self.bi_valid) as u16;
            self.bi_valid += length;
        }
    }

    #[inline]
    fn send_code(&mut self, c: usize, tree: &[Ct]) {
        self.send_bits(i32::from(tree[c].fc), i32::from(tree[c].dl));
    }

    /// `bi_flush(s)`.
    pub(super) fn bi_flush(&mut self) {
        if self.bi_valid == 16 {
            let b = self.bi_buf;
            self.put_short(b);
            self.bi_buf = 0;
            self.bi_valid = 0;
        } else if self.bi_valid >= 8 {
            self.put_byte(self.bi_buf as u8);
            self.bi_buf >>= 8;
            self.bi_valid -= 8;
        }
    }

    /// `bi_windup(s)`.
    pub(super) fn bi_windup(&mut self) {
        if self.bi_valid > 8 {
            let b = self.bi_buf;
            self.put_short(b);
        } else if self.bi_valid > 0 {
            self.put_byte(self.bi_buf as u8);
        }
        self.bi_buf = 0;
        self.bi_valid = 0;
    }

    /// `send_tree(s, tree, max_code)`.
    fn send_tree(&mut self, bl_tree: &[Ct], tree: &[Ct], max_code: i32) {
        let mut prevlen: i32 = -1;
        let mut nextlen = i32::from(tree[0].dl);
        let mut count: i32 = 0;
        let (mut max_count, mut min_count) = (7, 4);
        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        }
        for n in 0..=max_code {
            let curlen = nextlen;
            nextlen = i32::from(tree[(n + 1) as usize].dl);
            count += 1;
            if count < max_count && curlen == nextlen {
                continue;
            } else if count < min_count {
                loop {
                    self.send_code(curlen as usize, bl_tree);
                    count -= 1;
                    if count == 0 {
                        break;
                    }
                }
            } else if curlen != 0 {
                if curlen != prevlen {
                    self.send_code(curlen as usize, bl_tree);
                    count -= 1;
                }
                self.send_code(REP_3_6, bl_tree);
                self.send_bits(count - 3, 2);
            } else if count <= 10 {
                self.send_code(REPZ_3_10, bl_tree);
                self.send_bits(count - 3, 3);
            } else {
                self.send_code(REPZ_11_138, bl_tree);
                self.send_bits(count - 11, 7);
            }
            count = 0;
            prevlen = curlen;
            if nextlen == 0 {
                max_count = 138;
                min_count = 3;
            } else if curlen == nextlen {
                max_count = 6;
                min_count = 3;
            } else {
                max_count = 7;
                min_count = 4;
            }
        }
    }

    /// `send_all_trees(s, lcodes, dcodes, blcodes)`.
    pub(super) fn send_all_trees(&mut self, trees: &Trees, lcodes: i32, dcodes: i32, blcodes: i32) {
        self.send_bits(lcodes - 257, 5);
        self.send_bits(dcodes - 1, 5);
        self.send_bits(blcodes - 4, 4);
        for &o in BL_ORDER.iter().take(blcodes as usize) {
            self.send_bits(i32::from(trees.bl_tree[o].dl), 3);
        }
        self.send_tree(&trees.bl_tree, &trees.dyn_ltree, lcodes - 1);
        self.send_tree(&trees.bl_tree, &trees.dyn_dtree, dcodes - 1);
    }

    /// `_tr_stored_block(s, buf, stored_len, last)`: `buf` `None` writes the header only
    /// (zlib's null buffer with a length of 0).
    pub(super) fn stored_block(&mut self, buf: Option<&[u8]>, stored_len: u64, last: i32) {
        self.send_bits((STORED_BLOCK << 1) + last, 3);
        self.bi_windup();
        self.put_short(stored_len as u16);
        self.put_short(!(stored_len as u16));
        if let Some(b) = buf {
            let n = stored_len as usize;
            self.buf[self.pending..self.pending + n].copy_from_slice(&b[..n]);
            self.pending += n;
        }
    }

    /// `_tr_align(s)`.
    pub(super) fn align(&mut self, t: &Tables) {
        self.send_bits(STATIC_TREES << 1, 3);
        self.send_code(END_BLOCK, &t.static_ltree);
        self.bi_flush();
    }

    /// `compress_block(s, ltree, dtree)`: the symbols `sym_buf[0..sym_next]` (at `sym` in the
    /// buffer).
    fn compress_block(
        &mut self,
        ltree: &[Ct],
        dtree: &[Ct],
        sym: usize,
        sym_next: u32,
        t: &Tables,
    ) {
        let mut sx = 0usize;
        if sym_next != 0 {
            loop {
                let mut dist = u32::from(self.buf[sym + sx]);
                sx += 1;
                dist += u32::from(self.buf[sym + sx]) << 8;
                sx += 1;
                let mut lc = i32::from(self.buf[sym + sx]);
                sx += 1;
                if dist == 0 {
                    self.send_code(lc as usize, ltree);
                } else {
                    let code = t.length_code[lc as usize] as usize;
                    self.send_code(code + LITERALS + 1, ltree);
                    let extra = EXTRA_LBITS[code];
                    if extra != 0 {
                        lc -= t.base_length[code];
                        self.send_bits(lc, extra);
                    }
                    dist -= 1;
                    let code = d_code(t, dist);
                    self.send_code(code, dtree);
                    let extra = EXTRA_DBITS[code];
                    if extra != 0 {
                        dist = dist.wrapping_sub(t.base_dist[code] as u32);
                        self.send_bits(dist as i32, extra);
                    }
                }
                if sx >= sym_next as usize {
                    break;
                }
            }
        }
        self.send_code(END_BLOCK, ltree);
    }
}

/// The dynamic trees (`dyn_ltree`, `dyn_dtree`, `bl_tree`) and their `max_code`s.
#[derive(Clone)]
pub(super) struct Trees {
    pub(super) dyn_ltree: [Ct; HEAP_SIZE],
    pub(super) dyn_dtree: [Ct; 2 * D_CODES + 1],
    pub(super) bl_tree: [Ct; 2 * BL_CODES + 1],
    pub(super) l_max: i32,
    pub(super) d_max: i32,
    pub(super) bl_max: i32,
}

impl Trees {
    pub(super) fn new() -> Trees {
        Trees {
            dyn_ltree: [Ct::default(); HEAP_SIZE],
            dyn_dtree: [Ct::default(); 2 * D_CODES + 1],
            bl_tree: [Ct::default(); 2 * BL_CODES + 1],
            l_max: 0,
            d_max: 0,
            bl_max: 0,
        }
    }

    /// `init_block(s)` (the trees' part; `sym_next` and `matches` are the caller's).
    pub(super) fn init_block(&mut self, h: &mut Heap) {
        for c in self.dyn_ltree.iter_mut().take(L_CODES) {
            c.fc = 0;
        }
        for c in self.dyn_dtree.iter_mut().take(D_CODES) {
            c.fc = 0;
        }
        for c in self.bl_tree.iter_mut().take(BL_CODES) {
            c.fc = 0;
        }
        self.dyn_ltree[END_BLOCK].fc = 1;
        h.opt_len = 0;
        h.static_len = 0;
    }

    /// `detect_data_type(s)`.
    pub(super) fn detect_data_type(&self) -> i32 {
        let mut black_mask: u64 = 0xf3ff_c07f;
        for n in 0..=31 {
            if black_mask & 1 != 0 && self.dyn_ltree[n].fc != 0 {
                return super::Z_BINARY;
            }
            black_mask >>= 1;
        }
        if self.dyn_ltree[9].fc != 0 || self.dyn_ltree[10].fc != 0 || self.dyn_ltree[13].fc != 0 {
            return super::Z_TEXT;
        }
        for n in 32..LITERALS {
            if self.dyn_ltree[n].fc != 0 {
                return super::Z_TEXT;
            }
        }
        super::Z_BINARY
    }

    /// `build_bl_tree(s)`: the index in `bl_order` of the last bit length code to send.
    pub(super) fn build_bl_tree(&mut self, h: &mut Heap, t: &Tables) -> i32 {
        let (l_max, d_max) = (self.l_max, self.d_max);
        scan_tree(&mut self.bl_tree, &mut self.dyn_ltree, l_max);
        scan_tree(&mut self.bl_tree, &mut self.dyn_dtree, d_max);
        self.bl_max = h.build_tree(&mut self.bl_tree, Desc::Bl, t);
        let mut max_blindex = (BL_CODES - 1) as i32;
        while max_blindex >= 3 {
            if self.bl_tree[BL_ORDER[max_blindex as usize]].dl != 0 {
                break;
            }
            max_blindex -= 1;
        }
        h.opt_len = h
            .opt_len
            .wrapping_add(3 * (max_blindex as u64 + 1) + 5 + 5 + 4);
        max_blindex
    }
}

/// The block writer's inputs: `_tr_flush_block`'s view of the state.
pub(super) struct Flush<'a> {
    pub(super) out: &'a mut Out,
    pub(super) trees: &'a mut Trees,
    pub(super) heap: &'a mut Heap,
    pub(super) level: i32,
    pub(super) strategy: i32,
    pub(super) data_type: &'a mut i32,
    pub(super) sym: usize,
    pub(super) sym_next: &'a mut u32,
    pub(super) matches: &'a mut u32,
}

/// `_tr_flush_block(s, buf, stored_len, last)`.
pub(super) fn flush_block(f: Flush, buf: Option<&[u8]>, stored_len: u64, last: i32) {
    let t = tables();
    let mut max_blindex = 0;
    let (mut opt_lenb, static_lenb);
    if f.level > 0 {
        if *f.data_type == super::Z_UNKNOWN {
            *f.data_type = f.trees.detect_data_type();
        }
        f.trees.l_max = f.heap.build_tree(&mut f.trees.dyn_ltree, Desc::L, t);
        f.trees.d_max = f.heap.build_tree(&mut f.trees.dyn_dtree, Desc::D, t);
        max_blindex = f.trees.build_bl_tree(f.heap, t);
        opt_lenb = f.heap.opt_len.wrapping_add(3 + 7) >> 3;
        static_lenb = f.heap.static_len.wrapping_add(3 + 7) >> 3;
        if static_lenb <= opt_lenb {
            opt_lenb = static_lenb;
        }
    } else {
        opt_lenb = stored_len.wrapping_add(5);
        static_lenb = opt_lenb;
    }
    if stored_len.wrapping_add(4) <= opt_lenb && buf.is_some() {
        f.out.stored_block(buf, stored_len, last);
    } else if f.strategy == super::Z_FIXED || static_lenb == opt_lenb {
        f.out.send_bits((STATIC_TREES << 1) + last, 3);
        f.out
            .compress_block(&t.static_ltree, &t.static_dtree, f.sym, *f.sym_next, t);
    } else {
        f.out.send_bits((DYN_TREES << 1) + last, 3);
        f.out.send_all_trees(
            f.trees,
            f.trees.l_max + 1,
            f.trees.d_max + 1,
            max_blindex + 1,
        );
        let (lt, dt) = (&f.trees.dyn_ltree, &f.trees.dyn_dtree);
        f.out.compress_block(lt, dt, f.sym, *f.sym_next, t);
    }
    f.trees.init_block(f.heap);
    *f.sym_next = 0;
    *f.matches = 0;
    if last != 0 {
        f.out.bi_windup();
    }
}
