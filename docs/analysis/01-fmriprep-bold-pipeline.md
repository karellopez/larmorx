<!-- Generated 2026-10-08 by a read-only analysis agent against shallow clones in the parent directory.
     fmriprep @ 21a490fb (26.0.0.dev). Line numbers refer to those clones and will drift. -->

# fMRIPrep BOLD pipeline: stages, external binaries, CLI toggles, tests and POSIX issues

**Scope.** HEAD is `21a490fb` (2026-10-07), version 26.0.0.dev; the last release was 25.2.6 (CHANGES.rst:1, :29).

**Path convention.** Paths are relative to `fmriprep/fmriprep/` unless they start with `niworkflows/`, `sdcflows/`, `nipype/` or `smriprep/`, which are sibling clones in the workspace root.

---

## 0. Most important findings

1. **All 4D BOLD resampling is already pure Python.** `interfaces/resampling.py` `ResampleSeries` uses nitransforms plus `scipy.ndimage.map_coordinates` (order 3, prefilter, mode `grid-constant`), runs volumes concurrently on a thread pool, and applies HMC, SDC (voxel-shift map), coregistration and the template warp in one step.
   - ANTs `.h5` warps are evaluated in Python via `nt.manip.load` (utils/transforms.py:21-23).
   - `antsApplyTransforms` is only used for single 3D volumes: masks, boldrefs, T2* maps, the carpet-plot segmentation and reportlets.
   - This is the biggest CPU consumer you already control. It is a prime Rust target, and it needs no binary-equivalence work.
2. **FreeSurfer is a hard dependency even with `--fs-no-reconall`.**
   - `cli/workflow.py:119-138` runs `mri_convert` as a license probe (niworkflows/utils/misc.py:358-378) and exits with code 126 if it fails.
   - The FSL coregistration path still runs `mri_coreg` (workflows/bold/registration.py:600-605) and `lta_convert` (:646, :704).
3. **ICA-AROMA was removed in 23.1.0** (CHANGES.rst:608, :627, PR #2966). `config.workflow.use_aroma`, `aroma_melodic_dim` and `aroma_err_on_warn` remain as dead fields (config.py:566-569, :638).
4. **Dead or no-op CLI options:**
   - `--force-bbr`, `--force-no-bbr` and `--force-syn` only print a deprecation notice (cli/parser.py:50-70). The attributes they set are never read; the code reads `config.workflow.force` instead.
   - `--fmap-bspline` and `--fmap-no-demean` are stored in `config.workflow.fmap_bspline` / `fmap_demean` but never consumed.
5. **Windows blockers inside fmriprep itself:**
   - `set_start_method('forkserver')` raises `ValueError` on Windows, but only `RuntimeError` is caught (config.py:103-106).
   - `os.path.join(os.getenv('HOME'), ...)` raises `TypeError` when `HOME` is unset (config.py:177).
   - A `NamedTemporaryFile` is passed to a subprocess while still open, which Windows does not allow (utils/bids.py:448-452).

---

## 1. Package layout

| Path | Role |
|---|---|
| `__main__.py`, `cli/run.py` (`main`, :28) | Entry point (`fmriprep = fmriprep.cli.run:main`). Parses args, writes config TOML to `work/<run_uuid>/config.toml` (:77-79), builds the workflow in a child `multiprocessing.Process` (:84-93), builds the boilerplate in another Process (:117-122), runs `fmriprep_wf.run(**config.nipype.get_plugin())` (:145), then generates reports and `dataset_description.json` (:211-229). |
| `cli/parser.py` | argparse definitions (`_build_parser`) and `parse_args` (:806+): consistency checks, output-spaces default, work-dir cleanup, bids-validator call, BIDSLayout init, processing groups. |
| `cli/workflow.py` | `build_workflow` (:35): FS-license gate (:119), `check_deps` (:141, return code 127 on missing binaries). `build_boilerplate` (:156): pandoc. |
| `cli/version.py` | Online "latest version" and "flagged version" checks. |
| `config.py` | Global singleton config. Sections are classes: `environment`, `nipype`, `execution`, `workflow`, `seeds`, `loggers`. Serialized with `dumps`/`to_filename`/`load` (TOML). `init_spaces` (:850-878) always adds MNI152NLin2009cAsym; with `--cifti-output` it adds MNI152NLin6Asym res-2 (91k) or res-1 (170k). |
| `workflows/base.py` | `init_fmriprep_wf` (:50) and `init_single_subject_wf` (:125): anatomy (smriprep `init_anat_fit_wf`, :364), fieldmap estimation (sdcflows, :629-815), BOLD coregistration groups (:842-924), per-run fit/apply wiring (:943-1203). Also `map_fieldmap_estimation` (:1208) and `get_estimator` (:1294). |
| `workflows/bold/fit.py` | `init_bold_fit_wf` (:103): fit stages 1-4. `init_bold_native_wf` (:571): STC, native resampling, multi-echo. `get_sbrefs` (:59). |
| `workflows/bold/reference.py` | `init_raw_boldref_wf` (:32) and `init_validation_and_dummies_wf` (:137). |
| `workflows/bold/hmc.py` | `init_bold_hmc_wf` (:36): mcflirt. |
| `workflows/bold/stc.py` | `init_bold_stc_wf` (:57) and the `TShift` patch (:43). |
| `workflows/bold/t2s.py` | `init_bold_t2s_wf` (:45, tedana `t2smap`) and `init_t2s_reporting_wf` (:140). |
| `workflows/bold/coreg.py` | `init_bold_anat_coreg_wf` (:65), which dispatches to `init_bold_run_coreg_wf` (:169) or `init_bold_template_coreg_wf` (:271). |
| `workflows/bold/template.py` | `init_bold_template_wf` (:33): mri_robust_template. |
| `workflows/bold/registration.py` | `init_bold_reg_wf` (:51), `init_bbreg_wf` (:200), `init_fsl_bbr_wf` (:464), `compare_xforms` (:726), `_conditional_downsampling` (:769). |
| `workflows/bold/apply.py` | `init_bold_volumetric_resample_wf` (:14): one-shot resampling to any grid. |
| `workflows/bold/base.py` | `init_bold_apply_wf` (:54): the transform/apply half (native, T1w, std, surfaces, CIFTI, confounds, carpet plot). |
| `workflows/bold/resampling.py` | Surface and goodvoxels workflows: `init_bold_surf_wf` (:56), `init_goodvoxels_bold_mask_wf` (:256), `init_wb_vol_surf_wf` (:519), `init_wb_surf_surf_wf` (:657), `init_bold_fsLR_resampling_wf` (:823), `init_bold_grayords_wf` (:1034). |
| `workflows/bold/confounds.py` | `init_bold_confs_wf` (:50) and `init_carpetplot_wf` (:631). |
| `workflows/bold/outputs.py` | DataSink sub-workflows plus `init_func_fit_reports_wf` (:146). `init_bold_preproc_report_wf` (:975) is defined but never used. |
| `interfaces/resampling.py` | `ResampleSeries`, `ReconstructFieldmap`, `DistortionParameters` and helpers (pure Python). |
| `interfaces/workbench.py` | `wb_command` wrappers: MetricDilate, MetricResample, VolumeToSurfaceMapping, MetricMask, MetricFillHoles*, MetricRemoveIslands* (*unused in fmriprep). |
| `interfaces/multiecho.py` | `T2SMap` CommandLine wrapper around the tedana `t2smap` CLI. |
| `interfaces/confounds.py` | aCompCorMasks, FSLRMSDeviation, FSLMotionParams, FramewiseDisplacement, FilterDropped, RenameACompCor, GatherConfounds, FMRISummary (pure Python). |
| `interfaces/patches.py` | RobustA/TCompCor (retry on LinAlgError), MRICoreg patch (allows `--ref` together with `--s`), FreeSurferSource patch (exposes T2). |
| `interfaces/nitransforms.py` | `ConvertAffine` (Python; covers what lta_convert / c3d_affine_tool would do). |
| `interfaces/maths.py`, `gifti.py`, `reports.py`, `bids.py`, `__init__.py` | Python helpers: Clip, Label2Mask, CreateROI (unused), LabeledHistogram, Summary HTML, BIDSURI, BIDSSourceFile, CreateFreeSurferID, DerivativesDataSink subclass. |
| `utils/` | `bids.py` (derivatives collection, validator, dataset_description), `confounds.py` (acompcor_masks), `misc.py` (check_deps, fips_enabled, estimate_bold_mem_usage), `transforms.py` (load_transforms), `asynctools.py` (thread worker), `telemetry.py` (sentry/migas), `testing.py` (derivative skeletons), `meepi.py`, `debug.py`. |
| `reports/core.py` | `run_reports` / `generate_reports` using nireports `Report` (Python). |
| `data/` | `flirtsch/bbr.sch` (fallback BBR schedule), `io_spec.json` and `fmap_spec.json` (derivative query specs), `reports-spec*.yml`, `boilerplate.bib`, `tests/` (mock `config.toml`, `derivatives.yml`, ds000005 header stubs, reportlet fixtures). |

### Fit vs apply split and `--level` (cli/parser.py:350-358)

- **`minimal`**:
  - Builds only `init_bold_fit_wf` (HMC reference, HMC transforms, fieldmap registration, coregistration reference and mask), `init_bold_anat_coreg_wf` and the fit reports.
  - It skips `bold_apply_wf` with `continue` at workflows/base.py:1111-1112.
  - At the anatomical level, the template iterator, FreeSurfer derivatives and CIFTI morphometrics only run when the level is `full` (workflows/base.py:427).
- **`resampling`**:
  - Also builds `init_bold_apply_wf`, but it returns right after `bold_native_wf`, echo outputs and T2* reportlets (bold/base.py:334-340).
  - Native BOLD is not written, because `run_out &= level == 'full'` (bold/base.py:271).
- **`full`**:
  - Everything: T1w and std volumes, surfaces, CIFTI, confounds and the carpet plot.

### Derivatives reuse (`--derivatives`)

- **Anatomy:** smriprep `collect_anat_derivatives` (workflows/base.py:278-291).
- **Fieldmaps:** `collect_fmap_derivatives` (utils/bids.py:264; spec in `data/fmap_spec.json`). A fieldmap with fieldmap, coefficients and magnitude is used directly instead of being estimated (workflows/base.py:654-686).
- **BOLD:** `collect_func_derivatives` (utils/bids.py:232; spec in `data/io_spec.json`) gives `hmc_boldref`, `run_boldref` and transforms `hmc`, `run2fmap`, `run2anat`, `run2template`, plus session/subject `boldref` and `xfm`.
  - `init_bold_fit_wf` skips stage 1 (fit.py:321/356), stage 2 (:372/394), stage 3 registration (:413/450) and stage 4 (:457/558) individually.
  - Coregistration skips per run (coreg.py:220-223) or per group (coreg.py:315-353).
  - Grouped caches are combined by `aggregate_coreg_precomputed` (utils/bids.py:133).

### How config is passed

- Workflow builders read the module-global `fmriprep.config.<section>.<attr>` directly. A few values are also passed as explicit keyword arguments.
- The config is shared across processes via TOML: `cli/run.py:79` writes it, `cli/workflow.py:46` loads it in the child, then the parent reloads it (`cli/run.py:104`). A per-subject copy goes to `log/<uuid>/fmriprep.toml` (workflows/base.py:119-120).
- `execution.layout` is a pybids `BIDSLayout` backed by SQLite at `work/<uuid>/bids_db` (config.py:502-516).
- Thread counts:
  - `omp_nthreads` defaults to `min(nprocs-1, 8)` (config.py:382).
  - The plugin is MultiProc with `maxtasksperchild=1` (config.py:322-327).
  - nipype copies node `n_procs` into the interface's `num_threads` (nipype/pipeline/engine/nodes.py:202-203), so `ResampleSeries` really uses `omp_nthreads` threads.

---

## 2. BOLD workflow, stage by stage (execution order)

**Subject-level order:**
1. smriprep `anat_fit_wf` (workflows/base.py:364).
2. sdcflows fieldmap estimation (:701; SyN preprocessing :783-815).
3. Per run: `bold_fit_wf` (:963).
4. Per coregistration group: `bold_anat_coreg_wf` (:910).
5. Per run: `bold_apply_wf` (:1119).
6. Fit reports per run (:1007).

`--bold-coreg-level` defaults to `run`. For `session`/`subject`, groups are checked by `is_valid_bold_template` (:857-868).

| # | Stage | Python file:function (lines) | Interfaces | External binary (actual exec) | Notes |
|---|---|---|---|---|---|
| 0 | FreeSurfer subjects dir | workflows/base.py `init_fmriprep_wf` :81-90 | niworkflows `BIDSFreeSurferDir` | none; Python `shutil.copytree` of `$FREESURFER_HOME/subjects/fsaverage*` | Needs FreeSurfer *data* (fsaverage trees). |
| 1a | Header validation and dummy detection | reference.py `init_validation_and_dummies_wf` :137; ValidateImage :202; NonsteadyStatesDetector :208; pass_dummy_scans :210 | niworkflows header/bold | none (Python) | Detector: nipype `is_outlier` on the global mean of the first 40 volumes (niworkflows/interfaces/bold.py:73-102). `t_mask` = dummy volumes if there are at least 2, else volumes `[min(T,40)-20, min(T,40))`. |
| 1b | HMC reference (`space-orig_desc-hmc_boldref`) | fit.py :316-354; reference.py `init_raw_boldref_wf` :32; RobustAverage :114 | niworkflows `RobustAverage` (images.py:240-355) | **`3dvolreg`** (AFNI): `-Fourier -twopass -zpad 4`, run inside RobustAverage (mc_method default `AFNI`, images.py:213-219, :319-330) | Clip to p99.8, normalize global drift, align the selected volumes (≤20), voxelwise median. Shortest echo only (fit.py:202). |
| 2 | Head-motion estimation | fit.py :371-392; hmc.py `init_bold_hmc_wf` :36; mcflirt :93; fsl2itk :95 | nipype `fsl.MCFLIRT(save_mats=True)`; niworkflows `MCFLIRT2ITK` (itk.py:57-80, Python/nitransforms) | **`mcflirt`** | Input is the validated **raw (pre-STC)** BOLD, all volumes; ref = HMC boldref. Only the `.mat` files are used; mcflirt's resampled 4D output is discarded. Output: `from-orig_to-boldref_mode-image_desc-hmc_xfm.txt` (ITK, N affines). |
| 3a | Fieldmap → boldref registration | fit.py :413-448; sdcflows `init_coeff2epi_wf` (sdcflows/workflows/apply/registration.py:129-131); ConcatenateXFMs :422 | niworkflows `FixHeaderRegistration` | **`antsRegistration`** with `fmap-any_registration.json` (or `_testing.json` under `--sloppy`) | Rigid, 2 stages: Mattes 32 bins; shrink 2→1; smoothing 8→2 mm; 50/20 iterations; histogram matching; winsorize 0.001/0.999; masks dilated by 5 voxels. Target = first sbref else HMC boldref (`_select_ref` :847). |
| 3b | Fieldmap reconstruction in boldref | fit.py :400 | `ReconstructFieldmap` (interfaces/resampling.py:158-188, :637-725) | none (Python: sdcflows `grid_bspline_weights` + scipy.sparse) | Evaluates the B-spline coefficient field in Hz in the target grid. Falls back to the fmap reference grid plus nitransforms resampling when grids are not aligned. |
| 4a | sbref reference (optional) | fit.py :460-467 | RobustAverage | **`3dvolreg`** (only if sbref is 4D) | `--ignore sbref` disables it. There is no explicit sbref↔boldref registration; they are assumed aligned in scanner space. |
| 4b | Coregistration reference: enhance and skull-strip | fit.py :469; niworkflows/func/util.py `init_enhance_and_skullstrip_bold_wf` :395-560 → `init_bold_premask_wf` :282-392 | AI, Registration, ApplyTransforms, N4, BET, Unifize, Automask, BinaryMaths | **`antsAI`** (:334-351): Mattes 32 Regular 0.2, Rigid[0.1], search factor (20, 0.12), grid (40,(0,40,40)). **`antsRegistration`** (:354-358): Affine to MNI152NLin2009cAsym res-2 `desc-fMRIPrep_boldref`; Mattes 64; 200 iterations; shrink 2; smoothing 2 mm; histogram matching; winsorize 0.05/0.98 (niworkflows/data/epi_atlasbased_brainmask.json). **`antsApplyTransforms`** (:360-375, Linear, MNI brain probseg → BOLD). **`N4BiasFieldCorrection`** (:487-493): `-d 3`, b-spline distance 200, shrink 2, rescale, weight = premask, 1 thread. **`bet`** `-f 0.2 -m` (:496). **`3dUnifize`** `-T2 -clfrac 0.2 -rbt 18.3 65.0 90.0` (:501-510). **`3dAutomask`** `-dilate 1` (:515). **`fslmaths -mul`** (:521). | `pre_mask=False` default, so the MNI affine premask runs for every BOLD run. Output `bias_corrected_file` is the coregistration reference when there is no SDC. |
| 4c | SDC-unwarped coregistration reference | fit.py :498-547 | DistortionParameters :499 (Python; sdcflows `get_trt`); `ResampleSeries` :509-514 (Python, 3D); `init_skullstrip_bold_wf` :516 (func/util.py:562-610) | **`bet -f 0.2 -m`**, **`3dAutomask -dilate 1`**, **`fslmaths -mul`** | Also used for a precomputed `run_boldref` (fit.py:562). Outputs `space-run_boldref` and `space-run_desc-brain_mask`. |
| 5a | BOLD→anat coregistration, FreeSurfer path (default) | coreg.py :65/:169/:271 → registration.py `init_bold_reg_wf` :51 → `init_bbreg_wf` :200 | patched `MRICoreg` :333-340; `BBRegister` :342-352; FreeSurferSource :331; ConcatenateXFMs :363 (Python) | **`mri_coreg`**: `--s <subj> --sd $SUBJECTS_DIR --mov <boldref> --dof {6,9,12} --sep 4 --ftol 0.0001 --linmintol 0.01 --lta`; with T2w init also `--ref <subj>/mri/T2.mgz --no-ref-mask` (:339-340, :400-405). **`bbregister`**: `--t2 --s --mov --init-reg <mri_coreg.lta>` (or `--init-header`) `--{dof} --lta --initcost bbregister.initcost`. bbregister is a tcsh script that runs `mri_segreg` and friends. | `use_bbr=None` (default) runs both and `compare_xforms` (:726-766, Python) falls back to mri_coreg if the normalized displacement is >15 mm. `--force bbr` gives BBR only (:445-448); `--force no-bbr` gives mri_coreg only (:408-411). Header init means no mri_coreg (:324-329, :353-354). |
| 5b | BOLD→anat coregistration, FSL path (`--fs-no-reconall`) | registration.py `init_fsl_bbr_wf` :464-723 | ApplyMask :598 (Python); nipype `MRICoreg` :600-605; `PatchedLTAConvert` :646, :704; `fsl.FLIRT` :648-651; ConvertAffine :607 (Python) | **`mri_coreg --mov boldref --ref T1w_brain --dof N --sep 4 --ftol 1e-4 --linmintol 0.01`**; **`lta_convert`** (LTA→FSL, plus FSL→LTA MapNode ×2 when use_bbr=None); **`flirt -cost bbr -dof N -wmseg <WM label 2 of dseg> -init <mri_coreg in FSL format> -schedule $FSLDIR/etc/flirtsch/bbr.sch -basescale 1`** (:653-659; packaged fallback `data/flirtsch/bbr.sch`) | Header init raises NotImplementedError (:587-588). A T2w init only logs a warning and uses the T1w (:589-592). `--sloppy` downsamples to 4 mm in Python (:672-684, :769-811). |
| 5c | Session/subject BOLD template (`--bold-coreg-level session\|subject`) | coreg.py `init_bold_template_coreg_wf` :271; template.py `init_bold_template_wf` :33; StructuralReference :93-107; to_itk :109-113; run2anat ConcatenateXFMs coreg.py:366 | niworkflows `StructuralReference` (nipype RobustTemplate) | **`mri_robust_template`**: `--satit --iscale --subsample 200 --inittp 1`, plus `--fixtp --noit` when fewer than 3 runs; outputs per-run LTAs. Also **`antsApplyTransforms`** Lanczos (coreg.py:459-464), only when rebuilding from precomputed run2template transforms. | One registration (5a or 5b) runs per template. Run→template→anat transforms are composed in Python. |
| 6 | Slice-timing correction | fit.py `init_bold_native_wf` :685, :744-750 (per-echo iterable :729-733) → stc.py `init_bold_stc_wf` :57; TShift :121-131; CopyXForm :133 (Python) | patched `afni.TShift` (:43-54: errors if fewer than 5 volumes remain after ignore) | **`3dTshift -tpattern @slice_timing.1D -TR <TR>s -tzero <t0> -ignore <skip_vols>`**, NIFTI_GZ output | t0 = round(min + f·(max−min), 3), with f = `--slice-time-ref` (:104-107). `SliceEncodingDirection` `k-` reverses the timing (nipype afni/preprocess.py:3292). No `interp` is passed, so AFNI's default (Fourier) applies. Runs only if `SliceTiming` is present and `slicetiming` is not ignored. Applied to raw data after HMC was estimated; HMC is applied later in the one-shot resample. |
| 7 | Native (boldref-space) one-shot resample: HMC + SDC | fit.py :772-802 | `ResampleSeries` (interfaces/resampling.py:77-128, :235-611); `ReconstructFieldmap` :793 | none (Python) | Per volume: target RAS → source voxel, then per-volume HMC vox2vox, then voxel-shift map (Hz × signed readout time) added along the PE axis, then `map_coordinates(order=3, prefilter=True, mode='grid-constant', cval=0)`. Optional Jacobian (1 + ∂VSM/∂PE). asyncio + thread executor, float32. Single-echo: this `bold_native` feeds confounds, while `bold_minimal` (STC only) is forwarded for later one-shot resamples. |
| 8 | Multi-echo combination | fit.py :804-836 → t2s.py `init_bold_t2s_wf` :45; BinaryDilation r=2 :119 (Python); T2SMap :121-125 | `interfaces/multiecho.py` `T2SMap` (:84-119) | **`t2smap`** (tedana console script, pure Python): `-d e1 e2 e3 -e TE1 TE2 TE3` (ms, :108-111) `--mask <dilated run_mask> --fittype {curvefit,loglin} --exclude 0:<skip_vols>` | Requires ≥3 echoes (fit.py:680-683); 2 echoes is an error (workflows/base.py:947-955). HMC is estimated on echo 1 and applied to all echoes. The optcom series becomes `bold_minimal`/`bold_native`, and `motion_xfm` and the fieldmap are dropped downstream (fit.py:821-822; bold/base.py:352, :414, :547). Multi-echo outputs are therefore **interpolated twice**. Reporting: `antsApplyTransforms` MultiLabel (t2s.py:193-196). |
| 9 | Volumetric one-shot resample to T1w / std / MNI6 | apply.py `init_bold_volumetric_resample_wf` :14; GenerateSamplingReference :126 (Python); chain `[motion_xfm, run2template, template2anat, anat2std]` :128-153; ResampleSeries :130-135; fieldmap recon in target :161-196 | ResampleSeries, ReconstructFieldmap, DistortionParameters | none for the 4D series. Companion files in each space use **`antsApplyTransforms`**: boldref (Lanczos), mask (MultiLabel), T2* (Lanczos) (outputs.py:873-930) | Instances: `bold_anat_wf` (bold/base.py:349, **always built at full level**), `bold_std_wf` (:411, per std space via smriprep template iterator), `bold_MNI6_wf` (:545, CIFTI only). Boilerplate (bold/base.py:178-185) states "nitransforms, cubic B-spline". |
| 10 | FreeSurfer surface sampling (fsnative / fsaverage*) | bold/base.py :498-537 → resampling.py `init_bold_surf_wf` :56; itk2lta :171-173 (Python); sampler :174-187; MedialNaNs :238-251; GiftiSetAnatomicalStructure (Python) | nipype `fs.SampleToSurface` | **`mri_vol2surf --hemi {lh,rh} --interp trilinear --projfrac-avg 0 1 0.2 --srcsubject <subj> --trgsubject {subj\|fsaverageN} --reg <lta> --out_type gii`** | Only with recon-all. Input is the T1w-space resampled BOLD from stage 9, so this is a second interpolation. |
| 11 | Goodvoxels mask (`--project-goodvoxels`) | bold/base.py :466-496 → resampling.py `init_goodvoxels_bold_mask_wf` :256-516 | fsl.maths.*, fsl.ImageStats, fsl.ApplyMask, ApplyTransforms | **`antsApplyTransforms`** (MultiLabel, identity; ribbon → BOLD grid, :308-312); **`fslmaths`** ×14 (`-Tstd`, `-Tmean`, `-div`, `-mas`, `-bin -s 5`, `-s 5 -div %s -dilD`, `-thr`, `-bin`, `-bin -sub %s -mul -1`, …); **`fslstats -M/-S`** ×4 (:338-348, :415-425) | HCP goodvoxels: CoV, locally normalized CoV, threshold at mean + 0.5·SD. Trivially numpy. Only wired into CIFTI (:560-565) and Workbench std surfaces (:659-664), not into mri_vol2surf, even though the CLI help says fsaverage/fsnative. |
| 12a | CIFTI surfaces (fsLR) | bold/base.py :539-628 → resampling.py `init_bold_fsLR_resampling_wf` :823-1031 (hemisphere iterable :916-920) | VolumeToSurfaceMapping, MetricDilate, MetricMask, MetricResample | **`wb_command -volume-to-surface-mapping <bold_T1w> <midthickness> out -ribbon-constrained <white> <pial> [-volume-roi goodvoxels]`** (:969-974); **`-metric-dilate … 10 -nearest`** (:975-980); **`-metric-mask`** cortex (:981); **`-metric-resample … ADAP_BARY_AREA -area-surfs <mid> <mid_fsLR> -current-roi <cortex>`** sphere_reg (fsLR or MSM) → fsLR 32k/59k sphere (:982-987); **`-metric-mask`** atlasroi (:989; smriprep/data/atlases) | Wrappers set `OMP_NUM_THREADS` from `num_threads` (interfaces/workbench.py:21-46). |
| 12b | CIFTI assembly | resampling.py `init_bold_grayords_wf` :1034; GenerateCifti :1105-1112 | niworkflows `GenerateCifti` (interfaces/cifti.py) | none (Python: nibabel, nilearn) | Combines the fsLR GIFTIs with the MNI152NLin6Asym res-2/1 volume from stage 9 (subcortical labels from TemplateFlow). |
| 12c | Workbench std surfaces (fsLR/onavg with density) | bold/base.py :630-729: `init_wb_vol_surf_wf` :643 (resampling.py:519); smriprep `init_resample_surfaces_wf` :675; `init_wb_surf_surf_wf` :682 (resampling.py:657) | as above, plus smriprep SurfaceResample | **`wb_command -volume-to-surface-mapping -ribbon-constrained`**, **`-metric-dilate 10 -nearest`**, **`-surface-resample`** (smriprep/interfaces/workbench.py:688), **`-metric-resample ADAP_BARY_AREA -area-surfs`** (:781-786) | Nested inside `if cifti_output` (:539→:630), so it only runs together with `--cifti-output`. Surfaces are re-resampled per BOLD run, which is redundant. |
| 13 | Confounds | bold/base.py :731-771 → confounds.py `init_bold_confs_wf` :50-628 | see list below | only **`antsApplyTransforms`**: T1w mask → BOLD, MultiLabel, inverse run2anat (:256-259); CSF/WM/combined PV maps → BOLD, Gaussian, ×3 MapNode (:287-292) | Everything else is pure Python; input is `bold_native` (boldref space). See the list below the table. |
| 14 | Carpet plot | bold/base.py :773-804 → confounds.py `init_carpetplot_wf` :631; resample_parc :732-749; FMRISummary :703-716 | ApplyTransforms, nireports fMRIPlot | **`antsApplyTransforms -u int`**, MultiLabel: MNI2009c `desc-carpet_dseg` → BOLD via [run2anat⁻¹, std2anat] | Always active at full level, because MNI2009c is always added internally (config.py:865). |
| 15 | Fit reports | outputs.py `init_func_fit_reports_wf` :146-453 | t1w_boldref :272-282 (Lanczos, inverse); boldref_wm :291-299 (NN); fmapref_boldref :342-352 (Lanczos, SDC only); SimpleBeforeAfter, FieldmapReportlet | **`antsApplyTransforms`** ×2-3 per run; optional `svgo`/`cwebp` | HTML assembly in reports/core.py (Python). |
| 16 | Boilerplate and validation | cli/workflow.py :156-222; utils/bids.py :364-454 | | **`pandoc`** ×2 (optional); **`bids-validator`** (Node.js, optional) | Once per invocation. |

**Confounds details (stage 13, all Python):**
- **DVARS:** nipype `ComputeDVARS` with `save_nstd`, `save_std`, `remove_zerovariance`, inside run_mask (:270-274).
- **Motion parameters:** `FSLMotionParams` rebuilds mcflirt-style parameters from the ITK affines: center of gravity, Euler XYZ, FSL convention (interfaces/confounds.py:154-196).
- **Framewise displacement:** Power FD with radius 50 mm (:208-229).
- **RMSD:** Jenkinson RMSD, Rmax = 80 (:102-142).
- **aCompCor:**
  - Masks from `utils/confounds.acompcor_masks`: GM PV >0.05, dilated with ball(3), subtracted from CSF/WM/combined PV maps.
  - Moved into BOLD space, multiplied by run_mask, thresholded at 0.99.
  - `RobustACompCor` with cosine pre-filter and 50% variance threshold, or all components with `--return-all-components` (:295-308, :341-346).
- **tCompCor:** top 2% variance voxels (:326-338).
- **Crown/edge CompCor:** 24 components; mask = dilate(union(run_mask, T1w mask in BOLD)) minus the union, clipped to voxels with nonzero variance (:260-267, :310-324).
- **Signals:** `SignalExtraction` for global_signal, csf, white_matter, csf_wm, tcompcor (:368-370).
- **Model expansion:** `ExpandModel` with `(dd1(rps + wm + csf + gsr))^^2 + others` (:449-452).
- **Spikes:** `SpikeRegressors` using the FD and standardized-DVARS thresholds (:455-458).

**Stages that are already pure Python/numpy:**
- **Reference and transforms:** validation, dummy detection, the median step of the reference, MCFLIRT→ITK conversion, all affine conversion and composition (nitransforms ConcatenateXFMs/ConvertAffine), `compare_xforms`.
- **All 4D BOLD resampling.** Fieldmap B-spline reconstruction. Multi-echo T2*/optcom (tedana, called as a subprocess).
- **Confounds:** every confound and CompCor variant.
- **Surfaces and CIFTI:** CIFTI assembly, MedialNaNs, GIFTI metadata.
- **Reports and outputs:** all reportlets (matplotlib/nilearn), DataSinks.

---

## 3. Consolidated binary table

Frequencies are per BOLD run at the default `--bold-coreg-level run` unless stated. Binaries whose actual call sites live in sdcflows/smriprep are marked; other agents own those internals.

| Binary / subcommand | Toolkit | Where used (file:line) | Purpose | How often | Compute | Notes for re-implementation |
|---|---|---|---|---|---|---|
| `mcflirt` | FSL | workflows/bold/hmc.py:93 | HMC parameter estimation | 1 per run (echo 1) | **High** (per-volume rigid registration of the full series) | nipype defaults: normcorr cost, 6-DOF, 3-stage coarse-to-fine, trilinear. `-reffile`, `-mats`. Only the per-volume matrices are needed (no output resampling). Equivalence is checked through `desc-motion_timeseries`, FD and RMSD. |
| `3dvolreg` | AFNI | niworkflows/interfaces/images.py:319-330 (via reference.py:114; fit.py:462) | Align ≤20 reference volumes before the median | 1 per run (+1 per 4D sbref) | Low | `-Fourier -twopass -zpad 4`, base = first selected volume. Could be swapped for the Rust HMC kernel. |
| `3dTshift` | AFNI | workflows/bold/stc.py:121-131 | Slice-timing correction | 1 per run **per echo** | Low-Med (per-voxel 1D FFT) | AFNI default Fourier interpolation; `-tzero`; `-ignore N` leaves the first N volumes unchanged; `-tpattern @file` (seconds); `k-` reversal; TR in seconds. |
| `antsAI` | ANTs | niworkflows/func/util.py:334-351 | Rigid init of BOLD ref → MNI fMRIPrep boldref template | 1 per run | Low-Med | Mattes 32 / Regular 0.2, Rigid[0.1], search factor 20° / 0.12, grid (40,(0,40,40)), convergence (10, 1e-6, 10). |
| `antsRegistration` | ANTs | niworkflows/func/util.py:354-358 | Affine BOLD ref → MNI (premask) | 1 per run | Med | `epi_atlasbased_brainmask.json`: Affine, Mattes 64, 200 iterations, shrink 2, σ = 2 mm, histogram matching, winsorize [0.05, 0.98]. |
| `antsRegistration` | ANTs | sdcflows/workflows/apply/registration.py:129-131 (via fit.py:415) | Fieldmap magnitude → boldref (rigid) | 1 per run with SDC | Med | 2-stage Rigid Mattes 32; shrink 2/1; σ 8/2 mm; 50/20 iterations; sampling 0.25/0.5 random; dilated masks. |
| `antsRegistration` (SyN) etc. | ANTs | sdcflows/workflows/fit/syn.py:235, :485 (via workflows/base.py:783-815) | Fieldmap-less SDC | per SyN estimator | High | Other agent (sdcflows). |
| `topup`, `prelude` | FSL | sdcflows/workflows/fit/pepolar.py:165; fieldmap.py:345 | PEPOLAR / phasediff fieldmap estimation | per estimator | High / Med | Other agent. |
| `N4BiasFieldCorrection` | ANTs | niworkflows/func/util.py:487-493 | Bias-correct the coregistration reference | 1 per run | Low | `-d 3`, b-spline distance 200, shrink 2, rescale intensities, weight = premask, 1 thread. |
| `bet` | FSL | niworkflows/func/util.py:496, :602 | BOLD ref brain mask (first pass) | 1-2 per run | Low | `-f 0.2 -m`. |
| `3dUnifize` | AFNI | niworkflows/func/util.py:501-510 | T2-contrast intensity unifying | 1 per run | Low | `-T2 -clfrac 0.2 -rbt 18.3 65.0 90.0`. |
| `3dAutomask` | AFNI | niworkflows/func/util.py:515-517, :603-605 | Refined mask | 1-2 per run | Low | `-dilate 1`. |
| `fslmaths` | FSL | niworkflows/func/util.py:521, :606 (mask `-mul`); workflows/bold/resampling.py:314-506 (goodvoxels) | Mask algebra / CoV maps | 1-2 per run (+14 with goodvoxels) | Low (Tstd/Tmean over 4D: Med) | Trivially numpy. |
| `fslstats` | FSL | workflows/bold/resampling.py:338-348, :415-425 | Mean/SD inside ribbon | 4 per run (goodvoxels) | Low | `-M` (non-zero mean), `-S` (non-zero SD). |
| `mri_coreg` | FreeSurfer | workflows/bold/registration.py:333-340 (FS path); :600-605 (FSL path) | Initial affine BOLD→T1w | 1 per coregistration group | Med | Default NMI cost with Powell; `--dof`, `--sep 4`, `--ftol 1e-4`, `--linmintol 0.01`. FS path uses `--s/--sd` (aparc+aseg reference mask) or the T2 reference with `--no-ref-mask`. n_procs = omp_nthreads. |
| `bbregister` (→ `mri_segreg`, …) | FreeSurfer (tcsh script) | workflows/bold/registration.py:342-352 | Boundary-based registration to the white surface | 1 per group (recon-all on) | Med-High (mem_gb = 12) | `--t2` contrast, DOF 6/9/12, init from mri_coreg LTA or header, writes `--lta` and `--initcost`. Cost value is read from the min-cost file (:430-442, :814-818). |
| `flirt` (BBR) | FSL | workflows/bold/registration.py:648-659 | BBR without FreeSurfer | 1 per group (`--fs-no-reconall`) | Med | `-cost bbr -dof N -wmseg WM -init <mri_coreg> -schedule bbr.sch -basescale 1`; reference = masked T1w; `--sloppy` uses a 4 mm reference. nipype also writes a resampled output image. |
| `lta_convert` | FreeSurfer | workflows/bold/registration.py:646, :704 (niworkflows PatchedLTAConvert, freesurfer.py:264) | Transform format conversion | 1-3 per group (FSL path) | Trivial | Replaceable by nitransforms; test fixtures already compare against it (interfaces/tests/test_nitransforms.py). |
| `mri_robust_template` | FreeSurfer | workflows/bold/template.py:93-107 | Session/subject BOLD template | 1 per group (session/subject level) | Med | `--satit --iscale --subsample 200 --inittp 1`, plus `[--fixtp --noit]` when fewer than 3 runs. Robust (Tukey) rigid 6-DOF. Writes per-run LTAs. |
| `mri_vol2surf` | FreeSurfer | workflows/bold/resampling.py:174-187 | Sample T1w-space BOLD onto fsnative/fsaverageN | per run × FS space × 2 hemispheres | Med | Trilinear; average of 6 samples at projfrac 0:0.2:1 between white and pial; target-subject resampling for fsaverage. |
| `mri_convert` | FreeSurfer | niworkflows/utils/misc.py:358-378 (called at cli/workflow.py:119) | FS license probe | 1 per invocation | Trivial | Hard gate (return code 126). Must be patched out or stubbed. |
| `wb_command -volume-to-surface-mapping` | Workbench | workflows/bold/resampling.py:613-618, :969-974 | Ribbon-constrained volume→surface | per run × 2 hemispheres (×2 if CIFTI and std surfaces) | **High** | Ribbon-constrained polyhedron weights between white and pial around midthickness; default voxel-subdiv 3; optional `-volume-roi` (goodvoxels); all timepoints. |
| `wb_command -metric-dilate` | Workbench | workflows/bold/resampling.py:639-644, :975-980 | Fill holes | per run × hemisphere (×1-2) | Low | distance 10 mm, `-nearest`. |
| `wb_command -metric-mask` | Workbench | workflows/bold/resampling.py:981, :989 | Cortex mask (native), atlasroi (fsLR) | 2 per run × hemisphere | Trivial | Multiply by ROI. |
| `wb_command -metric-resample` | Workbench | workflows/bold/resampling.py:781-786, :982-987 | Native sphere → fsLR/onavg mesh | per run × hemisphere × target space | Med | ADAP_BARY_AREA with `-area-surfs` (subject midthickness / resampled midthickness); `-current-roi cortex` in the CIFTI path. |
| `wb_command -surface-resample` | Workbench | smriprep/src/smriprep/interfaces/workbench.py:688 (via bold/base.py:675 and workflows/base.py:546) | Resample midthickness etc. to the template mesh | per run × std surface space (also per subject for CIFTI) | Low | smriprep scope. |
| `t2smap` | tedana (Python CLI) | interfaces/multiecho.py:84-119 (via workflows/bold/t2s.py:121-125) | T2*/S0 fit and optimal combination | 1 per multi-echo run | Med (loglin) / **High** (curvefit, voxelwise nonlinear least squares) | TEs passed in ms; mask = run_mask dilated by 2; `--exclude 0:N`. Outputs `T2starmap.nii.gz` and `desc-optcom_bold.nii.gz`. |
| `antsApplyTransforms` | ANTs | outputs.py:272-282, :291-299, :342-352, :873-882, :923-930; confounds.py:256-259, :287-292, :732-749; t2s.py:193-196; resampling.py:308-312; coreg.py:459-464; niworkflows/func/util.py:360-375 | 3D only: masks, boldrefs, T2*, parcellation, TPMs, reportlets | ≈ 10 + 2-3 × (number of output volume spaces) per run | Low each | Interpolations used: LanczosWindowedSinc, MultiLabel, Gaussian, NearestNeighbor, Linear; `-u int`; `float=True`. Transform lists are in ANTs order with invert flags. nitransforms already handles Linear/NN; you would need Lanczos, MultiLabel and Gaussian. |
| `bids-validator` | Node.js | utils/bids.py:452 | Input validation | 1 per invocation | Low | Optional (`--skip-bids-validation`; FileNotFoundError tolerated). |
| `pandoc` | pandoc | cli/workflow.py:188-221 | CITATION.html / CITATION.tex | 2 per invocation | Trivial | Optional (`--md-only-boilerplate`; errors tolerated). |
| `svgo`, `cwebp` | Node / libwebp | nireports/reportlets/utils.py:100-150; niworkflows/viz/utils.py:53-67 | Reportlet compression | per reportlet if installed | Trivial | Optional; `shell=True` subprocess. |
| Version probes | AFNI/ANTs/WB/FSL/FS | nipype afni/base.py:33 (`afni --version`), ants/base.py:31, workbench/base.py:25 (`wb_command -version`); fsl/base.py:64 (reads `$FSLDIR/etc/fslversion`); freesurfer/base.py:49 (reads `build-stamp.txt`) | Boilerplate version strings at graph-build time (e.g. hmc.py:84, stc.py:109, registration.py:550) | at build | Trivial | Replacement interfaces must provide `.version`. |

---

## 4. CLI options that toggle tools (cli/parser.py)

| Option (parser line) | Default | Effect / binaries added (+) or removed (−) | Code |
|---|---|---|---|
| `--fs-no-reconall` (:688) | recon-all on | − recon-all (smriprep), − `bbregister`, − `mri_vol2surf` (no FS spaces). + `flirt` BBR, + `lta_convert`. **`mri_coreg` and the `mri_convert` license probe remain.** | workflows/base.py:246, :910-924; registration.py:163-177; bold/base.py:498 |
| `--force bbr` / `--force no-bbr` (:385-397) | auto (`use_bbr=None`) | `bbr`: BBR only, no compare. `no-bbr`: mri_coreg only (− bbregister/flirt/lta_convert). Default: both plus compare_xforms. Setting both raises an error (:848-853). | workflows/base.py:870-876; registration.py:408-448, :638-696 |
| `--force-bbr`, `--force-no-bbr`, `--force-syn` (:446-455, :657-662) | — | Deprecated; **no effect** (prints only). | cli/parser.py:50-70 |
| `--bold2anat-init {auto,t1w,t2w,header}` (:419-426) | auto (T2w if available) | `t2w`: `mri_coreg --ref T2.mgz --no-ref-mask` (FS path only; FSL path warns and uses T1w). `header`: `bbregister --init-header`, − mri_coreg (FSL path: NotImplementedError). | workflows/base.py:827-834; registration.py:296-340, :587-592 |
| `--bold2anat-dof {6,9,12}` (:427-435) | 6 | Passed as `--dof` (mri_coreg), bbregister DOF, `flirt -dof`. | registration.py:334, :344, :601, :649 |
| `--bold-coreg-level {run,session,subject}` (:436-445) | run | session/subject: + `mri_robust_template`, one BBR per group; validity check on SDC/PE. Subject level is incompatible with sessionwise anatomy (:825-831). | workflows/base.py:842-868; coreg.py:151 |
| `--ignore slicetiming` (:375-384) | — | − `3dTshift` (also skipped when there is no `SliceTiming`). | fit.py:685; workflows/base.py:1115-1117 |
| `--slice-time-ref` (:456-466) | 0.5 | Changes `-tzero` only. | stc.py:104-107 |
| `--dummy-scans N` (:467-474) | auto | Overrides skip_vols for `3dTshift -ignore`, CompCor `ignore_initial_volumes`, `t2smap --exclude`, carpet-plot `drop_trs`. **Does not** change the reference-averaging `t_mask`. | reference.py:210-230; fit.py:328 |
| `--ignore sbref` | — | − sbref `3dvolreg` path; the HMC boldref becomes the SDC/coregistration target. | fit.py:213-215, :460-467 |
| `--ignore fieldmaps` | — | − all SDC (sdcflows estimators, coeff2epi `antsRegistration`, fieldmap reconstruction). | workflows/base.py:640, :1218 |
| `--use-syn-sdc [warn\|error]` (:647-655) | off | + fieldmap-less SyN (sdcflows: antsRegistration SyN, N4, antsApplyTransforms) for runs without a fieldmap. | workflows/base.py:641, :1229-1247 |
| `--force syn-sdc` | — | + SyN even when fieldmaps exist (forced fieldmap-less). | workflows/base.py:642 |
| `--force fmap-jacobian` / `--ignore fmap-jacobian` | Jacobian only for PEPOLAR | Python-only change (ResampleSeries `jacobian`). Force+ignore raises an error (:854-859). | workflows/base.py:878-887 |
| `--fallback-total-readout-time` (:475-483) | None | Metadata only (DistortionParameters / sdcflows). | workflows/base.py:699-718 |
| `--me-t2s-fit-method {curvefit,loglin}` (:492-505) | curvefit | `t2smap --fittype`. | t2s.py:122 |
| `--me-output-echos` (:517-523) | off | Writes the per-echo native corrected series (no new binary). | bold/base.py:272-296 |
| `--echo-idx` (:242-247) | — | Selects one echo, so the run becomes single-echo (− `t2smap`). | workflows/base.py:236-237 |
| `--output-spaces` (:398-413) | `MNI152NLin2009cAsym:res-native` (:871-875) | std 3D: + ResampleSeries plus 2-3 `antsApplyTransforms` per space. `anat`/`T1w`: writes `bold_anat_wf`. `func`/`run`/`boldref`: writes native. `fsnative`/`fsaverage*`: + `mri_vol2surf` (needs recon-all). `fsLR`/`onavg` with `den`: + Workbench (only with `--cifti-output`). MNI2009c is always added internally. | bold/base.py:239-242, :270, :379, :407, :498, :630; config.py:850-878 |
| `--cifti-output [91k\|170k]` (:555-565) | off | + MNI152NLin6Asym res-2/1 ResampleSeries; + `wb_command` volume-to-surface / metric-dilate / metric-mask ×2 / metric-resample per hemisphere; GenerateCifti (Python); smriprep CIFTI morphometrics (+ `-surface-resample`, MSM if `--msm`). | bold/base.py:539-729; workflows/base.py:514-624 |
| `--project-goodvoxels` (:541-547) | off | + `antsApplyTransforms`, 14 `fslmaths`, 4 `fslstats`; only affects CIFTI and Workbench std surfaces. | bold/base.py:466-496 |
| `--medial-surface-nan` (:533-540) | off | + MedialNaNs (Python) for FS spaces. | resampling.py:242-251 |
| `--msm/--no-msm` (:566-572) | on | Anatomical `msm` binary (smriprep); BOLD uses `sphere_reg_msm` vs `sphere_reg_fsLR`. | workflows/base.py:585, :1140 |
| `--level` (:350-358) | full | See section 1. | workflows/base.py:427, :1111; bold/base.py:271, :334 |
| `--sloppy` (:341-346) | off | coeff2epi `_testing.json`; FLIRT-BBR 4 mm downsampling (Python); smriprep effects. | fit.py:418; registration.py:672 |
| `--return-all-components`, `--fd-spike-threshold`, `--dvars-spike-threshold` (:575-601) | off, 0.5, 1.5 | Python only. | confounds.py:341-346, :455-458 |
| `--fmap-bspline`, `--fmap-no-demean` (:632-643) | — | **Never consumed.** | config.py:585-588 |
| `--skull-strip-*`, `--random-seed`, `--skull-strip-fixed-seed` | — | Anatomical (antsBrainExtraction); `ANTS_RANDOM_SEED` env (config.py:715-719). | — |
| `--skip-bids-validation` (:212-218) | off | − `bids-validator`. | parser.py:948-959 |
| `--md-only-boilerplate` (:549-554) | off | − `pandoc`. | cli/workflow.py:177 |
| `--fs-license-file` (:666-672) | `$FS_LICENSE` or `$FREESURFER_HOME/license.txt` | Sets `FS_LICENSE` (config.py:169-174, :480-481). | — |
| `--omp-nthreads`, `--nprocs`, `--mem`, `--use-plugin` (:302-340) | auto | `num_threads` for ResampleSeries/wb_command/mri_coreg/ANTs; MultiProc pool. | config.py:316-382 |
| `--use-aroma` | **removed** in 23.1.0 | — | CHANGES.rst:608 |

---

## 5. Testing assets

### Unit and construction tests (pytest; `pyproject.toml [tool.pytest.ini_options]`)

- **Options:** `--doctest-modules`, `PYTHONHASHSEED=0`, strict xfail. Doctests get a copy of `interfaces/tests/data` in a temp directory (interfaces/conftest.py:29-43).
- **Mock config:** `workflows/tests/__init__.py` `mock_config` loads `data/tests/config.toml`. Workflow docstrings contain `.. workflow::` directives that build graphs for the docs.

| Test file | # tests | What it checks |
|---|---|---|
| `workflows/tests/test_base.py` | 19 (heavily parametrized: level × anat_only × spaces; coreg level × layout × anatomical reference) | Graph construction only, with zero-filled NIfTIs from niworkflows `generate_bids_skeleton` (`workflows/tests/layouts.py`); `_create_flat_graph()`; estimator mapping. No execution. |
| `workflows/bold/tests/test_base.py` | 1 (task × fieldmap × freesurfer × level × init) | `init_bold_apply_wf` builds. |
| `workflows/bold/tests/test_fit.py` | 4 | sbref selection; precomputed derivatives include/omit (uses `utils/testing.deriv_skeleton` + `data/tests/derivatives.yml`); `init_bold_native_wf` with/without STC. |
| `interfaces/tests/test_confounds.py` | 5 | **Numerical against tool output**: FSLRMSDeviation and FSLMotionParams vs a real mcflirt-derived `desc-motion_timeseries.tsv` (atol 1e-4 mm / 1e-6 rad; RMSD atol 1e-4); FD equality; RenameACompCor / FilterDropped against TSV fixtures. |
| `interfaces/tests/test_nitransforms.py` | 1 | **Numerical**: ConvertAffine vs real `lta_convert` and `c3d_affine_tool` outputs of an `mri_coreg.lta` (atol 1e-4). |
| `interfaces/tests/test_maths.py`, `test_reports.py`, `test_bids.py` | 1, 1, 3 | Clip; PE-direction to world; BIDSURI/BIDSSourceFile/datasink paths. |
| `utils/tests/test_derivative_cache.py`, `test_bids.py`, `test_testing.py` | 13, 1, 6 | Derivative discovery and aggregation; template validity; skeleton generator. |
| `tests/test_fsl6.py` | 1 | Runs real `bet` with very long paths (skipped without FSL). |
| `tests/test_config.py` | 3 | Config reset, spaces, PRNG/ANTs seed. |
| `cli/tests/test_parser.py`, `test_version.py` | 12, 5 | Argument parsing, mem, slice-time ref, use-syn-sdc, derivatives, config reuse; version checks. |
| `reports/tests/*` | 2 | Report assembly from `data/tests/work/reportlets` fixtures; error handling. |

**Fixtures:**
- `data/tests/ds000005/` contains 83-126-byte **header stubs** (no real image data).
- The only real image/motion data:
  - `interfaces/tests/data/sub-01_task-mixedgamblestask_run-01_desc-hmc_boldref.nii.gz` (234 KB)
  - the matching hmc `xfm.txt` and `desc-motion_timeseries.tsv`
  - the `mri_coreg*.lta` / `.txt` / `.mat` set
  - CompCor TSVs.

### CI

- **GitHub Actions** (`.github/workflows/test.yml`): tox on **ubuntu-latest only**, Python 3.12/3.13/3.14 latest plus 3.12 min (tox.ini). It runs `scripts/fetch_templates.py`, then `pytest -n auto`. No external binaries are installed, so tool-dependent tests are skipped. **No macOS or Windows CI.**
- **CircleCI** (`.circleci/config.yml`):
  - Builds Docker images (production and test targets).
  - `test_pytest` (:389-450) runs `pytest --pyargs fmriprep -svx --doctest-modules` inside the test image (all binaries present).
  - Integration datasets are downsampled tarballs from OSF project `fvuh8` (:284-312), plus cached FreeSurfer/sMRIPrep derivatives for "fast-track" runs (:315-356).

| Job | Dataset | Key options exercised |
|---|---|---|
| ds005 (:452-617) | ds000005 sub-01, single-echo, 3 runs | anat-only (`--sloppy`, MNI2009c + MNI6); full with FreeSurfer (`--output-spaces fsaverage5 fsnative`, LegacyMultiProc plugin `.circleci/legacy.yml`); partial re-run with `--use-syn-sdc --fallback-total-readout-time 0.03125 --cifti-output --project-goodvoxels`, spaces MNI2009c fsaverage5 fsnative MNI6 anat; a run without a T1w using `--level minimal`; a run without PE metadata expected to fail. |
| ds054 (:619-756) | ds000054 sub-100185, with fieldmap `auto00000` (type not verified; likely phasediff) | `--fs-no-reconall` (FLIRT-BBR path), spaces `MNI152NLin2009cAsym:res-2 anat func`, `--debug compcor`; then `--bold-coreg-level session` and `subject` at `--level minimal`, with output-location checks. |
| ds210 (:758-865) | ds000210 sub-02, **multi-echo** | `--me-output-echos --fs-no-reconall --ignore slicetiming --use-syn-sdc --fallback-total-readout-time 0.0625 --dummy-scans 1`. |

- **How outputs are compared:**
  - `_check_outputs` (:97-123) runs `find` over the outputs (pruning figures/log/sourcedata), sorts the list and `diff`s it against `.circleci/<ds>[_partial]{_fasttrack,}_outputs.txt`.
  - That is a **file-list comparison only; there is no numerical comparison of images or TSVs.**
  - Nipype config uses `hash_method = content` (:373-387).
- **Other references:**
  - `docs/outputs.rst` documents output names.
  - `docs/benchmarks.rst` has runtime, scratch and output sizes for two datasets (fit vs fit+transform).
  - `scripts/fetch_templates.py` lists the TemplateFlow files required.

**Harness implications.** You need to build your own numerical comparisons. Good targets:
- per-stage transforms: HMC `xfm.txt`, coreg `xfm.txt`, fmap `xfm.txt`
- `confounds_timeseries.tsv`
- `desc-preproc_bold` in boldref/T1w/MNI
- GIFTI/CIFTI files.

The existing atol values (1e-4 mm, 1e-6 rad) are a usable precedent for transform-derived quantities. The graph-construction tests, combined with `check_deps` (utils/misc.py:55-61, which enumerates `node.interface._cmd`), let you list programmatically every external command a given configuration would execute.

---

## 6. POSIX-specific / portability issues in fmriprep code

| Issue | Location | Impact on Windows / macOS |
|---|---|---|
| `set_start_method('forkserver')` with only `RuntimeError` caught | config.py:92, :103-106 | **Windows: ValueError at `import fmriprep.config`**, so the whole package fails to import. macOS is fine. |
| `os.path.join(os.getenv('HOME'), '.cache', 'templateflow')` evaluated eagerly as the getenv default | config.py:176-178 | **Windows without HOME: TypeError on import**, even if TEMPLATEFLOW_HOME is set. Use `Path.home()`. |
| `/proc/1/cgroup` (container detection) | config.py:161-167 | Guarded by the `IS_DOCKER_8395080871` env var; harmless. |
| `/proc/sys/vm/overcommit_*` | config.py:188-205 | Guarded by `.exists()`; Linux-only diagnostics. |
| `/proc/sys/crypto/fips_enabled` | utils/misc.py:64-73 | Guarded; Linux-only. |
| FreeSurfer license gate via `mri_convert` subprocess | cli/workflow.py:119-138 → niworkflows/utils/misc.py:358-378 | Hard exit 126 without a working FreeSurfer (no native Windows build). Must be removed or conditional. |
| `bbregister` is a tcsh script; Dockerfile.base installs `tcsh`, `perl`, `bc` for FreeSurfer | registration.py:342; Dockerfile.base:74-87 | Not runnable on Windows. |
| `check_deps` uses nipype `which(_cmd.split()[0])` | utils/misc.py:55-61; cli/workflow.py:141-146 | Exit 127 if any CommandLine interface binary is missing; replacement interfaces should not define `_cmd`, or should be found on PATH (PATHEXT `.exe`). |
| `NamedTemporaryFile` kept open while a subprocess reads it | utils/bids.py:448-452 | Windows sharing violation, so bids-validator cannot read its config. Use `delete=False` / `delete_on_close=False`. |
| Subprocesses for external tools: `bids-validator` (Node), `pandoc` | utils/bids.py:452; cli/workflow.py:188-221 | Optional; errors caught. |
| `svgo`/`cwebp` via `subprocess.run(..., shell=True)` with a POSIX-style command string | niworkflows/viz/utils.py:53-67; nireports/reportlets/utils.py:100-150 | Optional (only if found by `which`). |
| `multiprocessing.Process` + `Manager` to build the workflow and boilerplate, passing the workflow object to the child | cli/run.py:84-93, :117-122 | Under spawn (Windows) or forkserver the Workflow is pickled; works but is slow. Needs a `__main__` guard (the entry point is a function, so OK). |
| nipype MultiProc, `maxtasksperchild=1`; LegacyMultiProc in CI | config.py:322-327; .circleci/legacy.yml | Process-pool semantics differ under spawn (nipype agent's scope). |
| Env-var coupling: `FREESURFER_HOME`, `FS_LICENSE`, `SUBJECTS_DIR`, `FSLDIR`, `FSLOUTPUTTYPE`, `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `ANTS_RANDOM_SEED`, `TEMPLATEFLOW_HOME`, `IS_DOCKER_8395080871` | workflows/base.py:84; config.py:169-177, :480-481, :718; registration.py:653-659; interfaces/workbench.py:28-46; Dockerfile:90-91, :112-115; Dockerfile.base:106-135 | Must be provided or replaced. `bbr.sch` has a packaged fallback (`data/flirtsch/bbr.sch`). |
| `wb_command` `_cmd` strings contain spaces / a trailing space (`'wb_command -metric-dilate '`); OMP thread control via env | interfaces/workbench.py:161, :279, :586, :672 | Fine on POSIX; the PATH lookup uses the first token. |
| SQLite pybids DB in the work dir | config.py:502-516 | Locking problems on network or synced filesystems (all OSes). |
| Deeply nested nipype work-dir paths (iterables, MapNodes) | engine behaviour; `tests/test_fsl6.py` tests long paths only for FSL | Windows MAX_PATH (260) risk. |
| Git symlink `NOTICE -> fmriprep/data/NOTICE` at repo root | repo root | Windows checkouts without `core.symlinks`. |
| Pixi environment declares `platforms = ["linux-64"]` only; FSL is pulled from the FSL conda channel | pyproject.toml `[tool.pixi.workspace]` | No macOS or Windows lock file. |
| `scipy.ndimage.filters` (deprecated module) | registration.py:777 | Future SciPy removal (`--sloppy` path only). |

Things I looked for and did not find in the fmriprep package: `os.symlink`, `signal`, `ulimit`/`resource`/`setrlimit`, `fcntl`/`flock`, hard-coded `/tmp`, `os.fork`, `getuid`. Temporary files use `tempfile` / nipype working directories. CircleCI uses `/tmp`, but only in CI.

---

## 7. Other behaviours relevant for re-implementation

- **Interpolation count:** single-echo outputs are interpolated once (STC is temporal, then one spatial resample). Multi-echo outputs are interpolated twice. FreeSurfer-space surface outputs go T1w-grid resample then mri_vol2surf trilinear (2 steps). CIFTI/Workbench outputs go T1w-grid resample then ribbon mapping then metric-resample.
- **Wasted compute you can avoid:**
  - mcflirt's resampled 4D output is unused.
  - `bold_anat_wf` (4D T1w-space resample) always runs at full level even when nothing consumes it, because nipype executes unconnected nodes.
  - smriprep surface resampling is repeated per BOLD run in the Workbench std-surface path (bold/base.py:675).
- **Unused code:** `init_bold_preproc_report_wf` (outputs.py:975), `CreateROI` (interfaces/gifti.py), `MetricFillHoles` and `MetricRemoveIslands` (interfaces/workbench.py:723, :790).
- **Coregistration reference cost:** building the reference includes a full affine ANTs registration to an MNI BOLD template per run (premask). That is a notable cost that is easy to overlook.