# Provenance: larmorx-ants

**Ported** from the release tags in [UPSTREAM.md](../../UPSTREAM.md). The logic is
re-expressed in Rust; no code was copied verbatim.

| larmorx | Upstream | Licence |
|---|---|---|
| `apply_transforms` (linear scan-line path, per-voxel path, inside test, default value) | ITK v5.4.5: `Modules/Filtering/ImageGrid/include/itkResampleImageFilter.hxx`; `Modules/Core/Common/include/itkImageBase.h/.hxx` (index ↔ point) | Apache-2.0 |
| `apply_transforms` for time series (shared index map, volumes extracted with `DirectionCollapseToSubmatrix`) | ANTs v2.6.5: `Examples/antsApplyTransforms.cxx` | Apache-2.0 |
| `cli::apply_transforms` (options, interpolator selection and defaults, output naming, `-u` casts, time-series output) | ANTs v2.6.5: `Examples/antsApplyTransforms.cxx`, `Examples/make_interpolator_snip.tmpl` | Apache-2.0 |
| `cli::parser` (word regrouping, option and value rules, `name[p1,p2]`, value order) | ANTs v2.6.5: `Utilities/antsCommandLineParser.cxx/.h`, `Utilities/antsCommandLineOption.cxx` | Apache-2.0 |
| `transform_points` | ANTs v2.6.5: `Examples/antsApplyTransformsToPoints.cxx` (the point mapping) | Apache-2.0 |

The behaviour found while porting is recorded in [docs/findings/](../../docs/findings/). The
parity record is [docs/validation/ants-apply-transforms.md](../../docs/validation/ants-apply-transforms.md).
