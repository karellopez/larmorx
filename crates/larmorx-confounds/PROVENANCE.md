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
| `python/larmorx/confounds/framewise.py` (`framewise_displacement`, `normalize_mc_params`, `FramewiseDisplacementResult`, `SOURCES`) | nipype 1.12.0 `FramewiseDisplacement` (`_run_interface`), `nipype/utils/misc.py` (`normalize_mc_params`; its `NIPY` branch since relay 2f-ii-b, below); numpy 2.5.4 `sum(axis=1)`, `mean()` (`_core.numpy_reduce`), and numpy's own `loadtxt`, `apply_along_axis`, `savetxt` | Apache-2.0, BSD-3-Clause |
| `python/larmorx/niworkflows/patches.py` (`robust_acompcor`, `robust_tcompcor`) | niworkflows 1.15.4 `interfaces/patches.py` (`RobustACompCor`, `RobustTCompCor`; their retries left out: PIPE-48) | Apache-2.0 |

## Relay 2f-ii-b: `FramewiseDisplacement`'s `NIPY` source (`nipy`)

nipype's `normalize_mc_params(params, "NIPY")` imports nipy's `to_matrix44` and `aff2euler`; nipy
0.6.1 (the newest release, BSD-3-Clause, Copyright (c) 2006-2024, NIPY Developers) from its GitHub
tag's tarball (`<workspace>/tags/nipy-0.6.1/`, SHA-256 `79babfd76618a3573a4d5920d1fe46829a19df92ff3b98efa16bd9a51f7a7610`:
PyPI holds wheels only), and nibabel 5.4.2's `mat2euler` (MIT; the same function in 5.3.2, fMRIPrep
25.2.6's). The notices are in `src/nipy.rs` and `python/larmorx/confounds/_nipy.py`.

| larmorx | Upstream | Licence |
|---|---|---|
| `nipy::{threshold, rotation_vec2mat, to_matrix44, MAX_ANGLE, SMALL_ANGLE, MAX_DIST, LOG_MAX_DIST}` | nipy 0.6.1 `nipy/algorithms/registration/affine.py` (`threshold`, `rotation_vec2mat`, `to_matrix44` and the module's constants); numpy 2.5.4's pairwise sum (`larmorx_core::numpy::reduce::add_reduce`) and `np.dot` of 3 × 3 matrices (`larmorx_transform::numpy_linalg::matmul3`: OpenBLAS's Haswell `dgemm`); `sin`, `cos`, `exp` through `larmorx_core::math::Libm` (CORE-MATH's, or glibc 2.35's bits for `reproduce`: numpy's path without AVX-512) | BSD-3-Clause |
| `nipy::aff2rot_zooms` | nipy 0.6.1 `nipy/algorithms/registration/scripting.py` (`aff2rot_zooms`); numpy 2.5.4 `sum(axis=0)` (`add_reduce_axis`), `linalg.det` (`numpy_linalg::det3_is_negative`), `linalg.svd` (`numpy_linalg::svd3`) | BSD-3-Clause |
| `nipy::{mat2euler, py_atan2}` | nibabel 5.4.2 `nibabel/eulerangles.py` (`mat2euler`, from Ken Shoemake's `EulerAngles.c`); the special values CPython 3.12's `math.atan2` returns (a NaN, infinities, zeros: C99's Annex F values, checked against it), `atan2` correctly rounded (`larmorx_core::math::atan2`, CORE-MATH's) | MIT |
| `nipy::{normalize, normalize_rows, NipyRows, NipyError, RowError}` | nipype 1.12.0 `nipype/utils/misc.py` (`normalize_mc_params`'s `NIPY` branch), numpy's `apply_along_axis` row by row (the first row that stops) | Apache-2.0 |
| `python/larmorx/confounds/_nipy.py` | nipy 0.6.1's five functions and nibabel 5.4.2's `mat2euler`, copied (the rows larmorx's own arithmetic does not take: another type or shape; with larmorx's `exact_squares` switch, PIPE-60) | BSD-3-Clause, MIT |

## Relay 2g-i: pandas' tables and fMRIPrep's and niworkflows' table steps (`pandas`, Python layer)

pandas 3.0.6 (the newest) and 2.2.3 (fMRIPrep 25.2.6's image), BSD-3-Clause (Copyright (c)
2008-2011 AQR Capital Management, LLC, Lambda Foundry, Inc. and PyData Development Team, and
2011-2026 Open source contributors), from their sdists in `<workspace>/tags/pandas-3.0.6/` and
`pandas-2.2.3/` (SHA-256 checked against PyPI by `validation/investigations/confounds/fetch_sdist.py`);
the notice is in `src/pandas.rs` and `python/larmorx/confounds/_tables.py`.

| larmorx | Upstream | Licence |
|---|---|---|
| `pandas::{precise_xstrtod, Xstrtod, PandasRelease, parse, try_double, NA_VALUES, is_na}` | pandas 3.0.6 and 2.2.3 `pandas/_libs/src/parser/tokenizer.c` (`precise_xstrtod`; its exponent: C's `strtol` in 3.0.6, its own digit loop in 2.2.3), `pandas/_libs/include/pandas/portable.h` (`isspace_ascii`, `isdigit_ascii`, `toupper_ascii`), `pandas/_libs/parsers.pyx` (`_try_double_nogil`: the missing-value set `STR_NA_VALUES` and the infinity words); the correctly rounded reading of the same texts by Rust's parser (PD-01) | BSD-3-Clause |
| `crates/larmorx-py/src/confounds/pandas.rs` (`_core.pandas_try_double`) | the binding of the above | Apache-2.0 |
| `python/larmorx/confounds/_tables.py` (`read_tsv`, `write_tsv`, `tsv_text`, `ujson_double`, `json_value`, `Table`, `Column`, `EmptyDataError`, `ParserError`) | pandas 3.0.6 and 2.2.3: `parsers.pyx` (`TextReader._get_header`'s renaming, `_convert_tokens` with `dtype_cast_order` int64, float64, bool, object, `_try_int64`, `_try_uint64`, `_try_pylong`, `_try_bool_flex`, `_maybe_upcast`: PD-03), `tokenizer.c` (`tokenize_bytes`' states for records, quotes and blank lines; `str_to_int64`, `str_to_uint64`, `to_boolean`), `io/common.py` (`infer_compression`, `get_handle`'s `gzip.GzipFile`), `io/formats/csvs.py` (`CSVFormatter`'s `csv.writer` settings), `core/indexes/base.py` (`get_values_for_csv`: `astype(str)`), `_libs/src/vendored/ujson/lib/ultrajsonenc.c` (`Buffer_AppendDoubleUnchecked`, the same in both releases; PD-02), `_libs/src/vendored/ujson/python/objToJSON.c` (NaN and infinities `null`) | BSD-3-Clause |
| `python/larmorx/confounds/fmriprep.py` (`fmriprep_framewise_displacement`, `filter_dropped`, `rename_acompcor`, `gather_confounds`, `RenameACompCorResult`, `GatherConfoundsResult`) | fMRIPrep 25.2.7 `fmriprep/interfaces/confounds.py` (`FramewiseDisplacement`, `FilterDropped`, `RenameACompCor`, `GatherConfounds`, `_gather_confounds`; the same in 25.2.6); pandas 3.0.6 `DataFrame.diff` (`algos.diff`, `diff_2d`), `DataFrame._reduce` with `axis=1` (the transpose, then numpy's `sum(axis=1)` of a Fortran-ordered block: column after column), `concat(axis=1)` with `Index.union(sort=False)` (`RangeIndex._union`, `Index._union`), `rename`, boolean and label indexing | Apache-2.0, BSD-3-Clause |
| `python/larmorx/niworkflows/utility.py` (`add_tsv_header`, `dict_merge`, `tsv2json`) | niworkflows 1.15.4 `niworkflows/interfaces/utility.py` (`AddTSVHeader`, `DictMerge`, `TSV2JSON`, `_tsv2json`; the same in 1.15.2): PIPE-61 and PIPE-62 in `AddTSVHeader`; numpy's own `loadtxt` and `savetxt` | Apache-2.0 |

## Relay 2g-ii: niworkflows' confound model steps (`expansion`, Python layer)

niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; `<workspace>/tags/niworkflows-1.15.4/`;
these functions are the same in 1.15.2, fMRIPrep 25.2.6's, and 1.15.3), with numpy's
arithmetic as pandas hands the tables to it (numpy 2.5.4, 2.3.5 and 2.2.6, BSD-3-Clause,
Copyright (c) 2005-2026, NumPy Developers).

| larmorx | Upstream | Licence |
|---|---|---|
| `expansion::{power, power_one}` | numpy 2.5.4 `numpy/_core/src/multiarray/number.c` (`fast_scalar_power`: an integer exponent of 2 is `square`, -1 `reciprocal`, a float 0.5 `sqrt`) and `numpy/_core/src/umath/loops_umath_fp.dispatch.c.src` (`DOUBLE_power`: the stride-0 shortcuts of 2.5.4 for -1, 0, 0.5, 1, 2; the C library's `pow` otherwise, Intel's SVML on AVX-512 not followed: NP-04's class), the same results in 2.3.5 and 2.2.6 for the integer exponents niworkflows' grammar gives; glibc 2.35's `pow` (`larmorx_core::math::pow_glibc`, GLIBC-07) or CORE-MATH's `cr_pow` (`larmorx_core::math::pow`) | Apache-2.0 AND BSD-3-Clause |
| `crates/larmorx-py/src/confounds/expansion.rs` (`_core.numpy_power_f64`) | the binding of the above | Apache-2.0 |
| `python/larmorx/niworkflows/confounds.py` (`expand_model`, `spike_regressors_node`, `normalize_motion_params`, `parse_formula`, `temporal_derivatives`, `exponential_terms`, `spike_regressors`, `ConfoundTable`, `ModelTerms`, `DEFAULT_FORMULA`, `FORMATS`) | niworkflows 1.15.4 `niworkflows/interfaces/confounds.py` (`ExpandModel`, `SpikeRegressors`, `NormalizeMotionParams`, `spike_regressors`, `temporal_derivatives`, `exponential_terms`, `_order_as_range`, `_check_and_expand_exponential`, `_check_and_expand_derivative`, `_check_and_expand_subformula`, `parse_expression`, `_get_matches_from_data`, `_get_variables_from_formula`, `_expand_shorthand`, `_unscramble_regressor_columns`, `parse_formula`; PIPE-43, PIPE-64 to PIPE-70); pandas 3.0.6 and 2.2.3 as these functions use it (`DataFrame.__getitem__` with a list: every column of a repeated name for each time it is named; `concat(axis=1)`; `DataFrame.__array__`'s common type; `DataFrame(columns=..., data=...)`'s shape check and message; `DataFrame ** o` block by block); numpy's own `diff`, `**`, `loadtxt`, `apply_along_axis` and `savetxt` | Apache-2.0 |

## Relay 2g-iii: fMRIPrep's motion parameters and root mean square deviation (`motion`, Python layer)

fMRIPrep 25.2.7 and 25.2.6 (Apache-2.0, the NiPreps developers; `<workspace>/tags/fmriprep-25.2.7/`,
`fmriprep-25.2.6/`), niworkflows 1.15.4 (its copies of the same interfaces; 1.15.2 and 1.15.3 carry
25.2.6's `FSLMotionParams`), nitransforms 25.1.0 (MIT, Copyright (c) 2021 The NiPy developers;
`<workspace>/tags/nitransforms-25.1.0/`), nibabel 5.4.2 (MIT; `voxel_sizes`, `from_matvec`), SciPy
1.18.1 (BSD-3-Clause, Copyright (c) 2001-2002 Enthought, Inc. 2003-2024, SciPy Developers;
`<workspace>/tags/scipy-1.18.1/`; SciPy 1.15.3's `_rotation.pyx`, `<workspace>/tags/scipy-1.15.3/`, has
the same arithmetic, and SciPy 1.15.2 in fMRIPrep 25.2.6's image gives the same bits) and numpy 2.5.4,
2.3.5 and 2.2.6 with OpenBLAS 0.3.34 and 0.3.30 (BSD-3-Clause) as `larmorx_transform` already follows
them. The notices are in `src/motion.rs`.

| larmorx | Upstream | Licence |
|---|---|---|
| `motion::{itk_to_ras, first_singular}` | nitransforms 25.1.0 `nitransforms/io/itk.py` (`ITKLinearTransform.to_ras`: `LPS.dot(c_pos.dot(matrix.dot(c_neg.dot(LPS))))`, the float32 offset), `nitransforms/linear.py` (`Affine.__init__`'s inverse, `LinearTransformsMapping`); nibabel 5.4.2 `nibabel/affines.py` (`from_matvec`); numpy's `dot` (OpenBLAS's `dgemm`: `larmorx_transform::openblas::matmul4`) and `inv` (`dgesv`: `openblas::inv4`) | Apache-2.0 AND MIT AND BSD-3-Clause |
| `motion::{FslSpace, fsl_from_ras, det3, det4}` | nitransforms 25.1.0 `nitransforms/io/fsl.py` (`FSLLinearTransform.from_ras`, `FSLLinearTransformArray.from_ras`, `_fsl_aff_adapt`); nibabel 5.4.2 `voxel_sizes`; numpy's `det` (`umath_linalg.cpp`: `slogdet_single_element`, `det_from_slogdet`, OpenBLAS's `getf2_k`: `openblas::lu_sign_and_diagonal3`, the new `lu_sign_and_diagonal4`) | Apache-2.0 AND MIT AND BSD-3-Clause |
| `motion::{scipy_from_matrix, quat_from_orthogonal, scipy_as_euler, EulerSeq, SliceOrder}` | SciPy 1.18.1 `scipy/spatial/transform/_rotation_cy.pyx` (`from_matrix`: the determinants, the Gramian `matrix @ transpose(matrix)` (numpy's `dsyrk` case: `openblas::matmul::{syrk_case, numpy_syrk}`) with `isclose(atol=1e-12)`, numpy's `svd` (`numpy_linalg::svd3`, JOBZ='A', both LAPACK releases) and `U @ Vt` (`numpy_linalg::matmul3`); `_from_matrix_orthogonal`, `_argmax4`, `_normalize4`; `as_euler`, `_get_angles`: Bernardes and Viollet's method with the C library's `atan2` and `hypot`, here CORE-MATH's correctly rounded `larmorx_core::math::{atan2, hypot}` (D4)); SciPy 1.15.3 `_rotation.pyx` (the same arithmetic; the gimbal warning per rotation) | Apache-2.0 AND BSD-3-Clause |
| `motion::{motion_params, center_of_gravity, rms_deviation, Motion, MotionRelease, MotionError}` | fMRIPrep 25.2.7 and 25.2.6 `fmriprep/interfaces/confounds.py` (`FSLMotionParams`: both releases' forms; `FSLRMSDeviation`); numpy's `matmul` of a matrix and a vector (`matmul.c.src`: `DOUBLE_gemv`, OpenBLAS's `dgemv_t` for `M`, `dgemv_n` for the strided `M.T`: `openblas::matmul::numpy_matmul`), stacked `inv` and `@`, `t @ t.T` (`dsyrk`: its diagonal, the Haswell kernel's fused chain; `0 + ddot` for one row), `trace` (the diagonal added in order) | Apache-2.0 AND BSD-3-Clause |
| `crates/larmorx-py/src/confounds/motion.rs` (`_core.nitransforms_itk_to_ras`, `numpy_first_singular4`, `numpy_det4_is_positive`, `nitransforms_fsl_from_ras`, `scipy_rotation_from_matrix`, `scipy_rotation_as_euler`, `fmriprep_motion_params`, `fmriprep_rms_deviation`) | the bindings of the above | Apache-2.0 |
| `python/larmorx/confounds/motion.py` (`fsl_motion_params`, `fsl_rms_deviation`, `MotionParamsResult`, `RmsDeviationResult`, `TransformFileError`, `FMRIPREP_NEWEST`) | fMRIPrep 25.2.7 and 25.2.6 `FSLMotionParams`, `FSLRMSDeviation` (`fname_presuffix`, the table written by pandas' `to_csv`: `larmorx.confounds._tables`); nitransforms 25.1.0 `nitransforms/io/itk.py` (`ITKLinearTransformArray.from_string`, `ITKLinearTransform.from_string`, with numpy's own `genfromtxt`), `nitransforms/linear.py` (`load`: one transform as one `Affine`, PIPE-71; `Affine`'s last-row check and message); SciPy 1.18.1 `scipy/ndimage/_measurements.py` (`center_of_mass`, `sum_labels`, `_stats` without labels) with numpy's sums in each release's order (`larmorx_core::numpy::reduce`: a byte-swapped float array in chunks of 8,192 in every release, measured); SciPy's `ValueError` message of a non-positive determinant and its gimbal-lock warning; PIPE-72 (rule B) and PIPE-73 (rule A) | Apache-2.0 AND MIT |
| `python/larmorx/niworkflows/confounds.py` (`fsl_motion_params`, `fsl_rms_deviation`, `framewise_displacement`, `NIWORKFLOWS_NEWEST`) | niworkflows 1.15.4 `niworkflows/interfaces/confounds.py` (`FSLMotionParams`: 25.2.7's form, 25.2.6's in 1.15.2 and 1.15.3; `FSLRMSDeviation`; `FramewiseDisplacement`) | Apache-2.0 |

## Relay 2g-iv: fMRIPrep's and niworkflows' masks and signals (`signals`, Python layer)

fMRIPrep 25.2.7 (Apache-2.0, the NiPreps developers; `<workspace>/tags/fmriprep-25.2.7/`; the same
functions in 25.2.6), niworkflows 1.15.4 (Apache-2.0, the NiPreps developers;
`<workspace>/tags/niworkflows-1.15.4/`; the same in 1.15.2 and 1.15.3), nibabel 5.4.2 (MIT; reading
and writing through `larmorx.io`, the empty array of an image without voxels), SciPy 1.18.1
(`binary_dilation`, `gaussian_filter` through `larmorx.ndimage`), scikit-image 0.26.0 (`ball` through
`larmorx.skimage`; 0.25.2 in fMRIPrep's image gives the same footprints) and numpy 2.5.4, 2.3.5 and
2.2.6 (BSD-3-Clause, Copyright (c) 2005-2026, NumPy Developers: the masked mean's summation order,
`percentile`'s weak `q` through `larmorx_core::numpy`, the casts the ports reproduce).

| larmorx | Upstream | Licence |
|---|---|---|
| `signals::{Series, voxel_offsets, signal_means}` | niworkflows 1.15.4 `niworkflows/interfaces/images.py` (`SignalExtraction._run_interface`: `data[mask, :].mean(axis=0)` of `get_fdata()`); numpy's boolean indexing (a C-ordered `(N, T)` copy in the mask's C order) and `mean` (`_methods._mean`: `umr_sum` along the outer axis in index order from `+0.0`, pairwise along a contiguous one: `larmorx_core::numpy::reduce::add_reduce_ordered`; the division by the count) | Apache-2.0 AND BSD-3-Clause |
| `crates/larmorx-py/src/confounds/signals.rs` (`_core.niworkflows_signal_means`) | the binding of the above | Apache-2.0 |
| `python/larmorx/confounds/masks.py` (`binary_union`, `get_zooms`, `mask2vf`, `acompcor_masks`, `acompcor_masks_node`, `AcompcorMasksResult`) | fMRIPrep 25.2.7 `fmriprep/workflows/bold/confounds.py` (`_binary_union`, `_get_zooms`), `fmriprep/utils/confounds.py` (`mask2vf`, `acompcor_masks`), `fmriprep/interfaces/confounds.py` (`aCompCorMasks` and its input spec); nibabel's `ArrayProxy.__array__(dtype)` (`_get_scaled`: the scaled values cast to `promote_types(dtype, int64)` or `uint8`, then numpy's cast: PIPE-74, PIPE-76) and `get_zooms`; numpy's `percentile`, `clip` and casts; PIPE-74, PIPE-75 (rule B), PIPE-76 | Apache-2.0 |
| `python/larmorx/niworkflows/morphology.py` (`binary_dilation`, `binary_subtraction`, `image_binary_dilation`) | niworkflows 1.15.4 `niworkflows/interfaces/morphology.py` (`BinaryDilation`, `BinarySubtraction`, `image_binary_dilation`; `traits.Int`'s radius; PIPE-77, rule B) | Apache-2.0 |
| `python/larmorx/niworkflows/images.py` (`signal_extraction`) | niworkflows 1.15.4 `niworkflows/interfaces/images.py` (`SignalExtraction` and its input spec: `prob_thres`'s range, `class_labels`, `out_file`; nibabel's `four_to_three`); numpy's `unique`, comparisons, `astype(str)` (the shortest `repr`), `vstack` and `savetxt(fmt="%s", delimiter="\t")` (a text file in the locale's encoding: UTF-8, `"\n"`); PIPE-78 (rule B) | Apache-2.0 |

## Relay 2h: fMRIPrep's confound estimation workflow (Python layer)

fMRIPrep 25.2.7 (Apache-2.0, the NiPreps developers; `<workspace>/tags/fmriprep-25.2.7/`; the same
`workflows/bold/confounds.py` and the same `ds_confounds` in 25.2.6) and niworkflows 1.15.4
(Apache-2.0, the NiPreps developers: `FixHeaderApplyTransforms`' header message, through
`larmorx.niworkflows.header.copy_xform`). No code of a crate: the workflow is composed in Python
from the ports above and the existing `lx.ants`/`larmorx._nipype` (antsApplyTransforms' nipype
line), `lx.niworkflows` (`apply_mask`, `binarize`, `derivatives_data_sink`) and `larmorx.io`
(nibabel's `set_qform` on the release's LAPACK: `use_lapack`).

| larmorx | Upstream | Licence |
|---|---|---|
| `python/larmorx/pipelines/larmorprepx/confounds.py` (`bold_confounds`, `BoldConfoundsResult`, `ConfoundsReportInputs`, `ds_confounds`, `select_cols`, `bold_confounds_description`, `SIGNALS_CLASS_LABELS`, `MODEL_FORMULA`, `MOTION_METADATA`) | fMRIPrep 25.2.7 `fmriprep/workflows/bold/confounds.py` (`init_bold_confs_wf`: its nodes, their parameters and connections, the reportlets' inputs (`mrg_compcor`, `merge_compcor_metadata`, `_last`, `_select_cols`), `workflow.__desc__`; PIPE-41, rule A; PIPE-49, rule C; PIPE-79, rule D, open), `fmriprep/workflows/bold/base.py` (`bold_confounds_wf`'s inputs, `ds_confounds`), `fmriprep/utils/bids.py` (`dismiss_echo`); nipype 1.12.0 `interfaces/utility/base.py` (`Merge`, `ravel_inputs`); niworkflows 1.15.4 `niworkflows/interfaces/fixes.py` (`FixHeaderApplyTransforms`), `niworkflows/utils/misc.py` (`listify`) | Apache-2.0 |

## Relay 2i: nipype's interface runner, `nipypecli run` (`larmorx nipype run`, Python layer)

`larmorx nipype run` runs the interfaces of the nine modules of fMRIPrep's confound estimation
from the shell as nipype 1.12.0's `nipypecli run` does, through the ports above and the field
maps' part's niworkflows functions. The corrections of nipype's command line (PIPE-81, PIPE-82,
rule A; PIPE-83 kept, rule C) are in `python/larmorx/nipype/_run.py`; the register's entries in
`larmorx_core::defects`.

| larmorx | Upstream | Licence |
|---|---|---|
| `python/larmorx/nipype/_run.py` (`run`, `list_interfaces`, `ported_interfaces`, `MODULES`) | nipype 1.12.0 `nipype/scripts/cli.py` (`run`), `scripts/instance.py` (`list_interfaces`), `scripts/utils.py` (`add_args_options`, `CONTEXT_SETTINGS`, `UNKNOWN_OPTIONS`, `check_not_none`), `utils/nipype_cmd.py` (`run_instance`), `interfaces/base/specs.py` (`BaseTraitedSpec.__repr__`, `_xor_warn`, `_deprecated_warn`), `interfaces/base/core.py` (`_check_requires`, `_check_xor`, `_check_mandatory_inputs`, `aggregate_outputs`' missing-file message), `interfaces/base/traits_extension.py` (`BasePath.validate`, `MultiObject.validate`, `OutputMultiObject.get`), `utils/logger.py` (the log lines' format); click 8.5.0 `click/parser.py` (`_process_args_for_options`, `_process_opts`, `_match_long_opt`, `_match_short_opt` with `ignore_unknown_options`) and `click/exceptions.py` (`UsageError.show`: the usage line only with a context); traits 7.2.0 `trait_errors.py` (`TraitError`'s message), `trait_base.py` (`add_article`), `trait_types.py` and `trait_handlers.py` (the validation of `Bool`, `Int`, `Float`, `Str`, `Enum`, `Range`, `List`, `Tuple`, `Dict`, `Any`, `Instance` and `Either`), `trait_list_object.py` (`Each element of the` for a list's item) | Apache-2.0 AND BSD-3-Clause |
| `python/larmorx/nipype/interfaces.json` | written by `validation/investigations/confounds/nipype_cli_spec.py` from nipype 1.12.0, niworkflows 1.15.4 and fMRIPrep 25.2.7 themselves (traits 7.2.0): each interface's `add_argument` calls (their help texts are nipype's `get_trait_desc` of the interfaces' descriptions: nipype's, niworkflows' and fMRIPrep's), the corrected calls, the inputs' printed defaults, validations, traits' texts and metadata, the outputs | Apache-2.0 |
| `python/larmorx/nipype/_adapters.py` | the interfaces' inputs and outputs as nipype's, niworkflows' and fMRIPrep's `_run_interface` and `_list_outputs` give them (CompCor's `pre_filter_file` and `metadata_file`, `ExpandModel`'s and `SpikeRegressors`' `output_file`: the names given) | Apache-2.0 |
| `python/larmorx/cli/__init__.py` (`nipype run` among the Python programs), `crates/larmorx-cli/src/lib.rs` (`PYTHON_PROGRAMS`, the family `nipype`) | larmorx's own | Apache-2.0 |
