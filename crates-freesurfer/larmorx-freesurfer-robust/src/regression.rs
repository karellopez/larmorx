// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `Regression<T>` (queue 3.2-03): the robust registration's solver of an overdetermined system
//! `A p = b`, translated from FreeSurfer 8.2.0's `mri_robust_register/Regression.h` and
//! `Regression.cpp` for `float` and `double`:
//!
//! - [`Regression::get_robust_est_w`] (`getRobustEstW`): Tukey's biweight by iteratively
//!   reweighted least squares (Beaton and Tukey; Reuter et al., NeuroImage 2010), with `A`
//!   (`getRobustEstWAB`) or for one location parameter (`getRobustEstWB`);
//! - [`Regression::get_ls_est`] (`getLSEst`): least squares through VNL's pseudo-inverse;
//! - [`Regression::get_weighted_ls_est`], [`Regression::get_weighted_ls_est_float`]: the weighted
//!   least squares by VNL's QR decomposition (the second in `float` whatever `T`);
//! - [`get_sigma_mad`] (`getSigmaMAD`) and [`get_sqrt_tukey_dia_weights`]
//!   (`getSqrtTukeyDiaWeights`).
//!
//! `getTukeyPartialSat` and `plotPartialSat` (no live caller), `VectorMedian` and the free
//! `getTukeyBiweight` (never called) are not translated. VNL's QR is
//! `larmorx_core::vnl_qr::Qr<T>` (LINPACK's `xqrdc`/`xqrsl`), its SVDs `larmorx_core::vnl_svd`
//! and `vnl_svd_f32` (LINPACK's `xsvdc`).
//!
//! Defects: **FS-1065** (`getRobustEstWB` with an empty weights vector writes through a null
//! pointer: FreeSurfer crashes; both modes size the weights to `b`), **FS-1066** (`getLSEst`
//! without `A` writes its mean past its one-element result: both modes return the mean),
//! **FS-1067** (fewer rows than parameters: VNL's QR reads past its arrays; an error by default),
//! **ITK-116** (VNL's SVD never returning: an error in both modes).

use std::io::Write;

use larmorx_core::cfmt::ostream;
use larmorx_core::defects::{FS_1065, FS_1066, FS_1067, ITK_113};
use larmorx_core::vnl_qr::{Qr, RANK_DEFICIENT};
use larmorx_core::vnl_svd::vnl_svd_full;
use larmorx_core::vnl_svd_f32::vnl_svd_f32_full;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::numerics::{ITK_ROOT, suspicious_svdc_f32};

use crate::mymatrix::suspicious_svdc;
use larmorx_core::itk_real::ItkReal;

use crate::real::{Mat, Real};
use crate::robust_gauss;

/// `SATr`: Tukey's saturation for Gaussian noise.
pub const SAT_R: f64 = 4.685;
/// The robust sigma's factor (`1.4826`, the default `sig` and `d`).
pub const SIG: f64 = 1.4826;

/// `Regression<T>`: the system and what the last solve left.
pub struct Regression<'a, T: Real> {
    a: Option<&'a Mat<T>>,
    b: &'a mut [T],
    /// `getLastError()`.
    pub lasterror: f64,
    /// `getLastWeightPercent()`.
    pub lastweight: f64,
    /// `getLastZeroWeightPercent()`.
    pub lastzero: f64,
    verbose: i32,
    floatsvd: bool,
}

/// The lines VNL's QR solve prints on standard error when the back-substitution meets a zero
/// diagonal of `R` (ITK 4.13.2's `vnl_qr.hxx`, the official build's path).
fn rank_deficient(err: &mut dyn Write, info: usize) {
    let _ = writeln!(err, "{ITK_ROOT}/{RANK_DEFICIENT}{info}");
}

impl<'a, T: Real> Regression<'a, T> {
    /// `Regression<T>(A, b)`.
    pub fn new(a: &'a Mat<T>, b: &'a mut [T]) -> Self {
        Regression {
            a: Some(a),
            b,
            lasterror: -1.0,
            lastweight: -1.0,
            lastzero: -1.0,
            verbose: 1,
            floatsvd: false,
        }
    }

    /// `Regression<T>(b)`: one location parameter (`A` the column of ones).
    pub fn new_b(b: &'a mut [T]) -> Self {
        Regression {
            a: None,
            b,
            lasterror: -1.0,
            lastweight: -1.0,
            lastzero: -1.0,
            verbose: 1,
            floatsvd: false,
        }
    }

    /// `setVerbose(v)`: clamped to `0..=2`.
    pub fn set_verbose(&mut self, v: i32) {
        self.verbose = v.clamp(0, 2);
    }

    /// `setFloatSvd(b)`: whether the weighted least squares run in `float`.
    pub fn set_float_svd(&mut self, b: bool) {
        self.floatsvd = b;
    }

    /// `b` as the last call left it (`getRobustEstWB` reorders it).
    pub fn b(&self) -> &[T] {
        self.b
    }

    /// `getRobustEst(sat, sig)`: [`Self::get_robust_est_w`] with an empty weights vector.
    pub fn get_robust_est(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        sat: f64,
        sig: f64,
    ) -> Result<Vec<T>, Fatal> {
        let mut w = Vec::new();
        self.get_robust_est_w(g, out, err, &mut w, sat, sig)
    }

    /// `getRobustEstW(w, sat, sig)`: the robust solution, the square roots of its weights into
    /// `w`; with `A` `getRobustEstWAB`, else `getRobustEstWB` (a one-element result).
    pub fn get_robust_est_w(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        w: &mut Vec<T>,
        sat: f64,
        sig: f64,
    ) -> Result<Vec<T>, Fatal> {
        if self.a.is_some() {
            self.get_robust_est_wab(g, out, err, w, sat, sig)
        } else {
            let mu = self.get_robust_est_wb(g, w, sat);
            Ok(vec![T::narrow(mu)])
        }
    }

    /// `getRobustEstWB(w, sat, sig)`: the robust location of `b` (Tukey's biweight from the
    /// median, at most 20 steps while the change is above `2e-6`; it stops when the robust sigma
    /// is below `2e-6`, the weights then 1). The median reorders `b` in place, as FreeSurfer's
    /// does. **FS-1065:** a weights vector of another size than `b` (FreeSurfer's only caller
    /// passes an empty one) is written past its end by FreeSurfer (a crash for the empty one);
    /// both modes size it to `b` first, noted.
    fn get_robust_est_wb(&mut self, g: &mut Globals, w: &mut Vec<T>, sat: f64) -> f64 {
        const MAXIT: i32 = 20;
        const EPS: f64 = 2e-6;
        let n = self.b.len();
        let mut mu = robust_gauss::median(g, self.b);
        // `muold = mu + 100` in T
        let mut muold = mu.sse_add(T::narrow(100.0));
        let mut count = 0;
        for x in w.iter_mut() {
            *x = T::ONE;
        }
        let mut sized = false;
        while (muold.sse_sub(mu)).fabs().wide() > EPS && count < MAXIT {
            count += 1;
            muold = mu;
            // r = muold - b: (-b[i]) + muold
            let r: Vec<T> = self.b.iter().map(|&x| x.negate().sse_add(muold)).collect();
            let sigma = get_sigma_mad(g, &r, T::narrow(SIG));
            if sigma.wide() < EPS {
                mu = muold;
                for x in w.iter_mut() {
                    *x = T::ONE;
                }
                break;
            }
            let s = T::narrow(1.0 / sigma.wide());
            let rdsigma: Vec<T> = r.iter().map(|&x| x.sse_mul(s)).collect();
            if w.len() < n && !sized {
                g.notices.add(
                    &FS_1065,
                    format!(
                        "the robust location's weights vector has {} entries for {n} values: sized to the values (FreeSurfer writes past it)",
                        w.len()
                    ),
                );
                w.resize(n, T::ZERO);
                sized = true;
            }
            get_sqrt_tukey_dia_weights(&rdsigma, w, sat);
            let (mut d1, mut d2) = (0.0f64, 0.0f64);
            for (i, &bi) in self.b.iter().enumerate() {
                let wi = w[i].sse_mul(w[i]).wide();
                // the official build multiplies b[i] * wi
                d1 = d1.sse_add(bi.wide().sse_mul(wi));
                d2 = d2.sse_add(wi);
            }
            // mu = muold - (T) d1 / d2: ((T) d1) / d2 in double, then muold - it
            let q = T::narrow(d1).wide().sse_div(d2);
            mu = T::narrow(muold.wide().sse_sub(q));
        }
        mu.wide()
    }

    /// `getRobustEstWAB(w, sat, sig)`: Tukey's biweight by iteratively reweighted least squares:
    /// from the residuals `b`, each step the robust sigma (`getSigmaMAD`), the square roots of the
    /// weights of the scaled residuals ([`get_sqrt_tukey_dia_weights`]; all 1 and `Sigma too small:
    /// <sigma> (identical images?)` on standard output when sigma is below `2e-12`), the weighted
    /// least squares, the new residuals and the error `sum(w r^2) / sum(w)`; at most 20 steps,
    /// until the error grows (the previous step is then kept) or falls to `2e-12`. Then the mean
    /// weight over the entries of `b` above `0.00001` in magnitude and the share of them below
    /// `0.1`. Verbose 2 prints the call, the step kept and these numbers.
    fn get_robust_est_wab(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        wfinal: &mut Vec<T>,
        sat: f64,
        sig: f64,
    ) -> Result<Vec<T>, Fatal> {
        let a = self.a.expect("getRobustEstWAB with A");
        if self.verbose > 1 {
            let _ = write!(
                out,
                "  Regression<T>::getRobustEstWAB( {} , {} ) {}\n",
                ostream(sat),
                ostream(sig),
                if self.floatsvd {
                    "  FLOAT version "
                } else {
                    "  DOUBLE version "
                }
            );
        }
        const MAXIT: usize = 20;
        const EPS: f64 = 2e-12;
        let mut errv = vec![T::ZERO; MAXIT + 1];
        errv[0] = T::narrow(f64::INFINITY);
        errv[1] = T::narrow(1e20);
        let (arows, acols) = (a.rows, a.cols);
        let mut r: Vec<T> = self.b.to_vec();
        let mut p: Vec<T> = vec![T::ZERO; acols];
        let mut w: Vec<T> = vec![T::ZERO; arows];
        let mut lastp: Vec<T> = vec![T::ZERO; acols];
        let mut lastw: Vec<T> = vec![T::ZERO; arows];
        let mut count = 0usize;
        let mut incr = 0;
        loop {
            count += 1;
            if count > 1 {
                std::mem::swap(&mut lastp, &mut p);
                std::mem::swap(&mut lastw, &mut w);
            }
            let sigma = get_sigma_mad(g, &r, T::narrow(SIG)).wide();
            if sigma < EPS {
                let _ = writeln!(out, "  Sigma too small: {} (identical images?)", ostream(sigma));
                w.fill(T::ONE);
            } else {
                let s = T::narrow(1.0 / sigma);
                for x in r.iter_mut() {
                    *x = x.sse_mul(s);
                }
                get_sqrt_tukey_dia_weights(&r, &mut w, sat);
            }
            drop(std::mem::take(&mut r)); // r->clear(): the residuals freed before the solve
            p = if self.floatsvd {
                self.get_weighted_ls_est_float(g, err, &w)?
            } else {
                self.get_weighted_ls_est(g, err, &w)?
            };
            let ap = a.mul_vec(&p);
            r = self
                .b
                .iter()
                .zip(&ap)
                .map(|(&x, &y)| x.sse_sub(y))
                .collect();
            // the official build multiplies t2 * t1 (the residual's square first)
            let (mut swr, mut sw) = (T::ZERO, T::ZERO);
            for (&wi, &ri) in w.iter().zip(&r) {
                let t1 = wi.sse_mul(wi);
                let t2 = ri.sse_mul(ri);
                sw = sw.sse_add(t1);
                swr = swr.sse_add(t2.sse_mul(t1));
            }
            errv[count] = swr.sse_div(sw);
            if errv[count - 1] <= errv[count] {
                incr += 1;
            }
            if !(incr < 1 && count < MAXIT && errv[count].wide() > EPS) {
                break;
            }
        }
        drop(r);
        let pfinal;
        if errv[count] > errv[count - 1] {
            pfinal = lastp;
            *wfinal = lastw;
            if self.verbose > 1 {
                let _ = writeln!(
                    out,
                    "     Step: {} ERR: {}",
                    count as i64 - 2,
                    ostream(errv[count - 1].wide())
                );
            }
            self.lasterror = errv[count - 1].wide();
        } else {
            pfinal = p;
            *wfinal = w;
            if self.verbose > 1 {
                let _ = writeln!(
                    out,
                    "     Step: {} ERR: {}",
                    count as i64 - 1,
                    ostream(errv[count].wide())
                );
            }
            self.lasterror = errv[count].wide();
        }
        // the statistics of the weights
        let (mut d, mut dd, mut ddcount) = (0.0f64, 0.0f64, 0.0f64);
        let mut zcount = 0i32;
        for (i, &val) in wfinal.iter().enumerate() {
            d = d.sse_add(val.wide());
            if self.b[i].fabs().wide() > 0.00001 {
                dd = dd.sse_add(val.wide());
                ddcount += 1.0;
                if val.wide() < 0.1 {
                    zcount += 1;
                }
            }
        }
        let _ = d;
        dd = dd.sse_div(ddcount);
        let zero = f64::from(zcount).sse_div(ddcount);
        if self.verbose > 1 {
            let _ = write!(
                out,
                "          weights average: {}  zero: {}",
                ostream(dd),
                ostream(zero)
            );
        }
        self.lastweight = dd;
        self.lastzero = zero;
        Ok(pfinal)
    }

    /// `getWeightedLSEst(w)`: `M p = v` with `M = diag(w) A` and `v = diag(w) b` by
    /// `vnl_qr<T>` (`w` the square roots of the weights). VNL prints `... vnl_qr<T>::solve() :
    /// matrix is rank-deficient by <j>` on standard error when the back-substitution meets a zero
    /// diagonal. **FS-1067** for fewer rows than columns.
    pub fn get_weighted_ls_est(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        w: &[T],
    ) -> Result<Vec<T>, Fatal> {
        let a = self.a.expect("getWeightedLSEst with A");
        let (rows, cols) = (a.rows, a.cols);
        too_few_rows(g, err, rows, cols)?;
        // wA, built column by column as VNL's QR stores it (FreeSurfer keeps wA and VNL's
        // transposed copy; one copy here, the same numbers). The official build's operand order
        // (of two NaNs the first comes out): `A * w` where it vectorises the row (four or more
        // columns in float, three or more in double), else `w * A`
        let mut wa = vec![T::ZERO; rows * cols];
        let a_first = cols >= if T::SINGLE { 4 } else { 3 };
        for rr in 0..rows {
            for cc in 0..cols {
                let (x, y) = (a.at(rr, cc), w[rr]);
                wa[rr + rows * cc] = if a_first { x.sse_mul(y) } else { y.sse_mul(x) };
            }
        }
        let qr = Qr::<T>::from_column_major(wa, rows, cols);
        let wb: Vec<T> = self
            .b
            .iter()
            .zip(w)
            .map(|(&x, &y)| x.sse_mul(y))
            .collect();
        let (p, info) = qr.solve(&wb);
        if info > 0 {
            rank_deficient(err, info);
            one_row_unwritten(g, rows);
        }
        Ok(p)
    }

    /// `getWeightedLSEstFloat(w)`: [`Self::get_weighted_ls_est`] in `float`: `M` and `v` the
    /// products in `T` rounded to `float`, `vnl_qr<float>`, the solution widened back to `T`.
    pub fn get_weighted_ls_est_float(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        w: &[T],
    ) -> Result<Vec<T>, Fatal> {
        let a = self.a.expect("getWeightedLSEstFloat with A");
        let (rows, cols) = (a.rows, a.cols);
        too_few_rows(g, err, rows, cols)?;
        // wA column by column (one copy, as above). The official build's operand order (of two
        // NaNs the first comes out): in float `A * w` for four or more columns, else `w * A`; in
        // double `A * w` but for the third of the columns after the last multiple of four (the
        // first three when there are fewer than four), `w * A`
        let mut wa = vec![0.0f32; rows * cols];
        let base = if cols >= 4 { cols / 4 * 4 } else { 0 };
        for rr in 0..rows {
            for cc in 0..cols {
                let (x, y) = (a.at(rr, cc), w[rr]);
                let a_first = if T::SINGLE {
                    cols >= 4
                } else {
                    cc < base || cc - base != 2
                };
                let v = if a_first { x.sse_mul(y) } else { y.sse_mul(x) };
                wa[rr + rows * cc] = v.f32_of();
            }
        }
        let qr = Qr::<f32>::from_column_major(wa, rows, cols);
        let wb: Vec<f32> = self
            .b
            .iter()
            .zip(w)
            .map(|(&x, &y)| x.sse_mul(y).f32_of())
            .collect();
        let (p, info) = qr.solve(&wb);
        if info > 0 {
            rank_deficient(err, info);
            one_row_unwritten(g, rows);
        }
        Ok(p.into_iter().map(T::of_f32).collect())
    }

    /// `getLSEst()`: least squares, `p = pinv(A) b` (VNL's SVD of `A`, `V W^+ U^T`), and the
    /// squared error `sum((A p - b)^2)` as the last error (the weight percentages -1). An SVD that
    /// did not converge: VNL's lines, then `    Regression<T>::getLSEst   could not compute
    /// pseudo inverse!` on standard error and the end of the program (status 1). Without `A`:
    /// the mean of `b` (FS-1066).
    pub fn get_ls_est(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
    ) -> Result<Vec<T>, Fatal> {
        self.lastweight = -1.0;
        self.lastzero = -1.0;
        let Some(a) = self.a else {
            let mut d = T::ZERO;
            for &x in self.b.iter() {
                d = d.sse_add(x);
            }
            g.notices.add(
                &FS_1066,
                "the least-squares location without A: its mean returned (FreeSurfer writes it past its one-element result)",
            );
            return Ok(vec![d.sse_div(T::narrow(self.b.len() as f64))]);
        };
        let (rows, cols) = (a.rows, a.cols);
        if rows < cols {
            g.notices.add(
                &FS_1067,
                format!("a least-squares system of {rows} rows for {cols} parameters"),
            );
            if !g.mode().reproduce() {
                let _ = writeln!(
                    err,
                    "Regression: {rows} equations for {cols} parameters: too few usable voxels for the transformation (FS-1067)"
                );
                return Err(Fatal::Exit(1));
            }
            let _ = writeln!(
                err,
                "Regression<T>::getLSEst of fewer rows than columns is not ported yet"
            );
            return Err(Fatal::Exit(1));
        }
        let Some(svd) = SvdT::of(g, a) else {
            let _ = writeln!(
                err,
                "Regression<T>::getLSEst: VNL's singular value decomposition never returns on this matrix (an infinite or NaN entry)"
            );
            return Err(Fatal::Exit(1));
        };
        if let Some(wn) = &svd.warning {
            let _ = err.write_all(wn.as_bytes());
            let _ = writeln!(
                err,
                "    Regression<T>::getLSEst   could not compute pseudo inverse!"
            );
            return Err(Fatal::Exit(1));
        }
        let ai = svd.pinverse();
        let p = ai.mul_vec(self.b);
        drop(ai);
        let ap = a.mul_vec(&p);
        let mut serror = 0.0f64;
        for (&x, &y) in ap.iter().zip(self.b.iter()) {
            let rr = x.sse_sub(y);
            serror = serror.sse_add(rr.sse_mul(rr).wide());
        }
        self.lasterror = serror;
        Ok(p)
    }
}

/// ITK-113: VNL's QR solve of one row whose entry is 0 leaves its solution unwritten (memory
/// contents; the port's 0), noted.
fn one_row_unwritten(g: &mut Globals, rows: usize) {
    if rows == 1 {
        g.notices.add(
            &ITK_113,
            "VNL's QR solve of a one-row system with a zero entry: the solution never written (0 here)",
        );
    }
}

/// FS-1067: fewer rows (usable voxels) than parameters. VNL's QR solve then reads past its
/// arrays (memory FreeSurfer does not own); by default a clear error (rule B), noted in both
/// modes; `reproduce` ends too (its numbers would be memory contents).
fn too_few_rows(g: &mut Globals, err: &mut dyn Write, rows: usize, cols: usize) -> Result<(), Fatal> {
    if rows >= cols {
        return Ok(());
    }
    g.notices.add(
        &FS_1067,
        format!("a weighted least-squares system of {rows} rows for {cols} parameters"),
    );
    let _ = writeln!(
        err,
        "Regression: {rows} equations for {cols} parameters: too few usable voxels for the transformation (FS-1067)"
    );
    Err(Fatal::Exit(1))
}

/// What `vnl_svd<T>(A)` keeps for its pseudo-inverse.
struct SvdT<T> {
    /// `U` (rows x cols, row-major).
    u: Mat<T>,
    /// `V` (cols x cols, row-major).
    v: Mat<T>,
    /// `Winverse_`'s diagonal and the rank (`zero_out_absolute(0)`).
    winv: Vec<T>,
    rank: usize,
    /// VNL's lines when LINPACK did not converge.
    warning: Option<String>,
}

impl<T: Real> SvdT<T> {
    /// `vnl_svd<T>(A)`, or `None` where VNL never returns (ITK-116, noted).
    fn of(g: &mut Globals, a: &Mat<T>) -> Option<SvdT<T>> {
        let (n, p) = (a.rows, a.cols);
        let (u, w, v, info, notices, warning);
        if T::SINGLE {
            let m: Vec<f32> = a.data.iter().map(|&x| x.f32_of()).collect();
            let s = vnl_svd_f32_full(&m, n, p);
            notices = s.notices.clone();
            if s.never_returns() {
                for x in &notices {
                    g.notices.add(x.defect, x.detail.clone());
                }
                return None;
            }
            warning = (s.info != 0).then(|| suspicious_svdc_f32(&m, n, p, s.info));
            info = s.info;
            u = s.u.iter().map(|&x| T::of_f32(x)).collect::<Vec<T>>();
            w = s.w.iter().map(|&x| T::of_f32(x)).collect::<Vec<T>>();
            v = s.v.iter().map(|&x| T::of_f32(x)).collect::<Vec<T>>();
        } else {
            let m: Vec<f64> = a.data.iter().map(|&x| x.wide()).collect();
            let s = vnl_svd_full(&m, n, p);
            notices = s.notices.clone();
            if s.never_returns() {
                for x in &notices {
                    g.notices.add(x.defect, x.detail.clone());
                }
                return None;
            }
            warning = (s.info != 0).then(|| suspicious_svdc(&m, n, p, s.info));
            info = s.info;
            u = s.u.iter().map(|&x| T::narrow(x)).collect::<Vec<T>>();
            w = s.w.iter().map(|&x| T::narrow(x)).collect::<Vec<T>>();
            v = s.v.iter().map(|&x| T::narrow(x)).collect::<Vec<T>>();
        }
        let _ = info;
        // U_(i, j) and V_(i, j) from the column-major outputs
        let mut um = Mat::zeros(n, p);
        for j in 0..p {
            for i in 0..n {
                *um.at_mut(i, j) = u[j * n + i];
            }
        }
        let mut vm = Mat::zeros(p, p);
        for j in 0..p {
            for i in 0..p {
                *vm.at_mut(i, j) = v[j * p + i];
            }
        }
        // W_(j, j) = |w_j| for j < min(n + 1, p), 0 beyond; zero_out_absolute(0)
        let mm = (n + 1).min(p);
        let mut rank = p;
        let mut winv = vec![T::ZERO; p];
        for k in 0..p {
            let weight = if k < mm { w[k].fabs() } else { T::ZERO };
            if weight.fabs().wide() <= 0.0 {
                winv[k] = T::ZERO;
                rank -= 1;
            } else {
                winv[k] = T::ONE.sse_div(weight);
            }
        }
        Some(SvdT {
            u: um,
            v: vm,
            winv,
            rank,
            warning,
        })
    }

    /// `pinverse()`: `V * W^+ * U^T` (VNL's products; `W^+`'s first `rank` entries).
    fn pinverse(&self) -> Mat<T> {
        let p = self.v.rows;
        let mut wi = Mat::zeros(p, p);
        for i in 0..self.rank.min(p) {
            *wi.at_mut(i, i) = self.winv[i];
        }
        self.v.mul(&wi).mul(&self.u.transpose())
    }
}

/// `getSigmaMAD(v, d)`: `d` times the median absolute deviation of a copy of `v`
/// (`RobustGaussian<T>::mad`).
pub fn get_sigma_mad<T: Real>(g: &mut Globals, v: &[T], d: T) -> T {
    let mut t = v.to_vec();
    robust_gauss::mad(g, &mut t, d)
}

/// `getSqrtTukeyDiaWeights(r, w, sat)`: the square roots of Tukey's biweights of the residuals
/// `r`: 0 where `|r| >= sat`, else `1 - (r / sat)^2` (in `double`, stored as `T`). `w` must have
/// `r`'s length.
pub fn get_sqrt_tukey_dia_weights<T: Real>(r: &[T], w: &mut [T], sat: f64) {
    for (rr, &x) in r.iter().enumerate() {
        if x.fabs().wide() >= sat {
            w[rr] = T::ZERO;
        } else {
            let t1 = x.wide().sse_div(sat);
            w[rr] = T::narrow(1.0f64.sse_sub(t1.sse_mul(t1)));
        }
    }
}

