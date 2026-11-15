// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
//! MINC 1.5.1's library functions on the writing path (`libsrc/netcdf_convenience.c`,
//! `minc_convenience.c`, `value_conversion.c`; MNI's notice, `LICENSES/LicenseRef-MNI.txt`):
//! `micreate`, the attribute puts, the standard variables with their attributes and their
//! parent/child pointers, the valid range, and the variable writes with `MI_convert_type`'s
//! conversion from the caller's type to the variable's.
//!
//! The C functions return `MI_ERROR` (`-1`) or a value; so do these (`Err(Exit)` for a fatal
//! error that ends the program).

use super::libsrc::{
    self as mi, Conversion, FILLVALUE_EPSILON, MI_ERR_NONNUMERIC, MI_ERROR, MI_MAX_ATTSTR_LEN,
    MI_PRIV_SIGNED, MI_PRIV_UNSIGNED, Typed,
};
use super::ncwrite;
use super::netcdf::{self as nc, NC_BYTE, NC_CHAR, NC_DOUBLE, NC_FLOAT, NC_INT, NC_SHORT, Values};
use super::{Ctx, Exit, PkgSite, Session};

pub const MI_ERR_BAD_STDVAR: i32 = 1336;
pub const MI_ERR_BADSUFFIX: i32 = 1337;
pub const MI_ERR_WRONGNDIMS: i32 = 1346;
pub const MI_ERR_MAXMIN_DIMS: i32 = 1348;

pub const MI_STDVAR: &[u8] = b"MINC standard variable";
pub const MI_GROUP: &[u8] = b"group________";
pub const MI_DIMENSION: &[u8] = b"dimension____";
pub const MI_DIM_WIDTH: &[u8] = b"dim-width____";
pub const MI_VARATT: &[u8] = b"var_attribute";
pub const MI_CURRENT_VERSION: &[u8] = b"MINC Version    1.0";
pub const MI_REGULAR: &[u8] = b"regular__";
pub const MI_IRREGULAR: &[u8] = b"irregular";
pub const MI_TRUE: &[u8] = b"true_";
pub const MI_FALSE: &[u8] = b"false";

/// `dimvarlist`: the dimensions with a dimension variable.
const DIMVARLIST: [&[u8]; 8] = [
    b"xspace",
    b"yspace",
    b"zspace",
    b"time",
    b"tfrequency",
    b"xfrequency",
    b"yfrequency",
    b"zfrequency",
];
/// `dimwidthlist`.
const DIMWIDTHLIST: [&[u8]; 8] = [
    b"xspace-width",
    b"yspace-width",
    b"zspace-width",
    b"time-width",
    b"tfrequency-width",
    b"xfrequency-width",
    b"yfrequency-width",
    b"zfrequency-width",
];
/// `varlist`.
const VARLIST: [&[u8]; 7] = [
    b"rootvariable",
    b"image",
    b"image-max",
    b"image-min",
    b"patient",
    b"study",
    b"acquisition",
];

/// `MI_CHK_ERR` on a value: `MI_RETURN_ERROR(MI_ERROR)` when it is `MI_ERROR`.
macro_rules! chk {
    ($s:expr, $c:expr, $v:expr) => {{
        let v = $v;
        if v == MI_ERROR {
            $s.mi_return_error($c)?;
            return Ok(MI_ERROR);
        }
        v
    }};
}

/// `micreate`: the new file's id (`nccreate`), or `MI_ERROR`.
pub fn micreate(s: &mut Session, c: &mut Ctx, path: &[u8], cmode: i32) -> Result<i32, Exit> {
    s.mi_enter("micreate");
    let status = chk!(s, c, ncwrite::nccreate(s, c.streams, path, cmode)?);
    s.mi_return();
    Ok(status)
}

/// `miattputstr`: the string and its terminating NUL.
pub fn miattputstr(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    name: &[u8],
    value: &[u8],
) -> Result<i32, Exit> {
    s.mi_enter("miattputstr");
    let mut v = nc::cstr(value).to_vec();
    v.push(0);
    chk!(
        s,
        c,
        ncwrite::ncattput(s, c.streams, cdfid, varid, name, NC_CHAR, &Values::Char(v))?
    );
    s.mi_return();
    Ok(0)
}

/// `miattputdbl`.
pub fn miattputdbl(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    name: &[u8],
    value: f64,
) -> Result<i32, Exit> {
    s.mi_enter("miattputdbl");
    chk!(
        s,
        c,
        ncwrite::ncattput(
            s,
            c.streams,
            cdfid,
            varid,
            name,
            NC_DOUBLE,
            &Values::Double(vec![value])
        )?
    );
    s.mi_return();
    Ok(0)
}

/// `miattputint`.
pub fn miattputint(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    name: &[u8],
    value: i32,
) -> Result<i32, Exit> {
    s.mi_enter("miattputint");
    chk!(
        s,
        c,
        ncwrite::ncattput(
            s,
            c.streams,
            cdfid,
            varid,
            name,
            NC_INT,
            &Values::Int(vec![value])
        )?
    );
    s.mi_return();
    Ok(0)
}

/// `ncvarinq`'s name of a variable, or `MI_ERROR`'s `None` after its message.
fn var_name(s: &mut Session, c: &mut Ctx, cdfid: i32, varid: i32) -> Result<Option<Vec<u8>>, Exit> {
    Ok(nc::ncvarinq(s, c.streams, cdfid, varid)?.map(|v| v.name))
}

/// `ncvarid` with `ncopts` 0 (no message).
fn quiet_varid(s: &mut Session, c: &mut Ctx, cdfid: i32, name: &[u8]) -> Result<i32, Exit> {
    let old = s.ncopts;
    s.ncopts = 0;
    let r = nc::ncvarid(s, c.streams, cdfid, name);
    s.ncopts = old;
    r
}

/// `miattput_pointer`: `name` of `varid` points at `ptrvarid` (`--->` and its name), which gets
/// `parent` and the `var_attribute` type.
pub fn miattput_pointer(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    name: &[u8],
    ptrvarid: i32,
) -> Result<i32, Exit> {
    s.mi_enter("miattput_pointer");
    let Some(ptrname) = var_name(s, c, cdfid, ptrvarid)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    let mut p = b"--->".to_vec();
    p.extend_from_slice(&ptrname);
    chk!(s, c, miattputstr(s, c, cdfid, varid, name, &p)?);
    let Some(parent) = var_name(s, c, cdfid, varid)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    chk!(
        s,
        c,
        miattputstr(s, c, cdfid, ptrvarid, b"parent", &parent)?
    );
    chk!(
        s,
        c,
        miattputstr(s, c, cdfid, ptrvarid, b"vartype", MI_VARATT)?
    );
    s.mi_return();
    Ok(0)
}

/// `miadd_child`: the child's name added to the parent's `children` (separated by newlines; a
/// name found anywhere in the list already, as a part of another name too, is not added again)
/// and the parent's name set as the child's `parent`.
pub fn miadd_child(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    parent_varid: i32,
    child_varid: i32,
) -> Result<i32, Exit> {
    s.mi_enter("miadd_child");
    let old = s.ncopts;
    s.ncopts = 0;
    let st = nc::ncattinq(s, c.streams, cdfid, parent_varid, b"children")?;
    s.ncopts = old;
    let mut size = match st {
        Some((NC_CHAR, n)) => n.max(0) as usize,
        _ => 0,
    };
    let mut list: Vec<u8> = Vec::new();
    if size > 0 {
        let Some(Values::Char(v)) = nc::ncattget(s, c.streams, cdfid, parent_varid, b"children")?
        else {
            s.mi_return_error(c)?;
            return Ok(MI_ERROR);
        };
        list = v;
        list.truncate(size);
        if list[size - 1] == 0 {
            size -= 1;
        }
        list.truncate(size);
        list.push(b'\n');
        size += 1;
    }
    let Some(child) = var_name(s, c, cdfid, child_varid)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    list.truncate(size);
    list.extend_from_slice(&child);
    // strstr(child_list, new_child) != new_child: the name occurs earlier in the list (as C
    // reads the list: up to its first NUL).
    let hay = nc::cstr(&list);
    let at = if child.is_empty() {
        Some(0)
    } else {
        hay.windows(child.len()).position(|w| w == child.as_slice())
    };
    if at != Some(size) && size > 0 {
        size -= 1;
        list.truncate(size);
    }
    if miattputstr(s, c, cdfid, parent_varid, b"children", &list)? == MI_ERROR {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    let Some(pname) = var_name(s, c, cdfid, parent_varid)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    if miattputstr(s, c, cdfid, child_varid, b"parent", &pname)? == MI_ERROR {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    s.mi_return();
    Ok(0)
}

/// `micreate_std_variable`: a standard variable (a dimension variable, a dimension width, the
/// image, its range variables, the root or a group) with its default attributes.
pub fn micreate_std_variable(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    name: &[u8],
    datatype: i32,
    ndims: i32,
    dims: &[i32],
) -> Result<i32, Exit> {
    s.mi_enter("micreate_std_variable");
    let name = nc::cstr(name);
    let varid;
    if DIMVARLIST.contains(&name) {
        varid = chk!(
            s,
            c,
            create_dim_variable(s, c, cdfid, name, datatype, ndims)?
        );
    } else if DIMWIDTHLIST.contains(&name) {
        varid = chk!(
            s,
            c,
            create_dimwidth_variable(s, c, cdfid, name, datatype, ndims)?
        );
    } else if VARLIST.contains(&name) {
        varid = match name {
            b"image" => chk!(
                s,
                c,
                create_image_variable(s, c, cdfid, name, datatype, ndims, dims)?
            ),
            b"image-max" | b"image-min" => {
                chk!(
                    s,
                    c,
                    create_imaxmin_variable(s, c, cdfid, name, datatype, ndims, dims)?
                )
            }
            b"rootvariable" => chk!(s, c, create_root_variable(s, c, cdfid, name)?),
            _ => chk!(s, c, create_simple_variable(s, c, cdfid, name)?),
        };
    } else {
        s.log_pkg_error(
            c,
            Some(PkgSite::BadStdVar),
            MI_ERR_BAD_STDVAR,
            &format!(
                "{} is not a standard MINC variable",
                String::from_utf8_lossy(name)
            ),
        )?;
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    s.mi_return();
    Ok(varid)
}

/// The three attributes every standard variable gets.
fn std_atts(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    vartype: &[u8],
) -> Result<i32, Exit> {
    if miattputstr(s, c, cdfid, varid, b"varid", MI_STDVAR)? == MI_ERROR
        || miattputstr(s, c, cdfid, varid, b"vartype", vartype)? == MI_ERROR
        || miattputstr(s, c, cdfid, varid, b"version", MI_CURRENT_VERSION)? == MI_ERROR
    {
        return Ok(MI_ERROR);
    }
    Ok(0)
}

/// `MI_create_dim_variable`.
fn create_dim_variable(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    name: &[u8],
    datatype: i32,
    ndims: i32,
) -> Result<i32, Exit> {
    s.mi_enter("MI_create_dim_variable");
    if ndims > 1 {
        s.log_pkg_error(
            c,
            Some(PkgSite::WrongNDims),
            MI_ERR_WRONGNDIMS,
            "Too many dimensions for a dimension variable",
        )?;
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    let dimid = chk!(s, c, ncwrite::ncdimid(s, c.streams, cdfid, name)?);
    let varid = chk!(
        s,
        c,
        ncwrite::ncvardef(s, c.streams, cdfid, name, datatype, ndims, &[dimid])?
    );
    chk!(s, c, std_atts(s, c, cdfid, varid, MI_DIMENSION)?);
    let comment: Option<&[u8]> = match name {
        b"xspace" => Some(b"X increases from patient left to right"),
        b"yspace" => Some(b"Y increases from patient posterior to anterior"),
        b"zspace" => Some(b"Z increases from patient inferior to superior"),
        _ => None,
    };
    if let Some(t) = comment {
        chk!(s, c, miattputstr(s, c, cdfid, varid, b"comments", t)?);
    }
    let spacing = if ndims == 0 { MI_REGULAR } else { MI_IRREGULAR };
    chk!(s, c, miattputstr(s, c, cdfid, varid, b"spacing", spacing)?);
    let alignment: &[u8] = if name == b"time" {
        b"start_"
    } else {
        b"centre"
    };
    chk!(
        s,
        c,
        miattputstr(s, c, cdfid, varid, b"alignment", alignment)?
    );
    s.mi_return();
    Ok(varid)
}

/// `MI_create_dimwidth_variable`.
fn create_dimwidth_variable(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    name: &[u8],
    datatype: i32,
    ndims: i32,
) -> Result<i32, Exit> {
    s.mi_enter("MI_create_dimwidth_variable");
    let Some(at) = name.windows(6).position(|w| w == b"-width") else {
        s.log_pkg_error(
            c,
            Some(PkgSite::BadSuffix),
            MI_ERR_BADSUFFIX,
            "Bad dimension width suffix",
        )?;
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    if ndims > 1 {
        s.log_pkg_error(
            c,
            Some(PkgSite::WrongNDims),
            MI_ERR_WRONGNDIMS,
            "Too many dimensions for a dimension variable",
        )?;
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    let dimid = chk!(s, c, ncwrite::ncdimid(s, c.streams, cdfid, &name[..at])?);
    let varid = chk!(
        s,
        c,
        ncwrite::ncvardef(s, c.streams, cdfid, name, datatype, ndims, &[dimid])?
    );
    chk!(s, c, std_atts(s, c, cdfid, varid, MI_DIM_WIDTH)?);
    let spacing = if ndims == 0 { MI_REGULAR } else { MI_IRREGULAR };
    chk!(s, c, miattputstr(s, c, cdfid, varid, b"spacing", spacing)?);
    chk!(
        s,
        c,
        miattputstr(s, c, cdfid, varid, b"filtertype", b"square____")?
    );
    s.mi_return();
    Ok(varid)
}

/// `MI_verify_maxmin_dims`: `image-max` / `image-min` may not vary over the image's two fastest
/// dimensions (three with a vector dimension).
fn verify_maxmin_dims(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    image_dim: &[i32],
    maxmin_dim: &[i32],
) -> Result<i32, Exit> {
    s.mi_enter("MI_verify_maxmin_dims");
    let last = image_dim.last().copied().unwrap_or(-1);
    let Some((dimname, _)) = nc::ncdiminq(s, c.streams, cdfid, last)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    let nbad = if dimname == b"vector_dimension" { 3 } else { 2 };
    let n = image_dim.len();
    for &id in &image_dim[n.saturating_sub(nbad)..] {
        if maxmin_dim.contains(&id) {
            s.log_pkg_error(
                c,
                Some(PkgSite::MaxMinDims),
                MI_ERR_MAXMIN_DIMS,
                "Imagemax/min dimensions vary over image dimensions",
            )?;
            s.mi_return_error(c)?;
            return Ok(MI_ERROR);
        }
    }
    s.mi_return();
    Ok(0)
}

/// `MI_create_image_variable`.
fn create_image_variable(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    name: &[u8],
    datatype: i32,
    ndims: i32,
    dims: &[i32],
) -> Result<i32, Exit> {
    let dims = &dims[..(ndims.max(0) as usize).min(dims.len())];
    s.mi_enter("MI_create_image_variable");
    let max_varid = quiet_varid(s, c, cdfid, b"image-max")?;
    let min_varid = quiet_varid(s, c, cdfid, b"image-min")?;
    for id in [max_varid, min_varid] {
        if id != MI_ERROR {
            let Some(inq) = nc::ncvarinq(s, c.streams, cdfid, id)? else {
                s.mi_return_error(c)?;
                return Ok(MI_ERROR);
            };
            chk!(s, c, verify_maxmin_dims(s, c, cdfid, dims, &inq.dimids)?);
        }
    }
    let varid = chk!(
        s,
        c,
        ncwrite::ncvardef(s, c.streams, cdfid, name, datatype, ndims, dims)?
    );
    chk!(s, c, add_stdgroup(s, c, cdfid, varid)?);
    if max_varid != MI_ERROR {
        chk!(
            s,
            c,
            miattput_pointer(s, c, cdfid, varid, b"image-max", max_varid)?
        );
    }
    if min_varid != MI_ERROR {
        chk!(
            s,
            c,
            miattput_pointer(s, c, cdfid, varid, b"image-min", min_varid)?
        );
    }
    s.mi_return();
    Ok(varid)
}

/// `MI_create_imaxmin_variable`: with `_FillValue` 0 for `image-min`, 1 for `image-max`.
fn create_imaxmin_variable(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    name: &[u8],
    datatype: i32,
    ndims: i32,
    dims: &[i32],
) -> Result<i32, Exit> {
    let dims = &dims[..(ndims.max(0) as usize).min(dims.len())];
    s.mi_enter("MI_create_imaxmin_variable");
    let image_varid = quiet_varid(s, c, cdfid, b"image")?;
    if image_varid != MI_ERROR {
        let Some(inq) = nc::ncvarinq(s, c.streams, cdfid, image_varid)? else {
            s.mi_return_error(c)?;
            return Ok(MI_ERROR);
        };
        chk!(s, c, verify_maxmin_dims(s, c, cdfid, &inq.dimids, dims)?);
    }
    let varid = chk!(
        s,
        c,
        ncwrite::ncvardef(s, c.streams, cdfid, name, datatype, ndims, dims)?
    );
    chk!(s, c, std_atts(s, c, cdfid, varid, MI_VARATT)?);
    let one = name == b"image-max";
    let fill = match datatype {
        NC_BYTE => Some(Values::Byte(vec![i8::from(one)])),
        NC_SHORT => Some(Values::Short(vec![i16::from(one)])),
        NC_INT => Some(Values::Int(vec![i32::from(one)])),
        NC_FLOAT => Some(Values::Float(vec![if one { 1.0 } else { 0.0 }])),
        NC_DOUBLE => Some(Values::Double(vec![if one { 1.0 } else { 0.0 }])),
        _ => None,
    };
    if let Some(f) = fill {
        chk!(
            s,
            c,
            ncwrite::ncattput(s, c.streams, cdfid, varid, b"_FillValue", datatype, &f)?
        );
    }
    if image_varid != MI_ERROR {
        chk!(
            s,
            c,
            miattput_pointer(s, c, cdfid, image_varid, name, varid)?
        );
    }
    s.mi_return();
    Ok(varid)
}

/// `MI_create_root_variable`.
fn create_root_variable(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    name: &[u8],
) -> Result<i32, Exit> {
    s.mi_enter("MI_create_root_variable");
    let varid = chk!(
        s,
        c,
        ncwrite::ncvardef(s, c.streams, cdfid, name, NC_INT, 0, &[])?
    );
    chk!(s, c, std_atts(s, c, cdfid, varid, MI_GROUP)?);
    chk!(s, c, miattputstr(s, c, cdfid, varid, b"parent", b"")?);
    s.mi_return();
    Ok(varid)
}

/// `MI_create_simple_variable`.
fn create_simple_variable(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    name: &[u8],
) -> Result<i32, Exit> {
    s.mi_enter("MI_create_simple_variable");
    let varid = chk!(
        s,
        c,
        ncwrite::ncvardef(s, c.streams, cdfid, name, NC_INT, 0, &[])?
    );
    chk!(s, c, add_stdgroup(s, c, cdfid, varid)?);
    s.mi_return();
    Ok(varid)
}

/// `MI_add_stdgroup`: the variable made a child of the root variable (created if missing).
fn add_stdgroup(s: &mut Session, c: &mut Ctx, cdfid: i32, varid: i32) -> Result<i32, Exit> {
    s.mi_enter("MI_add_stdgroup");
    let mut root = quiet_varid(s, c, cdfid, b"rootvariable")?;
    if root == MI_ERROR {
        root = chk!(s, c, create_root_variable(s, c, cdfid, b"rootvariable")?);
    }
    chk!(s, c, miadd_child(s, c, cdfid, root, varid)?);
    chk!(s, c, std_atts(s, c, cdfid, varid, MI_GROUP)?);
    s.mi_return();
    Ok(0)
}

/// `miset_valid_range`: floats for a float image, doubles for the others.
pub fn miset_valid_range(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    imgid: i32,
    range: [f64; 2],
) -> Result<i32, Exit> {
    s.mi_enter("miset_valid_range");
    let Some((ty, _)) = mi::miget_datatype(s, c, cdfid, imgid)? else {
        s.mi_return();
        return Ok(MI_ERROR);
    };
    let status = if ty == NC_FLOAT {
        let v = Values::Float(vec![mi::f64_to_f32(range[0]), mi::f64_to_f32(range[1])]);
        ncwrite::ncattput(s, c.streams, cdfid, imgid, b"valid_range", ty, &v)?
    } else {
        let v = Values::Double(range.to_vec());
        ncwrite::ncattput(s, c.streams, cdfid, imgid, b"valid_range", NC_DOUBLE, &v)?
    };
    s.mi_return();
    Ok(status)
}

/// The bits of value `i` of `t` read as type `ty` with `sign` (`MI_TO_DOUBLE` of a buffer of
/// that type: the bytes reinterpreted).
fn typed_to_double(t: &Typed, i: usize, ty: i32, sign: i32) -> f64 {
    let raw: u64 = match t {
        Typed::U8(v) => v[i] as u64,
        Typed::I8(v) => v[i] as u8 as u64,
        Typed::U16(v) => v[i] as u64,
        Typed::I16(v) => v[i] as u16 as u64,
        Typed::U32(v) => v[i] as u64,
        Typed::I32(v) => v[i] as u32 as u64,
        Typed::F32(v) => v[i].to_bits() as u64,
        Typed::F64(v) => v[i].to_bits(),
    };
    match ty {
        NC_BYTE => {
            if sign == MI_PRIV_UNSIGNED {
                raw as u8 as f64
            } else {
                raw as u8 as i8 as f64
            }
        }
        NC_SHORT => {
            if sign == MI_PRIV_UNSIGNED {
                raw as u16 as f64
            } else {
                raw as u16 as i16 as f64
            }
        }
        NC_INT => {
            if sign == MI_PRIV_UNSIGNED {
                raw as u32 as f64
            } else {
                raw as u32 as i32 as f64
            }
        }
        NC_FLOAT => mi::f32_to_f64(f32::from_bits(raw as u32)),
        _ => f64::from_bits(raw),
    }
}

/// The type and sign of a buffer (its variant).
fn typed_kind(t: &Typed) -> (i32, i32) {
    match t {
        Typed::U8(_) => (NC_BYTE, MI_PRIV_UNSIGNED),
        Typed::I8(_) => (NC_BYTE, MI_PRIV_SIGNED),
        Typed::U16(_) => (NC_SHORT, MI_PRIV_UNSIGNED),
        Typed::I16(_) => (NC_SHORT, MI_PRIV_SIGNED),
        Typed::U32(_) => (NC_INT, MI_PRIV_UNSIGNED),
        Typed::I32(_) => (NC_INT, MI_PRIV_SIGNED),
        Typed::F32(_) => (NC_FLOAT, MI_PRIV_SIGNED),
        Typed::F64(_) => (NC_DOUBLE, MI_PRIV_SIGNED),
    }
}

/// The raw bits of value `i` of `t` stored into `out` (C's `memcpy` between buffers of the same
/// type).
fn push_raw(out: &mut Typed, t: &Typed, i: usize) {
    let raw: u64 = match t {
        Typed::U8(v) => v[i] as u64,
        Typed::I8(v) => v[i] as u8 as u64,
        Typed::U16(v) => v[i] as u64,
        Typed::I16(v) => v[i] as u16 as u64,
        Typed::U32(v) => v[i] as u64,
        Typed::I32(v) => v[i] as u32 as u64,
        Typed::F32(v) => v[i].to_bits() as u64,
        Typed::F64(v) => v[i].to_bits(),
    };
    match out {
        Typed::U8(o) => o.push(raw as u8),
        Typed::I8(o) => o.push(raw as u8 as i8),
        Typed::U16(o) => o.push(raw as u16),
        Typed::I16(o) => o.push(raw as u16 as i16),
        Typed::U32(o) => o.push(raw as u32),
        Typed::I32(o) => o.push(raw as u32 as i32),
        Typed::F32(o) => o.push(f32::from_bits(raw as u32)),
        Typed::F64(o) => o.push(f64::from_bits(raw)),
    }
}

/// `MI_convert_type` from the caller's buffer `vals` (values `from..from+n`, of type `intype`
/// with `insign`) into `out` (of its own type and sign), through `conv` when given.
pub(crate) fn convert_from(
    c: &mut Ctx,
    vals: &Typed,
    from: usize,
    n: usize,
    intype: i32,
    insign: i32,
    out: &mut Typed,
    conv: Option<&Conversion>,
) {
    let insgn = mi::get_sign(intype, insign);
    let (outtype, outsgn) = typed_kind(out);
    let (do_scale, do_fill) = conv.map_or((false, false), |k| (k.do_scale, k.do_fillvalue));
    if intype == outtype && insgn == outsgn && !do_scale && !do_fill {
        for i in from..from + n {
            push_raw(out, vals, i);
        }
        return;
    }
    let (dmin, dmax, fill) = match conv {
        Some(k) => {
            let eps = ((k.fill_valid_max - k.fill_valid_min) * FILLVALUE_EPSILON).abs();
            (k.fill_valid_min - eps, k.fill_valid_max + eps, k.fillvalue)
        }
        None => (0.0, 0.0, 0.0),
    };
    for i in from..from + n {
        let mut dv = typed_to_double(vals, i, intype, insgn);
        if do_fill && (dv < dmin || dv > dmax) {
            dv = fill;
        } else if do_scale {
            let k = conv.expect("a conversion");
            dv = k.scale * dv + k.offset;
        }
        mi::from_double(c, out, dv);
    }
}

/// `MI_varaccess` for a put: `values` (the caller's buffer of `datatype` with `sign`, `MI_PRIV_*`)
/// written to the hyperslab, converted to the variable's type and sign through `conv` (an ICV's
/// scale and fill check) when given; `MI_ERROR` or 0.
pub(crate) fn mi_varaccess_put(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    start: &[i64],
    count: &[i64],
    datatype: i32,
    sign: i32,
    values: &Typed,
    conv: Option<&Conversion>,
) -> Result<i32, Exit> {
    s.mi_enter("MI_varaccess");
    let Some(inq) = nc::ncvarinq(s, c.streams, cdfid, varid)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    if datatype == NC_CHAR || inq.ty == NC_CHAR {
        s.log_pkg_error(
            c,
            Some(PkgSite::VarNonNumeric),
            MI_ERR_NONNUMERIC,
            "Non-numeric datatype",
        )?;
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    let old = s.ncopts;
    s.ncopts = 0;
    let mut buf = Vec::new();
    let st = mi::miattgetstr(s, c, cdfid, varid, b"signtype", MI_MAX_ATTSTR_LEN, &mut buf)?;
    s.ncopts = old;
    let var_sign = mi::sign_from_string(inq.ty, st.as_deref());
    let call_sign = mi::get_sign(datatype, sign);
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
    if fast {
        let x = ncwrite::var_bytes(values);
        if ncwrite::ncvarput(s, c.streams, cdfid, varid, &st_, &ct_, &x)? == MI_ERROR {
            s.mi_return_error(c)?;
            return Ok(MI_ERROR);
        }
        s.mi_return();
        return Ok(0);
    }
    let value_size = nc::nctypelen(s, c.streams, inq.ty)? as i64;
    nc::nctypelen(s, c.streams, datatype)?;
    let pieces = if value_size > 0 {
        mi::var_loop_pieces(&st_, &ct_, value_size)
    } else {
        vec![(st_.clone(), ct_.clone())]
    };
    let mut at = 0usize;
    for (ps, pc) in pieces {
        let n: i64 = pc.iter().product::<i64>().max(0);
        let n = n as usize;
        let Some(mut out) = Typed::new(inq.ty, var_sign) else {
            s.mi_return_error(c)?;
            return Ok(MI_ERROR);
        };
        convert_from(c, values, at, n, datatype, call_sign, &mut out, conv);
        let x = ncwrite::var_bytes(&out);
        if ncwrite::ncvarput(s, c.streams, cdfid, varid, &ps, &pc, &x)? == MI_ERROR {
            s.mi_return_error(c)?;
            return Ok(MI_ERROR);
        }
        at += n;
    }
    s.mi_return();
    Ok(0)
}

/// `mivarput`: `values` (of `datatype`, `sign` as MINC's strings) written to the hyperslab.
pub fn mivarput(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    start: &[i64],
    count: &[i64],
    datatype: i32,
    sign: Option<&[u8]>,
    values: &Typed,
) -> Result<i32, Exit> {
    s.mi_enter("mivarput");
    let sg = mi::sign_from_string(datatype, sign);
    chk!(
        s,
        c,
        mi_varaccess_put(s, c, cdfid, varid, start, count, datatype, sg, values, None)?
    );
    s.mi_return();
    Ok(0)
}

/// `mivarput1`: one value at `mindex`.
pub fn mivarput1(
    s: &mut Session,
    c: &mut Ctx,
    cdfid: i32,
    varid: i32,
    mindex: &[i64],
    datatype: i32,
    sign: Option<&[u8]>,
    value: &Typed,
) -> Result<i32, Exit> {
    s.mi_enter("mivarput1");
    let count = vec![1i64; nc::MAX_VAR_DIMS];
    let sg = mi::sign_from_string(datatype, sign);
    chk!(
        s,
        c,
        mi_varaccess_put(
            s, c, cdfid, varid, mindex, &count, datatype, sg, value, None
        )?
    );
    s.mi_return();
    Ok(0)
}
