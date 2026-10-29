// SPDX-License-Identifier: Apache-2.0
//! Reading and writing images the way the tool sees them (`specs/mcflirt.md` §4.1, §10.1).
//!
//! On reading, every value becomes float32 (scaled by `scl_slope`/`scl_inter` when they ask
//! for it), voxel sizes come from `pixdim` (absolute values, 0 replaced by 1), and images whose
//! affine has a positive determinant have their x axis reversed in memory. Output images are
//! reversed back, so files keep their storage order.
//!
//! Two of mcflirt's output rules are defects that larmorx corrects by default (D13,
//! `docs/api/upstream-defects.md`): int8 input is written as uint8 and integer outputs are
//! truncated (FSL-09, [`output_type`], [`convert_values`]), and the units are always
//! millimetres and seconds (FSL-10, [`output_units`]).

use std::path::Path;

use larmorx_core::affine::Affine;
use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::element::{DataType, Element, RealElement};
use larmorx_core::linalg::det3;
use larmorx_core::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use larmorx_core::parallel;
use larmorx_core::{DynArray, dispatch_real_dyn_array};
use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, ReadOptions, Scaling, WriteOptions};
use rayon::prelude::*;

use super::files::{self, Compression, ImageFiles};
use super::volume::Volume;

/// Reading or writing an image failed.
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error(transparent)]
    Nifti(#[from] nifti::Error),
    #[error("{0}")]
    Invalid(String),
}

/// A series as read: its volumes in memory order and what is needed to write results like it.
#[derive(Clone, Debug)]
pub struct Series {
    pub volumes: Vec<Volume>,
    /// The header as read (nibabel's load-time fixes applied).
    pub header: NiftiHeader,
    /// Whether the x axis is reversed in memory (a positive-determinant image).
    pub flipped: bool,
    /// The stored data type.
    pub stored: DataType,
}

impl Series {
    /// The 3D shape of every volume.
    pub fn shape(&self) -> [usize; 3] {
        self.volumes.first().map_or([0; 3], |v| v.shape)
    }

    /// The voxel-to-world affine (RAS mm) of the file: the sform if its code is set, else the
    /// qform, else nibabel's default from the voxel sizes.
    pub fn affine(&self) -> Affine {
        self.header
            .best_affine()
            .unwrap_or_else(|_| Affine::from_zooms(self.voxel_size().map(f64::from), [0.0; 3]))
    }

    /// The voxel sizes from `pixdim` (absolute values, 0 replaced by 1).
    pub fn voxel_size(&self) -> [f32; 3] {
        voxel_size(&self.header)
    }
}

/// The voxel sizes of a header: `|pixdim[1..=3]|`, 0 replaced by 1, as float32.
pub fn voxel_size(h: &NiftiHeader) -> [f32; 3] {
    [1, 2, 3].map(|k| {
        let d = h.pixdim[k].abs() as f32;
        if d == 0.0 || !d.is_finite() { 1.0 } else { d }
    })
}

/// Whether an image is stored "neurologically" (§4.1): the determinant of its sform (if
/// `sform_code > 0`) or else qform (if `qform_code > 0`) is positive. Images whose sform and
/// qform determinants (both set) differ in sign, or with neither, are not.
pub fn is_neurological(h: &NiftiHeader) -> bool {
    let sdet = (h.sform_code > 0).then(|| det3(&h.sform().linear()));
    // The qform's determinant has the sign of qfac (its rotation is proper, its voxel sizes
    // positive).
    let qdet = (h.qform_code > 0).then(|| if h.pixdim[0] < 0.0 { -1.0 } else { 1.0 });
    match (sdet, qdet) {
        (Some(s), Some(q)) => s * q > 0.0 && s > 0.0,
        (Some(s), None) => s > 0.0,
        (None, Some(q)) => q > 0.0,
        (None, None) => false,
    }
}

/// The FSL scaling of a header: `Some((slope, inter))` when values must be scaled (a slope
/// that is neither 0 nor non-finite, and not the identity).
pub fn scaling(h: &NiftiHeader) -> Option<(f32, f32)> {
    let (slope, inter) = (h.scl_slope, h.scl_inter);
    if slope == 0.0 || !slope.is_finite() || (slope == 1.0 && inter == 0.0) {
        None
    } else {
        Some((
            slope as f32,
            if inter.is_finite() { inter as f32 } else { 0.0 },
        ))
    }
}

fn to_f32_vec<T: RealElement>(a: &ArrayD<T>, scale: Option<(f32, f32)>, n: usize) -> Vec<f32> {
    // The array is Fortran-ordered as read: its transposed view is contiguous, x fastest.
    let t = a.t();
    let src: Vec<T> = match t.as_slice() {
        Some(s) => s.to_vec(),
        None => t.iter().copied().collect(),
    };
    let convert = |v: &T| match scale {
        None => v.to_f32(),
        Some((s, i)) => v.to_f32() * s + i,
    };
    parallel::with_threads(n, || {
        src.par_iter().with_min_len(1 << 16).map(convert).collect()
    })
    .unwrap_or_else(|_| src.iter().map(convert).collect())
}

impl Series {
    /// A series from float32 values already scaled (x fastest, volumes last) of `dims`
    /// (`[nx, ny, nz, nt]`), placed by `header`, which decides the voxel sizes and whether the
    /// x axis is reversed in memory.
    pub fn from_values(dims: [usize; 4], values: &[f32], header: NiftiHeader) -> Series {
        let [nx, ny, nz, _] = dims;
        let per = nx * ny * nz;
        let flipped = is_neurological(&header);
        let vox = voxel_size(&header);
        let volumes = values
            .chunks_exact(per.max(1))
            .map(|c| {
                let v = Volume::new([nx, ny, nz], vox, c.to_vec());
                if flipped { v.flipped_x() } else { v }
            })
            .collect();
        Series {
            volumes,
            header,
            flipped,
            stored: DataType::F32,
        }
    }
}

/// The values of `volumes` in storage order (x fastest, volumes last), the x axis reversed
/// back if `flipped`.
pub fn storage_values(volumes: &[Volume], flipped: bool) -> Vec<f32> {
    let mut values = Vec::with_capacity(volumes.iter().map(Volume::len).sum());
    for v in volumes {
        if flipped {
            values.extend_from_slice(&v.flipped_x().data);
        } else {
            values.extend_from_slice(&v.data);
        }
    }
    values
}

/// Reads a series: a 3D image is a series of one volume; dimensions beyond the fourth count
/// as more volumes. The name is resolved as FSL 6.0.7.23 resolves it (§4.1: completed by
/// [`files::resolve_input`] when it has no image extension) and its files are read by their
/// content (gzip, zstd, bzip2 or uncompressed), whatever their names.
pub fn read_series(path: &Path, n_threads: usize) -> Result<Series, ImageError> {
    read_series_files(&input_files(path)?, n_threads)
}

/// The files of the input image `path` (§4.1): its name completed when it has no image
/// extension ([`files::resolve_input`]'s errors are FSL's words).
pub fn input_files(path: &Path) -> Result<ImageFiles, ImageError> {
    let name = files::resolve_input(&path.to_string_lossy()).map_err(ImageError::Invalid)?;
    ImageFiles::of(&name)
        .ok_or_else(|| ImageError::Invalid(format!("{name}: not an image file name")))
}

/// The header and the stored values of an image whose files are not what larmorx's NIfTI
/// reader reads (zstd or bzip2 content, `.zst` or `.bz2` names, or FSL-60's renamed header of
/// a pair): every file decompressed in memory and parsed.
fn read_decompressed(
    files: &ImageFiles,
    compression: (Compression, Option<Compression>),
) -> Result<(NiftiHeader, DynArray), ImageError> {
    let path = files.header.as_path();
    let invalid = |p: &Path, m: String| ImageError::Invalid(format!("{}: {m}", p.display()));
    let load = |p: &Path, c: Compression| {
        std::fs::read(p)
            .and_then(|raw| c.decompress(raw))
            .map_err(|e| invalid(p, e.to_string()))
    };
    let head = load(path, compression.0)?;
    let version = if head.len() >= 4 {
        NiftiHeader::sniff_version(&head[..4]).map_err(|e| invalid(path, e.to_string()))?
    } else {
        return Err(invalid(path, "the file ends inside the header".into()));
    };
    let size = match version {
        NiftiVersion::V1 => larmorx_io::nifti::header::NIFTI1_HEADER_SIZE,
        NiftiVersion::V2 => larmorx_io::nifti::header::NIFTI2_HEADER_SIZE,
    };
    if head.len() < size {
        return Err(invalid(path, "the file ends inside the header".into()));
    }
    let mut header = NiftiHeader::parse(&head[..size]).map_err(|e| invalid(path, e.to_string()))?;
    header
        .apply_load_fixes()
        .map_err(|e| invalid(path, e.to_string()))?;
    let shape = header.shape().map_err(|e| invalid(path, e.to_string()))?;
    let data_type = header
        .data_type()
        .map_err(|e| invalid(path, e.to_string()))?;
    let offset = header.vox_offset;
    if !(offset >= 0.0 && offset.is_finite()) {
        return Err(invalid(path, format!("invalid vox_offset {offset}")));
    }
    let offset = offset as usize;
    let (bytes, data_path) = match (&files.data, compression.1) {
        (Some(d), Some(c)) => (load(d, c)?, d.as_path()),
        _ => {
            if offset == 0 {
                return Err(invalid(
                    path,
                    "vox_offset is 0 in a single-file image".into(),
                ));
            }
            (head, path)
        }
    };
    let n = shape
        .iter()
        .try_fold(1usize, |a, &d| a.checked_mul(d))
        .and_then(|n| n.checked_mul(data_type.size()))
        .ok_or_else(|| invalid(path, format!("shape {shape:?} is too large")))?;
    let available = bytes.len().saturating_sub(offset);
    if available < n {
        return Err(invalid(
            data_path,
            format!("expected {n} bytes of voxel data, found {available}"),
        ));
    }
    let raw = &bytes[offset..offset + n];
    let swap = !header.byte_order.is_native();
    fn typed<T: larmorx_core::element::Element>(
        raw: &[u8],
        swap: bool,
        shape: &[usize],
    ) -> Result<DynArray, String> {
        let mut v = vec![T::default(); raw.len() / std::mem::size_of::<T>()];
        bytemuck::cast_slice_mut(&mut v).copy_from_slice(raw);
        if swap {
            v.par_iter_mut()
                .with_min_len(1 << 16)
                .for_each(|x| *x = x.swap_bytes());
        }
        ArrayD::from_shape_vec(IxDyn(shape).f(), v)
            .map(T::into_dyn_array)
            .map_err(|e| e.to_string())
    }
    let data = match data_type {
        DataType::U8 => typed::<u8>(raw, swap, &shape),
        DataType::I8 => typed::<i8>(raw, swap, &shape),
        DataType::U16 => typed::<u16>(raw, swap, &shape),
        DataType::I16 => typed::<i16>(raw, swap, &shape),
        DataType::U32 => typed::<u32>(raw, swap, &shape),
        DataType::I32 => typed::<i32>(raw, swap, &shape),
        DataType::U64 => typed::<u64>(raw, swap, &shape),
        DataType::I64 => typed::<i64>(raw, swap, &shape),
        DataType::F32 => typed::<f32>(raw, swap, &shape),
        DataType::F64 => typed::<f64>(raw, swap, &shape),
        DataType::C64 | DataType::C128 => {
            return Err(invalid(path, "complex images are not supported".into()));
        }
    }
    .map_err(|m| invalid(path, m))?;
    Ok((header, data))
}

/// Reads a series from its files ([`ImageFiles`]: a header file and, for a pair, a data
/// file), each by its content.
pub fn read_series_files(files: &ImageFiles, n_threads: usize) -> Result<Series, ImageError> {
    let path = files.header.as_path();
    let compression_of = |p: &Path| {
        Compression::of_file(p).map_err(|e| ImageError::Invalid(format!("{}: {e}", p.display())))
    };
    let compression = (
        compression_of(path)?,
        files.data.as_deref().map(compression_of).transpose()?,
    );
    let plain_or_gzip = |c: Compression| matches!(c, Compression::None | Compression::Gzip);
    let standard = files.standard_names()
        && plain_or_gzip(compression.0)
        && compression.1.is_none_or(plain_or_gzip);
    let (header, data) = if standard {
        let img = nifti::read(
            path,
            &ReadOptions {
                scaling: Scaling::Raw,
                n_threads,
            },
        )?;
        (img.header, img.data)
    } else {
        parallel::with_threads(n_threads, || read_decompressed(files, compression))
            .map_err(|e| ImageError::Invalid(e.to_string()))??
    };
    let shape = data.shape().to_vec();
    if shape.is_empty() || shape.iter().product::<usize>() == 0 {
        return Err(ImageError::Invalid(format!(
            "{}: the image has no voxels",
            path.display()
        )));
    }
    let dims: [usize; 3] = [0, 1, 2].map(|k| shape.get(k).copied().unwrap_or(1));
    let stored = data.data_type();
    let scale = scaling(&header);
    let values = dispatch_real_dyn_array!(&data, a => to_f32_vec(a, scale, n_threads), complex => {
        return Err(ImageError::Invalid(format!("{}: complex images are not supported", path.display())));
    });
    let flipped = is_neurological(&header);
    let vox = voxel_size(&header);
    let per = dims.iter().product::<usize>();
    let volumes = values
        .chunks_exact(per)
        .map(|c| {
            let v = Volume::new(dims, vox, c.to_vec());
            if flipped { v.flipped_x() } else { v }
        })
        .collect();
    Ok(Series {
        volumes,
        header,
        flipped,
        stored,
    })
}

/// Reads a reference image: the first volume of a 3D or 4D image. Returns the volume and
/// whether the file had more than one volume.
pub fn read_reference(path: &Path, n_threads: usize) -> Result<(Volume, Series, bool), ImageError> {
    read_reference_files(&input_files(path)?, n_threads)
}

/// [`read_reference`] from the image's files.
pub fn read_reference_files(
    files: &ImageFiles,
    n_threads: usize,
) -> Result<(Volume, Series, bool), ImageError> {
    let mut s = read_series_files(files, n_threads)?;
    let multiple = s.volumes.len() > 1;
    let first = s.volumes.swap_remove(0);
    s.volumes.clear();
    Ok((first, s, multiple))
}

/// The output types (§10.1): the stored type of the input, except where noted, and float32
/// for scaled input (float64 stays float64). mcflirt writes int8 input as uint8, which loses
/// its negative values (FSL-09); [`UpstreamDefects::Fix`] keeps int8.
pub fn output_type(stored: DataType, header: &NiftiHeader, defects: UpstreamDefects) -> DataType {
    let slope = header.scl_slope;
    let scaled = slope != 0.0 && (slope != 1.0 || header.scl_inter != 0.0);
    match stored {
        DataType::F64 => DataType::F64,
        _ if scaled => DataType::F32,
        DataType::I8 if defects.fix() => DataType::I8,
        DataType::U8 | DataType::I8 => DataType::U8,
        DataType::I16 => DataType::I16,
        DataType::I32 | DataType::U16 => DataType::I32,
        _ => DataType::F32,
    }
}

/// The values of `FSLOUTPUTTYPE` FSL 6.0.7.23 accepts, in the order of its message (§10.1).
pub const OUTPUT_TYPES: [&str; 16] = [
    "NIFTI",
    "NIFTI_PAIR",
    "NIFTI2",
    "NIFTI2_PAIR",
    "NIFTI_GZ",
    "NIFTI_PAIR_GZ",
    "NIFTI2_GZ",
    "NIFTI2_PAIR_GZ",
    "NIFTI_ZST",
    "NIFTI_PAIR_ZST",
    "NIFTI2_ZST",
    "NIFTI2_PAIR_ZST",
    "NIFTI_BZ2",
    "NIFTI_PAIR_BZ2",
    "NIFTI2_BZ2",
    "NIFTI2_PAIR_BZ2",
];

/// FSL 6.0.7.23's message for an `FSLOUTPUTTYPE` it does not know (§10.1, observed).
pub fn unknown_output_type_message(value: &str) -> String {
    format!(
        "ERROR:: Unrecognised value ({value}) of environment variable FSLOUTPUTTYPE\nLegal values \
         are: {}",
        OUTPUT_TYPES.join(", ")
    )
}

/// The output file name for `out` (any image extension removed) with the extension of
/// `output_type` (`FSLOUTPUTTYPE`, one of [`OUTPUT_TYPES`]: `NIFTI` `.nii`, `NIFTI_PAIR` `.hdr`
/// and `.img`, `NIFTI2` likewise in NIfTI-2, each optionally `_GZ` `.gz`, `_ZST` `.zst` or
/// `_BZ2` `.bz2`). Returns the name (the header's for a pair) and the NIfTI version.
pub fn output_name(out: &str, output_type: &str) -> Option<(String, NiftiVersion)> {
    if !OUTPUT_TYPES.contains(&output_type) {
        return None;
    }
    let (kind, suffix) = match output_type.rsplit_once('_') {
        Some((k, "GZ")) => (k, ".gz"),
        Some((k, "ZST")) => (k, ".zst"),
        Some((k, "BZ2")) => (k, ".bz2"),
        _ => (output_type, ""),
    };
    let (ext, version) = match kind {
        "NIFTI" => (".nii", NiftiVersion::V1),
        "NIFTI2" => (".nii", NiftiVersion::V2),
        "NIFTI_PAIR" => (".hdr", NiftiVersion::V1),
        "NIFTI2_PAIR" => (".hdr", NiftiVersion::V2),
        _ => return None,
    };
    Some((
        format!("{}{ext}{suffix}", strip_image_extension(out)),
        version,
    ))
}

/// `name` without a trailing image extension (`.nii`, `.hdr`, `.img`, each optionally `.gz`,
/// `.zst` or `.bz2`).
pub fn strip_image_extension(name: &str) -> &str {
    files::strip_image_extension(name)
}

fn convert<T: Copy + Send + Sync>(values: &[f32], f: impl Fn(f32) -> T + Sync + Send) -> Vec<T> {
    values
        .par_iter()
        .with_min_len(1 << 16)
        .map(|&v| f(v))
        .collect()
}

/// x86-64's conversion of a float to a 32-bit integer (`cvttss2si`): truncation toward zero,
/// and `i32::MIN` for NaN and values outside the 32-bit range (C++ leaves those undefined).
#[inline]
fn x86_i32(v: f32) -> i32 {
    if v.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&v) {
        i32::MIN
    } else {
        v as i32
    }
}

/// How integer outputs are converted, and what that did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Conversion {
    /// Values that rounding and truncation convert differently.
    pub rounded: usize,
    /// Values beyond the type's range (clamped when corrected, wrapped as x86-64 does when
    /// reproduced).
    pub out_of_range: usize,
}

/// Converts float32 values to an integer output type (§9.4, FSL-09). mcflirt truncates toward
/// zero and does not clamp: on x86-64 a value beyond the type's range wraps around (through a
/// 32-bit integer), which [`UpstreamDefects::Reproduce`] reproduces. [`UpstreamDefects::Fix`]
/// rounds to the nearest integer (half to even) and clamps to the type's range; NaN becomes 0.
/// Returns the converted values and what the two conversions disagree on.
pub fn convert_values<T>(
    values: &[f32],
    defects: UpstreamDefects,
    lo: f64,
    hi: f64,
    from_i32: impl Fn(i32) -> T + Sync + Send,
    from_f64: impl Fn(f64) -> T + Sync + Send,
) -> (Vec<T>, Conversion)
where
    T: Copy + Send + Sync,
{
    let stats = values
        .par_iter()
        .with_min_len(1 << 16)
        .map(|&v| {
            let r = f64::from(v).round_ties_even();
            let t = f64::from(v).trunc();
            let out = !(lo..=hi).contains(&t) || v.is_nan();
            (usize::from(r != t && !out), usize::from(out))
        })
        .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    let data = if defects.fix() {
        convert(values, |v| {
            if v.is_nan() {
                from_f64(0.0)
            } else {
                from_f64(f64::from(v).round_ties_even().clamp(lo, hi))
            }
        })
    } else {
        convert(values, |v| from_i32(x86_i32(v)))
    };
    (
        data,
        Conversion {
            rounded: stats.0,
            out_of_range: stats.1,
        },
    )
}

/// The `xyzt_units` of an output written like an input with `units`: mcflirt always writes
/// millimetres and seconds (FSL-10); [`UpstreamDefects::Fix`] keeps the input's spatial and
/// temporal units, and uses millimetres or seconds only where the input gives none. Also
/// returns the FSL-10 notice when the two differ.
pub fn output_units(units: i32, defects: UpstreamDefects) -> (i32, Option<Notice>) {
    let (space, time) = (units & 0x07, units & 0x38);
    let corrected = (if space == 0 { 2 } else { space }) | (if time == 0 { 8 } else { time });
    let mcflirt = 2 | 8;
    let notice = (corrected != mcflirt).then(|| {
        let name = |code: i32| match code {
            1 => "metres",
            2 => "millimetres",
            3 => "micrometres",
            8 => "seconds",
            16 => "milliseconds",
            24 => "microseconds",
            32 => "hertz",
            40 => "ppm",
            48 => "radians per second",
            _ => "unknown units",
        };
        let theirs = format!("{} and {}", name(corrected & 0x07), name(corrected & 0x38));
        let detail = match defects {
            UpstreamDefects::Fix => format!(
                "the output keeps the input's units ({theirs}); mcflirt labels them millimetres \
                 and seconds without converting pixdim"
            ),
            UpstreamDefects::Reproduce => format!(
                "the output is labelled millimetres and seconds, as mcflirt labels it, although \
                 the input's pixdim are in {theirs}"
            ),
        };
        Notice::new(&defects::FSL_10, defects, detail)
    });
    let units = if defects.fix() { corrected } else { mcflirt };
    (units, notice)
}

/// Writes a corrected series (or any float32 image on the input's grid) like the input
/// (§10.1): the input's header with `scl_slope = 1`, `scl_inter = 0`, `cal_min`/`cal_max` from
/// the float32 data (with `cal`; else the input's), the slice fields cleared, the units of
/// [`output_units`], little-endian, 3D for a single volume (the other `pixdim` entries kept);
/// the data flipped back to storage order and converted to `data_type` ([`convert_values`]).
/// Returns the upstream defects that changed the file (FSL-09, FSL-10), or would have.
///
/// `path` names the file (the header's for a pair) with any image extension of
/// [`files::image_extension`]; its compression follows the extension (§10.1).
#[allow(clippy::too_many_arguments)]
pub fn write_like(
    path: &Path,
    template: &NiftiHeader,
    version: NiftiVersion,
    flipped: bool,
    volumes: &[Volume],
    data_type: DataType,
    descrip: &str,
    cal: bool,
    n_threads: usize,
    defects: UpstreamDefects,
) -> Result<Vec<Notice>, ImageError> {
    let name = path.to_string_lossy();
    let files = ImageFiles::of(&name)
        .ok_or_else(|| ImageError::Invalid(format!("{name}: not an image file name")))?;
    write_like_files(
        &files, template, version, flipped, volumes, data_type, descrip, cal, n_threads, defects,
    )
}

/// The header block of a file written as `files` (what larmorx's NIfTI writer sets: the data
/// type, the shape, the magic string, NIfTI-2's line-ending check, `vox_offset`; the extender
/// of a single file), for [`write_like_files`]' own writer.
fn header_block(
    h: &NiftiHeader,
    data_type: DataType,
    shape: &[usize],
    single: bool,
) -> Result<Vec<u8>, ImageError> {
    let invalid = |e: larmorx_io::nifti::HeaderError| ImageError::Invalid(e.to_string());
    let mut h = h.clone();
    h.set_data_type(data_type);
    if h.shape().ok().as_deref() != Some(shape) {
        h.set_shape(shape).map_err(invalid)?;
    }
    h.magic = h.version.magic(single);
    if h.version == NiftiVersion::V2 {
        h.nifti2.eol_check = *b"\r\n\x1a\n";
    }
    h.vox_offset = if single {
        h.version.min_vox_offset() as f64
    } else {
        0.0
    };
    let mut out = h.to_bytes();
    if single {
        out.extend_from_slice(&[0; 4]);
    }
    Ok(out)
}

/// [`write_like`] to the files `files`, which may be any of FSL 6.0.7.23's names: gzip and
/// uncompressed files named as larmorx's NIfTI writer names them are written by it; zstd and
/// bzip2 files, and a pair whose header is not named after its data file (FSL-60 reproduced),
/// by this module's own writer, each file compressed as its name says.
#[allow(clippy::too_many_arguments)]
pub fn write_like_files(
    files: &ImageFiles,
    template: &NiftiHeader,
    version: NiftiVersion,
    flipped: bool,
    volumes: &[Volume],
    data_type: DataType,
    descrip: &str,
    cal: bool,
    n_threads: usize,
    defects: UpstreamDefects,
) -> Result<Vec<Notice>, ImageError> {
    let path = files.header.as_path();
    let Some(first) = volumes.first() else {
        return Err(ImageError::Invalid("no volumes to write".into()));
    };
    let shape3 = first.shape;
    let mut values = Vec::with_capacity(first.len() * volumes.len());
    for v in volumes {
        if flipped {
            values.extend_from_slice(&v.flipped_x().data);
        } else {
            values.extend_from_slice(&v.data);
        }
    }
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for &v in &values {
        if v < lo {
            lo = v;
        }
        if v > hi {
            hi = v;
        }
    }
    let mut h = template.with_version(version);
    h.byte_order = larmorx_io::nifti::ByteOrder::Little;
    h.scl_slope = 1.0;
    h.scl_inter = 0.0;
    if cal {
        let finite = |v: f32| if v.is_finite() { f64::from(v) } else { 0.0 };
        h.cal_min = finite(lo);
        h.cal_max = finite(hi);
    }
    h.dim_info = 0;
    h.slice_code = 0;
    h.slice_start = 0;
    h.slice_end = 0;
    h.slice_duration = 0.0;
    let (units, units_notice) = output_units(template.xyzt_units, defects);
    h.xyzt_units = units;
    let mut notices: Vec<Notice> = units_notice.into_iter().collect();
    h.descrip = [0; 80];
    let d = descrip.as_bytes();
    h.descrip[..d.len().min(79)].copy_from_slice(&d[..d.len().min(79)]);
    h.extensions.clear();
    // A qform quaternion whose (b, c, d) is not shorter than 1 is normalised, as the NIfTI
    // standard reads it and as mcflirt writes it back (observed on ds000122).
    let q2 = h.quatern_b * h.quatern_b + h.quatern_c * h.quatern_c + h.quatern_d * h.quatern_d;
    if 1.0 - q2 < 1e-7 && q2 > 0.0 {
        let s = 1.0 / q2.sqrt();
        h.quatern_b *= s;
        h.quatern_c *= s;
        h.quatern_d *= s;
    }
    let nt = volumes.len();
    let shape: Vec<usize> = if nt > 1 {
        vec![shape3[0], shape3[1], shape3[2], nt]
    } else {
        shape3.to_vec()
    };
    if version == NiftiVersion::V1 && shape.iter().any(|&n| n > i16::MAX as usize) {
        return Err(ImageError::Invalid(format!(
            "{}: a dimension of {shape:?} is too large for NIfTI-1",
            path.display()
        )));
    }
    // Set the dimensions here so that writing keeps the other pixdim entries (the TR).
    h.dim = [1; 8];
    h.dim[0] = shape.len() as i64;
    for (k, &n) in shape.iter().enumerate() {
        h.dim[k + 1] = n as i64;
    }
    let options = WriteOptions {
        compression_level: 2,
        n_threads,
    };
    let dims = IxDyn(&shape).f();
    let (array, conversion) = parallel::with_threads(
        n_threads,
        || -> Result<(DynArray, Conversion), ImageError> {
            let invalid = |e: larmorx_core::ndarray::ShapeError| ImageError::Invalid(e.to_string());
            Ok(match data_type {
                DataType::U8 => {
                    let (v, c) =
                        convert_values(&values, defects, 0.0, 255.0, |i| i as u8, |f| f as u8);
                    (ArrayD::from_shape_vec(dims, v).map_err(invalid)?.into(), c)
                }
                DataType::I8 => {
                    let (v, c) =
                        convert_values(&values, defects, -128.0, 127.0, |i| i as i8, |f| f as i8);
                    (ArrayD::from_shape_vec(dims, v).map_err(invalid)?.into(), c)
                }
                DataType::I16 => {
                    let (v, c) = convert_values(
                        &values,
                        defects,
                        -32768.0,
                        32767.0,
                        |i| i as i16,
                        |f| f as i16,
                    );
                    (ArrayD::from_shape_vec(dims, v).map_err(invalid)?.into(), c)
                }
                DataType::I32 => {
                    let (v, c) = convert_values(
                        &values,
                        defects,
                        -2_147_483_648.0,
                        2_147_483_647.0,
                        |i| i,
                        |f| f as i32,
                    );
                    (ArrayD::from_shape_vec(dims, v).map_err(invalid)?.into(), c)
                }
                DataType::F64 => (
                    ArrayD::from_shape_vec(dims, convert(&values, f64::from))
                        .map_err(invalid)?
                        .into(),
                    Conversion::default(),
                ),
                _ => (
                    ArrayD::from_shape_vec(dims, values)
                        .map_err(invalid)?
                        .into(),
                    Conversion::default(),
                ),
            })
        },
    )
    .map_err(|e| ImageError::Invalid(e.to_string()))??;
    if conversion != Conversion::default() {
        let what = format!(
            "{} of its values convert differently when rounded than when truncated, {} lie \
             outside the {data_type} range",
            conversion.rounded, conversion.out_of_range
        );
        let detail = match defects {
            UpstreamDefects::Fix => format!(
                "{}: integer output is rounded to the nearest and clamped; mcflirt truncates and \
                 wraps around ({what})",
                path.display()
            ),
            UpstreamDefects::Reproduce => format!(
                "{}: integer output is truncated and wraps around beyond its range, as mcflirt \
                 does on x86-64 ({what})",
                path.display()
            ),
        };
        defects::push_once(&mut notices, Notice::new(&defects::FSL_09, defects, detail));
    }
    let compression = Compression::of_name(&path.to_string_lossy());
    if files.standard_names() && matches!(compression, Compression::None | Compression::Gzip) {
        // Not inside the pool above: the writer runs its own parallel compression.
        nifti::write_dyn(path, &h, &array, &options)?;
        return Ok(notices);
    }
    // FSL 6.0.7.23's other names: the header block and the data, each file compressed as its
    // name says (little-endian data, as `h` says on every supported platform).
    let head = header_block(&h, data_type, &shape, files.data.is_none())?;
    let swap = !h.byte_order.is_native();
    let write = |p: &Path, pieces: &[&[u8]]| -> Result<(), ImageError> {
        let io = |e: std::io::Error| ImageError::Invalid(format!("{}: {e}", p.display()));
        let file = std::fs::File::create(p).map_err(io)?;
        let w = std::io::BufWriter::with_capacity(1 << 16, file);
        Compression::of_name(&p.to_string_lossy())
            .write_pieces(pieces, w)
            .map_err(io)
    };
    let write_data = |data: &[u8]| {
        parallel::with_threads(n_threads, || match &files.data {
            None => write(path, &[&head, data]),
            Some(d) => {
                write(path, &[&head])?;
                write(d, &[data])
            }
        })
        .map_err(|e| ImageError::Invalid(e.to_string()))?
    };
    // The data as stored, without a copy unless the bytes must be swapped.
    larmorx_core::dispatch_dyn_array!(&array, a => {
        let s = a.as_slice_memory_order().expect("arrays built here are contiguous");
        if swap {
            let swapped: Vec<_> = s.iter().map(|v| v.swap_bytes()).collect();
            write_data(bytemuck::cast_slice(&swapped))
        } else {
            write_data(bytemuck::cast_slice(s))
        }
    })?;
    Ok(notices)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIX: UpstreamDefects = UpstreamDefects::Fix;
    const REPRODUCE: UpstreamDefects = UpstreamDefects::Reproduce;

    /// FSL-09: integer outputs truncated and wrapped (mcflirt on x86-64), or rounded and
    /// clamped (corrected).
    #[test]
    fn integer_conversion_both_modes() {
        let values = [12.7f32, -0.5, 2.5, 3.5, 255.6, 300.2, -3.2, f32::NAN];
        let (mcflirt, c) = convert_values(&values, REPRODUCE, 0.0, 255.0, |i| i as u8, |f| f as u8);
        assert_eq!(mcflirt, [12, 0, 2, 3, 255, 44, 253, 0]);
        assert_eq!(
            c,
            Conversion {
                rounded: 3,
                out_of_range: 3
            }
        );
        let (fixed, c2) = convert_values(&values, FIX, 0.0, 255.0, |i| i as u8, |f| f as u8);
        assert_eq!(fixed, [13, 0, 2, 4, 255, 255, 0, 0]);
        assert_eq!(c, c2);
        let (i16s, _) = convert_values(
            &[32767.9f32, 32768.5, -40000.0],
            REPRODUCE,
            -32768.0,
            32767.0,
            |i| i as i16,
            |f| f as i16,
        );
        assert_eq!(i16s, [32767, -32768, 25536]);
        assert_eq!(x86_i32(3e9), i32::MIN);
        assert_eq!(x86_i32(-2.9), -2);
    }

    /// FSL 6.0.7.23's output types (§10.1): the zstd and bzip2 ones included, any image
    /// extension of `out` removed; an unknown type has none.
    #[test]
    fn output_names_of_every_type() {
        let name = |t: &str| output_name("dir/x.nii.zst", t);
        assert_eq!(
            name("NIFTI_GZ"),
            Some(("dir/x.nii.gz".into(), NiftiVersion::V1))
        );
        assert_eq!(
            name("NIFTI_ZST"),
            Some(("dir/x.nii.zst".into(), NiftiVersion::V1))
        );
        assert_eq!(
            name("NIFTI2_BZ2"),
            Some(("dir/x.nii.bz2".into(), NiftiVersion::V2))
        );
        assert_eq!(
            name("NIFTI_PAIR_ZST"),
            Some(("dir/x.hdr.zst".into(), NiftiVersion::V1))
        );
        assert_eq!(
            name("NIFTI2_PAIR"),
            Some(("dir/x.hdr".into(), NiftiVersion::V2))
        );
        assert_eq!(
            name("NIFTI2_PAIR_BZ2"),
            Some(("dir/x.hdr.bz2".into(), NiftiVersion::V2))
        );
        for t in OUTPUT_TYPES {
            assert!(name(t).is_some(), "{t}");
        }
        assert_eq!(name("NIFTI_XZ"), None);
        assert_eq!(name("nifti_gz"), None);
        assert_eq!(name(""), None);
        assert!(unknown_output_type_message("X").starts_with(
            "ERROR:: Unrecognised value (X) of environment variable FSLOUTPUTTYPE\nLegal values \
             are: NIFTI, NIFTI_PAIR, NIFTI2, NIFTI2_PAIR, NIFTI_GZ,"
        ));
    }

    /// Every output type written and read back by content, a pair whose header FSL-60 renamed
    /// included: the same header fields and values as the `.nii.gz` file.
    #[test]
    fn compressed_types_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[4, 3, 2, 2], DataType::I16).unwrap();
        h.pixdim[1..5].copy_from_slice(&[2.0, 2.0, 3.0, 1.5]);
        h.xyzt_units = 10;
        let vols: Vec<Volume> = (0..2)
            .map(|t| {
                let data = (0..24).map(|i| (i * 7 + t * 3) as f32 - 20.0).collect();
                Volume::new([4, 3, 2], [2.0, 2.0, 3.0], data)
            })
            .collect();
        let write = |files: &ImageFiles| {
            write_like_files(
                files,
                &h,
                NiftiVersion::V1,
                false,
                &vols,
                DataType::I16,
                "t",
                true,
                2,
                FIX,
            )
            .unwrap();
            read_series_files(files, 2).unwrap()
        };
        let at = |name: &str| dir.path().join(name).to_string_lossy().into_owned();
        let reference = write(&ImageFiles::of(&at("a.nii.gz")).unwrap());
        assert_eq!(reference.volumes, vols);
        for t in OUTPUT_TYPES {
            let (name, version) = output_name(&at(t), t).unwrap();
            let files = ImageFiles::of(&name).unwrap();
            write_like_files(
                &files,
                &h,
                version,
                false,
                &vols,
                DataType::I16,
                "t",
                true,
                2,
                FIX,
            )
            .unwrap();
            let back = read_series_files(&files, 2).unwrap();
            assert_eq!(back.volumes, vols, "{t}");
            assert_eq!(back.header.version, version, "{t}");
            assert_eq!(back.header.cal_max, reference.header.cal_max, "{t}");
            // And by the name alone, completed (§4.1).
            assert_eq!(
                read_series(Path::new(&at(t)), 1).unwrap().volumes,
                vols,
                "{t}"
            );
        }
        // FSL-60 reproduced: the header under another name, the data under the pair's own.
        std::fs::create_dir(dir.path().join("p.hdr.d")).unwrap();
        std::fs::create_dir(dir.path().join("p.img.d")).unwrap();
        let split = ImageFiles {
            header: dir.path().join("p.hdr.d/o.hdr.zst"),
            data: Some(dir.path().join("p.img.d/o.img.zst")),
        };
        assert_eq!(write(&split).volumes, vols);
        // A bzip2 file named .nii is read by its content.
        let bz = at("b.nii.bz2");
        write_like_files(
            &ImageFiles::of(&bz).unwrap(),
            &h,
            NiftiVersion::V1,
            false,
            &vols,
            DataType::I16,
            "t",
            true,
            1,
            FIX,
        )
        .unwrap();
        std::fs::rename(&bz, at("renamed.nii")).unwrap();
        assert_eq!(
            read_series(Path::new(&at("renamed.nii")), 1)
                .unwrap()
                .volumes,
            vols
        );
        // Two completions: FSL's error.
        std::fs::copy(at("a.nii.gz"), at("two.nii.gz")).unwrap();
        std::fs::copy(at("NIFTI_ZST.nii.zst"), at("two.nii.zst")).unwrap();
        let e = read_series(Path::new(&at("two")), 1)
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("Multiple possible filenames detected for basename"),
            "{e}"
        );
        let e = read_series(Path::new(&at("none")), 1)
            .unwrap_err()
            .to_string();
        assert!(e.contains("No image files match"), "{e}");
    }

    /// FSL-09: int8 input stays int8 when corrected; mcflirt writes uint8.
    #[test]
    fn output_types_both_modes() {
        let h = NiftiHeader::new(NiftiVersion::V1, &[2, 2, 2, 2], DataType::I8).unwrap();
        assert_eq!(output_type(DataType::I8, &h, FIX), DataType::I8);
        assert_eq!(output_type(DataType::I8, &h, REPRODUCE), DataType::U8);
        assert_eq!(output_type(DataType::I16, &h, FIX), DataType::I16);
        assert_eq!(output_type(DataType::U16, &h, FIX), DataType::I32);
    }

    /// FSL-10: units kept (corrected) or millimetres and seconds (mcflirt).
    #[test]
    fn units_both_modes() {
        // Millimetres and milliseconds.
        let (u, n) = output_units(2 | 16, FIX);
        assert_eq!(u, 2 | 16);
        assert_eq!(n.unwrap().defect.id, "FSL-10");
        let (u, n) = output_units(2 | 16, REPRODUCE);
        assert_eq!(u, 10);
        assert_eq!(n.unwrap().mode, REPRODUCE);
        // Millimetres and seconds, or none given: the same in both modes, nothing said.
        for units in [10, 0, 2, 8] {
            assert_eq!(output_units(units, FIX), (10, None), "{units}");
            assert_eq!(output_units(units, REPRODUCE), (10, None), "{units}");
        }
        // Metres.
        assert_eq!(output_units(1 | 8, FIX).0, 9);
    }
}
