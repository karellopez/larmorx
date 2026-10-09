//! Datasets held as AFNI holds them, and AFNI's rules for reading and writing NIfTI
//! (`specs/3dTshift.md` §6).
//!
//! AFNI keeps voxels as bytes, shorts or floats (its *datum*), each volume with an optional
//! scale (*brick factor*). Reading a NIfTI file chooses the datum and factor from the stored
//! type and `scl_slope`/`scl_inter`; writing stores the datum and the factor back.

use std::path::Path;

use larmorx_core::affine::Affine;
use larmorx_core::array::DynArray;
use larmorx_core::element::DataType;
use larmorx_core::ndarray::{ArrayD, ArrayViewD, IxDyn, ShapeBuilder};
use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, ReadOptions, Scaling, WriteOptions};

use crate::timing::{self, HeaderTiming};

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

/// A stored value as a float: multiplied by the brick factor only when it is positive.
#[inline]
pub fn extract<T: Datum>(x: T, factor: f32) -> f32 {
    if factor > 0.0 {
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
    /// What AFNI would have warned about while reading.
    pub warnings: Vec<String>,
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

/// Reads a NIfTI file as AFNI 25.2.09 does (`specs/3dTshift.md` §6):
///
/// | Stored type | Header scaling | Kept as | Values |
/// |---|---|---|---|
/// | uint8, int16, float32 | slope ≠ 0 and intercept ≠ 0 | float32 | `slope·x + inter` |
/// | uint8, int16, float32 | otherwise | as stored | a finite slope ∉ {0, 1} becomes the brick factor |
/// | int8, uint16, int32, uint32, float64 | any | float32 | `slope·x + inter` if the slope is finite and ≠ 0 |
///
/// Non-finite `scl_slope`/`scl_inter` count as 0; non-finite float values, and scaled values
/// that overflow, become 0. Files with more than one volume beyond the fourth dimension, and
/// 64-bit integer or complex data, are rejected.
pub fn read(path: impl AsRef<Path>, n_threads: usize) -> Result<AfniImage, ReadError> {
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
    let mut warnings = Vec::new();
    let (data, factor) = match image.data {
        DynArray::U8(a) if both => (
            BrickData::Float(scaled_f32(into_vec(a), f32::from, slope, inter)),
            0.0,
        ),
        DynArray::I16(a) if both => (
            BrickData::Float(scaled_f32(into_vec(a), f32::from, slope, inter)),
            0.0,
        ),
        DynArray::F32(a) if both => (
            BrickData::Float(scaled_f32(into_vec(a), |x| x, slope, inter)),
            0.0,
        ),
        DynArray::U8(a) => (BrickData::Byte(into_vec(a)), factor),
        DynArray::I16(a) => (BrickData::Short(into_vec(a)), factor),
        DynArray::F32(a) => (
            BrickData::Float(into_vec(a).into_iter().map(clean).collect()),
            factor,
        ),
        other => {
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
                o => {
                    return Err(unsupported(format!(
                        "{:?} data are not supported",
                        o.data_type()
                    )));
                }
            };
            warnings.push(format!(
                "{}: {:?} data are converted to float32",
                path.display(),
                image.header.data_type().ok()
            ));
            (BrickData::Float(v), 0.0)
        }
    };
    if factor < 0.0 {
        warnings.push(format!(
            "{}: negative scl_slope {factor}: as AFNI does, it is not applied when the data are \
             read but is stored in the output, so the output is sign-flipped",
            path.display()
        ));
    }
    let has_time_axis = header.dim[0] >= 4;
    let per_second = timing::time_units_per_second(&header);
    let mut tr = timing::to_seconds(header.pixdim[4], per_second);
    if tr.is_nan() || tr <= 0.0 {
        tr = 1.0;
    }
    let toffset = timing::to_seconds(finite_or_zero(header.toffset), per_second);
    let slice_timing = timing::header_timing(&header, shape4[2]);
    let bricks = Bricks::new(shape4, data, vec![factor; shape4[3]]).map_err(unsupported)?;
    Ok(AfniImage {
        bricks,
        header,
        has_time_axis,
        tr,
        toffset,
        slice_timing,
        warnings,
    })
}

// ------------------------------------------------------------------------------------------------
// Geometry

/// The voxel-to-world matrix AFNI takes from a header (observed): the sform if
/// `sform_code > 0`, else the qform if `qform_code > 0`, else the voxel sizes on the
/// diagonal; scaled to millimetres from metres or micrometres. Also returns the xform code
/// AFNI writes back: 1 for codes 1 and 2 (or none), 3 for 3 and 5, 4 for 4.
pub fn afni_geometry(hdr: &NiftiHeader) -> Result<(Affine, i32), String> {
    let (matrix, code) = if hdr.sform_code > 0 {
        (hdr.sform(), hdr.sform_code)
    } else if hdr.qform_code > 0 {
        (hdr.qform().map_err(|e| e.to_string())?, hdr.qform_code)
    } else {
        let size = |i: usize| {
            let p = hdr.pixdim[i].abs();
            if p > 0.0 && p.is_finite() { p } else { 1.0 }
        };
        (Affine::from_zooms([size(1), size(2), size(3)], [0.0; 3]), 1)
    };
    let mm = match hdr.xyzt_units & 0x07 {
        1 => 1000.0,
        3 => 0.001,
        _ => 1.0,
    };
    let matrix = if mm == 1.0 {
        matrix
    } else {
        let rows = matrix.rows();
        Affine::from_rows([
            rows[0].map(|v| v * mm),
            rows[1].map(|v| v * mm),
            rows[2].map(|v| v * mm),
            rows[3],
        ])
    };
    let code = match code {
        3 | 5 => 3,
        4 => 4,
        _ => 1,
    };
    Ok((matrix, code))
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
    /// Slice timing kept in the header (the "copy of the input" cases), or `None` for none.
    pub slices: Option<HeaderTiming>,
}

/// The header AFNI writes for `bricks` with the geometry of `input` (observed): NIfTI-1, both
/// qform and sform from AFNI's matrix with the same code, `scl_slope` = the brick factor,
/// millimetres and seconds, the slice axis (`dim_info`) third, and the slice range
/// `0..nz−1` unless slice timing is kept. A dataset with one volume is written as 3D, without
/// time fields.
pub fn output_header(
    input: &NiftiHeader,
    bricks: &Bricks,
    timing: &OutputTiming,
) -> Result<NiftiHeader, String> {
    let [nx, ny, nz, nt] = bricks.shape;
    let shape: Vec<usize> = if nt > 1 {
        vec![nx, ny, nz, nt]
    } else {
        vec![nx, ny, nz]
    };
    let mut hdr = NiftiHeader::new(NiftiVersion::V1, &shape, bricks.data.data_type())
        .map_err(|e| e.to_string())?;
    let (matrix, code) = afni_geometry(input)?;
    hdr.set_sform(&matrix, code);
    hdr.set_qform(&matrix, code).map_err(|e| e.to_string())?;
    hdr.scl_slope = f64::from(bricks.factors.first().copied().unwrap_or(0.0));
    hdr.scl_inter = 0.0;
    for p in &mut hdr.pixdim[4..] {
        *p = 0.0;
    }
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
    Ok(hdr)
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

    #[test]
    fn storing_rounds_half_to_even_and_clamps() {
        assert_eq!(store::<i16>(2.5, 0.0), 2);
        assert_eq!(store::<i16>(3.5, 0.0), 4);
        assert_eq!(store::<i16>(-40000.0, 0.0), -32767);
        assert_eq!(store::<i16>(40000.0, 0.0), 32767);
        assert_eq!(store::<u8>(-3.0, 0.0), 0);
        assert_eq!(store::<u8>(255.7, 0.0), 255);
        assert_eq!(store::<u8>(0.5, 0.0), 0);
        // Brick factors: divide on store, multiply on extract (positive factors only).
        assert_eq!(store::<i16>(10.0, 0.25), 40);
        assert_eq!(extract(40i16, 0.25), 10.0);
        assert_eq!(extract(40i16, -0.5), 40.0);
        assert_eq!(store::<i16>(40.0, -0.5), -80);
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
        h
    }

    #[test]
    fn geometry_prefers_the_sform_and_maps_codes() {
        let (m, code) = afni_geometry(&header_with(1, 2)).unwrap();
        assert_eq!(m.translation(), [-5.0, 7.0, 11.0]);
        assert_eq!(code, 1);
        let (m, code) = afni_geometry(&header_with(4, 0)).unwrap();
        assert_eq!(m.translation(), [1.0, 2.0, 3.0]);
        assert_eq!(code, 4);
        assert_eq!(afni_geometry(&header_with(0, 5)).unwrap().1, 3);
        let (m, code) = afni_geometry(&header_with(0, 0)).unwrap();
        assert_eq!(m.translation(), [0.0; 3]);
        assert_eq!(code, 1);
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
        let h = output_header(&input, &bricks, &timing).unwrap();
        assert_eq!(h.dim[..5], [4, 2, 2, 2, 3]);
        assert_eq!((h.qform_code, h.sform_code), (1, 1));
        assert_eq!(h.sform().translation(), [-5.0, 7.0, 11.0]);
        assert_eq!((h.scl_slope, h.scl_inter), (0.5, 0.0));
        assert_eq!((h.pixdim[4], h.toffset), (2.0, 0.75));
        assert_eq!((h.xyzt_units, h.dim_info), (10, 48));
        assert_eq!((h.slice_code, h.slice_start, h.slice_end), (0, 0, 1));
    }
}
