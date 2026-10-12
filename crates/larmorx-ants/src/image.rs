// SPDX-License-Identifier: Apache-2.0
//! Images as ANTs programs hold them, and how they read and write them.
//!
//! An ANTs program declares an ITK image type, `itk::Image<T, D>`, and reads its inputs with
//! `ReadImage<ImageType>` (`Utilities/ReadWriteData.h`, ANTs v2.6.5), which goes through ITK's
//! `ImageFileReader` and `NiftiImageIO`. [`AntsImage`] is that image: voxels converted to the
//! pixel type `T` (a C++ `static_cast` from what the NIfTI reader produced), the geometry of a
//! `D`-dimensional ITK image, and the two header strings ITK keeps in its metadata
//! dictionary. [`read_with`] reproduces `ReadImage`, including:
//! - names shorter than 3 characters fail silently (ANTS-03), missing files with
//!   " file <name> does not exist . ";
//! - a file with more dimensions than `D` gives its first `D`-dimensional block, with the
//!   real origin and spacing but an **identity direction** (`ImageFileReader` uses the
//!   ImageIO's default direction in that case; ITK-02);
//! - a file with fewer dimensions is padded with axes of one voxel, spacing 1;
//! - out-of-range values convert to integer pixels as x86-64 converts them (ANTS-06);
//! - asked for `x.nii.gz`, the voxels of `x.nii` are read if it exists (ITK-08).
//!
//! [`write_with`] reproduces `ANTs::WriteImage`: nothing for names shorter than 3
//! characters, else ITK's NIfTI writer ([`larmorx_io::nifti::itk::write_itk_image`]).
//!
//! The defects among these (ANTS-03, ANTS-06, ITK-02, ITK-08) are corrected by default and
//! reproduced with [`UpstreamDefects::Reproduce`] (D13, `docs/api/upstream-defects.md`):
//! every function takes a [`Notices`] with the mode and adds a notice where a defect changed
//! (or would have changed) the result.
//!
//! The store is a trait so that the same program code reads files on the command line and
//! in-memory images from Python ([`FileStore`] is the files).

use std::path::Path;

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_core::element::Element;
use larmorx_io::nifti::itk::{
    GeometrySource, ItkGeometry, ItkImage, ItkMeta, ItkVoxels, read_itk_image,
    read_itk_image_with_data_from, write_itk_image,
};
use larmorx_io::nifti::{self, WriteOptions};
use rayon::prelude::*;

/// An ITK image `itk::Image<T, D>` as an ANTs program holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct AntsImage<T> {
    /// Voxels in Fortran order (`x` fastest).
    pub data: Vec<T>,
    /// The geometry, with `ndim == D`.
    pub geometry: ItkGeometry,
    /// The metadata ITK's writer reads back: the input file's `descrip` and `aux_file` for an
    /// image that was read (and is written after changes in place), empty for an image made
    /// by a filter or by `AllocImage`.
    pub meta: ItkMeta,
}

impl<T> AntsImage<T> {
    /// The number of voxels.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The image dimension `D`.
    pub fn dim(&self) -> usize {
        self.geometry.ndim
    }

    /// The size along each axis.
    pub fn size(&self) -> &[usize] {
        &self.geometry.size
    }

    /// A new image on the same grid with `data` and an empty dictionary: what
    /// `AllocImage<ImageType>(image)` or an ITK filter's output is.
    pub fn derived<U>(&self, data: Vec<U>) -> AntsImage<U> {
        assert_eq!(data.len(), self.data.len(), "voxel count mismatch");
        AntsImage {
            data,
            geometry: self.geometry.clone(),
            meta: ItkMeta::default(),
        }
    }

    /// The same image object with new voxels (kept dictionary): an image changed in place.
    pub fn with_data<U>(self, data: Vec<U>) -> AntsImage<U> {
        assert_eq!(data.len(), self.data.len(), "voxel count mismatch");
        AntsImage {
            data,
            geometry: self.geometry,
            meta: self.meta,
        }
    }
}

impl<T> AntsImage<T> {
    /// The voxels as a [`larmorx_image::VolumeRef`] for the spatial filters: the image's own
    /// `D` dimensions, size and spacing (ITK's spatial filters work in all `D` dimensions: a
    /// 4D image is filtered along time too).
    pub fn view(&self) -> larmorx_image::VolumeRef<'_, T> {
        larmorx_image::VolumeRef::new(&self.data, &self.geometry.size, &self.geometry.spacing)
    }
}

/// A pixel type ANTs programs read images as: the C++ `static_cast` from the value ITK's
/// NIfTI reader produced.
///
/// C++ leaves the conversion of a floating value to an integer type undefined when the value
/// is NaN or its truncation is outside the type's range (defect ANTS-06). [`Pixel::from_f32`]
/// and [`Pixel::from_f64`] answer as x86-64 builds of ANTs do (the value wraps around);
/// [`Pixel::from_f64_clamped`] is larmorx's default (clamped to the type's range, NaN as 0);
/// [`convert_voxels`] chooses between them with the D13 mode.
pub trait Pixel: Element + Copy + Send + Sync + 'static {
    /// The C++ name of the type (`unsigned char`), for messages.
    const NAME: &'static str;

    /// x86-64's conversion (see the trait).
    fn from_f32(v: f32) -> Self;
    /// x86-64's conversion (see the trait).
    fn from_f64(v: f64) -> Self;

    /// Whether C++ defines the conversion of `v` to this type: always for floating types; for
    /// integer types, when `v` is not NaN and its truncation is within the type's range.
    fn defined(v: f64) -> bool;

    /// larmorx's conversion where C++ leaves it undefined: truncation toward zero, clamped to
    /// the type's range, NaN as 0 (Rust's `as`). Equal to [`Pixel::from_f64`] wherever
    /// [`Pixel::defined`] holds.
    fn from_f64_clamped(v: f64) -> Self;

    /// The first `count` values of `v`, converted (in parallel in the current pool: callers
    /// run it inside `larmorx_core::parallel::with_threads`; without a copy where the type is
    /// already right).
    fn from_f32_vec(mut v: Vec<f32>, count: usize) -> Vec<Self> {
        v.truncate(count);
        v.par_iter()
            .with_min_len(1 << 16)
            .map(|&x| Self::from_f32(x))
            .collect()
    }

    /// [`Pixel::from_f32_vec`] for double values.
    fn from_f64_vec(mut v: Vec<f64>, count: usize) -> Vec<Self> {
        v.truncate(count);
        v.par_iter()
            .with_min_len(1 << 16)
            .map(|&x| Self::from_f64(x))
            .collect()
    }

    /// ITK's `CastPixelWithBoundsChecking` (the resampler's output): `v` clamped to the
    /// type's range (`NonpositiveMin` to `max`, compared in double), then a `static_cast`.
    fn from_f64_bounded(v: f64) -> Self;
}

/// x86-64's conversion of a double to a 64-bit integer (`cvttsd2si` with a 64-bit
/// destination, which GCC uses for `unsigned int`): truncation, with NaN and values out of
/// range giving `i64::MIN`.
pub fn x86_to_i64(v: f64) -> i64 {
    if v.is_nan() || v >= 9_223_372_036_854_775_808.0 || v < -9_223_372_036_854_775_808.0 {
        i64::MIN
    } else {
        v as i64
    }
}

/// `v` clamped to `[min, max]` as ITK's `CastPixelWithBoundsChecking` compares (NaN passes).
fn bounded(v: f64, min: f64, max: f64) -> f64 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// x86-64's conversion of a double to a 32-bit integer (`cvttsd2si`): truncation, with NaN
/// and values out of range giving `i32::MIN`. C++ leaves out-of-range conversions undefined;
/// this is what the x86-64 builds of ANTs (and its oracle) do.
pub fn x86_to_i32(v: f64) -> i32 {
    if v.is_nan() || v >= 2_147_483_648.0 || v <= -2_147_483_649.0 {
        i32::MIN
    } else {
        v as i32
    }
}

impl Pixel for f32 {
    const NAME: &'static str = "float";
    fn from_f32(v: f32) -> Self {
        v
    }
    fn from_f64_bounded(v: f64) -> Self {
        bounded(v, -f64::from(f32::MAX), f64::from(f32::MAX)) as f32
    }
    fn from_f64(v: f64) -> Self {
        v as f32
    }
    fn defined(_: f64) -> bool {
        true
    }
    fn from_f64_clamped(v: f64) -> Self {
        v as f32
    }
    fn from_f32_vec(mut v: Vec<f32>, count: usize) -> Vec<Self> {
        v.truncate(count);
        v
    }
}

impl Pixel for f64 {
    const NAME: &'static str = "double";
    fn from_f32(v: f32) -> Self {
        f64::from(v)
    }
    /// For `double` pixels ITK returns the interpolated value as it is (the
    /// `CastComponentWithBoundsChecking` overload for an unchanged component type).
    fn from_f64_bounded(v: f64) -> Self {
        v
    }
    fn from_f64(v: f64) -> Self {
        v
    }
    fn defined(_: f64) -> bool {
        true
    }
    fn from_f64_clamped(v: f64) -> Self {
        v
    }
    fn from_f64_vec(mut v: Vec<f64>, count: usize) -> Vec<Self> {
        v.truncate(count);
        v
    }
}

/// Whether C++ defines the conversion of `v` to an integer type with range `[min, max]`: `v`
/// is not NaN and its truncation is within the range (`min − 1 < v < max + 1`).
fn integer_defined(v: f64, min: f64, max: f64) -> bool {
    v > min - 1.0 && v < max + 1.0
}

impl Pixel for i32 {
    const NAME: &'static str = "int";
    fn from_f32(v: f32) -> Self {
        x86_to_i32(f64::from(v))
    }
    fn from_f64_bounded(v: f64) -> Self {
        x86_to_i32(bounded(v, f64::from(i32::MIN), f64::from(i32::MAX)))
    }
    fn from_f64(v: f64) -> Self {
        x86_to_i32(v)
    }
    fn defined(v: f64) -> bool {
        integer_defined(v, f64::from(i32::MIN), f64::from(i32::MAX))
    }
    fn from_f64_clamped(v: f64) -> Self {
        v as i32
    }
}

/// Integer pixels that a C++ `static_cast` from `float`/`double` fills with the low bits of
/// x86-64's 32-bit truncation (`char`, `unsigned char`, `short`, `unsigned short`).
macro_rules! small_int_pixel {
    ($($t:ty, $name:literal);*) => {$(
        impl Pixel for $t {
            const NAME: &'static str = $name;
            fn from_f32(v: f32) -> Self {
                x86_to_i32(f64::from(v)) as $t
            }
            fn from_f64(v: f64) -> Self {
                x86_to_i32(v) as $t
            }
            fn from_f64_bounded(v: f64) -> Self {
                x86_to_i32(bounded(v, f64::from(<$t>::MIN), f64::from(<$t>::MAX))) as $t
            }
            fn defined(v: f64) -> bool {
                integer_defined(v, f64::from(<$t>::MIN), f64::from(<$t>::MAX))
            }
            fn from_f64_clamped(v: f64) -> Self {
                v as $t
            }
        }
    )*};
}

small_int_pixel!(u8, "unsigned char"; i8, "char"; i16, "short"; u16, "unsigned short");

/// `unsigned int`: GCC converts through a 64-bit truncation and keeps the low 32 bits.
impl Pixel for u32 {
    const NAME: &'static str = "unsigned int";
    fn from_f32(v: f32) -> Self {
        x86_to_i64(f64::from(v)) as u32
    }
    fn from_f64(v: f64) -> Self {
        x86_to_i64(v) as u32
    }
    fn from_f64_bounded(v: f64) -> Self {
        x86_to_i64(bounded(v, 0.0, f64::from(u32::MAX))) as u32
    }
    fn defined(v: f64) -> bool {
        integer_defined(v, 0.0, f64::from(u32::MAX))
    }
    fn from_f64_clamped(v: f64) -> Self {
        v as u32
    }
}

/// The voxels the reader produced, converted to `T` by ANTs' `static_cast` (the first `count`
/// of them): where C++ leaves the conversion undefined (ANTS-06), x86-64's answer with
/// `reproduce`, [`Pixel::from_f64_clamped`] with `fix`. Returns the values and how many
/// conversions were undefined. Runs in the current thread pool.
pub fn convert_voxels<T: Pixel>(
    voxels: ItkVoxels,
    count: usize,
    mode: UpstreamDefects,
) -> (Vec<T>, usize) {
    fn undefined<T: Pixel, U: Copy + Sync>(v: &[U], to_f64: impl Fn(U) -> f64 + Sync) -> usize {
        v.par_iter()
            .with_min_len(1 << 16)
            .filter(|&&x| !T::defined(to_f64(x)))
            .count()
    }
    match voxels {
        ItkVoxels::F32(mut v) => {
            v.truncate(count);
            let n = undefined::<T, f32>(&v, f64::from);
            if n == 0 || mode.reproduce() {
                (T::from_f32_vec(v, count), n)
            } else {
                let out = v
                    .par_iter()
                    .with_min_len(1 << 16)
                    .map(|&x| T::from_f64_clamped(f64::from(x)))
                    .collect();
                (out, n)
            }
        }
        ItkVoxels::F64(mut v) => {
            v.truncate(count);
            let n = undefined::<T, f64>(&v, |x| x);
            if n == 0 || mode.reproduce() {
                (T::from_f64_vec(v, count), n)
            } else {
                let out = v
                    .par_iter()
                    .with_min_len(1 << 16)
                    .map(|&x| T::from_f64_clamped(x))
                    .collect();
                (out, n)
            }
        }
    }
}

/// The notice of ANTS-06 for `n` undefined conversions of `what` (an image) to `T`.
pub fn conversion_notice<T: Pixel>(notices: &mut Notices, what: &str, n: usize) {
    if n == 0 {
        return;
    }
    let detail = if notices.fix() {
        format!(
            "{n} value(s) of {what} are NaN or outside the range of {}: clamped to the range \
             (NaN to 0); ANTs on x86-64 wraps them around",
            T::NAME
        )
    } else {
        format!(
            "{n} value(s) of {what} are NaN or outside the range of {}: wrapped around as \
             ANTs on x86-64 converts them",
            T::NAME
        )
    };
    notices.add(&defects::ANTS_06, detail);
}

/// Why `ReadImage` returned no image.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    /// A name shorter than 3 characters: ANTs returns no image and prints nothing.
    #[error("")]
    TooShort,
    /// The file does not exist (ANTs prints the message).
    #[error(" file {0} does not exist . ")]
    Missing(String),
    /// ITK has no reader for the file (ANTs prints the message).
    #[error(" file {0} is not recognized as a supported image format . ")]
    NotImage(String),
    /// The reader failed (ANTs prints "Exception caught during reference file reading").
    #[error("Exception caught during reference file reading \n{message} file {name}")]
    Failed { name: String, message: String },
    /// ANTs reads it, larmorx does not yet.
    #[error("larmorx: {name}: {message}")]
    Unsupported { name: String, message: String },
}

impl ReadError {
    /// The message ANTs prints to stderr (none for [`ReadError::TooShort`]).
    pub fn ants_message(&self) -> Option<String> {
        match self {
            ReadError::TooShort => None,
            other => Some(other.to_string()),
        }
    }
}

/// An image that a program writes, with its pixel type known at run time. To write another
/// pixel type, add a variant, a `From` impl, and its arms in [`OutputImage::geometry`],
/// [`FileStore::save`] and `output_to_py` in `crates/larmorx-py/src/ants_filters/mod.rs`.
#[derive(Clone, Debug, PartialEq)]
pub enum OutputImage {
    F32(AntsImage<f32>),
    F64(AntsImage<f64>),
    I32(AntsImage<i32>),
    U8(AntsImage<u8>),
    I8(AntsImage<i8>),
    I16(AntsImage<i16>),
    U16(AntsImage<u16>),
    U32(AntsImage<u32>),
}

impl From<AntsImage<f32>> for OutputImage {
    fn from(i: AntsImage<f32>) -> Self {
        OutputImage::F32(i)
    }
}
impl From<AntsImage<f64>> for OutputImage {
    fn from(i: AntsImage<f64>) -> Self {
        OutputImage::F64(i)
    }
}
impl From<AntsImage<i32>> for OutputImage {
    fn from(i: AntsImage<i32>) -> Self {
        OutputImage::I32(i)
    }
}
impl From<AntsImage<u8>> for OutputImage {
    fn from(i: AntsImage<u8>) -> Self {
        OutputImage::U8(i)
    }
}
impl From<AntsImage<i8>> for OutputImage {
    fn from(i: AntsImage<i8>) -> Self {
        OutputImage::I8(i)
    }
}
impl From<AntsImage<i16>> for OutputImage {
    fn from(i: AntsImage<i16>) -> Self {
        OutputImage::I16(i)
    }
}
impl From<AntsImage<u16>> for OutputImage {
    fn from(i: AntsImage<u16>) -> Self {
        OutputImage::U16(i)
    }
}
impl From<AntsImage<u32>> for OutputImage {
    fn from(i: AntsImage<u32>) -> Self {
        OutputImage::U32(i)
    }
}

impl OutputImage {
    pub fn geometry(&self) -> &ItkGeometry {
        match self {
            OutputImage::F32(i) => &i.geometry,
            OutputImage::F64(i) => &i.geometry,
            OutputImage::I32(i) => &i.geometry,
            OutputImage::U8(i) => &i.geometry,
            OutputImage::I8(i) => &i.geometry,
            OutputImage::I16(i) => &i.geometry,
            OutputImage::U16(i) => &i.geometry,
            OutputImage::U32(i) => &i.geometry,
        }
    }
}

/// Where a program's images come from and go to.
pub trait ImageStore {
    /// Reads `name` as ITK's NIfTI reader does (values and geometry as stored, before the
    /// conversion to a pixel type and a dimension). `notices` carries the D13 mode (ITK-08).
    fn load(
        &mut self,
        name: &str,
        n_threads: usize,
        notices: &mut Notices,
    ) -> Result<ItkImage, ReadError>;

    /// Writes `image` to `name`.
    fn save(&mut self, name: &str, image: &OutputImage, n_threads: usize) -> Result<(), String>;
}

/// ANTs' `ReadImage<itk::Image<T, dim>>(target, name)` from `store`.
///
/// ANTs reads nothing, silently, for a name of fewer than 3 characters (ANTS-03): with
/// `reproduce` this returns [`ReadError::TooShort`]; with `fix` the name is read like any
/// other. The notice comes only when such an image exists, since only then does the result
/// change (a missing image is not read either way).
pub fn read_with<T: Pixel, S: ImageStore + ?Sized>(
    store: &mut S,
    name: &str,
    dim: usize,
    n_threads: usize,
    notices: &mut Notices,
) -> Result<AntsImage<T>, ReadError> {
    if name.len() < 3 {
        if notices.reproduce() {
            let mut probe = Notices::new(notices.mode());
            if store.load(name, n_threads, &mut probe).is_ok() {
                notices.add(
                    &defects::ANTS_03,
                    format!("the image '{name}' is not read (ANTs reads no name of fewer than 3 characters)"),
                );
            }
            return Err(ReadError::TooShort);
        }
        let image = store.load(name, n_threads, notices)?;
        notices.add(
            &defects::ANTS_03,
            format!("the image '{name}' is read like any other name; ANTs reads no name of fewer than 3 characters and goes on without it"),
        );
        return from_itk(image, name, dim, n_threads, notices);
    }
    let image = store.load(name, n_threads, notices)?;
    from_itk(image, name, dim, n_threads, notices)
}

/// `ANTs::WriteImage(image, name)` to `store`: whether it wrote.
///
/// ANTs writes nothing, silently, for a name of fewer than 3 characters (ANTS-03): with
/// `reproduce` this returns `false`; with `fix` the name is written like any other (no image
/// file has so short a name, so writing it fails with a message).
pub fn write_with<S: ImageStore + ?Sized>(
    store: &mut S,
    name: &str,
    image: OutputImage,
    n_threads: usize,
    notices: &mut Notices,
) -> Result<bool, String> {
    if name.len() < 3 {
        if notices.reproduce() {
            notices.add(
                &defects::ANTS_03,
                format!("nothing is written to '{name}' (ANTs writes no name of fewer than 3 characters)"),
            );
            return Ok(false);
        }
        notices.add(
            &defects::ANTS_03,
            format!("'{name}' is written like any other name; ANTs writes nothing for a name of fewer than 3 characters"),
        );
        if name.is_empty() {
            return Err("an empty output file name".into());
        }
    }
    store.save(name, &image, n_threads)?;
    Ok(true)
}

/// Converts what the NIfTI reader produced (from the file `name`) into `itk::Image<T, dim>`
/// as `ImageFileReader` does.
///
/// Two D13 defects meet here: the conversion to `T` (ANTS-06, [`convert_voxels`]), and the
/// direction of a file with more dimensions than `dim` (ITK-02): ITK gives the first
/// `dim`-dimensional block the ImageIO's default (identity) direction; with `fix` it keeps
/// the file's direction of its first `dim` axes when they do not reach into the other
/// physical axes (as in every NIfTI time series read in 3D). Where they do (an oblique 3D
/// file read in 2D) no `dim`-dimensional direction describes the block, and ITK's identity
/// is kept in both modes.
pub fn from_itk<T: Pixel>(
    image: ItkImage,
    name: &str,
    dim: usize,
    n_threads: usize,
    notices: &mut Notices,
) -> Result<AntsImage<T>, ReadError> {
    let unsupported = |message: String| ReadError::Unsupported {
        name: name.to_owned(),
        message,
    };
    let g = &image.geometry;
    let file_voxels: usize = g.size.iter().product();
    if image.voxels.len() != file_voxels {
        return Err(unsupported(
            "vector, tensor and multi-component images are not supported yet".into(),
        ));
    }
    let n = g.ndim;
    let mut geometry = ItkGeometry {
        ndim: dim,
        size: vec![1; dim],
        spacing: vec![1.0; dim],
        origin: vec![0.0; dim],
        direction: (0..dim)
            .map(|i| (0..dim).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
            .collect(),
        source: g.source,
        flipped: vec![false; dim],
    };
    for i in 0..dim.min(n) {
        geometry.size[i] = g.size[i];
        geometry.spacing[i] = g.spacing[i];
        geometry.origin[i] = g.origin[i];
        geometry.flipped[i] = g.flipped.get(i).copied().unwrap_or(false);
    }
    if n > dim {
        // The ImageIO's default (identity) direction, flipped where the spacing was negative.
        geometry.source = GeometrySource::Default;
        for j in 0..dim {
            if geometry.flipped[j] {
                geometry.direction[j][j] = -1.0;
            }
        }
        // ITK-02: the file's own direction of the first `dim` axes, where it describes them.
        let block: Vec<Vec<f64>> = g.direction[..dim]
            .iter()
            .map(|row| row[..dim].to_vec())
            .collect();
        let closed = g.direction[dim..n]
            .iter()
            .all(|row| row[..dim].iter().all(|&v| v == 0.0));
        if closed && block != geometry.direction {
            let detail = |verb: &str| {
                format!(
                    "the first {dim}-dimensional block of the {n}-dimensional image '{name}' {verb}"
                )
            };
            if notices.fix() {
                notices.add(
                    &defects::ITK_02,
                    detail("keeps its direction; ITK reads it with the identity direction"),
                );
                geometry.direction = block;
                geometry.source = g.source;
            } else {
                notices.add(
                    &defects::ITK_02,
                    detail("is read with the identity direction, as ITK reads it, not its own"),
                );
            }
        }
    } else {
        for i in 0..n {
            for j in 0..n {
                geometry.direction[i][j] = g.direction[i][j];
            }
        }
    }
    let count: usize = geometry.size.iter().product();
    let mode = notices.mode();
    let (data, undefined) = larmorx_core::parallel::with_threads(n_threads, || {
        convert_voxels::<T>(image.voxels, count, mode)
    })
    .map_err(|e| ReadError::Failed {
        name: name.to_owned(),
        message: e.to_string(),
    })?;
    conversion_notice::<T>(notices, &format!("'{name}'"), undefined);
    Ok(AntsImage {
        data,
        geometry,
        meta: image.meta,
    })
}

/// Images in NIfTI files, read and written as ITK 5.4.5 does.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileStore;

impl ImageStore for FileStore {
    /// Reads `name`. nifti_clib reads the voxels from the first file it finds among the
    /// name's siblings (`x.nii` before `x.nii.gz`, ITK-08,
    /// [`nifti::NiftiPaths::nifti_clib_image_path`]): with `reproduce` so does this; with
    /// `fix` it reads the file named.
    fn load(
        &mut self,
        name: &str,
        n_threads: usize,
        notices: &mut Notices,
    ) -> Result<ItkImage, ReadError> {
        let path = Path::new(name);
        if !path.exists() {
            return Err(ReadError::Missing(name.to_owned()));
        }
        let Some(paths) = nifti::NiftiPaths::from_path(path) else {
            return Err(ReadError::Unsupported {
                name: name.to_owned(),
                message: "only NIfTI images (.nii, .nii.gz, .hdr/.img) are supported".into(),
            });
        };
        let failed = |e: nifti::Error| ReadError::Failed {
            name: name.to_owned(),
            message: e.to_string(),
        };
        let found = paths.nifti_clib_image_path();
        let Some(other) = found.filter(|f| f != paths.image_path()) else {
            return read_itk_image(path, n_threads).map_err(failed);
        };
        // ITK-08: nifti_clib reads the voxels from `other`. Both are read, and the notice
        // comes only when their voxels differ.
        let named = read_itk_image(path, n_threads);
        let clib = read_itk_image_with_data_from(path, &other, n_threads);
        let differ = match (&named, &clib) {
            (Ok(a), Ok(b)) => a.voxels != b.voxels,
            _ => true,
        };
        if differ {
            let other_name = other.display().to_string();
            notices.add(
                &defects::ITK_08,
                if notices.reproduce() {
                    format!(
                        "the voxels of '{name}' are read from '{other_name}', as ITK reads them"
                    )
                } else {
                    format!("'{name}' is read as named; ITK reads its voxels from '{other_name}'")
                },
            );
        }
        if notices.reproduce() { clib } else { named }.map_err(failed)
    }

    fn save(&mut self, name: &str, image: &OutputImage, n_threads: usize) -> Result<(), String> {
        let options = WriteOptions {
            n_threads,
            ..Default::default()
        };
        let result = match image {
            OutputImage::F32(i) => write_itk_image(name, &i.geometry, &i.meta, &i.data, &options),
            OutputImage::F64(i) => write_itk_image(name, &i.geometry, &i.meta, &i.data, &options),
            OutputImage::I32(i) => write_itk_image(name, &i.geometry, &i.meta, &i.data, &options),
            OutputImage::U8(i) => write_itk_image(name, &i.geometry, &i.meta, &i.data, &options),
            OutputImage::I8(i) => write_itk_image(name, &i.geometry, &i.meta, &i.data, &options),
            OutputImage::I16(i) => write_itk_image(name, &i.geometry, &i.meta, &i.data, &options),
            OutputImage::U16(i) => write_itk_image(name, &i.geometry, &i.meta, &i.data, &options),
            OutputImage::U32(i) => write_itk_image(name, &i.geometry, &i.meta, &i.data, &options),
        };
        result.map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::{Affine, DataType};
    use larmorx_io::nifti::{NiftiHeader, NiftiVersion};

    fn write_file(path: &Path, shape: &[usize], pixdim0: f64) {
        let n: usize = shape.iter().product();
        let data: Vec<f32> = (0..n).map(|i| i as f32 * 0.5).collect();
        let mut h = NiftiHeader::new(NiftiVersion::V1, shape, DataType::F32).unwrap();
        let affine = Affine::from_zooms([2.0, 3.0, 4.0], [1.0, 2.0, 3.0]);
        h.set_qform(&affine, 1).unwrap();
        h.set_sform(&affine, 1);
        h.pixdim[1] *= pixdim0;
        h.descrip[..4].copy_from_slice(b"test");
        let a = larmorx_core::ndarray::ArrayD::from_shape_vec(
            larmorx_core::ndarray::IxDyn(shape).f(),
            data,
        )
        .unwrap();
        nifti::write(path, &h, a.view(), &WriteOptions::default()).unwrap();
    }

    use larmorx_core::ndarray::ShapeBuilder;

    fn reproduce() -> Notices {
        Notices::new(UpstreamDefects::Reproduce)
    }

    fn fix() -> Notices {
        Notices::new(UpstreamDefects::Fix)
    }

    fn read<T: Pixel>(name: &str, dim: usize, notices: &mut Notices) -> AntsImage<T> {
        read_with(&mut FileStore, name, dim, 1, notices).unwrap()
    }

    #[test]
    fn reading_follows_ants_read_image() {
        let dir = tempfile::tempdir().unwrap();
        let p3 = dir.path().join("a.nii");
        write_file(&p3, &[4, 3, 2], 1.0);
        let n = &mut reproduce();
        let img: AntsImage<f32> = read(p3.to_str().unwrap(), 3, n);
        assert_eq!(img.size(), [4, 3, 2]);
        let vol = img.view();
        assert_eq!(
            (vol.size, vol.spacing),
            (&[4, 3, 2][..], &[2.0, 3.0, 4.0][..])
        );
        assert_eq!(img.meta.descrip, b"test");
        assert_eq!(img.data[5], 2.5);
        let ints: AntsImage<i32> = read(p3.to_str().unwrap(), 3, n);
        assert_eq!(ints.data[5], 2);
        assert!(n.is_empty());
        assert_eq!(
            read_with::<f32, _>(&mut FileStore, "ab", 3, 1, n).unwrap_err(),
            ReadError::TooShort
        );
        let missing = dir.path().join("none.nii");
        let e =
            read_with::<f32, _>(&mut FileStore, missing.to_str().unwrap(), 3, 1, n).unwrap_err();
        assert!(e.ants_message().unwrap().contains("does not exist"));

        // A 4D file read as 3D: the first volume, identity direction.
        let p4 = dir.path().join("b.nii.gz");
        write_file(&p4, &[4, 3, 2, 5], 1.0);
        let n = &mut reproduce();
        let img: AntsImage<f32> = read(p4.to_str().unwrap(), 3, n);
        assert_eq!(img.size(), [4, 3, 2]);
        assert_eq!(img.data.len(), 24);
        let identity = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        assert_eq!(img.geometry.direction, identity);
        assert_eq!(img.geometry.origin, [-1.0, -2.0, 3.0]);
        assert!(n.contains(&defects::ITK_02));
        // And read as 4D it keeps its direction (RAS diagonal = LPS flips).
        let img4: AntsImage<f32> = read(p4.to_str().unwrap(), 4, &mut reproduce());
        assert_eq!(img4.geometry.direction[0][0], -1.0);
        assert_eq!(img4.size(), [4, 3, 2, 5]);
        // ITK-02 corrected: the first volume keeps the file's direction.
        let n = &mut fix();
        let img: AntsImage<f32> = read(p4.to_str().unwrap(), 3, n);
        assert_eq!(
            img.geometry.direction,
            img4.geometry.direction[..3]
                .iter()
                .map(|r| r[..3].to_vec())
                .collect::<Vec<_>>()
        );
        assert_eq!(img.geometry.direction[0][0], -1.0);
        assert_eq!(
            img.data,
            read::<f32>(p4.to_str().unwrap(), 3, &mut reproduce()).data
        );
        assert!(n.contains(&defects::ITK_02));
    }

    #[test]
    fn short_names_are_names_when_corrected() {
        // A missing short name: not read either way, so nothing to say.
        let n = &mut fix();
        let e = read_with::<f32, _>(&mut FileStore, "ab", 3, 1, n).unwrap_err();
        assert_eq!(e, ReadError::Missing("ab".into()));
        assert!(n.is_empty());
        let n = &mut reproduce();
        let e = read_with::<f32, _>(&mut FileStore, "ab", 3, 1, n).unwrap_err();
        assert_eq!(e, ReadError::TooShort);
        assert!(n.is_empty());

        // An existing one: read when corrected, skipped by ANTs.
        struct One(ItkImage);
        impl ImageStore for One {
            fn load(
                &mut self,
                name: &str,
                _: usize,
                _: &mut Notices,
            ) -> Result<ItkImage, ReadError> {
                match name {
                    "ab" => Ok(self.0.clone()),
                    _ => Err(ReadError::Missing(name.into())),
                }
            }
            fn save(&mut self, _: &str, _: &OutputImage, _: usize) -> Result<(), String> {
                Ok(())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.nii.gz");
        write_file(&p, &[4, 3, 2], 1.0);
        let image = FileStore.load(p.to_str().unwrap(), 1, &mut fix()).unwrap();
        let mut store = One(image);
        let n = &mut fix();
        let img = read_with::<f32, _>(&mut store, "ab", 3, 1, n).unwrap();
        assert_eq!(img.size(), [4, 3, 2]);
        assert!(n.contains(&defects::ANTS_03));
        let n = &mut reproduce();
        let e = read_with::<f32, _>(&mut store, "ab", 3, 1, n).unwrap_err();
        assert_eq!(e, ReadError::TooShort);
        assert!(n.contains(&defects::ANTS_03));
    }

    #[test]
    fn writing_skips_short_names_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.nii.gz");
        write_file(&p, &[4, 3, 2], -1.0);
        let n = &mut reproduce();
        let img: AntsImage<f32> = read(p.to_str().unwrap(), 3, n);
        assert!(!write_with(&mut FileStore, "x", img.clone().into(), 1, n).unwrap());
        assert!(n.contains(&defects::ANTS_03));
        let out = dir.path().join("b.nii");
        let n = &mut reproduce();
        assert!(
            write_with(
                &mut FileStore,
                out.to_str().unwrap(),
                img.clone().into(),
                1,
                n
            )
            .unwrap()
        );
        let back: AntsImage<f32> = read(out.to_str().unwrap(), 3, n);
        assert_eq!(back.data, img.data);
        assert_eq!(back.geometry.direction, img.geometry.direction);
        assert_eq!(back.meta, img.meta);
        assert!(n.is_empty());
        assert!(write_with(&mut FileStore, "out.xyz", img.clone().into(), 1, n).is_err());
        // Corrected: a short name is written like any other, and fails as one.
        let n = &mut fix();
        assert!(write_with(&mut FileStore, "x", img.into(), 1, n).is_err());
        assert!(n.contains(&defects::ANTS_03));
    }

    #[test]
    fn integer_conversions_clamp_or_wrap() {
        let values = ItkVoxels::F32(vec![256.0, -1.0, 255.9, -0.9, 3.5, f32::NAN]);
        let (wrapped, n) = convert_voxels::<u8>(values.clone(), 6, UpstreamDefects::Reproduce);
        assert_eq!((wrapped, n), (vec![0, 255, 255, 0, 3, 0], 3));
        let (clamped, n) = convert_voxels::<u8>(values.clone(), 6, UpstreamDefects::Fix);
        assert_eq!((clamped, n), (vec![255, 0, 255, 0, 3, 0], 3));
        let (ints, n) = convert_voxels::<i32>(values, 6, UpstreamDefects::Reproduce);
        assert_eq!(ints[5], i32::MIN);
        assert_eq!(n, 1);
        let big = ItkVoxels::F64(vec![-1.0, 4_294_967_296.0, 7.9]);
        let (u, n) = convert_voxels::<u32>(big.clone(), 3, UpstreamDefects::Reproduce);
        assert_eq!((u, n), (vec![u32::MAX, 0, 7], 2));
        let (u, _) = convert_voxels::<u32>(big, 3, UpstreamDefects::Fix);
        assert_eq!(u, vec![0, u32::MAX, 7]);
        // Floating pixels are always defined.
        let (f, n) = convert_voxels::<f32>(ItkVoxels::F64(vec![1e300]), 1, UpstreamDefects::Fix);
        assert_eq!((f[0], n), (f32::INFINITY, 0));
        let mut notices = fix();
        conversion_notice::<u8>(&mut notices, "'m.nii'", 3);
        assert!(notices.as_slice()[0].detail.contains("unsigned char"));
    }

    #[test]
    fn nifti_clib_reads_the_uncompressed_voxels() {
        let dir = tempfile::tempdir().unwrap();
        let gz = dir.path().join("x.nii.gz");
        let plain = dir.path().join("x.nii");
        write_file(&gz, &[4, 3, 2], 1.0);
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[4, 3, 2], DataType::F32).unwrap();
        h.set_sform(&Affine::from_zooms([1.0; 3], [0.0; 3]), 1);
        let a = larmorx_core::ndarray::ArrayD::from_elem(
            larmorx_core::ndarray::IxDyn(&[4, 3, 2]).f(),
            7.0f32,
        );
        nifti::write(&plain, &h, a.view(), &WriteOptions::default()).unwrap();
        let name = gz.to_str().unwrap();
        let n = &mut reproduce();
        let img: AntsImage<f32> = read(name, 3, n);
        assert!(img.data.iter().all(|&v| v == 7.0));
        // The header is x.nii.gz's.
        assert_eq!(img.geometry.spacing, [2.0, 3.0, 4.0]);
        assert!(n.contains(&defects::ITK_08));
        let n = &mut fix();
        let img: AntsImage<f32> = read(name, 3, n);
        assert_eq!(img.data[5], 2.5);
        assert!(n.contains(&defects::ITK_08));
        // Asked for x.nii: nothing to say.
        let n = &mut fix();
        let _: AntsImage<f32> = read(plain.to_str().unwrap(), 3, n);
        assert!(n.is_empty());
    }
}
