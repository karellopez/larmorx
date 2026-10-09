//! NIfTI-1 and NIfTI-2 images: `.nii`, `.nii.gz`, and `.hdr`/`.img` pairs (optionally gzipped),
//! in either byte order.
//!
//! Reading follows nibabel 5.x (the reference for fMRIPrep's I/O): the same header fixes, the
//! same affine (sform, else qform, else a default from the voxel sizes), the same scaling rules
//! and result types. Writing produces the header nibabel would write for the same image, and
//! compresses in parallel with output that does not depend on the thread count.
//!
//! Deliberate differences from nibabel, all for robustness:
//! - gzip is detected from the file's first bytes, not from its name;
//! - a single file whose `vox_offset` is 0 is rejected instead of read from byte 0;
//! - data are written in the array's own type, never rescaled to the header's type.

mod bytes;
pub mod datatype;
pub mod extension;
pub mod header;
pub mod paths;

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use larmorx_core::affine::Affine;
use larmorx_core::array::DynArray;
use larmorx_core::element::{DataType, Element, RealElement};
use larmorx_core::image::DynImage;
use larmorx_core::ndarray::{ArrayD, ArrayViewD, IxDyn, ShapeBuilder};
use larmorx_core::parallel::{self, ThreadPoolError};
use larmorx_core::{Complex, dispatch_dyn_array};
use rayon::prelude::*;

pub use extension::{Extension, ExtensionError};
pub use header::{ByteOrder, HeaderError, HeaderFix, NiftiHeader, NiftiVersion, text_field, xform};
pub use paths::NiftiPaths;

use crate::gzip::ParallelGzEncoder;

/// A NIfTI read or write failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{}: {source}", path.display())]
    Header {
        path: PathBuf,
        #[source]
        source: HeaderError,
    },
    #[error("{}: bad header extension: {source}", path.display())]
    Extension {
        path: PathBuf,
        #[source]
        source: ExtensionError,
    },
    #[error("{}: {message}", path.display())]
    Format { path: PathBuf, message: String },
    #[error("{0}")]
    InvalidArgument(String),
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
}

type Result<T, E = Error> = std::result::Result<T, E>;

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ {
    move |source| Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn header_err(path: &Path) -> impl FnOnce(HeaderError) -> Error + '_ {
    move |source| Error::Header {
        path: path.to_path_buf(),
        source,
    }
}

fn format_err(path: &Path, message: impl Into<String>) -> Error {
    Error::Format {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

/// How stored values become the returned array.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scaling {
    /// nibabel's `img.dataobj`: the stored type when the header has no scaling, float64
    /// `stored · scl_slope + scl_inter` (complex128 for complex data) when it has.
    #[default]
    Auto,
    /// The stored values in the stored type; `scl_slope`/`scl_inter` are ignored.
    Raw,
    /// float32, scaled if the header says so (nibabel's `get_fdata(dtype=np.float32)`).
    F32,
    /// float64, scaled if the header says so (nibabel's `get_fdata()`).
    F64,
}

/// Options for [`read`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadOptions {
    pub scaling: Scaling,
    /// Threads for byte swapping and scaling (0 = all logical CPUs).
    pub n_threads: usize,
}

impl Default for ReadOptions {
    fn default() -> Self {
        ReadOptions {
            scaling: Scaling::Auto,
            n_threads: 1,
        }
    }
}

/// Options for [`write`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WriteOptions {
    /// gzip level for `.gz` names: 0 (store) to 9 (smallest). The default is 2: with zlib-rs,
    /// level 1 is a faster strategy that compresses much less, while level 2 gives the size of
    /// zlib's level 1 (what nibabel writes) and is still faster than it.
    pub compression_level: u32,
    /// Threads for compression (0 = all logical CPUs). The output does not depend on it.
    pub n_threads: usize,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            compression_level: 2,
            n_threads: 1,
        }
    }
}

/// A NIfTI file as read: its header (after nibabel's load-time fixes) and its voxel data.
#[derive(Clone, Debug, PartialEq)]
pub struct NiftiImage {
    pub header: NiftiHeader,
    pub data: DynArray,
    /// Header fields that were fixed while reading (see [`NiftiHeader::apply_load_fixes`]).
    pub fixes: Vec<HeaderFix>,
    /// Whether `scl_slope`/`scl_inter` were applied to `data`.
    pub scaled: bool,
}

impl NiftiImage {
    /// The image affine (see [`NiftiHeader::best_affine`]).
    pub fn affine(&self) -> Result<Affine, HeaderError> {
        self.header.best_affine()
    }

    /// The data and the affine, without the header.
    pub fn into_image(self) -> Result<DynImage, HeaderError> {
        let affine = self.header.best_affine()?;
        Ok(DynImage {
            data: self.data,
            affine,
        })
    }
}

// ------------------------------------------------------------------------------------------------
// Reading

/// Opens a file for sequential reading, decompressing it if it starts with the gzip magic.
fn open_stream(path: &Path) -> Result<Box<dyn Read + Send>> {
    let mut file = File::open(path).map_err(io_err(path))?;
    let mut magic = [0u8; 2];
    let n = extension::read_up_to(&mut file, &mut magic).map_err(io_err(path))?;
    file.seek(SeekFrom::Start(0)).map_err(io_err(path))?;
    let reader = BufReader::with_capacity(1 << 20, file);
    Ok(if n == 2 && magic == [0x1f, 0x8b] {
        Box::new(flate2::read::MultiGzDecoder::new(reader))
    } else {
        Box::new(reader)
    })
}

fn checked_paths(path: &Path) -> Result<NiftiPaths> {
    NiftiPaths::from_path(path).ok_or_else(|| {
        Error::InvalidArgument(format!(
            "{}: not a NIfTI file name (expected .nii, .nii.gz, .hdr, .img, .hdr.gz or .img.gz)",
            path.display()
        ))
    })
}

/// Reads the header block, the extender and the extensions. Returns the header and the number
/// of bytes consumed from the stream.
fn read_header_from(
    reader: &mut dyn Read,
    path: &Path,
    single_file: bool,
) -> Result<(NiftiHeader, usize)> {
    let mut block = vec![0u8; header::NIFTI2_HEADER_SIZE];
    let got = extension::read_up_to(reader, &mut block[..4]).map_err(io_err(path))?;
    if got < 4 {
        return Err(header_err(path)(HeaderError::NotNifti));
    }
    let size = match NiftiHeader::sniff(&block[..4]).map_err(header_err(path))?.0 {
        NiftiVersion::V1 => header::NIFTI1_HEADER_SIZE,
        NiftiVersion::V2 => header::NIFTI2_HEADER_SIZE,
    };
    block.truncate(size);
    if extension::read_up_to(reader, &mut block[4..]).map_err(io_err(path))? < size - 4 {
        return Err(format_err(path, "file ends inside the header"));
    }
    let mut hdr = NiftiHeader::parse(&block).map_err(header_err(path))?;
    let mut consumed = size;
    let mut extender = [0u8; 4];
    let got = extension::read_up_to(reader, &mut extender).map_err(io_err(path))?;
    consumed += got;
    if got == 4 && extender[0] != 0 {
        let limit = if single_file {
            let offset = vox_offset_bytes(&hdr, path)?;
            Some(offset.saturating_sub(consumed))
        } else {
            None
        };
        hdr.extensions = Extension::read_all(reader, hdr.byte_order, limit).map_err(|source| {
            Error::Extension {
                path: path.to_path_buf(),
                source,
            }
        })?;
        consumed += hdr
            .extensions
            .iter()
            .map(Extension::size_on_disk)
            .sum::<usize>();
    }
    Ok((hdr, consumed))
}

/// `vox_offset` as a byte count (truncated, as nibabel does).
fn vox_offset_bytes(hdr: &NiftiHeader, path: &Path) -> Result<usize> {
    let offset = hdr.vox_offset;
    if !(offset >= 0.0 && offset.is_finite()) {
        return Err(format_err(path, format!("invalid vox_offset {offset}")));
    }
    Ok(offset as usize)
}

/// Reads the header of a NIfTI file exactly as stored (no fixes applied), with its extensions.
pub fn read_header(path: impl AsRef<Path>) -> Result<NiftiHeader> {
    let path = path.as_ref();
    let paths = checked_paths(path)?;
    let mut reader = open_stream(paths.header_path())?;
    Ok(read_header_from(&mut *reader, paths.header_path(), paths.is_single())?.0)
}

/// Reads a NIfTI image.
pub fn read(path: impl AsRef<Path>, options: &ReadOptions) -> Result<NiftiImage> {
    let path = path.as_ref();
    let paths = checked_paths(path)?;
    let hpath = paths.header_path();
    let mut reader = open_stream(hpath)?;
    let (mut hdr, consumed) = read_header_from(&mut *reader, hpath, paths.is_single())?;
    let fixes = hdr.apply_load_fixes().map_err(header_err(hpath))?;
    let shape = hdr.shape().map_err(header_err(hpath))?;
    let data_type = hdr.data_type().map_err(header_err(hpath))?;

    let offset = vox_offset_bytes(&hdr, hpath)?;
    let ipath = paths.image_path();
    let mut data_reader = if paths.is_single() {
        if offset == 0 {
            return Err(format_err(hpath, "vox_offset is 0 in a single-file image"));
        }
        let skip = offset.checked_sub(consumed).ok_or_else(|| {
            format_err(
                hpath,
                format!("vox_offset {offset} points inside the header or extensions"),
            )
        })?;
        io::copy(&mut (&mut reader).take(skip as u64), &mut io::sink()).map_err(io_err(hpath))?;
        reader
    } else {
        let mut r = open_stream(ipath)?;
        io::copy(&mut (&mut r).take(offset as u64), &mut io::sink()).map_err(io_err(ipath))?;
        r
    };

    let n_elements = shape
        .iter()
        .try_fold(1usize, |acc, &d| acc.checked_mul(d))
        .ok_or_else(|| format_err(hpath, format!("shape {shape:?} is too large")))?;
    let order = hdr.byte_order;
    let n_threads = options.n_threads;
    let raw = parallel::with_threads(n_threads, || -> Result<DynArray> {
        let r = &mut *data_reader;
        Ok(match data_type {
            DataType::U8 => read_array::<u8>(r, &shape, n_elements, order, ipath)?.into(),
            DataType::I8 => read_array::<i8>(r, &shape, n_elements, order, ipath)?.into(),
            DataType::U16 => read_array::<u16>(r, &shape, n_elements, order, ipath)?.into(),
            DataType::I16 => read_array::<i16>(r, &shape, n_elements, order, ipath)?.into(),
            DataType::U32 => read_array::<u32>(r, &shape, n_elements, order, ipath)?.into(),
            DataType::I32 => read_array::<i32>(r, &shape, n_elements, order, ipath)?.into(),
            DataType::U64 => read_array::<u64>(r, &shape, n_elements, order, ipath)?.into(),
            DataType::I64 => read_array::<i64>(r, &shape, n_elements, order, ipath)?.into(),
            DataType::F32 => read_array::<f32>(r, &shape, n_elements, order, ipath)?.into(),
            DataType::F64 => read_array::<f64>(r, &shape, n_elements, order, ipath)?.into(),
            DataType::C64 => {
                read_array::<Complex<f32>>(r, &shape, n_elements, order, ipath)?.into()
            }
            DataType::C128 => {
                read_array::<Complex<f64>>(r, &shape, n_elements, order, ipath)?.into()
            }
        })
    })??;

    let slope_inter = hdr.slope_inter().map_err(header_err(hpath))?;
    let (data, scaled) = parallel::with_threads(n_threads, || {
        apply_scaling(raw, slope_inter, options.scaling)
    })?
    .map_err(|m| format_err(hpath, m))?;
    Ok(NiftiImage {
        header: hdr,
        data,
        fixes,
        scaled,
    })
}

/// Reads `n` elements in `order` into an array of `shape` in Fortran order.
fn read_array<T: Element>(
    reader: &mut dyn Read,
    shape: &[usize],
    n: usize,
    order: ByteOrder,
    path: &Path,
) -> Result<ArrayD<T>> {
    n.checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| format_err(path, "image too large"))?;
    let mut values = vec![T::zeroed(); n];
    let bytes: &mut [u8] = bytemuck::cast_slice_mut(&mut values);
    let got = extension::read_up_to(reader, bytes).map_err(io_err(path))?;
    if got < bytes.len() {
        return Err(format_err(
            path,
            format!("expected {} bytes of voxel data, found {got}", bytes.len()),
        ));
    }
    if !order.is_native() {
        values
            .par_iter_mut()
            .with_min_len(1 << 16)
            .for_each(|v| *v = v.swap_bytes());
    }
    ArrayD::from_shape_vec(IxDyn(shape).f(), values).map_err(|e| format_err(path, e.to_string()))
}

/// nibabel's scaling: multiply only if the slope is not 1, add only if the intercept is not 0
/// (so `-0.0` survives a zero intercept), in float64.
fn scale(value: f64, slope: f64, inter: f64) -> f64 {
    let mut v = value;
    if slope != 1.0 {
        v *= slope;
    }
    if inter != 0.0 {
        v += inter;
    }
    v
}

fn map_real<T: RealElement, U: Element>(a: &ArrayD<T>, f: impl Fn(T) -> U + Sync) -> ArrayD<U> {
    let src = a
        .as_slice_memory_order()
        .expect("arrays read from files are contiguous");
    let out: Vec<U> = src
        .par_iter()
        .with_min_len(1 << 15)
        .map(|&v| f(v))
        .collect();
    ArrayD::from_shape_vec(IxDyn(a.shape()).f(), out).expect("same shape")
}

fn to_float<T: RealElement>(a: &ArrayD<T>, si: Option<(f64, f64)>, f32_out: bool) -> DynArray {
    let (s, i) = si.unwrap_or((1.0, 0.0));
    if f32_out {
        // Through f64 first, as nibabel does (exact except for 64-bit integers).
        map_real(a, move |v| scale(v.to_f64(), s, i) as f32).into()
    } else {
        map_real(a, move |v| scale(v.to_f64(), s, i)).into()
    }
}

/// nibabel's scaling of complex data, with numpy's arithmetic: the real slope and intercept are
/// promoted to complex (`s + 0i`) and combined with full complex products and sums, so special
/// values propagate as in numpy (e.g. `(-0.0 + inf i)·2` has a NaN real part).
fn scale_complex<T: Element>(
    a: &ArrayD<T>,
    s: f64,
    i: f64,
    f: impl Fn(T) -> Complex<f64> + Sync,
) -> DynArray {
    let src = a.as_slice_memory_order().expect("contiguous");
    let out: Vec<Complex<f64>> = src
        .par_iter()
        .map(|&v| {
            let Complex { mut re, mut im } = f(v);
            if s != 1.0 {
                (re, im) = (re * s - im * 0.0, re * 0.0 + im * s);
            }
            if i != 0.0 {
                (re, im) = (re + i, im + 0.0);
            }
            Complex::new(re, im)
        })
        .collect();
    ArrayD::from_shape_vec(IxDyn(a.shape()).f(), out)
        .expect("same shape")
        .into()
}

/// Applies `scaling`; returns the data and whether a slope/intercept was applied.
fn apply_scaling(
    raw: DynArray,
    slope_inter: Option<(f64, f64)>,
    scaling: Scaling,
) -> Result<(DynArray, bool), String> {
    let effective = slope_inter.filter(|&(s, i)| s != 1.0 || i != 0.0);
    let f32_out = match scaling {
        Scaling::Raw => return Ok((raw, false)),
        Scaling::Auto if effective.is_none() => return Ok((raw, false)),
        Scaling::Auto | Scaling::F64 => false,
        Scaling::F32 => true,
    };
    if f32_out && effective.is_none() && raw.data_type() == DataType::F32 {
        return Ok((raw, false));
    }
    if !f32_out && effective.is_none() && raw.data_type() == DataType::F64 {
        return Ok((raw, false));
    }
    let out = match &raw {
        DynArray::U8(a) => to_float(a, effective, f32_out),
        DynArray::I8(a) => to_float(a, effective, f32_out),
        DynArray::U16(a) => to_float(a, effective, f32_out),
        DynArray::I16(a) => to_float(a, effective, f32_out),
        DynArray::U32(a) => to_float(a, effective, f32_out),
        DynArray::I32(a) => to_float(a, effective, f32_out),
        DynArray::U64(a) => to_float(a, effective, f32_out),
        DynArray::I64(a) => to_float(a, effective, f32_out),
        DynArray::F32(a) => to_float(a, effective, f32_out),
        DynArray::F64(a) => to_float(a, effective, f32_out),
        DynArray::C64(a) if scaling == Scaling::Auto => {
            let (s, i) = effective.expect("checked above");
            scale_complex(a, s, i, |c| Complex::new(f64::from(c.re), f64::from(c.im)))
        }
        DynArray::C128(a) if scaling == Scaling::Auto => {
            let (s, i) = effective.expect("checked above");
            scale_complex(a, s, i, |c| c)
        }
        DynArray::C64(_) | DynArray::C128(_) => {
            return Err("complex data cannot be read as real floats".into());
        }
    };
    Ok((out, effective.is_some()))
}

// ------------------------------------------------------------------------------------------------
// Writing

/// The header nibabel would write for an image of `shape`, `data_type` and `affine`.
///
/// Without a `template`, a fresh header with the affine in the sform (code 2, aligned) and the
/// qform (code 0, unknown). With one, the template's fields are kept; its qform/sform are kept
/// too if `affine` is close to the template's own affine (numpy `allclose`, rtol 1e-5,
/// atol 1e-8), and replaced as above otherwise. The scale factors are set to "no scaling"
/// (slope 1, intercept 0, as nibabel writes them), because [`write`] stores the data as they
/// are. `version` defaults to the template's, or to NIfTI-1 when the shape fits, else NIfTI-2.
pub fn header_for_image(
    template: Option<&NiftiHeader>,
    version: Option<NiftiVersion>,
    shape: &[usize],
    data_type: DataType,
    affine: &Affine,
) -> Result<NiftiHeader, HeaderError> {
    let fits_v1 = NiftiHeader::new(NiftiVersion::V1, shape, data_type).is_ok();
    let version = version.unwrap_or(match template {
        Some(t) if t.version == NiftiVersion::V2 || fits_v1 => t.version,
        _ if fits_v1 => NiftiVersion::V1,
        _ => NiftiVersion::V2,
    });
    let mut hdr = match template {
        Some(t) => {
            let mut h = t.with_version(version);
            h.set_data_type(data_type);
            set_shape_if_changed(&mut h, shape)?;
            h
        }
        None => NiftiHeader::new(version, shape, data_type)?,
    };
    let keep = template.is_some()
        && hdr
            .best_affine()
            .is_ok_and(|a| affine.allclose(&a, 1e-5, 1e-8));
    if !keep {
        hdr.set_affine(affine)?;
    }
    hdr.scl_slope = 1.0;
    hdr.scl_inter = 0.0;
    Ok(hdr)
}

/// Sets the shape only if it differs, as nibabel does: setting it resets `pixdim` beyond the
/// last dimension, which would lose the voxel sizes of 1D and 2D images.
fn set_shape_if_changed(hdr: &mut NiftiHeader, shape: &[usize]) -> Result<(), HeaderError> {
    if hdr.shape().ok().as_deref() != Some(shape) {
        hdr.set_shape(shape)?;
    }
    Ok(())
}

/// The header block, extender and extensions of a file to be written.
fn header_bytes(hdr: &NiftiHeader) -> io::Result<Vec<u8>> {
    let mut out = hdr.to_bytes();
    if hdr.extensions.is_empty() {
        if hdr.is_single_file() {
            out.extend_from_slice(&[0; 4]);
        }
    } else {
        out.extend_from_slice(&[1, 0, 0, 0]);
        for e in &hdr.extensions {
            e.write(&mut out, hdr.byte_order)?;
        }
    }
    Ok(out)
}

/// Writes `data` in Fortran order and `order` byte order.
fn write_data<T: Element>(
    w: &mut dyn Write,
    data: &ArrayViewD<'_, T>,
    order: ByteOrder,
) -> io::Result<()> {
    const CHUNK: usize = 1 << 16;
    let swap = !order.is_native();
    let fortran = data.t();
    if let Some(slice) = fortran.as_slice() {
        if !swap {
            return w.write_all(bytemuck::cast_slice(slice));
        }
        for chunk in slice.chunks(CHUNK) {
            let swapped: Vec<T> = chunk.iter().map(|v| v.swap_bytes()).collect();
            w.write_all(bytemuck::cast_slice(&swapped))?;
        }
        return Ok(());
    }
    let mut buf = Vec::with_capacity(CHUNK);
    for &v in fortran.iter() {
        buf.push(if swap { v.swap_bytes() } else { v });
        if buf.len() == CHUNK {
            w.write_all(bytemuck::cast_slice(&buf))?;
            buf.clear();
        }
    }
    w.write_all(bytemuck::cast_slice(&buf))
}

/// Creates `path` and runs `body` with a writer that compresses when the name ends in `.gz`.
fn with_output(
    path: &Path,
    level: u32,
    body: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<()> {
    let run = || -> io::Result<()> {
        let file = BufWriter::with_capacity(1 << 20, File::create(path)?);
        if paths::wants_gzip(path) {
            let mut enc = ParallelGzEncoder::new(file, level)?;
            body(&mut enc)?;
            enc.finish()?
                .into_inner()
                .map_err(io::IntoInnerError::into_error)?;
        } else {
            let mut file = file;
            body(&mut file)?;
            file.into_inner().map_err(io::IntoInnerError::into_error)?;
        }
        Ok(())
    };
    run().map_err(|source| {
        let _ = std::fs::remove_file(path);
        Error::Io {
            path: path.to_path_buf(),
            source,
        }
    })
}

/// Writes `data` with `header` to `path` (`.nii`, `.nii.gz`, `.hdr`/`.img`, `.hdr.gz`/`.img.gz`).
///
/// The data type, `bitpix`, `dim`, magic string and `vox_offset` are set from `data` and the
/// file name; every other field is written as given (see [`header_for_image`] to build a
/// header from an affine). Data are written in Fortran order in the header's byte order.
pub fn write<T: Element>(
    path: impl AsRef<Path>,
    header: &NiftiHeader,
    data: ArrayViewD<'_, T>,
    options: &WriteOptions,
) -> Result<()> {
    let path = path.as_ref();
    let paths = checked_paths(path)?;
    let mut hdr = header.clone();
    hdr.set_data_type(T::DATA_TYPE);
    set_shape_if_changed(&mut hdr, data.shape()).map_err(header_err(path))?;
    hdr.magic = hdr.version.magic(paths.is_single());
    if hdr.version == NiftiVersion::V2 {
        hdr.nifti2.eol_check = *b"\r\n\x1a\n";
    }
    let ext_bytes: usize = hdr.extensions.iter().map(Extension::size_on_disk).sum();
    hdr.vox_offset = if paths.is_single() {
        (hdr.version.min_vox_offset() + ext_bytes) as f64
    } else {
        0.0
    };
    let head = header_bytes(&hdr).map_err(io_err(path))?;
    let level = options.compression_level;
    let order = hdr.byte_order;
    parallel::with_threads(options.n_threads, || match &paths {
        NiftiPaths::Single(p) => with_output(p, level, |w| {
            w.write_all(&head)?;
            write_data(w, &data, order)
        }),
        NiftiPaths::Pair { header, image } => {
            with_output(header, level, |w| w.write_all(&head))?;
            with_output(image, level, |w| write_data(w, &data, order))
        }
    })?
}

/// [`write`] for an array whose element type is known at run time.
pub fn write_dyn(
    path: impl AsRef<Path>,
    header: &NiftiHeader,
    data: &DynArray,
    options: &WriteOptions,
) -> Result<()> {
    dispatch_dyn_array!(data, a => write(path, header, a.view(), options))
}

#[cfg(test)]
mod tests;
