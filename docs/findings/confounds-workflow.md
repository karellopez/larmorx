# fMRIPrep's confounds: the workflow, its files, and what the port changes on the 36 real runs

What porting fMRIPrep's `init_bold_confs_wf` (`workflows/bold/confounds.py`) and `base.py`'s
`ds_confounds` as one plain Python function showed, the defects of the wiring (PIPE-41, PIPE-42,
PIPE-49, PIPE-79) and of nipype's command lines (PIPE-80), fMRIPrep's own confounds files compared column
by column, and the part's golden and memory findings. fMRIPrep, niworkflows and nipype are Apache-2.0:
not restricted. The port: `larmorx.pipelines.larmorprepx.confounds.bold_confounds` on the ports of the
other confounds files ([docs/api/confounds.md](../api/confounds.md), its last section), `python/larmorx/_nipype.py`
for the command lines. The scripts: `check_workflow.py`, `golden_cases.py`, `memory_call.py`,
`survey_runs.py`, `survey_sizes.py`, `node_inputs.py`, `bench.py`
([validation/investigations/confounds](../../validation/investigations/confounds/README.md)). The steps:
[confounds-dvars.md](confounds-dvars.md), [confounds-compcor.md](confounds-compcor.md),
[confounds-pandas.md](confounds-pandas.md), [confounds-model.md](confounds-model.md),
[confounds-motion.md](confounds-motion.md), [confounds-masks.md](confounds-masks.md); the arithmetic
under them: [confounds-blas.md](confounds-blas.md), [confounds-numpy.md](confounds-numpy.md),
[confounds-fft.md](confounds-fft.md).

## What fMRIPrep runs, and which oracle runs reach which path

- **Inputs** (read): the BOLD series in its reference space after slice timing, head motion and
  distortion correction (a multi-echo run's optimal combination), the BOLD mask, the head-motion
  reference and its ITK text transforms (`AffineTransform_float_3_3`, about 6 digits), the dummy scans
  (niworkflows' `NonsteadyStatesDetector`), the T1w mask, the GM/WM/CSF probability maps, the
  coregistration. fMRIPrep's defaults: `regressors_all_comps=False` (`variance_threshold=0.5`), FD 0.5,
  DVARS 1.5, `freesurfer=run_reconall` (false in every oracle run). fMRIPrep 25.2.6's and 25.2.7's
  `workflows/bold/confounds.py` and `base.py`'s `ds_confounds` are identical.
- **The nodes** (fMRIPrep's settings, read from the runs' `_report/report.rst`): `t1w_mask_tfm`
  (`MultiLabel`), `union_mask`, `dilated_mask` (radius 2), `subtract_mask` (the crown), `dvars`,
  `add_dvars_header`, `add_std_dvars_header`, `motion_params`, `rmsd`, `fdisp`, `acc_masks`, `acc_msk_tfm`
  (`Gaussian`, three items), `acc_msk_brain` (0.5), `acc_msk_bin` (0.99: its `out_file`, the values above
  0.99 kept, feeds aCompCor and the signals, not `out_mask`), `acompcor`, `crowncompcor` (24 components),
  `tcompcor` (`percentile_threshold=0.02`; its `pre_filter.tsv` gives the confounds' `cosine_XX` and
  `non_steady_state_outlier_XX` columns), `rename_acompcor`, `merge_rois` + `signals` (`global_signal`,
  `csf`, `white_matter`, `csf_wm`, `tcompcor`), `concat`, the metadata filters and formats,
  `merge_confound_metadata` + `merge_confound_metadata2`, `model_expansion`, `spike_regressors` (whose
  file is the final `desc-confounds_timeseries.tsv` byte for byte), `ds_confounds` (the JSON with sorted
  keys, two-space indents).
- **Not ported here** (with larmorprepx's reports): the reportlets `rois_plot`, `compcor_plot`,
  `conf_corr_plot` and their sinks, `init_carpetplot_wf`; `bold_confounds` returns their inputs.
  nipype deletes the files of outputs nothing reads (aCompCor's and the crown's `pre_filter.tsv`,
  `Binarize`'s mask files); larmorx keeps them.
- **The paths** (`survey_sizes.py`, 36 confounds workflows in 21 oracle runs; ds001600 and ds004173 are
  anatomy only): all four thin `DGESDD` paths, blocked LAPACK (`min(m, n) > 128`) on most runs and
  unblocked on ds005003, ds005040, ds006736, ds004884's naming run, ds005752's 10-volume run and the
  tiny masks; `DBDSDC` with `n <= 25` (ds005752's 10 volumes, masks of 11 to 19 voxels) and divide and
  conquer elsewhere; the cosine basis with `K = 1` (ds005040, ds005752's short run) to 11 (ds002422);
  dummy scans 0 to 3; fewer components than asked (ds005752: the crown gives 9 of 24); strong motion
  (ds000256: 15, 69 and 92 outliers, with tiny WM masks; ds000228, ds003592, ds006736); multi-echo
  (ds003592, ds000210, ds000258, the last stored **big-endian** float32: nibabel's byte swap on every
  read); the largest, ds005454 (160 × 160 × 96 × 280, 436,984 brain voxels, a crown of 60,968) and
  ds004958 (104 × 104 × 72 × 600, crown 30,453). Not reached by any oracle run (synthetic and direct
  calls only): the aseg path, `--return-all-components`, the header's TR, `failure_mode='error'`, SVD
  failures, empty masks, non-finite voxels, `K > 25`, nipype's options fMRIPrep never passes, `TSNR`.
- **The metadata JSON** (ds005454: 549 keys, sorted): the crown's 256 components not retained appear as
  `dropped_0..255` (PIPE-49); no `global_signal`, `csf`, ... entries (PIPE-41).
- **The motion and FD intermediates are gzipped TSVs named `*.tsv.nii.gz`** (`fname_presuffix` on the
  reference's name; pandas infers gzip from `.gz`): compared decompressed.

## Defects of the wiring (rules A to D)

- **PIPE-41** (rule A; `larmorx_core::defects`): `merge_confound_metadata.inputs.in3` is set to the
  tissue signals' metadata (`{"Method": "Mean"}` for `global_signal`, `csf`, `white_matter`, `csf_wm`,
  `tcompcor`) and then connected to the crown's metadata, which replaces it: the confounds' JSON never
  describes those five columns (all 36 oracle files). Default: both kept (`[tcc, acc, signals, crown,
  motion]` into `DictMerge`); `reproduce`: fMRIPrep's four; the notice with every run (the defect always
  applies). On the 36 real workflows the default's JSON adds exactly the five keys and changes no other
  entry.
- **PIPE-42** (rule A, with larmorprepx's reports): `merge_compcor_metadata = Merge(2)` with the crown
  wired to `in3`: the CompCor variance reportlet never shows crownCompCor. The reports are drawn later;
  the workflow returns the three metadata files.
- **PIPE-49** (rule C, no switch): the crown's metadata are not filtered (`FilterDropped` runs on the
  tCompCor and aCompCor metadata only), so the JSON lists the crown's components not retained as
  `dropped_<k>`. Rule C rather than A: fMRIPrep's own documentation (`docs/outputs.rst`: "Entries that are
  not saved in the data file for denoising are still stored in metadata with the ``dropped`` prefix",
  with a `dropped_0` example) describes them, so users may rely on them.
- **PIPE-79** (rule D, open): the aseg path fed FAST's maps ([confounds-masks.md](confounds-masks.md)),
  measured once fMRIPrep runs with FreeSurfer exist; fMRIPrep's wiring kept in both modes until then.

## nipype's command lines with a blank in a file name (PIPE-80, rule A)

- **What** (read; verified with antsApplyTransforms below a folder named `a b`): nipype writes some file
  names into its command lines unquoted, and the line runs through the shell, so a name with a blank is
  split into several words (one with a quote breaks the line): the files inside ANTs' bracketed
  arguments (`ApplyTransforms`' `--transform [ f, 1 ]` and `--output [ out, 1 ]`, `Registration`'s
  `--metric MI[ fixed, moving, ... ]`, `--masks [ f, m ]`, `--initial-moving-transform [ f, 0 ]` and
  `--output [ prefix, warped, inverse ]`, `N4BiasFieldCorrection`'s `--output [ out, bias ]`,
  `DenoiseImage`'s `-o [ out, noise ]`), `ApplyTransforms`' `--transform f` and `--output f`,
  `N4BiasFieldCorrection`'s `--output f`, `MultiImageMaths`' operands, `TOPUP`'s
  `--datain=<node>/<stem>_encfile.txt`. ANTs' parser joins the words between `[` and `]` without the
  blank (`RegroupCommandLineArguments`), so antsApplyTransforms reports "Transform file does not exist:
  .../ab/...". `BET` makes its input's name relative **after** quoting it (`os.path.relpath("'/a
  b/x.nii.gz'", start=cwd)`: a relative name below the process's folder with the quotes inside): BET gets
  another file. So a dataset, working directory or output folder whose path holds a blank or a quote
  makes fMRIPrep's ANTs, TOPUP and BET nodes fail; the oracle runs have no blank in their paths.
- **larmorx** (`python/larmorx/_nipype.py`): every `Command` keeps nipype's line (`cmdline`, `args`, as
  its `command.txt` holds it: the field maps' command-line records are unchanged) and a quoted one
  (`quoted_cmdline`, `quoted_args`, `quoted_run_args`; `None` when no name needs quoting); the program
  gets the quoted line's words by default and nipype's under `reproduce` (a line the shell cannot split
  fails as the shell does), with the notice `PIPE-80 (nipype command lines)` in both modes whenever the
  two differ. `run()` (through which the confounds' `t1w_mask_tfm` and `acc_msk_tfm` run), sdcflows'
  `topup_node` and niworkflows' 3dvolreg step use it.
- **A larmorx mistake fixed on the way** (no ID): the absolute output names larmorx gives a program that
  runs in this process (nipype writes them relative to the node's folder, one word) were unquoted, so a
  node folder with a blank broke larmorx in both modes where nipype runs; now quoted always.
- **Proof**: `tests/python/test_nipype_paths.py` (the two lines, the words and notices in both modes,
  antsApplyTransforms below `a b/` in both modes: the default runs and writes the input's voxels through
  the identity, `reproduce` fails as nipype; a node folder alone with a blank runs in both modes without a
  notice; Registration's and TOPUP's quoted forms); the golden case `py.larmorprepx.confounds` below
  `a b/` runs (only its PIPE-80 notices change), `py.larmorprepx.confounds.reproduce` fails there as
  nipype does; the field maps' checks re-run through the changed module (the golden case
  `py.larmorprepx.fieldmaps`, `tests/python/test_sdcflows_syn.py`, the smoke tiers of
  `sdcflows-estimation` and `niworkflows-helpers`, whose `nipype-command-lines` case compares every
  program's line with every oracle run's `command.txt`): all pass.

## fMRIPrep's own confounds files, column by column (validated)

`check_workflow.py`, full tier, in numpy 2.5.4 / pandas 3.0.6 and numpy 2.3.5 / pandas 3.0.6 with nipype
1.12.0 and fMRIPrep 25.2.7's tag, the same counts, CHECK PASS:

- **`graph`**: fMRIPrep 25.2.7's `init_bold_confs_wf` built by nipype for eight option sets
  (`--return-all-components`, FreeSurfer, a `RepetitionTime` or the header's, other spike thresholds)
  against larmorx's run of the same options on a synthetic BOLD run with every step recorded: every node
  fMRIPrep runs called (a MapNode's items in order), 208 node parameter sets, every connection, the
  outputs, the reportlets' inputs, PIPE-41 in both modes and the description identical; a control
  (formula and `merge_rois` order perturbed) fails the data flow and the parameters.
- **`real`, the 36 confounds workflows of the 21 oracle runs**, each run end to end from its own inputs
  with fMRIPrep 25.2.6's libraries (`fmriprep_version="25.2.6"`, `niworkflows_version="1.15.2"`,
  `numpy_version="2.2.6"`, `scipy_version="1.15.2"`, `pandas_version="2.2.3"`) and `reproduce`: **all 36
  `desc-confounds_timeseries.tsv` identical to fMRIPrep's column by column** (by name, PIPE-43: 25
  directly, 11 once the 15 `rot_*` cells glibc's `atan2` rounds otherwise (D4) are taken from the run's
  own motion table, reaching FD, the expansion and the spikes) and **all 36 JSON files byte for byte**;
  every other node file byte for byte (970, and 124 of the motion chain); the four metadata
  dictionaries identical. The column order differs from fMRIPrep's in 36 of 36 (PIPE-43, never
  reproduced). Notices under `reproduce`: GLIBC-07 81, PD-01 287, PIPE-41 36, PIPE-43 36.
- **The aCompCor masks after `-n Gaussian`** hold other values in 5,298 voxels (`acc_msk_tfm`), 4,714
  (`acc_msk_brain`) and 2 (`acc_msk_bin`): vnl's `erf` through glibc's `exp`/`log` in fMRIPrep's ANTs,
  correctly rounded in larmorx's ANTs port in both modes ([platform-math.md](platform-math.md)); no
  voxel crosses `ApplyMask`'s 0.5 or `Binarize`'s 0.99, so aCompCor and the signals read the same masks
  and the confounds are not affected. The `MultiLabel` resampling of `t1w_mask_tfm` is identical in 36
  of 36.
- **nibabel's `set_qform` on the release's LAPACK**: ds006736's `t1w_mask_tfm` header differs in
  `quatern_b` (1.5e-16 against 1.46e-16) with LAPACK 3.12.1, equal with 3.12.0: larmorx computes the
  x-forms the nodes write with `numpy_version`'s LAPACK.
- **The step to the newest libraries** (`reproduce`, newest against fMRIPrep 25.2.6's): 263,181 cells
  of the 36 tables change (the motion parameters: fMRIPrep 25.2.7's `FSLMotionParams`; CompCor: numpy's
  SVD under LAPACK 3.12.1; DVARS: SciPy's FFT and numpy's sums; the FD spikes: 2 `motion_outlier` columns
  more and 4 fewer over the 36), and 6 JSON values.
- **The default** (`fix`, newest, against `reproduce`, newest): 218,474 cells of the 36 tables (PD-01's
  pandas-read columns: motion, FD, RMSD, DVARS, signals and their expansions; GLIBC-07's CompCor
  columns: [confounds-compcor.md](confounds-compcor.md) for the sign flips), 4 JSON values, and PIPE-41's
  five entries added to every JSON. Notices of the default: GLIBC-07 81, PD-01 293, PIPE-41 36, PIPE-43
  36.

## Importing fMRIPrep silences larmorx's notices in that process (a finding, no ID)

`fmriprep/_warnings.py` replaces `warnings.warn` and `warnings.showwarning` with a logger when fMRIPrep
is imported: a larmorx user who also imports fMRIPrep sees larmorx's `UpstreamDefectWarning`s only in
fMRIPrep's `py.warnings` log (the check restores both). A follow-up: larmorx's notices should not depend
on `warnings.warn` being intact, or the documentation says so.

## The golden cases: Windows line ends (a larmorx mistake, fixed)

The confounds' 15 golden cases (`tests/golden/registry.py`, "fMRIPrep's confound estimation"; 533
outputs, about 0.4 s together; every case run twice, the second below a folder named `RUNNER~1`, and at
1 and 4 threads: every fingerprint identical, no temporary path in an output) found that numpy's
`savetxt` opens a path in text mode, so on Windows its `\n` becomes `\r\n`: larmorx's own tables were
Linux's everywhere, but CompCor's components, pre-filter and metadata files, DVARS' files, nipype's FD
file, `AddTSVHeader` and `NormalizeMotionParams` followed the platform, and the golden cases would have
failed on both Windows runners. Now `larmorx.confounds._tables.savetxt` (numpy's `savetxt` into a
binary file, UTF-8: Linux's bytes, proved identical to numpy's on Linux with a non-ASCII header) and
`newline="\n"` for the two hand-written writers. The inputs use plain arithmetic only (a T1w grid of 32
× 30 × 28 with FAST-like maps, 4-volume series; the SVD and filter inputs 2-D arrays of at most 32,500
values, so blocking and divide and conquer are reached on every platform); array outputs with NaNs are
hashed as the array with 0 there plus where the NaNs are (numpy's NaN sign is the processor's);
`skip_vols` is 0 in the workflow cases on purpose: with 4 volumes and one skipped, tCompCor's
`regress_poly(2)` fits 3 points exactly and its mask is chosen from rounding noise (236 voxels with
numpy 2.5.4, 52 with 2.2.6).

## The memory cases (validated; a larmorx excess fixed)

`tests/parity/test_memory_peak.py`'s `test_confounds_need_no_more_memory_than_the_originals`
(`memory_call.py`): fMRIPrep's `acompcor`, `tcompcor`, `dvars` and `signals` nodes with their own inputs
on ds000005 (64 × 64 × 34 × 240, 32,069 brain voxels) and `acompcor`, `tcompcor` on ds000228 (64 × 64 ×
32 × 168), nipype's and niworkflows' interfaces in the newest environment through the oracle cache
against larmorx in both D13 modes at one thread and by default at eight, `reproduce`'s files the
originals' byte for byte: 6 of 6 pass. Peaks (larmorx / original, MiB): acompcor 176 / 236 and 128 /
193, tcompcor 343 / 367 and 275 / 302 (352 and 281 at eight threads), dvars 193 / 422, signals 170 / 506.
**Found and fixed first** (the memory rule, the same bits): CompCor held two series when volumes were
skipped (`dataobj.astype(np.float32)` copied the float32 view: 293 MiB against nipype's 234, over the
allowance of 265), and tCompCor's mask passed a float64 copy of the mask's rows beside the float32 ones,
which the binding copied again (429 against 367, allowance 412); now the float32 view is used as it is
(`astype(..., copy=False)`) and `_core.nipype_tcompcor_mask` takes float32 or float64 rows.

## The parity suites

`confounds-linalg`, `confounds-nipype` and `confounds-fmriprep` (`validation/src/larmorx_validation/parity/
confounds_scripts.py`) run every check script of the confounds as cases, each in the environment its id
names (`e1` the newest with `OPENBLAS_L2_SIZE=512`, `e1-zen` the same at the machine's `GEMM_Q` 512, `e2`
the shared environment, `e3` fMRIPrep 25.2.6's image through Apptainer), with each script's settings as
validated: `smoke` the quick tiers of `check_blas`, `check_dvars` and `check_tables` (about 35 s);
`standard` every script's quick tier in every environment it was validated in and the quick negative
controls (62 of 62 cases pass, 3,227 checks); `full` the full tiers, `e1-zen`, the full controls and the
extra runs. A negative control passes when the releases are told apart. The records (standard tier, from
one clean commit, 2026-11-13, with the permanent cases of random testing): `confounds-linalg` 32 of 32 cases
pass (1,673 checks), `confounds-nipype` 24 of 24 (570 checks; 8 permanent cases), `confounds-fmriprep` 16 of
16 (990 checks; 2 permanent cases) ([docs/validation/](../validation/confounds-fmriprep.md)): 3,233 checks
of 72 cases, against the earlier run's 3,227 of 62 (since then the scripts' timing lines are left out of
the records and the 10 permanent cases joined).

## Random testing (validated in part; one difference open)

Three tools, one per suite (`validation/src/larmorx_validation/difftest/tools/confounds.py`, runner
`parity/confounds_random.py`; [docs/validation/difftest.md](../validation/difftest.md)), one drawn call per
case against the original in-process, every output bit for bit, then the D13 fix pass. The probes before the
trial and the trial (`oracles/difftest/2026-11-12-confounds-trial`, 30 minutes, 334,124 cases: 332,076 agree,
528 notices, 28 differ, 1,492 skipped while numpy's `pow` is AVX-512's SVML) found these larmorx mistakes,
each fixed with a permanent case (`validation/difftest/cases/confounds-*/`) and the triage in
`validation/difftest/triage.toml`:

- CompCor's refused inputs raised larmorx's own messages: now traits' `TraitError` texts (as `ValueError`),
  nipype's `OSError` of two exclusive inputs and its `ValueError` of a missing `mask_files`.
- `regress_poly` of Fortran-ordered rows with degree 0: numpy's `pinv(X).dot(data.T)` takes `gemv` on a
  C-ordered `data.T`, whose sums run in another order (`probe_regress_layout.py`, 264 of 264 after).
- The `KeyError` of `FilterDropped` and `RenameACompCor` for a `retained` column of text prints pandas'
  `Index` repr with its line breaks at width 80 (`probe_index_repr.py`, 260 of 260 in pandas 3.0.6 and 2.2.3).
- CompCor with no volume left (`ignore_initial_volumes` at least the series' length) raises numpy's
  `cannot reshape array of size 0 into shape (0)` where nipype's mask loop meets it; PIPE-44's correction
  is named in that error and in the shape error of a mask nipype never reaches (`probe_zero_volumes.py`,
  224 of 224).
- Notices that fired where nothing changed are named now only where the result changes: GLIBC-07 in the
  cosine basis (the witness compares the basis values: a cosine one bit off can round to the same
  product), DVARS' PIPE-46, -47 and -50 (an all-constant series is NaN both ways), PIPE-67 after
  `minimum_contiguous`'s filling, PIPE-44 where a later mask adds components or changes the basis.

Explained: PD-01's notice names the table read's correction, whose values change; `ExpandModel`, fMRIPrep's
`FramewiseDisplacement` and `SpikeRegressors` without `concatenate` may not show it. A harness matter: two
refused CompCor inputs at once, whose error nipype chooses by its caller's keyword order.

**Fixed (a larmorx mistake, relay 3a-iv)**: numpy's `svd`/`lstsq` of the cosine drift of 370 volumes at a
cut-off that gives more columns than volumes (5 s at TR 3 s: 370 × 444; 4 s at TR 1 s: 185 × 370) differed
from numpy 2.3.5's LAPACK by 1 to 4 ulps in a cluster of near-equal singular values (finding 54131;
`scan_drift_svd.py`: 742 of 744 drift matrices identical). At `DBDSDC`'s real epsilon (`DLAMCH('E')` =
2^-53) the rotated problem is one subproblem; its `DLASD0` differed in one `DLASDQ` leaf (21 rows, the
diagonals nearly equal, the superdiagonals at the convergence tolerance), and that leaf's `DBDSQR`,
emulated in Python from `dbdsqr.f` with the library's own `DLARTG`, `DLAS2` and `DLASV2`, gave the
library's bits and not larmorx's. The cause: larmorx's `TOLMUL` (`EPS**MEIGTH`, which gfortran computes
with glibc's `pow` at run time) was the constant 98.70092381500355 instead of 2^6.625 =
98.70149282610821, correctly rounded, so `TOL` was 5.8e-6 too small in relative terms and a superdiagonal
between the two tolerances converged one sweep later. The constant is fixed (`numpy_linalg::DBDSQR_TOLMUL`,
a unit test against `larmorx_core::math::pow` and `pow_glibc`); it serves every `DBDSQR` of the port
(`DLASDQ` under `DBDSDC`, `DLALSD` and `DLASDA`, SciPy's `gesvd`, the `m × 2` SVD and `lstsq` of tedana's
fits, the 3 × 3 SVD of nibabel's header arithmetic). After it: `scan_drift_svd.py` 744 of 744, the wide
scan (`--t 300 471 1 --lstsq`: every `T` from 300 to 470, six cut-offs, both orientations, `svd` and
`lstsq`) 4,104 of 4,104 identical in numpy 2.3.5 and in numpy 2.5.4 (`OPENBLAS_L2_SIZE=512`); the quick
tiers of `check_svd.py`, `check_gesdd.py`, `check_gelsd.py`, `check_gesvd.py`, `check_compcor.py` (and
CompCor's real nodes in fMRIPrep 25.2.6's image) and the field maps' `check_numpy_linalg.py` unchanged
(every case identical); the finding agrees on replay and is the permanent case
`cosine-filter-wide-basis-dbdsqr-tolerance`. No fMRIPrep call reaches the shape (its cut-off of 128 s keeps
the basis narrower than the series).

**What else the fix reaches, re-checked (2026-11-13).** Every other part whose code calls this `DBDSQR`
(a grep of `numpy_linalg`'s callers: the field maps through nibabel's header arithmetic and `io_orientation`,
`lx.niworkflows`, tedana's `t2smap` replica in `larmorx-gpl` and its clean room through `lstsq` and
`curve_fit`'s `m × 2` SVD) had its records re-made from one clean commit of this branch, the programs built
from it, and compared case by case with the committed ones
(`validation/investigations/confounds/tolmul_reach.sh`, `compare_remade_records.py`): **no result changed**.
`sdcflows-fieldmap` 37 of 37 cases identical (821 checks), `sdcflows-estimation` 19 of 19 (the contract
audit's counts of public functions and stubs, which the confounds raise, and `match-histogram`'s message
text, which a later fix of the field maps' random testing made numpy's, are the only fields that differ;
its 8 permanent cases added since pass), `sdcflows-estimation-full` 14 of 14 (245 checks),
`sdcflows-estimation-app-shared` 8 of 8 (152), `niworkflows-helpers` 5 of 5 (its 2 permanent cases added
since pass), `tedana-t2smap-replica` 102 of 102 (1,000 checks: 62 pass, 40 refused by both alike),
`tedana-t2smap-original` 125 of 125 (1,637 checks). The golden checksums recorded after the fix: every one
of the 318 cases recorded before it unchanged (the 15 new ones of this part added: 333 cases, 3,586
outputs). Records made after the field maps' `129854e9` whose code calls none of these routines
(`fsl-prelude`, `mri-hmc`, `ndimage-skimage`, `nifti-io`) were not re-made; `numpy-linalg`'s
`ensure_positive_cosines` takes vnl's SVD, not this one. A superdiagonal between the two tolerances
(relative width 5.8e-6) is the only way a result could have changed, and none of these calls meets one.
The full tiers of this part's scripts through `DBDSQR`, after the fix: the record `confounds-linalg-full`
(`check_svd`, `check_gesdd`, `check_gelsd`, `check_gesvd` in numpy 2.3.5, in numpy 2.5.4 and in numpy 2.5.4 at
the machine's `GEMM_Q` 512: 12 of 12 cases, 840 checks, up to CompCor's largest real shapes) and
`confounds-nipype-full` (`check_compcor`'s full tier: fMRIPrep's 108 real CompCor nodes against fMRIPrep's
files and against nipype, in the three environments: 3 of 3 cases, 90 checks).

## Follow-ups (open; each a bounded piece)

- **PD-01 per step**: name PD-01 in `ExpandModel`, fMRIPrep's `FramewiseDisplacement` and
  `SpikeRegressors` only where the step's output changes.

- **`OPENBLAS_L2_SIZE` for everyone**: numpy 2.5.4 on this AMD Zen 4 machine blocks its products'
  depth by 512, not 256 ([confounds-blas.md](confounds-blas.md)); the confounds' runs set
  `OPENBLAS_L2_SIZE=512` (any value other than 1024 would do). The proposal: set it in
  `scripts/orchestration/env.sh` for every part at a quiet boundary, add it to the oracle cache's
  environment family (so cached entries are keyed by it), and re-check the earlier numpy 2.5.4 records
  made on this machine whose products have a depth over 256 (tedana's `t2smap` replica record,
  sdcflows' fit, `numpy_linalg`'s tall matrices): where such a product is reached, their "identical in
  both environments" holds for this machine's OpenBLAS setting only, and the record says so or is
  re-made with the 256 setting.
- **glibc's `atan2`** as a clean room from a behaviour spec (as `sincos_glibc`): makes `reproduce` exact
  on the 15 real rotation cells ([confounds-motion.md](confounds-motion.md)); `hypot` needs none.
- **Not ported, refused with a clear error**: LAPACK's `DLASQ1` (values without vectors:
  `svd(compute_uv=False)`, `DLASDA(ICOMPQ = 0)` with `2 <= N <= 25`, `DBDSQR` without vectors), numpy's
  `sgesdd`, `fallback_svd(full_matrices=True)`, SciPy's FFTs of lengths that are not powers of two and
  ducc0's Bailey algorithm (series of more than 32,768 volumes), nitime's `AR_est_YW` above order 1 and of
  float64 series, nitransforms' `.mat`, `.h5` and LTA readers. Port each when a caller needs it.
- **fMRIPrep's `warnings.warn` replacement** (above).
- **PIPE-42** with larmorprepx's reports; **PIPE-79** measured on fMRIPrep runs with FreeSurfer.
- **Speed** (the bits unchanged): the level-1/2 kernels' FMA dispatch and column splitting, `VT` not
  computed where CompCor does not use it, finer `dgemm` tiles, the FFT passes' buffers, DVARS' fused
  products: [docs/benchmarks/confounds.md](../benchmarks/confounds.md) measures where the time goes.
