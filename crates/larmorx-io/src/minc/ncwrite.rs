// SPDX-License-Identifier: Apache-2.0 AND NetCDF
//! netCDF 3.6.0-p1's classic-format writer as MINC 1 writes through it (UCAR/Unidata's notice:
//! `LICENSES/NetCDF.txt`; the files in `PROVENANCE.md`): `nc_create` with posixio's new file
//! (`nc.c`, `posixio.c`), the definitions (`dim.c`, `var.c`, `attr.c`, `string.c`), the end of
//! the definitions (`NC_begins`, the header writer of `v1hpg.c`, the fill of `putget.c`), the
//! hyperslab writes in each variable's own type (`putget.c`, `ncx.c`), the close (`NC_sync`,
//! `nc_abort`) and the version 2 entry points with their messages (`v2i.c`).
//!
//! The file is written through posixio's two-block buffer call by call (`netcdf`'s `Px`),
//! so the bytes on disk are posixio's at every step, an interrupted write included (a program that
//! ends without closing leaves what the buffer had paged out). `NC_SHARE`'s unbuffered I/O is not
//! ported (MINC never asks for it): a file created with it is buffered as without it.

use larmorx_core::cfmt::Streams;

use super::netcdf::{
    self as nc, Att, Dim, EINVAL, NC_BYTE, NC_CHAR, NC_DOUBLE, NC_EBADDIM, NC_EBADID, NC_EBADTYPE,
    NC_EINVAL, NC_ENOTVAR, NC_FLOAT, NC_GLOBAL, NC_INT, NC_SHORT, NcFile, Px, Source, Values, Var,
    X_INT_MAX, X_UINT_MAX, advise, cstr,
};
use super::{Exit, Session};

/// `NC_CLOBBER`.
pub const NC_CLOBBER: i32 = 0;
/// `NC_NOCLOBBER`.
pub const NC_NOCLOBBER: i32 = 0x4;
/// `NC_64BIT_OFFSET`: version 2 (64-bit offsets).
pub const NC_64BIT_OFFSET: i32 = 0x0200;
/// `NC_SHARE`.
pub const NC_SHARE: i32 = 0x0800;
/// `NC_UNLIMITED`: the size of the record dimension.
pub const NC_UNLIMITED: i64 = 0;

pub const NC_EEXIST: i32 = -35;
pub const NC_EPERM: i32 = -37;
pub const NC_ENOTINDEFINE: i32 = -38;
pub const NC_EINDEFINE: i32 = -39;
pub const NC_EMAXDIMS: i32 = -41;
pub const NC_ENAMEINUSE: i32 = -42;
pub const NC_EMAXATTS: i32 = -44;
pub const NC_EMAXVARS: i32 = -48;
pub const NC_EMAXNAME: i32 = -53;
pub const NC_EUNLIMIT: i32 = -54;
pub const NC_EBADNAME: i32 = -59;
pub const NC_ERANGE: i32 = -60;
pub const NC_EVARSIZE: i32 = -62;
pub const NC_EDIMSIZE: i32 = -63;

const NC_MAX_DIMS: usize = 512;
const NC_MAX_ATTRS: u64 = 4096;
const NC_MAX_VARS: usize = 4096;
const NC_MAX_NAME: usize = 128;
/// `MIN_NC_XSZ`: the size of an empty header.
const MIN_NC_XSZ: u64 = 32;
/// `X_OFF_MAX` of a version 1 file.
const X_OFF_MAX: i64 = 2_147_483_647;
/// `ERANGE` (the errno `ncx_put_off_t` gives for a negative offset).
const ERANGE: i32 = 34;
/// `EEXIST`.
const EEXIST: i32 = 17;
/// `NFILL * X_SIZEOF_DOUBLE`: the bytes of fill values `fill_NC_var` copies at a time.
const XFILL: usize = 16 * 8;

const NC_DIMENSION: u32 = 10;
const NC_VARIABLE: u32 = 11;
const NC_ATTRIBUTE: u32 = 12;

/// The state of a file being written (`NC`'s flags and layout).
#[derive(Clone, Debug)]
pub(crate) struct WriteState {
    /// `NC_CREAT`: in its creation, until the definitions end.
    creat: bool,
    /// `NC_INDEF`.
    indef_flag: bool,
    /// `NC_HDIRTY`: the header changed in data mode.
    hdirty: bool,
    /// `NC_NDIRTY`: the record count changed.
    ndirty: bool,
    /// `NC_NOFILL`.
    pub(crate) nofill: bool,
    /// `ncp->xsz`: the header's size.
    xsz: u64,
    begin_var: i64,
    begin_rec: i64,
    /// `ncp->chunk`: posixio's block size.
    chunk: u64,
    /// 4, or 8 for a version 2 file.
    sizeof_off_t: u64,
}

impl WriteState {
    /// `NC_indef`.
    pub(crate) fn indef(&self) -> bool {
        self.creat || self.indef_flag
    }
}

/// `NC_check_name`: an empty name, a character other than a letter, a digit or one of
/// `_-+.:@()`, or more than 128 bytes.
fn check_name(name: &[u8]) -> Result<(), i32> {
    let name = cstr(name);
    if name.is_empty() {
        return Err(NC_EBADNAME);
    }
    for &c in name {
        // C's isalnum in the C locale: a byte of 0x80 or above is not a letter.
        if !c.is_ascii_alphanumeric() && !b"_-+.:@()".contains(&c) {
            return Err(NC_EBADNAME);
        }
    }
    if name.len() > NC_MAX_NAME {
        return Err(NC_EMAXNAME);
    }
    Ok(())
}

/// `nc_cktype`.
fn cktype(ty: i32) -> Result<(), i32> {
    if (NC_BYTE..=NC_DOUBLE).contains(&ty) {
        Ok(())
    } else {
        Err(NC_EBADTYPE)
    }
}

fn rndup4(n: u64) -> u64 {
    n.div_ceil(4) * 4
}

/// The system error number `open` gives for a file that cannot be created, as Linux gives it on
/// every system (the situation decides it where the system's own error differs).
fn create_errno(path: &std::path::Path, e: &std::io::Error) -> i32 {
    #[cfg(not(target_os = "linux"))]
    {
        if path.is_dir() {
            return 21;
        }
        let mut p = path.parent();
        while let Some(d) = p {
            if d.as_os_str().is_empty() {
                break;
            }
            if let Ok(m) = std::fs::metadata(d) {
                if !m.is_dir() {
                    return 20;
                }
                break;
            }
            p = d.parent();
        }
        if let Some(d) = path.parent()
            && !d.as_os_str().is_empty()
            && !d.exists()
        {
            return 2;
        }
    }
    let _ = path;
    crate::xfm::files::errno_of(e)
}

/// `nc_create` (`nc__create_mp`, `ncio_create`): the new file's id (its descriptor) or netCDF's
/// error (a system error number or a netCDF code).
pub(crate) fn nc_create(s: &mut Session, path: &[u8], cmode: i32) -> Result<i32, i32> {
    let path = cstr(path);
    let sizeof_off_t: u64 = if cmode & NC_64BIT_OFFSET != 0 { 8 } else { 4 };
    let xsz = MIN_NC_XSZ;
    if path.is_empty() {
        return Err(EINVAL);
    }
    let p = crate::xfm::files::to_path(path);
    let mut o = std::fs::OpenOptions::new();
    o.read(true).write(true);
    if cmode & NC_NOCLOBBER != 0 {
        o.create_new(true);
    } else {
        o.create(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o666);
    }
    let file = match o.open(&p) {
        Ok(f) => f,
        Err(e) => {
            let n = create_errno(&p, &e);
            return Err(if n == EEXIST { NC_EEXIST } else { n });
        }
    };
    let ncid = s.alloc_fd();
    let blksz = nc::block_size(&file);
    let mut px = Px::new_file(Source::File(file), blksz);
    // fgrow to the initial size (the empty header's): a long of zeros written at its end.
    let initialsz = xsz;
    if px.len < initialsz {
        let r = px.write_raw(initialsz - 8, &[0u8; 8]);
        if let Err(e) = r {
            s.free_fd(ncid);
            return Err(e);
        }
    }
    // The initial region (the header's 32 bytes) and the empty header written into it.
    let d = match px.get_rgn(0, xsz, true) {
        Ok(d) => d,
        Err(e) => {
            s.free_fd(ncid);
            return Err(e);
        }
    };
    let mut f = NcFile {
        ncid,
        path: path.to_vec(),
        px,
        version: if sizeof_off_t == 8 { 2 } else { 1 },
        numrecs: 0,
        recsize: 0,
        dims: Vec::new(),
        gatts: Vec::new(),
        vars: Vec::new(),
        negative_offset: false,
        wr: Some(WriteState {
            creat: true,
            indef_flag: false,
            hdirty: false,
            ndirty: false,
            nofill: false,
            xsz,
            begin_var: 0,
            begin_rec: 0,
            chunk: blksz,
            sizeof_off_t,
        }),
    };
    // ncx_put_NC with the region given (its offset is passed as sizeof_off_t).
    let r = {
        let mut ps = WStream {
            px: &mut f.px,
            offset: sizeof_off_t,
            extent: xsz,
            d: Some(d),
            pos: 0,
            version: f.version,
        };
        let r = put_header_items(&mut ps, &f.dims, &f.gatts, &f.vars, 0, sizeof_off_t);
        let rr = ps.release();
        r.and(rr)
    };
    if let Err(e) = r {
        s.free_fd(ncid);
        let _ = std::fs::remove_file(&p);
        return Err(e);
    }
    s.files.push(f);
    Ok(ncid)
}

/// The header stream (`v1hs`) for writing: a window of `extent` bytes at `offset` got with
/// `RGN_WRITE`, released as modified when the next one is needed.
struct WStream<'a> {
    px: &'a mut Px,
    offset: u64,
    extent: u64,
    /// The window's start in posixio's buffer (`base`), `None` before the first.
    d: Option<usize>,
    pos: u64,
    version: u8,
}

impl WStream<'_> {
    /// `fault_v1hs`.
    fn fault(&mut self, extent: u64) -> Result<(), i32> {
        if self.d.is_some() {
            let incr = self.pos;
            self.release()?;
            self.offset += incr;
        }
        if extent > self.extent {
            self.extent = extent;
        }
        self.d = Some(self.px.get_rgn(self.offset, self.extent, true)?);
        self.pos = 0;
        Ok(())
    }
    /// `check_v1hs`.
    fn check(&mut self, nextread: u64) -> Result<(), i32> {
        if self.d.is_some() && self.pos + nextread <= self.extent {
            return Ok(());
        }
        self.fault(nextread)
    }
    fn write(&mut self, b: &[u8]) {
        let at = self.d.expect("a window") + self.pos as usize;
        self.px.base[at..at + b.len()].copy_from_slice(b);
        self.pos += b.len() as u64;
    }
    /// `rel_v1hs`.
    fn release(&mut self) -> Result<(), i32> {
        if self.d.take().is_some() {
            self.px.rel(true)?;
        }
        Ok(())
    }
    /// `v1h_put_size_t`: a check of 4 (8 in version 2), then 4 bytes.
    fn put_size(&mut self, v: u64) -> Result<(), i32> {
        self.check(if self.version == 1 { 4 } else { 8 })?;
        self.write(&(v as u32).to_be_bytes());
        Ok(())
    }
    /// `v1h_put_nc_type` / `v1h_put_NCtype`.
    fn put_int(&mut self, v: u32) -> Result<(), i32> {
        self.check(4)?;
        self.write(&v.to_be_bytes());
        Ok(())
    }
    /// `v1h_put_NC_string`.
    fn put_string(&mut self, name: &[u8]) -> Result<(), i32> {
        let n = name.len() as u64;
        self.put_size(n)?;
        self.check(rndup4(n))?;
        self.write(name);
        let pad = (rndup4(n) - n) as usize;
        self.write(&[0u8; 3][..pad]);
        Ok(())
    }
    fn put_atts(&mut self, atts: &[Att]) -> Result<(), i32> {
        if atts.is_empty() {
            self.put_int(0)?;
            return self.put_size(0);
        }
        self.put_int(NC_ATTRIBUTE)?;
        self.put_size(atts.len() as u64)?;
        for a in atts {
            self.put_string(&a.name)?;
            self.put_int(a.ty as u32)?;
            self.put_size(a.nelems)?;
            // v1h_put_NC_attrV: copied in pieces of the stream's extent at its start.
            let perchunk = self.extent;
            let mut at = 0usize;
            let mut remaining = a.xvalue.len() as u64;
            loop {
                let nbytes = perchunk.min(remaining);
                self.check(nbytes)?;
                self.write(&a.xvalue[at..at + nbytes as usize]);
                at += nbytes as usize;
                remaining -= nbytes;
                if remaining == 0 {
                    break;
                }
            }
        }
        Ok(())
    }
}

/// The header after its magic number and record count (`v1h_put_NC_dimarray`,
/// `_attrarray`, `_vararray`), preceded by those two (`ncx_put_NC`).
fn put_header_items(
    ps: &mut WStream,
    dims: &[Dim],
    gatts: &[Att],
    vars: &[Var],
    numrecs: u64,
    sizeof_off_t: u64,
) -> Result<(), i32> {
    ps.write(&[b'C', b'D', b'F', ps.version]);
    ps.write(&(numrecs as u32).to_be_bytes());
    if dims.is_empty() {
        ps.put_int(0)?;
        ps.put_size(0)?;
    } else {
        ps.put_int(NC_DIMENSION)?;
        ps.put_size(dims.len() as u64)?;
        for dm in dims {
            ps.put_string(&dm.name)?;
            ps.put_size(dm.size)?;
        }
    }
    ps.put_atts(gatts)?;
    if vars.is_empty() {
        ps.put_int(0)?;
        ps.put_size(0)?;
    } else {
        ps.put_int(NC_VARIABLE)?;
        ps.put_size(vars.len() as u64)?;
        for v in vars {
            ps.put_string(&v.name)?;
            ps.put_size(v.dimids.len() as u64)?;
            ps.check(4 * v.dimids.len() as u64)?;
            for &id in &v.dimids {
                ps.write(&id.to_be_bytes());
            }
            ps.put_atts(&v.atts)?;
            ps.put_int(v.ty as u32)?;
            ps.put_size(v.len)?;
            ps.check(sizeof_off_t)?;
            if v.begin < 0 {
                return Err(ERANGE);
            }
            if sizeof_off_t == 4 {
                ps.write(&(v.begin as u32).to_be_bytes());
            } else {
                ps.write(&(v.begin as u64).to_be_bytes());
            }
        }
    }
    Ok(())
}

/// `ncx_len_NC_string`.
fn len_string(name: &[u8]) -> u64 {
    4 + rndup4(name.len() as u64)
}

fn len_atts(atts: &[Att]) -> u64 {
    8 + atts
        .iter()
        .map(|a| len_string(&a.name) + 8 + a.xvalue.len() as u64)
        .sum::<u64>()
}

/// `ncx_len_NC`.
fn header_len(f: &NcFile, sizeof_off_t: u64) -> u64 {
    let mut x = 8u64;
    x += 8 + f.dims.iter().map(|d| len_string(&d.name) + 4).sum::<u64>();
    x += len_atts(&f.gatts);
    x += 8 + f
        .vars
        .iter()
        .map(|v| {
            len_string(&v.name)
                + 4
                + 4 * v.dimids.len() as u64
                + len_atts(&v.atts)
                + 8
                + sizeof_off_t
        })
        .sum::<u64>();
    x
}

fn wstate(f: &mut NcFile) -> &mut WriteState {
    f.wr.as_mut().expect("a file being written")
}

/// `write_NC` (`ncx_put_NC` with no region given): the header written in windows of the header's
/// size (at most the block size; the block size, at most 4096, for an empty header).
fn write_header(f: &mut NcFile) -> Result<(), i32> {
    let w = f.wr.as_ref().expect("a file being written");
    let mut extent = w.xsz;
    if extent <= MIN_NC_XSZ {
        extent = w.chunk.min(4096);
    } else if extent > w.chunk {
        extent = w.chunk;
    }
    let sizeof_off_t = w.sizeof_off_t;
    let r = {
        let mut ps = WStream {
            px: &mut f.px,
            offset: 0,
            extent,
            d: None,
            pos: 0,
            version: f.version,
        };
        let r = match ps.fault(extent) {
            Ok(()) => {
                put_header_items(&mut ps, &f.dims, &f.gatts, &f.vars, f.numrecs, sizeof_off_t)
            }
            Err(e) => {
                return Err(e);
            }
        };
        let _ = ps.release();
        r
    };
    if r.is_ok() {
        let w = wstate(f);
        w.ndirty = false;
        w.hdirty = false;
    }
    r
}

/// `write_numrecs`.
fn write_numrecs(f: &mut NcFile) -> Result<(), i32> {
    let n = (f.numrecs as u32).to_be_bytes();
    f.px.put(4, &n)?;
    wstate(f).ndirty = false;
    Ok(())
}

/// `NC_sync`.
fn sync(f: &mut NcFile) -> Result<(), i32> {
    let w = f.wr.as_ref().expect("a file being written");
    if w.hdirty {
        return write_header(f);
    }
    if w.ndirty {
        return write_numrecs(f);
    }
    Ok(())
}

/// The external bytes of `NC_FILL_<type>`.
fn default_fill(ty: i32) -> Vec<u8> {
    match ty {
        NC_BYTE => vec![0x81],
        NC_CHAR => vec![0],
        NC_SHORT => (-32767i16).to_be_bytes().to_vec(),
        NC_INT => (-2_147_483_647i32).to_be_bytes().to_vec(),
        NC_FLOAT => 9.969_209_968_386_869e36_f32
            .to_bits()
            .to_be_bytes()
            .to_vec(),
        _ => 9.969_209_968_386_869e36_f64
            .to_bits()
            .to_be_bytes()
            .to_vec(),
    }
}

/// `fill_NC_var`: the variable's values (one record of a record variable) set to its fill value
/// (its `_FillValue` attribute, else netCDF's default), in pieces of at most one block.
fn fill_var(f: &mut NcFile, vi: usize, recno: u64) -> Result<(), i32> {
    let v = &f.vars[vi];
    let step = v.xsz as usize;
    let mut xfill = vec![0u8; XFILL];
    match v.atts.iter().find(|a| cstr(&a.name) == b"_FillValue") {
        Some(a) => {
            if a.ty != v.ty || a.nelems != 1 {
                return Err(NC_EBADTYPE);
            }
            for c in xfill.chunks_mut(step) {
                c.copy_from_slice(&a.xvalue[..step]);
            }
        }
        None => {
            let one = default_fill(v.ty);
            for c in xfill.chunks_mut(step) {
                c.copy_from_slice(&one);
            }
        }
    }
    let mut offset = v.begin as u64;
    if v.is_recvar() {
        offset = offset.wrapping_add(f.recsize.wrapping_mul(recno));
    }
    let mut remaining = v.len;
    let chunk = f.wr.as_ref().expect("a file being written").chunk;
    loop {
        let chunksz = remaining.min(chunk);
        let d = f.px.get_rgn(offset, chunksz, true)?;
        let mut at = d;
        let full = chunksz as usize / XFILL;
        for _ in 0..full {
            f.px.base[at..at + XFILL].copy_from_slice(&xfill);
            at += XFILL;
        }
        let rem = chunksz as usize % XFILL;
        f.px.base[at..at + rem].copy_from_slice(&xfill[..rem]);
        f.px.rel(true)?;
        remaining -= chunksz;
        if remaining == 0 {
            return Ok(());
        }
        offset += chunksz;
    }
}

/// `NC_check_vlen`: whether the variable's size in bytes fits the format.
fn check_vlen(v: &Var, vlen_max: u64) -> bool {
    let mut prod = v.xsz;
    let i0 = usize::from(v.is_recvar());
    for &s in &v.shape[i0.min(v.shape.len())..] {
        if prod == 0 || s > vlen_max / prod {
            return false;
        }
        prod = prod.wrapping_mul(s);
    }
    true
}

/// `NC_check_vlens`: one variable too big for the format is allowed, the last one of its kind.
fn check_vlens(f: &NcFile) -> Result<(), i32> {
    if f.vars.is_empty() {
        return Ok(());
    }
    let w = f.wr.as_ref().expect("a file being written");
    let vlen_max = if w.sizeof_off_t == 8 {
        X_UINT_MAX - 3
    } else {
        X_INT_MAX - 3
    };
    let mut large = 0;
    let mut recs = 0;
    let mut last = false;
    for v in &f.vars {
        if !v.is_recvar() {
            last = false;
            if !check_vlen(v, vlen_max) {
                large += 1;
                last = true;
            }
        } else {
            recs += 1;
        }
    }
    if large > 1 || (large == 1 && !last) {
        return Err(NC_EVARSIZE);
    }
    if recs > 0 {
        if large == 1 && last {
            return Err(NC_EVARSIZE);
        }
        large = 0;
        for v in &f.vars {
            if v.is_recvar() {
                last = false;
                if !check_vlen(v, vlen_max) {
                    large += 1;
                    last = true;
                }
            }
        }
        if large > 1 || (large == 1 && !last) {
            return Err(NC_EVARSIZE);
        }
    }
    Ok(())
}

/// `NC_begins` with no free space and no alignment (`nc_enddef`'s `0, 1, 0, 1`).
fn begins(f: &mut NcFile) -> Result<(), i32> {
    let sizeof_off_t = f.wr.as_ref().expect("a file being written").sizeof_off_t;
    let xsz = header_len(f, sizeof_off_t);
    wstate(f).xsz = xsz;
    if f.vars.is_empty() {
        return Ok(());
    }
    let w = wstate(f);
    if w.begin_var < xsz as i64 {
        w.begin_var = xsz as i64;
    }
    let mut index = w.begin_var;
    for v in f.vars.iter_mut().filter(|v| !v.is_recvar()) {
        if sizeof_off_t == 4 && !(0..=X_OFF_MAX).contains(&index) {
            return Err(NC_EVARSIZE);
        }
        v.begin = index;
        index = index.wrapping_add(v.len as i64);
    }
    let w = wstate(f);
    if w.begin_rec < index {
        w.begin_rec = index;
    }
    let mut index = w.begin_rec;
    let mut recsize: u64 = 0;
    let mut last: Option<usize> = None;
    for (i, v) in f.vars.iter_mut().enumerate() {
        if !v.is_recvar() {
            continue;
        }
        if sizeof_off_t == 4 && !(0..=X_OFF_MAX).contains(&index) {
            return Err(NC_EVARSIZE);
        }
        v.begin = index;
        index = index.wrapping_add(v.len as i64);
        recsize = recsize.wrapping_add(v.len);
        last = Some(i);
    }
    if let Some(i) = last
        && recsize == f.vars[i].len
    {
        recsize = f.vars[i].dsizes[0].wrapping_mul(f.vars[i].xsz);
    }
    f.recsize = recsize;
    if wstate(f).creat {
        f.numrecs = 0;
    }
    Ok(())
}

/// `NC_endef`: the layout, the header, the fill of a new file, the buffer written back.
fn endef(f: &mut NcFile) -> Result<(), i32> {
    check_vlens(f)?;
    begins(f)?;
    write_header(f)?;
    let w = f.wr.as_ref().expect("a file being written");
    if !w.nofill && w.creat {
        for vi in 0..f.vars.len() {
            if !f.vars[vi].is_recvar() {
                fill_var(f, vi, 0)?;
            }
        }
    }
    let w = wstate(f);
    w.creat = false;
    w.indef_flag = false;
    f.px.sync()
}

fn file_w(s: &mut Session, ncid: i32) -> Result<&mut NcFile, i32> {
    s.file_mut(ncid).ok_or(NC_EBADID)
}

/// `nc_inq_dimid`.
pub fn inq_dimid(s: &Session, ncid: i32, name: &[u8]) -> Result<i32, i32> {
    let f = s.file(ncid).ok_or(NC_EBADID)?;
    let name = cstr(name);
    f.dims
        .iter()
        .position(|d| cstr(&d.name) == name)
        .map(|i| i as i32)
        .ok_or(NC_EBADDIM)
}

/// `ncdimid`.
pub fn ncdimid(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    name: &[u8],
) -> Result<i32, Exit> {
    match inq_dimid(s, ncid, name) {
        Ok(v) => Ok(v),
        Err(e) => {
            advise(s, streams, "ncdimid", e, &format!("ncid {ncid}"))?;
            Ok(-1)
        }
    }
}

/// `nccreate`: the new file's id, or `-1` after its message.
pub fn nccreate(
    s: &mut Session,
    streams: &mut dyn Streams,
    path: &[u8],
    cmode: i32,
) -> Result<i32, Exit> {
    match nc_create(s, path, cmode) {
        Ok(id) => Ok(id),
        Err(e) => {
            let mut msg = b"filename \"".to_vec();
            msg.extend_from_slice(cstr(path));
            msg.push(b'"');
            nc::advise_bytes(s, streams, "nccreate", e, &msg)?;
            Ok(-1)
        }
    }
}

/// `nc_def_dim`.
fn def_dim(f: &mut NcFile, name: &[u8], size: u64) -> Result<i32, i32> {
    let w = f.wr.as_ref().ok_or(NC_EPERM)?;
    if !w.indef() {
        return Err(NC_ENOTINDEFINE);
    }
    check_name(name)?;
    let max = if w.sizeof_off_t == 8 {
        X_UINT_MAX - 3
    } else {
        X_INT_MAX - 3
    };
    if size > max {
        return Err(NC_EDIMSIZE);
    }
    if size == 0 && f.dims.iter().any(|d| d.size == 0) {
        return Err(NC_EUNLIMIT);
    }
    if f.dims.len() >= NC_MAX_DIMS {
        return Err(NC_EMAXDIMS);
    }
    if f.dims.iter().any(|d| cstr(&d.name) == cstr(name)) {
        return Err(NC_ENAMEINUSE);
    }
    f.dims.push(Dim {
        name: cstr(name).to_vec(),
        size,
    });
    Ok(f.dims.len() as i32 - 1)
}

/// `ncdimdef`: the dimension's id, or `-1` after its message (`length` cast to `size_t`).
pub fn ncdimdef(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    name: &[u8],
    length: i64,
) -> Result<i32, Exit> {
    let r = file_w(s, ncid).and_then(|f| def_dim(f, name, length as u64));
    match r {
        Ok(id) => Ok(id),
        Err(e) => {
            advise(s, streams, "ncdimdef", e, &format!("ncid {ncid}"))?;
            Ok(-1)
        }
    }
}

/// `nc_def_var` (`ndims` negative: C's count cast to `unsigned long`).
fn def_var(f: &mut NcFile, name: &[u8], ty: i32, ndims: i32, dimids: &[i32]) -> Result<i32, i32> {
    let w = f.wr.as_ref().ok_or(NC_EPERM)?;
    if !w.indef() {
        return Err(NC_ENOTINDEFINE);
    }
    check_name(name)?;
    cktype(ty)?;
    if ndims < 0 {
        return Err(NC_EINVAL);
    }
    let dimids = &dimids[..(ndims as usize).min(dimids.len())];
    if f.vars.len() >= NC_MAX_VARS {
        return Err(NC_EMAXVARS);
    }
    if f.vars.iter().any(|v| cstr(&v.name) == cstr(name)) {
        return Err(NC_ENAMEINUSE);
    }
    let mut v = Var {
        name: cstr(name).to_vec(),
        dimids: dimids.to_vec(),
        shape: Vec::new(),
        dsizes: Vec::new(),
        atts: Vec::new(),
        ty,
        xsz: 0,
        len: 0,
        begin: 0,
    };
    nc::var_shape(&mut v, &f.dims)?;
    f.vars.push(v);
    Ok(f.vars.len() as i32 - 1)
}

/// `ncvardef`: the variable's id, or `-1` after its message (`ndims` of `dimids`).
pub fn ncvardef(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    name: &[u8],
    ty: i32,
    ndims: i32,
    dimids: &[i32],
) -> Result<i32, Exit> {
    let r = file_w(s, ncid).and_then(|f| def_var(f, name, ty, ndims, dimids));
    match r {
        Ok(id) => Ok(id),
        Err(e) => {
            advise(s, streams, "ncvardef", e, &format!("ncid {ncid}"))?;
            Ok(-1)
        }
    }
}

/// The external bytes of an attribute's values (`ncx_pad_putn_<type>_<type>`: big-endian, zeros
/// to a multiple of 4 bytes).
fn att_bytes(v: &Values) -> Vec<u8> {
    let mut b = match v {
        Values::Char(x) => x.clone(),
        Values::Byte(x) => x.iter().map(|&c| c as u8).collect(),
        Values::Short(x) => x.iter().flat_map(|c| c.to_be_bytes()).collect(),
        Values::Int(x) => x.iter().flat_map(|c| c.to_be_bytes()).collect(),
        Values::Float(x) => x.iter().flat_map(|c| c.to_bits().to_be_bytes()).collect(),
        Values::Double(x) => x.iter().flat_map(|c| c.to_bits().to_be_bytes()).collect(),
    };
    let n = rndup4(b.len() as u64) as usize;
    b.resize(n, 0);
    b
}

/// `nc_put_att_<type>` with the values in the attribute's own type (as MINC always passes them):
/// a new attribute or one redefined in define mode; in data mode an attribute that keeps its size
/// or shrinks is rewritten in place and the header marked for writing.
fn put_att(f: &mut NcFile, varid: i32, name: &[u8], ty: i32, vals: &Values) -> Result<(), i32> {
    let Some(w) = f.wr.as_ref() else {
        return Err(NC_EPERM);
    };
    let indef = w.indef();
    let list = if varid == NC_GLOBAL {
        &mut f.gatts
    } else {
        match usize::try_from(varid).ok().and_then(|i| f.vars.get_mut(i)) {
            Some(v) => &mut v.atts,
            None => return Err(NC_ENOTVAR),
        }
    };
    let text = ty == NC_CHAR;
    if text {
        check_name(name)?;
    } else {
        cktype(ty)?;
    }
    let nelems = vals.len() as u64;
    if nelems > X_INT_MAX {
        return Err(NC_EINVAL);
    }
    let xvalue = att_bytes(vals);
    let found = list.iter().position(|a| cstr(&a.name) == cstr(name));
    match found {
        Some(i) if !indef => {
            if xvalue.len() > list[i].xvalue.len() {
                return Err(NC_ENOTINDEFINE);
            }
            let a = &mut list[i];
            a.ty = ty;
            a.nelems = nelems;
            a.xvalue = xvalue;
            wstate(f).hdirty = true;
            return Ok(());
        }
        Some(_) => {}
        None => {
            if !indef {
                return Err(NC_ENOTINDEFINE);
            }
            if list.len() as u64 >= NC_MAX_ATTRS {
                return Err(NC_EMAXATTS);
            }
        }
    }
    if !text {
        check_name(name)?;
    }
    let a = Att {
        name: cstr(name).to_vec(),
        ty,
        nelems,
        xvalue,
    };
    match found {
        Some(i) => list[i] = a,
        None => list.push(a),
    }
    Ok(())
}

/// `ncattput` (the values in the attribute's type `ty`): `0`, or `-1` after its message.
pub fn ncattput(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    varid: i32,
    name: &[u8],
    ty: i32,
    vals: &Values,
) -> Result<i32, Exit> {
    let r = file_w(s, ncid).and_then(|f| {
        if f.wr.is_none() {
            return Err(NC_EPERM);
        }
        put_att(f, varid, name, ty, vals)
    });
    match r {
        Ok(()) => Ok(0),
        Err(e) => {
            advise(s, streams, "ncattput", e, &format!("ncid {ncid}"))?;
            Ok(-1)
        }
    }
}

/// `ncendef`: `0`, or `-1` after its message.
pub fn ncendef(s: &mut Session, streams: &mut dyn Streams, ncid: i32) -> Result<i32, Exit> {
    let r = file_w(s, ncid).and_then(|f| match f.wr.as_ref() {
        Some(w) if w.indef() => endef(f),
        _ => Err(NC_ENOTINDEFINE),
    });
    match r {
        Ok(()) => Ok(0),
        Err(e) => {
            advise(s, streams, "ncendef", e, &format!("ncid {ncid}"))?;
            Ok(-1)
        }
    }
}

/// `NCvnrecs`: the records up to `numrecs` filled.
fn vnrecs(f: &mut NcFile, numrecs: u64) -> Result<(), i32> {
    if numrecs > f.numrecs {
        wstate(f).ndirty = true;
        if f.wr.as_ref().expect("a file being written").nofill {
            f.numrecs = numrecs;
        } else {
            while f.numrecs < numrecs {
                let cur = f.numrecs;
                for vi in 0..f.vars.len() {
                    if f.vars[vi].is_recvar() {
                        fill_var(f, vi, cur)?;
                    }
                }
                f.numrecs = cur + 1;
            }
        }
    }
    Ok(())
}

/// `putNCvx_<type>_<type>`: `nelems` values (external bytes) at `start`, in pieces of at most one
/// block.
fn put_run(f: &mut NcFile, v: &Var, start: &[u64], nelems: u64, x: &[u8]) -> Result<(), i32> {
    if nelems == 0 {
        return Ok(());
    }
    let mut offset = nc::var_offset(f, v, start) as u64;
    let mut remaining = v.xsz.wrapping_mul(nelems);
    let chunk = f.wr.as_ref().expect("a file being written").chunk;
    let mut at = 0usize;
    loop {
        let extent = remaining.min(chunk);
        f.px.put(offset, &x[at..at + extent as usize])?;
        at += extent as usize;
        remaining -= extent;
        if remaining == 0 {
            return Ok(());
        }
        offset += extent;
    }
}

/// `nc_put_vara` in the variable's own type: `x` the external bytes of the hyperslab in C order.
fn put_vara(f: &mut NcFile, varid: i32, start: &[u64], edges: &[u64], x: &[u8]) -> Result<(), i32> {
    let Some(w) = f.wr.as_ref() else {
        return Err(NC_EPERM);
    };
    if w.indef() {
        return Err(NC_EINDEFINE);
    }
    let vi = usize::try_from(varid)
        .ok()
        .filter(|&i| i < f.vars.len())
        .ok_or(NC_ENOTVAR)?;
    let v = f.vars[vi].clone();
    let n = v.shape.len();
    let start = &start[..n.min(start.len())];
    let edges = &edges[..n.min(edges.len())];
    nc::coord_check(f, &v, start)?;
    nc::edge_check(&v, start, edges)?;
    if n == 0 {
        return put_run(f, &v, start, 1, x);
    }
    if v.is_recvar() {
        vnrecs(f, start[0].wrapping_add(edges[0]))?;
        if n == 1 && f.recsize <= v.len {
            return put_run(f, &v, start, edges[0], x);
        }
    }
    let (ii, count) = nc::iocount(f, &v, edges);
    if ii == -1 {
        return put_run(f, &v, start, count, x);
    }
    let index = ii as usize;
    let mut coord = start.to_vec();
    let upper: Vec<u64> = start.iter().zip(edges).map(|(s, e)| s + e).collect();
    let step = (count * v.xsz) as usize;
    let mut at = 0usize;
    while coord[0] < upper[0] {
        put_run(f, &v, &coord, count, &x[at.min(x.len())..])?;
        at += step;
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
    Ok(())
}

/// The external bytes of `vals` in netCDF type `ty` (the values of a variable of that type).
pub(crate) fn var_bytes(vals: &super::libsrc::Typed) -> Vec<u8> {
    use super::libsrc::Typed;
    match vals {
        Typed::U8(x) => x.clone(),
        Typed::I8(x) => x.iter().map(|&c| c as u8).collect(),
        Typed::U16(x) => x.iter().flat_map(|c| c.to_be_bytes()).collect(),
        Typed::I16(x) => x.iter().flat_map(|c| c.to_be_bytes()).collect(),
        Typed::U32(x) => x.iter().flat_map(|c| c.to_be_bytes()).collect(),
        Typed::I32(x) => x.iter().flat_map(|c| c.to_be_bytes()).collect(),
        Typed::F32(x) => x.iter().flat_map(|c| c.to_bits().to_be_bytes()).collect(),
        Typed::F64(x) => x.iter().flat_map(|c| c.to_bits().to_be_bytes()).collect(),
    }
}

/// `ncvarput`: the hyperslab at `start` / `count` written from `x` (the variable's own type's
/// external bytes); `0`, or `-1` after its message.
pub fn ncvarput(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    varid: i32,
    start: &[i64],
    count: &[i64],
    x: &[u8],
) -> Result<i32, Exit> {
    let r = file_w(s, ncid).and_then(|f| {
        let st: Vec<u64> = start.iter().map(|&v| v as u64).collect();
        let ct: Vec<u64> = count.iter().map(|&v| v as u64).collect();
        put_vara(f, varid, &st, &ct, x)
    });
    match r {
        Ok(()) => Ok(0),
        Err(e) => {
            advise(s, streams, "ncvarput", e, &format!("ncid {ncid}"))?;
            Ok(-1)
        }
    }
}

/// `nc_put_var1` in the variable's own type.
fn put_var1(f: &mut NcFile, varid: i32, coord: &[u64], x: &[u8]) -> Result<(), i32> {
    let vi = usize::try_from(varid)
        .ok()
        .filter(|&i| i < f.vars.len())
        .ok_or(NC_ENOTVAR)?;
    let Some(w) = f.wr.as_ref() else {
        return Err(NC_EPERM);
    };
    if w.indef() {
        return Err(NC_EINDEFINE);
    }
    let v = f.vars[vi].clone();
    let coord = &coord[..v.shape.len().min(coord.len())];
    nc::coord_check(f, &v, coord)?;
    if v.is_recvar() {
        vnrecs(f, coord[0] + 1)?;
    }
    put_run(f, &v, coord, 1, x)
}

/// `ncvarput1`: one value at `index` written from `x`; `0`, or `-1` after its message.
pub fn ncvarput1(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
    varid: i32,
    index: &[i64],
    x: &[u8],
) -> Result<i32, Exit> {
    let r = file_w(s, ncid).and_then(|f| {
        let c: Vec<u64> = index.iter().map(|&v| v as u64).collect();
        put_var1(f, varid, &c, x)
    });
    match r {
        Ok(()) => Ok(0),
        Err(e) => {
            advise(s, streams, "ncvarput1", e, &format!("ncid {ncid}"))?;
            Ok(-1)
        }
    }
}

/// `nc_close` of a file being written: the definitions ended (a failure aborts the file: it is
/// written back and, new, removed), or the header or record count written; the buffer written
/// back; the file closed.
fn close_written(s: &mut Session, ncid: i32) -> Result<(), i32> {
    let i = s
        .files
        .iter()
        .position(|f| f.ncid == ncid && ncid >= 0)
        .ok_or(NC_EBADID)?;
    let mut status = Ok(());
    let indef = s.files[i]
        .wr
        .as_ref()
        .expect("a file being written")
        .indef();
    if indef {
        if let Err(e) = endef(&mut s.files[i]) {
            // nc_abort: a new file is written back, closed and removed.
            let mut f = s.files.remove(i);
            let unlink = f.wr.as_ref().expect("a file being written").creat;
            let _ = sync(&mut f);
            let _ = f.px.sync();
            let path = crate::xfm::files::to_path(&f.path);
            drop(f);
            if unlink {
                let _ = std::fs::remove_file(path);
            }
            s.free_fd(ncid);
            return Err(e);
        }
    } else {
        status = sync(&mut s.files[i]);
    }
    let mut f = s.files.remove(i);
    let _ = f.px.sync();
    drop(f);
    s.free_fd(ncid);
    status
}

/// `ncclose` of a file being written.
pub(crate) fn ncclose_written(
    s: &mut Session,
    streams: &mut dyn Streams,
    ncid: i32,
) -> Result<i32, Exit> {
    match close_written(s, ncid) {
        Ok(()) => Ok(0),
        Err(e) => {
            advise(s, streams, "ncclose", e, &format!("ncid {ncid}"))?;
            Ok(-1)
        }
    }
}
