# ImageMath

`lx.ants.image_math` (the command line on in-memory images) and typed wrappers of its
operations: `lx.ants.image_arithmetic`, `add_to_zero`, `negative_image`,
`truncate_image_intensity`, `normalize_image`, `rescale_image`, `discrete_gaussian`,
`laplacian`, `gradient_magnitude`, `unsharp_mask`. Command line
`larmorx ants ImageMath`. Rust: `larmorx_ants::image_math` (operations),
`larmorx_ants::cli::image_math` (the dispatcher), `larmorx_image` (ITK filters).
A replica of ANTs 2.6.5 on ITK 5.4.5 (Apache-2.0).

**Status: `validated`** for the operations below, against ImageMath itself (see the
[validation record](../validation/ants-image-math.md)): 162 cases, 145 outputs compared,
142 bit-identical; every output has ANTs' exact header bytes. The 3 others are `^`, where
ANTs calls glibc's `powf`, which is not correctly rounded; larmorx's is, so up to 35 of
35,840 voxels differ by one ulp ([why](../findings/platform-math.md#powf)). Every Gaussian
case (`G`, `Laplacian`, `Grad`, `UnsharpMask`; 47 compared, in 2, 3 and 4 dimensions,
fMRIPrep's `Laplacian 1.5 1` on real T1w, boldref and template images included) is
bit-identical.

**Speed** ([benchmark](../benchmarks/ants-programs.md)): fMRIPrep's
`TruncateImageIntensity 0.01 0.999 256` on a raw T1w takes 234 ms on one thread and 83 ms on
12, against 388 ms and 175 ms for ANTs; `addtozero`, `Normalize`, `RescaleImage` and
`ImageMath 4 m` are 1.3–2.7× faster on one thread and up to 5× with threads. Reading the
gzipped input is most of the time. The Gaussian operations
([benchmark](../benchmarks/ants-gaussian.md)): fMRIPrep's `Laplacian 1.5 1` on a raw T1w takes
350 ms on one thread and 131 ms on 12, against 1.23 s and 269 ms for ANTs (7 ms against 48 ms
on a boldref); `Grad 1` 309 ms against 1.20 s; `G 2` 125 ms against 738 ms.

**Implemented operations** (the others answer "not supported yet"):

| Group | Operations |
|---|---|
| arithmetic | `m`, `+`, `-`, `/`, `^`, `exp`, `max`, `abs`, `addtozero`, `overadd`, `Decision`, `total`, `mean`, `vtotal`, `Neg` |
| intensity | `TruncateImageIntensity`, `Normalize`, `RescaleImage` |
| gaussian | `G`, `Laplacian`, `Grad`, `UnsharpMask` |

All of them run in 2, 3 and 4 dimensions, as in ANTs; the Gaussian operations filter in all
of the image's dimensions (a 4D image along time too).

## Quick start

```python
import larmorx as lx

# fMRIPrep's intensity clipping of the T1w before registration
t1 = lx.ants.truncate_image_intensity("sub-01_T1w.nii.gz", 0.01, 0.999, 256)

# The command line, on in-memory images (exactly the command-line code)
t1 = lx.ants.image_math("TruncateImageIntensity", "sub-01_T1w.nii.gz", 0.01, 0.999, 256)
both = lx.ants.add_to_zero(wm_mask, gm_mask)

# The Laplacian feature channel of fMRIPrep's brain extraction (ImageMath 3 out Laplacian in 1.5 1)
lap = lx.ants.laplacian(t1, 1.5, normalize=True)
```

```bash
larmorx ants ImageMath 3 t1_trunc.nii.gz TruncateImageIntensity sub-01_T1w.nii.gz 0.01 0.999 256
```

## `lx.ants.image_math(operation, *operands, dimension=3, n_threads=1)`

Runs `ImageMath <dimension> <output> <operation> <operands...>` with images held in memory:
`lx.Image` objects, nibabel images and `(array, affine)` pairs are passed without files;
paths, numbers and strings are passed as on the command line (numbers as their shortest
round-trip text). Returns the image the operation writes, in the type and geometry ANTs
gives it. Raises `lx.ants.ImageMathError` with the program's messages when it fails or writes
nothing. `lx.ants.image_math_operations()` lists the implemented operations.

In-memory images are placed by their affine (the spatial axes) and their header's
repetition time (a fourth axis; spacing 1 without a header); files are read as ITK reads
them.

## Typed functions

Each takes an image (path, `lx.Image`, nibabel image or `(array, affine)`), returns an
`lx.Image` with float32 voxels and the input's affine and header, and has an explicit
`n_threads` (the result does not depend on it). Paths are read as ITK reads them; arrays are
converted to float32 as a C++ `static_cast` converts them.

| ImageMath | Python | Notes |
|---|---|---|
| `m`, `+`, `-`, `/`, `^`, `exp`, `max`, `abs`, `addtozero`, `overadd`, `Decision`, `total`, `mean` | `image_arithmetic(image, operation, operand=1.0)` | `operation`: ImageMath's name or `multiply`, `add`, `subtract`, `divide`, `power`, `exp`, `max`, `abs`, `addtozero`, `overadd`, `decision`, `total`, `mean`. `operand`: a number or an image (read at `image`'s voxel indices) |
| `addtozero a b` | `add_to_zero(a, b)` | |
| `Neg a` | `negative_image(a)` | |
| `TruncateImageIntensity a lo hi bins mask` | `truncate_image_intensity(a, lower_quantile=0.025, upper_quantile=None, bins=64, mask=None)` | `upper_quantile` defaults to `1 - lower_quantile` |
| `Normalize a` / `Normalize a 1` / `Normalize a mask` | `normalize_image(a)` / `normalize_image(a, by_mean=True)` / `normalize_image(a, mask)` | |
| `RescaleImage a min max` | `rescale_image(a, minimum, maximum)` | |
| `G a sigma` | `discrete_gaussian(a, sigma)` | `DiscreteGaussianImageFilter`, variance `sigma²` (in float) in mm², maximum error 0.01, kernels of at most 32 voxels each side; `sigma`: one or one per axis |
| `Laplacian a sigma` | `laplacian(a, sigma=1.0, normalize=False)` | `LaplacianRecursiveGaussianImageFilter`, sigma in mm; ANTs reads `normalize` from the sigma argument itself (`stoi("1.5")` is 1), so fMRIPrep's `Laplacian a 1.5 1` is `laplacian(a, 1.5, normalize=True)` |
| `Grad a sigma normalize` | `gradient_magnitude(a, sigma=1.0, normalize=False)` | `GradientMagnitudeRecursiveGaussianImageFilter`, sigma in mm |
| `UnsharpMask a amount radius threshold physical` | `unsharp_mask(a, amount=0.5, radius=1.0, threshold=0.0, radius_in_physical_units=False)` | `UnsharpMaskImageFilter` in float; `radius` is the Gaussian's sigma |

## Command-line behaviour (as in ANTs)

| Situation | ANTs | larmorx |
|---|---|---|
| fewer than 4 arguments | usage, exit 1 (`--help`, `-h`: exit 0) | same |
| dimension not 2, 3 or 4 | " Dimension d is not supported ", exit 1 | same |
| dimension not a number | aborts (`std::stoi` throws) | message, exit 1 |
| unknown operation (case-sensitive) | " Operation op not found or not supported for dimension d", exit 1 | same |
| operation ANTs has but larmorx not yet | – | "not supported yet", exit 1 |
| missing input file | " file f does not exist . ", then a crash (SIGSEGV) | the message, then exit 1 |
| an operation that reports an error and returns | exit 0 | exit 0 |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` | threads | threads (results do not depend on it) |

## Behaviour worth knowing

All reproduced from ANTs; details and sources in
[findings/ants-image-programs.md](../findings/ants-image-programs.md):
- **Operands** are numbers if `istringstream >> float` reaches the end: no operand at all
  is the number 1, `1e` is 0, `2.nii` is a file.
- **`/`** divides only where the divisor is positive; elsewhere the voxel keeps the previous
  voxel's value.
- **`total` and `mean`** put the running sum in every voxel and print the total (and
  total × voxel volume) or the mean; **`vtotal`** writes zeros.
- **`addtozero` / `overadd`** treat values within `FloatAlmostEqual` of 0 (like 1e-9) as 0.
- **`TruncateImageIntensity`** counts positive, finite voxels inside the mask (read as
  `int`, so ≥ 1), with a histogram range that depends on voxel order, and clips every voxel.
- **Non-finite input values are read as 0** (ITK's NIfTI reader), so NaN never reaches an
  operation.
- `Neg`, `TruncateImageIntensity` and `Normalize` keep the input's `descrip` in the output
  header (they write the image they read); the others do not.
- **Gaussian operations** ([findings](../findings/ants-gaussian-filters.md)): the recursive
  filters need at least 4 voxels along every axis (ITK throws; larmorx exits 1) and treat
  the image edges as extending to infinity; a sigma ≤ 0 becomes 0.5 for `Laplacian` and
  `Grad`; `G` without a valid sigma prints "Incorrect sigma vector size" and returns the
  image unchanged, and `G` with a variance above about 709 voxels² writes NaN, as ANTs
  does.

## Deliberate differences

- `^` is correctly rounded (ANTs: glibc's `powf`, one ulp off in about 0.06 % of calls).
- Where ANTs crashes, larmorx exits 1 with a message. A second image smaller than the first
  is an error (ANTs reads out of bounds).
- The coefficients of the recursive Gaussians use correctly rounded `sin`, `cos` and `exp`;
  ANTs uses glibc's, which differ in the last bit for about 0.6 % of sigmas. That has not
  changed a single output value: each pass rounds to float.
- Images are NIfTI only (ANTs reads every format ITK knows).
