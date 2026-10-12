// SPDX-License-Identifier: Apache-2.0
//! Datasets held as AFNI holds them, and AFNI's rules for reading and writing NIfTI
//! (`specs/3dTshift.md` §6, §6.1 and §7.1).
//!
//! AFNI keeps voxels as bytes, shorts or floats (its *datum*), each volume with an optional
//! scale (*brick factor*). Reading a NIfTI file chooses the datum and factor from the stored
//! type and `scl_slope`/`scl_inter`; writing stores the datum and the factor back.
//!
//! Several of AFNI's rules are defects that larmorx corrects by default (D13,
//! `docs/api/upstream-defects.md`): a negative brick factor is applied on output only
//! (AFNI-01, [`extract`]), float32 data with a slope and an intercept are scaled twice
//! (AFNI-02, [`read`]), metre and micron units leave the origin or an oblique matrix
//! unconverted (AFNI-07) and nearly plumb grids are written plumb (AFNI-16; both in
//! [`crate::geometry`]), and `AFNI_NIFTI_SCALE=NO` drops the intercept of integer data
//! (AFNI-23, [`intercept_plan`]). `UpstreamDefects::Reproduce` follows AFNI.

use std::path::Path;

use larmorx_core::array::DynArray;
use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::element::DataType;
use larmorx_core::ndarray::{ArrayD, ArrayViewD, IxDyn, ShapeBuilder};
use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, ReadOptions, Scaling, WriteOptions};

use crate::cfmt::g32;
use crate::geometry::{self, InputGeometry};
use crate::msg::Message;
use crate::settings::{AfniSettings, write_type};
use crate::timing::{self, HeaderTiming, SliceFields};
use crate::tshift::Restore;

/// The voxels of a dataset in one of AFNI's storage types, in Fortran order (x fastest).
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

    /// The NIfTI data type of the storage.
    pub fn data_type(&self) -> DataType {
        match self {
            BrickData::Byte(_) => DataType::U8,
            BrickData::Short(_) => DataType::I16,
            BrickData::Float(_) => DataType::F32,
        }
    }
}

/// A 4D dataset `(nx, ny, nz, nt)` as AFNI holds it: voxels and one brick factor per volume
/// (0 for none).
///
/// A value is the stored number times the factor, when the factor is positive. (A negative
/// factor, which AFNI takes from a negative `scl_slope`, is not applied when values are read
/// but is when they are stored: `specs/3dTshift.md` §6.)
#[derive(Clone, Debug, PartialEq)]
pub struct Bricks {
    pub shape: [usize; 4],
    pub data: BrickData,
    pub factors: Vec<f32>,
}

impl Bricks {
    /// Checks that the data and factors fit the shape.
    pub fn new(shape: [usize; 4], data: BrickData, factors: Vec<f32>) -> Result<Bricks, String> {
        let n = shape.iter().try_fold(1usize, |a, &d| a.checked_mul(d));
        if n != Some(data.len()) {
            return Err(format!(
                "{} voxel values for a dataset of shape {shape:?}",
                data.len()
            ));
        }
        if factors.len() != shape[3] {
            return Err(format!(
                "{} brick factors for {} volumes",
                factors.len(),
                shape[3]
            ));
        }
        Ok(Bricks {
            shape,
            data,
            factors,
        })
    }
}

/// An element type AFNI stores (`u8`, `i16`, `f32`).
pub trait Datum: Copy + Send + Sync + 'static {
    fn to_f32(self) -> f32;
    /// Stores a value (already divided by the brick factor): integers are clamped to their
    /// range (shorts to ±32767) and rounded half to even.
    fn from_f64(v: f64) -> Self;
}

impl Datum for u8 {
    fn to_f32(self) -> f32 {
        f32::from(self)
    }
    fn from_f64(v: f64) -> Self {
        v.clamp(0.0, 255.0).round_ties_even() as u8
    }
}

impl Datum for i16 {
    fn to_f32(self) -> f32 {
        f32::from(self)
    }
    fn from_f64(v: f64) -> Self {
        v.clamp(-32767.0, 32767.0).round_ties_even() as i16
    }
}

impl Datum for f32 {
    fn to_f32(self) -> f32 {
        self
    }
    fn from_f64(v: f64) -> Self {
        v as f32
    }
}

/// A stored value as a float: multiplied by the brick factor. AFNI multiplies only when the
/// factor is positive, so a negative factor (a negative `scl_slope`), which [`store`] does
/// divide by, flips the sign of every value it processes (observed; AFNI-01). With
/// [`UpstreamDefects::Fix`] any non-zero factor is applied.
#[inline]
pub fn extract<T: Datum>(x: T, factor: f32, defects: UpstreamDefects) -> f32 {
    if factor > 0.0 || (factor != 0.0 && defects.fix()) {
        x.to_f32() * factor
    } else {
        x.to_f32()
    }
}

/// A value stored back: divided by a non-zero brick factor (times `1/factor`, in double).
#[inline]
pub fn store<T: Datum>(v: f32, factor: f32) -> T {
    if factor != 0.0 {
        T::from_f64(f64::from(v) * (1.0 / f64::from(factor)))
    } else {
        T::from_f64(f64::from(v))
    }
}

// ------------------------------------------------------------------------------------------------
// Reading

/// Data that `AFNI_NIFTI_SCALE=NO` kept as stored (uint8 or int16 with both a slope and an
/// intercept): the slope became the brick factor and the intercept was dropped (AFNI-23).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeptIntercept {
    pub slope: f32,
    pub inter: f32,
    /// The stored type.
    pub stored: DataType,
}

/// A NIfTI file read with AFNI's rules.
#[derive(Clone, Debug)]
pub struct AfniImage {
    /// The voxels; 3D files have one volume.
    pub bricks: Bricks,
    /// The header as stored in the file.
    pub header: NiftiHeader,
    /// Whether the file has a time axis (`dim[0] ≥ 4`).
    pub has_time_axis: bool,
    /// The repetition time in seconds (`pixdim[4]`, converted from ms or µs; 1 if ≤ 0).
    pub tr: f32,
    /// `toffset` in seconds.
    pub toffset: f32,
    /// The slice timing the header gives, if any.
    pub slice_timing: Option<HeaderTiming>,
    /// The matrix AFNI took from the header, and the code it writes.
    pub geometry: InputGeometry,
    /// AFNI's messages while reading, in its order.
    pub messages: Vec<Message>,
    /// The upstream defects that changed how the file was read (AFNI-02).
    pub notices: Vec<Notice>,
    /// Set when `AFNI_NIFTI_SCALE=NO` kept integer data and dropped their intercept.
    pub kept_intercept: Option<KeptIntercept>,
}

/// A NIfTI file could not be read as an AFNI dataset.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error(transparent)]
    Nifti(#[from] nifti::Error),
    #[error("{path}: {message}")]
    Unsupported { path: String, message: String },
}

/// `x` unless it is not finite, then 0.
fn finite_or_zero(x: f64) -> f64 {
    if x.is_finite() { x } else { 0.0 }
}

/// `v` if finite, else 0 (AFNI zeroes non-finite floats on input).
#[inline]
fn clean(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

fn into_vec<T: Clone>(a: ArrayD<T>) -> Vec<T> {
    // Arrays read from files are Fortran-contiguous, so this does not reorder.
    match a.as_slice_memory_order() {
        Some(_) if a.t().is_standard_layout() => a.into_raw_vec_and_offset().0,
        _ => a.t().iter().cloned().collect(),
    }
}

/// `slope·x + inter` as AFNI computes it (observed): the stored value converted to float32,
/// then the product and sum in double, rounded to float32.
fn scaled_f32<T: Copy>(v: Vec<T>, to_f32: impl Fn(T) -> f32, slope: f32, inter: f32) -> Vec<f32> {
    let (s, i) = (f64::from(slope), f64::from(inter));
    v.into_iter()
        .map(|x| clean((s * f64::from(to_f32(x)) + i) as f32))
        .collect()
}

/// AFNI's name of a NIfTI data type in its conversion message.
fn type_name(code: i16) -> &'static str {
    match code {
        2 => "UINT8",
        4 => "INT16",
        8 => "INT32",
        16 => "FLOAT32",
        64 => "FLOAT64",
        256 => "INT8",
        512 => "UINT16",
        768 => "UINT32",
        _ => "UNKNOWN",
    }
}

/// Reads a NIfTI file as AFNI 25.2.09 does (`specs/3dTshift.md` §6), with AFNI's variables
/// `settings` (§7.1: `AFNI_NIFTI_PRIORITY`, `AFNI_NIFTI_SCALE`, `AFNI_NIFTI_TYPE_WARN`,
/// `AFNI_NIFTI_VIEW`, `AFNI_DEFAULT_STD_SPACE`, `AFNI_NO_OBLIQUE_WARNING`). `shown` is the
/// input's name in messages.
///
/// | Stored type | Header scaling | Kept as | Values |
/// |---|---|---|---|
/// | uint8, int16, float32 | slope ≠ 0 and intercept ≠ 0 | float32 | `slope·x + inter` |
/// | uint8, int16, float32 | otherwise | as stored | a finite slope ∉ {0, 1} becomes the brick factor |
/// | int8, uint16, int32, uint32, float64 | any | float32 | `slope·x + inter` if the slope is finite and ≠ 0 |
///
/// Non-finite `scl_slope`/`scl_inter` count as 0; non-finite float values, and scaled values
/// that overflow, become 0. Files with more than one volume beyond the fourth dimension, and
/// 64-bit integer or complex data, are rejected. With `AFNI_NIFTI_SCALE` no, uint8 and int16
/// data with both a slope and an intercept are kept as stored, the intercept dropped
/// ([`AfniImage::kept_intercept`], AFNI-23).
///
/// AFNI-02 (observed): for float32 data with a slope and an intercept, AFNI also keeps the
/// slope as the brick factor, so the scaled values are scaled once more when they are
/// processed and written. [`UpstreamDefects::Fix`] scales them once (no factor);
/// [`UpstreamDefects::Reproduce`] keeps the factor. The header's slice timing follows
/// `defects` too ([`timing::header_timing`], AFNI-03).
pub fn read(
    path: impl AsRef<Path>,
    shown: &str,
    n_threads: usize,
    defects: UpstreamDefects,
    settings: &AfniSettings,
) -> Result<AfniImage, ReadError> {
    let path = path.as_ref();
    let header = nifti::read_header(path)?;
    let image = nifti::read(
        path,
        &ReadOptions {
            scaling: Scaling::Raw,
            n_threads,
        },
    )?;
    let unsupported = |message: String| ReadError::Unsupported {
        path: path.display().to_string(),
        message,
    };
    let shape = image.data.shape().to_vec();
    if shape.len() > 4 && shape[4..].iter().any(|&d| d > 1) {
        return Err(unsupported(format!(
            "datasets with more than 4 dimensions are not supported ({shape:?})"
        )));
    }
    let dim = |i: usize| shape.get(i).copied().unwrap_or(1);
    let shape4 = [dim(0), dim(1), dim(2), dim(3)];
    let slope = finite_or_zero(header.scl_slope) as f32;
    let inter = finite_or_zero(header.scl_inter) as f32;
    let both = slope != 0.0 && inter != 0.0;
    let factor = if slope != 0.0 && slope != 1.0 {
        slope
    } else {
        0.0
    };
    let scale_no = settings.no("AFNI_NIFTI_SCALE");
    let mut notices = Vec::new();
    let mut kept_intercept = None;
    let mut converted = false;
    let (data, factor) = match image.data {
        DynArray::U8(a) if both && scale_no => {
            kept_intercept = Some(KeptIntercept {
                slope,
                inter,
                stored: DataType::U8,
            });
            (BrickData::Byte(into_vec(a)), factor)
        }
        DynArray::I16(a) if both && scale_no => {
            kept_intercept = Some(KeptIntercept {
                slope,
                inter,
                stored: DataType::I16,
            });
            (BrickData::Short(into_vec(a)), factor)
        }
        DynArray::U8(a) if both => {
            converted = true;
            (
                BrickData::Float(scaled_f32(into_vec(a), f32::from, slope, inter)),
                0.0,
            )
        }
        DynArray::I16(a) if both => {
            converted = true;
            (
                BrickData::Float(scaled_f32(into_vec(a), f32::from, slope, inter)),
                0.0,
            )
        }
        DynArray::F32(a) if both => {
            if factor != 0.0 {
                let detail = match defects {
                    UpstreamDefects::Fix => format!(
                        "{}: float32 data with scl_slope {slope} and scl_inter {inter} are scaled \
                         once; AFNI also keeps the slope as a scale factor, scaling them twice",
                        path.display()
                    ),
                    UpstreamDefects::Reproduce => format!(
                        "{}: float32 data with scl_slope {slope} and scl_inter {inter} are scaled, \
                         and the slope is kept as a scale factor, as AFNI does: the output is \
                         scaled twice",
                        path.display()
                    ),
                };
                notices.push(Notice::new(&defects::AFNI_02, defects, detail));
            }
            let kept = if defects.fix() { 0.0 } else { factor };
            (
                BrickData::Float(scaled_f32(into_vec(a), |x| x, slope, inter)),
                kept,
            )
        }
        DynArray::U8(a) => (BrickData::Byte(into_vec(a)), factor),
        DynArray::I16(a) => (BrickData::Short(into_vec(a)), factor),
        DynArray::F32(a) => (
            BrickData::Float(into_vec(a).into_iter().map(clean).collect()),
            factor,
        ),
        other => {
            let stored = other.data_type();
            let (s, i) = if slope != 0.0 {
                (slope, inter)
            } else {
                (1.0, 0.0)
            };
            let v = match other {
                DynArray::I8(a) => scaled_f32(into_vec(a), f32::from, s, i),
                DynArray::U16(a) => scaled_f32(into_vec(a), f32::from, s, i),
                DynArray::I32(a) => scaled_f32(into_vec(a), |x| x as f32, s, i),
                DynArray::U32(a) => scaled_f32(into_vec(a), |x| x as f32, s, i),
                DynArray::F64(a) => scaled_f32(into_vec(a), |x| x as f32, s, i),
                _ => {
                    return Err(unsupported(format!(
                        "{stored} data are not supported by 3dTshift"
                    )));
                }
            };
            converted = true;
            (BrickData::Float(v), 0.0)
        }
    };
    let mut messages = Vec::new();
    // AFNI's conversion message comes first (observed), without a label.
    if converted && !settings.no("AFNI_NIFTI_TYPE_WARN") {
        let mut text = format!(
            "** AFNI converts NIFTI_datatype={} ({}) in file {shown} to FLOAT32\n",
            header.datatype,
            type_name(header.datatype)
        );
        if !settings.yes("AFNI_NIFTI_TYPE_WARN") {
            text.push_str(
                "     Warnings of this type will be muted for this session.\n     Set \
                 AFNI_NIFTI_TYPE_WARN to YES to see them all, NO to see none.\n",
            );
        }
        messages.push(Message::Raw(text));
    }
    let (geometry, geometry_messages) = geometry::read_geometry(&header, settings, shown);
    messages.extend(geometry_messages);
    let has_time_axis = header.dim[0] >= 4;
    let tr = header_tr(&header);
    let per_second = timing::time_units_per_second(&header);
    let toffset = timing::to_seconds(finite_or_zero(header.toffset), per_second);
    let slice_timing = timing::header_timing(&header, shape4[2], defects);
    let bricks = Bricks::new(shape4, data, vec![factor; shape4[3]]).map_err(unsupported)?;
    Ok(AfniImage {
        bricks,
        header,
        has_time_axis,
        tr,
        toffset,
        slice_timing,
        geometry,
        messages,
        notices,
        kept_intercept,
    })
}

/// The TR AFNI reads from a header, in seconds: `pixdim[4]` converted from milliseconds or
/// microseconds, or 1 if it is not positive.
pub fn header_tr(header: &NiftiHeader) -> f32 {
    let tr = timing::to_seconds(header.pixdim[4], timing::time_units_per_second(header));
    if tr.is_nan() || tr <= 0.0 { 1.0 } else { tr }
}

// ------------------------------------------------------------------------------------------------
// AFNI_NIFTI_SCALE=NO (AFNI-23)

/// What the output does with the intercept that `AFNI_NIFTI_SCALE=NO` dropped (spec §7.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InterceptPlan {
    /// AFNI's output: the integers and the factor, the intercept dropped.
    Afni,
    /// AFNI's integers, with `scl_slope` (1 where AFNI writes 0) and the intercept as
    /// `scl_inter` (the default where the output carries the intercept in every volume).
    InHeader(OutputScaling),
    /// The data read as without the variable (float32, `slope·x + inter`): the default for
    /// `-rlt` with `-ignore`, whose ignored volumes need the intercept and the others do not.
    ReadScaled,
}

/// The plan for data kept by `AFNI_NIFTI_SCALE=NO` (AFNI-23), and its notice: an output that
/// carries the intercept in every volume (anything but a shifted `-rlt` output) keeps it in
/// the header by default; a shifted `-rlt` output with `-ignore` is read as without the
/// variable; a shifted `-rlt` output without `-ignore` is AFNI's, which is right.
pub fn intercept_plan(
    kept: &KeptIntercept,
    factor: f32,
    copy: bool,
    restore: Restore,
    ignore: usize,
    defects: UpstreamDefects,
) -> (InterceptPlan, Option<Notice>) {
    let name = type_name(match kept.stored {
        DataType::U8 => 2,
        _ => 4,
    });
    let (slope, inter) = (g32(kept.slope), g32(kept.inter));
    let rlt = !copy && restore == Restore::None;
    if rlt && ignore == 0 {
        return (InterceptPlan::Afni, None);
    }
    let (plan, detail) = match (rlt, defects) {
        (false, UpstreamDefects::Fix) => (
            InterceptPlan::InHeader(OutputScaling {
                slope: if factor == 0.0 { 1.0 } else { factor },
                inter: kept.inter,
            }),
            format!(
                "AFNI_NIFTI_SCALE=NO keeps the {name} data as stored, with scl_slope {slope} as \
                 their scale factor: the output keeps scl_inter {inter} too, so its values are \
                 right; AFNI drops the intercept and its values are off by {inter}"
            ),
        ),
        (false, UpstreamDefects::Reproduce) => (
            InterceptPlan::Afni,
            format!(
                "AFNI_NIFTI_SCALE=NO keeps the {name} data as stored, with scl_slope {slope} as \
                 their scale factor, and drops scl_inter {inter}, as AFNI does: the values \
                 written are off by {inter}"
            ),
        ),
        (true, UpstreamDefects::Fix) => (
            InterceptPlan::ReadScaled,
            format!(
                "AFNI_NIFTI_SCALE=NO with -rlt and -ignore {ignore}: the ignored volumes keep \
                 scl_inter {inter} and the detrended ones lose it, which one scl_inter cannot \
                 describe, so the {name} data are read as float32 with scl_slope {slope} and \
                 scl_inter {inter} applied, as without the variable; AFNI keeps them as {name} \
                 and drops the intercept, so its {ignore} ignored volumes are off by {inter}"
            ),
        ),
        (true, UpstreamDefects::Reproduce) => (
            InterceptPlan::Afni,
            format!(
                "AFNI_NIFTI_SCALE=NO with -rlt and -ignore {ignore}: the {name} data are kept as \
                 stored and scl_inter {inter} is dropped, as AFNI does: the {ignore} ignored \
                 volumes are off by {inter}"
            ),
        ),
    };
    (plan, Some(Notice::new(&defects::AFNI_23, defects, detail)))
}

/// `bricks` (kept as stored by `AFNI_NIFTI_SCALE=NO`) read as without the variable: float32
/// `slope·x + inter`, no factor.
pub fn read_scaled(bricks: &mut Bricks, kept: &KeptIntercept) {
    let data = match &bricks.data {
        BrickData::Byte(v) => scaled_f32(v.clone(), f32::from, kept.slope, kept.inter),
        BrickData::Short(v) => scaled_f32(v.clone(), f32::from, kept.slope, kept.inter),
        BrickData::Float(_) => return,
    };
    bricks.data = BrickData::Float(data);
    bricks.factors.iter_mut().for_each(|f| *f = 0.0);
}

// ------------------------------------------------------------------------------------------------
// Writing

/// The time fields of an output header.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputTiming {
    /// `pixdim[4]` (seconds).
    pub tr: f32,
    /// `toffset` (seconds).
    pub toffset: f32,
    /// The slice fields of a copy that keeps the header's slice times (spec §6.2), or `None`
    /// for AFNI's defaults (no slice code, `slice_end` = nz − 1).
    pub slices: Option<SliceFields>,
}

/// `scl_slope` and `scl_inter` of an output, when they are not the brick factor and 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputScaling {
    pub slope: f32,
    pub inter: f32,
}

/// The header AFNI writes for `bricks` read with `geometry` (spec §6 and §6.1, observed):
/// NIfTI-1 (NIfTI-2 with `AFNI_NIFTI_WRITE_TYPE` 2, or for a dimension above 32767), both
/// forms from AFNI's matrix with one code, `scl_slope` = the brick factor, millimetres and
/// seconds, the slice axis (`dim_info`) third, and the slice range `0..nz−1` unless a copy
/// keeps slice timing. A dataset with one volume is written as 3D, without time fields. The
/// geometry follows `defects` (AFNI-07, AFNI-16), whose notices are returned, with AFNI's
/// messages.
pub fn output_header(
    geometry: &InputGeometry,
    bricks: &Bricks,
    timing: &OutputTiming,
    scaling: Option<OutputScaling>,
    defects: UpstreamDefects,
    settings: &AfniSettings,
) -> Result<(NiftiHeader, Vec<Notice>, Vec<Message>), String> {
    let [nx, ny, nz, nt] = bricks.shape;
    let shape: Vec<usize> = if nt > 1 {
        vec![nx, ny, nz, nt]
    } else {
        vec![nx, ny, nz]
    };
    let mut messages = Vec::new();
    let large = shape.iter().any(|&d| d > 32767);
    let version = match write_type(settings.get("AFNI_NIFTI_WRITE_TYPE")) {
        2 => NiftiVersion::V2,
        1 if large => {
            messages.push(Message::Warning(
                "will try to write as NIFTI-1, though NIFTI-2 seems necessary".into(),
            ));
            return Err(
                "larmorx: a dimension above 32767 needs NIfTI-2 (AFNI_NIFTI_WRITE_TYPE \
                        is 1)"
                    .into(),
            );
        }
        _ if large => NiftiVersion::V2,
        _ => NiftiVersion::V1,
    };
    let mut hdr =
        NiftiHeader::new(version, &shape, bricks.data.data_type()).map_err(|e| e.to_string())?;
    let (g, notices) = geometry::output_geometry(geometry, defects);
    for (row, srow) in g
        .srow
        .iter()
        .zip([&mut hdr.srow_x, &mut hdr.srow_y, &mut hdr.srow_z])
    {
        *srow = row.map(f64::from);
    }
    hdr.sform_code = geometry.out_code;
    hdr.qform_code = geometry.out_code;
    // NIfTI-1 stores the quaternion as float32 (the header rounds it when written).
    [hdr.quatern_b, hdr.quatern_c, hdr.quatern_d] = g.quatern;
    [hdr.qoffset_x, hdr.qoffset_y, hdr.qoffset_z] = g.qoffset.map(f64::from);
    hdr.pixdim = [0.0; 8];
    hdr.pixdim[0] = g.qfac;
    for j in 0..3 {
        hdr.pixdim[j + 1] = f64::from(g.pixdim[j]);
    }
    let (slope, inter) = match scaling {
        Some(s) => (s.slope, s.inter),
        None => (bricks.factors.first().copied().unwrap_or(0.0), 0.0),
    };
    hdr.scl_slope = f64::from(slope);
    hdr.scl_inter = f64::from(inter);
    hdr.analyze.regular = b'r';
    if nt > 1 {
        hdr.pixdim[4] = f64::from(timing.tr);
        hdr.toffset = f64::from(timing.toffset);
        hdr.xyzt_units = 2 | 8;
        hdr.dim_info = 3 << 4;
        hdr.slice_start = 0;
        hdr.slice_end = nz as i64 - 1;
        if let Some(s) = &timing.slices {
            hdr.slice_code = s.code;
            hdr.slice_start = s.start as i64;
            hdr.slice_end = s.end as i64;
            hdr.slice_duration = f64::from(s.duration);
        }
    } else {
        hdr.xyzt_units = 2;
    }
    Ok((hdr, notices, messages))
}

/// Writes `bricks` to `path` (`.nii` or `.nii.gz`) with `header` (see [`output_header`]).
pub fn write(
    path: impl AsRef<Path>,
    header: &NiftiHeader,
    bricks: &Bricks,
    n_threads: usize,
) -> Result<(), nifti::Error> {
    let [nx, ny, nz, nt] = bricks.shape;
    let shape: Vec<usize> = if nt > 1 {
        vec![nx, ny, nz, nt]
    } else {
        vec![nx, ny, nz]
    };
    let options = WriteOptions {
        n_threads,
        ..Default::default()
    };
    fn view<'a, T>(v: &'a [T], shape: &[usize]) -> ArrayViewD<'a, T> {
        ArrayViewD::from_shape(IxDyn(shape).f(), v).expect("the shape matches the data")
    }
    match &bricks.data {
        BrickData::Byte(v) => nifti::write(path, header, view(v, &shape), &options),
        BrickData::Short(v) => nifti::write(path, header, view(v, &shape), &options),
        BrickData::Float(v) => nifti::write(path, header, view(v, &shape), &options),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::affine::Affine;

    const FIX: UpstreamDefects = UpstreamDefects::Fix;
    const REPRODUCE: UpstreamDefects = UpstreamDefects::Reproduce;

    #[test]
    fn storing_rounds_half_to_even_and_clamps() {
        assert_eq!(store::<i16>(2.5, 0.0), 2);
        assert_eq!(store::<i16>(3.5, 0.0), 4);
        assert_eq!(store::<i16>(-40000.0, 0.0), -32767);
        assert_eq!(store::<i16>(40000.0, 0.0), 32767);
        assert_eq!(store::<u8>(-3.0, 0.0), 0);
        assert_eq!(store::<u8>(255.7, 0.0), 255);
        assert_eq!(store::<u8>(0.5, 0.0), 0);
        // Brick factors: divide on store, multiply on extract.
        assert_eq!(store::<i16>(10.0, 0.25), 40);
        assert_eq!(extract(40i16, 0.25, FIX), 10.0);
        assert_eq!(extract(40i16, 0.25, REPRODUCE), 10.0);
        // AFNI-01: AFNI multiplies by positive factors only, but divides by negative ones.
        assert_eq!(extract(40i16, -0.5, REPRODUCE), 40.0);
        assert_eq!(store::<i16>(40.0, -0.5), -80);
        assert_eq!(extract(40i16, -0.5, FIX), -20.0);
        assert_eq!(store::<i16>(-20.0, -0.5), 40);
        assert_eq!(extract(40i16, 0.0, FIX), 40.0);
        // The division is a multiplication by 1/factor in double.
        let v = 1033.0437f32 * 0.3;
        assert_eq!(
            store::<f32>(v, 0.3),
            (f64::from(v) * (1.0 / f64::from(0.3f32))) as f32
        );
    }

    fn header_with(qcode: i32, scode: i32) -> NiftiHeader {
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[2, 2, 2, 3], DataType::F32).unwrap();
        let a = Affine::from_zooms([2.0, 2.0, 3.0], [1.0, 2.0, 3.0]);
        let b = Affine::from_zooms([-2.0, 2.0, 3.0], [-5.0, 7.0, 11.0]);
        h.set_qform(&a, qcode).unwrap();
        h.set_sform(&b, scode);
        h.qform_code = qcode;
        h.xyzt_units = 10;
        h
    }

    fn geometry_of(h: &NiftiHeader) -> InputGeometry {
        geometry::read_geometry(h, &AfniSettings::new(), "in.nii").0
    }

    #[test]
    fn output_header_fields() {
        let input = header_with(1, 1);
        let bricks =
            Bricks::new([2, 2, 2, 3], BrickData::Short(vec![0; 24]), vec![0.5; 3]).unwrap();
        let timing = OutputTiming {
            tr: 2.0,
            toffset: 0.75,
            slices: None,
        };
        let s = AfniSettings::new();
        let (h, notices, messages) =
            output_header(&geometry_of(&input), &bricks, &timing, None, FIX, &s).unwrap();
        assert!(notices.is_empty() && messages.is_empty());
        assert_eq!(h.dim[..5], [4, 2, 2, 2, 3]);
        assert_eq!((h.qform_code, h.sform_code), (1, 1));
        assert_eq!(h.sform().translation(), [-5.0, 7.0, 11.0]);
        assert_eq!((h.scl_slope, h.scl_inter), (0.5, 0.0));
        assert_eq!((h.pixdim[4], h.toffset), (2.0, 0.75));
        assert_eq!((h.xyzt_units, h.dim_info), (10, 48));
        assert_eq!((h.slice_code, h.slice_start, h.slice_end), (0, 0, 1));
        assert_eq!(h.analyze.regular, b'r');
        // The qform of the cardinal matrix: x flipped.
        assert_eq!(h.pixdim[..4], [-1.0, 2.0, 2.0, 3.0]);
        // NIfTI-2 with AFNI_NIFTI_WRITE_TYPE=2; NIfTI-1 otherwise.
        let two = AfniSettings::from_pairs([("AFNI_NIFTI_WRITE_TYPE", "2")]);
        let (h, _, _) =
            output_header(&geometry_of(&input), &bricks, &timing, None, FIX, &two).unwrap();
        assert_eq!(h.version, NiftiVersion::V2);
    }

    /// AFNI-23: the plan for data `AFNI_NIFTI_SCALE=NO` kept as stored.
    #[test]
    fn intercept_plans() {
        let kept = KeptIntercept {
            slope: 0.37,
            inter: 12.5,
            stored: DataType::I16,
        };
        let plan = |copy, restore, ignore, mode| {
            let (p, n) = intercept_plan(&kept, 0.37, copy, restore, ignore, mode);
            (p, n.map(|n| n.defect.id))
        };
        let header = InterceptPlan::InHeader(OutputScaling {
            slope: 0.37,
            inter: 12.5,
        });
        assert_eq!(
            plan(false, Restore::Trend, 0, FIX),
            (header, Some("AFNI-23"))
        );
        assert_eq!(plan(true, Restore::None, 2, FIX), (header, Some("AFNI-23")));
        assert_eq!(
            plan(false, Restore::Trend, 0, REPRODUCE),
            (InterceptPlan::Afni, Some("AFNI-23"))
        );
        assert_eq!(
            plan(false, Restore::None, 0, FIX),
            (InterceptPlan::Afni, None)
        );
        assert_eq!(
            plan(false, Restore::None, 2, FIX),
            (InterceptPlan::ReadScaled, Some("AFNI-23"))
        );
        assert_eq!(
            plan(false, Restore::None, 2, REPRODUCE),
            (InterceptPlan::Afni, Some("AFNI-23"))
        );
        // Slope 1: no factor; the default writes scl_slope 1.
        let (p, _) = intercept_plan(&kept, 0.0, false, Restore::Trend, 0, FIX);
        assert_eq!(
            p,
            InterceptPlan::InHeader(OutputScaling {
                slope: 1.0,
                inter: 12.5
            })
        );
        let mut b =
            Bricks::new([1, 1, 1, 2], BrickData::Short(vec![10, -4]), vec![0.37; 2]).unwrap();
        read_scaled(&mut b, &kept);
        assert_eq!(b.factors, [0.0, 0.0]);
        let BrickData::Float(v) = &b.data else {
            panic!()
        };
        assert_eq!(v[0], (0.37f64 * 10.0 + 12.5) as f32);
    }
}
