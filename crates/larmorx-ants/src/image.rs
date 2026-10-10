// SPDX-License-Identifier: Apache-2.0
//! Images as ANTs programs hold them, and how they read and write them.
//!
//! An ANTs program declares an ITK image type, `itk::Image<T, D>`, and reads its inputs with
//! `ReadImage<ImageType>` (`Utilities/ReadWriteData.h`, ANTs v2.6.5), which goes through ITK's
//! `ImageFileReader` and `NiftiImageIO`. [`AntsImage`] is that image: voxels converted to the
//! pixel type `T` (a C++ `static_cast` from what the NIfTI reader produced), the geometry of a
//! `D`-dimensional ITK image, and the two header strings ITK keeps in its metadata
//! dictionary. [`ImageStore::read`] reproduces `ReadImage`, including:
//! - names shorter than 3 characters fail silently, missing files with
//!   " file <name> does not exist . ";
//! - a file with more dimensions than `D` gives its first `D`-dimensional block, with the
//!   real origin and spacing but an **identity direction** (`ImageFileReader` uses the
//!   ImageIO's default direction in that case);
//! - a file with fewer dimensions is padded with axes of one voxel, spacing 1.
//!
//! [`ImageStore::write`] reproduces `ANTs::WriteImage`: nothing for names shorter than 3
//! characters, else ITK's NIfTI writer ([`larmorx_io::nifti::itk::write_itk_image`]).
//!
//! The store is a trait so that the same program code reads files on the command line and
//! in-memory images from Python ([`FileStore`] is the files).

use std::path::Path;

use larmorx_core::element::Element;
use larmorx_io::nifti::itk::{
    GeometrySource, ItkGeometry, ItkImage, ItkMeta, ItkVoxels, read_itk_image, write_itk_image,
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
pub trait Pixel: Element + Copy + Send + Sync + 'static {
    fn from_f32(v: f32) -> Self;
    fn from_f64(v: f64) -> Self;

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
    fn from_f32(v: f32) -> Self {
        v
    }
    fn from_f64_bounded(v: f64) -> Self {
        bounded(v, -f64::from(f32::MAX), f64::from(f32::MAX)) as f32
    }
    fn from_f64(v: f64) -> Self {
        v as f32
    }
    fn from_f32_vec(mut v: Vec<f32>, count: usize) -> Vec<Self> {
        v.truncate(count);
        v
    }
}

impl Pixel for f64 {
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
    fn from_f64_vec(mut v: Vec<f64>, count: usize) -> Vec<Self> {
        v.truncate(count);
        v
    }
}

impl Pixel for i32 {
    fn from_f32(v: f32) -> Self {
        x86_to_i32(f64::from(v))
    }
    fn from_f64_bounded(v: f64) -> Self {
        x86_to_i32(bounded(v, f64::from(i32::MIN), f64::from(i32::MAX)))
    }
    fn from_f64(v: f64) -> Self {
        x86_to_i32(v)
    }
}

/// Integer pixels that a C++ `static_cast` from `float`/`double` fills with the low bits of
/// x86-64's 32-bit truncation (`char`, `unsigned char`, `short`, `unsigned short`).
macro_rules! small_int_pixel {
    ($($t:ty),*) => {$(
        impl Pixel for $t {
            fn from_f32(v: f32) -> Self {
                x86_to_i32(f64::from(v)) as $t
            }
            fn from_f64(v: f64) -> Self {
                x86_to_i32(v) as $t
            }
            fn from_f64_bounded(v: f64) -> Self {
                x86_to_i32(bounded(v, f64::from(<$t>::MIN), f64::from(<$t>::MAX))) as $t
            }
        }
    )*};
}

small_int_pixel!(u8, i8, i16, u16);

/// `unsigned int`: GCC converts through a 64-bit truncation and keeps the low 32 bits.
impl Pixel for u32 {
    fn from_f32(v: f32) -> Self {
        x86_to_i64(f64::from(v)) as u32
    }
    fn from_f64(v: f64) -> Self {
        x86_to_i64(v) as u32
    }
    fn from_f64_bounded(v: f64) -> Self {
        x86_to_i64(bounded(v, 0.0, f64::from(u32::MAX))) as u32
    }
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
    /// conversion to a pixel type and a dimension).
    fn load(&mut self, name: &str, n_threads: usize) -> Result<ItkImage, ReadError>;

    /// Writes `image` to `name` (the name has at least 3 characters).
    fn save(&mut self, name: &str, image: &OutputImage, n_threads: usize) -> Result<(), String>;

    /// ANTs' `ReadImage<itk::Image<T, dim>>(target, name)`.
    fn read<T: Pixel>(
        &mut self,
        name: &str,
        dim: usize,
        n_threads: usize,
    ) -> Result<AntsImage<T>, ReadError>
    where
        Self: Sized,
    {
        read_with(self, name, dim, n_threads)
    }

    /// `ANTs::WriteImage(image, name)`: returns `false` (and writes nothing) for names
    /// shorter than 3 characters.
    fn write(
        &mut self,
        name: &str,
        image: impl Into<OutputImage>,
        n_threads: usize,
    ) -> Result<bool, String>
    where
        Self: Sized,
    {
        write_with(self, name, image.into(), n_threads)
    }
}

/// [`ImageStore::read`] for trait objects.
pub fn read_with<T: Pixel, S: ImageStore + ?Sized>(
    store: &mut S,
    name: &str,
    dim: usize,
    n_threads: usize,
) -> Result<AntsImage<T>, ReadError> {
    if name.len() < 3 {
        return Err(ReadError::TooShort);
    }
    let image = store.load(name, n_threads)?;
    from_itk(image, name, dim, n_threads)
}

/// [`ImageStore::write`] for trait objects.
pub fn write_with<S: ImageStore + ?Sized>(
    store: &mut S,
    name: &str,
    image: OutputImage,
    n_threads: usize,
) -> Result<bool, String> {
    if name.len() < 3 {
        return Ok(false);
    }
    store.save(name, &image, n_threads)?;
    Ok(true)
}

/// Converts what the NIfTI reader produced (from the file `name`) into `itk::Image<T, dim>`
/// as `ImageFileReader` does.
pub fn from_itk<T: Pixel>(
    image: ItkImage,
    name: &str,
    dim: usize,
    n_threads: usize,
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
    } else {
        for i in 0..n {
            for j in 0..n {
                geometry.direction[i][j] = g.direction[i][j];
            }
        }
    }
    let count: usize = geometry.size.iter().product();
    let data: Vec<T> = larmorx_core::parallel::with_threads(n_threads, || match image.voxels {
        ItkVoxels::F32(v) => T::from_f32_vec(v, count),
        ItkVoxels::F64(v) => T::from_f64_vec(v, count),
    })
    .map_err(|e| ReadError::Failed {
        name: name.to_owned(),
        message: e.to_string(),
    })?;
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
    fn load(&mut self, name: &str, n_threads: usize) -> Result<ItkImage, ReadError> {
        let path = Path::new(name);
        if !path.exists() {
            return Err(ReadError::Missing(name.to_owned()));
        }
        if nifti::NiftiPaths::from_path(path).is_none() {
            return Err(ReadError::Unsupported {
                name: name.to_owned(),
                message: "only NIfTI images (.nii, .nii.gz, .hdr/.img) are supported".into(),
            });
        }
        read_itk_image(path, n_threads).map_err(|e| ReadError::Failed {
            name: name.to_owned(),
            message: e.to_string(),
        })
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

    #[test]
    fn reading_follows_ants_read_image() {
        let dir = tempfile::tempdir().unwrap();
        let p3 = dir.path().join("a.nii");
        write_file(&p3, &[4, 3, 2], 1.0);
        let mut store = FileStore;
        let img: AntsImage<f32> = store.read(p3.to_str().unwrap(), 3, 1).unwrap();
        assert_eq!(img.size(), [4, 3, 2]);
        let vol = img.view();
        assert_eq!(
            (vol.size, vol.spacing),
            (&[4, 3, 2][..], &[2.0, 3.0, 4.0][..])
        );
        assert_eq!(img.meta.descrip, b"test");
        assert_eq!(img.data[5], 2.5);
        let ints: AntsImage<i32> = store.read(p3.to_str().unwrap(), 3, 1).unwrap();
        assert_eq!(ints.data[5], 2);
        assert_eq!(
            store.read::<f32>("ab", 3, 1).unwrap_err(),
            ReadError::TooShort
        );
        let missing = dir.path().join("none.nii");
        let e = store
            .read::<f32>(missing.to_str().unwrap(), 3, 1)
            .unwrap_err();
        assert!(e.ants_message().unwrap().contains("does not exist"));

        // A 4D file read as 3D: the first volume, identity direction.
        let p4 = dir.path().join("b.nii.gz");
        write_file(&p4, &[4, 3, 2, 5], 1.0);
        let img: AntsImage<f32> = store.read(p4.to_str().unwrap(), 3, 1).unwrap();
        assert_eq!(img.size(), [4, 3, 2]);
        assert_eq!(img.data.len(), 24);
        assert_eq!(
            img.geometry.direction,
            vec![
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![0.0, 0.0, 1.0]
            ]
        );
        assert_eq!(img.geometry.origin, [-1.0, -2.0, 3.0]);
        // And read as 4D it keeps its direction (RAS diagonal = LPS flips).
        let img4: AntsImage<f32> = store.read(p4.to_str().unwrap(), 4, 1).unwrap();
        assert_eq!(img4.geometry.direction[0][0], -1.0);
        assert_eq!(img4.size(), [4, 3, 2, 5]);
    }

    #[test]
    fn writing_skips_short_names_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.nii.gz");
        write_file(&p, &[4, 3, 2], -1.0);
        let mut store = FileStore;
        let img: AntsImage<f32> = store.read(p.to_str().unwrap(), 3, 1).unwrap();
        assert!(!store.write("x", img.clone(), 1).unwrap());
        let out = dir.path().join("b.nii");
        assert!(store.write(out.to_str().unwrap(), img.clone(), 1).unwrap());
        let back: AntsImage<f32> = store.read(out.to_str().unwrap(), 3, 1).unwrap();
        assert_eq!(back.data, img.data);
        assert_eq!(back.geometry.direction, img.geometry.direction);
        assert_eq!(back.meta, img.meta);
        assert!(store.write("out.xyz", img, 1).is_err());
    }
}
