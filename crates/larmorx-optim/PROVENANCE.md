# Provenance: larmorx-optim

**Ported** (an exact port of permissive upstreams, in the main package: `docs/licensing.md`,
`docs/porting-guide.md` §1) from SciPy **1.18.1** (the newest release; PyPI sdist in
`<workspace>/tags/scipy-1.18.1`) and **1.15.3** (the shared environment's; `tags/scipy-1.15.3`),
BSD-3-Clause, with the numpy 2.5.4 and 2.3.5 arithmetic they reach (BSD-3-Clause). The two SciPy
releases compute the same values on this path except for the memory order of the SVD factors
(`Scipy`); both are verified. The logic is re-expressed in Rust; no code was copied. The notices
are in `src/lib.rs` and NOTICE.

| larmorx | Upstream | Licence |
|---|---|---|
| `curve_fit::curve_fit2`, `CurveFit`, `CurveFitError` (bounds → `'trf'`, `asarray_chkfinite` of `ydata` and `xdata`, the residual wrapper, `max_nfev` → `None`, the `RuntimeError` of status 0, the covariance: `svd(res.jac)`, the threshold `eps · max(shape) · s[0]`, `np.dot(VT.T / s**2, VT)`, the NaN check, `cost / (ysize - p0.size)`, the `OptimizeWarning` cases) | SciPy `scipy/optimize/_minpack_py.py` (`curve_fit`, `_wrap_func`, `_lightweight_memoizer`, `_initialize_feasible`), `scipy/optimize/_lsq/least_squares.py` (`prepare_bounds`) | BSD-3-Clause |
| `least_squares::least_squares_trf2`, `LeastSquares`, `LeastSquaresError` (the argument checks and messages, `make_strictly_feasible` of `x0`, `VectorFunction`'s order of the first residuals and Jacobian, `trf_bounds`, `select_step`, `solve_lsq_trust_region`, `intersect_trust_region`, `build_quadratic_1d`, `minimize_quadratic_1d`, `evaluate_quadratic`, `step_size_to_bound`, `find_active_constraints`, `make_strictly_feasible`, `CL_scaling_vector`, `compute_grad`, `update_tr_radius`, `check_termination`, `TERMINATION_MESSAGES`) | SciPy `scipy/optimize/_lsq/least_squares.py`, `_lsq/trf.py`, `_lsq/common.py`, `scipy/optimize/_differentiable_functions.py` (`VectorFunction`, `_VectorFunWrapper`, `_VectorJacWrapper`) | BSD-3-Clause |
| `least_squares::approx_jacobian` (`'2-point'`: `_compute_absolute_step` with `_eps_for_method`'s `EPS ** 0.5 = 2^-26`, `_adjust_scheme_to_bounds('1-sided')`, `_dense_difference`) | SciPy `scipy/optimize/_numdiff.py` | BSD-3-Clause |
| numpy's rules the method relies on: `linalg.norm` (`sqrt(x.dot(x))`, `abs(x).max()`), `maximum`/`minimum`/`argmin` with NaNs, `np.sum` of two values (`larmorx_core::numpy::reduce`), `x ** 2` as `square`, an array's `x ** 0.5` as `sqrt` (`fast_scalar_power`), `x ** 3` and a scalar's `x ** y` as C's `pow` (`power`'s loop, `scalarmath`), `nextafter` | numpy `numpy/linalg/_linalg.py`, `numpy/_core/src/multiarray/number.c`, `numpy/_core/src/umath/loops_umath_fp.dispatch.c.src`, `numpy/_core/src/umath/scalarmath.c.src`, `numpy/_core/src/umath/loops_minmax.dispatch.c.src` | BSD-3-Clause |

The products with numpy (`numpy.dot`) and the SVD (`scipy.linalg.svd`) are
`larmorx_transform::numpy_linalg::{blas, gesdd}` (OpenBLAS's Haswell kernels and LAPACK 3.12.0,
see that crate's PROVENANCE.md); C's `pow` is `larmorx_core::math::Libm` (glibc 2.35's with
`reproduce`, correctly rounded otherwise).

**Verified** (2026-11-08) bit for bit against SciPy 1.18.1 with numpy 2.5.4 and SciPy 1.15.3
with numpy 2.3.5, numpy's AVX-512 kernels off and OpenBLAS's Haswell kernels
(`validation/investigations/t2smap/numerics/check_curve_fit.py`, `run_curve_fit_check.sh`):
synthetic and edge voxels (every exit SciPy reached: statuses 1 to 4, the evaluation budget, the
covariance warning, a start outside the bounds, values that are not finite) and tedana's own
calls on fMRIPrep's four multi-echo `t2smap` calls. `intersect_trust_region`'s two errors are
ported but no voxel of the checks reached them (none of 30,000 random fits did,
`search_rare_exits.py`).
