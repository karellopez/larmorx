// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
//! MINC 1.5.1's image conversion variables (`libsrc/image_conversion.c`, `dim_conversion.c`;
//! MNI's notice, `LICENSES/LicenseRef-MNI.txt`) as volume_io uses them: an image read in a type
//! of the caller's choice, each slice's values mapped from its own real range (`image-max`,
//! `image-min`) onto the caller's valid range for the whole image's real range, values outside
//! the file's valid range replaced by a fill value.
//!
//! The dimension conversions (flipping, resizing, a vector dimension averaged to a scalar) are
//! checked as MINC checks them; a conversion that would change the image (only a vector image
//! does, with volume_io's settings) is not ported yet: its read fails.

use larmorx_core::defects::{self as d};

use super::libsrc::{
    self as mi, Conversion, MI_DEFAULT_MAX, MI_DEFAULT_MIN, MI_ERR_BADICV, MI_ERR_BADPROP,
    MI_ERR_ICVATTACHED, MI_ERR_ICVNOTATTACHED, MI_ERR_NONNUMERIC, MI_ERR_TOOFEWDIMS, MI_ERROR,
    MI_PRIV_SIGNED, Typed,
};
use super::netcdf::{self as nc, NC_CHAR, NC_DOUBLE, NC_FLOAT, NC_SHORT};
use super::{Ctx, Exit, PkgSite, Session};

pub const MI_ICV_TYPE: i32 = 1;
pub const MI_ICV_SIGN: i32 = 2;
pub const MI_ICV_DO_RANGE: i32 = 3;
pub const MI_ICV_VALID_MAX: i32 = 4;
pub const MI_ICV_VALID_MIN: i32 = 5;
pub const MI_ICV_DO_NORM: i32 = 6;
pub const MI_ICV_USER_NORM: i32 = 7;
pub const MI_ICV_IMAGE_MAX: i32 = 8;
pub const MI_ICV_IMAGE_MIN: i32 = 9;
pub const MI_ICV_NORM_MAX: i32 = 10;
pub const MI_ICV_NORM_MIN: i32 = 11;
pub const MI_ICV_DO_DIM_CONV: i32 = 12;
pub const MI_ICV_DO_SCALAR: i32 = 13;
pub const MI_ICV_XDIM_DIR: i32 = 14;
pub const MI_ICV_YDIM_DIR: i32 = 15;
pub const MI_ICV_ZDIM_DIR: i32 = 16;
pub const MI_ICV_KEEP_ASPECT: i32 = 19;
pub const MI_ICV_DO_FILLVALUE: i32 = 30;
pub const MI_ICV_FILLVALUE: i32 = 31;

const MI_ICV_POSITIVE: i32 = 1;
const MI_ICV_NEGATIVE: i32 = -1;
const MI_ICV_ANYDIR: i32 = 0;
const MI_MAX_IMGDIMS: usize = 100;
/// `MI_MAX_NUM_ICV` (`MAX_NC_OPEN`): the list grows by this many slots.
const MI_MAX_NUM_ICV: usize = 32;

/// An image conversion variable (`mi_icv_type`): the caller's settings and what attaching it to
/// a variable derived.
#[derive(Clone, Debug)]
pub struct Icv {
    pub user_type: i32,
    pub user_sign: i32,
    pub user_do_range: bool,
    pub user_vmax: f64,
    pub user_vmin: f64,
    pub user_do_norm: bool,
    pub user_user_norm: bool,
    pub user_maxvar: Vec<u8>,
    pub user_minvar: Vec<u8>,
    pub user_imgmax: f64,
    pub user_imgmin: f64,
    pub user_do_dimconv: bool,
    pub user_do_scalar: bool,
    pub user_xdim_dir: i32,
    pub user_ydim_dir: i32,
    pub user_zdim_dir: i32,
    pub user_num_imgdims: usize,
    pub user_keep_aspect: bool,
    pub user_do_fillvalue: bool,
    pub user_fillvalue: f64,
    pub user_dim_size: Vec<i64>,
    pub cdfid: i32,
    pub varid: i32,
    pub var_type: i32,
    pub var_dim: Vec<i32>,
    pub var_sign: i32,
    pub var_vmax: f64,
    pub var_vmin: f64,
    pub var_is_vector: bool,
    pub var_vector_size: i64,
    pub var_dim_size: Vec<i64>,
    pub derv_imgmax: f64,
    pub derv_imgmin: f64,
    pub derv_firstdim: i64,
    pub derv_var_float: bool,
    pub derv_usr_float: bool,
    pub derv_dim_flip: Vec<bool>,
    pub derv_dim_grow: Vec<bool>,
    pub derv_dim_scale: Vec<i64>,
    pub derv_dim_off: Vec<i64>,
    pub derv_dim_step: Vec<f64>,
    pub derv_dim_start: Vec<f64>,
    pub derv_do_zero: bool,
    pub imgmaxid: i32,
    pub imgminid: i32,
    pub do_scale: bool,
    pub do_dimconvert: bool,
    pub do_fillvalue: bool,
    pub fill_valid_min: f64,
    pub fill_valid_max: f64,
    pub scale: f64,
    pub offset: f64,
}

fn default_range(_c: &mut Ctx, ty: i32, sign: i32) -> Result<[f64; 2], Exit> {
    Ok(mi::miget_default_range(ty, sign == MI_PRIV_SIGNED))
}

impl Icv {
    fn new() -> Icv {
        let r = mi::default_range_of(NC_SHORT, true);
        Icv {
            user_type: NC_SHORT,
            user_sign: MI_PRIV_SIGNED,
            user_do_range: true,
            user_vmax: r[1],
            user_vmin: r[0],
            user_do_norm: false,
            user_user_norm: false,
            user_maxvar: b"image-max".to_vec(),
            user_minvar: b"image-min".to_vec(),
            user_imgmax: MI_DEFAULT_MAX,
            user_imgmin: MI_DEFAULT_MIN,
            user_do_dimconv: false,
            user_do_scalar: true,
            user_xdim_dir: MI_ICV_POSITIVE,
            user_ydim_dir: MI_ICV_POSITIVE,
            user_zdim_dir: MI_ICV_POSITIVE,
            user_num_imgdims: 2,
            user_keep_aspect: true,
            user_do_fillvalue: false,
            user_fillvalue: -f64::MAX,
            user_dim_size: vec![-1; MI_MAX_IMGDIMS],
            cdfid: MI_ERROR,
            varid: MI_ERROR,
            var_type: 0,
            var_dim: Vec::new(),
            var_sign: 0,
            var_vmax: 0.0,
            var_vmin: 0.0,
            var_is_vector: false,
            var_vector_size: 1,
            var_dim_size: vec![0; MI_MAX_IMGDIMS],
            derv_imgmax: MI_DEFAULT_MAX,
            derv_imgmin: MI_DEFAULT_MIN,
            derv_firstdim: -1,
            derv_var_float: false,
            derv_usr_float: false,
            derv_dim_flip: vec![false; MI_MAX_IMGDIMS],
            derv_dim_grow: vec![true; MI_MAX_IMGDIMS],
            derv_dim_scale: vec![1; MI_MAX_IMGDIMS],
            derv_dim_off: vec![0; MI_MAX_IMGDIMS],
            derv_dim_step: vec![0.0; MI_MAX_IMGDIMS],
            derv_dim_start: vec![0.0; MI_MAX_IMGDIMS],
            derv_do_zero: false,
            imgmaxid: MI_ERROR,
            imgminid: MI_ERROR,
            do_scale: false,
            do_dimconvert: false,
            do_fillvalue: false,
            fill_valid_min: -f64::MAX,
            fill_valid_max: f64::MAX,
            scale: 0.0,
            offset: 0.0,
        }
    }
}

/// `MI_icv_chkid`.
fn chkid(s: &mut Session, c: &mut Ctx, icvid: i32) -> Result<Option<usize>, Exit> {
    s.mi_enter("MI_icv_chkid");
    let ok = usize::try_from(icvid)
        .ok()
        .filter(|&i| i < s.icvs.len() && s.icvs[i].is_some());
    match ok {
        Some(i) => {
            s.mi_return();
            Ok(Some(i))
        }
        None => {
            s.log_pkg_error(c, None, MI_ERR_BADICV, "Illegal icv identifier")?;
            s.mi_return_error(c)?;
            Ok(None)
        }
    }
}

fn icv(s: &mut Session, i: usize) -> &mut Icv {
    s.icvs[i].as_mut().expect("a live icv")
}

/// `miicv_create`.
pub fn miicv_create(s: &mut Session) -> i32 {
    s.mi_enter("miicv_create");
    let slot = match s.icvs.iter().position(|v| v.is_none()) {
        Some(i) => i,
        None => {
            let n = s.icvs.len();
            s.icvs.resize(n + MI_MAX_NUM_ICV, None);
            n
        }
    };
    s.icvs[slot] = Some(Icv::new());
    s.mi_return();
    slot as i32
}

/// `miicv_free`.
pub fn miicv_free(s: &mut Session, c: &mut Ctx, icvid: i32) -> Result<i32, Exit> {
    s.mi_enter("miicv_free");
    let Some(i) = chkid(s, c, icvid)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    if icv(s, i).cdfid != MI_ERROR && miicv_detach(s, c, icvid)? == MI_ERROR {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    s.icvs[i] = None;
    s.mi_return();
    Ok(0)
}

/// `miicv_setdbl`.
pub fn miicv_setdbl(
    s: &mut Session,
    c: &mut Ctx,
    icvid: i32,
    prop: i32,
    value: f64,
) -> Result<i32, Exit> {
    s.mi_enter("miicv_setdbl");
    let Some(i) = chkid(s, c, icvid)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    if icv(s, i).cdfid != MI_ERROR {
        s.log_pkg_error(
            c,
            Some(PkgSite::IcvAttached),
            MI_ERR_ICVATTACHED,
            "Attempt to modify an attached image conversion variable",
        )?;
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    if prop == MI_ICV_TYPE {
        let t = mi::cvt_i32(value);
        let sign = icv(s, i).user_sign;
        icv(s, i).user_type = t;
        nc::nctypelen(s, c.streams, t)?;
        let r = default_range(c, t, sign)?;
        let v = icv(s, i);
        v.user_vmax = r[1];
        v.user_vmin = r[0];
        s.mi_return();
        return Ok(0);
    }
    let v = icv(s, i);
    let b = value != 0.0;
    let dir = |x: f64| {
        let ival = mi::cvt_i32(x);
        if ival == MI_ICV_POSITIVE || ival == MI_ICV_NEGATIVE {
            ival
        } else {
            MI_ICV_ANYDIR
        }
    };
    match prop {
        MI_ICV_DO_RANGE => v.user_do_range = b,
        MI_ICV_VALID_MAX => v.user_vmax = value,
        MI_ICV_VALID_MIN => v.user_vmin = value,
        MI_ICV_DO_NORM => v.user_do_norm = b,
        MI_ICV_USER_NORM => v.user_user_norm = b,
        MI_ICV_IMAGE_MAX => v.user_imgmax = value,
        MI_ICV_IMAGE_MIN => v.user_imgmin = value,
        MI_ICV_DO_FILLVALUE => v.user_do_fillvalue = b,
        MI_ICV_FILLVALUE => v.user_fillvalue = value,
        MI_ICV_DO_DIM_CONV => v.user_do_dimconv = b,
        MI_ICV_DO_SCALAR => v.user_do_scalar = b,
        MI_ICV_XDIM_DIR => v.user_xdim_dir = dir(value),
        MI_ICV_YDIM_DIR => v.user_ydim_dir = dir(value),
        MI_ICV_ZDIM_DIR => v.user_zdim_dir = dir(value),
        MI_ICV_KEEP_ASPECT => v.user_keep_aspect = b,
        MI_ICV_SIGN => {
            s.log_pkg_error(
                c,
                Some(PkgSite::IcvBadProperty),
                MI_ERR_BADPROP,
                "Tried to set icv string property to a number",
            )?;
            s.mi_return_error(c)?;
            return Ok(MI_ERROR);
        }
        _ => {
            s.log_pkg_error(
                c,
                Some(PkgSite::IcvBadProperty),
                MI_ERR_BADPROP,
                "Tried to set unknown or illegal icv property",
            )?;
            s.mi_return_error(c)?;
            return Ok(MI_ERROR);
        }
    }
    s.mi_return();
    Ok(0)
}

/// `miicv_setint`.
pub fn miicv_setint(
    s: &mut Session,
    c: &mut Ctx,
    icvid: i32,
    prop: i32,
    value: i32,
) -> Result<i32, Exit> {
    s.mi_enter("miicv_setint");
    if miicv_setdbl(s, c, icvid, prop, value as f64)? == MI_ERROR {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    s.mi_return();
    Ok(0)
}

/// `miicv_setstr` for the sign (the only string property volume_io sets).
pub fn miicv_setstr_sign(
    s: &mut Session,
    c: &mut Ctx,
    icvid: i32,
    value: &[u8],
) -> Result<i32, Exit> {
    s.mi_enter("miicv_setstr");
    let Some(i) = chkid(s, c, icvid)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    if icv(s, i).cdfid != MI_ERROR {
        s.log_pkg_error(
            c,
            Some(PkgSite::IcvAttached),
            MI_ERR_ICVATTACHED,
            "Attempt to modify an attached image conversion variable",
        )?;
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    let (t, sign) = {
        let v = icv(s, i);
        v.user_sign = mi::sign_from_string(v.user_type, Some(value));
        (v.user_type, v.user_sign)
    };
    let r = default_range(c, t, sign)?;
    let v = icv(s, i);
    v.user_vmax = r[1];
    v.user_vmin = r[0];
    s.mi_return();
    Ok(0)
}

/// `miicv_inqdbl` for the normalisation range (the only properties volume_io inquires about).
pub fn miicv_inq_norm(
    s: &mut Session,
    c: &mut Ctx,
    icvid: i32,
    max: bool,
) -> Result<Option<f64>, Exit> {
    s.mi_enter("miicv_inqdbl");
    let Some(i) = chkid(s, c, icvid)? else {
        s.mi_return_error(c)?;
        return Ok(None);
    };
    let v = icv(s, i);
    let r = if max { v.derv_imgmax } else { v.derv_imgmin };
    s.mi_return();
    Ok(Some(r))
}

/// `MI_icv_get_type`.
fn get_type(s: &mut Session, c: &mut Ctx, i: usize, cdfid: i32, varid: i32) -> Result<bool, Exit> {
    s.mi_enter("MI_icv_get_type");
    let Some(inq) = nc::ncvarinq(s, c.streams, cdfid, varid)? else {
        s.mi_return_error(c)?;
        return Ok(false);
    };
    if inq.dimids.len() > nc::MAX_VAR_DIMS {
        c.notices.add(
            &d::FS_783,
            format!(
                "an image of {} dimensions refused (MINC's arrays hold {})",
                inq.dimids.len(),
                nc::MAX_VAR_DIMS
            ),
        );
        s.mi_return_error(c)?;
        return Ok(false);
    }
    {
        let v = icv(s, i);
        v.var_type = inq.ty;
        v.var_dim = inq.dimids.clone();
    }
    if inq.ty == NC_CHAR {
        s.log_pkg_error(
            c,
            Some(PkgSite::IcvNonNumeric),
            MI_ERR_NONNUMERIC,
            "Non-numeric datatype",
        )?;
        s.mi_return_error(c)?;
        return Ok(false);
    }
    let old = s.ncopts;
    s.ncopts = 0;
    let mut buf = Vec::new();
    let st = mi::miattgetstr(
        s,
        c,
        cdfid,
        varid,
        b"signtype",
        mi::MI_MAX_ATTSTR_LEN,
        &mut buf,
    )?;
    s.ncopts = old;
    let user_type = {
        let v = icv(s, i);
        v.var_sign = mi::sign_from_string(inq.ty, st.as_deref());
        v.user_type
    };
    nc::nctypelen(s, c.streams, inq.ty)?;
    nc::nctypelen(s, c.streams, user_type)?;
    s.mi_return();
    Ok(true)
}

/// `MI_icv_get_norm`.
fn get_norm(s: &mut Session, c: &mut Ctx, i: usize, cdfid: i32) -> Result<bool, Exit> {
    s.mi_enter("MI_icv_get_norm");
    {
        let v = icv(s, i);
        v.derv_var_float = v.var_type == NC_DOUBLE || v.var_type == NC_FLOAT;
        v.derv_usr_float = v.user_type == NC_DOUBLE || v.user_type == NC_FLOAT;
        v.derv_firstdim = -1;
    }
    let old = s.ncopts;
    s.ncopts = 0;
    let (maxvar, minvar) = {
        let v = icv(s, i);
        (v.user_maxvar.clone(), v.user_minvar.clone())
    };
    let imax = nc::ncvarid(s, c.streams, cdfid, &maxvar)?;
    let imin = nc::ncvarid(s, c.streams, cdfid, &minvar)?;
    s.ncopts = old;
    {
        let v = icv(s, i);
        v.imgmaxid = imax;
        v.imgminid = imin;
    }
    if !icv(s, i).user_do_norm {
        let v = icv(s, i);
        v.derv_imgmax = MI_DEFAULT_MAX;
        v.derv_imgmin = MI_DEFAULT_MIN;
    } else {
        if icv(s, i).user_user_norm {
            let v = icv(s, i);
            v.derv_imgmax = v.user_imgmax;
            v.derv_imgmin = v.user_imgmin;
        } else {
            let Some(r) = mi::miget_image_range(s, c, cdfid)? else {
                s.mi_return_error(c)?;
                return Ok(false);
            };
            let v = icv(s, i);
            v.derv_imgmin = r[0];
            v.derv_imgmax = r[1];
        }
        if imin != MI_ERROR && imax != MI_ERROR {
            for vid in [imin, imax] {
                let Some(inq) = nc::ncvarinq(s, c.streams, cdfid, vid)? else {
                    s.mi_return_error(c)?;
                    return Ok(false);
                };
                let v = icv(s, i);
                for &dm in &inq.dimids {
                    for (k, &vd) in v.var_dim.iter().enumerate() {
                        if vd == dm {
                            v.derv_firstdim = v.derv_firstdim.max(k as i64);
                        }
                    }
                }
            }
        }
    }
    s.mi_return();
    Ok(true)
}

/// `miicv_ndattach`.
fn miicv_ndattach(
    s: &mut Session,
    c: &mut Ctx,
    icvid: i32,
    cdfid: i32,
    varid: i32,
) -> Result<i32, Exit> {
    s.mi_enter("miicv_ndattach");
    let Some(i) = chkid(s, c, icvid)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    if icv(s, i).cdfid != MI_ERROR && miicv_detach(s, c, icvid)? == MI_ERROR {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    if !get_type(s, c, i, cdfid, varid)? {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    if !icv(s, i).user_do_range {
        icv(s, i).derv_firstdim = -1;
    } else {
        s.mi_enter("MI_icv_get_vrange");
        let vr = mi::miget_valid_range(s, c, cdfid, varid)?;
        s.mi_return();
        let Some(vr) = vr else {
            s.mi_return_error(c)?;
            return Ok(MI_ERROR);
        };
        {
            let v = icv(s, i);
            v.var_vmin = vr[0];
            v.var_vmax = vr[1];
        }
        if !get_norm(s, c, i, cdfid)? {
            s.mi_return_error(c)?;
            return Ok(MI_ERROR);
        }
    }
    let v = icv(s, i);
    v.var_is_vector = false;
    v.var_vector_size = 1;
    v.derv_do_zero = false;
    for k in 0..v.user_num_imgdims {
        v.derv_dim_flip[k] = false;
        v.derv_dim_grow[k] = true;
        v.derv_dim_scale[k] = 1;
        v.derv_dim_off[k] = 0;
        v.derv_dim_step[k] = 0.0;
        v.derv_dim_start[k] = 0.0;
    }
    v.do_scale = v.user_do_range
        && (v.user_vmax != v.var_vmax
            || v.user_vmin != v.var_vmin
            || (v.user_do_norm && v.user_user_norm)
            || (v.user_do_norm && v.derv_firstdim >= 0));
    if v.derv_usr_float && v.derv_var_float {
        v.do_scale = false;
    }
    v.do_dimconvert = false;
    v.cdfid = cdfid;
    v.varid = varid;
    s.mi_return();
    Ok(0)
}

/// `MI_icv_get_dim` and what it calls (`MI_get_dim_flip`, `MI_get_dim_scale`): the image's fastest
/// dimensions, their flips, scales and offsets.
fn get_dim(s: &mut Session, c: &mut Ctx, i: usize, cdfid: i32) -> Result<bool, Exit> {
    s.mi_enter("MI_icv_get_dim");
    let (ndims, nimg, last) = {
        let v = icv(s, i);
        (
            v.var_dim.len(),
            v.user_num_imgdims,
            v.var_dim.last().copied(),
        )
    };
    if ndims < nimg {
        s.log_pkg_error(
            c,
            Some(PkgSite::IcvTooFewDims),
            MI_ERR_TOOFEWDIMS,
            "Variable has too few dimensions to be an image",
        )?;
        s.mi_return_error(c)?;
        return Ok(false);
    }
    let Some((lastname, vsize)) = nc::ncdiminq(s, c.streams, cdfid, last.unwrap_or(-1))? else {
        s.mi_return_error(c)?;
        return Ok(false);
    };
    let is_vector = lastname == b"vector_dimension";
    {
        let v = icv(s, i);
        v.var_vector_size = vsize;
        v.var_is_vector = is_vector;
    }
    if is_vector && ndims < nimg + 1 {
        s.log_pkg_error(
            c,
            Some(PkgSite::IcvTooFewDimsVector),
            MI_ERR_TOOFEWDIMS,
            "Variable has too few dimensions to be an image",
        )?;
        s.mi_return_error(c)?;
        return Ok(false);
    }
    let first = if is_vector { ndims - 2 } else { ndims - 1 };
    let subsc: Vec<usize> = (0..nimg).map(|k| first - k).collect();
    let mut dimvid = Vec::with_capacity(nimg);
    let mut names = Vec::with_capacity(nimg);
    for k in 0..nimg {
        let dimid = icv(s, i).var_dim[subsc[k]];
        let Some((name, size)) = nc::ncdiminq(s, c.streams, cdfid, dimid)? else {
            s.mi_return_error(c)?;
            return Ok(false);
        };
        icv(s, i).var_dim_size[k] = size;
        let old = s.ncopts;
        s.ncopts = 0;
        dimvid.push(nc::ncvarid(s, c.streams, cdfid, &name)?);
        s.ncopts = old;
        names.push(name);
    }
    // MI_get_dim_flip.
    s.mi_enter("MI_get_dim_flip");
    for k in 0..nimg {
        let dimid = icv(s, i).var_dim[subsc[k]];
        if nc::ncdiminq(s, c.streams, cdfid, dimid)?.is_none() {
            s.mi_return_error(c)?;
            s.mi_return_error(c)?;
            return Ok(false);
        }
        let name = &names[k];
        let v = icv(s, i);
        v.derv_dim_flip[k] = false;
        let dir = if name == b"xspace" || name == b"xfrequency" {
            v.user_xdim_dir
        } else if name == b"yspace" || name == b"yfrequency" {
            v.user_ydim_dir
        } else if name == b"zspace" || name == b"zfrequency" {
            v.user_zdim_dir
        } else {
            MI_ICV_ANYDIR
        };
        if dir != MI_ICV_ANYDIR {
            let mut step = 1.0;
            if dimvid[k] != MI_ERROR {
                let old = s.ncopts;
                s.ncopts = 0;
                mi::miattget1(s, c, cdfid, dimvid[k], b"step", &mut step)?;
                s.ncopts = old;
            }
            let v = icv(s, i);
            if dir == MI_ICV_POSITIVE {
                v.derv_dim_flip[k] = step < 0.0;
            } else if dir == MI_ICV_NEGATIVE {
                v.derv_dim_flip[k] = step > 0.0;
            }
        }
    }
    s.mi_return();
    // MI_get_dim_scale.
    s.mi_enter("MI_get_dim_scale");
    let (mut min_grow, mut min_scale) = (true, 1i64);
    for k in 0..nimg {
        let v = icv(s, i);
        if v.user_dim_size[k] <= 0 {
            v.derv_dim_grow[k] = true;
            v.derv_dim_scale[k] = 1;
        } else {
            v.derv_dim_grow[k] = v.var_dim_size[k] <= v.user_dim_size[k];
            if v.derv_dim_grow[k] {
                v.derv_dim_scale[k] = v.user_dim_size[k] / v.var_dim_size[k].max(1);
            } else {
                v.derv_dim_scale[k] = 1 + (v.var_dim_size[k] - 1) / v.user_dim_size[k];
            }
        }
        let (g, sc) = (v.derv_dim_grow[k], v.derv_dim_scale[k]);
        if k == 0
            || (min_grow && !g)
            || (min_grow && g && min_scale > sc)
            || (!min_grow && !g && min_scale < sc)
        {
            min_grow = g;
            min_scale = sc;
        }
    }
    for k in 0..nimg {
        {
            let v = icv(s, i);
            if v.user_keep_aspect {
                v.derv_dim_grow[k] = min_grow;
                v.derv_dim_scale[k] = min_scale;
            }
            let user_size = if v.user_dim_size[k] <= 0 {
                v.var_dim_size[k]
            } else {
                v.user_dim_size[k]
            };
            v.derv_dim_off[k] = if v.derv_dim_grow[k] {
                (user_size - v.var_dim_size[k] * v.derv_dim_scale[k]) / 2
            } else {
                (user_size - 1 - (v.var_dim_size[k] - 1) / v.derv_dim_scale[k]) / 2
            };
        }
        let old = s.ncopts;
        s.ncopts = 0;
        let mut step = 1.0;
        mi::miattget1(s, c, cdfid, dimvid[k], b"step", &mut step)?;
        let v = icv(s, i);
        if v.derv_dim_flip[k] {
            step *= -1.0;
        }
        v.derv_dim_step[k] = if v.derv_dim_grow[k] {
            step / v.derv_dim_scale[k] as f64
        } else {
            step * v.derv_dim_scale[k] as f64
        };
        let mut start = 0.0;
        mi::miattget1(s, c, cdfid, dimvid[k], b"start", &mut start)?;
        let v = icv(s, i);
        if v.derv_dim_flip[k] {
            start -= step * (v.var_dim_size[k] - 1) as f64;
        }
        v.derv_dim_start[k] = start + (v.derv_dim_step[k] - step) / 2.0
            - v.derv_dim_off[k] as f64 * v.derv_dim_step[k];
        s.ncopts = old;
    }
    s.mi_return();
    s.mi_return();
    Ok(true)
}

/// `miicv_attach`.
pub fn miicv_attach(
    s: &mut Session,
    c: &mut Ctx,
    icvid: i32,
    cdfid: i32,
    varid: i32,
) -> Result<i32, Exit> {
    s.mi_enter("miicv_attach");
    let Some(i) = chkid(s, c, icvid)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    if miicv_ndattach(s, c, icvid, cdfid, varid)? == MI_ERROR {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    if !icv(s, i).user_do_dimconv {
        s.mi_return();
        return Ok(0);
    }
    {
        let v = icv(s, i);
        v.cdfid = MI_ERROR;
        v.varid = MI_ERROR;
    }
    if !get_dim(s, c, i, cdfid)? {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    }
    let v = icv(s, i);
    v.do_dimconvert = v.user_do_scalar && v.var_is_vector;
    let n = v.user_num_imgdims;
    for k in 0..n {
        if v.derv_dim_flip[k] || v.derv_dim_scale[k] != 1 || v.derv_dim_off[k] != 0 {
            v.do_dimconvert = true;
        }
    }
    v.derv_do_zero = false;
    for k in 0..n {
        let user_size = if v.user_dim_size[k] <= 0 {
            v.var_dim_size[k]
        } else {
            v.user_dim_size[k]
        };
        let diff = if v.derv_dim_grow[k] {
            user_size - v.var_dim_size[k] * v.derv_dim_scale[k]
        } else {
            user_size - 1 - (v.var_dim_size[k] - 1) / v.derv_dim_scale[k]
        };
        if v.derv_dim_off[k] != 0 || diff != 0 {
            v.derv_do_zero = true;
        }
    }
    v.cdfid = cdfid;
    v.varid = varid;
    s.mi_return();
    Ok(0)
}

/// `miicv_detach`.
pub fn miicv_detach(s: &mut Session, c: &mut Ctx, icvid: i32) -> Result<i32, Exit> {
    s.mi_enter("miicv_detach");
    let Some(i) = chkid(s, c, icvid)? else {
        s.mi_return_error(c)?;
        return Ok(MI_ERROR);
    };
    let v = icv(s, i);
    if v.cdfid == MI_ERROR {
        s.mi_return();
        return Ok(0);
    }
    v.cdfid = MI_ERROR;
    v.varid = MI_ERROR;
    s.mi_return();
    Ok(0)
}

/// `MI_icv_calc_scale` for a get, at the chunk starting at `coords`.
fn calc_scale(s: &mut Session, c: &mut Ctx, i: usize, coords: &[i64]) -> Result<bool, Exit> {
    s.mi_enter("MI_icv_calc_scale");
    let v = icv(s, i).clone();
    let (mut var_vmax, mut var_vmin) = (v.var_vmax, v.var_vmin);
    let (mut usr_imgmax, mut usr_imgmin, mut var_imgmax, mut var_imgmin);
    if !v.derv_var_float && !v.derv_usr_float && !v.user_do_norm {
        usr_imgmax = MI_DEFAULT_MAX;
        var_imgmax = MI_DEFAULT_MAX;
        usr_imgmin = MI_DEFAULT_MIN;
        var_imgmin = MI_DEFAULT_MIN;
    } else {
        let mut slice_imgmax = MI_DEFAULT_MAX;
        let mut slice_imgmin = MI_DEFAULT_MIN;
        if (!v.derv_var_float || !v.user_do_norm)
            && v.imgmaxid != MI_ERROR
            && v.imgminid != MI_ERROR
        {
            for (vid, out) in [
                (v.imgmaxid, &mut slice_imgmax),
                (v.imgminid, &mut slice_imgmin),
            ] {
                let Some(mm) = mi::mitranslate_coords(s, c, v.cdfid, v.varid, coords, vid)? else {
                    s.mi_return_error(c)?;
                    return Ok(false);
                };
                if mm.iter().any(|x| x.is_none()) {
                    c.notices.add(
                        &d::FS_785,
                        "an image-max or image-min dimension the image does not have read at index 0 (MINC reads a coordinate it never set)",
                    );
                }
                let mm: Vec<i64> = mm.into_iter().map(|x| x.unwrap_or(0)).collect();
                let Some(val) = mi::mivarget1(s, c, v.cdfid, vid, &mm)? else {
                    s.mi_return_error(c)?;
                    return Ok(false);
                };
                *out = val;
            }
        }
        if v.user_do_norm {
            usr_imgmax = v.derv_imgmax;
            usr_imgmin = v.derv_imgmin;
        } else {
            usr_imgmax = slice_imgmax;
            usr_imgmin = slice_imgmin;
        }
        if v.derv_var_float {
            var_imgmax = var_vmax;
            var_imgmin = var_vmin;
        } else {
            var_imgmax = slice_imgmax;
            var_imgmin = slice_imgmin;
        }
    }
    if v.derv_var_float {
        var_imgmax = var_vmax;
        var_imgmin = var_vmin;
    }
    let (mut usr_vmax, mut usr_vmin) = if v.derv_usr_float {
        (usr_imgmax, usr_imgmin)
    } else {
        (v.user_vmax, v.user_vmin)
    };
    if v.derv_usr_float {
        usr_imgmax = MI_DEFAULT_MAX;
        usr_vmax = MI_DEFAULT_MAX;
        usr_imgmin = MI_DEFAULT_MIN;
        usr_vmin = MI_DEFAULT_MIN;
    }
    if v.derv_var_float {
        var_imgmax = MI_DEFAULT_MAX;
        var_vmax = MI_DEFAULT_MAX;
        var_imgmin = MI_DEFAULT_MIN;
        var_vmin = MI_DEFAULT_MIN;
    }
    let denom = usr_imgmax - usr_imgmin;
    let usr_scale = if denom != 0.0 {
        (usr_vmax - usr_vmin) / denom
    } else {
        0.0
    };
    let denom = var_vmax - var_vmin;
    let scale = if denom != 0.0 {
        usr_scale * (var_imgmax - var_imgmin) / denom
    } else {
        0.0
    };
    let offset = usr_vmin - scale * var_vmin + usr_scale * (var_imgmin - usr_imgmin);
    let w = icv(s, i);
    w.scale = scale;
    w.offset = offset;
    if scale == 0.0 {
        if w.derv_var_float && w.derv_usr_float {
            w.do_scale = false;
            w.do_fillvalue = false;
        } else {
            w.do_fillvalue = true;
        }
    }
    s.mi_return();
    Ok(true)
}

/// `miicv_get`: the hyperslab at `start` / `count` in the ICV's type (`MI_icv_access`), or
/// `None` (`MI_ERROR`).
pub fn miicv_get(
    s: &mut Session,
    c: &mut Ctx,
    icvid: i32,
    start: &[i64],
    count: &[i64],
) -> Result<Option<Typed>, Exit> {
    s.mi_enter("miicv_get");
    let Some(i) = chkid(s, c, icvid)? else {
        s.mi_return_error(c)?;
        return Ok(None);
    };
    s.mi_enter("MI_icv_access");
    if icv(s, i).cdfid == MI_ERROR {
        s.log_pkg_error(
            c,
            Some(PkgSite::IcvNotAttached),
            MI_ERR_ICVNOTATTACHED,
            "ICV is not attached to an image variable",
        )?;
        s.mi_return_error(c)?;
        s.mi_return_error(c)?;
        return Ok(None);
    }
    if icv(s, i).do_dimconvert {
        // A vector image averaged to scalars (or a flip, a resize): not ported yet.
        c.streams
            .err("larmorx: MINC's dimension conversion of a vector image is not ported yet\n");
        s.mi_return_error(c)?;
        s.mi_return_error(c)?;
        return Ok(None);
    }
    let v = icv(s, i).clone();
    // MI_icv_access's chunk size: the ICV type's size.
    nc::nctypelen(s, c.streams, v.user_type)?;
    let ndims = v.var_dim.len();
    let var_start: Vec<i64> = (0..ndims).map(|k| start[k]).collect();
    let var_count: Vec<i64> = (0..ndims).map(|k| count[k]).collect();
    let var_end: Vec<i64> = (0..ndims).map(|k| var_start[k] + var_count[k]).collect();
    let mut chunk_start = var_start.clone();
    let mut chunk_count = vec![1i64; ndims];
    let lo = (v.derv_firstdim + 1).max(0) as usize;
    if lo < ndims {
        chunk_count[lo..ndims].copy_from_slice(&var_count[lo..ndims]);
    }
    let firstdim = v.derv_firstdim.max(0) as usize;
    // A type netCDF does not know has no buffer: MINC's reads below fail with its messages.
    let mut out = Typed::new(v.user_type, v.user_sign);
    if ndims == 0 {
        s.mi_return();
        s.mi_return();
        return Ok(out);
    }
    while chunk_start[0] < var_end[0] {
        {
            let w = icv(s, i);
            w.do_fillvalue = w.user_do_fillvalue;
            w.fill_valid_min = w.var_vmin;
            w.fill_valid_max = w.var_vmax;
        }
        if icv(s, i).do_scale && !calc_scale(s, c, i, &chunk_start)? {
            s.mi_return_error(c)?;
            s.mi_return_error(c)?;
            return Ok(None);
        }
        let w = icv(s, i).clone();
        let conv = Conversion {
            do_scale: w.do_scale,
            scale: w.scale,
            offset: w.offset,
            do_fillvalue: w.do_fillvalue,
            fillvalue: w.user_fillvalue,
            fill_valid_min: w.fill_valid_min,
            fill_valid_max: w.fill_valid_max,
        };
        let Some(part) = mi::mi_varaccess_get(
            s,
            c,
            w.cdfid,
            w.varid,
            &chunk_start,
            &chunk_count,
            w.user_type,
            w.user_sign,
            Some(&conv),
        )?
        else {
            s.mi_return_error(c)?;
            s.mi_return_error(c)?;
            return Ok(None);
        };
        match out.as_mut() {
            Some(o) => append(o, part),
            None => out = Some(part),
        }
        chunk_start[firstdim] += chunk_count[firstdim];
        let mut k = firstdim;
        while k > 0 && chunk_start[k] >= var_end[k] {
            chunk_start[k] = var_start[k];
            chunk_start[k - 1] += 1;
            k -= 1;
        }
    }
    s.mi_return();
    s.mi_return();
    Ok(out)
}

fn append(out: &mut Typed, part: Typed) {
    match (out, part) {
        (Typed::U8(a), Typed::U8(b)) => a.extend(b),
        (Typed::I8(a), Typed::I8(b)) => a.extend(b),
        (Typed::U16(a), Typed::U16(b)) => a.extend(b),
        (Typed::I16(a), Typed::I16(b)) => a.extend(b),
        (Typed::U32(a), Typed::U32(b)) => a.extend(b),
        (Typed::I32(a), Typed::I32(b)) => a.extend(b),
        (Typed::F32(a), Typed::F32(b)) => a.extend(b),
        (Typed::F64(a), Typed::F64(b)) => a.extend(b),
        _ => unreachable!("an icv read in its own type"),
    }
}
