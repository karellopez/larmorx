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

**Speed** ([benchmark](../benchmarks/sdcflows-fieldmap.md), every output byte-identical):
`BSplineApprox` on fMRIPrep's seven `bs_filter` calls takes 0.30 to 4.0 s at 14 threads where
sdcflows takes 2.0 to 27.5 s (6.5 to 7.8 times faster; ds002603's 128³ field map: 2,097,152
voxels × 9,072 coefficients, 4.0 s against 27.5 s), and at one thread 1.09 to 1.14 times
faster than scikit-learn in a fresh process (the collocation matrix stored once at one thread;
with several threads the weights are evaluated in parallel at every product, faster there and
holding no matrix; sdcflows holds it at least three times); `ApplyCoeffsField` on 60 volumes of
ds005454 1.7 times faster at 1 and 14 threads.

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
| (none) | `low_memory` | by default the fit reads the stored collocation matrix at one thread (its rows: 8 bytes per weight, below the 24 or more scikit-learn holds for sdcflows' fit; faster than scikit-learn) and evaluates the weights at every product with several threads (faster there: parallel, no matrix held); `True` evaluates them at every product also at one thread (memory of the order of the voxels and knots, slower); the same bits either way |
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

## The brain-extraction, magnitude and field-map workflows (part B)

sdcflows' graphs as plain functions (`python/larmorx/sdcflows/workflows.py`): each node runs
as nipype runs it, writes its files where nipype's node folder holds them under `out_dir`
(without it, a temporary folder: only the images are returned) and the next node reads them
back. `numpy_version` and `scipy_version` select the libraries' arithmetic where their releases
differ (`IntensityClip`'s and `brain_masker`'s percentiles, numpy 2.2's buffered sums, the
random walker's row order; fMRIPrep 25.2.6's image: `"2.2.6"`, `"1.15.2"`); `upstream_defects`
reaches every node (by default ANTs' N4 leaves the clipped image's zero background out of its
fit: ANTS-40); `n_threads` never changes a result.

| larmorx | sdcflows | Nodes (folders) and outputs |
|---|---|---|
| `brainextraction_workflow(in_file, bspline_dist=None, *, out_dir=None, numpy_version, scipy_version, upstream_defects="fix", n_threads=1)` → `BrainExtractionWorkflowResult` | `ancillary.init_brainextraction_wf` (`inputnode.bspline_dist`) | `clipper_pre` (`IntensityClip()`), `n4` (`N4BiasFieldCorrection -d 3 --convergence [ 50x50x50x50x50, 1e-07 ] --shrink-factor 4`, `--bspline-fitting [ %g ]` with `bspline_dist`; nipype's `copy_header`: ANTs' values in the input's header and stored type), `clipper_post` (`IntensityClip(p_min=0.01, p_max=99.9)`), `masker` (`BrainExtraction`); `out_file`, `out_brain`, `out_mask`, `out_probseg` |
| `magnitude_workflow(magnitude, *, out_dir=None, ...)` → `MagnitudeWorkflowResult` | `fit/fieldmap.py: init_magnitude_wf` | `magmrg` (`IntraModalMerge(hmc=False, to_ras=False)`: `lx.niworkflows.intra_modal_merge`), `brainextraction_wf`; `fmap_ref`, `fmap_mask`, `fmap_probseg` |
| `fmap_workflow(magnitude, fieldmap, mode="phasediff", *, sloppy=False, debug=False, out_dir=None, numpy_version, scipy_version, prelude_implementation="auto", upstream_defects="fix", n_threads=1)` → `FmapWorkflowResult` | `fit/fieldmap.py: init_fmap_wf(sloppy, debug, mode)` (`inputnode.magnitude`, `inputnode.fieldmap`: `(path, metadata)` pairs) | `check_register`, `magnitude_wf`; `"phasediff"`: `phdiff_wf` (`phdiff_workflow`, PRELUDE's file named `<phase>_unwrapped.nii.gz` as nipype's FSL output type gives it); any other mode (sdcflows' direct branch): `fmapmrg` (`IntraModalMerge(zero_based_avg=False, hmc=False, to_ras=False)`), `units` (`CheckB0Units` with the first map's `Units`: `KeyError('Units')` without); `bs_filter` (`BSplineApprox(bs_spacing=[(16, 16, 10)], extrapolate=not debug, zooms_min=4 if sloppy)` in `fmap_mask`); `fmap` (the extrapolated field; the fit in `debug`), `fmap_ref`, `fmap_mask`, `fmap_coeff`, `method` |

Checked node file by node file against sdcflows' own nipype graphs (`upstream_defects="reproduce"`;
ANTs' N4 at one thread with correctly rounded maths and FSL's PRELUDE through the oracle cache):
`validation/investigations/sdcflows-fieldmap-b/check_workflows.py`.

## The PEPOLAR estimation (part B)

sdcflows' `init_topup_wf` and its steps (`python/larmorx/sdcflows/pepolar.py`), in **both
wirings**: `wiring="2.17"` (sdcflows 2.17.1, the default) and `wiring="2.15"` (sdcflows 2.15,
fMRIPrep 25.2.x's; 2.15.1 and 2.15.2 are identical on this file). Nodes run and write as in
`fmap_workflow` (above); the programs underneath are larmorx's: AFNI's `3dvolreg` (the replica
by default, `volreg_implementation`), FSL's `topup` (the replica when `larmorx-nc` is
installed, else the clean-room original: `topup_implementation`).

| larmorx | sdcflows | Notes |
|---|---|---|
| `get_trt(in_meta, in_file=None, *, use_estimate=False, fallback=None)` | `utils/epimanip.get_trt` | `TotalReadoutTime` (or `EstimatedTotalReadoutTime` with `use_estimate`) as given (falsy: `ValueError("'<value>'")`); with `PhaseEncodingDirection` and one of the echo-spacing keys, the voxels along the phase encoding of `in_file`: `EES (N - 1)`, `EchoSpacing (N // ParallelReductionFactorInPlane - 1)`, Philips' `WaterFatShift / (wfs_hz (EPIFactor + 1)) (N - 1)`; then a truthy `fallback`; else `ValueError('Unknown total-readout time specification')` |
| `get_readout_time(metadata, in_file=None, use_estimate=False, fallback=None)` → `GetReadoutTimeResult(readout_time, pe_direction, pe_dir_fsl)` | `interfaces/epi.GetReadoutTime` | `fallback` 0 is none; no `PhaseEncodingDirection`: `KeyError` |
| `select_pe_volumes(in_data, pe_dirs_fsl, readout_times, max_vols_per_pe=3, *, out_dir)` → `SelectPEVolumesResult` | `interfaces/epi.SelectPEVolumes` (2.17.1) | the runs in order, squeezed and split; at most `max_vols_per_pe` volumes per direction, `<in>_vol<i>`; lists of different lengths: `zip(strict=True)`'s `ValueError` |
| `sort_pe_blips(in_data, pe_dirs_fsl, readout_times)` → `SortPEBlipsResult(out_data, pe_dirs, pe_dirs_fsl, readout_times)` | `interfaces/epi.SortPEBlips` | sorted by the blip (`+` before `-`), the readout time, the path |
| `uniform_grid(in_data, reference=0, *, out_dir)` → `UniformGridResult(out_data, reference, resampled)` | `interfaces/utils.UniformGrid` | an equal grid kept, a close affine (`np.allclose`) replaced by the reference's, any other grid resampled (nitransforms' cubic `Affine.apply`, `constant` 0, the input's type), `<in>_regrid<i>`; the warning of 2.17.1 for a rotation that differs; 3D inputs only on the resampling path (`NotImplementedError` for 4D: the paths sdcflows runs give 3D) |
| `pad_slices(in_file, axis=2, *, out_dir)` → `(file, padded)` | `interfaces/utils.PadSlices` (2.15's wiring) | an odd count along `axis`: one empty slice appended, `<in>_padded` |
| `reorient_image_and_metadata(in_file, target_orientation, pe_dir, *, out_dir, upstream_defects="fix")` → `(file, pe_dirs)` | `interfaces/utils.ReorientImageAndMetadata` | `<in><TARGET>`, or the input when nothing moves; letters other than RASLPI: `ValueError('Invalid orientation code …')`; SDC-12 |
| `select_topup_config(in_file, sloppy=False, *, fsldir=None)`, `topup_config_file(name, fsldir=None)` | `workflows/fit/pepolar._select_topup_config` | `b02b0_4.cnf` (every dimension divisible by 4), `_2` (all even), `_1`; `_quick` with `sloppy`. sdcflows' own files (`b02b0.cnf`, `b02b0_quick.cnf`, `b02b0_{1,2,4}_quick.cnf`, Apache-2.0) ship with larmorx; FSL's `b02b0_{1,2,4}.cnf` (FSL Licence) do not: they are read from `fsldir`'s `etc/flirtsch`, else from `larmorx-nc`'s FSL files, else `FileNotFoundError` naming both (the user's D9, for now) |
| `topup_node(in_file, encoding_direction, readout_times, config, *, out_dir, fsldir=None, implementation="auto", upstream_defects="fix", n_threads=1)` → `TopupNodeResult` | nipype 1.12.0's `TOPUP` | `topup --config --datain=<in>_encfile.txt --imain --out=<in>_base --iout --fout --jacout=jac --logout --rbmout=xfm --dfout=warpfield` in `out_dir`, `FSLOUTPUTTYPE=NIFTI_GZ`; the encoding file as nipype's (`"%d %d %d %.8f"`; one readout time for all rows, else one per row: nipype's `ValueError`) |
| `topup_mats_to_world(in_mats, in_reference)` | `workflows/fit/pepolar._topup_mats_to_world` (2.17.1) | nitransforms' FSL matrices as RAS affines (`.tolist()`) |
| `topup_workflow(in_data, metadata, *, wiring="2.17", grid_reference=0, use_metadata_estimates=False, fallback_total_readout_time=None, omp_nthreads=1, sloppy=False, debug=False, topup_config=None, max_vols_per_pe=3, out_dir=None, fsldir=None, numpy_version=None, scipy_version=None, topup_implementation="auto", volreg_implementation="auto", upstream_defects="fix", n_threads=1)` → `TopupWorkflowResult` | `workflows/fit/pepolar.init_topup_wf` | 2.17: `readout_time`, `select_volumes`, `regrid`, `sort_pe_blips`, `concat_blips` (`MergeSeries(affine_tolerance=1e-4)`), `to_las`, `select_config` (unless `topup_config`), `topup`, `fix_coeff` (on the regridded reference), `from_las`, `from_las_fmap`, `ref_average` (`RobustAverage`), `brainextraction_wf`; 2.15: `runwise_avg` (`RobustAverage` per run), `regrid`, `readout_time` (on the regridded averages), `sort_pe_blips`, `concat_blips`, `pad_blip_slices`, `setwise_avg` (`RobustAverage` of the set), `to_las`, `topup` (sdcflows' `b02b0.cnf`), `fix_coeff` (on the set's average), `from_las`, `from_las_fmap`, `ref_average`, `brainextraction_wf`; `debug`: `unwarp` (`ApplyCoeffsField(jacobian=True)`, with 2.17.1's `convert_xfms` matrices) and `concat_corrected` instead of TOPUP's corrected volumes and field. Outputs `fmap`, `fmap_ref`, `fmap_mask`, `fmap_coeff`, `jacobians`, `xfms`, `out_warps`, `method`, `topup_config`, and every node's result (`nodes`) |

Checked node file by node file against sdcflows' own graphs of both releases (2.15.1's
`pepolar.py` from its release, 2.17.1 installed; 3dvolreg, topup and N4 through the oracle
cache) and against fMRIPrep 25.2.6's own node files of its two PEPOLAR estimators:
`validation/investigations/sdcflows-fieldmap-b/check_pepolar.py`
([findings](../findings/sdcflows-estimation.md)).

## The field map's registration to the EPI (part B)

`coeff2epi_workflow` is sdcflows' `init_coeff2epi_wf` (`python/larmorx/sdcflows/registration.py`;
the same in 2.15.1 and 2.17.1): both masks dilated by a ball of radius 5 (`BinaryDilation`), then
ANTs' `antsRegistration` with sdcflows' `fmap-any_registration.json` (two rigid stages, Mattes,
`LanczosWindowedSinc`) through niworkflows' `FixHeaderRegistration` command line, the field map's
reference fixed and the target moving, the dilated masks per stage.

| larmorx | sdcflows | Notes |
|---|---|---|
| `target_ref`, `target_mask`, `fmap_ref`, `fmap_mask`, `fmap_coeff=None` | `inputnode` | `fmap_coeff` is passed through |
| `sloppy=False` | `sloppy` | `fmap-any_registration_testing.json` |
| `debug=False` | `debug` | the warped and inverse-warped images too (`transform_Warped.nii.gz`, `transform_InverseWarped.nii.gz`), their x-forms copied from the fixed and the moving image (`copy_xform`, `descrip` naming `niworkflows_version`) |
| `write_coeff=False` | `write_coeff` | a warning only (deprecated), as sdcflows |
| `ants_random_seed=None` | the environment's `ANTS_RANDOM_SEED` | fMRIPrep sets it; `None`: larmorx's fixed seed (ANTs seeds from the clock: never reproduced) |
| `out_dir=None`, `upstream_defects="fix"`, `n_threads=1` | `omp_nthreads` | the nodes' folders `dilate_target_mask`, `dilate_fmap_mask`, `coregister`; `n_threads` never changes a result |
| → `Coeff2EpiResult(target2fmap_xfm, target_ref, fmap_coeff, dilate_target_mask, dilate_fmap_mask, coregister_cmdline)` | `outputnode` | `target2fmap_xfm`: ANTs' `transform0GenericAffine.mat` |

`registration_settings(name)` is the path of one of sdcflows 2.17.1's ANTs settings files shipped
with larmorx (Apache-2.0): `fmap-any_registration.json`, `fmap-any_registration_testing.json`,
`affine.json`, `sd_syn.json`, `sd_syn_sloppy.json`, `translation_rigid.json`. Checked node file by
node file against sdcflows' own graph (ANTs 2.6.5 at one thread with CORE-MATH through the oracle
cache) and against fMRIPrep's own nodes on the oracle runs:
`validation/investigations/sdcflows-fieldmap-b/check_bold_fmap.py`.

## The fieldmap-less (SyN) preprocessing (part B)

`syn_preprocessing_workflow` is sdcflows' `init_syn_preprocessing_wf`
(`python/larmorx/sdcflows/syn.py`; the same in 2.15.1 and 2.17.1): the EPI reference
(`lx.niworkflows.epi_reference_workflow`), made plumb (`deoblique`) and brain-extracted; the
anatomical image masked (`ApplyMask`), clipped (`IntensityClip(0, 99.8)`) and denoised (ANTs'
`DenoiseImage -n Gaussian -s 1` with nipype's `copy_header`); the registration of the EPI reference
to it (`antsRegistration` with sdcflows' `affine.json`, `--masks [ anat_dil, NULL ]` then
`[ anat_dil, epi_dil ]`, both masks dilated by 3); the sampling grid
(`lx.niworkflows.generate_sampling_reference`); the anatomical reference (`Linear`) and mask
(`MultiLabel`, then `uint8`) resampled onto it through the inverted registration
(`antsApplyTransforms`, `FixHeaderApplyTransforms`); and the prior of strong distortions through
`[epi2anat (inverted), std2anat_xfm, fmap_atlas_2_MNI152NLin2009cAsym_affine.mat]`, thresholded
above `atlas_threshold` (`Binarize`).

| larmorx | sdcflows | Notes |
|---|---|---|
| `in_epis`, `in_meta`, `in_anat`, `mask_anat`, `std2anat_xfm=None`, `t_masks=None` | `inputnode` | `std2anat_xfm` is needed with `sd_prior` |
| `atlas_threshold=3`, `auto_bold_nss=False`, `sd_prior=True`, `omp_nthreads=1` | the same | fMRIPrep 25.2.6: `auto_bold_nss=True`, the BOLD and SBRef files of the estimator and their metadata, its T1w, mask and MNI-to-T1w transform |
| `debug=False` | `debug` | sdcflows' fast settings regrid the T1w with niworkflows' `RegridToZooms`: `NotImplementedError` until that port (fMRIPrep sets it only with `--sloppy`) |
| `t1w_inversion=False` | `t1w_inversion` | accepted and unused, as in sdcflows |
| `prior_file=None` | (`data/fmap_atlas.nii.gz`) | a larmorx addition: another prior image (tests); by default sdcflows' atlas (`fmap_atlas()`) |
| `ants_random_seed=None` | the environment's `ANTS_RANDOM_SEED` | as `coeff2epi_workflow` |
| `niworkflows_version="1.15.4"`, `numpy_version="2.5.4"`, `scipy_version="1.18.1"`, `nilearn_version="0.14.1"`, `volreg_implementation="auto"`, `upstream_defects="fix"`, `n_threads=1`, `out_dir` | | the libraries' arithmetic where their releases differ (fMRIPrep 25.2.6's image: `"1.15.2"`, `"2.2.6"`, `"1.15.2"`, `"0.11.1"`); the nodes' folders under `out_dir/syn_preprocessing_wf` |
| → `SynPreprocessingResult(epi_ref, epi_mask, anat_ref, anat_mask, sd_prior, epi_reference, deob_epi, sampling_ref, epi2anat_xfm, files, commands)` | `outputnode` | `epi_ref` is `(file, the first input's metadata)` (`_merge_meta`); `commands`: nipype's lines |

| Helper | sdcflows | Notes |
|---|---|---|
| `fmap_atlas()` → path | `data/fmap_atlas.nii.gz` (5.8 MB, Apache-2.0) | not shipped: `LARMORX_SDCFLOWS_ATLAS` (a local copy), else larmorx's data cache (`LARMORX_DATA_DIR`, or the user's cache folder), fetched there once from sdcflows 2.17.1's release; every copy checked against its SHA-256 (`ValueError`) |
| `fmap_atlas_affine()` → path | `data/fmap_atlas_2_MNI152NLin2009cAsym_affine.mat` | shipped (193 bytes) |
| `obliquity(affine)` | nibabel's `affines.obliquity` | radians, numpy's `arccos` |
| `deoblique(in_file, in_affine=None, *, out_dir=".", n_threads=1)` → path | `interfaces/utils.Deoblique` (`_deoblique`) | the input when every axis's obliquity is below `OBLIQUE_THRESHOLD_DEG` (SDC-13); else `<in>_plumb` with the plumb affine (or `in_affine`'s) as qform and sform, code 1 |
| `reoblique(in_epi, in_plumb, in_field, in_mask=None, *, out_dir=".", n_threads=1)` → `(epi, field, mask)` | `interfaces/utils.Reoblique` (`_reoblique`) | unchanged for a plumb EPI; else `<name>_oriented` with the EPI's affine (the plumb EPI with the EPI's whole header; the field and mask with the EPI's qform and sform) |
| `set_dtype(in_file, dtype="int16", *, n_threads=1)` → path | `fit/syn._set_dtype` | the input when stored as `dtype`; else `<in>_<dtype>` beside it |
| `merge_meta(epi_ref, meta_list)` | `fit/syn._merge_meta` | `(epi_ref, meta_list[0])` |

Checked on the three `--use-syn-sdc` fMRIPrep runs node by node (each node fed fMRIPrep's own
inputs; header bytes as written) and as a whole: `validate_nii`, `select_volumes`,
`per_run_avgs`, `clip_avgs`, `clip_bg_noise`, `deob_epi`, `anat_dilmsk`, `mask_anat`, `clip_anat`,
`epi_brain`, `epi_dilmsk`, `sampling_ref`, `anat2epi`, `mask2epi`, `mask_dtype`, `prior2epi`,
`prior_msk` and the command lines are fMRIPrep's files; `n4_avgs` and `ref_anat` are ANTs 2.6.5's
one-thread results with correctly rounded maths (fMRIPrep's own ran on 4 threads, on which N4's
fit and DenoiseImage depend, with glibc's maths: `docs/findings/ants-n4.md`,
`ants-denoise.md`); the registration is ANTs' one-thread result
(`validation/investigations/sdcflows-fieldmap-b/check_syn_preprocessing.py`).

## The fieldmap-less (SyN) estimation (part B)

`syn_sdc_workflow` is sdcflows' `init_syn_sdc_wf` (`python/larmorx/sdcflows/syn.py`; the same
in 2.15.1 and 2.17.1), on `syn_preprocessing_workflow`'s outputs: the readout time and phase
encoding from the EPI's metadata (`get_readout_time`); the EPI reference clipped
(`IntensityClip(35, 99.9)`); both references' Laplacians (ANTs' `ImageMath 3 <out> Laplacian
<in> 1.5 1` with nipype's `copy_header`) normalised (`norm_lap`); the EPI's mask joined with the
anatomical mask, dilated by 3 and brought onto the EPI's grid (`antsApplyTransforms -t identity`,
`MultiLabel`, `Union`); the registration of the EPI to the anatomy (`antsRegistration` with
sdcflows' `sd_syn.json`: two SyN stages, two Mattes metrics each (image and Laplacian), the
deformation restricted to the phase-encoding axis (`warp_dir`), SyN's field variance in voxels
(`mm2vox`), fixed masks `[anat_mask, sd_prior]`, moving masks `[NULL, union]`); the displacement
field as a field map in Hz (`displacements_field_to_fieldmap`); the field onto the EPI
reference's grid (`antsApplyTransforms -u float -t identity`, `BSpline`) and the anatomical mask
with it (`-u uchar`, `MultiLabel`); sdcflows' B-spline fit (`bspline_approx(recenter=False)`, the
16 x 16 x 10 mm grid, no mask); and the EPI reference unwarped with it (`apply_coeffs_field`, no
Jacobian). The nodes write into `out_dir/<name>/<node>` as nipype's (fMRIPrep names the workflow
`wf_<estimator id>`).

| larmorx | sdcflows | Notes |
|---|---|---|
| `epi_ref` (`(file, metadata)`), `epi_mask`, `anat_ref`, `anat_mask`, `sd_prior` | `inputnode` (`INPUT_FIELDS`) | `syn_preprocessing_workflow`'s outputs, as fMRIPrep connects them |
| `use_metadata_estimates=False`, `fallback_total_readout_time=None` | the same | `GetReadoutTime`'s `use_estimate` and `fallback` |
| `sloppy=False` | `sloppy` | `sd_syn_sloppy.json` (20 x 10 and 2 iterations, ANTs' verbose output) and the fit at `zooms_min=4` mm |
| `debug=False` | `debug` | the registration's warped images (`files["syn_warped_image"]`, `["syn_inverse_warped_image"]`) and `--write-interval-volumes 2`; the fit without extrapolation |
| `laplacian_weight=None` | `laplacian_weight` | two weights, each clipped to [0, `MAX_LAPLACIAN_WEIGHT` = 0.5]: `metric_weight = [[1 - w0, w0], [1 - w1, w1]]` |
| `omp_nthreads=1` | `omp_nthreads` | the registration's `num_threads` in nipype's line |
| `name="syn_sdc_wf"` | `name` | the workflow's folder; fMRIPrep's estimators: `wf_auto_00000`, ... |
| `ants_random_seed=None` | the environment's `ANTS_RANDOM_SEED` | as `coeff2epi_workflow` (fMRIPrep sets it) |
| `niworkflows_version="1.15.4"`, `numpy_version="2.5.4"`, `upstream_defects="fix"`, `n_threads=1`, `out_dir` | | `descrip`'s version, the clip's percentile arithmetic (fMRIPrep 25.2.6's image: `"1.15.2"`, `"2.2.6"`) |
| → `SynSdcResult(fmap, fmap_ref, fmap_coeff, fmap_mask, out_warp, method, readout_time, pe_direction, zooms, files, commands)` | `outputnode` | `out_warp` is declared and never connected by sdcflows (`None`); `method` = `FLB ("fieldmap-less", SyN-based)`; `zooms`: `find_zooms`' value |

| Helper | sdcflows | Notes |
|---|---|---|
| `warp_dir(moving_image, fixed_image, pe_dir, nlevels=2, *, upstream_defects="fix")` | `fit/syn._warp_dir` | 0.1 on two world axes, 1.0 on the one closest to the moving image's PE axis, per level (ANTs weighs the deformation's physical components; sdcflows indexes by the fixed image's voxel axes: SDC-16); logs sdcflows' "Running fieldmap-less registration on an oblique dataset" (`larmorx.sdcflows`, warning) when the fixed image's obliquity exceeds 0.05 radians (sdcflows calls `Logger.warn`, a deprecated alias of `warning`); a PE letter or axis it cannot match: `ValueError` as sdcflows' `index` |
| `mm2vox(moving_image, fixed_image, pe_dir, registration_config)` | `fit/syn._mm2vox` | each level's third transform parameter divided by the fixed image's voxel size along that axis (numpy's float64) |
| `adjust_zooms(in_anat, in_epi, z_max=2.2, z_min=1.8)` | `fit/syn._adjust_zooms` | `min(round(max(0.5 * (anat + epi), z_min), 2), z_max)` three times, numpy 2's float32 arithmetic (a float32, or the Python bound) |
| `norm_lap(in_file, *, out_dir=".", upstream_defects="fix", n_threads=1)` → path | `fit/syn._norm_lap` | minus the median; positives over their 99.8th percentile, negatives over minus their 0.2th; clipped to [-1, 1]; float32 `<name>_norm`; SDC-14 |

Not computed: `zooms_epi` (niworkflows' `RegridToZooms` of the clipped EPI to `find_zooms`'
size), whose output sdcflows never connects (SDC-15); its port comes with `RegridToZooms`
(`syn_preprocessing_workflow(debug=True)` needs it too).

Checked on fMRIPrep's five `--use-syn-sdc` estimators node by node (each node fed fMRIPrep's
own inputs and, after the registration, fMRIPrep's own warp; header bytes as written): every
file, value and command line is fMRIPrep's; the registration itself is ANTs 2.6.5's
one-thread result with correctly rounded maths (fMRIPrep's own ran on 4 threads, on which SyN's
sampled metric depends); and the whole function against sdcflows' own graph
(`validation/investigations/sdcflows-fieldmap-b/check_syn_sdc.py`).

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
| SDC-12 | `ReorientImageAndMetadata` checks the upper-cased orientation code but reorients to the code as given (a lower-case one fails in nibabel) | the upper-cased code it checked | `reorient_image_and_metadata` |
| SDC-13 | `Deoblique`/`Reoblique` compare nibabel's obliquity (radians) with `OBLIQUE_THRESHOLD_DEG = 0.5` (degrees): they act only beyond 28.6 degrees; `_deoblique`'s plumb grid centres the image in voxels, not millimetres, and compares the axis codes with `RAS` by position | **under measurement** (rule D, with `init_syn_sdc_wf`'s port): sdcflows' behaviour in both modes until then | `deoblique`, `reoblique`, `syn_preprocessing_workflow` |
| SDC-14 | `_norm_lap` of an image with no positive (or no negative) value after the median's subtraction fails with numpy's `IndexError` (the percentile of nothing) | a `ValueError` saying so (rule B) | `norm_lap`, `syn_sdc_workflow` |
| SDC-16 | `_warp_dir` puts the weight 1.0 at the position of the phase-encoding axis among the fixed image's voxel axes, while ANTs' `--restrict-deformation` weighs the physical axes (x, y, z): a sagittal or coronal fixed grid frees another world axis and holds the phase encoding at 0.1 | the 1.0 on the phase encoding's world axis (rule A); the same restriction for a grid in R, A, S order (fMRIPrep's axial EPIs) | `warp_dir`, `syn_sdc_workflow` |
| SDC-15 | `init_syn_sdc_wf`'s `zooms_epi` regrids the clipped EPI and nothing reads it | kept as sdcflows (rule C: no result depends on it); larmorx does not compute it (`zooms` holds its voxel size) | `syn_sdc_workflow` |

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
