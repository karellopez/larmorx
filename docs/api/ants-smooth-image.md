# SmoothImage

`lx.ants.smooth_image` (command line `larmorx ants SmoothImage`; Rust
`larmorx_ants::smooth_image`, `larmorx_image::gaussian`, `larmorx_image::median`). A replica of
ANTs 2.6.5 on ITK 5.4.5 (Apache-2.0): Gaussian smoothing with
`SmoothingRecursiveGaussianImageFilter`, or median filtering with `MedianImageFilter`.

**Status: `validated`** against SmoothImage itself (see the
[validation record](../validation/ants-smooth-image.md)): 38 cases, 30 outputs compared, all
bit-identical with ANTs' exact header bytes, in 2, 3 and 4 dimensions.

**Speed** ([benchmark](../benchmarks/ants-gaussian.md)): sigma 1 voxel on a raw T1w
(160×192×192) takes 119 ms on one thread and 63 ms on 12, against 442 ms and 134 ms for
ANTs; fMRIPrep's BOLD series smoothed in 4D 641 ms against 1.84 s on one thread; the median
filter (radius 1) 1.26 s and 259 ms against 2.00 s and 361 ms.

## Quick start

```python
import larmorx as lx

smoothed = lx.ants.smooth_image("sub-01_T1w.nii.gz", 1.5)                 # sigma 1.5 voxels
smoothed = lx.ants.smooth_image(t1w, [2.0, 2.0, 3.0], sigma_in_physical_units=True)
filtered = lx.ants.smooth_image(t1w, 1, median=True)                      # 3x3x3 median
```

```bash
larmorx ants SmoothImage 3 sub-01_T1w.nii.gz 1.5 smoothed.nii.gz
larmorx ants SmoothImage 3 sub-01_T1w.nii.gz 2x2x3 smoothed.nii.gz 1
larmorx ants SmoothImage 3 sub-01_T1w.nii.gz 1 median.nii.gz 0 1
```

## Option mapping

| SmoothImage | Python | Notes |
|---|---|---|
| `d` = 2, 3, 4 | the image's own dimensions | a 4D image is smoothed along time too (the time step is its spacing) |
| `image.ext` | `image` | path, `lx.Image`, nibabel image or `(array, affine)`; read as `float` |
| `smoothingsigma` (`1.5` or `1x1x2`) | `sigma` (a number or one per axis) | in voxels: multiplied by the spacing **in float**, as ANTs does |
| `outimage.ext` | the returned `lx.Image` | float32, the input's grid |
| fifth argument `1` | `sigma_in_physical_units=True` | sigma in mm (s for time) |
| sixth argument `1` | `median=True` | `MedianImageFilter`, radius `int(sigma)` voxels per axis |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` | `n_threads` | results do not depend on it |

## Behaviour worth knowing

From ANTs, reproduced ([findings](../findings/ants-gaussian-filters.md#smoothimage-examplessmoothimagecxx)):
- the recursive Gaussian smooths the **last axis first**, then the others, rounding to
  `float` after every axis; borders extend the edge values;
- every axis needs at least 4 voxels (ITK throws; larmorx exits 1, Python `ValueError`);
  a sigma of 0 throws too;
- a sigma list of the wrong length prints "Incorrect sigma vector size" and smooths with
  1 mm (the filter's default); Python raises `ValueError` instead;
- the median radius is truncated (`1.7` is 1);
- reading follows ANTs (`dim 3` on a 4D file takes its first volume; non-finite values read
  as 0).

## Deliberate differences

- Where ANTs crashes (a missing input, no output name), larmorx exits 1 with a message.
- The median orders `-0.0` before `+0.0` (and NaN last); ITK's `std::nth_element` leaves
  that to chance.
- Images are NIfTI only.
