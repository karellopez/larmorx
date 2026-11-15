// SPDX-License-Identifier: Apache-2.0 AND NetCDF
//! netCDF 3.6.0-p1's classic-format reader as MINC 1 reads through it (UCAR/Unidata's notice:
//! `LICENSES/NetCDF.txt`; the files in `PROVENANCE.md`): `nc_open` and the header parser
//! (`nc.c`, `v1hpg.c`, `dim.c`, `attr.c`, `var.c`), posixio's windowed reads (`posixio.c`), the
//! hyperslab reads in each variable's own type (`putget.c`, `ncx.c`), the error texts
//! (`error.c`) and the version 2 entry points with their messages (`v2i.c`).
//!
//! The version 2 entry points return what the C functions return (an id, `0`, or `-1` after an
//! error); a message is printed when [`Session::ncopts`] has `NC_VERBOSE`, and an error with
//! `NC_FATAL` ends the program: [`Exit`].

use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{self as d};

use super::{Exit, Session};

pub const NC_FATAL: i32 = 1;
pub const NC_VERBOSE: i32 = 2;
pub const NC_NOWRITE: i32 = 0;
pub const NC_GLOBAL: i32 = -1;
pub const NC_NOERR: i32 = 0;

pub const NC_BYTE: i32 = 1;
pub const NC_CHAR: i32 = 2;
pub const NC_SHORT: i32 = 3;
pub const NC_INT: i32 = 4;
pub const NC_FLOAT: i32 = 5;
pub const NC_DOUBLE: i32 = 6;

pub const NC_EBADID: i32 = -33;
pub const NC_EINVAL: i32 = -36;
pub const NC_EINVALCOORDS: i32 = -40;
pub const NC_ENOTATT: i32 = -43;
pub const NC_EBADTYPE: i32 = -45;
pub const NC_EBADDIM: i32 = -46;
pub const NC_EUNLIMPOS: i32 = -47;
pub const NC_ENOTVAR: i32 = -49;
pub const NC_ENOTNC: i32 = -51;
pub const NC_ECHAR: i32 = -56;
pub const NC_EEDGE: i32 = -57;
pub const NC_ENOMEM: i32 = -61;
/// `NC_SYSERR`: what `ncerr` becomes after a system error.
pub const NC_SYSERR: i32 = -31;

/// The C library's error numbers the reader meets (glibc's values).
const EINVAL: i32 = 22;
const E2BIG: i32 = 7;
const EISDIR: i32 = 21;

/// `X_UINT_MAX`.
const X_UINT_MAX: u64 = 4_294_967_295;
/// `X_INT_MAX`.
const X_INT_MAX: u64 = 2_147_483_647;
/// `NC_MAX_VAR_DIMS`: the size of the dimension arrays MINC and volume_io pass to `ncvarinq`.
pub const MAX_VAR_DIMS: usize = 512;
/// `MAX_NC_NAME`.
pub const MAX_NC_NAME: usize = 128;

/// More list entries than this in one header (with the zeros read past the end of a file, a
/// count netCDF parses until its memory runs out): refused, as an allocation that failed
/// (FS-778).
const MAX_HEADER_ELEMENTS: u64 = 1 << 24;
/// An attribute larger than this that also reaches this far past the end of the file: refused
/// as an allocation that failed (FS-778).
const MAX_ATT_BYTES: u64 = 1 << 28;

/// `nc_strerror`.
pub fn strerror(err: i32) -> String {
    if err > 0 {
        // glibc's texts (xfm's table holds the file errors).
        let extra = match err {
            3 => Some("No such process"),
            7 => Some("Argument list too long"),
            8 => Some("Exec format error"),
            10 => Some("No child processes"),
            15 => Some("Block device required"),
            18 => Some("Invalid cross-device link"),
            25 => Some("Inappropriate ioctl for device"),
            29 => Some("Illegal seek"),
            31 => Some("Too many links"),
            32 => Some("Broken pipe"),
            _ => None,
        };
        return match extra {
            Some(s) => s.to_string(),
            None => crate::xfm::files::strerror(err),
        };
    }
    match err {
        0 => "No error",
        -33 => "Not a netCDF id",
        -34 => "Too many netCDF files open",
        -35 => "netCDF file exists && NC_NOCLOBBER",
        -36 => "Invalid argument",
        -37 => "Write to read only",
        -38 => "Operation not allowed in data mode",
        -39 => "Operation not allowed in define mode",
        -40 => "Index exceeds dimension bound",
        -41 => "NC_MAX_DIMS exceeded",
        -42 => "String match to name in use",
        -43 => "Attribute not found",
        -44 => "NC_MAX_ATTRS exceeded",
        -45 => "Not a netCDF data type or _FillValue type mismatch",
        -46 => "Invalid dimension id or name",
        -47 => "NC_UNLIMITED in the wrong index",
        -48 => "NC_MAX_VARS exceeded",
        -49 => "Variable not found",
        -50 => "Action prohibited on NC_GLOBAL varid",
        -51 => "Not a netCDF file",
        -52 => "In Fortran, string too short",
        -53 => "NC_MAX_NAME exceeded",
        -54 => "NC_UNLIMITED size already in use",
        -55 => "nc_rec op when there are no record vars",
        -56 => "Attempt to convert between text & numbers",
        -57 => "Edge+start exceeds dimension bound",
        -58 => "Illegal stride",
        -59 => "Attribute or variable name contains illegal characters",
        -60 => "Numeric conversion not representable",
        -61 => "Memory allocation (malloc) failure",
        -62 => "One or more variable sizes violate format constraints",
        -63 => "Invalid dimension size",
        _ => "Unknown Error",
    }
    .to_string()
}

/// `ncx_szof` (0 for a type netCDF does not know).
pub fn type_size(ty: i32) -> usize {
    match ty {
        NC_BYTE | NC_CHAR => 1,
        NC_SHORT => 2,
        NC_INT | NC_FLOAT => 4,
        NC_DOUBLE => 8,
        _ => 0,
    }
}

/// `nctypelen`: the size of a type, or `-1` with its message (`nctypelen: Bad type`).
pub fn nctypelen(s: &mut Session, streams: &mut dyn Streams, ty: i32) -> Result<i32, Exit> {
    match type_size(ty) {
        0 => {
            advise(
                s,
                streams,
                "nctypelen",
                NC_EBADTYPE,
                &format!("Unknown type {ty}"),
            )?;
            Ok(-1)
        }
        n => Ok(n as i32),
    }
}

/// A dimension (`NC_dim`): its name as stored (bytes; C reads it up to its first NUL) and size.
#[derive(Clone, Debug)]
pub struct Dim {
    pub name: Vec<u8>,
    pub size: u64,
}

/// An attribute (`NC_attr`): its type as stored (netCDF does not check it), element count and
/// external bytes (`xsz` of them: none for a type netCDF does not know).
#[derive(Clone, Debug)]
pub struct Att {
    pub name: Vec<u8>,
    pub ty: i32,
    pub nelems: u64,
    pub xvalue: Vec<u8>,
}

/// A variable (`NC_var`) with its shape as `NC_var_shape` computed it.
#[derive(Clone, Debug)]
pub struct Var {
    pub name: Vec<u8>,
    pub dimids: Vec<i32>,
    pub shape: Vec<u64>,
    pub dsizes: Vec<u64>,
    pub atts: Vec<Att>,
    pub ty: i32,
    pub xsz: u64,
    pub len: u64,
    pub begin: i64,
}

impl Var {
    /// `IS_RECVAR`.
    pub fn is_recvar(&self) -> bool {
        !self.shape.is_empty() && self.shape[0] == 0
    }
}

/// Where an open file's bytes come from.
pub(crate) enum Source {
    File(std::fs::File),
    Memory(Vec<u8>),
}

impl Source {
    fn len(&self) -> u64 {
        match self {
            Source::Memory(m) => m.len() as u64,
            Source::File(f) => f.metadata().map(|m| m.len()).unwrap_or(0),
        }
    }
    /// The bytes at `offset..offset+buf.len()`, zero past the end (`px_pgin`), or a system error.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), i32> {
        buf.fill(0);
        match self {
            Source::Memory(m) => {
                if (offset as u128) < m.len() as u128 {
                    let o = offset as usize;
                    let n = buf.len().min(m.len() - o);
                    buf[..n].copy_from_slice(&m[o..o + n]);
                }
                Ok(())
            }
            Source::File(f) => {
                use std::io::{Read, Seek, SeekFrom};
                if f.seek(SeekFrom::Start(offset)).is_err() {
                    return Err(EINVAL);
                }
                let mut at = 0;
                while at < buf.len() {
                    match f.read(&mut buf[at..]) {
                        Ok(0) => break,
                        Ok(n) => at += n,
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(e) => return Err(crate::xfm::files::errno_of(&e)),
                    }
                }
                Ok(())
            }
        }
    }
}

/// posixio's buffer (`ncio_px`, `px_get`, `px_pgin`): two blocks of the file, filled by `read`
/// and zeros past its end. Its state decides what a read past the end of a file gives: moving
/// forward one block when the count of valid bytes ends in the first keeps the old first block
/// under the new block's offset (bytes of an earlier block, FS-793), which `bytes` gives with
/// `reproduce` and corrects (zeros) otherwise.
pub(crate) struct Px {
    pub(crate) source: Source,
    pub(crate) blksz: u64,
    len: u64,
    bf_offset: Option<u64>,
    bf_extent: u64,
    bf_cnt: u64,
    base: Vec<u8>,
    /// Correct the reads past the end of the file (not `reproduce`).
    pub(crate) fix: bool,
    /// A read gave other bytes than the file's (zeros past its end) under `reproduce`, or would
    /// have without it.
    pub(crate) stale: bool,
}

impl Px {
    fn new(source: Source, blksz: u64) -> Px {
        let len = source.len();
        Px {
            source,
            blksz,
            len,
            bf_offset: None,
            bf_extent: 0,
            bf_cnt: 0,
            base: vec![0; 2 * blksz as usize],
            fix: true,
            stale: false,
        }
    }

    /// `ncio_px_sync` of a file opened read-only: the buffer invalidated.
    fn sync(&mut self) {
        self.bf_offset = None;
        self.bf_cnt = 0;
    }

    /// `px_pgin`: `extent` bytes at `offset` into the buffer at `at`; the count `read` gave.
    fn pgin(&mut self, offset: u64, extent: u64, at: usize) -> Result<u64, i32> {
        let buf = &mut self.base[at..at + extent as usize];
        self.source.read_at(offset, buf)?;
        Ok(self.len.saturating_sub(offset).min(extent))
    }

    /// `px_get`: the region's start in the buffer.
    fn get(&mut self, offset: u64, extent: u64) -> Result<usize, i32> {
        let b = self.blksz;
        let blkoffset = offset - offset % b;
        let mut diff = offset - blkoffset;
        let blkextent = (diff + extent).div_ceil(b) * b;
        if 2 * b < blkextent {
            return Err(E2BIG);
        }
        'done: {
            let Some(bo) = self.bf_offset else {
                self.bf_cnt = self.pgin(blkoffset, blkextent, 0)?;
                self.bf_offset = Some(blkoffset);
                self.bf_extent = blkextent;
                break 'done;
            };
            if blkoffset == bo {
                if blkextent > self.bf_extent {
                    self.bf_cnt = self.pgin(bo + b, b, b as usize)?;
                    self.bf_extent = 2 * b;
                    self.bf_cnt += b;
                }
                break 'done;
            }
            if self.bf_extent > b && blkoffset == bo + b {
                if blkextent == b {
                    diff += b;
                    break 'done;
                }
                if self.bf_cnt > b {
                    self.bf_cnt -= b;
                    let n = self.bf_cnt as usize;
                    self.base.copy_within(b as usize..b as usize + n, 0);
                }
                self.bf_offset = Some(blkoffset);
                self.bf_cnt = self.pgin(blkoffset + b, b, b as usize)?;
                self.bf_extent = 2 * b;
                self.bf_cnt += b;
                break 'done;
            }
            if blkoffset + b == bo {
                let mut upper_cnt = 0;
                if self.bf_cnt > b {
                    self.bf_cnt = b;
                    self.bf_extent = b;
                }
                if self.bf_cnt > 0 {
                    self.base.copy_within(0..b as usize, b as usize);
                    upper_cnt = self.bf_cnt;
                }
                self.bf_cnt = self.pgin(blkoffset, b, 0)?;
                self.bf_offset = Some(blkoffset);
                if upper_cnt != 0 {
                    self.bf_extent = 2 * b;
                    self.bf_cnt = b + upper_cnt;
                } else {
                    self.bf_extent = b;
                }
                break 'done;
            }
            self.bf_cnt = self.pgin(blkoffset, blkextent, 0)?;
            self.bf_offset = Some(blkoffset);
            self.bf_extent = blkextent;
        }
        if self.bf_cnt < extent + diff {
            self.bf_cnt = extent + diff;
        }
        Ok(diff as usize)
    }

    /// The bytes at `offset..offset+extent` as `px_get` gives them (with `fix`, the file's own,
    /// zeros past its end; a difference is noted in [`Px::stale`]).
    pub(crate) fn bytes(&mut self, offset: u64, extent: u64) -> Result<Vec<u8>, i32> {
        let d = self.get(offset, extent)?;
        let mut v = self.base[d..d + extent as usize].to_vec();
        if offset + extent > self.len {
            let mut want = vec![0u8; extent as usize];
            self.source.read_at(offset, &mut want)?;
            if want != v {
                self.stale = true;
                if self.fix {
                    v = want;
                }
            }
        }
        Ok(v)
    }
}

/// An open netCDF file (`NC`).
pub struct NcFile {
    pub ncid: i32,
    pub path: Vec<u8>,
    /// posixio's buffer over the file; its block size is the file system's preferred size, at
    /// least 8192 (`blksize`).
    pub(crate) px: Px,
    pub version: u8,
    pub numrecs: u64,
    pub recsize: u64,
    pub dims: Vec<Dim>,
    pub gatts: Vec<Att>,
    pub vars: Vec<Var>,
    /// A read met a negative offset (FS-776).
    pub(crate) negative_offset: bool,
}

/// The header stream (`v1hs`) over posixio's `get` (`px_get`): a window of `extent` bytes at
/// `offset`, zero past the end of the file; a window spanning more than two blocks is refused
/// (`E2BIG`).
struct Stream<'a> {
    src: &'a mut Px,
    src_len: u64,
    offset: u64,
    extent: u64,
    buf: Vec<u8>,
    pos: usize,
    have: bool,
}

impl Stream<'_> {
    fn fault(&mut self, extent: u64) -> Result<(), i32> {
        if self.have {
            self.offset += self.pos as u64;
        }
        if extent > self.extent {
            self.extent = extent;
        }
        self.buf = self.src.bytes(self.offset, self.extent)?;
        self.pos = 0;
        self.have = true;
        Ok(())
    }
    fn check(&mut self, nextread: u64) -> Result<(), i32> {
        if (self.pos as u64) + nextread <= self.buf.len() as u64 {
            return Ok(());
        }
        self.fault(nextread)
    }
    fn take(&mut self, n: usize) -> &[u8] {
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        s
    }
    fn get_u32(&mut self) -> Result<u32, i32> {
        self.check(4)?;
        let b = self.take(4);
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// `v1h_get_size_t`: 4 bytes (`ncx_get_size_t`) after a check of 4, or 8 in version 2.
    fn get_size(&mut self, version: u8) -> Result<u64, i32> {
        self.check(if version == 1 { 4 } else { 8 })?;
        let b = self.take(4);
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64)
    }
    fn get_string(&mut self, version: u8) -> Result<Vec<u8>, i32> {
        let nchars = self.get_size(version)?;
        let padded = nchars.div_ceil(4) * 4;
        self.check(padded)?;
        let v = self.take(nchars as usize).to_vec();
        self.pos += (padded - nchars) as usize;
        Ok(v)
    }
}

/// `ncx_len_NC_attrV`.
fn att_xsz(ty: i32, nelems: u64) -> u64 {
    let sz = type_size(ty) as u64;
    (sz.wrapping_mul(nelems)).div_ceil(4) * 4
}

fn get_atts(g: &mut Stream, version: u8, budget: &mut u64) -> Result<Vec<Att>, i32> {
    let tag = g.get_u32()?;
    let n = g.get_size(version)?;
    if n == 0 {
        return Ok(Vec::new());
    }
    if tag != 0x0C {
        return Err(EINVAL);
    }
    let mut v = Vec::new();
    for _ in 0..n {
        if *budget == 0 {
            return Err(NC_ENOMEM);
        }
        *budget -= 1;
        let name = g.get_string(version)?;
        g.check(4)?;
        let ty = {
            let b = g.take(4);
            i32::from_be_bytes([b[0], b[1], b[2], b[3]])
        };
        let nelems = g.get_size(version)?;
        let xsz = att_xsz(ty, nelems);
        if xsz > MAX_ATT_BYTES && g.offset + g.pos as u64 + xsz > g.src_len + MAX_ATT_BYTES {
            return Err(NC_ENOMEM);
        }
        // v1h_get_NC_attrV: copied in pieces of the stream's extent.
        let mut xvalue = Vec::with_capacity(xsz.min(1 << 20) as usize);
        let mut remaining = xsz;
        loop {
            let nget = g.extent.min(remaining);
            g.check(nget)?;
            xvalue.extend_from_slice(g.take(nget as usize));
            remaining -= nget;
            if remaining == 0 {
                break;
            }
        }
        v.push(Att {
            name,
            ty,
            nelems,
            xvalue,
        });
    }
    Ok(v)
}

/// `nc_get_NC` (with `NC_computeshapes`): the header, or netCDF's error.
fn read_header(f: &mut NcFile) -> Result<(), i32> {
    let src_len = f.px.len;
    let blksz = f.px.blksz;
    f.px.sync();
    let mut g = Stream {
        src: &mut f.px,
        src_len,
        offset: 0,
        extent: 0,
        buf: Vec::new(),
        pos: 0,
        have: false,
    };
    // The first window: the chunk (the block size), at most 4096 bytes.
    g.fault(blksz.min(4096))?;
    let magic = g.take(4).to_vec();
    if &magic[..3] != b"CDF" {
        return Err(NC_ENOTNC);
    }
    let version = match magic[3] {
        1 => 1,
        2 => 2,
        _ => return Err(NC_ENOTNC),
    };
    f.version = version;
    // The record count: read without a check (the first window holds it).
    {
        let b = g.take(4);
        f.numrecs = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64;
    }
    let mut budget = MAX_HEADER_ELEMENTS;
    // Dimensions.
    let tag = g.get_u32()?;
    let n = g.get_size(version)?;
    if n > 0 {
        if tag != 0x0A {
            return Err(EINVAL);
        }
        for _ in 0..n {
            if budget == 0 {
                return Err(NC_ENOMEM);
            }
            budget -= 1;
            let name = g.get_string(version)?;
            let size = g.get_size(version)?;
            f.dims.push(Dim { name, size });
        }
    }
    f.gatts = get_atts(&mut g, version, &mut budget)?;
    // Variables.
    let tag = g.get_u32()?;
    let n = g.get_size(version)?;
    if n > 0 {
        if tag != 0x0B {
            return Err(EINVAL);
        }
        for _ in 0..n {
            if budget == 0 {
                return Err(NC_ENOMEM);
            }
            budget -= 1;
            let name = g.get_string(version)?;
            let ndims = g.get_size(version)?;
            if ndims > budget {
                return Err(NC_ENOMEM);
            }
            g.check(4 * ndims)?;
            let mut dimids = Vec::with_capacity(ndims as usize);
            for _ in 0..ndims {
                let b = g.take(4);
                dimids.push(i32::from_be_bytes([b[0], b[1], b[2], b[3]]));
            }
            let atts = get_atts(&mut g, version, &mut budget)?;
            g.check(4)?;
            let ty = {
                let b = g.take(4);
                i32::from_be_bytes([b[0], b[1], b[2], b[3]])
            };
            // The stored size is read and replaced by NC_var_shape's.
            let _vsize = g.get_size(version)?;
            let begin = if version == 1 {
                g.check(4)?;
                let b = g.take(4);
                i32::from_be_bytes([b[0], b[1], b[2], b[3]]) as i64
            } else {
                g.check(8)?;
                let b = g.take(8);
                i64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
            };
            f.vars.push(Var {
                name,
                shape: Vec::new(),
                dsizes: Vec::new(),
                dimids,
                atts,
                ty,
                xsz: 0,
                len: 0,
                begin,
            });
        }
    }
    // NC_computeshapes.
    let mut first_rec: Option<usize> = None;
    let mut recsize: u64 = 0;
    for i in 0..f.vars.len() {
        var_shape(&mut f.vars[i], &f.dims)?;
        let v = &f.vars[i];
        if v.is_recvar() {
            if first_rec.is_none() {
                first_rec = Some(i);
            }
            recsize = recsize.wrapping_add(v.len);
        }
    }
    if let Some(r) = first_rec {
        let v = &f.vars[r];
        if recsize == v.len {
            recsize = v.dsizes[0].wrapping_mul(v.xsz);
        }
    }
    f.recsize = recsize;
    Ok(())
}

/// `NC_var_shape`.
fn var_shape(v: &mut Var, dims: &[Dim]) -> Result<(), i32> {
    v.xsz = type_size(v.ty) as u64;
    let mut product: u64 = 1;
    if !v.dimids.is_empty() {
        v.shape = Vec::with_capacity(v.dimids.len());
        for (i, &id) in v.dimids.iter().enumerate() {
            if id < 0 || id as usize >= dims.len().max(if dims.is_empty() { 1 } else { 0 }) {
                return Err(NC_EBADDIM);
            }
            if dims.is_empty() {
                return Err(NC_EBADDIM);
            }
            let s = dims[id as usize].size;
            if s == 0 && i != 0 {
                return Err(NC_EUNLIMPOS);
            }
            v.shape.push(s);
        }
        v.dsizes = vec![0; v.shape.len()];
        let rec = v.shape[0] == 0;
        for i in (0..v.shape.len()).rev() {
            if !(i == 0 && rec) {
                product = product.wrapping_mul(v.shape[i]);
            }
            v.dsizes[i] = product;
        }
    }
    v.len = if product != 0 && v.xsz <= X_UINT_MAX / product {
        product * v.xsz
    } else {
        // A product that wrapped to 0 divides by zero in netCDF (a crash): the length of a
        // variable too big for the format (FS-777).
        X_UINT_MAX
    };
    if matches!(v.ty, NC_BYTE | NC_CHAR | NC_SHORT) && !v.len.is_multiple_of(4) {
        v.len += 4 - v.len % 4;
    }
    Ok(())
}

/// The block size posixio uses for a file (`blksize`): the file system's preferred I/O size when
/// it is at least 8192, else 8192.
fn block_size(f: &std::fs::File) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(m) = f.metadata() {
            let b = m.blksize();
            if b >= 8192 {
                return b;
            }
        }
    }
    let _ = f;
    8192
}

/// `nc_open` for reading: the file's header, or netCDF's error (a system error number or a
/// netCDF code). `data` stands for the file's bytes when the caller has them (a decompressed
/// file).
pub(crate) fn nc_open(s: &mut Session, path: &[u8], data: Option<Vec<u8>>) -> Result<i32, i32> {
    if path.is_empty() {
        return Err(EINVAL);
    }
    let (source, blksz) = match data {
        Some(m) => (Source::Memory(m), 8192),
        None => {
            let p = crate::xfm::files::to_path(path);
            let f = match std::fs::File::open(&p) {
                Ok(f) => f,
                Err(e) => return Err(crate::xfm::files::errno_of(&e)),
            };
            if f.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                // open succeeds; the first read fails.
                return Err(EISDIR);
            }
            let b = block_size(&f);
            (Source::File(f), b)
        }
    };
    let ncid = s.alloc_fd();
    let mut px = Px::new(source, blksz);
    px.fix = !s.reproduce;
    let mut f = NcFile {
        ncid,
        path: path.to_vec(),
        px,
        version: 0,
        numrecs: 0,
        recsize: 0,
        dims: Vec::new(),
        gatts: Vec::new(),
        vars: Vec::new(),
        negative_offset: false,
    };
    let r = read_header(&mut f);
    if f.vars
        .iter()
        .any(|v| v.len == X_UINT_MAX && v.xsz != 0 && v.dsizes.first() == Some(&0))
    {
        s.pending.push((
            &d::FS_777,
            format!(
                "{}: a variable whose element count wraps to 0 read as too big for the format (netCDF divides by zero)",
                String::from_utf8_lossy(path)
            ),
        ));
    }
    match r {
        Ok(()) => {
            s.files.push(f);
            Ok(ncid)
        }
        Err(e) => {
            if e == NC_ENOMEM {
                s.pending.push((
                    &d::FS_778,
                    format!(
                        "{}: a header list longer than {MAX_HEADER_ELEMENTS} entries (or an attribute larger than the file) refused (netCDF reads on through zeros until its memory runs out)",
                        String::from_utf8_lossy(path)
                    ),
                ));
            }
            s.free_fd(ncid);
            Err(e)
        }
    }
}

/// `nc_advise`: `ncerr` set, the message printed with `NC_VERBOSE`, the program ended with
/// `NC_FATAL`.
pub fn advise(
    s: &mut Session,
    streams: &mut dyn Streams,
    routine: &str,
    err: i32,
    msg: &str,
) -> Result<(), Exit> {
    advise_bytes(s, streams, routine, err, msg.as_bytes())
}

/// [`advise`] of a message holding names (bytes as C prints them).
pub fn advise_bytes(
    s: &mut Session,
    streams: &mut dyn Streams,
    routine: &str,
    err: i32,
    msg: &[u8],
) -> Result<(), Exit> {
    s.ncerr = if err > 0 { NC_SYSERR } else { err };
    if s.ncopts & NC_VERBOSE != 0 {
        let mut t = format!("{routine}: ").into_bytes();
        t.extend_from_slice(msg);
        if err != NC_NOERR {
            t.extend_from_slice(b": ");
            t.extend_from_slice(strerror(err).as_bytes());
        }
        t.push(b'\n');
        streams.err_bytes(&t);
    }
    if s.ncopts & NC_FATAL != 0 && err != NC_NOERR {
        return Err(Exit { status: s.ncopts });
    }
    Ok(())
}

impl Session {
    pub(crate) fn file(&self, ncid: i32) -> Option<&NcFile> {
        if ncid < 0 {
            return None;
        }
        self.files.iter().find(|f| f.ncid == ncid)
    }
    fn file_mut(&mut self, ncid: i32) -> Option<&mut NcFile> {
        if ncid < 0 {
            return None;
        }
        self.files.iter_mut().find(|f| f.ncid == ncid)
    }
}

/// `ncopen`: `nc_open`'s id (the file descriptor), or `-1` after its message. `data` is the
/// file's content when the caller decompressed it.
pub fn ncopen(
    s: &mut Session,
    streams: &mut dyn Streams,
    path: &[u8],
    data: Option<Vec<u8>>,
) -> Result<i32, Exit> {
    match nc_open(s, path, data) {
        Ok(id) => Ok(id),
        Err(e) => {
            let mut msg = b"filename \"".to_vec();
            msg.extend_from_slice(cstr(path));
            msg.push(b'"');
            advise_bytes(s, streams, "ncopen", e, &msg)?;
            Ok(-1)
        }
    }
}

/// `ncclose`.
pub fn ncclose(s: &mut Session, streams: &mut dyn Streams, ncid: i32) -> Result<i32, Exit> {
    match s.files.iter().position(|f| f.ncid == ncid && ncid >= 0) {
        Some(i) => {
            s.files.remove(i);
            s.free_fd(ncid);
            Ok(0)
        }
        None => {
            advise(s, streams, "ncclose", NC_EBADID, &format!("ncid {ncid}"))?;
            Ok(-1)
        }
    }
}

/// A name as C compares it: up to its first NUL.
pub fn cstr(b: &[u8]) -> &[u8] {
    match b.iter().position(|&c| c == 0) {
        Some(i) => &b[..i],
        None => b,
    }
}

/// `nc_inq_varid`.
pub fn inq_varid(s: &Session, ncid: i32, name: &[u8]) -> Result<i32, i32> {
    let f = s.file(ncid).ok_or(NC_EBADID)?;
    let name = cstr(name);
    f.vars
        .iter()
        .position(|v| cstr(&v.name) == name)
        .map(|i| i as i32)
        .ok_or(NC_ENOTVAR)
}

/// `ncvarid`.
pub fn ncvarid(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    name: &[u8],
) -> Result<i32, Exit> {
    match inq_varid(s, ncid, name) {
        Ok(v) => Ok(v),
        Err(e) => {
            advise(s, streams, "ncvarid", e, &format!("ncid {ncid}"))?;
            Ok(-1)
        }
    }
}

/// What `ncvarinq` reports.
#[derive(Clone, Debug)]
pub struct VarInq {
    pub name: Vec<u8>,
    pub ty: i32,
    pub dimids: Vec<i32>,
    pub natts: usize,
}

/// `ncvarinq`: the variable, or `None` after its message.
pub fn ncvarinq(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    varid: i32,
) -> Result<Option<VarInq>, Exit> {
    let r = match s.file(ncid) {
        None => Err(NC_EBADID),
        Some(f) => match usize::try_from(varid).ok().and_then(|i| f.vars.get(i)) {
            None => Err(NC_ENOTVAR),
            Some(v) => Ok(VarInq {
                name: cstr(&v.name).to_vec(),
                ty: v.ty,
                dimids: v.dimids.clone(),
                natts: v.atts.len(),
            }),
        },
    };
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            advise(s, streams, "ncvarinq", e, &format!("ncid {ncid}"))?;
            Ok(None)
        }
    }
}

/// `ncdiminq`: the dimension's name (up to its first NUL) and its size as `ncdiminq` casts it
/// to `int`, or `None` after its message.
pub fn ncdiminq(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    dimid: i32,
) -> Result<Option<(Vec<u8>, i64)>, Exit> {
    let r = match s.file(ncid) {
        None => Err(NC_EBADID),
        Some(f) => match usize::try_from(dimid).ok().and_then(|i| f.dims.get(i)) {
            None => Err(NC_EBADDIM),
            Some(dm) => {
                let size = if dm.size == 0 { f.numrecs } else { dm.size };
                Ok((cstr(&dm.name).to_vec(), size as u32 as i32 as i64))
            }
        },
    };
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            advise(s, streams, "ncdiminq", e, &format!("ncid {ncid}"))?;
            Ok(None)
        }
    }
}

/// The attribute list of `varid` (`NC_attrarray0`).
fn att_list(f: &NcFile, varid: i32) -> Option<&[Att]> {
    if varid == NC_GLOBAL {
        Some(&f.gatts)
    } else {
        usize::try_from(varid)
            .ok()
            .and_then(|i| f.vars.get(i))
            .map(|v| v.atts.as_slice())
    }
}

/// `NC_lookupattr`.
pub fn lookup_att<'a>(s: &'a Session, ncid: i32, varid: i32, name: &[u8]) -> Result<&'a Att, i32> {
    let f = s.file(ncid).ok_or(NC_EBADID)?;
    let list = att_list(f, varid).ok_or(NC_ENOTVAR)?;
    let name = cstr(name);
    list.iter()
        .find(|a| cstr(&a.name) == name)
        .ok_or(NC_ENOTATT)
}

/// `ncattinq`: the attribute's type and length (cast to `int`), or `None` after its message.
pub fn ncattinq(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    varid: i32,
    name: &[u8],
) -> Result<Option<(i32, i32)>, Exit> {
    match lookup_att(s, ncid, varid, name) {
        Ok(a) => Ok(Some((a.ty, a.nelems as u32 as i32))),
        Err(e) => {
            let mut msg = format!("ncid {ncid}; varid {varid}; attname \"").into_bytes();
            msg.extend_from_slice(cstr(name));
            msg.push(b'"');
            advise_bytes(s, streams, "ncattinq", e, &msg)?;
            Ok(None)
        }
    }
}

/// A value of netCDF's types in the machine's order.
#[derive(Clone, Debug, PartialEq)]
pub enum Values {
    Byte(Vec<i8>),
    Char(Vec<u8>),
    Short(Vec<i16>),
    Int(Vec<i32>),
    Float(Vec<f32>),
    Double(Vec<f64>),
}

impl Values {
    pub fn len(&self) -> usize {
        match self {
            Values::Byte(v) => v.len(),
            Values::Char(v) => v.len(),
            Values::Short(v) => v.len(),
            Values::Int(v) => v.len(),
            Values::Float(v) => v.len(),
            Values::Double(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// External (big-endian) bytes of `ty` decoded (`ncx_getn_<type>_<type>`).
pub fn decode(ty: i32, x: &[u8], n: usize) -> Option<Values> {
    Some(match ty {
        NC_BYTE => Values::Byte(x[..n].iter().map(|&b| b as i8).collect()),
        NC_CHAR => Values::Char(x[..n].to_vec()),
        NC_SHORT => Values::Short(
            x[..2 * n]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| i16::from_be_bytes([c[0], c[1]]))
                .collect(),
        ),
        NC_INT => Values::Int(
            x[..4 * n]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| i32::from_be_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
        ),
        // The bits as stored: a signalling NaN stays one.
        NC_FLOAT => Values::Float(
            x[..4 * n]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f32::from_bits(u32::from_be_bytes([c[0], c[1], c[2], c[3]])))
                .collect(),
        ),
        NC_DOUBLE => Values::Double(
            x[..8 * n]
                .as_chunks::<8>()
                .0
                .iter()
                .map(|c| {
                    f64::from_bits(u64::from_be_bytes([
                        c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7],
                    ]))
                })
                .collect(),
        ),
        _ => return None,
    })
}

/// `ncattget`: the attribute's values in its own type, or `None` after its message.
pub fn ncattget(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    varid: i32,
    name: &[u8],
) -> Result<Option<Values>, Exit> {
    let r = match lookup_att(s, ncid, varid, name) {
        Err(e) => Err(e),
        Ok(a) => decode(a.ty, &a.xvalue, a.nelems as usize).ok_or(NC_EBADTYPE),
    };
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            advise(s, streams, "ncattget", e, &format!("ncid {ncid}"))?;
            Ok(None)
        }
    }
}

/// `NC_varoffset`.
fn var_offset(f: &NcFile, v: &Var, coord: &[u64]) -> i64 {
    let n = v.shape.len();
    if n == 0 {
        return v.begin;
    }
    if n == 1 {
        if v.is_recvar() {
            return v
                .begin
                .wrapping_add((coord[0] as i64).wrapping_mul(f.recsize as i64));
        }
        return v
            .begin
            .wrapping_add((coord[0] as i64).wrapping_mul(v.xsz as i64));
    }
    let mut lcoord = coord[n - 1] as i64;
    let rec = v.is_recvar();
    let (mut up, mut ip) = (1usize, 0usize);
    if rec {
        up += 1;
        ip += 1;
    }
    while up < n {
        lcoord = lcoord.wrapping_add((v.dsizes[up] as i64).wrapping_mul(coord[ip] as i64));
        up += 1;
        ip += 1;
    }
    lcoord = lcoord.wrapping_mul(v.xsz as i64);
    if rec {
        lcoord = lcoord.wrapping_add((coord[0] as i64).wrapping_mul(f.recsize as i64));
    }
    lcoord.wrapping_add(v.begin)
}

/// `NCcoordck` for a file opened read-only.
fn coord_check(f: &NcFile, v: &Var, coord: &[u64]) -> Result<(), i32> {
    if v.shape.is_empty() {
        return Ok(());
    }
    let mut i = 0;
    if v.is_recvar() {
        if coord[0] > X_INT_MAX || coord[0] >= f.numrecs {
            return Err(NC_EINVALCOORDS);
        }
        i = 1;
    }
    while i < v.shape.len() {
        if coord[i] >= v.shape[i] {
            return Err(NC_EINVALCOORDS);
        }
        i += 1;
    }
    Ok(())
}

/// `NCedgeck`.
fn edge_check(v: &Var, start: &[u64], edges: &[u64]) -> Result<(), i32> {
    if v.shape.is_empty() {
        return Ok(());
    }
    let i0 = usize::from(v.is_recvar());
    for i in i0..v.shape.len() {
        if edges[i] > v.shape[i] || start[i].wrapping_add(edges[i]) > v.shape[i] {
            return Err(NC_EEDGE);
        }
    }
    Ok(())
}

/// `getNCvx_<type>_<type>`: `nelems` values at `start`, in pieces of at most one block.
fn get_run(
    f: &mut NcFile,
    v: &Var,
    start: &[u64],
    nelems: u64,
    out: &mut Vec<u8>,
) -> Result<(), i32> {
    if nelems == 0 {
        return Ok(());
    }
    let mut offset = var_offset(f, v, start);
    let mut remaining = v.xsz.wrapping_mul(nelems);
    let chunk = f.px.blksz;
    loop {
        let extent = remaining.min(chunk);
        if offset < 0 {
            // A negative offset (a 32-bit begin of 2 GiB or more): netCDF reads memory beside
            // its buffer or fails (FS-776); refused.
            f.negative_offset = true;
            return Err(EINVAL);
        }
        let b = f.px.bytes(offset as u64, extent)?;
        out.extend_from_slice(&b);
        remaining -= extent;
        if remaining == 0 {
            return Ok(());
        }
        offset += extent as i64;
    }
}

/// `NCiocount`: the index of the dimension the odometer steps (`-1` for one run) and the run's
/// length.
fn iocount(f: &NcFile, v: &Var, edges: &[u64]) -> (i64, u64) {
    let n = v.shape.len();
    let mut edp0 = 0usize;
    if v.is_recvar() {
        if n == 1 && f.recsize <= v.len {
            return (0, edges[0]);
        }
        edp0 = 1;
    }
    let mut edp = n;
    while edp > edp0 {
        edp -= 1;
        if edges[edp] < v.shape[edp] {
            let mut z = edp;
            loop {
                if edges[z] == 0 {
                    return (edp as i64 - 1, 0);
                }
                if z == edp0 {
                    break;
                }
                z -= 1;
            }
            break;
        }
    }
    let mut count: u64 = 1;
    for &e in &edges[edp..n] {
        count = count.wrapping_mul(e);
    }
    (edp as i64 - 1, count)
}

/// `nc_get_vara` in the variable's own type: the external bytes of the hyperslab in C order,
/// read in netCDF's runs.
fn get_vara(f: &mut NcFile, varid: i32, start: &[u64], edges: &[u64]) -> Result<Vec<u8>, i32> {
    let vi = usize::try_from(varid)
        .ok()
        .filter(|&i| i < f.vars.len())
        .ok_or(NC_ENOTVAR)?;
    let v = f.vars[vi].clone();
    if type_size(v.ty) == 0 {
        return Err(NC_EBADTYPE);
    }
    let n = v.shape.len();
    let start = &start[..n.min(start.len())];
    let edges = &edges[..n.min(edges.len())];
    coord_check(f, &v, start)?;
    edge_check(&v, start, edges)?;
    let mut out = Vec::new();
    if n == 0 {
        get_run(f, &v, start, 1, &mut out)?;
        return Ok(out);
    }
    if v.is_recvar() {
        if start[0].wrapping_add(edges[0]) > f.numrecs {
            return Err(NC_EEDGE);
        }
        if n == 1 && f.recsize <= v.len {
            get_run(f, &v, start, edges[0], &mut out)?;
            return Ok(out);
        }
    }
    let (ii, count) = iocount(f, &v, edges);
    if ii == -1 {
        get_run(f, &v, start, count, &mut out)?;
        return Ok(out);
    }
    let index = ii as usize;
    let mut coord = start.to_vec();
    let upper: Vec<u64> = start.iter().zip(edges).map(|(s, e)| s + e).collect();
    while coord[0] < upper[0] {
        get_run(f, &v, &coord, count, &mut out)?;
        // odo1
        let mut k = index;
        loop {
            coord[k] += 1;
            if k == 0 || coord[k] < upper[k] {
                break;
            }
            coord[k] = start[k];
            k -= 1;
        }
    }
    Ok(out)
}

/// `ncvarget`: the hyperslab's values in the variable's own type, or `None` after its message.
pub fn ncvarget(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    varid: i32,
    start: &[i64],
    count: &[i64],
) -> Result<Option<Values>, Exit> {
    let r = match s.file_mut(ncid) {
        None => Err(NC_EBADID),
        Some(f) => {
            let st: Vec<u64> = start.iter().map(|&x| x as u64).collect();
            let ct: Vec<u64> = count.iter().map(|&x| x as u64).collect();
            match get_vara(f, varid, &st, &ct) {
                Err(e) => Err(e),
                Ok(bytes) => {
                    let v = &f.vars[varid as usize];
                    let n = bytes.len() / type_size(v.ty);
                    decode(v.ty, &bytes, n).ok_or(NC_EBADTYPE)
                }
            }
        }
    };
    if let Some(f) = s.file_mut(ncid)
        && f.px.stale
    {
        f.px.stale = false;
        let p = String::from_utf8_lossy(&f.path).into_owned();
        let fix = f.px.fix;
        s.pending.push((
            &d::FS_793,
            if fix {
                format!("{p}: values past the end of the file read as 0 (netCDF's buffer gives bytes of an earlier block)")
            } else {
                format!("{p}: values past the end of the file read from netCDF's buffer: bytes of an earlier block (reproduce)")
            },
        ));
    }
    if let Some(f) = s.file_mut(ncid)
        && f.negative_offset
    {
        f.negative_offset = false;
        let p = String::from_utf8_lossy(&f.path).into_owned();
        s.pending.push((
            &d::FS_776,
            format!("{p}: a variable at a negative offset (a version 1 begin of 2 GiB or more) refused with Invalid argument"),
        ));
    }
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            advise(
                s,
                streams,
                "ncvarget",
                e,
                &format!("ncid {ncid}; varid {varid}"),
            )?;
            Ok(None)
        }
    }
}
