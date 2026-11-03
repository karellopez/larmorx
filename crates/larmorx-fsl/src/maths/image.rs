// SPDX-License-Identifier: Apache-2.0
//! Images as fslmaths reads and writes them (`specs/fslmaths.md` §3).
//!
//! Reading keeps every volume and converts the stored values to the **computing type**
//! (§3.1): an unscaled value with C's conversion, a scaled one as `v × slope + inter` in single
//! precision (double for float64 data), then converted; an image whose voxel-to-world
//! determinant is positive is mirrored along x (FSL's radiological frame) and mirrored back when
//! written. Names, compressions and the ANALYZE reading are BET's (`specs/bet.md` §3), shared
//! through [`crate::bet::image`] and `larmorx_mri::hmc::image`.
//!
//! Writing (§3.3): the current image's grid and geometry with a fresh header, the values
//! rounded half away from zero when a floating computing type meets an integer output type,
//! then converted (FSL-151, FSL-157).

use larmorx_core::DynArray;
use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};
use larmorx_core::element::{DataType, RealElement};
use larmorx_io::nifti::NiftiHeader;
use larmorx_mri::hmc::files::{self, ImageFiles};
use larmorx_mri::hmc::image::{ImageError, is_neurological, read_stored_files};
use rayon::prelude::*;

use super::num::{Kind, Num};
use crate::bet::image::{self as bimage, Output, Values};
use crate::cli::Settings;

/// The `descrip` field of the images larmorx's fslmaths writes.
pub const DESCRIP: &str = "larmorx fsl fslmaths";

/// Rows converted per parallel task.
const ROWS: usize = 64;

/// An image in the computing type `T`: its values in FSL's radiological frame (x fastest, then
/// y, z, volumes) and what writing it needs.
#[derive(Clone, Debug)]
pub struct Img<T> {
    /// `[nx, ny, nz, nt]` (axes beyond the fourth counted as volumes).
    pub dims: [usize; 4],
    pub data: Vec<T>,
    /// The header it was read with (its geometry is written with it).
    pub header: NiftiHeader,
    /// Whether x is mirrored in memory (a positive determinant).
    pub flipped: bool,
    /// Its display range: the file's as read, `(0, 0)` once an operation changed it.
    pub cal: (f64, f64),
}

impl<T> Img<T> {
    /// The voxels of one volume.
    pub fn per_volume(&self) -> usize {
        self.dims[0] * self.dims[1] * self.dims[2]
    }

    /// FSL's `d(v)`: the 1-based index of the last axis whose size exceeds 1 (at least 1).
    pub fn last_axis(&self) -> usize {
        (0..4)
            .rev()
            .find(|&k| self.dims[k] > 1)
            .map_or(1, |k| k + 1)
    }

    /// The voxel sizes (the stored magnitudes, as FSL holds them in single precision).
    pub fn voxel(&self) -> [f64; 3] {
        [1, 2, 3].map(|k| f64::from((self.header.pixdim[k] as f32).abs()))
    }
}

/// An image as stored, before its conversion to a computing type.
pub struct Stored {
    pub header: NiftiHeader,
    /// The stored data type.
    pub stored: DataType,
    /// `scl_slope`, `scl_inter` when they scale the values.
    pub scale: Option<(f64, f64)>,
    pub flipped: bool,
    pub dims: [usize; 4],
    values: StoredValues,
}

enum StoredValues {
    /// The file's array, in its order.
    File(DynArray),
    /// Values already in single precision and the radiological frame (an ANALYZE pair).
    Radiological(Vec<f32>),
}

impl Stored {
    /// The type `-dt input` and `-odt input` name: a scaled image counts as float32 unless it
    /// is stored as float64 (§2.4).
    pub fn effective_type(&self) -> DataType {
        if self.scale.is_some() && self.stored != DataType::F64 {
            DataType::F32
        } else {
            self.stored
        }
    }

    /// The values converted to `T` in the radiological frame, and whether FSL-151's correction
    /// changed one (`fix`); the conversions run on the current thread pool.
    pub fn values<T: Num>(&self, fix: bool) -> (Vec<T>, bool) {
        let nx = self.dims[0].max(1);
        match &self.values {
            StoredValues::Radiological(v) => {
                let mut out = vec![T::default(); v.len()];
                let changed = out
                    .par_chunks_mut(1 << 16)
                    .zip(v.par_chunks(1 << 16))
                    .map(|(o, s)| {
                        let mut ch = false;
                        for (o, &s) in o.iter_mut().zip(s) {
                            let (t, c) = T::convert(super::num::to_f64(s), fix);
                            *o = t;
                            ch |= c;
                        }
                        ch
                    })
                    .reduce(|| false, |a, b| a || b);
                (out, changed)
            }
            StoredValues::File(a) => {
                let scale = self.scale;
                let flipped = self.flipped;
                larmorx_core::dispatch_real_dyn_array!(
                    a,
                    arr => convert_array::<_, T>(arr, nx, flipped, scale, fix),
                    complex => (Vec::new(), false)
                )
            }
        }
    }
}

impl Stored {
    /// [`Stored::values`], the stored values moved instead of copied where they already have the
    /// computing type and no scaling (mirrored in place for a neurological image; a NaN as the
    /// conversion gives it), so that reading holds one copy of the image, as FSL does; the stored
    /// values are dropped either way.
    pub fn take_values<T: Num>(&mut self, fix: bool) -> (Vec<T>, bool) {
        let values = std::mem::replace(&mut self.values, StoredValues::Radiological(Vec::new()));
        let moved = if self.scale.is_some() {
            Err(values)
        } else {
            same_type::<T>(values, self.dims[0].max(1), self.flipped)
        };
        match moved {
            Ok(mut v) => {
                if T::KIND == Kind::Float {
                    v.par_iter_mut()
                        .with_min_len(1 << 16)
                        .filter(|x| x.is_nan())
                        .for_each(|x| *x = T::convert(x.to_f64(), fix).0);
                }
                (v, false)
            }
            Err(values) => {
                self.values = values;
                let r = self.values::<T>(fix);
                self.values = StoredValues::Radiological(Vec::new());
                r
            }
        }
    }
}

/// The stored values as a `Vec<T>` without a copy when `T` is their type (radiological order).
fn same_type<T: Num>(
    values: StoredValues,
    nx: usize,
    flipped: bool,
) -> Result<Vec<T>, StoredValues> {
    use std::any::Any;
    fn file_order<S: Copy + Send>(
        a: larmorx_core::ndarray::ArrayD<S>,
        nx: usize,
        flipped: bool,
    ) -> Result<Vec<S>, larmorx_core::ndarray::ArrayD<S>> {
        // Fortran order as read: x fastest in memory, from the buffer's start
        if !a.t().is_standard_layout() {
            return Err(a);
        }
        let n = a.len();
        let (v, offset) = a.into_raw_vec_and_offset();
        let off = offset.unwrap_or(0);
        // the values from the buffer's offset (a copy only where they are not the whole buffer)
        let mut v = if off == 0 && v.len() == n {
            v
        } else {
            v[off..off + n].to_vec()
        };
        if flipped && nx > 1 {
            v.par_chunks_mut(nx).for_each(|row| row.reverse());
        }
        Ok(v)
    }
    fn cast<T: Num, S: 'static>(v: Vec<S>) -> Vec<T> {
        let b: Box<dyn Any> = Box::new(v);
        *b.downcast::<Vec<T>>().expect("the same type")
    }
    macro_rules! moved {
        ($a:expr, $variant:ident, $kind:ident) => {
            if T::KIND == Kind::$kind {
                return match file_order($a, nx, flipped) {
                    Ok(v) => Ok(cast::<T, _>(v)),
                    Err(a) => Err(StoredValues::File(DynArray::$variant(a))),
                };
            } else {
                Err(StoredValues::File(DynArray::$variant($a)))
            }
        };
    }
    match values {
        StoredValues::Radiological(v) if T::KIND == Kind::Float => Ok(cast::<T, f32>(v)),
        StoredValues::File(DynArray::F32(a)) => moved!(a, F32, Float),
        StoredValues::File(DynArray::F64(a)) => moved!(a, F64, Double),
        StoredValues::File(DynArray::I16(a)) => moved!(a, I16, Short),
        StoredValues::File(DynArray::I32(a)) => moved!(a, I32, Int),
        StoredValues::File(DynArray::U8(a)) => moved!(a, U8, UChar),
        StoredValues::File(DynArray::I8(a)) => moved!(a, I8, Char),
        other => Err(other),
    }
}

fn convert_array<S: RealElement + Sync, T: Num>(
    a: &larmorx_core::ndarray::ArrayD<S>,
    nx: usize,
    flipped: bool,
    scale: Option<(f64, f64)>,
    fix: bool,
) -> (Vec<T>, bool) {
    // Fortran order as read: the transposed view is contiguous, x fastest
    let t = a.t();
    let owned: Vec<S>;
    let src: &[S] = match t.as_slice() {
        Some(s) => s,
        None => {
            owned = t.iter().copied().collect();
            &owned
        }
    };
    let double = S::DATA_TYPE == DataType::F64;
    let value = |s: S| -> f64 {
        match scale {
            None => s.to_f64(),
            Some((m, b)) if double => s.to_f64() * m + b,
            Some((m, b)) => super::num::to_f64(s.to_f32() * m as f32 + b as f32),
        }
    };
    let mut out = vec![T::default(); src.len()];
    let changed = out
        .par_chunks_mut(nx * ROWS)
        .zip(src.par_chunks(nx * ROWS))
        .map(|(o, s)| {
            let mut ch = false;
            for (orow, srow) in o.chunks_mut(nx).zip(s.chunks(nx)) {
                for (k, &v) in srow.iter().enumerate() {
                    let (t, c) = T::convert(value(v), fix);
                    let at = if flipped { srow.len() - 1 - k } else { k };
                    orow[at] = t;
                    ch |= c;
                }
            }
            ch
        })
        .reduce(|| false, |a, b| a || b);
    (out, changed)
}

/// Reads the image `name` as stored (FSL's names and compressions; FSL-60 with `reproduce`).
pub fn read_stored(
    settings: &Settings,
    name: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
    n_threads: usize,
) -> Result<Stored, ImageError> {
    let files = crate::cli::bet2::resolve_image(settings, name, mode, notices)?;
    read_files(&files, n_threads)
}

fn read_files(files: &ImageFiles, n_threads: usize) -> Result<Stored, ImageError> {
    if bimage::analyze_header(files) {
        let (vols, input) = bimage::read(files, n_threads)?;
        let stored = input.header.data_type().unwrap_or(DataType::F32);
        let [nx, ny, nz] = input.vol.shape;
        let values: Vec<f32> = vols.into_iter().flat_map(|v| v.data).collect();
        let nt = values.len() / (nx * ny * nz).max(1);
        return Ok(Stored {
            header: input.header,
            stored,
            scale: None,
            flipped: input.flipped,
            dims: [nx, ny, nz, nt.max(1)],
            values: StoredValues::Radiological(values),
        });
    }
    let (header, data) = read_stored_files(files, n_threads)?;
    let shape = data.shape().to_vec();
    if shape.is_empty() || shape.iter().product::<usize>() == 0 {
        return Err(ImageError::Invalid(format!(
            "{}: the image has no voxels",
            files.header.display()
        )));
    }
    let d = |k: usize| shape.get(k).copied().unwrap_or(1);
    let nt: usize = shape.iter().skip(3).product::<usize>().max(1);
    let stored = data.data_type();
    let (m, b) = (header.scl_slope, header.scl_inter);
    // a slope below 1e-30 in magnitude means no scaling (§3.1)
    let scale = (m.abs() >= 1e-30 && (m != 1.0 || b != 0.0) && !m.is_nan()).then_some((
        f64::from(m as f32),
        if b.is_finite() {
            f64::from(b as f32)
        } else {
            0.0
        },
    ));
    let flipped = is_neurological(&header);
    Ok(Stored {
        header,
        stored,
        scale,
        flipped,
        dims: [d(0), d(1), d(2), nt],
        values: StoredValues::File(data),
    })
}

/// The output types (§2.4): `char` written as unsigned bytes; `SChar` an int8 input's type
/// under `-odt input` with FSL-157 corrected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutType {
    Char,
    SChar,
    Short,
    Int,
    Float,
    Double,
}

impl OutType {
    pub fn is_int(self) -> bool {
        !matches!(self, OutType::Float | OutType::Double)
    }
}

/// How the final image is written.
#[derive(Clone, Copy, Debug)]
pub struct WriteSpec {
    pub out: OutType,
    /// Round half away from zero before an integer type (§3.3: always, but FSL-157's
    /// truncation of `-odt input` on int8 and uint16 with `reproduce`).
    pub round: bool,
    pub fix: bool,
}

/// The values of `img` in the output type, and whether FSL-151's correction changed one.
pub fn output_values<T: Num>(img: &Img<T>, w: WriteSpec) -> (Values, bool) {
    let round = w.round && w.out.is_int() && !T::KIND.is_int();
    let prepared = |v: T| -> f64 {
        let x = v.to_f64();
        if round {
            if x > 0.0 {
                (x + 0.5).trunc()
            } else {
                (x - 0.5).trunc()
            }
        } else {
            x
        }
    };
    fn conv<T: Num, O: Num>(
        data: &[T],
        f: &(dyn Fn(T) -> f64 + Sync),
        fix: bool,
    ) -> (Vec<O>, bool) {
        let mut out = vec![O::default(); data.len()];
        let ch = out
            .par_chunks_mut(1 << 16)
            .zip(data.par_chunks(1 << 16))
            .map(|(o, s)| {
                let mut ch = false;
                for (o, &v) in o.iter_mut().zip(s) {
                    let (x, c) = O::convert(f(v), fix);
                    *o = x;
                    ch |= c;
                }
                ch
            })
            .reduce(|| false, |a, b| a || b);
        (out, ch)
    }
    let fix = w.fix;
    match w.out {
        OutType::Char => {
            let (v, c) = conv::<T, u8>(&img.data, &prepared, fix);
            (Values::U8(v), c)
        }
        OutType::SChar => {
            let (v, c) = conv::<T, i8>(&img.data, &prepared, fix);
            (Values::I8(v), c)
        }
        OutType::Short => {
            let (v, c) = conv::<T, i16>(&img.data, &prepared, fix);
            (Values::I16(v), c)
        }
        OutType::Int => {
            let (v, c) = conv::<T, i32>(&img.data, &prepared, fix);
            (Values::I32(v), c)
        }
        OutType::Float => {
            let (v, c) = conv::<T, f32>(&img.data, &prepared, fix);
            (Values::F32(v), c)
        }
        OutType::Double => {
            let (v, c) = conv::<T, f64>(&img.data, &prepared, fix);
            (Values::F64(v), c)
        }
    }
}

/// The values of `img` moved out as the output's where writing them converts nothing (the
/// output type is the computing type's: float, double, short, int; unsigned char from the
/// unsigned char computing type, signed char from char's), so that writing holds no second copy
/// of the image; `None` otherwise (and `img` untouched). A float NaN is quieted, as the
/// conversion does.
pub fn identity_values<T: Num>(img: &mut Img<T>, w: WriteSpec) -> Option<Values> {
    use std::any::Any;
    let same = matches!(
        (w.out, T::KIND),
        (OutType::Float, Kind::Float)
            | (OutType::Double, Kind::Double)
            | (OutType::Short, Kind::Short)
            | (OutType::Int, Kind::Int)
            | (OutType::Char, Kind::UChar)
            | (OutType::SChar, Kind::Char)
    );
    if !same {
        return None;
    }
    let mut data = std::mem::take(&mut img.data);
    if T::KIND == Kind::Float {
        data.par_iter_mut()
            .with_min_len(1 << 16)
            .filter(|x| x.is_nan())
            .for_each(|x| *x = T::convert(x.to_f64(), w.fix).0);
    }
    let any: Box<dyn Any> = Box::new(data);
    let taken = |what: &str| -> ! { unreachable!("{what}: the output type is the computing type") };
    Some(match w.out {
        OutType::Float => Values::F32(*any.downcast().unwrap_or_else(|_| taken("float"))),
        OutType::Double => Values::F64(*any.downcast().unwrap_or_else(|_| taken("double"))),
        OutType::Short => Values::I16(*any.downcast().unwrap_or_else(|_| taken("short"))),
        OutType::Int => Values::I32(*any.downcast().unwrap_or_else(|_| taken("int"))),
        OutType::Char => Values::U8(*any.downcast().unwrap_or_else(|_| taken("char"))),
        OutType::SChar => Values::I8(*any.downcast().unwrap_or_else(|_| taken("schar"))),
    })
}

/// The output type of the computing type `kind` written as it is (`-save`).
pub fn save_type(kind: Kind) -> OutType {
    match kind {
        Kind::Char | Kind::UChar => OutType::Char,
        Kind::Short => OutType::Short,
        Kind::Int => OutType::Int,
        Kind::Float => OutType::Float,
        Kind::Double => OutType::Double,
    }
}

/// The geometry of a new grid along x (`-subsamp2`, `-subsamp2offc`, the reductions along X;
/// findings 24): `nx_new` voxels along x, and `map` the operation's map of a new voxel to the
/// old in the frame the image is held in (`None`: the identity, a reduction). As FSL's outputs
/// show: a matrix that is set (its code above 0) maps through the held frame and the operation,
/// `M · F(n) · S · F(n')`, one that is unset through the mirroring only, `M · F(n) · F(n')`
/// (`F(n)` mirrors x over `n` voxels for an image held mirrored, else the identity); the
/// matrices as the header holds them in single precision (the qform's from its quaternion,
/// rounded), the products in double. Only the translation changes (the linear part is the
/// operation's scaling, which the voxel sizes carry); NIfTI-1 stores the offsets rounded.
pub fn regrid_x<T>(img: &mut Img<T>, nx_new: usize, map: Option<[[f64; 4]; 3]>) {
    let n = img.dims[0] as f64;
    let n2 = nx_new as f64;
    // F(n) · S · F(n') applied to a voxel (x', y', z') of the new grid: the old voxel
    let along = |m: Option<[[f64; 4]; 3]>| -> [[f64; 4]; 3] {
        let s = m.unwrap_or([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ]);
        if !img.flipped {
            return s;
        }
        // x_r' = n' − 1 − x', x_r = s00 x_r' + s03, x = n − 1 − x_r
        let mut a = s;
        a[0][0] = s[0][0];
        a[0][3] = n - 1.0 - (s[0][0] * (n2 - 1.0) + s[0][3]);
        a
    };
    let h = &mut img.header;
    let single = h.version == larmorx_io::nifti::NiftiVersion::V1;
    let round = |x: f64| if single { f64::from(x as f32) } else { x };
    // the new translation of a matrix (rows `m`, its translation `t`) under `a`
    let moved = |m: [[f64; 3]; 3], t: [f64; 3], a: [[f64; 4]; 3]| -> [f64; 3] {
        [0, 1, 2].map(|r| (0..3).fold(0.0, |acc, k| acc + round(m[r][k]) * a[k][3]) + round(t[r]))
    };
    let set_s = h.sform_code > 0;
    let a = along(if set_s { map } else { None });
    let rows = [h.srow_x, h.srow_y, h.srow_z];
    let lin = rows.map(|r| [r[0], r[1], r[2]]);
    let t = moved(lin, rows.map(|r| r[3]), a);
    for (row, v) in [&mut h.srow_x, &mut h.srow_y, &mut h.srow_z]
        .into_iter()
        .zip(t)
    {
        let scale = [a[0][0], a[1][1], a[2][2]];
        for k in 0..3 {
            row[k] *= scale[k];
        }
        row[3] = v;
    }
    if let Ok(q) = h.qform() {
        let a = along(if h.qform_code > 0 { map } else { None });
        let r = *q.rows();
        let lin = [0, 1, 2].map(|i| [r[i][0], r[i][1], r[i][2]]);
        let t = moved(lin, [r[0][3], r[1][3], r[2][3]], a);
        h.qoffset_x = t[0];
        h.qoffset_y = t[1];
        h.qoffset_z = t[2];
    }
}

/// Why an image could not be written.
pub enum WriteError {
    /// `FSLOUTPUTTYPE` unset or unknown: FSL's message.
    OutputType(String),
    /// The file could not be written.
    File(String),
}

/// Writes `values` (radiological frame) under the name `out` with `FSLOUTPUTTYPE`'s format.
pub fn write<T>(
    settings: &Settings,
    img: &Img<T>,
    out: &str,
    values: Values,
    mode: UpstreamDefects,
    notices: &mut Notices,
    n_threads: usize,
) -> Result<(), WriteError> {
    let ot = match settings.get("FSLOUTPUTTYPE") {
        None => {
            return Err(WriteError::OutputType(
                crate::cli::bet2::FSLOUTPUTTYPE_UNSET.to_owned(),
            ));
        }
        Some(t) if !bimage::OUTPUT_TYPES.contains(&t) => {
            return Err(WriteError::OutputType(format!(
                "{}\n",
                bimage::unknown_output_type_message(t)
            )));
        }
        Some(t) => t.to_owned(),
    };
    let stem = settings.name(files::strip_image_extension(out));
    let Some((name, version)) = bimage::output_file(&stem, &ot) else {
        return Err(WriteError::File(stem));
    };
    let extensions = settings
        .get("FSL_LOAD_NIFTI_EXTENSIONS")
        .is_some_and(|v| crate::cli::options::strtod(v).0 != 0.0);
    let [nx, ny, nz, nt] = img.dims;
    let o = Output {
        template: &img.header,
        flipped: img.flipped,
        shape: [nx, ny, nz],
        volumes: nt,
        cal: Some(img.cal),
        extensions,
        defects: mode,
        n_threads,
        descrip: DESCRIP,
        rewrite_quaternion: true,
    };
    match bimage::write(&name, version, &o, values) {
        Ok(ns) => {
            notices.extend(ns);
            Ok(())
        }
        Err(e) => Err(WriteError::File(format!("{name}: {e}"))),
    }
}

/// FSL-151's notice for a conversion the correction changed.
pub fn fsl151(notices: &mut Notices, what: &str) {
    let detail = match notices.mode() {
        UpstreamDefects::Fix => {
            format!("{what}: values outside the type's range clamped to it, NaN as 0")
        }
        UpstreamDefects::Reproduce => {
            format!("{what}: values outside the type's range converted as x86-64 converts them")
        }
    };
    notices.add(&defects::FSL_151, detail);
}
