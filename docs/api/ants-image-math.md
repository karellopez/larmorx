# ImageMath

`lx.ants.image_math` (the command line on in-memory images) and typed wrappers of its
operations: `lx.ants.image_arithmetic`, `add_to_zero`, `negative_image`,
`truncate_image_intensity`, `normalize_image`, `rescale_image`, `discrete_gaussian`,
`laplacian`, `gradient_magnitude`, `unsharp_mask`, `morphology`, `morphological_dilate`,
`morphological_erode`, `morphological_open`, `morphological_close`, `grayscale_dilate`,
`grayscale_erode`, `grayscale_open`, `grayscale_close`, `fill_holes`, `pad_image`,
`largest_component`, `distance_map`, `maurer_distance`, `extract_contours`,
`threshold_at_mean`, `replace_voxel_value`. Command line
`larmorx ants ImageMath`. Rust: `larmorx_ants::image_math` (operations),
`larmorx_ants::cli::image_math` (the dispatcher), `larmorx_image` (ITK filters).
A replica of ANTs 2.6.5 on ITK 5.4.5 (Apache-2.0).

**Status: `validated`** for the operations below, against ImageMath itself (see the
[validation record](../validation/ants-image-math.md)): 327 cases, 301 outputs compared,
296 bit-identical; every output has ANTs' exact header bytes. Of the 5 others, 3 are `^`,
where ANTs calls glibc's `powf`, which is not correctly rounded; larmorx's is, so up to 35
of 35,840 voxels differ by one ulp ([why](../findings/platform-math.md#powf)); 2 are
grayscale morphology on an image made to hold `-0.0` next to `+0.0`, where only the sign of
a zero result differs ([why](../findings/ants-morphology.md#grayscale-morphology)). Every
Gaussian case (`G`, `Laplacian`, `Grad`, `UnsharpMask`; 47 compared, in 2, 3 and 4
dimensions, fMRIPrep's `Laplacian 1.5 1` on real T1w, boldref and template images included)
is bit-identical, and so is every other morphology case (95 compared: `MD`, `ME`, `MO`,
`MC`, `GD`, `GE`, `GO`, `GC`, `FillHoles`, `PadImage` in 2, 3 and 4 dimensions, sMRIPrep's
`MD 2/5`, `ME 2/10`, `FillHoles 2` and `PadImage ±10` on real masks included), and
every component and distance-map case (59 compared: `GetLargestComponent`, `D`,
`MaurerDistance`, `ExtractContours`, `ThresholdAtMean`, `ReplaceVoxelValue`, in 2, 3 and
4 dimensions, on real masks and probability maps too). Compared with ANTs' defects reproduced
(`LARMORX_UPSTREAM_DEFECTS=reproduce`); the record's "D13 fix pass" lists what larmorx's
default changes. One more case, `FillHoles 0.5` with a hole on the first slice, compares
everything but the values, which ANTs takes from memory outside the image (ANTS-19).

**Speed** ([benchmark](../benchmarks/ants-programs.md)): fMRIPrep's
`TruncateImageIntensity 0.01 0.999 256` on a raw T1w takes 234 ms on one thread and 83 ms on
12, against 388 ms and 175 ms for ANTs; `addtozero`, `Normalize`, `RescaleImage` and
`ImageMath 4 m` are 1.3–2.7× faster on one thread and up to 5× with threads. Reading the
gzipped input is most of the time. The Gaussian operations
([benchmark](../benchmarks/ants-gaussian.md)): fMRIPrep's `Laplacian 1.5 1` on a raw T1w takes
351 ms on one thread and 128 ms on 12, against 1.23 s and 277 ms for ANTs (7 ms against 52 ms
on a boldref); `Grad 1` 306 ms against 1.23 s; `G 2` 119 ms against 716 ms. The morphology
operations ([benchmark](../benchmarks/ants-morphology.md)) on a 1 mm brain mask: `ME 10`
278 ms on one thread and 72 ms on 12, against 956 ms for ANTs (which does not gain from
threads here); `MD 2` 130 ms against 248 ms; `FillHoles 2` 68 ms against 2.91 s;
`GetLargestComponent` 55 ms against 240 ms; `D` 547 ms against 2.90 s (both sequential);
`MaurerDistance` 213 ms against 518 ms (58 ms against 115 ms on 12 threads); `GD 2` on a
raw T1w 170 ms against 1.84 s.

**Implemented operations** (the others answer "not supported yet"):

| Group | Operations |
|---|---|
| arithmetic | `m`, `+`, `-`, `/`, `^`, `exp`, `max`, `abs`, `addtozero`, `overadd`, `Decision`, `total`, `mean`, `vtotal`, `Neg` |
| intensity | `TruncateImageIntensity`, `Normalize`, `RescaleImage`, `ThresholdAtMean`, `ReplaceVoxelValue` |
| gaussian | `G`, `Laplacian`, `Grad`, `UnsharpMask` |
| morphology | `MD`, `ME`, `MO`, `MC`, `GD`, `GE`, `GO`, `GC`, `FillHoles`, `PadImage` |
| components | `GetLargestComponent`, `D`, `MaurerDistance`, `ExtractContours` |

All of them run in 2, 3 and 4 dimensions, as in ANTs; the Gaussian and morphology operations
work in all of the image's dimensions (a 4D image along time too).

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

# sMRIPrep's mask clean-up steps (ImageMath MD 2, ME 10, FillHoles 2, PadImage 10)
mask = lx.ants.morphological_dilate("brain_mask.nii.gz", 2)
csf = lx.ants.morphological_erode(csf_mask, 10)
filled = lx.ants.fill_holes(gm_mask, 2)
padded = lx.ants.pad_image(mask, 10)
largest = lx.ants.largest_component(mask)
dist = lx.ants.maurer_distance(mask)  # signed, mm, negative inside
```

```bash
larmorx ants ImageMath 3 t1_trunc.nii.gz TruncateImageIntensity sub-01_T1w.nii.gz 0.01 0.999 256
```

## `lx.ants.image_math(operation, *operands, dimension=3, n_threads=1, upstream_defects="fix")`

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
converted to float32 as a C++ `static_cast` converts them. The functions whose ANTs operation
has a defect (below, "Upstream defects") take `upstream_defects="fix"` or `"reproduce"`.

| ImageMath | Python | Notes |
|---|---|---|
| `m`, `+`, `-`, `/`, `^`, `exp`, `max`, `abs`, `addtozero`, `overadd`, `Decision`, `total`, `mean` | `image_arithmetic(image, operation, operand=1.0)` | `operation`: ImageMath's name or `multiply`, `add`, `subtract`, `divide`, `power`, `exp`, `max`, `abs`, `addtozero`, `overadd`, `decision`, `total`, `mean`. `operand`: a number or an image (read at `image`'s voxel indices) |
| `addtozero a b` | `add_to_zero(a, b)` | |
| `Neg a` | `negative_image(a)` | |
| `TruncateImageIntensity a lo hi bins mask` | `truncate_image_intensity(a, lower_quantile=0.025, upper_quantile=None, bins=64, mask=None)` | `upper_quantile` defaults to `1 - lower_quantile` |
| `Normalize a` / `Normalize a 1` / `Normalize a mask` | `normalize_image(a)` / `normalize_image(a, by_mean=True)` / `normalize_image(a, mask)` | |
| `RescaleImage a min max` | `rescale_image(a, minimum, maximum)` | |
| `G a sigma` | `discrete_gaussian(a, sigma)` | `DiscreteGaussianImageFilter`, variance `sigma²` (in float) in mm², maximum error 0.01, kernels of at most 32 voxels each side; `sigma`: one or one per axis |
| `Laplacian a sigma normalize` | `laplacian(a, sigma=1.0, normalize=False)` | `LaplacianRecursiveGaussianImageFilter`, sigma in mm; fMRIPrep's `Laplacian a 1.5 1` is `laplacian(a, 1.5, normalize=True)` (ANTs reads the flag from the sigma's argument, ANTS-16 below) |
| `Grad a sigma normalize` | `gradient_magnitude(a, sigma=1.0, normalize=False)` | `GradientMagnitudeRecursiveGaussianImageFilter`, sigma in mm |
| `UnsharpMask a amount radius threshold physical` | `unsharp_mask(a, amount=0.5, radius=1.0, threshold=0.0, radius_in_physical_units=False)` | `UnsharpMaskImageFilter` in float; `radius` is the Gaussian's sigma |
| `MD a r value` | `morphological_dilate(a, radius=1, value=1.0)` | ITK's `BinaryDilateImageFilter` with its ball (offsets with `Σ o² ≤ r(r+1)`, in voxels); `value` is the foreground |
| `ME a r value` | `morphological_erode(a, radius=1, value=1.0)` | `BinaryErodeImageFilter` (the border does not erode), then 1 where the eroded value and the input exceed 0.5, else 0 |
| `MO a r value`, `MC a r value` | `morphological_open(...)`, `morphological_close(...)` | the binary opening (eroded voxels set to 0) and closing (padded by `r` first; voxels that do not end as `value` keep their input) |
| `GD a r`, `GE a r`, `GO a r`, `GC a r` | `grayscale_dilate(a, radius=1)`, `grayscale_erode`, `grayscale_open`, `grayscale_close` | maximum / minimum over the ball; opening and closing pad by `r` |
| any of the eight | `morphology(a, operation, radius=1, value=1.0)` | `operation` is ImageMath's name |
| `FillHoles a p` | `fill_holes(a, hole_param=2.0)` | the background regions other than the largest set to 1 (all of them for 2; by their share of object neighbours for `p ≤ 1`) |
| `GetLargestComponent a n` | `largest_component(a, min_size=50)` | 1 in the largest face-connected component(s) of the voxels in `[0.25, 1e9]` (ties keep all of them), 0 elsewhere; components under `n` voxels are dropped first, and if none is left every voxel becomes 0 (ANTs: 1, ANTS-20) |
| `D a` | `distance_map(a)` | ITK's Danielsson map: the distance (mm) to the nearest non-zero voxel; sequential, as ITK's |
| `MaurerDistance a f` | `maurer_distance(a, foreground=1.0)` | ITK's signed Maurer map of the voxels equal to `f`: the distance (mm) to the object's inner contour, negative inside, `-0.0` on the contour |
| `ExtractContours a full` | `extract_contours(a, fully_connected=True)` | ITK's `LabelContourImageFilter` on the values truncated to integer labels |
| `ThresholdAtMean a f` | `threshold_at_mean(a, fraction=1.0)` | 1 where `mean·f ≤ v ≤ max` (float mean in image order) |
| `ReplaceVoxelValue a lo hi v` | `replace_voxel_value(a, low, high, value)` | voxels in `[lo, hi]` become `v` |
| `PadImage a p value` | `pad_image(a, pad, value=0.0)` | `p` voxels of `value` added on each side of every axis (removed if negative); the origin moves so voxels keep their positions; a fractional `p` grows by `int(2p)` and shifts by `int(p)` |

## Command-line behaviour (as in ANTs)

| Situation | ANTs | larmorx |
|---|---|---|
| fewer than 4 arguments | usage, exit 1 (`--help`, `-h`: exit 0) | same |
| dimension not 2, 3 or 4 | " Dimension d is not supported ", exit 1 | same |
| dimension not a number | aborts (`std::stoi` throws) | message, exit 1 |
| unknown operation (case-sensitive) | " Operation op not found or not supported for dimension d", exit 1 | same |
| operation ANTs has but larmorx not yet | – | "not supported yet", exit 1 |
| missing input file | " file f does not exist . ", then a crash (SIGSEGV) | the message, then exit 1 |
| an operation that reports an error and returns | exit 0 | exit 1 (ANTS-04; none of the ported operations reports one) |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` (or `NSLOTS`, as ITK reads them) | threads | threads (results do not depend on it) |

## Behaviour worth knowing

All from ANTs (the defects among them are listed under "Upstream defects" below); details
and sources in [findings/ants-image-programs.md](../findings/ants-image-programs.md):
- **Operands** are numbers if `istringstream >> float` reaches the end (`2.nii` is a file,
  `7 ` with a trailing blank too).
- **`total` and `mean`** print the total (and total × voxel volume) or the mean.
- **`addtozero` / `overadd`** treat values within `FloatAlmostEqual` of 0 (like 1e-9) as 0.
- **`TruncateImageIntensity`** counts positive, finite voxels inside the mask (read as
  `int`, so ≥ 1) and clips every voxel.
- **Non-finite input values are read as 0** (ITK's NIfTI reader), so NaN never reaches an
  operation.
- `Neg`, `TruncateImageIntensity` and `Normalize` keep the input's `descrip` in the output
  header (they write the image they read); the others do not.
- **Gaussian operations** ([findings](../findings/ants-gaussian-filters.md)): the recursive
  filters need at least 4 voxels along every axis (ITK throws; larmorx exits 1) and treat
  the image edges as extending to infinity; a sigma ≤ 0 becomes 0.5 for `Laplacian` and
  `Grad`.
- **Morphology** ([findings](../findings/ants-morphology.md)): radii are in voxels and
  truncated (`2.9` is 2; `-1` makes ANTs abort); `ME`'s output is 0/1 and keeps voxels that
  are above 0.5 but not the foreground (a label 2 stays 1); `MO r 0` is a dilation of the
  zeros (the opening's eroded voxels become 0, the foreground); the binary erosion does not
  erode at the image border. Their outputs and `PadImage`'s have no `descrip`; `FillHoles` writes
  the image it read (keeping it). `PadImage` needs its pad argument (ANTs crashes without
  it) and crashes ANTs when an axis would end with 0 or fewer voxels.
- **Components and distance maps** ([findings](../findings/ants-morphology.md)):
  `GetLargestComponent` keeps every component of the largest size; it writes the image it
  read (keeping its `descrip`). `D` is Danielsson's
  propagation, not an exact Euclidean transform, and an image without non-zero voxels gets
  distances near `2·max(size)` mm; `MaurerDistance` gives `∓1.8446743e19` when there is
  nothing to measure from. `ExtractContours` truncates values to integer labels (1.7 is 1,
  0.6 is 0). `ThresholdAtMean` with a fraction above
  `max/mean` and `ReplaceVoxelValue` with fewer than three numbers make ANTs abort or
  crash; larmorx exits 1.

## Upstream defects (D13)

ANTs' deterministic defects are corrected by default and reproduced bit for bit on request:
`upstream_defects="reproduce"` in Python (`image_math` and the typed functions),
`LARMORX_UPSTREAM_DEFECTS=reproduce` on the command line. A warning (on the command line a
line on standard error, `larmorx: upstream defect ANTS-08 (ImageMath) corrected: ...`) names
the entry wherever it changed the result, in both modes
([upstream-defects.md](upstream-defects.md); each entry in the
[register](../findings/upstream-defects.md)).

| ID | ANTs (what `reproduce` gives) | larmorx's default |
|---|---|---|
| ANTS-03 | a file name of fewer than 3 characters is neither read nor written, silently, and the arithmetic, `TruncateImageIntensity`, `Normalize`, morphology, `GetLargestComponent` and `D` operations write no name of 3 characters or fewer; `PadImage` reads no such name (and crashes) | read and written like any other name; the warning comes when such a file was read or written |
| ANTS-04 | exit 0 after an operation that reports an error and writes nothing | exit 1 (no operation ported so far reports one; their failures are crashes in ANTs and exit 1 in both modes) |
| ANTS-05 | the operand of the arithmetic operations: `1e` and `-` are the number 0, `1e40` is `FLT_MAX`, no operand at all is the number 1 | a malformed or out-of-range number is an error; no operand is an error, except for `abs`, `exp`, `total`, `mean` and `vtotal`, where ANTs' 1 means the image alone |
| ANTS-06 | `ExtractContours` labels a negative value by its wrapped conversion (near 2^64; x86-64's answer to an undefined conversion) | negative values and NaN clamp to 0 (background), with a warning |
| ANTS-07 | a second image smaller than the first is read at the first image's indices, past its own voxels (the reads beyond its memory are undefined: `reproduce` takes them as 0 and warns) | an error |
| ANTS-08 | `/` divides only where the divisor is positive; other voxels keep the previous voxel's result | divides wherever the divisor is not 0, and writes 0 where it is |
| ANTS-09 | `total` and `mean` write the running sum into each voxel | every voxel holds the printed value (the total, or the mean) |
| ANTS-10 | `vtotal` writes an image of zeros (ANTs implements nothing for it) | an error |
| ANTS-11 | `Neg` finds the range with `else if`, so the minimum is missed on images whose values increase in memory order | the true range |
| ANTS-12 | `TruncateImageIntensity` finds the histogram's range with `else if`: the brightest voxel can fall outside the histogram | the true range |
| ANTS-13 | `Normalize` starts the range at a maximum of 0 and a minimum of 1e9 | the true range |
| ANTS-14 | `G` with a variance above about 709 voxels² writes NaN everywhere (the kernel's Bessel function overflows) | the kernel computed with exponentially scaled Bessel functions, so it stays finite; elsewhere ITK's kernel bit for bit |
| ANTS-15 | `G` with a sigma list of the wrong length prints "Incorrect sigma vector size" and returns the image unchanged, exit 0 | an error |
| ANTS-16 | `Laplacian` reads its normalize flag from the sigma's argument (`stoi("1.5")` is 1; `.5` aborts) | the flag from its own argument, as `Grad`; the warning comes when the two readings differ |
| ANTS-19 | `FillHoles` with `p ≤ 1` counts neighbours outside the image at their unclamped offsets: the next row's voxels, or memory before or after the image on the first or last slice (undefined: `reproduce` takes it as background and warns) | neighbours outside the image are not counted |
| ANTS-20 | `GetLargestComponent` turns every voxel into 1 when no component is kept | every voxel 0 |
| ITK-02 | a file with more dimensions than the operation's (a 4D series read in 3D) is read with an identity direction | the file's direction of its first `D` axes, where they do not reach into the others (every NIfTI time series) |
| ITK-03 | the connected components relabelled in a float image stop getting new labels at 2^24 | counted in integers |
| ITK-08 | asked for `x.nii.gz`, ITK's NIfTI reader takes the voxels from `x.nii` when that file exists too | the named file's |

## Deliberate differences

- `^` is correctly rounded (ANTs: glibc's `powf`, one ulp off in about 0.06 % of calls).
- Where ANTs crashes, larmorx exits 1 with a message, in both modes.
- The coefficients of the recursive Gaussians use correctly rounded `sin`, `cos` and `exp`;
  ANTs uses glibc's, which differ in the last bit for about 0.6 % of sigmas. That has not
  changed a single output value: each pass rounds to float.
- Grayscale morphology: where a neighbourhood holds both `-0.0` and `+0.0` and its extreme
  is zero, larmorx returns `+0.0` for a maximum and `-0.0` for a minimum; ITK's moving
  histogram returns either, depending on the order it was filled in.
- Radii above 65535 voxels are refused.
- `ClusterThresholdVariate` is not ported: ANTs decides with a value read outside the image
  (its result is undefined).
- Images are NIfTI only (ANTs reads every format ITK knows).
