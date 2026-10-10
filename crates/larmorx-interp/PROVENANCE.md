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
| `gaussian` | ITK v5.4.5: `itkGaussianInterpolateImageFunction.hxx` | Apache-2.0 |
| `multi_label` | ITK v5.4.5: `itkLabelImageGaussianInterpolateImageFunction.hxx` | Apache-2.0 |
| `generic_label` | ITK remote module ITKGenericLabelInterpolator at `ebf2436469cc` (pinned by ITK v5.4.5's `Modules/Remote/GenericLabelInterpolator.remote.cmake`): `itkLabelImageGenericInterpolateImageFunction.h/.hxx` | Apache-2.0 |
| `windowed_sinc`, `window` (cosine, Hamming, Welch, Lanczos, Blackman; radius 3, constant boundary, or the default nearest-edge boundary for `WindowedSincEdge`) | ITK v5.4.5: `itkWindowedSincInterpolateImageFunction.h/.hxx`, `Modules/Core/Common/include/itkConstantBoundaryCondition.hxx`, `itkZeroFluxNeumannBoundaryCondition.hxx` | Apache-2.0 |
| `vnl::erf` (`vnl_erf`, `vnl_gamma_p`, series and continued fraction, `vnl_log_gamma`) | VXL as bundled with ITK v5.4.5: `Modules/ThirdParty/VNL/src/vxl/core/vnl/vnl_erf.h`, `vnl_gamma.cxx` | BSD (VXL) |

**Deliberate difference:** `exp`, `log`, `sin` and `cos` come from `larmorx_core::math`
(correctly rounded CORE-MATH ports, CLAUDE.md rule 5), not the platform's math library. They
agree with glibc, and so with ANTs on Linux, except where glibc is not correctly rounded
(about 0.003 % of `exp`/`log` calls and 0.15 % of `sin`/`cos` calls); there a result can
differ by an ulp. `pow` (the B-spline prefilter's pole powers) still comes from the `libm`
crate. See [docs/findings/platform-math.md](../../docs/findings/platform-math.md).

## SciPy's spline interpolation (`src/ndimage/`)

**Ported** from SciPy 1.15.2 (tag `v1.15.2` → `0f1fd4a7268b`, BSD-3-Clause, `UPSTREAM.md`;
SciPy 1.15.3, in the development venv, has the same `ndimage` interpolation code). The
operation order is kept, so results are bit-identical to SciPy on x86-64 Linux
(`docs/validation/resample-series.md`). Each file reproduces SciPy's notice.

| larmorx | SciPy 1.15.2 | Licence |
|---|---|---|
| `ndimage::splines`: `filter_poles`, `interpolation_weights`, `LineFilter` (gain, causal/anticausal initialisations for mirror, wrap and reflect, the recursions), `apply_rows` (the same per line, many lines at once) | `scipy/ndimage/src/ni_splines.c` (`get_filter_poles`, `get_spline_interpolation_weights`, `_init_*`, `_apply_filter`, `apply_filter`) | BSD-3-Clause |
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
