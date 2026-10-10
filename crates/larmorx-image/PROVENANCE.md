# Provenance: larmorx-image

**Ported** from ITK v5.4.5 (`f51594ad8819`, Apache-2.0), as ANTs v2.6.5 uses these filters.
The logic is re-expressed in Rust; no code was copied verbatim. Each module lists its
upstream files below as it is ported.

| larmorx module | ITK v5.4.5 files |
|---|---|
| `itk_math` (`float_almost_equal_*`, `almost_equal_*`) | `Modules/Core/Common/include/itkMath.h` (`FloatAlmostEqual`), `itkMathDetail.h` (`FloatIEEE::AsULP`) |
| `statistics::Histogram` (`new`, `index`, `quantile`, `measurement`) | `Modules/Numerics/Statistics/include/itkHistogram.hxx` (`Initialize`, `GetIndex`, `Quantile`, `GetMeasurement`) |
| `statistics::scalar_image_histogram` | `Modules/Numerics/Statistics/include/itkScalarImageToHistogramGenerator.hxx`, `itkSampleToHistogramFilter.hxx` (automatic bounds, marginal scale), `itkStatisticsAlgorithm.hxx` (`FindSampleBound`) |
| `statistics::label_histogram`, `label_minimum_maximum` | `Modules/Filtering/ImageStatistics/include/itkLabelStatisticsImageFilter.h/.hxx` |
| `statistics::minimum_maximum` | `Modules/Core/Common/include/itkMinimumMaximumImageCalculator.hxx` |
| `threshold::binary_threshold` | `Modules/Filtering/Thresholding/include/itkBinaryThresholdImageFilter.h/.hxx` |
| `threshold::otsu_thresholds` | `Modules/Filtering/Thresholding/include/itkOtsuMultipleThresholdsCalculator.h/.hxx` |
| `threshold::threshold_labels` | `Modules/Filtering/Thresholding/include/itkThresholdLabelerImageFilter.h/.hxx` |
| `threshold::otsu_multiple_thresholds` | `Modules/Filtering/Thresholding/include/itkOtsuMultipleThresholdsImageFilter.h/.hxx` (ITK 5 defaults: 128 bins, bin upper bounds) |
| `intensity::rescale_intensity` | `Modules/Filtering/ImageIntensity/include/itkRescaleIntensityImageFilter.h/.hxx` |

Parity records: [ants-image-math](../../docs/validation/ants-image-math.md),
[ants-threshold-image](../../docs/validation/ants-threshold-image.md). Behaviour found while
porting: [docs/findings/ants-image-programs.md](../../docs/findings/ants-image-programs.md).
