// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! `scipy.optimize.curve_fit(f, xdata, ydata, p0=p0, bounds=(lb, ub))` for a model of two
//! parameters, as SciPy 1.18.1 and 1.15.3 compute it (crate documentation; notices in
//! `lib.rs`): bounds make the method `'trf'`; `xdata` and `ydata` must be finite
//! (`asarray_chkfinite`, `ydata` cast to float64); the residuals are `f(xdata, *p) - ydata`;
//! [`least_squares_trf2`] with `jac='2-point'` and the default budget; a status of 0 raises
//! `RuntimeError`; the covariance is the Moore-Penrose inverse of `JᵀJ` from the SVD of the
//! final Jacobian (singular values above `eps · max(m, 2) · s₀`), scaled by `2·cost / (m - 2)`,
//! or infinite (with SciPy's `OptimizeWarning`) when it holds a NaN or `m <= 2`.

use crate::Scipy;
use crate::least_squares::{LeastSquaresError, least_squares_trf2};
use larmorx_core::math::Libm;
use larmorx_transform::numpy_linalg::{GesddError, LapackRelease, gesdd_s2};
use larmorx_transform::openblas;

/// The model `f(xdata, *p)`: `model(xdata, p, out)` writes its `xdata.len()` values at `p`.
pub type Model<'a> = dyn FnMut(&[f64], &[f64; 2], &mut [f64]) + 'a;

/// What `curve_fit(..., full_output=True)` returns.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveFit {
    /// The fitted parameters.
    pub popt: [f64; 2],
    /// Their covariance, row-major.
    pub pcov: [[f64; 2]; 2],
    /// SciPy warned `OptimizeWarning('Covariance of the parameters could not be estimated')`
    /// (the covariance is then infinite).
    pub warn_cov: bool,
    /// Residual evaluations counted by `least_squares` (`infodict['nfev']`).
    pub nfev: usize,
    /// `least_squares`' status (`ier`).
    pub status: i32,
}

/// Why `curve_fit` raised.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CurveFitError {
    /// `np.asarray_chkfinite` of `ydata` or `xdata`: `ValueError`.
    #[error("array must not contain infs or NaNs")]
    NotFinite,
    /// `ValueError`.
    #[error("`ydata` must not be empty!")]
    EmptyData,
    /// Raised inside `least_squares`.
    #[error(transparent)]
    LeastSquares(#[from] LeastSquaresError),
    /// `RuntimeError` (status 0).
    #[error(
        "Optimal parameters not found: The maximum number of function evaluations is exceeded."
    )]
    NotFound,
    /// `scipy.linalg.svd` of the final Jacobian.
    #[error(transparent)]
    CovarianceSvd(GesddError),
}

impl CurveFitError {
    /// The Python exception's class and message.
    pub fn python(&self) -> (&'static str, String) {
        match self {
            CurveFitError::LeastSquares(e) => e.python(),
            CurveFitError::NotFound => ("RuntimeError", self.to_string()),
            CurveFitError::CovarianceSvd(GesddError::DidNotConverge) => {
                ("LinAlgError", self.to_string())
            }
            _ => ("ValueError", self.to_string()),
        }
    }
}

/// `curve_fit(model, xdata, ydata, p0=p0, bounds=(lb, ub), full_output=True)` (module
/// documentation). `model(xdata, p, out)` writes the model's `ydata.len()` values at `p`;
/// `libm` gives C's `pow` inside the method (the model takes its own functions).
#[allow(clippy::too_many_arguments)]
pub fn curve_fit2(
    model: &mut Model<'_>,
    xdata: &[f64],
    ydata: &[f64],
    p0: [f64; 2],
    lb: [f64; 2],
    ub: [f64; 2],
    scipy: Scipy,
    libm: &Libm,
) -> Result<CurveFit, CurveFitError> {
    if !ydata.iter().all(|v| v.is_finite()) || !xdata.iter().all(|v| v.is_finite()) {
        return Err(CurveFitError::NotFinite);
    }
    if ydata.is_empty() {
        return Err(CurveFitError::EmptyData);
    }
    let m = ydata.len();
    let mut fun = |p: &[f64; 2], f: &mut [f64]| {
        model(xdata, p, f);
        for (fk, yk) in f.iter_mut().zip(ydata) {
            *fk -= yk;
        }
    };
    let res = least_squares_trf2(&mut fun, m, p0, lb, ub, scipy, libm)?;
    if res.status <= 0 {
        return Err(CurveFitError::NotFound);
    }
    let cost = 2.0 * res.cost;
    // Do Moore-Penrose inverse discarding zero singular values.
    let svd =
        gesdd_s2(&res.jac, m, LapackRelease::V3_12_0).map_err(CurveFitError::CovarianceSvd)?;
    let threshold = f64::EPSILON * m.max(2) as f64 * svd.s[0];
    // s = s[s > threshold]; VT = VT[:s.size] (s decreases, so the kept values come first).
    let kept: Vec<usize> = (0..2).filter(|&i| svd.s[i] > threshold).collect();
    // pcov = np.dot(VT.T / s**2, VT) for VT = VT[:k]: OpenBLAS's dgemm, a fused chain over k.
    let mut a = vec![0.0f64; 2 * kept.len()];
    let mut b = vec![0.0f64; kept.len() * 2];
    for (t, &r) in kept.iter().enumerate() {
        let s2 = svd.s[r] * svd.s[r];
        for i in 0..2 {
            a[i * kept.len() + t] = svd.vt[r + 2 * i] / s2;
            b[t * 2 + i] = svd.vt[r + 2 * i];
        }
    }
    let mut pcov = if kept.is_empty() {
        [[0.0; 2]; 2]
    } else {
        let p = openblas::gemm(&a, &b, 2, kept.len(), 2);
        [[p[0], p[1]], [p[2], p[3]]]
    };
    let mut warn_cov = false;
    if pcov.iter().flatten().any(|v| v.is_nan()) {
        pcov = [[f64::INFINITY; 2]; 2];
        warn_cov = true;
    } else if m > 2 {
        let s_sq = cost / (m - 2) as f64;
        for row in pcov.iter_mut() {
            for v in row.iter_mut() {
                *v = x86_mul(*v, s_sq);
            }
        }
    } else {
        pcov = [[f64::INFINITY; 2]; 2];
        warn_cov = true;
    }
    Ok(CurveFit {
        popt: res.x,
        pcov,
        warn_cov,
        nfev: res.nfev,
        status: res.status,
    })
}

/// `a · b` with x86-64's NaN for `0 · inf` (the default NaN, sign bit set), on every platform.
fn x86_mul(a: f64, b: f64) -> f64 {
    let p = a * b;
    if p.is_nan() && !a.is_nan() && !b.is_nan() {
        f64::from_bits(0xfff8_0000_0000_0000)
    } else {
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn fits_and_estimates_a_covariance() {
        let libm = Libm::quiet(UpstreamDefects::Reproduce);
        let x: Vec<f64> = [10.0, 20.0, 30.0, 40.0, 50.0]
            .iter()
            .flat_map(|&t| [t, t, t])
            .collect();
        let y: Vec<f64> = x
            .iter()
            .enumerate()
            .map(|(k, &t)| 800.0 * (-t / 35.0f64).exp() + ((k % 3) as f64 - 1.0))
            .collect();
        let mut model = |x: &[f64], p: &[f64; 2], out: &mut [f64]| {
            for (o, &t) in out.iter_mut().zip(x) {
                *o = p[0] * libm.exp(-t / p[1]);
            }
        };
        for scipy in [Scipy::V1_18_1, Scipy::V1_15_3] {
            let r = curve_fit2(
                &mut model,
                &x,
                &y,
                [700.0, 30.0],
                [100.0, 0.0],
                [f64::INFINITY; 2],
                scipy,
                &libm,
            )
            .unwrap();
            assert!((r.popt[0] - 800.0).abs() < 5.0 && (r.popt[1] - 35.0).abs() < 1.0);
            assert!(!r.warn_cov && r.pcov[0][0] > 0.0 && r.pcov[1][1] > 0.0);
            assert!((r.pcov[0][1] - r.pcov[1][0]).abs() <= 1e-12 * r.pcov[0][1].abs());
        }
        let bad = [f64::NAN; 15];
        assert_eq!(
            curve_fit2(
                &mut model,
                &x,
                &bad,
                [700.0, 30.0],
                [100.0, 0.0],
                [f64::INFINITY; 2],
                Scipy::V1_18_1,
                &libm
            ),
            Err(CurveFitError::NotFinite)
        );
    }
}
