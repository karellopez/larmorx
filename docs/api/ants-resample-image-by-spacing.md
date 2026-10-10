# ResampleImageBySpacing

`lx.ants.resample_image_by_spacing` (command line `larmorx ants ResampleImageBySpacing`; Rust
`larmorx_ants::resample_image_by_spacing`, `larmorx_ants::resample`). A replica of ANTs 2.6.5
on ITK 5.4.5 (Apache-2.0): optional recursive Gaussian smoothing, then ITK's
`ResampleImageFilter` with an identity transform onto a grid with a new spacing.

**Status: `validated`** against ResampleImageBySpacing itself (see the
[validation record](../validation/ants-resample-image-by-spacing.md)): 35 cases, 28 outputs
compared, all bit-identical with ANTs' exact header bytes and printed text, in 2, 3 and 4
dimensions. One expected divergence: where ANTs resamples uninitialised memory,
larmorx stops.

**Speed** ([benchmark](../benchmarks/ants-gaussian.md)): a raw T1w to 2 mm takes 149 ms on
one thread and 80 ms on 12, against 496 ms and 151 ms for ANTs; the 1 mm MNI template to
3 mm 252 ms against 629 ms on one thread.

## Quick start

```python
import larmorx as lx

low = lx.ants.resample_image_by_spacing("sub-01_T1w.nii.gz", [2.0, 2.0, 2.0])
up = lx.ants.resample_image_by_spacing(boldref, [1.0, 1.0, 1.0], smooth=False)
labels = lx.ants.resample_image_by_spacing(seg, [1, 1, 1], smooth=False, nearest=True)
```

```bash
larmorx ants ResampleImageBySpacing 3 sub-01_T1w.nii.gz t1w_2mm.nii.gz 2 2 2
larmorx ants ResampleImageBySpacing 3 seg.nii.gz seg_1mm.nii.gz 1 1 1 0 0 1
```

## Option mapping

| ResampleImageBySpacing | Python | Notes |
|---|---|---|
| `ImageDimension` (2, 3, 4) | `len(spacing)` | a path is read in that many dimensions; an in-memory image must have them |
| `inputImageFile` | `image` | path, `lx.Image`, nibabel image or `(array, affine)`; read as `float` |
| `outputImageFile` | the returned `lx.Image` | float32; same origin and direction, the new spacing |
| `outxspc outyspc [outzspc] [outtspc]` | `spacing` | one value per axis (`atof`); 4D: the last is the time step in seconds |
| `dosmooth` (default 1) | `smooth=True` | per axis, a recursive Gaussian of sigma `out/in − 1` (read by ITK in mm) where positive |
| `addvox` (default 0) | `add_voxels=0` | added to every axis's size |
| `nn-interp` (default 0) | `nearest=False` | nearest neighbour instead of linear |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` | `n_threads` | results do not depend on it |

The output size on each axis is `int(size · old spacing / new spacing + add_voxels)`. Output
voxels beyond the input take the input's value at index `(1, 1, …)` (before smoothing). The
Python result keeps the input's header, with the new time step in 4D.

## Behaviour worth knowing

From ANTs, reproduced ([findings](../findings/ants-gaussian-filters.md#resampleimagebyspacing-examplesresampleimagebyspacingcxx)):
- **the smoothing sigma is in millimetres but computed as a ratio**: 1 → 2 mm smooths with
  one voxel, 2 → 4 mm with half a voxel;
- it prints the input spacing, the new spacing, each axis's sigma and the output size;
- in 2D the `nn` flag is read from the `addvox` argument (whenever an `nn` argument follows);
  in 4D an `addvox` argument without an `nn` argument aborts; with smoothing, every spacing
  must be given (else ANTs crashes);
- 4D linear interpolation runs in four dimensions (time included), as ITK's
  `LinearInterpolateImageFunction` does.

## Deliberate differences

- **An axis of fewer than 4 voxels that needs smoothing:** ITK throws, ANTs catches it and
  resamples the smoothing filter's unfilled buffer (uninitialised memory, different on every
  run, exit 0). larmorx stops with ITK's message (exit 1; Python `ValueError`).
- `nearest=True` works in 2D in Python (the command line keeps ANTs' quirk).
- Where ANTs crashes or reads outside the image (an axis of one voxel for the `(1, 1, …)`
  default value), larmorx exits 1 with a message.
- Images are NIfTI only.
