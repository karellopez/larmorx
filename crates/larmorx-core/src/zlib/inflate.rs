// SPDX-License-Identifier: Apache-2.0 AND Zlib
//! zlib 1.2.11's decompressor as Ubuntu 22.04 builds it: `inflate.c` (the zlib and raw
//! formats: `inflateInit2_`, `inflateReset`, `inflateResetKeep`, `inflateReset2`,
//! `inflatePrime`, `inflate`, `inflateEnd`, `inflateSync` with `syncsearch`, `updatewindow`,
//! `fixedtables`), `inftrees.c` (`inflate_table`) and `inffast.c` (`inflate_fast`), with
//! `GUNZIP` on, not `INFLATE_STRICT`, not `INFLATE_ALLOW_INVALID_DISTANCE_TOOFAR_ARRR`, not
//! `PKZIP_BUG_WORKAROUND`; Ubuntu's IBM Z hooks compile to nothing on x86-64.
//!
//! gzip headers (`windowBits` above 15 + 16) are not translated: [`Inflate::init2`] refuses
//! them with `Z_STREAM_ERROR` (no caller in larmorx asks for them here; gzip files are read
//! through [`crate::miniz`] and zlib-rs). Everything a zlib or raw stream drives is: the
//! messages (`strm->msg`, which stays set until the next reset, as zlib's does), the order in
//! which input is consumed and output written in each call, the window, `inflateSync`'s search
//! (with zlib's shift of the bit buffer to the left, `hold <<= bits & 7`).

use super::*;

const MAXBITS: usize = 15;
const ENOUGH_LENS: usize = 852;
const ENOUGH_DISTS: usize = 592;
const ENOUGH: usize = ENOUGH_LENS + ENOUGH_DISTS;
const DEF_WBITS: i32 = MAX_WBITS;

/// `code`: the operation, the bits, the value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Code {
    op: u8,
    bits: u8,
    val: u16,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CodeType {
    Codes,
    Lens,
    Dists,
}

/// `inflate_mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u16)]
pub enum Mode {
    Head = 16180,
    Flags,
    Time,
    Os,
    Exlen,
    Extra,
    Name,
    Comment,
    Hcrc,
    DictId,
    Dict,
    Type,
    TypeDo,
    Stored,
    Copy_,
    Copy,
    Table,
    LenLens,
    CodeLens,
    Len_,
    Len,
    LenExt,
    Dist,
    DistExt,
    Match,
    Lit,
    Check,
    Length,
    Done,
    Bad,
    Mem,
    Sync,
}

/// Where a decoding table lives: in the state's `codes`, or one of the fixed tables.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Table {
    Codes(usize),
    FixedLen,
    FixedDist,
}

static LBASE: [u16; 31] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258, 0, 0,
];
static LEXT: [u16; 31] = [
    16, 16, 16, 16, 16, 16, 16, 16, 17, 17, 17, 17, 18, 18, 18, 18, 19, 19, 19, 19, 20, 20, 20, 20,
    21, 21, 21, 21, 16, 77, 202,
];
static DBASE: [u16; 32] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 0, 0,
];
static DEXT: [u16; 32] = [
    16, 16, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 23, 24, 24, 25, 25, 26, 26,
    27, 27, 28, 28, 29, 29, 64, 64,
];
static ORDER: [u16; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// `inflate_table(type, lens, codes, &next, &bits, work)` writing from `table[*next..]`: the
/// return value (0, 1 for "not enough room", -1 for an over-subscribed or incomplete set);
/// `*next` and `*bits` updated as zlib updates `*table` and `*bits`.
fn inflate_table(
    ty: CodeType,
    lens: &[u16],
    codes: usize,
    table: &mut [Code],
    next_at: &mut usize,
    bits: &mut u32,
    work: &mut [u16; 288],
) -> i32 {
    let mut count = [0u16; MAXBITS + 1];
    let mut offs = [0u16; MAXBITS + 1];
    for &l in &lens[..codes] {
        count[l as usize] = count[l as usize].wrapping_add(1);
    }
    let mut root = *bits;
    let mut max = MAXBITS as u32;
    while max >= 1 {
        if count[max as usize] != 0 {
            break;
        }
        max -= 1;
    }
    if root > max {
        root = max;
    }
    if max == 0 {
        let here = Code {
            op: 64,
            bits: 1,
            val: 0,
        };
        table[*next_at] = here;
        *next_at += 1;
        table[*next_at] = here;
        *next_at += 1;
        *bits = 1;
        return 0;
    }
    let mut min = 1u32;
    while min < max {
        if count[min as usize] != 0 {
            break;
        }
        min += 1;
    }
    if root < min {
        root = min;
    }
    let mut left: i32 = 1;
    for &c in &count[1..=MAXBITS] {
        left <<= 1;
        left -= i32::from(c);
        if left < 0 {
            return -1;
        }
    }
    if left > 0 && (ty == CodeType::Codes || max != 1) {
        return -1;
    }
    offs[1] = 0;
    for len in 1..MAXBITS {
        offs[len + 1] = offs[len].wrapping_add(count[len]);
    }
    for (sym, &l) in lens[..codes].iter().enumerate() {
        if l != 0 {
            work[offs[l as usize] as usize] = sym as u16;
            offs[l as usize] += 1;
        }
    }
    // base and extra: work itself for CODES (never indexed: match is 20, beyond its symbols).
    let (base, extra, mtch): (&[u16], &[u16], u32) = match ty {
        CodeType::Codes => (&[], &[], 20),
        CodeType::Lens => (&LBASE, &LEXT, 257),
        CodeType::Dists => (&DBASE, &DEXT, 0),
    };
    let mut huff: u32 = 0;
    let mut sym: usize = 0;
    let mut len: u32 = min;
    let mut next = *next_at;
    let mut curr = root;
    let mut drop: u32 = 0;
    let mut low: u32 = u32::MAX;
    let mut used: u32 = 1u32 << root;
    let mask = used - 1;
    if (ty == CodeType::Lens && used as usize > ENOUGH_LENS)
        || (ty == CodeType::Dists && used as usize > ENOUGH_DISTS)
    {
        return 1;
    }
    loop {
        let w = u32::from(work[sym]);
        let here = if w + 1 < mtch {
            Code {
                op: 0,
                bits: (len - drop) as u8,
                val: w as u16,
            }
        } else if w >= mtch {
            Code {
                op: extra[(w - mtch) as usize] as u8,
                bits: (len - drop) as u8,
                val: base[(w - mtch) as usize],
            }
        } else {
            Code {
                op: 32 + 64,
                bits: (len - drop) as u8,
                val: 0,
            }
        };
        let incr = 1u32 << (len - drop);
        let mut fill = 1u32 << curr;
        min = fill;
        loop {
            fill -= incr;
            table[next + ((huff >> drop) + fill) as usize] = here;
            if fill == 0 {
                break;
            }
        }
        let mut incr = 1u32 << (len - 1);
        while huff & incr != 0 {
            incr >>= 1;
        }
        if incr != 0 {
            huff &= incr - 1;
            huff += incr;
        } else {
            huff = 0;
        }
        sym += 1;
        count[len as usize] -= 1;
        if count[len as usize] == 0 {
            if len == max {
                break;
            }
            len = u32::from(lens[work[sym] as usize]);
        }
        if len > root && (huff & mask) != low {
            if drop == 0 {
                drop = root;
            }
            next += min as usize;
            curr = len - drop;
            let mut left: i32 = 1 << curr;
            while curr + drop < max {
                left -= i32::from(count[(curr + drop) as usize]);
                if left <= 0 {
                    break;
                }
                curr += 1;
                left <<= 1;
            }
            used += 1u32 << curr;
            if (ty == CodeType::Lens && used as usize > ENOUGH_LENS)
                || (ty == CodeType::Dists && used as usize > ENOUGH_DISTS)
            {
                return 1;
            }
            low = huff & mask;
            table[*next_at + low as usize] = Code {
                op: curr as u8,
                bits: root as u8,
                val: (next - *next_at) as u16,
            };
        }
    }
    if huff != 0 {
        table[next + huff as usize] = Code {
            op: 64,
            bits: (len - drop) as u8,
            val: 0,
        };
    }
    *next_at += used as usize;
    *bits = root;
    0
}

/// The fixed tables (`inffixed.h`, which zlib generates with `inflate_table`).
struct Fixed {
    len: [Code; 512],
    dist: [Code; 32],
}

fn fixed() -> &'static Fixed {
    static FIXED: std::sync::OnceLock<Fixed> = std::sync::OnceLock::new();
    FIXED.get_or_init(|| {
        let mut lens = [0u16; 320];
        let mut work = [0u16; 288];
        let mut sym = 0;
        while sym < 144 {
            lens[sym] = 8;
            sym += 1;
        }
        while sym < 256 {
            lens[sym] = 9;
            sym += 1;
        }
        while sym < 280 {
            lens[sym] = 7;
            sym += 1;
        }
        while sym < 288 {
            lens[sym] = 8;
            sym += 1;
        }
        let mut t = vec![Code::default(); 544];
        let mut next = 0usize;
        let mut bits = 9u32;
        inflate_table(
            CodeType::Lens,
            &lens,
            288,
            &mut t,
            &mut next,
            &mut bits,
            &mut work,
        );
        let mut len = [Code::default(); 512];
        len.copy_from_slice(&t[..512]);
        for l in lens.iter_mut().take(32) {
            *l = 5;
        }
        let mut t2 = vec![Code::default(); 32];
        let mut next = 0usize;
        let mut bits = 5u32;
        inflate_table(
            CodeType::Dists,
            &lens,
            32,
            &mut t2,
            &mut next,
            &mut bits,
            &mut work,
        );
        let mut dist = [Code::default(); 32];
        dist.copy_from_slice(&t2[..32]);
        Fixed { len, dist }
    })
}

/// A `z_stream` with its `inflate_state`. The input and output of each call are slices; the
/// counts and the message are fields.
#[derive(Clone)]
pub struct Inflate {
    mode: Mode,
    last: bool,
    wrap: i32,
    havedict: bool,
    flags: i32,
    dmax: u32,
    check: u64,
    total: u64,
    wbits: u32,
    wsize: u32,
    whave: u32,
    wnext: u32,
    /// The window (`state->window`; empty while not allocated).
    window: Vec<u8>,
    hold: u64,
    bits: u32,
    length: u32,
    offset: u32,
    extra: u32,
    lencode: Table,
    distcode: Table,
    lenbits: u32,
    distbits: u32,
    ncode: u32,
    nlen: u32,
    ndist: u32,
    have: u32,
    next: usize,
    lens: [u16; 320],
    work: [u16; 288],
    codes: Vec<Code>,
    sane: bool,
    back: i32,
    was: u32,
    /// `strm->msg`: the last error's message, kept until the next reset (`None` is C's null
    /// pointer, which glibc's `printf` writes as `(null)`).
    pub msg: Option<&'static str>,
    /// `strm->total_in`, `strm->total_out`, `strm->adler`, `strm->data_type`.
    pub total_in: u64,
    pub total_out: u64,
    pub adler: u64,
    pub data_type: i32,
}

impl Inflate {
    /// `inflateInit(strm)`.
    pub fn init() -> Result<Inflate, i32> {
        Inflate::init2(DEF_WBITS)
    }

    /// `inflateInit2(strm, windowBits)` for the zlib (8 to 15, or 0: the header's size) and
    /// raw (-8 to -15) formats; gzip (`windowBits` 16 and above) is not translated and
    /// refused with `Z_STREAM_ERROR`.
    pub fn init2(window_bits: i32) -> Result<Inflate, i32> {
        let mut s = Inflate {
            mode: Mode::Head,
            last: false,
            wrap: 0,
            havedict: false,
            flags: 0,
            dmax: 0,
            check: 0,
            total: 0,
            wbits: 0,
            wsize: 0,
            whave: 0,
            wnext: 0,
            window: Vec::new(),
            hold: 0,
            bits: 0,
            length: 0,
            offset: 0,
            extra: 0,
            lencode: Table::Codes(0),
            distcode: Table::Codes(0),
            lenbits: 0,
            distbits: 0,
            ncode: 0,
            nlen: 0,
            ndist: 0,
            have: 0,
            next: 0,
            lens: [0; 320],
            work: [0; 288],
            codes: vec![Code::default(); ENOUGH],
            sane: false,
            back: 0,
            was: 0,
            msg: None,
            total_in: 0,
            total_out: 0,
            adler: 0,
            data_type: 0,
        };
        match s.reset2(window_bits) {
            Z_OK => Ok(s),
            e => Err(e),
        }
    }

    /// `inflateResetKeep(strm)`.
    pub fn reset_keep(&mut self) -> i32 {
        self.total_in = 0;
        self.total_out = 0;
        self.total = 0;
        self.msg = None;
        if self.wrap != 0 {
            self.adler = (self.wrap & 1) as u64;
        }
        self.mode = Mode::Head;
        self.last = false;
        self.havedict = false;
        self.dmax = 32768;
        self.hold = 0;
        self.bits = 0;
        self.lencode = Table::Codes(0);
        self.distcode = Table::Codes(0);
        self.next = 0;
        self.sane = true;
        self.back = -1;
        Z_OK
    }

    /// `inflateReset(strm)`.
    pub fn reset(&mut self) -> i32 {
        self.wsize = 0;
        self.whave = 0;
        self.wnext = 0;
        self.reset_keep()
    }

    /// `inflateReset2(strm, windowBits)` (gzip refused, as [`Inflate::init2`]).
    pub fn reset2(&mut self, window_bits: i32) -> i32 {
        let mut wb = window_bits;
        let wrap;
        if wb < 0 {
            wrap = 0;
            wb = -wb;
        } else {
            wrap = (wb >> 4) + 5;
            if wb < 48 {
                wb &= 15;
            }
        }
        if wrap & 2 != 0 {
            return Z_STREAM_ERROR;
        }
        if wb != 0 && !(8..=15).contains(&wb) {
            return Z_STREAM_ERROR;
        }
        if !self.window.is_empty() && self.wbits != wb as u32 {
            self.window = Vec::new();
        }
        self.wrap = wrap;
        self.wbits = wb as u32;
        self.reset()
    }

    /// `inflatePrime(strm, bits, value)`.
    pub fn prime(&mut self, bits: i32, value: i32) -> i32 {
        if bits < 0 {
            self.hold = 0;
            self.bits = 0;
            return Z_OK;
        }
        if bits > 16 || self.bits + bits as u32 > 32 {
            return Z_STREAM_ERROR;
        }
        let value = (i64::from(value) & ((1i64 << bits) - 1)) as u32;
        self.hold = self.hold.wrapping_add(u64::from(value << self.bits));
        self.bits += bits as u32;
        Z_OK
    }

    /// `inflateEnd(strm)`.
    pub fn end(self) -> i32 {
        Z_OK
    }

    /// The decoder's state (`state->mode`).
    pub fn mode(&self) -> Mode {
        self.mode
    }

    fn entry(&self, t: Table, i: usize) -> Code {
        match t {
            Table::Codes(o) => self.codes[o + i],
            Table::FixedLen => fixed().len[i],
            Table::FixedDist => fixed().dist[i],
        }
    }

    /// `updatewindow(strm, end, copy)`: the last `copy` bytes written are `out[..end]`'s end.
    fn updatewindow(&mut self, out: &[u8], end: usize, copy: u32) {
        if self.window.is_empty() {
            self.window = vec![0; 1usize << self.wbits];
        }
        if self.wsize == 0 {
            self.wsize = 1u32 << self.wbits;
            self.wnext = 0;
            self.whave = 0;
        }
        let wsize = self.wsize as usize;
        let mut copy = copy as usize;
        if copy >= wsize {
            self.window[..wsize].copy_from_slice(&out[end - wsize..end]);
            self.wnext = 0;
            self.whave = self.wsize;
        } else {
            let wnext = self.wnext as usize;
            let dist = (wsize - wnext).min(copy);
            self.window[wnext..wnext + dist].copy_from_slice(&out[end - copy..end - copy + dist]);
            copy -= dist;
            if copy != 0 {
                self.window[..copy].copy_from_slice(&out[end - copy..end]);
                self.wnext = copy as u32;
                self.whave = self.wsize;
            } else {
                self.wnext += dist as u32;
                if self.wnext == self.wsize {
                    self.wnext = 0;
                }
                if self.whave < self.wsize {
                    self.whave += dist as u32;
                }
            }
        }
    }

    /// `inflate(strm, flush)` with `next_in`/`avail_in` = `input` and `next_out`/`avail_out` =
    /// `output`: the status, the input bytes used, the output bytes written.
    pub fn inflate(&mut self, input: &[u8], output: &mut [u8], flush: i32) -> (i32, usize, usize) {
        if self.mode == Mode::Type {
            self.mode = Mode::TypeDo;
        }
        // LOAD()
        let avail_in = input.len().min(u32::MAX as usize) as u32;
        let avail_out = output.len().min(u32::MAX as usize) as u32;
        let mut next: usize = 0;
        let mut put: usize = 0;
        let mut have: u32 = avail_in;
        let mut left: u32 = avail_out;
        let mut hold: u64 = self.hold;
        let mut bits: u32 = self.bits;
        let in0 = have;
        let mut out = left;
        let mut ret = Z_OK;

        'leave: loop {
            macro_rules! pullbyte {
                () => {{
                    if have == 0 {
                        break 'leave;
                    }
                    have -= 1;
                    hold = hold.wrapping_add(u64::from(input[next]) << bits);
                    next += 1;
                    bits += 8;
                }};
            }
            macro_rules! needbits {
                ($n:expr) => {{
                    while bits < ($n) as u32 {
                        pullbyte!();
                    }
                }};
            }
            macro_rules! getbits {
                ($n:expr) => {
                    (hold as u32) & ((1u32 << ($n)) - 1)
                };
            }
            macro_rules! dropbits {
                ($n:expr) => {{
                    hold >>= ($n);
                    bits -= ($n) as u32;
                }};
            }
            macro_rules! initbits {
                () => {{
                    hold = 0;
                    bits = 0;
                }};
            }
            macro_rules! bytebits {
                () => {{
                    hold >>= bits & 7;
                    bits -= bits & 7;
                }};
            }
            match self.mode {
                Mode::Head => {
                    if self.wrap == 0 {
                        self.mode = Mode::TypeDo;
                        continue;
                    }
                    needbits!(16);
                    self.flags = 0;
                    if self.wrap & 1 == 0
                        || !((u64::from(getbits!(8)) << 8) + (hold >> 8)).is_multiple_of(31)
                    {
                        self.msg = Some("incorrect header check");
                        self.mode = Mode::Bad;
                        continue;
                    }
                    if getbits!(4) != Z_DEFLATED as u32 {
                        self.msg = Some("unknown compression method");
                        self.mode = Mode::Bad;
                        continue;
                    }
                    dropbits!(4);
                    let len = getbits!(4) + 8;
                    if self.wbits == 0 {
                        self.wbits = len;
                    }
                    if len > 15 || len > self.wbits {
                        self.msg = Some("invalid window size");
                        self.mode = Mode::Bad;
                        continue;
                    }
                    self.dmax = 1u32 << len;
                    self.check = adler32(0, None);
                    self.adler = self.check;
                    self.mode = if hold & 0x200 != 0 {
                        Mode::DictId
                    } else {
                        Mode::Type
                    };
                    initbits!();
                }
                Mode::Flags
                | Mode::Time
                | Mode::Os
                | Mode::Exlen
                | Mode::Extra
                | Mode::Name
                | Mode::Comment
                | Mode::Hcrc => {
                    // gzip headers: unreachable (refused at init).
                    return (Z_STREAM_ERROR, 0, 0);
                }
                Mode::DictId => {
                    needbits!(32);
                    self.check = u64::from((hold as u32).swap_bytes());
                    self.adler = self.check;
                    initbits!();
                    self.mode = Mode::Dict;
                }
                Mode::Dict => {
                    if !self.havedict {
                        // RESTORE()
                        self.hold = hold;
                        self.bits = bits;
                        return (Z_NEED_DICT, next, put);
                    }
                    self.check = adler32(0, None);
                    self.adler = self.check;
                    self.mode = Mode::Type;
                }
                Mode::Type | Mode::TypeDo => {
                    if self.mode == Mode::Type && (flush == Z_BLOCK || flush == Z_TREES) {
                        break 'leave;
                    }
                    if self.last {
                        bytebits!();
                        self.mode = Mode::Check;
                        continue;
                    }
                    needbits!(3);
                    self.last = getbits!(1) != 0;
                    dropbits!(1);
                    match getbits!(2) {
                        0 => self.mode = Mode::Stored,
                        1 => {
                            self.lencode = Table::FixedLen;
                            self.lenbits = 9;
                            self.distcode = Table::FixedDist;
                            self.distbits = 5;
                            self.mode = Mode::Len_;
                            if flush == Z_TREES {
                                dropbits!(2);
                                break 'leave;
                            }
                        }
                        2 => self.mode = Mode::Table,
                        _ => {
                            self.msg = Some("invalid block type");
                            self.mode = Mode::Bad;
                        }
                    }
                    dropbits!(2);
                }
                Mode::Stored => {
                    bytebits!();
                    needbits!(32);
                    if (hold & 0xffff) != ((hold >> 16) ^ 0xffff) {
                        self.msg = Some("invalid stored block lengths");
                        self.mode = Mode::Bad;
                        continue;
                    }
                    self.length = (hold as u32) & 0xffff;
                    initbits!();
                    self.mode = Mode::Copy_;
                    if flush == Z_TREES {
                        break 'leave;
                    }
                }
                Mode::Copy_ => self.mode = Mode::Copy,
                Mode::Copy => {
                    let mut copy = self.length;
                    if copy != 0 {
                        copy = copy.min(have).min(left);
                        if copy == 0 {
                            break 'leave;
                        }
                        let c = copy as usize;
                        output[put..put + c].copy_from_slice(&input[next..next + c]);
                        have -= copy;
                        next += c;
                        left -= copy;
                        put += c;
                        self.length -= copy;
                        continue;
                    }
                    self.mode = Mode::Type;
                }
                Mode::Table => {
                    needbits!(14);
                    self.nlen = getbits!(5) + 257;
                    dropbits!(5);
                    self.ndist = getbits!(5) + 1;
                    dropbits!(5);
                    self.ncode = getbits!(4) + 4;
                    dropbits!(4);
                    if self.nlen > 286 || self.ndist > 30 {
                        self.msg = Some("too many length or distance symbols");
                        self.mode = Mode::Bad;
                        continue;
                    }
                    self.have = 0;
                    self.mode = Mode::LenLens;
                }
                Mode::LenLens => {
                    while self.have < self.ncode {
                        needbits!(3);
                        self.lens[ORDER[self.have as usize] as usize] = getbits!(3) as u16;
                        self.have += 1;
                        dropbits!(3);
                    }
                    while self.have < 19 {
                        self.lens[ORDER[self.have as usize] as usize] = 0;
                        self.have += 1;
                    }
                    self.next = 0;
                    self.lencode = Table::Codes(0);
                    self.lenbits = 7;
                    let mut nx = self.next;
                    let mut lb = self.lenbits;
                    let lens = self.lens;
                    let r = inflate_table(
                        CodeType::Codes,
                        &lens,
                        19,
                        &mut self.codes,
                        &mut nx,
                        &mut lb,
                        &mut self.work,
                    );
                    self.next = nx;
                    self.lenbits = lb;
                    if r != 0 {
                        self.msg = Some("invalid code lengths set");
                        self.mode = Mode::Bad;
                        continue;
                    }
                    self.have = 0;
                    self.mode = Mode::CodeLens;
                }
                Mode::CodeLens => {
                    while self.have < self.nlen + self.ndist {
                        let mut here;
                        loop {
                            here = self.entry(self.lencode, getbits!(self.lenbits) as usize);
                            if u32::from(here.bits) <= bits {
                                break;
                            }
                            pullbyte!();
                        }
                        if here.val < 16 {
                            dropbits!(here.bits);
                            self.lens[self.have as usize] = here.val;
                            self.have += 1;
                        } else {
                            let len;
                            let copy;
                            if here.val == 16 {
                                needbits!(u32::from(here.bits) + 2);
                                dropbits!(here.bits);
                                if self.have == 0 {
                                    self.msg = Some("invalid bit length repeat");
                                    self.mode = Mode::Bad;
                                    break;
                                }
                                len = self.lens[(self.have - 1) as usize];
                                copy = 3 + getbits!(2);
                                dropbits!(2);
                            } else if here.val == 17 {
                                needbits!(u32::from(here.bits) + 3);
                                dropbits!(here.bits);
                                len = 0;
                                copy = 3 + getbits!(3);
                                dropbits!(3);
                            } else {
                                needbits!(u32::from(here.bits) + 7);
                                dropbits!(here.bits);
                                len = 0;
                                copy = 11 + getbits!(7);
                                dropbits!(7);
                            }
                            if self.have + copy > self.nlen + self.ndist {
                                self.msg = Some("invalid bit length repeat");
                                self.mode = Mode::Bad;
                                break;
                            }
                            for _ in 0..copy {
                                self.lens[self.have as usize] = len;
                                self.have += 1;
                            }
                        }
                    }
                    if self.mode == Mode::Bad {
                        continue;
                    }
                    if self.lens[256] == 0 {
                        self.msg = Some("invalid code -- missing end-of-block");
                        self.mode = Mode::Bad;
                        continue;
                    }
                    self.next = 0;
                    self.lencode = Table::Codes(0);
                    self.lenbits = 9;
                    let lens = self.lens;
                    let mut nx = self.next;
                    let mut lb = self.lenbits;
                    let r = inflate_table(
                        CodeType::Lens,
                        &lens,
                        self.nlen as usize,
                        &mut self.codes,
                        &mut nx,
                        &mut lb,
                        &mut self.work,
                    );
                    self.next = nx;
                    self.lenbits = lb;
                    if r != 0 {
                        self.msg = Some("invalid literal/lengths set");
                        self.mode = Mode::Bad;
                        continue;
                    }
                    self.distcode = Table::Codes(self.next);
                    self.distbits = 6;
                    let mut nx = self.next;
                    let mut db = self.distbits;
                    let r = inflate_table(
                        CodeType::Dists,
                        &lens[self.nlen as usize..],
                        self.ndist as usize,
                        &mut self.codes,
                        &mut nx,
                        &mut db,
                        &mut self.work,
                    );
                    self.next = nx;
                    self.distbits = db;
                    if r != 0 {
                        self.msg = Some("invalid distances set");
                        self.mode = Mode::Bad;
                        continue;
                    }
                    self.mode = Mode::Len_;
                    if flush == Z_TREES {
                        break 'leave;
                    }
                }
                Mode::Len_ => self.mode = Mode::Len,
                Mode::Len => {
                    if have >= 6 && left >= 258 {
                        // RESTORE(); inflate_fast(strm, out); LOAD();
                        self.hold = hold;
                        self.bits = bits;
                        let (n, p) = self.inflate_fast(input, next, have, output, put, left, out);
                        have -= (n - next) as u32;
                        next = n;
                        left -= (p - put) as u32;
                        put = p;
                        hold = self.hold;
                        bits = self.bits;
                        if self.mode == Mode::Type {
                            self.back = -1;
                        }
                        continue;
                    }
                    self.back = 0;
                    let mut here;
                    loop {
                        here = self.entry(self.lencode, getbits!(self.lenbits) as usize);
                        if u32::from(here.bits) <= bits {
                            break;
                        }
                        pullbyte!();
                    }
                    if here.op != 0 && (here.op & 0xf0) == 0 {
                        let last = here;
                        loop {
                            let i = u32::from(last.val)
                                + (getbits!(u32::from(last.bits) + u32::from(last.op))
                                    >> last.bits);
                            here = self.entry(self.lencode, i as usize);
                            if u32::from(last.bits) + u32::from(here.bits) <= bits {
                                break;
                            }
                            pullbyte!();
                        }
                        dropbits!(last.bits);
                        self.back += i32::from(last.bits);
                    }
                    dropbits!(here.bits);
                    self.back += i32::from(here.bits);
                    self.length = u32::from(here.val);
                    if here.op == 0 {
                        self.mode = Mode::Lit;
                        continue;
                    }
                    if here.op & 32 != 0 {
                        self.back = -1;
                        self.mode = Mode::Type;
                        continue;
                    }
                    if here.op & 64 != 0 {
                        self.msg = Some("invalid literal/length code");
                        self.mode = Mode::Bad;
                        continue;
                    }
                    self.extra = u32::from(here.op) & 15;
                    self.mode = Mode::LenExt;
                }
                Mode::LenExt => {
                    if self.extra != 0 {
                        needbits!(self.extra);
                        self.length += getbits!(self.extra);
                        dropbits!(self.extra);
                        self.back += self.extra as i32;
                    }
                    self.was = self.length;
                    self.mode = Mode::Dist;
                }
                Mode::Dist => {
                    let mut here;
                    loop {
                        here = self.entry(self.distcode, getbits!(self.distbits) as usize);
                        if u32::from(here.bits) <= bits {
                            break;
                        }
                        pullbyte!();
                    }
                    if (here.op & 0xf0) == 0 {
                        let last = here;
                        loop {
                            let i = u32::from(last.val)
                                + (getbits!(u32::from(last.bits) + u32::from(last.op))
                                    >> last.bits);
                            here = self.entry(self.distcode, i as usize);
                            if u32::from(last.bits) + u32::from(here.bits) <= bits {
                                break;
                            }
                            pullbyte!();
                        }
                        dropbits!(last.bits);
                        self.back += i32::from(last.bits);
                    }
                    dropbits!(here.bits);
                    self.back += i32::from(here.bits);
                    if here.op & 64 != 0 {
                        self.msg = Some("invalid distance code");
                        self.mode = Mode::Bad;
                        continue;
                    }
                    self.offset = u32::from(here.val);
                    self.extra = u32::from(here.op) & 15;
                    self.mode = Mode::DistExt;
                }
                Mode::DistExt => {
                    if self.extra != 0 {
                        needbits!(self.extra);
                        self.offset += getbits!(self.extra);
                        dropbits!(self.extra);
                        self.back += self.extra as i32;
                    }
                    self.mode = Mode::Match;
                }
                Mode::Match => {
                    if left == 0 {
                        break 'leave;
                    }
                    let mut copy = out - left;
                    if self.offset > copy {
                        copy = self.offset - copy;
                        if copy > self.whave && self.sane {
                            self.msg = Some("invalid distance too far back");
                            self.mode = Mode::Bad;
                            continue;
                        }
                        let from = if copy > self.wnext {
                            copy -= self.wnext;
                            (self.wsize - copy) as usize
                        } else {
                            (self.wnext - copy) as usize
                        };
                        if copy > self.length {
                            copy = self.length;
                        }
                        if copy > left {
                            copy = left;
                        }
                        left -= copy;
                        self.length -= copy;
                        for k in 0..copy as usize {
                            output[put] = self.window[from + k];
                            put += 1;
                        }
                    } else {
                        let from = put - self.offset as usize;
                        copy = self.length;
                        if copy > left {
                            copy = left;
                        }
                        left -= copy;
                        self.length -= copy;
                        // Byte by byte, in order: the source may overlap what is being written.
                        for k in 0..copy as usize {
                            output[put + k] = output[from + k];
                        }
                        put += copy as usize;
                    }
                    if self.length == 0 {
                        self.mode = Mode::Len;
                    }
                }
                Mode::Lit => {
                    if left == 0 {
                        break 'leave;
                    }
                    output[put] = self.length as u8;
                    put += 1;
                    left -= 1;
                    self.mode = Mode::Len;
                }
                Mode::Check => {
                    if self.wrap != 0 {
                        needbits!(32);
                        out -= left;
                        self.total_out += u64::from(out);
                        self.total += u64::from(out);
                        if self.wrap & 4 != 0 && out != 0 {
                            self.check =
                                adler32(self.check, Some(&output[put - out as usize..put]));
                            self.adler = self.check;
                        }
                        out = left;
                        if self.wrap & 4 != 0 && u64::from((hold as u32).swap_bytes()) != self.check
                        {
                            self.msg = Some("incorrect data check");
                            self.mode = Mode::Bad;
                            continue;
                        }
                        initbits!();
                    }
                    self.mode = Mode::Length;
                }
                Mode::Length => {
                    // gzip's length check (state->flags is 0 for these formats).
                    self.mode = Mode::Done;
                }
                Mode::Done => {
                    ret = Z_STREAM_END;
                    break 'leave;
                }
                Mode::Bad => {
                    ret = Z_DATA_ERROR;
                    break 'leave;
                }
                Mode::Mem => return (Z_MEM_ERROR, next, put),
                Mode::Sync => return (Z_STREAM_ERROR, next, put),
            }
        }
        // inf_leave: RESTORE()
        self.hold = hold;
        self.bits = bits;
        let avail_out_now = left;
        let avail_in_now = have;
        if self.wsize != 0
            || (out != avail_out_now
                && self.mode < Mode::Bad
                && (self.mode < Mode::Check || flush != Z_FINISH))
        {
            self.updatewindow(output, put, out - avail_out_now);
        }
        let inn = in0 - avail_in_now;
        let outn = out - avail_out_now;
        self.total_in += u64::from(inn);
        self.total_out += u64::from(outn);
        self.total += u64::from(outn);
        if self.wrap & 4 != 0 && outn != 0 {
            self.check = adler32(self.check, Some(&output[put - outn as usize..put]));
            self.adler = self.check;
        }
        self.data_type = self.bits as i32
            + if self.last { 64 } else { 0 }
            + if self.mode == Mode::Type { 128 } else { 0 }
            + if self.mode == Mode::Len_ || self.mode == Mode::Copy_ {
                256
            } else {
                0
            };
        if ((inn == 0 && outn == 0) || flush == Z_FINISH) && ret == Z_OK {
            ret = Z_BUF_ERROR;
        }
        (ret, next, put)
    }

    /// `inflate_fast(strm, start)` from `input[next..next + have]` into
    /// `output[put..put + left]` (`start`: `avail_out` at the start of the `inflate` call):
    /// the new `next` and `put`.
    #[allow(clippy::too_many_arguments)]
    fn inflate_fast(
        &mut self,
        input: &[u8],
        next0: usize,
        have: u32,
        output: &mut [u8],
        put0: usize,
        left: u32,
        start: u32,
    ) -> (usize, usize) {
        let mut inp = next0;
        let last = next0 + (have - 5) as usize;
        let mut out = put0;
        let beg = put0 - (start - left) as usize;
        let end = put0 + (left - 257) as usize;
        let wsize = self.wsize;
        let whave = self.whave;
        let wnext = self.wnext;
        let mut hold = self.hold;
        let mut bits = self.bits;
        let lcode = self.lencode;
        let dcode = self.distcode;
        let lmask = (1u64 << self.lenbits) - 1;
        let dmask = (1u64 << self.distbits) - 1;
        'outer: loop {
            if bits < 15 {
                hold = hold.wrapping_add(u64::from(input[inp]) << bits);
                inp += 1;
                bits += 8;
                hold = hold.wrapping_add(u64::from(input[inp]) << bits);
                inp += 1;
                bits += 8;
            }
            let mut here = self.entry(lcode, (hold & lmask) as usize);
            // dolen:
            loop {
                let op = u32::from(here.bits);
                hold >>= op;
                bits -= op;
                let op = u32::from(here.op);
                if op == 0 {
                    output[out] = here.val as u8;
                    out += 1;
                    break;
                } else if op & 16 != 0 {
                    let mut len = u32::from(here.val);
                    let op = op & 15;
                    if op != 0 {
                        if bits < op {
                            hold = hold.wrapping_add(u64::from(input[inp]) << bits);
                            inp += 1;
                            bits += 8;
                        }
                        len += (hold as u32) & ((1u32 << op) - 1);
                        hold >>= op;
                        bits -= op;
                    }
                    if bits < 15 {
                        hold = hold.wrapping_add(u64::from(input[inp]) << bits);
                        inp += 1;
                        bits += 8;
                        hold = hold.wrapping_add(u64::from(input[inp]) << bits);
                        inp += 1;
                        bits += 8;
                    }
                    let mut here = self.entry(dcode, (hold & dmask) as usize);
                    // dodist:
                    loop {
                        let op = u32::from(here.bits);
                        hold >>= op;
                        bits -= op;
                        let op = u32::from(here.op);
                        if op & 16 != 0 {
                            let mut dist = u32::from(here.val);
                            let op = op & 15;
                            if bits < op {
                                hold = hold.wrapping_add(u64::from(input[inp]) << bits);
                                inp += 1;
                                bits += 8;
                                if bits < op {
                                    hold = hold.wrapping_add(u64::from(input[inp]) << bits);
                                    inp += 1;
                                    bits += 8;
                                }
                            }
                            dist += (hold as u32) & ((1u32 << op) - 1);
                            hold >>= op;
                            bits -= op;
                            let mut op = (out - beg) as u32;
                            if dist > op {
                                op = dist - op;
                                if op > whave && self.sane {
                                    self.msg = Some("invalid distance too far back");
                                    self.mode = Mode::Bad;
                                    break 'outer;
                                }
                                let mut from: usize;
                                let mut from_window = true;
                                if wnext == 0 {
                                    from = (wsize - op) as usize;
                                    if op < len {
                                        len -= op;
                                        for _ in 0..op {
                                            output[out] = self.window[from];
                                            out += 1;
                                            from += 1;
                                        }
                                        from = out - dist as usize;
                                        from_window = false;
                                    }
                                } else if wnext < op {
                                    from = (wsize + wnext - op) as usize;
                                    op -= wnext;
                                    if op < len {
                                        len -= op;
                                        for _ in 0..op {
                                            output[out] = self.window[from];
                                            out += 1;
                                            from += 1;
                                        }
                                        from = 0;
                                        if wnext < len {
                                            op = wnext;
                                            len -= op;
                                            for _ in 0..op {
                                                output[out] = self.window[from];
                                                out += 1;
                                                from += 1;
                                            }
                                            from = out - dist as usize;
                                            from_window = false;
                                        }
                                    }
                                } else {
                                    from = (wnext - op) as usize;
                                    if op < len {
                                        len -= op;
                                        for _ in 0..op {
                                            output[out] = self.window[from];
                                            out += 1;
                                            from += 1;
                                        }
                                        from = out - dist as usize;
                                        from_window = false;
                                    }
                                }
                                for _ in 0..len {
                                    output[out] = if from_window {
                                        self.window[from]
                                    } else {
                                        output[from]
                                    };
                                    out += 1;
                                    from += 1;
                                }
                            } else {
                                let from = out - dist as usize;
                                // Byte by byte, in order (the copy overlaps when dist < len).
                                for k in 0..len as usize {
                                    output[out + k] = output[from + k];
                                }
                                out += len as usize;
                            }
                            break;
                        } else if op & 64 == 0 {
                            here = self.entry(
                                dcode,
                                (u32::from(here.val) + ((hold as u32) & ((1u32 << op) - 1)))
                                    as usize,
                            );
                            continue;
                        } else {
                            self.msg = Some("invalid distance code");
                            self.mode = Mode::Bad;
                            break 'outer;
                        }
                    }
                    break;
                } else if op & 64 == 0 {
                    here = self.entry(
                        lcode,
                        (u32::from(here.val) + ((hold as u32) & ((1u32 << op) - 1))) as usize,
                    );
                    continue;
                } else if op & 32 != 0 {
                    self.mode = Mode::Type;
                    break 'outer;
                } else {
                    self.msg = Some("invalid literal/length code");
                    self.mode = Mode::Bad;
                    break 'outer;
                }
            }
            if !(inp < last && out < end) {
                break;
            }
        }
        let len = bits >> 3;
        inp -= len as usize;
        bits -= len << 3;
        hold &= (1u64 << bits) - 1;
        self.hold = hold;
        self.bits = bits;
        (inp, out)
    }

    /// `inflateSync(strm)` on `input` (`next_in`/`avail_in`): the status and the input bytes
    /// it skipped.
    pub fn sync(&mut self, input: &[u8]) -> (i32, usize) {
        if input.is_empty() && self.bits < 8 {
            return (Z_BUF_ERROR, 0);
        }
        if self.mode != Mode::Sync {
            self.mode = Mode::Sync;
            self.hold = self.hold.wrapping_shl(self.bits & 7);
            self.bits -= self.bits & 7;
            let mut buf = [0u8; 4];
            let mut len = 0usize;
            while self.bits >= 8 {
                buf[len] = self.hold as u8;
                len += 1;
                self.hold >>= 8;
                self.bits -= 8;
            }
            self.have = 0;
            let mut h = self.have;
            syncsearch(&mut h, &buf[..len]);
            self.have = h;
        }
        let avail = input.len().min(u32::MAX as usize);
        let mut h = self.have;
        let len = syncsearch(&mut h, &input[..avail]);
        self.have = h;
        self.total_in += len as u64;
        if self.have != 4 {
            return (Z_DATA_ERROR, len);
        }
        let (tin, tout) = (self.total_in, self.total_out);
        self.reset();
        self.total_in = tin;
        self.total_out = tout;
        self.mode = Mode::Type;
        (Z_OK, len)
    }
}

/// `syncsearch(&have, buf, len)`.
fn syncsearch(have: &mut u32, buf: &[u8]) -> usize {
    let mut got = *have;
    let mut next = 0usize;
    while next < buf.len() && got < 4 {
        if i32::from(buf[next]) == if got < 2 { 0 } else { 0xff } {
            got += 1;
        } else if buf[next] != 0 {
            got = 0;
        } else {
            got = 4 - got;
        }
        next += 1;
    }
    *have = got;
    next
}
