# sdcflows' field-map estimation (part B): findings

The field maps' part B ports the rest of sdcflows' estimation and correction (sdcflows 2.17.1,
2.15.1 for fMRIPrep 25.2.6's runs), niworkflows' helpers (1.15.4) and fMRIPrep's own field-map
steps. This file records what the port found about the originals and the libraries under them.
Part A's findings are in [sdcflows-fieldmap.md](sdcflows-fieldmap.md); SciPy's filters in
[scipy-ndimage-filters.md](scipy-ndimage-filters.md).

## numpy 2.2 adds whole arrays in buffered chunks (relay 3, 2026-11-08)

`np.add.reduce(a, axis=None)` of a contiguous array (and so `a.sum()`, `a.mean()`, `a.var()`,
`a.std()`) is computed differently by numpy 2.2 and numpy 2.3 onwards:

- numpy 2.2.6 (fMRIPrep 25.2.6's image): the buffered reduction's inner loop gets at most
  `NPY_BUFSIZE` = 8192 elements, so the sum is `((0 + pairwise(c0)) + pairwise(c1)) + ...` over
  chunks of 8192 elements;
- numpy 2.3.5 and 2.5.4: `0 + pairwise(a)` over the whole array (the iterator's buffering was
  rewritten in numpy 2.3).

Measured on `brain_masker`'s random-walker input of ds002603 (138³ float32 values): the sum is
2125824.0 in numpy 2.2.6 and 2125823.8 in numpy 2.3.5; the chunk model gives numpy 2.2.6's
value exactly, the whole-array pairwise sum numpy 2.3.5's. The difference reaches scikit-image's
random walker through `data.std()` (0.4744121 against 0.4744123), so the edge weights, the
Laplacian and the probabilities of five of the nine `brain_masker` calls of the oracle runs
differ in their last bits between fMRIPrep's image and the shared environment (masks identical).
`larmorx_core::numpy::reduce::SumOrder` holds both orders; `lx.sdcflows.brain_masker` takes the
order from `numpy_version` (`"2.2.6"` gives fMRIPrep's own files, 27 of 27). **Any port compared
with fMRIPrep's image that takes a whole-array sum, mean, variance or standard deviation of
more than 8192 elements needs it** (tedana's `t2smap` in fMRIPrep's runs, for instance).
Probes: `relay3/probe_rw_steps.py`, `relay3/probe_sum.py`.

## SciPy 1.15 and 1.18 order the random walker's Laplacian rows differently (relay 1, measured again in relay 3)

`scipy.sparse`'s `setdiag` on a CSR array without the diagonal goes through COO: SciPy 1.15
leaves the new diagonal after each row's other entries; SciPy 1.18 sorts the indices afterwards
(`_compressed.py`, `_setdiag`). The fancy row and column indexing of `_build_linear_system`
keeps each row's order, so the conjugate gradients' `A @ p` adds the products in a different
order and the probabilities differ (74-90 % of `brain_masker`'s probability voxels; the masks
in 0 or 1 voxel). `larmorx_ndimage::skimage::SparseOrder`; `scipy_version` in `lx.skimage` and
`lx.sdcflows`.

## OpenBLAS's Haswell `ddot` does not fuse its tail (relay 3)

numpy's float64 `np.dot` runs OpenBLAS's `ddot`: the first `n & -16` elements in the Haswell
assembly kernel (four ymm accumulators of fused multiply-adds), the rest added as `dot += y[i] *
x[i]`. The kernel file is compiled with FMA, but the tail's products are rounded before the
sum (300 random vectors of each length 2-15: all equal to the unfused sum, 30-50 % different
from the fused one). `larmorx_core::numpy::dot::ddot`.

## scikit-image's random walker with a mask (SKI-01, rule A)

With negative labels or isolated seeds, `_preprocess` returns a mask, `_build_laplacian`
removes the edges touching pruned pixels and renumbers the remaining nodes by `np.unique(edges)`
(the ends of the remaining edges), while `_build_linear_system` numbers the labels by the mask
(`labels[mask]`). A pixel of the mask with no neighbour left in it (a seed surrounded by pruned
pixels: `_preprocess` keeps such seeds in the mask) is absent from the first numbering, so every
later node is paired with the next pixel's label: the system is wrong (on the check's two cases,
probabilities off by up to 0.59 against the mask's numbering). Rule A (the input makes sense:
scikit-image documents negative labels as pruned pixels): larmorx numbers both by the mask and
warns; `upstream_defects='reproduce'` gives scikit-image's result bit for bit (2 of 2).
sdcflows' `brain_masker` never reaches it (no negative label, no isolated seed).

## niworkflows' `IntraModalMerge`: the header goes through nibabel's constructors (relay 4)

`IntraModalMerge` averages sdcflows' magnitude images (`magmrg`) and direct field maps
(`fmapmrg`). Its average's header is not the first file's header: it is the header after every
image nibabel builds on the way (`squeeze_image` when it drops an axis, `four_to_three`,
`concat_images`, then `Nifti1Image(average, affine, header)`), and each constructor runs
`update_header`, which calls `set_data_shape` **only when the shape changed** and then sets every
`dim` beyond the image's dimensions to 1 and every `pixdim` beyond them to 1.0. A scanner's 3D
magnitude (`dim = [3 64 64 44 1 0 0 0]`, `pixdim[4] = 1.5`, the later pixdims 0) merged with a
second one becomes 4D (`dim[5:] = 1`, `pixdim[5:] = 1`) and its 3D average then has
`dim[4:] = 1` and `pixdim[4:] = 1.0`: the repetition time is gone. Writing the average with the
first file's header gives a different header in 6 of fMRIPrep's 7 `magmrg` files; carrying the
header through each constructor (`larmorx.niworkflows.images._constructed`) gives all 7 bit for
bit. *Measured:* `validation/investigations/sdcflows-fieldmap-b/check_merge.py` (62 merges in
each kept environment, scanner-like headers among them).

**The average is numpy's sequential sum.** `concat_images` fills a `float64` array of shape
`(n, x, y, z)` and rolls it to `(x, y, z, n)`; `get_fdata(dtype='float32')` keeps that memory
order, so `mean(axis=3)` sums each voxel's volumes one after the other from `+0.0` (the summed
axis is the outermost in memory, not a pairwise inner loop) and divides by the count in
`float64`. *Measured* in numpy 2.2.6 (fMRIPrep's image), 2.3.5 and 2.5.4 with 2 to 130 volumes
(`relay4/probe_merge_mean.py`): sequential in all three; a C-ordered copy would sum pairwise
from 8 volumes on.

**Two defects** (rules A and B; IDs from this part's range):

- **PIPE-18** `grand_mean_scaling` indexes the `(x, y, z, n)` series with
  `mask[..., np.newaxis]`, a boolean index of shape `(x, y, z, 1)` that numpy refuses for every
  series of two or more volumes (`IndexError: boolean index did not match indexed array along
  axis 3; size of axis is n but size of corresponding boolean axis is 1`), so the option can
  never run (a series of one volume returns before it). Rule A (the input makes sense): larmorx
  takes each volume's median inside the mask, as the code means (`series[mask]`), and scales each
  volume by the largest median over its own, with a warning; `upstream_defects='reproduce'`
  raises numpy's `IndexError`. Checked against niworkflows' lines with the mask's shape fixed
  (2 of 2) and the error against niworkflows' own (2 of 2). No caller sets the option.
- **PIPE-19** a 6- or 7-dimensional image passes the check for a 5D image and is averaged over
  its fourth axis. Rule B: niworkflows' error ("Input image (...) is 6D."); `reproduce` gives
  niworkflows' result (1 of 1).

## sdcflows' workflows as functions (relay 4)

`lx.sdcflows.brainextraction_workflow`, `magnitude_workflow` and `fmap_workflow` run sdcflows'
graphs node by node, each node's files written where nipype's node directory holds them and
read back by the next node. Every NIfTI file the nipype graphs write is reproduced bit for bit
(data and header) with `upstream_defects='reproduce'`, in both kept environments, with ANTs'
N4 at one thread with correctly rounded maths and FSL's PRELUDE (and larmorx's replica) under
both (`check_workflows.py`): the nine `brainextraction_wf` inputs of the oracle runs, the seven
phase-difference estimators from their BIDS files and fMRIPrep's metadata, and ds005752's direct
field map (Hz, `int16`; also in rad/s, `int16` and `float32`, and two maps merged).

- **PRELUDE's output name.** nipype's `PRELUDE` names its output with FSL's output type
  (`NIFTI_GZ` in fMRIPrep's and sdcflows' environments): an uncompressed phase map (ds002422's
  `.nii` files) gives `<phase>_rads_unwrapped.nii.gz`, and the field map and the fit's files
  after it are `.nii.gz`. Part A's `phdiff_workflow` kept the input's extension; corrected.
- **N4's `copy_header`.** nipype's `N4BiasFieldCorrection(copy_header=True)` copies the input's
  header onto ANTs' output with `keep_dtype=False`: ANTs' `float32` values are stored in the
  clipped image's `int16` with nibabel's scaling, and the next `IntensityClip` reads the
  re-quantised values. With the defaults (`upstream_defects='fix'`) larmorx's N4 leaves the
  clipped image's zero background out of its fit (ANTS-40), so the corrected image differs from
  sdcflows' by design.
- **fMRIPrep's own files.** fMRIPrep 25.2.6 ran sdcflows' N4 at eight threads (`NSLOTS=8`) with
  glibc's maths; larmorx computes ANTs' one-thread result, so the files from `n4` on differ
  within ANTs' own thread band (`docs/findings/ants-n4.md`); `magmrg` and `clipper_pre` are
  fMRIPrep's files bit for bit, and the nodes after N4 fed with fMRIPrep's own inputs are too
  (`check_skimage.py`'s `clip` and `masker`).

## `brain_masker` at one thread (relay 4)

Relay 3's port took 16.2 s at one thread on ds002603's magnitude (128³ padded to 138³) where
sdcflows takes 8.2 s. Measured step by step (`relay4/profile_masker.py`), the binary dilation
and closing by `ball(5)` were five times slower than SciPy's (each position computed all 515
element offsets before a scan that stops early) and the random walker's conjugate gradients
spent 70 % of their time in `A @ p` (every product and sum through the NaN rules' branches, the
two labels' systems solved one after the other, each reading the 2.4-million-row matrix). Three
changes, every result unchanged (the logical value of a binary erosion does not depend on the
order of its elements; IEEE arithmetic is SSE's except for a NaN's payload, and every result
that is a NaN is recomputed with the exact rules):

- the binary erosion's pass prepares each line once (the structure's elements inside the array
  on the other axes, their offsets) and, for `uint8`/`bool` inputs with a contiguous last axis
  and no mask, decides eight positions at once with 64-bit words (each byte's truth by a
  bitwise test): `binary_dilation` by `ball(5)` 0.20 s at one thread against SciPy's 0.44 s
  (1.38 s before);
- the minimum, maximum and rank filters read the interior positions' elements at the
  position's offset plus each element's relative offset;
- the conjugate gradients of all labels run in lockstep (one pass over the matrix for two
  labels), with fast paths for the products and the vector updates.

## Candidates for the register (the part's defects relay decides them)

- **SDC-h** `brain_masker`: a rough mask without a connected component makes the background
  (label 0, of size 0) the largest, so the whole image is the brain; a constant inverted image
  divides 0 by 0 (NaN). Now: a clear error by default (rule B), `reproduce` gives sdcflows'.
- **brain_masker without brain seeds**: when the rough mask's erosion by `ball(5)` is empty, the
  markers have no label 1, the random walker renumbers label 2 as 1, and `labels[0]` is the
  background's probability, taken as the brain's. Now: a clear error by default (rule B).
- **SDC-g** `brain_masker`'s `grey_closing(..., structure=ball(1))` is a non-flat closing with a
  flat 3³ footprint: to measure (rule D).
- scikit-image's `random_walker` returns the renumbered labels (consecutive from 1), not the
  input's values, when these are not consecutive; `return_full_prob=True` with no unlabeled pixel
  returns a boolean concatenation along the last axis.
- niworkflows' `_advanced_clip` of a constant image: 0 / 0, then `round(255 * NaN)` cast to
  `int16` (PIPE-a).
- PIPE-18 and PIPE-19 (above) are already numbered and switched in `lx.niworkflows`
  (`intra_modal_merge`); the register's entries and `larmorx_core::defects` wait for the
  part's defects relay.
