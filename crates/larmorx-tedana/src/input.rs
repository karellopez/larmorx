// SPDX-License-Identifier: Apache-2.0
//! Reading the echoes, the reference and the mask (`specs/t2smap.md` §4).
//!
//! The masked data are held voxel-major, `x[(v * E + e) * T + t]` (numpy's `(S, E, T)` with the
//! volumes innermost), filled one echo at a time: one echo's grid is in memory at once.

use std::path::{Path, PathBuf};

use larmorx_core::affine::Affine;
use larmorx_core::array::DynArray;
use larmorx_interp::nilearn::{self, EpiData, Layout, Maths};
use larmorx_io::nifti::{self, NiftiHeader, NiftiPaths, NiftiVersion, ReadOptions, Scaling};
use rayon::prelude::*;

use crate::log::{Log, PyError};

/// A path as given, against the run's working directory.
pub fn resolve(cwd: Option<&Path>, p: &str) -> PathBuf {
    let path = Path::new(p);
    match cwd {
        Some(d) if path.is_relative() => d.join(path),
        _ => path.to_path_buf(),
    }
}

/// An image file's kind, as nibabel would load it (§4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// A NIfTI-1 or NIfTI-2 single file (`.nii`, `.nii.gz`).
    Single(NiftiVersion),
    /// A NIfTI `.hdr`/`.img` pair (converted to NIfTI-1 by tedana).
    Pair,
}

fn kind(path: &Path, given: &str) -> Result<Kind, PyError> {
    let Some(paths) = NiftiPaths::from_path(path) else {
        return Err(unsupported(given));
    };
    let header = nifti::read_header(path).map_err(|e| read_error(given, &e))?;
    Ok(if paths.is_single() {
        Kind::Single(header.version)
    } else {
        Kind::Pair
    })
}

/// Decision D5: formats nibabel reads that larmorx does not (and, in this relay of the
/// clean-room original, ANALYZE 7.5 and MGH/MGZ), refused with a clear message.
fn unsupported(given: &str) -> PyError {
    PyError::value(format!(
        "{given}: larmorx's clean-room t2smap reads NIfTI-1 and NIfTI-2 images (single files \
         and .hdr/.img pairs); this file is not one of them"
    ))
}

fn read_error(given: &str, e: &nifti::Error) -> PyError {
    match e {
        nifti::Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            PyError::new(
                "FileNotFoundError",
                format!("No such file or no access: '{given}'"),
            )
        }
        nifti::Error::Header { .. } | nifti::Error::InvalidArgument(_) => unsupported(given),
        other => PyError::new("OSError", format!("{given}: {other}")),
    }
}

/// The reference image (§4.2): the first echo's image, or a z-concatenated file's first slab.
#[derive(Clone, Debug)]
pub struct Reference {
    pub header: NiftiHeader,
    pub shape: Vec<usize>,
    pub affine: Affine,
    /// The four voxel sizes the 4D outputs carry (§11.3).
    pub zooms: [f64; 4],
    /// The outputs' NIfTI version without a mask (§11.3): NIfTI-2 for a NIfTI-2 single file.
    pub version: NiftiVersion,
}

/// Reads the reference's header (step 8). `n_zcat`: the echo count of a z-concatenated file.
pub fn reference(
    cwd: Option<&Path>,
    given: &str,
    n_zcat: Option<usize>,
) -> Result<Reference, PyError> {
    let path = resolve(cwd, given);
    let k = kind(&path, given)?;
    let header = nifti::read_header(&path).map_err(|e| read_error(given, &e))?;
    let mut h = header.clone();
    h.apply_load_fixes().map_err(|_| unsupported(given))?;
    let shape = h.shape().map_err(|_| unsupported(given))?;
    let affine = h.best_affine_nibabel().map_err(|_| unsupported(given))?;
    let z = h.zooms().map_err(|_| unsupported(given))?;
    let mut zooms = [1.0f64; 4];
    for (i, zz) in zooms.iter_mut().enumerate() {
        if let Some(&v) = z.get(i) {
            *zz = v;
        }
    }
    let converted = n_zcat.is_some() || k == Kind::Pair;
    if converted {
        // a new NIfTI-1 image holds its voxel sizes as float32
        for zz in &mut zooms {
            *zz = *zz as f32 as f64;
        }
    }
    let version = match k {
        Kind::Single(v) if n_zcat.is_none() => v,
        _ => NiftiVersion::V1,
    };
    let mut shape = shape;
    if let Some(e) = n_zcat
        && shape.len() >= 3
    {
        shape[2] /= e.max(1);
    }
    Ok(Reference {
        header: h,
        shape,
        affine,
        zooms,
        version,
    })
}

/// The analysis mask on the grid (§4.3).
#[derive(Clone, Debug)]
pub struct Mask {
    /// `true` inside, in C order of `shape` (x slowest, z fastest).
    pub inside: Vec<bool>,
    /// The mask's shape as tedana holds it (a NIfTI single file's (X, Y, Z, 1) kept).
    pub shape: Vec<usize>,
    pub affine: Affine,
}

impl Mask {
    /// The grid's three sizes.
    pub fn grid(&self) -> [usize; 3] {
        [self.shape[0], self.shape[1], self.shape[2]]
    }

    /// Whether tedana reads the echoes through nilearn's masking with this mask (§4.4: a mask
    /// of four axes, a single volume).
    pub fn four_d(&self) -> bool {
        self.shape.len() == 4
    }

    /// The mask's voxels as offsets into a Fortran-ordered grid volume (x fastest), in C order.
    pub fn offsets(&self) -> Vec<usize> {
        let [nx, ny, nz] = self.grid();
        let mut out = Vec::new();
        for x in 0..nx {
            for y in 0..ny {
                for z in 0..nz {
                    if self.inside[(x * ny + y) * nz + z] {
                        out.push(x + nx * (y + ny * z));
                    }
                }
            }
        }
        out
    }
}

/// The user mask (`--mask`): value > 0 in float64 with the header's scaling (§4.3).
pub fn user_mask(cwd: Option<&Path>, given: &str) -> Result<Mask, PyError> {
    let path = resolve(cwd, given);
    let k = kind(&path, given)?;
    let img = nifti::read(
        &path,
        &ReadOptions {
            scaling: Scaling::F64,
            n_threads: 1,
        },
    )
    .map_err(|e| read_error(given, &e))?;
    let affine = img
        .header
        .best_affine_nibabel()
        .map_err(|_| unsupported(given))?;
    let mut shape = img.data.shape().to_vec();
    if k == Kind::Pair {
        while shape.len() > 3 && shape.last() == Some(&1) {
            shape.pop();
        }
        if shape.len() > 3 {
            return Err(PyError::value(format!(
                "{given} has more than 3 dimensions"
            )));
        }
    }
    let DynArray::F64(a) = &img.data else {
        unreachable!("Scaling::F64 reads float64");
    };
    let f = a.as_slice_memory_order().expect("contiguous");
    if shape.len() < 3 || (shape.len() == 4 && shape[3] != 1) || shape.len() > 4 {
        return Err(PyError::value(format!(
            "{given}: larmorx's clean-room t2smap takes a mask of three axes (or four with one \
             volume); this one has the shape {}",
            py_tuple(&shape)
        )));
    }
    let [nx, ny, nz] = [shape[0], shape[1], shape[2]];
    let mut inside = vec![false; nx * ny * nz];
    for x in 0..nx {
        for y in 0..ny {
            for z in 0..nz {
                inside[(x * ny + y) * nz + z] = f[x + nx * (y + ny * z)] > 0.0;
            }
        }
    }
    Ok(Mask {
        inside,
        shape,
        affine,
    })
}

/// nilearn's `compute_epi_mask` of the reference (§4.3), and whether it came out empty (its
/// `MaskWarning`).
pub fn computed_mask(
    cwd: Option<&Path>,
    given: &str,
    reference: &Reference,
    n_zcat: Option<usize>,
    maths: Maths,
    n_threads: usize,
) -> Result<(Mask, bool), PyError> {
    let path = resolve(cwd, given);
    // the mean over time of the data as nibabel holds them: the stored values when unscaled,
    // float64 when scaled (and for a z-concatenated reference)
    let img = nifti::read(
        &path,
        &ReadOptions {
            scaling: Scaling::Auto,
            n_threads,
        },
    )
    .map_err(|e| read_error(given, &e))?;
    let shape = img.data.shape().to_vec();
    let result = match (n_zcat, img.data) {
        (Some(e), data) => {
            // the first slab's float64 values in C order (`get_fdata()[:, :, :nz, :]`)
            let f64data = to_f64(&data);
            let (nx, ny, z) = (shape[0], shape[1], shape[2]);
            let nt = shape.get(3).copied().unwrap_or(1);
            let nz = z / e.max(1);
            let mut c = vec![0.0f64; nx * ny * nz * nt];
            for x in 0..nx {
                for y in 0..ny {
                    for zz in 0..nz {
                        for t in 0..nt {
                            c[((x * ny + y) * nz + zz) * nt + t] =
                                f64data[x + nx * (y + ny * (zz + z * t))];
                        }
                    }
                }
            }
            let s = if shape.len() == 4 {
                vec![nx, ny, nz, nt]
            } else {
                vec![nx, ny, nz]
            };
            nilearn::compute_epi_mask(EpiData::F64(&c), &s, Layout::C, maths)
        }
        (None, DynArray::F32(a)) => {
            let f = a.as_slice_memory_order().expect("contiguous");
            nilearn::compute_epi_mask(EpiData::F32(f), &shape, Layout::Fortran, maths)
        }
        (None, data) => {
            let f = to_f64(&data);
            nilearn::compute_epi_mask(EpiData::F64(&f), &shape, Layout::Fortran, maths)
        }
    };
    let m = match result {
        Ok(m) => m,
        Err(e) => {
            let (class, message) = e.python();
            return Err(PyError::new(class, message));
        }
    };
    let s3 = vec![reference.shape[0], reference.shape[1], reference.shape[2]];
    // nilearn's mask is Fortran-ordered; the mask's voxels are taken in C order
    let (nx, ny, nz) = (s3[0], s3[1], s3[2]);
    let mut inside = vec![false; nx * ny * nz];
    for x in 0..nx {
        for y in 0..ny {
            for z in 0..nz {
                inside[(x * ny + y) * nz + z] = m.mask[x + nx * (y + ny * z)];
            }
        }
    }
    Ok((
        Mask {
            inside,
            shape: s3,
            affine: reference.affine,
        },
        m.empty,
    ))
}

fn to_f64(data: &DynArray) -> Vec<f64> {
    use larmorx_core::element::RealElement;
    fn conv<T: RealElement>(a: &ndarray::ArrayD<T>) -> Vec<f64> {
        a.as_slice_memory_order()
            .expect("contiguous")
            .iter()
            .map(|v| v.to_f64())
            .collect()
    }
    match data {
        DynArray::U8(a) => conv(a),
        DynArray::I8(a) => conv(a),
        DynArray::U16(a) => conv(a),
        DynArray::I16(a) => conv(a),
        DynArray::U32(a) => conv(a),
        DynArray::I32(a) => conv(a),
        DynArray::U64(a) => conv(a),
        DynArray::I64(a) => conv(a),
        DynArray::F32(a) => conv(a),
        DynArray::F64(a) => conv(a),
        // complex data: nilearn's mean is complex; t2smap's path never meets it (D5)
        DynArray::C64(_) | DynArray::C128(_) => Vec::new(),
    }
}

/// Python's `str` of a tuple of integers.
pub fn py_tuple(v: &[usize]) -> String {
    match v.len() {
        1 => format!("({},)", v[0]),
        _ => format!(
            "({})",
            v.iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// The masked data of every echo (§4.4).
#[derive(Clone, Debug)]
pub struct Data {
    /// `x[(v * n_echo + e) * n_t + t]`, float32.
    pub x: Vec<f32>,
    pub n_vox: usize,
    pub n_echo: usize,
    pub n_t: usize,
}

impl Data {
    /// Voxel `v`'s echo `e` series.
    #[inline]
    pub fn series(&self, v: usize, e: usize) -> &[f32] {
        let o = (v * self.n_echo + e) * self.n_t;
        &self.x[o..o + self.n_t]
    }
}

fn read_f32(path: &Path, given: &str, n_threads: usize) -> Result<nifti::NiftiImage, PyError> {
    nifti::read(
        path,
        &ReadOptions {
            scaling: Scaling::F32,
            n_threads,
        },
    )
    .map_err(|e| read_error(given, &e))
}

fn f32_slice(img: &nifti::NiftiImage) -> &[f32] {
    let DynArray::F32(a) = &img.data else {
        unreachable!("Scaling::F32 reads float32");
    };
    a.as_slice_memory_order().expect("contiguous")
}

/// 3D echo files: tedana's two chained errors (§4.4).
fn not_4d() -> PyError {
    PyError::value("Expected 4D image")
}

/// tedana's direct read failed (`first`); nilearn's masking then failed too (`e`).
fn after_direct(e: PyError, first: PyError) -> PyError {
    e.during(first)
}

/// Gathers one echo's volume series of the mask's voxels into `x` (slot `e` of `n_echo`).
#[allow(clippy::too_many_arguments)]
fn gather(
    x: &mut [f32],
    src: &[f32],
    offsets: &[usize],
    nvox_grid: usize,
    e: usize,
    n_echo: usize,
    n_t: usize,
    finite_only: bool,
) {
    const CHUNK: usize = 256;
    let row = n_echo * n_t;
    if row == 0 {
        return;
    }
    x.par_chunks_mut(CHUNK * row)
        .enumerate()
        .for_each(|(c, block)| {
            let vox = &offsets[c * CHUNK..(c * CHUNK + block.len() / row)];
            for t in 0..n_t {
                let vol = &src[t * nvox_grid..(t + 1) * nvox_grid];
                for (j, &f) in vox.iter().enumerate() {
                    let v = vol[f];
                    block[(j * n_echo + e) * n_t + t] = if finite_only && !v.is_finite() {
                        0.0
                    } else {
                        v
                    };
                }
            }
        });
}

/// Reads the echoes (step 11): `files` the `-d` values, `n_te` the echo times' count, the
/// mask; with one file, the z-concatenated form. Returns the data and whether the
/// z-concatenation warning is due.
pub fn read_data(
    cwd: Option<&Path>,
    files: &[String],
    n_te: usize,
    mask: &Mask,
    n_threads: usize,
    log: &mut Log<'_>,
) -> Result<Data, PyError> {
    let offsets = mask.offsets();
    let n_vox = offsets.len();
    if files.len() == 1 {
        log.warning(
            "io:load_data_nilearn:1104",
            "DEPRECATION WARNING: We are planning to deprecate supplying a single z-concatenated \
             data file at the end of 2026. If you are using this option, please comment on \
             https://github.com/ME-ICA/tedana/issues/1313.",
        );
        let path = resolve(cwd, &files[0]);
        let img = read_f32(&path, &files[0], n_threads)?;
        let shape = img.data.shape().to_vec();
        if shape.len() != 4 {
            return Err(not_4d());
        }
        let (nx, ny, z, nt) = (shape[0], shape[1], shape[2], shape[3]);
        let n_echo = n_te;
        let nz = z / n_echo.max(1);
        if mask.four_d() || mask.grid() != [nx, ny, nz] {
            return Err(PyError::value("Z-cat echo slice/mask shape mismatch"));
        }
        let src = f32_slice(&img);
        let mut x = vec![0.0f32; n_vox * n_echo * nt];
        // the slab of echo e: slices e·nz .. (e+1)·nz − 1 of every volume
        let slab = nx * ny * nz;
        let mut buf = vec![0.0f32; slab * nt];
        for e in 0..n_echo {
            for t in 0..nt {
                let from = t * nx * ny * z + e * slab;
                buf[t * slab..(t + 1) * slab].copy_from_slice(&src[from..from + slab]);
            }
            gather(&mut x, &buf, &offsets, slab, e, n_echo, nt, false);
        }
        return Ok(Data {
            x,
            n_vox,
            n_echo,
            n_t: nt,
        });
    }
    let n_echo = files.len();
    let nilearn_path = mask.four_d()
        || files.iter().any(|f| {
            nifti::read_header(resolve(cwd, f))
                .ok()
                .and_then(|h| h.shape().ok())
                .is_some_and(|s| s.len() >= 3 && s[..3] != mask.grid())
        });
    let mut x: Vec<f32> = Vec::new();
    let mut n_t = 0;
    // the files are decoded side by side, `n_threads` at a time (inflating one file is
    // sequential), then checked and gathered in echo order: the first echo that fails is the
    // one reported, as when they are read one after the other (tedana holds every echo at once,
    // so the batch stays within its memory)
    let batch = n_threads.clamp(1, n_echo.max(1));
    let inner = (n_threads / batch).max(1);
    let mut decoded: Vec<Result<nifti::NiftiImage, PyError>> = Vec::new();
    for (e, given) in files.iter().enumerate() {
        if e % batch == 0 {
            let names = &files[e..(e + batch).min(n_echo)];
            decoded = names
                .par_iter()
                .map(|g| read_f32(&resolve(cwd, g), g, inner))
                .collect();
            decoded.reverse();
        }
        let img = decoded.pop().expect("one decoded file per echo")?;
        let shape = img.data.shape().to_vec();
        if nilearn_path {
            let affine = img
                .header
                .best_affine_nibabel()
                .map_err(|_| unsupported(given))?;
            let first = || PyError::value("Image/mask shape mismatch");
            if !mask.affine.allclose(&affine, 1e-5, 1e-8) {
                return Err(after_direct(
                    nilearn_affine_error(&mask.affine, &affine),
                    first(),
                ));
            }
            if shape.len() < 3 || shape[..3] != mask.grid() {
                return Err(after_direct(
                    PyError::value(format!(
                        "Mask shape: {}\n is different from img shape:{}",
                        py_tuple(&mask.grid()),
                        py_tuple(&shape[..3.min(shape.len())])
                    )),
                    first(),
                ));
            }
        }
        if shape.len() != 4 {
            // tedana's direct read, then nilearn's, each refuse a 3D file
            return Err(after_direct(not_4d(), not_4d()));
        }
        let nt = shape[3];
        if e == 0 {
            n_t = nt;
            x = vec![0.0f32; n_vox * n_echo * nt];
        } else if nt != n_t {
            return Err(PyError::value(
                "all input array dimensions except for the concatenation axis must match exactly",
            ));
        }
        let grid = shape[0] * shape[1] * shape[2];
        gather(
            &mut x,
            f32_slice(&img),
            &offsets,
            grid,
            e,
            n_echo,
            nt,
            nilearn_path,
        );
    }
    Ok(Data {
        x,
        n_vox,
        n_echo,
        n_t,
    })
}

/// nilearn's error for a mask whose affine is not close to the image's (§4.4, TED-04).
pub fn nilearn_affine_error(mask: &Affine, img: &Affine) -> PyError {
    PyError::value(format!(
        "Mask affine:\n{}\n is different from img affine:\n{}",
        affine_str(mask),
        affine_str(img)
    ))
}

/// numpy's `str` of a 4 × 4 float64 array.
pub fn affine_str(a: &Affine) -> String {
    let rows = a.rows();
    let flat: Vec<f64> = rows.iter().flat_map(|r| r.iter().copied()).collect();
    larmorx_core::numpy::format::array_str_2d(&flat, 4)
}
