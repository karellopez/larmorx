// SPDX-License-Identifier: Apache-2.0
//! The zlib probe (README.md): `larmorx_core::zlib` and `larmorx_io::znz`'s gzip writer against
//! zlib 1.2.11 as FreeSurfer 8.2.0's official programs load it (Ubuntu 22.04's `libz.so.1`) and
//! as released (`libz-up.so`, built by build.sh from the pinned tarball), in one process.
//!
//! Usage:
//!   zlib-deflate cases <libdir> <seed> <n> <threads>   deflate streams, every call compared
//!   zlib-deflate one <libdir> <seed> <case>            one case's transcripts
//!   zlib-deflate sums <libdir> <seed> <n>              adler32, crc32, compress2, compressBound
//!   zlib-deflate gz <libdir> <seed> <n> <scratch>      gzopen/gzwrite/.../gzclose files
//!   zlib-deflate files <libdir> <list>                 gzip files re-made (one path per line)

use larmorx_core::zlib::{self as z, Deflate, GzHeader};
use std::ffi::{CStr, CString, c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

/// zlib 1.2.11's `z_stream` on LP64.
#[repr(C)]
struct ZStream {
    next_in: *const u8,
    avail_in: c_uint,
    total_in: c_ulong,
    next_out: *mut u8,
    avail_out: c_uint,
    total_out: c_ulong,
    msg: *const c_char,
    state: *mut c_void,
    zalloc: *mut c_void,
    zfree: *mut c_void,
    opaque: *mut c_void,
    data_type: c_int,
    adler: c_ulong,
    reserved: c_ulong,
}

/// zlib's `gz_header`.
#[repr(C)]
struct GzHeaderC {
    text: c_int,
    time: c_ulong,
    xflags: c_int,
    os: c_int,
    extra: *mut u8,
    extra_len: c_uint,
    extra_max: c_uint,
    name: *mut u8,
    name_max: c_uint,
    comment: *mut u8,
    comm_max: c_uint,
    hcrc: c_int,
    done: c_int,
}

type P = *mut ZStream;
#[allow(clippy::type_complexity)]
struct Lib {
    init2: unsafe extern "C" fn(P, c_int, c_int, c_int, c_int, c_int, *const c_char, c_int) -> c_int,
    deflate: unsafe extern "C" fn(P, c_int) -> c_int,
    end: unsafe extern "C" fn(P) -> c_int,
    reset: unsafe extern "C" fn(P) -> c_int,
    reset_keep: unsafe extern "C" fn(P) -> c_int,
    params: unsafe extern "C" fn(P, c_int, c_int) -> c_int,
    set_dict: unsafe extern "C" fn(P, *const u8, c_uint) -> c_int,
    get_dict: unsafe extern "C" fn(P, *mut u8, *mut c_uint) -> c_int,
    set_header: unsafe extern "C" fn(P, *mut GzHeaderC) -> c_int,
    pending: unsafe extern "C" fn(P, *mut c_uint, *mut c_int) -> c_int,
    prime: unsafe extern "C" fn(P, c_int, c_int) -> c_int,
    tune: unsafe extern "C" fn(P, c_int, c_int, c_int, c_int) -> c_int,
    bound: unsafe extern "C" fn(P, c_ulong) -> c_ulong,
    copy: unsafe extern "C" fn(P, P) -> c_int,
    compress2: unsafe extern "C" fn(*mut u8, *mut c_ulong, *const u8, c_ulong, c_int) -> c_int,
    compress_bound: unsafe extern "C" fn(c_ulong) -> c_ulong,
    adler32: unsafe extern "C" fn(c_ulong, *const u8, c_uint) -> c_ulong,
    crc32: unsafe extern "C" fn(c_ulong, *const u8, c_uint) -> c_ulong,
    inflate_init2: unsafe extern "C" fn(P, c_int, *const c_char, c_int) -> c_int,
    inflate: unsafe extern "C" fn(P, c_int) -> c_int,
    inflate_end: unsafe extern "C" fn(P) -> c_int,
    gzopen: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void,
    gzwrite: unsafe extern "C" fn(*mut c_void, *const u8, c_uint) -> c_int,
    gzputc: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
    gzputs: unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int,
    gzflush: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
    gzseek: unsafe extern "C" fn(*mut c_void, c_long, c_int) -> c_long,
    gztell: unsafe extern "C" fn(*mut c_void) -> c_long,
    gzclose: unsafe extern "C" fn(*mut c_void) -> c_int,
}

fn load(path: &Path) -> Lib {
    let c = CString::new(path.to_string_lossy().as_bytes()).unwrap();
    // SAFETY: the dynamic loader with a NUL-terminated name (RTLD_NOW | RTLD_LOCAL).
    let h = unsafe { dlopen(c.as_ptr(), 2) };
    assert!(!h.is_null(), "dlopen({})", path.display());
    let sym = |s: &str| {
        let c = CString::new(s).unwrap();
        // SAFETY: a symbol lookup in the library opened above.
        let p = unsafe { dlsym(h, c.as_ptr()) };
        assert!(!p.is_null(), "{s}");
        p
    };
    macro_rules! f {
        ($n:expr) => {
            // SAFETY: zlib 1.2.11's exported functions have the signatures declared in Lib.
            unsafe { std::mem::transmute::<*mut c_void, _>(sym($n)) }
        };
    }
    Lib {
        init2: f!("deflateInit2_"),
        deflate: f!("deflate"),
        end: f!("deflateEnd"),
        reset: f!("deflateReset"),
        reset_keep: f!("deflateResetKeep"),
        params: f!("deflateParams"),
        set_dict: f!("deflateSetDictionary"),
        get_dict: f!("deflateGetDictionary"),
        set_header: f!("deflateSetHeader"),
        pending: f!("deflatePending"),
        prime: f!("deflatePrime"),
        tune: f!("deflateTune"),
        bound: f!("deflateBound"),
        copy: f!("deflateCopy"),
        compress2: f!("compress2"),
        compress_bound: f!("compressBound"),
        adler32: f!("adler32"),
        crc32: f!("crc32"),
        inflate_init2: f!("inflateInit2_"),
        inflate: f!("inflate"),
        inflate_end: f!("inflateEnd"),
        gzopen: f!("gzopen"),
        gzwrite: f!("gzwrite"),
        gzputc: f!("gzputc"),
        gzputs: f!("gzputs"),
        gzflush: f!("gzflush"),
        gzseek: f!("gzseek"),
        gztell: f!("gztell"),
        gzclose: f!("gzclose"),
    }
}

/// xorshift64* (the probe's own generator).
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0xd1b5_4a32_d192_ed03 | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    fn range(&mut self, a: i64, b: i64) -> i64 {
        a + self.below((b - a + 1) as u64) as i64
    }
    fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}

/// Generated data of one of ten kinds.
fn gen_data(r: &mut Rng, len: usize) -> Vec<u8> {
    let kind = r.below(11);
    let mut v = Vec::with_capacity(len);
    match kind {
        0 => v.resize(len, 0),
        1 => (0..len).for_each(|_| v.push(r.next() as u8)),
        2 => {
            let k = r.range(2, 16) as u64;
            (0..len).for_each(|_| v.push(r.below(k) as u8));
        }
        3 => {
            while v.len() < len {
                let run = r.range(1, 600) as usize;
                let b = r.next() as u8;
                for _ in 0..run.min(len - v.len()) {
                    v.push(b);
                }
            }
        }
        4 => {
            let period = r.range(1, 70_000) as usize;
            let base: Vec<u8> = (0..period).map(|_| r.below(40) as u8).collect();
            let rate = r.range(1, 200) as u64;
            while v.len() < len {
                let i = v.len() % period;
                v.push(if r.chance(rate) { r.next() as u8 } else { base[i] });
            }
        }
        5 => {
            let words: Vec<Vec<u8>> = (0..r.range(5, 300))
                .map(|_| (0..r.range(1, 12)).map(|_| b'a' + r.below(26) as u8).collect())
                .collect();
            while v.len() < len {
                let w = &words[r.below(words.len() as u64) as usize];
                v.extend_from_slice(w);
                v.push(if r.chance(12) { b'\n' } else { b' ' });
            }
            v.truncate(len);
        }
        6 => {
            // a smooth "image" of little-endian float32 or int16 voxels (integer arithmetic)
            let f32s = r.chance(2);
            let nx = r.range(4, 200);
            let noise = r.range(0, 50);
            let mut i = 0i64;
            while v.len() < len {
                let (x, y) = (i % nx, (i / nx) % nx);
                let val = (x - nx / 2) * (x - nx / 2) + (y - nx / 3) * (y - nx / 3) + r.range(0, noise);
                if f32s {
                    v.extend_from_slice(&(val as f32 * 0.25).to_le_bytes());
                } else {
                    v.extend_from_slice(&((val % 30000) as i16).to_le_bytes());
                }
                i += 1;
            }
            v.truncate(len);
        }
        7 => {
            while v.len() < len {
                let n = r.range(1, 5000) as usize;
                let n = n.min(len - v.len());
                let part = gen_data(r, n);
                v.extend_from_slice(&part);
            }
        }
        8 => {
            let rate = r.range(2, 400) as u64;
            (0..len).for_each(|_| v.push(if r.chance(rate) { r.next() as u8 } else { 0 }));
        }
        9 => {
            let gray = [7u8, 8, 11, 12, 26, 27];
            (0..len).for_each(|_| v.push(gray[r.below(6) as usize]));
        }
        _ => {
            // far repeats: blocks copied from 3 bytes to 40,000 bytes back
            while v.len() < len {
                if v.len() > 300 && !r.chance(4) {
                    let back = r.range(3, (v.len() as i64).min(40_000)) as usize;
                    let n = r.range(3, 300) as usize;
                    for _ in 0..n.min(len - v.len()) {
                        let b = v[v.len() - back];
                        v.push(b);
                    }
                } else {
                    v.push(r.next() as u8);
                }
            }
        }
    }
    v
}

fn gen_len(r: &mut Rng) -> usize {
    match r.below(20) {
        0 => 0,
        1..=3 => r.range(1, 64) as usize,
        4..=9 => r.range(64, 5000) as usize,
        10..=15 => r.range(5000, 70_000) as usize,
        16..=18 => r.range(70_000, 400_000) as usize,
        _ => r.range(400_000, 2_000_000) as usize,
    }
}

#[derive(Clone, Debug)]
enum Op {
    Deflate { add: usize, space: usize, flush: i32 },
    Params { level: i32, strategy: i32, space: usize },
    Prime { bits: i32, value: i32 },
    Reset,
    ResetKeep,
    Tune { good: i32, lazy: i32, nice: i32, chain: i32 },
    Bound { len: u64 },
    Pending,
    GetDict,
    Copy,
}

#[derive(Clone, Debug)]
struct Case {
    level: i32,
    method: i32,
    wbits: i32,
    mem: i32,
    strategy: i32,
    data: Vec<u8>,
    dict: Option<Vec<u8>>,
    header: Option<GzHeader>,
    ops: Vec<Op>,
}

fn gen_case(seed: u64, i: u64) -> Case {
    let mut r = Rng::new(seed.wrapping_mul(1_000_003).wrapping_add(i));
    let level = if r.chance(40) { r.range(-3, 11) as i32 } else { r.range(-1, 9) as i32 };
    let method = if r.chance(60) { 7 } else { 8 };
    let wb = r.range(8, 15) as i32;
    let wbits = match r.below(10) {
        0..=4 => wb,
        5..=6 => -wb,
        7..=8 => wb + 16,
        _ => *[7, 16, 32, -7, 0, -16].get(r.below(6) as usize).unwrap(),
    };
    let mem = if r.chance(40) { r.range(0, 10) as i32 } else { r.range(1, 9) as i32 };
    let strategy = if r.chance(40) { r.range(-1, 5) as i32 } else { r.range(0, 4) as i32 };
    let len = gen_len(&mut r);
    let data = gen_data(&mut r, len);
    let dict = if r.chance(6) {
        let n = r.range(0, 40_000) as usize;
        Some(gen_data(&mut r, n))
    } else {
        None
    };
    let header = if r.chance(5) {
        let field = |r: &mut Rng, max: i64| -> Option<Vec<u8>> {
            if r.chance(2) {
                let n = r.range(0, max) as usize;
                Some((0..n).map(|_| r.range(1, 255) as u8).collect())
            } else {
                None
            }
        };
        let extra = field(&mut r, 2000);
        let extra_len = extra.as_ref().map_or(0, |e| e.len() as u32);
        Some(GzHeader {
            text: r.below(2) as i32,
            time: r.next() & 0xffff_ffff,
            os: r.range(0, 300) as i32,
            extra,
            extra_len,
            name: field(&mut r, 1500),
            comment: field(&mut r, 1500),
            hcrc: r.below(2) as i32,
        })
    } else {
        None
    };
    let nops = r.below(30) as usize;
    let mut ops = Vec::new();
    for _ in 0..nops {
        let space = |r: &mut Rng| -> usize {
            match r.below(10) {
                0 => 0,
                1 => r.range(1, 16) as usize,
                2..=3 => r.range(1, 400) as usize,
                4..=6 => r.range(1, 20_000) as usize,
                7 => 8192,
                8 => 16384,
                _ => r.range(20_000, 400_000) as usize,
            }
        };
        let op = match r.below(100) {
            0..=69 => Op::Deflate {
                add: match r.below(6) {
                    0 => 0,
                    1 => r.range(1, 8) as usize,
                    2 => r.range(1, 400) as usize,
                    3 => r.range(1, 30_000) as usize,
                    4 => r.range(1, 300_000) as usize,
                    _ => usize::MAX,
                },
                space: space(&mut r),
                flush: match r.below(20) {
                    0..=10 => z::Z_NO_FLUSH,
                    11 => z::Z_PARTIAL_FLUSH,
                    12..=13 => z::Z_SYNC_FLUSH,
                    14 => z::Z_FULL_FLUSH,
                    15 => z::Z_FINISH,
                    16..=17 => z::Z_BLOCK,
                    18 => z::Z_TREES,
                    _ => -1,
                },
            },
            70..=77 => Op::Params {
                level: r.range(-2, 10) as i32,
                strategy: r.range(-1, 5) as i32,
                space: space(&mut r),
            },
            78..=81 => Op::Prime { bits: r.range(-1, 17) as i32, value: r.next() as i32 },
            82..=83 => Op::Reset,
            84 => Op::ResetKeep,
            85..=87 => Op::Tune {
                good: r.range(0, 300) as i32,
                lazy: r.range(3, 300) as i32,
                nice: r.range(3, 258) as i32,
                chain: r.range(4, 5000) as i32,
            },
            88..=90 => Op::Bound { len: r.next() >> r.below(64) },
            91..=93 => Op::Pending,
            94..=96 => Op::GetDict,
            _ => Op::Copy,
        };
        ops.push(op);
    }
    Case { level, method, wbits, mem, strategy, data, dict, header, ops }
}

/// One implementation's run of a case: a transcript of every call and the output bytes.
struct Run {
    lines: Vec<String>,
    out: Vec<u8>,
}

/// The stream interface both implementations are driven through.
trait Stream {
    fn deflate(&mut self, inp: &[u8], out: &mut [u8], flush: i32) -> (i32, usize, usize);
    fn params(&mut self, l: i32, s: i32, inp: &[u8], out: &mut [u8]) -> (i32, usize, usize);
    fn prime(&mut self, bits: i32, value: i32) -> i32;
    fn reset(&mut self, keep: bool) -> i32;
    fn tune(&mut self, a: i32, b: i32, c: i32, d: i32) -> i32;
    fn bound(&mut self, len: u64) -> u64;
    fn pending(&mut self) -> (u32, i32);
    fn get_dict(&mut self) -> Vec<u8>;
    fn copy(&mut self) -> i32;
    fn set_dict(&mut self, d: &[u8]) -> i32;
    fn set_header(&mut self, h: &GzHeader) -> i32;
    fn state(&self) -> String;
    fn end(self: Box<Self>) -> i32;
}

struct Port(Deflate);

impl Stream for Port {
    fn deflate(&mut self, inp: &[u8], out: &mut [u8], flush: i32) -> (i32, usize, usize) {
        self.0.deflate(inp, out, flush)
    }
    fn params(&mut self, l: i32, s: i32, inp: &[u8], out: &mut [u8]) -> (i32, usize, usize) {
        self.0.params(l, s, inp, out)
    }
    fn prime(&mut self, bits: i32, value: i32) -> i32 {
        self.0.prime(bits, value)
    }
    fn reset(&mut self, keep: bool) -> i32 {
        if keep { self.0.reset_keep() } else { self.0.reset() }
    }
    fn tune(&mut self, a: i32, b: i32, c: i32, d: i32) -> i32 {
        self.0.tune(a, b, c, d)
    }
    fn bound(&mut self, len: u64) -> u64 {
        self.0.bound(len)
    }
    fn pending(&mut self) -> (u32, i32) {
        self.0.pending()
    }
    fn get_dict(&mut self) -> Vec<u8> {
        self.0.get_dictionary()
    }
    fn copy(&mut self) -> i32 {
        let c = self.0.clone();
        self.0 = c;
        z::Z_OK
    }
    fn set_dict(&mut self, d: &[u8]) -> i32 {
        self.0.set_dictionary(d)
    }
    fn set_header(&mut self, h: &GzHeader) -> i32 {
        self.0.set_header(h.clone())
    }
    fn state(&self) -> String {
        let d = &self.0;
        format!(
            "ti {} to {} adler {} dt {} msg {:?}",
            d.total_in,
            d.total_out,
            d.adler,
            d.data_type,
            d.msg.unwrap_or("")
        )
    }
    fn end(self: Box<Self>) -> i32 {
        self.0.end()
    }
}

struct CStream<'l> {
    lib: &'l Lib,
    z: Box<ZStream>,
    /// The header and its fields stay alive while the stream may read them.
    hdr: Vec<(Box<GzHeaderC>, Vec<Vec<u8>>)>,
}

fn zeroed() -> Box<ZStream> {
    Box::new(ZStream {
        next_in: std::ptr::null(),
        avail_in: 0,
        total_in: 0,
        next_out: std::ptr::null_mut(),
        avail_out: 0,
        total_out: 0,
        msg: std::ptr::null(),
        state: std::ptr::null_mut(),
        zalloc: std::ptr::null_mut(),
        zfree: std::ptr::null_mut(),
        opaque: std::ptr::null_mut(),
        data_type: 0,
        adler: 0,
        reserved: 0,
    })
}

static VERSION: &[u8] = b"1.2.11\0";

impl CStream<'_> {
    fn call(&mut self, inp: &[u8], out: &mut [u8], f: impl FnOnce(&Lib, P) -> c_int) -> (i32, usize, usize) {
        let mut dummy = [0u8; 1];
        self.z.next_in = inp.as_ptr();
        self.z.avail_in = inp.len() as c_uint;
        self.z.next_out = if out.is_empty() { dummy.as_mut_ptr() } else { out.as_mut_ptr() };
        self.z.avail_out = out.len() as c_uint;
        let r = f(self.lib, &mut *self.z);
        let used = inp.len() - self.z.avail_in as usize;
        let made = out.len() - self.z.avail_out as usize;
        (r, used, made)
    }
}

impl Stream for CStream<'_> {
    fn deflate(&mut self, inp: &[u8], out: &mut [u8], flush: i32) -> (i32, usize, usize) {
        // SAFETY: a stream initialised by deflateInit2_ with valid buffers.
        self.call(inp, out, |l, p| unsafe { (l.deflate)(p, flush) })
    }
    fn params(&mut self, lv: i32, s: i32, inp: &[u8], out: &mut [u8]) -> (i32, usize, usize) {
        // SAFETY: as above.
        self.call(inp, out, |l, p| unsafe { (l.params)(p, lv, s) })
    }
    fn prime(&mut self, bits: i32, value: i32) -> i32 {
        // SAFETY: as above.
        unsafe { (self.lib.prime)(&mut *self.z, bits, value) }
    }
    fn reset(&mut self, keep: bool) -> i32 {
        // SAFETY: as above.
        unsafe {
            if keep { (self.lib.reset_keep)(&mut *self.z) } else { (self.lib.reset)(&mut *self.z) }
        }
    }
    fn tune(&mut self, a: i32, b: i32, c: i32, d: i32) -> i32 {
        // SAFETY: as above.
        unsafe { (self.lib.tune)(&mut *self.z, a, b, c, d) }
    }
    fn bound(&mut self, len: u64) -> u64 {
        // SAFETY: as above.
        unsafe { (self.lib.bound)(&mut *self.z, len) }
    }
    fn pending(&mut self) -> (u32, i32) {
        let (mut p, mut b) = (0, 0);
        // SAFETY: as above.
        unsafe { (self.lib.pending)(&mut *self.z, &mut p, &mut b) };
        (p, b)
    }
    fn get_dict(&mut self) -> Vec<u8> {
        let mut buf = vec![0u8; 1 << 16];
        let mut n: c_uint = 0;
        // SAFETY: a buffer of 64 KiB holds any window's dictionary (at most 32 KiB).
        unsafe { (self.lib.get_dict)(&mut *self.z, buf.as_mut_ptr(), &mut n) };
        buf.truncate(n as usize);
        buf
    }
    fn copy(&mut self) -> i32 {
        let mut d = zeroed();
        // SAFETY: deflateCopy into a fresh z_stream, then the source is ended.
        let r = unsafe { (self.lib.copy)(&mut *d, &mut *self.z) };
        if r == z::Z_OK {
            // SAFETY: the source stream is valid; it is replaced by its copy.
            unsafe { (self.lib.end)(&mut *self.z) };
            self.z = d;
        }
        r
    }
    fn set_dict(&mut self, d: &[u8]) -> i32 {
        // SAFETY: a valid stream and dictionary.
        unsafe { (self.lib.set_dict)(&mut *self.z, d.as_ptr(), d.len() as c_uint) }
    }
    fn set_header(&mut self, h: &GzHeader) -> i32 {
        let mut fields = Vec::new();
        let mut zt = |v: &Option<Vec<u8>>, nul: bool| -> *mut u8 {
            match v {
                None => std::ptr::null_mut(),
                Some(b) => {
                    let mut b = b.clone();
                    if nul {
                        b.push(0);
                    }
                    b.push(0);
                    fields.push(b);
                    fields.last_mut().unwrap().as_mut_ptr()
                }
            }
        };
        let extra = zt(&h.extra, false);
        let name = zt(&h.name, true);
        let comment = zt(&h.comment, true);
        let mut hc = Box::new(GzHeaderC {
            text: h.text,
            time: h.time,
            xflags: 0,
            os: h.os,
            extra,
            extra_len: h.extra_len,
            extra_max: 0,
            name,
            name_max: 0,
            comment,
            comm_max: 0,
            hcrc: h.hcrc,
            done: 0,
        });
        // SAFETY: the header and its fields are kept alive in self.hdr.
        let r = unsafe { (self.lib.set_header)(&mut *self.z, &mut *hc) };
        self.hdr.push((hc, fields));
        r
    }
    fn state(&self) -> String {
        let msg = if self.z.msg.is_null() {
            String::new()
        } else {
            // SAFETY: zlib's messages are static NUL-terminated strings.
            unsafe { CStr::from_ptr(self.z.msg) }.to_string_lossy().into_owned()
        };
        format!(
            "ti {} to {} adler {} dt {} msg {:?}",
            self.z.total_in, self.z.total_out, self.z.adler, self.z.data_type, msg
        )
    }
    fn end(mut self: Box<Self>) -> i32 {
        // SAFETY: a valid stream.
        unsafe { (self.lib.end)(&mut *self.z) }
    }
}

fn open_c<'l>(lib: &'l Lib, c: &Case) -> Result<CStream<'l>, i32> {
    let mut zs = zeroed();
    // SAFETY: deflateInit2_ on a zeroed z_stream with the version and size it checks.
    let r = unsafe {
        (lib.init2)(
            &mut *zs,
            c.level,
            c.method,
            c.wbits,
            c.mem,
            c.strategy,
            VERSION.as_ptr() as *const c_char,
            std::mem::size_of::<ZStream>() as c_int,
        )
    };
    if r != z::Z_OK {
        return Err(r);
    }
    Ok(CStream { lib, z: zs, hdr: Vec::new() })
}

fn run_case(c: &Case, s: Result<Box<dyn Stream + '_>, i32>) -> Run {
    let mut lines = Vec::new();
    let mut out = Vec::new();
    let mut s = match s {
        Err(e) => {
            lines.push(format!("init {e}"));
            return Run { lines, out };
        }
        Ok(s) => s,
    };
    lines.push("init 0".into());
    if let Some(h) = &c.header {
        lines.push(format!("header {}", s.set_header(h)));
    }
    if let Some(d) = &c.dict {
        lines.push(format!("dict {} {}", s.set_dict(d), s.state()));
    }
    let (mut ip, mut avail) = (0usize, 0usize);
    let mut buf = Vec::new();
    let mut step = |s: &mut Box<dyn Stream + '_>,
                    out: &mut Vec<u8>,
                    ip: &mut usize,
                    avail: &mut usize,
                    space: usize,
                    params: Option<(i32, i32)>,
                    flush: i32|
     -> (i32, usize, usize) {
        buf.clear();
        buf.resize(space, 0xa5);
        let inp = &c.data[*ip..*ip + *avail];
        let (r, used, made) = match params {
            None => s.deflate(inp, &mut buf, flush),
            Some((l, st)) => s.params(l, st, inp, &mut buf),
        };
        out.extend_from_slice(&buf[..made]);
        *ip += used;
        *avail -= used;
        (r, used, made)
    };
    for op in &c.ops {
        match *op {
            Op::Deflate { add, space, flush } => {
                let rest = c.data.len() - ip - avail;
                avail += add.min(rest);
                let (r, u, m) = step(&mut s, &mut out, &mut ip, &mut avail, space, None, flush);
                lines.push(format!("d {r} {u} {m} {}", s.state()));
            }
            Op::Params { level, strategy, space } => {
                let (r, u, m) =
                    step(&mut s, &mut out, &mut ip, &mut avail, space, Some((level, strategy)), 0);
                lines.push(format!("p {r} {u} {m} {}", s.state()));
            }
            Op::Prime { bits, value } => lines.push(format!("P {}", s.prime(bits, value))),
            Op::Reset => lines.push(format!("r {} {}", s.reset(false), s.state())),
            Op::ResetKeep => lines.push(format!("k {} {}", s.reset(true), s.state())),
            Op::Tune { good, lazy, nice, chain } => {
                lines.push(format!("t {}", s.tune(good, lazy, nice, chain)))
            }
            Op::Bound { len } => lines.push(format!("b {}", s.bound(len))),
            Op::Pending => lines.push(format!("n {:?}", s.pending())),
            Op::GetDict => {
                let d = s.get_dict();
                lines.push(format!("g {} {:016x}", d.len(), fnv(&d)));
            }
            Op::Copy => lines.push(format!("c {}", s.copy())),
        }
    }
    // finish: all the input, output in pieces of 1 to 64 KiB, at most 100,000 calls
    let mut r = Rng::new(c.data.len() as u64 ^ 0x5151);
    for _ in 0..100_000 {
        avail = c.data.len() - ip;
        let space = r.range(1, 65536) as usize;
        let (ret, u, m) = step(&mut s, &mut out, &mut ip, &mut avail, space, None, z::Z_FINISH);
        if ret != z::Z_OK || (u == 0 && m == 0 && ret == z::Z_OK && avail == 0 && r.chance(1000)) {
            lines.push(format!("f {ret} {}", s.state()));
            if ret != z::Z_OK {
                break;
            }
        }
    }
    lines.push(format!("end {}", s.end()));
    Run { lines, out }
}

fn fnv(b: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in b {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn libs(dir: &str) -> (Lib, Lib) {
    let sys = load(Path::new("/lib/x86_64-linux-gnu/libz.so.1"));
    let up = load(&Path::new(dir).join("libz-up.so"));
    (sys, up)
}

fn compare(what: &str, a: &Run, b: &Run) -> Option<String> {
    if a.lines != b.lines {
        let i = a.lines.iter().zip(&b.lines).position(|(x, y)| x != y).unwrap_or(a.lines.len().min(b.lines.len()));
        return Some(format!(
            "{what}: line {i}: {:?} vs {:?}",
            a.lines.get(i),
            b.lines.get(i)
        ));
    }
    if a.out != b.out {
        let i = a.out.iter().zip(&b.out).position(|(x, y)| x != y).unwrap_or(a.out.len().min(b.out.len()));
        return Some(format!("{what}: output byte {i} of {} / {}", a.out.len(), b.out.len()));
    }
    None
}

fn cases(dir: &str, seed: u64, n: u64, threads: usize) {
    let (sys, up) = libs(dir);
    let next = AtomicUsize::new(0);
    let fails = AtomicUsize::new(0);
    let up_differs = AtomicUsize::new(0);
    let bytes = AtomicUsize::new(0);
    let digest = std::sync::Mutex::new(vec![0u64; n as usize]);
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed) as u64;
                if i >= n {
                    break;
                }
                let c = gen_case(seed, i);
                let a = run_case(&c, open_c(&sys, &c).map(|s| Box::new(s) as Box<dyn Stream>));
                let b = run_case(&c, open_c(&up, &c).map(|s| Box::new(s) as Box<dyn Stream>));
                let p = run_case(
                    &c,
                    Deflate::init2(c.level, c.method, c.wbits, c.mem, c.strategy)
                        .map(|d| Box::new(Port(d)) as Box<dyn Stream>),
                );
                bytes.fetch_add(c.data.len(), Ordering::Relaxed);
                if let Some(m) = compare("released", &a, &b) {
                    up_differs.fetch_add(1, Ordering::Relaxed);
                    eprintln!("case {i}: Ubuntu's vs released: {m}");
                }
                if let Some(m) = compare("port", &a, &p) {
                    fails.fetch_add(1, Ordering::Relaxed);
                    eprintln!("case {i}: DIFFER {m}");
                }
                let mut h = fnv(&p.out);
                for l in &p.lines {
                    h ^= fnv(l.as_bytes()).rotate_left(7);
                }
                digest.lock().unwrap()[i as usize] = h;
            });
        }
    });
    let d = digest.into_inner().unwrap();
    let mut all = Vec::new();
    for h in d {
        all.extend_from_slice(&h.to_le_bytes());
    }
    println!(
        "cases {n} seed {seed}: input {} bytes; port differs {}; released differs from Ubuntu's {}; digest {:016x}",
        bytes.load(Ordering::Relaxed),
        fails.load(Ordering::Relaxed),
        up_differs.load(Ordering::Relaxed),
        fnv(&all)
    );
    println!("{}", if fails.load(Ordering::Relaxed) == 0 { "VERIFY-PASS" } else { "VERIFY-FAIL" });
}

fn one(dir: &str, seed: u64, i: u64) {
    let (sys, _up) = libs(dir);
    let c = gen_case(seed, i);
    println!(
        "level {} method {} wbits {} mem {} strategy {} data {} dict {:?} header {} ops {:?}",
        c.level,
        c.method,
        c.wbits,
        c.mem,
        c.strategy,
        c.data.len(),
        c.dict.as_ref().map(|d| d.len()),
        c.header.is_some(),
        c.ops
    );
    let a = run_case(&c, open_c(&sys, &c).map(|s| Box::new(s) as Box<dyn Stream>));
    let p = run_case(
        &c,
        Deflate::init2(c.level, c.method, c.wbits, c.mem, c.strategy)
            .map(|d| Box::new(Port(d)) as Box<dyn Stream>),
    );
    for (k, (x, y)) in a.lines.iter().zip(&p.lines).enumerate() {
        println!("{k:4} {} {x}\n          {y}", if x == y { " " } else { "*" });
    }
    println!("lines {} / {}; output {} / {} bytes, {}", a.lines.len(), p.lines.len(), a.out.len(), p.out.len(),
        if a.out == p.out { "same" } else { "DIFFERENT" });
}

fn sums(dir: &str, seed: u64, n: u64) {
    let (sys, _) = libs(dir);
    let mut r = Rng::new(seed ^ 0x5eed);
    let mut bad = 0;
    for i in 0..n {
        let len = gen_len(&mut r).min(300_000);
        let data = gen_data(&mut r, len);
        let a0 = if r.chance(3) { 1 } else { r.next() & 0xffff_ffff };
        let c0 = if r.chance(3) { 0 } else { r.next() & 0xffff_ffff };
        // SAFETY: valid buffers for zlib's checksums.
        let (ca, cc) = unsafe {
            (
                (sys.adler32)(a0, data.as_ptr(), data.len() as c_uint),
                (sys.crc32)(c0, data.as_ptr(), data.len() as c_uint),
            )
        };
        let (pa, pc) = (z::adler32(a0, Some(&data)), z::crc32(c0, Some(&data)));
        if (ca, cc) != (pa, pc) {
            bad += 1;
            eprintln!("sums case {i}: adler {ca} {pa} crc {cc} {pc}");
        }
        let level = r.range(-2, 10) as i32;
        // SAFETY: as above.
        let bound = unsafe { (sys.compress_bound)(data.len() as c_ulong) };
        let room = match r.below(4) {
            0 => r.below(bound + 1) as usize,
            _ => bound as usize,
        };
        let mut a = vec![0u8; room.max(1)];
        let mut alen = room as c_ulong;
        // SAFETY: a destination of `room` bytes.
        let ar = unsafe { (sys.compress2)(a.as_mut_ptr(), &mut alen, data.as_ptr(), data.len() as c_ulong, level) };
        let mut b = vec![0u8; room];
        let (br, blen) = z::compress2(&mut b, &data, level);
        if bound != z::compress_bound(data.len() as u64)
            || ar != br
            || alen as usize != blen
            || a[..alen as usize] != b[..blen]
        {
            bad += 1;
            eprintln!("compress2 case {i}: level {level} room {room}: {ar} {alen} vs {br} {blen}");
        }
    }
    // SAFETY: null buffers give the initial values.
    let (a1, c1) = unsafe { ((sys.adler32)(0, std::ptr::null(), 0), (sys.crc32)(0, std::ptr::null(), 0)) };
    if (a1, c1) != (z::adler32(0, None), z::crc32(0, None)) {
        bad += 1;
    }
    println!("sums {n}: {bad} differ");
    println!("{}", if bad == 0 { "VERIFY-PASS" } else { "VERIFY-FAIL" });
}

/// gzopen/gzwrite/gzputc/gzputs/gzseek/gzflush/gzclose through zlib and through
/// `larmorx_io::znz::GzFile`, the two files compared byte for byte.
fn gz(dir: &str, seed: u64, n: u64, scratch: &str) {
    use larmorx_io::znz::{GzFile, Whence};
    let (sys, _) = libs(dir);
    let (mut bad, mut fs78) = (0, 0);
    std::fs::create_dir_all(scratch).unwrap();
    for i in 0..n {
        let mut r = Rng::new(seed.wrapping_mul(7919).wrapping_add(i));
        let mut mode = String::from(if r.chance(10) { "ab" } else { "wb" });
        if r.chance(2) {
            mode.push((b'0' + r.below(10) as u8) as char);
        }
        if r.chance(5) {
            mode.push(*[b'f', b'h', b'R', b'F'].get(r.below(4) as usize).unwrap() as char);
        }
        if r.chance(30) {
            mode.push('T');
        }
        let pa = format!("{scratch}/c{i}.gz");
        let pb = format!("{scratch}/p{i}.gz");
        let _ = std::fs::remove_file(&pa);
        let _ = std::fs::remove_file(&pb);
        let ca = CString::new(pa.clone()).unwrap();
        let cm = CString::new(mode.clone()).unwrap();
        // SAFETY: gzopen with NUL-terminated strings.
        let g = unsafe { (sys.gzopen)(ca.as_ptr(), cm.as_ptr()) };
        let mut p = GzFile::open(pb.as_bytes(), mode.as_bytes());
        assert_eq!(g.is_null(), p.is_none(), "case {i} mode {mode}");
        let Some(mut p) = p.take() else { continue };
        let mut lines = (Vec::new(), Vec::new());
        let nops = r.range(0, 40);
        for _ in 0..nops {
            match r.below(20) {
                0..=13 => {
                    let len = match r.below(4) {
                        0 => r.range(0, 10),
                        1 => r.range(0, 9000),
                        2 => r.range(8000, 20_000),
                        _ => r.range(0, 300_000),
                    } as usize;
                    let data = gen_data(&mut r, len);
                    // SAFETY: a valid gzFile and buffer.
                    let a = unsafe { (sys.gzwrite)(g, data.as_ptr(), data.len() as c_uint) };
                    lines.0.push(format!("w {a}"));
                    lines.1.push(format!("w {}", p.gzwrite(&data)));
                }
                14 => {
                    let c = r.range(-300, 300) as i32;
                    // SAFETY: as above.
                    let a = unsafe { (sys.gzputc)(g, c) };
                    lines.0.push(format!("c {a}"));
                    lines.1.push(format!("c {}", p.gzputc(c)));
                }
                15 => {
                    let s: Vec<u8> = (0..r.range(0, 50)).map(|_| r.range(1, 255) as u8).collect();
                    let cs = CString::new(s.clone()).unwrap();
                    // SAFETY: as above.
                    let a = unsafe { (sys.gzputs)(g, cs.as_ptr()) };
                    lines.0.push(format!("s {a}"));
                    lines.1.push(format!("s {}", p.gzputs(&s)));
                }
                16..=17 => {
                    let off = r.range(-100, 40_000);
                    let wh = r.below(2) as i32;
                    // SAFETY: as above.
                    let a = unsafe { (sys.gzseek)(g, off as c_long, wh) };
                    let w = if wh == 0 { Whence::Set } else { Whence::Cur };
                    lines.0.push(format!("k {a}"));
                    lines.1.push(format!("k {}", p.gzseek(off, w)));
                }
                18 => {
                    // SAFETY: as above.
                    let a = unsafe { (sys.gzflush)(g, z::Z_SYNC_FLUSH) };
                    lines.0.push(format!("f {a}"));
                    lines.1.push(format!("f {}", p.gzflush()));
                }
                _ => {
                    // SAFETY: as above.
                    let a = unsafe { (sys.gztell)(g) };
                    lines.0.push(format!("t {a}"));
                    lines.1.push(format!("t {}", p.gztell()));
                }
            }
        }
        // SAFETY: as above.
        let a = unsafe { (sys.gzclose)(g) };
        lines.0.push(format!("x {a}"));
        let (x, notices) = p.gzclose_with_notices();
        lines.1.push(format!("x {x}"));
        let fa = std::fs::read(&pa).unwrap();
        let fb = std::fs::read(&pb).unwrap();
        if lines.0 == lines.1 && fa != fb && !notices.is_empty() {
            // FS-78: zeros where zlib compresses memory it never set
            fs78 += 1;
            let _ = std::fs::remove_file(&pa);
            let _ = std::fs::remove_file(&pb);
        } else if lines.0 != lines.1 || fa != fb {
            bad += 1;
            let at = fa.iter().zip(&fb).position(|(x, y)| x != y);
            eprintln!(
                "gz case {i} mode {mode}: calls {} files {} / {} bytes, first difference {at:?}",
                if lines.0 == lines.1 { "same" } else { "DIFFER" },
                fa.len(),
                fb.len()
            );
            if lines.0 != lines.1 {
                eprintln!("  zlib {:?}\n  port {:?}", lines.0, lines.1);
            }
        } else {
            let _ = std::fs::remove_file(&pa);
            let _ = std::fs::remove_file(&pb);
        }
    }
    println!("gz {n}: {bad} differ; {fs78} differ where FS-78's notice says zlib compressed memory it never set");
    println!("{}", if bad == 0 { "VERIFY-PASS" } else { "VERIFY-FAIL" });
}

/// The length of a gzip member's header (RFC 1952: the fixed 10 bytes, then FEXTRA, FNAME,
/// FCOMMENT, FHCRC as its flags say).
fn gzip_header_len(m: &[u8]) -> usize {
    let flg = m[3];
    let mut at = 10;
    if flg & 4 != 0 {
        at += 2 + usize::from(m[at]) + (usize::from(m[at + 1]) << 8);
    }
    for bit in [8u8, 16] {
        if flg & bit != 0 {
            while m[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flg & 2 != 0 {
        at += 2;
    }
    at
}

/// A raw deflate stream of `data` (window 15, memory level 8, default strategy) by zlib or by
/// the port.
fn raw(sys: Option<&Lib>, data: &[u8], level: i32) -> Vec<u8> {
    let mut out = vec![0u8; data.len() + data.len() / 100 + 1024];
    let made = match sys {
        Some(l) => {
            let mut c = open_c(l, &Case {
                level, method: 8, wbits: -15, mem: 8, strategy: 0,
                data: Vec::new(), dict: None, header: None, ops: Vec::new(),
            }).unwrap();
            let (r, _, made) = c.deflate(data, &mut out, z::Z_FINISH);
            assert_eq!(r, z::Z_STREAM_END);
            Box::new(c).end();
            made
        }
        None => {
            let mut d = Deflate::init2(level, 8, -15, 8, 0).unwrap();
            let (r, _, made) = d.deflate(data, &mut out, z::Z_FINISH);
            assert_eq!(r, z::Z_STREAM_END);
            made
        }
    };
    out.truncate(made);
    out
}

/// Each gzip file's first member inflated by zlib, then its deflate data made again by zlib
/// and by the port (raw deflate, window 15, memory level 8, default strategy: `gzopen(path,
/// "wb")` is level 6; when zlib's level 6 does not give the file's bytes, the levels 1 to 9 are
/// tried, as a Python writer may use another); the port compared with the file at the level
/// whose zlib bytes are the file's. The header and trailer: zlib's own when the header is the
/// 10 bytes `gzopen` writes.
fn files(dir: &str, list: &str) {
    let (sys, _) = libs(dir);
    let paths = std::fs::read_to_string(list).unwrap();
    let (mut n, mut port_same, mut no_level, mut total) = (0, 0, 0, 0u64);
    let mut by_level = [0u32; 10];
    for p in paths.lines().filter(|l| !l.is_empty()) {
        let file = std::fs::read(p).unwrap();
        let mut zs = zeroed();
        // SAFETY: inflateInit2_ on a zeroed stream (gzip wrapper).
        unsafe {
            (sys.inflate_init2)(
                &mut *zs,
                31,
                VERSION.as_ptr() as *const c_char,
                std::mem::size_of::<ZStream>() as c_int,
            )
        };
        let mut data = Vec::new();
        let mut buf = vec![0u8; 1 << 20];
        zs.next_in = file.as_ptr();
        zs.avail_in = file.len() as c_uint;
        let consumed;
        loop {
            zs.next_out = buf.as_mut_ptr();
            zs.avail_out = buf.len() as c_uint;
            // SAFETY: valid buffers.
            let r = unsafe { (sys.inflate)(&mut *zs, z::Z_NO_FLUSH) };
            data.extend_from_slice(&buf[..buf.len() - zs.avail_out as usize]);
            if r == z::Z_STREAM_END {
                consumed = file.len() - zs.avail_in as usize;
                break;
            }
            assert_eq!(r, z::Z_OK, "inflate {p}");
        }
        // SAFETY: as above.
        unsafe { (sys.inflate_end)(&mut *zs) };
        let member = &file[..consumed];
        let h = gzip_header_len(member);
        let payload = &member[h..consumed - 8];
        n += 1;
        total += data.len() as u64;
        let mut level = None;
        for l in [6, 1, 2, 3, 4, 5, 7, 8, 9] {
            if raw(Some(&sys), &data, l) == payload {
                level = Some(l);
                break;
            }
        }
        let verdict = match level {
            None => {
                no_level += 1;
                "NO-ZLIB-LEVEL".to_string()
            }
            Some(l) => {
                by_level[l as usize] += 1;
                let ok = raw(None, &data, l) == payload;
                // the whole member when the header is gzopen's
                let whole = if member[..10] == [0x1f, 0x8b, 8, 0, 0, 0, 0, 0, if l == 9 { 2 } else if l < 2 { 4 } else { 0 }, 3] {
                    let mut d = Deflate::init2(l, 8, 31, 8, 0).unwrap();
                    let mut out = vec![0u8; data.len() + data.len() / 100 + 1024];
                    let (_, _, made) = d.deflate(&data, &mut out, z::Z_FINISH);
                    if out[..made] == *member { " whole-member-same" } else { " whole-member-DIFF" }
                } else {
                    " (header not gzopen's)"
                };
                if ok {
                    port_same += 1;
                }
                format!("{} level {l}{whole}", if ok { "SAME" } else { "DIFF" })
            }
        };
        println!(
            "{verdict} {p} {} bytes ({} inflated){}",
            member.len(),
            data.len(),
            if consumed != file.len() {
                format!(", {} bytes after the member", file.len() - consumed)
            } else {
                String::new()
            }
        );
        std::io::stdout().flush().unwrap();
    }
    println!(
        "files {n} ({total} bytes inflated): port = file {port_same}; zlib's level per file {by_level:?}; no zlib level gives the file {no_level}"
    );
    println!(
        "{}",
        if port_same + no_level == n { "VERIFY-PASS" } else { "VERIFY-FAIL" }
    );
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let num = |i: usize| a[i].parse::<u64>().unwrap();
    match a.get(1).map(String::as_str) {
        Some("cases") => cases(&a[2], num(3), num(4), num(5) as usize),
        Some("one") => one(&a[2], num(3), num(4)),
        Some("sums") => sums(&a[2], num(3), num(4)),
        Some("gz") => gz(&a[2], num(3), num(4), &a[5]),
        Some("files") => files(&a[2], &a[3]),
        _ => {
            eprintln!("usage: see the source's header");
            std::process::exit(2);
        }
    }
}
