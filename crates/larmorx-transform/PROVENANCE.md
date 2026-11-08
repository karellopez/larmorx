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

## numpy's small linear algebra (`numpy_linalg`)

**Ported** from the OpenBLAS 0.3.34 and 0.3.30 release tarballs (BSD-3-Clause; the LAPACK they
bundle, `lapack-netlib`, BSD-3-Clause) and numpy 2.5.4 (`numpy/linalg/umath_linalg.cpp`,
BSD-3-Clause): `numpy.linalg.svd` of a 3 × 3 matrix (`svd3`: `dgesdd` path 5a, `dgebrd` →
`dgebd2`, `dbdsdc` → `dlasdq` → `dbdsqr`, `dormbr` → `dorm2r`/`dorml2`), `eigh` of a 4 × 4 one
(`eigh4`: `dsyevd`, `dsytrd` → `dsytd2`, `dstedc` → `dsteqr`, `dormtr` → `dorm2r`), `det(a) > 0`
(`det3_is_positive`: OpenBLAS's `getf2_k`, numpy's `det_from_slogdet`), `matmul3`
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
