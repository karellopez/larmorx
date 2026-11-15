// SPDX-License-Identifier: Apache-2.0 AND Zlib
//! zlib 1.2.11's `deflate.c` as Ubuntu 22.04 builds it (not `FASTEST`, not `ASMV`, not
//! `UNALIGNED_OK`, `GZIP` on, the IBM Z hooks empty): the stream's states, the window, the hash
//! chains, the match finders and the five compression functions.

use super::trees::{
    self, Flush, Heap, LITERALS, MAX_MATCH, MIN_MATCH, Out, Tables, Trees, d_code, tables,
};
use super::*;
use crate::defects::{FS_809, FS_810, FS_811, FS_812, Notices, UpstreamDefects};

const INIT_STATE: i32 = 42;
const GZIP_STATE: i32 = 57;
const EXTRA_STATE: i32 = 69;
const NAME_STATE: i32 = 73;
const COMMENT_STATE: i32 = 91;
const HCRC_STATE: i32 = 103;
const BUSY_STATE: i32 = 113;
const FINISH_STATE: i32 = 666;

const NIL: u32 = 0;
const TOO_FAR: u32 = 4096;
const MIN_LOOKAHEAD: u32 = MAX_MATCH + MIN_MATCH + 1;
const WIN_INIT: u64 = MAX_MATCH as u64;
const MAX_STORED: u32 = 65535;
const PRESET_DICT: u32 = 0x20;
/// `OS_CODE` of the Unix build.
const OS_CODE: u8 = 3;
const BUF_SIZE: i32 = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BlockState {
    NeedMore,
    BlockDone,
    FinishStarted,
    FinishDone,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Func {
    Stored,
    Fast,
    Slow,
}

/// `configuration_table`: good, lazy, nice, chain, the function.
const CONFIG: [(u16, u16, u16, u16, Func); 10] = [
    (0, 0, 0, 0, Func::Stored),
    (4, 4, 8, 4, Func::Fast),
    (4, 5, 16, 8, Func::Fast),
    (4, 6, 32, 32, Func::Fast),
    (4, 4, 16, 16, Func::Slow),
    (8, 16, 32, 32, Func::Slow),
    (8, 16, 128, 128, Func::Slow),
    (8, 32, 128, 256, Func::Slow),
    (32, 128, 258, 1024, Func::Slow),
    (32, 258, 258, 4096, Func::Slow),
];

/// `RANK(f)`: `Z_BLOCK` between `Z_NO_FLUSH` and `Z_PARTIAL_FLUSH`.
fn rank(f: i32) -> i32 {
    (f * 2) - if f > 4 { 9 } else { 0 }
}

/// `gz_header` for writing: the fields `deflate` reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GzHeader {
    pub text: i32,
    pub time: u64,
    pub os: i32,
    /// The extra field (its length is `extra_len`, at most the buffer's).
    pub extra: Option<Vec<u8>>,
    pub extra_len: u32,
    /// The name and the comment without their terminating zero (zlib writes one).
    pub name: Option<Vec<u8>>,
    pub comment: Option<Vec<u8>>,
    pub hcrc: i32,
}

/// One call's `next_in`/`avail_in` and `next_out`/`avail_out`.
struct Io<'a, 'b> {
    inp: &'a [u8],
    ip: usize,
    out: &'b mut [u8],
    op: usize,
}

impl Io<'_, '_> {
    #[inline]
    fn avail_in(&self) -> u32 {
        (self.inp.len() - self.ip) as u32
    }
    #[inline]
    fn avail_out(&self) -> u32 {
        (self.out.len() - self.op) as u32
    }
}

/// A deflate stream (`z_stream` with its `deflate_state`).
#[derive(Clone)]
pub struct Deflate {
    /// `total_in`, `total_out`, `adler`, `data_type` and `msg` of the `z_stream`.
    pub total_in: u64,
    pub total_out: u64,
    pub adler: u64,
    pub data_type: i32,
    pub msg: Option<&'static str>,
    /// D13's mode for zlib's defects (FS-809, FS-810) and the notices they gave.
    pub notices: Notices,
    s: Box<State>,
}

#[derive(Clone)]
struct State {
    status: i32,
    o: Out,
    pending_buf_size: usize,
    wrap: i32,
    gzhead: Option<GzHeader>,
    gzindex: u64,
    last_flush: i32,
    w_size: u32,
    w_bits: u32,
    w_mask: u32,
    window: Vec<u8>,
    window_size: u64,
    prev: Vec<u16>,
    head: Vec<u16>,
    ins_h: u32,
    hash_bits: u32,
    hash_mask: u32,
    hash_shift: u32,
    block_start: i64,
    match_length: u32,
    prev_match: u32,
    match_available: bool,
    strstart: u32,
    match_start: u32,
    lookahead: u32,
    prev_length: u32,
    max_chain_length: u32,
    max_lazy_match: u32,
    level: i32,
    strategy: i32,
    good_match: u32,
    nice_match: i32,
    trees: Trees,
    heap: Heap,
    /// `sym_buf`'s offset in the pending buffer (`lit_bufsize`).
    sym: usize,
    sym_next: u32,
    sym_end: u32,
    matches: u32,
    insert: u32,
    high_water: u64,
}

impl Deflate {
    /// `deflateInit(strm, level)`.
    pub fn init(level: i32) -> Result<Deflate, i32> {
        Deflate::init2(
            level,
            Z_DEFLATED,
            MAX_WBITS,
            DEF_MEM_LEVEL,
            Z_DEFAULT_STRATEGY,
        )
    }

    /// `deflateInit2(strm, level, method, windowBits, memLevel, strategy)`: the stream, or the
    /// error zlib returns (`Z_STREAM_ERROR` for a parameter out of range).
    pub fn init2(
        level: i32,
        method: i32,
        window_bits: i32,
        mem_level: i32,
        strategy: i32,
    ) -> Result<Deflate, i32> {
        let mut level = level;
        let mut window_bits = window_bits;
        let mut wrap = 1;
        if level == Z_DEFAULT_COMPRESSION {
            level = 6;
        }
        if window_bits < 0 {
            wrap = 0;
            window_bits = window_bits.wrapping_neg();
        } else if window_bits > 15 {
            wrap = 2;
            window_bits -= 16;
        }
        if !(1..=MAX_MEM_LEVEL).contains(&mem_level)
            || method != Z_DEFLATED
            || !(8..=15).contains(&window_bits)
            || !(0..=9).contains(&level)
            || !(0..=Z_FIXED).contains(&strategy)
            || (window_bits == 8 && wrap != 1)
        {
            return Err(Z_STREAM_ERROR);
        }
        if window_bits == 8 {
            window_bits = 9;
        }
        let w_bits = window_bits as u32;
        let w_size = 1u32 << w_bits;
        let hash_bits = mem_level as u32 + 7;
        let hash_size = 1u32 << hash_bits;
        let lit_bufsize = 1u32 << (mem_level + 6);
        let pending_buf_size = lit_bufsize as usize * 4;
        let s = State {
            status: INIT_STATE,
            o: Out {
                buf: vec![0u8; pending_buf_size],
                pending: 0,
                pending_out: 0,
                bi_buf: 0,
                bi_valid: 0,
            },
            pending_buf_size,
            wrap,
            gzhead: None,
            gzindex: 0,
            last_flush: 0,
            w_size,
            w_bits,
            w_mask: w_size - 1,
            window: vec![0u8; 2 * w_size as usize],
            window_size: 0,
            prev: vec![0u16; w_size as usize],
            head: vec![0u16; hash_size as usize],
            ins_h: 0,
            hash_bits,
            hash_mask: hash_size - 1,
            hash_shift: (hash_bits + MIN_MATCH - 1) / MIN_MATCH,
            block_start: 0,
            match_length: 0,
            prev_match: 0,
            match_available: false,
            strstart: 0,
            match_start: 0,
            lookahead: 0,
            prev_length: 0,
            max_chain_length: 0,
            max_lazy_match: 0,
            level,
            strategy,
            good_match: 0,
            nice_match: 0,
            trees: Trees::new(),
            heap: Heap::new(),
            sym: lit_bufsize as usize,
            sym_next: 0,
            sym_end: (lit_bufsize - 1) * 3,
            matches: 0,
            insert: 0,
            high_water: 0,
        };
        let mut d = Deflate {
            total_in: 0,
            total_out: 0,
            adler: 0,
            data_type: Z_UNKNOWN,
            msg: None,
            notices: Notices::new(UpstreamDefects::Fix),
            s: Box::new(s),
        };
        d.reset();
        Ok(d)
    }

    /// `deflateResetKeep(strm)`.
    pub fn reset_keep(&mut self) -> i32 {
        self.total_in = 0;
        self.total_out = 0;
        self.msg = None;
        self.data_type = Z_UNKNOWN;
        let s = &mut *self.s;
        s.o.pending = 0;
        s.o.pending_out = 0;
        if s.wrap < 0 {
            s.wrap = -s.wrap;
        }
        s.status = if s.wrap == 2 {
            GZIP_STATE
        } else if s.wrap != 0 {
            INIT_STATE
        } else {
            BUSY_STATE
        };
        self.adler = if s.wrap == 2 {
            crc32(0, None)
        } else {
            adler32(0, None)
        };
        s.last_flush = -2;
        // _tr_init
        s.o.bi_buf = 0;
        s.o.bi_valid = 0;
        s.trees.init_block(&mut s.heap);
        s.sym_next = 0;
        s.matches = 0;
        Z_OK
    }

    /// `deflateReset(strm)`.
    pub fn reset(&mut self) -> i32 {
        let r = self.reset_keep();
        if r == Z_OK {
            self.s.lm_init();
        }
        r
    }

    /// `deflateEnd(strm)`: `Z_DATA_ERROR` when the stream was freed in the middle of its data.
    pub fn end(self) -> i32 {
        if self.s.status == BUSY_STATE {
            Z_DATA_ERROR
        } else {
            Z_OK
        }
    }

    /// `deflateSetHeader(strm, head)`.
    pub fn set_header(&mut self, head: GzHeader) -> i32 {
        if self.s.wrap != 2 {
            return Z_STREAM_ERROR;
        }
        self.s.gzhead = Some(head);
        Z_OK
    }

    /// `deflatePending(strm, &pending, &bits)`.
    pub fn pending(&self) -> (u32, i32) {
        (self.s.o.pending as u32, self.s.o.bi_valid)
    }

    /// The stream with D13's mode for zlib's defects (FS-809, FS-810).
    pub fn with_upstream_defects(mut self, mode: UpstreamDefects) -> Deflate {
        self.notices = Notices::new(mode);
        self
    }

    /// `deflatePrime(strm, bits, value)`. While bytes wait to be handed out behind some already
    /// handed out, zlib puts the bits at the position the waiting count gives (FS-809): by
    /// default they go after the waiting bytes; with `reproduce` where zlib puts them (the bytes
    /// it never wrote are the buffer's, zeros or older bytes). Bits that would not fit in the
    /// buffer are refused (`Z_BUF_ERROR`, FS-810) in both modes: zlib writes past it.
    pub fn prime(&mut self, bits: i32, value: i32) -> i32 {
        let s = &mut *self.s;
        if !(0..=16).contains(&bits) || s.sym < s.o.pending_out + ((BUF_SIZE as usize + 7) >> 3) {
            return Z_BUF_ERROR;
        }
        if s.o.pending_out != 0 && s.o.pending != 0 {
            self.notices.add(
                &FS_809,
                format!(
                    "deflatePrime with {} bytes waiting from offset {} of the output buffer",
                    s.o.pending, s.o.pending_out
                ),
            );
            if self.notices.fix() {
                let (po, n) = (s.o.pending_out, s.o.pending);
                s.o.buf.copy_within(po..po + n, 0);
                s.o.pending_out = 0;
            }
        }
        // the bytes the bit buffer may hand to the pending buffer during this call
        let add = ((s.o.bi_valid + bits) / 8) as usize;
        let size = s.pending_buf_size;
        if s.o.pending + add > size || s.o.pending_out + s.o.pending + add > size {
            self.notices.add(
                &FS_810,
                format!(
                    "deflatePrime of {bits} bits with {} bytes waiting from offset {} of a {size}-byte buffer",
                    s.o.pending, s.o.pending_out
                ),
            );
            return Z_BUF_ERROR;
        }
        let (mut bits, mut value) = (bits, value);
        loop {
            let mut put = BUF_SIZE - s.o.bi_valid;
            if put > bits {
                put = bits;
            }
            let mask = ((1i64 << put) - 1) as i32;
            s.o.bi_buf |= (((value & mask) as u32) << s.o.bi_valid) as u16;
            s.o.bi_valid += put;
            s.o.bi_flush();
            value >>= put;
            bits -= put;
            if bits == 0 {
                break;
            }
        }
        Z_OK
    }

    /// `deflateTune(strm, good_length, max_lazy, nice_length, max_chain)`.
    pub fn tune(
        &mut self,
        good_length: i32,
        max_lazy: i32,
        nice_length: i32,
        max_chain: i32,
    ) -> i32 {
        let s = &mut *self.s;
        s.good_match = good_length as u32;
        s.max_lazy_match = max_lazy as u32;
        s.nice_match = nice_length;
        s.max_chain_length = max_chain as u32;
        Z_OK
    }

    /// `deflateBound(strm, sourceLen)`.
    pub fn bound(&self, source_len: u64) -> u64 {
        let complen = source_len
            .wrapping_add((source_len.wrapping_add(7)) >> 3)
            .wrapping_add((source_len.wrapping_add(63)) >> 6)
            .wrapping_add(5);
        let s = &*self.s;
        let wraplen: u64 = match s.wrap {
            0 => 0,
            1 => 6 + if s.strstart != 0 { 4 } else { 0 },
            2 => {
                let mut w = 18u64;
                if let Some(h) = &s.gzhead {
                    if h.extra.is_some() {
                        w += 2 + u64::from(h.extra_len);
                    }
                    if let Some(n) = &h.name {
                        w += zlen(n) + 1;
                    }
                    if let Some(c) = &h.comment {
                        w += zlen(c) + 1;
                    }
                    if h.hcrc != 0 {
                        w += 2;
                    }
                }
                w
            }
            _ => 6,
        };
        if s.w_bits != 15 || s.hash_bits != 8 + 7 {
            return complen.wrapping_add(wraplen);
        }
        source_len
            .wrapping_add(source_len >> 12)
            .wrapping_add(source_len >> 14)
            .wrapping_add(source_len >> 25)
            .wrapping_add(13 - 6)
            .wrapping_add(wraplen)
    }

    /// `deflateGetDictionary(strm, dictionary, &dictLength)`: the window's last bytes (up to
    /// `w_size`). Bytes zlib would copy from past the window's end (FS-812) are zeros, in both
    /// modes.
    pub fn get_dictionary(&mut self) -> Vec<u8> {
        let s = &*self.s;
        let mut len = s.strstart + s.lookahead;
        if len > s.w_size {
            len = s.w_size;
        }
        let end = (s.strstart + s.lookahead) as usize;
        let start = end - len as usize;
        let mut v = vec![0u8; len as usize];
        let have = end.min(s.window.len());
        v[..have - start].copy_from_slice(&s.window[start..have]);
        if end > s.window.len() {
            self.notices.add(
                &FS_812,
                format!(
                    "the dictionary's last {} bytes lie past the {}-byte window",
                    end - s.window.len(),
                    s.window.len()
                ),
            );
        }
        v
    }

    /// `deflateSetDictionary(strm, dictionary, dictLength)`.
    pub fn set_dictionary(&mut self, dictionary: &[u8]) -> i32 {
        let wrap = self.s.wrap;
        if wrap == 2 || (wrap == 1 && self.s.status != INIT_STATE) || self.s.lookahead != 0 {
            return Z_STREAM_ERROR;
        }
        if wrap == 1 {
            self.adler = adler32(self.adler, Some(dictionary));
        }
        self.s.wrap = 0;
        let mut dict = dictionary;
        let w_size = self.s.w_size as usize;
        if dict.len() >= w_size {
            if wrap == 0 {
                self.s.clear_hash();
                self.s.strstart = 0;
                self.s.block_start = 0;
                self.s.insert = 0;
            }
            dict = &dict[dict.len() - w_size..];
        }
        let mut empty: [u8; 0] = [];
        let mut io = Io {
            inp: dict,
            ip: 0,
            out: &mut empty,
            op: 0,
        };
        let (mut ti, mut ad, mut dt) = (self.total_in, self.adler, self.data_type);
        {
            let mut st = Strm {
                s: &mut self.s,
                io: &mut io,
                total_in: &mut ti,
                total_out: &mut self.total_out,
                adler: &mut ad,
                data_type: &mut dt,
                notices: &mut self.notices,
            };
            st.fill_window();
            while st.s.lookahead >= MIN_MATCH {
                let mut str = st.s.strstart;
                let mut n = st.s.lookahead - (MIN_MATCH - 1);
                loop {
                    let c = st.s.window[(str + MIN_MATCH - 1) as usize];
                    st.s.update_hash(c);
                    st.s.prev[(str & st.s.w_mask) as usize] = st.s.head[st.s.ins_h as usize];
                    st.s.head[st.s.ins_h as usize] = str as u16;
                    str += 1;
                    n -= 1;
                    if n == 0 {
                        break;
                    }
                }
                st.s.strstart = str;
                st.s.lookahead = MIN_MATCH - 1;
                st.fill_window();
            }
        }
        // read_buf added the dictionary to total_in and (wrap was 0) no check value: zlib
        // restores next_in and avail_in only, so total_in keeps the dictionary's bytes.
        self.total_in = ti;
        self.adler = ad;
        self.data_type = dt;
        let s = &mut *self.s;
        s.strstart += s.lookahead;
        s.block_start = i64::from(s.strstart);
        s.insert = s.lookahead;
        s.lookahead = 0;
        s.match_length = MIN_MATCH - 1;
        s.prev_length = MIN_MATCH - 1;
        s.match_available = false;
        s.wrap = wrap;
        Z_OK
    }

    /// `deflateParams(strm, level, strategy)` with the stream's current input and output: the
    /// status and the input and output bytes the flush it may run used.
    pub fn params(
        &mut self,
        level: i32,
        strategy: i32,
        input: &[u8],
        output: &mut [u8],
    ) -> (i32, usize, usize) {
        let mut level = level;
        if level == Z_DEFAULT_COMPRESSION {
            level = 6;
        }
        if !(0..=9).contains(&level) || !(0..=Z_FIXED).contains(&strategy) {
            return (Z_STREAM_ERROR, 0, 0);
        }
        let func = CONFIG[self.s.level as usize].4;
        let (mut used, mut made) = (0, 0);
        if (strategy != self.s.strategy || func != CONFIG[level as usize].4)
            && self.s.last_flush != -2
        {
            let (err, u, m) = self.deflate(input, output, Z_BLOCK);
            used = u;
            made = m;
            if err == Z_STREAM_ERROR {
                return (err, used, made);
            }
            let s = &*self.s;
            if input.len() > used
                || (i64::from(s.strstart) - s.block_start) + i64::from(s.lookahead) != 0
            {
                return (Z_BUF_ERROR, used, made);
            }
        }
        let s = &mut *self.s;
        if s.level != level {
            if s.level == 0 && s.matches != 0 {
                if s.matches == 1 {
                    s.slide_hash();
                } else {
                    s.clear_hash();
                }
                s.matches = 0;
            }
            s.level = level;
            let c = CONFIG[level as usize];
            s.max_lazy_match = u32::from(c.1);
            s.good_match = u32::from(c.0);
            s.nice_match = i32::from(c.2);
            s.max_chain_length = u32::from(c.3);
        }
        s.strategy = strategy;
        (Z_OK, used, made)
    }

    /// `deflate(strm, flush)`: the status, the input bytes consumed and the output bytes
    /// written.
    pub fn deflate(&mut self, input: &[u8], output: &mut [u8], flush: i32) -> (i32, usize, usize) {
        // `avail_in` and `avail_out` are 32-bit counts.
        let input = &input[..input.len().min(u32::MAX as usize)];
        let n = output.len().min(u32::MAX as usize);
        let output = &mut output[..n];
        let mut io = Io {
            inp: input,
            ip: 0,
            out: output,
            op: 0,
        };
        let r = {
            let mut st = Strm {
                s: &mut self.s,
                io: &mut io,
                total_in: &mut self.total_in,
                total_out: &mut self.total_out,
                adler: &mut self.adler,
                data_type: &mut self.data_type,
                notices: &mut self.notices,
            };
            st.deflate(flush)
        };
        let r = match r {
            Ok(v) => v,
            Err(e) => {
                self.msg = Some(err_msg(e));
                e
            }
        };
        (r, io.ip, io.op)
    }

    /// The compression level (`s->level`).
    pub fn level(&self) -> i32 {
        self.s.level
    }
}

/// `strlen` of a zero-terminated field (the bytes up to the first zero).
fn zlen(v: &[u8]) -> u64 {
    v.iter().position(|&b| b == 0).unwrap_or(v.len()) as u64
}

/// The byte `i` of a zero-terminated field (its zero after the end).
fn zbyte(v: &[u8], i: u64) -> u8 {
    v.get(i as usize).copied().unwrap_or(0)
}

impl State {
    #[inline]
    fn update_hash(&mut self, c: u8) {
        self.ins_h = ((self.ins_h << self.hash_shift) ^ u32::from(c)) & self.hash_mask;
    }

    /// `INSERT_STRING(s, str, match_head)`: the previous head.
    #[inline]
    fn insert_string(&mut self, str: u32) -> u32 {
        let c = self.window[(str + MIN_MATCH - 1) as usize];
        self.update_hash(c);
        let h = self.head[self.ins_h as usize];
        self.prev[(str & self.w_mask) as usize] = h;
        self.head[self.ins_h as usize] = str as u16;
        u32::from(h)
    }

    /// `CLEAR_HASH(s)`.
    fn clear_hash(&mut self) {
        self.head.fill(0);
    }

    /// `slide_hash(s)`.
    fn slide_hash(&mut self) {
        let wsize = self.w_size;
        for p in self.head.iter_mut() {
            let m = u32::from(*p);
            *p = if m >= wsize { (m - wsize) as u16 } else { 0 };
        }
        for p in self.prev.iter_mut() {
            let m = u32::from(*p);
            *p = if m >= wsize { (m - wsize) as u16 } else { 0 };
        }
    }

    /// `lm_init(s)`.
    fn lm_init(&mut self) {
        self.window_size = 2 * u64::from(self.w_size);
        self.clear_hash();
        let c = CONFIG[self.level as usize];
        self.max_lazy_match = u32::from(c.1);
        self.good_match = u32::from(c.0);
        self.nice_match = i32::from(c.2);
        self.max_chain_length = u32::from(c.3);
        self.strstart = 0;
        self.block_start = 0;
        self.lookahead = 0;
        self.insert = 0;
        self.match_length = MIN_MATCH - 1;
        self.prev_length = MIN_MATCH - 1;
        self.match_available = false;
        self.ins_h = 0;
    }

    #[inline]
    fn max_dist(&self) -> u32 {
        self.w_size - MIN_LOOKAHEAD
    }

    /// `longest_match(s, cur_match)`.
    fn longest_match(&mut self, cur_match: u32) -> u32 {
        let mut cur_match = cur_match;
        let mut chain_length = self.max_chain_length;
        let scan = self.strstart as usize;
        let mut best_len = self.prev_length as i32;
        let mut nice_match = self.nice_match;
        let limit = if self.strstart > self.max_dist() {
            self.strstart - self.max_dist()
        } else {
            NIL
        };
        let wmask = self.w_mask;
        let w = &self.window;
        let mut scan_end1 = w[scan + best_len as usize - 1];
        let mut scan_end = w[scan + best_len as usize];
        if self.prev_length >= self.good_match {
            chain_length >>= 2;
        }
        if nice_match as u32 > self.lookahead {
            nice_match = self.lookahead as i32;
        }
        loop {
            let m = cur_match as usize;
            if w[m + best_len as usize] != scan_end
                || w[m + best_len as usize - 1] != scan_end1
                || w[m] != w[scan]
                || w[m + 1] != w[scan + 1]
            {
            } else {
                // offsets 3 ..= 257 compared (2 is equal by the hash); the length is the
                // first offset that differs, at most MAX_MATCH.
                let a = &w[scan + 3..scan + MAX_MATCH as usize];
                let b = &w[m + 3..m + MAX_MATCH as usize];
                let common = a.iter().zip(b).take_while(|(x, y)| x == y).count();
                let len = 3 + common as i32;
                if len > best_len {
                    self.match_start = cur_match;
                    best_len = len;
                    if len >= nice_match {
                        break;
                    }
                    scan_end1 = w[scan + best_len as usize - 1];
                    scan_end = w[scan + best_len as usize];
                }
            }
            cur_match = u32::from(self.prev[(cur_match & wmask) as usize]);
            if cur_match <= limit {
                break;
            }
            chain_length = chain_length.wrapping_sub(1);
            if chain_length == 0 {
                break;
            }
        }
        if best_len as u32 <= self.lookahead {
            best_len as u32
        } else {
            self.lookahead
        }
    }
}

/// The state and one call's buffers.
struct Strm<'s, 'a, 'b, 'c> {
    s: &'s mut State,
    io: &'c mut Io<'a, 'b>,
    total_in: &'s mut u64,
    total_out: &'s mut u64,
    adler: &'s mut u64,
    data_type: &'s mut i32,
    notices: &'s mut Notices,
}

impl Strm<'_, '_, '_, '_> {
    /// FS-811: more bytes waiting to be hashed than lie before the current position (a slide
    /// of the stored window kept the count): zlib 1.2.11 reads far before its window here and
    /// crashes; larmorx hashes the bytes there are (zlib 1.2.12's limit), in both modes.
    fn limit_insert(&mut self) {
        if self.s.insert > self.s.strstart {
            self.notices.add(
                &FS_811,
                format!(
                    "{} bytes to hash before position {} of the window",
                    self.s.insert, self.s.strstart
                ),
            );
            self.s.insert = self.s.strstart;
        }
    }

    /// `read_buf(strm, window + at, size)`.
    fn read_buf_window(&mut self, at: usize, size: u32) -> u32 {
        let mut len = self.io.avail_in();
        if len > size {
            len = size;
        }
        if len == 0 {
            return 0;
        }
        let src = &self.io.inp[self.io.ip..self.io.ip + len as usize];
        self.s.window[at..at + len as usize].copy_from_slice(src);
        self.check(at, len, true);
        self.io.ip += len as usize;
        *self.total_in += u64::from(len);
        len
    }

    /// `read_buf(strm, strm->next_out, size)`: input copied to the output.
    fn read_buf_out(&mut self, size: u32) -> u32 {
        let mut len = self.io.avail_in();
        if len > size {
            len = size;
        }
        if len == 0 {
            return 0;
        }
        let (ip, op, n) = (self.io.ip, self.io.op, len as usize);
        self.io.out[op..op + n].copy_from_slice(&self.io.inp[ip..ip + n]);
        self.check(op, len, false);
        self.io.ip += n;
        *self.total_in += u64::from(len);
        len
    }

    /// The check value over bytes just copied (to the window or to the output).
    fn check(&mut self, at: usize, len: u32, window: bool) {
        let b: &[u8] = if window {
            &self.s.window[at..at + len as usize]
        } else {
            &self.io.out[at..at + len as usize]
        };
        if self.s.wrap == 1 {
            *self.adler = adler32(*self.adler, Some(b));
        } else if self.s.wrap == 2 {
            *self.adler = crc32(*self.adler, Some(b));
        }
    }

    /// `flush_pending(strm)`.
    fn flush_pending(&mut self) {
        self.s.o.bi_flush();
        let mut len = self.s.o.pending as u32;
        if len > self.io.avail_out() {
            len = self.io.avail_out();
        }
        if len == 0 {
            return;
        }
        let (po, op, n) = (self.s.o.pending_out, self.io.op, len as usize);
        self.io.out[op..op + n].copy_from_slice(&self.s.o.buf[po..po + n]);
        self.io.op += n;
        self.s.o.pending_out += n;
        *self.total_out += u64::from(len);
        self.s.o.pending -= n;
        if self.s.o.pending == 0 {
            self.s.o.pending_out = 0;
        }
    }

    fn put_short_msb(&mut self, b: u32) {
        self.s.o.put_byte((b >> 8) as u8);
        self.s.o.put_byte((b & 0xff) as u8);
    }

    /// `HCRC_UPDATE(beg)`.
    fn hcrc_update(&mut self, beg: usize) {
        let hcrc = self.s.gzhead.as_ref().is_some_and(|h| h.hcrc != 0);
        if hcrc && self.s.o.pending > beg {
            let b = &self.s.o.buf[beg..self.s.o.pending];
            *self.adler = crc32(*self.adler, Some(b));
        }
    }

    /// `_tr_flush_block` of the window from `block_start` (or none when it was slid away).
    fn tr_flush_block(&mut self, from: Option<usize>, stored_len: u64, last: i32) {
        let s = &mut *self.s;
        let buf = from.map(|f| &s.window[f..]);
        trees::flush_block(
            Flush {
                out: &mut s.o,
                trees: &mut s.trees,
                heap: &mut s.heap,
                level: s.level,
                strategy: s.strategy,
                data_type: self.data_type,
                sym: s.sym,
                sym_next: &mut s.sym_next,
                matches: &mut s.matches,
            },
            buf,
            stored_len,
            last,
        );
    }

    /// `FLUSH_BLOCK_ONLY(s, last)`.
    fn flush_block_only(&mut self, last: i32) {
        let from = if self.s.block_start >= 0 {
            Some(self.s.block_start as usize)
        } else {
            None
        };
        let len = (i64::from(self.s.strstart) - self.s.block_start) as u64;
        self.tr_flush_block(from, len, last);
        self.s.block_start = i64::from(self.s.strstart);
        self.flush_pending();
    }

    /// `_tr_tally_lit`: true when the block must be flushed.
    #[inline]
    fn tally_lit(&mut self, c: u8) -> bool {
        let s = &mut *self.s;
        let at = s.sym + s.sym_next as usize;
        s.o.buf[at] = 0;
        s.o.buf[at + 1] = 0;
        s.o.buf[at + 2] = c;
        s.sym_next += 3;
        let f = &mut s.trees.dyn_ltree[c as usize].fc;
        *f = f.wrapping_add(1);
        s.sym_next == s.sym_end
    }

    /// `_tr_tally_dist`.
    #[inline]
    fn tally_dist(&mut self, distance: u32, length: u32, t: &Tables) -> bool {
        let s = &mut *self.s;
        let len = length as u8;
        let dist = distance as u16;
        let at = s.sym + s.sym_next as usize;
        s.o.buf[at] = dist as u8;
        s.o.buf[at + 1] = (dist >> 8) as u8;
        s.o.buf[at + 2] = len;
        s.sym_next += 3;
        let dist = dist.wrapping_sub(1);
        let lc = t.length_code[len as usize] as usize + LITERALS + 1;
        s.trees.dyn_ltree[lc].fc = s.trees.dyn_ltree[lc].fc.wrapping_add(1);
        let dc = d_code(t, u32::from(dist));
        s.trees.dyn_dtree[dc].fc = s.trees.dyn_dtree[dc].fc.wrapping_add(1);
        s.sym_next == s.sym_end
    }

    /// `fill_window(s)`.
    fn fill_window(&mut self) {
        let wsize = self.s.w_size;
        loop {
            let mut more = (self.s.window_size
                - u64::from(self.s.lookahead)
                - u64::from(self.s.strstart)) as u32;
            if self.s.strstart >= wsize + self.s.max_dist() {
                let s = &mut *self.s;
                let n = (wsize - more) as usize;
                s.window.copy_within(wsize as usize..wsize as usize + n, 0);
                s.match_start = s.match_start.wrapping_sub(wsize);
                s.strstart -= wsize;
                s.block_start -= i64::from(wsize);
                s.slide_hash();
                more += wsize;
            }
            if self.io.avail_in() == 0 {
                break;
            }
            let at = (self.s.strstart + self.s.lookahead) as usize;
            let n = self.read_buf_window(at, more);
            self.s.lookahead += n;
            if self.s.lookahead + self.s.insert >= MIN_MATCH {
                self.limit_insert();
            }
            let s = &mut *self.s;
            if s.lookahead + s.insert >= MIN_MATCH {
                let mut str = s.strstart - s.insert;
                s.ins_h = u32::from(s.window[str as usize]);
                let c = s.window[str as usize + 1];
                s.update_hash(c);
                while s.insert != 0 {
                    let c = s.window[(str + MIN_MATCH - 1) as usize];
                    s.update_hash(c);
                    s.prev[(str & s.w_mask) as usize] = s.head[s.ins_h as usize];
                    s.head[s.ins_h as usize] = str as u16;
                    str += 1;
                    s.insert -= 1;
                    if s.lookahead + s.insert < MIN_MATCH {
                        break;
                    }
                }
            }
            if !(self.s.lookahead < MIN_LOOKAHEAD && self.io.avail_in() != 0) {
                break;
            }
        }
        let s = &mut *self.s;
        if s.high_water < s.window_size {
            let curr = u64::from(s.strstart) + u64::from(s.lookahead);
            if s.high_water < curr {
                let mut init = s.window_size - curr;
                if init > WIN_INIT {
                    init = WIN_INIT;
                }
                s.window[curr as usize..(curr + init) as usize].fill(0);
                s.high_water = curr + init;
            } else if s.high_water < curr + WIN_INIT {
                let mut init = curr + WIN_INIT - s.high_water;
                if init > s.window_size - s.high_water {
                    init = s.window_size - s.high_water;
                }
                let hw = s.high_water as usize;
                s.window[hw..hw + init as usize].fill(0);
                s.high_water += init;
            }
        }
    }

    /// `deflate_stored(s, flush)`.
    fn deflate_stored(&mut self, flush: i32) -> BlockState {
        let mut min_block =
            (self.s.pending_buf_size as u64 - 5).min(u64::from(self.s.w_size)) as u32;
        let mut last = 0i32;
        let mut used = self.io.avail_in();
        loop {
            let mut len = MAX_STORED;
            let mut have = ((self.s.o.bi_valid + 42) >> 3) as u32;
            if self.io.avail_out() < have {
                break;
            }
            have = self.io.avail_out() - have;
            let mut left = (i64::from(self.s.strstart) - self.s.block_start) as u32;
            if u64::from(len) > u64::from(left) + u64::from(self.io.avail_in()) {
                len = left + self.io.avail_in();
            }
            if len > have {
                len = have;
            }
            if len < min_block
                && ((len == 0 && flush != Z_FINISH)
                    || flush == Z_NO_FLUSH
                    || len != left.wrapping_add(self.io.avail_in()))
            {
                break;
            }
            last = if flush == Z_FINISH && len == left.wrapping_add(self.io.avail_in()) {
                1
            } else {
                0
            };
            self.s.o.stored_block(None, 0, last);
            let p = self.s.o.pending;
            let b = &mut self.s.o.buf;
            b[p - 4] = len as u8;
            b[p - 3] = (len >> 8) as u8;
            b[p - 2] = !len as u8;
            b[p - 1] = (!len >> 8) as u8;
            self.flush_pending();
            if left != 0 {
                if left > len {
                    left = len;
                }
                let (bs, op, n) = (self.s.block_start as usize, self.io.op, left as usize);
                self.io.out[op..op + n].copy_from_slice(&self.s.window[bs..bs + n]);
                self.io.op += n;
                *self.total_out += u64::from(left);
                self.s.block_start += i64::from(left);
                len -= left;
            }
            if len != 0 {
                self.read_buf_out(len);
                self.io.op += len as usize;
                *self.total_out += u64::from(len);
            }
            if last != 0 {
                break;
            }
        }
        used -= self.io.avail_in();
        if used != 0 {
            let s = &mut *self.s;
            let ws = s.w_size as usize;
            if used >= s.w_size {
                s.matches = 2;
                let ip = self.io.ip;
                s.window[..ws].copy_from_slice(&self.io.inp[ip - ws..ip]);
                s.strstart = s.w_size;
            } else {
                if s.window_size - u64::from(s.strstart) <= u64::from(used) {
                    s.strstart -= s.w_size;
                    let st = s.strstart as usize;
                    s.window.copy_within(ws..ws + st, 0);
                    if s.matches < 2 {
                        s.matches += 1;
                    }
                }
                let (ip, st, u) = (self.io.ip, s.strstart as usize, used as usize);
                s.window[st..st + u].copy_from_slice(&self.io.inp[ip - u..ip]);
                s.strstart += used;
            }
            s.block_start = i64::from(s.strstart);
            s.insert += used.min(s.w_size - s.insert);
        }
        if self.s.high_water < u64::from(self.s.strstart) {
            self.s.high_water = u64::from(self.s.strstart);
        }
        if last != 0 {
            return BlockState::FinishDone;
        }
        if flush != Z_NO_FLUSH
            && flush != Z_FINISH
            && self.io.avail_in() == 0
            && i64::from(self.s.strstart) == self.s.block_start
        {
            return BlockState::BlockDone;
        }
        let mut have = (self.s.window_size - u64::from(self.s.strstart) - 1) as u32;
        if self.io.avail_in() > have && self.s.block_start >= i64::from(self.s.w_size) {
            let s = &mut *self.s;
            s.block_start -= i64::from(s.w_size);
            s.strstart -= s.w_size;
            let (ws, st) = (s.w_size as usize, s.strstart as usize);
            s.window.copy_within(ws..ws + st, 0);
            if s.matches < 2 {
                s.matches += 1;
            }
            have += s.w_size;
        }
        if have > self.io.avail_in() {
            have = self.io.avail_in();
        }
        if have != 0 {
            let at = self.s.strstart as usize;
            self.read_buf_window(at, have);
            self.s.strstart += have;
        }
        if self.s.high_water < u64::from(self.s.strstart) {
            self.s.high_water = u64::from(self.s.strstart);
        }
        let have = ((self.s.o.bi_valid + 42) >> 3) as u64;
        let have = (self.s.pending_buf_size as u64 - have).min(u64::from(MAX_STORED)) as u32;
        min_block = have.min(self.s.w_size);
        let left = (i64::from(self.s.strstart) - self.s.block_start) as u32;
        if left >= min_block
            || ((left != 0 || flush == Z_FINISH)
                && flush != Z_NO_FLUSH
                && self.io.avail_in() == 0
                && left <= have)
        {
            let len = left.min(have);
            last = if flush == Z_FINISH && self.io.avail_in() == 0 && len == left {
                1
            } else {
                0
            };
            let s = &mut *self.s;
            let bs = s.block_start as usize;
            s.o.stored_block(Some(&s.window[bs..]), u64::from(len), last);
            s.block_start += i64::from(len);
            self.flush_pending();
        }
        if last != 0 {
            BlockState::FinishStarted
        } else {
            BlockState::NeedMore
        }
    }

    /// The end of `deflate_fast`, `deflate_slow`, `deflate_rle` and `deflate_huff`.
    fn finish_blocks(&mut self, flush: i32) -> BlockState {
        if flush == Z_FINISH {
            self.flush_block_only(1);
            if self.io.avail_out() == 0 {
                return BlockState::FinishStarted;
            }
            return BlockState::FinishDone;
        }
        if self.s.sym_next != 0 {
            self.flush_block_only(0);
            if self.io.avail_out() == 0 {
                return BlockState::NeedMore;
            }
        }
        BlockState::BlockDone
    }

    /// `deflate_fast(s, flush)`.
    fn deflate_fast(&mut self, flush: i32) -> BlockState {
        let t = tables();
        loop {
            if self.s.lookahead < MIN_LOOKAHEAD {
                self.fill_window();
                if self.s.lookahead < MIN_LOOKAHEAD && flush == Z_NO_FLUSH {
                    return BlockState::NeedMore;
                }
                if self.s.lookahead == 0 {
                    break;
                }
            }
            let mut hash_head = NIL;
            if self.s.lookahead >= MIN_MATCH {
                hash_head = self.s.insert_string(self.s.strstart);
            }
            if hash_head != NIL && self.s.strstart - hash_head <= self.s.max_dist() {
                self.s.match_length = self.s.longest_match(hash_head);
            }
            let bflush;
            if self.s.match_length >= MIN_MATCH {
                let (d, l) = (
                    self.s.strstart - self.s.match_start,
                    self.s.match_length - MIN_MATCH,
                );
                bflush = self.tally_dist(d, l, t);
                let s = &mut *self.s;
                s.lookahead -= s.match_length;
                if s.match_length <= s.max_lazy_match && s.lookahead >= MIN_MATCH {
                    s.match_length -= 1;
                    loop {
                        s.strstart += 1;
                        s.insert_string(s.strstart);
                        s.match_length -= 1;
                        if s.match_length == 0 {
                            break;
                        }
                    }
                    s.strstart += 1;
                } else {
                    s.strstart += s.match_length;
                    s.match_length = 0;
                    s.ins_h = u32::from(s.window[s.strstart as usize]);
                    let c = s.window[s.strstart as usize + 1];
                    s.update_hash(c);
                }
            } else {
                let c = self.s.window[self.s.strstart as usize];
                bflush = self.tally_lit(c);
                self.s.lookahead -= 1;
                self.s.strstart += 1;
            }
            if bflush {
                self.flush_block_only(0);
                if self.io.avail_out() == 0 {
                    return BlockState::NeedMore;
                }
            }
        }
        self.s.insert = if self.s.strstart < MIN_MATCH - 1 {
            self.s.strstart
        } else {
            MIN_MATCH - 1
        };
        self.finish_blocks(flush)
    }

    /// `deflate_slow(s, flush)`.
    fn deflate_slow(&mut self, flush: i32) -> BlockState {
        let t = tables();
        loop {
            if self.s.lookahead < MIN_LOOKAHEAD {
                self.fill_window();
                if self.s.lookahead < MIN_LOOKAHEAD && flush == Z_NO_FLUSH {
                    return BlockState::NeedMore;
                }
                if self.s.lookahead == 0 {
                    break;
                }
            }
            let mut hash_head = NIL;
            if self.s.lookahead >= MIN_MATCH {
                hash_head = self.s.insert_string(self.s.strstart);
            }
            {
                let s = &mut *self.s;
                s.prev_length = s.match_length;
                s.prev_match = s.match_start;
                s.match_length = MIN_MATCH - 1;
                if hash_head != NIL
                    && s.prev_length < s.max_lazy_match
                    && s.strstart - hash_head <= s.max_dist()
                {
                    s.match_length = s.longest_match(hash_head);
                    if s.match_length <= 5
                        && (s.strategy == Z_FILTERED
                            || (s.match_length == MIN_MATCH
                                && s.strstart.wrapping_sub(s.match_start) > TOO_FAR))
                    {
                        s.match_length = MIN_MATCH - 1;
                    }
                }
            }
            if self.s.prev_length >= MIN_MATCH && self.s.match_length <= self.s.prev_length {
                let max_insert = self.s.strstart + self.s.lookahead - MIN_MATCH;
                let (d, l) = (
                    self.s.strstart - 1 - self.s.prev_match,
                    self.s.prev_length - MIN_MATCH,
                );
                let bflush = self.tally_dist(d, l, t);
                let s = &mut *self.s;
                s.lookahead -= s.prev_length - 1;
                s.prev_length -= 2;
                loop {
                    s.strstart += 1;
                    if s.strstart <= max_insert {
                        s.insert_string(s.strstart);
                    }
                    s.prev_length -= 1;
                    if s.prev_length == 0 {
                        break;
                    }
                }
                s.match_available = false;
                s.match_length = MIN_MATCH - 1;
                s.strstart += 1;
                if bflush {
                    self.flush_block_only(0);
                    if self.io.avail_out() == 0 {
                        return BlockState::NeedMore;
                    }
                }
            } else if self.s.match_available {
                let c = self.s.window[self.s.strstart as usize - 1];
                if self.tally_lit(c) {
                    self.flush_block_only(0);
                }
                self.s.strstart += 1;
                self.s.lookahead -= 1;
                if self.io.avail_out() == 0 {
                    return BlockState::NeedMore;
                }
            } else {
                self.s.match_available = true;
                self.s.strstart += 1;
                self.s.lookahead -= 1;
            }
        }
        if self.s.match_available {
            let c = self.s.window[self.s.strstart as usize - 1];
            self.tally_lit(c);
            self.s.match_available = false;
        }
        self.s.insert = if self.s.strstart < MIN_MATCH - 1 {
            self.s.strstart
        } else {
            MIN_MATCH - 1
        };
        self.finish_blocks(flush)
    }

    /// `deflate_rle(s, flush)`.
    fn deflate_rle(&mut self, flush: i32) -> BlockState {
        let t = tables();
        loop {
            if self.s.lookahead <= MAX_MATCH {
                self.fill_window();
                if self.s.lookahead <= MAX_MATCH && flush == Z_NO_FLUSH {
                    return BlockState::NeedMore;
                }
                if self.s.lookahead == 0 {
                    break;
                }
            }
            self.s.match_length = 0;
            if self.s.lookahead >= MIN_MATCH && self.s.strstart > 0 {
                let s = &mut *self.s;
                let w = &s.window;
                let at = s.strstart as usize;
                let prev = w[at - 1];
                if prev == w[at] && prev == w[at + 1] && prev == w[at + 2] {
                    // offsets 3 ..= 257 from strstart; the run's length is the first that
                    // differs, at most MAX_MATCH.
                    let run = w[at + 3..at + MAX_MATCH as usize]
                        .iter()
                        .take_while(|&&b| b == prev)
                        .count();
                    s.match_length = 3 + run as u32;
                    if s.match_length > s.lookahead {
                        s.match_length = s.lookahead;
                    }
                }
            }
            let bflush;
            if self.s.match_length >= MIN_MATCH {
                bflush = self.tally_dist(1, self.s.match_length - MIN_MATCH, t);
                let s = &mut *self.s;
                s.lookahead -= s.match_length;
                s.strstart += s.match_length;
                s.match_length = 0;
            } else {
                let c = self.s.window[self.s.strstart as usize];
                bflush = self.tally_lit(c);
                self.s.lookahead -= 1;
                self.s.strstart += 1;
            }
            if bflush {
                self.flush_block_only(0);
                if self.io.avail_out() == 0 {
                    return BlockState::NeedMore;
                }
            }
        }
        self.s.insert = 0;
        self.finish_blocks(flush)
    }

    /// `deflate_huff(s, flush)`.
    fn deflate_huff(&mut self, flush: i32) -> BlockState {
        loop {
            if self.s.lookahead == 0 {
                self.fill_window();
                if self.s.lookahead == 0 {
                    if flush == Z_NO_FLUSH {
                        return BlockState::NeedMore;
                    }
                    break;
                }
            }
            self.s.match_length = 0;
            let c = self.s.window[self.s.strstart as usize];
            let bflush = self.tally_lit(c);
            self.s.lookahead -= 1;
            self.s.strstart += 1;
            if bflush {
                self.flush_block_only(0);
                if self.io.avail_out() == 0 {
                    return BlockState::NeedMore;
                }
            }
        }
        self.s.insert = 0;
        self.finish_blocks(flush)
    }

    /// The header states' early return (`s->last_flush = -1; return Z_OK`).
    fn wait(&mut self) -> Result<i32, i32> {
        self.s.last_flush = -1;
        Ok(Z_OK)
    }

    /// `deflate(strm, flush)`: `Err` for `ERR_RETURN` (the message set by the caller).
    fn deflate(&mut self, flush: i32) -> Result<i32, i32> {
        if !(0..=Z_BLOCK).contains(&flush) {
            return Ok(Z_STREAM_ERROR);
        }
        if self.s.status == FINISH_STATE && flush != Z_FINISH {
            return Err(Z_STREAM_ERROR);
        }
        if self.io.avail_out() == 0 {
            return Err(Z_BUF_ERROR);
        }
        let old_flush = self.s.last_flush;
        self.s.last_flush = flush;
        if self.s.o.pending != 0 {
            self.flush_pending();
            if self.io.avail_out() == 0 {
                return self.wait();
            }
        } else if self.io.avail_in() == 0 && rank(flush) <= rank(old_flush) && flush != Z_FINISH {
            return Err(Z_BUF_ERROR);
        }
        if self.s.status == FINISH_STATE && self.io.avail_in() != 0 {
            return Err(Z_BUF_ERROR);
        }
        if self.s.status == INIT_STATE {
            let mut header = (Z_DEFLATED as u32 + ((self.s.w_bits - 8) << 4)) << 8;
            let level_flags = if self.s.strategy >= Z_HUFFMAN_ONLY || self.s.level < 2 {
                0
            } else if self.s.level < 6 {
                1
            } else if self.s.level == 6 {
                2
            } else {
                3
            };
            header |= level_flags << 6;
            if self.s.strstart != 0 {
                header |= PRESET_DICT;
            }
            header += 31 - (header % 31);
            self.put_short_msb(header);
            if self.s.strstart != 0 {
                let a = *self.adler;
                self.put_short_msb((a >> 16) as u32);
                self.put_short_msb((a & 0xffff) as u32);
            }
            *self.adler = adler32(0, None);
            self.s.status = BUSY_STATE;
            self.flush_pending();
            if self.s.o.pending != 0 {
                return self.wait();
            }
        }
        if self.s.status == GZIP_STATE {
            *self.adler = crc32(0, None);
            let o = &mut self.s.o;
            o.put_byte(31);
            o.put_byte(139);
            o.put_byte(8);
            let xfl = if self.s.level == 9 {
                2
            } else if self.s.strategy >= Z_HUFFMAN_ONLY || self.s.level < 2 {
                4
            } else {
                0
            };
            match self.s.gzhead.clone() {
                None => {
                    let o = &mut self.s.o;
                    for _ in 0..5 {
                        o.put_byte(0);
                    }
                    o.put_byte(xfl);
                    o.put_byte(OS_CODE);
                    self.s.status = BUSY_STATE;
                    self.flush_pending();
                    if self.s.o.pending != 0 {
                        return self.wait();
                    }
                }
                Some(h) => {
                    let o = &mut self.s.o;
                    o.put_byte(
                        (if h.text != 0 { 1 } else { 0 })
                            + (if h.hcrc != 0 { 2 } else { 0 })
                            + (if h.extra.is_none() { 0 } else { 4 })
                            + (if h.name.is_none() { 0 } else { 8 })
                            + (if h.comment.is_none() { 0 } else { 16 }),
                    );
                    o.put_byte((h.time & 0xff) as u8);
                    o.put_byte(((h.time >> 8) & 0xff) as u8);
                    o.put_byte(((h.time >> 16) & 0xff) as u8);
                    o.put_byte(((h.time >> 24) & 0xff) as u8);
                    o.put_byte(xfl);
                    o.put_byte((h.os & 0xff) as u8);
                    if h.extra.is_some() {
                        o.put_byte((h.extra_len & 0xff) as u8);
                        o.put_byte(((h.extra_len >> 8) & 0xff) as u8);
                    }
                    if h.hcrc != 0 {
                        let p = o.pending;
                        *self.adler = crc32(*self.adler, Some(&o.buf[..p]));
                    }
                    self.s.gzindex = 0;
                    self.s.status = EXTRA_STATE;
                }
            }
        }
        if self.s.status == EXTRA_STATE {
            let extra = self.s.gzhead.as_ref().and_then(|h| h.extra.clone());
            if let Some(extra) = extra {
                let elen = self.s.gzhead.as_ref().map_or(0, |h| h.extra_len);
                let mut beg = self.s.o.pending;
                let mut left = (elen & 0xffff) as u64 - self.s.gzindex;
                while self.s.o.pending as u64 + left > self.s.pending_buf_size as u64 {
                    let copy = self.s.pending_buf_size - self.s.o.pending;
                    let (p, gi) = (self.s.o.pending, self.s.gzindex as usize);
                    self.s.o.buf[p..p + copy].copy_from_slice(&extra[gi..gi + copy]);
                    self.s.o.pending = self.s.pending_buf_size;
                    self.hcrc_update(beg);
                    self.s.gzindex += copy as u64;
                    self.flush_pending();
                    if self.s.o.pending != 0 {
                        return self.wait();
                    }
                    beg = 0;
                    left -= copy as u64;
                }
                let (p, gi, l) = (self.s.o.pending, self.s.gzindex as usize, left as usize);
                self.s.o.buf[p..p + l].copy_from_slice(&extra[gi..gi + l]);
                self.s.o.pending += l;
                self.hcrc_update(beg);
                self.s.gzindex = 0;
            }
            self.s.status = NAME_STATE;
        }
        if self.s.status == NAME_STATE {
            let name = self.s.gzhead.as_ref().and_then(|h| h.name.clone());
            if let Some(name) = name {
                let mut beg = self.s.o.pending;
                loop {
                    if self.s.o.pending == self.s.pending_buf_size {
                        self.hcrc_update(beg);
                        self.flush_pending();
                        if self.s.o.pending != 0 {
                            return self.wait();
                        }
                        beg = 0;
                    }
                    let val = zbyte(&name, self.s.gzindex);
                    self.s.gzindex += 1;
                    self.s.o.put_byte(val);
                    if val == 0 {
                        break;
                    }
                }
                self.hcrc_update(beg);
                self.s.gzindex = 0;
            }
            self.s.status = COMMENT_STATE;
        }
        if self.s.status == COMMENT_STATE {
            let comment = self.s.gzhead.as_ref().and_then(|h| h.comment.clone());
            if let Some(comment) = comment {
                let mut beg = self.s.o.pending;
                loop {
                    if self.s.o.pending == self.s.pending_buf_size {
                        self.hcrc_update(beg);
                        self.flush_pending();
                        if self.s.o.pending != 0 {
                            return self.wait();
                        }
                        beg = 0;
                    }
                    let val = zbyte(&comment, self.s.gzindex);
                    self.s.gzindex += 1;
                    self.s.o.put_byte(val);
                    if val == 0 {
                        break;
                    }
                }
                self.hcrc_update(beg);
            }
            self.s.status = HCRC_STATE;
        }
        if self.s.status == HCRC_STATE {
            let hcrc = self.s.gzhead.as_ref().is_some_and(|h| h.hcrc != 0);
            if hcrc {
                if self.s.o.pending + 2 > self.s.pending_buf_size {
                    self.flush_pending();
                    if self.s.o.pending != 0 {
                        return self.wait();
                    }
                }
                let a = *self.adler;
                self.s.o.put_byte((a & 0xff) as u8);
                self.s.o.put_byte(((a >> 8) & 0xff) as u8);
                *self.adler = crc32(0, None);
            }
            self.s.status = BUSY_STATE;
            self.flush_pending();
            if self.s.o.pending != 0 {
                return self.wait();
            }
        }
        if self.io.avail_in() != 0
            || self.s.lookahead != 0
            || (flush != Z_NO_FLUSH && self.s.status != FINISH_STATE)
        {
            let bstate = if self.s.level == 0 {
                self.deflate_stored(flush)
            } else if self.s.strategy == Z_HUFFMAN_ONLY {
                self.deflate_huff(flush)
            } else if self.s.strategy == Z_RLE {
                self.deflate_rle(flush)
            } else if CONFIG[self.s.level as usize].4 == Func::Fast {
                self.deflate_fast(flush)
            } else {
                self.deflate_slow(flush)
            };
            if bstate == BlockState::FinishStarted || bstate == BlockState::FinishDone {
                self.s.status = FINISH_STATE;
            }
            if bstate == BlockState::NeedMore || bstate == BlockState::FinishStarted {
                if self.io.avail_out() == 0 {
                    self.s.last_flush = -1;
                }
                return Ok(Z_OK);
            }
            if bstate == BlockState::BlockDone {
                if flush == Z_PARTIAL_FLUSH {
                    self.s.o.align(tables());
                } else if flush != Z_BLOCK {
                    self.s.o.stored_block(None, 0, 0);
                    if flush == Z_FULL_FLUSH {
                        self.s.clear_hash();
                        if self.s.lookahead == 0 {
                            self.s.strstart = 0;
                            self.s.block_start = 0;
                            self.s.insert = 0;
                        }
                    }
                }
                self.flush_pending();
                if self.io.avail_out() == 0 {
                    self.s.last_flush = -1;
                    return Ok(Z_OK);
                }
            }
        }
        if flush != Z_FINISH {
            return Ok(Z_OK);
        }
        if self.s.wrap == 2 {
            let (a, ti) = (*self.adler, *self.total_in);
            let o = &mut self.s.o;
            o.put_byte((a & 0xff) as u8);
            o.put_byte(((a >> 8) & 0xff) as u8);
            o.put_byte(((a >> 16) & 0xff) as u8);
            o.put_byte(((a >> 24) & 0xff) as u8);
            o.put_byte((ti & 0xff) as u8);
            o.put_byte(((ti >> 8) & 0xff) as u8);
            o.put_byte(((ti >> 16) & 0xff) as u8);
            o.put_byte(((ti >> 24) & 0xff) as u8);
        } else if self.s.wrap == 1 {
            let a = *self.adler;
            self.put_short_msb((a >> 16) as u32);
            self.put_short_msb((a & 0xffff) as u32);
        }
        self.flush_pending();
        if self.s.wrap > 0 {
            self.s.wrap = -self.s.wrap;
        }
        if self.s.o.pending == 0 {
            return Ok(Z_STREAM_END);
        }
        Ok(Z_OK)
    }
}
