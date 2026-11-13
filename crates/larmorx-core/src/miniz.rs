// SPDX-License-Identifier: Apache-2.0
//! miniz 1.15's inflater (Rich Geldreich, `miniz.c` v1.15 r4, 2013; public domain, the
//! Unlicense; NOTICE): the low-level decompressor `tinfl_decompress` and the zlib-style stream
//! API over it (`mz_inflateInit`, `mz_inflateInit2`, `mz_inflate`, `mz_inflateEnd`,
//! `mz_uncompress`, `mz_error`), as the C compiles on a 64-bit Linux (a 64-bit bit buffer;
//! unaligned 8-byte copies, which give the bytes of the byte-by-byte copy).
//!
//! The decompressor is a translation of tinfl's resumable state machine: it suspends where the
//! C returns (more input needed, more output room needed) and resumes where the C resumes, so
//! the bytes it consumes and produces, its statuses, `total_in`, `total_out` and `adler` are the
//! C's for every way of feeding it (whole buffers, pieces, any flush). Only what the C cannot
//! define is changed, with a notice ([`crate::defects`]):
//!
//! - FS-281: a stream that ends early, read with no more input to come, is padded with zero
//!   bytes by miniz (it decodes them); corrected: the stream fails there.
//! - FS-282: length symbols 286 and 287 and distance symbols 30 and 31 (deflate defines none of
//!   them) are read as a length of 0 (miniz then copies 3 bytes, past the buffer when it ends
//!   within them) and a distance of 0 (the byte under the cursor); corrected: the stream fails.
//! - FS-283: an incomplete Huffman code (no codes, or one code) is accepted, and a bit pattern
//!   without a code is decoded as symbol 0 (taking no bits in the lookup table); corrected:
//!   decoding such a pattern fails.
//! - FS-284: in the stream API, a distance farther back than the output so far (or a distance
//!   of 0 before the dictionary was ever filled) reads miniz's 32 KiB dictionary where nothing
//!   was written yet (memory never written: never reproduced; read as zeros with `reproduce`);
//!   corrected: the stream fails.
//!
//! With [`UpstreamDefects::Reproduce`] each gives miniz's result where it is defined (the
//! zeros of FS-281, the copies of FS-282 within the buffer, the symbols of FS-283); a write or
//! read past the buffers is never reproduced (the stream fails, named).

use crate::defects::{self, Notices, UpstreamDefects};

/// `mz_inflate`'s return codes.
pub const MZ_OK: i32 = 0;
pub const MZ_STREAM_END: i32 = 1;
pub const MZ_NEED_DICT: i32 = 2;
pub const MZ_ERRNO: i32 = -1;
pub const MZ_STREAM_ERROR: i32 = -2;
pub const MZ_DATA_ERROR: i32 = -3;
pub const MZ_MEM_ERROR: i32 = -4;
pub const MZ_BUF_ERROR: i32 = -5;
pub const MZ_VERSION_ERROR: i32 = -6;
pub const MZ_PARAM_ERROR: i32 = -10000;

/// The flush values.
pub const MZ_NO_FLUSH: i32 = 0;
pub const MZ_PARTIAL_FLUSH: i32 = 1;
pub const MZ_SYNC_FLUSH: i32 = 2;
pub const MZ_FULL_FLUSH: i32 = 3;
pub const MZ_FINISH: i32 = 4;
pub const MZ_BLOCK: i32 = 5;

/// The window miniz supports (a zlib stream with `15`, raw deflate with `-15`).
pub const MZ_DEFAULT_WINDOW_BITS: i32 = 15;

/// `tinfl_decompress`'s flags.
pub const TINFL_FLAG_PARSE_ZLIB_HEADER: u32 = 1;
pub const TINFL_FLAG_HAS_MORE_INPUT: u32 = 2;
pub const TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF: u32 = 4;
pub const TINFL_FLAG_COMPUTE_ADLER32: u32 = 8;

/// The size of the stream API's dictionary.
pub const TINFL_LZ_DICT_SIZE: usize = 32768;

/// `tinfl_status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TinflStatus {
    BadParam = -3,
    Adler32Mismatch = -2,
    Failed = -1,
    Done = 0,
    NeedsMoreInput = 1,
    HasMoreOutput = 2,
}

impl TinflStatus {
    fn negative(self) -> bool {
        (self as i32) < 0
    }
}

const FAST_BITS: u32 = 10;
const FAST_SIZE: usize = 1 << FAST_BITS;

const LENGTH_BASE: [u32; 31] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258, 0, 0,
];
const LENGTH_EXTRA: [u32; 31] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0, 0, 0,
];
const DIST_BASE: [u32; 32] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 0, 0,
];
const DIST_EXTRA: [u32; 32] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13, 0, 0,
];
const DEZIGZAG: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];
const MIN_TABLE_SIZES: [u32; 3] = [257, 1, 4];

#[derive(Clone)]
struct HuffTable {
    code_size: [u8; 288],
    look_up: [i16; FAST_SIZE],
    tree: [i16; 576],
    /// Not miniz's: whether the code is incomplete (FS-283), and its one code's length.
    incomplete: bool,
    single_len: u32,
}

impl HuffTable {
    fn new() -> HuffTable {
        HuffTable {
            code_size: [0; 288],
            look_up: [0; FAST_SIZE],
            tree: [0; 576],
            incomplete: false,
            single_len: 0,
        }
    }
}

/// Where the state machine stands (the C's `m_state`, by the place in its code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum St {
    Start,
    ZHdr0,
    ZHdr1,
    BlockHeader,
    StoredAlign,
    StoredHdr,
    StoredBits,
    StoredBitsWrite,
    StoredCopy,
    DynSizes,
    DynClens,
    Build,
    Lens,
    LensExtra,
    Literals,
    LiteralWrite,
    LenExtra,
    DistDecode,
    DistExtra,
    SlowCopy,
    BlockEnd,
    TrailerAlign,
    Trailer,
    Done,
    Failed,
}

/// A suspension (the C's `TINFL_CR_RETURN`) with its status.
struct Stop(TinflStatus);

/// `tinfl_decompressor`: miniz's inflater state.
#[derive(Clone)]
pub struct Tinfl {
    st: St,
    num_bits: u32,
    zhdr0: u32,
    zhdr1: u32,
    z_adler32: u32,
    final_: u32,
    type_: u32,
    check_adler32: u32,
    dist: u32,
    counter: u32,
    num_extra: u32,
    table_sizes: [u32; 3],
    bit_buf: u64,
    dist_from_out_buf_start: usize,
    tables: [HuffTable; 3],
    raw_header: [u32; 4],
    len_codes: [u8; 288 + 32 + 137],
    // Not miniz's: where a Huffman decode's byte-by-byte fill stopped (the C resumes inside
    // it), and the output written in the stream's whole life (FS-284).
    in_fill: bool,
    fill_at_get: bool,
    history: u64,
}

impl Default for Tinfl {
    fn default() -> Self {
        Tinfl::new()
    }
}

/// One call's buffers: the input, the output from its start (`out[0]` is the C's
/// `pOut_buf_start`), the cursors and the flags.
struct Io<'a> {
    inp: &'a [u8],
    ip: usize,
    out: &'a mut [u8],
    op: usize,
    out_end: usize,
    flags: u32,
    mask: usize,
    op0: usize,
}

impl Tinfl {
    /// `tinfl_init`.
    pub fn new() -> Tinfl {
        Tinfl {
            st: St::Start,
            num_bits: 0,
            zhdr0: 0,
            zhdr1: 0,
            z_adler32: 1,
            final_: 0,
            type_: 0,
            check_adler32: 1,
            dist: 0,
            counter: 0,
            num_extra: 0,
            table_sizes: [0; 3],
            bit_buf: 0,
            dist_from_out_buf_start: 0,
            tables: [HuffTable::new(), HuffTable::new(), HuffTable::new()],
            raw_header: [0; 4],
            len_codes: [0; 457],
            in_fill: false,
            fill_at_get: false,
            history: 0,
        }
    }

    /// `tinfl_get_adler32`.
    pub fn adler32(&self) -> u32 {
        self.check_adler32
    }

    /// `tinfl_decompress`: decodes from `inp` into `out[out_next..]` (`out[0]` is the output's
    /// start, against which distances are taken; without
    /// [`TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF`] `out.len()` must be a power of two and the
    /// output wraps). Returns the status, the input bytes consumed and the output bytes made.
    pub fn decompress(
        &mut self,
        inp: &[u8],
        out: &mut [u8],
        out_next: usize,
        flags: u32,
        notices: &mut Notices,
    ) -> (TinflStatus, usize, usize) {
        let mask = if flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0 {
            usize::MAX
        } else {
            out.len().wrapping_sub(1)
        };
        if (mask.wrapping_add(1) & mask) != 0 || out_next > out.len() {
            return (TinflStatus::BadParam, 0, 0);
        }
        let out_end = out.len();
        let mut io = Io {
            inp,
            ip: 0,
            out,
            op: out_next,
            out_end,
            flags,
            mask,
            op0: out_next,
        };
        let Stop(mut status) = loop {
            if let Err(stop) = self.step(&mut io, notices) {
                break stop;
            }
        };
        let consumed = io.ip;
        let made = io.op - out_next;
        if flags & (TINFL_FLAG_PARSE_ZLIB_HEADER | TINFL_FLAG_COMPUTE_ADLER32) != 0
            && !status.negative()
        {
            self.check_adler32 = adler32_update(self.check_adler32, &io.out[out_next..io.op]);
            if status == TinflStatus::Done
                && flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0
                && self.check_adler32 != self.z_adler32
            {
                status = TinflStatus::Adler32Mismatch;
            }
        }
        self.history += made as u64;
        (status, consumed, made)
    }

    fn fail(&mut self) -> Stop {
        self.st = St::Failed;
        Stop(TinflStatus::Failed)
    }

    /// `TINFL_GET_BYTE`.
    fn get_byte(&mut self, io: &mut Io, notices: &mut Notices) -> Result<u32, Stop> {
        if io.ip < io.inp.len() {
            io.ip += 1;
            return Ok(io.inp[io.ip - 1] as u32);
        }
        if io.flags & TINFL_FLAG_HAS_MORE_INPUT != 0 {
            return Err(Stop(TinflStatus::NeedsMoreInput));
        }
        notices.add(
            &defects::FS_281,
            "a deflate stream ended early with no more input to come (miniz reads zero bytes there)",
        );
        if notices.reproduce() {
            Ok(0)
        } else {
            Err(self.fail())
        }
    }

    /// `TINFL_NEED_BITS` (at least one byte).
    fn need_bits(&mut self, io: &mut Io, n: u32, notices: &mut Notices) -> Result<(), Stop> {
        loop {
            let c = self.get_byte(io, notices)?;
            self.bit_buf |= (c as u64) << self.num_bits;
            self.num_bits += 8;
            if self.num_bits >= n {
                return Ok(());
            }
        }
    }

    /// `TINFL_GET_BITS`.
    fn get_bits(&mut self, io: &mut Io, n: u32, notices: &mut Notices) -> Result<u32, Stop> {
        if self.num_bits < n {
            self.need_bits(io, n, notices)?;
        }
        let b = (self.bit_buf & ((1u64 << n) - 1)) as u32;
        self.bit_buf >>= n;
        self.num_bits -= n;
        Ok(b)
    }

    /// `TINFL_SKIP_BITS`.
    fn skip_bits(&mut self, io: &mut Io, n: u32, notices: &mut Notices) -> Result<(), Stop> {
        if self.num_bits < n {
            self.need_bits(io, n, notices)?;
        }
        self.bit_buf >>= n;
        self.num_bits -= n;
        Ok(())
    }

    /// The table's entry for the bits in the buffer, (symbol, code length): the C's lookup
    /// and tree walk.
    fn lookup(&self, t: usize) -> (u32, u32) {
        let tab = &self.tables[t];
        let mut temp = tab.look_up[(self.bit_buf as usize) & (FAST_SIZE - 1)] as i32;
        if temp >= 0 {
            ((temp & 511) as u32, (temp >> 9) as u32)
        } else {
            let mut code_len = FAST_BITS;
            loop {
                temp = tab.tree[(!temp + ((self.bit_buf >> code_len) & 1) as i32) as usize] as i32;
                code_len += 1;
                if temp >= 0 {
                    break;
                }
            }
            (temp as u32, code_len)
        }
    }

    /// FS-283: whether decoding `code_len` bits from table `t` used a pattern without a code
    /// (an incomplete code has at most one code: all zero bits, of its length).
    fn check_code(&mut self, t: usize, code_len: u32, notices: &mut Notices) -> Result<(), Stop> {
        let tab = &self.tables[t];
        if tab.incomplete
            && (tab.single_len == 0
                || code_len != tab.single_len
                || (self.bit_buf & ((1u64 << code_len) - 1)) != 0)
        {
            notices.add(
                &defects::FS_283,
                "a deflate stream used a bit pattern its incomplete Huffman code gives no symbol (miniz decodes it as symbol 0)",
            );
            if notices.fix() {
                return Err(self.fail());
            }
        }
        Ok(())
    }

    /// `TINFL_HUFF_DECODE`, resumable inside its byte-by-byte fill.
    fn huff_decode(&mut self, io: &mut Io, t: usize, notices: &mut Notices) -> Result<u32, Stop> {
        if !self.in_fill && self.num_bits < 15 {
            if io.inp.len() - io.ip < 2 {
                self.in_fill = true;
                self.fill_at_get = false;
            } else {
                self.bit_buf |= ((io.inp[io.ip] as u64) << self.num_bits)
                    | ((io.inp[io.ip + 1] as u64) << (self.num_bits + 8));
                io.ip += 2;
                self.num_bits += 16;
            }
        }
        if self.in_fill {
            loop {
                if !self.fill_at_get {
                    let tab = &self.tables[t];
                    let mut temp = tab.look_up[(self.bit_buf as usize) & (FAST_SIZE - 1)] as i32;
                    if temp >= 0 {
                        let code_len = (temp >> 9) as u32;
                        if code_len != 0 && self.num_bits >= code_len {
                            break;
                        }
                    } else if self.num_bits > FAST_BITS {
                        let mut code_len = FAST_BITS;
                        loop {
                            temp = tab.tree
                                [(!temp + ((self.bit_buf >> code_len) & 1) as i32) as usize]
                                as i32;
                            code_len += 1;
                            if !(temp < 0 && self.num_bits >= code_len + 1) {
                                break;
                            }
                        }
                        if temp >= 0 {
                            break;
                        }
                    }
                }
                self.fill_at_get = true;
                let c = self.get_byte(io, notices)?;
                self.fill_at_get = false;
                self.bit_buf |= (c as u64) << self.num_bits;
                self.num_bits += 8;
                if self.num_bits >= 15 {
                    break;
                }
            }
            self.in_fill = false;
        }
        let (sym, code_len) = self.lookup(t);
        self.check_code(t, code_len, notices)?;
        self.bit_buf >>= code_len;
        self.num_bits -= code_len;
        Ok(sym)
    }

    /// Builds table `t` from its code sizes (the C's loop body); `false`: an over-subscribed
    /// or incomplete code of more than one symbol (`TINFL_STATUS_FAILED`).
    fn build(&mut self, t: usize) -> bool {
        let size = self.table_sizes[t] as usize;
        let tab = &mut self.tables[t];
        let mut total_syms = [0u32; 16];
        let mut next_code = [0u32; 17];
        tab.look_up = [0; FAST_SIZE];
        tab.tree = [0; 576];
        for i in 0..size {
            total_syms[tab.code_size[i] as usize] += 1;
        }
        let mut used_syms = 0u32;
        let mut total = 0u32;
        for i in 1..=15 {
            used_syms += total_syms[i];
            total = (total + total_syms[i]) << 1;
            next_code[i + 1] = total;
        }
        if total != 65536 && used_syms > 1 {
            return false;
        }
        tab.incomplete = total != 65536;
        tab.single_len = 0;
        let mut tree_next: i32 = -1;
        for sym_index in 0..size {
            let code_size = tab.code_size[sym_index] as u32;
            if code_size == 0 {
                continue;
            }
            if tab.incomplete {
                tab.single_len = code_size;
            }
            let mut cur_code = next_code[code_size as usize];
            next_code[code_size as usize] += 1;
            let mut rev_code = 0u32;
            for _ in 0..code_size {
                rev_code = (rev_code << 1) | (cur_code & 1);
                cur_code >>= 1;
            }
            if code_size <= FAST_BITS {
                let k = ((code_size << 9) | sym_index as u32) as i16;
                while (rev_code as usize) < FAST_SIZE {
                    tab.look_up[rev_code as usize] = k;
                    rev_code += 1 << code_size;
                }
                continue;
            }
            let slot = (rev_code as usize) & (FAST_SIZE - 1);
            let mut tree_cur = tab.look_up[slot] as i32;
            if tree_cur == 0 {
                tab.look_up[slot] = tree_next as i16;
                tree_cur = tree_next;
                tree_next -= 2;
            }
            rev_code >>= FAST_BITS - 1;
            let mut j = code_size;
            while j > FAST_BITS + 1 {
                rev_code >>= 1;
                tree_cur -= (rev_code & 1) as i32;
                let idx = (-tree_cur - 1) as usize;
                if tab.tree[idx] == 0 {
                    tab.tree[idx] = tree_next as i16;
                    tree_cur = tree_next;
                    tree_next -= 2;
                } else {
                    tree_cur = tab.tree[idx] as i32;
                }
                j -= 1;
            }
            rev_code >>= 1;
            tree_cur -= (rev_code & 1) as i32;
            tab.tree[(-tree_cur - 1) as usize] = sym_index as i16;
        }
        true
    }

    fn wait_out(&self, io: &Io) -> Result<(), Stop> {
        if io.op >= io.out_end {
            Err(Stop(TinflStatus::HasMoreOutput))
        } else {
            Ok(())
        }
    }

    /// One state's code, up to the next state or a suspension.
    fn step(&mut self, io: &mut Io, notices: &mut Notices) -> Result<(), Stop> {
        match self.st {
            St::Start => {
                self.bit_buf = 0;
                self.num_bits = 0;
                self.dist = 0;
                self.counter = 0;
                self.num_extra = 0;
                self.zhdr0 = 0;
                self.zhdr1 = 0;
                self.z_adler32 = 1;
                self.check_adler32 = 1;
                self.st = if io.flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0 {
                    St::ZHdr0
                } else {
                    St::BlockHeader
                };
            }
            St::ZHdr0 => {
                self.zhdr0 = self.get_byte(io, notices)?;
                self.st = St::ZHdr1;
            }
            St::ZHdr1 => {
                self.zhdr1 = self.get_byte(io, notices)?;
                let (h0, h1) = (self.zhdr0, self.zhdr1);
                let mut bad = (h0 * 256 + h1) % 31 != 0 || (h1 & 32) != 0 || (h0 & 15) != 8;
                if io.flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF == 0 {
                    let w = 1u64 << (8 + (h0 >> 4));
                    bad |= w > 32768 || (io.mask as u64).wrapping_add(1) < w;
                }
                if bad {
                    return Err(self.fail());
                }
                self.st = St::BlockHeader;
            }
            St::BlockHeader => {
                self.final_ = self.get_bits(io, 3, notices)?;
                self.type_ = self.final_ >> 1;
                match self.type_ {
                    0 => self.st = St::StoredAlign,
                    3 => return Err(self.fail()),
                    1 => {
                        let p = &mut self.tables[0].code_size;
                        p[..144].fill(8);
                        p[144..256].fill(9);
                        p[256..280].fill(7);
                        p[280..288].fill(8);
                        self.table_sizes[0] = 288;
                        self.table_sizes[1] = 32;
                        self.tables[1].code_size[..32].fill(5);
                        self.st = St::Build;
                    }
                    _ => {
                        self.counter = 0;
                        self.st = St::DynSizes;
                    }
                }
            }
            St::StoredAlign => {
                let n = self.num_bits & 7;
                self.skip_bits(io, n, notices)?;
                self.counter = 0;
                self.st = St::StoredHdr;
            }
            St::StoredHdr => {
                while self.counter < 4 {
                    let v = if self.num_bits != 0 {
                        self.get_bits(io, 8, notices)?
                    } else {
                        self.get_byte(io, notices)?
                    };
                    self.raw_header[self.counter as usize] = v;
                    self.counter += 1;
                }
                let h = self.raw_header;
                self.counter = h[0] | (h[1] << 8);
                if self.counter != (0xFFFF ^ (h[2] | (h[3] << 8))) {
                    return Err(self.fail());
                }
                self.st = St::StoredBits;
            }
            St::StoredBits => {
                if self.counter != 0 && self.num_bits != 0 {
                    self.dist = self.get_bits(io, 8, notices)?;
                    self.st = St::StoredBitsWrite;
                } else {
                    self.st = St::StoredCopy;
                }
            }
            St::StoredBitsWrite => {
                self.wait_out(io)?;
                io.out[io.op] = self.dist as u8;
                io.op += 1;
                self.counter -= 1;
                self.st = St::StoredBits;
            }
            St::StoredCopy => {
                if self.counter == 0 {
                    self.st = St::BlockEnd;
                    return Ok(());
                }
                self.wait_out(io)?;
                if io.ip >= io.inp.len() {
                    if io.flags & TINFL_FLAG_HAS_MORE_INPUT != 0 {
                        return Err(Stop(TinflStatus::NeedsMoreInput));
                    }
                    return Err(self.fail());
                }
                let n = (io.out_end - io.op)
                    .min(io.inp.len() - io.ip)
                    .min(self.counter as usize);
                io.out[io.op..io.op + n].copy_from_slice(&io.inp[io.ip..io.ip + n]);
                io.ip += n;
                io.op += n;
                self.counter -= n as u32;
            }
            St::DynSizes => {
                while self.counter < 3 {
                    let nbits = [5, 5, 4][self.counter as usize];
                    let v = self.get_bits(io, nbits, notices)?;
                    self.table_sizes[self.counter as usize] =
                        v + MIN_TABLE_SIZES[self.counter as usize];
                    self.counter += 1;
                }
                self.tables[2].code_size = [0; 288];
                self.counter = 0;
                self.st = St::DynClens;
            }
            St::DynClens => {
                while self.counter < self.table_sizes[2] {
                    let s = self.get_bits(io, 3, notices)?;
                    self.tables[2].code_size[DEZIGZAG[self.counter as usize]] = s as u8;
                    self.counter += 1;
                }
                self.table_sizes[2] = 19;
                self.st = St::Build;
            }
            St::Build => {
                // for ( ; (int)r->m_type >= 0; r->m_type--): the code-length table (type 2)
                // first and the lengths read with it, then tables 1 and 0.
                if (self.type_ as i32) < 0 {
                    self.st = St::Literals;
                    return Ok(());
                }
                let t = self.type_ as usize;
                if !self.build(t) {
                    return Err(self.fail());
                }
                if t == 2 {
                    self.counter = 0;
                    self.st = St::Lens;
                } else {
                    self.type_ = self.type_.wrapping_sub(1);
                }
            }
            St::Lens => {
                let total = self.table_sizes[0] + self.table_sizes[1];
                if self.counter >= total {
                    if total != self.counter {
                        return Err(self.fail());
                    }
                    let (n0, n1) = (self.table_sizes[0] as usize, self.table_sizes[1] as usize);
                    let lc = self.len_codes;
                    self.tables[0].code_size[..n0].copy_from_slice(&lc[..n0]);
                    self.tables[1].code_size[..n1].copy_from_slice(&lc[n0..n0 + n1]);
                    self.type_ = self.type_.wrapping_sub(1);
                    self.st = St::Build;
                    return Ok(());
                }
                self.dist = self.huff_decode(io, 2, notices)?;
                if self.dist < 16 {
                    self.len_codes[self.counter as usize] = self.dist as u8;
                    self.counter += 1;
                    return Ok(());
                }
                if self.dist == 16 && self.counter == 0 {
                    return Err(self.fail());
                }
                self.num_extra = [2, 3, 7][(self.dist - 16) as usize];
                self.st = St::LensExtra;
            }
            St::LensExtra => {
                let mut s = self.get_bits(io, self.num_extra, notices)?;
                s += [3, 3, 11][(self.dist - 16) as usize];
                let c = self.counter as usize;
                let v = if self.dist == 16 {
                    self.len_codes[c - 1]
                } else {
                    0
                };
                self.len_codes[c..c + s as usize].fill(v);
                self.counter += s;
                self.st = St::Lens;
            }
            St::Literals => {
                if !self.in_fill && io.inp.len() - io.ip >= 4 && io.out_end - io.op >= 2 {
                    // The fast path: two symbols from one refill of 4 bytes.
                    if self.num_bits < 30 {
                        let w = u32::from_le_bytes(io.inp[io.ip..io.ip + 4].try_into().unwrap());
                        self.bit_buf |= (w as u64) << self.num_bits;
                        io.ip += 4;
                        self.num_bits += 32;
                    }
                    let (sym, code_len) = self.lookup(0);
                    self.check_code(0, code_len, notices)?;
                    self.counter = sym;
                    self.bit_buf >>= code_len;
                    self.num_bits -= code_len;
                    if self.counter & 256 != 0 {
                        return self.length_symbol();
                    }
                    let (sym2, code_len2) = self.lookup(0);
                    self.check_code(0, code_len2, notices)?;
                    self.bit_buf >>= code_len2;
                    self.num_bits -= code_len2;
                    io.out[io.op] = self.counter as u8;
                    if sym2 & 256 != 0 {
                        io.op += 1;
                        self.counter = sym2;
                        return self.length_symbol();
                    }
                    io.out[io.op + 1] = sym2 as u8;
                    io.op += 2;
                } else {
                    self.counter = self.huff_decode(io, 0, notices)?;
                    if self.counter >= 256 {
                        return self.length_symbol();
                    }
                    self.st = St::LiteralWrite;
                }
            }
            St::LiteralWrite => {
                self.wait_out(io)?;
                io.out[io.op] = self.counter as u8;
                io.op += 1;
                self.st = St::Literals;
            }
            St::LenExtra => {
                if self.num_extra != 0 {
                    let e = self.get_bits(io, self.num_extra, notices)?;
                    self.counter += e;
                }
                self.st = St::DistDecode;
            }
            St::DistDecode => {
                self.dist = self.huff_decode(io, 1, notices)?;
                if self.dist >= 30 {
                    notices.add(
                        &defects::FS_282,
                        "a deflate stream used a distance symbol deflate does not define (miniz reads a distance of 0)",
                    );
                    if notices.fix() {
                        return Err(self.fail());
                    }
                }
                self.num_extra = DIST_EXTRA[self.dist as usize];
                self.dist = DIST_BASE[self.dist as usize];
                self.st = St::DistExtra;
            }
            St::DistExtra => {
                if self.num_extra != 0 {
                    let e = self.get_bits(io, self.num_extra, notices)?;
                    self.dist += e;
                }
                return self.copy_match(io, notices);
            }
            St::SlowCopy => loop {
                // while (counter--) { wait; *cur++ = start[(dfobs++ - dist) & mask]; }
                if self.counter == 0 {
                    self.counter = u32::MAX;
                    self.st = St::Literals;
                    return Ok(());
                }
                self.wait_out(io)?;
                self.counter -= 1;
                let src = self
                    .dist_from_out_buf_start
                    .wrapping_sub(self.dist as usize)
                    & io.mask;
                self.dist_from_out_buf_start = self.dist_from_out_buf_start.wrapping_add(1);
                io.out[io.op] = io.out[src];
                io.op += 1;
            },
            St::BlockEnd => {
                if self.final_ & 1 == 0 {
                    self.st = St::BlockHeader;
                } else if io.flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0 {
                    self.st = St::TrailerAlign;
                } else {
                    self.st = St::Done;
                }
            }
            St::TrailerAlign => {
                let n = self.num_bits & 7;
                self.skip_bits(io, n, notices)?;
                self.counter = 0;
                self.st = St::Trailer;
            }
            St::Trailer => {
                while self.counter < 4 {
                    let s = if self.num_bits != 0 {
                        self.get_bits(io, 8, notices)?
                    } else {
                        self.get_byte(io, notices)?
                    };
                    self.z_adler32 = (self.z_adler32 << 8) | s;
                    self.counter += 1;
                }
                self.st = St::Done;
            }
            St::Done => return Err(Stop(TinflStatus::Done)),
            St::Failed => return Err(Stop(TinflStatus::Failed)),
        }
        Ok(())
    }

    /// After a symbol of 256 or more: the end of the block, or a length.
    fn length_symbol(&mut self) -> Result<(), Stop> {
        self.counter &= 511;
        if self.counter == 256 {
            self.st = St::BlockEnd;
            return Ok(());
        }
        let i = (self.counter - 257) as usize;
        self.num_extra = LENGTH_EXTRA[i];
        self.counter = LENGTH_BASE[i];
        self.st = St::LenExtra;
        Ok(())
    }

    /// The copy of a match: `counter` bytes from `dist` back.
    fn copy_match(&mut self, io: &mut Io, notices: &mut Notices) -> Result<(), Stop> {
        let counter = self.counter as usize;
        let dist = self.dist as usize;
        if counter == 0 {
            notices.add(
                &defects::FS_282,
                "a deflate stream used a length symbol deflate does not define (miniz copies 3 bytes)",
            );
            if notices.fix() {
                return Err(self.fail());
            }
        }
        self.dist_from_out_buf_start = io.op;
        let non_wrapping = io.flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0;
        if dist > io.op && non_wrapping {
            return Err(self.fail());
        }
        if !non_wrapping {
            let written = self.history + (io.op - io.op0) as u64;
            if dist as u64 > written || (dist == 0 && written < io.out_end as u64) {
                notices.add(
                    &defects::FS_284,
                    "a deflate stream copied from farther back than its output so far (miniz reads its dictionary where nothing was written: read as zeros)",
                );
                if notices.fix() {
                    return Err(self.fail());
                }
            }
        }
        let src = io.op.wrapping_sub(dist) & io.mask;
        if counter == 0 {
            // The C's 3-byte copy loop runs once: 3 bytes from src, the cursor moved by 3.
            if io.op + 3 > io.out_end || src + 3 > io.out_end {
                notices.add(
                    &defects::FS_282,
                    "a length symbol deflate does not define made miniz write or read past its buffer: not reproduced",
                );
                return Err(self.fail());
            }
            for k in 0..3 {
                io.out[io.op + k] = io.out[src + k];
            }
            io.op += 3;
            self.st = St::Literals;
            return Ok(());
        }
        if io.op.max(src) + counter > io.out_end {
            self.st = St::SlowCopy;
            return Ok(());
        }
        // Within the buffer: byte by byte (the C's 8- and 3-byte copies give these bytes).
        for k in 0..counter {
            io.out[io.op + k] = io.out[src + k];
        }
        io.op += counter;
        self.st = St::Literals;
        Ok(())
    }
}

/// The adler-32 update the C makes at its exit (blocks of 5,552 bytes, the first of the length
/// modulo 5,552).
fn adler32_update(adler: u32, mut buf: &[u8]) -> u32 {
    let mut s1 = adler & 0xffff;
    let mut s2 = adler >> 16;
    let mut block_len = buf.len() % 5552;
    while !buf.is_empty() {
        for &b in &buf[..block_len] {
            s1 += b as u32;
            s2 += s1;
        }
        s1 %= 65521;
        s2 %= 65521;
        buf = &buf[block_len..];
        block_len = 5552;
    }
    (s2 << 16) + s1
}

struct InflateState {
    decomp: Tinfl,
    dict_ofs: usize,
    dict_avail: usize,
    first_call: bool,
    has_flushed: bool,
    window_bits: i32,
    dict: Box<[u8; TINFL_LZ_DICT_SIZE]>,
    last_status: TinflStatus,
}

/// What one [`InflateStream::inflate`] call did: its return code and the bytes it consumed
/// from the input and wrote to the output (the C's advance of `next_in` and `next_out`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inflated {
    pub ret: i32,
    pub consumed: usize,
    pub produced: usize,
}

/// `mz_stream` for inflating: the fields a caller reads (`total_in`, `total_out`, `adler`,
/// `data_type`), the state, and the notices of larmorx's corrections.
pub struct InflateStream {
    state: Option<Box<InflateState>>,
    pub total_in: u64,
    pub total_out: u64,
    pub adler: u64,
    pub data_type: i32,
    pub notices: Notices,
}

impl InflateStream {
    /// A stream with every field zero (the C's `memset(&stream, 0, sizeof(stream))`).
    pub fn new(mode: UpstreamDefects) -> InflateStream {
        InflateStream {
            state: None,
            total_in: 0,
            total_out: 0,
            adler: 0,
            data_type: 0,
            notices: Notices::new(mode),
        }
    }

    /// `mz_inflateInit2`.
    pub fn init2(&mut self, window_bits: i32) -> i32 {
        if window_bits != MZ_DEFAULT_WINDOW_BITS && -window_bits != MZ_DEFAULT_WINDOW_BITS {
            return MZ_PARAM_ERROR;
        }
        self.data_type = 0;
        self.adler = 0;
        self.total_in = 0;
        self.total_out = 0;
        self.state = Some(Box::new(InflateState {
            decomp: Tinfl::new(),
            dict_ofs: 0,
            dict_avail: 0,
            first_call: true,
            has_flushed: false,
            window_bits,
            dict: Box::new([0; TINFL_LZ_DICT_SIZE]),
            last_status: TinflStatus::NeedsMoreInput,
        }));
        MZ_OK
    }

    /// `mz_inflateInit`.
    pub fn init(&mut self) -> i32 {
        self.init2(MZ_DEFAULT_WINDOW_BITS)
    }

    /// `mz_inflateEnd`.
    pub fn end(&mut self) -> i32 {
        self.state = None;
        MZ_OK
    }

    /// `mz_inflate` with `next_in`/`avail_in` = `input` and `next_out`/`avail_out` = `output`
    /// (lengths above `u32::MAX` cut to it: the C's counts are `unsigned int`s).
    pub fn inflate(&mut self, input: &[u8], output: &mut [u8], flush: i32) -> Inflated {
        let input = &input[..input.len().min(u32::MAX as usize)];
        let out_len = output.len().min(u32::MAX as usize);
        let output = &mut output[..out_len];
        let mut r = Inflated {
            ret: MZ_STREAM_ERROR,
            consumed: 0,
            produced: 0,
        };
        let Some(st) = self.state.as_mut() else {
            return r;
        };
        let flush = if flush == MZ_PARTIAL_FLUSH {
            MZ_SYNC_FLUSH
        } else {
            flush
        };
        if flush != 0 && flush != MZ_SYNC_FLUSH && flush != MZ_FINISH {
            return r;
        }
        let mut flags = TINFL_FLAG_COMPUTE_ADLER32;
        if st.window_bits > 0 {
            flags |= TINFL_FLAG_PARSE_ZLIB_HEADER;
        }
        let orig_avail_in = input.len();
        let first_call = st.first_call;
        st.first_call = false;
        if st.last_status.negative() {
            r.ret = MZ_DATA_ERROR;
            return r;
        }
        if st.has_flushed && flush != MZ_FINISH {
            return r;
        }
        st.has_flushed |= flush == MZ_FINISH;
        let notices = &mut self.notices;

        if flush == MZ_FINISH && first_call {
            flags |= TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
            let (status, inb, outb) = st.decomp.decompress(input, output, 0, flags, notices);
            st.last_status = status;
            self.total_in += inb as u64;
            self.adler = st.decomp.adler32() as u64;
            self.total_out += outb as u64;
            r.consumed = inb;
            r.produced = outb;
            r.ret = if status.negative() {
                MZ_DATA_ERROR
            } else if status != TinflStatus::Done {
                st.last_status = TinflStatus::Failed;
                MZ_BUF_ERROR
            } else {
                MZ_STREAM_END
            };
            return r;
        }
        if flush != MZ_FINISH {
            flags |= TINFL_FLAG_HAS_MORE_INPUT;
        }
        if st.dict_avail != 0 {
            let n = st.dict_avail.min(output.len());
            output[..n].copy_from_slice(&st.dict[st.dict_ofs..st.dict_ofs + n]);
            self.total_out += n as u64;
            st.dict_avail -= n;
            st.dict_ofs = (st.dict_ofs + n) & (TINFL_LZ_DICT_SIZE - 1);
            r.produced = n;
            r.ret = if st.last_status == TinflStatus::Done && st.dict_avail == 0 {
                MZ_STREAM_END
            } else {
                MZ_OK
            };
            return r;
        }
        let (mut ip, mut op) = (0usize, 0usize);
        let status = loop {
            let (status, inb, outb) =
                st.decomp
                    .decompress(&input[ip..], &mut st.dict[..], st.dict_ofs, flags, notices);
            st.last_status = status;
            ip += inb;
            self.total_in += inb as u64;
            self.adler = st.decomp.adler32() as u64;
            st.dict_avail = outb;
            let n = st.dict_avail.min(output.len() - op);
            output[op..op + n].copy_from_slice(&st.dict[st.dict_ofs..st.dict_ofs + n]);
            op += n;
            self.total_out += n as u64;
            st.dict_avail -= n;
            st.dict_ofs = (st.dict_ofs + n) & (TINFL_LZ_DICT_SIZE - 1);
            r.consumed = ip;
            r.produced = op;
            let (avail_in, avail_out) = (input.len() - ip, output.len() - op);
            if status.negative() {
                r.ret = MZ_DATA_ERROR;
                return r;
            } else if status == TinflStatus::NeedsMoreInput && orig_avail_in == 0 {
                r.ret = MZ_BUF_ERROR;
                return r;
            } else if flush == MZ_FINISH {
                if status == TinflStatus::Done {
                    r.ret = if st.dict_avail != 0 {
                        MZ_BUF_ERROR
                    } else {
                        MZ_STREAM_END
                    };
                    return r;
                } else if avail_out == 0 {
                    r.ret = MZ_BUF_ERROR;
                    return r;
                }
            } else if status == TinflStatus::Done
                || avail_in == 0
                || avail_out == 0
                || st.dict_avail != 0
            {
                break status;
            }
        };
        r.ret = if status == TinflStatus::Done && st.dict_avail == 0 {
            MZ_STREAM_END
        } else {
            MZ_OK
        };
        r
    }
}

/// `mz_uncompress`: a zlib stream from `src` into `dest`; `dest_len` in: the room, out: the
/// length written (on success only, as the C). Returns the code and the notices.
pub fn uncompress(
    dest: &mut [u8],
    dest_len: &mut u64,
    src: &[u8],
    mode: UpstreamDefects,
) -> (i32, Notices) {
    let mut s = InflateStream::new(mode);
    if (src.len() as u64 | *dest_len) > 0xFFFF_FFFF {
        return (MZ_PARAM_ERROR, s.notices);
    }
    let room = (*dest_len as usize).min(dest.len());
    let status = s.init();
    if status != MZ_OK {
        return (status, s.notices);
    }
    let r = s.inflate(src, &mut dest[..room], MZ_FINISH);
    if r.ret != MZ_STREAM_END {
        s.end();
        let code = if r.ret == MZ_BUF_ERROR && src.len() == r.consumed {
            MZ_DATA_ERROR
        } else {
            r.ret
        };
        return (code, s.notices);
    }
    *dest_len = s.total_out;
    (s.end(), s.notices)
}

/// `mz_error`: the text of a return code (`None` for a code it does not know).
pub fn error_text(err: i32) -> Option<&'static str> {
    Some(match err {
        MZ_OK => "",
        MZ_STREAM_END => "stream end",
        MZ_NEED_DICT => "need dictionary",
        MZ_ERRNO => "file error",
        MZ_STREAM_ERROR => "stream error",
        MZ_DATA_ERROR => "data error",
        MZ_MEM_ERROR => "out of memory",
        MZ_BUF_ERROR => "buf error",
        MZ_VERSION_ERROR => "version error",
        MZ_PARAM_ERROR => "parameter error",
        _ => return None,
    })
}

#[doc(hidden)]
pub mod streams;

#[cfg(test)]
mod tests;
