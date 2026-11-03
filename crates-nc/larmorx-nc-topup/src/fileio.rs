// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-topup 2203.5: topup_file_io.{h,cpp},
// topup_matrices.{h,cpp}; fsl-miscmaths 2412.6: write_ascii_matrix). Copyright (C) 2012
// University of Oxford (Jesper Andersson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! topup's files: the acquisition-parameter file (`TopupDatafileReader`), the spline
//! coefficients, the field and the movement parameters (`TopupFileWriter`), and the
//! movement-parameter matrices (`MovePar2Matrix`, `Matrix2MovePar`).

use larmorx_core::defects;
use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::basisfield::{FieldIndex, SplineField};
use larmorx_nc_core::cnum;
use larmorx_nc_core::miscmaths::{self, MatrixFileError};
use larmorx_nc_core::nifti;
use larmorx_nc_core::volume::{self, FslError, Volume};

use crate::Console;

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
pub fn field_volume(
    rf: &Volume,
    field: &mut SplineField,
    sf: f64,
    con: &Console,
) -> Result<Volume, FslError> {
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
    // FSL-119 (rule A): FSL passes intent_param(0), (1), (2) of newimage's 1-based parameters
    let shifted = [0.0, p[0], p[1]];
    if shifted != p {
        con.notice(
            &defects::FSL_119,
            if con.reproduce() {
                "the field's intent parameters shifted by one place"
            } else {
                "the field's intent parameters as the input's"
            },
        );
    }
    out.intent_params = if con.reproduce() { shifted } else { p };
    out.display_max = 0.0;
    out.display_min = 0.0;
    if sf != 1.0 {
        crate::vol::scale(&mut out, sf as f32);
    }
    Ok(out)
}

/// `(unsigned int) x` for a `double` on x86-64 (`cvttsd2si` to 64 bits, the low 32 kept).
fn d2u(x: f64) -> u32 {
    let wide = if x.is_nan() || x >= 9_223_372_036_854_775_808.0 || x < -9_223_372_036_854_775_808.0
    {
        i64::MIN
    } else {
        x as i64
    };
    wide as u32
}

/// `FieldFileType`: what `--topup` named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldFileType {
    /// Spline coefficients (`<base>_fieldcoef`, cubic or quadratic).
    Spline,
    /// A field image topup wrote (`--fout`: intent `FSL_TOPUP_FIELD`).
    Field,
    /// Any other image, taken as a field in Hz (fugue's, for example).
    Unknown,
    Invalid,
}

/// `TopupFileReader`: the field (spline coefficients, or a field image) and the movements of a
/// topup run.
#[derive(Clone, Debug)]
pub struct TopupFileReader {
    pub ft: FieldFileType,
    mp_valid: bool,
    mp: Vec<[f64; 6]>,
    moves: Vec<Mat>,
    coef: Option<SplineField>,
    vol: Option<Volume>,
    /// The coefficients may be laid with the quadratic B-splines' full count (FSL-145
    /// corrected: topup's default writes it); FSL's count only when `false`.
    general: bool,
    /// FSL-145: a coefficient file laid with the full count on a short axis was read (FSL
    /// refuses it as internally inconsistent).
    pub fsl145: bool,
}

impl TopupFileReader {
    /// `TopupFileReader(fname)`: `<fname>_fieldcoef` and `<fname>_movpar.txt`; if either fails,
    /// `fname` itself as a field image. A name with an extension is refused. The lines FSL
    /// prints on the way (a movement file it cannot open, exceptions it catches) go to `cerr`.
    /// With `general` (D13's default), a quadratic coefficient file laid with the full count
    /// on an axis shorter than two knot spacings (FSL-145) is read too; FSL's count is always.
    pub fn read(
        fname: &str,
        fsl: &larmorx_nc_core::settings::Settings,
        general: bool,
        cerr: &mut Vec<String>,
    ) -> Result<TopupFileReader, FslError> {
        if let Some(dot) = fname.rfind('.') {
            let refuse = match crate::options::last_separator(fname) {
                None => true,
                Some(slash) => dot > slash,
            };
            if refuse {
                return Err(fileio_error(&format!(
                    "TopupFileReader::TopupFileReader: Filename must be given without \
                     extension, fname={fname}"
                )));
            }
        }
        let mut r = TopupFileReader {
            ft: FieldFileType::Invalid,
            mp_valid: false,
            mp: Vec::new(),
            moves: Vec::new(),
            coef: None,
            vol: None,
            general,
            fsl145: false,
        };
        let first = r
            .common_read(&format!("{fname}_fieldcoef"), fsl, cerr)
            .and_then(|()| r.read_movements(&format!("{fname}_movpar.txt"), cerr));
        if let Err(e) = first {
            // catch (...): what the exception printed stays printed
            cerr.extend(e.printed);
            if let Err(e2) = r.common_read(fname, fsl, cerr) {
                cerr.extend(e2.printed);
                return Err(fileio_error(&format!(
                    "TopupFileReader::TopupFileReader: Unable to read file {fname}"
                )));
            }
        }
        Ok(r)
    }

    /// `common_read(fname)`: by the image's intent, spline coefficients (read in the file's
    /// storage order) or a field image.
    fn common_read(
        &mut self,
        fname: &str,
        fsl: &larmorx_nc_core::settings::Settings,
        cerr: &mut Vec<String>,
    ) -> Result<(), FslError> {
        if volume::return_validimagefilename(fname).is_err() {
            return Err(fileio_error(&format!(
                "TOPUP::common_read: Cannot read file {fname}"
            )));
        }
        let hdr = volume::read_volume_hdr_only(fname, fsl)?;
        match hdr.intent_code {
            CUBIC_SPLINE_COEFFICIENTS | QUADRATIC_SPLINE_COEFFICIENTS => {
                let (v, _, _) = volume::read_orig_volume::<f32>(fname, fsl)?;
                let (field, fsl145) = read_coef_file(&v, self.general)?;
                self.coef = Some(field);
                self.fsl145 = fsl145;
                self.vol = None;
                self.ft = FieldFileType::Spline;
            }
            code => {
                let (v, warning) = volume::read_volume::<f32>(fname, fsl)?;
                if let Some(w) = warning {
                    cerr.push(w);
                }
                self.vol = Some(v);
                self.coef = None;
                self.ft = if code == TOPUP_FIELD {
                    FieldFileType::Field
                } else {
                    FieldFileType::Unknown
                };
            }
        }
        Ok(())
    }

    /// `ReadMovements(fname)`: six parameters per row (rigid-body matrices about the field's
    /// centre), or 4 x 4 matrices (four columns, a multiple of four rows: the matrices, but
    /// not valid movement parameters); anything else reads nothing.
    fn read_movements(&mut self, fname: &str, cerr: &mut Vec<String>) -> Result<(), FslError> {
        let vol = if let Some(v) = &self.vol {
            v.clone()
        } else if let Some(c) = &self.coef {
            let fs = c.field_size();
            let mut t = Volume::new(i64::from(fs[0]), i64::from(fs[1]), i64::from(fs[2]), 1);
            t.setdims(c.vxs_x() as f32, c.vxs_y() as f32, c.vxs_z() as f32);
            t
        } else {
            return Err(fileio_error(
                "TopupFileReader::ReadMovements: Attempting to read movement parameters before \
                 knowing FOV",
            ));
        };
        let tmp = read_matrix(fname, cerr)?;
        if tmp.cols() == 6 {
            self.moves.clear();
            self.mp.clear();
            for i in 0..tmp.rows() {
                let p = [
                    tmp.at(i, 0),
                    tmp.at(i, 1),
                    tmp.at(i, 2),
                    tmp.at(i, 3),
                    tmp.at(i, 4),
                    tmp.at(i, 5),
                ];
                self.mp.push(p);
                self.moves.push(movepar2matrix(&p, &vol));
                self.mp_valid = true;
            }
        } else if tmp.cols() == 4 && tmp.rows() % 4 == 0 {
            self.moves.clear();
            for i in 0..tmp.rows() / 4 {
                self.moves.push(tmp.sub(i * 4, i * 4 + 4, 0, 4));
                self.mp_valid = false;
            }
        }
        Ok(())
    }

    /// `FieldAsVolume()`: the field (Hz) on its grid (from the coefficients: a new volume of the
    /// field's size and voxel sizes, no other header property).
    pub fn field_as_volume(&mut self) -> Result<Volume, FslError> {
        if self.vol.is_none() {
            let Some(c) = self.coef.as_mut() else {
                return Err(fileio_error(
                    "TopupFileReader::ensure_volume: Attempt to access uninitialised field",
                ));
            };
            let fs = c.field_size();
            let mut v = Volume::new(i64::from(fs[0]), i64::from(fs[1]), i64::from(fs[2]), 1);
            v.setdims(c.vxs_x() as f32, c.vxs_y() as f32, c.vxs_z() as f32);
            c.as_volume(&mut v, FieldIndex::Field)?;
            self.vol = Some(v);
        }
        Ok(self.vol.clone().expect("set"))
    }

    /// `Field()`: the spline field; a field image is not one (an exception).
    pub fn field(&self) -> Result<SplineField, FslError> {
        self.coef.clone().ok_or_else(|| {
            fileio_error("TopupFileReader::ensure_field: Attempt to access uninitialised field")
        })
    }

    /// `MoveMatrix(i)` (1-based).
    pub fn move_matrix(&self, i: u32) -> Result<Mat, FslError> {
        if i < 1 || i as usize > self.moves.len() {
            return Err(fileio_error(
                "TopupFileReader::MoveMatrix: Index i out of bound",
            ));
        }
        Ok(self.moves[i as usize - 1].clone())
    }

    /// `MoveParValid()`.
    pub fn move_par_valid(&self) -> bool {
        self.mp_valid
    }

    /// Whether any movement matrix was read.
    pub fn has_moves(&self) -> bool {
        !self.moves.is_empty()
    }
}

/// `read_coef_file(vcoef)`: the field's size from the qform's translation, its voxel sizes
/// from the intent parameters, the knot spacings from the voxel sizes (rounded), the order
/// from the intent. With `general`, a file laid with the quadratic B-splines' full count on a
/// short axis is accepted as well as FSL's layout (FSL-145; the second value says that such a
/// file, which FSL refuses, was read).
fn read_coef_file(v: &Volume, general: bool) -> Result<(SplineField, bool), FslError> {
    let q = &v.qform;
    let sz = [d2u(q.at(0, 3)), d2u(q.at(1, 3)), d2u(q.at(2, 3))];
    let vxs = [
        f64::from(v.intent_params[0]),
        f64::from(v.intent_params[1]),
        f64::from(v.intent_params[2]),
    ];
    let order = if v.intent_code == QUADRATIC_SPLINE_COEFFICIENTS {
        2
    } else {
        3
    };
    let ksp = [
        d2u(f64::from(v.xdim()) + 0.5),
        d2u(f64::from(v.ydim()) + 0.5),
        d2u(f64::from(v.zdim()) + 0.5),
    ];
    let fits = |f: &SplineField| {
        let cs = f.coef_size();
        i64::from(cs[0]) == v.xsize()
            && i64::from(cs[1]) == v.ysize()
            && i64::from(cs[2]) == v.zsize()
    };
    let mut field = SplineField::new(&sz, &vxs, &ksp, order, 1)?;
    let mut fsl145 = false;
    if !fits(&field) && general {
        let full = SplineField::new_counted(&sz, &vxs, &ksp, order, 1, true)?;
        if fits(&full) {
            field = full;
            fsl145 = true;
        }
    }
    if !fits(&field) {
        return Err(fileio_error(
            "read_coef_file: Internally inconsistent coef-file",
        ));
    }
    let c: Vec<f64> = v.data.iter().map(|&x| f64::from(x)).collect();
    field.set_coef(&c)?;
    Ok((field, fsl145))
}
