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
| `TransformChain` (the `-t` list as ANTs builds it: options last first, composites expanded, inverses by `GetInverseTransform`) | ANTs v2.6.5 (`fdce4d2f84b6`): `Examples/itkantsRegistrationHelper.h`, `GetCompositeTransformFromParserOption` | Apache-2.0 |

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
wheels bundle (scipy-openblas64 0.3.30, `numpy.show_config()`). Only the operation order of
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
