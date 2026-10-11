# ResampleImage

`lx.ants.resample_image` (command line `larmorx ants ResampleImage`; Rust
`larmorx_ants::resample_image`, `larmorx_ants::resample`, `larmorx_interp`). A replica of ANTs
2.6.5 on ITK 5.4.5 (Apache-2.0): ITK's `ResampleImageFilter` with an identity transform onto a
grid with a new spacing or size, in any of ANTs' eight pixel types, without smoothing.

**Status: `validated`** against ResampleImage itself (see the
[validation record](../validation/ants-resample-image.md)): 57 cases, 46 outputs compared,
all bit-identical with ANTs' exact header bytes: the eight pixel types, every interpolator (B-spline
orders 0 to 5, Gaussian, windowed sinc), by spacing and by size, in 2, 3 and 4 dimensions,
and a size of 1 (ANTS-29 below). One expected divergence: B-spline, Gaussian and windowed-sinc
interpolation of 2D and 4D images are not supported yet.

**Speed** ([benchmark](../benchmarks/ants-gaussian.md)): the 1 mm MNI template to 2 mm (linear)
takes 91 ms against 431 ms for ANTs; a raw T1w to 1 mm by B-spline as `unsigned short` 1.49 s
on one thread and 334 ms on 12, against 3.17 s and 947 ms; a boldref to 1.5 mm by windowed sinc
1.80 s and 329 ms, against 7.05 s and 1.21 s.

## Quick start

```python
import larmorx as lx

t1w_2mm = lx.ants.resample_image("sub-01_T1w.nii.gz", 2.0)
on_grid = lx.ants.resample_image(t1w, size=[128, 128, 96], interpolation="bspline")
labels = lx.ants.resample_image(seg, 1.0, interpolation="nearest", pixel_type="short")
```

```bash
larmorx ants ResampleImage 3 sub-01_T1w.nii.gz t1w_2mm.nii.gz 2
larmorx ants ResampleImage 3 t1w.nii.gz on_grid.nii.gz 128x128x96 1 4
larmorx ants ResampleImage 3 t1w.nii.gz sinc.nii.gz 1.5 0 3 w 3      # Welch window, unsigned short
larmorx ants ResampleImage 3 t1w.nii.gz smooth.nii.gz 2 0 2 1.5x1.5x1.5 2 1   # Gaussian, uchar
```

## Option mapping

| ResampleImage | Python | Notes |
|---|---|---|
| `imageDimension` (2, 3, 4) | `dimension` (default: the image's) | a path is read in that many dimensions |
| `inputImage`, `outputImage` | `image`, the returned `lx.Image` | the output keeps the input's origin and direction |
| `MxNxO` with `0` (default) | `spacing` (one value or one per axis) | size `int(old spacing · old size / spacing + 0.5)` |
| `MxNxO` with `1` | `size=` (one value or one per axis) | spacing `old · (old size − 1) / (size − 1)` |
| interpolation `0` (default) / `1` | `interpolation="linear"` / `"nearest"` | any dimension |
| `2 [sigma] [alpha]` | `"gaussian"`, `sigma=`, `alpha=1.0` | sigma in mm, default the input spacing; 3D |
| `3 [window]` | `"sinc"`, `window="hamming"` | `c`osine, `w`elch, `b`lackman, `l`anczos, `h`amming (the first letter); radius 3; outside the image the nearest edge voxel; 3D |
| `4 [order]` | `"bspline"`, `order=3` | orders 0 to 5 (others are 3); 3D |
| `pixeltype` 0 to 7, after the interpolator's parameters | `pixel_type="char"`, `"uchar"`, `"short"`, `"ushort"`, `"int"`, `"uint"`, `"float"` (default), `"double"` | the type the image is read as and written in; see "The seventh argument" |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` (or `NSLOTS`, as ITK reads them) | `n_threads` | results do not depend on it |

Values are clamped to the pixel type's range and truncated toward zero for integer types (ITK's
`CastPixelWithBoundsChecking`); reading converts as a C++ `static_cast`, values out of the
type's range clamped (ANTS-06 below). Points outside the input get 0.

## The seventh argument

ANTs' usage reads `ResampleImage d in out MxNxO [size=1,spacing=0] [interpolate type] [pixeltype]`,
and its code reads `argv[7]` twice: as the pixel type (`std::stoi`) and as the interpolator's
parameter (ANTS-30). So a Gaussian with sigmas `1.5x1.5x1.5` resamples an `unsigned char`
image, a B-spline of order 5 an `unsigned int` image, and a windowed sinc with a window letter
aborts (`std::stoi("w")` throws), so only the default Hamming window can be chosen.

larmorx reads the command line as the usage groups it, each interpolator followed by its own
parameters, where ANTs reads them, and the pixel type after them:

```text
ResampleImage d in out MxNxO [size=1,spacing=0] [interpolate type [its parameters]] [pixeltype]
```

| Interpolation | Its parameters | The pixel type |
|---|---|---|
| `0` linear (default), `1` nearest neighbour, any other number | none | argument 7, as in ANTs |
| `2` Gaussian | `sigma` (argument 7, `AxBxC` or one value, mm) and `alpha` (8) | argument 9 |
| `3` windowed sinc | the window's letter (7): `c`osine, `w`elch, `b`lackman, `l`anczos, `h`amming | argument 8 |
| `4` B-spline | the order (7) | argument 8 |

So every command line ANTs reads with an interpolator without parameters means the same, and
a command line written for ANTs that gives a parameter now gets the default float pixel type
unless it names one after it; a notice names ANTS-30 whenever the two readings differ. ANTs'
`b` window builds a second Lanczos (`ResampleImage.cxx:76`, unreachable in ANTs); larmorx's is
Blackman, as the usage says. With `LARMORX_UPSTREAM_DEFECTS=reproduce` the arguments are read
as ANTs reads them. The Python function is not affected: its options are separate.

## Behaviour worth knowing

From ANTs ([findings](../findings/ants-gaussian-filters.md#resampleimage-examplesresampleimagecxx)):
- the windowed sinc uses ITK's default boundary condition (the nearest edge voxel), not the
  zero outside that `antsApplyTransforms` uses;
- an interpolation number other than 1 to 4 is linear; a B-spline order outside 0 to 5 is 3.

## Upstream defects (D13)

ANTs' deterministic defects are corrected by default and reproduced bit for bit on request:
`upstream_defects="reproduce"` in Python, `LARMORX_UPSTREAM_DEFECTS=reproduce` on the command
line. A warning (on the command line a line on standard error) names the entry wherever it
changed the result ([upstream-defects.md](upstream-defects.md); each entry in the
[register](../findings/upstream-defects.md)).

| ID | ANTs (what `reproduce` gives) | larmorx's default |
|---|---|---|
| ANTS-30 | the seventh argument is both the pixel type and the interpolator's parameter | "The seventh argument", above |
| ANTS-29 | a size of 1: the spacing `old · (n − 1) / 0` is infinite, and ANTs writes an image of zeros with an infinite spacing and a NaN origin, exit 0 (reproduced byte for byte) | an error (exit 1; Python `ValueError`) |
| ANTS-06 | values out of an integer pixel type's range (read from the input) and sizes out of `unsigned int`'s range are converted as x86-64 converts them: they wrap around (C++ leaves it undefined) | clamped to the range, NaN to 0, with a warning |
| ITK-01 | ITK's floor of a continuous index just below 0 is 0, not −1 (x86-64), in the windowed sinc and 4D linear interpolation | the exact floor; the warning counts the output voxels this changed |
| ANTS-03, ITK-02, ITK-08 | as for every ANTs program: names of fewer than 3 characters, the direction of a file with more dimensions than `d`, `x.nii` read for `x.nii.gz` ([ImageMath](ants-image-math.md#upstream-defects-d13)) | as there |

## Deliberate differences

- B-spline, Gaussian and windowed-sinc interpolation of 2D and 4D images are not supported
  yet (ANTs runs them).
- In Python every window and B-spline order is reachable, independently of the pixel type.
- Where ANTs crashes (a missing input, an invalid spacing list), larmorx exits 1.
- Images are NIfTI only.
