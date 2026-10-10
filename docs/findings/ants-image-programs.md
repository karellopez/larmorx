# ANTs image programs: ImageMath, ThresholdImage, MultiplyImages (ANTs v2.6.5)

What these programs really do, found while porting them (milestone A2). Line numbers are at
the tags `ANTs-v2.6.5` and `ITK-v5.4.5`. The parity records are
[ants-image-math](../validation/ants-image-math.md),
[ants-threshold-image](../validation/ants-threshold-image.md) and
[ants-multiply-images](../validation/ants-multiply-images.md).

## How the programs read and write images

**Images are read as `itk::Image<float, D>`** through ANTs' `ReadImage`
(`Utilities/ReadWriteData.h:198-264`). Values are converted from what the NIfTI reader produced
with a C++ `static_cast`; masks are often read as `int` (`TruncateImageIntensity`,
`ThresholdImage`), so a mask value of 0.999 is 0 and 1.7 is 1. *Read; validated*
(`truncate/fractional-mask`, `otsu/fractional-mask`).
*larmorx:* `larmorx_ants::image::Pixel` reproduces the casts (x86-64's truncation for `int`).

**Names shorter than 3 characters fail silently; missing files print
" file <name> does not exist . "** and leave a null image (`ReadWriteData.h:204, 227`). Most
operations then dereference it: **ImageMath crashes (SIGSEGV)** on a missing input.
*Verified.* *larmorx:* the same message, then exit 1.

**A file with more dimensions than `D` is read as its first `D`-dimensional block with an
identity direction** (`itkImageFileReader.hxx`, `GenerateOutputInformation`: it uses the
ImageIO's *default* direction when the file has more dimensions). Origin and spacing are the
file's. So `ImageMath 3 ... m bold.nii.gz 2` writes the first volume, axis-aligned.
*Read; validated* (`arithmetic/dim3-of-4d`). *larmorx:* reproduced in `image::from_itk`.

**`ANTs::WriteImage` writes nothing for names shorter than 3 characters**
(`ReadWriteData.h:526`), and ImageMath's arithmetic writes only names longer than 3
(`ImageMath_Templates.hxx:5755`). *Read.* *larmorx:* reproduced.

**ITK's NIfTI writer** is described in [itk-nifti.md](itk-nifti.md#writing). Outputs of
operations that change the image they read in place (`Neg`, `TruncateImageIntensity`,
`Normalize`) keep the input's `descrip` and `aux_file`; outputs of filters and of
`AllocImage` do not. *Verified; validated:* every output of the three suites has the same
348 header bytes as ANTs'.

## ImageMath (`Examples/ImageMath.cxx`, `ImageMath_Templates.hxx`)

**Dispatch.** Fewer than four arguments print the usage (exit 0 for `--help`/`-h`, else 1).
The dimension is read with `std::stoi` (text aborts; `3d` reads as 3). Only 2, 3 and 4
exist. Each dimension tries a sequence of tables (`ImageMathHelper3D.cxx`: all dimensions,
3D only, 2D or 3D, 3D or 4D); an unknown name prints " Operation <op> not found or not
supported for dimension <d>" and exits 1. Names are case-sensitive. *Read; validated.*

**A known operation always exits 0** (`ImageMath_Templates.hxx:14892-15435`): the tables
call the operation and return `EXIT_SUCCESS` whatever it returned (`Mattes` and
`ComponentToVector` excepted). An operation that prints an error and returns without
writing still exits 0. Where it crashes instead (null images, `throw std::exception()`,
uncaught ITK exceptions), the process dies with SIGSEGV or SIGABRT. *Read; verified.*
*larmorx:* exit 0 where ANTs returns normally, exit 1 with a message where ANTs crashes
(`cli::image_math::OpError`).

**Operands: `from_string<float>`** (`ImageMath_Templates.hxx:212`) decides whether the last
argument is a number or an image: `istringstream >> std::dec >> value`, then "is the stream
at its end?". So `2.nii` is an image, `7 ` (trailing space) is an image, `1e` is the number 0
(the stream is consumed but `strtof` fails), `-` is 0, `1e40` is `FLT_MAX`, and **no operand
at all is the number 1** (the stream fails before touching the value, which starts at 1).
*Read (libstdc++'s `num_get::_M_extract_float`); validated* (`arithmetic/operand-*`).
*larmorx:* `cli::cstd::from_string_f32`.

**`ImageMath<DIM>`** (`m + - / ^ exp max abs addtozero overadd Decision total mean vtotal`,
`ImageMath_Templates.hxx:5583-5762`) computes voxel by voxel in float, in image order, into a
result variable that is not reset between voxels:
- **`/` divides only where the divisor is positive**; elsewhere the voxel gets the previous
  voxel's result (0 before the first division). *Read; validated* (`arithmetic//-*`).
- **`total` and `mean` write the running sum into every voxel** and print
  `total: <sum> total-volume: <sum × voxel volume>` or `<sum / count>` with `std::cout`'s
  six significant digits. *Validated* (stdout compared).
- **`vtotal` is dispatched to this function but has no branch**: the output is all zeros.
  *Read; validated.*
- **`exp` and `Decision` call the double-precision `exp`** and round to float, while
  **`^` calls `powf`** (`std::pow(float, float)`). *Verified* on 262,144 voxels: `exp` matches
  `(float)exp((double)x)` exactly and differs from glibc's `expf` in 164 of them; `^`
  matches glibc's `powf` exactly. *larmorx:* `exp` uses `larmorx_core::math::exp`;
  `^` uses `larmorx_core::math::powf` (CORE-MATH, correctly rounded), see
  [platform-math.md](platform-math.md#powf).
- **`max` is `std::max(a, b)`**: `b` if `a < b`, else `a`, so a NaN first operand wins.
- **`addtozero` and `overadd` test for zero with `itk::Math::FloatAlmostEqual`** (4 ulps or
  `|x| ≤ 0.1·FLT_EPSILON`): `1e-9` counts as zero. *Read; validated* (`operand-zeros` has
  ±1e-9 and −0.0).
- The second image is read **at the first image's voxel indices** with its own strides; its
  geometry is ignored. A larger image works; a smaller one reads out of bounds.
  *Read; validated* (`arithmetic/larger-operand`). *larmorx:* larger is reproduced, smaller
  is an error.

**`Neg`** (`NegativeImage`, `ImageMath_Templates.hxx:7495`) finds the range with
`if (v > max) max = v; else if (v < min) min = v;` from `max = −1e12`, `min = 1e12`: a voxel
that raises the maximum is never compared with the minimum. In an image whose values only
increase, the minimum stays 1e12 and the result (`max − v` algebraically) carries rounding
errors of about 1e-4; a constant image has `min = max`, replaced by 1 and 0, so `c` maps to
`1 − c`. It writes the image it read (keeping `descrip`). *Read; validated.*

**`TruncateImageIntensity`** (`ImageMath_Templates.hxx:966-1110`):
- defaults: lower quantile 0.025, upper `1 − lower` (in float), **64 bins** (the usage text
  says 65); the bin count is read with `std::stoi`;
- the histogram holds the voxels with a value `> 0` where the `int` mask is `≥ 0.5`, and not
  NaN or infinite (a missing mask file prints its message and is ignored);
- **its range is order-dependent**: `if (v < min) min = v; else if (v > max) max = v;` from
  `FLT_MAX` and `−FLT_MAX`, before non-finite voxels are excluded. If the first positive voxel
  in memory is the brightest, it only sets the minimum, the maximum becomes the next
  brightest, and the brightest voxel falls outside the histogram (`ClipBinsAtEnds`);
- the histogram is `LabelStatisticsImageFilter`'s (`Histogram::Initialize`: the bin width is
  computed in **float**, bounds as `lower + float(j · width)`, the last bin ends at `upper`),
  the quantiles `Histogram::Quantile` (linear within the bin, walking from the low end below
  0.5 and from the high end above);
- every voxel, in the mask or not, is clipped; the image read is written back.
*Read; validated* (`truncate/*`, including `first-voxel-max` and fMRIPrep's
`0.01 0.999 256` on a raw T1w).

**`Normalize`** (`NormalizeImage`, `ImageMath_Templates.hxx:9039`): the optional argument is
first read as a **float** mask image; if that fails, it is a number (`atof`). Range mode
starts from `max = 0`, `min = 1e9` (an image with no positive voxel keeps `max = 0`); the mean
is a float sum in image order; the masked mean counts non-zero mask voxels in a float
counter. *Read; validated.*

**`RescaleImage`** is ITK's `RescaleIntensityImageFilter` (`MinimumMaximumImageCalculator`,
scale in double, shift `out_min − in_min · scale`, result clamped). Fewer than three operands
`throw std::exception()` (abort); a minimum above the maximum makes ITK throw (abort).
A constant non-zero image gets `scale = range / value`, so every voxel maps to `out_min`.
*Read; validated.*

**2D and 4D.** The voxel-wise operations of this group work for `ImageMath 2` and
`ImageMath 4` unchanged (the store converts files to `D` dimensions as ITK does); they are
validated in 2D and 4D. Spatial operations need more:
- ITK's neighbourhoods and structuring elements are `D`-dimensional, so `ImageMath 4 MD`
  dilates across time too, and a 2D image is not a 3D image with one slice (a 3D ball
  reaches outside a one-slice image);
- `larmorx_image::Volume` is 3D: spatial filters need a `D`-generic size and spacing (or
  separate 2D and 4D paths), and the boundary conditions of ITK's iterators in each
  dimension.

## ThresholdImage (`Examples/ThresholdImage.cxx`)

- **The sixth argument is always read as a mask image first** (`ThresholdImage.cxx:431`),
  even for a range threshold: `... 0.5 1 255 0` prints " file 255 does not exist . " (names
  of 3 or more characters), then uses 255 as the inside value. *Verified.*
- **A mode other than exactly `Otsu` or `Kmeans` is a range threshold** with bounds read by
  `atof` and stored as float: `otsu` thresholds at `[0, atof(argv[5])]`; `nan`, `inf` and
  `0x40` are accepted (glibc's `strtod`). *Validated.*
- Range thresholds are ITK's `BinaryThresholdImageFilter` (inclusive, compared in float;
  NaN is outside; lower above upper aborts). Inside defaults to 1, outside to 0.
- **Otsu without a mask** is ITK's `OtsuMultipleThresholdsImageFilter`: a 128-bin histogram
  from `ScalarImageToHistogramGenerator` (bounds from the data, the upper one raised by
  `(max − min) / 128 / 100`), the exhaustive search of `OtsuMultipleThresholdsCalculator`
  (a new maximum must exceed the old one by more than one ulp, so the first of equal
  configurations wins), thresholds at the **upper bounds** of the bins (ITK 5 without
  ITKv4 compatibility; v4 used the bin middles), labels `0..n` from
  `ThresholdLabelerImageFilter` (`v ≤ t`, compared in double).
- **Otsu with a mask is ANTs' own code** (`ThresholdImage.cxx:152-262`, the maximum at line 186): only mask value
  exactly 1 is the region; a 200-bin `LabelStatisticsImageFilter` histogram whose range comes
  from a loop that starts the maximum at **`NumericTraits<float>::min()` = `FLT_MIN`** (the
  smallest positive float) and updates it only when a voxel does not lower the minimum;
  labels `1..n+1` inside (compared in float, `v < t`), 0 outside; the output takes the
  **mask's geometry**. *Read; validated.*
- **`Otsu 0` hangs ANTs**: with no threshold, `IncrementThresholds` never returns false.
  *Read* (not run). *larmorx:* an error.
- `Kmeans` (`KdTreeBasedKmeansEstimator`) is not ported yet; larmorx says so.

## MultiplyImages (`Examples/MultiplyImages.cxx`)

- Pixels are `itk::Vector<float, N>` (N from the first file's components); a scalar file
  gives `N = 1` and an ordinary float output.
- **If the second argument cannot be read as an image it is a number, `atof(argv[3])`**
  (`MultiplyImages.cxx:80-86`), so
  a missing or misspelt file multiplies by 0 without a message. *Verified.* *larmorx:* the
  command line reproduces it; `lx.ants.multiply_images` raises `FileNotFoundError` instead.
- The product is computed at the first image's indices (geometry of the second ignored).
- Without an output name it prints "missing output filename" (line 53) and aborts (`throw;` with no
  active exception). A missing first file crashes before anything is printed
  (`CreateImageIO` returns null). Dimension 1 is accepted by ANTs; larmorx supports 2 to 4.
