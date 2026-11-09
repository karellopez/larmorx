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
part's records. The BOLD runs' field-map nodes composed as larmorprepx composes them
(`larmorx.pipelines.larmorprepx.fieldmaps`, below) are bit-identical to fMRIPrep's files on the
same runs: every node of the 7 BOLD fits (14 calls: registering and with the transform given)
and of the 21 resampling workflows, in both environments (`check_bold_stage.py`).

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
| PIPE-20 | C for the interface, B for the pipeline | `DistortionParameters` turns a missing readout time or phase-encoding direction into undefined outputs (its documented design: "downstream interfaces should be prepared to handle this"), so the BOLD run is resampled without its field map although one is assigned to it, and fMRIPrep's report still names the correction | `distortion_parameters`: the same `None` outputs, and a `UserWarning` naming the missing value and `--fallback-total-readout-time`; larmorprepx's `bold_fit_fieldmap` and `resampling_fieldmap`: a `FieldmapMetadataError` with fMRIPrep's own message for a missing readout time ("Missing readout time information. See documentation for `--fallback-total-readout-time`.", the message fMRIPrep stops with when an estimator lacks one) | the same outputs, no warning; the larmorprepx functions resample without the field map, as fMRIPrep, with an `UpstreamDefectWarning` |
| PIPE-21 | A | the direct evaluation returns the field on the target projected into the field map's space and made positive: the projected affine, and for a target with an axis against its world axis (a LAS BOLD reference) the voxels reversed along it, while `ResampleSeries` reads the field voxel for voxel on the target's grid: a mirrored field (a LAS target with the identity: up to 159 Hz from the field on the target's grid in `check_bold_steps.py`'s `synth`) | the same values on the target's grid: its voxel order and affine (equal to the resampled path's field) | fMRIPrep's image bit for bit |

Only the direct path is affected; none of the earlier oracle runs' 28 nodes takes it (a
registration's rotation fails `aligned`): PIPE-21 changes nothing there.

## Differences and limits

- A series of affines in `transforms` (a head-motion file) raises `ValueError`: it cannot place
  a field map (nitransforms would treat the series as one affine of the wrong shape).

## larmorprepx: the field-map nodes of a BOLD run

`larmorx.pipelines.larmorprepx.fieldmaps` composes the steps above as fMRIPrep 25.2.7's BOLD
workflows wire them (`workflows/bold/fit.py`, `workflows/bold/apply.py`, `workflows/base.py`),
plain functions with no graph: each node is a call of its larmorx port, in fMRIPrep's order, its
files in a folder named as fMRIPrep's node. The steps before them (the estimators, the
motion-correction reference, niworkflows' `init_enhance_and_skullstrip_bold_wf`) are the caller's
until larmorx ports them.

### `bold_fit_fieldmap(bold_file, metadata, fieldmap_id, estimates, hmc_boldref, coreg_ref, coreg_mask, *, out_dir, sbref_files=None, boldref2fmap_xfm=None, jacobian=False, fallback_total_readout_time=None, sloppy=False, debug=False, ants_random_seed=None, sink=None, fsl_implementation="auto", n_threads=1, upstream_defects="fix")` → `BoldFitFieldmapResult`

`init_bold_fit_wf`'s stage 3 ("Register fieldmap to boldref and reconstruct in BOLD space") and
stage 4's field-map branch, for a run with a field map:

| Node (fMRIPrep) | larmorx | Inputs |
|---|---|---|
| `fmapref_buffer` (`_select_ref`) | `select_ref(sbref_files, boldref_files)` (the motion-correction reference as `boldref_files`) | the first single-band reference, else the motion-correction reference |
| `fmap_select` (`KeySelect`) | `lx.niworkflows.key_select` | `estimates` (`FieldmapEstimates`: `fmap_id`, `fmap_ref`, `fmap_coeff`, `fmap_mask`, `sdc_method`) at `fieldmap_id` |
| `fmapreg_wf` (sdcflows' `init_coeff2epi_wf`) | `lx.sdcflows.coeff2epi_workflow` | `coreg_ref`, `coreg_mask` (`init_enhance_and_skullstrip_bold_wf`'s `bias_corrected_file`, `mask_file`), the estimator's reference and mask; `sloppy`, `debug`, `ants_random_seed`; skipped when `boldref2fmap_xfm` is given ("Found fieldmap transform - skipping registration") |
| `itk_mat2txt` (`ConcatenateXFMs(out_fmt='itk')`) | `lx.niworkflows.concatenate_xfms` | the registration's transform; then the `desc-fmap_xfm` derivative (`sink`) |
| `boldref_fmap` (`ReconstructFieldmap(inverse=[True])`) | `lx.transforms.reconstruct_fieldmap_node` | the coefficients onto the reference of `fmapref_buffer` through the transform, inverted |
| `distortion_params` (`DistortionParameters`) | `lx.transforms.distortion_parameters` | `metadata`, `bold_file`, `fallback_total_readout_time`; PIPE-20 at the pipeline level |
| `unwarp_boldref` (`ResampleSeries(jacobian)`) | `lx.transforms.resample_series` | `coreg_ref` onto `fmapref_buffer`'s grid with the field map, written as `<coreg_ref>resampled`; then the `desc-coreg_boldref` derivative |
| `skullstrip_bold_wf` | `lx.niworkflows.skullstrip_bold_workflow` | the coregistration reference; then the `desc-brain_mask` derivative |

| Argument | fMRIPrep | Notes |
|---|---|---|
| `jacobian` | `init_bold_wf(jacobian=...)` | `fieldmap_jacobian(method, force, ignore)`: `--force fmap-jacobian` always, `--ignore fmap-jacobian` never, otherwise PEPOLAR only |
| `fallback_total_readout_time` | `--fallback-total-readout-time` | `"estimated"` or a readout time in s |
| `sloppy`, `debug` | `--sloppy`, `--debug fieldmaps` | sdcflows' testing registration settings; the registration's warped images |
| `ants_random_seed` | `ANTS_RANDOM_SEED` (fMRIPrep's `--random-seed`) | unseeded, ANTs seeds its sampling from the clock |
| `sink` | `DerivativesDataSink` | called with each `DerivativeRequest(role, in_file, source_file, entities, sources)` (`role`: `boldref2fmap_xfm`, `coreg_boldref`, `bold_mask`; `entities`: fMRIPrep's `desc`, `suffix`, `from`, `to`, `mode`, `extension`, `compress`, `dismiss_entities`); it returns the file the later nodes read, as fMRIPrep's later nodes read the derivative files. Without it they read the node files (the same data; the skull-stripping files then take the node file's name) |
| `fsl_implementation` | | BET's and fslmaths' implementation (`auto`, `replica`, `original`) |
| `n_threads` | `omp_nthreads` | the results do not depend on it (ANTs' registration gives its one-thread result) |
| `out_dir` | the BOLD fit's folder | required: `fmapreg_wf/`, `itk_mat2txt/`, `boldref_fmap/`, `unwarp_boldref/`, `skullstrip_bold_wf/` |

`BoldFitFieldmapResult`: `sdc_boldref`, `fmap_ref`, `fmap_coeff`, `fmap_mask`, `sdc_method` (the
report's distortion-correction line), `boldref2fmap_xfm` (the file the later nodes read),
`registration` and `itk_mat2txt` (`None` when the transform was given), `fieldmap`,
`readout_time`, `pe_direction`, `unwarped_boldref`, `coreg_boldref`, `skullstrip`, `bold_mask`,
`derivatives` (the requests).

### `resampling_fieldmap(fieldmap_id, estimates, target_ref_file, boldref2fmap_xfm, metadata, in_file=None, boldref2target=None, *, fallback_total_readout_time=None, out_dir=None, n_threads=1, upstream_defects="fix")` → `ResamplingFieldmapResult`

The field-map nodes of `init_bold_native_wf` (`boldref2target` `None`: the field map on the
coregistration reference) and of each `init_bold_volumetric_resample_wf` (`bold_anat_wf`,
`bold_std_wf`'s spaces: `boldref2target` the transforms from the BOLD reference to the target,
`[boldref2anat]` or `[boldref2anat, anat2std]`; `target_ref_file` `gen_ref`'s grid):
`fmap_select`, `distortion_params` (`in_file`: the raw BOLD in `bold_native_wf`, the minimally
processed one in the resampling workflows), `fmap2target` (`[boldref2fmap_xfm, *boldref2target]`,
nipype's `Merge`), `inverses` (`gen_inverses(inlist)`: `[True, False, ...]`) and `boldref_fmap` /
`fmap_recon` (written to `out_dir` when given). The result (`fmap_ref`, `fmap_coeff`,
`transforms`, `inverse`, `fieldmap`, `out_file`, `readout_time`, `pe_direction`) feeds
`lx.transforms.resample_series` (`fieldmap`, `ro_time`, `pe_dir`). fMRIPrep gives the resampling
workflows no field map for a multi-echo run: the caller's rule.

`FieldmapMetadataError` (a `ValueError`): no readout time or no phase-encoding direction for a run
with a field map (PIPE-20, `upstream_defects="fix"`).

Checked by `validation/investigations/sdcflows-fieldmap-b/check_bold_stage.py` (`fit`,
`resampling`, `pipe20`) on the earlier oracle runs (ds002422, ds002603, ds005040, ds005454):
`fmapref_buffer`, `fmap_select`, both dilated masks, `itk_mat2txt`, the three derivatives (through
fMRIPrep's own `DerivativesDataSink` as the sink: names and files), `boldref_fmap`,
`distortion_params`, `unwarp_boldref` and the 70 `skullstrip_bold_wf` files identical; the
registration within 6.7e-13 of fMRIPrep's (its ANTs ran with 8 threads; its `"%g"` text identical);
the 21 resampling workflows' `fmap_select`, `inverses`, `fmap2target`, `distortion_params` and
field maps identical; PIPE-20's error, and `reproduce` = fMRIPrep's `ResampleSeries` without a
readout time.

## larmorprepx: the field-map estimators of a subject

`larmorx.pipelines.larmorprepx.estimators` is fMRIPrep 25.2.7's choice of estimators
(`workflows/base.py`, `config.py`, `utils/bids.py`; the same in 25.2.6), on
[`lx.bids`](bids.md) and sdcflows' `find_estimators` ([sdcflows-fieldmap.md](sdcflows-fieldmap.md)).

| larmorx | fMRIPrep | Notes |
|---|---|---|
| `fmriprep_layout(bids_dir, participant_label=None, *, bids_database_dir=None, pybids_version="0.22.0")` | `config.execution.init`'s `BIDSLayout` | ignores `code`, `stimuli`, `sourcedata`, `models`, the subjects' `beh`, `dwi`, `eeg`, `ieeg`, `meg`, `perf` folders and, with `participant_label` and no database folder, every other subject (fMRIPrep's `^\.` pattern never matches a `/`-rooted path: dot files are pybids' own `/\.` rule); the root's `dataset_description.json` checked |
| `map_fieldmap_estimation(layout, subject_id, session_id, bold_data, ignore_fieldmaps, use_syn, force_syn, filters, *, logger=None, registry=None, sdcflows_version="2.17.1", upstream_defects="fix")` → `(estimators, {bold_file: estimator id})` | `map_fieldmap_estimation` | `--use-syn-sdc` (`True`/`"warn"`/`"error"`: the "PhaseEncodingDirection information appears to be absent" message, an error for `"error"`), `--force syn-sdc`, `--ignore fieldmaps` (only fieldmap-less estimators kept), each run's first estimator ("Several fieldmaps <...> are 'IntendedFor' <...>, using ..."), estimators no run uses dropped; the messages through `logger` (fMRIPrep: `nipype.workflow`); PIPE-25 |
| `get_estimator(layout, fname, *, registry=None)` | `get_estimator` | `B0FieldSource`, else the estimators `IntendedFor` the file's path below `sub-<label>/` (`/` on every platform) |
| `collect_fieldmaps(derivatives_dir, entities, spec=None)` | `utils/bids.collect_fieldmaps` | niworkflows' `nipreps.json` index of the derivatives; `FMAP_SPEC` = `data/fmap_spec.json`'s queries |
| `split_precomputed(all_estimators, fmap_cache, *, logger=None)` | `init_single_subject_wf` | the estimators with precomputed field maps (identifier without non-alphanumerics) and those to run; fMRIPrep's "Precomputed B0 field inhomogeneity maps found ..." and "B0 field inhomogeneity map will be estimated with the following ..." messages |
| `setting_up_message(estimator)`, `check_pepolar_scheme(estimator)` | `init_single_subject_wf` | the "Setting up fieldmap ..." line; `NotImplementedError('Sophisticated PEPOLAR schemes are unsupported.')` unless one modality or two `epi`/`bold`/`sbref` sources |

| ID | Rule | What fMRIPrep does | larmorx by default | `reproduce` |
|---|---|---|---|---|
| PIPE-25 | A | a BOLD run's `B0FieldSource` must equal the estimator's identifier; when the field map's sources list several `B0FieldIdentifier` values (aliases, as sdcflows' own test data has), sdcflows names the estimator after one of them, in a Python `set`'s order (SDC-19), so a run naming another alias is left without a field map (or not, from one process to the next) | the alias reaches the estimator whose sources list it (an `UpstreamDefectWarning` when that changes the run's estimator) | fMRIPrep's lookup |

Checked (`validation/investigations/sdcflows-fieldmap-b/check_estimators.py`, `fmriprep`,
`runs`, `image`): fMRIPrep 25.2.7's `map_fieldmap_estimation` and `get_estimator` (taken from its
source) against these on every dataset of the index check for 24 combinations of the options and
filters, with their log messages; `collect_fieldmaps` on sdcflows' eight own runs' derivatives;
the 23 fMRIPrep oracle runs: the estimators set up, the "Several fieldmaps" and "estimated with
the following" lines of each `fmriprep.log` and its `fmap_preproc_wf/wf_*` folders, identical
(fMRIPrep 25.2.6's image, pybids 0.20.0, sdcflows 2.15.1: `reproduce`).

## larmorprepx: the field-map estimation stage of a subject

`larmorx.pipelines.larmorprepx.estimators` also holds fMRIPrep 25.2.7's (= 25.2.6's)
field-map estimation stage of `init_single_subject_wf` (`workflows/base.py`, from
`fmap_cache = {}` to the BOLD workflows): the caller of sdcflows' `init_fmap_preproc_wf`
([`lx.sdcflows.fmap_preproc_workflow`](sdcflows-fieldmap.md)). It is split as fMRIPrep's graph
is: what is set up when the graph is built (the **plan**: its checks, messages and inputs) and
what runs with the anatomical workflow's outputs (**run**), into the six field-map buffers every
BOLD workflow reads (`FieldmapEstimates`, the input of `bold_fit_fieldmap` and
`resampling_fieldmap` above).

| larmorx | fMRIPrep | Notes |
|---|---|---|
| `precomputed_fieldmaps(derivatives, subject, *, logger=None)` → `{fmapid: {"fieldmap", "coeffs", "magnitude"}}` | `fmap_cache` (`--derivatives`) | `collect_fieldmaps` of every derivatives folder (a mapping of names to folders, a sequence of folders or one folder) in order, a later folder's identifier replacing an earlier one's; the debug line "Detected precomputed fieldmaps in ... for fieldmap IDs: [...]" |
| `plan_fieldmap_estimation(all_estimators, fmap_cache=None, *, output_dir, subject, omp_nthreads=1, fallback_total_readout_time=None, debug=False, sloppy=False, exit_on_missing_readout=True, syn_options=None, logger=None, upstream_defects="fix", n_threads=1, **workflow_options)` → `FieldmapEstimationPlan` | the graph's construction | `split_precomputed` (its messages); `init_fmap_preproc_wf` built (`lx.sdcflows.build_workflows`) with `use_metadata_estimates` for `--fallback-total-readout-time estimated`, the fallback when it is a number (an integer too; another string: `ValueError`, fMRIPrep's parser refuses it), `debug` (`--debug fieldmaps`), `omp_nthreads`, `output_dir`, `subject`, `out_path_base=''` (fMRIPrep's data sinks); a missing readout time: fMRIPrep's critical "Missing readout time information. See documentation for `--fallback-total-readout-time`." with the traceback and `SystemExit(65)` (`os.EX_DATAERR`, the same number on Windows; `exit_on_missing_readout=False`: sdcflows' `RuntimeError`); then for each estimator its "Setting up fieldmap" line, a PEPOLAR one's scheme checked (`NotImplementedError('Sophisticated PEPOLAR schemes are unsupported.')`) and its `in_data` and `metadata` set, a fieldmap-less one's `init_syn_preprocessing_wf` (`SynPreprocessingCall`: `name="syn_preprocessing_<sanitized_id>"`, its `bold` and `sbref` sources and their metadata, `omp_nthreads`, `debug=sloppy` (`--sloppy`), `auto_bold_nss=True`, `t1w_inversion=False`); `workflow_options` go to the estimators' workflows (the libraries' versions, the implementations, sdcflows 2.15's `wiring`, `ants_random_seed`, ...), `syn_options` to `syn_preprocessing_workflow`; PIPE-31 |
| `FieldmapEstimationPlan(all_estimators, estimator_types, precomputed, precomputed_buffers, estimators, preproc_options, inputs, syn_preprocessing, desc, workflows)`, `.needs_anat` | the graph | `estimator_types`: each estimator's method by identifier (fMRIPrep's Jacobian rule: `fieldmap_jacobian`); `precomputed_buffers`: the buffers' first inputs (the precomputed files, `'MISSING'` masks, the identifiers, `'precomputed'`); `estimators`: those left to estimate; `desc`: the boilerplate ("A total of N fieldmaps were found available ..."); `needs_anat`: whether `fmap_select_std` is connected |
| `FieldmapEstimationPlan.run(*, work_dir, t1w_preproc=None, t1w_mask=None, std2anat_xfm=None, templates=None)` → `FieldmapEstimationResult(estimates, estimator_types, std2anat_xfm, syn_preprocessing, preproc)` | running it | `fmap_select_std` (niworkflows' `KeySelect` of MNI152NLin2009cAsym in `templates`; without `templates`, `std2anat_xfm` is that transform), each `syn_preprocessing_workflow` in `work_dir/<name>` (`t1w_preproc` → `in_anat`, `t1w_mask` → `mask_anat`), `fmap_preproc_workflow` in `work_dir/fmap_preproc_wf` (the PEPOLAR inputs and the preprocessing's five outputs as its `in_<sanitized_id>` inputs), the buffers merged; `work_dir` is the subject's workflow folder (fMRIPrep's `sub_<label>_wf`); a fieldmap-less estimator without the anatomical inputs: `ValueError` |
| `estimate_fieldmaps(all_estimators, fmap_cache=None, *, output_dir, subject, work_dir, t1w_preproc=None, t1w_mask=None, std2anat_xfm=None, templates=None, **options)` | the stage | `plan_fieldmap_estimation(...)` then `.run(...)` |
| `merge_buffers(precomputed_buffers, preproc)` → `FieldmapEstimates` | `fmap_buffers` (`Merge(2)`) | the precomputed field maps first, then `init_fmap_preproc_wf`'s outputs (`preproc`: `fmap_preproc_workflow`'s result or `None`); each input's items appended, a coefficient item a list (or one file, as the derivatives held it) |
| `BUFFER_FIELDS`, `EX_DATAERR`, `SYN_PRIOR_TEMPLATE`, `FMAP_PREPROC_DESC` | | `fmap`, `fmap_ref`, `fmap_coeff`, `fmap_mask`, `fmap_id`, `sdc_method`; 65; `MNI152NLin2009cAsym`; the boilerplate |

| ID | Rule | What fMRIPrep does | larmorx by default | `reproduce` |
|---|---|---|---|---|
| PIPE-31 | B | a precomputed field map whose derivatives lack its field map, reference or coefficients file (`collect_fieldmaps` finds only some of the three) stops the graph's construction with a bare `KeyError` (`'fieldmap'`, `'magnitude'` or `'coeffs'`) after the "Precomputed ... found" line | a `ValueError` naming the identifier, the missing files and the files found | fMRIPrep's `KeyError` |
| PIPE-32 | A | every participant of a run is searched with sdcflows' one registry, so a second participant whose `B0FieldIdentifier` an earlier one used (BIDS asks it to be unique only within a participant) stops the run with `KeyError: "'<id>' is already  in mapping"` (fMRIPrep 25.2.7's own `map_fieldmap_estimation`, `relay17/probe_shared_identifier.py`) | `map_fieldmap_estimation` searches each participant in a registry of its own, which holds the earlier participants' automatic names (so `auto_NNNNN` stay fMRIPrep's), then adds its entries to `registry` where their names are free (a shared name keeps the first participant's sources there; the estimators returned hold their own), with an `UpstreamDefectWarning` | fMRIPrep's `KeyError` |

Checked (`validation/investigations/sdcflows-fieldmap-b/check_fmap_stage.py`, shared
environment): `graph`, fMRIPrep 25.2.7's own code of the stage (its source lines, the same in
25.2.6's image) executed as `init_single_subject_wf` executes it to build its nipype graph, on
larmorx's estimators of the 12 fMRIPrep oracle runs with field maps (sdcflows' estimators made
from the same sources) with fMRIPrep's options and five variants (`estimated`, a fallback of
0.05, `--debug fieldmaps`, `--sloppy`, the run's own derivatives), a derivatives folder holding
one of two estimators, PIPE-31 and synthetic cases (a missing readout time: exit status 65; a
sophisticated PEPOLAR scheme), 85 cases: the log lines, the errors, `init_fmap_preproc_wf`'s
arguments, the `in_<id>` inputs, each `init_syn_preprocessing_wf`'s arguments and inputs, the
buffers' first inputs, the data sinks' `out_path_base`, every connection, the boilerplate and the
estimator types identical; `nodes`, the 12 runs' work folders: the folders fMRIPrep made, the
PEPOLAR inputs (6 estimators), the fieldmap-less inputs and `fmap_select_std`'s transform (5
estimators), the six buffers (each `*_merge` node's output) and the log lines identical; `run`,
`estimate_fieldmaps` end to end on ds005752's direct field map and ds001600's four estimators
(fMRIPrep's choice): all 25 derivative files byte for byte as sdcflows 2.17.1's own
`init_fmap_preproc_wf` writes them, and the buffers' files named as the runs' (against fMRIPrep's
own files, as information: every field-map estimator's brain mask and coefficients identical
and ds005752's field map too; the magnitude references not (fMRIPrep's N4 ran at four threads
with glibc's `exp`/`log`), nor ds001600's three phase-difference field maps (their coefficients
are identical: an open question for the records); the PEPOLAR estimator not, as expected: this
run used sdcflows 2.17.1's wiring, fMRIPrep 2.15's).
