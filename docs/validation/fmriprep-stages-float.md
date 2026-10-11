# Stage by stage: larmorx against fMRIPrep 25.2.6

larmorx's tools run on fMRIPrep's **own inputs and parameters** for each step it already implements, read from nipype's working directory (`command.txt`, `_report/report.rst`), and their outputs are compared with what fMRIPrep produced at that step. So every stage is judged on its own, with no error carried over from earlier stages. **The headline numbers are against fMRIPrep as it ran**; where they differ, the step is run again in fMRIPrep's own container and with the repository's oracle builds to find out why.

- **Oracle:** fMRIPrep 25.2.6, image `nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e` (Apptainer), run `ds000005-sub-01-noreconall`: `/data /out participant --participant-label 01 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 8 --omp-nthreads 8 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall`. Input: ds000005 sub-01 (one T1w, three BOLD runs of 240 volumes, no field maps, no slice timing).
- **Tools inside fMRIPrep's image:** ANTs 2.6.2.dev1-gff61a3d (conda-forge `ants` 2.6.2) on ITK 5.4.4; FSL `mcflirt` 2111.0 (`newimage` 2501.4); AFNI 25.2.09; nitransforms 25.1.0, SciPy 1.15.2, numpy 2.2.6 on OpenBLAS 0.3.30; glibc 2.35.
- **larmorx:** 0.0.1 (9110347274ba), on Linux x86_64, Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs.
- **References for the causes:** fMRIPrep's own tools re-run in its image, at its thread count and at one thread, as built and with CORE-MATH's correctly rounded functions preloaded (`scripts/build_crmath_preload.sh`; the image has this machine's glibc 2.35); ANTs 2.6.5 on ITK 5.4.5 built by `scripts/build_ants_oracle.sh` (one thread, as built and with CORE-MATH preloaded); FSL 6.0.7's `mcflirt` on this machine.
- **Generated:** 2026-10-11, with `python -m larmorx_validation pipeline-stages --oracle <workspace>/oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall --out docs/validation --stage A06 A11 F08 F09 F12 --reuse --name fmriprep-stages-float`. Times are **on a shared machine** (load average 5.1 at the start; other jobs: 0), so they are indicative only; fMRIPrep's own times come from its run (8 jobs at once on a machine shared with a development agent, see `RUN_INFO.txt`), the re-runs of fMRIPrep's tools from this invocation, at fMRIPrep's thread counts.

## Results

**26 of 28 compared nodes are bit-identical to fMRIPrep's own output** (plus 0 head-motion corrections, which larmorx does with a clean-room algorithm and cannot match bit for bit). The other differences are explained by fMRIPrep's thread count, glibc's maths or the ANTs build (fMRIPrep's conda-forge 2.6.2 against the repository's build of 2.6.5; with those removed, larmorx is bit-identical).

| Stage | Step | Nodes | larmorx | Against fMRIPrep as it ran | Why they differ | PLAN.md §11.3 | fMRIPrep (its run) | fMRIPrep's tool re-run | larmorx |
|---|---|---|---|---|---|---|---|---|---|
| A06 | Brain extraction: template priors to the T1w (Gaussian, inverse warp) | 2 | `lx.ants.apply_transforms` | 0 of 2 outputs bit-identical; worst: 1,588 of 5,898,240 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 | glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (max rel ≤ 0.0001) | 20.7 s (1t) | 18.9 s (1t) | 21.8 s (1t) |
| A11 | Anatomical outputs to MNI (`.h5` composite; Lanczos, MultiLabel, Gaussian) | 8 | `lx.ants.apply_transforms` | **bit-identical**: 8 of 8 outputs | – | pass (bit-identical) | 54.4 s (1t) | 50.3 s (1t) | 37.4 s (1t) |
| F08 | boldref and BOLD mask to T1w (Lanczos, MultiLabel) | 6 | `lx.ants.apply_transforms` | **bit-identical**: 6 of 6 outputs | – | pass (bit-identical) | 4.03 s (1t) | 6.11 s (1t) | 2.21 s (1t) |
| F09 | boldref and BOLD mask to MNI (Lanczos, MultiLabel, `.h5`) | 6 | `lx.ants.apply_transforms` | **bit-identical**: 6 of 6 outputs | – | pass (bit-identical) | 35.7 s (1t) | 35.5 s (1t) | 25.1 s (1t) |
| F12 | Report: T1w and WM mask to the boldref (Lanczos, NearestNeighbor) | 6 | `lx.ants.apply_transforms` | **bit-identical**: 6 of 6 outputs | – | pass (bit-identical) | 2.89 s (1t) | 4.30 s (1t) | 1.02 s (1t) |

Times are sums over the stage's nodes, with the thread counts used. fMRIPrep's run times include nipype's own overhead for each node; the re-runs time the program alone (inside the image, without Apptainer's start-up), and larmorx's the call (the command line in-process for the ANTs programs and head-motion correction; computing and writing the output for the resampling).

**A selection:** only the stages or nodes given with `--stage` (`A06`, `A11`, `F08`, `F09`, `F12`) were compared in this run.

## What larmorx cannot run yet

Every other node of the run that runs a neuroimaging program, or a Python step larmorx will need, by the tool it needs. The time is fMRIPrep's (its run, 8 jobs at once). These are the next tools to port (CLAUDE.md, "Next").

| Tool | fMRIPrep nodes | fMRIPrep time | What fMRIPrep uses it for | larmorx milestone |
|---|---|---|---|---|
| antsRegistration | 5 | 2639 s | rigid/affine (BOLD premask) and SyN (brain extraction, MNI normalisation) | A4 (linear), A5 (SyN) |
| antsAI | 4 | 117 s | affine initialisation by rotation search | A4 |
| Atropos | 1 | 12.0 s | K-means + MRF segmentation in brain extraction | A6 |
| 3dvolreg | 3 | 32.0 s | aligns the reference volumes before the median (niworkflows' RobustAverage, inside the HMC-reference node) | AFNI |
| 3dUnifize | 3 | 169 s | T2-contrast intensity unifying of the boldref | AFNI |
| 3dAutomask | 3 | 0.47 s | BOLD brain mask, second pass | AFNI |
| BET | 3 | 4.22 s | BOLD brain mask, first pass | FSL (clean-room + `larmorx-nc`) |
| FAST | 1 | 40.4 s | GM/WM/CSF segmentation and probability maps | FSL |
| FLIRT-BBR | 3 | 139 s | boundary-based BOLD-to-T1w registration | FSL |
| fslmaths | 3 | 0.45 s | mask product of the two BOLD masks | FSL (trivial) |
| mri_coreg | 3 | 48.1 s | BOLD-to-T1w initialisation | FreeSurfer |
| lta_convert | 9 | 1.06 s | transform format conversion (LTA <-> FSL) | FreeSurfer (trivial; nitransforms has it) |
| confounds | 33 | 53.2 s | DVARS, CompCor (a/t/crown), global signals, expansion, spikes (fMRIPrep's Python); FD, RMSD and motion parameters are computed here from larmorx's HMC | larmorprepx (numpy) |

## A06: Brain extraction: template priors to the T1w (Gaussian, inverse warp)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `map_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans.nii.gz | 1,588 of 5,898,240 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 | pass: max rel 6.7e-16 | same | 10.3 s (1t) | 10.5 s | 13.9 s (1t) |
| `map_wmmask` | summap_trans.nii.gz | 2,104 of 5,898,240 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 | pass: max rel 5.6e-16 | same | 10.4 s (1t) | 8.42 s | 7.90 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `map_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans.nii.gz | same | 1,588 of 5,898,240 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 | same | same | explained | 1,588 of 5,898,240 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 |
| `map_wmmask` | summap_trans.nii.gz | same | 2,104 of 5,898,240 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 | same | same | explained | 2,104 of 5,898,240 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 |

## A11: Anatomical outputs to MNI (`.h5` composite; Lanczos, MultiLabel, Gaussian)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| MNI152NLin2009cAsym.res2 `mask_std` | sub-01_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 8.97 s (1t) | 7.13 s | 6.24 s (1t) |
| MNI152NLin2009cAsym.res2 `t1w_std` | sub-01_desc-preproc_T1w_trans.nii.gz | bit-identical | pass: bit-identical | same | 8.27 s (1t) | 6.03 s | 1.96 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_dseg` | sub-01_dseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 11.7 s (1t) | 11.8 s | 9.86 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_mask` | sub-01_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 10.5 s (1t) | 10.2 s | 8.35 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_t1w` | sub-01_desc-preproc_T1w_masked_trans.nii.gz | bit-identical | pass: bit-identical | same | 7.31 s (1t) | 6.14 s | 2.96 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms0` | sub-01_label-GM_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 2.72 s (1t) | 3.47 s | 2.89 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms1` | sub-01_label-WM_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 2.55 s (1t) | 3.34 s | 2.74 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms2` | sub-01_label-CSF_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 2.34 s (1t) | 2.17 s | 2.38 s (1t) |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| MNI152NLin2009cAsym.res2 `anat2std_dseg` | sub-01_dseg_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_dseg.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_mask` | sub-01_desc-brain_mask_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_t1w` | sub-01_desc-preproc_T1w_masked_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_tpms0` | sub-01_label-GM_probseg_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_label-GM_probseg.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_tpms1` | sub-01_label-WM_probseg_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_label-WM_probseg.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_tpms2` | sub-01_label-CSF_probseg_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_label-CSF_probseg.nii.gz | bit-identical |

## F08: boldref and BOLD mask to T1w (Lanczos, MultiLabel)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-01 `resample_mask` | sub-01_task-mixedgamblestask_run-01_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.75 s (1t) | 1.37 s | 0.50 s (1t) |
| run-01 `resample_ref` | sub-01_task-mixedgamblestask_run-01_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.55 s (1t) | 1.00 s | 0.20 s (1t) |
| run-02 `resample_mask` | sub-01_task-mixedgamblestask_run-02_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.71 s (1t) | 1.05 s | 0.53 s (1t) |
| run-02 `resample_ref` | sub-01_task-mixedgamblestask_run-02_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.52 s (1t) | 0.89 s | 0.20 s (1t) |
| run-03 `resample_mask` | sub-01_task-mixedgamblestask_run-03_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.72 s (1t) | 0.98 s | 0.57 s (1t) |
| run-03 `resample_ref` | sub-01_task-mixedgamblestask_run-03_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.77 s (1t) | 0.82 s | 0.21 s (1t) |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| run-01 `resample_mask` | sub-01_task-mixedgamblestask_run-01_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-01_space-T1w_desc-brain_mask.nii.gz | bit-identical |
| run-01 `resample_ref` | sub-01_task-mixedgamblestask_run-01_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-01_space-T1w_boldref.nii.gz | bit-identical |
| run-02 `resample_mask` | sub-01_task-mixedgamblestask_run-02_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-02_space-T1w_desc-brain_mask.nii.gz | bit-identical |
| run-02 `resample_ref` | sub-01_task-mixedgamblestask_run-02_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-02_space-T1w_boldref.nii.gz | bit-identical |
| run-03 `resample_mask` | sub-01_task-mixedgamblestask_run-03_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-03_space-T1w_desc-brain_mask.nii.gz | bit-identical |
| run-03 `resample_ref` | sub-01_task-mixedgamblestask_run-03_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-03_space-T1w_boldref.nii.gz | bit-identical |

## F09: boldref and BOLD mask to MNI (Lanczos, MultiLabel, `.h5`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-01 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-01_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 6.49 s (1t) | 8.60 s | 6.79 s (1t) |
| run-01 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-01_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 5.03 s (1t) | 6.29 s | 2.66 s (1t) |
| run-02 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-02_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 7.21 s (1t) | 5.62 s | 6.71 s (1t) |
| run-02 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-02_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 5.79 s (1t) | 4.34 s | 1.84 s (1t) |
| run-03 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-03_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 6.20 s (1t) | 6.32 s | 5.33 s (1t) |
| run-03 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-03_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 5.02 s (1t) | 4.32 s | 1.76 s (1t) |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| run-01 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-01_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-01_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| run-01 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-01_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-01_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz | bit-identical |
| run-02 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-02_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-02_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| run-02 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-02_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-02_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz | bit-identical |
| run-03 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-03_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-03_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| run-03 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-03_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-03_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz | bit-identical |

## F12: Report: T1w and WM mask to the boldref (Lanczos, NearestNeighbor)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-01 `boldref_wm` | sub-01_dseg_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.29 s (1t) | 0.52 s | 0.04 s (1t) |
| run-01 `t1w_boldref` | sub-01_desc-preproc_T1w_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.82 s (1t) | 0.93 s | 0.32 s (1t) |
| run-02 `boldref_wm` | sub-01_dseg_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.24 s (1t) | 0.52 s | 0.04 s (1t) |
| run-02 `t1w_boldref` | sub-01_desc-preproc_T1w_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.68 s (1t) | 0.93 s | 0.32 s (1t) |
| run-03 `boldref_wm` | sub-01_dseg_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.25 s (1t) | 0.50 s | 0.03 s (1t) |
| run-03 `t1w_boldref` | sub-01_desc-preproc_T1w_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.63 s (1t) | 0.90 s | 0.28 s (1t) |

## Reproduce

```bash
scripts/run_fmriprep_oracle.sh ~/.cache/larmorx-testdata/data/openneuro/ds000005 01 ds000005-sub-01-noreconall --fs-no-reconall   # the oracle run (about an hour)
scripts/build_ants_oracle.sh && scripts/build_crmath_preload.sh   # the references
python -m larmorx_validation pipeline-stages --oracle <workspace>/oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall --out docs/validation --stage A06 A11 F08 F09 F12 --reuse --name fmriprep-stages-float
```

larmorx's outputs and the reference runs are kept in `<workspace>/oracles/compare/fmriprep-25.2.6/ds000005-sub-01-noreconall/<node path>/<run>/` (`larmorx`, `container-<n>t`, `container-1t-cr`, `ants-2.6.5-1t`, `ants-2.6.5-1t-cr`, `ants-2.6.5-1t-cr-double`, `fsl-6.0.7`, `confounds-*`), each with `run.json` (arguments, environment, exit status, time). `--reuse` reuses the one-thread reference runs of an earlier invocation; `--stage A01 F05` limits the comparison; `--render` writes this page again from the JSON record, without running anything.

