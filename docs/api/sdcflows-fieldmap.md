# sdcflows' field-map estimation

`lx.sdcflows` (Rust: `larmorx_sdcflows`): sdcflows' B-spline field-map model and the
phase-difference preparation, the steps fMRIPrep runs to turn a field map into the smooth
field (Hz) that corrects susceptibility distortion. This page covers:

- the B-spline control grid and its weights (`bspline_grid`, `grid_bspline_weights`,
  `collocation_matrix`), and scikit-learn's ridge solver on them (`ridge_sparse_cg`);
- the fit of a field map (`bspline_approx`: `BSplineApprox`), its resampling helpers
  (`resample_to_zooms`, `rescale_affine`, `deoblique_and_zooms`);
- the field given by its coefficients, evaluated and applied (`B0FieldTransform`,
  `apply_coeffs_field`: `ApplyCoeffsField`), field maps and displacement fields
  (`fmap_to_disp`, `disp_to_fmap`, `displacements_field_to_fieldmap`), the coefficients'
  orientation (`topup_coeff_reorient`, `transform_coefficients`, `reorient_pedir`);
- the phase-difference preparation (`au2rads`, `subtract_phases`, `delta_te`, `phdiff2fmap`,
  `check_b0_units`, `check_register`, `phdiff_workflow`: `init_phdiff_wf` with PRELUDE
  through `lx.fsl.prelude`).

**Port.** sdcflows is Apache-2.0 (the NiPreps developers), so this is a port in the main
package, bit for bit: sdcflows 2.17.1 (the newest release; fMRIPrep 25.2.6 runs 2.15.1, whose
code on these paths is byte-identical), with the arithmetic of the libraries it calls:
scikit-learn 1.9.1's `Ridge` (`sparse_cg`, BSD-3-Clause), SciPy 1.18.1's `cg`, `BSpline`,
sparse matrices and `ndimage` (BSD-3-Clause; the same on these paths in 1.15.3), numpy 2.5.4's
reductions and dot products over OpenBLAS 0.3.34's Haswell kernels (BSD-3-Clause; the same in
2.3.5 with 0.3.30), nibabel 5.4.2 and nitransforms 25.1.0 (MIT), niworkflows 1.15.3's
`reorient_image` (Apache-2.0). Provenance: [larmorx-sdcflows](../../crates/larmorx-sdcflows/PROVENANCE.md).
A clean-room original of these programs comes later (the user, 2026-11-06), in another crate.

**Status: `validated`** against sdcflows itself, run in-process, in both kept environments
(`oracles/python-newest`: sdcflows 2.17.1, scikit-learn 1.9.1, SciPy 1.18.1, numpy 2.5.4;
the shared `.venv`: SciPy 1.15.3, numpy 2.3.5) and on fMRIPrep 25.2.6's own files of the oracle
runs (see the [validation record](../validation/sdcflows-fieldmap.md): 37 cases, 821 checks,
every output **bit-identical**; the checks in
[validation/investigations/sdcflows-fieldmap](../../validation/investigations/sdcflows-fieldmap/README.md);
the findings in [sdcflows-fieldmap.md](../findings/sdcflows-fieldmap.md)). The record's step
from the shared releases to the newest: 4 of its 37 cases changed in sdcflows' own outputs, all
from numpy's LAPACK 3.12.0 → 3.12.1 (the quaternions nibabel writes, an orientation tie),
which larmorx reproduces in each release.

**Speed:** the fit of fMRIPrep's field maps (weights and ridge solve) takes 0.3 to 3.8 s on 14
threads where scikit-learn takes 1.9 to 28 s (ds002603's 128³ field map: 2,097,152 voxels ×
9,072 coefficients, 126 million stored weights: 28.1 s against 3.8 s), with the same bits; the
collocation matrix is applied without being stored (sdcflows holds it at least three times).

## Quick start

```python
import larmorx as lx

# fMRIPrep's phase-difference field map: phase to radians, PRELUDE, Hz, then the B-spline fit
prep = lx.sdcflows.phdiff_workflow(
    "sub-01_magnitude1_brain.nii.gz",            # the magnitude reference (part B prepares it)
    ["sub-01_phasediff.nii.gz"],
    [{"EchoTime1": 0.00492, "EchoTime2": 0.00738}],
    "sub-01_magnitude1_brain_mask.nii.gz",
    out_dir="work",
)
fit = lx.sdcflows.bspline_approx(prep.fieldmap.image, "sub-01_brain_mask.nii.gz",
                                 out_dir="work", n_threads=8)
field = fit.extrapolated                         # what fMRIPrep keeps: the field everywhere

# the field on a BOLD reference, and a series unwarped with it
xf = lx.sdcflows.B0FieldTransform(coeffs=list(fit.coefficients))
xf.fit("sub-01_boldref.nii.gz")
corrected = xf.apply("sub-01_bold.nii.gz", pe_dir="j-", ro_time=0.0425, n_threads=8)
```

Inputs are NIfTI paths, `lx.Image`s, nibabel images or `(array, affine)` pairs. A path is read
as sdcflows reads it (nibabel's `get_fdata(dtype="float32")`, scaling applied). Functions that
write files take `out_dir` and name them as sdcflows does (`<input>_field.nii.gz`, ...); without
it they return images only. `n_threads` (0 = all logical CPUs) never changes a result.

## The B-spline grid and its weights

| larmorx | sdcflows 2.17.1 | Notes |
|---|---|---|
| `bspline_grid(img, control_zooms_mm=(40, 40, 20), *, upstream_defects="fix")` | `interfaces/bspline.py: bspline_grid(img, control_zooms_mm)` | the control grid as a zero-filled float32 `lx.Image`: the image's orientation, `control_zooms_mm` apart, `extent // spacing + 3` points per axis, centred. A 4D header of one volume: SDC-06 |
| `grid_bspline_weights(target, ctrl, dtype="float32", *, n_threads=1, upstream_defects="fix")` | `transform.py: grid_bspline_weights(target_nii, ctrl_nii, dtype)` | the cubic B-spline weights of every target voxel (rows) for every control point (columns) as a `CsrMatrix` (`data`, `indices`, `indptr`, `shape`; `to_scipy()`, `toarray()`), sdcflows' stored entries included (scipy's `kron` stores an axis densely, explicit zeros included, when its weights fill half of it). sdcflows returns COO or BSR; this is the same matrix as CSR. SDC-01, SDC-02 |
| `collocation_matrix(target, levels, dtype="float32", *, n_threads=1, upstream_defects="fix")` | `sparse_hstack([grid_bspline_weights(target, l) for l in levels]).tocsr()` (`BSplineApprox`, `B0FieldTransform.fit`, fMRIPrep's `ReconstructFieldmap`) | levels side by side |
| `ridge_sparse_cg(X, y, alpha=1.0, *, tol=1e-4, max_iter=None, n_threads=1)` → `RidgeResult(coef, intercept, info, n_iter, system)` | scikit-learn `Ridge(alpha, fit_intercept=False, solver="auto")` on a sparse `X` (`_solve_sparse_cg`, SciPy's `cg`) | float32 only, no intercept (the only path sdcflows reaches); both of scikit-learn's systems (`X^T X + αI` when samples ≥ features, the kernel system on samples otherwise); `max_iter` is ignored on the kernel system, as scikit-learn ignores it |
| `DEFAULT_ZOOMS_MM`, `DEFAULT_LF_ZOOMS_MM`, `DEFAULT_HF_ZOOMS_MM`, `NOT_ALIGNED` | `DEFAULT_ZOOMS_MM` (40, 40, 20), `DEFAULT_LF_ZOOMS_MM` (100, 100, 40), `DEFAULT_HF_ZOOMS_MM` (16, 16, 10); the warning's text | fMRIPrep uses the high-frequency grid |

Warnings: sdcflows' `UserWarning` "Image's and B-Spline's grids are not aligned." with
`upstream_defects="reproduce"` (sdcflows warns and goes on); corrected, the weights are exact
and an `lx.UpstreamDefectWarning` (SDC-02) says so.

## `bspline_approx`: `BSplineApprox`

`lx.sdcflows.bspline_approx(in_data, in_mask=None, bs_spacing=((16, 16, 10),), ridge_alpha=1e-4,
recenter="median", extrapolate=True, zooms_min=1.0, debug=False, *, out_dir=None, name=None,
n_threads=1, upstream_defects="fix", low_memory=False)` → `BSplineApproxResult`.

| `BSplineApprox` input | larmorx | Meaning |
|---|---|---|
| `in_data` (mandatory) | `in_data` | the field map (Hz), 3D (a 4D header of one volume: SDC-06) |
| `in_mask` | `in_mask` | voxels whose value, **scaled** by the header (nibabel's `dataobj`), is above 1e-4; every voxel without one |
| `bs_spacing` | `bs_spacing` | one 3-tuple (mm) per control grid |
| `ridge_alpha` | `ridge_alpha` | `Ridge(alpha, fit_intercept=False)` |
| `recenter` | `recenter` | `"median"`, `"mode"` (SciPy's: the smallest most frequent value), `"mean"` (numpy's pairwise float32 sum) or `False`: subtracted inside the mask before the fit |
| `extrapolate` | `extrapolate` | also evaluate the fit outside the mask |
| `zooms_min` | `zooms_min` | a finer field map (and its mask) is resampled to `max(zooms, zooms_min)` first (`resample_to_zooms`), fitted there and evaluated on the original grid; 0 never resizes |
| `debug` | `debug` | accepted; sdcflows' interface never reads it |
| (none) | `low_memory` | the fit reads the stored collocation matrix (the default: its rows, and with several threads their transpose: 8 or 16 bytes per weight, below the 24 or more scikit-learn holds for sdcflows' fit; faster than scikit-learn at one thread); `True` evaluates the weights at every product of the solver (memory of the order of the voxels and knots, slower); the same bits either way |
| outputs `out_field`, `out_coeff`, `out_error`, `out_extrapolated`, `out_intercept` | `field`, `coefficients`, `error`, `extrapolated` (images), `intercept` (0.0); with `out_dir`: `out_field`, `out_coeff`, `out_error`, `out_extrapolated` (paths); also `center`, `n_inside`, `n_iter` | the field inside the mask (0 outside; `cal_max` = max \|field\|, `cal_min` = −`cal_max`); the coefficients of each grid (`<in>_coeff000.nii.gz`, ...); `(data − field) × mask` in the input's stored type, scaled as nibabel writes it (`lx.io.save_nibabel`; `cal_min` 0, `cal_max` its maximum); the field everywhere (`<in>_extra`, or the field's own file when the mask holds every voxel; absent without `extrapolate`) |

Log records on the `larmorx.sdcflows` logger, with sdcflows' texts: "Resampling image with
resolution exceeding 'zooms_min' (A → B)." (info), "Approximating B-Splines grids (and
AxBxC [knots]) on a grid of XxYxZ (N) voxels, of which M fall within the mask." (info), "Model
fit attempt 0: max(|coeffs|) = …" (debug; the number in Python's float repr). A fit whose
largest coefficient reaches 1e4 raises sdcflows' `RuntimeError` "Spline fit of input file …
failed. Extreme value … detected in spline coefficients." (sdcflows tries three times on the
same inputs; the fit is deterministic, so larmorx fits once, with the same outcome and records).

Files and images are read and written with nibabel's own arithmetic (`lx.io.load_nibabel`,
`lx.io.save_nibabel`): a qform-only input's affine as nibabel reads it, every written qform as
nibabel's `set_qform` stores it (numpy's SVD and eigh replicated), and orientations by nibabel's
`io_orientation` with numpy's SVD. A mask of a 4D field map of one volume may be 3D (SDC-06
corrected). scikit-learn's message for an infinite value names numpy's dtype as sdcflows' data
has it (`dtype('<f4')` for an unscaled little-endian float32 file, as nibabel returns the raw
array).

## The field given by its coefficients

### `B0FieldTransform(coeffs)`

`coeffs`: the coefficient files or images of one or more control grids (as `BSplineApprox` and
`TOPUPCoeffReorient` write them). `mapped`: the field after `fit`.

- **`fit(target_reference, xfm_data2fmap=None, approx=True, *, n_threads=1,
  upstream_defects="fix")`**: the field on the target's grid. `xfm_data2fmap` (4 × 4 RAS+)
  maps the target into the field map's space; the target's grid is projected through it and
  its axes made positive (`ensure_positive_cosines`). With `approx`, a transform given and a
  target rotated relative to the last grid, the field is evaluated on a grid aligned with the
  coefficients, 4 times finer, covering the target (`deoblique_and_zooms`), and resampled onto
  the target (cubic, `constant` 0). `mapped` has the target's header with intent "estimate"
  named "fieldmap Hz", float32, `cal_max`/`cal_min`. Returns `True`. SDC-01, SDC-02, SDC-10.
- **`apply(moving, pe_dir, ro_time, xfms=None, jacobian=True, xfm_data2fmap=None,
  approx=True, order=3, mode="constant", cval=0.0, prefilter=True, output_dtype=None,
  num_threads=None, allow_negative=False, *, n_threads=1, upstream_defects="fix")`**: the
  series unwarped (sdcflows' `unwarp_parallel`/`_sdc_unwarp`): per volume, the voxel shift
  `field × ro` along the phase-encoding axis (`ro` negated where the axis runs to L, P or I or
  the direction ends with `-`), SciPy's `map_coordinates` (`order`, `mode`, `cval`,
  `prefilter`), times `1 + ∂shift/∂pe` with `jacobian`, values below 0 set to `cval` unless
  `allow_negative`, in `output_dtype` (default: the input's stored type, written as nibabel
  scales it), back in the input's orientation (niworkflows' `reorient_image`). Always 4D (a 3D
  input gives one volume), as sdcflows. Warnings, as sdcflows: "The fieldmap has been already
  fit, the user is responsible for ensuring the parameters of the EPI target are consistent."
  and, with `xfms`, "Head-motion compensating (realignment) transforms are ignored when
  applying the unwarp with SDCFlows. This feature will be enabled as soon as unit tests are
  implemented for its quality assurance." `num_threads` is accepted (sdcflows' concurrency) and
  has no effect; `n_threads` sets the threads. SDC-05, SDC-08 (and `fit`'s).
- **`to_displacements(ro_time, pe_dir, itk_format=True)`**: `fmap_to_disp(mapped, ...)`.

### `apply_coeffs_field`: `ApplyCoeffsField`

`apply_coeffs_field(in_data, in_coeff, ro_time, pe_dir, *, fmap2data_xfm=None,
data2fmap_xfm=None, in_xfms=None, jacobian=False, num_threads=None, approx=True, out_dir=None,
n_threads=1, upstream_defects="fix")` → `ApplyCoeffsFieldResult(corrected, field,
out_corrected, out_field)`.

| `ApplyCoeffsField` input | larmorx |
|---|---|
| `in_data`, `in_coeff` (list) | `in_data`, `in_coeff` |
| `fmap2data_xfm` xor `data2fmap_xfm` (ITK transform files; the first of a list) | the same; read with nitransforms' `Affine.from_filename(fmt="itk")` conventions, `fmap2data` inverted |
| `in_xfms` | `in_xfms` (ignored by `apply`, with its warning) |
| `ro_time`, `pe_dir` (lists) | `ro_time`, `pe_dir` |
| `jacobian` (False), `num_threads`, `approx` (True) | the same |
| outputs `out_corrected` (`<in>_sdc`), `out_field` (`<in>_field`) | `corrected`, `field`; the files with `out_dir` |

### Field maps and displacement fields

| larmorx | sdcflows | Notes |
|---|---|---|
| `fmap_to_disp(fmap_nii, ro_time, pe_dir, itk_format=True)` | `transform.py: fmap_to_disp` | a 5D `(X, Y, Z, 1, 3)` float64 displacement field (mm), intent "vector" named "SDC"; ITK's LPS signs with `itk_format` |
| `disp_to_fmap(xyz_nii, epi_nii, ro_time, pe_dir, itk_format=True)` | `transform.py: disp_to_fmap` | the field map (Hz) of a displacement field, intent "estimate" named "Delta_B0 [Hz]"; a readout time of 0 gives infinities and NaN, as sdcflows |
| `displacements_field_to_fieldmap(transform, epi, ro_time, pe_dir, demean=False, itk_transform=True, *, out_dir=None)` → `DisplacementsResult(fieldmap, out_file)` | `interfaces/fmap.py: DisplacementsField2Fieldmap` | `demean` subtracts the median (the header keeps the `cal_max` from before, as sdcflows); `<transform>_Hz` |

### The coefficients' orientation

| larmorx | sdcflows | Notes |
|---|---|---|
| `topup_coeff_reorient(in_coeff, fmap_ref, pe_dir, *, out_dir=None, upstream_defects="fix")` → `CoefficientsResult(coefficients, out_coeff)` | `interfaces/bspline.py: TOPUPCoeffReorient` (`_fix_topup_fieldcoeff`) | TOPUP's coefficients on a grid centred on the reference (RAS); `ValueError` "Shape of coefficients file … does not meet the expected shape … (toupup factors are …)." as sdcflows; `<coeff>_fixed`. SDC-07 |
| `transform_coefficients(in_coeff, fmap_ref, transform, fmap_target=None, *, out_dir=None, n_threads=1)` → `CoefficientsResult` | `interfaces/bspline.py: TransformCoefficients` (`transform.py: _move_coeff`) | the coefficients' affine moved by an ITK rigid transform; `fmap_target` writes sdcflows' debug image `debug_fmapref.nii.gz` (into `out_dir`, or the working directory as sdcflows); `<coeff>_space-target` |
| `reorient_pedir(pe_dir, source_ornt, target_ornt=None)` | `utils/tools.py: reorient_pedir` | the phase-encoding direction in another orientation (BIDS letters and FSL's `x`/`y`/`z`) |
| `deoblique_and_zooms(in_reference, oblique, factor=4, padding=1, factor_tol=1e-4)` | `utils/tools.py: deoblique_and_zooms` | a grid with the reference's orientation covering every corner of `oblique` plus `padding` voxels, `factor` times finer |
| `resample_to_zooms(in_file, zooms, order=3, prefilter=True, *, n_threads=1, upstream_defects="fix")` | `utils/tools.py: resample_to_zooms` | nibabel's `rescale_affine` and nitransforms' `Affine(reference=...).apply` (3D and 4D images of one volume); a grid of one voxel: SDC-11 |
| `rescale_affine(affine, shape, zooms, new_shape=None)` | nibabel `affines.rescale_affine` | |

## The phase-difference preparation

| larmorx | sdcflows | Messages and failures |
|---|---|---|
| `au2rads(in_file, *, out_dir=None, upstream_defects="fix")` → `PhaseResult(image, out_file, metadata)` | `PhaseMap2rads` (`utils/phasemanip.py: au2rads`): `(d − min) × 2π / (max − min)`, float32, clipped to [0, 2π]; `<in>_rads` | a constant phase map: SDC-04 |
| `subtract_phases(in_phases, in_meta, *, out_dir=None, upstream_defects="fix")` | `SubtractPhases`: one map passed through; two ordered by echo time, `phase2 − phase1`, negatives + 2π, clipped; metadata merged with `EchoTime1`/`EchoTime2`; `<phase1>_phdiff` | "Length of input phase-difference maps and metadata files should match."; no `EchoTime`: SDC-09 |
| `delta_te(in_values)` | `utils/phasemanip.py: delta_te` | "Phase/phase-difference fieldmaps: no echo-times information.", "Could not interpret metadata <EchoTimeDifference=…>.", "Could not interpret metadata <EchoTime(1,2)=(…)>." |
| `phdiff2fmap(in_file, delta_te=None, *, metadata=None, out_dir=None)` | `Phasediff2Fieldmap` (`phdiff2fmap`): `phase / (2π ΔTE)` in float32; `<in>_fmap` | `delta_te` or `metadata` needed |
| `check_b0_units(in_file, units, *, out_dir=None)` | `CheckB0Units`: `Hz` passed through; `rad/s` divided by 2π (scaling applied) and stored in the input's type as nibabel scales it; `<in>_Hz` | `units` must be `Hz` or `rad/s` |
| `check_register(mag_files, fmap_files, rot_thresh=0.02, trans_thresh=1.0, *, out_dir=None)` → `CheckRegisterResult(mag_files, fmap_files)` | `CheckRegister` (`_check_gross_geometry`, `_conform_img`) | "Images have shape mismatch: …", "Images have orientation mismatch: …" (logged as critical, raised as `ValueError`); images whose affine differs from the first magnitude's are rewritten with it ("Copying affine to …"). **Two magnitudes with different affines** need FreeSurfer's `mri_robust_register`: `NotImplementedError` until the FreeSurfer part (no oracle run reaches it) |
| `phdiff_workflow(magnitude, phase, phase_meta, mask, *, out_dir=None, prelude_implementation="auto", n_threads=1, upstream_defects="fix")` → `PhdiffWorkflowResult(rads, phase_diff, unwrapped, fieldmap)` | `workflows/fit/fieldmap.py: init_phdiff_wf`: `PhaseMap2rads` per map, `SubtractPhases`, FSL's PRELUDE (nipype), `Phasediff2Fieldmap` | PRELUDE through `lx.fsl.prelude` (`prelude_implementation`: `"auto"`, `"original"`, `"replica"`); its output written as `<phase>_unwrapped` |

`init_fmap_wf` (the whole estimator: magnitude preparation, brain mask, the fit) needs part B's
magnitude preparation and masks; until then `phdiff_workflow` and `bspline_approx` take the
magnitude reference and mask as inputs.

## Upstream defects (D13)

Corrected by default; `upstream_defects="reproduce"` gives sdcflows' result bit for bit, and
an `lx.UpstreamDefectWarning` names the defect whenever it changes (or, reproduced, would have
changed) a result. Rules A to D and the evidence:
[findings](../findings/sdcflows-fieldmap.md), [register](../findings/upstream-defects.md).

| ID | sdcflows does | larmorx by default | Where |
|---|---|---|---|
| SDC-01 | a position beyond the first or the last control point gets SciPy's extrapolated end polynomial (−3.27 instead of 0.0208 at 1.5 spacings beyond) | the cubic B-spline's weight (the same bits inside the grid) | `grid_bspline_weights`, `collocation_matrix`, `B0FieldTransform.fit`/`apply`, `apply_coeffs_field`, `bspline_approx` |
| SDC-02 | a rotated target: per-axis weights from one index each (warns and goes on) | the weights at every voxel's own position | the same |
| SDC-03 | `CheckRegister`'s translation test is signed (a large negative shift passes) | (in the FreeSurfer branch, not ported yet) | `check_register` |
| SDC-04 | a constant phase map becomes NaN everywhere | `ValueError` | `au2rads`, `phdiff_workflow` |
| SDC-05 | an integer or numpy float32 readout time fails (`TypeError`) | accepted as the float | `B0FieldTransform.apply`, `apply_coeffs_field` |
| SDC-06 | a 4D header of one volume fails (numpy's broadcasting `ValueError`) | the first three zooms (several volumes: a clear error) | `bspline_grid`, `bspline_approx` |
| SDC-07 | TOPUP's coefficients negated for a BIDS `i` direction (the field inverted) | never negated | `topup_coeff_reorient` |
| SDC-08 | a positive-axes series read from a file fails (`AttributeError`) | unwarped | `B0FieldTransform.apply`, `apply_coeffs_field` |
| SDC-09 | no `EchoTime`: Python's `TypeError` of a comparison with None | a `ValueError` naming the metadata | `subtract_phases`, `phdiff_workflow` |
| SDC-10 | `fit(approx=True)` on a rotated target ignores `xfm_data2fmap` | the approximation built and sampled where the transform puts the target | `B0FieldTransform.fit`/`apply`, `apply_coeffs_field` |
| SDC-11 | a resize (`zooms_min`) onto a grid of a single voxel fails (nitransforms' bare `AssertionError`) | the voxel resampled and the fit made | `resample_to_zooms`, `bspline_approx` |

fMRIPrep 25.2.6 reaches none of them on its own paths (it reconstructs fields with its own
`ReconstructFieldmap` on the field map's grid, passes FSL's letters to `TOPUPCoeffReorient`,
and always has `EchoTime`): measured on the oracle runs (the findings).

## Rust API

`crates/larmorx-sdcflows`: `bspline` (`bspline_grid`, `AxisWeights`, `grid_bspline_weights`
with `UpstreamDefects`, `LevelWeights`, `ExactLevel`, `Collocation`: `matvec_f32`,
`rmatvec_f32`, `to_csr`), `blas` (numpy's float32 `dot` over OpenBLAS's Haswell `sdot`,
`norm`), `ridge` (SciPy's `cg`, scikit-learn's `sparse_cg` on a `Collocation` or a stored CSR),
`approx` (`bspline_approx_logged`, `ApproxParams`, `Approx`), `tools` (`rescale_affine`,
nitransforms' affine-resampling coordinates, nibabel's `apply_affine`), `unwarp`
(`unwarp_volume`), `npy` (numpy's and Python's small rules: `floor_divide`, the float repr).

## Command line

None for these interfaces (nipype interfaces have no program or original arguments), as for the
resampler. sdcflows' own BIDS app (`sdcflows bids_dir output_dir participant`) runs whole
workflows with part B's steps: it is ported at the end of part B.

## Differences and limits

- Not ported (private, used by no workflow): `_split_itk_file`, `_b0_resampler` (whose
  head-motion branch fails in sdcflows itself: `unwarp.xfm` on a slots class).
- `apply`'s `num_threads` (asyncio concurrency in sdcflows) has no effect; volumes are unwarped
  one after the other with `n_threads` threads each, so memory holds one volume's coordinates.
- `bspline_approx` replaces `_field.` in the output file's **name** only (sdcflows replaces it
  in the whole path: different only when a directory name holds `_field.`).
- numpy's 16-byte `intent_name` truncates "B-Spline coefficients" to "B-Spline coeffic", kept.
- The BLAS kernel: numpy's float32 dot products follow OpenBLAS's Haswell `sdot` on every
  platform (the convention of [numpy-blas.md](../findings/numpy-blas.md)); fMRIPrep's image picks
  another kernel on AVX-512 machines (Cooperlake), so it is run with `OPENBLAS_CORETYPE=Haswell`
  for comparisons.
