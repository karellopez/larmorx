// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! `tif_pixarlog.c`: the PixarLog decoder (Pixar's companded 11-bit codes, horizontally
//! differenced, deflated; over [`larmorx_core::zlib`]), its pseudo-tags and the companding
//! tables. libtiff 3.6.1's encoder is not ported (FreeSurfer writes Deflate only); the
//! ABGR output (`PIXARLOGDATAFMT_8BITABGR`) only a caller's `TIFFSetField` can choose, and no
//! caller of this port can, so it is left out.
//!
//! The tables come from `exp` and `log` of a few fixed arguments: by default the correctly
//! rounded functions of [`larmorx_core::math`], with `reproduce` glibc 2.35's; they give the
//! same tables (`tests::the_tables_are_the_same_with_either_exp`).
//!
//! libtiff 3.6.1 sizes its row buffer by the rows per strip (the default 2^32 - 1 overflows it),
//! never checks that allocation, and reads the first code of a row through the tables without
//! masking it to 11 bits; for 11- and 12-bit samples a scanline holds fewer codes than a row,
//! and the accumulators read codes never inflated and write past the buffer (FS-840). By default
//! the row buffer is what a row needs, the first code is masked and the codes are cut to whole
//! rows with libtiff 4.0.10's warning; with `reproduce` the reads of codes never inflated give 0
//! and the writes past the caller's buffer are dropped.

use std::sync::OnceLock;

use super::Tiff;
use super::fields::tag;
use super::next::Out;
use larmorx_core::cfmt::Streams;
use larmorx_core::defects::FS_840;
use larmorx_core::zlib::{self as z, Inflate};

const TSIZE: usize = 2048;
const TSIZEP1: usize = 2049;
const ONE: f64 = 1250.0;
const RATIO: f64 = 1.004;
const CODE_MASK: u32 = 0x7ff;

pub(crate) const FMT_UNKNOWN: i32 = -1;
const FMT_8BIT: i32 = 0;
const FMT_8BITABGR: i32 = 1;
const FMT_11BITLOG: i32 = 2;
const FMT_12BITPICIO: i32 = 3;
const FMT_16BIT: i32 = 4;
const FMT_FLOAT: i32 = 5;

/// `ToLinearF`, `ToLinear16`, `ToLinear8` (the decoder's tables).
pub struct Tables {
    pub to_f: [f32; TSIZEP1],
    pub to_16: [u16; TSIZEP1],
    pub to_8: [u8; TSIZEP1],
}

/// `PixarLogMakeTables`'s decoding tables, with glibc's `exp` and `log` (`glibc`) or the
/// correctly rounded ones.
pub fn tables(glibc: bool) -> &'static Tables {
    static CR: OnceLock<Tables> = OnceLock::new();
    static GL: OnceLock<Tables> = OnceLock::new();
    let make = || {
        let (exp, log): (fn(f64) -> f64, fn(f64) -> f64) = if glibc {
            (larmorx_core::math::exp_glibc, larmorx_core::math::log_glibc)
        } else {
            (larmorx_core::math::exp, larmorx_core::math::log)
        };
        let mut c = log(RATIO);
        // `nlin = (int)1./c`: the cast binds to the 1
        let nlin = (1.0 / c) as i32;
        c = 1.0 / f64::from(nlin);
        let b = exp(-c * ONE);
        let linstep = b * c * exp(1.0);
        let mut to_f = [0f32; TSIZEP1];
        let mut j = 0usize;
        for i in 0..nlin {
            to_f[j] = (f64::from(i) * linstep) as f32;
            j += 1;
        }
        for i in nlin..TSIZE as i32 {
            to_f[j] = (b * exp(c * f64::from(i))) as f32;
            j += 1;
        }
        to_f[2048] = to_f[2047];
        let mut to_16 = [0u16; TSIZEP1];
        let mut to_8 = [0u8; TSIZEP1];
        for i in 0..TSIZEP1 {
            let v = f64::from(to_f[i]) * 65535.0 + 0.5;
            to_16[i] = if v > 65535.0 { 65535 } else { v as u16 };
            let v = f64::from(to_f[i]) * 255.0 + 0.5;
            to_8[i] = if v > 255.0 { 255 } else { v as u8 };
        }
        Tables { to_f, to_16, to_8 }
    };
    if glibc {
        GL.get_or_init(make)
    } else {
        CR.get_or_init(make)
    }
}

/// `PixarLogState` (the decoder's part).
#[derive(Default)]
pub(crate) struct PixarLog {
    inflate: Option<Inflate>,
    in_at: usize,
    in_left: u32,
    /// The row buffer (`tbuf`) as 16-bit codes, the codes inflate has written since it was
    /// allocated, and libtiff 3.6.1's allocation in bytes.
    tbuf: Vec<u16>,
    tbuf_def: Vec<bool>,
    tbuf_bytes: u64,
    stride: u16,
    state_init: bool,
    pub user_datafmt: i32,
    pub quality: i32,
}

/// The accumulators' notes.
#[derive(Default)]
struct Flags {
    undefined: bool,
    beyond_tbuf: bool,
    unmasked: bool,
}

impl Tiff {
    /// `TIFFInitPixarLog`.
    pub(crate) fn pixarlog_init(&mut self) {
        use super::codec::{Cleanup, Decode, PreDecode, SetupDecode};
        use super::dir::Layer;
        use super::fields::Table;
        self.codec.pixarlog = Some(Box::new(PixarLog {
            user_datafmt: FMT_UNKNOWN,
            quality: z::Z_DEFAULT_COMPRESSION,
            ..PixarLog::default()
        }));
        let k = &mut self.codec;
        k.setupdecode = SetupDecode::PixarLog;
        k.predecode = PreDecode::PixarLog;
        k.decoderow = Decode::PixarLog;
        k.decodestrip = Decode::PixarLog;
        k.decodetile = Decode::PixarLog;
        k.cleanup = Cleanup::PixarLog;
        self.fields.merge(Table::PixarLog);
        self.codec.layers.insert(0, Layer::PixarLog);
        self.predictor_init();
    }

    /// `PixarLogVSetField` for its pseudo-tags (`None`: its parent's).
    pub(crate) fn pixarlog_vsetfield(&mut self, t: u32, v: u32) -> Option<bool> {
        let sp = self.codec.pixarlog.as_mut()?;
        match t {
            tag::PIXARLOGQUALITY => {
                // the stream is an inflater when reading: no deflateParams
                sp.quality = v as i32;
                Some(true)
            }
            tag::PIXARLOGDATAFMT => {
                // only a caller's TIFFSetField reaches this (a file's tags are 16-bit)
                sp.user_datafmt = v as i32;
                Some(true)
            }
            _ => None,
        }
    }

    /// `PixarLogVGetField`.
    pub(crate) fn pixarlog_vgetfield(&self, t: u32) -> Option<super::Value> {
        let sp = self.codec.pixarlog.as_ref()?;
        match t {
            tag::PIXARLOGQUALITY => Some(super::Value::I32(sp.quality)),
            tag::PIXARLOGDATAFMT => Some(super::Value::I32(sp.user_datafmt)),
            _ => None,
        }
    }

    /// `PixarLogSetupDecode`.
    pub(crate) fn pixarlog_setup(&mut self, s: &mut dyn Streams) -> bool {
        const MODULE: &str = "PixarLogSetupDecode";
        self.codec.postdecode = super::dir::PostDecode::None;
        let d = &self.dir;
        let stride: u16 = if d.planarconfig == super::values::PLANARCONFIG_CONTIG {
            d.samplesperpixel
        } else {
            1
        };
        let bytes = u64::from(
            u32::from(stride)
                .wrapping_mul(d.imagewidth)
                .wrapping_mul(d.rowsperstrip),
        ) * 2;
        let (bps, format) = (d.bitspersample, d.sampleformat);
        let name = self.name.clone();
        let sp = self.codec.pixarlog.as_mut().expect("PixarLog state");
        sp.stride = stride;
        sp.tbuf.clear();
        sp.tbuf_def.clear();
        sp.tbuf_bytes = bytes;
        let wanted = u64::from(stride) * u64::from(self.dir.imagewidth) * u64::from(self.dir.rowsperstrip) * 2;
        if !self.fix() && (bytes != wanted || bytes > 1 << 31) {
            self.notices.add(
                &FS_840,
                format!(
                    "the PixarLog row buffer of {name} is allocated with {bytes} bytes \
                     ({stride} samples x {} pixels x {} rows per strip x 2, in 32 bits) and \
                     never checked: where that allocation fails libtiff 3.6.1 decodes through a \
                     null pointer (a crash)",
                    self.dir.imagewidth, self.dir.rowsperstrip
                ),
            );
        }
        let sp = self.codec.pixarlog.as_mut().expect("PixarLog state");
        if sp.user_datafmt == FMT_UNKNOWN {
            sp.user_datafmt = guess_data_fmt(bps, format);
        }
        if sp.user_datafmt == FMT_UNKNOWN {
            let m = format!(
                "PixarLog compression can't handle bits depth/data format combination (depth: {bps})"
            );
            self.error(Some(MODULE), &m, s);
            return false;
        }
        match Inflate::init() {
            Ok(i) => {
                sp.inflate = Some(i);
                sp.state_init = true;
                true
            }
            Err(_) => {
                let m = format!("{name}: (null)");
                self.error(Some(MODULE), &m, s);
                false
            }
        }
    }

    /// `PixarLogPreDecode`.
    pub(crate) fn pixarlog_pre_decode(&mut self) -> bool {
        let rawcc = self.rawcc;
        let sp = self.codec.pixarlog.as_mut().expect("PixarLog state");
        sp.in_at = 0;
        sp.in_left = rawcc as u32;
        sp.inflate.as_mut().expect("inflate").reset() == z::Z_OK
    }

    /// `PixarLogDecode`.
    pub(crate) fn pixarlog_decode(&mut self, op: &mut [u8], occ: i32, s: &mut dyn Streams) -> i32 {
        const MODULE: &str = "PixarLogDecode";
        let fix = self.fix();
        let fmt = self.codec.pixarlog.as_ref().expect("PixarLog").user_datafmt;
        // `occ / sizeof(...)`: a tsize_t divided by a size_t
        let nsamples: i32 = match fmt {
            FMT_FLOAT => (i64::from(occ) as u64 / 4) as i32,
            FMT_16BIT | FMT_12BITPICIO | FMT_11BITLOG => (i64::from(occ) as u64 / 2) as i32,
            FMT_8BIT | FMT_8BITABGR => occ,
            _ => {
                let m = format!("{} bit input not supported in PixarLog", self.dir.bitspersample);
                self.error(Some(&self.name.clone()), &m, s);
                return 0;
            }
        };
        let sp = self.codec.pixarlog.as_ref().expect("PixarLog");
        let llen = i32::from(sp.stride).wrapping_mul(self.dir.imagewidth as i32);
        let avail0 = (i64::from(nsamples) as u64).wrapping_mul(2) as u32;
        let name = self.name.clone();
        if sp.inflate.is_none() {
            return 0;
        }
        let (past, tbuf_bytes) = (sp.in_at + sp.in_left as usize > self.raw_defined, sp.tbuf_bytes);
        if past {
            self.past_raw_notice();
        }
        if u64::from(avail0) > tbuf_bytes && !fix {
            self.notices.add(
                &FS_840,
                format!(
                    "decoding {name}: PixarLog inflates {avail0} bytes into a row buffer of \
                     {tbuf_bytes} (memory past it)"
                ),
            );
        }
        // inflate into the row buffer (native 16-bit codes)
        let mut sp = self.codec.pixarlog.take().expect("PixarLog");
        let mut bytes = vec![0u8; avail0 as usize];
        // the codes left from an earlier call (stale bytes the buffer still holds)
        for (k, v) in sp.tbuf.iter().take(bytes.len() / 2).enumerate() {
            bytes[2 * k..2 * k + 2].copy_from_slice(&v.to_le_bytes());
        }
        let inf = sp.inflate.as_mut().expect("inflate");
        let mut out_at = 0usize;
        let mut avail_out = avail0;
        let mut r = 1;
        loop {
            let input = &self.rawdata[sp.in_at.min(self.raw_defined)
                ..(sp.in_at + sp.in_left as usize).min(self.raw_defined)];
            let (state, used, wrote) = inf.inflate(input, &mut bytes[out_at..], z::Z_PARTIAL_FLUSH);
            sp.in_at += used;
            sp.in_left -= used as u32;
            out_at += wrote;
            avail_out -= wrote as u32;
            if state == z::Z_STREAM_END {
                break;
            }
            if state == z::Z_DATA_ERROR {
                let m = format!(
                    "{name}: Decoding error at scanline {}, {}",
                    self.row as i32,
                    inf.msg.unwrap_or("(null)")
                );
                self.error(Some(MODULE), &m, s);
                let input = &self.rawdata[sp.in_at.min(self.raw_defined)
                    ..(sp.in_at + sp.in_left as usize).min(self.raw_defined)];
                let (sr, skipped) = inf.sync(input);
                sp.in_at += skipped;
                sp.in_left -= skipped as u32;
                if sr != z::Z_OK {
                    r = 0;
                    break;
                }
                if avail_out > 0 {
                    continue;
                }
                break;
            }
            if state != z::Z_OK {
                let m = format!("{name}: zlib error: {}", inf.msg.unwrap_or("(null)"));
                self.error(Some(MODULE), &m, s);
                r = 0;
                break;
            }
            if avail_out == 0 {
                break;
            }
        }
        // the codes inflate wrote (a last odd byte keeps the other half of its code)
        let ncodes = out_at.div_ceil(2);
        if sp.tbuf.len() < ncodes {
            sp.tbuf.resize(ncodes, 0);
            sp.tbuf_def.resize(ncodes, false);
        }
        for k in 0..ncodes {
            sp.tbuf[k] = u16::from_le_bytes([bytes[2 * k], bytes[2 * k + 1]]);
            if 2 * k + 1 < out_at {
                sp.tbuf_def[k] = true;
            }
        }
        if r == 0 {
            self.codec.pixarlog = Some(sp);
            return 0;
        }
        if avail_out != 0 {
            let m = format!(
                "{name}: Not enough data at scanline {} (short {} bytes)",
                self.row as i32, avail_out as i32
            );
            self.error(Some(MODULE), &m, s);
            self.codec.pixarlog = Some(sp);
            return 0;
        }
        if self.swab() {
            for v in sp.tbuf.iter_mut().take(nsamples.max(0) as usize) {
                *v = v.swap_bytes();
            }
        }
        let mut nsamples = nsamples;
        if fix && llen > 0 && nsamples % llen != 0 {
            self.notices.add(
                &FS_840,
                format!(
                    "decoding {name}: a PixarLog scanline of {nsamples} codes is not a whole \
                     number of rows of {llen}: cut to whole rows (corrected as libtiff 4.0.10 \
                     does; libtiff 3.6.1 reads codes never inflated and writes past the buffer)"
                ),
            );
            let m = format!(
                "stride {llen} is not a multiple of sample count, {nsamples}, data truncated."
            );
            self.warning(Some(MODULE), &m, s);
            nsamples -= nsamples % llen;
        }
        let t = tables(!fix);
        let stride = i32::from(sp.stride);
        let mut fl = Flags::default();
        let mut o = Out { buf: op, past: false };
        let mut hang = false;
        {
            let mut i = 0i32;
            let mut up = 0i64;
            let mut at = 0i64;
            let size = match fmt {
                FMT_FLOAT => 4,
                FMT_16BIT | FMT_12BITPICIO | FMT_11BITLOG => 2,
                _ => 1,
            };
            while i < nsamples {
                if llen <= 0 {
                    hang = true;
                    break;
                }
                if at > o.buf.len() as i64 {
                    // every row from here is past the caller's buffer
                    o.past = true;
                    break;
                }
                let mut a = Acc {
                    tbuf: &mut sp.tbuf,
                    def: &mut sp.tbuf_def,
                    cap: sp.tbuf_bytes / 2,
                    fix,
                    fl: &mut fl,
                };
                accumulate(&mut a, up, llen, stride, fmt, t, &mut o, at);
                i = i.wrapping_add(llen);
                up += i64::from(llen);
                at += i64::from(llen) * size;
            }
        }
        self.codec.pixarlog = Some(sp);
        if fl.unmasked {
            self.notices.add(
                &FS_840,
                format!(
                    "decoding {name}: a PixarLog row starts with a code above 2047, which \
                     libtiff 3.6.1 looks up unmasked (memory past its tables){}",
                    if fix { "; masked to 11 bits (as libtiff 4.0.10 does)" } else { "; read as 0" }
                ),
            );
        }
        if fl.undefined || fl.beyond_tbuf {
            self.notices.add(
                &FS_840,
                format!(
                    "decoding {name}: the PixarLog accumulators read codes never inflated into \
                     the row buffer{} (memory contents); read as 0",
                    if fl.beyond_tbuf { " or past it" } else { "" }
                ),
            );
        }
        if hang {
            self.notices.add(
                &FS_840,
                format!(
                    "decoding {name}: a PixarLog row of {llen} samples for {nsamples} codes: \
                     libtiff 3.6.1 never returns (a hang); nothing decoded"
                ),
            );
            return 0;
        }
        if o.past {
            self.past_buffer_notice("PixarLog");
        }
        1
    }
}

/// `PixarLogGuessDataFmt`.
fn guess_data_fmt(bps: u16, format: u16) -> i32 {
    use super::values::{SAMPLEFORMAT_IEEEFP, SAMPLEFORMAT_INT, SAMPLEFORMAT_UINT};
    const SAMPLEFORMAT_VOID: u16 = 4;
    match bps {
        32 if format == SAMPLEFORMAT_IEEEFP => FMT_FLOAT,
        16 if format == SAMPLEFORMAT_VOID || format == SAMPLEFORMAT_UINT => FMT_16BIT,
        12 if format == SAMPLEFORMAT_VOID || format == SAMPLEFORMAT_INT => FMT_12BITPICIO,
        11 if format == SAMPLEFORMAT_VOID || format == SAMPLEFORMAT_UINT => FMT_11BITLOG,
        8 if format == SAMPLEFORMAT_VOID || format == SAMPLEFORMAT_UINT => FMT_8BIT,
        _ => FMT_UNKNOWN,
    }
}

/// The row buffer as the accumulators see it: each code with whether it is defined (inflated
/// in this or an earlier call, or summed from defined codes).
struct Acc<'a> {
    tbuf: &'a mut Vec<u16>,
    def: &'a mut Vec<bool>,
    cap: u64,
    fix: bool,
    fl: &'a mut Flags,
}

impl Acc<'_> {
    fn get(&mut self, i: i64) -> (u32, bool) {
        if i < 0 {
            return (0, false);
        }
        let k = i as usize;
        if !self.fix && k as u64 >= self.cap {
            // past libtiff 3.6.1's allocation: heap memory
            return (0, false);
        }
        match (self.tbuf.get(k), self.def.get(k)) {
            (Some(&v), Some(&true)) => (u32::from(v), true),
            _ => (0, false),
        }
    }
    fn set(&mut self, i: i64, v: u32, defined: bool) {
        if i < 0 {
            return;
        }
        let k = i as usize;
        if !self.fix && k as u64 >= self.cap {
            self.fl.beyond_tbuf = true;
            return;
        }
        if k >= self.tbuf.len() {
            self.tbuf.resize(k + 1, 0);
            self.def.resize(k + 1, false);
        }
        self.tbuf[k] = v as u16;
        self.def[k] = defined;
    }
}

/// A table lookup of an index the original does not mask (the first codes of a row).
fn first(a: &mut Acc, v: u32) -> usize {
    if v as usize >= TSIZEP1 {
        a.fl.unmasked = true;
    }
    if a.fix {
        (v & CODE_MASK) as usize
    } else {
        v as usize
    }
}

/// One output value: sample `k` of the row at byte `at`, from `code` (a table index; for
/// 11-bit output the value itself, `masked` or not).
#[allow(clippy::too_many_arguments)]
fn put(o: &mut Out, fl: &mut Flags, fmt: i32, t: &Tables, at: i64, k: i64, code: usize, defined: bool, masked: bool) {
    if !defined {
        fl.undefined = true;
    }
    match fmt {
        FMT_FLOAT => {
            let f = if code < TSIZEP1 { t.to_f[code] } else { 0.0 };
            for (b, v) in f.to_le_bytes().into_iter().enumerate() {
                o.set(at + 4 * k + b as i64, v);
            }
        }
        FMT_16BIT => {
            let v = if code < TSIZEP1 { t.to_16[code] } else { 0 };
            for (b, x) in v.to_le_bytes().into_iter().enumerate() {
                o.set(at + 2 * k + b as i64, x);
            }
        }
        FMT_12BITPICIO => {
            let f = if code < TSIZEP1 { t.to_f[code] } else { 0.0 };
            let t0 = (f64::from(f) * 2048.0) as f32;
            let v: u16 = if t0 < 3071.0 { t0 as u16 } else { 3071 };
            for (b, x) in v.to_le_bytes().into_iter().enumerate() {
                o.set(at + 2 * k + b as i64, x);
            }
        }
        FMT_11BITLOG => {
            let v = if masked {
                (code as u32 & CODE_MASK) as u16
            } else {
                code as u16
            };
            for (b, x) in v.to_le_bytes().into_iter().enumerate() {
                o.set(at + 2 * k + b as i64, x);
            }
        }
        _ => {
            let v = if code < TSIZEP1 { t.to_8[code] } else { 0 };
            o.set(at + k, v);
        }
    }
}

/// One of `horizontalAccumulateF`, `...12`, `...16`, `...11`, `...8` over the row of `n` codes
/// at `up` into the output at byte `at`.
#[allow(clippy::too_many_arguments)]
fn accumulate(
    a: &mut Acc,
    up: i64,
    n: i32,
    stride: i32,
    fmt: i32,
    t: &Tables,
    o: &mut Out,
    at: i64,
) {
    if n < stride {
        return;
    }
    let mask = CODE_MASK;
    if stride == 3 || stride == 4 {
        let s = i64::from(stride);
        let mut c = [0u32; 4];
        let mut d = [true; 4];
        for j in 0..s as usize {
            let (v, def) = a.get(up + j as i64);
            c[j] = v;
            d[j] = def;
            let idx = if fmt == FMT_11BITLOG {
                v as usize
            } else {
                first(a, v)
            };
            put(o, a.fl, fmt, t, at, j as i64, idx, def, false);
        }
        let mut n = n - stride;
        let mut w = 0i64;
        while n > 0 {
            w += s;
            n -= stride;
            for j in 0..s as usize {
                let (v, def) = a.get(up + w + j as i64);
                c[j] = c[j].wrapping_add(v);
                d[j] &= def;
                put(o, a.fl, fmt, t, at, w + j as i64, (c[j] & mask) as usize, d[j], true);
            }
        }
    } else {
        // REPEAT(stride, ...) runs at least once; `wp[stride]` is the next pixel's sample
        let reps = i64::from(stride.max(1));
        let s = i64::from(stride);
        let mut w = 0i64;
        for _ in 0..reps {
            let (v, def) = a.get(up + w);
            put(o, a.fl, fmt, t, at, w, (v & mask) as usize, def, true);
            w += 1;
        }
        let mut n = n.wrapping_sub(stride);
        while n > 0 {
            for _ in 0..reps {
                let (cur, dc) = a.get(up + w);
                let (next, dn) = a.get(up + w + s);
                let sum = next.wrapping_add(cur) & 0xffff;
                a.set(up + w + s, sum, dc && dn);
                // horizontalAccumulate12 looks up wp[stride] after the sum, the others *wp
                let (code, dcode) = if fmt == FMT_12BITPICIO {
                    (sum, dc && dn)
                } else {
                    (cur, dc)
                };
                put(o, a.fl, fmt, t, at, w, (code & mask) as usize, dcode, true);
                w += 1;
            }
            n = n.wrapping_sub(stride);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_are_the_same_with_either_exp() {
        let (a, b) = (tables(false), tables(true));
        assert_eq!(a.to_16, b.to_16);
        assert_eq!(a.to_8, b.to_8);
        for i in 0..TSIZEP1 {
            assert_eq!(a.to_f[i].to_bits(), b.to_f[i].to_bits(), "{i}");
        }
        assert_eq!(a.to_16[1250], 65535);
        assert_eq!(a.to_8[0], 0);
    }
}
