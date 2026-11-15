// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
//! MINC 1.5.1's library functions on the reading path (`libsrc/minc_error.c`,
//! `netcdf_convenience.c`, `minc_convenience.c`, `value_conversion.c`; MNI's notice,
//! `LICENSES/LicenseRef-MNI.txt`): the error log with its routine name and call depth, `miopen`
//! with `miexpand_file`'s decompression, the attribute and range functions, and the type
//! conversion of `MI_convert_type`.
//!
//! The C functions return `MI_ERROR` (`-1`) or a value; here `Ok(None)` stands for `MI_ERROR` and
//! `Err(Exit)` for a fatal error that ends the program.

use larmorx_core::defects::{self as d};

use super::netcdf::{self as nc, NC_BYTE, NC_CHAR, NC_DOUBLE, NC_FLOAT, NC_INT, NC_SHORT, Values};
use super::{Ctx, Exit, PkgSite, Session};

pub const MI_ERROR: i32 = -1;
pub const MI_PRIV_DEFSIGN: i32 = 0;
pub const MI_PRIV_SIGNED: i32 = 1;
pub const MI_PRIV_UNSIGNED: i32 = 2;
pub const MI_MAX_ATTSTR_LEN: usize = 64;
pub const MI_SIGNED: &[u8] = b"signed__";
pub const MI_UNSIGNED: &[u8] = b"unsigned";
pub const MI_DEFAULT_MAX: f64 = 1.0;
pub const MI_DEFAULT_MIN: f64 = 0.0;
/// `MI_MAX_VAR_BUFFER_SIZE`.
pub const MI_MAX_VAR_BUFFER_SIZE: i64 = 10000;

pub const MI_ERR_NONNUMERIC: i32 = 1331;
pub const MI_ERR_NONCHAR: i32 = 1332;
pub const MI_ERR_NONSCALAR: i32 = 1333;
pub const MI_ERR_BADICV: i32 = 1339;
pub const MI_ERR_BADPROP: i32 = 1340;
pub const MI_ERR_ICVATTACHED: i32 = 1341;
pub const MI_ERR_TOOFEWDIMS: i32 = 1342;
pub const MI_ERR_ICVNOTATTACHED: i32 = 1343;
pub const MI_ERR_UNCOMPRESS: i32 = 1349;

/// `FLT_MAX` as a double.
pub const FLT_MAX: f64 = f32::MAX as f64;
/// `FILLVALUE_EPSILON`: `10.0 * FLT_EPSILON`.
pub const FILLVALUE_EPSILON: f64 = 10.0 * (f32::EPSILON as f64);

/// C's `MAX(x, y)`: `(x >= y) ? x : y` (a NaN `y` wins, a NaN `x` loses).
pub fn c_max(x: f64, y: f64) -> f64 {
    if x >= y { x } else { y }
}

/// C's `MIN(x, y)`: `(x <= y) ? x : y`.
pub fn c_min(x: f64, y: f64) -> f64 {
    if x <= y { x } else { y }
}

/// x86's `cvttsd2si` to 32 bits: truncation, `INT_MIN` for NaN or a value out of range.
pub fn cvt_i32(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// x86's `cvttsd2si` to 64 bits.
pub fn cvt_i64(x: f64) -> i64 {
    if x.is_nan() || x >= 9_223_372_036_854_775_808.0 || x < -9_223_372_036_854_775_808.0 {
        i64::MIN
    } else {
        x as i64
    }
}

/// A float widened as x86 widens it (`cvtss2sd`): a signalling NaN made quiet, the payload kept.
pub fn f32_to_f64(v: f32) -> f64 {
    if v.is_nan() {
        let b = v.to_bits() | 0x0040_0000;
        let sign = ((b >> 31) as u64) << 63;
        let mant = ((b & 0x007F_FFFF) as u64) << 29;
        f64::from_bits(sign | 0x7FF0_0000_0000_0000 | mant)
    } else {
        v as f64
    }
}

/// A double narrowed as x86 narrows it (`cvtsd2ss`): a NaN made quiet, the payload's top bits kept.
pub fn f64_to_f32(v: f64) -> f32 {
    if v.is_nan() {
        let b = v.to_bits();
        let sign = ((b >> 63) as u32) << 31;
        let mant = ((b >> 29) & 0x007F_FFFF) as u32 | 0x0040_0000;
        f32::from_bits(sign | 0x7F80_0000 | mant)
    } else {
        v as f32
    }
}

/// Values of netCDF's numeric types with MINC's signs: what MINC's conversions write.
#[derive(Clone, Debug, PartialEq)]
pub enum Typed {
    U8(Vec<u8>),
    I8(Vec<i8>),
    U16(Vec<u16>),
    I16(Vec<i16>),
    U32(Vec<u32>),
    I32(Vec<i32>),
    F32(Vec<f32>),
    F64(Vec<f64>),
}

impl Typed {
    /// An empty buffer of `ty` and `sign` (`MI_PRIV_*`, the type's default applied).
    pub fn new(ty: i32, sign: i32) -> Option<Typed> {
        let s = get_sign(ty, sign);
        Some(match (ty, s) {
            (NC_BYTE, MI_PRIV_UNSIGNED) => Typed::U8(Vec::new()),
            (NC_BYTE, _) => Typed::I8(Vec::new()),
            (NC_SHORT, MI_PRIV_UNSIGNED) => Typed::U16(Vec::new()),
            (NC_SHORT, _) => Typed::I16(Vec::new()),
            (NC_INT, MI_PRIV_UNSIGNED) => Typed::U32(Vec::new()),
            (NC_INT, _) => Typed::I32(Vec::new()),
            (NC_FLOAT, _) => Typed::F32(Vec::new()),
            (NC_DOUBLE, _) => Typed::F64(Vec::new()),
            _ => return None,
        })
    }
    pub fn len(&self) -> usize {
        match self {
            Typed::U8(v) => v.len(),
            Typed::I8(v) => v.len(),
            Typed::U16(v) => v.len(),
            Typed::I16(v) => v.len(),
            Typed::U32(v) => v.len(),
            Typed::I32(v) => v.len(),
            Typed::F32(v) => v.len(),
            Typed::F64(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The value at `i` as a C double (`(double)` of the stored value; a float as x86 widens it).
    pub fn get(&self, i: usize) -> f64 {
        match self {
            Typed::U8(v) => v[i] as f64,
            Typed::I8(v) => v[i] as f64,
            Typed::U16(v) => v[i] as f64,
            Typed::I16(v) => v[i] as f64,
            Typed::U32(v) => v[i] as f64,
            Typed::I32(v) => v[i] as f64,
            Typed::F32(v) => f32_to_f64(v[i]),
            Typed::F64(v) => v[i],
        }
    }
}

/// `MI_TO_DOUBLE`: a value of `ty` with `sign` (already resolved by [`get_sign`]) as a double;
/// `None` for a sign the macro has no case for (the value is left as it was: 0 here).
fn to_double(v: &Values, i: usize, sign: i32) -> f64 {
    match v {
        Values::Byte(b) => match sign {
            MI_PRIV_UNSIGNED => b[i] as u8 as f64,
            _ => b[i] as f64,
        },
        Values::Short(b) => match sign {
            MI_PRIV_UNSIGNED => b[i] as u16 as f64,
            _ => b[i] as f64,
        },
        Values::Int(b) => match sign {
            MI_PRIV_UNSIGNED => b[i] as u32 as f64,
            _ => b[i] as f64,
        },
        Values::Float(b) => f32_to_f64(b[i]),
        Values::Double(b) => b[i],
        Values::Char(b) => b[i] as f64,
    }
}

/// `ROUND` of `minc_useful.h`: `x + (x >= 0 ? 0.5 : -0.5)`.
fn round(x: f64) -> f64 {
    x + if x >= 0.0 { 0.5 } else { -0.5 }
}

/// A double that is not a number, converted to an integer type (FS-786): x86's value with
/// `reproduce` (`int_value`), else 0.
fn nan_int(c: &mut Ctx, x86: i64) -> i64 {
    if c.notices.reproduce() {
        if x86 != 0 {
            c.notices.add(
                &d::FS_786,
                "a value that is not a number converted to an integer (x86's value, reproduce)",
            );
        }
        x86
    } else {
        if x86 != 0 {
            c.notices.add(
                &d::FS_786,
                "a value that is not a number converted to 0 (MINC's conversion gives x86's integer indefinite)",
            );
        }
        0
    }
}

/// `MI_FROM_DOUBLE`: `d` clamped to the type's range, rounded (integers) and stored.
pub(crate) fn from_double(c: &mut Ctx, out: &mut Typed, dv: f64) {
    match out {
        Typed::U8(v) => {
            let x = round(c_min(255.0, c_max(0.0, dv)));
            let r = if x.is_nan() {
                nan_int(c, 0) as u8
            } else {
                cvt_i32(x) as u8
            };
            v.push(r);
        }
        Typed::I8(v) => {
            let x = round(c_min(127.0, c_max(-128.0, dv)));
            let r = if x.is_nan() {
                nan_int(c, 0) as i8
            } else {
                cvt_i32(x) as i8
            };
            v.push(r);
        }
        Typed::U16(v) => {
            let x = round(c_min(65535.0, c_max(0.0, dv)));
            let r = if x.is_nan() {
                nan_int(c, 0) as u16
            } else {
                cvt_i32(x) as u16
            };
            v.push(r);
        }
        Typed::I16(v) => {
            let x = round(c_min(32767.0, c_max(-32768.0, dv)));
            let r = if x.is_nan() {
                nan_int(c, 0) as i16
            } else {
                cvt_i32(x) as i16
            };
            v.push(r);
        }
        Typed::U32(v) => {
            let x = round(c_min(4_294_967_295.0, c_max(0.0, dv)));
            let r = if x.is_nan() {
                nan_int(c, 0) as u32
            } else {
                cvt_i64(x) as u32
            };
            v.push(r);
        }
        Typed::I32(v) => {
            let x = round(c_min(2_147_483_647.0, c_max(-2_147_483_648.0, dv)));
            let r = if x.is_nan() {
                nan_int(c, i32::MIN as i64) as i32
            } else {
                cvt_i32(x)
            };
            v.push(r);
        }
        Typed::F32(v) => {
            let x = c_min(FLT_MAX, c_max(-FLT_MAX, dv));
            v.push(f64_to_f32(x));
        }
        Typed::F64(v) => v.push(dv),
    }
}

/// `MI_get_sign`.
pub fn get_sign(ty: i32, sign: i32) -> i32 {
    if ty == NC_FLOAT || ty == NC_DOUBLE {
        MI_PRIV_SIGNED
    } else if sign == MI_PRIV_SIGNED || sign == MI_PRIV_UNSIGNED {
        sign
    } else if ty == NC_BYTE {
        MI_PRIV_UNSIGNED
    } else {
        MI_PRIV_SIGNED
    }
}

/// `MI_get_sign_from_string`.
pub fn sign_from_string(ty: i32, sign: Option<&[u8]>) -> i32 {
    let s = match sign {
        None | Some(b"") => MI_PRIV_DEFSIGN,
        Some(s) if s == MI_SIGNED => MI_PRIV_SIGNED,
        Some(s) if s == MI_UNSIGNED => MI_PRIV_UNSIGNED,
        Some(_) => MI_PRIV_DEFSIGN,
    };
    get_sign(ty, s)
}

/// The conversion a variable access applies (`do_scale`, `scale`, `offset`, `do_fillvalue`, the
/// fill value and the valid range checked): the parts of an ICV `MI_convert_type` reads.
#[derive(Clone, Copy, Debug)]
pub struct Conversion {
    pub do_scale: bool,
    pub scale: f64,
    pub offset: f64,
    pub do_fillvalue: bool,
    pub fillvalue: f64,
    pub fill_valid_min: f64,
    pub fill_valid_max: f64,
}

/// `MI_convert_type` from netCDF values in their own type (`insign` resolved) to `out`.
pub(crate) fn convert(
    c: &mut Ctx,
    vals: &Values,
    insign: i32,
    out: &mut Typed,
    n: usize,
    conv: Option<&Conversion>,
) {
    let insgn = get_sign(values_type(vals), insign);
    let (do_scale, do_fill) = conv.map_or((false, false), |k| (k.do_scale, k.do_fillvalue));
    // The copy when nothing is converted.
    if !do_scale && !do_fill && same_type(vals, insgn, out) {
        copy_raw(vals, out, n);
        return;
    }
    let (mut dmin, mut dmax, fill) = match conv {
        Some(k) => {
            let eps = ((k.fill_valid_max - k.fill_valid_min) * FILLVALUE_EPSILON).abs();
            (k.fill_valid_min - eps, k.fill_valid_max + eps, k.fillvalue)
        }
        None => (0.0, 0.0, 0.0),
    };
    if conv.is_none() {
        dmin = 0.0;
        dmax = 0.0;
    }
    for i in 0..n {
        let mut dv = to_double(vals, i, insgn);
        if do_fill && (dv < dmin || dv > dmax) {
            dv = fill;
        } else if do_scale {
            let k = conv.unwrap();
            dv = k.scale * dv + k.offset;
        }
        from_double(c, out, dv);
    }
}

pub(crate) fn values_type(v: &Values) -> i32 {
    match v {
        Values::Byte(_) => NC_BYTE,
        Values::Char(_) => NC_CHAR,
        Values::Short(_) => NC_SHORT,
        Values::Int(_) => NC_INT,
        Values::Float(_) => NC_FLOAT,
        Values::Double(_) => NC_DOUBLE,
    }
}

fn same_type(v: &Values, insgn: i32, out: &Typed) -> bool {
    matches!(
        (v, insgn, out),
        (Values::Byte(_), MI_PRIV_UNSIGNED, Typed::U8(_))
            | (Values::Byte(_), MI_PRIV_SIGNED, Typed::I8(_))
            | (Values::Short(_), MI_PRIV_UNSIGNED, Typed::U16(_))
            | (Values::Short(_), MI_PRIV_SIGNED, Typed::I16(_))
            | (Values::Int(_), MI_PRIV_UNSIGNED, Typed::U32(_))
            | (Values::Int(_), MI_PRIV_SIGNED, Typed::I32(_))
            | (Values::Float(_), _, Typed::F32(_))
            | (Values::Double(_), _, Typed::F64(_))
    )
}

fn copy_raw(v: &Values, out: &mut Typed, n: usize) {
    match (v, out) {
        (Values::Byte(a), Typed::U8(o)) => o.extend(a[..n].iter().map(|&x| x as u8)),
        (Values::Byte(a), Typed::I8(o)) => o.extend_from_slice(&a[..n]),
        (Values::Short(a), Typed::U16(o)) => o.extend(a[..n].iter().map(|&x| x as u16)),
        (Values::Short(a), Typed::I16(o)) => o.extend_from_slice(&a[..n]),
        (Values::Int(a), Typed::U32(o)) => o.extend(a[..n].iter().map(|&x| x as u32)),
        (Values::Int(a), Typed::I32(o)) => o.extend_from_slice(&a[..n]),
        (Values::Float(a), Typed::F32(o)) => o.extend_from_slice(&a[..n]),
        (Values::Double(a), Typed::F64(o)) => o.extend_from_slice(&a[..n]),
        _ => unreachable!("copy_raw of different types"),
    }
}

impl Session {
    /// `MI_SAVE_ROUTINE_NAME`.
    pub(crate) fn mi_enter(&mut self, name: &'static str) {
        if self.depth == 0 {
            self.routine = name;
            self.callers_ncopts = self.ncopts;
            self.ncopts &= !nc::NC_FATAL;
        }
        self.depth += 1;
    }

    /// `MI_RETURN`.
    pub(crate) fn mi_return(&mut self) {
        self.depth -= 1;
        if self.depth == 0 {
            self.ncopts = self.callers_ncopts;
        }
    }

    /// `MI_RETURN_ERROR`: at the outermost call, the caller's `ncopts` restored and the error
    /// logged (`MINC package entry point`).
    pub(crate) fn mi_return_error(&mut self, c: &mut Ctx) -> Result<(), Exit> {
        self.depth -= 1;
        if self.depth == 0 {
            self.ncopts = self.callers_ncopts;
            if c.dialect.logs_entry_point() {
                let e = self.ncerr;
                self.log_pkg_error(c, None, e, "MINC package entry point")?;
            }
        }
        Ok(())
    }

    /// `MI_log_pkg_error2` (the dialect's handling first, when `site` names one).
    pub(crate) fn log_pkg_error(
        &mut self,
        c: &mut Ctx,
        site: Option<PkgSite>,
        code: i32,
        msg: &str,
    ) -> Result<(), Exit> {
        if let Some(s) = site
            && let Some(e) = c.dialect.pkg_error(s, code, msg, c.streams)
        {
            return Err(e);
        }
        self.ncerr = code;
        if self.ncopts & nc::NC_VERBOSE != 0 {
            c.streams.err(&format!("{}: {msg}\n", self.routine));
        }
        if self.ncopts & nc::NC_FATAL != 0 && self.ncerr != 0 {
            return Err(Exit {
                status: self.ncopts,
            });
        }
        Ok(())
    }
}

/// A name that holds a character the shell splits at or interprets.
fn shell_special(name: &[u8]) -> bool {
    name.iter()
        .any(|ch| b" \t\n\"'`$\\;&|<>()*?[]#~={}!^%".contains(ch))
}

/// The compression types of `miexpand_file`, by extension.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Compress {
    Bzipped,
    Gzipped,
    Compressed,
    Packed,
    Zipped,
}

fn compress_type(ext: &[u8]) -> Option<Compress> {
    Some(match ext {
        b".bz" | b".bz2" => Compress::Bzipped,
        b".gz" => Compress::Gzipped,
        b".Z" => Compress::Compressed,
        b".z" => Compress::Packed,
        b".zip" => Compress::Zipped,
        _ => return None,
    })
}

const COMPRESSION_ENDINGS: [&[u8]; 6] = [b".bz", b".bz2", b".gz", b".Z", b".z", b".zip"];

/// Decompresses as `gunzip -c` (gzip members; for `.bz`/`.bz2` `bunzip2 -c`) would.
fn decompress(kind: Compress, data: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    match kind {
        Compress::Bzipped => {
            let mut out = Vec::new();
            let mut r = bzip2::read::MultiBzDecoder::new(data);
            r.read_to_end(&mut out).ok().map(|_| out)
        }
        _ => {
            let mut out = Vec::new();
            let mut rest = data;
            if rest.len() < 2 || rest[0] != 0x1f || rest[1] != 0x8b {
                return None;
            }
            while !rest.is_empty() {
                if rest.len() < 2 || rest[0] != 0x1f || rest[1] != 0x8b {
                    return None;
                }
                let mut g = flate2::bufread::GzDecoder::new(rest);
                g.read_to_end(&mut out).ok()?;
                rest = g.into_inner();
            }
            Some(out)
        }
    }
}

/// What `miexpand_file` gives `miopen`: the name to open again, and the decompressed bytes when
/// it decompressed (MINC writes them into a temporary file; here they stay in memory).
pub(crate) struct Expanded {
    pub name: Vec<u8>,
    pub data: Option<Vec<u8>>,
}

/// `miexpand_file` (`header_only` false): `None` is its error.
pub(crate) fn miexpand_file(
    s: &mut Session,
    c: &mut Ctx,
    path: &[u8],
) -> Result<Option<Expanded>, Exit> {
    s.mi_enter("miexpand_file");
    let old = s.ncopts;
    s.ncopts = 0;
    let status = nc::ncopen(s, c.streams, path, None)?;
    if status != -1 {
        nc::ncclose(s, c.streams, status)?;
    }
    s.ncopts = old;
    if status != -1 {
        s.mi_return();
        return Ok(Some(Expanded {
            name: path.to_vec(),
            data: None,
        }));
    }
    let mut first_ncerr = s.ncerr;
    let p = crate::xfm::files::to_path(path);
    if first_ncerr == nc::NC_NOERR && std::fs::File::open(&p).is_err() {
        first_ncerr = nc::NC_SYSERR;
    }
    let ext: &[u8] = match path.iter().rposition(|&ch| ch == b'.') {
        Some(i) => &path[i..],
        None => &[],
    };
    let mut kind = compress_type(ext);
    let mut source = path.to_vec();
    if first_ncerr == nc::NC_SYSERR && kind.is_none() {
        let mut found = None;
        for e in COMPRESSION_ENDINGS {
            let mut cand = path.to_vec();
            cand.extend_from_slice(e);
            if std::fs::File::open(crate::xfm::files::to_path(&cand)).is_ok() {
                found = Some((cand, compress_type(e)));
                break;
            }
        }
        match found {
            None => {
                s.mi_return();
                return Ok(Some(Expanded {
                    name: path.to_vec(),
                    data: None,
                }));
            }
            Some((cand, k)) => {
                source = cand;
                kind = k;
            }
        }
    } else if first_ncerr == nc::NC_SYSERR || kind.is_none() {
        s.mi_return();
        return Ok(Some(Expanded {
            name: path.to_vec(),
            data: None,
        }));
    }
    let kind = kind.expect("a compression type");
    let decompressed = if c.dialect.decompresses(c.notices) {
        if shell_special(&source) {
            c.notices.add(
                &d::FS_774,
                format!(
                    "{} decompressed in the process (MINC passes the name to the shell unquoted)",
                    String::from_utf8_lossy(&source)
                ),
            );
        }
        std::fs::read(crate::xfm::files::to_path(&source))
            .ok()
            .and_then(|data| decompress(kind, &data))
    } else {
        None
    };
    match decompressed {
        Some(data) => {
            s.mi_return();
            Ok(Some(Expanded {
                name: c.dialect.temporary_name(),
                data: Some(data),
            }))
        }
        None => {
            s.log_pkg_error(
                c,
                Some(PkgSite::Uncompress),
                MI_ERR_UNCOMPRESS,
                "Cannot uncompress the file",
            )?;
            s.mi_return_error(c)?;
            Ok(None)
        }
    }
}

/// `miopen` for reading (`NC_NOWRITE`): the file's id, or `None` (`MI_ERROR`).
pub fn miopen(s: &mut Session, c: &mut Ctx, path: &[u8]) -> Result<Option<i32>, Exit> {
    let r = miopen_inner(s, c, path);
    s.drain_notices(c.notices);
    r
}

fn miopen_inner(s: &mut Session, c: &mut Ctx, path: &[u8]) -> Result<Option<i32>, Exit> {
    s.reproduce = c.notices.reproduce();
    s.mi_enter("miopen");
    let old = s.ncopts;
    s.ncopts = 0;
    let status = nc::ncopen(s, c.streams, path, None)?;
    s.ncopts = old;
    if status != -1 {
        s.mi_return();
        return Ok(Some(status));
    }
    let Some(exp) = miexpand_file(s, c, path)? else {
        s.mi_return_error(c)?;
        return Ok(None);
    };
    let status = nc::ncopen(s, c.streams, &exp.name, exp.data)?;
    if status == -1 {
        s.mi_return_error(c)?;
        return Ok(None);
    }
    s.mi_return();
    Ok(Some(status))
}

/// `miclose`.
pub fn miclose(s: &mut Session, c: &mut Ctx, cdfid: i32) -> Result<i32, Exit> {
    s.mi_enter("miclose");
    let st = nc::ncclose(s, c.streams, cdfid)?;
    if st == MI_ERROR {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    s.mi_return();
    Ok(st)
}

/// `miattget_with_sign` asked for doubles (every caller on the reading path asks for
/// `NC_DOUBLE`): the values written into `value` (at most `max_length`) and the attribute's
/// length, or `None` (`MI_ERROR`).
pub fn miattget_with_sign(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    name: &[u8],
    insign: Option<&[u8]>,
    max_length: i32,
    value: &mut [f64],
) -> Result<Option<i32>, Exit> {
    s.mi_enter("miattget_with_sign");
    let Some((att_type, actual_length)) = nc::ncattinq(s, c.streams, cdfid, varid, name)? else {
        s.mi_return_error(c)?;
        return Ok(None);
    };
    if att_type == NC_CHAR {
        s.log_pkg_error(
            c,
            Some(PkgSite::AttNonNumeric),
            MI_ERR_NONNUMERIC,
            "Non-numeric datatype",
        )?;
        s.mi_return_error(c)?;
        return Ok(None);
    }
    if att_type == NC_DOUBLE && actual_length <= max_length {
        let Some(Values::Double(v)) = nc::ncattget(s, c.streams, cdfid, varid, name)? else {
            s.mi_return_error(c)?;
            return Ok(None);
        };
        // ncattget writes the whole attribute (at most max_length values here).
        for (i, x) in v.iter().enumerate() {
            if i < value.len() {
                value[i] = *x;
            }
        }
        s.mi_return();
        return Ok(Some(actual_length));
    }
    let tl = nc::nctypelen(s, c.streams, att_type)?;
    if tl < 0 && actual_length != 0 {
        // MALLOC of a negative size: the allocation fails.
        sys_error(s, c, PkgSite::AttGetAlloc, "miattget")?;
        s.mi_return_error(c)?;
        return Ok(None);
    }
    let Some(vals) = nc::ncattget(s, c.streams, cdfid, varid, name)? else {
        s.mi_return_error(c)?;
        return Ok(None);
    };
    let att_sign = sign_from_string(att_type, insign);
    let n = (max_length.min(actual_length)).max(0) as usize;
    let mut out = Typed::F64(Vec::new());
    convert(c, &vals, att_sign, &mut out, n.min(vals.len()), None);
    if let Typed::F64(v) = out {
        for (i, x) in v.iter().enumerate() {
            if i < value.len() {
                value[i] = *x;
            }
        }
    }
    s.mi_return();
    Ok(Some(actual_length))
}

/// `MI_log_sys_error1` after an allocation that failed (`errno` `ENOMEM`).
fn sys_error(s: &mut Session, c: &mut Ctx, site: PkgSite, p1: &str) -> Result<(), Exit> {
    if let Some(e) = c.dialect.pkg_error(site, nc::NC_SYSERR, p1, c.streams) {
        return Err(e);
    }
    if s.ncopts & nc::NC_VERBOSE != 0 {
        s.ncerr = nc::NC_SYSERR;
        c.streams
            .err(&format!("{}{p1}: Cannot allocate memory\n", s.routine));
    }
    if s.ncopts & nc::NC_FATAL != 0 {
        return Err(Exit { status: s.ncopts });
    }
    Ok(())
}

/// `miattget` (doubles).
pub fn miattget(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    name: &[u8],
    max_length: i32,
    value: &mut [f64],
) -> Result<Option<i32>, Exit> {
    s.mi_enter("miattget");
    match miattget_with_sign(s, c, cdfid, varid, name, None, max_length, value)? {
        None => {
            s.mi_return_error(c)?;
            Ok(None)
        }
        Some(n) => {
            s.mi_return();
            Ok(Some(n))
        }
    }
}

/// `miattget1` (a double): `Some(())` when it read one value.
pub fn miattget1(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    name: &[u8],
    value: &mut f64,
) -> Result<Option<()>, Exit> {
    s.mi_enter("miattget1");
    let mut v = [*value];
    let Some(len) = miattget(s, c, cdfid, varid, name, 1, &mut v)? else {
        *value = v[0];
        s.mi_return_error(c)?;
        return Ok(None);
    };
    *value = v[0];
    if len != 1 {
        s.log_pkg_error(
            c,
            Some(PkgSite::AttNonScalar),
            MI_ERR_NONSCALAR,
            "Attribute is not a scalar value",
        )?;
        s.mi_return_error(c)?;
        return Ok(None);
    }
    s.mi_return();
    Ok(Some(()))
}

/// `miattgetstr`: the string read into the caller's buffer `value` (`maxlen` bytes, C's
/// content before the call), returned as C reads it (up to its first NUL), or `None`.
pub fn miattgetstr(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    name: &[u8],
    maxlen: usize,
    value: &mut Vec<u8>,
) -> Result<Option<Vec<u8>>, Exit> {
    s.mi_enter("miattgetstr");
    if value.len() < maxlen + 1 {
        value.resize(maxlen + 1, 0);
    }
    let Some((att_type, att_length)) = nc::ncattinq(s, c.streams, cdfid, varid, name)? else {
        s.mi_return_error(c)?;
        return Ok(None);
    };
    if att_type != NC_CHAR {
        s.log_pkg_error(
            c,
            Some(PkgSite::AttNonChar),
            MI_ERR_NONCHAR,
            "Non-character datatype",
        )?;
        s.mi_return_error(c)?;
        return Ok(None);
    }
    let Some(Values::Char(att)) = nc::ncattget(s, c.streams, cdfid, varid, name)? else {
        s.mi_return_error(c)?;
        return Ok(None);
    };
    if att_length <= maxlen as i32 {
        if att_length <= 0 {
            // value[att_length - 1] is the byte before the buffer (FS-784): read as not NUL,
            // so the string is terminated at its start.
            c.notices.add(
                &d::FS_784,
                format!(
                    "the empty attribute {} read as an empty string (MINC checks the byte before its buffer)",
                    String::from_utf8_lossy(name)
                ),
            );
            value[0] = 0;
        } else {
            let n = att_length as usize;
            value[..n].copy_from_slice(&att[..n]);
            if value[n - 1] != 0 {
                if n == maxlen {
                    value[n - 1] = 0;
                } else {
                    value[n] = 0;
                }
            }
        }
    } else {
        let n = maxlen - 1;
        let mut i = 0;
        while i < n && i < att.len() && att[i] != 0 {
            value[i] = att[i];
            i += 1;
        }
        while i < n {
            value[i] = 0;
            i += 1;
        }
        value[maxlen - 1] = 0;
    }
    s.mi_return();
    Ok(Some(nc::cstr(value).to_vec()))
}

/// `miget_datatype`: the variable's type and sign, or `None`.
pub fn miget_datatype(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    imgid: i32,
) -> Result<Option<(i32, bool)>, Exit> {
    s.mi_enter("miget_datatype");
    let Some(inq) = nc::ncvarinq(s, c.streams, cdfid, imgid)? else {
        s.mi_return();
        return Ok(None);
    };
    let ty = inq.ty;
    let old = s.ncopts;
    s.ncopts = 0;
    let mut buf = Vec::new();
    let st = miattgetstr(s, c, cdfid, imgid, b"signtype", MI_MAX_ATTSTR_LEN, &mut buf)?;
    let signed = match st {
        Some(a) if a == MI_SIGNED => Some(true),
        Some(a) if a == MI_UNSIGNED => Some(false),
        _ => None,
    };
    s.ncopts = old;
    s.mi_return();
    Ok(Some((ty, signed.unwrap_or(ty != NC_BYTE))))
}

/// `miget_default_range`.
pub fn miget_default_range(ty: i32, is_signed: bool) -> [f64; 2] {
    default_range_of(ty, is_signed)
}

/// MINC 1.5.1's `miget_default_range` table.
pub fn default_range_of(ty: i32, is_signed: bool) -> [f64; 2] {
    match ty {
        NC_INT => {
            if is_signed {
                [i32::MIN as f64, i32::MAX as f64]
            } else {
                [0.0, u32::MAX as f64]
            }
        }
        NC_SHORT => {
            if is_signed {
                [i16::MIN as f64, i16::MAX as f64]
            } else {
                [0.0, u16::MAX as f64]
            }
        }
        NC_BYTE => {
            if is_signed {
                [i8::MIN as f64, i8::MAX as f64]
            } else {
                [0.0, u8::MAX as f64]
            }
        }
        NC_FLOAT => [-FLT_MAX, FLT_MAX],
        NC_DOUBLE => [-f64::MAX, f64::MAX],
        _ => [MI_DEFAULT_MIN, MI_DEFAULT_MAX],
    }
}

/// `(int) x` of a valid range bound: x86's value with `reproduce`, else clamped (FS-787).
fn range_to_int(c: &mut Ctx, x: f64) -> f64 {
    let x86 = cvt_i32(x);
    let exact = !x.is_nan() && x > -2_147_483_649.0 && x < 2_147_483_648.0;
    if exact {
        return x86 as f64;
    }
    if c.notices.reproduce() {
        c.notices.add(
            &d::FS_787,
            format!("the valid range bound {x} converted to int as x86 converts it (reproduce)"),
        );
        x86 as f64
    } else {
        c.notices.add(
            &d::FS_787,
            format!(
                "the valid range bound {x} clamped to int's range (MINC's conversion is undefined)"
            ),
        );
        if x.is_nan() {
            0.0
        } else {
            x.clamp(i32::MIN as f64, i32::MAX as f64).trunc()
        }
    }
}

/// `(unsigned int) x` of a valid range bound (x86: through 64 bits).
fn range_to_uint(c: &mut Ctx, x: f64) -> f64 {
    let x86 = cvt_i64(x) as u32;
    let exact = !x.is_nan() && x > -1.0 && x < 4_294_967_296.0;
    if exact {
        return x86 as f64;
    }
    if c.notices.reproduce() {
        c.notices.add(
            &d::FS_787,
            format!(
                "the valid range bound {x} converted to unsigned int as x86 converts it (reproduce)"
            ),
        );
        x86 as f64
    } else {
        c.notices.add(
            &d::FS_787,
            format!(
                "the valid range bound {x} clamped to unsigned int's range (MINC's conversion is undefined)"
            ),
        );
        if x.is_nan() {
            0.0
        } else {
            x.clamp(0.0, u32::MAX as f64).trunc()
        }
    }
}

/// `miget_valid_range`.
pub fn miget_valid_range(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    imgid: i32,
) -> Result<Option<[f64; 2]>, Exit> {
    s.mi_enter("miget_valid_range");
    let Some((ty, is_signed)) = miget_datatype(s, c, cdfid, imgid)? else {
        s.mi_return();
        return Ok(None);
    };
    let old = s.ncopts;
    s.ncopts = 0;
    let att_sign: &[u8] = if is_signed { MI_SIGNED } else { MI_UNSIGNED };
    let mut vr = [0.0f64; 2];
    let st = miattget_with_sign(
        s,
        c,
        cdfid,
        imgid,
        b"valid_range",
        Some(att_sign),
        2,
        &mut vr,
    )?;
    // A copy of MINC may check the type's default range here, whether or not it uses it.
    if let Some(e) = c.dialect.default_range(ty, c.notices, c.streams) {
        return Err(e);
    }
    if st != Some(2) {
        vr = miget_default_range(ty, is_signed);
        let mut mx = [vr[1]];
        miattget_with_sign(s, c, cdfid, imgid, b"valid_max", Some(att_sign), 1, &mut mx)?;
        vr[1] = mx[0];
        let mut mn = [vr[0]];
        miattget_with_sign(s, c, cdfid, imgid, b"valid_min", Some(att_sign), 1, &mut mn)?;
        vr[0] = mn[0];
    }
    s.ncopts = old;
    if vr[1] < vr[0] {
        vr.swap(0, 1);
    }
    match ty {
        NC_INT | NC_SHORT | NC_BYTE => {
            if is_signed {
                vr[0] = range_to_int(c, vr[0]);
                vr[1] = range_to_int(c, vr[1]);
            } else {
                vr[0] = range_to_uint(c, vr[0]);
                vr[1] = range_to_uint(c, vr[1]);
            }
        }
        NC_FLOAT => {
            vr[0] = f32_to_f64(f64_to_f32(vr[0]));
            vr[1] = f32_to_f64(f64_to_f32(vr[1]));
        }
        _ => {}
    }
    s.mi_return();
    Ok(Some(vr))
}

/// `miget_image_range`.
pub fn miget_image_range(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
) -> Result<Option<[f64; 2]>, Exit> {
    s.mi_enter("miget_image_range");
    let mut range = [MI_DEFAULT_MIN, MI_DEFAULT_MAX];
    let old = s.ncopts;
    s.ncopts = 0;
    let vmin = nc::ncvarid(s, c.streams, cdfid, b"image-min")?;
    let vmax = nc::ncvarid(s, c.streams, cdfid, b"image-max")?;
    s.ncopts = old;
    let imgid = nc::ncvarid(s, c.streams, cdfid, b"image")?;
    if imgid == MI_ERROR {
        s.mi_return();
        return Ok(None);
    }
    let Some((ty, _signed)) = miget_datatype(s, c, cdfid, imgid)? else {
        s.mi_return();
        return Ok(None);
    };
    if vmin == MI_ERROR || vmax == MI_ERROR {
        let is_float = ty == NC_FLOAT || ty == NC_DOUBLE;
        let mut no_range = false;
        if is_float {
            let Some(vr) = miget_valid_range(s, c, cdfid, imgid)? else {
                s.mi_return();
                return Ok(None);
            };
            range = vr;
            no_range = (ty == NC_FLOAT && range[1] == FLT_MAX)
                || (ty == NC_DOUBLE && range[1] == f64::MAX);
        }
        if !is_float || no_range {
            range = [MI_DEFAULT_MIN, MI_DEFAULT_MAX];
        }
    } else {
        range = [f64::MAX, -f64::MAX];
        for vid in [vmin, vmax] {
            let Some(inq) = nc::ncvarinq(s, c.streams, cdfid, vid)? else {
                s.mi_return_error(c)?;
                return Ok(None);
            };
            let mut count = Vec::with_capacity(inq.dimids.len());
            let mut size: i64 = 1;
            for &dim in &inq.dimids {
                let Some((_, n)) = nc::ncdiminq(s, c.streams, cdfid, dim)? else {
                    s.mi_return_error(c)?;
                    return Ok(None);
                };
                count.push(n);
                size = size.wrapping_mul(n);
            }
            let start = vec![0i64; inq.dimids.len()];
            let Some(buf) = mivarget(s, c, cdfid, vid, &start, &count, NC_DOUBLE, None)? else {
                s.mi_return_error(c)?;
                return Ok(None);
            };
            for i in 0..(size.max(0) as usize).min(buf.len()) {
                let v = buf.get(i);
                range[0] = c_min(range[0], v);
                range[1] = c_max(range[1], v);
            }
        }
    }
    if ty == NC_FLOAT {
        range[0] = f32_to_f64(f64_to_f32(range[0]));
        range[1] = f32_to_f64(f64_to_f32(range[1]));
    }
    s.mi_return();
    Ok(Some(range))
}

/// `MI_varaccess` for a get: the values of the hyperslab converted to `datatype` with `sign`
/// (`MI_PRIV_*`), through `conv` (an ICV's scale and fill check) when given; or `None`.
pub(crate) fn mi_varaccess_get(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    start: &[i64],
    count: &[i64],
    datatype: i32,
    sign: i32,
    conv: Option<&Conversion>,
) -> Result<Option<Typed>, Exit> {
    s.mi_enter("MI_varaccess");
    let Some(inq) = nc::ncvarinq(s, c.streams, cdfid, varid)? else {
        s.mi_return_error(c)?;
        return Ok(None);
    };
    if datatype == NC_CHAR || inq.ty == NC_CHAR {
        s.log_pkg_error(
            c,
            Some(PkgSite::VarNonNumeric),
            MI_ERR_NONNUMERIC,
            "Non-numeric datatype",
        )?;
        s.mi_return_error(c)?;
        return Ok(None);
    }
    let old = s.ncopts;
    s.ncopts = 0;
    let mut buf = Vec::new();
    let st = miattgetstr(s, c, cdfid, varid, b"signtype", MI_MAX_ATTSTR_LEN, &mut buf)?;
    s.ncopts = old;
    let var_sign = sign_from_string(inq.ty, st.as_deref());
    let call_sign = get_sign(datatype, sign);
    let ndims = inq.dimids.len();
    let st_: Vec<i64> = (0..ndims)
        .map(|i| start.get(i).copied().unwrap_or(0))
        .collect();
    let ct_: Vec<i64> = (0..ndims)
        .map(|i| count.get(i).copied().unwrap_or(1))
        .collect();
    let fast = datatype == inq.ty
        && call_sign == var_sign
        && conv.is_none_or(|k| !k.do_scale && !k.do_fillvalue);
    if !fast {
        // MI_varaccess's sizes of the two types (nctypelen's messages for a type netCDF does not
        // know), then MI_var_loop's buffer.
        let value_size = nc::nctypelen(s, c.streams, inq.ty)? as i64;
        nc::nctypelen(s, c.streams, datatype)?;
        if value_size < 0 && !var_loop_alloc_ok(&ct_, value_size) {
            sys_error(s, c, PkgSite::VarLoopAlloc, "MI_var_loop")?;
            s.mi_return_error(c)?;
            return Ok(None);
        }
    }
    if fast {
        let got = nc::ncvarget(s, c.streams, cdfid, varid, &st_, &ct_)?;
        s.drain_notices(c.notices);
        let Some(vals) = got else {
            s.mi_return_error(c)?;
            return Ok(None);
        };
        let mut out = Typed::new(datatype, call_sign).expect("a numeric type");
        copy_raw(&vals, &mut out, vals.len());
        s.mi_return();
        return Ok(Some(out));
    }
    // MI_var_loop: pieces of at most MI_MAX_VAR_BUFFER_SIZE bytes, each read and converted.
    let value_size = nc::type_size(inq.ty) as i64;
    let pieces = if value_size > 0 {
        var_loop_pieces(&st_, &ct_, value_size)
    } else {
        vec![(st_.clone(), ct_.clone())]
    };
    let mut out = Typed::new(datatype, call_sign);
    for (ps, pc) in pieces {
        let got = nc::ncvarget(s, c.streams, cdfid, varid, &ps, &pc)?;
        s.drain_notices(c.notices);
        let Some(vals) = got else {
            s.mi_return_error(c)?;
            return Ok(None);
        };
        let Some(o) = out.as_mut() else {
            // MI_convert_type: the sizes of the input and output types (the second's message).
            nc::nctypelen(s, c.streams, datatype)?;
            s.mi_return_error(c)?;
            return Ok(None);
        };
        convert(c, &vals, var_sign, o, vals.len(), conv);
    }
    s.mi_return();
    Ok(Some(out.unwrap_or(Typed::F64(Vec::new()))))
}

/// `MI_var_loop`'s pieces of a hyperslab (no buffer size steps): each `(start, count)` one read.
fn var_loop_pieces(start: &[i64], count: &[i64], value_size: i64) -> Vec<(Vec<i64>, Vec<i64>)> {
    let ndims = count.len();
    if ndims == 0 {
        return vec![(Vec::new(), Vec::new())];
    }
    let mut nvalues: i64 = 1;
    let mut newn: i64 = 1;
    let mut firstdim = ndims as i64 - 1;
    while firstdim >= 1 {
        newn = newn.wrapping_mul(count[firstdim as usize]);
        if newn.wrapping_mul(value_size) > MI_MAX_VAR_BUFFER_SIZE {
            break;
        }
        nvalues = newn;
        firstdim -= 1;
    }
    let fd = firstdim.max(0) as usize;
    let ntimes = (MI_MAX_VAR_BUFFER_SIZE / (nvalues * value_size).max(1)).min(count[fd]);
    let mut var_count: Vec<i64> = (0..ndims)
        .map(|i| {
            if i > fd {
                count[i]
            } else if i == fd {
                ntimes
            } else {
                1
            }
        })
        .collect();
    let mut var_start = start.to_vec();
    let var_end: Vec<i64> = start.iter().zip(count).map(|(s, c)| s + c).collect();
    let mut out = Vec::new();
    if ntimes <= 0 && count[fd] > 0 {
        return vec![(start.to_vec(), count.to_vec())];
    }
    while var_start[0] < var_end[0] {
        var_count[fd] = ntimes.min(var_end[fd] - var_start[fd]);
        out.push((var_start.clone(), var_count.clone()));
        var_start[fd] += var_count[fd];
        let mut i = fd;
        while i > 0 && var_start[i] >= var_end[i] {
            var_start[i] = start[i];
            i -= 1;
            var_start[i] += 1;
        }
    }
    out
}

/// Whether `MI_var_loop`'s buffer (`ntimes * nvalues * value_size` bytes, C's `long`
/// arithmetic) can be allocated, for a value size of `nctypelen`'s `-1`: a negative or huge size
/// fails as `malloc` fails.
fn var_loop_alloc_ok(count: &[i64], value_size: i64) -> bool {
    let ndims = count.len() as i64;
    let mut nvalues: i64 = 1;
    let mut newn: i64 = 1;
    let mut firstdim = ndims - 1;
    while firstdim >= 1 {
        newn = newn.wrapping_mul(count[firstdim as usize]);
        if newn.wrapping_mul(value_size) > MI_MAX_VAR_BUFFER_SIZE {
            break;
        }
        nvalues = newn;
        firstdim -= 1;
    }
    let ntimes = if firstdim < 0 {
        1
    } else {
        let d = nvalues.wrapping_mul(value_size);
        if d == 0 {
            return false;
        }
        (MI_MAX_VAR_BUFFER_SIZE / d).min(count[firstdim as usize])
    };
    let bytes = ntimes.wrapping_mul(nvalues).wrapping_mul(value_size);
    (0..1 << 40).contains(&bytes)
}

/// `mivarget`.
pub fn mivarget(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    start: &[i64],
    count: &[i64],
    datatype: i32,
    sign: Option<&[u8]>,
) -> Result<Option<Typed>, Exit> {
    s.mi_enter("mivarget");
    let sg = sign_from_string(datatype, sign);
    match mi_varaccess_get(s, c, cdfid, varid, start, count, datatype, sg, None)? {
        None => {
            s.mi_return_error(c)?;
            Ok(None)
        }
        Some(v) => {
            s.mi_return();
            Ok(Some(v))
        }
    }
}

/// `mivarget1` (a double).
pub fn mivarget1(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    mindex: &[i64],
) -> Result<Option<f64>, Exit> {
    s.mi_enter("mivarget1");
    let count = vec![1i64; nc::MAX_VAR_DIMS];
    let sg = sign_from_string(NC_DOUBLE, None);
    match mi_varaccess_get(s, c, cdfid, varid, mindex, &count, NC_DOUBLE, sg, None)? {
        Some(v) if !v.is_empty() => {
            s.mi_return();
            Ok(Some(v.get(0)))
        }
        _ => {
            s.mi_return_error(c)?;
            Ok(None)
        }
    }
}

/// `mitranslate_coords`: `outcoords` for `outvar` from `incoords` of `invar`; a dimension of
/// `outvar` that `invar` does not have keeps the value `outcoords` held (`None` marks it), or
/// `None` after an error.
pub fn mitranslate_coords(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    invar: i32,
    incoords: &[i64],
    outvar: i32,
) -> Result<Option<Vec<Option<i64>>>, Exit> {
    s.mi_enter("mitranslate_coords");
    let a = nc::ncvarinq(s, c.streams, cdfid, invar)?;
    let b = match a {
        Some(_) => nc::ncvarinq(s, c.streams, cdfid, outvar)?,
        None => None,
    };
    let (Some(a), Some(b)) = (a, b) else {
        s.mi_return_error(c)?;
        return Ok(None);
    };
    let out = b
        .dimids
        .iter()
        .map(|od| {
            a.dimids
                .iter()
                .position(|id| id == od)
                .map(|j| incoords.get(j).copied().unwrap_or(0))
        })
        .collect();
    s.mi_return();
    Ok(Some(out))
}
