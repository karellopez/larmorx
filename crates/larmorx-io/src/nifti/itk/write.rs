// SPDX-License-Identifier: Apache-2.0
//! How ITK 5.4.5 (and so every ANTs program) writes a NIfTI-1 file.
//!
//! ITK's `NiftiImageIO::WriteImageInformation` fills a `nifti_image` from the image's size,
//! spacing, origin and direction, and nifti_clib turns it into a header
//! (`nifti_convert_nim2nhdr`). The direction becomes a qform through
//! `nifti_make_orthog_mat44` and `nifti_mat44_to_quatern`, both computed in single precision
//! with nifti_clib's polar decomposition; the sform is the same matrix with each column
//! scaled by the spacing, in float. Both codes are `NIFTI_XFORM_SCANNER_ANAT`, the units are
//! millimetres and seconds, and `descrip` and `aux_file` come from the image's metadata
//! dictionary (the values read from the input file, when the written image is the one that
//! was read). Ported from ITK v5.4.5 (Apache-2.0) and its bundled nifti_clib (public
//! domain); see `PROVENANCE.md`. Every float step is kept in float, so the header matches
//! ITK's bit for bit.

use std::path::Path;

use larmorx_core::element::{DataType, Element};
use larmorx_core::ndarray::{ArrayViewD, IxDyn, ShapeBuilder};

use super::super::header::{NiftiHeader, NiftiVersion};
use super::super::{Error, WriteOptions};
use super::ItkGeometry;

/// The header fields ITK's NIfTI writer takes from an image's metadata dictionary.
///
/// ITK's reader stores the input's `descrip` (as `ITK_FileNotes`) and `aux_file` in the
/// dictionary of the image it returns. An ANTs program that writes that same image object
/// (after changing its voxels in place) writes them back; images made by filters start with
/// an empty dictionary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItkMeta {
    /// `descrip` without its terminating NUL (at most 79 bytes).
    pub descrip: Vec<u8>,
    /// `aux_file` without its terminating NUL (at most 23 bytes).
    pub aux_file: Vec<u8>,
}

impl ItkMeta {
    /// The dictionary ITK's reader builds from a header: nifti_clib keeps the first 79 bytes
    /// of `descrip` and 23 of `aux_file`, and ITK copies them up to the first NUL.
    pub fn from_header(h: &NiftiHeader) -> Self {
        fn c_string(bytes: &[u8], max: usize) -> Vec<u8> {
            let bytes = &bytes[..max.min(bytes.len())];
            let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            bytes[..end].to_vec()
        }
        ItkMeta {
            descrip: c_string(&h.descrip, 79),
            aux_file: c_string(&h.aux_file, 23),
        }
    }
}

/// ITK cannot write the image.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ItkWriteError {
    #[error("Dimension({axis}) = {size} is greater than maximum possible dimension 32767")]
    TooLarge { axis: usize, size: usize },
    #[error("ITK images have 1 to 7 dimensions, not {0}")]
    BadNdim(usize),
    #[error("{0}: not a NIfTI file name (ITK's NIfTI writer needs .nii, .nii.gz, .hdr or .img)")]
    BadFileName(String),
}

// ------------------------------------------------------------------------------------------------
// nifti_clib's single-precision matrix helpers (nifti1_io.c)

type M33 = [[f32; 3]; 3];

fn mat33_determ(r: &M33) -> f32 {
    let [[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]] = r.map(|row| row.map(f64::from));
    (r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
        - r31 * r22 * r13) as f32
}

fn mat33_inverse(r: &M33) -> M33 {
    let [[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]] = r.map(|row| row.map(f64::from));
    let mut deti =
        r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
            - r31 * r22 * r13;
    if deti != 0.0 {
        deti = 1.0 / deti;
    }
    [
        [
            (deti * (r22 * r33 - r32 * r23)) as f32,
            (deti * (-r12 * r33 + r32 * r13)) as f32,
            (deti * (r12 * r23 - r22 * r13)) as f32,
        ],
        [
            (deti * (-r21 * r33 + r31 * r23)) as f32,
            (deti * (r11 * r33 - r31 * r13)) as f32,
            (deti * (-r11 * r23 + r21 * r13)) as f32,
        ],
        [
            (deti * (r21 * r32 - r31 * r22)) as f32,
            (deti * (-r11 * r32 + r31 * r12)) as f32,
            (deti * (r11 * r22 - r21 * r12)) as f32,
        ],
    ]
}

/// Max row norm; `fabs` promotes to double, the sum is rounded to float.
fn mat33_rownorm(a: &M33) -> f32 {
    let row = |i: usize| {
        (f64::from(a[i][0]).abs() + f64::from(a[i][1]).abs() + f64::from(a[i][2]).abs()) as f32
    };
    let (mut r1, r2, r3) = (row(0), row(1), row(2));
    if r1 < r2 {
        r1 = r2;
    }
    if r1 < r3 {
        r1 = r3;
    }
    r1
}

fn mat33_colnorm(a: &M33) -> f32 {
    let col = |j: usize| {
        (f64::from(a[0][j]).abs() + f64::from(a[1][j]).abs() + f64::from(a[2][j]).abs()) as f32
    };
    let (mut r1, r2, r3) = (col(0), col(1), col(2));
    if r1 < r2 {
        r1 = r2;
    }
    if r1 < r3 {
        r1 = r3;
    }
    r1
}

/// nifti_clib's polar decomposition (Higham 1986): the orthogonal matrix closest to `a`.
fn mat33_polar(a: &M33) -> M33 {
    let mut x = *a;
    let mut dif = 1.0f32;
    let mut k = 0;
    let mut gam = mat33_determ(&x);
    while gam == 0.0 {
        gam = (0.00001 * (0.001 + f64::from(mat33_rownorm(&x)))) as f32;
        x[0][0] += gam;
        x[1][1] += gam;
        x[2][2] += gam;
        gam = mat33_determ(&x);
    }
    loop {
        let y = mat33_inverse(&x);
        let gmi;
        if f64::from(dif) > 0.3 {
            let alp = f64::from(mat33_rownorm(&x) * mat33_colnorm(&x)).sqrt() as f32;
            let bet = f64::from(mat33_rownorm(&y) * mat33_colnorm(&y)).sqrt() as f32;
            gam = f64::from(bet / alp).sqrt() as f32;
            gmi = (1.0 / f64::from(gam)) as f32;
        } else {
            gam = 1.0;
            gmi = 1.0;
        }
        let z: M33 = std::array::from_fn(|i| {
            std::array::from_fn(|j| (0.5 * f64::from(gam * x[i][j] + gmi * y[j][i])) as f32)
        });
        let mut sum = 0.0f64;
        for i in 0..3 {
            for j in 0..3 {
                sum += f64::from(z[i][j] - x[i][j]).abs();
            }
        }
        dif = sum as f32;
        k += 1;
        if k > 100 || f64::from(dif) < 3.0e-6 {
            return z;
        }
        x = z;
    }
}

/// `nifti_make_orthog_mat44`: rows normalised in float (a zero third row becomes the cross
/// product of the first two), then orthogonalised. The translation is zero.
fn make_orthog(rows: [[f32; 3]; 3]) -> M33 {
    let mut q = rows;
    for (i, default) in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]].iter().enumerate() {
        let val = f64::from(q[i][0] * q[i][0] + q[i][1] * q[i][1] + q[i][2] * q[i][2]);
        if val > 0.0 {
            let s = (1.0 / val.sqrt()) as f32;
            q[i] = q[i].map(|v| v * s);
        } else {
            q[i] = *default;
        }
    }
    let val = f64::from(q[2][0] * q[2][0] + q[2][1] * q[2][1] + q[2][2] * q[2][2]);
    if val > 0.0 {
        let s = (1.0 / val.sqrt()) as f32;
        q[2] = q[2].map(|v| v * s);
    } else {
        q[2] = [
            q[0][1] * q[1][2] - q[0][2] * q[1][1],
            q[0][2] * q[1][0] - q[0][0] * q[1][2],
            q[0][0] * q[1][1] - q[0][1] * q[1][0],
        ];
    }
    mat33_polar(&q)
}

/// `nifti_mat44_to_quatern` for the rotation part `r` (single precision in and out; the
/// arithmetic in between is double, where nifti_clib mixes in `long double` constants whose
/// effect does not survive the final rounding to float).
fn mat33_to_quatern(r: &M33) -> ([f32; 3], f32) {
    let [
        [mut r11, mut r12, mut r13],
        [mut r21, mut r22, mut r23],
        [mut r31, mut r32, mut r33],
    ] = r.map(|row| row.map(f64::from));
    let mut xd = (r11 * r11 + r21 * r21 + r31 * r31).sqrt();
    let mut yd = (r12 * r12 + r22 * r22 + r32 * r32).sqrt();
    let mut zd = (r13 * r13 + r23 * r23 + r33 * r33).sqrt();
    if xd == 0.0 {
        r11 = 1.0;
        r21 = 0.0;
        r31 = 0.0;
        xd = 1.0;
    }
    if yd == 0.0 {
        r22 = 1.0;
        r12 = 0.0;
        r32 = 0.0;
        yd = 1.0;
    }
    if zd == 0.0 {
        r33 = 1.0;
        r13 = 0.0;
        r23 = 0.0;
        zd = 1.0;
    }
    r11 /= xd;
    r21 /= xd;
    r31 /= xd;
    r12 /= yd;
    r22 /= yd;
    r32 /= yd;
    r13 /= zd;
    r23 /= zd;
    r33 /= zd;
    let q: M33 = [
        [r11 as f32, r12 as f32, r13 as f32],
        [r21 as f32, r22 as f32, r23 as f32],
        [r31 as f32, r32 as f32, r33 as f32],
    ];
    let p = mat33_polar(&q).map(|row| row.map(f64::from));
    let [
        [r11, r12, mut r13],
        [r21, r22, mut r23],
        [r31, r32, mut r33],
    ] = p;
    let det =
        r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
            - r31 * r22 * r13;
    let qfac = if det > 0.0 {
        1.0
    } else {
        r13 = -r13;
        r23 = -r23;
        r33 = -r33;
        -1.0
    };
    let mut a = r11 + r22 + r33 + 1.0;
    let (b, c, d);
    if a > 0.5 {
        a = 0.5 * a.sqrt();
        b = 0.25 * (r32 - r23) / a;
        c = 0.25 * (r13 - r31) / a;
        d = 0.25 * (r21 - r12) / a;
    } else {
        let xd = 1.0 + r11 - (r22 + r33);
        let yd = 1.0 + r22 - (r11 + r33);
        let zd = 1.0 + r33 - (r11 + r22);
        let (mut bb, mut cc, mut dd);
        if xd > 1.0 {
            bb = 0.5 * xd.sqrt();
            cc = 0.25 * (r12 + r21) / bb;
            dd = 0.25 * (r13 + r31) / bb;
            a = 0.25 * (r32 - r23) / bb;
        } else if yd > 1.0 {
            cc = 0.5 * yd.sqrt();
            bb = 0.25 * (r12 + r21) / cc;
            dd = 0.25 * (r23 + r32) / cc;
            a = 0.25 * (r13 - r31) / cc;
        } else {
            dd = 0.5 * zd.sqrt();
            bb = 0.25 * (r13 + r31) / dd;
            cc = 0.25 * (r23 + r32) / dd;
            a = 0.25 * (r21 - r12) / dd;
        }
        if a < 0.0 {
            bb = -bb;
            cc = -cc;
            dd = -dd;
        }
        (b, c, d) = (bb, cc, dd);
    }
    ([b as f32, c as f32, d as f32], qfac)
}

// ------------------------------------------------------------------------------------------------
// The header

/// The NIfTI-1 header ITK 5.4.5 writes for a scalar image with `geometry`, pixels of
/// `data_type` and dictionary `meta` (`WriteImageInformation`,
/// `SetNIfTIOrientationFromImageIO`, `nifti_convert_nim2nhdr`).
///
/// The magic string and `vox_offset` are set when the file is written (see
/// [`write_itk_image`]).
pub fn itk_header(
    geometry: &ItkGeometry,
    data_type: DataType,
    meta: &ItkMeta,
) -> Result<NiftiHeader, ItkWriteError> {
    let n = geometry.ndim;
    if !(1..=7).contains(&n) {
        return Err(ItkWriteError::BadNdim(n));
    }
    for (axis, &size) in geometry.size.iter().enumerate() {
        if size > i16::MAX as usize {
            return Err(ItkWriteError::TooLarge { axis, size });
        }
    }
    let mut h =
        NiftiHeader::new(NiftiVersion::V1, &geometry.size, data_type).expect("sizes checked above");
    // nifti_clib starts from `nifti_simple_init_nim`: dim 1, pixdim 1 for the first three
    // axes and 0 beyond; ITK then sets the image's axes.
    h.dim = [n as i64, 1, 1, 1, 1, 1, 1, 1];
    h.pixdim = [0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    for i in 0..n {
        h.dim[i + 1] = geometry.size[i] as i64;
        h.pixdim[i + 1] = f64::from((geometry.spacing[i] as f32).abs());
    }
    h.analyze = Default::default();
    h.analyze.regular = b'r';
    h.scl_slope = 1.0;
    h.scl_inter = 0.0;
    h.xyzt_units = 2 | 8; // NIFTI_UNITS_MM | NIFTI_UNITS_SEC
    h.toffset = if n >= 4 {
        f64::from(geometry.origin[3] as f32)
    } else {
        0.0
    };

    // The direction: each axis as a row, LPS → RAS (x and y negated), orthogonalised; the
    // matrix is transposed so the axes become columns.
    let dir = |axis: usize, i: usize| -> f32 {
        if axis < n && i < n {
            geometry.direction[i][axis] as f32
        } else {
            0.0
        }
    };
    let mut dirx = [-dir(0, 0), -dir(0, 1), -dir(0, 2)];
    if n < 3 {
        dirx[2] = 0.0;
    }
    let mut diry = [0.0f32; 3];
    if n > 1 {
        diry = [-dir(1, 0), -dir(1, 1), -dir(1, 2)];
        if n < 3 {
            diry[2] = 0.0;
        }
    }
    let dirz;
    if n > 2 {
        dirz = [-dir(2, 0), -dir(2, 1), dir(2, 2)];
        dirx[2] = -dirx[2];
        diry[2] = -diry[2];
    } else {
        dirz = [0.0, 0.0, 1.0];
    }
    let rows = make_orthog([dirx, diry, dirz]);
    let m: M33 = std::array::from_fn(|i| std::array::from_fn(|j| rows[j][i]));
    let origin = |i: usize| geometry.origin.get(i).copied().unwrap_or(0.0);
    let offset = [
        -origin(0) as f32,
        if n > 1 { -origin(1) as f32 } else { 0.0 },
        if n > 2 { origin(2) as f32 } else { 0.0 },
    ];

    let ([b, c, d], qfac) = mat33_to_quatern(&m);
    h.qform_code = 1;
    h.sform_code = 1;
    h.quatern_b = f64::from(b);
    h.quatern_c = f64::from(c);
    h.quatern_d = f64::from(d);
    h.qoffset_x = f64::from(offset[0]);
    h.qoffset_y = f64::from(offset[1]);
    h.qoffset_z = f64::from(offset[2]);
    h.pixdim[0] = if qfac >= 0.0 { 1.0 } else { -1.0 };
    let limit = n.min(3);
    let srow = |i: usize| -> [f64; 4] {
        let mut row = [0.0; 4];
        for (j, v) in row.iter_mut().take(3).enumerate() {
            let s = if i < limit && j < limit {
                geometry.spacing[j] as f32 * m[i][j]
            } else {
                m[i][j]
            };
            *v = f64::from(s);
        }
        row[3] = f64::from(offset[i]);
        row
    };
    h.srow_x = srow(0);
    h.srow_y = srow(1);
    h.srow_z = srow(2);

    h.descrip = [0; 80];
    let k = meta.descrip.len().min(79);
    h.descrip[..k].copy_from_slice(&meta.descrip[..k]);
    h.aux_file = [0; 24];
    let k = meta.aux_file.len().min(23);
    h.aux_file[..k].copy_from_slice(&meta.aux_file[..k]);
    Ok(h)
}

/// Whether ITK's NIfTI writer accepts `path` (`nifti_find_file_extension`: `.nii`, `.nii.gz`,
/// `.hdr`, `.img`, `.hdr.gz`, `.img.gz`).
pub fn is_itk_nifti_name(path: &Path) -> bool {
    super::super::paths::NiftiPaths::from_path(path).is_some()
}

/// Writes `data` (Fortran order) as ITK 5.4.5 writes an image with `geometry` and `meta`.
pub fn write_itk_image<T: Element>(
    path: impl AsRef<Path>,
    geometry: &ItkGeometry,
    meta: &ItkMeta,
    data: &[T],
    options: &WriteOptions,
) -> Result<(), Error> {
    let path = path.as_ref();
    if !is_itk_nifti_name(path) {
        return Err(Error::InvalidArgument(
            ItkWriteError::BadFileName(path.display().to_string()).to_string(),
        ));
    }
    let header = itk_header(geometry, T::DATA_TYPE, meta)
        .map_err(|e| super::super::format_err(path, e.to_string()))?;
    let view = ArrayViewD::from_shape(IxDyn(&geometry.size).f(), data)
        .map_err(|e| Error::InvalidArgument(format!("{}: {e}", path.display())))?;
    super::super::write(path, &header, view, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nifti::itk::{GeometrySource, itk_geometry};

    fn geometry(direction: [[f64; 3]; 3], spacing: [f64; 3], origin: [f64; 3]) -> ItkGeometry {
        ItkGeometry {
            ndim: 3,
            size: vec![4, 5, 6],
            spacing: spacing.to_vec(),
            origin: origin.to_vec(),
            direction: direction.iter().map(|r| r.to_vec()).collect(),
            source: GeometrySource::Sform,
            flipped: vec![false; 3],
        }
    }

    #[test]
    fn lps_identity_is_written_as_ras() {
        let g = geometry(
            [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]],
            [2.0, 3.0, 4.0],
            [10.0, -20.0, 30.0],
        );
        let h = itk_header(&g, DataType::F32, &ItkMeta::default()).unwrap();
        assert_eq!(h.srow_x, [2.0, 0.0, 0.0, -10.0]);
        assert_eq!(h.srow_y, [0.0, 3.0, 0.0, 20.0]);
        assert_eq!(h.srow_z, [0.0, 0.0, 4.0, 30.0]);
        assert_eq!((h.quatern_b, h.quatern_c, h.quatern_d), (0.0, 0.0, 0.0));
        assert_eq!(h.pixdim[..5], [1.0, 2.0, 3.0, 4.0, 0.0]);
        assert_eq!((h.qform_code, h.sform_code, h.xyzt_units), (1, 1, 10));
        // Read back as ITK reads it.
        let back = itk_geometry(&h).unwrap();
        assert_eq!(back.origin, g.origin);
        assert_eq!(back.direction, g.direction);
    }

    #[test]
    fn flipped_axes_give_qfac_and_quaternions() {
        // LPS identity direction: RAS x and y flipped, a proper rotation by pi about z.
        let g = geometry(
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [1.0; 3],
            [0.0; 3],
        );
        let h = itk_header(&g, DataType::F32, &ItkMeta::default()).unwrap();
        assert_eq!(h.pixdim[0], 1.0);
        assert_eq!((h.quatern_b, h.quatern_c, h.quatern_d), (0.0, 0.0, 1.0));
        // An improper direction (one axis flipped) has qfac -1.
        let g = geometry(
            [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]],
            [1.0; 3],
            [0.0; 3],
        );
        let h = itk_header(&g, DataType::F32, &ItkMeta::default()).unwrap();
        assert_eq!(h.pixdim[0], -1.0);
        assert_eq!(h.srow_z, [0.0, 0.0, -1.0, 0.0]);
    }

    #[test]
    fn oblique_directions_round_trip_through_itk_reading() {
        let (s, c) = (0.3f64.sin(), 0.3f64.cos());
        let g = geometry(
            [[-c, s, 0.0], [-s, -c, 0.0], [0.0, 0.0, 1.0]],
            [1.5, 1.5, 3.0],
            [-5.0, 7.0, 2.0],
        );
        let h = itk_header(&g, DataType::I16, &ItkMeta::default()).unwrap();
        let back = itk_geometry(&h).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                assert!((back.direction[i][j] - g.direction[i][j]).abs() < 1e-6);
            }
            assert!((back.origin[i] - g.origin[i]).abs() < 1e-5);
        }
    }

    #[test]
    fn meta_strings_are_cut_at_nul_and_length() {
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[1, 1, 1], DataType::U8).unwrap();
        h.descrip[..5].copy_from_slice(b"hello");
        h.aux_file = [b'a'; 24];
        let meta = ItkMeta::from_header(&h);
        assert_eq!(meta.descrip, b"hello");
        assert_eq!(meta.aux_file.len(), 23);
    }
}
