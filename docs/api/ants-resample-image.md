# ResampleImage

`lx.ants.resample_image` (command line `larmorx ants ResampleImage`; Rust
`larmorx_ants::resample_image`, `larmorx_ants::resample`, `larmorx_interp`). A replica of ANTs
2.6.5 on ITK 5.4.5 (Apache-2.0): ITK's `ResampleImageFilter` with an identity transform onto a
grid with a new spacing or size, in any of ANTs' eight pixel types, without smoothing.

**Status: `validated`** against ResampleImage itself (see the
[validation record](../validation/ants-resample-image.md)): 57 cases, 46 outputs compared,
all bit-identical with ANTs' exact header bytes: the eight pixel types, every interpolator (B-spline
orders 0 to 5, Gaussian, windowed sinc), by spacing and by size, in 2, 3 and 4 dimensions. Two
expected divergences: B-spline, Gaussian and windowed-sinc interpolation of 2D and 4D images
are not supported yet, and a size of 1 is refused.

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
| `3 [window]` | `"sinc"`, `window="hamming"` | radius 3; outside the image the nearest edge voxel; 3D |
| `4 [order]` | `"bspline"`, `order=3` | orders 0 to 5; 3D |
| `pixeltype` 0 to 7 | `pixel_type="char"`, `"uchar"`, `"short"`, `"ushort"`, `"int"`, `"uint"`, `"float"` (default), `"double"` | the type the image is read as and written in |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` | `n_threads` | results do not depend on it |

Values are clamped to the pixel type's range and truncated toward zero for integer types (ITK's
`CastPixelWithBoundsChecking`); reading converts as a C++ `static_cast` on x86-64 (so negative
values wrap in unsigned types). Points outside the input get 0.

## Behaviour worth knowing

From ANTs, reproduced ([findings](../findings/ants-gaussian-filters.md#resampleimage-examplesresampleimagecxx)):
- **the seventh argument is read twice**, as the pixel type and as the interpolator's
  parameter: a Gaussian with sigmas `1.5x1.5x1.5` resamples an `unsigned char` image, a
  B-spline of order 5 an `unsigned int` image, and a windowed sinc with a window letter
  (`l`, `c`, `w`, `b`) aborts ANTs (`std::stoi` of a letter), so only the default Hamming
  window is reachable on the command line; ANTs' "blackman" (`b`) is a second Lanczos;
- the windowed sinc uses ITK's default boundary condition (the nearest edge voxel), not the
  zero outside that `antsApplyTransforms` uses;
- an interpolation number other than 1 to 4 is linear.

## Deliberate differences

- B-spline, Gaussian and windowed-sinc interpolation of 2D and 4D images are not supported
  yet (ANTs runs them).
- A size of 1: ANTs divides by zero and writes a one-voxel image with infinite spacing;
  larmorx refuses it.
- In Python every window and B-spline order is reachable, independently of the pixel type.
- Where ANTs crashes (a missing input, an invalid spacing list), larmorx exits 1.
- Images are NIfTI only.
