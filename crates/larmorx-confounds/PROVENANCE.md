# Provenance: larmorx-confounds

**Ported** (an exact port of permissive upstreams, in the main package: `docs/licensing.md`,
`docs/porting-guide.md` §1) for fMRIPrep's confound estimation: nipype 1.12.0, niworkflows
1.15.4 and fMRIPrep 25.2.7 (Apache-2.0), with the arithmetic of the libraries they call, followed
operation by operation because it decides bit-identity. The logic is re-expressed in Rust; no
code was copied. The notices are in the modules that follow each upstream. The crate is filled by
the confounds' part relay by relay; this file lists what is in it.

## Relay 2e-i: SciPy's single-precision FFTs (`scipy_fft`)

nitime 0.12.1's `fftconvolve` (under nipype's DVARS AR(1) coefficients) calls
`scipy.fftpack.fft` and `ifft` of float32 and complex64 arrays of power-of-two length. SciPy
1.18.1 (the newest; PyPI sdist `scipy-1.18.1.tar.gz`, SHA-256
`52c4b7422442aba924d03ad4019852b08a92e64ea187b933135687bfe2747307`, `<workspace>/tags/scipy-1.18.1`)
computes them with ducc0 (`scipy.fft._duccfft`); SciPy 1.15.3 (the shared environment's; sdist
SHA-256 `eae3cf522bc7df64b42cad3925c876e1b0b6c35c1337c93e12c0f366f55b0eaf`,
`<workspace>/tags/scipy-1.15.3`) and 1.15.2 (fMRIPrep 25.2.6's image) with pocketfft
(`scipy.fft._pocketfft`). pocketfft and ducc0: Copyright (C) 2010-2023 Max-Planck-Society,
Copyright (C) 2019 Peter Bell (authors Martin Reinecke, Peter Bell), BSD-3-Clause (ducc0's files
are "BSD-3-Clause OR GPL-2.0-or-later"; the BSD-3-Clause terms are the ones used). SciPy:
Copyright (c) 2001-2002 Enthought, Inc. 2003, SciPy Developers, BSD-3-Clause.

| larmorx | Upstream | Licence |
|---|---|---|
| `scipy_fft::roots::UnityRoots`, `calc`, `twiddle_survey` | SciPy 1.15.3 `scipy/_lib/pocketfft/pocketfft_hdronly.h` (`sincos_2pibyn`); SciPy 1.18.1 `subprojects/duccfft/ducc0/math/unity_roots.h` (`UnityRoots`, the same construction) | BSD-3-Clause |
| `scipy_fft::cpass::{CPass, factorize, pocketfft_plan}`, `pass2`, `pass4`, `pass8` | SciPy 1.15.3 `pocketfft_hdronly.h` (`cfftp`: `pass2`, `pass4`, `pass8`, `ROTX45`, `ROTX135`, `pass_all`, `factorize`, `comp_twiddle`; `pocketfft_c`'s choice of `cfftp` for powers of two; `cmplx`, `special_mul`, `PM`, `PMINPLACE`, `ROTX90`) | BSD-3-Clause |
| `scipy_fft::cpass::{ducc0_pass, ducc0_plan}`, `CPass::Multi`, `CPass::Vec4` | SciPy 1.18.1 `ducc0/fft/fft1d_impl.h` (`cfftp1`, `cfftp2`, `cfftp4`, `cfftp8`, `cfft_multipass` (its packets above 10000, the column twiddles, the transposition), `cfftp_vecpass` (four lanes), `cfftpass::make_pass`), `ducc0/fft/fft.h` (`cfftpass::factorize`, `util1d::prime_factors`, `pocketfft_c`, `PM`, `PMINPLACE`, `special_mul`), `ducc0/math/cmplx.h` (`Cmplx`) | BSD-3-Clause |
| `scipy_fft::rpass::{RPass, pocketfft_plan, ducc0_plan}`, `radf2`, `radf4`, `RPass::Complexify` | SciPy 1.15.3 `pocketfft_hdronly.h` (`rfftp`: `radf2`, `radf4`, `MULPM`, `exec` with `r2hc`, `copy_and_norm`, `factorize`, `comp_twiddle`; `pocketfft_r`); SciPy 1.18.1 `ducc0/fft/fft1d_impl.h` (`rfftp1`, `rfftp2`, `rfftp4`, `rfft_multipass`, `rfftp_complexify` forward, `rfftpass::make_pass`), `ducc0/fft/fft.h` (`rfftpass::factorize`, `pocketfft_r`) | BSD-3-Clause |
| `scipy_fft::{ComplexFft, RealFft, fftpack_real, fftpack_complex, FftLibrary, FftError}` | SciPy `scipy/fftpack/_basic.py` (`fft`, `ifft`), `scipy/fft/_pocketfft/` and `scipy/fft/_duccfft/` `basic.py` (`c2c`), `helper.py` (`_asfarray`, `_fix_shape`, `_fix_shape_1d`, `_normalization`), `pypocketfft.cxx` / `pyduccfft.cxx` (`c2c`, `c2c_internal`, `c2c_sym_internal`, `norm_fct`); pocketfft's `general_r2c`, `ExecC2C`, `rev_iter`; ducc0's `fftnd_impl.h` (`general_r2c`, `general_nd`'s one-dimensional path, `c2c`'s Bailey branch: the refused lengths), `fft.h` (`hermiteHelper`) | BSD-3-Clause |

The twiddles' sine and cosine: both libraries call glibc 2.35's `sincos` (GCC merges each
`cos`/`sin` pair; the extensions import `sincos` only). The tables use `larmorx_core::math`'s
correctly rounded `sin` and `cos` instead, which round to the same `float` entries on every
power of two up to 2^31 (`examples/twiddle_survey.rs`, which builds them with
`larmorx_core::math::sincos_glibc` too). The arithmetic is SSE's single precision
(`larmorx_core::numpy::NpFloat`); the extensions are built for the x86-64 baseline (no fused
multiply-add, checked with `objdump`).

## Relay 2e-ii: nitime's AR(1) and nipype's DVARS (`nitime`, `dvars`)

nipype 1.12.0's `compute_dvars` (PyPI sdist `nipype-1.12.0.tar.gz`, SHA-256
`784098a8a54839836fa0eba2fa1c2040a365a272d0b28692578b5b15b475aeeb`, `<workspace>/tags/nipype-1.12.0`;
Copyright (c) 2009-2016, Nipype developers, Apache-2.0) with nitime 0.12.1's AR(1) estimate (PyPI
sdist `nitime-0.12.1.tar.gz`, SHA-256
`12cbf488b0655dc05aa18a0ca5982de970302604e461fc92e87ebd9b7539b6ff`, `<workspace>/tags/nitime-0.12.1`;
Copyright (c) 2006-2011, NIPY Developers, BSD-3-Clause; nitime 0.11, fMRIPrep 25.2.6's, sdist
SHA-256 `e087bc7ee93d08a767ffae13b027cdd8da36754d7a202a4145661eaeaa85d3fd`, `<workspace>/tags/nitime-0.11`:
numerically the same on these functions), SciPy 1.18.1's and 1.15.3's `linalg.solve` (the 1 × 1
paths; BSD-3-Clause, notice above) and numpy 2.5.4's complex64 product and `np.dot` dispatch
(Copyright (c) 2005-2025, NumPy Developers, BSD-3-Clause).

| larmorx | Upstream | Licence |
|---|---|---|
| `nitime::{cmul, cmul_plain, cmul_inplace}` | numpy 2.5.4 `numpy/_core/src/umath/loops_arithm_fp.dispatch.c.src` (`@TYPE@_multiply` for `CFLOAT`: the contiguous SIMD loop, `simd_cmul_f32`, `npyv_muladdsub_f32`; the scalar loop it takes for a zero output stride) | BSD-3-Clause |
| `nitime::{CrosscovPlan, fft_size}` (`fftconvolve_reversed`, `crosscov`, `crosscov_all_lags`, `autocorr_01`) | nitime 0.12.1 `nitime/utils.py` (`fftconvolve` with `axis`, `crosscov`, `autocov`, `autocorr`) | BSD-3-Clause |
| `nitime::{scipy_solve_1x1, SolveRelease, SolveError}` | SciPy 1.18.1 `scipy/linalg/_basic.py` (`solve`: `_asarray_validated(check_finite)`, `a_is_scalar`, `LinAlgError("A singular matrix detected.")`); SciPy 1.15.3 `scipy/linalg/_basic.py` (`solve`: the shape checks, `_find_matrix_structure`'s "diagonal", `_matrix_norm_diagonal`'s `asarray_chkfinite`, `x = (b.T / diag(a)).T`, `_solve_check`); `scipy/linalg/_special_matrices.py` (`toeplitz`) | BSD-3-Clause |
| `nitime::{ar_est_yw1, ArYw1}` | nitime 0.12.1 `nitime/algorithms/autoregressive.py` (`AR_est_YW`); numpy 2.5.4 `arraytypes.c.src` (`FLOAT_dot`, OpenBLAS's `sdot` of one element) for `sigma_v` | BSD-3-Clause |
| `dvars::{compute_dvars, DvarsOptions, Dvars, DvarsError}` | nipype 1.12.0 `nipype/algorithms/confounds.py` (`compute_dvars`, `_AR_est_YW`, `regress_poly`); numpy 2.5.4 `numpy/_core/src/common/cblasfuncs.c` (`cblas_matrixproduct`: the vector-matrix `gemv`, the dot product of one voxel, the scalar and outer products), `numpy/lib/_shape_base_impl.py` (`apply_along_axis`'s order and its empty-axis `ValueError`), `numpy/_core/_methods.py` (`_mean`), `numpy/_core/src/umath/ufunc_object.c` (the floating-point error order: divide, overflow, underflow, invalid) | Apache-2.0, BSD-3-Clause |

The numerics underneath are the parent crates': numpy's `pinv` (`larmorx_transform::numpy_linalg::svd`),
OpenBLAS's `dgemv_t` and `ddot` (`larmorx_transform::openblas::level2`), numpy's median,
percentiles and reductions (`larmorx_core::numpy`), SciPy's FFTs (`scipy_fft`, above).

## Relay 2f-i: nipype's CompCor (`compcor`)

nipype 1.12.0's CompCor numerics (`nipype/algorithms/confounds.py`; sdist and notice above;
`_cosine_drift` and `_full_rank` are nipype's copies of nipy 0.4.1's `nipy/modalities/fmri/
design_matrix.py`, Copyright (c) 2006-2024, NIPY Developers, BSD-3-Clause), with numpy 2.5.4's,
2.3.5's and 2.2.6's arithmetic (BSD-3-Clause, notice above).

| larmorx | Upstream | Licence |
|---|---|---|
| `compcor::cosine_drift` | nipype 1.12.0 `_cosine_drift` (from nipy 0.4.1); the cosine through `larmorx_core::math::Libm` (CORE-MATH's correctly rounded `cos`, or glibc 2.35's bits for `reproduce`: `cos_glibc`) | Apache-2.0, BSD-3-Clause |
| `compcor::{regress_poly, Filtered, Basis}` | nipype 1.12.0 `regress_poly`; numpy 2.5.4 `numpy/polynomial/legendre.py` (`Legendre.basis`, `legval`, via `larmorx_core::numpy::polynomial`), `numpy/_core/function_base.py` (`linspace`), `numpy/linalg/_linalg.py` (`pinv`), `numpy/_core/src/common/cblasfuncs.c` (`np.dot`'s dispatch) | Apache-2.0, BSD-3-Clause |
| `compcor::cosine_filter` | nipype 1.12.0 `cosine_filter`, `_full_rank` (from nipy 0.4.1, `larmorx_transform::numpy_linalg::gesvd::nipype_full_rank`); numpy 2.5.4 `lstsq` (`larmorx_transform::numpy_linalg::gelsd::numpy_lstsq`) | Apache-2.0, BSD-3-Clause |
| `compcor::{compute_tstd, tcompcor_mask}` | nipype 1.12.0 `_compute_tSTD`, `TCompCor._process_masks`; numpy 2.5.4 `numpy/_core/_methods.py` (`_var`, `_std`), `numpy/lib/_function_base_impl.py` (`percentile`'s linear method with an integer `q`), `numpy/_core/fromnumeric.py` (`round`) | Apache-2.0, BSD-3-Clause |
| `compcor::{compute_noise_components, NoiseOptions, NoiseComponents, MaskRows, MaskMetadata, Libraries, Criterion, FilterType, FailureMode, CompcorError}` | nipype 1.12.0 `compute_noise_components`, `fallback_svd` (`larmorx_transform::numpy_linalg::gesvd::nipype_fallback_svd`); numpy 2.5.4 `numpy/_core/fromnumeric.py` (`sum`, `cumsum`, `searchsorted`) | Apache-2.0, BSD-3-Clause |

The Python layer (`python/larmorx/confounds/compcor.py`) follows nipype 1.12.0's `CompCor`,
`ACompCor`, `TCompCor` (`_run_interface`, `_process_masks`, `_make_headers`), `combine_mask_files`
and the functions' argument handling, and nibabel 5.4.2's `squeeze_image`, `get_zooms`,
`get_xyzt_units` (MIT, Copyright the NiBabel developers) on larmorx's nibabel-compatible reader and
writer.

## Relay 2f-ii: nipype's TSNR, NonSteadyStateDetector and FramewiseDisplacement (Python layer)

No Rust in this crate: the three interfaces are Python over the numerics already ported
(`larmorx_core::numpy::reduce` through `_core.numpy_reduce`, `compcor::regress_poly`) and over
larmorx's nibabel-compatible reader and writer (`larmorx_io::nifti::read_first_volumes`, new, and
`save_nibabel`).

| larmorx | Upstream | Licence |
|---|---|---|
| `python/larmorx/confounds/tsnr.py` (`tsnr`, `TsnrResult`) | nipype 1.12.0 `TSNR` (`_run_interface`, `_list_outputs`); numpy 2.5.4 `numpy/_core/_methods.py` (`_mean`, `_var`, `_std`: the summation order of the series' memory), `numpy/lib/_type_check_impl.py` (`nan_to_num`); nibabel 5.4.2 `get_fdata`, `Nifti1Image(data, affine, header)` and `save` | Apache-2.0, BSD-3-Clause, MIT |
| `python/larmorx/confounds/nonsteady.py` (`nonsteady_state_detector`, `NonSteadyStateResult`) | nipype 1.12.0 `NonSteadyStateDetector`, `is_outlier` (`larmorx.niworkflows.bold.is_outlier`); numpy 2.5.4 `mean(axis=0)` of a Fortran-ordered array (float32 in float32, integers in float64); nibabel 5.4.2 `dataobj[:, :, :, :50]` | Apache-2.0, BSD-3-Clause, MIT |
| `python/larmorx/confounds/framewise.py` (`framewise_displacement`, `normalize_mc_params`, `FramewiseDisplacementResult`, `SOURCES`) | nipype 1.12.0 `FramewiseDisplacement` (`_run_interface`), `nipype/utils/misc.py` (`normalize_mc_params`, but its `NIPY` branch: not ported yet); numpy 2.5.4 `sum(axis=1)`, `mean()` (`_core.numpy_reduce`), and numpy's own `loadtxt`, `apply_along_axis`, `savetxt` | Apache-2.0, BSD-3-Clause |
| `python/larmorx/niworkflows/patches.py` (`robust_acompcor`, `robust_tcompcor`) | niworkflows 1.15.4 `interfaces/patches.py` (`RobustACompCor`, `RobustTCompCor`; their retries left out: PIPE-48) | Apache-2.0 |

