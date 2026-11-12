# Provenance: larmorx-transform

**Ported** from the release tags in [UPSTREAM.md](../../UPSTREAM.md). The logic is
re-expressed in Rust; no code was copied verbatim. Arithmetic follows the upstream operation
order, because that order decides bit-identity with ANTs
([docs/findings/itk-transforms.md](../../docs/findings/itk-transforms.md)).

| larmorx | Upstream (ITK v5.4.5, `f51594ad`) | Licence |
|---|---|---|
| `LinearTransform::transform_point`, offset from centre and translation (`ComputeOffset`), `inverse` | `Modules/Core/Transform/include/itkMatrixOffsetTransformBase.hxx` | Apache-2.0 |
| `LinearKind::Euler3D` (matrix `Rz·Rx·Ry`, or `Rz·Ry·Rx` with `ComputeZYX`) | `Modules/Core/Transform/include/itkEuler3DTransform.hxx` | Apache-2.0 |
| `LinearKind::VersorRigid3D`, `Similarity3D` (versor to matrix, scale) | `itkVersorRigid3DTransform.hxx`, `itkSimilarity3DTransform.hxx`, `Modules/Core/Common/include/itkVersor.hxx` | Apache-2.0 |
| `LinearKind::Translation`, `Identity` | `itkTranslationTransform.hxx`, `itkIdentityTransform.h` | Apache-2.0 |
| `Transform::Composite` (queue applied back to front), its inverse | `Modules/Core/Transform/include/itkCompositeTransform.hxx` | Apache-2.0 |
| `DisplacementField::transform_point` (inside test, vector linear interpolation over 8 neighbours with clamped indices), fixed parameters | `Modules/Filtering/DisplacementField/include/itkDisplacementFieldTransform.hxx`, `Modules/Core/ImageFunction/include/itkVectorLinearInterpolateImageFunction.hxx` | Apache-2.0 |
| `BSplineTransform::transform_point_as` (`InsideValidRegion`, the interpolation weights, the weighted coefficients in `R`), `from_itk`, `fixed_parameters` | `Modules/Core/Transform/include/itkBSplineTransform.hxx`, `itkBSplineBaseTransform.hxx`, `Modules/Core/Common/include/itkBSplineInterpolationWeightFunction.hxx`, `itkBSplineKernelFunction.h`, `itkImageBase.hxx` (`TransformPhysicalPointToContinuousIndex`), `itkMath.h` (`FloatAlmostEqual`, `FloatAddULP`) | Apache-2.0 |
| `TransformChain` (the `-t` list as ANTs builds it: options last first, composites expanded, inverses by `GetInverseTransform`) | ANTs v2.6.5 (`fdce4d2f84b6`): `Examples/itkantsRegistrationHelper.h`, `GetCompositeTransformFromParserOption` | Apache-2.0 |
| `MatrixOffset<R>`, `TransformChain::with_precision`, `PrecisionChain`, `DisplacementField::transform_point_as` (every transform in the computation type `R`, as `antsApplyTransforms --float 1` instantiates it: parameters cast to `R`, centres stored as `R`, `ComputeOffset`, `TransformPoint`, `GetInverse` with `itk::Matrix<R>::GetInverse`, Euler `std::cos/sin(R)`, versor norms and `Versor<R>::Set/GetMatrix` with their mixed float and double arithmetic, Similarity's double norm, `TranslationTransform`, the float vector field lookup) | ITK v5.4.5: `itkMatrixOffsetTransformBase.hxx`, `itkMatrix.h/.hxx`, `itkEuler3DTransform.hxx`, `itkVersorRigid3DTransform.hxx`, `itkVersorTransform.hxx`, `itkSimilarity3DTransform.hxx`, `itkTranslationTransform.hxx`, `Modules/Core/Common/include/itkVersor.hxx`, `itkVector.h/.hxx`, `itkDisplacementFieldTransform.hxx`, `itkVectorLinearInterpolateImageFunction.hxx`, `itkImageFunction.h`; VXL: `vnl_matrix_fixed.h` (`vnl_matrix_fixed_mat_mat_mult`) | Apache-2.0, BSD (VXL) |

| `itk_object` (the chain as ITK objects: the class each transform is read as, `GetParameters`/`GetFixedParameters` per class, `GetInverseTransform` and the class it makes, `MatrixOffsetState`'s `ComputeOffset`, `ComputeTranslation`, `SetCenter`/`SetMatrix`/`SetTranslation`/`SetOffset`, `Compose(other, true)`; `TransformChain::collapse_linear`; `TransformChain::displacement_field`) | ITK v5.4.5: `Modules/Core/Transform/include/itkMatrixOffsetTransformBase.h/.hxx`, `itkTransform.h` (`InvertTransform`), `itkAffineTransform.hxx`, `itkEuler3DTransform.hxx` (`GetFixedParameters`), `itkVersorRigid3DTransform.hxx`, `itkSimilarity3DTransform.hxx`, `itkTranslationTransform.hxx`, `itkIdentityTransform.h`, `itkCompositeTransform.h/.hxx`, `Modules/Core/Common/include/itkMatrix.hxx`, `itkImageBase.h`, `Modules/Filtering/DisplacementField/include/itkTransformToDisplacementFieldFilter.h/.hxx`; ANTs v2.6.5: `Examples/itkantsRegistrationHelper.hxx` (`CollapseLinearTransforms`), `Examples/itkantsRegistrationHelper.h` (the identity `MatrixOffsetTransformBase`) | Apache-2.0 |

`larmorx_core::linalg::inverse3_itk` reproduces `itk::Matrix::GetInverse`
(`Modules/Core/Common/include/itkMatrix.h`, an SVD inverse through vnl) exactly: it is an
operation-for-operation port of vnl's SVD inverse (`larmorx-core`, `vnl_svd.rs`).

## nitransforms chains and fMRIPrep's one-shot resampler

**Ported** from nitransforms 25.1.0 (tag `25.1.0` → `c10f63d1a1f7`, MIT; the version
fMRIPrep's lockfile pins), fMRIPrep at `21a490fb` (Apache-2.0) and nibabel 5.4.2 (MIT). The
operation order is kept, including where numpy's BLAS fuses multiply-adds, so results are
bit-identical to fMRIPrep on x86-64 Linux (`docs/validation/resample-series.md`). Matrices
(inverses, ITK-to-RAS conversions, head-motion products) are computed with nitransforms' and
fMRIPrep's expressions by the Python layer (`python/larmorx/transforms/`), through
`openblas` below, so they carry the bits numpy gives on x86-64 with FMA, on every platform.

| larmorx | Upstream | Licence |
|---|---|---|
| `nitransforms::ndcoords`, `dot_row`, `to_f32`, `affine_map` | nitransforms `base.py` (`ImageGrid.ndcoords`, `_as_homogeneous`, `_apply_affine`), `linear.py` (`Affine.map`) | MIT |
| `nitransforms::DenseField` (`from_deltas`, `map`: on-grid lookup, cubic interpolation with NaN outside, fallback to the input point) | nitransforms `nonlinear.py` (`DenseFieldTransform.__init__`, `.map`) | MIT |
| `nitransforms::map_points`, `Step` | nitransforms `manip.py` (`TransformChain.map`) | MIT |
| `resample_series::SeriesResampler` (coordinates once per run, `resample_volume`, `resample_series`), `PeInfo`, `ResampleOptions`, the voxel-shift map and Jacobian | fMRIPrep `fmriprep/interfaces/resampling.py` (`resample_image`, `resample_series`, `resample_series_async`, `resample_vol`) | Apache-2.0 |
| `resample_series::apply_affine` | nibabel `affines.py` (`apply_affine`) | MIT |
| `resample_series::jacobian` | behaviour of numpy's `gradient` (unit spacing, first-order edges), not translated | BSD-3-Clause (numpy), reference only |
| `nitransforms::dot_row` (`+ 0`: the zero sign `dgemm` stores), `dot_row_single`, `resample_series::apply_affine_single` (one point: numpy's `dgemv` path) | OpenBLAS 0.3.30: `kernel/x86_64/dgemm_kernel_4x8_haswell.S`, `dgemv_t_4.c` and `dgemv_t_microk_haswell-4.c` (see below) | BSD-3-Clause |
| `nitransforms::closest_orthogonal` | nibabel `orientations.py` (`io_orientation`: `P[:, keep] @ Qs[keep]`, the rank tolerance), with vnl's SVD (`larmorx_core::vnl_svd`) in place of numpy's `dgesdd` | MIT |

## numpy's 4×4 products and inverses (`openblas`)

**Ported** from the OpenBLAS 0.3.30 release tarball (SHA-256 `27342cff5186…`, tag `v0.3.30` →
`993fad6aebbc`, BSD-3-Clause; notice in the file and in NOTICE), the version numpy 2.3.5's
wheels bundle (scipy-openblas64 0.3.30, `numpy.show_config()`); **checked against numpy 2.5.4
and the OpenBLAS 0.3.34 its wheels bundle** (scipy-openblas64 0.3.34.106.0, 2026-10-29): every
file below is the same source in 0.3.34 or changes nothing for float64 4 × 4 matrices, the
wheel's Haswell kernels are compiled alike, and the `numpy-linalg` suite agrees as before
(`docs/findings/numpy-blas.md`, "numpy 2.5.4 and OpenBLAS 0.3.34"). Only the operation order of
the kernels `kernel/x86_64/KERNEL.HASWELL` selects is reproduced, for 4×4 float64 matrices; it
was checked against the disassembly of numpy's `libscipy_openblas64_` (GCC 10 compiled the C
kernels with `-mavx2` but without `-mfma`, except `dgemv_t_4.c`) and validated bit for bit
(`docs/validation/numpy-linalg.md`). Which routine numpy calls, and with which layout, was read
in numpy 2.3.5 (BSD-3-Clause, reference only): `numpy/_core/src/umath/matmul.c.src`,
`numpy/_core/src/common/cblasfuncs.c`, `numpy/linalg/umath_linalg.cpp` (`inv`,
`linearize_matrix`).

| larmorx | Upstream (OpenBLAS 0.3.30) | Licence |
|---|---|---|
| `openblas::gemm`, `matmul4` (fused chain from +0 in index order, `alpha · acc + C` with C cleared to +0) | `interface/gemm.c` (no small-matrix kernel for Haswell: `kernel/generic/gemm_small_matrix_permit.c`), `driver/level3/level3.c` (`BETA_OPERATION`), `kernel/x86_64/dgemm_beta_skylakex.c`, `kernel/x86_64/dgemm_kernel_4x8_haswell.S` (`INIT4x4`, `KERNEL4x4_SUB`, `SAVE4x4`) | BSD-3-Clause |
| `openblas::inv4` (B = I, then `getrs`), `getf2` (left-looking LU: strided `ddot`, `dgemv_n` tails, `idamax`, row swap, reciprocal-pivot `dscal`, the `DBL_MIN` test) | `interface/lapack/gesv.c`, `lapack/getrf/getrf_single.c`, `lapack/getf2/getf2_k.c`, `lapack/getrs/getrs_single.c`, `lapack/laswp/generic/laswp_k_4.c`, `driver/level3/trsm_L.c`, `kernel/generic/trsm_kernel_LT.c`, `trsm_kernel_LN.c`, `trsm_ltcopy_4.c`, `trsm_utcopy_4.c`, `kernel/x86_64/ddot.c`, `dgemv_n_4.c`, `dscal.c`, `param.h` (Haswell `DGEMM_UNROLL_N` = 8) | BSD-3-Clause |
| `openblas::iamax` (lane order of `maxpd`, `comisd` matching NaN, the clamp in `getf2_k`) | `kernel/x86_64/iamax_sse2.S` | BSD-3-Clause |

The Python side (`python/larmorx/transforms/chain.py`, `resample.py`) ports nitransforms'
ITK readers (`io/itk.py`: `ITKLinearTransform`, `ITKLinearTransformArray`,
`ITKCompositeH5`), `linear.py` (`Affine`, `LinearTransformsMapping`, `load`), `manip.py`
(`TransformChain.from_filename` for `.h5`), fMRIPrep's `ResampleSeries._run_interface`,
`resample_image` and `utils/transforms.py` (`load_transforms`), sdcflows'
`ensure_positive_cosines` (Apache-2.0) and nibabel's orientation helpers
(`orientations.py`: `io_orientation`, `ornt_transform`, `ornt2axcodes`, `inv_ornt_aff`,
`apply_orientation`; `affines.from_matvec`).

**fMRIPrep's field-map steps** (field maps part B, relay 7; Python, no new Rust):
`python/larmorx/transforms/fieldmap.py` ports fMRIPrep 25.2.7's `interfaces/resampling.py`
(Apache-2.0; the same file in 25.2.6): `ReconstructFieldmap._run_interface`,
`reconstruct_fieldmap`, `as_affine`, `aligned` and `DistortionParameters._run_interface`, with
nitransforms 25.1.0's (MIT) `manip.TransformChain.asaffine`, `linear.Affine.__matmul__` and
`resampling.apply` (`base.ImageGrid.ndcoords`/`index`, `_as_homogeneous`), and sdcflows
2.17.1's `utils/tools.ensure_positive_cosines` (Apache-2.0; the B-spline sum is part A's
`larmorx_sdcflows::bspline`). `chain.py`'s `_itk_linear_text` ports nitransforms 25.1.0's
`io/itk.py` writer (`ITKLinearTransform.from_ras`, `__str__`, `to_string`), used by
`python/larmorx/niworkflows/nitransforms.py` (niworkflows 1.15.4's `interfaces/nitransforms.py`:
`ConcatenateXFMs`, `concatenate_xfms`, `XFM_FMT`; Apache-2.0, the same in 1.15.2).

**fMRIPrep's field-map nodes of a BOLD run** (field maps part B, relay 8; Python, no new Rust):
`python/larmorx/pipelines/larmorprepx/fieldmaps.py` ports fMRIPrep 25.2.7's (Apache-2.0; the same
in 25.2.6) wiring of them as plain functions: `workflows/bold/fit.py` (`init_bold_fit_wf`'s stage 3
and stage 4's field-map branch, `init_bold_native_wf`'s field-map nodes, `_select_ref`),
`workflows/bold/apply.py` (`init_bold_volumetric_resample_wf`'s field-map nodes, `_gen_inverses`),
`workflows/bold/outputs.py` (the `DerivativesDataSink` settings of `init_ds_registration_wf`,
`init_ds_boldref_wf`, `init_ds_boldmask_wf`), `utils/bids.py` (`dismiss_echo`'s default) and
`workflows/base.py` (the Jacobian rule, the missing-readout-time message), with niworkflows
1.15.4's `utils/connections.listify` and nipype 1.12.0's `Merge` (Apache-2.0).

## OpenBLAS's level 3 and `dgemv`, numpy's 2-D `matmul` (`openblas::{level3, level2, matmul}`; the confounds' part, relay 2a, 2026-11-11)

**Ported** from OpenBLAS 0.3.30 (the release, numpy 2.3.5's scipy-openblas64 0.3.30) and from
the commit numpy 2.5.4's wheels bundle, OpenBLAS **v0.3.34-106-g446c436e** (scipy-openblas64
0.3.34.106.0, `MacPython/openblas-libs`' `openblas_commit.txt`; `<workspace>/tags/
OpenBLAS-0.3.34-106-g446c436e`, codeload tarball SHA-256 `ddecf3872f66…`), the 0.3.34 release
read beside it (`tags/OpenBLAS-0.3.34`): the files below are the same in the three but for
cancellation hooks in `level3.c` and allocation checks in `gemm.c`/`syrk.c`/`gemv.c` (no
arithmetic). BSD-3-Clause; the notice is in `openblas.rs` and NOTICE. Which routine numpy calls,
and with which layout and transposition, was read in numpy 2.5.4 and 2.3.5 (BSD-3-Clause):
`numpy/_core/src/umath/matmul.c.src`, `numpy/_core/src/multiarray/arraytypes.c.src`
(`DOUBLE_dot`). The logic is re-expressed in Rust; no code was copied.

| larmorx | Upstream files | Licence |
|---|---|---|
| `openblas::level3::dgemm`, `dgemm_with`, `depth_blocks`, `GEMM_Q`, `GEMM_Q_ZEN4` (the interface's returns, `beta`, the depth blocks, the kernel's paths and their classes, `c = alpha·acc + c`) | `interface/gemm.c`, `driver/level3/level3.c` (`level3_thread.c` read for its other depth rule), `kernel/x86_64/dgemm_kernel_4x8_haswell.S` (every path: `INIT*`, `KERNEL*_I/M1/M2/E/SUB`, `KERNEL4x1`, `SAVE*`), `dgemm_beta_skylakex.c`, `KERNEL.HASWELL`, `param.h` (`HASWELL`), `kernel/setparam-ref.c` (the Haswell table; the AMD Zen 4/5 block of 0.3.31 and later), `driver/others/parameter.c`, `common_param.h`, `common_x86_64.h` (`BUFFER_SIZE`) | BSD-3-Clause |
| `openblas::level3::dsyrk_lower`, `dsyrk_lower_with`, `syrk_depth_blocks` (lower triangle, both `TRANS`) | `interface/syrk.c`, `driver/level3/syrk_k.c` (`syrk_beta`), `level3_syrk.c` (`LOWER`), `syrk_kernel.c` | BSD-3-Clause |
| `openblas::level2::dgemv` (`N`, `T`; `alpha`, `beta`, positive increments), `ddot` (strided) | `interface/gemv.c`, `kernel/x86_64/dgemv_n_4.c`, `dgemv_n_microk_haswell-4.c`, `dgemv_t_4.c`, `dgemv_t_microk_haswell-4.c`, `dscal.c`, `ddot.c` | BSD-3-Clause |
| `openblas::matmul::numpy_matmul`, `numpy_matmul_with`, `syrk_case`, `numpy_syrk`, `numpy_syrk_with` | numpy 2.5.4 and 2.3.5 `matmul.c.src` (`DOUBLE_matmul`, `is_blasable2d`, `DOUBLE_gemv`, `DOUBLE_matmul_matrixmatrix`, `DOUBLE_matmul_inner_noblas`, `DOUBLE_matrix_copy`), `arraytypes.c.src` (`DOUBLE_dot`); OpenBLAS's CBLAS paths of `interface/gemm.c`, `gemv.c`, `syrk.c` | BSD-3-Clause |

Validated bit for bit against numpy's own library and `@`/`np.dot` in both kept environments
(`validation/investigations/confounds/check_blas.py`, 2026-11-11: numpy 2.3.5 4,503 of 4,503;
numpy 2.5.4 every defined case with `GEMM_Q` 512 on the AMD Zen 4 main machine, where that
library sets it and crashes on the cases its Haswell kernel's stack buffer cannot hold).

**Relay 2a-ii** (`openblas::{trmm, level1}`, `level2::{dger, dtrmv}`; the same three OpenBLAS
trees, whose files below are identical but for `volatile` in three of the TRMM copies and the
interfaces' threading, allocation and export lines):

| larmorx | Upstream files | Licence |
|---|---|---|
| `openblas::trmm::dtrmm`, `dtrmm_with`, `Side`, `Uplo`, `Diag`, `TRMM_RIGHT_MAX_N` (the interface's checks and returns, `alpha` first, the drivers' block orders and kernel calls, the TRMM kernel's groups, step ranges and paths, the copies' stored zeros and unit diagonal) | `interface/trsm.c` (compiled as `TRMM`), `driver/level3/trmm_L.c`, `trmm_R.c`, `kernel/x86_64/dtrmm_kernel_4x8_haswell.c`, `kernel/generic/trmm_uncopy_4.c`, `trmm_lncopy_4.c`, `trmm_utcopy_4.c`, `trmm_ltcopy_4.c`, `trmm_uncopy_8.c`, `trmm_lncopy_8.c`, `trmm_utcopy_8.c`, `trmm_ltcopy_8.c`, `kernel/Makefile.L3`, `KERNEL.HASWELL` (the C paths' arithmetic checked in the disassembly of both wheels' `dtrmm_kernel_*_HASWELL`: no fused multiply-adds) | BSD-3-Clause |
| `openblas::level2::dger`, `dtrmv` (`DTB_ENTRIES` 64; `NU`/`TL` from `trmv_U.c`, `NL`/`TU` from `trmv_L.c`) | `interface/ger.c`, `kernel/generic/ger.c`, `interface/trmv.c`, `driver/level2/trmv_U.c`, `trmv_L.c`, `driver/level2/Makefile`, `common.h` | BSD-3-Clause |
| `openblas::level1::axpy_k`, `daxpy`, `dscal`, `dcopy`, `dswap`, `idamax` (`iamax_sse2.S`'s lanes, reductions and searches, both alignments), `dnrm2` (the interface around `numpy_linalg`'s x87 kernel), `at` | `interface/axpy.c`, `scal.c`, `copy.c`, `swap.c`, `imax.c`, `nrm2.c`, `kernel/x86_64/daxpy.c`, `daxpy_microk_haswell-2.c`, `dscal.c`, `dscal_microk_haswell-2.c`, `iamax_sse2.S`, `nrm2.S`, `KERNEL`, `KERNEL.HASWELL` | BSD-3-Clause |

Validated bit for bit against numpy's own library through its Fortran entry points in both kept
environments (`check_blas.py`'s `trmm`, `ger`, `trmv`, `level1` sections, 2026-11-11).

**Relay 2b-i** (`numpy_linalg::blocked`: LAPACK's blocked QR and LQ; `numpy_linalg::dlarfg` made
public). Ported from the two `lapack-netlib/SRC/` trees above (OpenBLAS 0.3.30: LAPACK 3.12.0;
v0.3.34-106-g446c436e: LAPACK 3.12.1), whose `.f` files gfortran compiles into the wheels; the
routines that differ between the trees are ported in both forms (`LapackRelease`):

| larmorx | Upstream files | Licence |
|---|---|---|
| `numpy_linalg::blocked::dgeqrf`, `dgelqf`, `lwork_opt`, `NB`, `NX` (the argument checks, the workspace query, `NB` lowered by a short `LWORK`, the panels while more than `NX` columns or rows remain, the unblocked rest; `DGELQF`'s panel copied for `DLARFB`) | `dgeqrf.f`, `dgelqf.f` (identical in both trees), `ilaenv.f` (`NB` 32, `NBMIN` 2, `NX` 128), `xerbla.f` | BSD-3-Clause |
| `numpy_linalg::blocked::dgeqr2`, `dgelq2` (3.12.0: the reflector's 1 stored for `DLARF`; 3.12.1: `DLARF1F`) | `dgeqr2.f`, `dgelq2.f` (both trees), `dlarfg.f` | BSD-3-Clause |
| `numpy_linalg::blocked::dlarft`, `DLARFT_LVL2_BELOW`, `Direct`, `Storev` (the recursive algorithm in its four forms; 3.12.1's crossover at `ILAENV(3, 'DLARFT') = 64` to `DLARFT_LVL2`, forward and backward, columnwise and rowwise, its scans for the vectors' last nonzero entries) | `dlarft.f` (both trees), `dlarft_lvl2.f` (0.3.34), `dlacpy.f`, `ilaenv.f` (0.3.34's `'LA'`/`'RFT'` entry) | BSD-3-Clause |
| `numpy_linalg::blocked::dlarfb` (every side, transpose, direction and storage) | `dlarfb.f` (identical in both trees) | BSD-3-Clause |
| `numpy_linalg::blocked::dlarf`, `dlarf1f` (any length, positive increments: the scans `ILADLC`/`ILADLR`, then OpenBLAS's `dgemv`, `daxpy`, `dscal`, `dger` ports) | `dlarf.f` (both trees: the same arithmetic), `dlarf1f.f` (0.3.34), `iladlc.f`, `iladlr.f` | BSD-3-Clause |
| `numpy_linalg::blocked::Lapack`, `IllegalValue` | (larmorx's: the release and the BLAS depth block a call follows; `XERBLA`'s message) | Apache-2.0 |

Validated bit for bit against numpy's own LAPACK through its exported Fortran entry points in
both kept environments (`validation/investigations/confounds/check_lapack.py`, full tier,
2026-11-11, commit `6d88f10e`): numpy 2.3.5 (LAPACK 3.12.0) every case identical (`DLARFG`
1,500, `DLARF` 3,000, `DGEQR2`/`DGELQ2` 132 each, `DLARFT` 2,006, `DLARFB` 2,506,
`DGEQRF`/`DGELQF` 209 each, up to 600 × 60,968 and 60,968 × 600); numpy 2.5.4 (LAPACK 3.12.1,
whose `DLARF1F` and `DLARFT_LVL2` are proved through `DGEQR2`/`DGELQ2`/`DLARFT`) the same counts
identical with `OPENBLAS_L2_SIZE=512` (the Haswell table's `GEMM_Q` 256 on the AMD Zen 4 machine),
and with the machine's own `GEMM_Q` 512 every defined case identical, the crashes of that library
all in its undefined class.

**Relays 2b-ii and 2b-iii** (`numpy_linalg::blocked`: the generation and application of `Q`, the
bidiagonal reduction). Ported from the same two `lapack-netlib/SRC/` trees, each routine that
differs between them in both forms (`LapackRelease`):

| larmorx | Upstream files | Licence |
|---|---|---|
| `numpy_linalg::blocked::dorgqr`, `dorglq`, `org_lwork_opt` (the checks, the query, `NB` lowered by a short `LWORK`, the zeros above the blocked part, the unblocked trailing part first, then the 32-reflector blocks from the last back: `DLARFT` with `T` in the workspace at leading dimension `LDWORK`, `DLARFB` from the left (`DORGQR`) or the right with `Hᵀ` (`DORGLQ`, the block's reflectors copied out for it), the block's own columns or rows) | `dorgqr.f`, `dorglq.f` (identical in both trees), `ilaenv.f` (`NB` 32, `NBMIN` 2, `NX` 128) | BSD-3-Clause |
| `numpy_linalg::blocked::dorg2r`, `dorgl2` (3.12.0: the reflector's 1 stored for `DLARF`; 3.12.1: `DLARF1F`; `DSCAL` by `-tau`, `1 - tau` on the diagonal) | `dorg2r.f`, `dorgl2.f` (both trees) | BSD-3-Clause |
| `numpy_linalg::blocked::dormqr`, `dormlq`, `orm_lwork_opt`, `NBMAX_ORM`, `LDT_ORM`, `TSIZE_ORM` (every side and transpose; `T` at leading dimension 65; the optimal workspace and the short-workspace `NB` of each release: 3.12.0 `NW·NB + 65·64`, `(LWORK - 65·64)/NW`; 3.12.1 `NW·NB + 65·NB`, `LWORK/(NW + 65)`; `DORMLQ`'s block reflector transposed) | `dormqr.f`, `dormlq.f` (both trees) | BSD-3-Clause |
| `numpy_linalg::blocked::dorm2r`, `dorml2` (every side and transpose; 3.12.0 stores each reflector's 1 in `A` for `DLARF` and restores the entry, 3.12.1 `DLARF1F`) | `dorm2r.f`, `dorml2.f` (both trees) | BSD-3-Clause |
| `numpy_linalg::blocked::dgebrd`, `gebrd_lwork_opt` (the checks, the query, `NX = max(NB, 128)`, a short workspace's `NB = LWORK/(M + N)` or the unblocked reduction below `2·(M + N)`, the panels by `DLABRD` with `X` and `Y` at leading dimensions `M` and `N`, the two `DGEMM` updates of depth `NB` (the rows above the second one's output copied out for it), `d` and `e` written back, `DGEBD2` on the rest) | `dgebrd.f` (identical in both trees), `ilaenv.f` (`'BRD'`: `NB` 32, `NBMIN` 2, `NX` 128) | BSD-3-Clause |
| `numpy_linalg::blocked::dlabrd` (both bidiagonal forms; every `DGEMV` with its own leading dimension and unit or non-unit increments, the vectors that share an array with the matrix passed through buffers) | `dlabrd.f` (identical in both trees) | BSD-3-Clause |
| `numpy_linalg::blocked::dgebd2` (both bidiagonal forms; 3.12.0 `DLARF` with the stored 1 and `d`/`e` put back, 3.12.1 `DLARF1F`; the last `taup` or `tauq` 0) | `dgebd2.f` (both trees) | BSD-3-Clause |

Validated bit for bit against numpy's own LAPACK through its exported Fortran entry points in both
kept environments (`validation/investigations/confounds/check_lapack.py`, sections `org`, `orm`,
`brd`, full tier, 2026-11-11): see `validation/investigations/confounds/README.md` for the counts
and commits; `control_lapack_release.py` shows that the cases tell the two releases apart.

**Relay 2c-i** (`numpy_linalg::bdsdc`: LAPACK's divide-and-conquer bidiagonal SVD with the
singular vectors). Ported from the same two `lapack-netlib/SRC/` trees (OpenBLAS 0.3.30, LAPACK
3.12.0, numpy 2.3.5; OpenBLAS v0.3.34-106-g446c436e, LAPACK 3.12.1, numpy 2.5.4), each routine
that differs between them in both forms (`LapackRelease`):

| larmorx | Upstream files | Licence |
|---|---|---|
| `numpy_linalg::bdsdc::dbdsdc`, `SMLSIZ` (`COMPQ = 'I'`, both bidiagonal forms: the checks, a lower matrix rotated to upper with the rotations kept for `U`, `DLASDQ` up to 25 rows, else the scaling by `DLANST`, the tiny entries set to `±0.9ε`, the subproblems split at `|e(i)| < 0.9ε` (the 1 × 1 one at `d(n)`), `DLASD0` on each, the descending selection sort, the rotations applied to `U`) | `dbdsdc.f` (both trees: layout only), `ilaenv.f` (`ILAENV(9, 'DBDSDC')` = 25), `dlaset.f`, `dlanst.f`, `INSTALL/dlamch.f` | BSD-3-Clause |
| `numpy_linalg::bdsdc::dlasd0`, `dlasdt`, `Tree` (the tree with `INT(LOG(n/(msub + 1))/LOG(2)) + 1` levels through glibc's `log`, `larmorx_core::math::log_glibc`; the leaves by `DLASDQ`, the merges bottom-up by `DLASD1`) | `dlasd0.f`, `dlasdt.f` (identical in both trees) | BSD-3-Clause |
| `numpy_linalg::bdsdc::dlasd1`, `dlasd3`, `Deflation`, `MergeWork` (the scaling, `DLASD2`, `DLASD3`'s secular equation and its four `DGEMM` groups on the column types' blocks, the unscaling, `DLAMRG`'s permutation) | `dlasd1.f`, `dlasd3.f` (identical in both trees), `dlacpy.f` | BSD-3-Clause |
| `numpy_linalg::bdsdc::dlasd2` (3.12.0: `TOL = 8ε·max(…)`, `|d(j) - d(jprev)| <= TOL`, `IDXP(K2) = JPREV`; 3.12.1: `64ε`, the signed difference, `IDXP(K2 + J - 1 - JP) = JP` for `JP = JPREV … J - 1`; `DROT`, `DLAPY2`, the column types, `COLTYP(1:4) = CTOT`) | `dlasd2.f` (both trees) | BSD-3-Clause |
| `numpy_linalg::bdsdc::dlasd4`, `dlasd5`, `dlaed6` (the secular equation's roots: the last one and the inner ones, the two-pole and three-pole steps, `DLAED6`'s fallback, the geometric-mean bound; `DLAED6`'s scaling by `2^-340` and `2^-680`) | `dlasd4.f`, `dlasd5.f`, `dlaed6.f` (identical in both trees) | BSD-3-Clause |
| `numpy_linalg::bdsdc::dlasdq` (both forms, `SQRE` 0 and 1, the rotations to upper form on `VT`, `U` and `C`, `DBDSQR` (the parent module's port, each release's `-0`), the ascending sort) | `dlasdq.f` (identical in both trees), `dbdsqr.f` (both trees) | BSD-3-Clause |
| `numpy_linalg::bdsdc::dlamrg`, `dlascl_g` (`TYPE = 'G'`: the checks, the safe steps) | `dlamrg.f` (identical), `dlascl.f` (both trees; the same arithmetic for `'G'`) | BSD-3-Clause |
| `numpy_linalg::blocked::dormbr`, `dorgbr`, `Vect`, `ormbr_lwork_opt`, `orgbr_lwork_opt` (`DGEBRD`'s `Q` and `P`: `DORMQR`/`DORMLQ` (the transpose flipped for `P`) on all the reflectors or on the `nq - 1` shifted ones, `DORMBR`'s own optimal workspace `NW·32` handed on as it is (short for `DORMQR`/`DORMLQ`: each release's `NB` rule then applies); `DORGQR`/`DORGLQ` or the shifted forms with the identity's first row and column; the checks, `WORK(1)`) | `dormbr.f`, `dorgbr.f` (identical in both trees) | BSD-3-Clause |

The BLAS calls are the ported Haswell kernels (`openblas::level3::dgemm_with` for `DLASD3`,
`level1::dnrm2` and `dcopy`, the parent module's `drot` and `dswap`). Validated bit for bit
against numpy's own LAPACK through its exported Fortran entry points in both kept environments
(`validation/investigations/confounds/check_svd.py`, every routine through its own entry point;
`DORMBR`/`DORGBR`: `check_lapack.py`'s section `orbr`, 2026-11-11): see `validation/investigations/confounds/README.md` for the counts and commits; its
`--control` shows that the cases tell the two releases apart.

**Relay 2c-ii** (`numpy_linalg::svd`: the thin SVD and the pseudo-inverse of a general matrix).
`dgesdd.f` is identical in both `lapack-netlib/SRC/` trees; the routines it calls are the ports
above, each in its release's form (`Lapack`):

| larmorx | Upstream files | Licence |
|---|---|---|
| `numpy_linalg::svd::dgesdd_s`, `Gesdd`, `DgesddError` (`JOBZ = 'S'`: the checks of `LDA`, `LDU`, `LDVT`, `LWORK`; `DLANGE('M')`, `INFO = -4` on a NaN, the scaling by `DLASCL` to `[SMLNUM, BIGNUM]` and its undoing on `S`; `MNTHR = ⌊min(m, n)·11/6⌋`; path 3 (`DGEQRF`, `R` copied with zeros below, `DORGQR`, `DGEBRD` of `R`, `DBDSDC('U', 'I')`, `DORMBR('Q', 'L', 'N')`, `DORMBR('P', 'R', 'T')`, `U` copied out, `DGEMM`), path 5 (`DGEBRD`, `DLASET`, `DBDSDC('U', 'I')`, the two `DORMBR`), path 3t (`DGELQF`, `L` copied with zeros above, `DORGLQ`, `DGEBRD` of `L`, `DBDSDC('U', 'I')`, the two `DORMBR`, `VT` copied out, `DGEMM`), path 5t (`DGEBRD`, `DLASET`, `DBDSDC('L', 'I')`, the two `DORMBR`); every callee given `LWORK - NWORK + 1`; an infinite entry refused, where LAPACK would scale it to NaN and loop or return garbage) | `dgesdd.f` (identical in both trees), `dlange.f`, `dlascl.f`, `dlaset.f`, `dlacpy.f`, `la_xisnan.F90`, `droundup_lwork.f`, `xerbla.f`, `INSTALL/dlamch.f` | BSD-3-Clause |
| `numpy_linalg::svd::dgesdd_s_lwork` (`DGESDD('S')`'s query: `MINWRK` and `MAXWRK` from the callees' queries `DGEQRF`, `DORGQR`, `DGELQF`, `DORGLQ`, `DGEBRD`, `DORMBR` and `BDSPAC = 3·k² + 4·k`, the same in both releases) | `dgesdd.f`, and the queries of the routines above | BSD-3-Clause |
| `numpy_linalg::svd::numpy_svd`, `Svd`, `SvdDidNotConverge` (`svd(a, full_matrices=False)`: the copy into Fortran order with `LDA = LDU = max(m, 1)`, `LDVT = max(1, min(m, n))`, numpy 2.5.4's `all_finite` refusal (also for 2.3.5's NaN, which LAPACK refuses with `INFO = -4`, and its infinity, which LAPACK does not survive), the optimal workspace, `LinAlgError('SVD did not converge')` on `INFO != 0`, the copies back in C order) | numpy 2.5.4 and 2.3.5 `numpy/linalg/umath_linalg.cpp` (`svd_wrapper`, `init_gesdd`, `call_gesdd`, `compute_urows_vtcolumns`, `all_finite` (2.5.4 only), `linearize_matrix`, `delinearize_matrix`), `numpy/linalg/_linalg.py` (`svd`, `_raise_linalgerror_svd_nonconvergence`) | BSD-3-Clause |
| `numpy_linalg::svd::numpy_pinv` (`pinv(a, rcond)`: an empty matrix's empty result, `cutoff = rcond·max(s)`, `1/s` where `s > cutoff` and 0 elsewhere, `s[:, None]·u.T` laid out in F order as numpy's multiply does, `matmul(vt.T, ·)` by `openblas::matmul::numpy_matmul_with`) | numpy 2.5.4 and 2.3.5 `numpy/linalg/_linalg.py` (`pinv`, identical in both) | BSD-3-Clause |

Validated bit for bit against numpy's own `scipy_dgesdd_64_` and its `np.linalg.svd` and
`np.linalg.pinv` in both kept environments (`validation/investigations/confounds/check_gesdd.py`,
2026-11-11): see `validation/investigations/confounds/README.md` for the counts and commits.

**Relay 2d-i** (`numpy_linalg::gelsd`: the minimum-norm least squares of a general matrix and
numpy's `lstsq`). `dgelsd.f`, `dlalsd.f` and `dlasrt.f` are identical in both `lapack-netlib/SRC/`
trees; the routines they call are the ports above, each in its release's form (`Lapack`):

| larmorx | Upstream files | Licence |
|---|---|---|
| `numpy_linalg::gelsd::dgelsd`, `Gelsd`, `DgelsdError` (the checks of `LDA`, `LDB`, `LWORK`; the quick return; `DLANGE('M')` of `A` and `B` and their scalings by `DLASCL` to `[SMLNUM, BIGNUM]` (`SMLNUM = DLAMCH('S')/DLAMCH('P')`) and the undoing on `B` and `S`; an all-zero `A`; `B`'s rows past `M` cleared for `M < N`; path 1a (`DGEQRF`, `DORMQR('L', 'T')`, zeros below `R`), path 1 (`DGEBRD`, `DORMBR('Q', 'L', 'T')`, `DLALSD('U')`, `DORMBR('P', 'L', 'N')`), path 2a (`DGELQF`, `L` copied with zeros above at `LDWORK = M` or `LDA` by the workspace's rule, `DGEBRD` of `L`, the two `DORMBR`, `DLALSD('U')`, `B`'s rows past `M` cleared, `DORMLQ('L', 'T')`), path 2 (`DGEBRD` lower, `DORMBR('Q')`, `DLALSD('L')`, `DORMBR('P')`); every callee given `LWORK - NWORK + 1`; `INFO` from `DLALSD` returned as LAPACK does; a non-finite `A` refused, where LAPACK scales an infinity to NaN and may loop) | `dgelsd.f` (identical in both trees), `dlange.f`, `dlascl.f`, `dlaset.f`, `dlacpy.f`, `la_xisnan.F90`, `xerbla.f`, `INSTALL/dlamch.f` | BSD-3-Clause |
| `numpy_linalg::gelsd::dgelsd_lwork`, `gelsd_mnthr`, `gelsd_nlvl` (`DGELSD`'s query: `MAXWRK`, `MINWRK`, `LIWORK`, `WLALSD` with `ILAENV(1, ...) = 32` for `DGEQRF`, `DORMQR`, `DGEBRD`, `DORMBR`, `DGELQF`, `DORMLQ`, `ILAENV(6, 'DGELSD') = INT(REAL(MIN(M, N))·1.6E0)` in single precision, `ILAENV(9) = 25`, `NLVL` through glibc's `log` as `larmorx_core::math::log_glibc`, the same in both releases) | `dgelsd.f`, `ilaenv.f` (ISPEC 1, 6 and 9: the same in both trees) | BSD-3-Clause |
| `numpy_linalg::gelsd::dlalsd` (the checks of `NRHS` and `LDB` as `INFO` (XERBLA's refusals), `RCND`, `N = 0` and `N = 1`, the lower bidiagonal rotated to upper by `DLARTG` with `DROT` on `B`'s rows (both orders of the original: one column's operations the same), `DLANST`, a zero bidiagonal, the scaling by `ORGNRM`; `N <= SMLSIZ`: `VT = I`, `DLASDQ('U', 0, N, N, 0, NRHS)` with `C = B`, `TOL = RCND·|D(IDAMAX)|`, the rows cleared or scaled by `DLASCL`, the rank, `DGEMM('T', 'N')` and `DLACPY`, the unscalings, `DLASRT('D')`; the divide-and-conquer path for `N > SMLSIZ`: relay 2d-ii-b below) | `dlalsd.f` (identical in both trees), `dlasdq.f`, `dlartg.f90`, `dlanst.f`, `dlascl.f`, `dlaset.f`, `dlacpy.f`; OpenBLAS `interface/rot.c`, `kernel/x86_64/drot.c`, `kernel/x86_64/iamax_sse2.S`, `interface/gemm.c` with the level-3 driver above | BSD-3-Clause |
| `numpy_linalg::gelsd::dlasrt`, `Sort` (`DLASRT('I' \| 'D')`: insertion sort on stretches of at most 20 steps, quicksort with the median of three, the stack's order) | `dlasrt.f` (identical in both trees) | BSD-3-Clause |
| `numpy_linalg::gelsd::numpy_lstsq` (`lstsq(a, b, rcond)`: `rcond = None` as `eps·max(m, n)`, the copies into Fortran order with `LDA = max(1, m)`, `LDB = max(1, m, n)`, no right-hand side as one zero column, numpy 2.5.4's `all_finite` refusal of `a` (for every release: NP-10), the optimal workspace, `LinAlgError` on `INFO != 0`, `x` from `B`'s first rows (0 for `m = 0`), the residuals by `abs2` at stride `m` kept only when the rank is `n` and `m > n`) | numpy 2.5.4 and 2.3.5 `numpy/linalg/umath_linalg.cpp` (`lstsq`, `init_gelsd`, `call_gelsd`, `release_gelsd`, `abs2`, `all_finite` (2.5.4 only), `linearize_matrix`, `delinearize_matrix`), `numpy/linalg/_linalg.py` (`lstsq`, `_raise_linalgerror_lstsq`, identical in both) | BSD-3-Clause |

Validated bit for bit against numpy's own `scipy_dgelsd_64_`, `scipy_dlalsd_64_`,
`scipy_dlasrt_64_` and its `np.linalg.lstsq` in both kept environments
(`validation/investigations/confounds/check_gelsd.py`, 2026-11-11): see
`validation/investigations/confounds/README.md` for the counts and commits.

**Relay 2d-ii-a** (`numpy_linalg::lasda`: one merge of `DLASDA`'s tree, for `DLALSD`'s
divide-and-conquer path). `dlasd6.f` and `dlasd8.f` are identical in both `lapack-netlib/SRC/`
trees; `dlasd7.f` differs as `dlasd2.f` does (the deflation test and the record of the deflated
positions):

| larmorx | Upstream files | Licence |
|---|---|---|
| `numpy_linalg::lasda::dlasd7`, `Lasd7`, `GivensRecord` (the checks; `Z` from `ALPHA·VL` and `BETA·VF`, the first half moved one position back and `IDXQ` shifted; `DSIGMA`/`ZW`/`VFW`/`VLW` as storage and the merge by `DLAMRG`; `TOL = 64·ε·max(|ALPHA|, |BETA|, |D(N)|)`; the deflation of small `Z` entries and of close values (3.12.0 `|D(j) - D(jprev)| <= TOL` and `IDXP(K2) = JPREV`; 3.12.1 the signed difference and the deflated positions recorded again in order) with `DLAPY2`, the Givens rotation on `VF` and `VL` by `DROT` and its record in `GIVCOL`/`GIVNUM` with `PERM` (`ICOMPQ = 1`); `DSIGMA(2)` raised to `TOL/2`; for `SQRE = 1` the extra row rotated into the first; `C` and `S` as LAPACK leaves them) | `dlasd7.f` (both trees), `dlamrg.f`, `dlapy2.f`, `INSTALL/dlamch.f`; OpenBLAS `interface/rot.c`, `kernel/x86_64/drot.c` | BSD-3-Clause |
| `numpy_linalg::lasda::dlasd8` (the checks; `K = 1`; `Z` normalised by `DNRM2` and `DLASCL`, `RHO = ρ²`; `DLASD4` for each root with `DELTA` and its work in `WORK`, the product formula for the new `Z` in LAPACK's order of operations, `DIFL`, `DIFR(·, 1)` (its last entry as LAPACK leaves it), `DIFR(·, 2)` with `ICOMPQ = 1`; the new `VF`/`VL` through `DDOT` (the Haswell kernel) over `DNRM2`, `DLAMC3` as a stored sum) | `dlasd8.f` (identical in both trees), `dlasd4.f`, `dlascl.f`, `dlaset.f`; OpenBLAS `interface/dot.c`, `nrm2.c`, `kernel/x86_64/ddot.c`, `ddot_microk_haswell-2.c`, `nrm2.S` | BSD-3-Clause |
| `numpy_linalg::lasda::dlasd6`, `Lasd6`, `MergeFactors` (the checks; `D(NL+1)` cleared, `ORGNRM` and the scaling of `D`, `ALPHA`, `BETA`; `DLASD7`, `DLASD8`; the poles with `ICOMPQ = 1`; the unscaling; `IDXQ` by `DLAMRG(K, N - K, D, 1, -1)`) | `dlasd6.f` (identical in both trees), `dlascl.f`, `dlamrg.f`, `dcopy` | BSD-3-Clause |

Validated bit for bit against numpy's own `scipy_dlasd7_64_`, `scipy_dlasd8_64_` and
`scipy_dlasd6_64_` in both kept environments (`validation/investigations/confounds/check_gelsd.py`
sections `lasd7`, `lasd8`, `lasd6`, 2026-11-11): see `validation/investigations/confounds/README.md`.

**Relay 2d-ii-b** (`numpy_linalg::lasda` and `gelsd`: the rest of `DLALSD`'s divide-and-conquer
path, so `DGELSD` and numpy's `lstsq` of any size). `dlasda.f`, `dlals0.f`, `dlalsa.f` and
`dlalsd.f` are identical in both `lapack-netlib/SRC/` trees; the release enters through `DLASDQ`'s
`DBDSQR` and `DLASD7`:

| larmorx | Upstream files | Licence |
|---|---|---|
| `numpy_linalg::lasda::dlasda`, `SvdTree`, `LasdaError` (the checks; `N <= SMLSIZ`: one `DLASDQ` on `D`, `E` and, with `ICOMPQ = 1`, `VT` and `U` as given; `DLASDT`'s tree; the leaves by `DLASDQ` from the identity (`ICOMPQ = 1`: `U` and `VT` in place at the leaf's rows; `ICOMPQ = 0`: `VT` in a work array of leading dimension `SMLSIZ + 1`), their first and last right-vector components into `VF` and `VL`, `IDXQ` per leaf; the merges level by level from the bottom by `DLASD6` with `ALPHA = D(IC)`, `BETA = E(IC)`, each node's arrays at its rows and its level's columns and its `K`, `GIVPTR`, `C`, `S` at the slot counted down from `2^NLVL - 1` (`ICOMPQ = 1`), all at the arrays' starts (`ICOMPQ = 0`); `INFO` from a leaf or a merge; `ICOMPQ = 0` with `2 <= N <= SMLSIZ` refused: LAPACK's `DLASDQ` without vectors goes through `DBDSQR`'s `DLASQ1`, the dqds algorithm, which is not ported) | `dlasda.f` (identical in both trees), `dlasdq.f`, `dlasdt.f`, `dlaset.f`, `dlasd6.f`; OpenBLAS `interface/copy.c` | BSD-3-Clause |
| `numpy_linalg::lasda::dlals0` (the checks; `ICOMPQ = 0`: the Givens rotations by `DROT`, `PERM` into `BX` by `DCOPY`, `K = 1` (copied, `DSCAL` by -1 for a negative `Z(1)`), each of the `K` rows from the secular equation's vector (`POLES`, `DIFL`, `DIFR`, `Z` with `DLAMC3` as a stored sum, `WORK(1) = -1`) by `DNRM2`, `DGEMV('T')` and `DLASCL`, the deflated rows by `DLACPY`, `INFO` the last `DLASCL`'s; `ICOMPQ = 1`: the transposed vectors by `DGEMV('T')`, the extra row's rotation by `C`, `S` for `SQRE = 1`, the deflated rows, back through `PERM`, the Givens rotations undone in reverse) | `dlals0.f` (identical in both trees), `dlascl.f`, `dlacpy.f`, `dlamc3` in `INSTALL/dlamch.f`; OpenBLAS `interface/rot.c`, `copy.c`, `scal.c`, `gemv.c`, `nrm2.c`, `kernel/x86_64/drot.c`, `dscal.c`, `dgemv_t_4.c`, `dgemv_t_microk_haswell-4.c`, `nrm2.S` | BSD-3-Clause |
| `numpy_linalg::lasda::dlalsa` (the checks; `DLASDT`'s tree; `ICOMPQ = 0`: each leaf's `Uᵀ·B` by `DGEMM('T', 'N')`, the centre rows copied, `DLALS0` bottom-up with `SQRE = 0`, the slots counted down; `ICOMPQ = 1`: `DLALS0` top-down (the last node of each level square, the others with their extra row), the slots counted up, then each leaf's `VTᵀ·B` (the last leaf's right child without its extra row)) | `dlalsa.f` (identical in both trees), `dlasdt.f`, `dlals0.f`; OpenBLAS `interface/gemm.c` with the level-3 driver and the Haswell kernel above (depth at most 26), `interface/copy.c` | BSD-3-Clause |
| `numpy_linalg::gelsd::dlalsd`'s path for `N > SMLSIZ` (`NLVL` through glibc's `log`; the workspace of `dlalsd.f` (`U`, `VT`, `DIFL`, `DIFR`, `Z`, `C`, `S`, `POLES`, `GIVNUM`, `BX`; `IWORK`'s `K`, `GIVPTR`, `PERM`, `GIVCOL`), each piece's at its first row with leading dimension `N`; `|D(i)| < EPS` set to `SIGN(EPS, D(i))`; the pieces at every `|E(i)| < EPS` and at the last row (its three cases, the last row a piece of its own when `|E(N-1)| < EPS`); one row copied, at most `SMLSIZ` rows by `DLASET` + `DLASDQ` with `C = B` + `DLACPY`, more by `DLASDA(ICOMPQ = 1)` and `DLALSA(ICOMPQ = 0)`; `TOL = RCND·|D(IDAMAX)|`, each row of `BX` cleared (`|D(i)| <= TOL`) or scaled by `DLASCL` by the signed `D(i)`, the rank, `D(i) = |D(i)|`; the pieces back by `DCOPY`, `DGEMM('T', 'N')` or `DLALSA(ICOMPQ = 1)`; the returns on a nonzero `INFO` (a refusal of `DLASDA` for `SMLSIZ < 3` as its negative `INFO`); the unscalings and `DLASRT('D')`) | `dlalsd.f` (identical in both trees), `dlasda.f`, `dlalsa.f`, `dlasdq.f`, `dlaset.f`, `dlacpy.f`, `dlascl.f`, `dlasrt.f`; OpenBLAS `interface/copy.c`, `imax.c` with `kernel/x86_64/iamax_sse2.S`, `interface/gemm.c` | BSD-3-Clause |

Validated bit for bit against numpy's own `scipy_dlasda_64_`, `scipy_dlals0_64_`,
`scipy_dlalsa_64_`, `scipy_dlalsd_64_`, `scipy_dgelsd_64_` and its `np.linalg.lstsq` in both kept
environments (`validation/investigations/confounds/check_gelsd.py`, sections `lasda`, `lals0`,
`lalsa` and the divide-and-conquer cases of `lalsd`, `gelsd`, `lstsq`, 2026-11-11): see
`validation/investigations/confounds/README.md`.

**Relay 2d-iii-a** (`numpy_linalg::gesvd`: SciPy's `linalg.svd(a, lapack_driver='gesvd')`, the
SVD nipype's `fallback_svd` falls back to, with SciPy's own LAPACK). SciPy 1.18.1's wheel bundles
scipy-openblas32 0.3.31.22.0 (OpenBLAS v0.3.31-22-g5ffbf38b, `MacPython/openblas-libs`'
`openblas_commit.txt`; `tags/OpenBLAS-0.3.31-22-g5ffbf38b`, codeload tarball SHA-256
`51257fb2f0aa7b4c23d9cf0302500b817dc82c0022ee681b24f9ab82dfd10312`), SciPy 1.15.3's
scipy-openblas32 0.3.28.0.2 (the v0.3.28 release its build checks out; `tags/OpenBLAS-0.3.28`,
release tarball SHA-256 `f1003466ad074e9b0c8d421a204121100b0751c96fc6fcf3d1456bd12f8a00a1`). Every
`lapack-netlib/SRC/` file `DGESVD` reaches is identical in both to OpenBLAS 0.3.30's but
`dlarft.f` of 0.3.28 (the column-by-column algorithm, before OpenBLAS 0.3.29's recursive one; the
same Fortran as 0.3.34's `dlarft_lvl2.f`); the BLAS files on these paths differ from 0.3.30's only
in threading heuristics and other architectures, and neither `kernel/setparam-ref.c` has 0.3.34's
`GEMM_Q = 512` rule for AMD Zen 4 and 5:

| larmorx | Upstream files | Licence |
|---|---|---|
| `numpy_linalg::blocked::DlarftForm`, the `dlarft` field of `Lapack`, `Lapack::with_gemm_q` (`ColumnByColumn`: every `DLARFT` call by the column-by-column algorithm, `dlarft_lvl2`) | OpenBLAS 0.3.28 `lapack-netlib/SRC/dlarft.f`; 0.3.34's `dlarft_lvl2.f` (identical but line breaks) | BSD-3-Clause |
| `numpy_linalg::gesvd::dgesvd`, `dgesvd_lwork`, `Job`, `Gesvd`, `DgesvdError` (`JOBU = JOBVT = 'S'` or `'A'`: the checks, the workspace query from the callees' queries and `ILAENV(6)` = `INT(REAL(MIN(M, N))·1.6E0)`, `BDSPAC = 5·MIN(M, N)`; the scaling by `DLASCL` outside `[SMLNUM, BIGNUM]`; paths 6 and 9 (`DGEQRF`; with `N·N + MAX(4N, BDSPAC)` (path 9: `MAX(N + M, 4N, BDSPAC)`) or more `R` into `WORK(IU)` at leading dimension `LDA` when `LWORK >= WRKBL + LDA·N`, else `N`, `DGEBRD`, `DORGBR` `'Q'` and `'P'`, `DBDSQR('U')`, `DGEMM`, path 9 through `A` and `DLACPY`; else `DORGQR` in `U`, `R` reduced in `VT`, `DORMBR('Q', 'R', 'N')`, `DORGBR('P')`, `DBDSQR('U')` on `U`); path 10 (`DGEBRD`, `DORGBR('Q')` of `N` or `M` columns, `DORGBR('P')`, `DBDSQR('U')`); paths 6t, 9t (`DGELQF`, `L` into `WORK(IU)` or `U`, `DORGLQ` in `A` or `VT`, `DORMBR('P', 'L', 'T')` on the slow path) and 10t (`DBDSQR('L')`); each callee with `LWORK - IWORK + 1`; the unconverged superdiagonal and the unscaling; a non-finite matrix refused) | `dgesvd.f` (identical in the 0.3.28, 0.3.30 and 0.3.31-22 trees), `dlange.f`, `dlascl.f`, `dlacpy.f`, `dlaset.f`, `ilaenv.f`, `INSTALL/dlamch.f`, and the routines of `blocked`; OpenBLAS `interface/gemm.c` with the level-3 driver and the Haswell kernel (depth `GEMM_Q` 256) | BSD-3-Clause |
| `numpy_linalg::gesvd::dbdsqr` (the checks; `N = 0`; without vectors refused: LAPACK's `DLASQ1`, the dqds algorithm, is not ported; a lower bidiagonal rotated to upper by `DLARTG`, the rotations applied to `U` by `DLASR('R', 'V', 'F')` and to `C` by `DLASR('L', 'V', 'F')`; then the parent module's `dbdsqr_upper_ncc`, 3.12.0's: a `-0` value kept) | `dbdsqr.f` (identical in the three trees), `dlartg.f90`, `dlasr.f` | BSD-3-Clause |
| `numpy_linalg::gesvd::ScipyLibrary`, `scipy_svd`, `ScipySvd`, `ScipySvdError` (`check_finite`'s `ValueError`; an empty matrix: identities with `full_matrices`, empty factors without, no LAPACK; the 32-bit size checks with each version's message; the copy into Fortran order, `LDA = LDU = M`, `LDVT = N` or `MIN(M, N)`, the query's `WORK(1)` truncated as `LWORK`; `INFO > 0` as `LinAlgError` with each version's message) | SciPy 1.15.3 `scipy/linalg/_decomp_svd.py` (`svd`), `flapack_gen.pyf.src` (`dgesvd`, `dgesvd_lwork`), `lapack.py` (`_compute_lwork`, `_check_work_float`), `scipy/_lib/_util.py` (`_asarray_validated`); SciPy 1.18.1 `scipy/linalg/_decomp_svd.py` (`svd`, `_format_emit_errors_warnings`), `src/_linalg_svd.hh` (`u_vh_shapes`, `_svd_gesvd`), `src/_batched_linalg_module.cc` (`_linalg_svd`) | BSD-3-Clause |
| `numpy_linalg::gesvd::nipype_fallback_svd`, `nipype_full_rank`, `FullRank` (`numpy.linalg.svd`, on `LinAlgError` SciPy's `gesvd`; the condition number `smax / smin`, at `cmax` or above `s + (smax - cmax·smin)/(cmax - 1)` and `np.dot(U, np.dot(np.diag(s), V))` through `openblas::matmul`) | nipype 1.12.0 `nipype/algorithms/confounds.py` (`fallback_svd`, `_full_rank`) | Apache-2.0 |

Validated bit for bit against SciPy's own `scipy_dgesvd_`, `scipy_dbdsqr_`, `scipy_dlarft_`, its
`scipy.linalg.svd(lapack_driver='gesvd')` and nipype's `fallback_svd` and `_full_rank` in both kept
environments (SciPy 1.15.3 and 1.18.1; `validation/investigations/confounds/check_gesvd.py`,
2026-11-11): see `validation/investigations/confounds/README.md`.

## SciPy's 4 × 4 inverse (`scipy_linalg`, field maps part B, relay 18)

**Ported** from SciPy 1.18.1 (`scipy/linalg/_basic.py` `inv`, `src/_linalg_inv.hh`
`invert_slice_general`, `invert_slice_triangular`, `invert_slice_diagonal`, `_inverse`,
`src/_common_array_utils.hh` `bandwidth`, `is_sym_or_herm`, `norm1_`, `swap_cf`,
`zero_other_triangle`, `_calc_lwork`) and SciPy 1.15.3 (`scipy/linalg/_basic.py` `inv`), BSD-3-Clause;
with the OpenBLAS their wheels bundle (0.3.31.dev and 0.3.28; read in the 0.3.30 release
tarball, `tags/OpenBLAS-0.3.30`: `interface/lapack/getrf.c`, `trtri.c`, `interface/gemv.c`,
`lapack/getrf/getrf_single.c`, `lapack/trtri/trtri_U_single.c`, `trtri_L_single.c`,
`lapack/trti2/trti2_U.c`, `trti2_L.c`, `driver/level2/trmv_U.c`, `trmv_L.c`,
`kernel/x86_64/daxpy.c`, `dscal.c`, `dgemv_n_4.c`, `dgemv_n_microk_haswell-4.c`,
`KERNEL.HASWELL`; the reference LAPACK in `lapack-netlib/SRC/dgetri.f`, `dgecon.f`), BSD-3-Clause.
`getf2` is `openblas::getf2` (numpy's path). Validated against both SciPy wheels
(`validation/investigations/sdcflows-fieldmap-b/check_scipy_inv.py`).
**Relay 19** (`scipy_lapack`): 1.18's condition estimates and symmetric path, from SciPy 1.18.1
(`src/_linalg_inv.hh` `invert_slice_cholesky`, `invert_slice_sym_herm`, the `POS_DEF` fallback of
`_inverse`; `src/_common_array_utils.hh` `norm1_sym_herm`, `norm1_sym_herm_lower`,
`fill_other_triangle`, `fill_other_triangle_noconj`, `init_status`, `_detect_problems`) and
OpenBLAS 0.3.30 (`lapack-netlib/SRC/dgecon.f`, `dtrcon.f`, `dpocon.f`, `dsycon.f`, `dlacn2.f`,
`dlatrs.f`, `drscl.f`, `dlantr.f`, `dlange.f`, `dpotri.f`, `dsytrf.f`, `dsytf2.f`, `dsytri.f`,
`dsytrs.f`, `INSTALL/dlamch.f`; `interface/asum.c`, `dot.c`, `axpy.c`, `scal.c`, `imax.c`,
`trsv.c`, `symv.c`, `syr.c`, `ger.c`, `gemv.c`, `lapack/potrf.c`, `lapack/trtri.c`,
`lapack/lauum.c`; `lapack/potf2/potf2_U.c`, `lapack/lauu2/lauu2_U.c`,
`lapack/potrf/potrf_U_single.c`, `lapack/lauum/lauum_U_single.c`; `driver/level2/trsv_U.c`,
`trsv_L.c`; `kernel/x86_64/dasum.c`, `dasum_microk_haswell-2.c`, `ddot.c`, `daxpy.c`, `dscal.c`,
`dsymv_U.c`, `dgemv_n_4.c`, `dgemv_t_4.c`; `kernel/generic/ger.c`), BSD-3-Clause; validated
against SciPy 1.18.1's wheel (0.3.31.dev) on 540,513 cases, every `rcond` against SciPy's own
LAPACK calls (`check_scipy_inv.py`). `openblas::iamax` is now `pub(crate)` (shared).
`openblas::affine_points` (relay 18): numpy's `dgemm`/`dgemv` of homogeneous points, as
`point_row`/`point_row_single`.

## numpy's small linear algebra (`numpy_linalg`)

**Ported** from the OpenBLAS 0.3.34 and 0.3.30 release tarballs (BSD-3-Clause; the LAPACK they
bundle, `lapack-netlib`, BSD-3-Clause) and numpy 2.5.4 (`numpy/linalg/umath_linalg.cpp`,
BSD-3-Clause): `numpy.linalg.svd` of a 3 × 3 matrix (`svd3`: `dgesdd` path 5a, `dgebrd` →
`dgebd2`, `dbdsdc` → `dlasdq` → `dbdsqr`, `dormbr` → `dorm2r`/`dorml2`), `eigh` of a 4 × 4 one
(`eigh4`: `dsyevd`, `dsytrd` → `dsytd2`, `dstedc` → `dsteqr`, `dormtr` → `dorm2r`), `det(a) > 0`
(`det3_is_positive`, and `det3_is_negative` for `det(a) < 0` since 2026-11-12, the confounds'
part's nipy source: OpenBLAS's `getf2_k`, numpy's `det_from_slogdet`), `matmul3`
(`openblas::gemm`) and nibabel's `io_orientation` polar factor (`io_orientation_polar`), with
`dlarfg`, `dlapy2`, `dlartg.f90`, `dlas2`, `dlasv2`, `dlaev2`, `dlasr`, `dlanst`, `dlansy`,
`dlange`, `dlascl`, `iladlc`, `iladlr`, `ilaenv`'s block sizes and `dlamch`; LAPACK 3.12.1's
`dlarf1f.f` (OpenBLAS 0.3.34) and 3.12.0's `dlarf.f` (0.3.30) as `LapackRelease`. The BLAS on
these lengths: `interface/{nrm2,scal,axpy,dot,rot,gemv,ger,symv,syr2}.c`, `kernel/x86_64/{nrm2.S,
dscal.c,daxpy.c,ddot.c,drot.c,drot_microk_haswell-2.c,dsymv_L.c,dgemv_n_4.c,dgemv_t_4.c}`,
`kernel/generic/ger.c`, `driver/level2/syr2_k.c` (the Haswell selection, `KERNEL.HASWELL`),
with the fused operations of the build numpy ships (checked in the disassembly of its
`libscipy_openblas64_`). The logic is re-expressed in Rust; no code was copied.

`numpy_linalg::lstsq` (2026-11-08, the t2smap replica part; tedana's log-linear fit):
`numpy.linalg.lstsq` of an `m × 2` design and any number of right-hand sides, from the same
tarballs and numpy 2.5.4 and 2.3.5 (`numpy/linalg/umath_linalg.cpp`'s `lstsq` and `abs2`,
`numpy/linalg/_linalg.py`'s `lstsq`): LAPACK `dgelsd.f` (paths 1 and 1a), `dgeqrf.f`,
`dgeqr2.f`, `dormqr.f`, `dorm2r.f`, `dgebrd.f`, `dgebd2.f`, `dormbr.f`, `dormlq.f`,
`dorml2.f`, `dlalsd.f` (`N <= SMLSIZ`), `dlasdq.f`, `dbdsqr.f` (now with `NCC`, and 3.12.1's
"avoid -ZERO" by release), `dlascl.f` (with its argument check), `dlaset.f`, `dlange.f`,
`dlanst.f`, `dlasrt.f`, `dlacpy.f`; the BLAS on long vectors and many columns:
`kernel/x86_64/{dgemv_t_4.c, dgemv_t_microk_haswell-4.c}` (row blocks of 2048, the 4×4
microkernel and the SSE2 4×2 and 4×1 kernels, the contracted tails), `daxpy.c` with
`daxpy_microk_haswell-2.c`, `drot.c`'s strided loop, `nrm2.S`'s four-accumulator main loop
(and the x87 range of every double), `dgemm_kernel_4x8_haswell.S`'s `2×N` paths with
`kernel/generic/gemm_small_matrix_permit.c`, `interface/gemm.c` and `driver/level3/level3.c`
(checked in the disassembly of both `libscipy_openblas64_` builds: GCC 14.2 for 0.3.34, GCC
10.2 for 0.3.30). Verified bit for bit against numpy 2.3.5 and 2.5.4 on 550 cases each
(`validation/investigations/t2smap/numerics/check_lstsq.py`), tedana's own calls on fMRIPrep's
three multi-echo `t2smap` calls included. The logic is re-expressed in Rust; no code was
copied.

`numpy_linalg::gesdd` and `numpy_linalg::blas` (2026-11-08, the t2smap replica part; SciPy's
`curve_fit` for tedana's monoexponential fit, `larmorx-optim`): the thin SVD of an `m × 2`
matrix, `scipy.linalg.svd(a, full_matrices=False)` and `numpy.linalg.svd(a,
full_matrices=False)` (`dgesdd` with `JOBZ='S'`), from the same tarballs' `lapack-netlib/SRC/`
(`dgesdd.f` paths 3 and 5s, `dgeqrf.f`, `dgeqr2.f`, `dorgqr.f`, `dorg2r.f`, `dgebrd.f`,
`dgebd2.f`, `dbdsdc.f`, `dlasdq.f`, `dbdsqr.f`, `dormbr.f`, `dormqr.f`, `dorm2r.f`,
`dormlq.f`, `dorml2.f`, `dlacpy.f`, `dlaset.f`, `dlange.f`, `dlascl.f`) and SciPy 1.18.1 and
1.15.3 `scipy/linalg/_decomp_svd.py` (BSD-3-Clause; SciPy's `scipy-openblas32` 0.3.31.dev and
0.3.28 builds, GCC 10.2, bundle LAPACK 3.12.0's routines: no `dlarf1f`); and `numpy.dot` of
vectors and matrices on the shapes `least_squares` gives it, from `interface/{gemv,dot}.c`,
`kernel/x86_64/{ddot.c, ddot_microk_haswell-2.c, dgemv_n_4.c, dgemv_t_4.c}` and numpy 2.5.4 and
2.3.5 `numpy/_core/src/common/cblasfuncs.c` (`cblas_matrixproduct`: row-major `dgemv` for a
C-contiguous matrix, column-major for another) and `numpy/_core/src/multiarray/arraytypes.c.src`
(`DOUBLE_dot`). Verified bit for bit against SciPy 1.18.1/numpy 2.5.4 and SciPy 1.15.3/numpy
2.3.5 (`validation/investigations/t2smap/numerics/check_curve_fit.py`: 54 products, 69 SciPy
and 67 numpy SVDs, and `curve_fit` on synthetic, edge and real voxels). The logic is re-expressed
in Rust; no code was copied.
