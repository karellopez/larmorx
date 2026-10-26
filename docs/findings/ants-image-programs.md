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
starts from `max = 0`, `min = 1e9` (an image with no positive voxel keeps `max = 0`, one whose
values all exceed 1e9 keeps `min = 1e9`; `ImageMath_Templates.hxx:9099-9118`); the mean
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
- `larmorx_image::Volume` was 3D: spatial filters need a `D`-generic size and spacing (or
  separate 2D and 4D paths), and the boundary conditions of ITK's iterators in each
  dimension. *Since the Gaussian group (2026-10-10):* `Volume` and `VolumeRef` have the
  image's own number of dimensions, `AntsImage::view()` gives one, and the Gaussian and median
  filters work in 2, 3 and 4 dimensions as ITK does
  ([ants-gaussian-filters.md](ants-gaussian-filters.md)).

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

## Found by differential random testing (2026-10-25)

- **The median's zero.** `MedianImageFilter` takes the element `std::nth_element` leaves in the
  middle; `-0.0` and `+0.0` compare equal, so which one ITK writes depends on libstdc++'s
  introselect. larmorx selected with `total_cmp` (`ants-smooth-image:20261025:564`: one voxel
  `+0` against ANTs' `-0`). Where the median is a zero of a box holding both, larmorx now selects
  again with GCC 14.3's algorithm, emulated statement by statement
  (`larmorx_image::libstdcxx::nth_element`), on ITK's box order (permanent case
  `difftest/median-zero-sign`).
- **ITK-95 at a larger radius.** ITK's moving-histogram morphology on an image whose first axis
  has one voxel is wrong above radius 0 too: `ImageMath 4 out GD a 3` on a 1x7x4x6 image writes
  99 at two voxels where the maximum over ITK's ball (offsets within 3.5 voxels) is 97
  (`ants-image-math:20261025:1541`, computed by brute force over the ball); larmorx gives the
  true value in both modes (`reproduce` does not yet give ITK's).
- **ANTS-163:** ImageMath `Normalize` and `TruncateImageIntensity` with a mask of another size
  read it outside its buffer; larmorx refuses (as before), now naming the defect.
- **ANTS-161:** ResampleImage's refusal of a Gaussian sigma or alpha of 0 or less now names the
  defect.
- **ANTS-162:** ResampleImage's and ResampleImageBySpacing's output grid is checked against the
  machine's memory before it is allocated (7.5 bytes per output voxel, the most it was
  measured to need); see [ants-n4.md](ants-n4.md), "Memory".

## Memory against ANTs (2026-10-25)

The user's rule (2026-10-25): larmorx never needs more memory than the original program for the
same call. Measured with `validation/investigations/difftest/memory_pair.py`: ANTs 2.6.5 and
larmorx's command (`python -m larmorx ants <program> ...`, what the `larmorx` console script
runs), each process's peak resident memory (`/usr/bin/time -v`), one thread, larmorx in both D13
modes; `rss_timeline.py` shows a run's memory over time, `scaled_memory.py` a heavy random case
at smaller scales (bytes per output voxel). `tests/parity/test_memory_peak.py` holds larmorx to
ANTs' peak plus 8 MiB (the Python interpreter; a call on 16 voxels needs 20 MiB in larmorx, 18
in ANTs) on 13 of the calls below (every program, and each operation that needed more), and
checks larmorx's output bit for bit in `reproduce`.

| Call (float input; one thread) | Output voxels | ANTs | larmorx before | larmorx |
|---|---|---|---|---|
| `ResampleImageBySpacing 3 in out 0.5 0.5 0.5` (128×128×64 at 2 mm) | 67 M | 279 MiB | 2.2 times | 283 MiB |
| `ResampleImageBySpacing 3 in out 0.4 0.4 0.4` (the same) | 131 M | 523 MiB | 2.2 times | 527 MiB |
| `ResampleImage 3 in out 0.4x0.4x0.4 0 0` (the same) | 131 M | 534 MiB | | 527 MiB |
| `ResampleImage 3 in out 1x1x1 0 4` (B-spline) | 8.4 M | 65 MiB | | 65 MiB |
| `SmoothImage 3 in 1.5 out` (256×256×128) | 8.4 M | 83 MiB | 90 MiB | 88 MiB |
| `ThresholdImage 3 in out 20 60` (the same) | 8.4 M | 83 MiB | 90 MiB | 88 MiB |
| `MultiplyImages 3 in 2.5 out` (the same) | 8.4 M | 116 MiB | 89 MiB | 88 MiB |
| `ImageMath 3 out G in 1.5` (the same) | 8.4 M | 121 MiB | 90 MiB | 89 MiB |
| `ImageMath 3 out m in in` (the same) | 8.4 M | 121 MiB | 121 MiB | 120 MiB |
| `ImageMath 3 out ME in 1` (the same) | 8.4 M | 97 MiB | 150 MiB | 94 MiB |
| `ImageMath 3 out MD in 1` (the same) | 8.4 M | 97 MiB | 101 MiB | 94 MiB |
| `ImageMath 3 out GD in 1` (the same) | 8.4 M | 88 MiB | 181 MiB | 85 MiB |
| `ImageMath 3 out GE in 1` (the same) | 8.4 M | 89 MiB | 213 MiB | 85 MiB |
| `ImageMath 3 out GO in 1`, `GC in 2` (the same) | 8.4 M | 187, 189 MiB | | 87, 89 MiB |
| `ImageMath 3 out MO in 1`, `MC in 1` (the same) | 8.4 M | 129, 163 MiB | | 125, 158 MiB |
| `ImageMath 3 out TruncateImageIntensity in 0.01 0.999 256` (the same) | 8.4 M | 89 MiB | 117 MiB | 85 MiB |
| `ImageMath 3 out GetLargestComponent in`, `FillHoles in 2` (the same) | 8.4 M | 250, 475 MiB | | 125, 134 MiB |
| `ImageMath 3 out Normalize in`, `PadImage in 10` (the same) | 8.4 M | 88, 99 MiB | | 84, 91 MiB |
| `N4BiasFieldCorrection -d 3 -i in -o out -s 2 -c [20x20,0]` (128×128×96 at 1.5 mm) | 1.6 M | 79 MiB | 104 MiB | 41 MiB |
| `DenoiseImage -d 3 -i in -o out` (96×96×64 at 2 mm) | 0.6 M | 40 MiB | 85 MiB | 44 MiB |
| `ThresholdImage 3 in out 20 60` (4×4×1: the interpreter) | 16 | 18 MiB | 32 MiB | 20 MiB |

What grows with the image costs larmorx no more than ANTs: per output voxel of the resamplers,
4.0 bytes for float outputs and 1.0 for `unsigned char` ones in both (fitted on
`ants-resample-image:20261025:556` at 94 to 756 M voxels: ANTs 1.003, larmorx 1.004), and 7.2
for a 2D ResampleImageBySpacing in `reproduce` as in ANTs (4.0 in larmorx's default;
`ants-resample-image-by-spacing:20261025:137`). The rest is a constant of a few MiB. What was
found and changed, each with the same output bit for bit:

- **ResampleImageBySpacing resampled twice** in the default, the second time with ANTs' outside
  value, to count exactly the voxels ANTS-32's notice names: 2.2 times ANTs' memory (16 bytes
  per output voxel against 7.3 on the heavy random cases). The voxels beyond the input are now
  counted in the one pass (`larmorx_ants::resample::resample_identity_outside`), the same count.
- **The gzip writer** copied a whole image into its input batch before compressing; it now
  holds two blocks per worker (at most 64; `larmorx_io::gzip`), the same bytes.
- **The command line imported NumPy** and every subpackage at start (`import larmorx`): 31.9 MiB
  before any work. The package now loads its subpackages and names on first use (PEP 562): 15.6
  MiB, ANTs' programs 18.
- **The line operations** (`larmorx_image::lines`: `filter_lines`, SmoothImage and the
  recursive and discrete Gaussians; `for_each_line`, morphology and distance maps) held one task
  reference per 32 voxels of a row along a strided axis (half a byte per voxel); now one per 256,
  32 lines at a time as before.
- **The resamplers' guard (ANTS-162)** counted 16 bytes per output voxel, the old cost; it now
  counts 7.5, the most measured, so a call that fits is no longer refused.
- **Grayscale morphology** (GD, GE, GO, GC) copied its keys (the values as integers in
  `total_cmp` order), widened its runs into a second buffer, made the complements for an
  erosion and the values at the end as new images: up to five images where ITK holds two. It
  now widens the runs in place on the keys it is given, the complements and values in place,
  and the command line hands it the image's own buffer
  (`larmorx_image::morphology::grayscale_dilate_owned` and the others;
  `larmorx_ants::image_math::grayscale_morphological_owned`).
- **ME** made both ANTs' thresholded image and the corrected one to count ANTS-17's voxels; it
  now counts them and writes the mode's values in one pass. The binary filters free their sites
  once the distances are made (MD and the others too).
- **TruncateImageIntensity** made a label image (4 bytes per voxel) for the histogram of the
  voxels it counts; they are decided where they are counted now
  (`larmorx_image::statistics::histogram_where`, the same histogram).
- **The gzip writer** holds one block per batch on one thread (two per worker otherwise) behind
  a 64 KiB buffer; the plain writer keeps its 1 MiB buffer.

The comparison of the random tests' heavy cases held both outputs (and float64 copies where
values differ) in the process that had run larmorx: `ants-resample-image:20261025:556` and
`ants-resample-image-by-spacing:20261025:938`, outputs of 6·10⁹ and 3.8·10⁹ voxels, were stopped
at 24 GiB and 48 GiB of swap by the harness, not by the programs. Outputs of more than 2²⁸ values
are now compared from their files, a chunk at a time
(`larmorx_validation.parity.ants_programs.StoredValues`), with the same numbers; the two cases
wait for a larger machine with the night's other heavy cases.

N4BiasFieldCorrection and DenoiseImage: [ants-n4.md](ants-n4.md) and
[ants-denoise.md](ants-denoise.md), "Memory against ANTs".

## Memory on the data panel's largest inputs (2026-10-26)

Each tool's largest call on the data panel, ANTs 2.6.5 (with CORE-MATH preloaded where its record
compares so) and larmorx's command at one thread, every output compared byte for byte
(`validation/investigations/panel-ants/memory_largest.py`; the 7 T MP2RAGE is 224 × 272 × 288,
ds004958's multiband-8 series 600 volumes, ds002603's multiband-4 series 588 volumes of
80 × 80 × 36):

| Call | ANTs | larmorx before | larmorx | ANTs s | larmorx s |
|---|---|---|---|---|---|
| ImageMath `TruncateImageIntensity`, 7 T | 190 MiB | | 154 MiB | 3.6 | 2.5 |
| ImageMath `m`, the 600-volume series | 3589 MiB | | 3588 MiB | 80.5 | 34.8 |
| ThresholdImage Otsu 4, 7 T | 186 MiB | | 158 MiB | 2.9 | 1.8 |
| MultiplyImages, the 588-volume series | 1570 MiB | | 1057 MiB | 13.6 | 8.4 |
| SmoothImage `2x2x2x0.55`, the 588-volume series | 1052 MiB | 1081 MiB | 1058 MiB | 26.1 | 17.7 |
| ResampleImage to 1 mm, B-spline, 7 T | 256 MiB | 384 MiB | 257 MiB | 4.7 | 3.6 |
| ResampleImageBySpacing to 2 mm, 7 T | 219 MiB | | 222 MiB | 2.3 | 1.0 |
| N4BiasFieldCorrection, sMRIPrep's final options, 7 T | 1814 MiB | | 814 MiB | 232.8 | 38.4 |
| DenoiseImage `-n Rician`, 7 T | 689 MiB | | 682 MiB | 273.7 | 37.0 |
| Atropos, sMRIPrep's call on ds005454 (7 T) | 1275 MiB | | 565 MiB | 42.9 | 17.3 |
| antsApplyTransforms `-e 3`, Lanczos, the 600-volume series to the T1w | 6501 MiB | | 4710 MiB | 850.7 | 399.7 |
| antsApplyTransforms, the 7 T MP2RAGE to MNI (fMRIPrep's `.h5`) | 722 MiB | | 536 MiB | 41.0 | 17.7 |
| antsRegistration, sMRIPrep's brain extraction, 7 T, whole | 2884 MiB | | 2394 MiB | 2431 | 1165 |
| antsRegistration, the same on every third voxel | 149 MiB | | 113 MiB | 83.7 | 33.5 |
| antsRegistration, the template registration on every third voxel, 7 T | 136 MiB | | 116 MiB | 432.1 | 94.6 |
| antsAI, the BOLD reference's `init_aff`, 7 T | 117 MiB | | 156 MiB | 157.8 | 287.3 |
| antsAffineInitializer, niworkflows' call, 7 T | 688 MiB | | 844 MiB | 331.8 | 1211.6 |

- **ResampleImage's B-spline coefficients** held their z columns all at once, a second copy of
  the volume (128 MiB more than ITK on the 7 T head); they are filtered a block of 4096 columns
  at a time now (`larmorx_interp`, the 3D and n-D decompositions).
- **The separable filters' strided axes** listed a reference per row piece for the whole image
  before filtering: where rows are shorter than a task's group (the y axis of an 80-voxel-wide
  series), 0.2 byte per voxel, 27 MB on the 588-volume series; the tasks are made and run a
  batch at a time now (`larmorx_image::lines`).
- **antsAI and antsAffineInitializer stay above ANTs** (their metric holds the images in double
  where ANTs holds them in float: `ants-ai.md`, "Memory and speed against ANTs"), a follow-up.
- New cases in `tests/parity/test_memory_peak.py`: ResampleImage B-spline of a 192 × 224 × 160
  input, SmoothImage of a 16 × 1024 × 1024 image. Every output above is bit-identical in
  `reproduce`; the golden checksums are unchanged.
