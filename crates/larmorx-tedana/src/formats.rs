// SPDX-License-Identifier: Apache-2.0
//! The image files t2smap reads (`specs/t2smap.md` §4.1): NIfTI-1 and NIfTI-2 through
//! `larmorx_io`, and ANALYZE 7.5 and MGH/MGZ as nibabel 5.4.2 (MIT) loads them.
//!
//! nibabel loads a `.hdr`/`.img` pair whose header is not NIfTI's as an SPM2 ANALYZE image: the
//! shape, data type and voxel sizes of the 348-byte header (the voxel sizes 0 set to 1 and
//! negative ones to their absolute values, as its header check fixes them), the affine from the
//! voxel sizes and SPM's `origin` field (x flipped; the image centre when the origin is unset or
//! out of range), or from a MATLAB `.mat` file beside it (`mat`, else `M` with x flipped,
//! then shifted to 0-based voxels), and SPM2's scaling: `funused1` (scl_slope) with `funused2`
//! (scl_inter) when the slope is finite and not 0, else the gl/cal ranges' ratio, applied in
//! float64. An MGH image (`.mgh`, `.mgz` compressed) is big-endian, unscaled, its affine built
//! from the direction cosines, voxel sizes and centre (float32 fields), its fourth voxel size the
//! repetition time of its footer. tedana holds either as an in-memory NIfTI-1 image of the same
//! data, affine and voxel sizes. Other formats nibabel reads stay refused (decision D5).

use std::io::Read;
use std::path::{Path, PathBuf};

use larmorx_core::affine::Affine;
use larmorx_core::array::DynArray;
use larmorx_io::nifti::{self, NiftiPaths, NiftiVersion, ReadOptions, Scaling};
use ndarray::{ArrayD, IxDyn, ShapeBuilder};
use rayon::prelude::*;

use crate::log::PyError;

/// How tedana holds an image (§4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A NIfTI-1 or NIfTI-2 single file (`.nii`, `.nii.gz`), kept as it is.
    Single(NiftiVersion),
    /// A NIfTI `.hdr`/`.img` pair (converted to NIfTI-1).
    Pair,
    /// An ANALYZE 7.5 pair, loaded as SPM2's ANALYZE (converted to NIfTI-1).
    Analyze,
    /// An MGH image (converted to NIfTI-1).
    Mgh,
}

impl Kind {
    /// Whether tedana converts the image to an in-memory NIfTI-1 image (every image but a
    /// NIfTI single file).
    pub fn converted(self) -> bool {
        !matches!(self, Kind::Single(_))
    }
}

/// The stored element types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stored {
    U8,
    I16,
    U16,
    I32,
    F32,
    F64,
}

impl Stored {
    fn size(self) -> usize {
        match self {
            Stored::U8 => 1,
            Stored::I16 | Stored::U16 => 2,
            Stored::I32 | Stored::F32 => 4,
            Stored::F64 => 8,
        }
    }
}

#[derive(Clone, Debug)]
enum Source {
    Nifti(PathBuf),
    Raw {
        file: PathBuf,
        offset: usize,
        stored: Stored,
        big_endian: bool,
        /// SPM2's (slope, intercept) when they scale (not (1, 0)).
        scale: Option<(f64, f64)>,
    },
}

/// An image's header as tedana holds it: its shape, nibabel's best affine and the header's voxel
/// sizes.
#[derive(Clone, Debug)]
pub struct Head {
    pub kind: Kind,
    pub shape: Vec<usize>,
    pub affine: Affine,
    pub zooms: Vec<f64>,
    source: Source,
}

/// Decision D5: formats nibabel reads that larmorx does not, refused with a clear message.
pub fn unsupported(given: &str) -> PyError {
    PyError::value(format!(
        "{given}: larmorx's clean-room t2smap reads NIfTI-1 and NIfTI-2 images (single files and \
         .hdr/.img pairs), ANALYZE 7.5 and MGH/MGZ images; this file is not one of them"
    ))
}

fn not_found(given: &str) -> PyError {
    PyError::new(
        "FileNotFoundError",
        format!("No such file or no access: '{given}'"),
    )
}

fn nifti_error(given: &str, e: &nifti::Error) -> PyError {
    match e {
        nifti::Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            not_found(given)
        }
        nifti::Error::Header { .. } | nifti::Error::InvalidArgument(_) => unsupported(given),
        other => PyError::new("OSError", format!("{given}: {other}")),
    }
}

fn damaged(given: &str, want: usize, got: usize) -> PyError {
    PyError::new(
        "OSError",
        format!(
            "Expected {want} bytes, got {got} bytes from {given}\n - could the file be damaged?"
        ),
    )
}

/// A file's bytes, inflated when they are gzip's (`.gz`, `.mgz`).
fn file_bytes(path: &Path, given: &str) -> Result<Vec<u8>, PyError> {
    let raw = std::fs::read(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            not_found(given)
        } else {
            PyError::new("OSError", format!("{given}: {e}"))
        }
    })?;
    if raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b {
        let mut out = Vec::new();
        flate2::read::MultiGzDecoder::new(&raw[..])
            .read_to_end(&mut out)
            .map_err(|e| PyError::new("OSError", format!("{given}: {e}")))?;
        Ok(out)
    } else {
        Ok(raw)
    }
}

fn is_mgh(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mgh") || e.eq_ignore_ascii_case("mgz"))
}

/// Opens an image's header (§4.1): `given` as the user wrote it, for messages.
pub fn open(path: &Path, given: &str) -> Result<Head, PyError> {
    if is_mgh(path) {
        return open_mgh(path, given);
    }
    let Some(paths) = NiftiPaths::from_path(path) else {
        return Err(unsupported(given));
    };
    if !paths.is_single() {
        let header = paths.header_path();
        if !header.exists() || !paths.image_path().exists() {
            return Err(not_found(given));
        }
        let bytes = file_bytes(header, given)?;
        if is_analyze(&bytes) {
            return open_analyze(&paths, &bytes, given);
        }
    }
    let mut h = nifti::read_header(path).map_err(|e| nifti_error(given, &e))?;
    h.apply_load_fixes().map_err(|_| unsupported(given))?;
    let shape = h.shape().map_err(|_| unsupported(given))?;
    let affine = h.best_affine_nibabel().map_err(|_| unsupported(given))?;
    let zooms = h.zooms().map_err(|_| unsupported(given))?;
    Ok(Head {
        kind: if paths.is_single() {
            Kind::Single(h.version)
        } else {
            Kind::Pair
        },
        shape,
        affine,
        zooms,
        source: Source::Nifti(path.to_path_buf()),
    })
}

impl Head {
    /// Reads the data with nibabel's `scaling` (§4.4), in Fortran order of `shape`.
    pub fn read(
        &self,
        scaling: Scaling,
        n_threads: usize,
        given: &str,
    ) -> Result<DynArray, PyError> {
        match &self.source {
            Source::Nifti(path) => nifti::read(path, &ReadOptions { scaling, n_threads })
                .map(|img| img.data)
                .map_err(|e| nifti_error(given, &e)),
            Source::Raw {
                file,
                offset,
                stored,
                big_endian,
                scale,
            } => {
                let bytes = file_bytes(file, given)?;
                let n: usize = self.shape.iter().product();
                let want = n * stored.size();
                let got = bytes.len().saturating_sub(*offset);
                if got < want {
                    return Err(damaged(given, want, got));
                }
                let raw = &bytes[*offset..*offset + want];
                Ok(convert(
                    raw,
                    *stored,
                    *big_endian,
                    *scale,
                    scaling,
                    &self.shape,
                    n_threads,
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// ANALYZE 7.5 (SPM2's reading)

fn i16_at(b: &[u8], o: usize, big: bool) -> i16 {
    let a = [b[o], b[o + 1]];
    if big {
        i16::from_be_bytes(a)
    } else {
        i16::from_le_bytes(a)
    }
}

fn i32_at(b: &[u8], o: usize, big: bool) -> i32 {
    let a = [b[o], b[o + 1], b[o + 2], b[o + 3]];
    if big {
        i32::from_be_bytes(a)
    } else {
        i32::from_le_bytes(a)
    }
}

fn f32_at(b: &[u8], o: usize, big: bool) -> f32 {
    f32::from_bits(i32_at(b, o, big) as u32)
}

/// SPM2's test: 348 in `sizeof_hdr` in either byte order, and not NIfTI-1's magic.
fn is_analyze(b: &[u8]) -> bool {
    b.len() >= 348
        && &b[344..348] != b"ni1\0"
        && &b[344..348] != b"n+1\0"
        && (i32_at(b, 0, false) == 348 || i32_at(b, 0, true) == 348)
}

fn open_analyze(paths: &NiftiPaths, b: &[u8], given: &str) -> Result<Head, PyError> {
    // nibabel's guess of the byte order: dim[0] in 1..7 read natively (little-endian on every
    // target), or 0 with sizeof_hdr not byte-swapped 348
    let dim0 = i16_at(b, 40, false);
    let big = if dim0 == 0 {
        i32_at(b, 0, true) == 348 && i32_at(b, 0, false) != 348
    } else {
        !(1..=7).contains(&dim0)
    };
    let header_error = |m: String| PyError::new("nibabel.spatialimages.HeaderDataError", m);
    let ndim = i16_at(b, 40, big);
    if !(1..=7).contains(&ndim) {
        return Err(unsupported(given));
    }
    let dims: Vec<i16> = (0..8).map(|i| i16_at(b, 40 + 2 * i, big)).collect();
    let code = i16_at(b, 70, big);
    let stored = match code {
        2 => Stored::U8,
        4 => Stored::I16,
        8 => Stored::I32,
        16 => Stored::F32,
        64 => Stored::F64,
        0 | 1 | 255 => return Err(header_error(format!("data code {code} not supported"))),
        // complex64 and RGB: nibabel reads them; t2smap's arithmetic does not apply (D5)
        32 | 128 => return Err(unsupported(given)),
        _ => return Err(header_error(format!("data code {code} not recognized"))),
    };
    let mut pixdim: Vec<f32> = (0..8).map(|i| f32_at(b, 76 + 4 * i, big)).collect();
    // the header check's fixes: spatial voxel sizes of 0 set to 1, negative ones to |x|
    for p in &mut pixdim[1..4] {
        if *p == 0.0 {
            *p = 1.0;
        } else if *p < 0.0 {
            *p = p.abs();
        }
    }
    let n = ndim as usize;
    if dims[1..=n].iter().any(|&d| d < 0) {
        return Err(unsupported(given));
    }
    let shape: Vec<usize> = dims[1..=n].iter().map(|&d| d as usize).collect();
    let zooms: Vec<f64> = pixdim[1..=n].iter().map(|&z| f64::from(z)).collect();
    let vox_offset = f32_at(b, 108, big);
    // Python's int() of the float: truncation
    if !(vox_offset.is_finite() && vox_offset > -1.0) {
        return Err(unsupported(given));
    }
    let offset = vox_offset.trunc() as usize;
    let scale = spm2_scaling(b, big);
    let mut affine = origin_affine(&pixdim, &dims, b, big);
    if let Some(m) = mat_file(paths, given)? {
        affine = m;
    }
    Ok(Head {
        kind: Kind::Analyze,
        shape,
        affine,
        zooms,
        source: Source::Raw {
            file: paths.image_path().to_path_buf(),
            offset,
            stored,
            big_endian: big,
            scale,
        },
    })
}

/// SPM2's `get_slope_inter` (nibabel's `Spm2AnalyzeHeader`), as the array proxy applies it:
/// `None` for (1, 0).
fn spm2_scaling(b: &[u8], big: bool) -> Option<(f64, f64)> {
    let slope32 = f32_at(b, 112, big);
    let slope = f64::from(slope32);
    let (slope, inter) = if slope.is_finite() && slope != 0.0 {
        let inter = f64::from(f32_at(b, 116, big));
        (slope, if inter.is_finite() { inter } else { 0.0 })
    } else {
        let (glmax, glmin) = (i32_at(b, 140, big), i32_at(b, 144, big));
        let (cal_max, cal_min) = (f32_at(b, 124, big), f32_at(b, 128, big));
        // numpy's int32 and float32 scalar subtractions (an int32 overflow wraps)
        let unscaled = glmax.wrapping_sub(glmin);
        let scaled = cal_max - cal_min;
        // Python's truth of the two numbers: NaN is true
        if unscaled != 0 && scaled != 0.0 {
            let slope = f64::from(scaled) / f64::from(unscaled);
            (slope, f64::from(cal_min) - slope * f64::from(glmin))
        } else {
            (1.0, 0.0)
        }
    };
    if slope == 1.0 && inter == 0.0 {
        None
    } else {
        Some((slope, inter))
    }
}

/// SPM's origin affine (nibabel's `Spm99AnalyzeHeader.get_origin_affine`): voxel sizes with x
/// flipped on the diagonal; the translation −origin · sizes from the 1-based `origin` when it is
/// set and within (−dims, 2·dims) (an int16 origin times float32 sizes: a float32 product), else
/// from the centre (dims − 1) / 2 (float64).
fn origin_affine(pixdim: &[f32], dims: &[i16], b: &[u8], big: bool) -> Affine {
    let mut z = [pixdim[1], pixdim[2], pixdim[3]];
    z[0] = -z[0];
    let origin: [i16; 3] = std::array::from_fn(|i| i16_at(b, 253 + 2 * i, big));
    let d = [dims[1], dims[2], dims[3]];
    let set = origin.iter().any(|&o| o != 0)
        && (0..3).all(|i| origin[i] > d[i].wrapping_neg() && origin[i] < d[i].wrapping_mul(2));
    let t: [f64; 3] = std::array::from_fn(|i| {
        if set {
            let o = origin[i].wrapping_sub(1);
            f64::from(f32::from(o.wrapping_neg()) * z[i])
        } else {
            let o = f64::from(d[i].wrapping_sub(1)) / 2.0;
            -o * f64::from(z[i])
        }
    });
    let mut rows = *Affine::IDENTITY.rows();
    for i in 0..3 {
        rows[i][i] = f64::from(z[i]);
        rows[i][3] = t[i];
    }
    Affine::from_rows(rows)
}

/// The affine of a MATLAB file beside an ANALYZE pair (nibabel's `Spm99AnalyzeImage`): `mat`
/// (including any flip), else `M` with x flipped, then `· to_111` (0-based voxels), with numpy's
/// 4 × 4 products. An empty file is ignored.
fn mat_file(paths: &NiftiPaths, given: &str) -> Result<Option<Affine>, PyError> {
    let image = paths.image_path();
    let name = image.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let lower = name.to_ascii_lowercase();
    let (stem, gz) = if lower.ends_with(".gz") {
        (&name[..name.len() - 7], &name[name.len() - 3..])
    } else {
        (&name[..name.len() - 4], "")
    };
    // nibabel gives the sidecar the case of the image's extension
    let ext = if name[stem.len()..].starts_with(".IMG") {
        ".MAT"
    } else {
        ".mat"
    };
    let path = image.with_file_name(format!("{stem}{ext}{gz}"));
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = file_bytes(&path, given)?;
    if bytes.is_empty() {
        return Ok(None);
    }
    let vars = matfile::read(&bytes).map_err(|m| {
        PyError::value(format!(
            "{}: larmorx's clean-room t2smap reads MATLAB files of 4 × 4 numeric matrices \
             (versions 4 to 7); {m}",
            path.display()
        ))
    })?;
    let get = |key: &str| vars.iter().find(|(k, _)| k == key).map(|(_, m)| *m);
    let base = if let Some(m) = get("mat") {
        m
    } else if let Some(m) = get("M") {
        let flip = [
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        larmorx_transform::openblas::matmul4(&flip, &m)
    } else {
        return Err(PyError::value(
            "mat file found but no \"mat\" or \"M\" in it",
        ));
    };
    let mut to_111 = *Affine::IDENTITY.rows();
    for row in to_111.iter_mut().take(3) {
        row[3] = 1.0;
    }
    Ok(Some(Affine::from_rows(
        larmorx_transform::openblas::matmul4(&base, &to_111),
    )))
}

// ---------------------------------------------------------------------------------------------
// MGH / MGZ

fn open_mgh(path: &Path, given: &str) -> Result<Head, PyError> {
    let bytes = file_bytes(path, given)?;
    if bytes.len() < 90 {
        return Err(unsupported(given));
    }
    let be_i32 =
        |o: usize| i32::from_be_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    let be_f32 = |o: usize| f32::from_bits(be_i32(o) as u32);
    let dims: [i32; 4] = std::array::from_fn(|i| be_i32(4 + 4 * i));
    if dims.contains(&0) {
        return Err(PyError::new(
            "nibabel.freesurfer.mghformat.MGHError",
            "Dimensions of the data should be non-zero",
        ));
    }
    if dims.iter().any(|&d| d < 0) {
        return Err(unsupported(given));
    }
    let stored = match be_i32(20) {
        0 => Stored::U8,
        1 => Stored::I32,
        3 => Stored::F32,
        4 => Stored::I16,
        10 => Stored::U16,
        _ => return Err(unsupported(given)),
    };
    if be_i32(0) != 1 {
        return Err(PyError::new(
            "nibabel.spatialimages.HeaderDataError",
            "Unknown MGH format version",
        ));
    }
    let good = i16::from_be_bytes([bytes[28], bytes[29]]) != 0;
    let (delta, mdc, c): ([f32; 3], [[f32; 3]; 3], [f32; 3]) = if good {
        (
            std::array::from_fn(|i| be_f32(30 + 4 * i)),
            std::array::from_fn(|r| std::array::from_fn(|k| be_f32(42 + 12 * r + 4 * k))),
            std::array::from_fn(|i| be_f32(78 + 4 * i)),
        )
    } else {
        // nibabel's default when goodRASFlag is 0
        (
            [1.0; 3],
            [[-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]],
            [0.0; 3],
        )
    };
    // MdcD = Mdc.T · delta (float32); the centre (MdcD · dims) / 2 in float64 (each product of
    // a float32 and an integer is exact in float64, so numpy's order of the sum does not show
    // for grids of realistic size); the translation Pxyz_c − centre, computed in float64 and
    // stored in nibabel's `from_matvec` array, which has MdcD's type: rounded to float32 (the
    // image's affine is then that float32 matrix widened)
    let mdcd: [[f32; 3]; 3] =
        std::array::from_fn(|i| std::array::from_fn(|j| mdc[j][i] * delta[j]));
    let mut rows = *Affine::IDENTITY.rows();
    for i in 0..3 {
        let centre = (f64::from(mdcd[i][0]) * f64::from(dims[0])
            + f64::from(mdcd[i][1]) * f64::from(dims[1])
            + f64::from(mdcd[i][2]) * f64::from(dims[2]))
            / 2.0;
        for j in 0..3 {
            rows[i][j] = f64::from(mdcd[i][j]);
        }
        rows[i][3] = f64::from((f64::from(c[i]) - centre) as f32);
    }
    let n: usize = dims.iter().map(|&d| d as usize).product();
    let footer = 284 + n * stored.size();
    let tr = if bytes.len() >= footer + 4 {
        f32::from_bits(u32::from_be_bytes([
            bytes[footer],
            bytes[footer + 1],
            bytes[footer + 2],
            bytes[footer + 3],
        ]))
    } else {
        0.0
    };
    let mut shape: Vec<usize> = dims.iter().map(|&d| d as usize).collect();
    let mut zooms: Vec<f64> = delta.iter().map(|&d| f64::from(d)).collect();
    if dims[3] == 1 {
        shape.truncate(3);
    } else {
        zooms.push(f64::from(tr));
    }
    Ok(Head {
        kind: Kind::Mgh,
        shape,
        affine: Affine::from_rows(rows),
        zooms,
        source: Source::Raw {
            file: path.to_path_buf(),
            offset: 284,
            stored,
            big_endian: true,
            scale: None,
        },
    })
}

// ---------------------------------------------------------------------------------------------
// The values

fn decode<T: Send, const N: usize>(
    raw: &[u8],
    big: bool,
    n_threads: usize,
    f: impl Fn([u8; N], bool) -> T + Sync,
) -> Vec<T> {
    let chunk = (raw.len() / N).div_ceil(n_threads.max(1) * 4).max(1 << 14) * N;
    raw.par_chunks(chunk)
        .flat_map_iter(|c| {
            c.as_chunks::<N>()
                .0
                .iter()
                .map(|b| f(*b, big))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The stored values as float64 (exact for every stored type).
fn as_f64(raw: &[u8], stored: Stored, big: bool, n_threads: usize) -> Vec<f64> {
    match stored {
        Stored::U8 => raw.iter().map(|&v| f64::from(v)).collect(),
        Stored::I16 => decode::<_, 2>(raw, big, n_threads, |b, big| {
            f64::from(if big {
                i16::from_be_bytes(b)
            } else {
                i16::from_le_bytes(b)
            })
        }),
        Stored::U16 => decode::<_, 2>(raw, big, n_threads, |b, big| {
            f64::from(if big {
                u16::from_be_bytes(b)
            } else {
                u16::from_le_bytes(b)
            })
        }),
        Stored::I32 => decode::<_, 4>(raw, big, n_threads, |b, big| {
            f64::from(if big {
                i32::from_be_bytes(b)
            } else {
                i32::from_le_bytes(b)
            })
        }),
        Stored::F32 => decode::<_, 4>(raw, big, n_threads, |b, big| {
            f64::from(f32::from_bits(if big {
                u32::from_be_bytes(b)
            } else {
                u32::from_le_bytes(b)
            }))
        }),
        Stored::F64 => decode::<_, 8>(raw, big, n_threads, |b, big| {
            f64::from_bits(if big {
                u64::from_be_bytes(b)
            } else {
                u64::from_le_bytes(b)
            })
        }),
    }
}

fn fortran<T>(shape: &[usize], v: Vec<T>) -> ArrayD<T> {
    ArrayD::from_shape_vec(IxDyn(shape).f(), v).expect("the shape's size")
}

/// nibabel's array proxy for the three requests t2smap makes (§4.4): `F32` (a float32
/// request), `F64` (`get_fdata()`), `Auto` (the stored type when unscaled, float64 when
/// scaled). The scaling is `stored · slope` then `+ inter` in float64 (numpy's two operations,
/// each skipped at 1 and 0), rounded to float32 for `F32`.
fn convert(
    raw: &[u8],
    stored: Stored,
    big: bool,
    scale: Option<(f64, f64)>,
    scaling: Scaling,
    shape: &[usize],
    n_threads: usize,
) -> DynArray {
    if scale.is_none() && matches!(scaling, Scaling::Auto | Scaling::Raw) {
        let v = as_f64(raw, stored, big, n_threads);
        return match stored {
            Stored::U8 => DynArray::U8(fortran(shape, v.into_iter().map(|x| x as u8).collect())),
            Stored::I16 => DynArray::I16(fortran(shape, v.into_iter().map(|x| x as i16).collect())),
            Stored::U16 => DynArray::U16(fortran(shape, v.into_iter().map(|x| x as u16).collect())),
            Stored::I32 => DynArray::I32(fortran(shape, v.into_iter().map(|x| x as i32).collect())),
            Stored::F32 => DynArray::F32(fortran(shape, v.into_iter().map(|x| x as f32).collect())),
            Stored::F64 => DynArray::F64(fortran(shape, v)),
        };
    }
    let mut v = as_f64(raw, stored, big, n_threads);
    if let Some((slope, inter)) = scale {
        v.par_iter_mut().with_min_len(1 << 14).for_each(|x| {
            if slope != 1.0 {
                *x *= slope;
            }
            if inter != 0.0 {
                *x += inter;
            }
        });
    }
    match scaling {
        Scaling::F32 => DynArray::F32(fortran(
            shape,
            v.par_iter()
                .with_min_len(1 << 14)
                .map(|&x| x as f32)
                .collect(),
        )),
        _ => DynArray::F64(fortran(shape, v)),
    }
}

/// MATLAB files (`.mat`), as SciPy's `loadmat` reads the numeric matrices nibabel asks for:
/// version 4 (nibabel's own writing), and versions 5 to 7 (uncompressed and compressed data
/// elements). Every variable that is a real 4 × 4 numeric matrix, by name.
mod matfile {
    use std::io::Read;

    pub type Vars = Vec<(String, [[f64; 4]; 4])>;

    pub fn read(b: &[u8]) -> Result<Vars, String> {
        // SciPy's version test: a zero among the first four bytes is version 4
        if b.len() >= 4 && b[..4].contains(&0) {
            v4(b)
        } else {
            v5(b)
        }
    }

    fn matrix(name: String, rows: usize, cols: usize, data: &[f64], out: &mut Vars) {
        if rows == 4 && cols == 4 && data.len() >= 16 {
            // column-major
            let m = std::array::from_fn(|i| std::array::from_fn(|j| data[i + 4 * j]));
            out.push((name, m));
        }
    }

    fn v4(b: &[u8]) -> Result<Vars, String> {
        let mut out = Vars::new();
        let mut o = 0usize;
        while o + 20 <= b.len() {
            let le = |o: usize| i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
            let be = |o: usize| i32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
            let (mut big, mut mopt) = (false, le(o));
            if !(0..5000).contains(&mopt) {
                big = true;
                mopt = be(o);
            }
            if !(0..5000).contains(&mopt) {
                return Err("not a MATLAB 4 file".into());
            }
            let int = |o: usize| if big { be(o) } else { le(o) };
            let (rows, cols, imag, namelen) = (int(o + 4), int(o + 8), int(o + 12), int(o + 16));
            if rows < 0 || cols < 0 || namelen < 0 {
                return Err("a damaged MATLAB 4 file".into());
            }
            let p = (mopt / 10) % 10;
            let t = mopt % 10;
            let (rows, cols, namelen) = (rows as usize, cols as usize, namelen as usize);
            o += 20;
            let name_end = o + namelen;
            if name_end > b.len() {
                return Err("a damaged MATLAB 4 file".into());
            }
            let name = String::from_utf8_lossy(&b[o..name_end])
                .trim_end_matches('\0')
                .to_owned();
            o = name_end;
            let size = match p {
                0 => 8,
                1 | 2 => 4,
                3 | 4 => 2,
                5 => 1,
                _ => return Err("a MATLAB 4 element type it does not know".into()),
            };
            let n = rows * cols * if imag != 0 { 2 } else { 1 };
            let end = o + n * size;
            if end > b.len() {
                return Err("a damaged MATLAB 4 file".into());
            }
            if t == 0 && imag == 0 {
                let data: Vec<f64> = b[o..end]
                    .chunks_exact(size)
                    .map(|c| num4(c, p, big))
                    .collect();
                matrix(name, rows, cols, &data, &mut out);
            }
            o = end;
        }
        Ok(out)
    }

    fn num4(c: &[u8], p: i32, big: bool) -> f64 {
        macro_rules! get {
            ($t:ty) => {{
                let a = c.try_into().expect("size");
                if big {
                    <$t>::from_be_bytes(a)
                } else {
                    <$t>::from_le_bytes(a)
                }
            }};
        }
        match p {
            0 => get!(f64),
            1 => f64::from(get!(f32)),
            2 => f64::from(get!(i32)),
            3 => f64::from(get!(i16)),
            4 => f64::from(get!(u16)),
            _ => f64::from(c[0]),
        }
    }

    fn v5(b: &[u8]) -> Result<Vars, String> {
        if b.len() < 128 {
            return Err("not a MATLAB file".into());
        }
        let big = match &b[126..128] {
            b"IM" => false,
            b"MI" => true,
            _ => return Err("not a MATLAB 5 file".into()),
        };
        let mut out = Vars::new();
        elements(&b[128..], big, &mut out, 0)?;
        Ok(out)
    }

    fn u32_at(b: &[u8], o: usize, big: bool) -> u32 {
        let a = [b[o], b[o + 1], b[o + 2], b[o + 3]];
        if big {
            u32::from_be_bytes(a)
        } else {
            u32::from_le_bytes(a)
        }
    }

    /// One data element's (type, data, the offset after it, padded to 8 bytes).
    fn element(b: &[u8], o: usize, big: bool) -> Result<(u32, &[u8], usize), String> {
        if o + 8 > b.len() {
            return Err("a damaged MATLAB 5 file".into());
        }
        let first = u32_at(b, o, big);
        if first >> 16 != 0 {
            // the small data element: type and size in one word, the data in the next
            let (t, n) = (first & 0xffff, (first >> 16) as usize);
            if n > 4 {
                return Err("a damaged MATLAB 5 file".into());
            }
            return Ok((t, &b[o + 4..o + 4 + n], o + 8));
        }
        let n = u32_at(b, o + 4, big) as usize;
        let end = o + 8 + n;
        if end > b.len() {
            return Err("a damaged MATLAB 5 file".into());
        }
        // compressed elements are not padded
        let next = if first == 15 {
            end
        } else {
            end.div_ceil(8) * 8
        };
        Ok((first, &b[o + 8..end], next))
    }

    fn elements(b: &[u8], big: bool, out: &mut Vars, depth: usize) -> Result<(), String> {
        let mut o = 0;
        while o < b.len() {
            let (t, data, next) = element(b, o, big)?;
            match t {
                15 if depth < 2 => {
                    let mut inflated = Vec::new();
                    flate2::read::ZlibDecoder::new(data)
                        .read_to_end(&mut inflated)
                        .map_err(|e| format!("a compressed element that does not inflate: {e}"))?;
                    elements(&inflated, big, out, depth + 1)?;
                }
                14 => matrix5(data, big, out)?,
                _ => {}
            }
            o = next.min(b.len());
        }
        Ok(())
    }

    fn matrix5(b: &[u8], big: bool, out: &mut Vars) -> Result<(), String> {
        let (_, flags, o) = element(b, 0, big)?;
        if flags.len() < 4 {
            return Err("a damaged MATLAB 5 matrix".into());
        }
        let class = u32_at(flags, 0, big) & 0xff;
        let complex = u32_at(flags, 0, big) & 0x0800 != 0;
        let (_, dims, o) = element(b, o, big)?;
        let dims: Vec<usize> = dims
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32_at(c, 0, big) as usize)
            .collect();
        let (_, name, o) = element(b, o, big)?;
        let name = String::from_utf8_lossy(name).to_string();
        // numeric classes: double, single, int8 ... uint64
        if !(6..=15).contains(&class) || complex || dims.len() != 2 {
            return Ok(());
        }
        let (t, real, _) = element(b, o, big)?;
        let size = match t {
            1 | 2 => 1,
            3 | 4 => 2,
            5..=7 => 4,
            9 | 12 | 13 => 8,
            _ => return Ok(()),
        };
        let data: Vec<f64> = real.chunks_exact(size).map(|c| num5(c, t, big)).collect();
        matrix(name, dims[0], dims[1], &data, out);
        Ok(())
    }

    fn num5(c: &[u8], t: u32, big: bool) -> f64 {
        macro_rules! get {
            ($t:ty) => {{
                let a = c.try_into().expect("size");
                if big {
                    <$t>::from_be_bytes(a)
                } else {
                    <$t>::from_le_bytes(a)
                }
            }};
        }
        match t {
            1 => f64::from(c[0] as i8),
            2 => f64::from(c[0]),
            3 => f64::from(get!(i16)),
            4 => f64::from(get!(u16)),
            5 => f64::from(get!(i32)),
            6 => f64::from(get!(u32)),
            7 => f64::from(get!(f32)),
            9 => get!(f64),
            12 => get!(i64) as f64,
            _ => get!(u64) as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// An ANALYZE 7.5 header: `dims` (x, y, z, t), the type code, voxel sizes, and the fields
    /// SPM2 reads for its scaling and origin.
    #[allow(clippy::too_many_arguments)]
    fn analyze_header(
        big: bool,
        dims: [i16; 4],
        code: i16,
        pixdim: [f32; 4],
        slope: f32,
        inter: f32,
        gl: (i32, i32),
        cal: (f32, f32),
        origin: [i16; 3],
    ) -> Vec<u8> {
        let mut b = vec![0u8; 348];
        let mut put = |o: usize, v: &[u8]| {
            let mut v = v.to_vec();
            if big {
                v.reverse();
            }
            b[o..o + v.len()].copy_from_slice(&v);
        };
        put(0, &348i32.to_le_bytes());
        put(40, &4i16.to_le_bytes());
        for (i, d) in dims.iter().enumerate() {
            put(42 + 2 * i, &d.to_le_bytes());
        }
        put(70, &code.to_le_bytes());
        for (i, p) in pixdim.iter().enumerate() {
            put(80 + 4 * i, &p.to_le_bytes());
        }
        put(112, &slope.to_le_bytes());
        put(116, &inter.to_le_bytes());
        put(124, &cal.1.to_le_bytes());
        put(128, &cal.0.to_le_bytes());
        put(140, &gl.1.to_le_bytes());
        put(144, &gl.0.to_le_bytes());
        for (i, o) in origin.iter().enumerate() {
            put(253 + 2 * i, &o.to_le_bytes());
        }
        b
    }

    fn f32s(d: &DynArray) -> Vec<f32> {
        let DynArray::F32(a) = d else {
            panic!("float32")
        };
        a.as_slice_memory_order().unwrap().to_vec()
    }

    #[test]
    fn analyze_scaling_origin_and_byte_order() {
        let dir = tempfile::tempdir().unwrap();
        let q: Vec<i16> = (0..2 * 3 * 2 * 2).map(|i| (i * 37 - 200) as i16).collect();
        for big in [false, true] {
            let stem = dir.path().join(if big { "be" } else { "le" });
            let h = analyze_header(
                big,
                [2, 3, 2, 2],
                4,
                [-3.0, 0.0, 3.5, 2.0],
                0.37,
                1.3,
                (0, 0),
                (0.0, 0.0),
                [2, 3, 1],
            );
            std::fs::write(stem.with_extension("hdr"), &h).unwrap();
            let data: Vec<u8> = q
                .iter()
                .flat_map(|v| {
                    if big {
                        v.to_be_bytes()
                    } else {
                        v.to_le_bytes()
                    }
                })
                .collect();
            std::fs::write(stem.with_extension("img"), data).unwrap();
            let head = open(&stem.with_extension("img"), "x.img").unwrap();
            assert_eq!(head.kind, Kind::Analyze);
            assert_eq!(head.shape, [2, 3, 2, 2]);
            // the header check's fixes: |−3| and 0 → 1
            assert_eq!(head.zooms, [3.0, 1.0, 3.5, 2.0]);
            // the origin (2, 3, 1), 1-based: −(o − 1) · sizes, x flipped
            let rows = head.affine.rows();
            assert_eq!(rows[0][..], [-3.0, 0.0, 0.0, 3.0]);
            assert_eq!(rows[1][..], [0.0, 1.0, 0.0, -2.0]);
            assert_eq!(rows[2][..], [0.0, 0.0, 3.5, -0.0]);
            let v = f32s(&head.read(Scaling::F32, 2, "x").unwrap());
            let (s, i) = (f64::from(0.37f32), f64::from(1.3f32));
            let want: Vec<f32> = q.iter().map(|&x| (f64::from(x) * s + i) as f32).collect();
            assert_eq!(v, want);
            let DynArray::F64(a) = head.read(Scaling::Auto, 1, "x").unwrap() else {
                panic!("float64 when scaled")
            };
            assert_eq!(
                a.as_slice_memory_order().unwrap()[1],
                f64::from(q[1]) * s + i
            );
        }
        // the gl/cal ranges' scaling, and the centre when no origin is set
        let stem = dir.path().join("cal");
        let h = analyze_header(
            false,
            [2, 3, 2, 2],
            4,
            [3.0, 3.0, 3.5, 2.0],
            0.0,
            0.0,
            (-1000, 1000),
            (-50.0, 690.0),
            [0, 0, 0],
        );
        std::fs::write(stem.with_extension("hdr"), &h).unwrap();
        let data: Vec<u8> = q.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(stem.with_extension("img"), data).unwrap();
        let head = open(&stem.with_extension("hdr"), "cal.hdr").unwrap();
        assert_eq!(head.affine.rows()[0][3], 1.5);
        assert_eq!(head.affine.rows()[1][3], -3.0);
        let v = f32s(&head.read(Scaling::F32, 1, "x").unwrap());
        let slope = f64::from(740.0f32) / 2000.0;
        let inter = -50.0 - slope * -1000.0;
        assert_eq!(v[3], (f64::from(q[3]) * slope + inter) as f32);
        // unscaled: the stored type as it is
        let stem = dir.path().join("raw");
        let h = analyze_header(
            false,
            [2, 3, 2, 2],
            4,
            [3.0, 3.0, 3.5, 2.0],
            1.0,
            0.0,
            (0, 0),
            (0.0, 0.0),
            [0, 0, 0],
        );
        std::fs::write(stem.with_extension("hdr"), &h).unwrap();
        let data: Vec<u8> = q.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(stem.with_extension("img"), &data[..10]).unwrap();
        let head = open(&stem.with_extension("hdr"), "raw.hdr").unwrap();
        let err = head.read(Scaling::Auto, 1, "raw.hdr").unwrap_err();
        assert!(err.message.contains("could the file be damaged?"));
        std::fs::write(stem.with_extension("img"), &data).unwrap();
        assert!(matches!(
            head.read(Scaling::Auto, 1, "raw.hdr").unwrap(),
            DynArray::I16(_)
        ));
    }

    /// A MATLAB 4 file of one double matrix, as nibabel writes it.
    fn mat4(name: &str, m: [[f64; 4]; 4]) -> Vec<u8> {
        let mut b = Vec::new();
        for v in [0i32, 4, 4, 0, name.len() as i32 + 1] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(name.as_bytes());
        b.push(0);
        for j in 0..4 {
            for row in &m {
                b.extend_from_slice(&row[j].to_le_bytes());
            }
        }
        b
    }

    /// A MATLAB 5 file of one double matrix, its element compressed or not.
    fn mat5(name: &str, m: [[f64; 4]; 4], compressed: bool) -> Vec<u8> {
        let mut el = Vec::new();
        let tag = |t: u32, n: usize, out: &mut Vec<u8>| {
            out.extend_from_slice(&t.to_le_bytes());
            out.extend_from_slice(&(n as u32).to_le_bytes());
        };
        let mut body = Vec::new();
        tag(6, 8, &mut body);
        body.extend_from_slice(&6u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        tag(5, 8, &mut body);
        body.extend_from_slice(&4i32.to_le_bytes());
        body.extend_from_slice(&4i32.to_le_bytes());
        tag(1, name.len(), &mut body);
        body.extend_from_slice(name.as_bytes());
        body.resize(body.len().div_ceil(8) * 8, 0);
        tag(9, 128, &mut body);
        for j in 0..4 {
            for row in &m {
                body.extend_from_slice(&row[j].to_le_bytes());
            }
        }
        tag(14, body.len(), &mut el);
        el.extend_from_slice(&body);
        let mut b = vec![b' '; 116];
        b[..10].copy_from_slice(b"MATLAB 5.0");
        b.extend_from_slice(&[0; 8]);
        b.extend_from_slice(&0x0100u16.to_le_bytes());
        b.extend_from_slice(b"IM");
        if compressed {
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            z.write_all(&el).unwrap();
            let z = z.finish().unwrap();
            tag(15, z.len(), &mut b);
            b.extend_from_slice(&z);
        } else {
            b.extend_from_slice(&el);
        }
        b
    }

    #[test]
    fn analyze_mat_files() {
        let dir = tempfile::tempdir().unwrap();
        let m = [
            [-3.0, 0.0, 0.1, 24.0],
            [0.0, 3.0, 0.0, -21.0],
            [0.0, 0.0, 3.5, -13.75],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let h = analyze_header(
            false,
            [2, 3, 2, 1],
            16,
            [3.0, 3.0, 3.5, 2.0],
            0.0,
            0.0,
            (0, 0),
            (0.0, 0.0),
            [0, 0, 0],
        );
        let shifted = |m: [[f64; 4]; 4]| -> [[f64; 4]; 4] {
            std::array::from_fn(|i| {
                let mut r = m[i];
                r[3] = ((m[i][0] + m[i][1]) + m[i][2]) + m[i][3];
                r
            })
        };
        for (k, (name, bytes)) in [
            ("mat", mat4("mat", m)),
            ("M", mat4("M", m)),
            ("mat", mat5("mat", m, false)),
            ("mat", mat5("mat", m, true)),
        ]
        .into_iter()
        .enumerate()
        {
            let stem = dir.path().join(format!("m{k}"));
            std::fs::write(stem.with_extension("hdr"), &h).unwrap();
            std::fs::write(stem.with_extension("img"), vec![0u8; 4 * 12]).unwrap();
            std::fs::write(stem.with_extension("mat"), bytes).unwrap();
            let head = open(&stem.with_extension("hdr"), "m.hdr").unwrap();
            let mut want = m;
            if name == "M" {
                for x in &mut want[0] {
                    *x = -*x + 0.0;
                }
            }
            assert_eq!(*head.affine.rows(), shifted(want), "case {k}");
        }
        let stem = dir.path().join("none");
        std::fs::write(stem.with_extension("hdr"), &h).unwrap();
        std::fs::write(stem.with_extension("img"), vec![0u8; 4 * 12]).unwrap();
        std::fs::write(stem.with_extension("mat"), mat4("other", m)).unwrap();
        let err = open(&stem.with_extension("hdr"), "none.hdr").unwrap_err();
        assert_eq!(err.message, "mat file found but no \"mat\" or \"M\" in it");
    }

    fn mgh(dims: [i32; 4], kind: i32, good: bool, values: &[f32], tr: Option<f32>) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&1i32.to_be_bytes());
        for d in dims {
            b.extend_from_slice(&d.to_be_bytes());
        }
        b.extend_from_slice(&kind.to_be_bytes());
        b.extend_from_slice(&0i32.to_be_bytes());
        b.extend_from_slice(&i16::from(good).to_be_bytes());
        for v in [2.0f32, 3.0, 3.5] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        // the transposed direction cosines: columns x = (−1, 0, 0), y = (0, 0, 1)... rotated
        let c = 0.8f32;
        let s = 0.6f32;
        for v in [-c, s, 0.0, -s, -c, 0.0, 0.0, 0.0, 1.0f32] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        for v in [1.25f32, -2.5, 7.0] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.resize(284, 0);
        for v in values {
            b.extend_from_slice(&v.to_be_bytes());
        }
        if let Some(tr) = tr {
            b.extend_from_slice(&tr.to_be_bytes());
        }
        b
    }

    #[test]
    fn mgh_affine_zooms_and_frames() {
        let dir = tempfile::tempdir().unwrap();
        let values: Vec<f32> = (0..3 * 2 * 2 * 2).map(|i| i as f32 * 1.5).collect();
        let p = dir.path().join("a.mgh");
        std::fs::write(&p, mgh([3, 2, 2, 2], 3, true, &values, Some(2000.0))).unwrap();
        let head = open(&p, "a.mgh").unwrap();
        assert_eq!(
            (head.kind, head.shape.clone()),
            (Kind::Mgh, vec![3, 2, 2, 2])
        );
        assert_eq!(head.zooms, [2.0, 3.0, 3.5, 2000.0]);
        let rows = head.affine.rows();
        let mdcd = [
            [-0.8f32 * 2.0, -0.6 * 3.0, 0.0],
            [0.6f32 * 2.0, -0.8 * 3.0, 0.0],
            [0.0, 0.0, 3.5f32],
        ];
        for i in 0..3 {
            let centre = (f64::from(mdcd[i][0]) * 3.0
                + f64::from(mdcd[i][1]) * 2.0
                + f64::from(mdcd[i][2]) * 2.0)
                / 2.0;
            let c = [1.25f32, -2.5, 7.0][i];
            // nibabel's from_matvec array is MdcD's float32: the translation rounded to it
            assert_eq!(rows[i][3], f64::from((f64::from(c) - centre) as f32));
            if i == 0 {
                assert_ne!(rows[i][3], f64::from(c) - centre);
            }
            for j in 0..3 {
                assert_eq!(rows[i][j], f64::from(mdcd[i][j]));
            }
        }
        assert_eq!(f32s(&head.read(Scaling::F32, 1, "a").unwrap()), values);
        // one frame: three axes and three voxel sizes; no footer
        let one: Vec<f32> = values[..12].to_vec();
        let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(&mgh([3, 2, 2, 1], 3, false, &one, None))
            .unwrap();
        let p = dir.path().join("b.mgz");
        std::fs::write(&p, z.finish().unwrap()).unwrap();
        let head = open(&p, "b.mgz").unwrap();
        assert_eq!(head.shape, [3, 2, 2]);
        // goodRASFlag 0: nibabel's default direction cosines, unit sizes, centre 0
        assert_eq!(head.zooms, [1.0, 1.0, 1.0]);
        assert_eq!(head.affine.rows()[1][..], [0.0, 0.0, -1.0, 1.0]);
        let p = dir.path().join("c.mgh");
        std::fs::write(&p, mgh([3, 0, 2, 1], 3, true, &[], None)).unwrap();
        assert_eq!(
            open(&p, "c.mgh").unwrap_err().message,
            "Dimensions of the data should be non-zero"
        );
    }
}
