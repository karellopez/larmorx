// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 26.3.05: src/thd_niftiread.c (THD_open_nifti, THD_load_nifti),
// src/thd_niftiwrite.c (populate_nifti_image, get_slice_timing_pattern, space_to_NIFTI_code),
// src/nifti/nifti2/nifti2_io.c (the header conversion of nifti_image_read and
// nifti_quatern_to_dmat44), src/thd_floatscan.c (thd_floatscan), src/edt_fullcopy.c
// (EDIT_full_copy), src/thd_atlas.c (THD_get_generic_space, with the space table of
// src/AFNI_atlas_spaces.niml), src/thd_coords.c (THD_report_obliquity), src/thd_writedset.c
// (the AFNI_ALLOW_MILLISECONDS and one-row tests of THD_write_3dim_dataset) and the
// DSET_UNMSEC macro of src/3ddata.h.
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released under the GNU GPL version 2 or any later version. thd_niftiread.c,
// thd_niftiwrite.c, thd_atlas.c and the NIfTI library were written at the NIH and are in the
// public domain.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! NIfTI files as AFNI reads and writes them, for 3dTshift.
//!
//! [`read`] gives the dataset 3dTshift works on: `EDIT_full_copy` of the dataset that
//! `THD_open_nifti` opened and `THD_load_nifti` loaded. That order matters:
//!
//! - **The datum** (byte, short or float) is chosen when the header is opened: uint8 and int16
//!   stay byte and short unless `scl_slope` and `scl_inter` are both non-zero; float32 stays
//!   float; int8, uint16, int32, uint32 and float64 become float.
//! - **The brick factors are the ones set when the header was opened:** the slope, for data
//!   that were not converted, unless it is 0 or 1. `EDIT_full_copy` copies them before the
//!   data are loaded, and loading changes only the source dataset's factors.
//! - **The values are the loaded ones:** converted to float where needed, non-finite floats
//!   set to 0, and scaled (`slope*x + inter`, in double) when the slope is non-zero and the
//!   intercept is non-zero or the data were converted.
//!
//! So float32 data with both a slope and an intercept are scaled when loaded *and* keep the
//! slope as their brick factor: AFNI applies the slope twice (`docs/findings/afni-tshift.md`).
//!
//! [`write`] writes a dataset as `THD_write_nifti` does, after `DSET_UNMSEC` turns milliseconds
//! into seconds.
//!
//! The geometry is carried as AFNI carries it ([`crate::axes`]): the matrix AFNI took from the
//! header (sform, else qform, else the voxel sizes) as `ijk_to_dicom_real`, and the axes'
//! orientations, steps and origins.
//!
//! An AFNI header extension (code 4) in the input replaces the time axis, the matrix and the
//! space, as AFNI applies it ([`crate::afni_ext`]); its other attributes are ignored, and no
//! extension is written (as AFNI writes with `AFNI_NIFTI_NOEXT=YES`).
//!
//! **AFNI's environment variables** come from the [`Settings`] table ([`crate::environ`]):
//! reading follows `AFNI_NOREALPATH` (the file names in messages), `AFNI_NIFTI_SCALE`,
//! `AFNI_NIFTI_TYPE_WARN`, `AFNI_NIFTI_PRIORITY`, `AFNI_NIFTI_VIEW`, `AFNI_DEFAULT_STD_SPACE`
//! and `AFNI_NO_OBLIQUE_WARNING`; writing follows `AFNI_NIFTI_WRITE_TYPE` and
//! `AFNI_ALLOW_MILLISECONDS` (AFNI-19).
//!
//! **Upstream defects (D13).** Four of AFNI's reading and writing rules are defects that
//! larmorx-gpl corrects by default and reproduces with [`UpstreamDefects::Reproduce`]
//! (`docs/findings/upstream-defects.md`): float32 data with a slope and an intercept scaled
//! twice (AFNI-02, [`read`]); metre and micrometre units leaving the origin, and an oblique
//! matrix, unconverted (AFNI-07, [`geometry`]); slice times all within 0.003 of 0 written with
//! `slice_end` 0 (AFNI-08, [`slice_fields`]); and a matrix within about 0.01° to 0.024° of plumb
//! written plumb (AFNI-16, [`write`], [`axes::snaps`]). A sixth comes with an environment
//! variable: `AFNI_NIFTI_SCALE=NO` keeps uint8 and int16 data with a slope and an intercept as
//! stored and drops the intercept (AFNI-23, [`DroppedIntercept`]); the default writes it back
//! as the output's `scl_inter`. Header slice code 2 giving every slice time 0 (AFNI-03) was
//! AFNI 25.2.09's; AFNI 26.3.05 reads the code as NIfTI defines it, as larmorx corrected it,
//! and so do both modes ([`slice_offsets`]).

use std::path::{Path, PathBuf};

use larmorx_core::array::DynArray;
use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_core::element::DataType;
use larmorx_core::ndarray::{ArrayD, ArrayViewD, IxDyn, ShapeBuilder};
use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, ReadOptions, Scaling, WriteOptions};

use crate::afni_ext;
use crate::axes::{self, Axes, ORI_I2S, ORI_L2R, ORI_P2A};
use crate::cnum::fmt_g;
use crate::environ::Settings;
use crate::message::Message;
use crate::x87;

/// The voxels of a dataset in one of AFNI's storage types (`MRI_byte`, `MRI_short`,
/// `MRI_float`), x fastest, then y, z and the sub-brick (time point).
#[derive(Clone, Debug, PartialEq)]
pub enum BrickData {
    Byte(Vec<u8>),
    Short(Vec<i16>),
    Float(Vec<f32>),
}

impl BrickData {
    pub fn len(&self) -> usize {
        match self {
            BrickData::Byte(v) => v.len(),
            BrickData::Short(v) => v.len(),
            BrickData::Float(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The NIfTI data type `populate_nifti_image` writes for the datum.
    pub fn data_type(&self) -> DataType {
        match self {
            BrickData::Byte(_) => DataType::U8,
            BrickData::Short(_) => DataType::I16,
            BrickData::Float(_) => DataType::F32,
        }
    }
}

/// `UNITS_*_TYPE` of a time axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUnits {
    Sec,
    Msec,
    Hz,
}

impl TimeUnits {
    /// `UNITS_TYPE_LABEL`.
    pub fn label(self) -> &'static str {
        match self {
            TimeUnits::Sec => "s",
            TimeUnits::Msec => "ms",
            TimeUnits::Hz => "Hz",
        }
    }
}

/// A dataset's time axis (`THD_timeaxis`): the TR (`ttdel`), the time origin (`ttorg`), their
/// units, and the slice time offsets (`toff_sl`, one per slice) if any.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeAxis {
    pub ttdel: f32,
    pub ttorg: f32,
    pub units: TimeUnits,
    pub toff_sl: Option<Vec<f32>>,
}

/// A dataset's geometry: its axes and `ijk_to_dicom_real` ([`crate::axes`]), and the xform
/// code `space_to_NIFTI_code` gives.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub axes: Axes,
    pub code: i32,
}

/// A dataset as 3dTshift holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Dataset {
    /// `nx`, `ny`, `nz`.
    pub nxyz: [usize; 3],
    /// The number of sub-bricks (time points).
    pub nvals: usize,
    pub data: BrickData,
    /// The brick factor of each sub-brick (0: none).
    pub factors: Vec<f32>,
    pub taxis: Option<TimeAxis>,
    pub geometry: Geometry,
    /// AFNI-23: the intercept `AFNI_NIFTI_SCALE=NO` dropped when the file was read, if any.
    pub dropped_inter: Option<DroppedIntercept>,
}

/// AFNI-23: uint8 or int16 data with both a slope and an intercept that `AFNI_NIFTI_SCALE=NO`
/// kept as stored. AFNI keeps the slope as their brick factor (none for a slope of 1) and drops
/// the intercept, which its brick factors cannot hold, so the values it works on and writes
/// are off by the intercept. By default larmorx-gpl writes the intercept back as the output's
/// `scl_inter` ([`write`]), wherever the values written still carry it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DroppedIntercept {
    /// The NIfTI data type code (uint8 or int16).
    pub datatype: i16,
    /// `scl_slope` as read.
    pub slope: f64,
    /// `scl_inter` as read.
    pub inter: f64,
    /// Whether the values written carry the intercept: true when read; false once `-rlt` has
    /// removed it from every volume with the trend, which leaves AFNI's output right.
    pub carried: bool,
}

impl DroppedIntercept {
    /// The data type's name in messages.
    pub fn type_name(&self) -> &'static str {
        datatype_string(self.datatype)
    }

    /// The values `THD_load_nifti` gives such data without the variable: converted to float,
    /// then `slope * x + inter` in double, rounded to float (non-finite results 0).
    pub fn scaled(&self, data: &BrickData) -> Vec<f32> {
        let f = |x: f32| {
            let v = (self.slope * f64::from(x) + self.inter) as f32;
            if v.is_finite() { v } else { 0.0 }
        };
        match data {
            BrickData::Byte(v) => v.iter().map(|&x| f(f32::from(x))).collect(),
            BrickData::Short(v) => v.iter().map(|&x| f(f32::from(x))).collect(),
            BrickData::Float(v) => v.iter().map(|&x| f(x)).collect(),
        }
    }
}

impl Dataset {
    /// `THD_need_brick_factor`: whether some sub-brick has a factor other than 0 and 1.
    pub fn need_brick_factor(&self) -> bool {
        self.factors.iter().any(|&f| f != 0.0 && f != 1.0)
    }
}

/// A NIfTI file could not be opened as an AFNI dataset.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error(transparent)]
    Nifti(#[from] nifti::Error),
    /// The reasons AFNI prints before giving up on a file.
    #[error("{0}")]
    Afni(String),
    #[error("{0}: not supported by larmorx-gpl")]
    Unsupported(String),
}

/// The NIfTI datatype codes (`nifti1.h`).
mod dt {
    pub const UINT8: i16 = 2;
    pub const INT16: i16 = 4;
    pub const INT32: i16 = 8;
    pub const FLOAT32: i16 = 16;
    pub const COMPLEX64: i16 = 32;
    pub const FLOAT64: i16 = 64;
    pub const RGB24: i16 = 128;
    pub const INT8: i16 = 256;
    pub const UINT16: i16 = 512;
    pub const UINT32: i16 = 768;
}

/// `nifti_datatype_string`.
fn datatype_string(code: i16) -> &'static str {
    match code {
        0 => "UNKNOWN",
        1 => "BINARY",
        2 => "UINT8",
        4 => "INT16",
        8 => "INT32",
        16 => "FLOAT32",
        32 => "COMPLEX64",
        64 => "FLOAT64",
        128 => "RGB24",
        256 => "INT8",
        512 => "UINT16",
        768 => "UINT32",
        1024 => "INT64",
        1280 => "UINT64",
        1536 => "FLOAT128",
        1792 => "COMPLEX128",
        2048 => "COMPLEX256",
        2304 => "RGBA32",
        _ => "**ILLEGAL**",
    }
}

/// `FIXED_FLOAT(x)`: `x`, or 0 if it is not finite.
fn fixed(x: f64) -> f64 {
    if x.is_finite() { x } else { 0.0 }
}

/// `thd_floatscan`: non-finite values become 0.
pub fn floatscan(v: &mut [f32]) {
    for x in v {
        if !x.is_finite() {
            *x = 0.0;
        }
    }
}

/// `NIFTI_UNITS_*` (`xyzt_units` bits).
const UNITS_METER: i32 = 1;
const UNITS_MM: i32 = 2;
const UNITS_MICRON: i32 = 3;
const UNITS_SEC: i32 = 8;
const UNITS_MSEC: i32 = 16;
const UNITS_USEC: i32 = 24;

/// The header after `nifti_image_read`'s conversion (`nifti_convert_n1hdr2nim` and its NIfTI-2
/// twin): bad dimensions and grid spacings fixed, non-finite floats zeroed (`FIXED_FLOAT`).
struct Nim {
    dim: [i64; 8],
    pixdim: [f64; 8],
    datatype: i16,
    scl_slope: f64,
    scl_inter: f64,
    toffset: f64,
    xyz_units: i32,
    time_units: i32,
    slice_dim: i32,
    slice_code: i64,
    slice_start: i64,
    slice_end: i64,
    slice_duration: f64,
    qform_code: i32,
    sform_code: i32,
    qto_xyz: [[f64; 4]; 4],
    sto_xyz: [[f64; 4]; 4],
}

/// `nifti_quatern_to_dmat44`, with its `long double` operations (`1.0l - (b*b + c*c + d*d)`,
/// `1.0l / sqrt(...)`, the products `2.0l * (b*c - a*d) * yd`) in x87 extended precision, as
/// AFNI's x86-64 build computes them ([`x87`]). (Their double rounding changes the last bit of
/// a matrix entry now and then; AFNI keeps the matrix in float, which hides it almost always.)
fn quatern_to_dmat44(
    qb: f64,
    qc: f64,
    qd: f64,
    offset: [f64; 3],
    d: [f64; 3],
    qfac: f64,
) -> [[f64; 4]; 4] {
    let (mut b, mut c, mut dd) = (qb, qc, qd);
    let mut a = x87::add(1.0, -(b * b + c * c + dd * dd));
    if x87::less_than_1e_minus_7_extended(a) {
        a = x87::div(1.0, (b * b + c * c + dd * dd).sqrt());
        b *= a;
        c *= a;
        dd *= a;
        a = 0.0;
    } else {
        a = a.sqrt();
    }
    let xd = if d[0] > 0.0 { d[0] } else { 1.0 };
    let yd = if d[1] > 0.0 { d[1] } else { 1.0 };
    let mut zd = if d[2] > 0.0 { d[2] } else { 1.0 };
    if qfac < 0.0 {
        zd = -zd;
    }
    // `2.0l * x * y`: the doubling is exact, the product with the step is rounded twice.
    let twice = |x: f64, step: f64| x87::mul(2.0 * x, step);
    [
        [
            (a * a + b * b - c * c - dd * dd) * xd,
            twice(b * c - a * dd, yd),
            twice(b * dd + a * c, zd),
            offset[0],
        ],
        [
            twice(b * c + a * dd, xd),
            (a * a + c * c - b * b - dd * dd) * yd,
            twice(c * dd - a * b, zd),
            offset[1],
        ],
        [
            twice(b * dd - a * c, xd),
            twice(c * dd + a * b, yd),
            (a * a + dd * dd - c * c - b * b) * zd,
            offset[2],
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

impl Nim {
    fn from_header(h: &NiftiHeader) -> Result<Nim, String> {
        if h.datatype == 0 || h.datatype == 1 {
            return Err("bad datatype".into());
        }
        let mut dim = h.dim;
        if dim[1] <= 0 {
            return Err("bad dim[1]".into());
        }
        let ndim = dim[0].clamp(0, 7) as usize;
        for d in dim.iter_mut().take(ndim + 1).skip(2) {
            if *d <= 0 {
                *d = 1;
            }
        }
        for d in dim.iter_mut().skip(ndim + 1) {
            if *d != 1 && *d != 0 {
                *d = 1;
            }
        }
        let mut pixdim = h.pixdim;
        for p in pixdim.iter_mut().take(ndim + 1).skip(1) {
            if *p == 0.0 || !p.is_finite() {
                *p = 1.0;
            }
        }
        let qto_xyz = if h.qform_code <= 0 {
            [
                [pixdim[1], 0.0, 0.0, 0.0],
                [0.0, pixdim[2], 0.0, 0.0],
                [0.0, 0.0, pixdim[3], 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]
        } else {
            let qfac = if h.pixdim[0] < 0.0 { -1.0 } else { 1.0 };
            quatern_to_dmat44(
                fixed(h.quatern_b),
                fixed(h.quatern_c),
                fixed(h.quatern_d),
                [fixed(h.qoffset_x), fixed(h.qoffset_y), fixed(h.qoffset_z)],
                [pixdim[1], pixdim[2], pixdim[3]],
                qfac,
            )
        };
        let sto_xyz = [h.srow_x, h.srow_y, h.srow_z, [0.0, 0.0, 0.0, 1.0]];
        // NIfTI-1 stores slice_code as a (signed) char; NIfTI-2 as an int.
        let slice_code = match h.version {
            NiftiVersion::V1 => i64::from(h.slice_code as u8 as i8),
            NiftiVersion::V2 => i64::from(h.slice_code),
        };
        Ok(Nim {
            dim,
            pixdim,
            datatype: h.datatype,
            scl_slope: fixed(h.scl_slope),
            scl_inter: fixed(h.scl_inter),
            toffset: fixed(h.toffset),
            xyz_units: h.xyzt_units & 0x07,
            time_units: h.xyzt_units & 0x38,
            slice_dim: i32::from((h.dim_info >> 4) & 0x03),
            slice_code,
            slice_start: h.slice_start,
            slice_end: h.slice_end,
            slice_duration: fixed(h.slice_duration),
            qform_code: h.qform_code.max(0),
            sform_code: h.sform_code.max(0),
            qto_xyz,
            sto_xyz,
        })
    }
}

/// AFNI's views (`VIEW_ORIGINAL_TYPE`, `VIEW_ACPCALIGNED_TYPE`, `VIEW_TALAIRACH_TYPE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Orig,
    Acpc,
    Tlrc,
}

/// `NIFTI_default_view`: `AFNI_NIFTI_VIEW` (`tlrc`, `orig` or `acpc`, any case), else orig.
fn default_view(settings: &Settings) -> View {
    match settings.get("AFNI_NIFTI_VIEW") {
        Some(v) if v.eq_ignore_ascii_case("TLRC") => View::Tlrc,
        Some(v) if v.eq_ignore_ascii_case("ACPC") => View::Acpc,
        _ => View::Orig,
    }
}

/// The template space `THD_open_nifti` gives a dataset whose transform has `form_code` (0 when
/// it has none): `NIFTI_code_to_view` for its view, then `NIFTI_code_to_space`. Codes 3 and 4
/// are TLRC and MNI; codes 0 and 1 the orig view, so ORIG; code 5 the tlrc view; code 2 and
/// the codes NIfTI does not define (with a warning) the view `AFNI_NIFTI_VIEW` names (orig by
/// default). A tlrc view's space is `AFNI_DEFAULT_STD_SPACE` (`TT_whereami_default_spc_name`,
/// TLRC by default), an acpc view's ACPC.
fn form_space(form_code: i32, settings: &Settings, messages: &mut Vec<Message>) -> String {
    let view = match form_code {
        3..=5 => View::Tlrc,
        0 | 1 => View::Orig,
        2 => default_view(settings),
        code => {
            messages.push(Message::Warning(format!(
                "I don't think this sform code '{code}' is allowed!"
            )));
            default_view(settings)
        }
    };
    match form_code {
        3 => "TLRC".to_owned(),
        4 => "MNI".to_owned(),
        _ => match view {
            View::Orig => "ORIG".to_owned(),
            View::Acpc => "ACPC".to_owned(),
            View::Tlrc => settings
                .get("AFNI_DEFAULT_STD_SPACE")
                .unwrap_or("TLRC")
                .to_owned(),
        },
    }
}

/// Millimetres per spatial unit of the header, the factor `THD_open_nifti` applies to the voxel
/// sizes: 1000 for metres, 0.001 for micrometres, else 1.
fn millimetres_per_unit(nim: &Nim) -> f64 {
    match nim.xyz_units {
        UNITS_METER => 1000.0,
        UNITS_MICRON => 0.001,
        _ => 1.0,
    }
}

/// `THD_open_nifti`'s geometry: the matrix it stores in `ijk_to_dicom_real` (a float `mat44`,
/// in DICOM coordinates), the axes it derives from it, and the code.
///
/// AFNI converts the voxel sizes (the axes' steps) of a header in metres or micrometres to
/// millimetres, but neither the matrix nor the origins. So a plumb grid comes out with its
/// origin unconverted, an oblique one with its whole sform unconverted ([`crate::axes`]):
/// AFNI-07, reproduced by [`UpstreamDefects::Reproduce`]. [`UpstreamDefects::Fix`] converts the
/// matrix and the origins too, in double before they are rounded to float.
///
/// With both a qform and an sform, `priority` (`b'S'`, AFNI's default, or `b'Q'`, from
/// `AFNI_NIFTI_PRIORITY`) chooses. The code is that of the template space ([`form_space`]).
fn geometry(
    nim: &Nim,
    defects: UpstreamDefects,
    priority: u8,
    settings: &Settings,
    messages: &mut Vec<Message>,
    path: &str,
) -> Geometry {
    let unit = millimetres_per_unit(nim);
    // The matrix and the origins: as read (AFNI), or converted to millimetres (the fix).
    let convert = if defects.fix() { unit } else { 1.0 };
    let scaled = |v: f64| if convert == 1.0 { v } else { v * convert };
    let (use_q, use_s) = match (nim.qform_code > 0, nim.sform_code > 0) {
        (true, true) => (priority == b'Q', priority != b'Q'),
        (true, false) => (true, false),
        (false, true) => (false, true),
        (false, false) => {
            messages.push(Message::Warning(format!(
                "NO spatial transform (neither qform nor sform), in NIfTI file '{path}'"
            )));
            (false, false)
        }
    };
    let form_code = if use_q {
        nim.qform_code
    } else if use_s {
        nim.sform_code
    } else {
        0
    };
    let code = space_code(&form_space(form_code, settings, messages));
    // `ijk_to_dicom44`: x and y negated (AFNI works in DICOM's RAI coordinates).
    let dicom = |m: &[[f64; 4]; 4]| -> [[f32; 4]; 3] {
        let row = |r: usize, sign: f64| m[r].map(|v| scaled(sign * v) as f32);
        [row(0, -1.0), row(1, -1.0), row(2, 1.0)]
    };
    // `orgx = (ORIENT_xyz[o] == 'z') ? m[2][3] : -m[row][3]` for each axis, as float.
    let origins = |m: &[[f64; 4]; 4], orient: Option<[u8; 3]>| -> [f32; 3] {
        let Some(orient) = orient else {
            return [0.0; 3];
        };
        orient.map(|o| {
            let c = axes::coordinate(o);
            let t = if c == 2 { m[2][3] } else { -m[c][3] };
            scaled(t) as f32
        })
    };
    // `(ORIENT_sign[o] == '+') ? d : -d`.
    let signed = |orient: Option<[u8; 3]>, d: [f32; 3]| -> [f32; 3] {
        let Some(orient) = orient else {
            return d;
        };
        [0, 1, 2].map(|i| {
            if axes::positive(orient[i]) {
                d[i]
            } else {
                -d[i]
            }
        })
    };
    if use_q {
        let m = &nim.qto_xyz;
        let r = [
            [-m[0][0], -m[0][1], -m[0][2]],
            [-m[1][0], -m[1][1], -m[1][2]],
            [m[2][0], m[2][1], m[2][2]],
        ]
        .map(|row| row.map(|v| v as f32));
        let orient = Some(axes::matrix_to_orientation(r));
        // `nim->dx *= 1000.0` (double), stored in a float THD_fvec3.
        let steps = [1, 2, 3].map(|i| {
            if unit == 1.0 {
                nim.pixdim[i] as f32
            } else {
                (nim.pixdim[i] * unit) as f32
            }
        });
        Geometry {
            axes: Axes {
                real: dicom(m),
                orient,
                del: signed(orient, steps),
                org: origins(m, orient),
            },
            code,
        }
    } else if use_s {
        let m = &nim.sto_xyz;
        let orient = axes::nifti_orientation([0, 1, 2].map(|i| [m[i][0], m[i][1], m[i][2]]));
        // `dxtmp = sqrt(...)` (float), then `dxtmp *= 1000.0`.
        let steps = [0, 1, 2].map(|j| {
            let norm = (m[0][j] * m[0][j] + m[1][j] * m[1][j] + m[2][j] * m[2][j]).sqrt() as f32;
            if unit == 1.0 {
                norm
            } else {
                (f64::from(norm) * unit) as f32
            }
        });
        Geometry {
            axes: Axes {
                real: dicom(m),
                orient,
                del: signed(orient, steps),
                org: origins(m, orient),
            },
            code,
        }
    } else {
        // No transform: orientation LPI, the voxel sizes as steps (1 if not positive), the
        // origin at 0, and the scaled identity as the matrix (not negated).
        let step = |i: usize| {
            let p = if nim.pixdim[i] > 0.0 {
                nim.pixdim[i] as f32
            } else {
                1.0
            };
            if unit == 1.0 {
                p
            } else {
                (f64::from(p) * unit) as f32
            }
        };
        let (dx, dy, dz) = (step(1), step(2), step(3));
        let orient = Some([ORI_L2R, ORI_P2A, ORI_I2S]);
        Geometry {
            axes: Axes {
                real: [
                    [dx, 0.0, 0.0, 0.0],
                    [0.0, dy, 0.0, 0.0],
                    [0.0, 0.0, dz, 0.0],
                ],
                orient,
                del: signed(orient, [dx, dy, dz]),
                org: [0.0; 3],
            },
            code,
        }
    }
}

/// The slice offsets `THD_open_nifti` builds from `slice_code`, `slice_start`, `slice_end`
/// and `slice_duration` (already in seconds), or `None` if the header gives none.
///
/// Code 2 (`NIFTI_SLICE_SEQ_DEC`) visits the slices from `slice_end` down to `slice_start`, as
/// NIfTI defines it and AFNI 26.3.05 reads it (`thd_niftiread.c:652`). AFNI 25.2.09's loop ran
/// `for( kk=slice_end ; kk >= slice_end ; kk-- )`, so every slice stayed at 0 (AFNI-03, which
/// larmorx corrected; both modes now follow 26.3.05).
fn slice_offsets(nim: &Nim, nz: i64, duration: f64) -> Option<Vec<f32>> {
    let (start, end) = (nim.slice_start, nim.slice_end);
    if !(nim.slice_dim == 3
        && nim.slice_code > 0
        && nim.slice_duration > 0.0
        && start >= 0
        && start < nz
        && end > start
        && end < nz)
    {
        return None;
    }
    let mut toff = vec![0.0f32; nz as usize];
    let mut tsl = 0.0f32;
    // `tsl += nim->slice_duration`: a float plus a double, stored as float.
    let mut put = |kk: i64| {
        toff[kk as usize] = tsl;
        tsl = (f64::from(tsl) + duration) as f32;
    };
    let up = |from: i64| (from..=end).step_by(2);
    let down = |from: i64| (start..=from.max(start - 1)).rev().step_by(2);
    match nim.slice_code {
        1 => (start..=end).for_each(&mut put),
        // NIFTI_SLICE_SEQ_DEC (AFNI 26.3.05: down to slice_start; AFNI-03 fixed upstream).
        2 => (start..=end).rev().for_each(&mut put),
        3 => up(start).chain(up(start + 1)).for_each(&mut put),
        4 => down(end).chain(down(end - 1)).for_each(&mut put),
        5 => up(start + 1).chain(up(start)).for_each(&mut put),
        6 => down(end - 1).chain(down(end)).for_each(&mut put),
        _ => {}
    }
    Some(toff)
}

/// `THD_is_symlink`: whether `path` itself is a symbolic link.
fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// `realpath(path)`: the absolute name of the file, with symbolic links, `.` and `..` resolved.
/// On Windows the `\\?\` prefix of such names is dropped where a plain name means the same.
fn realpath(path: &Path) -> std::io::Result<String> {
    let real = std::fs::canonicalize(path)?.display().to_string();
    Ok(if let Some(rest) = real.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = real
        .strip_prefix(r"\\?\")
        .filter(|r| r.as_bytes().get(1) == Some(&b':'))
    {
        rest.to_owned()
    } else {
        real
    })
}

/// The file `THD_open_nifti` opens for `path`, and the name it uses for it in its messages
/// (`pathnew`): `path`, or `path.gz` when only that exists (unless `path` contains `.gz`);
/// then its real path ([`realpath`]) unless `AFNI_NOREALPATH` is YES or the file is a symbolic
/// link. "reading X instead of non-existing Y" is printed only when the name is not replaced
/// by its real path.
fn nifti_file(
    path: &str,
    settings: &Settings,
    messages: &mut Vec<Message>,
) -> Result<(PathBuf, String), ReadError> {
    let mut file = PathBuf::from(path);
    let mut gz = false;
    if !file.is_file() && !path.contains(".gz") {
        let candidate = PathBuf::from(format!("{path}.gz"));
        if candidate.is_file() {
            file = candidate;
            gz = true;
        }
    }
    if !file.is_file() {
        // Opening it gives the reason (missing, a directory, ...).
        nifti::read_header(&file)?;
    }
    if !settings.yes("AFNI_NOREALPATH") && !is_symlink(&file) {
        let shown = realpath(&file).map_err(|e| nifti::Error::Io {
            path: file.clone(),
            source: e,
        })?;
        return Ok((file, shown));
    }
    let shown = file.display().to_string();
    if gz {
        messages.push(Message::InInfo(format!(
            "reading {shown} instead of non-existing {path}"
        )));
    }
    Ok((file, shown))
}

/// Reads a NIfTI file as 3dTshift gets it: `THD_open_dataset` + `EDIT_full_copy` (see the
/// module documentation). The messages AFNI prints while reading are added to `messages`, and
/// the upstream defects that change how the file is read (AFNI-02, AFNI-07) to `notices`, whose
/// mode chooses between AFNI's behaviour and the correction. AFNI's environment variables come
/// from `settings`.
pub fn read(
    path: &str,
    n_threads: usize,
    settings: &Settings,
    messages: &mut Vec<Message>,
    notices: &mut Notices,
) -> Result<Dataset, ReadError> {
    read_as(path, n_threads, settings, messages, notices, false)
}

/// Reads a NIfTI file as `THD_open_dataset` + `DSET_load` give it, without the copy 3dTshift
/// makes (3dAutomask, 3dUnifize). The only difference from [`read`] is a brick factor that
/// loading sets: data stored without conversion, with a finite non-zero `scl_slope` and no
/// scaling at load (`scl_inter` 0), get the slope as their factor even when it is 1
/// (`THD_load_nifti`, "pass slope as brick_fac"), where the copy has none. A factor of 1 scales
/// nothing, but `3dAutomask -apply_prefix` copies it into its output, as `scl_slope` 1.
pub fn read_loaded(
    path: &str,
    n_threads: usize,
    settings: &Settings,
    messages: &mut Vec<Message>,
    notices: &mut Notices,
) -> Result<Dataset, ReadError> {
    read_as(path, n_threads, settings, messages, notices, true)
}

fn read_as(
    path: &str,
    n_threads: usize,
    settings: &Settings,
    messages: &mut Vec<Message>,
    notices: &mut Notices,
    loaded: bool,
) -> Result<Dataset, ReadError> {
    let mode = notices.mode();
    let lower = path.to_ascii_lowercase();
    if !(lower.ends_with(".nii") || lower.ends_with(".nii.gz")) {
        return Err(ReadError::Unsupported(format!(
            "{path}: larmorx-gpl reads NIfTI files (.nii, .nii.gz) only"
        )));
    }
    let (file, shown) = nifti_file(path, settings, messages)?;
    let header = nifti::read_header(&file)?;
    let nim = Nim::from_header(&header).map_err(|m| ReadError::Afni(format!("{shown}: {m}")))?;

    // 4th dimension = time; 5th = bucket: mutually exclusive in AFNI.
    let mut ntt = nim.dim[4].max(1);
    let mut nbuc = nim.dim[5].max(1);
    let nz = nim.dim[3].max(1);
    if ntt > 1 && nbuc > 1 {
        return Err(ReadError::Afni(format!(
            "AFNI can't deal with 5 dimensional NIfTI({shown})"
        )));
    }
    let mut nvals = ntt.max(nbuc);
    if nim.dim[6] > 1 {
        nvals *= nim.dim[6];
    }
    if nim.dim[7] > 1 {
        nvals *= nim.dim[7];
    }
    if ntt > 1 {
        ntt = nvals;
    } else {
        nbuc = nvals;
    }
    let _ = nbuc;

    // The datum, chosen when the header is opened. `AFNI_NIFTI_SCALE=NO` keeps uint8 and int16
    // data with a slope and an intercept as they are stored: the slope becomes their scale
    // factor and the intercept is dropped (AFNI-23, in both modes; the default writes it back).
    let scaled_pair = nim.scl_slope != 0.0 && nim.scl_inter != 0.0;
    let scale_open = scaled_pair && !settings.no("AFNI_NIFTI_SCALE");
    let dropped_inter = (scaled_pair
        && !scale_open
        && matches!(nim.datatype, dt::UINT8 | dt::INT16))
    .then_some(DroppedIntercept {
        datatype: nim.datatype,
        slope: nim.scl_slope,
        inter: nim.scl_inter,
        carried: true,
    });
    let (float_datum, xform_data) = match nim.datatype {
        dt::UINT8 | dt::INT16 => (scale_open, scale_open),
        dt::FLOAT32 => (true, false),
        dt::INT8 | dt::UINT16 | dt::INT32 | dt::UINT32 | dt::FLOAT64 => (true, true),
        dt::COMPLEX64 | dt::RGB24 => {
            return Err(ReadError::Unsupported(format!(
                "{shown}: NIfTI datatype {} ({})",
                nim.datatype,
                datatype_string(nim.datatype)
            )));
        }
        other => {
            return Err(ReadError::Afni(format!(
                "AFNI can't handle NIFTI datatype={other} ({}) in file {shown}",
                datatype_string(other)
            )));
        }
    };
    // The conversion to float is announced unless AFNI_NIFTI_TYPE_WARN is NO; with YES without
    // the lines that say further ones are muted. (AFNI mutes them after the first file it
    // opens; 3dTshift opens one.)
    if xform_data && !settings.no("AFNI_NIFTI_TYPE_WARN") {
        let mut text = format!(
            "** AFNI converts NIFTI_datatype={} ({}) in file {shown} to FLOAT32\n",
            nim.datatype,
            datatype_string(nim.datatype)
        );
        if !settings.yes("AFNI_NIFTI_TYPE_WARN") {
            text.push_str(
                "     Warnings of this type will be muted for this session.\n     Set \
                 AFNI_NIFTI_TYPE_WARN to YES to see them all, NO to see none.\n",
            );
        }
        messages.push(Message::Raw(text));
    }

    // `AFNI_NIFTI_PRIORITY`: the qform (Q) or the sform (S, the default) when both are set.
    let mut priority = b'S';
    if let Some(p) = settings.get("AFNI_NIFTI_PRIORITY") {
        match p.as_bytes().first().map(u8::to_ascii_uppercase) {
            Some(c @ (b'S' | b'Q')) => priority = c,
            _ => messages.push(Message::Warning(format!(
                "Illegal AFNI_NIFTI_PRIORITY='{p}'"
            ))),
        }
    }
    if nim.qform_code > 0 && nim.sform_code > 0 {
        // MAT_DET of the two 3x3 matrices, rounded to float (THD_mat33).
        let det = |m: &[[f64; 4]; 4]| {
            let a = |i: usize, j: usize| m[i][j] as f32;
            a(0, 0) * a(1, 1) * a(2, 2) - a(0, 0) * a(1, 2) * a(2, 1) - a(1, 0) * a(0, 1) * a(2, 2)
                + a(1, 0) * a(0, 2) * a(2, 1)
                + a(2, 0) * a(0, 1) * a(1, 2)
                - a(2, 0) * a(0, 2) * a(1, 1)
        };
        if det(&nim.qto_xyz) * det(&nim.sto_xyz) < 0.0 {
            messages.push(Message::Warning(format!(
                "NIfTI('{shown}'): Qform/Sform handedness differ; {} wins!",
                priority as char
            )));
        }
    }
    // The brick factors of the copy: set at open time, before loading.
    let mut factor = if !xform_data && nim.scl_slope != 0.0 && nim.scl_slope != 1.0 {
        nim.scl_slope as f32
    } else {
        0.0
    };
    // AFNI-02: float32 data with a slope and an intercept are scaled when loaded (below) and
    // keep the slope as their brick factor, so they are scaled twice.
    if nim.datatype == dt::FLOAT32 && nim.scl_inter != 0.0 && factor != 0.0 {
        let (slope, inter) = (fmt_g(nim.scl_slope), fmt_g(nim.scl_inter));
        notices.add(
            &defects::AFNI_02,
            if mode.fix() {
                format!(
                    "{shown}: float32 data with scl_slope {slope} and scl_inter {inter} are \
                     scaled once; AFNI also keeps the slope as a scale factor, scaling them twice"
                )
            } else {
                format!(
                    "{shown}: float32 data with scl_slope {slope} and scl_inter {inter} are \
                     scaled, and the slope is kept as a scale factor, as AFNI does: the output \
                     is scaled twice"
                )
            },
        );
        if mode.fix() {
            factor = 0.0;
        }
    }

    // AFNI-07: the geometry the other mode gives, to say whether the correction changes it.
    let mut other_geometry = (millimetres_per_unit(&nim) != 1.0).then(|| {
        let other = if mode.fix() {
            UpstreamDefects::Reproduce
        } else {
            UpstreamDefects::Fix
        };
        geometry(&nim, other, priority, settings, &mut Vec::new(), &shown)
    });
    let mut geometry = geometry(&nim, mode, priority, settings, messages, &shown);
    let mut taxis = (ntt > 1).then(|| {
        let (mut dtt, mut toffset, mut duration) = (nim.pixdim[4], nim.toffset, nim.slice_duration);
        match nim.time_units {
            UNITS_MSEC => {
                dtt *= 0.001;
                toffset *= 0.001;
                duration *= 0.001;
            }
            UNITS_USEC => {
                dtt *= 1.0e-6;
                toffset *= 1.0e-6;
                duration *= 1.0e-6;
            }
            _ => {}
        }
        let toff_sl = slice_offsets(&nim, nz, duration);
        TimeAxis {
            ttdel: dtt as f32,
            ttorg: toffset as f32,
            units: TimeUnits::Sec,
            toff_sl,
        }
    });

    // An AFNI extension overrides the time axis, the matrix and the space.
    let extensions: Vec<(i32, &[u8])> = header
        .extensions
        .iter()
        .map(|e| (e.code, e.content.as_slice()))
        .collect();
    if let Some(ext) = afni_ext::parse(&extensions) {
        apply_afni_ext(&ext, &nim, &shown, &mut geometry, &mut taxis, messages);
        if let Some(other) = &mut other_geometry {
            apply_afni_ext(&ext, &nim, &shown, other, &mut None, &mut Vec::new());
        }
    }
    if let Some(other) = &other_geometry
        && let Some(detail) = units_detail(&nim, &geometry, other, mode)
    {
        notices.add(&defects::AFNI_07, detail);
    }

    // THD_report_obliquity, when THD_open_dataset has opened the file: an oblique matrix (as
    // AFNI reads it, in the file's units) is reported unless AFNI_NO_OBLIQUE_WARNING is YES.
    if !settings.yes("AFNI_NO_OBLIQUE_WARNING") {
        let as_read = match (&other_geometry, mode.fix()) {
            (Some(other), true) => &other.axes.real,
            _ => &geometry.axes.real,
        };
        let angle = axes::oblique_angle(as_read);
        if angle != 0.0 {
            messages.push(Message::Warning(format!(
                "  If you are performing spatial transformations on an oblique dset,\n  such \
                 as {shown},\n  or viewing/combining it with volumes of differing obliquity,\n  \
                 you should consider running: \n     3dWarp -deoblique \n  on this and  other \
                 oblique datasets in the same session.\n See 3dWarp -help for details.\n"
            )));
            messages.push(Message::Info(format!(
                "Oblique dataset:{shown} is {:.6} degrees from plumb.\n",
                f64::from(angle)
            )));
        }
    }

    // THD_load_nifti.
    let image = nifti::read(
        &file,
        &ReadOptions {
            scaling: Scaling::Raw,
            n_threads,
        },
    )?;
    let nxyz = [nim.dim[1] as usize, nim.dim[2].max(1) as usize, nz as usize];
    let nvox = nxyz[0] * nxyz[1] * nxyz[2];
    let need_copy = float_datum && nim.datatype != dt::FLOAT32;
    let slope = nim.scl_slope;
    let inter = nim.scl_inter;
    let scale_load = slope != 0.0 && (inter != 0.0 || need_copy);
    let data = match image.data {
        DynArray::U8(a) if !float_datum => BrickData::Byte(memory_order(a)),
        DynArray::I16(a) if !float_datum => BrickData::Short(memory_order(a)),
        DynArray::F32(a) => {
            let mut v = memory_order(a);
            floatscan(&mut v);
            BrickData::Float(v)
        }
        other => {
            // `CPF(ityp)`: `far[ii] = (float)sar[ii]`; float64 is scanned afterwards.
            let mut v: Vec<f32> = match other {
                DynArray::U8(a) => memory_order(a).into_iter().map(f32::from).collect(),
                DynArray::I16(a) => memory_order(a).into_iter().map(f32::from).collect(),
                DynArray::I8(a) => memory_order(a).into_iter().map(f32::from).collect(),
                DynArray::U16(a) => memory_order(a).into_iter().map(f32::from).collect(),
                DynArray::I32(a) => memory_order(a).into_iter().map(|x| x as f32).collect(),
                DynArray::U32(a) => memory_order(a).into_iter().map(|x| x as f32).collect(),
                DynArray::F64(a) => memory_order(a).into_iter().map(|x| x as f32).collect(),
                _ => unreachable!("the datatype was checked above"),
            };
            if nim.datatype == dt::FLOAT64 {
                floatscan(&mut v);
            }
            BrickData::Float(v)
        }
    };
    let data = match data {
        BrickData::Float(mut v) if scale_load => {
            for x in &mut v {
                // `far[ii] = nim->scl_slope * far[ii] + nim->scl_inter` in double.
                *x = (slope * f64::from(*x) + inter) as f32;
                if !x.is_finite() {
                    *x = 0.0;
                }
            }
            BrickData::Float(v)
        }
        other => other,
    };
    if data.len() != nvox * nvals as usize {
        return Err(ReadError::Afni(format!(
            "{shown}: {} voxel values for a grid of {nvox} voxels and {nvals} sub-bricks",
            data.len()
        )));
    }
    // THD_load_nifti: without scaling at load, a finite non-zero slope becomes the factor of
    // the loaded dataset (the copy keeps the factor set when the header was opened).
    if loaded && !scale_load && slope.is_finite() && slope != 0.0 {
        factor = slope as f32;
    }
    Ok(Dataset {
        nxyz,
        nvals: nvals as usize,
        data,
        factors: vec![factor; nvals as usize],
        taxis,
        geometry,
        dropped_inter,
    })
}

/// The AFNI-07 notice's detail when the geometry written in `mode` differs from the one the
/// other mode writes (`other`), or `None`: a header in metres or micrometres whose origin is
/// not 0, or whose matrix is oblique.
fn units_detail(
    nim: &Nim,
    geometry: &Geometry,
    other: &Geometry,
    mode: UpstreamDefects,
) -> Option<String> {
    // Only the units differ between the two. Whether that changes the output can depend on
    // AFNI-16 (a nearly plumb grid at the origin: written plumb, its steps converted either
    // way; kept, its matrix in metres or in millimetres), so the conversion is named when it
    // matters in either AFNI-16 mode: the same notices in both modes.
    let same =
        |m: UpstreamDefects| axes::written(&geometry.axes, m) == axes::written(&other.axes, m);
    if same(UpstreamDefects::Fix) && same(UpstreamDefects::Reproduce) {
        return None;
    }
    let unit = if nim.xyz_units == UNITS_METER {
        "metres"
    } else {
        "micrometres"
    };
    // The geometry as AFNI writes it: oblique (the whole matrix unconverted) or plumb (only
    // the origin unconverted).
    let afni = if mode.fix() { other } else { geometry };
    let oblique = !axes::is_plumb(&afni.axes.real);
    Some(match (mode.fix(), oblique) {
        (true, false) => format!(
            "the input's units are {unit}: its origin is converted to millimetres with its \
             voxel sizes; AFNI converts only the voxel sizes"
        ),
        (true, true) => format!(
            "the input's units are {unit}: its oblique matrix and origin are converted to \
             millimetres with its voxel sizes; AFNI converts only the voxel sizes and writes \
             the matrix (sform) in {unit}"
        ),
        (false, false) => format!(
            "the input's units are {unit}: its voxel sizes are converted to millimetres and its \
             origin is not, as AFNI does"
        ),
        (false, true) => format!(
            "the input's units are {unit}: its voxel sizes are converted to millimetres and its \
             oblique matrix (sform) and origin are not, as AFNI does"
        ),
    })
}

/// `space_to_NIFTI_code` for a `TEMPLATE_SPACE`: the space's generic space (the table of
/// AFNI's `AFNI_atlas_spaces.niml`; a space it does not list is its own generic space) gives
/// 3 for TLRC, 4 for MNI, 1 for ORIG and ACPC, and 5 (`NIFTI_XFORM_TEMPLATE_OTHER`) for any
/// other.
fn space_code(space: &str) -> i32 {
    let generic = match space {
        "TT_N27" | "TT_avg" | "TLRC" => "TLRC",
        "MNI_152" | "MNI" | "MNI_SPM2" | "MNI_FSL" | "MNI_OTHER" | "MNI_2009c_asym" | "MNI_N27" => {
            "MNI"
        }
        "MNIa" | "MNI_ANAT" => "MNI_ANAT",
        other => other,
    };
    match generic {
        "TLRC" => 3,
        "MNI" => 4,
        "ORIG" | "ACPC" => 1,
        _ => 5,
    }
}

/// `THD_nifti_process_afni_ext` + `THD_datablock_apply_atr`, for the attributes 3dTshift's
/// output depends on: the time axis (`TAXIS_NUMS`, `TAXIS_FLOATS`, `TAXIS_OFFSETS`), the
/// matrix (`IJK_TO_DICOM_REAL`) and the space (`TEMPLATE_SPACE`).
fn apply_afni_ext(
    ext: &afni_ext::AfniAttributes,
    nim: &Nim,
    path: &str,
    geometry: &mut Geometry,
    taxis: &mut Option<TimeAxis>,
    messages: &mut Vec<Message>,
) {
    if let Some(nums) = &ext.nifti_nums {
        let now = format!(
            "{},{},{},{},{},{}",
            nim.dim[1], nim.dim[2], nim.dim[3], nim.dim[4], nim.dim[5], nim.datatype
        );
        if &now != nums {
            // fprintf(stderr), after a blank line the first time.
            messages.push(Message::Raw(format!(
                "\n** WARNING: NIfTI file {path} dimensions altered since AFNI extension was \
                 added\n"
            )));
        }
    }
    if let Some(m) = ext.floats("IJK_TO_DICOM_REAL").filter(|m| m.len() >= 12) {
        // AFNI's matrix, in DICOM (RAI) order; the axes stay those of the NIfTI header.
        geometry.axes.real = [
            [m[0], m[1], m[2], m[3]],
            [m[4], m[5], m[6], m[7]],
            [m[8], m[9], m[10], m[11]],
        ];
    }
    if let (Some(nums), Some(fl)) = (
        ext.ints("TAXIS_NUMS").filter(|v| v.len() >= 3),
        ext.floats("TAXIS_FLOATS").filter(|v| v.len() >= 5),
    ) {
        let nsl = nums[1];
        let units = match nums[2] {
            77001 => TimeUnits::Msec,
            77003 => TimeUnits::Hz,
            _ => TimeUnits::Sec,
        };
        let toff_sl = if nsl > 0 {
            ext.floats("TAXIS_OFFSETS")
                .filter(|o| o.len() >= nsl as usize)
                .map(|o| o[..nsl as usize].to_vec())
        } else {
            None
        };
        *taxis = Some(TimeAxis {
            ttdel: fl[1],
            ttorg: fl[0],
            units,
            toff_sl,
        });
    }
    if let Some(space) = ext.string("TEMPLATE_SPACE") {
        geometry.code = space_code(&space);
    }
}

/// The array's values in memory order (Fortran order as read: x fastest).
fn memory_order<T: Clone>(a: ArrayD<T>) -> Vec<T> {
    if a.t().is_standard_layout() {
        a.into_raw_vec_and_offset().0
    } else {
        a.t().iter().cloned().collect()
    }
}

// ------------------------------------------------------------------------------------------------
// Writing

/// `MYFPEQ(a, b)`: `fabs(a - b) < min_timing_diff` (0.003).
fn fpeq(a: f32, b: f32) -> bool {
    f64::from(a - b).abs() < f64::from(0.003f32)
}

/// `get_slice_timing_pattern(times, len, &delta)`: the NIfTI slice code of evenly spaced
/// slice times, and the spacing.
fn slice_timing_pattern(times: &[f32]) -> (i32, f32) {
    let len = times.len();
    if len < 2 {
        return (0, 0.0);
    }
    if len == 2 {
        let delta = (times[1] - times[0]).abs();
        return (if times[1] > times[0] { 1 } else { 2 }, delta);
    }
    // Sort, keeping the original indices (`qsort_floatint`: ascending values).
    let mut idx: Vec<usize> = (0..len).collect();
    idx.sort_by(|&a, &b| times[a].total_cmp(&times[b]).then(a.cmp(&b)));
    let flist: Vec<f32> = idx.iter().map(|&i| times[i]).collect();
    let diff = flist[1] - flist[0];
    if (1..len - 1).any(|c| !fpeq(diff, flist[c + 1] - flist[c])) {
        return (0, 0.0);
    }
    let matches = |seq: &mut dyn Iterator<Item = usize>| seq.zip(&idx).all(|(a, &b)| a == b);
    let alt = |first: usize, len: usize| {
        let mut index = first;
        (0..len).map(move |_| {
            let v = index;
            index += 2;
            if index >= len {
                index = 1;
            }
            v
        })
    };
    let alt_dec = |first: usize, len: usize| {
        let mut index = first as i64;
        (0..len).map(move |_| {
            let v = index as usize;
            index -= 2;
            if index < 0 {
                index = len as i64 - 2;
            }
            v
        })
    };
    let pattern = if matches(&mut (0..len)) {
        1
    } else if matches(&mut (0..len).rev()) {
        2
    } else if matches(&mut alt(0, len)) {
        3
    } else if matches(&mut alt_dec(len - 1, len)) {
        4
    } else if matches(&mut alt2(len)) {
        5
    } else if matches(&mut alt_dec2(len)) {
        6
    } else {
        0
    };
    (pattern, if pattern == 0 { 0.0 } else { diff })
}

/// NIFTI_SLICE_ALT_INC2 order: 1, 3, 5, ..., then 0, 2, 4, ...
fn alt2(len: usize) -> impl Iterator<Item = usize> {
    (1..len).step_by(2).chain((0..len).step_by(2))
}

/// NIFTI_SLICE_ALT_DEC2 order: len-2, len-4, ..., then len-1, len-3, ...
fn alt_dec2(len: usize) -> impl Iterator<Item = usize> {
    (0..len - 1)
        .rev()
        .step_by(2)
        .chain((0..len).rev().step_by(2))
}

/// The slice-timing fields `populate_nifti_image` writes for the offsets `tlist`:
/// `(slice_code, slice_start, slice_end, slice_duration)`.
///
/// AFNI's test for "all zeros" (`sfirst == slast && MYFPEQ(tlist[sfirst],0.0)`) can never be
/// true, so a list of zeros (every time within 0.003 of 0) goes through the pattern search,
/// finds none, and gets `slice_end = 0` (AFNI-08). [`UpstreamDefects::Fix`] makes the test
/// what it was meant to be, no time away from 0, so such a list gets the defaults: no code,
/// no duration, `slice_end = nz - 1`.
pub fn slice_fields(tlist: &[f32], defects: UpstreamDefects) -> (i32, i64, i64, f32) {
    let nz = tlist.len() as i64;
    let t = |i: i64| tlist[i as usize];
    let mut ii = 0;
    while ii < nz && fpeq(t(ii), 0.0) {
        ii += 1;
    }
    let mut sfirst = ii;
    ii = nz - 1;
    while ii >= sfirst && fpeq(t(ii), 0.0) {
        ii -= 1;
    }
    let mut slast = ii;
    // The defaults: slice_start = 0, slice_end = nz - 1, no code, no duration.
    let all_zeros = if defects.fix() {
        sfirst > slast
    } else {
        sfirst == slast && fpeq(t(sfirst), 0.0)
    };
    if all_zeros {
        return (0, 0, nz - 1, 0.0);
    }
    let tlen = slast - sfirst + 2;
    let part = |from: i64, len: i64| &tlist[from as usize..(from + len) as usize];
    let mut pattern = 0;
    let mut dur = 0.0f32;
    // Try including a leading zero, then a trailing zero, then neither.
    if sfirst > 0 {
        (pattern, dur) = slice_timing_pattern(part(sfirst - 1, tlen));
        if pattern != 0 {
            sfirst -= 1;
        }
    }
    if pattern == 0 && slast < nz - 1 {
        (pattern, dur) = slice_timing_pattern(part(sfirst, tlen));
        if pattern != 0 {
            slast += 1;
        }
    }
    if pattern == 0 {
        (pattern, dur) = slice_timing_pattern(part(sfirst, tlen - 1));
    }
    if pattern == 0 {
        (0, 0, 0, 0.0)
    } else {
        (pattern, sfirst, slast, dur)
    }
}

/// Writes `ds` to `path` (`.nii` or `.nii.gz`) as AFNI's `THD_write_nifti` does: the datum,
/// `scl_slope` = the first brick factor (0 = none), the geometry AFNI carries, the time axis
/// (after `DSET_UNMSEC`) and the slice timing it keeps.
///
/// The geometry is what `THD_set_dataset_attributes` and `populate_nifti_image` make of the
/// dataset's axes ([`axes::written`]), and the slice fields what `populate_nifti_image` derives
/// from the slice times it keeps ([`slice_fields`]); both follow `notices`' mode (AFNI-16 and
/// AFNI-08), which is noted when the two modes write different fields.
///
/// From `settings`: `AFNI_NIFTI_WRITE_TYPE` 2 writes NIfTI-2, 1 NIfTI-1 (with a warning, added
/// to `messages`, when a dimension needs NIfTI-2), anything else NIfTI-1 unless a dimension
/// exceeds 32767; `AFNI_ALLOW_MILLISECONDS` YES keeps a millisecond time axis, of which AFNI
/// then writes the TR in seconds but the time offset in milliseconds (AFNI-19, corrected by
/// default).
pub fn write(
    path: &Path,
    ds: &Dataset,
    n_threads: usize,
    settings: &Settings,
    messages: &mut Vec<Message>,
    notices: &mut Notices,
) -> Result<(), nifti::Error> {
    let mode = notices.mode();
    let ms = |v: f32| (f64::from(v) * 0.001) as f32;
    let taxis = ds.taxis.as_ref().map(|t| {
        if t.units != TimeUnits::Msec {
            return t.clone();
        }
        // DSET_UNMSEC: milliseconds become seconds (`*= 0.001` in double, stored as float).
        let unmsec = TimeAxis {
            ttdel: ms(t.ttdel),
            ttorg: ms(t.ttorg),
            units: TimeUnits::Sec,
            toff_sl: t
                .toff_sl
                .as_ref()
                .map(|v| v.iter().map(|&x| ms(x)).collect()),
        };
        if !settings.yes("AFNI_ALLOW_MILLISECONDS") {
            return unmsec;
        }
        // AFNI_ALLOW_MILLISECONDS: no DSET_UNMSEC; populate_nifti_image converts the TR
        // (`TR *= 0.001`) but writes the time origin and the slice times as they are, all
        // labelled seconds (AFNI-19).
        let kept = TimeAxis {
            ttdel: ms(t.ttdel),
            ttorg: t.ttorg,
            units: TimeUnits::Sec,
            toff_sl: t.toff_sl.clone(),
        };
        if kept != unmsec {
            let toffset = |a: &TimeAxis| fmt_g(f64::from(a.ttorg));
            notices.add(
                &defects::AFNI_19,
                if mode.fix() {
                    format!(
                        "AFNI_ALLOW_MILLISECONDS is set and the TR is in milliseconds: the time \
                         offset is written in seconds ({}), with the TR; AFNI writes it in \
                         milliseconds ({}) and labels it seconds",
                        toffset(&unmsec),
                        toffset(&kept)
                    )
                } else {
                    format!(
                        "AFNI_ALLOW_MILLISECONDS is set and the TR is in milliseconds: the time \
                         offset is written in milliseconds ({}) and labelled seconds, as AFNI \
                         does",
                        toffset(&kept)
                    )
                },
            );
        }
        if mode.fix() { unmsec } else { kept }
    });
    let [nx, ny, nz] = ds.nxyz;
    let nvals = ds.nvals;
    // AFNI-20: AFNI writes a dataset of one row as a 1D text file, under the output's name,
    // unless AFNI_WRITE_1D_AS_PREFIX is YES. larmorx writes NIfTI in both modes: AFNI's text
    // file carries a random identifier, so it cannot be reproduced.
    if ny == 1 && nz == 1 && !settings.yes("AFNI_WRITE_1D_AS_PREFIX") {
        notices.extend([defects::Notice::new(
            &defects::AFNI_20,
            UpstreamDefects::Fix,
            format!(
                "the dataset has one row ({nx} x 1 x 1 voxels): written as NIfTI, in both modes; \
                 AFNI writes a 1D text file under that name (unless AFNI_WRITE_1D_AS_PREFIX is \
                 YES), with a random identifier"
            ),
        )]);
    }
    // `nt = DSET_NUM_TIMES` when there is a time axis, else the sub-bricks go to `nu`.
    let time = taxis.is_some();
    let shape: Vec<usize> = if nvals > 1 {
        if time {
            vec![nx, ny, nz, nvals]
        } else {
            vec![nx, ny, nz, 1, nvals]
        }
    } else if nz > 1 {
        vec![nx, ny, nz]
    } else if ny > 1 {
        vec![nx, ny]
    } else {
        vec![nx]
    };
    // NIfTI-2 when a dimension exceeds a signed 16-bit integer, or AFNI_NIFTI_WRITE_TYPE says
    // so (`AFNI_numenv`, then converted to int).
    let needs_nifti2 = shape.iter().any(|&d| d > i16::MAX as usize);
    let write_type = settings.num("AFNI_NIFTI_WRITE_TYPE") as i32;
    if write_type == 1 && needs_nifti2 {
        messages.push(Message::Warning(
            "will try to write as NIFTI-1, though NIFTI-2 seems necessary".to_owned(),
        ));
    }
    let nifti2 = match write_type {
        2 => true,
        1 => false,
        _ => needs_nifti2,
    };
    let version = if nifti2 {
        NiftiVersion::V2
    } else {
        NiftiVersion::V1
    };
    let mut h = NiftiHeader::new(version, &shape, ds.data.data_type()).map_err(|e| {
        nifti::Error::Format {
            path: path.to_path_buf(),
            message: e.to_string(),
        }
    })?;
    // `nifti_convert_nim2n1hdr`.
    h.analyze.regular = b'r';
    // The sform is the matrix AFNI carries (or the axes' cardinal one), the qform its
    // quaternion with the axes' origins, the voxel sizes the axes' steps. AFNI-16: a matrix
    // whose obliquity AFNI discards is kept by default.
    let w = axes::written(&ds.geometry.axes, notices.mode());
    if axes::snaps(&ds.geometry.axes) {
        notices.add(
            &defects::AFNI_16,
            plumb_detail(&ds.geometry.axes, notices.mode()),
        );
    }
    let code = ds.geometry.code;
    [h.srow_x, h.srow_y, h.srow_z] = w.sform;
    h.sform_code = code;
    h.qform_code = code;
    [h.quatern_b, h.quatern_c, h.quatern_d] = w.quatern;
    [h.qoffset_x, h.qoffset_y, h.qoffset_z] = w.qoffset;
    h.pixdim[0] = if w.qfac >= 0.0 { 1.0 } else { -1.0 };
    h.pixdim[1..4].copy_from_slice(&w.voxel);
    for p in &mut h.pixdim[4..] {
        *p = 0.0;
    }
    let fac0 = ds.factors.first().copied().unwrap_or(0.0);
    h.scl_slope = f64::from(fac0);
    h.scl_inter = 0.0;
    // AFNI-23: the intercept AFNI_NIFTI_SCALE=NO dropped, written back by default where the
    // values still carry it (a slope of 1 gives no factor: then scl_slope 1).
    if let Some(d) = ds.dropped_inter.filter(|d| d.carried) {
        notices.add(&defects::AFNI_23, dropped_inter_detail(&d, fac0, mode));
        if mode.fix() {
            h.scl_slope = if fac0 != 0.0 { f64::from(fac0) } else { 1.0 };
            h.scl_inter = d.inter;
        }
    }
    h.cal_min = 0.0;
    h.cal_max = 0.0;
    h.slice_code = 0;
    h.slice_start = 0;
    h.slice_end = 0;
    h.slice_duration = 0.0;
    h.toffset = 0.0;
    if let Some(t) = &taxis {
        if shape.len() == 4 {
            h.pixdim[4] = f64::from(t.ttdel);
        }
        h.dim_info = 3 << 4;
        h.slice_end = nz as i64 - 1;
        h.toffset = f64::from(t.ttorg);
        if let Some(tlist) = &t.toff_sl {
            let mode = notices.mode();
            let (code, start, end, dur) = slice_fields(tlist, mode);
            let other = if mode.fix() {
                UpstreamDefects::Reproduce
            } else {
                UpstreamDefects::Fix
            };
            if slice_fields(tlist, other) != (code, start, end, dur) {
                let n = tlist.len();
                notices.add(
                    &defects::AFNI_08,
                    if mode.fix() {
                        format!(
                            "the {n} slice times kept in the output are all within 0.003 of 0: \
                             no slice timing, slice_end {}; AFNI writes slice_end 0",
                            n - 1
                        )
                    } else {
                        "the slice times kept in the output are all within 0.003 of 0: \
                         slice_end 0, as AFNI writes it"
                            .to_owned()
                    },
                );
            }
            h.slice_code = code;
            h.slice_start = start;
            h.slice_end = end;
            h.slice_duration = f64::from(dur);
            if t.ttorg == 0.0 && code != 0 {
                let tmin = tlist.iter().copied().fold(tlist[0], f32::min);
                if tmin > 0.0 {
                    h.toffset = f64::from(tmin);
                }
            }
        }
        h.xyzt_units = UNITS_MM | UNITS_SEC;
    } else {
        h.dim_info = 0;
        h.xyzt_units = UNITS_MM;
    }
    let options = WriteOptions {
        n_threads,
        ..Default::default()
    };
    fn view<'a, T>(v: &'a [T], shape: &[usize]) -> ArrayViewD<'a, T> {
        ArrayViewD::from_shape(IxDyn(shape).f(), v).expect("the shape matches the data")
    }
    match &ds.data {
        BrickData::Byte(v) => nifti::write(path, &h, view(v, &shape), &options),
        BrickData::Short(v) => nifti::write(path, &h, view(v, &shape), &options),
        BrickData::Float(v) => nifti::write(path, &h, view(v, &shape), &options),
    }
}

/// The AFNI-16 notice's detail for a matrix whose obliquity AFNI discards ([`axes::snaps`]):
/// how far from plumb it is, and how far the plumb matrix's entries are from it.
fn plumb_detail(geometry: &Axes, mode: UpstreamDefects) -> String {
    let degrees = fmt_g(axes::obliquity_degrees(&geometry.real));
    let kept = axes::written(geometry, UpstreamDefects::Fix).sform;
    let plumb = axes::written(geometry, UpstreamDefects::Reproduce).sform;
    let apart = kept
        .iter()
        .flatten()
        .zip(plumb.iter().flatten())
        .fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
    let apart = fmt_g(apart);
    if mode.fix() {
        format!(
            "the input's matrix is {degrees} degrees from plumb: kept, in the sform and the \
             qform; AFNI writes the plumb matrix of its axes instead (entries up to {apart} \
             apart)"
        )
    } else {
        format!(
            "the input's matrix is {degrees} degrees from plumb: written as the plumb matrix \
             of its axes, as AFNI does (entries up to {apart} from the input's)"
        )
    }
}

/// The AFNI-23 notice's detail for an output that carries the dropped intercept `d`, written
/// with the brick factor `fac0`.
fn dropped_inter_detail(d: &DroppedIntercept, fac0: f32, mode: UpstreamDefects) -> String {
    let (ty, slope, inter) = (d.type_name(), fmt_g(d.slope), fmt_g(d.inter));
    let factor = if fac0 != 0.0 {
        format!("scl_slope {slope} as their scale factor")
    } else {
        format!("no scale factor (scl_slope {slope})")
    };
    if mode.fix() {
        format!(
            "AFNI_NIFTI_SCALE=NO keeps the {ty} data as stored, with {factor}: the output keeps \
             scl_inter {inter} too, so its values are right; AFNI drops the intercept and its \
             values are off by {inter}"
        )
    } else {
        format!(
            "AFNI_NIFTI_SCALE=NO keeps the {ty} data as stored, with {factor}, and drops \
             scl_inter {inter}, as AFNI does: the values written are off by {inter}"
        )
    }
}

/// `printf("%g")` of a time, for messages.
pub fn show_time(t: f32) -> String {
    fmt_g(f64::from(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_patterns_round_trip() {
        let mk = |order: &[usize], dt: f32| {
            let mut t = vec![0.0f32; order.len()];
            let mut s = 0.0f32;
            for &k in order {
                t[k] = s;
                s += dt;
            }
            t
        };
        assert_eq!(slice_timing_pattern(&mk(&[0, 1, 2, 3], 0.5)), (1, 0.5));
        assert_eq!(slice_timing_pattern(&mk(&[3, 2, 1, 0], 0.5)), (2, 0.5));
        assert_eq!(slice_timing_pattern(&mk(&[0, 2, 4, 1, 3], 0.4)), (3, 0.4));
        assert_eq!(slice_timing_pattern(&mk(&[4, 2, 0, 3, 1], 0.4)), (4, 0.4));
        assert_eq!(slice_timing_pattern(&mk(&[1, 3, 0, 2, 4], 0.4)), (5, 0.4));
        assert_eq!(slice_timing_pattern(&mk(&[3, 1, 4, 2, 0], 0.4)), (6, 0.4));
        assert_eq!(slice_timing_pattern(&[0.0, 0.1, 0.5]).0, 0);
        // Whole lists, with the leading zero.
        let t = mk(&[0, 2, 1, 3], 0.5);
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            assert_eq!(slice_fields(&t, mode), (3, 0, 3, 0.5));
            assert_eq!(slice_fields(&[0.0, 0.0, 0.5, 0.0], mode).0, 1);
        }
        // AFNI-08: every time within 0.003 of 0. AFNI writes slice_end 0; corrected, the
        // defaults (slice_end nz - 1).
        let zeros = [0.0, 0.0005, 0.001, 0.0015];
        assert_eq!(
            slice_fields(&zeros, UpstreamDefects::Reproduce),
            (0, 0, 0, 0.0)
        );
        assert_eq!(slice_fields(&zeros, UpstreamDefects::Fix), (0, 0, 3, 0.0));
        assert_eq!(
            slice_fields(&[0.0; 4], UpstreamDefects::Fix),
            (0, 0, 3, 0.0)
        );
    }

    #[test]
    fn header_slice_offsets() {
        let h = NiftiHeader::new(NiftiVersion::V1, &[2, 2, 4, 3], DataType::F32).unwrap();
        let mut nim = Nim::from_header(&h).unwrap();
        nim.slice_dim = 3;
        nim.slice_start = 0;
        nim.slice_end = 3;
        nim.slice_duration = 0.5;
        for (code, want) in [
            (1, [0.0, 0.5, 1.0, 1.5]),
            // AFNI 26.3.05 (AFNI-03 fixed): from slice_end down to slice_start.
            (2, [1.5, 1.0, 0.5, 0.0]),
            (3, [0.0, 1.0, 0.5, 1.5]),
            (4, [1.5, 0.5, 1.0, 0.0]),
            (5, [1.0, 0.0, 1.5, 0.5]),
            (6, [0.5, 1.5, 0.0, 1.0]),
            (7, [0.0; 4]),
        ] {
            nim.slice_code = code;
            assert_eq!(slice_offsets(&nim, 4, 0.5).unwrap(), want, "code {code}");
        }
        nim.slice_code = 2;
        (nim.slice_start, nim.slice_end) = (1, 2);
        assert_eq!(slice_offsets(&nim, 4, 0.5).unwrap(), [0.0, 0.5, 0.0, 0.0]);
        nim.slice_duration = 0.0;
        assert_eq!(slice_offsets(&nim, 4, 0.0), None);
    }
}
