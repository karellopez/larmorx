// SPDX-License-Identifier: Apache-2.0
//! Reading and writing images the way the tool sees them (`specs/mcflirt.md` §4.1, §10.1).
//!
//! On reading, every value becomes float32 (scaled by `scl_slope`/`scl_inter` when they ask
//! for it), voxel sizes come from `pixdim` (absolute values, 0 replaced by 1), and images whose
//! affine has a positive determinant have their x axis reversed in memory. Output images are
//! reversed back, so files keep their storage order.

use std::path::Path;

use larmorx_core::affine::Affine;
use larmorx_core::element::{DataType, RealElement};
use larmorx_core::linalg::det3;
use larmorx_core::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use larmorx_core::parallel;
use larmorx_core::{DynArray, dispatch_real_dyn_array};
use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, ReadOptions, Scaling, WriteOptions};
use rayon::prelude::*;

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
/// as more volumes.
pub fn read_series(path: &Path, n_threads: usize) -> Result<Series, ImageError> {
    let img = nifti::read(
        path,
        &ReadOptions {
            scaling: Scaling::Raw,
            n_threads,
        },
    )?;
    let header = img.header;
    let shape = img.data.shape().to_vec();
    if shape.is_empty() || shape.iter().product::<usize>() == 0 {
        return Err(ImageError::Invalid(format!(
            "{}: the image has no voxels",
            path.display()
        )));
    }
    let dims: [usize; 3] = [0, 1, 2].map(|k| shape.get(k).copied().unwrap_or(1));
    let stored = img.data.data_type();
    let scale = scaling(&header);
    let values = dispatch_real_dyn_array!(&img.data, a => to_f32_vec(a, scale, n_threads), complex => {
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
    let mut s = read_series(path, n_threads)?;
    let multiple = s.volumes.len() > 1;
    let first = s.volumes.swap_remove(0);
    s.volumes.clear();
    Ok((first, s, multiple))
}

/// The output types (§10.1): the stored type of the input, except where noted, and float32
/// for scaled input (float64 stays float64).
pub fn output_type(stored: DataType, header: &NiftiHeader) -> DataType {
    let slope = header.scl_slope;
    let scaled = slope != 0.0 && (slope != 1.0 || header.scl_inter != 0.0);
    match stored {
        DataType::F64 => DataType::F64,
        _ if scaled => DataType::F32,
        DataType::U8 | DataType::I8 => DataType::U8,
        DataType::I16 => DataType::I16,
        DataType::I32 | DataType::U16 => DataType::I32,
        _ => DataType::F32,
    }
}

/// The output file name for `out` (any image extension removed) with the extension of
/// `output_type` (`FSLOUTPUTTYPE`: `NIFTI_GZ`, `NIFTI`, `NIFTI2_GZ`, `NIFTI2`, `NIFTI_PAIR`,
/// `NIFTI_PAIR_GZ`, `NIFTI2_PAIR`, `NIFTI2_PAIR_GZ`). Returns the name and the NIfTI version.
pub fn output_name(out: &str, output_type: &str) -> Option<(String, NiftiVersion)> {
    let (ext, version) = match output_type {
        "NIFTI_GZ" => (".nii.gz", NiftiVersion::V1),
        "NIFTI" => (".nii", NiftiVersion::V1),
        "NIFTI2_GZ" => (".nii.gz", NiftiVersion::V2),
        "NIFTI2" => (".nii", NiftiVersion::V2),
        "NIFTI_PAIR" => (".hdr", NiftiVersion::V1),
        "NIFTI_PAIR_GZ" => (".hdr.gz", NiftiVersion::V1),
        "NIFTI2_PAIR" => (".hdr", NiftiVersion::V2),
        "NIFTI2_PAIR_GZ" => (".hdr.gz", NiftiVersion::V2),
        _ => return None,
    };
    Some((format!("{}{ext}", strip_image_extension(out)), version))
}

/// `name` without a trailing image extension (`.nii`, `.nii.gz`, `.hdr`, `.hdr.gz`, `.img`,
/// `.img.gz`).
pub fn strip_image_extension(name: &str) -> &str {
    for ext in [".nii.gz", ".hdr.gz", ".img.gz", ".nii", ".hdr", ".img"] {
        if let Some(stem) = name.strip_suffix(ext) {
            return stem;
        }
    }
    name
}

/// Converts float32 values to the output type by truncation toward zero (§9.4), saturating
/// at the type's limits (NaN becomes 0).
fn convert<T: Copy + Send + Sync>(values: &[f32], f: impl Fn(f32) -> T + Sync + Send) -> Vec<T> {
    values
        .par_iter()
        .with_min_len(1 << 16)
        .map(|&v| f(v))
        .collect()
}

/// Writes a corrected series (or any float32 image on the input's grid) like the input
/// (§10.1): the input's header with `scl_slope = 1`, `scl_inter = 0`, `cal_min`/`cal_max` from
/// the float32 data (with `cal`; else 0), the slice fields cleared, units mm and s,
/// little-endian, 3D for a single volume (the other `pixdim` entries kept); the data flipped
/// back to storage order and converted to `data_type`.
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
) -> Result<(), ImageError> {
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
    let finite = |v: f32| {
        if cal && v.is_finite() {
            f64::from(v)
        } else {
            0.0
        }
    };
    h.cal_min = finite(lo);
    h.cal_max = finite(hi);
    h.dim_info = 0;
    h.slice_code = 0;
    h.slice_start = 0;
    h.slice_end = 0;
    h.slice_duration = 0.0;
    h.xyzt_units = 2 | 8; // mm and s
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
    let array = parallel::with_threads(n_threads, || -> Result<DynArray, ImageError> {
        match data_type {
            DataType::U8 => {
                ArrayD::from_shape_vec(dims, convert(&values, |v| v as u8)).map(Into::into)
            }
            DataType::I16 => {
                ArrayD::from_shape_vec(dims, convert(&values, |v| v as i16)).map(Into::into)
            }
            DataType::I32 => {
                ArrayD::from_shape_vec(dims, convert(&values, |v| v as i32)).map(Into::into)
            }
            DataType::F64 => {
                ArrayD::from_shape_vec(dims, convert(&values, f64::from)).map(Into::into)
            }
            _ => ArrayD::from_shape_vec(dims, values).map(Into::into),
        }
        .map_err(|e| ImageError::Invalid(e.to_string()))
    })
    .map_err(|e| ImageError::Invalid(e.to_string()))??;
    // Not inside the pool above: the writer runs its own parallel compression.
    nifti::write_dyn(path, &h, &array, &options)?;
    Ok(())
}
