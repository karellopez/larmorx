# fMRIPrep's field-map steps of the BOLD workflows

`lx.transforms.reconstruct_fieldmap`, `reconstruct_fieldmap_node` and `distortion_parameters`:
fMRIPrep's `ReconstructFieldmap` and `DistortionParameters` (`fmriprep/interfaces/resampling.py`,
fMRIPrep 25.2.7, the same in 25.2.6, the oracle runs'; Apache-2.0, the NiPreps developers), the
steps between sdcflows' field-map estimators and the one-shot resampler
([resample-series.md](resample-series.md)): the field map evaluated from its B-spline
coefficients on a BOLD reference, a T1w reference or a template grid (`boldref_fmap` in
`bold_fit_wf` and `bold_native_wf`, `fmap_recon` in `bold_anat_wf` and `bold_std_wf`), and the
readout time and phase-encoding direction of the BOLD run (`distortion_params`). niworkflows'
`ConcatenateXFMs` (`itk_mat2txt`, the BOLD-to-field-map transform written as fMRIPrep's
`desc-fmap_xfm.txt`) is in [niworkflows-helpers.md](niworkflows-helpers.md).

A port in the main package, with sdcflows 2.17.1's `grid_bspline_weights` and
`ensure_positive_cosines` (part A's collocation, `crates/larmorx-sdcflows`), nitransforms
25.1.0's chain and resampling and nibabel 5.4.2's orientation and header arithmetic (MIT); a
clean-room original comes later (the user, 2026-11-06).

**Status: `experimental`**: every node of the oracle runs made on the old laptop is
**bit-identical** in both kept environments (numpy 2.3.5 / SciPy 1.15.3 and numpy 2.5.4 / SciPy
1.18.1), against fMRIPrep's own files and against the interfaces run in-process: 28
`ReconstructFieldmap` files (data and header bytes), 28 `DistortionParameters` outputs, and the 7
`unwarp_boldref` images the reconstructed fields go into (`lx.transforms.resample_series`).
`validation/investigations/sdcflows-fieldmap-b/check_bold_steps.py`; the record comes with the
part's records.

## `reconstruct_fieldmap(coefficients, fmap_reference, target, transforms=None, inverse=False, *, n_threads=1, upstream_defects="fix")` → `Image`

| larmorx | fMRIPrep | Notes |
|---|---|---|
| `coefficients` | `coefficients` (nibabel images) | paths or images, one per level, as sdcflows writes them; their affines place the knots |
| `fmap_reference` | `fmap_reference` | the grid the field is evaluated on before it is resampled |
| `target` | `target` | the grid the field is wanted on (a path, an image) |
| `transforms`, `inverse` | `transforms` (`load_transforms(files, inverse)`) | target RAS+ to field-map RAS+: a loaded chain (`lx.transforms.load_transforms`), or files in fMRIPrep's order with one flag or one per file; `None` is the identity (`nt.Affine()`) |
| `n_threads` | | the collocation's rows and the resampling's points; the result does not depend on it |
| `upstream_defects` | | PIPE-21 below; sdcflows' SDC-01 and SDC-02 follow the same switch |

The two paths, as fMRIPrep chooses them:

- **Direct** when the transforms collapse to one affine (`as_affine`: an `Affine`, or a chain of
  affines combined as nitransforms' `TransformChain.asaffine`, numpy's products) and the target
  projected through it is aligned with the last level's grid (`aligned`: the norms of the cross
  products of the axis columns within 1e-3, so in mm², and opposite axes count as aligned): the
  B-spline sum at every voxel of the projected target, its axes made positive
  (`ensure_positive_cosines`), with a fresh nibabel header. Any rotation of a few hundredths of a
  degree fails the test, so a registration's transform takes the other path: none of the 28
  nodes of the earlier runs is direct.
- **Resampled** otherwise: the field on the field map's reference (made RAS with
  `as_closest_canonical` when the coefficients are RAS and the reference is not: fMRIPrep's
  "hack"; `ValueError("Reference passed is not aligned with spline grids")` when it is rotated
  relative to the coefficients), its axes made positive, then nitransforms' `resampling.apply`
  onto the target: the target's voxel coordinates through the chain (each step rounding its
  points to float32, a displacement field looked up on its grid or interpolated, NaN outside
  meaning no displacement) and the field's `ras2vox`, SciPy's cubic `map_coordinates`
  (`constant` 0, prefiltered), with the target's header.

The header in both: intent `estimate` named `fieldmap Hz`, float32, `cal_max = max(|min|, max)`,
`cal_min = -cal_max`. The values are float32, float64 where SciPy's sparse product upcasts (a
level with an axis of no weights); the file is float32.

## `reconstruct_fieldmap_node(in_coeffs, target_ref_file, fmap_ref_file, transforms, inverse=(False,), *, out_dir=None, n_threads=1, upstream_defects="fix")` → `ReconstructFieldmapResult(fieldmap, out_file)`

| larmorx | fMRIPrep's `ReconstructFieldmap` | Notes |
|---|---|---|
| `in_coeffs`, `target_ref_file`, `fmap_ref_file`, `transforms` | the same inputs | files that must exist; `in_coeffs` and `transforms` not empty |
| `inverse=(False,)` | `inverse` (default `[False]`) | fMRIPrep's `boldref_fmap` passes `[True]`, `fmap_recon` `[True, False, ...]` |
| `out_dir` | the node's folder | `<last coefficient file's stem>rec<ext>` (nipype's `fname_presuffix`), written as nibabel writes it; nothing written without it |

## `distortion_parameters(metadata, in_file=None, fallback=None, *, upstream_defects="fix")` → `DistortionParametersResult(readout_time, pe_direction)`

| larmorx | fMRIPrep's `DistortionParameters` | Notes |
|---|---|---|
| `metadata` | `metadata` | the BOLD run's BIDS metadata |
| `in_file` | `in_file` | the image whose shape gives the number of phase-encoding steps (`get_trt`); a falsy value is none |
| `fallback` | `fallback` (`None`, `'estimated'` or a float) | `'estimated'`: `get_trt(..., use_estimate=True)`; a number (an int becomes a float): the fallback readout time; anything else: a `TypeError` (the trait's message) |
| → `readout_time`, `pe_direction` | `readout_time`, `pe_direction` | `lx.sdcflows.get_trt` and `metadata["PhaseEncodingDirection"]`; a `KeyError` or `ValueError` on the way leaves the outputs from there on `None` (PIPE-20); a readout time that is not a number and a direction that is not `i`, `i-`, `j`, `j-`, `k`, `k-` raise (the output traits); a missing `in_file` where one is needed raises `TypeError` |

## Upstream defects (D13)

| ID | Rule | What fMRIPrep does | larmorx by default | `reproduce` |
|---|---|---|---|---|
| PIPE-20 | C (kept) | `DistortionParameters` turns a missing readout time or phase-encoding direction into undefined outputs, so the BOLD run is resampled without its field map although one is assigned to it (its documented design: "downstream interfaces should be prepared to handle this") | the same `None` outputs, and a `UserWarning` naming the missing value and `--fallback-total-readout-time` | the same outputs, no warning |
| PIPE-21 | A | the direct evaluation returns the field on the target projected into the field map's space and made positive: the projected affine, and for a target with an axis against its world axis (a LAS BOLD reference) the voxels reversed along it, while `ResampleSeries` reads the field voxel for voxel on the target's grid: a mirrored field (a LAS target with the identity: up to 159 Hz from the field on the target's grid in `check_bold_steps.py`'s `synth`) | the same values on the target's grid: its voxel order and affine (equal to the resampled path's field) | fMRIPrep's image bit for bit |

Only the direct path is affected; none of the earlier oracle runs' 28 nodes takes it (a
registration's rotation fails `aligned`): PIPE-21 changes nothing there.

## Differences and limits

- A series of affines in `transforms` (a head-motion file) raises `ValueError`: it cannot place
  a field map (nitransforms would treat the series as one affine of the wrong shape).
