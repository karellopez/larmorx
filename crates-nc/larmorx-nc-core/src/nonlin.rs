// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-miscmaths 2412.4: nonlin.h, nonlin.cpp). FSL is
// Copyright (C) 2007 University of Oxford. Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! fsl-miscmaths' non-linear optimisers, as topup uses them: the parameters object
//! (`NonlinParam`: tolerances, the iteration count, the cost-function, `λ` and parameter
//! histories), the cost-function interface (`NonlinCF`: `cf`, and `grad` and `hess` with their
//! finite-difference defaults), and the two methods topup calls, Levenberg-Marquardt
//! (`levmar`, the Hessian's diagonal nudged and the step solved by [`BFMatrix::solve_for_x`]) and
//! Møller's scaled conjugate gradients (`sccngr`).
//!
//! Not translated (topup does not use them): the variable-metric, conjugate-gradient, gradient
//! descent and Nelder-Mead methods (and their line searches), and the verbose printing of
//! `SetPar`/`SetCF` (topup never turns it on). NEWMAT's vector algebra is Armadillo's element
//! by element; `DotProduct` is [`crate::arma::dot_blas`] on `blas_threads` OpenBLAS threads (FSL's
//! programs: 1).

use crate::arma::dot_blas;
use crate::bfmatrix::BFMatrix;
use crate::spmat::MatrixType;
use crate::volume::FslError;

/// `MISCMATHS::EPS`.
pub const EPS: f64 = 2.0e-16;

/// `NonlinException(msg)`: `what()` is `"Nonlin: msg=" + msg`.
pub fn nonlin_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "MISCMATHS::NonlinException",
        what: format!("Nonlin: msg={msg}"),
    }
}

/// `NLMethod`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NlMethod {
    Vm,
    Cg,
    Scg,
    Lm,
    Gd,
    Nm,
}

/// `LMType`: Gauss-Newton, Levenberg or Levenberg-Marquardt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LmType {
    Gn,
    L,
    Lm,
}

/// `NonlinOut`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonlinOut {
    Undefined,
    MaxIter,
    LmMaxIter,
    InvFail,
    ParConv,
    GradConv,
    CfConv,
    CfIncConv,
    LConv,
}

/// `NonlinParam`: the settings and the state of one minimisation.
#[derive(Clone, Debug)]
pub struct NonlinParam {
    npar: usize,
    mtd: NlMethod,
    logcf: bool,
    loglambda: bool,
    logpar: bool,
    maxiter: i32,
    cftol: f64,
    gtol: f64,
    ptol: f64,
    lmtype: LmType,
    ltol: f64,
    cg_maxiter: i32,
    cg_tol: f64,
    lambda: Vec<f64>,
    cf: Vec<f64>,
    par: Vec<Vec<f64>>,
    niter: i32,
    status: NonlinOut,
    /// OpenBLAS's thread count for `DotProduct` (not FSL's: its programs run OpenBLAS on one
    /// thread); default 1.
    pub blas_threads: usize,
}

impl NonlinParam {
    /// `NonlinParam(npar, method, par)` with FSL's defaults (no histories, 200 iterations,
    /// tolerances 1e-8 for the cost-function and the gradient and `4·EPS` for the parameters,
    /// Levenberg-Marquardt, `λ` convergence at 1e20, the equation solver 200 iterations to
    /// 1e-6, `λ` 0.1); `par` empty starts from zeros.
    pub fn new(npar: usize, mtd: NlMethod, par: &[f64]) -> Result<NonlinParam, FslError> {
        let mut p = NonlinParam {
            npar,
            mtd,
            logcf: false,
            loglambda: false,
            logpar: false,
            maxiter: 200,
            cftol: 1.0e-8,
            gtol: 1.0e-8,
            ptol: 4.0 * EPS,
            lmtype: LmType::Lm,
            ltol: 1.0e20,
            cg_maxiter: 200,
            cg_tol: 1.0e-6,
            lambda: vec![0.1],
            cf: Vec::new(),
            par: Vec::new(),
            niter: 0,
            status: NonlinOut::Undefined,
            blas_threads: 1,
        };
        if !par.is_empty() {
            p.set_starting_estimate(par)?;
        } else {
            let z = vec![0.0; npar];
            p.set_starting_estimate(&z)?;
        }
        Ok(p)
    }

    pub fn npar(&self) -> usize {
        self.npar
    }
    pub fn method(&self) -> NlMethod {
        self.mtd
    }
    pub fn max_iter(&self) -> i32 {
        self.maxiter
    }
    pub fn n_iter(&self) -> i32 {
        self.niter
    }
    pub fn fractional_cf_tolerance(&self) -> f64 {
        self.cftol
    }
    pub fn fractional_gradient_tolerance(&self) -> f64 {
        self.gtol
    }
    pub fn fractional_parameter_tolerance(&self) -> f64 {
        self.ptol
    }
    pub fn gauss_newton_type(&self) -> LmType {
        self.lmtype
    }
    pub fn lambda_convergence_criterion(&self) -> f64 {
        self.ltol
    }
    pub fn equation_solver_max_iter(&self) -> i32 {
        self.cg_maxiter
    }
    pub fn equation_solver_tol(&self) -> f64 {
        self.cg_tol
    }
    /// `Lambda()`: the latest `λ`.
    pub fn lambda(&self) -> f64 {
        *self.lambda.last().unwrap()
    }
    /// `Par()`: the latest parameters.
    pub fn par(&self) -> &[f64] {
        self.par.last().unwrap()
    }
    /// `CF()`: the latest cost-function value.
    pub fn cf(&self) -> f64 {
        *self.cf.last().unwrap()
    }
    /// `CFHistory()` (with `LogCF`).
    pub fn cf_history(&self) -> Result<&[f64], FslError> {
        if self.logcf {
            Ok(&self.cf)
        } else {
            Err(nonlin_error("CFHistory: Cost-function not logged"))
        }
    }
    /// `LambdaHistory()` (with `LogLambda`).
    pub fn lambda_history(&self) -> Result<&[f64], FslError> {
        if self.loglambda {
            Ok(&self.lambda)
        } else {
            Err(nonlin_error("InitialLabda: Lambda not logged"))
        }
    }
    /// `ParHistory()` (with `LogPar`).
    pub fn par_history(&self) -> Result<&[Vec<f64>], FslError> {
        if self.logpar {
            Ok(&self.par)
        } else {
            Err(nonlin_error("ParHistory: Parameters not logged"))
        }
    }
    pub fn status(&self) -> NonlinOut {
        self.status
    }
    /// `Success()`.
    pub fn success(&self) -> bool {
        !matches!(
            self.status,
            NonlinOut::Undefined | NonlinOut::MaxIter | NonlinOut::LmMaxIter
        )
    }
    /// `TextStatus()`.
    pub fn text_status(&self) -> &'static str {
        match self.status {
            NonlinOut::Undefined => {
                "Status is undefined. Object has been created but no minimisation has been performed"
            }
            NonlinOut::MaxIter => {
                "The optimisation did not converge because the maximum number of iterations was exceeded"
            }
            NonlinOut::LmMaxIter => {
                "The optimisation did not converge because the maximum number of iterations for a single line minimisation was exceeded"
            }
            NonlinOut::ParConv => {
                "The optimisation converged. The convergence criterion was that the last step in parameter space was very short"
            }
            NonlinOut::GradConv => {
                "The optimisation converged. The convergence criterion was that all the elements of the gradient were very small"
            }
            NonlinOut::CfConv => {
                "The optimisation converged. The convergence criterion was that the last step changed the cost-function by an insignificant amount"
            }
            NonlinOut::LConv => {
                "The optimisation converged. The convergence criterion was that lambda became too large"
            }
            NonlinOut::InvFail | NonlinOut::CfIncConv => {
                "Impossible status. This indicates there is a bug"
            }
        }
    }

    pub fn set_method(&mut self, m: NlMethod) {
        self.mtd = m;
    }
    pub fn log_cf(&mut self, flag: bool) {
        self.logcf = flag;
    }
    pub fn log_par(&mut self, flag: bool) {
        self.logpar = flag;
    }
    pub fn log_lambda(&mut self, flag: bool) {
        self.loglambda = flag;
    }
    /// `SetStartingEstimate(sp)`: refused once iterations were counted.
    pub fn set_starting_estimate(&mut self, sp: &[f64]) -> Result<(), FslError> {
        if self.niter != 0 {
            return Err(nonlin_error(
                "SetStartingEstimates: Object has to be reset before setting new starting parameters",
            ));
        }
        self.set_par(sp)
    }
    pub fn set_max_iter(&mut self, m: u32) {
        self.maxiter = m as i32;
    }
    /// `SetFractionalCFTolerance(t)`: above 0.5 refused; a tolerance of 0 or less is accepted
    /// (FSL builds the exception for it without throwing it).
    pub fn set_fractional_cf_tolerance(&mut self, t: f64) -> Result<(), FslError> {
        if t > 0.5 {
            return Err(nonlin_error(
                "SetFractionalCFTolerance: Nonsensically large tolerance",
            ));
        }
        self.cftol = t;
        Ok(())
    }
    /// `SetFractionalGradientTolerance(t)` (as the cost-function's).
    pub fn set_fractional_gradient_tolerance(&mut self, t: f64) -> Result<(), FslError> {
        if t > 0.5 {
            return Err(nonlin_error(
                "SetFractionalGradientTolerance: Nonsensically large tolerance",
            ));
        }
        self.gtol = t;
        Ok(())
    }
    /// `SetFractionalParameterTolerance(t)` (as the cost-function's).
    pub fn set_fractional_parameter_tolerance(&mut self, t: f64) -> Result<(), FslError> {
        if t > 0.5 {
            return Err(nonlin_error(
                "SetFractionalParameterTolerance: Nonsensically large tolerance",
            ));
        }
        self.ptol = t;
        Ok(())
    }
    pub fn set_gauss_newton_type(&mut self, t: LmType) {
        self.lmtype = t;
    }
    /// `SetLambdaConvergenceCriterion(t)`: below 1 refused.
    pub fn set_lambda_convergence_criterion(&mut self, t: f64) -> Result<(), FslError> {
        if t < 1.0 {
            return Err(nonlin_error(
                "SetLambdaConvergenceCriterion: Nonsensically small tolerance",
            ));
        }
        self.ltol = t;
        Ok(())
    }
    pub fn set_equation_solver_max_iter(&mut self, m: i32) {
        self.cg_maxiter = m;
    }
    pub fn set_equation_solver_tol(&mut self, t: f64) {
        self.cg_tol = t;
    }

    /// `SetPar(p)`: appended with `LogPar` (or as the first), else replaces the only one.
    pub fn set_par(&mut self, p: &[f64]) -> Result<(), FslError> {
        if p.len() != self.npar {
            return Err(nonlin_error(
                "SetPar: Mismatch between starting vector and # of parameters",
            ));
        }
        if self.logpar || self.par.is_empty() {
            self.par.push(p.to_vec());
        } else {
            self.par[0] = p.to_vec();
        }
        Ok(())
    }
    /// `SetCF(cf)`.
    pub fn set_cf(&mut self, cf: f64) {
        if self.logcf || self.cf.is_empty() {
            self.cf.push(cf);
        } else {
            self.cf[0] = cf;
        }
    }
    /// `SetLambda(l)`.
    pub fn set_lambda(&mut self, l: f64) {
        if self.loglambda || self.lambda.is_empty() {
            self.lambda.push(l);
        } else {
            self.lambda[0] = l;
        }
    }
    /// `NextIter(success)`: counts an iteration only after a successful one; false once the
    /// count reached `MaxIter`.
    pub fn next_iter(&mut self, success: bool) -> bool {
        if success {
            let n = self.niter;
            self.niter += 1;
            if n >= self.maxiter {
                return false;
            }
        }
        true
    }
    pub fn set_status(&mut self, s: NonlinOut) {
        self.status = s;
    }
}

/// `NonlinCF`: a cost-function to minimise. `grad` and `hess` default to FSL's finite
/// differences (steps `1e-8·max(p, 1)` and `1e-4·max(p, 1)`); topup overrides both.
pub trait NonlinCf {
    /// `sf()`: a scale for the cost-function (1).
    fn sf(&self) -> f64 {
        1.0
    }
    fn cf(&mut self, p: &[f64]) -> Result<f64, FslError>;
    fn grad(&mut self, p: &[f64]) -> Result<Vec<f64>, FslError> {
        numerical_grad(self, p)
    }
    /// `hess(p, iptr)`: `iptr` is the previous Hessian, which an implementation may reuse.
    fn hess(&mut self, p: &[f64], iptr: Option<BFMatrix>) -> Result<BFMatrix, FslError> {
        numerical_hess(self, p, iptr)
    }
}

/// `NonlinCF::grad`: forward differences.
pub fn numerical_grad<C: NonlinCf + ?Sized>(cfo: &mut C, p: &[f64]) -> Result<Vec<f64>, FslError> {
    let mut gradv = vec![0.0; p.len()];
    let mut tmpp = p.to_vec();
    let tiny = 1e-8;
    let cf0 = cfo.cf(&tmpp)?;
    for i in 0..p.len() {
        let step = tiny * cmax(tmpp[i], 1.0);
        tmpp[i] += step;
        gradv[i] = (cfo.cf(&tmpp)? - cf0) / step;
        tmpp[i] -= step;
    }
    Ok(gradv)
}

/// `NonlinCF::hess`: second differences into `iptr` when it has the right size, else a new
/// dense matrix.
pub fn numerical_hess<C: NonlinCf + ?Sized>(
    cfo: &mut C,
    p: &[f64],
    iptr: Option<BFMatrix>,
) -> Result<BFMatrix, FslError> {
    let n = p.len();
    let mut hessm = match iptr {
        Some(h) if h.nrows() as usize == n && h.ncols() as usize == n => h,
        _ => BFMatrix::Full(crate::arma::Mat::zeros(n, n)),
    };
    let mut tmpp = p.to_vec();
    let tiny = 1e-4;
    let fx0y0 = cfo.cf(&tmpp)?;
    let mut fdx = vec![0.0; n];
    let mut step = vec![0.0; n];
    for i in 0..n {
        step[i] = tiny * cmax(tmpp[i], 1.0);
        tmpp[i] += step[i];
        fdx[i] = cfo.cf(&tmpp)?;
        tmpp[i] -= step[i];
    }
    for i in 0..n {
        for j in i..n {
            let (ri, rj) = (i as u32 + 1, j as u32 + 1);
            if i == j {
                tmpp[i] -= step[i];
                let tmp = cfo.cf(&tmpp)?;
                hessm.set(ri, ri, (fdx[i] + tmp - 2.0 * fx0y0) / (step[i] * step[i]))?;
                tmpp[i] += step[i];
            } else {
                tmpp[i] += step[i];
                tmpp[j] += step[j];
                let v = (cfo.cf(&tmpp)? + fx0y0 - fdx[i] - fdx[j]) / (step[i] * step[j]);
                hessm.set(ri, rj, v)?;
                let back = hessm.peek(ri, rj)?;
                hessm.set(rj, ri, back)?;
                tmpp[i] -= step[i];
                tmpp[j] -= step[j];
            }
        }
    }
    Ok(hessm)
}

/// `zero_grad_conv`: the largest `|g_i|·max(|p_i|, 1)`, over `max(cf, 1)`, below `gtol`.
pub fn zero_grad_conv(par: &[f64], grad: &[f64], cf: f64, gtol: f64) -> bool {
    let mut test = 0.0f64;
    for i in 0..par.len() {
        test = cmax(test, grad[i].abs() * cmax(par[i].abs(), 1.0));
    }
    test /= cmax(cf, 1.0);
    test < gtol
}

/// `zero_cf_diff_conv`: `2|cfo − cfn| ≤ tol·(|cfo| + |cfn| + EPS)`.
pub fn zero_cf_diff_conv(cfo: f64, cfn: f64, cftol: f64) -> bool {
    2.0 * (cfo - cfn).abs() <= cftol * (cfo.abs() + cfn.abs() + EPS)
}

/// `zero_par_step_conv`: the largest `|step_i|/max(|p_i|, 1)` below `ptol`.
pub fn zero_par_step_conv(par: &[f64], step: &[f64], ptol: f64) -> bool {
    let mut test = 0.0f64;
    for i in 0..par.len() {
        test = cmax(test, step[i].abs() / cmax(par[i].abs(), 1.0));
    }
    test < ptol
}

/// `std::max(a, b)`: `a < b ? b : a` (the first when they compare equal or one is NaN).
#[inline(always)]
fn cmax(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

fn add_scaled(a: &[f64], k: f64, b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(&x, &y)| x + y * k).collect()
}

/// `nonlin(p, cfo)`: runs the method `p` names. Only Levenberg-Marquardt and scaled conjugate
/// gradients are translated; the others are refused.
pub fn nonlin<C: NonlinCf + ?Sized>(
    p: &mut NonlinParam,
    cfo: &mut C,
) -> Result<NonlinOut, FslError> {
    match p.method() {
        NlMethod::Lm => levmar(p, cfo),
        NlMethod::Scg => sccngr(p, cfo),
        m => Err(nonlin_error(&format!(
            "nonlin: method {m:?} is not translated (larmorx-nc-core translates LM and SCG)"
        ))),
    }
}

/// `levmar`: Levenberg-Marquardt. After a step that lowered the cost the gradient and Hessian
/// are re-evaluated; the Hessian's diagonal is nudged (`H_ii·(1 + λ)/(1 + λ_old)` for LM,
/// `+ (λ − λ_old)` for Levenberg), the step solved as `−H⁻¹g` (`SYM_POSDEF`, the equation
/// solver's tolerance and iterations); a failure there or in the cost of the new point counts as
/// an unsuccessful step. Success divides `λ` by 10, failure multiplies it by 10 (convergence when
/// above `ltol`).
pub fn levmar<C: NonlinCf + ?Sized>(
    p: &mut NonlinParam,
    cfo: &mut C,
) -> Result<NonlinOut, FslError> {
    let cf0 = cfo.cf(p.par())?;
    p.set_cf(cf0);
    let mut success = true;
    let mut olambda = 0.0f64;
    let mut g: Vec<f64> = Vec::new();
    let mut h: Option<BFMatrix> = None;
    while p.next_iter(success) {
        if success {
            let par = p.par().to_vec();
            g = cfo.grad(&par)?;
            h = Some(cfo.hess(&par, h.take())?);
        }
        let hm = h.as_mut().expect("the Hessian of the first iteration");
        for i in 1..=p.npar() as u32 {
            match p.gauss_newton_type() {
                LmType::Lm => {
                    let v = ((1.0 + p.lambda()) / (1.0 + olambda)) * hm.peek(i, i)?;
                    hm.set(i, i, v)?;
                }
                LmType::L => hm.add_to(i, i, p.lambda() - olambda)?,
                LmType::Gn => {}
            }
        }
        let mut ncf = 0.0;
        let mut step: Vec<f64> = Vec::new();
        let attempt = (|| -> Result<(Vec<f64>, f64), FslError> {
            let (x, _) = hm.solve_for_x(
                &g,
                MatrixType::SymPosdef,
                p.equation_solver_tol(),
                p.equation_solver_max_iter(),
                p.blas_threads,
            )?;
            let st: Vec<f64> = x.iter().map(|v| -v).collect();
            let np: Vec<f64> = p.par().iter().zip(&st).map(|(a, b)| a + b).collect();
            let c = cfo.cf(&np)?;
            Ok((st, c))
        })();
        let inv_fail = match attempt {
            Ok((st, c)) => {
                step = st;
                ncf = c;
                false
            }
            Err(_) => true,
        };
        success = !inv_fail && ncf < p.cf();
        if success {
            olambda = 0.0;
            let np: Vec<f64> = p.par().iter().zip(&step).map(|(a, b)| a + b).collect();
            p.set_par(&np)?;
            p.set_lambda(p.lambda() / 10.0);
            if zero_cf_diff_conv(p.cf(), ncf, p.fractional_cf_tolerance()) {
                p.set_cf(ncf);
                p.set_status(NonlinOut::CfConv);
                return Ok(p.status());
            }
            p.set_cf(ncf);
        } else {
            if p.gauss_newton_type() == LmType::Gn {
                if inv_fail {
                    p.set_status(NonlinOut::InvFail);
                } else {
                    // Not a failure, so the cost did not decrease (`!(ncf < CF)`).
                    p.set_status(NonlinOut::CfIncConv);
                }
                return Ok(p.status());
            }
            success = false;
            olambda = p.lambda();
            p.set_lambda(10.0 * p.lambda());
            let c = p.cf();
            p.set_cf(c);
            if p.lambda() > p.lambda_convergence_criterion() {
                p.set_status(NonlinOut::LConv);
                return Ok(p.status());
            }
        }
    }
    p.set_status(NonlinOut::MaxIter);
    Ok(p.status())
}

/// `sccngr`: Møller's scaled conjugate gradients (σ 1e-2: the Hessian times the direction from
/// the gradient at `w + σ/|p|·p`; `λ` raised when that is not positive definite, halved after a
/// very good step (Δ > 0.75), quadrupled after a poor one (Δ < 0.25); the direction reset to the
/// steepest descent every `npar` iterations; convergence by the gradient test).
pub fn sccngr<C: NonlinCf + ?Sized>(
    np: &mut NonlinParam,
    cfo: &mut C,
) -> Result<NonlinOut, FslError> {
    let bt = np.blas_threads;
    let c0 = cfo.cf(np.par())?;
    np.set_cf(c0);
    let sigma = 1.0e-2;
    let mut lambda_bar = 0.0f64;
    let par0 = np.par().to_vec();
    let mut r: Vec<f64> = cfo.grad(&par0)?.iter().map(|v| -v).collect();
    let mut p = r.clone();
    let mut success = true;
    let mut delta = 0.0f64;
    let mut s = vec![0.0; np.npar()];
    while np.next_iter(true) {
        let p2 = dot_blas(&p, &p, bt);
        if success {
            let sigma_k = sigma / p2.sqrt();
            let wp = add_scaled(np.par(), sigma_k, &p);
            let gw = cfo.grad(&wp)?;
            s = gw
                .iter()
                .zip(&r)
                .map(|(&a, &b)| (a + b) / sigma_k)
                .collect();
            delta = dot_blas(&p, &s, bt);
        }
        let k = np.lambda() - lambda_bar;
        for (si, &pi) in s.iter_mut().zip(&p) {
            *si += pi * k;
        }
        delta += k * p2;
        if delta <= 0.0 {
            let k2 = np.lambda() - 2.0 * (delta / p2);
            for (si, &pi) in s.iter_mut().zip(&p) {
                *si += pi * k2;
            }
            lambda_bar = 2.0 * (np.lambda() - delta / p2);
            delta = np.lambda() * p2 - delta;
            np.set_lambda(lambda_bar);
        }
        let mu = dot_blas(&p, &r, bt);
        let alpha = mu / delta;
        let trial = add_scaled(np.par(), alpha, &p);
        let tmp_cf = cfo.cf(&trial)?;
        let big_delta = 2.0 * delta * (np.cf() - tmp_cf) / (mu * mu);
        if big_delta >= 0.0 {
            np.set_cf(tmp_cf);
            let newp = add_scaled(np.par(), alpha, &p);
            np.set_par(&newp)?;
            lambda_bar = 0.0;
            success = true;
            if np.n_iter() % np.npar() as i32 == 0 {
                let par = np.par().to_vec();
                r = cfo.grad(&par)?.iter().map(|v| -v).collect();
                p = r.clone();
            } else {
                let oldr = r.clone();
                let par = np.par().to_vec();
                r = cfo.grad(&par)?.iter().map(|v| -v).collect();
                let beta = (dot_blas(&r, &r, bt) - dot_blas(&oldr, &r, bt)) / mu;
                p = add_scaled(&r, beta, &p);
            }
            if big_delta > 0.75 {
                np.set_lambda(np.lambda() / 2.0);
            }
        } else {
            lambda_bar = np.lambda();
            success = false;
        }
        if big_delta < 0.25 {
            np.set_lambda(4.0 * np.lambda());
        }
        if zero_grad_conv(np.par(), &r, np.cf(), np.fractional_gradient_tolerance()) {
            np.set_status(NonlinOut::GradConv);
            return Ok(np.status());
        }
    }
    np.set_status(NonlinOut::MaxIter);
    Ok(np.status())
}
