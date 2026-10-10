// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09: src/thd_niftiread.c (THD_open_nifti, THD_load_nifti),
// src/thd_niftiwrite.c (populate_nifti_image, get_slice_timing_pattern, space_to_NIFTI_code),
// src/nifti/nifti2/nifti2_io.c (the header conversion of nifti_image_read and
// nifti_quatern_to_dmat44), src/thd_floatscan.c (thd_floatscan), src/edt_fullcopy.c
// (EDIT_full_copy), src/thd_atlas.c (THD_get_generic_space, with the space table of
// src/AFNI_atlas_spaces.niml) and the DSET_UNMSEC macro of src/3ddata.h.
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
//! The geometry is carried as AFNI carries it in `ijk_to_dicom_real`: the matrix AFNI took from
//! the header (sform, else qform, else the voxel sizes), stored as `float`.
//!
//! An AFNI header extension (code 4) in the input replaces the time axis, the matrix and the
//! space, as AFNI applies it ([`crate::afni_ext`]); its other attributes are ignored, and no
//! extension is written. The `AFNI_NIFTI_*` environment variables are not read.

use std::path::{Path, PathBuf};

use larmorx_core::affine::Affine;
use larmorx_core::array::DynArray;
use larmorx_core::element::DataType;
use larmorx_core::ndarray::{ArrayD, ArrayViewD, IxDyn, ShapeBuilder};
use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, ReadOptions, Scaling, WriteOptions};

use crate::afni_ext;
use crate::cnum::fmt_g;

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

/// The geometry AFNI writes back: `ijk_to_dicom_real` turned back to NIfTI's RAS convention
/// (float values), the grid steps `|xxdel|`, `|yydel|`, `|zzdel|`, and the xform code
/// `space_to_NIFTI_code` gives.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub matrix: [[f64; 4]; 4],
    pub steps: [f32; 3],
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

/// `nifti_quatern_to_dmat44` (in double; the C code uses long double for some products).
fn quatern_to_dmat44(
    qb: f64,
    qc: f64,
    qd: f64,
    offset: [f64; 3],
    d: [f64; 3],
    qfac: f64,
) -> [[f64; 4]; 4] {
    let (mut b, mut c, mut dd) = (qb, qc, qd);
    let mut a = 1.0 - (b * b + c * c + dd * dd);
    if a < 1.0e-7 {
        a = 1.0 / (b * b + c * c + dd * dd).sqrt();
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
    [
        [
            (a * a + b * b - c * c - dd * dd) * xd,
            2.0 * (b * c - a * dd) * yd,
            2.0 * (b * dd + a * c) * zd,
            offset[0],
        ],
        [
            2.0 * (b * c + a * dd) * xd,
            (a * a + c * c - b * b - dd * dd) * yd,
            2.0 * (c * dd - a * b) * zd,
            offset[1],
        ],
        [
            2.0 * (b * dd - a * c) * xd,
            2.0 * (c * dd + a * b) * yd,
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

/// `NIFTI_code_to_view` + `NIFTI_code_to_space` + `space_to_NIFTI_code`, for a header without
/// an AFNI extension: TLRC space (code 3, and code 5, whose view is +tlrc) gives 3, MNI (4)
/// gives 4, every other code (orig view, ORIG space) gives 1 (`NIFTI_XFORM_SCANNER_ANAT`).
fn output_code(form_code: i32) -> i32 {
    match form_code {
        3 | 5 => 3,
        4 => 4,
        _ => 1,
    }
}

/// `THD_open_nifti`'s geometry: the matrix it stores in `ijk_to_dicom_real` (a float `mat44`),
/// written back with x and y negated (NIfTI's convention), the grid steps, and the code.
fn geometry(nim: &Nim, warnings: &mut Vec<String>, path: &str) -> Geometry {
    let to_f32 = |m: [[f64; 4]; 4]| m.map(|r| r.map(|v| f64::from(v as f32)));
    let units = |v: f32| match nim.xyz_units {
        UNITS_METER => (f64::from(v) * 1000.0) as f32,
        UNITS_MICRON => (f64::from(v) * 0.001) as f32,
        _ => v,
    };
    let (use_q, use_s) = match (nim.qform_code > 0, nim.sform_code > 0) {
        // Both present: the sform wins (AFNI's default `form_priority = 'S'`).
        (true, true) => (false, true),
        (true, false) => (true, false),
        (false, true) => (false, true),
        (false, false) => {
            warnings.push(format!(
                "NO spatial transform (neither qform nor sform), in NIfTI file '{path}'"
            ));
            (false, false)
        }
    };
    if use_q {
        let steps = [1, 2, 3].map(|i| {
            // `nim->dx *= 1000.0` (double), stored in a float THD_fvec3.
            let d = match nim.xyz_units {
                UNITS_METER => nim.pixdim[i] * 1000.0,
                UNITS_MICRON => nim.pixdim[i] * 0.001,
                _ => nim.pixdim[i],
            };
            (d as f32).abs()
        });
        Geometry {
            matrix: to_f32(nim.qto_xyz),
            steps,
            code: output_code(nim.qform_code),
        }
    } else if use_s {
        let m = nim.sto_xyz;
        let steps = [0, 1, 2].map(|j| {
            let norm = (m[0][j] * m[0][j] + m[1][j] * m[1][j] + m[2][j] * m[2][j]).sqrt() as f32;
            units(norm).abs()
        });
        Geometry {
            matrix: to_f32(m),
            steps,
            code: output_code(nim.sform_code),
        }
    } else {
        // No transform: AFNI sets the orientation to LPI with the voxel sizes as steps and the
        // origin at 0, and writes diag(dx, dy, dz) in NIfTI's (RAS) convention: the matrix of
        // those axes, not the `ijk_to_dicom44` it loads here with x and y negated back.
        // Verified with the oracle; where AFNI rebuilds the matrix was not traced.
        let step = |i: usize| {
            let p = nim.pixdim[i] as f32;
            units(if p > 0.0 { p } else { 1.0 })
        };
        let (dx, dy, dz) = (step(1), step(2), step(3));
        Geometry {
            matrix: [
                [f64::from(dx), 0.0, 0.0, 0.0],
                [0.0, f64::from(dy), 0.0, 0.0],
                [0.0, 0.0, f64::from(dz), 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            steps: [dx.abs(), dy.abs(), dz.abs()],
            code: output_code(0),
        }
    }
}

/// The slice offsets `THD_open_nifti` builds from `slice_code`, `slice_start`, `slice_end`
/// and `slice_duration` (already in seconds), or `None` if the header gives none.
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
        // NIFTI_SLICE_SEQ_DEC: AFNI's loop runs `for( kk=slice_end ; kk >= slice_end ; kk-- )`,
        // so only slice_end gets a time (0) and every slice stays at 0.
        2 => put(end),
        3 => up(start).chain(up(start + 1)).for_each(&mut put),
        4 => down(end).chain(down(end - 1)).for_each(&mut put),
        5 => up(start + 1).chain(up(start)).for_each(&mut put),
        6 => down(end - 1).chain(down(end)).for_each(&mut put),
        _ => {}
    }
    Some(toff)
}

/// Reads a NIfTI file as 3dTshift gets it: `THD_open_dataset` + `EDIT_full_copy` (see the
/// module documentation). Messages AFNI prints while reading are added to `warnings`.
pub fn read(
    path: &str,
    n_threads: usize,
    warnings: &mut Vec<String>,
) -> Result<Dataset, ReadError> {
    let lower = path.to_ascii_lowercase();
    if !(lower.ends_with(".nii") || lower.ends_with(".nii.gz")) {
        return Err(ReadError::Unsupported(format!(
            "{path}: larmorx-gpl reads NIfTI files (.nii, .nii.gz) only"
        )));
    }
    // THD_open_nifti: a missing `x.nii` is read as `x.nii.gz` if that exists.
    let mut file = PathBuf::from(path);
    if !file.is_file() && !path.contains(".gz") {
        let gz = PathBuf::from(format!("{path}.gz"));
        if gz.is_file() {
            warnings.push(format!(
                "reading {} instead of non-existing {path}",
                gz.display()
            ));
            file = gz;
        }
    }
    let header = nifti::read_header(&file)?;
    let shown = file.display().to_string();
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

    // The datum, chosen when the header is opened.
    let scale_open = nim.scl_slope != 0.0 && nim.scl_inter != 0.0;
    let (float_datum, xform_data) = match nim.datatype {
        dt::UINT8 | dt::INT16 => (scale_open, scale_open),
        dt::FLOAT32 => (true, false),
        dt::INT8 | dt::UINT16 | dt::INT32 | dt::UINT32 | dt::FLOAT64 => {
            warnings.push(format!(
                "AFNI converts NIFTI_datatype={} ({}) in file {shown} to FLOAT32",
                nim.datatype,
                datatype_string(nim.datatype)
            ));
            (true, true)
        }
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
    // The brick factors of the copy: set at open time, before loading.
    let factor = if !xform_data && nim.scl_slope != 0.0 && nim.scl_slope != 1.0 {
        nim.scl_slope as f32
    } else {
        0.0
    };

    let mut geometry = geometry(&nim, warnings, &shown);
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
        TimeAxis {
            ttdel: dtt as f32,
            ttorg: toffset as f32,
            units: TimeUnits::Sec,
            toff_sl: slice_offsets(&nim, nz, duration),
        }
    });

    // An AFNI extension overrides the time axis, the matrix and the space.
    let extensions: Vec<(i32, &[u8])> = header
        .extensions
        .iter()
        .map(|e| (e.code, e.content.as_slice()))
        .collect();
    if let Some(ext) = afni_ext::parse(&extensions) {
        apply_afni_ext(&ext, &nim, &shown, &mut geometry, &mut taxis, warnings);
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
    Ok(Dataset {
        nxyz,
        nvals: nvals as usize,
        data,
        factors: vec![factor; nvals as usize],
        taxis,
        geometry,
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
    warnings: &mut Vec<String>,
) {
    if let Some(nums) = &ext.nifti_nums {
        let now = format!(
            "{},{},{},{},{},{}",
            nim.dim[1], nim.dim[2], nim.dim[3], nim.dim[4], nim.dim[5], nim.datatype
        );
        if &now != nums {
            warnings.push(format!(
                "NIfTI file {path} dimensions altered since AFNI extension was added"
            ));
        }
    }
    if let Some(m) = ext.floats("IJK_TO_DICOM_REAL").filter(|m| m.len() >= 12) {
        // AFNI's matrix is in DICOM (RAI) order; populate_nifti_image negates the first two
        // rows to write it.
        let v = |i: usize| f64::from(m[i]);
        geometry.matrix = [
            [-v(0), -v(1), -v(2), -v(3)],
            [-v(4), -v(5), -v(6), -v(7)],
            [v(8), v(9), v(10), v(11)],
            [0.0, 0.0, 0.0, 1.0],
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
/// true, so a list of zeros goes through the pattern search, finds none, and gets
/// `slice_end = 0`.
fn slice_fields(tlist: &[f32]) -> (i32, i64, i64, f32) {
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
    if sfirst == slast && fpeq(t(sfirst), 0.0) {
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
pub fn write(path: &Path, ds: &Dataset, n_threads: usize) -> Result<(), nifti::Error> {
    // DSET_UNMSEC: milliseconds become seconds (`*= 0.001` in double, stored as float).
    let taxis = ds.taxis.as_ref().map(|t| {
        if t.units == TimeUnits::Msec {
            let ms = |v: f32| (f64::from(v) * 0.001) as f32;
            TimeAxis {
                ttdel: ms(t.ttdel),
                ttorg: ms(t.ttorg),
                units: TimeUnits::Sec,
                toff_sl: t
                    .toff_sl
                    .as_ref()
                    .map(|v| v.iter().map(|&x| ms(x)).collect()),
            }
        } else {
            t.clone()
        }
    });
    let [nx, ny, nz] = ds.nxyz;
    let nvals = ds.nvals;
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
    let version = if [nx, ny, nz, nvals].iter().any(|&d| d > i16::MAX as usize) {
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
    // The sform is the matrix AFNI carries, and the qform is computed from it.
    let affine = Affine::from_rows(ds.geometry.matrix);
    let code = ds.geometry.code;
    h.set_sform(&affine, code);
    if h.set_qform(&affine, code).is_err() {
        h.qform_code = code;
    }
    for (i, &s) in ds.geometry.steps.iter().enumerate() {
        h.pixdim[i + 1] = f64::from(s);
    }
    for p in &mut h.pixdim[4..] {
        *p = 0.0;
    }
    let fac0 = ds.factors.first().copied().unwrap_or(0.0);
    h.scl_slope = f64::from(fac0);
    h.scl_inter = 0.0;
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
            let (code, start, end, dur) = slice_fields(tlist);
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
        assert_eq!(slice_fields(&t), (3, 0, 3, 0.5));
        assert_eq!(slice_fields(&[0.0; 4]), (0, 0, 0, 0.0));
        assert_eq!(slice_fields(&[0.0, 0.0, 0.5, 0.0]).0, 1);
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
            (2, [0.0; 4]),
            (3, [0.0, 1.0, 0.5, 1.5]),
            (4, [1.5, 0.5, 1.0, 0.0]),
            (5, [1.0, 0.0, 1.5, 0.5]),
            (6, [0.5, 1.5, 0.0, 1.0]),
            (7, [0.0; 4]),
        ] {
            nim.slice_code = code;
            assert_eq!(slice_offsets(&nim, 4, 0.5).unwrap(), want, "code {code}");
        }
        nim.slice_duration = 0.0;
        assert_eq!(slice_offsets(&nim, 4, 0.0), None);
    }
}
