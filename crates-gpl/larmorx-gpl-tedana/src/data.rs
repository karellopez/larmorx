// SPDX-License-Identifier: GPL-3.0-only
// Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA Developers; GNU LGPL 2.1), under the
// GNU GPL version 3 as section 3 of the LGPL 2.1 allows; modified 2026 by Karel Lopez Vilaret.
//! The images t2smap reads: the reference image (`tedana.io.load_ref_img`), the user's mask
//! (`tedana.utils.load_mask`) and the masked multi-echo data (`tedana.io.load_data_nilearn`:
//! the fast path of boolean indexing, and nilearn's `apply_mask` when it fails), with nibabel's
//! classes and scaling.
//!
//! nibabel reads NIfTI-1 single files as `Nifti1Image`, which tedana uses as they are; NIfTI-1
//! pairs and NIfTI-2 images it converts (`_convert_to_nifti1`: the data, the affine and the
//! zooms into a new `Nifti1Image`, squeezed to three dimensions for a mask). The values are the
//! same either way: float32 from the stored type, through float64 where the header scales them.
//! Other formats nibabel reads (ANALYZE, MGH, AFNI, MINC, PAR/REC, compressed with bzip2 or
//! zstd) are refused with a clear message (a documented gap, decision D5).
//!
//! Masked voxels are taken in numpy's order of a boolean index on the first three axes: the
//! first axis slowest. `Masked::data` is the `(voxels, echoes, volumes)` C-ordered float32
//! array tedana calls `data_cat`.

use std::path::{Path, PathBuf};

use larmorx_core::numpy::format::array_str_2d;
use larmorx_core::{Affine, DynArray};
use larmorx_io::nifti::{self, NiftiHeader, NiftiPaths, ReadOptions, Scaling};
use rayon::prelude::*;

use crate::log::PyError;

/// The nibabel class a file loads as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageClass {
    Nifti1Image,
    Nifti1Pair,
    Nifti2Image,
    Nifti2Pair,
}

/// An image header as nibabel loads it, before the data.
#[derive(Clone, Debug)]
pub struct Header {
    pub path: PathBuf,
    pub class: ImageClass,
    pub header: NiftiHeader,
    pub shape: Vec<usize>,
    /// nibabel's `img.affine`.
    pub affine: Affine,
}

fn image_file_error(path: &str) -> PyError {
    PyError::new(
        "nibabel.filebasedimages.ImageFileError",
        format!("Cannot work out file type of \"{path}\""),
        "nibabel/loadsave.py, in load",
    )
}

fn io_error(e: impl std::fmt::Display) -> PyError {
    PyError::new("OSError", e.to_string(), "nibabel/loadsave.py, in load")
}

/// `nibabel.load(path)`'s class and header.
pub fn load_header(path: &str) -> Result<Header, PyError> {
    let lower = path.to_ascii_lowercase();
    let stem = lower.strip_suffix(".gz").unwrap_or(&lower).to_owned();
    for (ext, what) in [
        (".bz2", "bzip2-compressed"),
        (".zst", "zstd-compressed"),
        (".mgh", "MGH"),
        (".mgz", "MGH"),
        (".head", "AFNI"),
        (".brik", "AFNI"),
        (".mnc", "MINC"),
        (".par", "PAR/REC"),
        (".rec", "PAR/REC"),
    ] {
        if lower.ends_with(ext) || stem.ends_with(ext) {
            return Err(PyError::new(
                "larmorx.UnsupportedFormat",
                format!(
                    "{path}: larmorx-gpl reads NIfTI-1 and NIfTI-2 images only; tedana reads \
                     {what} images through nibabel (a documented gap)"
                ),
                "tedana/io.py, in load_ref_img",
            ));
        }
    }
    let single = stem.ends_with(".nii");
    let pair = stem.ends_with(".hdr") || stem.ends_with(".img");
    if !single && !pair {
        return Err(image_file_error(path));
    }
    let p = Path::new(path);
    let header = nifti::read_header(p).map_err(|e| match e {
        nifti::Error::Header { .. } => image_file_error(path),
        other => io_error(other),
    })?;
    let magic = &header.magic[..3];
    let class = match (magic, single) {
        (b"n+1" | b"ni1", true) => ImageClass::Nifti1Image,
        (b"n+2" | b"ni2", true) => ImageClass::Nifti2Image,
        (b"ni1" | b"n+1", false) => ImageClass::Nifti1Pair,
        (b"ni2" | b"n+2", false) => ImageClass::Nifti2Pair,
        (_, true) => return Err(image_file_error(path)),
        (_, false) => {
            return Err(PyError::new(
                "larmorx.UnsupportedFormat",
                format!(
                    "{path}: larmorx-gpl reads NIfTI-1 and NIfTI-2 images only; tedana reads \
                     ANALYZE images through nibabel (a documented gap)"
                ),
                "tedana/io.py, in load_ref_img",
            ));
        }
    };
    // nibabel's load-time fixes are applied when the data are read; the shape and affine are
    // those of the fixed header.
    let mut fixed = header.clone();
    fixed.apply_load_fixes().map_err(io_error)?;
    let shape = fixed.shape().map_err(io_error)?;
    // nibabel's `img.affine`, bit for bit (its qform in x87 extended precision).
    let affine = fixed.best_affine_nibabel().map_err(io_error)?;
    Ok(Header {
        path: p.to_path_buf(),
        class,
        header: fixed,
        shape,
        affine,
    })
}

/// Reads the voxels of `h` with `scaling` (nibabel's `np.asarray(img.dataobj, dtype)`).
fn read_data(h: &Header, scaling: Scaling, n_threads: usize) -> Result<DynArray, PyError> {
    nifti::read(&h.path, &ReadOptions { scaling, n_threads })
        .map(|img| img.data)
        .map_err(io_error)
}

fn as_f32(a: DynArray) -> Vec<f32> {
    match a {
        DynArray::F32(a) => a.into_raw_vec_and_offset().0,
        _ => unreachable!("read as float32"),
    }
}

fn as_f64(a: DynArray) -> Vec<f64> {
    match a {
        DynArray::F64(a) => a.into_raw_vec_and_offset().0,
        _ => unreachable!("read as float64"),
    }
}

/// The reference image (`load_ref_img`): the first echo, or the first `n_z` slices of a
/// z-concatenated file.
#[derive(Clone, Debug)]
pub struct RefImage {
    pub header: Header,
    /// The shape of the reference (for z-concatenated data, the first slab's).
    pub shape: Vec<usize>,
    /// `ref_img.header.get_zooms()`.
    pub zooms: Vec<f64>,
}

/// `load_ref_img(data, n_echos)`.
pub fn load_ref(data: &[String], n_echos: usize) -> Result<RefImage, PyError> {
    let h = load_header(&data[0])?;
    let zooms = h.header.zooms().map_err(io_error)?;
    let mut shape = h.shape.clone();
    if data.len() == 1 {
        // `data_img.shape[2] // n_echos`, then `slicer[:, :, :n_z, :]`.
        if shape.len() < 3 {
            return Err(PyError::new(
                "IndexError",
                "tuple index out of range",
                "tedana/io.py, in load_ref_img",
            ));
        }
        if shape.len() < 4 {
            return Err(PyError::new(
                "IndexError",
                "Too many indices",
                "nibabel/arrayproxy.py, in _get_scaled",
            ));
        }
        shape[2] /= n_echos;
    }
    Ok(RefImage {
        header: h,
        shape,
        zooms,
    })
}

/// The user's mask (`load_mask(ref_img, mask=mask)`): `get_fdata() > 0` of the file, in its own
/// shape (Fortran order), squeezed to three dimensions for an image nibabel does not load as a
/// `Nifti1Image`.
#[derive(Clone, Debug)]
pub struct UserMask {
    pub header: Header,
    pub shape: Vec<usize>,
    /// Fortran-ordered, over `shape`.
    pub inside: Vec<bool>,
}

/// `load_mask(ref_img, mask=path)` (the user-defined branch).
pub fn load_user_mask(path: &str, n_threads: usize) -> Result<UserMask, PyError> {
    let h = load_header(path)?;
    let mut shape = h.shape.clone();
    if h.class != ImageClass::Nifti1Image && shape.len() > 3 {
        // `nb.funcs.squeeze_image`: trailing axes of length 1 beyond the third.
        while shape.len() > 3 && *shape.last().expect("non-empty") == 1 {
            shape.pop();
        }
        if shape.len() > 3 {
            let name = NiftiPaths::from_path(&h.path)
                .map(|p| p.image_path().display().to_string())
                .unwrap_or_else(|| path.to_owned());
            return Err(PyError::value_error(
                format!("{name} has more than 3 dimensions"),
                "tedana/io.py, in _convert_to_nifti1",
            ));
        }
    }
    let values = as_f64(read_data(&h, Scaling::F64, n_threads)?);
    let inside = values.par_iter().map(|&v| v > 0.0).collect();
    Ok(UserMask {
        header: h,
        shape,
        inside,
    })
}

/// The multi-echo data inside the mask.
#[derive(Clone, Debug)]
pub struct Masked {
    /// The spatial shape the mask indexes.
    pub spatial: [usize; 3],
    /// Each masked voxel's Fortran-order index in `spatial`, in numpy's order.
    pub voxels: Vec<usize>,
    pub n_echos: usize,
    pub n_vols: usize,
    /// `data_cat`: `(voxels, echoes, volumes)`, C-ordered.
    pub data: Vec<f32>,
}

/// The masked voxels of a Fortran-ordered mask over `shape`, in numpy's C order.
pub fn masked_voxels(inside: &[bool], shape: [usize; 3]) -> Vec<usize> {
    let [nx, ny, nz] = shape;
    let mut out = Vec::new();
    for x in 0..nx {
        for y in 0..ny {
            for z in 0..nz {
                let f = x + nx * (y + ny * z);
                if inside[f] {
                    out.push(f);
                }
            }
        }
    }
    out
}

/// `(voxels, volumes)` of one Fortran-ordered 4D float32 array of spatial size `nxyz`.
fn gather(arr: &[f32], nxyz: usize, voxels: &[usize], n_vols: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; voxels.len() * n_vols];
    out.par_chunks_mut(n_vols.max(1))
        .zip(voxels.par_iter())
        .for_each(|(row, &v)| {
            for (t, slot) in row.iter_mut().enumerate() {
                *slot = arr[v + nxyz * t];
            }
        });
    out
}

fn stack(per_echo: Vec<Vec<f32>>, n_voxels: usize, n_vols: usize) -> Vec<f32> {
    let e = per_echo.len();
    let mut out = vec![0.0f32; n_voxels * e * n_vols];
    out.par_chunks_mut((e * n_vols).max(1))
        .enumerate()
        .for_each(|(v, block)| {
            for (k, echo) in per_echo.iter().enumerate() {
                block[k * n_vols..(k + 1) * n_vols]
                    .copy_from_slice(&echo[v * n_vols..(v + 1) * n_vols]);
            }
        });
    out
}

fn tuple_str(s: &[usize]) -> String {
    match s.len() {
        1 => format!("({},)", s[0]),
        _ => format!(
            "({})",
            s.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `numpy.allclose(a, b)`.
fn allclose(a: &Affine, b: &Affine) -> bool {
    a.rows()
        .iter()
        .flatten()
        .zip(b.rows().iter().flatten())
        .all(|(&x, &y)| (x - y).abs() <= 1e-8 + 1e-5 * y.abs())
}

fn affine_str(a: &Affine) -> String {
    let v: Vec<f64> = a.rows().iter().flatten().copied().collect();
    array_str_2d(&v, 4)
}

/// How the data were read: the fast path, or nilearn's (non-finite values set to zero).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadPath {
    Fast,
    Nilearn,
    ZCat,
}

/// `load_data_nilearn(data, mask_img, n_echos)`. The mask is the user's (`mask_shape` its
/// shape, `inside` its Fortran-ordered voxels, `mask_affine` its affine).
pub fn load_data(
    data: &[String],
    mask: &UserMask,
    n_echos: usize,
    n_threads: usize,
) -> Result<(Masked, LoadPath), PyError> {
    if data.len() == 1 {
        return load_zcat(&data[0], mask, n_echos, n_threads).map(|m| (m, LoadPath::ZCat));
    }
    match load_fast(data, mask, n_threads) {
        Ok(m) => Ok((m, LoadPath::Fast)),
        Err(_) => load_nilearn(data, mask, n_threads).map(|m| (m, LoadPath::Nilearn)),
    }
}

fn spatial3(shape: &[usize]) -> Option<[usize; 3]> {
    (shape.len() == 3).then(|| [shape[0], shape[1], shape[2]])
}

fn load_fast(data: &[String], mask: &UserMask, n_threads: usize) -> Result<Masked, PyError> {
    let mut per_echo = Vec::with_capacity(data.len());
    let mut n_vols = None;
    let mut voxels = Vec::new();
    let mut spatial = [0; 3];
    for f in data {
        let h = load_header(f)?;
        if h.shape.len() < 3 || h.shape[..3] != mask.shape[..] {
            return Err(PyError::value_error(
                "Image/mask shape mismatch",
                "_mask_4d",
            ));
        }
        if h.shape.len() != 4 {
            return Err(PyError::value_error("Expected 4D image", "_mask_4d"));
        }
        spatial = spatial3(&mask.shape).expect("matches a 4D image's first three axes");
        if voxels.is_empty() {
            voxels = masked_voxels(&mask.inside, spatial);
        }
        let t = h.shape[3];
        if n_vols.is_some_and(|n| n != t) {
            return Err(PyError::value_error(
                "all input arrays must have the same shape",
                "numpy stack",
            ));
        }
        n_vols = Some(t);
        let arr = as_f32(read_data(&h, Scaling::F32, n_threads)?);
        per_echo.push(gather(&arr, spatial.iter().product(), &voxels, t));
    }
    let n_vols = n_vols.unwrap_or(0);
    let n_voxels = voxels.len();
    Ok(Masked {
        spatial,
        voxels,
        n_echos: data.len(),
        n_vols,
        data: stack(per_echo, n_voxels, n_vols),
    })
}

/// nilearn's `masking.apply_mask(img, mask_img)` per echo (the fallback).
fn load_nilearn(data: &[String], mask: &UserMask, n_threads: usize) -> Result<Masked, PyError> {
    // Every echo is loaded before the first is masked.
    let headers = data
        .iter()
        .map(|f| load_header(f))
        .collect::<Result<Vec<_>, _>>()?;
    // check_niimg_3d: a 4D mask of one volume is squeezed; other shapes are refused.
    let mut mshape = mask.shape.clone();
    if mshape.len() == 4 && mshape[3] == 1 {
        mshape.pop();
    }
    if mshape.len() != 3 {
        return Err(PyError::new(
            "nilearn.exceptions.DimensionError",
            format!(
                "Input data has incompatible dimensionality: Expected dimension is 3D and you \
                 provided a {}D image. See \
                 https://nilearn.github.io/stable/manipulating_images/input_output.html.",
                mshape.len()
            ),
            "nilearn/image/image.py, in check_niimg",
        ));
    }
    let spatial = spatial3(&mshape).expect("three axes");
    // load_mask_img: an empty mask is refused.
    if !mask.inside.iter().any(|&b| b) {
        return Err(PyError::value_error(
            "The mask is invalid as it is empty: it masks all data.",
            "nilearn/masking.py, in load_mask_img",
        ));
    }
    let voxels = masked_voxels(&mask.inside, spatial);
    let mut per_echo = Vec::with_capacity(data.len());
    let mut n_vols = None;
    for h in headers {
        if !allclose(&mask.header.affine, &h.affine) {
            return Err(PyError::value_error(
                format!(
                    "Mask affine:\n{}\n is different from img affine:\n{}",
                    affine_str(&mask.header.affine),
                    affine_str(&h.affine)
                ),
                "nilearn/masking.py, in apply_mask_fmri",
            ));
        }
        let img3: Vec<usize> = h.shape.iter().take(3).copied().collect();
        if img3 != mshape {
            return Err(PyError::value_error(
                format!(
                    "Mask shape: {}\n is different from img shape:{}",
                    tuple_str(&mshape),
                    tuple_str(&img3)
                ),
                "nilearn/masking.py, in apply_mask_fmri",
            ));
        }
        if h.shape.len() != 4 {
            return Err(PyError::value_error(
                "Expected 4D image",
                "tedana/io.py, in load_data_nilearn",
            ));
        }
        let t = h.shape[3];
        if n_vols.is_some_and(|n| n != t) {
            return Err(PyError::value_error(
                "all input arrays must have the same shape",
                "numpy/_core/shape_base.py, in stack",
            ));
        }
        n_vols = Some(t);
        // smooth_array(..., ensure_finite=True): non-finite values become 0.
        let mut arr = as_f32(read_data(&h, Scaling::F32, n_threads)?);
        arr.par_iter_mut().for_each(|v| {
            if !v.is_finite() {
                *v = 0.0;
            }
        });
        per_echo.push(gather(&arr, spatial.iter().product(), &voxels, t));
    }
    let n_vols = n_vols.unwrap_or(0);
    let n_voxels = voxels.len();
    Ok(Masked {
        spatial,
        voxels,
        n_echos: data.len(),
        n_vols,
        data: stack(per_echo, n_voxels, n_vols),
    })
}

/// The z-concatenated branch: one file, echo `e` in slices `[e·n_z, (e+1)·n_z)`.
fn load_zcat(
    path: &str,
    mask: &UserMask,
    n_echos: usize,
    n_threads: usize,
) -> Result<Masked, PyError> {
    let h = load_header(path)?;
    if h.shape.len() != 4 {
        return Err(PyError::value_error(
            "Expected 4D z-concatenated image",
            "tedana/io.py, in load_data_nilearn",
        ));
    }
    let [nx, ny, nzt, nt] = [h.shape[0], h.shape[1], h.shape[2], h.shape[3]];
    let n_z = nzt / n_echos;
    if mask.shape != [nx, ny, n_z] {
        return Err(PyError::value_error(
            "Z-cat echo slice/mask shape mismatch",
            "tedana/io.py, in load_data_nilearn",
        ));
    }
    let spatial = [nx, ny, n_z];
    let voxels = masked_voxels(&mask.inside, spatial);
    let arr = as_f32(read_data(&h, Scaling::F32, n_threads)?);
    let full = nx * ny * nzt;
    let per_echo: Vec<Vec<f32>> = (0..n_echos)
        .map(|e| {
            // The slab's Fortran index (x, y, z) is x + nx·(y + ny·z); in the file, z + e·n_z.
            let shifted: Vec<usize> = voxels.iter().map(|&v| v + nx * ny * e * n_z).collect();
            gather(&arr, full, &shifted, nt)
        })
        .collect();
    let n_voxels = voxels.len();
    Ok(Masked {
        spatial,
        voxels,
        n_echos,
        n_vols: nt,
        data: stack(per_echo, n_voxels, nt),
    })
}

/// `load_mask(ref_img)` without a mask: nilearn's `compute_epi_mask(ref_img)` on the reference
/// as tedana holds it (the first echo's `np.asanyarray(dataobj)`, Fortran-ordered: float32, or
/// float64 for integers and scaled data; for z-concatenated data the first slab's
/// `get_fdata()`, float64 and C-ordered), as a mask over the reference's first three axes with
/// the reference's affine. `maths` selects numpy's own float64 `exp` and `log` for the
/// smoothing kernel (or correctly rounded ones); while its witness looks, the mask is also
/// computed with the other choice, and a different mask is noted (`Family::ExpLog`). Also
/// returns nilearn's empty-mask warning flag.
pub fn compute_mask(
    data: &[String],
    reference: &RefImage,
    choice: &crate::maths::Maths,
    n_threads: usize,
) -> Result<(UserMask, bool), PyError> {
    use crate::maths::Family;
    use larmorx_core::math;
    use larmorx_core::numpy::umath;
    use larmorx_interp::nilearn::{
        EpiData, EpiMask, EpiMaskError, Layout, Maths, compute_epi_mask,
    };

    let h = &reference.header;
    let pick = |numpy: bool| {
        if numpy {
            Maths {
                exp: umath::exp_f64,
                log: umath::log_f64,
            }
        } else {
            Maths {
                exp: math::exp,
                log: math::log,
            }
        }
    };
    let both = choice.watching(Family::ExpLog);
    let numpy = choice.numpy_explog();
    // The mask with the chosen functions, and with the other while the witness looks.
    let run = |f: &dyn Fn(Maths) -> Result<EpiMask, EpiMaskError>| {
        let m = f(pick(numpy));
        if both {
            let o = f(pick(!numpy));
            let same = match (&m, &o) {
                (Ok(a), Ok(b)) => a.mask == b.mask && a.empty == b.empty,
                (Err(a), Err(b)) => a.python() == b.python(),
                _ => false,
            };
            choice.note(Family::ExpLog, !same);
        }
        m
    };
    let place = "nilearn/masking.py, in compute_epi_mask";
    let fail = |e: larmorx_interp::nilearn::EpiMaskError| {
        let (class, msg) = e.python();
        PyError::new(class, msg, place)
    };
    let (mask, shape) = if data.len() == 1 {
        // z-concatenated: get_fdata() of the first slab is C-ordered.
        let arr = as_f64(read_data(h, Scaling::F64, n_threads)?);
        let s = &h.shape;
        let (nx, ny, nz, nt) = (s[0], s[1], s[2], s[3]);
        let nzs = reference.shape[2];
        let mut c = Vec::with_capacity(nx * ny * nzs * nt);
        for x in 0..nx {
            for y in 0..ny {
                for z in 0..nzs {
                    for t in 0..nt {
                        c.push(arr[x + nx * (y + ny * (z + nz * t))]);
                    }
                }
            }
        }
        let shape = [nx, ny, nzs, nt];
        (
            run(&|m| compute_epi_mask(EpiData::F64(&c), &shape, Layout::C, m)).map_err(fail)?,
            shape.to_vec(),
        )
    } else {
        let a = read_data(h, Scaling::Auto, n_threads)?;
        let shape = a.shape().to_vec();
        let m = match a {
            DynArray::F32(a) => {
                let v: Vec<f32> = a.t().iter().copied().collect();
                run(&|m| compute_epi_mask(EpiData::F32(&v), &shape, Layout::Fortran, m))
            }
            DynArray::C64(_) | DynArray::C128(_) => {
                return Err(PyError::new(
                    "TypeError",
                    "complex data are not supported by compute_epi_mask",
                    place,
                ));
            }
            other => {
                fn f64s<T: larmorx_core::RealElement>(
                    a: &larmorx_core::ndarray::ArrayD<T>,
                ) -> Vec<f64> {
                    a.t().iter().map(|&x| x.to_f64()).collect()
                }
                let v = match &other {
                    DynArray::F64(a) => f64s(a),
                    DynArray::U8(a) => f64s(a),
                    DynArray::I8(a) => f64s(a),
                    DynArray::U16(a) => f64s(a),
                    DynArray::I16(a) => f64s(a),
                    DynArray::U32(a) => f64s(a),
                    DynArray::I32(a) => f64s(a),
                    DynArray::U64(a) => f64s(a),
                    DynArray::I64(a) => f64s(a),
                    _ => unreachable!("float32 and complex handled above"),
                };
                run(&|m| compute_epi_mask(EpiData::F64(&v), &shape, Layout::Fortran, m))
            }
        }
        .map_err(fail)?;
        (m, shape)
    };
    let empty = mask.empty;
    let mut header = h.clone();
    header.shape = shape[..3].to_vec();
    Ok((
        UserMask {
            header,
            shape: shape[..3].to_vec(),
            inside: mask.mask,
        },
        empty,
    ))
}
