# fMRIPrep's confounds: the masks and the tissue signals

What porting fMRIPrep's `_binary_union`, `_get_zooms`, `mask2vf`, `acompcor_masks` and `aCompCorMasks`
and niworkflows' `BinaryDilation`, `BinarySubtraction`, `image_binary_dilation` and `SignalExtraction`
showed, and the defects found (PIPE-74 to PIPE-79). fMRIPrep and niworkflows are Apache-2.0, SciPy,
scikit-image and nibabel permissive: not restricted. fMRIPrep 25.2.7 = 25.2.6 and niworkflows 1.15.4 =
1.15.3 = 1.15.2 on these functions (`diff` of the sources; the shared environment's fMRIPrep
26.0.0.dev1 has an unreleased `_clip_mask_to_coverage`, not ported: the checks run the tag's code). The
port: `python/larmorx/confounds/masks.py`, `python/larmorx/niworkflows/morphology.py` and `images.py`,
`crates/larmorx-confounds/src/signals.rs` ([docs/api/confounds.md](../api/confounds.md),
[docs/api/niworkflows-helpers.md](../api/niworkflows-helpers.md)). The scripts: `check_masks.py`,
`probe_mask_casts.py`, `probe_signals_memory.py`
([validation/investigations/confounds](../../validation/investigations/confounds/README.md)).

## What the originals compute (read, verified)

- `_binary_union`: `np.asanyarray(dataobj, dtype=int) > 0` per mask (nibabel's `_get_scaled`, then
  `astype(promote_types(scaled, int64))`, then `astype(int64)`: integers other than `uint64` exact;
  `uint64` and floats through float64, truncated), `mask1 | mask2` (numpy's broadcasting), written as
  `uint8` with `mask1`'s class, affine and header (`mask_union.nii.gz`).
- `_get_zooms`: `tuple(header.get_zooms()[:3])` (`np.float32` values, after nibabel's load-time fixes).
- `mask2vf` (the aseg path): `get_fdata(dtype=float32)`, SciPy's `gaussian_filter(sigma = 0.5 · zooms /
  imgzooms)` (float32 output), `np.percentile(data[data > 0], 99)` (numpy's weak `q`: `np.float32` in
  2.2.6, 2.3.5 and 2.5.4; numpy 2.5 changed the arithmetic), `np.clip(data / max_data, 0, 1)` in float32,
  the file as `nb.Nifti1Image` float32 with the input's header copy.
- `acompcor_masks`: GM `get_fdata() > 0.05` (with `is_aseg`: `np.asanyarray(dataobj, np.uint8) > 0`),
  SciPy's `binary_dilation(gm, ball(3))`, `wm[gm] = 0`, `comb = csf + wm`, `comb[gm] = 0`, both written
  as `nb.Nifti1Image(data, gm.affine, gm.header)` (the GM map's stored type, nibabel's scaling); the CSF
  output is the input path itself (or `mask2vf`'s `acompcor_csf.nii.gz` with `is_aseg`).
- `BinaryDilation`: `np.bool_(dataobj)` (a NaN is true), `ndi.binary_dilation(mask.astype(bool),
  ball(radius)).astype(int)`, `uint8`; `BinarySubtraction`: `np.bool_` of the base, `base[np.bool_(sub)]
  = False`, `uint8`. fMRIPrep: radius 2, the crown = the dilated union minus the union.
- `SignalExtraction`: only the first mask's shape checked (`NotImplementedError`); one 4-D file →
  `four_to_three`; several files → `dataobj >= prob_thres` (a Python float: numpy's weak scalar, the
  map's own type); one file → the labels `np.unique` without 0; `series[:, j] =
  get_fdata()[mask, :].mean(axis=0)` (a C-ordered `(N, T)` copy in the mask's C order, summed over the
  outer axis from +0.0 in order; pairwise for `T = 1`; numpy 2.2's 8,192 chunks);
  `np.vstack((labels, series.astype(str)))` and `np.savetxt(fmt="%s", delimiter="\t")`: **numpy 2.x's
  `savetxt` opens a path in text mode with `encoding=None`, the locale's encoding (UTF-8 here and in
  fMRIPrep's image), not latin-1** (latin-1 only for an open file object); `"\n"` (`\r\n` on Windows:
  larmorx writes Linux's bytes).
- larmorx's `signal_extraction` reads each region's means where its voxels are (`signals.rs`, threads
  over volumes), from the stored float32 series: on ds005454's real node larmorx's peak is 2,790 MiB
  against niworkflows' 8,030 MiB (its float64 copy and the masked copies; `probe_signals_memory.py`).

## Measured facts (`probe_mask_casts.py`, the same in numpy 2.5.4, 2.3.5 and 2.2.6)

numpy's float64 `astype(str)` = Python's `repr` (400,009 values); the float → int64 and float → uint8
casts out of range ([confounds-numpy.md](confounds-numpy.md)); scikit-image's `ball` of a negative radius
has `int(abs(2r + 1))` points from `-r` down to `r` (`ball(-1)`: one false element, `ball(-2)`: the
7-voxel cross, `ball(-3)` = `ball(2)`'s 33 voxels, `ball(-0.5)`: empty, SciPy's `RuntimeError: structure
must not be empty`); nibabel reads an image without voxels as a one-dimensional empty array, so
`SignalExtraction` on a series without volumes stops with numpy's `IndexError: too many indices for
array: array is 1-dimensional, but 4 were indexed`; nipype's `File(exists=True)` texts at input-setting
time; `traits.Int` and `traits.Range` take `True` (1, 1.0).

**A larmorx mistake, fixed**: `lx.skimage.ball` gave an empty footprint for every radius below -0.5 (the
field maps' port); scikit-image's `mgrid` takes `int(abs(2r + 1))` points. Fixed in
`crates/larmorx-ndimage/src/skimage/mod.rs` (one line; non-negative radii unchanged: no golden checksum
moves), with a permanent unit case (`skimage::tests::balls`) and the golden case
`py.skimage.ball.negative`. Found because `BinaryDilation(radius=-1)` reaches it (PIPE-77's `reproduce`).

## Defects (rules A to D; `larmorx_core::defects`)

- **PIPE-74** (the D13 rule for undefined conversions): `_binary_union`'s int64 cast of NaN, ±inf and
  `|x| >= 2^63` (and `uint64` from 2^63, through float64) is undefined in C, INT64_MIN on x86-64
  (outside the union). Default clamped (+inf and huge positives inside; NaN and -inf outside in both
  modes: no notice for them), `reproduce` x86-64's; the notice counts the voxels at or above 2^63. The
  default is `value >= 1` for floats. fMRIPrep's inputs (uint8 and MultiLabel masks): never reached.
- **PIPE-75** (rule B): `_binary_union` broadcasts masks of different shapes silently; default
  `ValueError` (also for shapes numpy cannot broadcast), `reproduce` numpy's broadcasting or its
  `ValueError`.
- **PIPE-76** (the D13 rule; rule A for the integer wrap): `acompcor_masks(is_aseg=True)`'s `uint8` cast
  of the GM map: integers wrap (-1 is 255, inside; 256 is 0, outside), floats out of int32's range or NaN
  are undefined (x86: 0), in-range floats keep the truncated int32's low byte (300.0 is 44); default
  clamped to [0, 255] first (GM = the voxels >= 1), `reproduce` numpy's x86-64 cast; the notice counts the
  voxels the two put differently. FAST's maps never reach it.
- **PIPE-77** (rule B): `BinaryDilation`/`image_binary_dilation` with a negative radius (`traits.Int`
  takes it) dilates with scikit-image's arbitrary footprint (`-1`: the dilated mask empty); default
  `ValueError`, `reproduce` niworkflows' (with the fixed ball).
- **PIPE-78** (rule B): `SignalExtraction` writes a class label holding a tab, `\n` or `\r` as it is (the
  table breaks); default `ValueError`, `reproduce` the file with a notice.
- **PIPE-79** (rule D, open; fMRIPrep `init_bold_confs_wf` / `aCompCorMasks`): fMRIPrep passes
  `is_aseg=freesurfer` (`run_reconall`, its default), but `t1w_tpms` are always FAST's partial-volume maps
  (sMRIPrep's `fast2bids`), so with FreeSurfer the aseg path smooths partial volumes again (`mask2vf`)
  and its GM is only the voxels whose GM fraction is exactly 1 (the uint8 truncation), dilated by
  `ball(3)`. Measured on the oracle runs' own FAST maps (T1w space, before resampling; voxels > 0.99):
  ds005454 WM 156,468 without FreeSurfer, 182,036 on the FreeSurfer path; CSF 243,706 → 33,031; CSF + WM
  165,010 → 216,811 (GM > 0.05: 1,466,165 voxels; GM == 1: 554,771); ds000005 WM 34,104 → 63,370, CSF
  104,127 → 10,914, CSF + WM 36,312 → 72,947. Whether correcting it helps is measured on fMRIPrep runs
  with FreeSurfer (none of the oracle runs has it: all `--fs-no-reconall`; runs with recon-all are
  planned); until then larmorx keeps fMRIPrep's wiring in both modes, documented.
- **Kept** (rule C, documented): `_binary_union`'s truncation of fractions (against `np.bool_` in the
  morphology); the second mask's and the WM/CSF maps' affines never compared; `np.bool_(NaN)` is true;
  `SignalExtraction`'s empty region gives NaN (a mean of nothing); the regions' affines not compared.
- **Both refuse** (as niworkflows and numpy): another later mask's shape (numpy's `IndexError`), 2-D or
  4-D masks for the dilation (SciPy's `RuntimeError`), maps of other shapes in `acompcor_masks`
  (`IndexError`), fewer than three maps (`IndexError`), `mask2vf` without a positive value (`IndexError`).

## Proof (validated)

`check_masks.py`'s full tier, CHECK PASS in numpy 2.5.4 / SciPy 1.18.1 / scikit-image 0.26.0 /
niworkflows 1.15.4, numpy 2.3.5 / SciPy 1.15.3 / niworkflows 1.15.3, and numpy 2.2.6 / SciPy 1.15.2 /
scikit-image 0.25.2 / niworkflows 1.15.2 / nibabel 5.3.2 inside fMRIPrep 25.2.6's image, the same counts:
`union` 31 + 25 defaults = `reproduce` + 3 against PIPE-74's model + 3 PIPE-75 errors; `morph` 96
`BinaryDilation` (radii -3 to 3 and `True`, four shapes, uint8, float32 with NaN and fractions, negative
int16) + 60 + 36 PIPE-77 errors, 12 `BinarySubtraction`, 20 `image_binary_dilation`, 7 errors; `acc` 31
`acompcor_masks` (both paths; GM stored float32, scaled uint8, int16, NIfTI-2; discrete segmentations;
PIPE-76's wraps and undefined casts) + 21 + 6 against PIPE-76's model, 11 `aCompCorMasks`, 32 `mask2vf`
arrays bit for bit + 16 files, 2 trait errors; `signals` 106 `SignalExtraction` + 104 defaults at 1
thread = `reproduce` at 3 + 2 PIPE-78 errors; `zooms` 5; **`real`, the 36 confounds workflows of 21
oracle runs: the `union_mask`, `dilated_mask`, `subtract_mask`, `acc_masks` and `signals` nodes byte for
byte (decompressed) with fMRIPrep's node files**, the default = `reproduce` on every node, and the
originals run again in each environment = larmorx: 36 of 36 each. The suite `confounds-fmriprep` runs it.
Speed: [docs/benchmarks/confounds.md](../benchmarks/confounds.md).
