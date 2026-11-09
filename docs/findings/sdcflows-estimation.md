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
- **The D8 PEPOLAR run of ds004958** (sub-fsm64ps, its spin-echo AP/PA pair linked, 104x104x72,
  2 mm; finished 2026-11-08 19:40): `check_pepolar.py` `wf` in both kept environments, every node
  file of sdcflows' graphs identical (2.15: 30, 2.17: 35, debug: 36, sloppy: 35; FSL's `b02b0_4.cnf`
  chosen alike), and `image`: fMRIPrep's own run averages, merge, set average, `fix_coeff`,
  `from_las`, `from_las_fmap`, `ref_average` and its `topup` line identical; topup's replica = FSL
  6.0.7.23 on fMRIPrep's input (8 images, 2 matrices). fMRIPrep's image's topup differs from FSL
  6.0.7.23 here too (0.03 % of the field's voxels identical, at most 0.197 Hz; its matrices
  identical): the second run after ds006736 (relay 5's note), ds001600 and ds005040 the same: the
  cause stays to be measured with the records.
- **The D8 PEPOLAR run of ds002603** (sub-01, the phase difference's `IntendedFor` removed: two
  EPI+BOLD estimators; finished 20:32): the same checks, ALL OK in both environments (2.15: 60,
  2.17: 70, debug: 36, sloppy: 35 node files; fMRIPrep's own nodes of both estimators identical;
  topup's replica = FSL 6.0.7.23 on fMRIPrep's inputs, 16 images and 4 matrices), its two BOLD runs'
  dilated masks and skull-stripping = fMRIPrep's files (`check_bold_fmap.py`). fMRIPrep's image's
  topup differs from FSL 6.0.7.23 more here: 0.06 % and 0.8 % of the fields' voxels identical, at most
  5.35 Hz and 17.8 Hz apart (matrices identical). Three runs of five now: to measure before the
  records (the image's topup on the main machine through the oracle cache: build or CPU kernels).
- **The likely cause: where the runs ran.** The five fMRIPrep runs whose topup files are FSL
  6.0.7.23's (ds001600, ds005040) ran on the old laptop (AVX2: OpenBLAS's Haswell kernels); the
  three that differ (ds006736, ds004958's and ds002603's PEPOLAR views) are the D8 runs of
  2026-11-08, made on the main machine (AVX-512; their reports' `hostname`) without
  `OPENBLAS_CORETYPE=Haswell`, so the image's OpenBLAS picked its AVX-512 kernels, which round
  differently ([sdcflows-fieldmap.md](sdcflows-fieldmap.md): `sdot`; `numpy-blas.md`). The same
  explains relay 5's quaternion note (ds006736's regrid: the image gives its own value without
  `OPENBLAS_CORETYPE` on the main machine) and the **direct D8 run** (ds005752, its field map
  linked; `check_workflows.py` `direct`): the magnitude's and the field map's merges and
  `clipper_pre` are fMRIPrep's files, the brain mask identical in every voxel, but the B-spline
  fit's field differs everywhere by at most 0.067 Hz (scikit-learn's CG on the image's AVX-512
  `sdot`), where the seven phase-difference fits of the old laptop's runs are bit-identical.
  **Confirmed on ds006736 and ds004958** (relay 8): the orchestrator re-made the seven runs with
  `OPENBLAS_CORETYPE=Haswell` (`oracles/logs/fmap-queue-haswell.log`); on the re-made
  `ds006736-sub-004-noreconall` and `ds004958-sub-fsm64ps-noreconall-pepolar` the image's topup
  gives FSL 6.0.7.23's files (7/7 images each, `descrip` apart, 2/2 matrices, the field identical
  in every voxel), ds006736's regrid is fMRIPrep's file again, and every other check of the part
  on them is identical in both environments (`relay8/recheck.sh`). **And on ds002603's EPI
  view** (relay 9): its two estimators' topup = FSL 6.0.7.23 (7/7 images, 2/2 matrices, the
  fields identical in every voxel; 17.8 Hz apart before), `runwise_avg`, `setwise_avg`,
  `concat_blips`, `fix_coeff`, `from_las(_fmap)`, `ref_average` fMRIPrep's files, the BOLD
  steps (8 reconstructed fields, all through the resampled path) and fits identical in both
  environments. The other re-made runs: the part's notes, as they finish.

## fMRIPrep's own field-map steps: `ReconstructFieldmap`, `DistortionParameters`, `ConcatenateXFMs` (relay 7)

fMRIPrep 25.2.7's `fmriprep/interfaces/resampling.py` (the same file in 25.2.6 and in the
environments' fMRIPrep) and niworkflows 1.15.4's `interfaces/nitransforms.py` (the same in
1.15.2), ported as `lx.transforms.reconstruct_fieldmap(_node)`, `lx.transforms.distortion_parameters`
and `lx.niworkflows.concatenate_xfms`. Checked by `check_bold_steps.py` on the four earlier runs
with field maps applied to BOLD runs (ds002422's three runs, ds002603's two, ds005040's PEPOLAR
run, ds005454's run; old laptop, Haswell kernels): **all identical in both environments**, against
fMRIPrep's own files and against the interfaces run here: 28 `ReconstructFieldmap` files (data
and header bytes; 14 on the BOLD reference, 7 on the T1w reference, 7 on MNI152NLin2009cAsym at
2 mm through the T1w-to-MNI composite `.h5`), 28 `DistortionParameters` outputs (with 13
synthetic metadata sets for every branch), the 7 `out_fwd.tfm` files byte for byte (and the
`desc-fmap_xfm.txt` derivatives they become), and the 7 `unwarp_boldref` images the fields go
into (`lx.transforms.resample_series`, `ResampleSeries`). Times on the 28 nodes: fMRIPrep's
interface 28.5 s, larmorx 12.3-13.1 s (4 threads).

- **Two paths, one taken.** `reconstruct_fieldmap` evaluates the B-spline sum directly on the
  target only when the transforms collapse to one affine and the projected target is "aligned"
  with the coefficients: the norms of the cross products of the axis columns within 1e-3. The
  columns carry the voxel sizes (3 mm × 3 mm here), so the test is in mm² and fails for any
  rotation above about 1e-4 rad: all 28 nodes take the resampled path (the field on the field
  map's reference, then nitransforms' cubic resampling through the chain). The direct path is
  reached by an identity or a pure translation (`check_bold_steps.py`'s `synth` covers it on
  RAS and LAS targets, two levels and a target beyond the grid: identical to fMRIPrep's
  `reproduce`).
- **PIPE-21 (rule A, corrected by default).** The direct path returns the field on the target
  projected into the field map's space with its axes made positive (sdcflows'
  `ensure_positive_cosines`), with that projected affine, while `ResampleSeries` reads the field
  voxel for voxel on the target's grid (`fmap_hz` is added to the target's coordinates): for a
  target with an axis against its world axis, a LAS BOLD reference as every run here has, the
  field is applied mirrored along that axis. Measured on a LAS target with the identity: up to
  159 Hz from the field on the target's grid; fMRIPrep's array is exactly the reversed one.
  larmorx returns the same values on the target's grid (voxel order and affine), equal to the
  resampled path's field there; `reproduce` gives fMRIPrep's image. It changes nothing on the
  earlier runs (none is direct); the SyN runs (their field map is estimated on the BOLD
  reference) are the ones to look at when they are re-made.
- **PIPE-20 (rule C, kept).** `DistortionParameters` catches `KeyError` and `ValueError`: a BOLD
  run without a readout time (no `TotalReadoutTime`, echo spacing or fallback) or without
  `PhaseEncodingDirection` gets undefined outputs, and `ResampleSeries` (`if pe_dir and
  ro_time`) then applies no field map, although the run has one assigned and the reports name
  its method. The interface documents it as its design ("downstream interfaces should be
  prepared to handle this"), so it is kept; larmorx warns (`fix`). fMRIPrep stops with
  "Missing readout time information" only when an estimator needs the readout time. At the
  pipeline's level, relay 8 decided rule B (below).
- **PIPE-22 (rule C, kept).** nitransforms writes ITK text parameters with `"%g"`: six
  significant digits, read back by every later `ReconstructFieldmap`. Measured: the BOLD grid's
  corners move by at most 7.9e-05 mm on the 7 transforms. A correction would bring no measurable
  gain.
- The resampled path's coordinates are nitransforms' `apply`: `ImageGrid.ndcoords` of the target,
  each chain step rounding its points to float32 (an affine's `_as_homogeneous`, a displacement
  field's `reference.index(np.array(x, dtype="float32"))`), then the field's `ras2vox`; the same
  arithmetic as `resample_series`' coordinates, so `larmorx_transform::nitransforms` serves
  both. SciPy's `map_coordinates` gets the float32 field (float64 where the sparse product
  upcasts) and returns that type; the header is the target's, with the intent and `cal_max`.

## fMRIPrep's field-map nodes of a BOLD run, composed (relay 8)

`larmorx.pipelines.larmorprepx.fieldmaps` wires the ports as fMRIPrep 25.2.7's BOLD workflows
wire them: `bold_fit_fieldmap` (`init_bold_fit_wf`'s stage 3 and stage 4's field-map branch:
`fmapref_buffer`, `fmap_select`, `fmapreg_wf`, `itk_mat2txt`, `boldref_fmap`,
`distortion_params`, `unwarp_boldref`, `skullstrip_bold_wf`) and `resampling_fieldmap`
(`bold_native_wf`'s and every `init_bold_volumetric_resample_wf`'s `fmap_select`,
`distortion_params`, `fmap2target`, `inverses`, `fmap_recon`). Checked by `check_bold_stage.py`
on fMRIPrep's inputs of the four earlier runs (7 BOLD fits, 21 resampling workflows; and the
re-made ds006736 and ds004958): **every node file identical to fMRIPrep's in both environments**, with
fMRIPrep's own `DerivativesDataSink` as the sink (so the later nodes read the derivative files,
as in fMRIPrep, named as fMRIPrep names them): 14 fits (registering, and with fMRIPrep's
transform given) × 14 node outputs, 70 `skullstrip_bold_wf` files, the three derivatives; the
21 resampling workflows' selections, inversion flags, transform lists, distortion parameters
and field maps. A call of `bold_fit_fieldmap` takes about 1 s (21 calls in 20.5 s, 6 threads).

- **fMRIPrep's later nodes read the derivatives.** `boldref_fmap` reads the `desc-fmap_xfm.txt`
  derivative (a copy of `itk_mat2txt`'s `out_fwd.tfm`), `skullstrip_bold_wf` the
  `desc-coreg_boldref` derivative (a copy of `unwarp_boldref`'s image: the header needs no
  fix), and the BOLD mask downstream is the `desc-brain_mask` derivative, which
  `DerivativesDataSink` rewrites: fslmaths' float32 mask becomes `uint8` (`np.rint`, its default
  type for masks). The data the later nodes read are the node files' either way; the names of
  the skull-stripping files follow the input's. larmorx takes a `sink` callable for these three
  requests; the port of `DerivativesDataSink` (relay 10) is its first real sink.
- **niworkflows' log line is wrong** (a candidate for relay 10): `DerivativesDataSink` logs
  "Changing <file> dtype from float32 to float32" when it converts the mask to `uint8`: its
  `LOGGER.warning('Changing %s dtype from %s to %s', out_file, orig_dtype, orig_dtype)` names the
  original type twice. Only the message; the file is right.
- **The registration's threads do not reach the later nodes.** fMRIPrep's ANTs ran the
  registration with 8 threads (`NSLOTS`); larmorx gives ANTs' one-thread result: the parameters
  differ by 1.3e-13 to 6.7e-13 on the earlier runs (4.3e-12 and 1.2e-12 on ds006736 and ds004958), and nitransforms'
  `"%g"` (PIPE-22) writes the same `out_fwd.tfm` text on all 9: every node after it is
  fMRIPrep's file even with fMRIPrep's own registration.
- **PIPE-20 at the pipeline level: rule B.** A BOLD run with a field map whose metadata give no
  readout time (or no phase-encoding direction) cannot be corrected as asked: fMRIPrep
  resamples it without the field map while its report names the correction (the summary's
  distortion-correction line is the estimator's method) and its derivatives' sidecars list the
  field map among their sources. That is no design pipelines rely on (fMRIPrep itself stops with
  "Missing readout time information. See documentation for `--fallback-total-readout-time`."
  when an estimator lacks the readout time, `workflows/base.py`), and the input cannot be
  completed by larmorx: so a clear error (`FieldmapMetadataError`, fMRIPrep's message, the run
  and the estimator named), with `--fallback-total-readout-time` (`fallback_total_readout_time`)
  as the way on. The interface itself (`lx.transforms.distortion_parameters`) keeps its
  documented undefined outputs (rule C, relay 7). `reproduce` resamples without the field map,
  as fMRIPrep, with an `UpstreamDefectWarning` (checked: `reproduce` = fMRIPrep's
  `ResampleSeries` without `ro_time` on the same files).

## The fieldmap-less (SyN) preprocessing (relay 9)

`lx.sdcflows.syn_preprocessing_workflow` (`init_syn_preprocessing_wf`) with niworkflows' EPI
reference (`init_epi_reference_wf`: `ValidateImage`, `NonsteadyStatesDetector`, `RobustAverage`,
two `IntensityClip`s, N4, `StructuralReference`'s one-file shortcut), `GenerateSamplingReference`
(nilearn's nearest `resample_img`), `Binarize`, sdcflows' `Deoblique`, `DenoiseImage`,
`BrainExtraction`, `BinaryDilation`, `antsRegistration` (`affine.json`), `antsApplyTransforms`
(now runnable from nipype's line: `_core.ants_file_program`) and the prior
(`check_syn_preprocessing.py`):

- **fMRIPrep's own nodes, each fed fMRIPrep's inputs** (the three `--use-syn-sdc` runs of
  2026-11-08, five SyN estimators; header bytes as written, `scl_slope`/`scl_inter` included):
  `validate_nii` (its report too), `select_volumes`, `per_run_avgs` (and the drift), `clip_avgs`,
  `clip_bg_noise`, `deob_epi`, `anat_dilmsk`, `mask_anat`, `clip_anat`, `epi_brain`'s mask,
  `epi_dilmsk`, `sampling_ref` (nilearn 0.11.1's path), `anat2epi`, `mask2epi`, `mask_dtype`,
  `prior2epi`, `prior_msk` and the command lines of `ref_anat`, `epi2anat`, `anat2epi`,
  `mask2epi` and `prior2epi` are fMRIPrep's files, with fMRIPrep 25.2.6's image's libraries as
  switches (numpy 2.2.6, SciPy 1.15.2, nilearn 0.11.1, niworkflows 1.15.2). Those runs were made
  with the AVX-512 kernels; the brain mask's random walker (OpenBLAS's `sdot`) still gave the
  same mask on all five (the probabilities are not kept by nipype).
- **What depends on fMRIPrep's threads**: `n4_avgs` (N4's B-spline fit), `ref_anat`
  (DenoiseImage) and `epi2anat` (antsRegistration) ran on `omp_nthreads` = 4, and N4 and
  DenoiseImage with glibc's `expf`/`exp` ([ants-n4.md](ants-n4.md), [ants-denoise.md](ants-denoise.md)):
  larmorx gives ANTs 2.6.5's one-thread results with correctly rounded maths, checked bit for bit
  against that build on fMRIPrep's inputs (through the oracle cache). fMRIPrep's own N4 output
  shares 40 to 45 % of its voxels with the one-thread result, its DenoiseImage output 97.8 to
  99.0 %; its registrations differ from the one-thread result by 4.5e-13 to 2.6e-11 in their
  parameters on four estimators and by **0.081 on ds005003's second** (`affine.json`'s second
  stage samples 40 % of the voxels at random: which points a thread draws depends on the number
  of threads, so the optimiser can end elsewhere). So fMRIPrep's later files of a SyN estimator
  are a 4-thread result; the whole workflow is checked against sdcflows' own graph instead
  (below). Every node of the five estimators, fed fMRIPrep's inputs, is identical: 27 checks x 5
  (fMRIPrep's files, or ANTs' one-thread result for N4, DenoiseImage and the registration, with
  the run's `ANTS_RANDOM_SEED`).
- **The whole workflow against sdcflows' own graph** (the shared environment's sdcflows 2.17.1 and
  niworkflows 1.15.3 run by nipype, ANTs' programs at one thread with correctly rounded maths and
  AFNI's 3dvolreg as stand-ins through the oracle cache): every NIfTI file of the graph's
  folders identical on the five estimators (105 files), the five registered series of
  `RobustAverage`'s 3dvolreg apart in `vox_offset` only (AFNI writes its history as a NIfTI
  extension, larmorx's replica does not: [fmriprep-oracle.md](fmriprep-oracle.md)). sdcflows'
  graph takes 118 to 155 s at one thread, larmorx 30 to 39 s on 6 threads (the same one-thread
  results).
- **nilearn changed `resample_img` between fMRIPrep's image (0.11.1) and the newest (0.14.1)**:
  0.13 copies the image's header by default (`copy_header=True`: the fixed EPI's header with
  `scl_slope = scl_inter = 0`, which nibabel then writes unscaled, and `cal_max`/`cal_min` set to
  the data's range) and always resamples (`force_resample=True`); 0.11 makes a new header and
  copies by whole voxels (after nilearn's `crop_img`) when the grids differ by whole voxels. So
  `GenerateSamplingReference`'s file differs between the two in its header (`xyzt_units`,
  `pixdim[4]`, `cal_max`, `scl_slope`): `nilearn_version` gives either. Checked against nilearn
  0.14.1 in both environments (oblique, plumb, LPS, 4D, NaN and infinite values, `fov_mask`,
  `target_resolution`, `xform_code`) and against fMRIPrep's 0.11.1 files.
- **SciPy changed `affine_transform`'s sums in 1.16**: `NI_GeometricTransform` adds a matrix
  row's products to the offset (SciPy 1.16 onwards) instead of adding the offset after them
  (1.15): `lx.ndimage.affine_transform(scipy_version=...)` gives both; bit for bit on 600 seeded
  cases in each environment (`relay9/check_affine_transform.py`).
- **nilearn's bounds**: `np.dot(transform, box)` of a 4 x 4 and a 4 x 8 matrix is one fused
  multiply-add per term from zero in OpenBLAS's Haswell `dgemm` (3,000 seeded affines in both
  environments, `relay9/probe_bounds.py`; a plain sum differs in a third of them). nilearn inverts
  the image's affine with `scipy.linalg.inv`, whose last bits differ from numpy's `inv` in tiny
  entries (1,995 of 3,000 oblique affines: SciPy's `getrf`/`getri` in its own OpenBLAS, and in
  1.18 a structure detection that inverts a triangular affine with `trtri`); larmorx uses numpy's.
  The inverse only places the sampling points: a nearest value would change only for a point
  within about 1e-12 voxel of a half-voxel boundary, and every sampling grid checked is
  nilearn's file. A port of SciPy's 4 x 4 `inv` is a follow-up.
- **SDC-13 (Deoblique's threshold, rule D)**: the SyN EPIs of the runs are oblique by up to
  20.7 degrees (pixar008: 12.2, 20.7, 18.8 degrees per axis; ds004884: 2.5, 6.3, 5.8; ds005003:
  plumb), below the 28.6 degrees sdcflows' radians make of its threshold, so `Deoblique` and
  `Reoblique` pass them through; with the threshold in degrees pixar008's and ds004884's would be
  made plumb. The measurement needs the SyN registration itself (relay 10).
- **ValidateImage's report**: the angle of a qform that only changes at a qform's precision is
  rounding noise (1e-17 radians); niworkflows' text needs the qform as the header stores it
  (`float32`) and transforms3d's `decompose44` as it computes it, then the same numpy `eig`.
- **PIPE-23** (nipype's `is_outlier`, rule A) and **PIPE-24** (`ValidateImage`'s report with an
  unknown unit code, rule A): `docs/api/niworkflows-helpers.md`.

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
  (`intra_modal_merge`), PIPE-20 to PIPE-22 (relay 7) in `lx.transforms` and
  `lx.niworkflows.concatenate_xfms` (PIPE-20's pipeline level in
  `larmorx.pipelines.larmorprepx.fieldmaps`, relay 8); the register's entries and
  `larmorx_core::defects` wait for the part's defects relay.
