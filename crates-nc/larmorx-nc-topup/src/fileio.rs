// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-topup 2203.5: topup_file_io.{h,cpp},
// topup_matrices.{h,cpp}; fsl-miscmaths 2412.6: write_ascii_matrix). Copyright (C) 2012
// University of Oxford (Jesper Andersson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! topup's files: the acquisition-parameter file (`TopupDatafileReader`), the spline
//! coefficients, the field and the movement parameters (`TopupFileWriter`), and the
//! movement-parameter matrices (`MovePar2Matrix`, `Matrix2MovePar`).

use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::basisfield::{FieldIndex, SplineField};
use larmorx_nc_core::cnum;
use larmorx_nc_core::miscmaths::{self, MatrixFileError};
use larmorx_nc_core::nifti;
use larmorx_nc_core::volume::{FslError, Volume};

/// `FSL_TOPUP_CUBIC_SPLINE_COEFFICIENTS`.
pub const CUBIC_SPLINE_COEFFICIENTS: i32 = 2016;
/// `FSL_TOPUP_QUADRATIC_SPLINE_COEFFICIENTS`.
pub const QUADRATIC_SPLINE_COEFFICIENTS: i32 = 2017;
/// `FSL_TOPUP_FIELD`.
pub const TOPUP_FIELD: i32 = 2018;

/// `TopupFileIOException(msg)`: `what()` is `"TopupFileIO::" + msg`.
pub fn fileio_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "TOPUP::TopupFileIOException",
        what: format!("TopupFileIO::{msg}"),
    }
}

/// `TopupMatrixException(msg)` (its text also starts `"TopupFileIO::"`).
pub fn matrix_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "TOPUP::TopupMatrixException",
        what: format!("TopupFileIO::{msg}"),
    }
}

/// What `read_ascii_matrix` did besides returning a matrix: the line it printed (a file that
/// cannot be opened gives an empty matrix), or the exception it threw.
pub fn read_matrix(fname: &str, cerr: &mut Vec<String>) -> Result<Mat, FslError> {
    match miscmaths::read_ascii_matrix(fname) {
        Ok(m) => Ok(m),
        Err(MatrixFileError::CannotOpen) => {
            cerr.push(format!("Could not open matrix file {fname}"));
            Ok(Mat::zeros(0, 0))
        }
        Err(MatrixFileError::NotEnoughNumbers) => Err(FslError {
            printed: Vec::new(),
            type_name: "armawrap::AWException",
            what: "Could not find enough numbers in matrix file".to_owned(),
        }),
    }
}

/// `NormFrobenius()` of a column (Armadillo's `vec_norm_2_direct_mem`: two accumulators over
/// pairs, the last odd element to the first).
fn norm_fro(v: &[f64]) -> f64 {
    let (mut a1, mut a2) = (0.0f64, 0.0f64);
    let mut i = 0;
    while i + 1 < v.len() {
        a1 += v[i] * v[i];
        a2 += v[i + 1] * v[i + 1];
        i += 2;
    }
    if i < v.len() {
        a1 += v[i] * v[i];
    }
    (a1 + a2).sqrt()
}

/// `TopupDatafileReader`: one row per volume, a phase-encoding vector (three columns) and a
/// read-out time.
#[derive(Clone, Debug)]
pub struct Datafile {
    /// The phase-encoding vectors, one per volume.
    pub pv: Vec<[f64; 3]>,
    /// The read-out times (s).
    pub rt: Vec<f64>,
}

impl Datafile {
    /// `TopupDatafileReader(fname)`: a file of four columns (or four rows, transposed); each
    /// phase-encoding vector of unit length within 0.01.
    pub fn read(fname: &str, cerr: &mut Vec<String>) -> Result<Datafile, FslError> {
        let mut tmp = read_matrix(fname, cerr)?;
        if tmp.cols() != 4 {
            if tmp.rows() == 4 {
                tmp = tmp.t();
            } else {
                return Err(fileio_error(&format!(
                    "TopupDatafileReader:: error reading file {fname}"
                )));
            }
        }
        let mut pv = Vec::with_capacity(tmp.rows());
        let mut rt = Vec::with_capacity(tmp.rows());
        for i in 0..tmp.rows() {
            let v = [tmp.at(i, 0), tmp.at(i, 1), tmp.at(i, 2)];
            if (1.0 - norm_fro(&v)).abs() > 0.01 {
                return Err(fileio_error(
                    "TopupDatafileReader:: phase-encode vectors must be unity length",
                ));
            }
            pv.push(v);
            rt.push(tmp.at(i, 3));
        }
        Ok(Datafile { pv, rt })
    }

    /// `N()`.
    pub fn n(&self) -> usize {
        self.pv.len()
    }
}

/// `MovePar2Matrix(mp, vol)`: the rigid-body matrix of `mp` (`dx dy dz rx ry rz`) about the
/// centre of `vol`'s field of view.
pub fn movepar2matrix<T: larmorx_nc_core::volume::Elem>(mp: &[f64; 6], vol: &Volume<T>) -> Mat {
    let tmp = Mat::col(&[mp[3], mp[4], mp[5], mp[0], mp[1], mp[2]]);
    let c = |n: i64, d: f32| f64::from((n - 1) as f32 * d) / 2.0;
    let cntr = Mat::col(&[
        c(vol.xsize(), vol.xdim()),
        c(vol.ysize(), vol.ydim()),
        c(vol.zsize(), vol.zdim()),
    ]);
    miscmaths::construct_rotmat_euler(&tmp, 6, &cntr)
}

/// `Matrix2MovePar(M, vol)`.
pub fn matrix2movepar<T: larmorx_nc_core::volume::Elem>(m: &Mat, vol: &Volume<T>) -> [f64; 6] {
    let rot = miscmaths::rotmat2euler(m);
    let mut mp = [0.0, 0.0, 0.0, rot[0], rot[1], rot[2]];
    let mm = movepar2matrix(&mp, vol);
    for k in 0..3 {
        mp[k] = m.at(k, 3) - mm.at(k, 3);
    }
    mp
}

/// `write_ascii_matrix(mat, filename)` (precision 10): `None` and the printed line where the
/// file cannot be opened (FSL returns -1); an empty name returns -1 without a message.
pub fn write_ascii_matrix(name: &str, m: &Mat) -> Result<(), Option<String>> {
    if name.is_empty() {
        return Err(None);
    }
    let mut t = String::new();
    for r in 0..m.rows() {
        for c in 0..m.cols() {
            t.push_str(&cnum::fmt_g(m.at(r, c), 10));
            t.push_str("  ");
        }
        t.push('\n');
    }
    std::fs::write(name, t).map_err(|_| Some(format!("Could not open file {name} for writing")))
}

/// `TopupFileWriter(fname, field)`: the coefficients as an image of the coefficients' size, its
/// voxel sizes the knot spacings, the intent the spline order with the field's voxel sizes, the
/// qform (scanner) the identity shifted by the field's size.
pub fn coefficient_volume(field: &SplineField) -> Result<Volume, FslError> {
    let csz = field.coef_size();
    let mut coefs = Volume::new(i64::from(csz[0]), i64::from(csz[1]), i64::from(csz[2]), 1);
    let ksp = field.ksp();
    let vxs = field.voxel_size();
    let code = match field.order() {
        2 => Some(QUADRATIC_SPLINE_COEFFICIENTS),
        3 => Some(CUBIC_SPLINE_COEFFICIENTS),
        _ => None,
    };
    if let Some(code) = code {
        coefs.intent_code = code;
        coefs.intent_params = [vxs[0] as f32, vxs[1] as f32, vxs[2] as f32];
    }
    coefs.setdims(ksp[0] as f32, ksp[1] as f32, ksp[2] as f32);
    let mut q = Mat::eye(4);
    let fsz = field.field_size();
    q.set(0, 3, f64::from(fsz[0]));
    q.set(1, 3, f64::from(fsz[1]));
    q.set(2, 3, f64::from(fsz[2]));
    coefs.qform = q;
    coefs.qform_code = nifti::XFORM_SCANNER_ANAT;
    for k in 0..csz[2] {
        for j in 0..csz[1] {
            for i in 0..csz[0] {
                let v = field.get_coef3(i, j, k)? as f32;
                coefs.set(i64::from(i), i64::from(j), i64::from(k), v);
            }
        }
    }
    Ok(coefs)
}

/// `TopupFileWriter(fname, ref, field, sf)`'s image: `ref` with the field (Hz) in it, the
/// intent `FSL_TOPUP_FIELD` with `ref`'s intent parameters shifted by one (FSL passes
/// `intent_param(0)`, which is 0, `intent_param(1)` and `intent_param(2)`), no display range,
/// scaled by `sf` unless it is 1.
pub fn field_volume(rf: &Volume, field: &mut SplineField, sf: f64) -> Result<Volume, FslError> {
    let fsz = field.field_size();
    if rf.xsize() != i64::from(fsz[0])
        || rf.ysize() != i64::from(fsz[1])
        || rf.zsize() != i64::from(fsz[2])
    {
        return Err(fileio_error(
            "TopupFileWriter::TopupFileWriter: Ref scan and field has different matrix size",
        ));
    }
    let vxs = field.voxel_size();
    if f64::from(rf.xdim()) != vxs[0]
        || f64::from(rf.ydim()) != vxs[1]
        || f64::from(rf.zdim()) != vxs[2]
    {
        return Err(fileio_error(
            "TopupFileWriter::TopupFileWriter: Ref scan and field has different voxel size",
        ));
    }
    let mut out = rf.clone();
    field.as_volume(&mut out, FieldIndex::Field)?;
    let p = out.intent_params;
    out.intent_code = TOPUP_FIELD;
    out.intent_params = [0.0, p[0], p[1]];
    out.display_max = 0.0;
    out.display_min = 0.0;
    if sf != 1.0 {
        crate::vol::scale(&mut out, sf as f32);
    }
    Ok(out)
}
