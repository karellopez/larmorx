# Provenance: larmorx-interp

**Ported** from the release tags in [UPSTREAM.md](../../UPSTREAM.md). The logic is
re-expressed in Rust; no code was copied verbatim. Each interpolator keeps ITK's edge
handling and operation order
([docs/findings/itk-resampling.md](../../docs/findings/itk-resampling.md)).

| larmorx | Upstream | Licence |
|---|---|---|
| `is_inside` | ITK v5.4.5: `Modules/Core/ImageFunction/include/itkImageFunction.h` (`IsInsideBuffer`), `itkImageFunction.hxx` | Apache-2.0 |
| `linear` | ITK v5.4.5: `itkLinearInterpolateImageFunction.h/.hxx` (3D optimized path) | Apache-2.0 |
| nearest neighbour | ITK v5.4.5: `itkNearestNeighborInterpolateImageFunction.h` | Apache-2.0 |
| `bspline` (prefilter, weights for orders 0–5, mirror boundary) | ITK v5.4.5: `itkBSplineDecompositionImageFilter.hxx`, `itkBSplineInterpolateImageFunction.hxx` | Apache-2.0 |
| `gaussian` | ITK v5.4.5: `itkGaussianInterpolateImageFunction.hxx` | Apache-2.0 |
| `multi_label` | ITK v5.4.5: `itkLabelImageGaussianInterpolateImageFunction.hxx` | Apache-2.0 |
| `generic_label` | ITK remote module ITKGenericLabelInterpolator at `ebf2436469cc` (pinned by ITK v5.4.5's `Modules/Remote/GenericLabelInterpolator.remote.cmake`): `itkLabelImageGenericInterpolateImageFunction.h/.hxx` | Apache-2.0 |
| `windowed_sinc`, `window` (cosine, Hamming, Welch, Lanczos, Blackman; radius 3, constant boundary) | ITK v5.4.5: `itkWindowedSincInterpolateImageFunction.h/.hxx`, `Modules/Core/Common/include/itkConstantBoundaryCondition.hxx` | Apache-2.0 |
| `vnl::erf` (`vnl_erf`, `vnl_gamma_p`, series and continued fraction, `vnl_log_gamma`) | VXL as bundled with ITK v5.4.5: `Modules/ThirdParty/VNL/src/vxl/core/vnl/vnl_erf.h`, `vnl_gamma.cxx` | BSD (VXL) |

**Deliberate difference:** `exp`, `log`, `sin` and `cos` come from `larmorx_core::math`
(correctly rounded CORE-MATH ports, CLAUDE.md rule 5), not the platform's math library. They
agree with glibc, and so with ANTs on Linux, except where glibc is not correctly rounded
(about 0.003 % of `exp`/`log` calls and 0.15 % of `sin`/`cos` calls); there a result can
differ by an ulp. `pow` (the B-spline prefilter's pole powers) still comes from the `libm`
crate. See [docs/findings/platform-math.md](../../docs/findings/platform-math.md).
