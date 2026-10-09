//! How ITK 5.4.5 (and so ANTs) places a NIfTI-1 image in physical space.
//!
//! ITK reads NIfTI through nifti_clib (`nifti_convert_nhdr2nim`) and then chooses between the
//! qform and the sform with its own rules (`NiftiImageIO::SetImageIOOrientationFromNIfTI`),
//! which differ from nibabel's. The result is ITK's geometry: origin, spacing and direction
//! cosines in **LPS** coordinates. ANTs tools work in that space, so larmorx must read headers
//! the same way to reproduce them. Ported from ITK v5.4.5 (Apache-2.0) and its bundled
//! nifti_clib (public domain); see `PROVENANCE.md`. Where ITK computes in single precision,
//! this port does too, so borderline choices come out the same.

use std::path::Path;

use larmorx_core::array::DynArray;
use larmorx_core::element::DataType;
use larmorx_core::element::RealElement;
use larmorx_core::{dispatch_real_dyn_array, linalg};
use rayon::prelude::*;

use super::header::{NiftiHeader, NiftiVersion, xform};
use super::{Error, ReadOptions, Scaling};

/// The geometry ITK gives an image read from a NIfTI file.
#[derive(Clone, Debug, PartialEq)]
pub struct ItkGeometry {
    /// Number of dimensions of the ITK image (trailing singleton dimensions beyond the third
    /// are dropped, as ITK does for scalar images).
    pub ndim: usize,
    pub size: Vec<usize>,
    /// Spacing in mm (and seconds for the fourth dimension). Always positive: like ITK's image
    /// reader, a negative `pixdim` flips the direction column instead.
    pub spacing: Vec<f64>,
    /// Origin in LPS mm (the fourth component is `toffset` in seconds).
    pub origin: Vec<f64>,
    /// Direction cosines, `ndim × ndim`, column `j` being the direction of index axis `j` in
    /// LPS. Identity beyond the third dimension.
    pub direction: Vec<Vec<f64>>,
    /// Which header transform ITK used.
    pub source: GeometrySource,
}

/// Which part of the header ITK took the geometry from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometrySource {
    Qform,
    Sform,
    /// Neither code set: origin 0, identity direction.
    Default,
}

/// ITK cannot read the image.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ItkGeometryError {
    #[error("ITK 5.4 reads only NIfTI-1 and Analyze headers, not NIfTI-2")]
    Nifti2,
    #[error("bad datatype {0}")]
    BadDatatype(i16),
    #[error("bad dim[1] = {0}")]
    BadDim1(i64),
    #[error("dim[0] = {0} is not a valid number of dimensions")]
    BadNdim(i64),
    #[error("NIFTI_INTENT_GENMATRIX images are not supported by ITK")]
    GenMatrix,
    #[error("ITK only supports orthonormal direction cosines; no orthonormal definition found")]
    NoOrthonormalTransform,
}

type M44 = [[f32; 4]; 4];

const INTENT_SYMMATRIX: i32 = 1005;
const INTENT_DISPVECT: i32 = 1006;
const INTENT_VECTOR: i32 = 1007;
const INTENT_GENMATRIX: i32 = 1004;

/// nifti_clib's `NIFTI_VERSION`: whether the magic string marks a NIfTI header.
fn is_nifti(magic: &[u8; 4]) -> bool {
    magic[0] == b'n'
        && magic[3] == 0
        && (magic[1] == b'i' || magic[1] == b'+')
        && (b'1'..=b'9').contains(&magic[2])
}

/// nifti_clib's `nifti_quatern_to_mat44`: the qform matrix, computed in double and stored in
/// float. A negative voxel size is replaced by 1 here (but stays negative as ITK's spacing).
fn quatern_to_mat44(h: &NiftiHeader, dx: f32, dy: f32, dz: f32, qfac: f32) -> M44 {
    let (mut b, mut c, mut d) = (
        f64::from(h.quatern_b as f32),
        f64::from(h.quatern_c as f32),
        f64::from(h.quatern_d as f32),
    );
    let mut a = 1.0 - (b * b + c * c + d * d);
    if a < 1.0e-7 {
        let n = 1.0 / (b * b + c * c + d * d).sqrt();
        b *= n;
        c *= n;
        d *= n;
        a = 0.0;
    } else {
        a = a.sqrt();
    }
    let xd = if dx > 0.0 { f64::from(dx) } else { 1.0 };
    let yd = if dy > 0.0 { f64::from(dy) } else { 1.0 };
    let mut zd = if dz > 0.0 { f64::from(dz) } else { 1.0 };
    if qfac < 0.0 {
        zd = -zd;
    }
    let f = |v: f64| v as f32;
    [
        [
            f((a * a + b * b - c * c - d * d) * xd),
            f(2.0 * (b * c - a * d) * yd),
            f(2.0 * (b * d + a * c) * zd),
            h.qoffset_x as f32,
        ],
        [
            f(2.0 * (b * c + a * d) * xd),
            f((a * a + c * c - b * b - d * d) * yd),
            f(2.0 * (c * d - a * b) * zd),
            h.qoffset_y as f32,
        ],
        [
            f(2.0 * (b * d - a * c) * xd),
            f(2.0 * (c * d + a * b) * yd),
            f((a * a + d * d - c * c - b * b) * zd),
            h.qoffset_z as f32,
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn top3(m: &M44) -> [[f32; 3]; 3] {
    [0, 1, 2].map(|i| [m[i][0], m[i][1], m[i][2]])
}

fn to_f64<const N: usize>(m: &[[f32; N]; N]) -> [[f64; N]; N] {
    m.map(|row| row.map(f64::from))
}

/// ITK's `IsAffine`: the bottom row is (0, 0, 0, 1) and the matrix is well conditioned.
fn is_affine(m: &M44) -> bool {
    let bottom =
        (f64::from(m[3][3]) - 1.0).abs() + (0..3).map(|i| f64::from(m[3][i]).abs()).sum::<f64>();
    if bottom > f64::from(f32::EPSILON) {
        return false;
    }
    let (sv, _) = linalg::svd_u(&to_f64(m));
    let condition = if sv[0] > 0.0 { sv[3] / sv[0] } else { 0.0 };
    condition > f64::EPSILON
}

/// Whether the 3×3 part of the sform has orthonormal columns once each is normalised (float
/// arithmetic, tolerance 1e-4, as ITK checks it).
fn sform_is_orthonormal(s: &M44) -> bool {
    let mut r = top3(s);
    for j in 0..3 {
        let norm = (r[0][j] * r[0][j] + r[1][j] * r[1][j] + r[2][j] * r[2][j]).sqrt();
        for row in &mut r {
            row[j] /= norm;
        }
    }
    (0..3).all(|i| {
        (0..3).all(|j| {
            let v: f32 = (0..3).map(|k| r[i][k] * r[j][k]).sum();
            (v - if i == j { 1.0 } else { 0.0 }).abs() <= 1.0e-4
        })
    })
}

/// ITK's first test: the qform and sform matrices are (almost) element-wise equal.
fn qform_sform_are_similar(s: &M44, q: &M44) -> bool {
    let rot_ok = (0..3).all(|i| (0..3).all(|j| f64::from((s[i][j] - q[i][j]).abs()) <= 1.0e-5));
    let trans: f32 = (0..4).map(|i| (s[i][3] - q[i][3]).abs()).sum();
    let bottom: f32 = (0..4).map(|j| (s[3][j] - q[3][j]).abs()).sum();
    rot_ok && f64::from(trans) <= 1.0e-7 && f64::from(bottom) <= 1.0e-7
}

/// ITK's second test: same singular values, rotations, offsets and bottom rows (1e-4).
fn sform_qform_very_similar(s: &M44, q: &M44) -> bool {
    let (sw, su) = linalg::svd_u(&to_f64(&top3(s)));
    let (qw, qu) = linalg::svd_u(&to_f64(&top3(q)));
    let spacing_similar = (0..3).all(|i| (sw[i] - qw[i]).abs() <= 1.0e-4);
    let Some(qu_inv) = linalg::inverse3(&qu) else {
        return false;
    };
    let candidate = linalg::mul3(&su, &qu_inv);
    let rotation_similar = (0..3).all(|i| {
        (0..3).all(|j| (candidate[i][j] - if i == j { 1.0 } else { 0.0 }).abs() <= 1.0e-4)
    });
    let offsets_similar = (0..3).all(|i| f64::from((s[i][3] - q[i][3]).abs()) <= 1.0e-4);
    let perspective_similar = (0..4).all(|j| f64::from((s[3][j] - q[3][j]).abs()) <= 1.0e-4);
    rotation_similar && offsets_similar && perspective_similar && spacing_similar
}

fn normalized(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n > 0.0 { v.map(|x| x / n) } else { v }
}

/// The geometry ITK 5.4.5 reads from `header` (a header as stored, before nibabel's fixes).
pub fn itk_geometry(header: &NiftiHeader) -> Result<ItkGeometry, ItkGeometryError> {
    if header.version == NiftiVersion::V2 {
        return Err(ItkGeometryError::Nifti2);
    }
    // nifti_clib knows these codes, and ITK has a pixel type for each (float128 and complex256
    // are known to nifti_clib but not to ITK; binary and unknown codes to neither).
    const ITK_DATATYPES: [i16; 14] = [
        2, 4, 8, 16, 32, 64, 128, 256, 512, 768, 1024, 1280, 1792, 2304,
    ];
    if !ITK_DATATYPES.contains(&header.datatype) {
        return Err(ItkGeometryError::BadDatatype(header.datatype));
    }
    let ndim_hdr = usize::try_from(header.dim[0])
        .ok()
        .filter(|n| (1..=7).contains(n))
        .ok_or(ItkGeometryError::BadNdim(header.dim[0]))?;
    if header.dim[1] <= 0 {
        return Err(ItkGeometryError::BadDim1(header.dim[1]));
    }
    // nifti_clib: fix bad dims and grid spacings within the defined range.
    let mut dim = header.dim;
    for d in dim.iter_mut().take(ndim_hdr + 1).skip(2) {
        if *d <= 0 {
            *d = 1;
        }
    }
    for d in dim.iter_mut().skip(ndim_hdr + 1) {
        if *d != 0 && *d != 1 {
            *d = 1;
        }
    }
    let mut pixdim: [f32; 8] = header.pixdim.map(|p| p as f32);
    for p in pixdim.iter_mut().take(ndim_hdr + 1).skip(1) {
        if *p == 0.0 || !p.is_finite() {
            *p = 1.0;
        }
    }
    let nifti = is_nifti(&header.magic);
    let (dx, dy, dz) = (pixdim[1], pixdim[2], pixdim[3]);
    let qform_code = if nifti && header.qform_code > 0 {
        header.qform_code
    } else {
        0
    };
    let sform_code = if nifti && header.sform_code > 0 {
        header.sform_code
    } else {
        0
    };
    let qto: M44 = if qform_code == 0 {
        [
            [dx, 0.0, 0.0, 0.0],
            [0.0, dy, 0.0, 0.0],
            [0.0, 0.0, dz, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    } else {
        let qfac = if pixdim[0] < 0.0 { -1.0 } else { 1.0 };
        quatern_to_mat44(header, dx, dy, dz, qfac)
    };
    let sto: M44 = if sform_code == 0 {
        [[0.0; 4]; 4]
    } else {
        let r = |row: &[f64; 4]| row.map(|v| v as f32);
        [
            r(&header.srow_x),
            r(&header.srow_y),
            r(&header.srow_z),
            [0.0, 0.0, 0.0, 1.0],
        ]
    };

    // Number of ITK dimensions.
    let intent = header.intent_code;
    let ndim = if nifti && matches!(intent, INTENT_DISPVECT | INTENT_VECTOR | INTENT_SYMMATRIX) {
        if dim[4] > 1 {
            4
        } else if dim[3] > 1 {
            3
        } else if dim[2] > 1 {
            2
        } else {
            1
        }
    } else if nifti && intent == INTENT_GENMATRIX {
        return Err(ItkGeometryError::GenMatrix);
    } else {
        let mut realdim = ndim_hdr;
        while realdim > 3 && dim[realdim] == 1 {
            realdim -= 1;
        }
        realdim
    };

    let spacing_scale = match header.xyzt_units & 0x07 {
        1 => 1.0e3,  // metre
        3 => 1.0e-3, // micron
        _ => 1.0,
    };
    let timing_scale = match header.xyzt_units & 0x38 {
        16 => 1.0e-3, // msec
        24 => 1.0e-6, // usec
        _ => 1.0,
    };
    let size: Vec<usize> = (0..ndim)
        .map(|i| usize::try_from(dim[i + 1]).unwrap_or(1))
        .collect();
    let spacing: Vec<f64> = (0..ndim)
        .map(|i| match i {
            0..=2 => f64::from(pixdim[i + 1]) * spacing_scale,
            3 => f64::from(pixdim[4]) * timing_scale,
            _ => f64::from(pixdim[i + 1]),
        })
        .collect();
    let mut origin = vec![0.0; ndim];
    let mut direction: Vec<Vec<f64>> = (0..ndim)
        .map(|i| (0..ndim).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();

    if qform_code == 0 && sform_code == 0 {
        // Origin 0 and identity direction (the Analyze orientation code is ignored with
        // ITK's default "ITK4Warning" Analyze flavour).
        return Ok(ItkGeometry {
            ndim,
            size,
            spacing,
            origin,
            direction,
            source: GeometrySource::Default,
        }
        .with_positive_spacing());
    }

    let mut prefer_sform = qform_sform_are_similar(&sto, &qto);
    if !prefer_sform || sform_code != 0 {
        let decomposable = is_affine(&sto) && sform_is_orthonormal(&sto);
        if decomposable {
            // ITK's three cases: the qform is unset; the sform is SCANNER_ANAT; or both are set
            // and describe nearly the same space.
            if (qform_code == 0 && sform_code != 0) || sform_code == xform::SCANNER_ANAT {
                prefer_sform = true;
            } else if qform_code != 0 && sform_code != 0 {
                prefer_sform = sform_qform_very_similar(&sto, &qto);
            }
        }
        // A non-orthogonal sform is only corrected when ITK_NIFTI_SFORM_PERMISSIVE is on
        // (off by default, and off in ANTs builds): it is then ignored.
    }
    let (m, source) = if prefer_sform {
        (sto, GeometrySource::Sform)
    } else if qform_code != 0 {
        (qto, GeometrySource::Qform)
    } else {
        return Err(ItkGeometryError::NoOrthonormalTransform);
    };

    let spatial = ndim.min(3);
    let lps = [-1.0, -1.0, 1.0];
    for i in 0..spatial {
        origin[i] = lps[i] * f64::from(m[i][3]) * spacing_scale;
    }
    if ndim > 3 {
        origin[3] = header.toffset * timing_scale;
    }
    for j in 0..spatial {
        let column = normalized([0, 1, 2].map(|i| {
            if i < spatial {
                lps[i] * f64::from(m[i][j])
            } else {
                0.0
            }
        }));
        for i in 0..spatial {
            direction[i][j] = column[i];
        }
    }
    Ok(ItkGeometry {
        ndim,
        size,
        spacing,
        origin,
        direction,
        source,
    }
    .with_positive_spacing())
}

impl ItkGeometry {
    /// ITK's `ImageFileReader` (which ANTs uses) makes every spacing positive, flipping the
    /// matching direction column instead; the geometry describes the image it produces.
    fn with_positive_spacing(mut self) -> Self {
        for j in 0..self.ndim {
            if self.spacing[j] < 0.0 {
                self.spacing[j] = -self.spacing[j];
                for row in &mut self.direction {
                    row[j] = -row[j];
                }
            }
        }
        self
    }

    /// The 3D grid ITK reads into a 3D image: the first three dimensions, with the top-left
    /// 3×3 of the direction (a 2D image gets a third axis of one voxel, spacing 1). `None` if
    /// the grid is singular.
    pub fn grid3(&self) -> Option<larmorx_core::Grid3> {
        let n = self.ndim;
        let pick = |v: &[f64], i: usize, default: f64| if i < n { v[i] } else { default };
        let size = std::array::from_fn(|i| if i < n { self.size[i] } else { 1 });
        let spacing = std::array::from_fn(|i| pick(&self.spacing, i, 1.0));
        let origin = std::array::from_fn(|i| pick(&self.origin, i, 0.0));
        let direction = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                if i < n && j < n {
                    self.direction[i][j]
                } else if i == j {
                    1.0
                } else {
                    0.0
                }
            })
        });
        larmorx_core::Grid3::new(size, spacing, origin, direction)
    }

    /// The voxel-to-world affine of the first three dimensions in **RAS+** (as nibabel would
    /// read a file ITK wrote with this geometry): `diag(-1,-1,1)·[D·diag(spacing) | origin]`.
    pub fn ras_affine(&self) -> larmorx_core::Affine {
        let lps = [-1.0, -1.0, 1.0];
        let direction = |i: usize, j: usize| {
            if i < self.ndim && j < self.ndim {
                self.direction[i][j]
            } else if i == j {
                1.0
            } else {
                0.0
            }
        };
        let linear = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                lps[i] * direction(i, j) * self.spacing.get(j).copied().unwrap_or(1.0)
            })
        });
        let translation =
            std::array::from_fn(|i| lps[i] * self.origin.get(i).copied().unwrap_or(0.0));
        larmorx_core::Affine::from_linear(linear, translation)
    }
}

// ------------------------------------------------------------------------------------------------
// Voxel values

/// Voxel values as ITK's NIfTI reader produces them, in the narrowest float type that holds
/// them exactly (ANTs then converts them to its pixel type, usually double, without loss).
#[derive(Clone, Debug, PartialEq)]
pub enum ItkVoxels {
    F32(Vec<f32>),
    F64(Vec<f64>),
}

impl ItkVoxels {
    pub fn len(&self) -> usize {
        match self {
            ItkVoxels::F32(v) => v.len(),
            ItkVoxels::F64(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// An image as ITK 5.4.5 reads it from a NIfTI file: its geometry and its voxel values
/// (Fortran order, `x` fastest; for vector images the components are the slowest axis, as
/// stored).
#[derive(Clone, Debug, PartialEq)]
pub struct ItkImage {
    pub geometry: ItkGeometry,
    pub voxels: ItkVoxels,
    /// The header's `intent_code`.
    pub intent_code: i32,
}

/// Reads a NIfTI image's geometry and voxel values the way ITK 5.4.5's `NiftiImageIO` does.
///
/// Values differ from nibabel's when the header has scaling: ITK ignores `scl_slope`/`scl_inter`
/// for Analyze files and when the slope is 0 (or not finite); integer data are converted to
/// float32 **before** scaling and the scaled value is rounded to float32 again
/// (`f32(f64(f32(x))·slope + inter)`), float32 data are scaled in double and rounded to
/// float32. Displacement vectors with intent `NIFTI_INTENT_DISPVECT` are converted from RAS to
/// LPS (x and y negated); other vector intents are taken as LPS already (ANTs writes
/// `NIFTI_INTENT_VECTOR`). Complex and RGB data are not supported.
pub fn read_itk_image(path: impl AsRef<Path>, n_threads: usize) -> Result<ItkImage, Error> {
    let path = path.as_ref();
    let stored = super::read_header(path)?;
    let geometry = itk_geometry(&stored).map_err(|e| super::format_err(path, e.to_string()))?;
    let image = super::read(
        path,
        &ReadOptions {
            scaling: Scaling::Raw,
            n_threads,
        },
    )?;
    let (slope, inter) = itk_rescale(&stored);
    let rescale = must_rescale(slope, inter);
    let is_float = matches!(image.data, DynArray::F32(_) | DynArray::F64(_));
    if stored.intent_code == INTENT_DISPVECT && !is_float && !rescale {
        return Err(super::format_err(
            path,
            "ITK converts NIFTI_INTENT_DISPVECT vectors only for float data",
        ));
    }
    let voxels = larmorx_core::parallel::with_threads(n_threads, || {
        dispatch_real_dyn_array!(&image.data, a => {
            Ok(itk_values(a.as_slice_memory_order().expect("contiguous"), slope, inter, rescale))
        }, complex => Err(super::format_err(
            path,
            "complex images are not supported by the ITK reader port",
        )))
    })??;
    let voxels = if stored.intent_code == INTENT_DISPVECT {
        ras_to_lps_vectors(voxels, path)?
    } else {
        voxels
    };
    Ok(ItkImage {
        geometry,
        voxels,
        intent_code: stored.intent_code,
    })
}

/// ITK's `m_RescaleSlope`/`m_RescaleIntercept`: nifti_clib zeroes non-finite values, ITK
/// replaces a zero slope by 1 and ignores scaling in Analyze files.
fn itk_rescale(h: &NiftiHeader) -> (f64, f64) {
    if !is_nifti(&h.magic) {
        return (1.0, 0.0);
    }
    let finite = |v: f64| if v.is_finite() { v } else { 0.0 };
    let (mut slope, inter) = (finite(h.scl_slope), finite(h.scl_inter));
    if slope.abs() < f64::EPSILON {
        slope = 1.0;
    }
    (slope, inter)
}

/// ITK's `NiftiImageIO::MustRescale`.
fn must_rescale(slope: f64, inter: f64) -> bool {
    slope.abs() > f64::EPSILON && ((slope - 1.0).abs() > f64::EPSILON || inter.abs() > f64::EPSILON)
}

/// ITK's values for stored values of type `T` (see [`read_itk_image`]).
fn itk_values<T: RealElement>(v: &[T], slope: f64, inter: f64, rescale: bool) -> ItkVoxels {
    fn map<T: Copy + Send + Sync, U: Send>(v: &[T], f: impl Fn(T) -> U + Send + Sync) -> Vec<U> {
        v.par_iter().with_min_len(1 << 16).map(|&x| f(x)).collect()
    }
    match T::DATA_TYPE {
        DataType::F64 if rescale => ItkVoxels::F64(map(v, |x| x.to_f64() * slope + inter)),
        DataType::F64 => ItkVoxels::F64(map(v, |x| x.to_f64())),
        // Float data are scaled in double and stored back as float; integer data are first
        // converted to float (`CastCopy<float>`).
        DataType::F32 if rescale => ItkVoxels::F32(map(v, |x| (x.to_f64() * slope + inter) as f32)),
        _ if rescale => ItkVoxels::F32(map(v, |x| (f64::from(x.to_f32()) * slope + inter) as f32)),
        // Unscaled: the stored values, in float32 when that is exact.
        DataType::F32 | DataType::U8 | DataType::I8 | DataType::U16 | DataType::I16 => {
            ItkVoxels::F32(map(v, |x| x.to_f32()))
        }
        _ => ItkVoxels::F64(map(v, |x| x.to_f64())),
    }
}

/// ITK's RAS→LPS conversion of displacement vectors (components are the slowest axis).
fn ras_to_lps_vectors(voxels: ItkVoxels, path: &Path) -> Result<ItkVoxels, Error> {
    let n = voxels.len();
    if !n.is_multiple_of(3) {
        return Err(super::format_err(
            path,
            "NIFTI_INTENT_DISPVECT needs 3-component vectors",
        ));
    }
    let flip = 2 * n / 3;
    Ok(match voxels {
        ItkVoxels::F32(mut v) => {
            v[..flip].iter_mut().for_each(|x| *x = -*x);
            ItkVoxels::F32(v)
        }
        ItkVoxels::F64(mut v) => {
            v[..flip].iter_mut().for_each(|x| *x = -*x);
            ItkVoxels::F64(v)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::{Affine, DataType};

    fn header(affine: &Affine, qcode: i32, scode: i32) -> NiftiHeader {
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[10, 11, 12], DataType::F32).unwrap();
        h.set_qform(affine, qcode).unwrap();
        h.set_sform(affine, scode);
        h
    }

    #[test]
    fn ras_input_gives_lps_geometry() {
        let affine = Affine::from_zooms([2.0, 3.0, 4.0], [-10.0, 20.0, 30.0]);
        let g = itk_geometry(&header(&affine, 1, 1)).unwrap();
        assert_eq!(g.source, GeometrySource::Sform);
        assert_eq!(g.spacing, vec![2.0, 3.0, 4.0]);
        assert_eq!(g.origin, vec![10.0, -20.0, 30.0]);
        assert_eq!(
            g.direction,
            vec![
                vec![-1.0, 0.0, 0.0],
                vec![0.0, -1.0, 0.0],
                vec![0.0, 0.0, 1.0]
            ]
        );
        assert!(g.ras_affine().allclose(&affine, 0.0, 1e-12));
    }

    #[test]
    fn source_selection_follows_itk() {
        let a = Affine::from_zooms([2.0, 2.0, 2.0], [0.0; 3]);
        let b = Affine::from_zooms([2.0, 2.0, 2.0], [5.0, 0.0, 0.0]);
        let mut h = header(&a, 1, 2);
        h.set_sform(&b, 2); // aligned sform that disagrees with the qform: ITK keeps the qform
        assert_eq!(itk_geometry(&h).unwrap().source, GeometrySource::Qform);
        h.sform_code = 1; // scanner sform: ITK prefers it
        assert_eq!(itk_geometry(&h).unwrap().source, GeometrySource::Sform);
        h.qform_code = 0; // qform unset, orthonormal sform
        h.sform_code = 4;
        assert_eq!(itk_geometry(&h).unwrap().source, GeometrySource::Sform);
        h.sform_code = 0; // neither
        let g = itk_geometry(&h).unwrap();
        assert_eq!(
            (g.source, g.origin.clone()),
            (GeometrySource::Default, vec![0.0; 3])
        );
        // A sheared sform with no qform cannot be used.
        let mut sheared = header(&a, 0, 2);
        sheared.srow_x[1] = 0.8;
        assert_eq!(
            itk_geometry(&sheared),
            Err(ItkGeometryError::NoOrthonormalTransform)
        );
    }

    #[test]
    fn negative_pixdim_flips_the_direction() {
        let mut h = header(&Affine::from_zooms([2.0, 2.0, 2.0], [0.0; 3]), 1, 0);
        h.pixdim[1] = -2.0;
        let g = itk_geometry(&h).unwrap();
        assert_eq!(g.spacing[0], 2.0);
        assert_eq!(g.direction[0][0], 1.0); // -(-1): the RAS x flip, then the negative spacing
    }

    #[test]
    fn dimensions_and_units() {
        let a = Affine::from_zooms([1.0, 1.0, 1.0], [0.0; 3]);
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[4, 5, 6, 7, 1], DataType::I16).unwrap();
        h.set_affine(&a).unwrap();
        h.pixdim[4] = 800.0;
        h.xyzt_units = 2 | 16; // mm, msec
        let g = itk_geometry(&h).unwrap();
        assert_eq!(g.ndim, 4);
        assert!((g.spacing[3] - 0.8).abs() < 1e-12);
        h.set_shape(&[4, 5, 6, 1, 1]).unwrap();
        assert_eq!(itk_geometry(&h).unwrap().ndim, 3);
        h.xyzt_units = 1; // metres
        assert_eq!(itk_geometry(&h).unwrap().spacing[0], 1000.0);
        assert_eq!(
            itk_geometry(&h.with_version(NiftiVersion::V2)),
            Err(ItkGeometryError::Nifti2)
        );
    }
}
