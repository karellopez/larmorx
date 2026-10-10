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
| `gaussian::RecursiveCoefficients`, `recursive_gaussian` (Deriche coefficients for orders 0–2, normalisation across scale, the causal and anti-causal recursion with its border coefficients) | `Modules/Filtering/Smoothing/include/itkRecursiveGaussianImageFilter.h/.hxx`, `Modules/Filtering/ImageFilterBase/include/itkRecursiveSeparableImageFilter.h/.hxx` |
| `gaussian::smoothing_recursive_gaussian` | `Modules/Filtering/Smoothing/include/itkSmoothingRecursiveGaussianImageFilter.h/.hxx` |
| `gaussian::laplacian_recursive_gaussian` | `Modules/Filtering/ImageFeature/include/itkLaplacianRecursiveGaussianImageFilter.h/.hxx` |
| `gaussian::gradient_magnitude_recursive_gaussian` | `Modules/Filtering/ImageGradient/include/itkGradientMagnitudeRecursiveGaussianImageFilter.h/.hxx`, `Modules/Filtering/ImageIntensity/include/itkSqrtImageFilter.h` |
| `discrete_gaussian` (`gaussian_kernel`, the Bessel functions, the separable convolution with nearest-edge boundaries) | `Modules/Filtering/Smoothing/include/itkDiscreteGaussianImageFilter.h/.hxx`, `Modules/Core/Common/include/itkGaussianOperator.h/.hxx`, `itkNeighborhoodOperator.hxx` (`CreateDirectional`, `FillCenteredDirectional`), `itkNeighborhoodInnerProduct.hxx`, `itkZeroFluxNeumannBoundaryCondition.hxx`, `Modules/Filtering/ImageFilterBase/include/itkNeighborhoodOperatorImageFilter.hxx` |
| `median` | `Modules/Filtering/Smoothing/include/itkMedianImageFilter.h/.hxx` (box neighbourhood, `ZeroFluxNeumannImageNeighborhoodPixelAccessPolicy`) |
| `lines` (running a line filter along an axis, `float → float` with double arithmetic) | the line iteration of `itkRecursiveSeparableImageFilter.hxx` (`DynamicThreadedGenerateData`); the grouping of lines is larmorx's |
| `volume` (`Volume`, `VolumeRef`) | original larmorx code |

Parity records: [ants-image-math](../../docs/validation/ants-image-math.md),
[ants-threshold-image](../../docs/validation/ants-threshold-image.md),
[ants-smooth-image](../../docs/validation/ants-smooth-image.md),
[ants-resample-image-by-spacing](../../docs/validation/ants-resample-image-by-spacing.md).
Behaviour found while porting: [ants-image-programs.md](../../docs/findings/ants-image-programs.md),
[ants-gaussian-filters.md](../../docs/findings/ants-gaussian-filters.md).
