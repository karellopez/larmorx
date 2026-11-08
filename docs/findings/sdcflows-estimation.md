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

*Measured* (newest environment, one thread, the machine shared with another agent): ds002603's
`brain_masker` 6.2 to 6.7 s against sdcflows' 7.6 to 8.0 s (three runs each, alternating); the
nine `brainextraction_wf` inputs 12.1 s against 15.6 s; peak memory 422 MiB at one thread and
432 MiB at fourteen against sdcflows' 1,439 MiB. The outputs stay sdcflows' own (27 of 27 files
in each environment, fMRIPrep's 27 of 27; `check_skimage.py`), and `lx.ndimage` stays SciPy's
in all 39,436 cases of `check_ndimage.py` in both environments.

## The PEPOLAR estimation (relay 5)

`lx.sdcflows.topup_workflow` runs sdcflows' `init_topup_wf` in both wirings (sdcflows 2.17.1's,
the default, and 2.15's, fMRIPrep 25.2.x's), node by node, and `check_pepolar.py` compares every
file:

- **The steps alone** (`units`): `get_trt` (192 cases: sdcflows' doctest metadata and every
  branch, the errors and messages), `GetReadoutTime`, `SelectPEVolumes`, `SortPEBlips`,
  `UniformGrid` (fMRIPrep's two estimators with each run as the reference: ds001600's runs are
  on different grids, 80x80x60 at 3 mm and 64x64x44 at 3.75x3.75x4 mm, so one is resampled;
  synthetic grids equal, close, shifted and rotated, `float32` and `int16`), `MergeSeries`,
  `PadSlices` (odd counts on each axis, scaled `int16`, `uint8`, `float32`),
  `ReorientImageAndMetadata` (LAS, RAS, LPI, PIR, lower case), `ReorientImage`,
  `_select_topup_config`, `_topup_mats_to_world`, nipype's `TOPUP` command line and encoding
  file: all identical in the shared environment.
- **niworkflows' `RobustAverage`** (`ravg`): every option (`t_mask`, `nonnegative`, `two_pass`,
  `mc_method=None`, `num_threads`), its 3D, one-volume and error paths, on the estimators' EPIs:
  every output and file identical. The drift's mean over each volume is numpy's
  `mean(axis=(0, 1, 2))` of a series whose volumes are C-ordered blocks: from numpy 2.3 the
  pairwise sum of each block, before (numpy 2.2.6, fMRIPrep's image) chunks of 8192 elements
  (the buffered order of the whole-array sums above); `numpy_add_reduce` gives either.
- **The workflows** (`wf`): sdcflows 2.15.1's and 2.17.1's `init_topup_wf` on ds001600,
  ds005040 and ds006736 (3D AP/PA EPIs on different grids, a D8 run of 2026-11-08), 2.17.1 also
  with `debug` (`ApplyCoeffsField` with `_topup_mats_to_world`'s matrices) and `sloppy` (the
  `_quick` configuration): every node file identical in both kept environments (shared: 61, 73,
  39 and 38 files of ds001600 and ds005040, and 25, 24, 25 and 24 of ds006736; newest: 55, 59, 36
  and 35 of ds005040 and ds006736),
  the configurations chosen alike (`b02b0_4.cnf` for 80x80x60 and 64x64x40, `b02b0_2.cnf` for
  74x64x58).
- **fMRIPrep's own node files** (`image`, numpy 2.2.6's arithmetic): for ds001600, ds005040 and
  ds006736 every node of the 2.15 wiring is fMRIPrep's file bit for bit: the run averages, the
  regridded run, the merge, the set's average and registered volumes (3dvolreg's replica), and,
  fed fMRIPrep's own topup outputs, `fix_coeff`, `from_las`, `from_las_fmap` and `ref_average`;
  nipype's `topup` command line is fMRIPrep's (directories apart). Two notes:
  - ds006736's regridded run: the data identical, `quatern_b` -1.4631e-16 where fMRIPrep's file
    has -1.5087e-16 (a quaternion component that is zero up to rounding). fMRIPrep's image gives
    its own value on the main machine without `OPENBLAS_CORETYPE` and larmorx's with
    `OPENBLAS_CORETYPE=Haswell` (`r5` probe: nibabel 5.3.2's `set_qform` through its numpy
    2.2.6's BLAS): a kernel dependence of numpy's linear algebra in fMRIPrep's image, counted
    apart (`quaternion rounding only`), to settle with the records (which kernel the old laptop
    ran).
  - **topup**: the replica gives FSL 6.0.7.23's own files on fMRIPrep's three `topup` inputs
    (24 images, `descrip` apart, and 6 matrices, FSL run through the oracle cache). fMRIPrep's
    image has the same topup release (fsl-topup 2203.5, another build and its own libraries, run
    on the old laptop): its files are FSL 6.0.7.23's bit for bit on ds001600 and ds005040, but
    not on ds006736 (0.15 % of the field's voxels identical, at most 0.58 Hz apart; the matrices
    identical). Whether the build, its libraries or the CPU's BLAS kernels make that difference
    is to be measured (the image's topup on the main machine through the oracle cache).
  Only `brainextraction_wf`'s N4 differs otherwise (eight threads there; above).
- **AFNI's history.** 3dvolreg writes the command line and the date into an AFNI extension of
  its NIfTI output, and nibabel carries it into files made from it (`to_las` in 2.15): those
  files differ from run to run in that extension (and `vox_offset`) only; their data and the rest
  of the header are compared exactly (counted apart as `descrip/extension only`).
- **FSL's configurations** (D9 for now): sdcflows 2.17.1 picks FSL's `b02b0_4/_2/_1.cnf`
  (byte-identical to FSL's own files, FSL Licence) by the grid's divisibility. larmorx ships
  only sdcflows' own (`b02b0.cnf`, fMRIPrep's; the `_quick` variants) and reads FSL's from an
  FSL installation (`fsldir`) or from larmorx-nc's FSL files (`topup_config_file`, one function);
  larmorx-nc carries them once the handoff adds them.
- **SDC-12** (rule A): `ReorientImageAndMetadata` validates `target_orientation.upper()` but hands
  the original string to nibabel's `axcodes2ornt`, which refuses lower-case codes; larmorx
  reorients to the code it checked (sdcflows calls it with `"LAS"` only).
- **UniformGrid resamples 3D images only here**: nitransforms' `apply` of one affine to a 4D image
  of fewer than eight volumes passes three coordinates for four axes (SciPy's error), and
  sdcflows' PEPOLAR paths give it 3D averages (2.15) or single volumes (2.17.1); larmorx raises
  `NotImplementedError` for 4D inputs on that path.

## nipype's command lines and the BOLD runs' registration and skull-stripping (relay 6)

- **nipype's lines, ported once** (`larmorx._nipype`): nipype 1.12.0 orders an interface's
  arguments by position, then by input name, negative positions last, quotes file names for the
  shell (`shlex.quote`; a list of files is not quoted: nipype computes the quoted list and does
  not use it), derives output names from `name_source`/`name_template` with the output type's
  extension, and some interfaces format their own pieces (`Registration` builds every stage under
  `transforms`; its two winsorize inputs give one argument and an empty one, so `command.txt`
  holds two spaces before `--write-composite-transform`; BET writes its input and output relative
  to the node's folder). Settings files (`from_file`) are converted by nipype's traits: sdcflows'
  `affine.json` gives `"smoothing_sigmas": [[6, 0], [2, 1]]`, written `6.0x0.0mm`, and
  `sd_syn.json`'s `[0.8, 6.0, 10.0]` stays three floats because nipype's `transform_parameters`
  tries its tuple forms in order (`F`, `FFF`, `FIII`, `FIFFFF`, `FFFI`, `FIIII`). Every
  `command.txt` of the fMRIPrep 25.2.6 oracle runs for the ported interfaces is formatted again
  character for character: 77 `antsRegistration` (`FixHeaderRegistration`), 663
  `antsApplyTransforms`, 95 `N4BiasFieldCorrection` (63 nipype's, 32 niworkflows'
  `FixN4BiasFieldCorrection`: none of their inputs had negative values, which it would rescale
  first), 577 `ImageMath`, 29 `DenoiseImage`, 46 `bet`, 46 `fslmaths` (`BinaryMaths`), 46
  `3dAutomask`, 4 `topup` (and its encoding file against nipype's own, the runs kept none): 1,583
  of 1,583 in both kept environments; nipype's own `cmdline` and outputs agree on 14 option sets
  the runs do not use (`check_nipype.py`). niworkflows' `RobustMNINormalization` (`norm`, 21
  nodes) builds its line itself and is not one of them.
- **fMRIPrep seeds ANTs.** fMRIPrep sets `ANTS_RANDOM_SEED` (50495 for `--random-seed 0`: each
  node's report lists it); without it, ANTs seeds its sampling from the clock, so an unseeded
  `antsRegistration` with `Random` sampling (sdcflows' `fmap-any_registration.json`) differs from
  run to run. `coeff2epi_workflow(ants_random_seed=...)` passes the seed as `--random-seed`, which
  ANTs prefers to the variable; with the run's seed larmorx's transform is ANTs 2.6.5's one-thread
  result bit for bit (sdcflows' own graph, ANTs through the oracle cache: 3 of 3, and 1 of 1 with
  `debug`, whose warped images are identical too), and fMRIPrep's own `coregister` transforms (six
  threads, its image's ANTs) within 1e-11 in every parameter on all 14 BOLD runs with a field map
  (the thread band of `docs/validation/ants-registration-panel.md`).
- **`skullstrip_bold_wf`**: BET's and fslmaths' replicas (FSL 6.0.7.23) and 3dAutomask's
  (AFNI 26.3.05) give fMRIPrep's own node files bit for bit on all 14 runs (BET's brain and mask,
  3dAutomask's mask, the product, the masked reference: 70 files), and niworkflows' own graph's on
  three runs (18 files; 6 of them identical but for AFNI's history extension).
- The two dilated masks of `init_coeff2epi_wf` are fMRIPrep's on all 14 runs (28 files).

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
