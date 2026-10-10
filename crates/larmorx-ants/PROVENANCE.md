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
| `image` (`ReadImage`/`WriteImage` rules, pixel casts, reading into fewer or more dimensions) | ANTs v2.6.5: `Utilities/ReadWriteData.h`, `ReadWriteData.cxx`; ITK v5.4.5: `Modules/IO/ImageBase/include/itkImageFileReader.hxx` (`GenerateOutputInformation`) | Apache-2.0 |
| `image_math::arithmetic` (`ImageMath<DIM>`, `NegativeImage`) | ANTs v2.6.5: `Examples/ImageMath_Templates.hxx` (`ImageMath`, `NegativeImage`) | Apache-2.0 |
| `image_math::intensity` (`TruncateImageIntensity`, `NormalizeImage`, `RescaleImage`) | ANTs v2.6.5: `Examples/ImageMath_Templates.hxx` | Apache-2.0 |
| `image_math::gaussian` (`G`, `Laplacian`, `Grad`, `UnsharpMask`) | ANTs v2.6.5: `Examples/ImageMath_Templates.hxx` (`SmoothImage<DIM>`, `LaplacianImage`, `GradientImage`, `UnsharpMaskImage`); ITK v5.4.5: `Modules/Filtering/ImageFeature/include/itkUnsharpMaskImageFilter.h/.hxx` | Apache-2.0 |
| `smooth_image`, `cli::smooth_image` | ANTs v2.6.5: `Examples/SmoothImage.cxx` | Apache-2.0 |
| `resample` (`ResampleImageFilter` with an `IdentityTransform` in 1 to 4 dimensions: the scan-line path, point ↔ index with vnl's inverse, 4D `EvaluateUnoptimized`) | ITK v5.4.5: `Modules/Filtering/ImageGrid/include/itkResampleImageFilter.hxx` (`LinearThreadedGenerateData`), `Modules/Core/ImageFunction/include/itkLinearInterpolateImageFunction.h/.hxx`, `itkNearestNeighborInterpolateImageFunction.h`, `Modules/Core/Common/include/itkImageBase.h/.hxx` | Apache-2.0 |
| `resample_image_by_spacing`, `cli::resample_image_by_spacing` | ANTs v2.6.5: `Examples/ResampleImageBySpacing.cxx` | Apache-2.0 |
| `threshold_image` | ANTs v2.6.5: `Examples/ThresholdImage.cxx` (`OtsuThreshold` with a mask, `BinaryThreshold_AltInsideOutside_threashold`) | Apache-2.0 |
| `multiply_images` | ANTs v2.6.5: `Examples/MultiplyImages.cxx` | Apache-2.0 |
| `cli::image_math` (dispatch tables, usage, exit codes) | ANTs v2.6.5: `Examples/ImageMath.cxx`, `ImageMathHelper{2D,3D,4D}.cxx`, the `ImageMathHelper*` tables of `ImageMath_Templates.hxx` | Apache-2.0 |
| `cli::threshold_image`, `cli::multiply_images` | ANTs v2.6.5: `Examples/ThresholdImage.cxx`, `Examples/MultiplyImages.cxx` (argument handling) | Apache-2.0 |
| `cli::cstd` (`from_string<float>`, `ConvertVector<float>`, `std::stoi`, `std::stof`, `cout << float`) | ANTs v2.6.5: `Examples/ImageMath_Templates.hxx` (`from_string`), `Examples/antsUtilities.h` (`ConvertVector`); the standard-library semantics are libstdc++'s (`num_get`) and glibc's (`strtod`, `strtof`, `printf %g`), reimplemented from their specifications | Apache-2.0 |

The behaviour found while porting is recorded in [docs/findings/](../../docs/findings/). The
parity records are [ants-apply-transforms](../../docs/validation/ants-apply-transforms.md),
[ants-image-math](../../docs/validation/ants-image-math.md),
[ants-threshold-image](../../docs/validation/ants-threshold-image.md),
[ants-multiply-images](../../docs/validation/ants-multiply-images.md),
[ants-smooth-image](../../docs/validation/ants-smooth-image.md) and
[ants-resample-image-by-spacing](../../docs/validation/ants-resample-image-by-spacing.md).
