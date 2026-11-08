# Provenance: larmorx-interp

**Ported** from the release tags in [UPSTREAM.md](../../UPSTREAM.md). The logic is
re-expressed in Rust; no code was copied verbatim. Each interpolator keeps ITK's edge
handling and operation order
([docs/findings/itk-resampling.md](../../docs/findings/itk-resampling.md)).

| larmorx | Upstream | Licence |
|---|---|---|
| `is_inside` | ITK v5.4.5: `Modules/Core/ImageFunction/include/itkImageFunction.h` (`IsInsideBuffer`), `itkImageFunction.hxx` | Apache-2.0 |
| `linear`, `LinearYz` (the same arithmetic, the y/z part computed once per row) | ITK v5.4.5: `itkLinearInterpolateImageFunction.h/.hxx` (3D optimized path) | Apache-2.0 |
| `nearest`, `NearestYz` | ITK v5.4.5: `itkNearestNeighborInterpolateImageFunction.h` | Apache-2.0 |
| `bspline` (prefilter, weights for orders 0–5, mirror boundary) | ITK v5.4.5: `itkBSplineDecompositionImageFilter.hxx`, `itkBSplineInterpolateImageFunction.hxx` | Apache-2.0 |
| `nd::InterpolationNd::BSplineCoefficients`, `bspline_line_to_coefficients` (the image taken as the coefficients, no prefilter; one line of the decomposition) | ITK v5.4.5: `itkBSplineResampleImageFunction.h`, `itkBSplineDecompositionImageFilter.hxx` (`DataToCoefficients1D`) | Apache-2.0 |
| `gaussian` | ITK v5.4.5: `itkGaussianInterpolateImageFunction.hxx` | Apache-2.0 |
| `multi_label` | ITK v5.4.5: `itkLabelImageGaussianInterpolateImageFunction.hxx` | Apache-2.0 |
| `generic_label` | ITK remote module ITKGenericLabelInterpolator at `ebf2436469cc` (pinned by ITK v5.4.5's `Modules/Remote/GenericLabelInterpolator.remote.cmake`): `itkLabelImageGenericInterpolateImageFunction.h/.hxx` | Apache-2.0 |
| `windowed_sinc`, `window` (cosine, Hamming, Welch, Lanczos, Blackman; radius 3, constant boundary, or the default nearest-edge boundary for `WindowedSincEdge`) | ITK v5.4.5: `itkWindowedSincInterpolateImageFunction.h/.hxx`, `Modules/Core/Common/include/itkConstantBoundaryCondition.hxx`, `itkZeroFluxNeumannBoundaryCondition.hxx` | Apache-2.0 |
| `nd` (`InterpolatorNd`: the B-spline prefilter and evaluation, the Gaussian and label-Gaussian interpolators, the windowed sincs, in any dimension; checked bit for bit against the 3D versions in 3D) | the same ITK v5.4.5 files as the 3D versions above (the classes are templated on the dimension) | Apache-2.0 |
| `vnl::erf` (`vnl_erf`, `vnl_gamma_p`, series and continued fraction, `vnl_log_gamma`; `vnl_log_gamma(0.5)` computed once, the same bits) | VXL as bundled with ITK v5.4.5: `Modules/ThirdParty/VNL/src/vxl/core/vnl/vnl_erf.h`, `vnl_gamma.cxx` | BSD (VXL) |
| `nd::bspline_coefficients`, `nd::bspline_at` (`BSplineInterpolateImageFunction` in any dimension: the coefficients' recursive prefilter and the evaluation, for Atropos' histogram models) | ITK v5.4.5: `Modules/Core/ImageFunction/include/itkBSplineInterpolateImageFunction.hxx`, `itkBSplineDecompositionImageFilter.hxx` | Apache-2.0 |
| `vnl::erfc` (`vnl_erfc`: W. J. Cody's rational approximations) | VXL in ITK v5.4.5: `Modules/ThirdParty/VNL/src/vxl/core/vnl/vnl_erf.cxx` | BSD (VXL) |
| `Interpolator::with_precision`, `nearest_as` and the coordinate type `R` of `gaussian`, `multi_label`, `windowed_sinc` (`TCoordRep = float`: `RoundHalfIntegerUp` in float, the Gaussian region's ends stored as float, the window functions `XWindowFunction<3, float, float>`, `Math::Floor` of the index) | ITK v5.4.5: `itkNearestNeighborInterpolateImageFunction.h`, `Modules/Core/Common/include/itkMathDetail.h` (`RoundHalfIntegerUp_64`, `Floor_64`), `itkGaussianInterpolateImageFunction.hxx` (`ComputeInterpolationRegion`), `itkWindowedSincInterpolateImageFunction.h/.hxx`; ANTs v2.6.5 `Examples/make_interpolator_snip.tmpl` (the template arguments) | Apache-2.0 |

**Deliberate difference:** `exp`, `log`, `sin` and `cos` come from `larmorx_core::math`
(correctly rounded CORE-MATH ports, CLAUDE.md rule 5), not the platform's math library. They
agree with glibc, and so with ANTs on Linux, except where glibc is not correctly rounded
(about 0.003 % of `exp`/`log` calls and 0.15 % of `sin`/`cos` calls); there a result can
differ by an ulp. `pow` (the B-spline prefilter's pole powers) still comes from the `libm`
crate. See [docs/findings/platform-math.md](../../docs/findings/platform-math.md).

## SciPy's spline interpolation (`src/ndimage/`)

**Ported** from SciPy 1.15.2 (tag `v1.15.2` → `0f1fd4a7268b`, BSD-3-Clause, `UPSTREAM.md`;
SciPy 1.15.3, in the development venv, has the same `ndimage` interpolation code) and **follows
SciPy 1.18.1 since 2026-10-29** (the version update's part 3: of the files below, only
`ni_splines.c`'s reflect initialisation changes a result, `_init_causal_reflect`, which SciPy
1.18 sums into a variable of its own: SCI-03 fixed upstream; `docs/findings/scipy-ndimage.md`,
"SciPy 1.18.1"). The operation order is kept, so results are bit-identical to SciPy 1.18.1 on
x86-64 Linux (`docs/validation/resample-series.md`, run in `oracles/python-newest`). Each file
reproduces SciPy's notice.

| larmorx | SciPy 1.15.2 | Licence |
|---|---|---|
| `ndimage::splines`: `filter_poles`, `interpolation_weights`, `LineFilter` (gain, causal/anticausal initialisations for mirror, wrap and reflect, the recursions), `apply_rows` (the same per line, many lines at once) | `scipy/ndimage/src/ni_splines.c` (`get_filter_poles`, `get_spline_interpolation_weights`, `_init_*`, `_apply_filter`, `apply_filter`); `_init_causal_reflect` from SciPy 1.18.1 | BSD-3-Clause |
| `ndimage::map_coordinate`, `Spline::sample` (the coordinate-array path), `filter_axis` (lines of an axis) | `scipy/ndimage/src/ni_interpolation.c` (`map_coordinate`, `NI_GeometricTransform`, `_get_spline_boundary_mode`, `NI_SplineFilter1D`), `ni_support.c` (line buffers) | BSD-3-Clause (Peter J. Verveer) |
| `Mode`, `Mode::extend`, `Mode::prepad`, `pad`, `map_coordinates`, `spline_filter`, `spline_filter1d`, `Output` (`CASE_INTERP_OUT*`) | `scipy/ndimage/_interpolation.py` (`map_coordinates`, `spline_filter`, `spline_filter1d`, `_prepad_for_spline_filter`), `_ni_support.py` (`_extend_mode_to_code`, `_get_output`) | BSD-3-Clause |

**Deliberate differences** (`docs/findings/scipy-ndimage.md`):
- `pow(z, n)` in the prefilter is `larmorx_core::math::powi`, correctly rounded; glibc's
  differs only for powers below 1e-100.
- `floor` is computed without the C library (same result for every input).
- C's casts of NaN and out-of-range doubles to integers follow x86-64 (`i64::MIN`); where SciPy
  would then read outside its buffer (index arithmetic that overflows), larmorx returns NaN.
- `Spline::sample_batch` evaluates interior points four at a time; each point keeps SciPy's
  own order of operations, so the values are the same.

## SciPy's 1D Gaussian filter and binary morphology, nilearn's EPI mask (`src/ndimage/filters.rs`, `src/ndimage/morphology.rs`, `src/nilearn.rs`)

**Ported** (2026-11-08, for tedana's `t2smap` without a mask) from SciPy 1.18.1 (the workspace's
`tags/scipy-1.18.1/`, BSD-3-Clause; SciPy 1.15.3 has the same code on these paths) and nilearn
0.14.1 (BSD-3-Clause, "Copyright (c) The nilearn developers"; as installed in both Python
environments). Results are bit-identical to them (`validation/investigations/t2smap/replica/
check_outputs.py`: tedana's masks without `--mask` on float32, scaled int16 and z-concatenated
data, in both environments).

| larmorx | Upstream | Licence |
|---|---|---|
| `ndimage::filters::gaussian_weights`, `gaussian_filter1d` (order 0) | SciPy 1.18.1 `scipy/ndimage/_filters.py` (`_gaussian_kernel1d`, `gaussian_filter1d`) | BSD-3-Clause |
| `ndimage::filters::correlate1d_reflect` (the symmetric, antisymmetric and general loops; the line buffer's reflection) | SciPy 1.18.1 `scipy/ndimage/src/ni_filters.c` (`NI_Correlate1D`), `src/ni_support.c` (`NI_ExtendLine`, `NI_EXTEND_REFLECT`); `_filters.py` (`correlate1d`) | BSD-3-Clause |
| `ndimage::morphology::{binary_erosion, binary_dilation}` (the default cross structure, `border_value=0`, iterations) | SciPy 1.18.1 `scipy/ndimage/_morphology.py` (`_binary_erosion`, `binary_erosion`, `binary_dilation`), `src/ni_morphology.c` (`NI_BinaryErosion`, `CASE_NI_ERODE_POINT`: the border value inverted for dilation) | BSD-3-Clause |
| `ndimage::morphology::label` (face connectivity; regions numbered by their first voxel in C order) | SciPy 1.18.1 `scipy/ndimage/_measurements.py` (`label`), `src/_ni_label.pyx` (the line scan and merge table) | BSD-3-Clause |
| `nilearn::compute_epi_mask` (`compute_mean` with `smooth=1`, `smooth_array`, `ensure_finite_data`, the cut-offs and gap threshold, `_post_process_mask`, `largest_connected_component`) | nilearn 0.14.1 `nilearn/masking.py` (`compute_epi_mask`, `_post_process_mask`), `nilearn/image/image.py` (`compute_mean`, `smooth_array`), `nilearn/_utils/ndimage.py` (`largest_connected_component`), `nilearn/_utils/niimg.py` (`ensure_finite_data`) | BSD-3-Clause |

The memory layout that sets the temporal mean's order (`nilearn::Layout`) was measured: nibabel's
arrays from files are Fortran-ordered, the `get_fdata()` of a sliced image (tedana's reference of
z-concatenated data) C-ordered. SciPy's `_nd_image` modules of both environments contain no fused
multiply-add instructions (`objdump`), so every product and sum of the correlation is rounded.
