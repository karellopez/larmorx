# ANTs' Gaussian filters and resampling: ImageMath G, Laplacian, Grad, UnsharpMask; SmoothImage; ResampleImageBySpacing; ResampleImage (ANTs v2.6.5, ITK v5.4.5)

What these programs and the ITK filters behind them really compute, found while porting them
(milestone A2, Gaussian group). Line numbers are at the tags `ANTs-v2.6.5` and `ITK-v5.4.5`.
The parity records are [ants-image-math](../validation/ants-image-math.md),
[ants-smooth-image](../validation/ants-smooth-image.md),
[ants-resample-image-by-spacing](../validation/ants-resample-image-by-spacing.md) and
[ants-resample-image](../validation/ants-resample-image.md). How the
programs read and write images is in [ants-image-programs.md](ants-image-programs.md).

## ITK's recursive Gaussian filters

**`RecursiveGaussianImageFilter` is Deriche's fourth-order recursion, run in double on
`float` images** (`Modules/Filtering/Smoothing/include/itkRecursiveGaussianImageFilter.hxx:70-322`,
`Modules/Filtering/ImageFilterBase/include/itkRecursiveSeparableImageFilter.hxx:83-312`).
- The coefficients are built from Deriche's constants (`A1 = 1.3530`, `W1 = 0.6681`,
  `L1 = −1.3932`, ...) for orders 0, 1 and 2 with `sigmad = sigma / spacing`: sigma is in
  **physical units**. Order 0 is normalised to unit gain (`alpha0 = 2·SN/SD − N0`), order 1
  and 2 by `alpha1`, `alpha2`; the second order mixes the order-0 and order-2 sets with
  `beta`. `NormalizeAcrossScale` multiplies orders 1 and 2 by `sigma` and `sigma²`; it does
  nothing for order 0, and ANTs never sets it.
- Each line is read into a `double` array, filtered causally and anti-causally, the two
  summed, and stored as `float`. **The borders extend the first and last value to
  infinity** through the `BN`/`BM` coefficients (`D·SN/SD`, `D·SM/SD`).
- Each multiply-add group is evaluated left to right (`a1*b1 + a2*b2 + a3*b3 + a4*b4`), and
  the oracle is built without FMA contraction (baseline x86-64, `-O3`).
- An axis with fewer than 4 voxels throws ("The number of pixels along direction d is less
  than 4"); a sigma ≤ 0 throws ("Sigma must be greater than zero"); NaN passes and gives NaN.
*Read; validated* (every Gaussian case bit-identical).
*larmorx:* `larmorx_image::gaussian::RecursiveCoefficients` and `recursive_gaussian`, term by
term.

**`SmoothingRecursiveGaussianImageFilter` smooths the last axis first**, then the others in
order (`itkSmoothingRecursiveGaussianImageFilter.hxx:36-55`), and its intermediate images are
`float` (`NumericTraits<float>::FloatType`, `itkSmoothingRecursiveGaussianImageFilter.h:80`):
every pass rounds to float. So the result depends on the axis order. Every axis needs 4
voxels (checked before filtering, `GenerateData`, `.hxx:210-217`). *Read; validated.* *larmorx:* the same order and
rounding.

**`LaplacianRecursiveGaussianImageFilter`**: for each axis `d`, the second derivative along
`d` (from the input), then smoothing along the other axes in increasing order, then
`cumulative = float(cumulative + b · (1 / spacing[d]²))` with `cumulative` a `float` image
starting at 0 (`itkLaplacianRecursiveGaussianImageFilter.hxx:184-216`). The derivative is per
voxel; the division by the squared spacing makes it per mm². *Read; validated.*

**`GradientMagnitudeRecursiveGaussianImageFilter`**: for each axis, the first derivative along
it, smoothed along the others, then `cumulative = float(cumulative + (b / spacing)²)` (the
square in double), and finally `float(sqrt(double(cumulative)))`
(`itkGradientMagnitudeRecursiveGaussianImageFilter.hxx:222-262`). *Read; validated.*

**In 4D these filters run along time too**, with the time step as the spacing, and in 2D they
run in two dimensions only. *Validated* (`dim2` and `dim4` cases of every program).
*larmorx:* `larmorx_image::VolumeRef` has the image's own number of dimensions.

**The coefficients call glibc's `sin`, `cos` and `exp`**, which are not always correctly
rounded; larmorx's (`larmorx_core::math`) are. Comparing the two over 20,000 sigmas from 0.25
to 20 voxels (CPython's `math`, which calls glibc, against mpmath), the coefficients differ in
the last bit for 0.5 % (order 0) to 0.7 % (orders 1 and 2) of sigmas, e.g. 1.626 and 2.013
voxels. **The outputs still match**: a pass rounds a double result to float, and a one-ulp
change of a double coefficient moves the result by about 1e-16 relative, so a float only
changes when its double value lies within that distance of a rounding boundary. The 1 mm MNI
template smoothed with sigma 1.626 mm and its Laplacian with sigma 2.013 mm are bit-identical
to ANTs (8.5 M voxels each). *Verified* (2026-10-10).

## ITK's discrete Gaussian (ImageMath `G`)

**`DiscreteGaussianImageFilter` convolves with `GaussianOperator` kernels**
(`Modules/Filtering/Smoothing/include/itkDiscreteGaussianImageFilter.hxx`,
`Modules/Core/Common/include/itkGaussianOperator.hxx:25-199`):
- the kernel is `e^{−t} I_k(t)` for the variance `t = variance / spacing²` (voxels²), from
  Numerical Recipes' polynomial approximations of `I_0` and `I_1` and Miller's downward
  recurrence for `I_k`; taps are added while the one-sided mass is below `1 − maximum_error`,
  up to `MaximumKernelWidth = 32` taps on each side, then normalised to sum 1;
- **the last axis is filtered first** (the kernels are stored in reverse, `.hxx:215-222`), each
  pass `float → float` with a double accumulator summed from the first tap
  (`NeighborhoodInnerProduct`), and the nearest edge voxel standing in outside the image
  (`ZeroFluxNeumannBoundaryCondition`);
- **a variance of 0 gives the kernel `[0, 1, 0]`** (`I_1(0) = 0` is kept as a tap), so the
  image is unchanged;
- **above a variance of about 709 voxels², `e^t` overflows in `I_0`** and the kernel is NaN:
  `ImageMath 3 out G img 41` on 1.5 mm voxels writes NaN. *Validated* (`G/huge-variance`).

**ImageMath `G`** (`SmoothImage<DIM>`, `ImageMath_Templates.hxx:8161-8211`) sets the variance
to `sigma²` **computed in float** (`itk::Math::sqr` of a `float`), in physical units, and the
maximum error to `0.01f` (0.009999999776). One sigma or one per axis; with another count
(including none) it prints "Incorrect sigma vector size" and filters with the default
variance 0: the image comes back unchanged. *Read; validated* (`G/no-sigma`,
`G/wrong-length`).

## ImageMath `Laplacian`, `Grad`, `UnsharpMask`

**`Laplacian` reads its normalize flag from the sigma argument** (`LaplacianImage`,
`ImageMath_Templates.hxx:9235-9285`): `argct` is not advanced after the sigma
(lines 9244-9257), so `sig = atof(argv[5])` and `normalize = std::stoi(argv[5])`. So:
- fMRIPrep's `Laplacian img 1.5 1` normalises because `std::stoi("1.5")` is 1; its `1` is
  ignored;
- `Laplacian img 1.5 0` normalises too, and `Laplacian img 0.8 1` does not;
- `Laplacian img .5` aborts (`std::stoi` finds no digits);
- a sigma ≤ 0 becomes 0.5, and `-1` normalises (`stoi` gives −1).
Normalising is `RescaleIntensityImageFilter` to `[0, 1]`. *Read; validated*
(`laplacian/flag-ignored`, `not-normalized`, `negative-sigma`, `leading-point`).
*larmorx:* the command line reproduces it; `lx.ants.laplacian(img, sigma, normalize=...)`
takes the two separately.

**`Grad`** (`GradientImage`, lines 9180-9233) does advance: `Grad img sigma normalize`.
A sigma ≤ 0 becomes 0.5. *Read; validated.*

**`UnsharpMask`** (`UnsharpMaskImage`, lines 2187-2246) reads amount, radius and threshold
with `std::stof` and the spacing flag with `std::stoi`; ITK's `UnsharpMaskImageFilter` has
`float` internal precision by default, so `v + (d − t)·amount` is computed in float, with
`d = v − smoothed` (`itkUnsharpMaskImageFilter.h:204-233`). The radius in voxels is
multiplied by the spacing **in double** (unlike `SmoothImage`). A negative threshold throws.
*Read; validated.*

## SmoothImage (`Examples/SmoothImage.cxx`)

- **A sigma in voxels is multiplied by the spacing in float**:
  `sigmaVector[d] * static_cast<float>(spacing[d])` (lines 71, 80), then the filter divides
  by the double spacing again, so the voxel sigma is not exactly the one given. *Read;
  validated* (bit-identical).
- The sigma is `ConvertVector<float>` (`antsUtilities.h:617-650`): pieces split at `x`, each
  read with `istringstream >> float` into one variable reused between pieces, so `1.5x`
  gives `1.5, 1.5`.
- A sigma list of the wrong length prints "Incorrect sigma vector size" and smooths with the
  filter's default, **1 mm** on every axis. *Validated* (`gaussian/wrong-length`).
- **The median filter** (sixth argument non-zero) is ITK's `MedianImageFilter` with radius
  `static_cast<unsigned long>(sigma)` (truncated: 1.7 is 1), the nearest edge voxel standing
  in outside. With a sigma list of the wrong length the radius is left uninitialised.
  *Validated* (`median/*`). *larmorx:* `larmorx_image::median`; it orders values with
  `f32::total_cmp`, where ITK's `nth_element` leaves `-0.0` against `+0.0` (and NaN) to chance.
- Without an output name (three arguments after the program) ANTs smooths, then
  `std::string(nullptr)` throws in `WriteImage` (`ReadWriteData.h:526`). A sigma of 0
  throws in ITK. Both abort. *Verified.* *larmorx:* exit 1 with a message.

## ResampleImageBySpacing (`Examples/ResampleImageBySpacing.cxx`)

- **The smoothing sigma is `out/in − 1`, read by ITK in millimetres** (lines 144, 277, 406):
  `float sig = atof(spacing) / inputSpacing − 1.0` is given to `RecursiveGaussianImageFilter`,
  which divides it by the spacing again. So 1 → 2 mm smooths with one voxel, 2 → 4 mm with
  half a voxel, 1.5 → 2 mm with 0.22 voxels. Axes whose sigma is ≤ 0 (upsampling or
  unchanged) are not smoothed. *Read; validated* (stdout prints the sigmas).
- Each axis is smoothed by its own filter, in axis order, each `float → float`.
- **If a smoothed axis has fewer than 4 voxels, ITK throws, ANTs prints "Exception catched !"
  and goes on with the filter's output, which was allocated but never written**: the result
  is uninitialised memory (values up to ±1e27 that change from run to run), and ANTs exits 0.
  *Verified* (three runs, three different outputs). *larmorx:* stops with ITK's message
  (expected divergence `errors/thin-smoothed-axis`).
- **The output grid** keeps the input's origin and direction; its size is
  `(size_t)(size · in_spacing / out_spacing + addvox)` (truncated). The resampler is
  `ResampleImageFilter` with an `IdentityTransform` (a linear transform, so ITK's scan-line
  path) and a linear or nearest-neighbour interpolator; **points beyond the input get the
  input's value at index `(1, 1, …)`**, before smoothing (line 190).
- **Argument quirks:**
  - in 2D the `nn` flag is read from `argv[7]`, the `addvox` argument, whenever an eighth
    argument exists (line 130): `2 in out 1 1 0 1 0` resamples by nearest neighbour;
  - in 4D `nn` is read from `argv[10]` as soon as `argc > 9` (line 393): an `addvox` without
    an `nn` argument aborts (`std::stoi` of a null pointer);
  - with smoothing (the default), every axis's sigma reads its spacing argument, so all must
    be given: `3 in out 2 2` crashes on `atof(nullptr)`;
  - a dimension other than 2, 3 or 4 does nothing and exits 0.
  *Read; validated* (each has a case).
- It prints the input spacing, the new spacing, each smoothing sigma (`float`, six digits) and
  the output size, with ITK's `[a, b, c]` vector format. *Validated* (stdout compared on
  every passing case).
- 4D linear interpolation is ITK's `EvaluateUnoptimized` (the weighted sum of the 16
  neighbours, clamped); 2D and 3D use the optimised paths, which skip an axis whose
  fractional distance is 0. *Read; validated* (`dim4-linear-time`).

## ResampleImage (`Examples/ResampleImage.cxx`)

- **The seventh argument is read twice** (lines 331-334 and 144-236): `std::stoi(argv[7])` is
  the pixel type (0 `char` … 7 `double`; anything else prints "Unsupported pixel type"), and
  the same text is the interpolator's parameter: the Gaussian's sigmas
  (`ConvertVector<double>`), the windowed sinc's window (its first character) or the B-spline
  order (if 0 to 5, else 3). So `… 2 1.5x1.5x1.5` resamples an `unsigned char` image,
  `… 4 5` uses order 5 on `unsigned int` pixels, `… 4 6` order 3 on `float`, and
  `… 3 l` aborts (`std::stoi("l")` throws before the window is read): only the default
  Hamming window is reachable. *Read; validated* (`interpolation/*`).
- **`b` ("blackman") is a second Lanczos**: `sb_interpolator` is declared with the Lanczos
  window (line 76). Unreachable anyway (above). *Read.*
- **The windowed sinc uses ITK's default boundary condition**, `ZeroFluxNeumannBoundaryCondition`
  (the nearest edge voxel outside the image; `WindowedSincInterpolateImageFunction<ImageType, 3>`
  with default template arguments), while `antsApplyTransforms` passes
  `ConstantBoundaryCondition` (zero outside). *Read; validated* (`interpolation/sinc*`,
  `real/boldref-sinc`). *larmorx:* `Interpolation::WindowedSincEdge`.
- **By spacing** the size is `static_cast<int>(old spacing · old size / spacing + 0.5)`
  (line 110); **by size** the spacing is `old · (old size − 1) / (size − 1)` (line 136), so a
  size of 1 divides by zero: ANTs writes a 1 × 1 × 1 image of value 0 with infinite spacing
  after vnl's SVD fails on the grid matrix (exit 0). *Verified.* *larmorx:* refuses sizes
  below 2 (expected divergence `errors/size-1`).
- A spacing list of the wrong length prints "Invalid spacing." and leaves the spacing
  uninitialised (ANTs then aborts in the resampler); a spacing of 0 is refused by ITK.
  *Verified.*
- **Pixel types.** The image is read as the pixel type (`ReadImage<itk::Image<T, D>>`: a C++
  `static_cast` of each value; for `unsigned int` GCC converts through a 64-bit integer and
  keeps the low 32 bits, so −1 becomes 4294967295), interpolated in double, and written by
  `CastPixelWithBoundsChecking` (`itkResampleImageFilter.hxx:297-317`): clamped to the
  type's range, then a `static_cast` (truncation toward zero). For `double` pixels the value
  is returned unchanged (the overload for an unchanged component type, line 289). Points
  outside the input get 0. *Read; validated* (`pixel-type/*`, with wrapping negative and hot
  voxels).
- The interpolation number is `std::stoi(argv[6])`; numbers other than 1 to 4 (0, 5, …) are
  linear. *Read; validated.*
