// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! SciPy's optimizers as larmorx's ports call them, bit for bit with SciPy's Python on x86-64
//! (the numpy and OpenBLAS arithmetic it reaches included), on every platform.
//!
//! - [`least_squares`]: `scipy.optimize.least_squares(method='trf')` with bounds, the `exact`
//!   trust-region solver, `x_scale = 1`, the linear loss and `jac='2-point'`, for two
//!   parameters: the trust-region reflective method (`_lsq/trf.py`'s `trf_bounds`, its helpers
//!   in `_lsq/common.py`) and the forward differences of `_numdiff.approx_derivative` with their
//!   bounds-adjusted steps;
//! - [`curve_fit`]: `scipy.optimize.curve_fit` with bounds (so `method='trf'`), no `sigma`, the
//!   default tolerances and evaluation budget, and its covariance from the SVD of the final
//!   Jacobian.
//!
//! tedana's monoexponential T2*/S0 fit (`t2smap --fittype curvefit`, fMRIPrep's default) calls
//! `curve_fit` this way for every voxel; the model function is the caller's.
//!
//! **Versions.** The algorithm is the same in SciPy 1.18.1 and 1.15.3 on this path (1.18.1
//! wraps the residuals in `VectorFunction`, which caches values but computes the same ones);
//! what differs is the memory order of the SVD factors SciPy returns (1.18.1's batched
//! `_svd` gives C-ordered `U` and `VT`, 1.15.3's f2py wrapper Fortran-ordered ones), which
//! decides whether numpy's products with them go through OpenBLAS's `dgemv_t` or `dgemv_n`
//! ([`Scipy`]). Both SciPy wheels bundle LAPACK 3.12.0's routines; numpy 2.5.4 and 2.3.5 the
//! same OpenBLAS kernels on these lengths (`larmorx_transform::numpy_linalg::blas`).
//!
//! **Maths.** The method's own powers (`x ** 0.5`, `Delta ** 2` on numpy scalars, `denom ** 3`
//! on arrays) are C's `pow`, through a [`larmorx_core::math::Libm`]: glibc 2.35's with
//! `reproduce`, correctly rounded otherwise (GLIBC-07's convention); square roots of arrays
//! and squares are numpy's `sqrt` and `square` (exact roundings).
//!
//! Upstream files (BSD-3-Clause; notice below and in NOTICE): SciPy 1.18.1 and 1.15.3
//! `scipy/optimize/_minpack_py.py` (`curve_fit`, `_wrap_func`, `_lightweight_memoizer`,
//! `_initialize_feasible`), `scipy/optimize/_lsq/least_squares.py`, `_lsq/trf.py`
//! (`trf_bounds`, `select_step`), `_lsq/common.py`, `scipy/optimize/_numdiff.py`
//! (`approx_derivative`, `_compute_absolute_step`, `_eps_for_method`,
//! `_adjust_scheme_to_bounds`, `_dense_difference`), `scipy/optimize/_differentiable_functions.py`
//! (`VectorFunction`), `scipy/linalg/_decomp_svd.py`; numpy 2.5.4 and 2.3.5
//! `numpy/linalg/_linalg.py` (`norm`), `numpy/_core/src/multiarray/number.c`
//! (`fast_scalar_power`), `numpy/_core/src/umath/loops_umath_fp.dispatch.c.src` (`power`),
//! `numpy/_core/src/umath/scalarmath.c.src` (scalar `power`). The logic is re-expressed in
//! Rust; no code was copied.
//!
//! SciPy: Copyright (c) 2001-2002 Enthought, Inc. 2003, SciPy Developers. All rights reserved.
//! numpy: Copyright (c) 2005-2025, NumPy Developers. All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without modification, are
//! permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice, this list of
//!    conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright notice, this list of
//!    conditions and the following disclaimer in the documentation and/or other materials
//!    provided with the distribution.
//! 3. Neither the name of the copyright holder nor the names of its contributors may be used to
//!    endorse or promote products derived from this software without specific prior written
//!    permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS
//! OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
//! MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
//! COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
//! EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE
//! GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
//! ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
//! NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
//! OF THE POSSIBILITY OF SUCH DAMAGE.
#![forbid(unsafe_code)]

pub mod curve_fit;
pub mod least_squares;

pub use curve_fit::{CurveFit, CurveFitError, Model, curve_fit2};
pub use least_squares::{LeastSquares, LeastSquaresError, least_squares_trf2};

/// The SciPy (with the numpy beside it) whose arithmetic a call follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scipy {
    /// SciPy 1.18.1 with numpy 2.5.4 (the newest; `oracles/python-newest`): `svd` returns
    /// C-ordered factors. The default.
    #[default]
    V1_18_1,
    /// SciPy 1.15.3 with numpy 2.3.5 (the shared environment): `svd` returns Fortran-ordered
    /// factors.
    V1_15_3,
}
