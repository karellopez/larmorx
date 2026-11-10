# scikit-image's ball, Otsu threshold, random walker and histogram matching

`lx.skimage` holds a replica of the three scikit-image functions under sdcflows' `brain_masker`:
`skimage.morphology.ball`, `skimage.filters.threshold_otsu` and
`skimage.segmentation.random_walker`, and of `skimage.exposure.match_histograms` (under sdcflows'
`match_histogram`; `exposure/histogram_matching.py`, numpy's `unique`, `bincount`, `cumsum` and
`interp` underneath). They are ported from **scikit-image 0.26.0**
(`morphology/footprints.py`, `filters/thresholding.py`, `exposure/exposure.py`,
`segmentation/random_walker_segmentation.py`, `util/dtype.py`); their arithmetic is the same in
scikit-image 0.25.2, fMRIPrep 25.2.6's. Under them run SciPy's sparse matrices and conjugate
gradients (`scipy/sparse/_compressed.py`, `_coo.py`, `sparsetools/csr.h`,
`sparse/linalg/_isolve/iterative.py`, SciPy 1.18.1 and 1.15.3) and numpy's `histogram`,
`linspace`, `dot`, `std` and float32/float64 `exp` (numpy 2.5.4 and 2.3.5, OpenBLAS's Haswell
`sdot`/`ddot`: `larmorx_core::numpy`). The Rust code is `larmorx_ndimage::skimage`.

**Status: experimental** (field maps part B, relay 3, 2026-11-08). Results are scikit-image's
bit for bit on seeded cases in the shared (numpy 2.3.5, SciPy 1.15.3) and newest (numpy 2.5.4,
SciPy 1.18.1) environments (`validation/investigations/sdcflows-fieldmap-b/check_skimage.py`):
`ball` 24 of 24, `threshold_otsu` 52 of 52 (float, integer images, histograms), numpy's
`histogram` 128 of 128 and `dot`/`norm` 1,218 of 1,218, `random_walker` 24 of 24 cases (2-D, 3-D,
float32 and float64 data and spacings, multichannel, integer data, negative labels, isolated
seeds, three labels, both outputs, every ported mode, the early returns; warnings identical), the
float64 systems with numpy's float64 `exp` from the C library (numpy's AVX-512 dispatch off;
on AVX-512 CPUs numpy 2.5.4 uses Intel's SVML `exp`, whose last bits differ).

```python
import numpy as np
import larmorx as lx

ball3 = lx.skimage.ball(3)
th = lx.skimage.threshold_otsu(image)
prob = lx.skimage.random_walker(
    data, markers, spacing=zooms, return_full_prob=True, scipy_version="1.18.1", n_threads=8
)
```

## Option mapping

| scikit-image | larmorx | notes |
|---|---|---|
| `ball(radius, dtype, strict_radius=, decomposition=None)` | `lx.skimage.ball(...)` | `decomposition='sequence'`: `NotImplementedError` |
| `threshold_otsu(image, nbins=256)` | `lx.skimage.threshold_otsu(image, nbins)` | float images: numpy's `histogram` with `nbins` bins; integer images: one bin per value (`nbins` ignored), as scikit-image |
| `threshold_otsu(hist=counts)` / `hist=(counts, centres)` | the same | zero counts at both ends trimmed |
| `random_walker(data, labels, beta=130, mode='cg_j', tol=1e-3, copy=True, return_full_prob=False, spacing=None, prob_tol=1e-3, channel_axis=None)` | `lx.skimage.random_walker(...)` | every argument (`copy=False`: the isolated unlabeled pixels set to -1 in the caller's labels, as scikit-image's `_preprocess` does); plus `scipy_version` (the sparse matrix's row order), `numpy_version` (numpy's `exp` and sums where releases differ), `upstream_defects`, `n_threads` |
| `mode='cg_j'` / `None` | the same | Jacobi-preconditioned conjugate gradients |
| `mode='cg'` | the same | no preconditioner; scikit-image's UMFPACK warning |
| `mode='cg_mg'` | falls back to `'cg_j'` with scikit-image's warning | scikit-image does the same without pyamg; pyamg's multigrid is not ported |
| `mode='bf'` | `NotImplementedError` | SciPy's `spsolve` (SuperLU) is not ported yet |
| `exposure.match_histograms(image, reference, *, channel_axis=None)` | `lx.skimage.match_histograms(..., upstream_defects="fix")` | numpy's `unique`/`bincount`, `cumsum` and `interp` (the same bits in numpy 2.2.6, 2.3.5 and 2.5.4); `channel_as_last_axis`'s moves and errors; float32 for a float32 or float16 image, else float64; SKI-02 (relay 14 of field maps part B) |

## What decides the bits

- **The types.** The gradients run in the type numpy promotes the data and the spacing to: a
  `float32` image with a `float32` spacing (nibabel's zooms) stays in `float32` end to end; the
  default spacing (`np.ones(3)`), a tuple of Python floats or any `float64` spacing makes the
  gradients, the weights, the Laplacian and the conjugate gradients `float64`; multichannel data
  make the scale `float64` too. `data.std()` is numpy's of the caller's array (its memory order
  decides numpy's summation order).
- **SciPy's row order** (`scipy_version`). SciPy 1.15 stores the Laplacian's diagonal after each
  row's other entries (`setdiag` through COO); SciPy 1.16 onwards sorts every row. The conjugate
  gradients' products are added in that order, so the probabilities' last bits differ between the
  releases (on sdcflows' `brain_masker`, 74-90 % of the voxels' probabilities; the masks differ in
  0 or 1 voxel).
- **numpy's dot products**: OpenBLAS's Haswell kernels (`OPENBLAS_CORETYPE=Haswell` on the
  validation machine; the kernel numpy runs on every AVX2 CPU), one thread.

## Upstream defects

- **SKI-01** (scikit-image's random walker with a mask): when labels are negative or seeds are
  isolated, scikit-image numbers the Laplacian's nodes by the ends of the remaining edges but the
  labels by the mask; a pixel of the mask with no neighbour left in it (a seed surrounded by pruned
  pixels) shifts every later node, so the system pairs labels with the wrong pixels (probabilities
  off by up to 0.59 on the check's cases). larmorx numbers both by the mask by default and warns
  (`UpstreamDefectWarning`); `upstream_defects='reproduce'` gives scikit-image's result bit for
  bit. Rule A (the input makes sense; the result is wrong).
- **SKI-02** (scikit-image's `match_histograms` of an unsigned-integer image): it counts the
  reference's values with numpy's `bincount`, which refuses a reference with non-integer values
  (`TypeError`: a float anatomy) or negative ones (`ValueError`: a signed image), and a `uint64`
  image (`TypeError`). The input makes sense (rule A): larmorx counts them with `unique`, as
  scikit-image does for every other image; the quantiles are the same, so the result is
  scikit-image's wherever scikit-image gives one. `upstream_defects='reproduce'` raises
  scikit-image's error. Checked on 6 array cases and sdcflows' `match_histogram` on 12 files
  and 7 real ones (`validation/investigations/sdcflows-fieldmap-b/check_match_histogram.py`, both
  kept environments).
- **SKI-03** (scikit-image's `random_walker` with labels that are not consecutive): it returns
  its consecutive renumbering of them (from 1; the negative ones to -1, -2, ...), not the labels
  given (labels 3 and 7 come back as 1 and 2), against its documentation (each pixel "labeled
  according to the marker"). Rule A: larmorx returns the labels given, with a warning;
  `upstream_defects='reproduce'` gives scikit-image's.
- **SKI-04** (`return_full_prob=True` with no pixel to segment: no zero label, or every
  unlabeled pixel isolated): scikit-image returns the seeds' boolean masks concatenated along
  the last axis (`(..., nlabels)` for 2-D labels, the volumes side by side for 3-D), not the
  documented `(nlabels, *labels.shape)` probabilities. Rule A: larmorx returns those
  probabilities (1.0 on each label's seeds), with a warning; `reproduce` scikit-image's.
  Neither reaches sdcflows' `brain_masker` (its labels are 1 and 2, with pixels to segment).
