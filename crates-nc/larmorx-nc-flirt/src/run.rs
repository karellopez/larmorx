// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (flirt.cc). Copyright (C) 1999-2023
// University of Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the original. Distributed under the FSL Licence (see LICENSE).
//! FLIRT's `main`: reading the volumes, the multi-resolution pyramid, the schedule (the search,
//! the optimisations, the matrix variables), the final matrix and the resampled image, and
//! `-applyxfm`. Function names follow `flirt.cc`.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::bbr::{self, BbrType, FieldMap};
use larmorx_nc_core::cnum;
use larmorx_nc_core::costfns::CostType;
use larmorx_nc_core::interp::round_f32;
use larmorx_nc_core::kernel;
use larmorx_nc_core::miscmaths::{self, MatrixFileError};
use larmorx_nc_core::newimagefns::{self, isotropic_resample};
use larmorx_nc_core::nifti::{self, dt};
use larmorx_nc_core::optimise::{self, OptEvent};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{
    self, Conversion, Extrapolation, FslError, Interpolation, SaveError, SaveOptions, SaveType,
    Volume,
};

use crate::options::{AngleRep, EXIT_MINUS_ONE, Interp, Options, Parsed, Window, parse};
use crate::pair::{BinsFailure, CostMessages, Pair, RefSlot};
use crate::schedule::{self, BadName, MatName, Scalars};

/// The `descrip` field of the images the replica writes (FLIRT writes FSL's build string).
pub const DESCRIP: &[u8] = b"larmorx-nc fsl flirt";

/// How a run stops before `main` returns normally.
#[derive(Debug)]
pub enum Stop {
    /// `exit(code)` (the messages already printed).
    Exit(u8),
    /// A C++ exception, caught by `main`'s `catch (std::exception& e)`: FSL prints what it
    /// printed when throwing, then `e.what()`, and returns 0.
    Exception(FslError),
    /// The C library aborts (signal 6) after printing this; the replica prints its own line
    /// after it and exits 1.
    Abort(String),
    /// A crash FSL has no message for (signal 8 or 11); the replica names it and exits 1.
    Crash(String),
    /// larmorx refuses (undefined behaviour of the original): exit 1 with this message.
    Refuse(String),
}

impl From<FslError> for Stop {
    fn from(e: FslError) -> Stop {
        if e.type_name.is_empty() {
            Stop::Abort(e.printed_text())
        } else {
            Stop::Exception(e)
        }
    }
}

type R<T> = Result<T, Stop>;

/// A row of a schedule matrix (17 numbers: the cost, then the 4 x 4 matrix column by column).
type Row = Vec<f64>;

/// The filters of `filter_image` and `filter_weight`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Filter {
    SubsampleBy2,
    Blur,
    ResampBlur,
}

/// The state of `std::cout` that matters: armawrap's matrix printing switches it to fixed
/// notation for good.
struct Cout<'a> {
    w: &'a mut dyn Write,
    fixed: bool,
}

impl Cout<'_> {
    fn s(&mut self, t: &str) {
        let _ = self.w.write_all(t.as_bytes());
    }

    fn num(&self, v: f64) -> String {
        if self.fixed {
            cnum::fmt_f(v, 6)
        } else {
            cnum::stream(v)
        }
    }

    /// `cout << matrix` (armawrap's `doPrint`): each element in fixed notation with 6 decimals and
    /// a blank, a newline per row; the stream stays fixed.
    fn mat(&mut self, m: &Mat) {
        self.fixed = true;
        let mut t = String::new();
        for r in 0..m.rows() {
            for c in 0..m.cols() {
                t.push_str(&cnum::fmt_f(m.at(r, c), 6));
                t.push(' ');
            }
            t.push('\n');
        }
        self.s(&t);
    }

    fn row(&mut self, v: &[f64]) {
        self.mat(&Mat::from_rows(1, v.len(), v));
    }
}

/// The run: FLIRT's `globaloptions` and global volumes, and `main`'s volumes.
pub struct Flirt<'a> {
    pub o: Options,
    mode: UpstreamDefects,
    settings: &'a Settings,
    threads: usize,
    out: Cout<'a>,
    err: &'a mut dyn Write,
    notices: Vec<Notice>,
    /// `construct_rotmat_quat`'s messages not yet written (it prints on standard error from
    /// inside the cost function).
    quat_msgs: std::cell::RefCell<Vec<String>>,
    // globaloptions (beyond the command line)
    usrmat: Vec<Vec<Row>>,
    searchoptmat: Vec<Row>,
    preoptsearchmat: Vec<Row>,
    initmat: Mat,
    impair: Option<Pair>,
    refparams: Mat,
    parammask: Mat,
    usrsubset: bool,
    searchdof: i32,
    currentcostfn: CostType,
    optimisationtype: String,
    lastsampling: f32,
    requestedscale: f32,
    smoothsize: f32,
    fuzzyfrac: f32,
    tolerance: Vec<f64>,
    boundguess: Vec<f64>,
    // flirt.cc's globals
    global_refweight: Volume,
    global_testweight: Volume,
    global_refweight1: Volume,
    global_refweight2: Volume,
    global_refweight4: Volume,
    global_refweight8: Volume,
    global_seg: Volume,
    global_init_testvol: Volume,
    global_init_testweight: Volume,
    global_fmap: Volume,
    global_fmap_mask: Volume,
    global_coords: Mat,
    global_norms: Mat,
    global_scale1ok: bool,
    read_testvol: bool,
    global_sampling: f32,
    // main's volumes (the pair refers to them)
    refvol: Volume,
    testvol: Volume,
    refvol_2: Volume,
    refvol_4: Volume,
    refvol_8: Volume,
}

/// An empty `volume<float>` (FSL's default-constructed volume).
fn empty() -> Volume {
    Volume::new(0, 0, 0, 0)
}

/// `MISCMATHS::Min`/`Max` of floats.
fn fmin(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}
fn fmax(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// `reshape(r, m, 4, 4)` of a row's 16 matrix numbers (column by column).
fn row_to_mat(row: &[f64]) -> Mat {
    Mat::from_col_major(4, 4, &row[1..17])
}

/// `reshape(reshaped, mat, 1, 16)`: the matrix column by column.
fn mat_to_16(m: &Mat) -> Vec<f64> {
    let mut v = Vec::with_capacity(16);
    for c in 0..4 {
        for r in 0..4 {
            v.push(m.at(r, c));
        }
    }
    v
}

/// `pinv(parammask)` for the masks FLIRT builds and its shipped schedules give (only 0 and 1,
/// no row with two non-zeros): a column of `k` ones gives `(1/sqrt(k))^2` at its rows, as
/// Armadillo's divide-and-conquer SVD on FSL's LAPACK gives it (`validation/probes/fsl-flirt-pinv`).
/// The flag is true for the masks whose bits the probe confirms: columns of single ones, the
/// search's 12 x 4 mask (one scale on rows 7 to 9, then the translations) and FSL's
/// `sch2D_4dof` (rz, tx, ty, one scale on rows 7 and 8). Other masks: the normal-equations
/// pseudo-inverse, not FSL's last bits (false; `setoption paramsubset` refuses them).
fn pinv_mask(m: &Mat) -> (Mat, bool) {
    let (rows, cols) = (m.rows(), m.cols());
    let disjoint = (0..rows).all(|r| (0..cols).filter(|&c| m.at(r, c) != 0.0).count() <= 1)
        && (0..rows).all(|r| (0..cols).all(|c| m.at(r, c) == 0.0 || m.at(r, c) == 1.0));
    let ones = |c: usize| (0..rows).filter(|&r| m.at(r, c) == 1.0).collect::<Vec<_>>();
    let confirmed = disjoint
        && ((0..cols).all(|c| ones(c).len() == 1)
            || (rows == 12
                && cols == 4
                && ones(0) == [6, 7, 8]
                && ones(1) == [3]
                && ones(2) == [4]
                && ones(3) == [5])
            || (rows == 12
                && cols == 4
                && ones(0) == [2]
                && ones(1) == [3]
                && ones(2) == [4]
                && ones(3) == [6, 7]));
    let mut out = Mat::zeros(cols, rows);
    if disjoint {
        for c in 0..cols {
            let k = (0..rows).filter(|&r| m.at(r, c) == 1.0).count();
            let v = if k == 1 {
                1.0
            } else {
                let r = 1.0 / (k as f64).sqrt();
                r * r
            };
            for r in 0..rows {
                if m.at(r, c) == 1.0 {
                    out.set(c, r, v);
                }
            }
        }
        return (out, confirmed);
    }
    // (M^T M)^-1 M^T by Gauss-Jordan on the small normal matrix.
    let mut a = vec![vec![0.0f64; 2 * cols]; cols];
    for i in 0..cols {
        for j in 0..cols {
            a[i][j] = (0..rows).map(|r| m.at(r, i) * m.at(r, j)).sum();
        }
        a[i][cols + i] = 1.0;
    }
    for i in 0..cols {
        let p = (i..cols)
            .max_by(|&x, &y| a[x][i].abs().total_cmp(&a[y][i].abs()))
            .unwrap_or(i);
        a.swap(i, p);
        let d = a[i][i];
        if d == 0.0 {
            continue;
        }
        for v in a[i].iter_mut() {
            *v /= d;
        }
        for j in 0..cols {
            if j != i {
                let f = a[j][i];
                for k in 0..2 * cols {
                    a[j][k] -= f * a[i][k];
                }
            }
        }
    }
    for i in 0..cols {
        for r in 0..rows {
            out.set(i, r, (0..cols).map(|j| a[i][cols + j] * m.at(r, j)).sum());
        }
    }
    (out, false)
}

/// `A * x` with the products rounded and summed column by column (OpenBLAS's `dgemv_n` gives
/// these bits for masks with at most one non-zero per row, and for their pseudo-inverses).
fn mat_vec(a: &Mat, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0f64; a.rows()];
    for c in 0..a.cols() {
        for r in 0..a.rows() {
            y[r] += a.at(r, c) * x[c];
        }
    }
    y
}

impl<'a> Flirt<'a> {
    fn new(
        o: Options,
        mode: UpstreamDefects,
        settings: &'a Settings,
        threads: usize,
        out: &'a mut dyn Write,
        err: &'a mut dyn Write,
    ) -> Flirt<'a> {
        let currentcostfn = CostType::CorrRatio;
        Flirt {
            o,
            mode,
            settings,
            threads,
            out: Cout {
                w: out,
                fixed: false,
            },
            err,
            notices: Vec::new(),
            quat_msgs: std::cell::RefCell::new(Vec::new()),
            usrmat: vec![Vec::new(); 27],
            searchoptmat: Vec::new(),
            preoptsearchmat: Vec::new(),
            initmat: Mat::eye(4),
            impair: None,
            refparams: Mat::col(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]),
            parammask: Mat::eye(12),
            usrsubset: false,
            searchdof: 12,
            currentcostfn,
            optimisationtype: "brent".to_owned(),
            lastsampling: 8.0,
            requestedscale: 1.0,
            smoothsize: 1.0,
            fuzzyfrac: 0.5,
            tolerance: vec![
                0.005, 0.005, 0.005, 0.2, 0.2, 0.2, 0.002, 0.002, 0.002, 0.001, 0.001, 0.001,
            ],
            boundguess: vec![10.0, 1.0],
            global_refweight: empty(),
            global_testweight: empty(),
            global_refweight1: empty(),
            global_refweight2: empty(),
            global_refweight4: empty(),
            global_refweight8: empty(),
            global_seg: empty(),
            global_init_testvol: empty(),
            global_init_testweight: empty(),
            global_fmap: empty(),
            global_fmap_mask: empty(),
            global_coords: Mat::zeros(0, 0),
            global_norms: Mat::zeros(0, 0),
            global_scale1ok: true,
            read_testvol: false,
            global_sampling: 1.0,
            refvol: empty(),
            testvol: empty(),
            refvol_2: empty(),
            refvol_4: empty(),
            refvol_8: empty(),
        }
    }

    fn e(&mut self, t: &str) {
        self.flush_quat_msgs();
        let _ = self.err.write_all(t.as_bytes());
    }

    fn el(&mut self, t: &str) {
        self.e(t);
        self.e("\n");
    }

    fn notice(&mut self, defect: &'static defects::Defect, detail: impl Into<String>) {
        defects::push_once(&mut self.notices, Notice::new(defect, self.mode, detail));
    }

    fn pair(&self) -> &Pair {
        self.impair.as_ref().expect("the image pair is set")
    }

    fn slot_vol(&self, s: RefSlot) -> &Volume {
        match s {
            RefSlot::Ref1 => &self.refvol,
            RefSlot::Ref2 => &self.refvol_2,
            RefSlot::Ref4 => &self.refvol_4,
            RefSlot::Ref8 => &self.refvol_8,
        }
    }

    // --------------------------------------------------------------------------------------
    // Reading

    /// `FLIRT_read_volume(target, filename)`: the volume (radiological), its voxel sizes divided
    /// by the base scale when it is not 1.
    fn read_volume(&mut self, filename: &str) -> R<Volume> {
        let (mut v, warning) = volume::read_volume::<f32>(filename, self.settings)?;
        if let Some(w) = warning {
            self.el(&w);
        }
        self.apply_basescale(&mut v);
        Ok(v)
    }

    fn apply_basescale(&self, v: &mut Volume) {
        let b = self.o.basescale;
        if (f64::from(b) - 1.0).abs() > 1e-5 {
            v.setdims(v.xdim() / b, v.ydim() / b, v.zdim() / b);
        }
    }

    /// `set_basescale(filenameA, filenameB)`: unless `-basescale`, voxels above 12 mm make the
    /// base scale the largest size over 8, and voxels below 0.75 mm the smallest size.
    fn set_basescale(&mut self) -> R<()> {
        if self.o.force_basescale {
            return Ok(());
        }
        let dims = |name: &str| -> R<[f32; 3]> {
            // read_volume_hdr_only: the name's own exception (printed when thrown), caught and
            // thrown again as #22.
            let valid = volume::return_validimagefilename(name).map_err(|e| {
                let mut again = FslError::imthrow(
                    format!("Failed to read volume {name}\nError : {}", e.what),
                    22,
                );
                let mut printed = e.printed.clone();
                printed.append(&mut again.printed);
                again.printed = printed;
                again
            })?;
            let h = nifti::load_header(std::path::Path::new(&valid), &self.settings.open_options())
                .map_err(|e| {
                    FslError::imthrow(format!("Failed to read volume {name}\nError : {}", e.0), 22)
                })?;
            Ok([
                (h.pixdim[1] as f32).abs(),
                (h.pixdim[2] as f32).abs(),
                (h.pixdim[3] as f32).abs(),
            ])
        };
        let a = dims(&self.o.reffname.clone())?;
        let b = dims(&self.o.inputfname.clone())?;
        let maxa = fmax(fmax(a[0], a[1]), a[2]);
        let maxb = fmax(fmax(b[0], b[1]), b[2]);
        if fmax(maxa, maxb) > 12.0 {
            self.o.basescale = fmax(maxa, maxb) / 8.0;
        }
        let mina = fmin(fmin(a[0], a[1]), a[2]);
        let minb = fmin(fmin(b[0], b[1]), b[2]);
        if f64::from(fmin(mina, minb)) < 0.75 {
            self.o.basescale = fmin(mina, minb);
        }
        Ok(())
    }

    /// `get_testvol(testvol)`.
    fn get_testvol(&mut self) -> R<()> {
        let mut dtype: i16 = 0;
        let (mut minval, mut maxval) = (0.0f32, 0.0f32);
        if !self.read_testvol {
            let mut tv = self.read_volume(&self.o.inputfname.clone())?;
            dtype = volume::file_dtype(&self.o.inputfname, self.settings)?;
            if !self.o.forcedatatype {
                self.o.datatype = dtype;
            }
            if tv.zsize() == 1 {
                double_end_slices(&mut tv);
            }
            (minval, maxval) = tv.robust_limits();
            if self.o.clamping {
                newimagefns::clamp(&mut tv, minval, maxval);
            }
            if self.o.useweights {
                if !self.o.testweightfname.is_empty() {
                    let mut w = self.read_volume(&self.o.testweightfname.clone())?;
                    if w.zsize() == 1 {
                        double_end_slices(&mut w);
                    }
                    self.global_testweight = w;
                } else {
                    let mut w = tv.clone();
                    w.data.iter_mut().for_each(|v| *v = 1.0);
                    self.global_testweight = w;
                }
            }
            self.global_init_testvol = tv.clone();
            self.global_init_testweight = self.global_testweight.clone();
            self.read_testvol = true;
            self.testvol = tv;
        } else {
            self.testvol = self.global_init_testvol.clone();
            self.global_testweight = self.global_init_testweight.clone();
        }
        if self.o.verbose >= 2 {
            let m = self.testvol.sampling_mat();
            self.out.s("Testvol sampling matrix =\n");
            self.out.mat(&m);
            self.out.s("\n");
            self.out.s(&format!("Testvol Data Type = {dtype}\n"));
            self.out.s("Testvol intensity ");
            if self.o.clamping {
                let t = format!(
                    "clamped between {} and {}\n",
                    self.out.num(f64::from(minval)),
                    self.out.num(f64::from(maxval))
                );
                self.out.s(&t);
            } else {
                let (mx, mn) = self.testvol.extrema();
                let t = format!(
                    "between {} and {}\n",
                    self.out.num(f64::from(mn)),
                    self.out.num(f64::from(mx))
                );
                self.out.s(&t);
            }
        }
        Ok(())
    }

    /// `get_refvol(refvol)`: the reference (and its weight, the field map, the segmentation or
    /// the boundary coordinates). Returns the volume; the globals are set.
    fn get_refvol(&mut self) -> R<Volume> {
        let mut rv = self.read_volume(&self.o.reffname.clone())?;
        if rv.zsize() == 1 && self.o.do_optimise {
            double_end_slices(&mut rv);
        }
        let (minval, maxval) = rv.robust_limits();
        if self.o.clamping {
            newimagefns::clamp(&mut rv, minval, maxval);
        }
        if self.o.useweights {
            if !self.o.refweightfname.is_empty() {
                let mut w = self.read_volume(&self.o.refweightfname.clone())?;
                if w.zsize() == 1 {
                    double_end_slices(&mut w);
                }
                self.global_refweight = w;
            } else {
                let mut w = rv.clone();
                w.data.iter_mut().for_each(|v| *v = 1.0);
                self.global_refweight = w;
            }
        }
        if !self.o.fmapfname.is_empty() {
            self.global_fmap = self.read_volume(&self.o.fmapfname.clone())?;
            if !self.o.fmapmaskfname.is_empty() {
                self.global_fmap_mask = self.read_volume(&self.o.fmapmaskfname.clone())?;
            } else {
                let mut m = self.global_fmap.clone();
                m.data.iter_mut().for_each(|v| *v = *v * 0.0 + 1.0);
                self.global_fmap_mask = m;
            }
        }
        if self.o.useseg {
            if !self.o.usecoords {
                let mut s = self.read_volume(&self.o.wmsegfname.clone())?;
                if s.zsize() == 1 {
                    double_end_slices(&mut s);
                }
                self.global_seg = s;
            } else {
                self.global_coords = self.read_matrix(&self.o.wmcoordsfname.clone())?;
                self.global_norms = self.read_matrix(&self.o.wmnormsfname.clone())?;
                if self.global_coords.rows() == 0
                    || self.global_norms.rows() == 0
                    || self.global_coords.rows() != self.global_norms.rows()
                {
                    let t = format!(
                        "Coordinate and Normal matrices {} and {} are either zero size or different sizes",
                        self.o.wmcoordsfname, self.o.wmnormsfname
                    );
                    self.el(&t);
                    return Err(Stop::Exit(1));
                }
            }
        }
        if self.o.verbose >= 2 {
            self.out.s("Refvol intensity ");
            if self.o.clamping {
                let t = format!(
                    "clamped between {} and {}\n",
                    self.out.num(f64::from(minval)),
                    self.out.num(f64::from(maxval))
                );
                self.out.s(&t);
            } else {
                let (mx, mn) = rv.extrema();
                let t = format!(
                    "between {} and {}\n",
                    self.out.num(f64::from(mn)),
                    self.out.num(f64::from(mx))
                );
                self.out.s(&t);
            }
        }
        Ok(rv)
    }

    /// `read_ascii_matrix(filename)` with FSL's message for a file it cannot open (an empty
    /// matrix) and its exception for one without enough numbers.
    fn read_matrix(&mut self, filename: &str) -> R<Mat> {
        match miscmaths::read_ascii_matrix(filename) {
            Ok(m) => Ok(m),
            Err(MatrixFileError::CannotOpen) => {
                self.el(&format!("Could not open matrix file {filename}"));
                Ok(Mat::zeros(0, 0))
            }
            Err(MatrixFileError::NotEnoughNumbers) => Err(Stop::Exception(FslError {
                printed: Vec::new(),
                type_name: "armawrap::AWException",
                what: "Could not find enough numbers in matrix file".to_owned(),
            })),
        }
    }

    // --------------------------------------------------------------------------------------
    // Matrices

    /// `scalemat(mat)`: `S * mat * S.i()` with `S = diag(1/b, 1/b, 1/b, 1)`.
    fn scalemat(&self, mat: &Mat) -> R<Mat> {
        // `1/basescale`: an int over a float, a single-precision quotient (0.01's is exactly 100)
        let inv_b = f64::from(larmorx_nc_core::sse::div_f32(1.0, self.o.basescale));
        let mut s = Mat::eye(4);
        for k in 0..3 {
            s.set(k, k, inv_b);
        }
        let si = arma::inv(&s).map_err(|e| singular(&e.to_string()))?;
        check_mul(&s, mat)?;
        check_mul(mat, &si)?;
        Ok(arma::mul3(&s, mat, &si))
    }

    /// `qsform_init_mat(refvol, testvol)`: the voxel-to-voxel map the sform/qform give, in FLIRT
    /// millimetres; identity unless both volumes have one.
    fn qsform_init_mat(&self, refvol: &Volume, testvol: &Volume) -> R<Mat> {
        if refvol.qform_code + refvol.sform_code > 0 && testvol.qform_code + testvol.sform_code > 0
        {
            let rinv =
                arma::inv(&newimagevox2mm_mat(refvol)).map_err(|e| singular(&e.to_string()))?;
            let tinv = arma::inv(&testvol.sampling_mat()).map_err(|e| singular(&e.to_string()))?;
            let m = arma::mul4(
                &refvol.sampling_mat(),
                &rinv,
                &newimagevox2mm_mat(testvol),
                &tinv,
            );
            return self.scalemat(&m);
        }
        Ok(Mat::eye(4))
    }

    /// `set_initmat(refvol, testvol)`.
    fn set_initmat(&mut self, refvol: &Volume, testvol: &Volume) -> R<()> {
        if !self.o.initmatfname.is_empty() {
            self.initmat = self.read_matrix(&self.o.initmatfname.clone())?;
        } else if self.o.initmatsqform {
            self.initmat = self.qsform_init_mat(refvol, testvol)?;
        }
        self.initmat = self.scalemat(&self.initmat.clone())?;
        if self.o.verbose >= 2 {
            self.out.s("Init Matrix = \n");
            let m = self.initmat.clone();
            self.out.mat(&m);
            self.out.s("\n");
        }
        Ok(())
    }

    /// `vector2affine(params, n, centre, aff)`: Euler angles, or with `-anglerep quaternion`
    /// a quaternion's last three components (whose messages FSL prints at every call).
    fn vector2affine_c(&self, params: &Mat, n: i32, centre: &[f64; 3]) -> Mat {
        let mut p = params.clone();
        if p.len() < 12 {
            let mut v = p.as_slice().to_vec();
            v.resize(12, 0.0);
            p = Mat::col(&v);
        }
        match self.o.anglerep {
            AngleRep::Euler => miscmaths::compose_aff(&p, n, &Mat::col(centre)),
            AngleRep::Quaternion => {
                let mut msgs = Vec::new();
                let a = miscmaths::compose_aff_quat(&p, n, &Mat::col(centre), &mut msgs);
                if !msgs.is_empty() {
                    let mut m = self.quat_msgs.borrow_mut();
                    m.extend(msgs);
                }
                a
            }
        }
    }

    /// The quaternion's messages collected since the last write, written to standard error
    /// first (before anything else written there: FSL prints them as they arise).
    fn flush_quat_msgs(&mut self) {
        let msgs: Vec<String> = self.quat_msgs.borrow_mut().drain(..).collect();
        for m in msgs {
            let _ = self.err.write_all(m.as_bytes());
            let _ = self.err.write_all(b"\n");
        }
    }

    /// `vector2affine(params, n, aff)`: about the pair's test centre of mass.
    fn vector2affine(&self, params: &Mat, n: i32) -> Mat {
        let c = self.pair().test_cog;
        self.vector2affine_c(params, n, &c)
    }

    /// `affmat2vector(aff, n, params)`.
    fn affmat2vector(&self, aff: &Mat) -> Mat {
        let c = self.pair().test_cog;
        match self.o.anglerep {
            AngleRep::Euler => Mat::col(&miscmaths::decompose_aff(aff, &Mat::col(&c))),
            AngleRep::Quaternion => Mat::col(&miscmaths::decompose_aff_quat(aff, &Mat::col(&c))),
        }
    }

    /// `default_nonlin_params()`: the field map's scale, echo spacing times the test volume's
    /// size along the phase-encoding axis over 2 pi (negative for a reversed direction).
    fn default_nonlin(&self) -> f64 {
        nonlin_of(self.o.pe_dir, self.o.echo_spacing, &self.testvol)
    }

    // --------------------------------------------------------------------------------------
    // The cost function interfaces

    /// `setcostfntype(imagepair, ctype)`: the cost type, and BBR's points, field map, type and
    /// slope when it is BBR and its points are not set yet.
    fn setcostfntype(&mut self, ctype: CostType) -> R<()> {
        let Some(mut p) = self.impair.take() else {
            return Ok(());
        };
        let r = self.setcostfntype_on(&mut p, ctype);
        self.impair = Some(p);
        r
    }

    fn setcostfntype_on(&mut self, p: &mut Pair, ctype: CostType) -> R<()> {
        p.costtype = ctype;
        if p.costtype == CostType::Bbr && !p.is_bbr_set() {
            if self.o.debug {
                self.el("Setting bbr seg 1");
                self.el(&format!("Cost is set to {}", p.costtype.code()));
                self.el("Cost == BBR is 1");
                self.el(&format!(
                    "Result (pre) of is_bbr_set is {}",
                    i32::from(p.is_bbr_set())
                ));
            }
            if self.o.usecoords {
                match bbr::points_from_coords(&self.global_coords, &self.global_norms) {
                    Some(pts) => p.bbr_points = pts,
                    None => self.el(
                        "ERROR::set_bbr_coords: coords and norms are different sizes or zero size",
                    ),
                }
            } else {
                let rv = self.slot_vol(p.ref_slot);
                // FSL-67 (rule C, measured 2026-11-01): the gradient divided by the reference
                // level's voxel sizes, not the segmentation's, in both modes.
                let dims = [rv.xdim(), rv.ydim(), rv.zdim()];
                let (pts, found) = bbr::points_from_seg(&self.global_seg, dims, self.threads);
                p.bbr_points = pts;
                if !found {
                    self.el("ERROR::set_bbr_seg: could not find any boundary points!");
                }
            }
            p.bbr.fieldmap = Some(FieldMap {
                fmap: self.global_fmap.clone(),
                mask: self.global_fmap_mask.clone(),
                pe_dir: self.o.pe_dir,
            });
            p.bbr.kind = BbrType::parse(&self.o.bbr_type)?;
            p.bbr.slope = self.o.bbr_slope;
            if self.o.debug {
                self.el(&format!(
                    "Result (post) of is_bbr_set is {}",
                    i32::from(p.is_bbr_set())
                ));
            }
        }
        Ok(())
    }

    /// `setup_costfn(imagepair, curcostfn, no_bins, smoothsize, fuzzyfrac)`.
    fn setup_costfn(&mut self, p: &mut Pair, ctype: CostType, bins: i32) -> R<()> {
        self.setcostfntype_on(p, ctype)?;
        let rv = self.slot_vol(p.ref_slot);
        if let Err(f) = p.set_no_bins(rv, bins) {
            return Err(match f {
                BinsFailure::DivisionByZero => Stop::Crash(format!(
                    "flirt is killed by signal 8 (SIGFPE: an integer division by zero): {bins} histogram bins at this scale (FSL-66)"
                )),
                BinsFailure::NegativeLength => Stop::Exception(FslError {
                    printed: Vec::new(),
                    type_name: "std::bad_array_new_length",
                    what: "std::bad_array_new_length".to_owned(),
                }),
            });
        }
        p.smoothsize = self.smoothsize;
        p.fuzzyfrac = self.fuzzyfrac;
        p.skip_nonfinite = self.mode.fix();
        // FSL-14: named whenever the pair's volumes hold NaN or infinite values (they may change
        // the cost, so the matrix, whichever mode runs)
        let in_ref = self.slot_vol(p.ref_slot).data.iter().any(|v| !v.is_finite());
        let in_test = self.testvol.data.iter().any(|v| !v.is_finite());
        if in_ref || in_test {
            let which = match (in_ref, in_test) {
                (true, true) => "the reference and the input have",
                (true, false) => "the reference has",
                _ => "the input has",
            };
            let detail = if self.mode.fix() {
                format!(
                    "{which} NaN or infinite voxels: they are left out of the cost (in FLIRT one \
                     makes the cost NaN or infinite)"
                )
            } else {
                format!("{which} NaN or infinite voxels: as in FLIRT, they enter the cost")
            };
            self.notice(&defects::FSL_14, detail);
        }
        Ok(())
    }

    /// The pair's cost of `affmat` (initmat applied by the caller), with its messages.
    fn pair_cost(&mut self, affmat: &Mat, nonlin: Option<f64>) -> R<f32> {
        if let Some(p) = &self.impair
            && p.costtype == CostType::Bbr
            && p.bbr.step <= 0
            && !p.bbr_points.is_empty()
        {
            return Err(Stop::Refuse(format!(
                "setoption bbrstep {}: FLIRT's loop over the boundary points never ends with a \
                 step below 1 (FSL-71)",
                p.bbr.step
            )));
        }
        let mut p = self.impair.take().expect("the image pair is set");
        let mut msgs = CostMessages::default();
        let v = {
            let rv = self.slot_vol(p.ref_slot);
            p.cost(
                rv,
                &self.testvol,
                (&self.global_refweight, &self.global_testweight),
                affmat,
                nonlin,
                &mut msgs,
            )
        };
        self.impair = Some(p);
        for l in &msgs.lines {
            self.el(l);
        }
        if msgs.heap_corrupted {
            // FSL-21: the correlation ratio with no bins writes before its arrays; glibc's
            // allocator finds the heap corrupted when they are freed.
            self.notice(
                &defects::FSL_21,
                "a correlation ratio with no histogram bins (-bins below the scale) writes \
                 before its arrays in FSL; the program aborts",
            );
            return Err(Stop::Abort("double free or corruption (out)\n".to_owned()));
        }
        Ok(v)
    }

    /// `costfn(uninitaffmat, nonlin_params)` and `costfn(uninitaffmat)`.
    fn costfn_mat(&mut self, uninit: &Mat) -> R<f32> {
        let affmat = arma::mul_tinysq(uninit, &self.initmat);
        let nonlin = if self.currentcostfn == CostType::Bbr && self.o.pe_dir != 0 {
            if self.global_fmap.data.is_empty() {
                return Err(Stop::Refuse(
                    "-pedir without -fieldmap: FLIRT's BBR cost interpolates an empty field map (FSL-73) \
                     (undefined behaviour)"
                        .to_owned(),
                ));
            }
            Some(self.default_nonlin())
        } else {
            None
        };
        self.setcostfntype(self.currentcostfn)?;
        self.pair_cost(&affmat, nonlin)
    }

    /// `costfn(params)`.
    fn costfn_params(&mut self, params: &Mat) -> R<f32> {
        let affmat = self.vector2affine(params, self.o.no_params);
        let retval = if self.pair().costtype == CostType::Bbr && self.o.pe_dir != 0 {
            let am = arma::mul_tinysq(&affmat, &self.initmat);
            let nl = self.default_nonlin();
            self.setcostfntype(self.currentcostfn)?;
            self.pair_cost(&am, Some(nl))?
        } else {
            self.costfn_mat(&affmat)?
        };
        if self.o.verbose >= 5 {
            let mut t = format!(
                "{} : {} :: ",
                self.pair().count,
                self.out.num(f64::from(retval))
            );
            for i in 0..self.o.no_params.max(0) as usize {
                t.push_str(&self.out.num(params.get(i)));
                t.push(' ');
            }
            t.push('\n');
            self.out.s(&t);
        }
        Ok(retval)
    }

    // --------------------------------------------------------------------------------------
    // Optimisation support

    fn estimate_scaling(&self) -> f32 {
        let rv = self.slot_vol(self.pair().ref_slot);
        fmin(fmin(rv.xdim(), rv.ydim()), rv.zdim())
    }

    /// `set_param_tols(param_tol, no_params)`: the tolerances times the requested scale.
    fn set_param_tols(&self, n: usize) -> R<Mat> {
        let mut t = vec![0.0f64; n.max(12)];
        for i in 0..n {
            let Some(&v) = self.tolerance.get(i) else {
                return Err(out_of_bounds());
            };
            t[i] = v;
        }
        let k = f64::from(self.requestedscale);
        for v in &mut t {
            *v *= k;
        }
        Ok(Mat::col(&t))
    }

    /// `optimise(params, no_params, param_tol, no_its, fans, costfunc, itmax)`, with
    /// `subset_costfn` (`subset`) or `costfn` as the cost.
    fn optimise(
        &mut self,
        params: &mut Mat,
        n: usize,
        tol: &Mat,
        subset: bool,
        itmax: i32,
    ) -> R<f32> {
        if params.max_abs() < 0.001 && params.len() >= 12 {
            *params = Mat::col(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
        }
        if self.o.verbose >= 5 {
            let a = self.vector2affine(params, n as i32);
            self.out.s("Starting with : \n");
            self.out.mat(&a);
        }
        let boundguess = self.boundguess.clone();
        let powell = self.optimisationtype == "powell";
        let mut failure: Option<Stop> = None;
        let mut events: Vec<OptEvent> = Vec::new();
        let fans = {
            let mut f = |p: &Mat| -> f32 {
                if failure.is_some() {
                    return 0.0;
                }
                let r = if subset {
                    self.subset_costfn(p)
                } else {
                    self.costfn_params(p)
                };
                match r {
                    Ok(v) => v,
                    Err(e) => {
                        failure = Some(e);
                        0.0
                    }
                }
            };
            let mut ev = |e: OptEvent| events.push(e);
            optimise::optimise_full(params, n, tol, &mut f, itmax, &boundguess, powell, &mut ev).0
        };
        if let Some(e) = failure {
            return Err(e);
        }
        for e in events {
            match e {
                OptEvent::Stderr(s) => self.el(&s),
                OptEvent::PowellApplied {
                    finit,
                    fend,
                    fextrap,
                } => {
                    self.out.s("Applying POWELL correction\n");
                    let t = format!(
                        "finit, fend, fextrap = {} , {} , {}\n",
                        self.out.num(f64::from(finit)),
                        self.out.num(f64::from(fend)),
                        self.out.num(f64::from(fextrap))
                    );
                    self.out.s(&t);
                }
                OptEvent::PowellFval(v) => {
                    let t = format!("fval = {}\n", self.out.num(f64::from(v)));
                    self.out.s(&t);
                }
            }
        }
        Ok(fans)
    }

    /// `params12toN(params)`: `pinv(parammask) * (params - refparams)`.
    fn params12_to_n(&self, params: &Mat) -> Mat {
        let d: Vec<f64> = (0..12)
            .map(|i| params.get(i) - self.refparams.get(i))
            .collect();
        let (pi, _) = pinv_mask(&self.parammask);
        Mat::col(&mat_vec(&pi, &d))
    }

    /// `paramsNto12(params)`: `parammask * params + refparams`.
    fn params_n_to_12(&self, params: &Mat) -> Mat {
        let y = mat_vec(&self.parammask, params.as_slice());
        Mat::col(
            &(0..12)
                .map(|i| y[i] + self.refparams.get(i))
                .collect::<Vec<f64>>(),
        )
    }

    /// `subset_costfn(params)`.
    fn subset_costfn(&mut self, params: &Mat) -> R<f32> {
        let param12 = self.params_n_to_12(params);
        let retval = self.costfn_params(&param12)?;
        if self.o.verbose >= 7 {
            let mut t = format!(
                "{} : {} :: ",
                self.pair().count,
                self.out.num(f64::from(retval))
            );
            self.out.s(&t);
            t = String::new();
            let _ = t;
            self.out.row(param12.as_slice());
        }
        Ok(retval)
    }

    // --------------------------------------------------------------------------------------
    // The search

    /// `set_rot_samplings(...)`: the coarse and fine angles of each axis.
    fn set_rot_samplings(&mut self) -> [[Vec<f64>; 3]; 2] {
        let o = &self.o;
        let n_of = |r: [f64; 2], delta: f32| -> usize {
            let v = (r[1] - r[0]) / f64::from(delta);
            let k = volume::round_f64(v) + 1;
            k.max(0) as usize
        };
        // set_rot_sampling(rots, float lowerbound, float upperbound): float arithmetic.
        let sample = |r: [f64; 2], n: usize| -> Vec<f64> {
            let (lb, ub) = (r[0] as f32, r[1] as f32);
            if n == 1 {
                return vec![f64::from(ub + lb) / 2.0];
            }
            (1..=n)
                .map(|k| f64::from(lb + ((k - 1) as f32) * (ub - lb) / ((n - 1) as f32)))
                .collect()
        };
        let mut out: [[Vec<f64>; 3]; 2] = Default::default();
        for a in 0..3 {
            out[0][a] = sample(o.searchr[a], n_of(o.searchr[a], o.coarsedelta));
            out[1][a] = sample(o.searchr[a], n_of(o.searchr[a], o.finedelta));
        }
        if self.o.verbose >= 4 {
            self.out.s("Coarse rotation samplings are:\n");
            for a in 0..3 {
                let v = out[0][a].clone();
                self.out.row(&v);
            }
            self.out.s(" and fine rotation samplings are:\n");
            for a in 0..3 {
                let v = out[1][a].clone();
                self.out.row(&v);
            }
            self.out.s("\n");
        }
        out
    }

    /// `search_cost(paramlist, costs, tx, ty, tz, scale)`: the coarse grid of rotations (each
    /// point's translation and scale optimised), the fine grid's costs from the interpolated
    /// optimisations, the sub-threshold points optimised, and the local minima's parameters.
    fn search_cost(&mut self) -> R<Vec<[f64; 12]>> {
        let storedverbose = self.o.verbose;
        let storeddof = self.o.dof;
        let useranglerep = self.o.anglerep;
        self.o.verbose -= 2;
        self.o.anglerep = AngleRep::Euler;
        self.currentcostfn = self.o.searchcostfn;
        self.o.dof = self.searchdof;
        let [coarse, fine] = self.set_rot_samplings();
        if self.o.dof > 6 {
            let mut m = Mat::zeros(12, 4);
            m.set(6, 0, 1.0);
            m.set(7, 0, 1.0);
            m.set(8, 0, 1.0);
            m.set(3, 1, 1.0);
            m.set(4, 2, 1.0);
            m.set(5, 3, 1.0);
            self.parammask = m;
        } else {
            let mut m = Mat::zeros(12, 3);
            m.set(3, 0, 1.0);
            m.set(4, 1, 1.0);
            m.set(5, 2, 1.0);
            self.parammask = m;
        }
        self.o.no_params = 12;
        let param_tol = self.subset_tolerance()?;
        let (nx, ny, nz) = (coarse[0].len(), coarse[1].len(), coarse[2].len());
        let mut tx = Volume::new(nx as i64, ny as i64, nz as i64, 1);
        let mut ty = tx.clone();
        let mut tz = tx.clone();
        let mut scale = tx.clone();
        let mut rp = vec![0.0f64; 12];
        rp[6] = 1.0;
        rp[7] = 1.0;
        rp[8] = 1.0;
        // trans = refvol.cog("scaled_mm") - initmat * testvol.cog("scaled_mm")
        let mut w = Vec::new();
        let tcog = self.testvol.cog_scaled_mm(&mut w);
        let rcog = self.slot_vol(self.pair().ref_slot).cog_scaled_mm(&mut w);
        for l in &w {
            self.el(l);
        }
        let tc = arma::gemv_tinysq(&self.initmat, &[tcog[0], tcog[1], tcog[2], 1.0], false);
        rp[3] = rcog[0] - tc[0];
        rp[4] = rcog[1] - tc[1];
        rp[5] = rcog[2] - tc[2];
        self.refparams = Mat::col(&rp);
        let ncols = self.parammask.cols();
        for ix in 0..nx {
            for iy in 0..ny {
                for iz in 0..nz {
                    let mut r = self.refparams.as_slice().to_vec();
                    r[0] = f64::from(coarse[0][ix] as f32);
                    r[1] = f64::from(coarse[1][iy] as f32);
                    r[2] = f64::from(coarse[2][iz] as f32);
                    self.refparams = Mat::col(&r);
                    let mut p8 = self.refparams.clone();
                    if self.o.verbose >= 4 {
                        self.out.s("Starting with ");
                        self.out.row(p8.as_slice());
                        self.out.s("  and tolerance ");
                        self.out.row(param_tol.as_slice());
                    }
                    p8 = self.params12_to_n(&p8);
                    self.optimise(&mut p8, ncols, &param_tol, true, 4)?;
                    let p12 = self.params_n_to_12(&p8);
                    let (i, j, k) = (ix as i64, iy as i64, iz as i64);
                    tx.set(i, j, k, p12.get(3) as f32);
                    ty.set(i, j, k, p12.get(4) as f32);
                    tz.set(i, j, k, p12.get(5) as f32);
                    scale.set(i, j, k, p12.get(6) as f32);
                    if self.o.verbose >= 4 {
                        self.out.s(" dearranged: ");
                        self.out.row(p12.as_slice());
                    }
                }
                if self.o.verbose >= 2 {
                    self.out.s("*");
                }
            }
        }
        if self.o.verbose >= 2 {
            self.out.s("\n");
        }
        let medianscale = percentile(&scale, 0.5);
        scale.data.iter_mut().for_each(|v| *v = medianscale);
        if self.o.verbose >= 2 {
            let t = format!("Median scale = {}\n", self.out.num(f64::from(medianscale)));
            self.out.s(&t);
        }
        if self.o.verbose >= 4 {
            self.safe_save(&tx, "tx")?;
            self.safe_save(&ty, "ty")?;
            self.safe_save(&tz, "tz")?;
            self.safe_save(&scale, "sc")?;
        }
        let factor = |c: usize, f: usize| -> f32 { (c as f32 - 1.0) / fmax(1.0, f as f32 - 1.0) };
        let (fx, fy, fz) = (
            factor(nx, fine[0].len()),
            factor(ny, fine[1].len()),
            factor(nz, fine[2].len()),
        );
        let (mx, my, mz) = (fine[0].len(), fine[1].len(), fine[2].len());
        let mut costs = Volume::new(mx as i64, my as i64, mz as i64, 1);
        let interp_at = |ix: usize, iy: usize, iz: usize| -> [f32; 4] {
            let (xf, yf, zf) = (ix as f32 * fx, iy as f32 * fy, iz as f32 * fz);
            [
                tx.interpolate(xf, yf, zf),
                ty.interpolate(xf, yf, zf),
                tz.interpolate(xf, yf, zf),
                scale.interpolate(xf, yf, zf),
            ]
        };
        let dof = self.o.dof;
        let fix_scale = |mut scv: f32| -> f32 {
            if scv < 0.5 || scv > 2.0 {
                scv = 1.0;
            }
            if dof <= 6 {
                scv = 1.0;
            }
            scv
        };
        for ix in 0..mx {
            for iy in 0..my {
                for iz in 0..mz {
                    let [txv, tyv, tzv, scv0] = interp_at(ix, iy, iz);
                    let scv = fix_scale(scv0);
                    let r = vec![
                        f64::from(fine[0][ix] as f32),
                        f64::from(fine[1][iy] as f32),
                        f64::from(fine[2][iz] as f32),
                        f64::from(txv),
                        f64::from(tyv),
                        f64::from(tzv),
                        f64::from(scv),
                        f64::from(scv),
                        f64::from(scv),
                        self.refparams.get(9),
                        self.refparams.get(10),
                        self.refparams.get(11),
                    ];
                    self.refparams = Mat::col(&r);
                    let p8 = self.refparams.clone();
                    let c = self.costfn_params(&p8)?;
                    costs.set(ix as i64, iy as i64, iz as i64, c);
                }
                if self.o.verbose >= 2 {
                    self.out.s("*");
                }
            }
        }
        if self.o.verbose >= 2 {
            self.out.s("\n");
        }
        if self.o.verbose >= 4 {
            self.safe_save(&costs, "costs")?;
        }
        let (costmax, costmin) = costs.extrema();
        let factor02: f32 = 0.2;
        let mut costthresh = fmin(
            costmin + factor02 * (costmax - costmin),
            percentile(&costs, 0.20),
        );
        if costthresh <= costmin {
            costthresh = fmax(
                (f64::from(costmin) * 1.0001) as f32,
                (f64::from(costmin) * 0.9999) as f32,
            );
        }
        if self.o.verbose >= 4 {
            let t = format!(
                "Cost threshold = {} and minimum is {}\n",
                self.out.num(f64::from(costthresh)),
                self.out.num(f64::from(costmin))
            );
            self.out.s(&t);
        }
        let mut numsubcost = costs.data.iter().filter(|&&c| c <= costthresh).count();
        if numsubcost == 0 {
            self.out.s("WARNING: Found 0 or less sub-threshold costs\n");
            numsubcost = 1;
        }
        let mut bestparams: Vec<Vec<f64>> = vec![vec![0.0; 13]; numsubcost];
        let mut n = 0usize;
        for ix in 0..mx {
            for iy in 0..my {
                for iz in 0..mz {
                    if costs.value(ix as i64, iy as i64, iz as i64) < costthresh {
                        let [txv, tyv, tzv, scv0] = interp_at(ix, iy, iz);
                        let scv = fix_scale(scv0);
                        let p8v = vec![
                            f64::from(fine[0][ix] as f32),
                            f64::from(fine[1][iy] as f32),
                            f64::from(fine[2][iz] as f32),
                            f64::from(txv),
                            f64::from(tyv),
                            f64::from(tzv),
                            f64::from(scv),
                            f64::from(scv),
                            f64::from(scv),
                            0.0,
                            0.0,
                            0.0,
                        ];
                        self.refparams = Mat::col(&p8v);
                        let mut p8 = self.params12_to_n(&Mat::col(&p8v));
                        let fans = self.optimise(&mut p8, ncols, &param_tol, true, 4)?;
                        let p12 = self.params_n_to_12(&p8);
                        costs.set(ix as i64, iy as i64, iz as i64, fans);
                        if n < bestparams.len() {
                            bestparams[n][0] = f64::from(fans);
                            bestparams[n][1..13].copy_from_slice(p12.as_slice());
                        } else {
                            return Err(out_of_bounds());
                        }
                        n += 1;
                        if self.o.verbose >= 3 {
                            let t = format!(
                                "({ix},{iy},{iz}) => {} with ",
                                self.out.num(f64::from(fans))
                            );
                            self.out.s(&t);
                            self.out.row(p12.as_slice());
                        }
                    }
                }
            }
        }
        if self.o.verbose >= 3 {
            self.safe_save(&costs, "costs")?;
            self.out.s("Costs (1st column) are:\n");
            let rows: Vec<f64> = bestparams.iter().flatten().copied().collect();
            let m = Mat::from_rows(bestparams.len(), 13, &rows);
            self.out.mat(&m);
            self.out.s("\n");
        }
        let bestpts = self.find_cost_minima(&costs);
        let mut paramlist = Vec::with_capacity(bestpts.len());
        for (bx, by, bz) in bestpts {
            let (ix, iy, iz) = (bx, by, bz);
            if self.o.verbose >= 3 {
                self.out.s(&format!("Cost minima at : {ix},{iy},{iz}\n"));
            }
            let [txv, tyv, tzv, scv0] = interp_at(ix as usize, iy as usize, iz as usize);
            let scv = fix_scale(scv0);
            paramlist.push([
                f64::from(fine[0][ix as usize] as f32),
                f64::from(fine[1][iy as usize] as f32),
                f64::from(fine[2][iz as usize] as f32),
                f64::from(txv),
                f64::from(tyv),
                f64::from(tzv),
                f64::from(scv),
                f64::from(scv),
                f64::from(scv),
                0.0,
                0.0,
                0.0,
            ]);
        }
        if self.o.verbose >= 3 {
            self.out.s("Chosen parameters:\n");
            let rows: Vec<f64> = paramlist.iter().flatten().copied().collect();
            let m = Mat::from_rows(paramlist.len(), 12, &rows);
            self.out.mat(&m);
            self.out.s("\n");
        }
        self.o.anglerep = useranglerep;
        self.o.verbose = storedverbose;
        self.o.dof = storeddof;
        Ok(paramlist)
    }

    /// The search's tolerance in the subset's parameters (`param_tol1 - param_tol0`).
    fn subset_tolerance(&self) -> R<Mat> {
        let t0 = self.params12_to_n(&self.refparams);
        let t = self.set_param_tols(12)?;
        let t1v: Vec<f64> = (0..12).map(|i| t.get(i) + self.refparams.get(i)).collect();
        let t1 = self.params12_to_n(&Mat::col(&t1v));
        Ok(Mat::col(
            &(0..t1.len())
                .map(|i| t1.get(i) - t0.get(i))
                .collect::<Vec<f64>>(),
        ))
    }

    /// `find_cost_minima(bestpts, cost)`: the grid points with no smaller neighbour (inside the
    /// last row, column and slice), else the smallest cost.
    fn find_cost_minima(&mut self, cost: &Volume) -> Vec<(i64, i64, i64)> {
        let (xb, yb, zb) = (cost.xsize(), cost.ysize(), cost.zsize());
        let mut minv = vec![0.0f32; (xb * yb * zb).max(0) as usize];
        let mut bestpt = [0.0f32; 3];
        let mut minima = 0;
        for z in 0..zb {
            for y in 0..yb {
                for x in 0..xb {
                    let c = cost.value(x, y, z);
                    let k = cost.idx(x, y, z);
                    for zo in -1..=1 {
                        for yo in -1..=1 {
                            for xo in -1..=1 {
                                if cost.in_bounds(x + xo, y + yo, z + zo)
                                    && cost.value(x + xo, y + yo, z + zo) < c
                                {
                                    minv[k] += 1.0;
                                }
                            }
                        }
                    }
                    let b = cost.get(
                        i64::from(round_f32(bestpt[0])),
                        i64::from(round_f32(bestpt[1])),
                        i64::from(round_f32(bestpt[2])),
                    );
                    if c < b {
                        bestpt = [x as f32, y as f32, z as f32];
                    }
                    if cost.in_bounds(x + 1, y + 1, z + 1) && minv[k] < 0.5 {
                        minima += 1;
                    }
                }
            }
        }
        if minima <= 0 {
            return vec![(
                i64::from(round_f32(bestpt[0])),
                i64::from(round_f32(bestpt[1])),
                i64::from(round_f32(bestpt[2])),
            )];
        }
        let mut pts = Vec::new();
        for z in 0..zb {
            for y in 0..yb {
                for x in 0..xb {
                    if cost.in_bounds(x + 1, y + 1, z + 1) && minv[cost.idx(x, y, z)] < 0.5 {
                        pts.push((x, y, z));
                        if self.o.verbose >= 3 {
                            self.out.s(&format!("COST minima at : {x},{y},{z}\n"));
                        }
                    }
                }
            }
        }
        pts
    }

    // --------------------------------------------------------------------------------------
    // Strategies

    /// `measure_cost(affmat, input_dof)`.
    fn measure_cost(&mut self, affmat: &Mat, input_dof: i32) -> R<f32> {
        if input_dof < 6 {
            self.el(&format!("Erroneous dof {input_dof} : using 6 instead"));
        } else if input_dof > 12 {
            self.el(&format!("Erroneous dof {input_dof} : using 12 instead"));
        }
        self.costfn_mat(affmat)
    }

    /// `aligncog(affmat)`: the translation that maps the test centre of mass onto the
    /// reference's.
    fn aligncog(&mut self, affmat: &mut Mat) {
        let mut resmat = affmat.clone();
        for r in 0..3 {
            resmat.set(r, 3, 0.0);
        }
        let mut w = Vec::new();
        let tcog = self.testvol.cog_scaled_mm(&mut w);
        let rcog = self.slot_vol(self.pair().ref_slot).cog_scaled_mm(&mut w);
        for l in &w {
            self.el(l);
        }
        let a3 = resmat.sub(0, 3, 0, 3);
        let t = arma::gemv_tinysq(&a3, &tcog, false);
        for r in 0..3 {
            resmat.set(r, 3, -(t[r] - rcog[r]));
        }
        *affmat = resmat;
    }

    /// `optimise_strategy0(matresult, fans, max_iterations)`: one optimisation in the user's
    /// parameter subset.
    fn optimise_strategy0(&mut self, matresult: &mut Mat, max_it: i32) -> R<f32> {
        if self.o.verbose > 3 {
            self.out.s("Using subset cost function\n");
        }
        self.o.no_params = 12;
        let params = self.affmat2vector(matresult);
        self.refparams = params;
        let param_tol = self.subset_tolerance()?;
        let mut params_n = self.params12_to_n(&self.refparams.clone());
        if self.o.verbose > 6 {
            self.out.s("Starting with ");
            self.out.row(params_n.as_slice());
            self.out.s("  and tolerance ");
            self.out.row(param_tol.as_slice());
        }
        let ncols = self.parammask.cols();
        let fans = self.optimise(&mut params_n, ncols, &param_tol, true, max_it)?;
        let p12 = self.params_n_to_12(&params_n);
        *matresult = self.vector2affine(&p12, 12);
        Ok(fans)
    }

    /// `optimise_strategy1(matresult, fans, input_dof, max_iterations)`.
    fn optimise_strategy1(&mut self, matresult: &mut Mat, input_dof: i32, max_it: i32) -> R<f32> {
        let mut dof = input_dof;
        if dof < 6 {
            self.el(&format!("Erroneous dof {dof} : using 6 instead"));
            dof = 6;
        }
        if dof > 12 {
            self.el(&format!("Erroneous dof {dof} : using 12 instead"));
            dof = 12;
        }
        self.o.no_params = dof;
        let param_tol = self.set_param_tols(12)?;
        let mut params = self.affmat2vector(matresult);
        if self.o.verbose > 6 {
            self.out.s("Starting with ");
            self.out.row(params.as_slice());
            self.out.s("  and tolerance ");
            self.out.row(param_tol.as_slice());
        }
        let fans = self.optimise(&mut params, dof as usize, &param_tol, false, max_it)?;
        *matresult = self.vector2affine(&params, dof);
        Ok(fans)
    }

    /// `optimise_strategy3(opt_matrixlist)`: the search, then each minimum optimised freely
    /// (up to 7 dof), listed by cost with near duplicates dropped.
    fn optimise_strategy3(&mut self) -> R<Vec<Vec<f64>>> {
        self.currentcostfn = self.o.searchcostfn;
        let paramlist = self.search_cost()?;
        let dof = self.o.dof.min(7);
        let rms_min = self.estimate_scaling();
        let mut list: Vec<Vec<f64>> = Vec::new();
        let verbose = self.o.verbose;
        self.o.verbose -= 2;
        if verbose >= 3 {
            self.out.s("After free optimisation, parameters are:\n");
        }
        for p in &paramlist {
            let mut matresult = self.vector2affine(&Mat::col(p), dof);
            let premat = matresult.clone();
            let costval = self.optimise_strategy1(&mut matresult, dof, 4)?;
            let mut row = vec![0.0f64; 34];
            row[0] = f64::from(costval);
            row[1..17].copy_from_slice(&mat_to_16(&matresult));
            row[17] = f64::from(self.costfn_mat(&premat)?);
            row[18..34].copy_from_slice(&mat_to_16(&premat));
            self.add2list(&row, &mut list, rms_min)?;
            if verbose >= 3 {
                let pv = self.affmat2vector(&matresult);
                let t = format!("{} ::: ", self.out.num(f64::from(costval)));
                self.out.s(&t);
                self.out.row(pv.as_slice());
            }
        }
        self.o.verbose = verbose;
        self.currentcostfn = self.o.maincostfn;
        if self.o.verbose >= 3 {
            self.out.s("Parameters (1st column costs) are:\n");
            let rows: Vec<f64> = list.iter().flatten().copied().collect();
            let m = Mat::from_rows(list.len(), 34, &rows);
            self.out.mat(&m);
            self.out.s("\n");
        }
        Ok(list)
    }

    /// `add2list(rowmat, opt_matrixlist, rms_min)`.
    fn add2list(&mut self, row: &[f64], list: &mut Vec<Vec<f64>>, rms_min: f32) -> R<()> {
        let mat1 = row_to_mat(row);
        let costval = row[0];
        let zero = Mat::col(&[0.0, 0.0, 0.0]);
        for n in 0..list.len() {
            let mat2 = row_to_mat(&list[n]);
            let d = match miscmaths::rms_deviation(&mat1, &mat2, &zero, 80.0) {
                Ok(d) => d,
                Err(_) => {
                    self.el("RMS_DEVIATION ERROR:: Could not invert matrix");
                    return Err(Stop::Exit(251));
                }
            };
            if d < rms_min {
                if costval < list[n][0] {
                    list.remove(n);
                    break;
                }
                return Ok(());
            }
        }
        let pos = list
            .iter()
            .position(|r| costval < r[0])
            .unwrap_or(list.len());
        list.insert(pos, row.to_vec());
        Ok(())
    }

    /// `set_perturbations(delta, perturbmask)`: the perturbation sizes (`delta`).
    fn set_perturbations(&mut self) -> R<Vec<f64>> {
        let [_, fine] = self.set_rot_samplings();
        let param_tol = self.set_param_tols(12)?;
        let mut del = [
            (3.0 * param_tol.get(0)) as f32,
            (3.0 * param_tol.get(1)) as f32,
            (3.0 * param_tol.get(2)) as f32,
        ];
        for a in 0..3 {
            if fine[a].len() > 1 {
                del[a] = (0.5 * (fine[a][1] - fine[a][0])) as f32;
            }
        }
        let delscale: f32 = if self.o.dof >= 7 { 0.1 } else { 0.0 };
        let mut delta = vec![0.05f64; 12];
        delta[0] = f64::from(del[0]);
        delta[1] = f64::from(del[1]);
        delta[2] = f64::from(del[2]);
        delta[6] = f64::from(delscale);
        delta[7] = f64::from(delscale);
        delta[8] = f64::from(delscale);
        Ok(delta)
    }

    // --------------------------------------------------------------------------------------
    // Filtering and resampling

    fn filter(&mut self, f: Filter, v: &Volume) -> R<Volume> {
        let s = self.global_sampling;
        Ok(match f {
            Filter::SubsampleBy2 => {
                // subsample_by_2 fills the half-size volume with the background value first
                background_defined(v)?;
                newimagefns::subsample_by_2(v)
            }
            Filter::Blur => newimagefns::blur(v, s, self.threads),
            Filter::ResampBlur => {
                let t = newimagefns::blur(v, s, self.threads);
                let mut w = Vec::new();
                let r = isotropic_resample(&t, s, &mut w);
                for l in &w {
                    self.el(l);
                }
                r
            }
        })
    }

    /// `filter_image(blur_im, orig_im, weight, useweight, filter)`: `filter(I .* B) ./ filter(B)`
    /// with `B` the weight above 0.01, or `filter(I)`. `weight_is_target`: the weight is the
    /// volume being filtered (FLIRT passes the same volume three times), so the second
    /// binarisation reads the filtered image.
    fn filter_image(
        &mut self,
        orig: &Volume,
        weight: &Volume,
        useweight: bool,
        f: Filter,
        weight_is_target: bool,
    ) -> R<Volume> {
        if !useweight {
            return self.filter(f, orig);
        }
        let b = newimagefns::binarise(weight, 0.01);
        let mut ib = orig.clone();
        newimagefns::multiply_in_place(&mut ib, &b)?;
        let blur_im = self.filter(f, &ib)?;
        let w2 = if weight_is_target { &blur_im } else { weight };
        let b2 = newimagefns::binarise(w2, 0.01);
        let fb = self.filter(f, &b2)?;
        Ok(newimagefns::divide(&blur_im, &fb, &fb)?)
    }

    /// `filter_weight(blur_w, weight, filter)`: `filter(W) .* (filter(B) >= 0.9)`.
    fn filter_weight(&mut self, weight: &Volume, f: Filter) -> R<Volume> {
        let mut tmp = newimagefns::binarise(weight, 0.01);
        tmp = self.filter(f, &tmp)?;
        newimagefns::binarise_range(&mut tmp, 0.9, f32::MAX);
        let mut blur_w = self.filter(f, weight)?;
        newimagefns::multiply_in_place(&mut blur_w, &tmp)?;
        Ok(blur_w)
    }

    /// `resample_refvol(refvol, sampling)` on `vol` (weighted by `weight` when weights are used;
    /// `weight_is_target` when they are the same volume).
    fn resample_refvol(
        &mut self,
        vol: &Volume,
        weight: &Volume,
        sampling: f32,
        weight_is_target: bool,
    ) -> R<Volume> {
        let mut sampl = sampling;
        if f64::from(sampl) < 1e-5 {
            self.el(&format!(
                "WARNING: sampling {} less than 0.00001 mm",
                cnum::stream(f64::from(sampl))
            ));
            self.el("         Setting to 1.0 sampling instead");
            sampl = 1.0;
        }
        if f64::from(sampl) < 1e-3 {
            self.el("WARNING: under minimum sampling of 0.001 mm");
        }
        if self.o.verbose >= 2 {
            let t = format!(
                "Resampling refvol isotropically: {}\n",
                self.out.num(f64::from(sampling))
            );
            self.out.s(&t);
        }
        self.global_sampling = sampl;
        let useweights = self.o.useweights;
        let r = self.filter_image(
            vol,
            weight,
            useweights,
            Filter::ResampBlur,
            weight_is_target,
        )?;
        if self.o.verbose >= 2 {
            self.print_volume_info(&r, "Refvol");
        }
        Ok(r)
    }

    fn print_volume_info(&mut self, v: &Volume, name: &str) {
        let (mx, mn) = v.extrema();
        let t = format!(
            "{name}:: Size = ({},{},{},{})\n{name}:: Dims = ({},{},{},{})\n{name}:: ROI Size = ({},{},{},{})\n{name}:: Minimum and maximum intensities are: {} and {}\n",
            v.xsize(),
            v.ysize(),
            v.zsize(),
            v.tsize(),
            self.out.num(f64::from(v.xdim())),
            self.out.num(f64::from(v.ydim())),
            self.out.num(f64::from(v.zdim())),
            self.out.num(f64::from(v.tdim())),
            v.xsize(),
            v.ysize(),
            v.zsize(),
            v.tsize(),
            self.out.num(f64::from(mn)),
            self.out.num(f64::from(mx)),
        );
        self.out.s(&t);
    }

    // --------------------------------------------------------------------------------------
    // Outputs

    /// The settings saving reads: corrected (FSL-11), `FSLOUTPUTTYPE` defaults to `NIFTI_GZ`.
    fn output_settings(&self) -> Settings {
        if self.mode.fix() && self.settings.get("FSLOUTPUTTYPE").is_none() {
            self.settings.clone().with("FSLOUTPUTTYPE", "NIFTI_GZ")
        } else {
            self.settings.clone()
        }
    }

    /// `save_volume_dtype(vol, name, datatype)` with FSL-09 corrected unless reproduced.
    fn save_dtype(&mut self, v: &Volume, name: &str, datatype: i16) -> R<()> {
        let ty = match volume::save_type_for(datatype) {
            Ok(t) => t,
            Err(msg) => {
                self.el(&msg);
                self.el(&format!(
                    "NEWIMAGE::save_volume_dtype: DT {datatype} not supported: Success"
                ));
                return Ok(());
            }
        };
        let integer = matches!(ty, SaveType::Char | SaveType::Short | SaveType::Int);
        // the correction changes a value that is not an integer, not finite, or out of the
        // type's range (FLIRT's conversion wraps it; random testing, 2026-11-01: a negative
        // -setbackground into uint8 was changed without the notice)
        let (lo, hi) = match ty {
            SaveType::Char => (0.0, 255.0),
            SaveType::Short => (-32768.0, 32767.0),
            _ => (f64::from(i32::MIN), f64::from(i32::MAX)),
        };
        let rounds = integer
            && v.data.iter().any(|x| {
                x.round_ties_even() != x.trunc()
                    || !x.is_finite()
                    || f64::from(x.round_ties_even()) < lo
                    || f64::from(x.round_ties_even()) > hi
            });
        let (ty, conv, code) = if self.mode.fix() {
            if datatype == dt::INT8 {
                self.notice(
                    &defects::FSL_09,
                    "int8 data are written as int8, rounded and clamped; FLIRT writes uint8 and \
                     truncates",
                );
                (SaveType::Int8, Conversion::RoundClamp, Some(dt::INT8))
            } else if integer {
                if rounds {
                    self.notice(
                        &defects::FSL_09,
                        "integer outputs are rounded to the nearest and clamped; FLIRT truncates \
                         toward zero and wraps out-of-range values",
                    );
                }
                (ty, Conversion::RoundClamp, None)
            } else {
                (ty, Conversion::X86Cast, None)
            }
        } else {
            if datatype == dt::INT8 {
                self.notice(
                    &defects::FSL_09,
                    "int8 data are written as uint8, as in FLIRT",
                );
            } else if rounds {
                self.notice(
                    &defects::FSL_09,
                    "integer outputs are truncated toward zero, as in FLIRT",
                );
            }
            (ty, Conversion::X86Cast, None)
        };
        let settings = self.output_settings();
        if volume::offset_defect_applies(v, &settings) {
            self.notice(
                &defects::FSL_56,
                format!(
                    "{} NIfTI extensions of different sizes: the data offset counts each \
                     extension's own size (NewNifti adds the first one's once per extension)",
                    v.extensions.len()
                ),
            );
        }
        let opts = SaveOptions {
            description: DESCRIP,
            units: nifti::UNITS_MM_SEC,
            reproduce_offset: self.mode.reproduce(),
        };
        match v.save_typed(name, ty, conv, &settings, &opts, code) {
            Ok(()) => Ok(()),
            Err(SaveError::OutputType(e)) => {
                for l in &e.printed {
                    self.el(l);
                }
                Err(Stop::Exit(1))
            }
            Err(SaveError::Exception(e)) => Err(e.into()),
        }
    }

    /// `safe_save_volume(source, filename)`: only without `-nosave` (FLIRT's default) in the
    /// input's data type.
    fn safe_save(&mut self, v: &Volume, name: &str) -> R<()> {
        if !self.o.nosave {
            let mut c = v.clone();
            c.display_max = 0.0;
            c.display_min = 0.0;
            let d = self.o.datatype;
            self.save_dtype(&c, name, d)?;
        }
        Ok(())
    }

    /// `output_dtype(outvol)`: the input's type, or float for a non-float type when the values
    /// span less than 1.5 (unless `-datatype`).
    fn output_dtype(&self, v: &Volume) -> i16 {
        let mut d = self.o.datatype;
        if !self.o.forcedatatype && d != dt::FLOAT && d != dt::DOUBLE {
            let (mx, mn) = v.extrema();
            if mx - mn < 1.5 {
                d = dt::FLOAT;
            }
        }
        d
    }

    /// `configure_interpolation(vol)`: the final interpolation, and the pad value (`-setbackground`
    /// with zero padding, else the background value with extraslice extrapolation).
    /// FSL computes the background value only without `-setbackground`, after the
    /// interpolation's set-up (its sinc kernel's messages); on a shape where that is undefined
    /// (FSL-75) the replica refuses there.
    fn configure_interpolation_bg(&mut self, vol: &mut Volume) -> R<()> {
        match self.o.interpmethod {
            Interp::NearestNeighbour => vol.interp = Interpolation::NearestNeighbour,
            Interp::Sinc => {
                let w = round_f32(self.o.sincwidth);
                let name = match self.o.sincwindow {
                    Window::Hanning => "hanning",
                    Window::Blackman => "blackman",
                    Window::Rect => "rectangular",
                };
                let mut msgs = Vec::new();
                vol.kernel = Some(std::sync::Arc::new(kernel::sinc_kernel(
                    name, w, 1201, &mut msgs,
                )));
                for l in &msgs {
                    self.el(l);
                }
                vol.interp = Interpolation::Sinc;
            }
            Interp::Spline => vol.interp = Interpolation::Spline,
            Interp::TriLinear => vol.interp = Interpolation::Trilinear,
        }
        if self.o.forcebackgnd {
            vol.padvalue = self.o.backgndval;
            vol.extrap = Extrapolation::Constpad;
        } else {
            background_defined(vol)?;
            vol.padvalue = vol.backgroundval();
            vol.extrap = Extrapolation::Extraslice;
        }
        Ok(())
    }

    /// `final_transform(testvol, refvol, finalmat, outputvol)`, the reference being the run's
    /// `refvol` (FLIRT's call passes it: `main`'s, or `do_applyxfm`'s, which that function puts
    /// there).
    fn final_transform(
        &mut self,
        testvol: &Volume,
        finalmat: &Mat,
        outputvol: &mut Volume,
    ) -> R<()> {
        let mut paddingsize = self.o.paddingsize;
        if self.o.mode2d {
            paddingsize = fmax(1.0, paddingsize);
        }
        if self.o.pe_dir == 0 {
            newimagefns::affine_transform_with(testvol, outputvol, finalmat, paddingsize, false)?;
            return Ok(());
        }
        // The field map: a pair on the final volumes, BBR's resampling.
        if self.global_fmap.data.is_empty() {
            return Err(Stop::Refuse(
                "-pedir without a field map read (no -fieldmap, or -applyxfm, which never reads \
                 it): FLIRT interpolates an empty field map (undefined behaviour, FSL-73)"
                    .to_owned(),
            ));
        }
        if self.o.debug {
            self.el("Start affine_and_fmap_transform");
        }
        if self.impair.is_none() {
            if self.o.debug {
                self.el("Pre new costfn");
            }
            let mut w = Vec::new();
            let cog = testvol.cog_scaled_mm(&mut w);
            let mut p = Pair::new(RefSlot::Ref1, false, cog);
            if self.o.debug {
                self.el("Pre setup_costfn");
            }
            self.setup_costfn(&mut p, CostType::Bbr, self.o.no_bins)?;
            self.impair = Some(p);
        }
        if self.o.debug {
            self.el("Pre bbr_resamp");
        }
        // default_nonlin_params(): the pair's test volume is this one.
        let nl = nonlin_of(self.o.pe_dir, self.o.echo_spacing, testvol);
        let r = bbr::bbr_resamp(&self.refvol, testvol, finalmat, nl, &self.pair().bbr);
        outputvol.data.copy_from_slice(&r.data);
        if self.o.debug {
            self.el("Post bbr_resamp");
        }
        Ok(())
    }

    /// `save_matrix_data(matresult)`: `write_ascii_matrix` (10 significant digits, two blanks
    /// after each number); nothing without `-omat`.
    fn save_matrix(&mut self, m: &Mat) {
        let name = self.o.outputmatascii.clone();
        if name.is_empty() {
            return;
        }
        let mut t = String::new();
        for r in 0..m.rows() {
            for c in 0..m.cols() {
                t.push_str(&cnum::fmt_g(m.at(r, c), 10));
                t.push_str("  ");
            }
            t.push('\n');
        }
        if std::fs::write(&name, t).is_err() {
            self.el(&format!("Could not open file {name} for writing"));
        }
    }
}

/// `default_nonlin_params()`: the field map's scale, `echo_spacing * N / (2 pi)` (`N` the test
/// volume's size along the phase-encoding axis; the product in float, the quotient in double,
/// stored as float), negative for a reversed direction.
fn nonlin_of(pe: i32, echo_spacing: f32, testvol: &Volume) -> f64 {
    let n = match pe.abs() {
        1 => testvol.xsize(),
        2 => testvol.ysize(),
        3 => testvol.zsize(),
        _ => 0,
    } as i32;
    let mut s = (f64::from(echo_spacing * n as f32) / (2.0 * std::f64::consts::PI)) as f32;
    if pe < 0 {
        s *= -1.0;
    }
    f64::from(s)
}

/// `double_end_slices(testvol)`: a single slice repeated above and below (3 slices of 8 mm), the
/// volume's other properties dropped (FLIRT builds a new volume).
fn double_end_slices(v: &mut Volume) {
    let mut n = Volume::new(v.xsize(), v.ysize(), v.zsize() + 2, 1);
    n.setdims(v.xdim(), v.ydim(), 8.0);
    for z in 0..=v.zsize() + 1 {
        let mut ez = z - 1;
        if ez < 0 {
            ez = 0;
        }
        if ez >= v.zsize() {
            ez = v.zsize() - 1;
        }
        for y in 0..v.ysize() {
            for x in 0..v.xsize() {
                n.set(x, y, z, v.value(x, y, ez));
            }
        }
    }
    *v = n;
}

/// `newimagevox2mm_mat()`: the sform, else the qform, else the voxel sizes.
fn newimagevox2mm_mat(v: &Volume) -> Mat {
    if v.sform_code != nifti::XFORM_UNKNOWN {
        v.sform.clone()
    } else if v.qform_code != nifti::XFORM_UNKNOWN {
        v.qform.clone()
    } else {
        let mut m = Mat::eye(4);
        m.set(0, 0, f64::from(v.xdim()));
        m.set(1, 1, f64::from(v.ydim()));
        m.set(2, 2, f64::from(v.zdim()));
        m
    }
}

/// `volume<float>::percentile(p)`: the sorted values' element at `(unsigned)(n * p)` (the last
/// when beyond).
fn percentile(v: &Volume, p: f32) -> f32 {
    let mut h = v.data.clone();
    if h.is_empty() {
        return 0.0;
    }
    if h.iter().any(|x| x.is_nan()) {
        let _ = larmorx_nc_core::stdsort::sort_f32_checked(&mut h);
    } else {
        h.sort_by(f32::total_cmp);
    }
    let n = h.len();
    let mut k = cnum::f32_to_u32(n as f32 * p) as usize;
    if k >= n {
        k = n - 1;
    }
    h[k]
}

fn singular(what: &str) -> Stop {
    Stop::Exception(FslError {
        printed: Vec::new(),
        type_name: "std::runtime_error",
        what: what.to_owned(),
    })
}

fn out_of_bounds() -> Stop {
    Stop::Exception(FslError {
        printed: Vec::new(),
        type_name: "std::logic_error",
        what: "Mat::operator(): index out of bounds".to_owned(),
    })
}

/// Armadillo's size check of a product (`matrix multiplication: incompatible matrix dimensions`).
fn check_mul(a: &Mat, b: &Mat) -> R<()> {
    if a.cols() != b.rows() {
        return Err(Stop::Exception(FslError {
            printed: Vec::new(),
            type_name: "std::logic_error",
            what: format!(
                "matrix multiplication: incompatible matrix dimensions: {}x{} and {}x{}",
                a.rows(),
                a.cols(),
                b.rows(),
                b.cols()
            ),
        }));
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// The schedule

impl Flirt<'_> {
    fn scalars(&self) -> Scalars {
        Scalars {
            maxdof: self.o.dof,
            minsampling: self.o.min_sampling,
            verbose: self.o.verbose,
        }
    }

    fn mats(&mut self, m: MatName) -> &mut Vec<Row> {
        match m {
            MatName::S => &mut self.searchoptmat,
            MatName::P => &mut self.preoptsearchmat,
            MatName::U(k) => &mut self.usrmat[k],
        }
    }

    /// `parsematname` with FSL's messages; an unusable name is refused (FSL then dereferences an
    /// uninitialised pointer).
    fn matname(&mut self, word: &str) -> R<(MatName, i32, i32)> {
        let s = self.scalars();
        let (m, r1, r2, msgs) = schedule::parsematname(word, &s);
        for l in &msgs {
            self.el(l);
        }
        match m {
            Ok(Some(n)) => Ok((n, r1, r2)),
            Ok(None) | Err(BadName::Empty) => Err(Stop::Refuse(format!(
                "the schedule names no matrix where one is needed ('{word}'): FLIRT then uses \
                 an uninitialised pointer (FSL-70)"
            ))),
            Err(BadName::Message(_)) => Err(Stop::Refuse(format!(
                "'{word}' is not a matrix name: FLIRT then uses an uninitialised pointer (FSL-70)"
            ))),
            Err(BadName::OutOfRange(n)) => Err(Stop::Refuse(format!(
                "'{n}' names a 28th matrix: FLIRT reads and writes beyond its 27 matrices (FSL-70)"
            ))),
        }
    }

    fn scalar_f(&mut self, w: &str) -> f32 {
        let s = self.scalars();
        let (v, m) = schedule::scalar_f(w, &s);
        if let Some(m) = m {
            self.el(&m);
        }
        v
    }

    fn scalar_i(&mut self, w: &str) -> i32 {
        let s = self.scalars();
        let (v, m) = schedule::scalar_i(w, &s, 0);
        if let Some(m) = m {
            self.el(&m);
        }
        v
    }

    fn wrong_args(&mut self, cmd: &str) -> Stop {
        self.el(&format!("Wrong number of args to {cmd}"));
        Stop::Exit(EXIT_MINUS_ONE)
    }

    /// The perturbation words of `optimise` and `measurecost` (up to `rel` or `abs`).
    fn perturbation(&mut self, words: &[String], from: usize) -> R<(Vec<f64>, bool, usize)> {
        let mut p = vec![0.0f64; 12];
        let mut wordno = from;
        while wordno < words.len() && words[wordno] != "rel" && words[wordno] != "abs" {
            let v = self.scalar_f(&words[wordno]);
            let k = wordno - 2;
            if !(1..=12).contains(&k) {
                return Err(out_of_bounds());
            }
            p[k - 1] = f64::from(v);
            wordno += 1;
        }
        let mut relative = true;
        if wordno < words.len() {
            if words[wordno] == "abs" {
                relative = false;
            }
            wordno += 1;
        }
        Ok((p, relative, wordno))
    }

    /// The rows `r1..=r2` (1-based, clipped) of a matrix variable.
    fn rows_of(&mut self, m: MatName, r1: i32, r2: i32) -> Vec<Row> {
        let v = self.mats(m);
        let hi = (r2.max(0) as usize).min(v.len());
        let lo = r1.max(1) as usize;
        if lo > hi {
            return Vec::new();
        }
        v[lo - 1..hi].to_vec()
    }

    /// Applies a perturbation to a row's matrix (`rel`: times `delta`).
    fn perturbed(&mut self, row: &Row, pert: &[f64], relative: bool, delta: &[f64]) -> Mat {
        let m = row_to_mat(row);
        let mut params = self.affmat2vector(&m);
        for i in 0..12 {
            let add = if relative {
                pert[i] * delta[i]
            } else {
                pert[i]
            };
            params.put(i, params.get(i) + add);
        }
        self.vector2affine(&params, 12)
    }

    fn push_u0(&mut self, cost: f32, m: &Mat) {
        let mut row = vec![f64::from(cost)];
        row.extend(mat_to_16(m));
        self.usrmat[0].push(row);
    }

    /// `usrsetscale(usrscale, usrforce, ...)`.
    fn usrsetscale(&mut self, usrscale: f32, usrforce: bool) -> R<()> {
        let scale = usrscale;
        let forcescale = usrforce || self.o.force_scaling;
        if usrscale > 0.0 {
            self.requestedscale = usrscale;
        }
        if self.o.debug {
            self.el(&format!(
                "MJ DEBUG OUTPUT: scale = {}",
                cnum::stream(f64::from(scale))
            ));
            self.el(&format!(
                "MJ DEBUG OUTPUT: min_sampling = {}",
                cnum::stream(f64::from(self.o.min_sampling))
            ));
            let tv = self.testvol.clone();
            self.print_volume_info(&tv, "testvol DEBUG");
        }
        self.get_testvol()?;
        if forcescale || f64::from(self.o.min_sampling) <= 1.25 * f64::from(scale) {
            self.lastsampling = scale;
            self.global_sampling = scale;
            let tv = self.testvol.clone();
            let tw = self.global_testweight.clone();
            let testvolnew = self.filter_image(&tv, &tw, self.o.useweights, Filter::Blur, false)?;
            if self.o.useweights {
                self.global_sampling = scale;
                self.global_testweight = self.filter_weight(&tw, Filter::Blur)?;
            }
            self.testvol = testvolnew;
            let near = |a: f32| (f64::from(scale) - f64::from(a)).abs() < 0.01;
            let slot = if near(8.0) {
                if self.o.useweights {
                    self.global_refweight = self.global_refweight8.clone();
                }
                RefSlot::Ref8
            } else if near(4.0) {
                if self.o.useweights {
                    self.global_refweight = self.global_refweight4.clone();
                }
                RefSlot::Ref4
            } else if near(2.0) {
                if self.o.useweights {
                    self.global_refweight = self.global_refweight2.clone();
                }
                RefSlot::Ref2
            } else if self.global_scale1ok && near(1.0) {
                if self.o.useweights {
                    self.global_refweight = self.global_refweight1.clone();
                }
                RefSlot::Ref1
            } else {
                self.global_scale1ok = false;
                let tmp = self.get_refvol()?;
                let rw = self.global_refweight.clone();
                let r = self.resample_refvol(&tmp, &rw, scale, false)?;
                self.refvol = r;
                if self.o.useweights {
                    let rw = self.global_refweight.clone();
                    let w = self.resample_refvol(&rw, &rw, scale, true)?;
                    self.global_refweight = w.clone();
                    self.global_refweight1 = w;
                }
                RefSlot::Ref1
            };
            let mut w = Vec::new();
            let cog = self.testvol.cog_scaled_mm(&mut w);
            for l in &w {
                self.el(l);
            }
            let mut p = Pair::new(slot, self.o.useweights, cog);
            let bins = (self.o.no_bins as f32 / scale) as i32;
            let bins = if (self.o.no_bins as f32 / scale).is_nan() {
                i32::MIN
            } else {
                bins
            };
            self.setup_costfn(&mut p, self.currentcostfn, bins)?;
            if self.o.verbose >= 3
                && let Some(old) = &self.impair
            {
                let t = format!(
                    "Previous scale used {} cost function evaluations\n",
                    old.count
                );
                self.out.s(&t);
            }
            self.impair = Some(p);
        }
        Ok(())
    }

    /// `usrsetoption(words)`.
    fn usrsetoption(&mut self, words: &[String]) -> R<()> {
        let len = words.len();
        if len < 3 {
            self.el("Must specify an option and a value");
            return Ok(());
        }
        let option = words[1].clone();
        let mut fvalues = Vec::with_capacity(len - 2);
        for w in &words[2..] {
            let v = self.scalar_f(w);
            fvalues.push(f64::from(v));
        }
        if self.o.verbose > 2 {
            let t = format!("setoptions: {option} ");
            self.out.s(&t);
            self.out.row(&fvalues);
            self.out.s("\n");
        }
        match option.as_str() {
            "smoothing" => {
                self.smoothsize = fvalues[0] as f32;
                if let Some(p) = self.impair.as_mut() {
                    p.smoothsize = fvalues[0] as f32;
                }
            }
            "fuzzyfraction" => {
                self.fuzzyfrac = fvalues[0] as f32;
                if let Some(p) = self.impair.as_mut() {
                    p.fuzzyfrac = fvalues[0] as f32;
                }
            }
            "optimisationtype" => self.optimisationtype = words[2].clone(),
            "costfunction" => {
                let Some(c) = CostType::from_name(&words[2]) else {
                    self.el(&format!("Unrecognised cost function: {}", words[2]));
                    return Err(Stop::Exit(EXIT_MINUS_ONE));
                };
                self.currentcostfn = c;
                self.o.maincostfn = c;
                self.o.searchcostfn = c;
                if self.impair.is_some() {
                    self.setcostfntype(c)?;
                }
            }
            "tolerance" => {
                let k = f64::from(self.requestedscale);
                self.tolerance = fvalues.iter().map(|v| v / k).collect();
            }
            "rescaletolerance" => {
                if fvalues.len() != 1 {
                    return Err(Stop::Exception(FslError {
                        printed: Vec::new(),
                        type_name: "std::logic_error",
                        what: format!(
                            "matrix multiplication: incompatible matrix dimensions: {}x1 and {}x1",
                            self.tolerance.len(),
                            fvalues.len()
                        ),
                    }));
                }
                for t in &mut self.tolerance {
                    *t *= fvalues[0];
                }
            }
            "boundguess" => self.boundguess = fvalues,
            "nosubset" => self.usrsubset = false,
            "paramsubset" => {
                self.usrsubset = true;
                let noparams = fvalues[0] as i32;
                if (fvalues.len() as i64) < 12 * i64::from(noparams) + 1 {
                    self.el(&format!(
                        "Must specify at least {} for a {noparams} parameter subset mask",
                        12 * noparams
                    ));
                    return Ok(());
                }
                if noparams < 0 {
                    return Err(Stop::Refuse(
                        "a negative number of subset parameters".to_owned(),
                    ));
                }
                let n = noparams as usize;
                let mut m = Mat::zeros(12, n);
                for c in 0..n {
                    for r in 0..12 {
                        m.set(r, c, fvalues[c * 12 + 1 + r]);
                    }
                }
                if n > 0 && !pinv_mask(&m).1 {
                    return Err(Stop::Refuse(
                        "setoption paramsubset: a mask other than FLIRT's own and FSL's shipped \
                         schedules'; its pseudo-inverse (LAPACK's SVD) is not replicated yet"
                            .to_owned(),
                    ));
                }
                self.parammask = m;
            }
            "minsampling" => self.o.min_sampling = fvalues[0] as f32,
            "bbrstep" => {
                // set_bbr_step((int) value): a step below 1 is refused when BBR's cost uses it.
                let step = cnum::f64_to_i32(fvalues[0]);
                if let Some(p) = self.impair.as_mut() {
                    p.bbr.step = step;
                }
            }
            _ => self.el(&format!("Option {option} is unrecognised - ignoring")),
        }
        Ok(())
    }

    /// `interpretcommand(comline, skip, ...)`.
    fn interpretcommand(&mut self, comline: &str, skip: &mut bool) -> R<()> {
        let words = schedule::parseline(comline);
        if words.is_empty() {
            return Ok(());
        }
        if words[0].as_bytes().first() == Some(&b'#') {
            return Ok(());
        }
        if *skip {
            *skip = false;
            return Ok(());
        }
        let n = words.len();
        match words[0].as_str() {
            "copy" => {
                if n < 3 {
                    return Err(self.wrong_args("COPY"));
                }
                let (src, d1, d2) = self.matname(&words[1])?;
                let (dest, _, _) = self.matname(&words[2])?;
                if src == dest {
                    // usrcopy re-reads the size at each row: copying a matrix into itself keeps
                    // appending its own rows until row d2.
                    let v = self.mats(src);
                    let mut crow = d1.max(1) as usize;
                    while crow <= (d2.max(0) as usize).min(v.len()) {
                        let r = v[crow - 1].clone();
                        v.push(r);
                        crow += 1;
                    }
                } else {
                    let rows = self.rows_of(src, d1, d2);
                    self.mats(dest).extend(rows);
                }
            }
            "clear" => {
                if n < 2 {
                    return Err(self.wrong_args("CLEAR"));
                }
                let (src, _, _) = self.matname(&words[1])?;
                self.mats(src).clear();
            }
            "exit" => return Err(Stop::Exit(0)),
            "print" => {
                if n < 2 {
                    return Err(self.wrong_args("PRINT"));
                }
                let (src, d1, d2) = self.matname(&words[1])?;
                for r in self.rows_of(src, d1, d2) {
                    self.out.row(&r);
                }
            }
            "save" => {
                if n < 3 {
                    return Err(self.wrong_args("SAVE"));
                }
                let (src, d1, d2) = self.matname(&words[1])?;
                let rows = self.rows_of(src, d1, d2);
                let mut t = String::new();
                for r in &rows {
                    for v in r {
                        t.push_str(&cnum::fmt_f(*v, 6));
                        t.push(' ');
                    }
                    t.push('\n');
                }
                t.push('\n');
                if std::fs::write(&words[2], t).is_err() {
                    self.el(&format!("Could not open file {} for writing", words[2]));
                }
            }
            "read" => {
                if n < 3 {
                    return Err(self.wrong_args("READ"));
                }
                let (src, _, _) = self.matname(&words[1])?;
                match read_rows(&words[2], 17) {
                    Some(rows) if rows.first().is_some_and(Vec::is_empty) => {
                        return Err(Stop::Refuse(format!(
                            "read {}: a word that is not a number makes FLIRT read for ever (FSL-72)",
                            words[2]
                        )));
                    }
                    Some(rows) => *self.mats(src) = rows,
                    None => self.el(&format!("Could not open file {} for reading", words[2])),
                }
            }
            "printparams" | "saveparams" => {
                let file = words[0] == "saveparams";
                if (!file && n < 2) || (file && n < 3) {
                    return Err(self.wrong_args(if file { "SAVEPARAMS" } else { "PRINTPARAMS" }));
                }
                let (src, d1, d2) = self.matname(&words[1])?;
                let rows = self.rows_of(src, d1, d2);
                let mut t = String::new();
                // The stream's notation: cout's, or a fresh file stream's (general); printing the
                // parameters' row switches it to fixed for good.
                let mut fixed = if file { false } else { self.out.fixed };
                for r in &rows {
                    let m = row_to_mat(r);
                    let p = self.affmat2vector(&m);
                    // (row)(1) << " " << params.t() << endl
                    t.push_str(&if fixed {
                        cnum::fmt_f(r[0], 6)
                    } else {
                        cnum::stream(r[0])
                    });
                    t.push(' ');
                    for v in p.as_slice() {
                        t.push_str(&cnum::fmt_f(*v, 6));
                        t.push(' ');
                    }
                    t.push_str("\n\n");
                    fixed = true;
                }
                if file {
                    t.push('\n');
                    if std::fs::write(&words[2], t).is_err() {
                        self.el(&format!("Could not open file {} for writing", words[2]));
                    }
                } else {
                    self.out.fixed = fixed;
                    self.out.s(&t);
                }
            }
            "readparams" => {
                if n < 3 {
                    return Err(self.wrong_args("READPARAMS"));
                }
                let (src, _, _) = self.matname(&words[1])?;
                match read_rows(&words[2], 13) {
                    Some(rows) if rows.first().is_some_and(Vec::is_empty) => {
                        return Err(Stop::Refuse(format!(
                            "readparams {}: a word that is not a number makes FLIRT read for ever (FSL-72)",
                            words[2]
                        )));
                    }
                    Some(rows) => {
                        let mut out = Vec::new();
                        for r in rows {
                            let p = Mat::col(&r[1..13]);
                            let a = self.vector2affine(&p, 12);
                            // `float costval; fptr >> costval;`
                            let mut row = vec![f64::from(r[0] as f32)];
                            row.extend(mat_to_16(&a));
                            out.push(row);
                        }
                        *self.mats(src) = out;
                    }
                    None => self.el(&format!("Could not open file {} for reading", words[2])),
                }
            }
            "dualsort" => {
                if n < 3 {
                    return Err(self.wrong_args("DUALSORT"));
                }
                let (m1, _, _) = self.matname(&words[1])?;
                let (m2, _, _) = self.matname(&words[2])?;
                let a = self.mats(m1).clone();
                let b = self.mats(m2).clone();
                if a.len() != b.len() {
                    self.e("Cannot dual sort matrices of unequal size");
                    return Ok(());
                }
                let mut joined: Vec<Row> = a
                    .iter()
                    .zip(&b)
                    .map(|(x, y)| {
                        let mut r = x.clone();
                        r.resize(17, 0.0);
                        r.extend(y.iter().take(17));
                        r
                    })
                    .collect();
                sort_rows(&mut joined);
                *self.mats(m1) = joined.iter().map(|r| r[..17].to_vec()).collect();
                *self.mats(m2) = joined.iter().map(|r| r[17..].to_vec()).collect();
            }
            "sort" => {
                if n < 2 {
                    return Err(self.wrong_args("SORT"));
                }
                let (src, _, _) = self.matname(&words[1])?;
                sort_rows(self.mats(src));
            }
            "search" => {
                if n > 2 {
                    return Err(self.wrong_args("SEARCH"));
                }
                let usrdof = if n == 2 { self.scalar_i(&words[1]) } else { 12 };
                self.searchdof = usrdof;
                let mut list = self.optimise_strategy3()?;
                sort_rows(&mut list);
                for r in list {
                    self.searchoptmat.push(r[..17].to_vec());
                    self.preoptsearchmat.push(r[17..34].to_vec());
                }
            }
            "optimise" | "measurecost" => {
                let measure = words[0] == "measurecost";
                if n < 5 {
                    return Err(self.wrong_args(if measure { "MEASURECOST" } else { "OPTIMISE" }));
                }
                let usrdof = self.scalar_i(&words[1]);
                let (src, r1, r2) = self.matname(&words[2])?;
                let (pert, relative, wordno) = self.perturbation(&words, 3)?;
                let mut maxit = 4;
                if !measure && wordno < n {
                    maxit = self.scalar_i(&words[wordno]);
                }
                let delta = self.set_perturbations()?;
                let dof = self.o.dof.min(usrdof);
                for row in self.rows_of(src, r1, r2) {
                    let mut m = self.perturbed(&row, &pert, relative, &delta);
                    let c = if measure {
                        self.measure_cost(&m, dof)?
                    } else if self.usrsubset {
                        self.optimise_strategy0(&mut m, maxit)?
                    } else {
                        self.optimise_strategy1(&mut m, dof, maxit)?
                    };
                    self.push_u0(c, &m);
                }
            }
            "gridmeasurecost" => {
                if n < 7 {
                    return Err(self.wrong_args("GRIDMEASURECOST"));
                }
                let usrdof = self.scalar_i(&words[1]);
                let (src, r1, r2) = self.matname(&words[2])?;
                let (mut p1, mut step, mut p2) =
                    (vec![0.0f64; 12], vec![0.0f64; 12], vec![0.0f64; 12]);
                let mut wordno = 3usize;
                let mut compidx = 1usize;
                while wordno + 2 < n && words[wordno] != "rel" && words[wordno] != "abs" {
                    if compidx > 12 {
                        return Err(out_of_bounds());
                    }
                    p1[compidx - 1] = f64::from(self.scalar_f(&words[wordno]));
                    wordno += 1;
                    step[compidx - 1] = f64::from(self.scalar_f(&words[wordno]));
                    wordno += 1;
                    p2[compidx - 1] = f64::from(self.scalar_f(&words[wordno]));
                    wordno += 1;
                    compidx += 1;
                }
                let mut relative = true;
                while wordno < n {
                    if words[wordno] == "abs" {
                        relative = false;
                    }
                    wordno += 1;
                }
                let delta = self.set_perturbations()?;
                let dof = self.o.dof.min(usrdof);
                for row in self.rows_of(src, r1, r2) {
                    let m0 = row_to_mat(&row);
                    let params0 = self.affmat2vector(&m0);
                    // nsteps(n) = ceil(Max((p2-p1)/step, 1e-6) + 0.000001)
                    let nsteps: Vec<f64> = (0..12)
                        .map(|k| {
                            let q = (p2[k] - p1[k]) / step[k];
                            let q = if q > 1e-6 { q } else { 1e-6 };
                            (q + 0.000001).ceil()
                        })
                        .collect();
                    let mut nit: i32 = 1;
                    for &s in &nsteps {
                        nit = nit.wrapping_mul(volume::round_f64(s));
                    }
                    let mut nvec = [0.0f64; 12];
                    for _ in 1..=nit.max(0) {
                        let mut stepflag = 1.0;
                        for m in 0..12 {
                            nvec[m] += stepflag;
                            if volume::round_f64(nvec[m]) < volume::round_f64(nsteps[m]) {
                                stepflag = 0.0;
                            } else {
                                nvec[m] = 0.0;
                            }
                        }
                        let mut params = params0.clone();
                        for k in 0..12 {
                            let up = p1[k] + nvec[k] * step[k];
                            let add = if relative { up * delta[k] } else { up };
                            params.put(k, params.get(k) + add);
                        }
                        let m = self.vector2affine(&params, 12);
                        let c = self.measure_cost(&m, dof)?;
                        self.push_u0(c, &m);
                    }
                }
            }
            "aligncog" => {
                if n < 2 {
                    return Err(self.wrong_args("ALIGNCOG"));
                }
                let (src, d1, d2) = self.matname(&words[1])?;
                for row in self.rows_of(src, d1, d2) {
                    let mut m = row_to_mat(&row);
                    self.aligncog(&mut m);
                    let mut r = vec![row[0]];
                    r.extend(mat_to_16(&m));
                    self.usrmat[0].push(r);
                }
            }
            "alignpaxes" => {
                return Err(Stop::Refuse(
                    "the schedule command alignpaxes (principal axes) is not replicated yet"
                        .to_owned(),
                ));
            }
            "setscale" => {
                if n < 2 {
                    return Err(self.wrong_args("SETSCALE"));
                }
                let usrscale = self.scalar_f(&words[1]);
                let force = n >= 3 && words[2] == "force";
                self.usrsetscale(usrscale, force)?;
            }
            "setrow" => {
                if n < 18 {
                    return Err(self.wrong_args("SETROW"));
                }
                let (src, _, _) = self.matname(&words[1])?;
                let mut row = vec![0.0f64; 17];
                for c in 2..=17 {
                    row[c - 1] = cnum::atof(&words[c]);
                }
                self.mats(src).push(row);
            }
            "setrowqsform" => {
                if n < 2 {
                    return Err(self.wrong_args("SETROWQSFORM"));
                }
                let (src, _, _) = self.matname(&words[1])?;
                let q = if self.o.initmatsqform {
                    Mat::eye(4)
                } else {
                    let rv = self.refvol.clone();
                    let tv = self.testvol.clone();
                    let q = self.qsform_init_mat(&rv, &tv)?;
                    let ii = arma::inv(&self.initmat).map_err(|e| singular(&e.to_string()))?;
                    arma::mul_tinysq(&q, &ii)
                };
                let mut row = vec![0.0f64];
                row.extend(mat_to_16(&q));
                self.mats(src).push(row);
            }
            "setrowparams" => {
                if n < 14 {
                    return Err(self.wrong_args("SETROWPARAMS"));
                }
                let (src, _, _) = self.matname(&words[1])?;
                let p: Vec<f64> = (2..=13).map(|c| cnum::atof(&words[c])).collect();
                let a = self.vector2affine(&Mat::col(&p), 12);
                let mut row = vec![0.0f64];
                row.extend(mat_to_16(&a));
                self.mats(src).push(row);
            }
            "setoption" => {
                if n < 3 {
                    return Err(self.wrong_args("SETOPTION"));
                }
                self.usrsetoption(&words)?;
            }
            "if" => {
                if n < 4 {
                    return Err(self.wrong_args("IF"));
                }
                let a1 = self.scalar_f(&words[1]);
                let a2 = self.scalar_f(&words[3]);
                // FLIRT skips the next line unless the comparison holds: with a NaN every
                // comparison fails, so `!(a > b)` is not `a <= b`.
                #[allow(clippy::neg_cmp_op_on_partial_ord)]
                let skip_next = match words[2].as_str() {
                    ">" => !(a1 > a2),
                    "<" => !(a1 < a2),
                    "==" => a1 != a2,
                    "!=" => a1 == a2,
                    "<=" => !(a1 <= a2),
                    ">=" => !(a1 >= a2),
                    op => {
                        self.e(&format!("Cannot recognise operator {op} in IF statement\n"));
                        return Err(Stop::Exit(EXIT_MINUS_ONE));
                    }
                };
                *skip = skip_next;
            }
            other => {
                self.el(&format!("Unrecognised command {other}"));
                self.el(" ... ignoring");
            }
        }
        Ok(())
    }
}

/// `sort(begin, end, firstelementless)` on rows: by the first number (libstdc++'s introsort; rows
/// with equal costs keep an order this stable sort reproduces only when the costs differ).
fn sort_rows(rows: &mut [Row]) {
    larmorx_nc_core::stdsort::sort_by(rows, |a, b| a[0] < b[0]);
}

/// `usrread` and `usrreadparams`: rows of `width` numbers from a text file (C++ stream input),
/// the last incomplete one dropped; `None` when it cannot be opened.
fn read_rows(path: &str, width: usize) -> Option<Vec<Row>> {
    let text = std::fs::read_to_string(path).ok()?;
    let words: Vec<&str> = text.split_ascii_whitespace().collect();
    let nums: Vec<f64> = words.iter().map_while(|t| t.parse::<f64>().ok()).collect();
    if nums.len() < words.len() {
        // A word that is not a number: FSL's stream fails without reaching its end and the
        // loop never ends (FSL-72). The caller refuses.
        return Some(vec![Vec::new()]);
    }
    let mut rows: Vec<Row> = nums
        .chunks(width)
        .filter(|c| c.len() == width)
        .map(<[f64]>::to_vec)
        .collect();
    // A row is kept only if the stream is not at its end after its last number: a file whose
    // last number has no blank after it loses its last row.
    let ends_blank = text.ends_with(|c: char| c.is_ascii_whitespace());
    if !ends_blank && nums.len().is_multiple_of(width) && !rows.is_empty() {
        rows.pop();
    }
    Some(rows)
}

// ------------------------------------------------------------------------------------------------
// main

impl Flirt<'_> {
    /// `do_applyxfm()`.
    fn do_applyxfm(&mut self) -> R<()> {
        self.o.basescale = 1.0;
        let mut refvol = self.read_volume(&self.o.reffname.clone())?;
        let (mut testvol, _, _) = volume::read_volume4d::<f32>(&self.o.inputfname, self.settings)?;
        self.apply_basescale(&mut testvol);
        let t0 = testvol.sub_volume(0)?;
        if refvol.sform_code != nifti::XFORM_UNKNOWN
            && t0.sform_code != nifti::XFORM_UNKNOWN
            && self.o.verbose > 0
        {
            self.el("WARNING: Both reference and input images have an sform matrix set");
        }
        let dtype = volume::file_dtype(&self.o.inputfname, self.settings)?;
        if !self.o.forcedatatype {
            self.o.datatype = dtype;
        }
        self.set_initmat(&refvol, &t0)?;
        if self.o.verbose > 0 {
            if refvol.sform_code != nifti::XFORM_UNKNOWN {
                self.out
                    .s("The output image will use the sform from the reference image\n");
            } else if t0.sform_code != nifti::XFORM_UNKNOWN {
                self.out
                    .s("The output image will use the transformed sform from the input image\n");
            }
        }
        if self.o.verbose > 0 || self.o.printinit {
            self.out.s("Init Matrix = \n");
            let m = self.initmat.clone();
            self.out.mat(&m);
            self.out.s("\n");
        }
        if self.o.iso {
            let w = self.global_refweight.clone();
            refvol = self.resample_refvol(&refvol, &w, self.o.isoscale, false)?;
        }
        let min_sampling_ref = fmin(refvol.xdim(), fmin(refvol.ydim(), refvol.zdim()));
        // The pair a field map would build refers to this reference (`new Costfn(refvol, ...)`).
        self.refvol = refvol;
        if !self.o.outputfname.is_empty() {
            let mut cfg = t0.clone();
            self.configure_interpolation_bg(&mut cfg)?;
            let nt = testvol.tsize();
            // One volume on the reference's grid, reused for every time point; a 3D input's is
            // the output itself (FLIRT writes into the output's volumes directly).
            let mut out_t = self.refvol.clone();
            let mut outputvol = if nt > 1 {
                let r = &self.refvol;
                let mut o = Volume::new(r.xsize(), r.ysize(), r.zsize(), nt);
                o.copy_properties(r);
                o
            } else {
                empty()
            };
            for t in 0..nt {
                let mut vt = testvol.sub_volume(t)?;
                // configure_interpolation(testvol) came first: the blur reads the volume beyond
                // its edges by the extrapolation set there (extraslice with the background, or
                // the -setbackground constant).
                vt.extrap = cfg.extrap;
                vt.padvalue = cfg.padvalue;
                if self.o.interpmethod != Interp::NearestNeighbour && self.o.interpblur {
                    self.global_sampling = min_sampling_ref;
                    vt = self.filter(Filter::Blur, &vt)?;
                }
                vt.interp = cfg.interp;
                vt.kernel = cfg.kernel.clone();
                vt.padvalue = cfg.padvalue;
                vt.extrap = cfg.extrap;
                if self.o.verbose >= 2 {
                    let rv = std::mem::replace(&mut self.refvol, empty());
                    self.print_volume_info(&rv, "refvol");
                    self.refvol = rv;
                    self.print_volume_info(&testvol, "inputvol");
                }
                let im = self.initmat.clone();
                self.final_transform(&vt, &im, &mut out_t)?;
                if nt > 1 {
                    outputvol.set_sub_volume(t, &out_t)?;
                }
            }
            if nt == 1 {
                // The output's header is the reference's (what the 4D container FLIRT builds
                // with addvolume(refvol) keeps).
                outputvol = out_t;
                let r = &self.refvol;
                outputvol.sform = r.sform.clone();
                outputvol.qform = r.qform.clone();
                outputvol.sform_code = r.sform_code;
                outputvol.qform_code = r.qform_code;
            }
            let outputdtype = self.output_dtype(&outputvol);
            outputvol.display_max = 0.0;
            outputvol.display_min = 0.0;
            outputvol.pixdims[3] = testvol.tdim();
            let name = self.o.outputfname.clone();
            self.save_dtype(&outputvol, &name, outputdtype)?;
            if self.o.verbose >= 2 {
                self.print_volume_info(&outputvol, "Resampled volume");
            }
        }
        if !self.o.outputmatascii.is_empty() {
            let m = self.initmat.clone();
            self.save_matrix(&m);
        }
        Err(Stop::Exit(0))
    }

    /// `main` (inside its `try`).
    fn main(&mut self) -> R<()> {
        if !self.o.do_optimise {
            self.do_applyxfm()?;
        }
        self.set_basescale()?;
        self.refvol = self.get_refvol()?;
        self.get_testvol()?;
        // The volumes moved out for the call (no copies: FSL holds one of each).
        let rv = std::mem::replace(&mut self.refvol, empty());
        let tv = std::mem::replace(&mut self.testvol, empty());
        let r = self.set_initmat(&rv, &tv);
        self.refvol = rv;
        self.testvol = tv;
        r?;
        if self.refvol.sform_code != nifti::XFORM_UNKNOWN
            && self.testvol.sform_code != nifti::XFORM_UNKNOWN
            && self.o.verbose > 0
        {
            self.el("WARNING: Both reference and input images have an sform matrix set");
        }
        if self.o.verbose > 0 {
            if self.refvol.sform_code != nifti::XFORM_UNKNOWN {
                self.out
                    .s("The output image will use the sform from the reference image\n");
            }
            if self.testvol.sform_code != nifti::XFORM_UNKNOWN {
                self.out
                    .s("The output image will use the transformed sform from the input image\n");
            }
        }
        if self.o.verbose > 0 || self.o.printinit {
            self.out.s("Init Matrix = \n");
            let m = self.initmat.clone();
            self.out.mat(&m);
            self.out.s("\n");
        }
        let r = &self.refvol;
        let t = &self.testvol;
        let min_ref = fmin(r.xdim(), fmin(r.ydim(), r.zdim()));
        let min_test = fmin(t.xdim(), fmin(t.ydim(), t.zdim()));
        let min_sampling = fmax(min_ref, min_test).ceil();
        if !self.o.force_scaling && self.o.min_sampling < min_sampling {
            self.o.min_sampling = min_sampling;
        }
        if self.o.verbose >= 3 {
            let mut w = Vec::new();
            let rc = self.refvol.cog_scaled_mm(&mut w);
            let tc = self.testvol.cog_scaled_mm(&mut w);
            for l in &w {
                self.el(l);
            }
            self.out.s("CoG for refvol is:  ");
            self.out.row(&rc);
            self.out.s("CoG for testvol is:  ");
            self.out.row(&tc);
        }
        let ms = self.o.min_sampling;
        let uw = self.o.useweights;
        if self.o.resample {
            if self.o.verbose >= 2 {
                self.out.s("Subsampling the volumes\n");
            }
            // filter_image(refvol, refvol, ...): the input is replaced by the result.
            let rv = std::mem::replace(&mut self.refvol, empty());
            let rw = std::mem::replace(&mut self.global_refweight, empty());
            let r = self.resample_refvol(&rv, &rw, ms, false);
            drop(rv);
            self.global_refweight = rw;
            self.refvol = r?;
            self.global_sampling = ms;
            let rw = self.global_refweight.clone();
            self.global_refweight = self.filter_weight_if(&rw, Filter::ResampBlur)?;
            self.global_refweight1 = self.global_refweight.clone();
            // scale 2
            if ms < 1.9 {
                let (rv, w1) = (self.refvol.clone(), self.global_refweight.clone());
                self.refvol_2 = self.filter_image(&rv, &w1, uw, Filter::SubsampleBy2, false)?;
                if uw {
                    let w1 = self.global_refweight1.clone();
                    self.global_refweight2 = self.filter_weight(&w1, Filter::SubsampleBy2)?;
                }
            } else {
                self.refvol_2 = self.refvol.clone();
                if uw {
                    self.global_refweight2 = self.global_refweight1.clone();
                }
            }
            // scale 4
            if ms < 3.9 {
                let (rv, w2) = (self.refvol_2.clone(), self.global_refweight2.clone());
                self.refvol_4 = self.filter_image(&rv, &w2, uw, Filter::SubsampleBy2, false)?;
                if uw {
                    self.global_refweight4 = self.filter_weight(&w2, Filter::SubsampleBy2)?;
                }
            } else {
                self.refvol_4 = self.refvol_2.clone();
                if uw {
                    self.global_refweight4 = self.global_refweight2.clone();
                }
            }
            // scale 8
            if ms < 7.9 {
                let (rv, w4) = (self.refvol_4.clone(), self.global_refweight4.clone());
                self.refvol_8 = self.filter_image(&rv, &w4, uw, Filter::SubsampleBy2, false)?;
                if uw {
                    self.global_refweight8 = self.filter_weight(&w4, Filter::SubsampleBy2)?;
                }
            } else {
                self.refvol_8 = self.refvol_4.clone();
                if uw {
                    self.global_refweight8 = self.global_refweight4.clone();
                }
            }
        } else {
            self.refvol_8 = self.refvol.clone();
            self.refvol_4 = self.refvol.clone();
            self.refvol_2 = self.refvol.clone();
            if uw {
                self.global_refweight1 = self.global_refweight.clone();
                self.global_refweight2 = self.global_refweight.clone();
                self.global_refweight4 = self.global_refweight.clone();
                self.global_refweight8 = self.global_refweight.clone();
            }
        }
        if self.o.resample {
            self.global_sampling = 8.0;
            let (tv, tw) = (self.testvol.clone(), self.global_testweight.clone());
            let t8 = self.filter_image(&tv, &tw, uw, Filter::Blur, false)?;
            self.global_sampling = 8.0;
            self.global_testweight = self.filter_weight_if(&tw, Filter::Blur)?;
            self.testvol = t8;
        }
        if self.o.debug {
            for (v, n) in [
                (self.refvol_8.clone(), "refvol_8"),
                (self.refvol_4.clone(), "refvol_4"),
                (self.refvol_2.clone(), "refvol_2"),
                (self.refvol.clone(), "refvol"),
                (self.global_refweight1.clone(), "global_refweight1"),
                (self.global_refweight2.clone(), "global_refweight2"),
                (self.global_refweight4.clone(), "global_refweight4"),
                (self.global_refweight8.clone(), "global_refweight8"),
                (self.testvol.clone(), "testvol"),
                (self.global_testweight.clone(), "testweight"),
            ] {
                self.save_float(&v, n)?;
            }
        }
        self.impair = None;
        self.global_refweight = self.global_refweight8.clone();
        self.lastsampling = 8.0;
        let mut w = Vec::new();
        let cog = self.testvol.cog_scaled_mm(&mut w);
        for l in &w {
            self.el(l);
        }
        let mut p = Pair::new(RefSlot::Ref8, uw, cog);
        self.currentcostfn = self.o.maincostfn;
        self.setup_costfn(&mut p, self.currentcostfn, self.o.no_bins / 8)?;
        self.impair = Some(p);
        if self.o.verbose >= 2 {
            let tv = self.testvol.clone();
            self.print_volume_info(&tv, "TESTVOL");
        }
        // The schedule.
        let schedulecoms: Vec<String> = if self.o.schedulefname.is_empty() {
            let s = if self.o.mode2d {
                schedule::DEFAULT_2D
            } else {
                schedule::DEFAULT_3D
            };
            s.iter().map(|l| (*l).to_owned()).collect()
        } else {
            match std::fs::read(&self.o.schedulefname) {
                Ok(bytes) => {
                    // getline until eof: the last piece (empty after a final newline) included.
                    let text = String::from_utf8_lossy(&bytes);
                    text.split('\n').map(str::to_owned).collect()
                }
                Err(_) => {
                    let t = format!("Could not open file{}", self.o.schedulefname);
                    self.el(&t);
                    return Err(Stop::Exit(EXIT_MINUS_ONE));
                }
            }
        };
        let mut skip = false;
        for comline in &schedulecoms {
            if self.o.verbose >= 1 {
                self.out.s(&format!(" >> {comline}\n"));
            }
            self.interpretcommand(comline, &mut skip)?;
        }
        if self.o.debug
            && self
                .impair
                .as_ref()
                .is_some_and(|p| p.costtype == CostType::Bbr)
        {
            return Err(Stop::Refuse(
                "-debug with the bbr cost: FLIRT's BBR debug volume (`debug`) is not replicated yet"
                    .to_owned(),
            ));
        }
        if self.o.debug {
            self.el("Final DEBUG call in FLIRT");
            self.el("Pointer to impair is (set)");
            self.el("Final DEBUG call in FLIRT 2");
            self.el(&format!(
                "Scale is:{}",
                cnum::stream(f64::from(self.requestedscale))
            ));
            self.interpretcommand(
                "measurecost 12 U:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 8",
                &mut skip,
            )?;
            self.el("Final DEBUG call in FLIRT 3");
            self.el("Final DEBUG call in FLIRT 4");
        }
        // The outputs.
        if let Some(first) = self.usrmat[0].first().cloned() {
            let matresult = row_to_mat(&first);
            let oldbasescale = self.o.basescale;
            self.o.basescale = 1.0;
            self.impair = None;
            self.testvol = self.read_volume(&self.o.inputfname.clone())?;
            self.refvol = self.read_volume(&self.o.reffname.clone())?;
            if self.o.verbose >= 2 {
                let (tv, rv) = (self.testvol.clone(), self.refvol.clone());
                self.print_volume_info(&tv, "testvol");
                self.print_volume_info(&rv, "refvol");
            }
            let mut finalmat = arma::mul_tinysq(&matresult, &self.initmat);
            for r in 0..3 {
                finalmat.set(r, 3, finalmat.at(r, 3) * f64::from(oldbasescale));
            }
            if self.o.verbose >= 2 {
                self.out.s("Final transform matrix is:\n");
                self.out.mat(&finalmat);
                self.out.s("\n");
            }
            self.save_matrix(&finalmat);
            if !self.o.outputfname.is_empty() {
                // No more copies than FLIRT holds: the reference, the output on its grid, the
                // input (blurred in place).
                let mut newtestvol = self.refvol.clone();
                let r = &self.refvol;
                let min_sampling_ref = fmin(r.xdim(), fmin(r.ydim(), r.zdim()));
                if self.o.interpmethod != Interp::NearestNeighbour && self.o.interpblur {
                    self.global_sampling = min_sampling_ref;
                    let tv = std::mem::replace(&mut self.testvol, empty());
                    self.testvol = self.filter(Filter::Blur, &tv)?;
                }
                let mut tv = std::mem::replace(&mut self.testvol, empty());
                let c = self.configure_interpolation_bg(&mut tv);
                self.testvol = tv;
                c?;
                let tv = std::mem::replace(&mut self.testvol, empty());
                let done = self.final_transform(&tv, &finalmat, &mut newtestvol);
                self.testvol = tv;
                done?;
                if self.o.verbose >= 2 {
                    self.print_volume_info(&newtestvol, "Transformed testvol");
                }
                let outputdtype = self.output_dtype(&newtestvol);
                newtestvol.display_max = 0.0;
                newtestvol.display_min = 0.0;
                let name = self.o.outputfname.clone();
                self.save_dtype(&newtestvol, &name, outputdtype)?;
            }
            if self.o.outputmatascii.is_empty() {
                self.out.s("\nFinal result: \n");
                self.out.mat(&finalmat);
                self.out.s("\n");
            }
        } else {
            self.el("Failed to calculate any transformation matrix");
        }
        Ok(())
    }

    /// `filter_weight(global_refweight, global_refweight, filter)` when weights are used (FLIRT
    /// filters the unused weight volume too; with no weights it is empty and nothing changes).
    fn filter_weight_if(&mut self, w: &Volume, f: Filter) -> R<Volume> {
        if w.nvoxels() == 0 || w.data.is_empty() {
            return Ok(w.clone());
        }
        self.filter_weight(w, f)
    }

    /// `save_volume(vol, name)` in float (FLIRT's `-debug` volumes).
    fn save_float(&mut self, v: &Volume, name: &str) -> R<()> {
        let settings = self.output_settings();
        match v.save(name, &settings, DESCRIP) {
            Ok(()) => Ok(()),
            Err(SaveError::OutputType(e)) => {
                for l in &e.printed {
                    self.el(l);
                }
                Err(Stop::Exit(1))
            }
            Err(SaveError::Exception(e)) => Err(e.into()),
        }
    }
}

/// The prefix of the replica's own lines on standard error.
pub const PREFIX: &str = "larmorx-nc";

/// `flirt` with FLIRT's arguments (without the program name); returns the exit status.
pub fn run(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let o = match parse(args) {
        Parsed::Run(o) => *o,
        Parsed::Exit {
            code,
            stdout,
            stderr,
        } => {
            let _ = out.write_all(stdout.as_bytes());
            let _ = err.write_all(stderr.as_bytes());
            return code;
        }
    };
    let mut f = Flirt::new(o, mode, settings, threads, out, err);
    let r = f.main();
    f.flush_quat_msgs();
    if matches!(r, Err(Stop::Exception(_))) {
        // FSL-68: FLIRT's main catches every exception and returns 0 (rule B: a clear error).
        let detail = if mode.fix() {
            "the run failed: exit status 1 (FLIRT prints the message and exits with status 0)"
        } else {
            "the run failed and exits with status 0, as FLIRT does"
        };
        f.notice(&defects::FSL_68, detail);
    }
    let notices = std::mem::take(&mut f.notices);
    let code = match r {
        Ok(()) => 0,
        Err(Stop::Exit(c)) => c,
        Err(Stop::Exception(e)) => {
            let t = format!("{}{}\n", e.printed_text(), e.what);
            f.e(&t);
            u8::from(mode.fix())
        }
        Err(Stop::Abort(t)) => {
            f.e(&t);
            f.e(&format!(
                "{PREFIX}: FLIRT aborts here (signal 6); the replica exits with status 1\n"
            ));
            1
        }
        Err(Stop::Crash(t)) => {
            f.e(&format!("{PREFIX}: {t}; the replica exits with status 1\n"));
            1
        }
        Err(Stop::Refuse(t)) => {
            f.e(&format!("{PREFIX}: error: {t}\n"));
            1
        }
    };
    for n in &notices {
        f.e(&format!("{PREFIX}: {}\n", n.line()));
    }
    let _ = f.out.w.flush();
    let _ = f.err.flush();
    code
}

#[cfg(test)]
mod tests {
    use super::{Mat, pinv_mask};

    fn mask(cols: &[&[usize]]) -> Mat {
        let mut m = Mat::zeros(12, cols.len());
        for (c, rows) in cols.iter().enumerate() {
            for &r in *rows {
                m.set(r, c, 1.0);
            }
        }
        m
    }

    #[test]
    fn the_masks_whose_bits_the_probe_confirms_are_accepted() {
        // FLIRT's 12-dof identity, the shipped schedules' single translations, the 2D subset
        let (p, ok) = pinv_mask(&Mat::eye(12));
        assert!(ok);
        assert_eq!(p.at(3, 3), 1.0);
        assert!(pinv_mask(&mask(&[&[3], &[4], &[5]])).1);
        assert!(pinv_mask(&mask(&[&[2], &[3], &[4]])).1);
        // the search's mask: one scale on rows 7 to 9 ((1/sqrt 3)^2, FSL's LAPACK bits)
        let (p, ok) = pinv_mask(&mask(&[&[6, 7, 8], &[3], &[4], &[5]]));
        assert!(ok);
        assert_eq!(p.at(0, 7).to_bits(), 0x3FD5_5555_5555_5557);
        assert_eq!(p.at(1, 3), 1.0);
        // FSL's sch2D_4dof: one scale for x and y ((1/sqrt 2)^2)
        let (p, ok) = pinv_mask(&mask(&[&[2], &[3], &[4], &[6, 7]]));
        assert!(ok);
        assert_eq!(p.at(3, 6).to_bits(), 0x3FDF_FFFF_FFFF_FFFE);
    }

    #[test]
    fn other_masks_are_not_claimed_exact() {
        assert!(!pinv_mask(&mask(&[&[0, 1]])).1);
        assert!(!pinv_mask(&mask(&[&[0, 1, 2], &[3, 4]])).1);
        assert!(!pinv_mask(&mask(&[&[6, 7, 8], &[3], &[4]])).1);
        let mut half = Mat::zeros(12, 1);
        half.set(0, 0, 0.5);
        assert!(!pinv_mask(&half).1);
        // a row shared by two columns
        assert!(!pinv_mask(&mask(&[&[0], &[0, 1]])).1);
    }
}

/// FSL-75: refused in both modes where FSL's background value of `v` is undefined (its
/// buffer too small for the border it writes, or empty).
fn background_defined(v: &Volume) -> R<()> {
    match v.background_undefined() {
        Some(why) => Err(Stop::Refuse(why)),
        None => Ok(()),
    }
}
