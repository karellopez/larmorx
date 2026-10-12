# Stage by stage: larmorx against fMRIPrep 25.2.6

larmorx's tools run on fMRIPrep's **own inputs and parameters** for each step it already implements, read from nipype's working directory (`command.txt`, `_report/report.rst`), and their outputs are compared with what fMRIPrep produced at that step. So every stage is judged on its own, with no error carried over from earlier stages. **The headline numbers are against fMRIPrep as it ran**; where they differ, the step is run again in fMRIPrep's own container and with the repository's oracle builds to find out why.

- **Oracle:** fMRIPrep 25.2.6, image `nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e` (Apptainer), run `ds005040-sub-002-noreconall`: `/data /out participant --participant-label 002 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 8 --omp-nthreads 8 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall`. Input: ds005040 sub-002: 1 T1w, 1 BOLD run of 63 volumes, slice timing (1 run); field maps: 2 `epi`, used by fMRIPrep (estimator `auto_00000`: PEPOLAR, `topup`).
- **Tools inside fMRIPrep's image:** ANTs 2.6.2.dev1-gff61a3d (conda-forge `ants` 2.6.2) on ITK 5.4.4; FSL `mcflirt` 2111.0 (`newimage` 2501.4); AFNI 25.2.09; nitransforms 25.1.0, SciPy 1.15.2, numpy 2.2.6 on OpenBLAS 0.3.30; glibc 2.35.
- **larmorx:** 0.0.1 (58230ae137bd), on Linux x86_64, Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs.
- **References for the causes:** fMRIPrep's own tools re-run in its image, at its thread count and at one thread, as built and with CORE-MATH's correctly rounded functions preloaded (`scripts/build_crmath_preload.sh`; the image has this machine's glibc 2.35); ANTs 2.6.5 on ITK 5.4.5 built by `scripts/build_ants_oracle.sh` with GCC 14.3 (one thread, as built and with CORE-MATH preloaded); FSL 6.0.7's `mcflirt` on this machine; AFNI 25.2.09 built by `scripts/build_afni_oracle.sh` (one thread, `HOME` an empty directory). 3dTshift runs in both larmorx implementations: the bit-exact replica (`larmorx-gpl afni 3dTshift`, GPL-3.0-or-later, the `crates-gpl` release build, which `larmorx afni 3dTshift` runs as a separate program) and the clean-room original (`LARMORX_IMPLEMENTATION=original`).
- **Generated:** 2026-10-12, with `python -m larmorx_validation pipeline-stages --oracle <workspace>/oracles/fmriprep-25.2.6/ds005040-sub-002-noreconall --out docs/validation --reuse --name fmriprep-stages-ds005040`. Times are **on a shared machine** (load average 1.8 at the start; other jobs: 0), so they are indicative only; fMRIPrep's own times come from its run (8 jobs at once on a machine shared with a development agent, see `RUN_INFO.txt`), the re-runs of fMRIPrep's tools from this invocation, at fMRIPrep's thread counts.

## Results

**56 of 65 compared nodes are bit-identical to fMRIPrep's own output** (plus 1 head-motion corrections, which larmorx does with a clean-room algorithm and cannot match bit for bit). The other differences are explained by fMRIPrep's thread count, glibc's maths or the ANTs build (fMRIPrep's conda-forge 2.6.2 against the repository's build of 2.6.5; with those removed, larmorx is bit-identical). Slice timing (3dTshift) counts by the bit-exact replica, which `larmorx afni 3dTshift` runs when it is installed: 1 of 1 bit-identical, headers included; the clean-room original is bit-identical in 0 of 1 and within [afni-tshift.md](afni-tshift.md)'s thresholds in 1 of 1.

**Field maps and PIPE-03.** Every field-map resampling above runs fMRIPrep's Jacobian (`upstream_defects="reproduce"`). larmorx's default corrects its axis (PIPE-03); against fMRIPrep's output it gives: S02 run-1 `unwarp_boldref`: unchanged; F05 run-1 `boldref_bold`: unchanged; F06 run-1 `resample`: 78.8 % of brain voxels change by more than 0.1 %, 2.8 % by more than 1 % (99th percentile 1.4 %, max 6.8 %); F07 run-1 MNI152NLin2009cAsym.res2 `resample`: 95.6 % of brain voxels change by more than 0.1 %, 59.5 % by more than 1 % (99th percentile 11.3 %, max 55.6 %).

| Stage | Step | Nodes | larmorx | Against fMRIPrep as it ran | Why they differ | PLAN.md §11.3 | fMRIPrep (its run) | fMRIPrep's tool re-run | larmorx |
|---|---|---|---|---|---|---|---|---|---|
| A01 | Denoise the T1w (`DenoiseImage -n Rician`) | 1 | `lx.ants.denoise_image` | 0 of 1 outputs bit-identical; worst: 4,395,882 of 15,842,304 differ; max \|diff\| 6.69e+03 (rel 0.076), NCC 1 - 1.1e-05 | fMRIPrep's thread count, glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 66.4 s (8t) | 49.1 s (8t) | 4.38 s (8t) |
| A03 | Brain extraction: truncate intensities (`TruncateImageIntensity`) | 1 | `lx.ants.image_math` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 3.51 s (1t) | 2.65 s (1t) | 1.20 s (1t) |
| A04 | Brain extraction: N4, first pass | 1 | `lx.ants.n4_bias_field_correction` | 0 of 1 outputs bit-identical; worst: 15,837,135 of 15,842,304 differ; max \|diff\| 0.383 (rel 5.6e-06), NCC 1 - 5.9e-13 | fMRIPrep's thread count, glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 55.2 s (8t) | 36.1 s (8t) | 8.94 s (8t) |
| A05 | Brain extraction: Laplacian of the T1w and the template | 2 | `lx.ants.image_math` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 18.6 s (1t) | 12.6 s (1t) | 3.92 s (1t) |
| A06 | Brain extraction: template priors to the T1w (Gaussian, inverse warp) | 2 | `lx.ants.apply_transforms` | 0 of 2 outputs bit-identical; worst: 4,369 of 15,842,304 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 | glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (max rel ≤ 0.0001) | 42.6 s (1t) | 39.5 s (1t) | 40.7 s (1t) |
| A07 | Brain extraction: threshold the brain prior (`ThresholdImage`) | 1 | `lx.ants.threshold_image` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 1.06 s (1t) | 1.08 s (1t) | 0.35 s (1t) |
| A08 | Brain extraction: mask and label morphology (`ImageMath` MD/ME/GetLargestComponent/FillHoles/PadImage/addtozero, `MultiplyImages`) | 26 | `lx.ants.image_math` | **bit-identical**: 26 of 26 outputs | – | pass (bit-identical) | 58.7 s (1t) | 60.2 s (1t) | 13.1 s (1t) |
| A09 | Brain extraction: final N4 (weighted by the WM posterior) | 1 | `lx.ants.n4_bias_field_correction` | 0 of 2 outputs bit-identical; worst: 15,841,685 of 15,842,304 differ; max \|diff\| 4.41e-06 (rel 4.4e-06), NCC 1 - 7.4e-12 | fMRIPrep's thread count, glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 20.5 s (8t) | 17.8 s (8t) | 2.92 s (8t) |
| A10 | Normalisation: truncate the moving T1w | 1 | `lx.ants.image_math` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 3.41 s (1t) | 2.62 s (1t) | 1.58 s (1t) |
| A11 | Anatomical outputs to MNI (`.h5` composite; Lanczos, MultiLabel, Gaussian) | 8 | `lx.ants.apply_transforms` | **bit-identical**: 8 of 8 outputs | – | pass (bit-identical) | 49.1 s (1t) | 35.5 s (1t) | 30.9 s (1t) |
| S01 | Field map: N4 of the field-map reference (sdcflows' brain extraction) | 1 | `lx.ants.n4_bias_field_correction` | 0 of 1 outputs bit-identical; worst: 98,545 of 163,840 differ; max \|diff\| 0.00418 (rel 2e-05), NCC 1 - 2.9e-11 | fMRIPrep's thread count; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 1.10 s (8t) | 1.20 s (8t) | 0.26 s (8t) |
| S02 | Unwarp the HMC boldref for coregistration (`ResampleSeries` with the field map) | 1 | `lx.transforms.resample_series` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 0.17 s (8t) | 1.56 s (8t) | 0.12 s (8t) |
| S03 | Field-map report: the field map and its reference to the boldref (Lanczos, `--float 1`) | 2 | `lx.ants.apply_transforms` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 1.75 s (1t) | 2.32 s (1t) | 0.53 s (1t) |
| F01 | Head-motion estimation (mcflirt on the raw BOLD, to the HMC boldref) | 1 | `lx.mri.hmc` | not bit-exact (clean-room); RMS deviation 95th pct ≤ 0.023 mm, median Δparam ≤ 0.0019 mm / 0.0038° | clean-room algorithm; mcflirt reproduces itself exactly | pass (≤ 0.05 mm / 0.05°; FD r > 0.99 where max FD ≥ 0.5 mm, or mcflirt's own band on the run) | 7.98 s (1t) | 8.21 s (1t) | 7.97 s (1t) |
| F02 | Slice-timing correction (`3dTshift`) | 1 | `lx.afni.tshift` | replica: **1 of 1 bit-identical**; clean-room original: 0 of 1 bit-identical (int16: 189 of 10,321,920 stored values differ, max \|diff\| 1 unit) | original: clean-room, within [afni-tshift.md](afni-tshift.md)'s thresholds (its Fourier shift computed in double precision, AFNI's in float32) | pass (float32 max rel ≤ 1e-05, integers ≤ 1 stored unit, the same type and scale factor) | 3.39 s (1t) | 2.71 s (1t) | 0.75 s (1t) |
| F03 | Coregistration reference: N4 of the HMC boldref | 1 | `lx.ants.n4_bias_field_correction` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 1.71 s (1t) | 1.16 s (1t) | 0.13 s (1t) |
| F04 | Coregistration reference: MNI brain prior to the boldref (Linear) | 1 | `lx.ants.apply_transforms` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 0.46 s (1t) | 0.57 s (1t) | 0.09 s (1t) |
| F05 | One-shot resampling to the boldref (HMC) | 1 | `lx.transforms.resample_series` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 3.36 s (8t) | 4.35 s (8t) | 0.83 s (8t) |
| F06 | One-shot resampling to T1w (HMC + coregistration) | 1 | `lx.transforms.resample_series` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 3.09 s (8t) | 3.58 s (8t) | 0.65 s (8t) |
| F07 | One-shot resampling to MNI res-2 (HMC + coregistration + `.h5` warp) | 1 | `lx.transforms.resample_series` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 22.6 s (8t) | 21.7 s (8t) | 6.74 s (8t) |
| F08 | boldref and BOLD mask to T1w (Lanczos, MultiLabel) | 2 | `lx.ants.apply_transforms` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 4.72 s (1t) | 2.02 s (1t) | 0.82 s (1t) |
| F09 | boldref and BOLD mask to MNI (Lanczos, MultiLabel, `.h5`) | 2 | `lx.ants.apply_transforms` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 12.2 s (1t) | 9.26 s (1t) | 6.61 s (1t) |
| F10 | Confound masks to the boldref (Gaussian, MultiLabel, inverse) | 4 | `lx.ants.apply_transforms` | 1 of 4 outputs bit-identical; worst: 101 of 163,840 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (max rel ≤ 0.0001) | 4.56 s (1t) | 3.89 s (1t) | 1.75 s (1t) |
| F11 | Carpet-plot segmentation to the boldref (MultiLabel, `.h5`, `-u int`) | 1 | `lx.ants.apply_transforms` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 4.06 s (1t) | 2.13 s (1t) | 1.74 s (1t) |
| F12 | Report: T1w and WM mask to the boldref (Lanczos, NearestNeighbor) | 2 | `lx.ants.apply_transforms` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 5.14 s (1t) | 1.74 s (1t) | 0.47 s (1t) |

Times are sums over the stage's nodes, with the thread counts used. fMRIPrep's run times include nipype's own overhead for each node; the re-runs time the program alone (inside the image, without Apptainer's start-up), and larmorx's the call (the command line in-process for the ANTs programs and head-motion correction; computing and writing the output for the resampling).

**Not run in this oracle run:**

- A02 N4 before the anatomical template (ANTs defaults): runs only with more than one T1w; this subject has one.

## What larmorx cannot run yet

Every other node of the run that runs a neuroimaging program, or a Python step larmorx will need, by the tool it needs. The time is fMRIPrep's (its run, 8 jobs at once). These are the next tools to port (CLAUDE.md, "Next").

| Tool | fMRIPrep nodes | fMRIPrep time | What fMRIPrep uses it for | larmorx milestone |
|---|---|---|---|---|
| antsRegistration | 4 | 2330 s | rigid/affine (BOLD premask) and SyN (brain extraction, MNI normalisation) | A4 (linear), A5 (SyN) |
| antsAI | 2 | 63.1 s | affine initialisation by rotation search | A4 |
| Atropos | 1 | 36.1 s | K-means + MRF segmentation in brain extraction | A6 |
| 3dvolreg | 1 | 22.4 s | aligns the reference volumes before the median (niworkflows' RobustAverage, inside the HMC-reference node) | AFNI |
| 3dUnifize | 1 | 91.3 s | T2-contrast intensity unifying of the boldref | AFNI |
| 3dAutomask | 2 | 0.22 s | BOLD brain mask, second pass | AFNI |
| BET | 2 | 2.54 s | BOLD brain mask, first pass | FSL (clean-room + `larmorx-nc`) |
| FAST | 1 | 120 s | GM/WM/CSF segmentation and probability maps | FSL |
| FLIRT-BBR | 1 | 91.5 s | boundary-based BOLD-to-T1w registration | FSL |
| fslmaths | 2 | 0.17 s | mask product of the two BOLD masks | FSL (trivial) |
| mri_coreg | 1 | 40.0 s | BOLD-to-T1w initialisation | FreeSurfer |
| lta_convert | 3 | 0.31 s | transform format conversion (LTA <-> FSL) | FreeSurfer (trivial; nitransforms has it) |
| topup | 1 | 62.0 s | PEPOLAR field-map estimation (sdcflows' PEPOLAR workflow) | FSL (clean-room + `larmorx-nc`) |
| prelude | – | – | phase unwrapping of the phase-difference field map | FSL (clean-room + `larmorx-nc`) |
| mri_robust_template | – | – | anatomical template of several T1w images | FreeSurfer |
| field-map B-splines | 5 | 11.0 s | sdcflows' B-spline fit of the field (`BSplineApprox`, `TOPUPCoeffReorient`) and fMRIPrep's `ReconstructFieldmap` (the field from its coefficients on a target grid, the input of `ResampleSeries`) | larmorprepx (Rust: B-spline evaluation) |
| field-map preparation | 18 | 5.55 s | sdcflows' Python steps: phase difference to Hz (`calc_phdiff`, `phmap2rads`, `compfmap`), PEPOLAR series sorted, padded, regridded and averaged, the field-map reference's brain mask, readout times and distortion parameters | larmorprepx (numpy) |
| confounds | 11 | 12.8 s | DVARS, CompCor (a/t/crown), global signals, expansion, spikes (fMRIPrep's Python); FD, RMSD and motion parameters are computed here from larmorx's HMC | larmorprepx (numpy) |

## A01: Denoise the T1w (`DenoiseImage -n Rician`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `denoise0` | sub-002_T1w_noise_corrected.nii.gz | 4,395,882 of 15,842,304 differ; max \|diff\| 6.69e+03 (rel 0.076), NCC 1 - 1.1e-05 | pass: NCC 0.9999893 | same | 66.4 s (8t) | 49.1 s | 4.38 s (8t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `denoise0` | sub-002_T1w_noise_corrected.nii.gz | 4,394,997 of 15,842,304 differ; max \|diff\| 6.69e+03 (rel 0.076), NCC 1 - 1.1e-05 | 5,205 of 15,842,304 differ; max \|diff\| 0.0159 (rel 1.8e-07), NCC 1 - 1.1e-16 | same | same | explained | 5,205 of 15,842,304 differ; max \|diff\| 0.0159 (rel 1.8e-07), NCC 1 - 1.1e-16 |

## A03: Brain extraction: truncate intensities (`TruncateImageIntensity`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `truncate_images0` | sub-002_T1w_noise_corrected_maths.nii.gz | bit-identical | pass: bit-identical | same | 3.51 s (1t) | 2.65 s | 1.20 s (1t) |

## A04: Brain extraction: N4, first pass

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `inu_n40` | sub-002_T1w_noise_corrected_maths_corrected.nii.gz | 15,837,135 of 15,842,304 differ; max \|diff\| 0.383 (rel 5.6e-06), NCC 1 - 5.9e-13 | pass: NCC 1.0000000 | same | 55.2 s (8t) | 36.1 s | 8.94 s (8t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `inu_n40` | sub-002_T1w_noise_corrected_maths_corrected.nii.gz | 15,842,304 of 15,842,304 differ; max \|diff\| 0.48 (rel 7e-06), NCC 1 - 6.5e-13 | 15,842,295 of 15,842,304 differ; max \|diff\| 0.336 (rel 4.9e-06), NCC 1 - 2.8e-13 | same | same | explained | 15,842,295 of 15,842,304 differ; max \|diff\| 0.336 (rel 4.9e-06), NCC 1 - 2.8e-13 |

## A05: Brain extraction: Laplacian of the T1w and the template

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `lap_target` | sub-002_T1w_noise_corrected_maths_corrected_maths.nii.gz | bit-identical | pass: bit-identical | same | 7.91 s (1t) | 6.11 s | 2.01 s (1t) |
| `lap_tmpl` | tpl-OASIS30ANTs_res-01_T1w_maths.nii.gz | bit-identical | pass: bit-identical | same | 10.7 s (1t) | 6.44 s | 1.91 s (1t) |

## A06: Brain extraction: template priors to the T1w (Gaussian, inverse warp)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `map_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans.nii.gz | 4,369 of 15,842,304 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 | pass: max rel 6.7e-16 | same | 21.2 s (1t) | 20.5 s | 20.4 s (1t) |
| `map_wmmask` | summap_trans.nii.gz | 7,254 of 15,842,304 differ; max \|diff\| 7.77e-16 (rel 7.8e-16), NCC 1 | pass: max rel 7.8e-16 | same | 21.4 s (1t) | 18.9 s | 20.3 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `map_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans.nii.gz | same | 4,369 of 15,842,304 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 | same | same | explained | 4,369 of 15,842,304 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 |
| `map_wmmask` | summap_trans.nii.gz | same | 7,254 of 15,842,304 differ; max \|diff\| 7.77e-16 (rel 7.8e-16), NCC 1 | same | same | explained | 7,254 of 15,842,304 differ; max \|diff\| 7.77e-16 (rel 7.8e-16), NCC 1 |

## A07: Brain extraction: threshold the brain prior (`ThresholdImage`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `thr_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled.nii.gz | bit-identical | pass: bit-identical | same | 1.06 s (1t) | 1.08 s | 0.35 s (1t) |

## A08: Brain extraction: mask and label morphology (`ImageMath` MD/ME/GetLargestComponent/FillHoles/PadImage/addtozero, `MultiplyImages`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `02_pad_segm` | sub-002_T1w_noise_corrected_maths_corrected_labeled_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.94 s (1t) | 1.16 s | 0.52 s (1t) |
| `03_pad_mask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.73 s (1t) | 1.11 s | 0.41 s (1t) |
| `05_get_wm` | sub-002_T1w_noise_corrected_maths_corrected_labeled_maths_class-03_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.50 s (1t) | 1.52 s | 0.34 s (1t) |
| `06_get_gm` | sub-002_T1w_noise_corrected_maths_corrected_labeled_maths_class-02_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.67 s (1t) | 1.51 s | 0.35 s (1t) |
| `07_fill_gm` | sub-002_T1w_noise_corrected_maths_corrected_labeled_maths_class-02_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 11.3 s (1t) | 11.2 s | 0.46 s (1t) |
| `08_mult_gm` | 08_mult_gm.nii.gz | bit-identical | pass: bit-identical | same | 1.00 s (1t) | 1.30 s | 0.42 s (1t) |
| `09_relabel_wm` | 09_relabel_wm.nii.gz | bit-identical | pass: bit-identical | same | 0.83 s (1t) | 1.01 s | 0.31 s (1t) |
| `10_me_csf` | sub-002_T1w_noise_corrected_maths_corrected_labeled_maths_class-01_maths.nii.gz | bit-identical | pass: bit-identical | same | 4.35 s (1t) | 4.18 s | 1.07 s (1t) |
| `11_add_gm` | 08_mult_gm_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.72 s (1t) | 1.65 s | 0.44 s (1t) |
| `12_relabel_gm` | 12_relabel_gm.nii.gz | bit-identical | pass: bit-identical | same | 0.81 s (1t) | 0.97 s | 0.31 s (1t) |
| `13_add_gm_wm` | 09_relabel_wm_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.77 s (1t) | 1.67 s | 0.47 s (1t) |
| `15_add_7` | 09_relabel_wm_maths_class-03_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.46 s (1t) | 1.52 s | 0.36 s (1t) |
| `16_me_7` | 09_relabel_wm_maths_class-03_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 3.10 s (1t) | 2.97 s | 1.10 s (1t) |
| `17_comp_7` | 09_relabel_wm_maths_class-03_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.63 s (1t) | 1.62 s | 0.37 s (1t) |
| `18_md_7` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.74 s (1t) | 1.67 s | 0.57 s (1t) |
| `19_fill_7` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 10.7 s (1t) | 10.6 s | 0.43 s (1t) |
| `20_add_7_2` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.77 s (1t) | 1.57 s | 0.43 s (1t) |
| `21_md_7_2` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.77 s (1t) | 1.63 s | 0.61 s (1t) |
| `22_me_7_2` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 3.12 s (1t) | 2.90 s | 1.07 s (1t) |
| `23_depad_mask` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.75 s (1t) | 1.07 s | 0.45 s (1t) |
| `24_depad_segm` | 09_relabel_wm_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.82 s (1t) | 1.13 s | 0.48 s (1t) |
| `25_depad_gm` | 12_relabel_gm_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.91 s (1t) | 1.16 s | 0.47 s (1t) |
| `26_depad_wm` | 09_relabel_wm_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.95 s (1t) | 1.21 s | 0.45 s (1t) |
| `27_depad_csf` | sub-002_T1w_noise_corrected_maths_corrected_labeled_maths_class-01_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.71 s (1t) | 1.05 s | 0.41 s (1t) |
| `dil_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.37 s (1t) | 1.41 s | 0.46 s (1t) |
| `get_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.30 s (1t) | 1.38 s | 0.32 s (1t) |

## A09: Brain extraction: final N4 (weighted by the WM posterior)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `inu_n4_final0` | corrected | 6,404,151 of 15,842,304 differ; max \|diff\| 1.75 (rel 6.6e-06), NCC 1 - 4.1e-13 | pass: NCC 1.0000000 | same | 20.5 s (8t) | 17.8 s | 2.92 s (8t) |
| `inu_n4_final0` | bias field | 15,841,685 of 15,842,304 differ; max \|diff\| 4.41e-06 (rel 4.4e-06), NCC 1 - 7.4e-12 | pass: NCC 1.0000000 | same | 20.5 s (8t) | 17.8 s | 2.92 s (8t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `inu_n4_final0` | corrected | 6,397,878 of 15,842,304 differ; max \|diff\| 2.03 (rel 7.7e-06), NCC 1 - 6.2e-13 | 6,301,102 of 15,842,304 differ; max \|diff\| 0.422 (rel 1.6e-06), NCC 1 - 7.7e-13 | same | same | explained | 6,301,102 of 15,842,304 differ; max \|diff\| 0.422 (rel 1.6e-06), NCC 1 - 7.7e-13 |
| `inu_n4_final0` | bias field | 15,581,672 of 15,842,304 differ; max \|diff\| 3.93e-06 (rel 3.9e-06), NCC 1 - 7.6e-12 | 15,735,809 of 15,842,304 differ; max \|diff\| 1.25e-06 (rel 1.2e-06), NCC 1 - 1.8e-12 | same | same | explained | 15,735,809 of 15,842,304 differ; max \|diff\| 1.25e-06 (rel 1.2e-06), NCC 1 - 1.8e-12 |

## A10: Normalisation: truncate the moving T1w

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| MNI152NLin2009cAsym `trunc_mov` | sub-002_T1w_noise_corrected_corrected_xform_maths.nii.gz | bit-identical | pass: bit-identical | same | 3.41 s (1t) | 2.62 s | 1.58 s (1t) |

## A11: Anatomical outputs to MNI (`.h5` composite; Lanczos, MultiLabel, Gaussian)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| MNI152NLin2009cAsym.res2 `mask_std` | sub-002_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 9.84 s (1t) | 6.78 s | 8.69 s (1t) |
| MNI152NLin2009cAsym.res2 `t1w_std` | sub-002_desc-preproc_T1w_trans.nii.gz | bit-identical | pass: bit-identical | same | 5.67 s (1t) | 3.96 s | 2.41 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_dseg` | sub-002_dseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 9.04 s (1t) | 7.40 s | 6.44 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_mask` | sub-002_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 8.37 s (1t) | 6.20 s | 5.66 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_t1w` | sub-002_desc-preproc_T1w_masked_trans.nii.gz | bit-identical | pass: bit-identical | same | 6.03 s (1t) | 4.04 s | 1.96 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms0` | sub-002_label-GM_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 4.00 s (1t) | 2.35 s | 1.93 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms1` | sub-002_label-WM_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 3.50 s (1t) | 2.41 s | 1.91 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms2` | sub-002_label-CSF_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 2.66 s (1t) | 2.40 s | 1.94 s (1t) |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| MNI152NLin2009cAsym.res2 `anat2std_dseg` | sub-002_dseg_trans.nii.gz | out/sub-002/anat/sub-002_space-MNI152NLin2009cAsym_res-2_dseg.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_mask` | sub-002_desc-brain_mask_trans.nii.gz | out/sub-002/anat/sub-002_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_t1w` | sub-002_desc-preproc_T1w_masked_trans.nii.gz | out/sub-002/anat/sub-002_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_tpms0` | sub-002_label-GM_probseg_trans.nii.gz | out/sub-002/anat/sub-002_space-MNI152NLin2009cAsym_res-2_label-GM_probseg.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_tpms1` | sub-002_label-WM_probseg_trans.nii.gz | out/sub-002/anat/sub-002_space-MNI152NLin2009cAsym_res-2_label-WM_probseg.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_tpms2` | sub-002_label-CSF_probseg_trans.nii.gz | out/sub-002/anat/sub-002_space-MNI152NLin2009cAsym_res-2_label-CSF_probseg.nii.gz | bit-identical |

## S01: Field map: N4 of the field-map reference (sdcflows' brain extraction)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `n4` | clipped_corrected.nii.gz | 98,545 of 163,840 differ; max \|diff\| 0.00418 (rel 2e-05), NCC 1 - 2.9e-11 | pass: NCC 1.0000000 | same | 1.10 s (8t) | 1.20 s | 0.26 s (8t) |

**nipype re-quantizes these outputs** (PIPE-16, [register](../findings/upstream-defects.md)): its `N4BiasFieldCorrection` interface with `copy_header=True` writes the corrected image with the input's data type (int16), so fMRIPrep's node output is N4's float32 result rounded to that type with a scale factor chosen by nibabel. Every run above (larmorx, fMRIPrep's program re-run, the ANTs references) is compared after the same step, nipype's own `copy_header` run in fMRIPrep's image on a copy of its output (`nipype-<name>`). larmorx's own float32 output against fMRIPrep's node output, i.e. the rounding itself plus any other difference:

| Node | Output | larmorx (float32) vs fMRIPrep (re-quantized) |
|---|---|---|
| `n4` | clipped_corrected.nii.gz | 98,545 of 163,840 differ; max \|diff\| 0.00404 (rel 1.9e-05), NCC 1 - 1.3e-10 |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `n4` | clipped_corrected.nii.gz | 98,545 of 163,840 differ; max \|diff\| 0.00418 (rel 2e-05), NCC 1 - 2.9e-11 | same | same | same | explained | same |

## S02: Unwarp the HMC boldref for coregistration (`ResampleSeries` with the field map)

`lx.transforms.resample_series` with the `ResampleSeries` node's inputs from its report: the node's `in_file` (fMRIPrep's own input: the BOLD series, after slice timing where fMRIPrep corrected it, or the boldref), the target grid, the transforms (`0` files), order 3, mode `grid-constant`, Jacobian True, float32 output, fMRIPrep's field map on the target grid (`ReconstructFieldmap`'s output), phase encoding `j`, readout 0.0137951 s. fMRIPrep's time includes writing the compressed output; larmorx's is shown as compute + write. fMRIPrep's re-run has the node's `num_threads` and `OMP_NUM_THREADS=1`, as fMRIPrep's image sets it: with more OpenBLAS threads, fMRIPrep's resampler corrupts some volumes ([fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx's result does not depend on any thread count.

| Run | Shape | larmorx vs fMRIPrep's node | larmorx vs the derivative | PLAN.md §11.3 | fMRIPrep re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|---|
| run-1 `unwarp_boldref` | 64×64×40 | bit-identical | bit-identical | pass: bit-identical | same | 0.17 s (8t) | 1.56 s | 0.11 s + 0.01 s (8t) |

**PIPE-03 on fMRIPrep's own field map** ([register](../findings/upstream-defects.md), [resample-series-jacobian.md](resample-series-jacobian.md)). The comparison above runs `upstream_defects="reproduce"`, fMRIPrep's Jacobian along the target axis that has the source's phase-encoding index, in target voxels. larmorx's default (`fix`) takes the derivative along the phase-encoding direction mapped into the target grid. Change of the default against fMRIPrep's output, per brain voxel (fMRIPrep's BOLD brain mask on that grid, e.g. `out/sub-002/func/sub-002_task-clip_run-1_desc-brain_mask.nii.gz`): the relative change of the voxel's temporal mean, which with the Jacobian is the ratio of the two factors. Where fMRIPrep ran without the Jacobian (its default for field maps other than PEPOLAR), larmorx's default leaves the output unchanged (checked), and both runs are repeated with the Jacobian on, as `--force fmap-jacobian` would run them. Where fMRIPrep's factor is close to 0 (a fold of the distortion), the relative change is huge and says little; the percentiles describe the bulk. Coordinates are world mm of the output grid (MNI for the standard space); clusters are 6-connected voxels changed by more than 1 %:

| Run | Jacobian in fMRIPrep's run | Shape | Brain voxels | Changed > 0.1 % | > 1 % | > 5 % | Median / 90th / 99th pct / max \|change\| | Signed range | Where it is largest |
|---|---|---|---|---|---|---|---|---|---|
| run-1 `unwarp_boldref` | on | 64×64×40 | – | none: bit-identical | 0 % | 0 % | 0 | – | – |

The two factors themselves (a volume of ones through the run's transforms without head motion, with fMRIPrep's field map on the target grid), inside the brain mask:

| Run | fMRIPrep's factor | larmorx's factor | Differ by > 0.01 | > 0.05 | Median / max difference |
|---|---|---|---|---|---|
| run-1 `unwarp_boldref` | 0.395 to 1.686 | 0.395 to 1.686 | 0 % | 0 % | 0.0000 / 0.000 |

## S03: Field-map report: the field map and its reference to the boldref (Lanczos, `--float 1`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-1 `fmap_boldref` | sub-002_fmapid-auto00000_desc-coeff_fieldmaprec_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.87 s (1t) | 1.18 s | 0.26 s (1t) |
| run-1 `fmapref_boldref` | sub-002_fmapid-auto00000_desc-epi_fieldmap_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.88 s (1t) | 1.14 s | 0.27 s (1t) |

## F01: Head-motion estimation (mcflirt on the raw BOLD, to the HMC boldref)

larmorx runs `larmorx mri hmc` with mcflirt's arguments from `command.txt` (the raw BOLD, fMRIPrep's HMC boldref as `-reffile`, `-mats`). Matrices against mcflirt's, per volume (RMS deviation over a sphere of 80 mm about the field of view's centre, Jenkinson 1999; parameter differences about the same centre); mcflirt again in fMRIPrep's image, and FSL 6.0.7's mcflirt on this machine, against fMRIPrep's run:

| Run | Volumes | RMS deviation median / 95th pct / max (mm) | Median Δparam (mm / °) | Max Δparam (mm / °) | mcflirt re-run: identical volumes | FSL 6.0.7 mcflirt: identical volumes |
|---|---|---|---|---|---|---|
| run-1 `mcflirt` | 63 | 0.0131 / 0.0229 / 0.0348 | 0.0019 / 0.0038 | 0.0176 / 0.0327 | 63 of 63 | 63 of 63 |

**What fMRIPrep derives from them**, computed by fMRIPrep's own code in its image (niworkflows' `MCFLIRT2ITK`, fMRIPrep's `FSLMotionParams`, `FramewiseDisplacement`, `FSLRMSDeviation`) from larmorx's matrices, against the confounds file fMRIPrep wrote (`desc-confounds_timeseries.tsv`) and its `desc-hmc_xfm.txt`. The same code on mcflirt's matrices reproduces fMRIPrep's file (last column). mcflirt's band is its own lowest FD correlation when its input is perturbed slightly: on this run, where PLAN.md §11.3's FD correlation applies and larmorx misses it (below), else the [mri-hmc](mri-hmc.md) band of the dataset's testdata case, where there is one (another series of the dataset, so for information only):

| Run | Median Δ motion params (mm / °) | Max Δ (mm / °) | FD r | Max FD (mm) | Mean FD fMRIPrep / larmorx (mm) | Max Δ RMSD (mm) | Max Δ ITK matrix entry (mm for translations) | Check: mcflirt's matrices |
|---|---|---|---|---|---|---|---|---|
| run-1 `mcflirt` | 0.0013 / 0.0038 | 0.0165 / 0.0327 | 0.7681 (testdata's band 0.8238) | 0.167 | 0.0866 / 0.0852 | 0.027 | 0.023 | same |

Times: fMRIPrep's node (its run), mcflirt re-run in fMRIPrep's image on this machine (one thread, as mcflirt runs), `larmorx mri hmc` with the same arguments (it also resamples and writes the series, as mcflirt does), and the Python API with `resample=False` (the matrices only, all fMRIPrep uses):

| Run | fMRIPrep | mcflirt re-run | larmorx CLI | larmorx API | larmorx API, all CPUs |
|---|---|---|---|---|---|
| run-1 `mcflirt` | 7.98 s | 8.21 s | 7.97 s (1t) | 6.06 s (1t) | 1.32 s (12t) |

## F02: Slice-timing correction (`3dTshift`)

`larmorx afni 3dTshift` with the node's arguments from `command.txt`, in both implementations, in `reproduce` mode (fMRIPrep as it ran). nipype's `TShift` writes the slice times into `slice_timing.1D` in the node directory and links the input there, and removes both after the run; the harness writes the file again exactly as nipype does (the times joined by tabs, as Python's `str` prints them, 40 slices) and gives the input's original path, from the node's hash file. The thresholds are [afni-tshift.md](afni-tshift.md)'s. Both larmorx implementations write no AFNI history extension (`docs/api/afni-tshift.md`), so `vox_offset` differs when AFNI wrote one.

| Run | Implementation | Against fMRIPrep's output | Thresholds | Header (as stored) | Against AFNI 25.2.09 (oracle build) | Time |
|---|---|---|---|---|---|---|
| run-1 `slice_timing_correction` | replica | bit-identical | pass: bit-identical | identical but `vox_offset` (AFNI's history extension, which larmorx does not write) | same | 0.75 s (1t) |
| run-1 `slice_timing_correction` | original | 189 of 10,321,920 differ; max \|diff\| 47.7 (rel 0.00024), NCC 1 - 2.6e-11 | pass: int16: 189 of 10,321,920 stored values differ, max \|diff\| 1 unit | identical but `vox_offset` (AFNI's history extension, which larmorx does not write) | 189 of 10,321,920 differ; max \|diff\| 47.7 (rel 0.00024), NCC 1 - 2.6e-11 | 0.66 s (1t) |
| run-1 `slice_timing_correction` | fMRIPrep's AFNI, re-run in its image / AFNI 25.2.09 oracle build | same / same |  |  |  | 3.39 s (1t, its run); 2.71 s (re-run) |

larmorx's default (`upstream_defects="fix"`, AFNI's defects corrected, D13) against the `reproduce` output above, and the defects it named:

| Run | Implementation | Data | Header fields | Notices |
|---|---|---|---|---|
| run-1 `slice_timing_correction` | replica | same | same | none |
| run-1 `slice_timing_correction` | original | same | same | none |

## F03: Coregistration reference: N4 of the HMC boldref

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-1 `n4_correct` | sub-002_task-clip_run-1_desc-hmc_boldref_corrected.nii.gz | bit-identical | pass: bit-identical | same | 1.71 s (1t) | 1.16 s | 0.13 s (1t) |

## F04: Coregistration reference: MNI brain prior to the boldref (Linear)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-1 `map_brainmask` | tpl-MNI152NLin2009cAsym_res-01_label-brain_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.46 s (1t) | 0.57 s | 0.09 s (1t) |

## F05: One-shot resampling to the boldref (HMC)

`lx.transforms.resample_series` with the `ResampleSeries` node's inputs from its report: the node's `in_file` (fMRIPrep's own input: the BOLD series, after slice timing where fMRIPrep corrected it, or the boldref), the target grid, the transforms (`1` files), order 3, mode `grid-constant`, Jacobian True, float32 output, fMRIPrep's field map on the target grid (`ReconstructFieldmap`'s output), phase encoding `j`, readout 0.0137951 s. fMRIPrep's time includes writing the compressed output; larmorx's is shown as compute + write. fMRIPrep's re-run has the node's `num_threads` and `OMP_NUM_THREADS=1`, as fMRIPrep's image sets it: with more OpenBLAS threads, fMRIPrep's resampler corrupts some volumes ([fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx's result does not depend on any thread count.

| Run | Shape | larmorx vs fMRIPrep's node | larmorx vs the derivative | PLAN.md §11.3 | fMRIPrep re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|---|
| run-1 `boldref_bold` | 64×64×40×63 | bit-identical | bit-identical | pass: bit-identical | same | 3.36 s (8t) | 4.35 s | 0.58 s + 0.26 s (8t) |

**PIPE-03 on fMRIPrep's own field map** ([register](../findings/upstream-defects.md), [resample-series-jacobian.md](resample-series-jacobian.md)). The comparison above runs `upstream_defects="reproduce"`, fMRIPrep's Jacobian along the target axis that has the source's phase-encoding index, in target voxels. larmorx's default (`fix`) takes the derivative along the phase-encoding direction mapped into the target grid. Change of the default against fMRIPrep's output, per brain voxel (fMRIPrep's BOLD brain mask on that grid, e.g. `out/sub-002/func/sub-002_task-clip_run-1_desc-brain_mask.nii.gz`): the relative change of the voxel's temporal mean, which with the Jacobian is the ratio of the two factors. Where fMRIPrep ran without the Jacobian (its default for field maps other than PEPOLAR), larmorx's default leaves the output unchanged (checked), and both runs are repeated with the Jacobian on, as `--force fmap-jacobian` would run them. Where fMRIPrep's factor is close to 0 (a fold of the distortion), the relative change is huge and says little; the percentiles describe the bulk. Coordinates are world mm of the output grid (MNI for the standard space); clusters are 6-connected voxels changed by more than 1 %:

| Run | Jacobian in fMRIPrep's run | Shape | Brain voxels | Changed > 0.1 % | > 1 % | > 5 % | Median / 90th / 99th pct / max \|change\| | Signed range | Where it is largest |
|---|---|---|---|---|---|---|---|---|---|
| run-1 `boldref_bold` | on | 64×64×40×63 | – | none: bit-identical | 0 % | 0 % | 0 | – | – |

The two factors themselves (a volume of ones through the run's transforms without head motion, with fMRIPrep's field map on the target grid), inside the brain mask:

| Run | fMRIPrep's factor | larmorx's factor | Differ by > 0.01 | > 0.05 | Median / max difference |
|---|---|---|---|---|---|
| run-1 `boldref_bold` | 0.395 to 1.686 | 0.395 to 1.686 | 0 % | 0 % | 0.0000 / 0.000 |

## F06: One-shot resampling to T1w (HMC + coregistration)

`lx.transforms.resample_series` with the `ResampleSeries` node's inputs from its report: the node's `in_file` (fMRIPrep's own input: the BOLD series, after slice timing where fMRIPrep corrected it, or the boldref), the target grid, the transforms (`2` files), order 3, mode `grid-constant`, Jacobian True, float32 output, fMRIPrep's field map on the target grid (`ReconstructFieldmap`'s output), phase encoding `j`, readout 0.0137951 s. fMRIPrep's time includes writing the compressed output; larmorx's is shown as compute + write. fMRIPrep's re-run has the node's `num_threads` and `OMP_NUM_THREADS=1`, as fMRIPrep's image sets it: with more OpenBLAS threads, fMRIPrep's resampler corrupts some volumes ([fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx's result does not depend on any thread count.

| Run | Shape | larmorx vs fMRIPrep's node | larmorx vs the derivative | PLAN.md §11.3 | fMRIPrep re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|---|
| run-1 `resample` | 50×62×44×63 | bit-identical | bit-identical | pass: bit-identical | same | 3.09 s (8t) | 3.58 s | 0.44 s + 0.21 s (8t) |

**PIPE-03 on fMRIPrep's own field map** ([register](../findings/upstream-defects.md), [resample-series-jacobian.md](resample-series-jacobian.md)). The comparison above runs `upstream_defects="reproduce"`, fMRIPrep's Jacobian along the target axis that has the source's phase-encoding index, in target voxels. larmorx's default (`fix`) takes the derivative along the phase-encoding direction mapped into the target grid. Change of the default against fMRIPrep's output, per brain voxel (fMRIPrep's BOLD brain mask on that grid, e.g. `out/sub-002/func/sub-002_task-clip_run-1_space-T1w_desc-brain_mask.nii.gz`): the relative change of the voxel's temporal mean, which with the Jacobian is the ratio of the two factors. Where fMRIPrep ran without the Jacobian (its default for field maps other than PEPOLAR), larmorx's default leaves the output unchanged (checked), and both runs are repeated with the Jacobian on, as `--force fmap-jacobian` would run them. Where fMRIPrep's factor is close to 0 (a fold of the distortion), the relative change is huge and says little; the percentiles describe the bulk. Coordinates are world mm of the output grid (MNI for the standard space); clusters are 6-connected voxels changed by more than 1 %:

| Run | Jacobian in fMRIPrep's run | Shape | Brain voxels | Changed > 0.1 % | > 1 % | > 5 % | Median / 90th / 99th pct / max \|change\| | Signed range | Where it is largest |
|---|---|---|---|---|---|---|---|---|---|
| run-1 `resample` | on | 50×62×44×63 | 51,660 | 78.8 % | 2.8 % | 0.031 % | 0.3 % / 0.7 % / 1.4 % / 6.8 % | -0.024 to +0.068 | max at (54.8, 5.4, 14.9); 322 voxels about (-3.2, 68.2, 29.1), up to 3.3 %; 172 voxels about (-48.2, 8.6, 8.4), up to 5.9 % |

The two factors themselves (a volume of ones through the run's transforms without head motion, with fMRIPrep's field map on the target grid), inside the brain mask:

| Run | fMRIPrep's factor | larmorx's factor | Differ by > 0.01 | > 0.05 | Median / max difference |
|---|---|---|---|---|---|
| run-1 `resample` | 0.376 to 1.633 | 0.386 to 1.649 | 2.8 % | 0 % | 0.0025 / 0.047 |

## F07: One-shot resampling to MNI res-2 (HMC + coregistration + `.h5` warp)

`lx.transforms.resample_series` with the `ResampleSeries` node's inputs from its report: the node's `in_file` (fMRIPrep's own input: the BOLD series, after slice timing where fMRIPrep corrected it, or the boldref), the target grid, the transforms (`3` files), order 3, mode `grid-constant`, Jacobian True, float32 output, fMRIPrep's field map on the target grid (`ReconstructFieldmap`'s output), phase encoding `j`, readout 0.0137951 s. fMRIPrep's time includes writing the compressed output; larmorx's is shown as compute + write. fMRIPrep's re-run has the node's `num_threads` and `OMP_NUM_THREADS=1`, as fMRIPrep's image sets it: with more OpenBLAS threads, fMRIPrep's resampler corrupts some volumes ([fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx's result does not depend on any thread count.

| Run | Shape | larmorx vs fMRIPrep's node | larmorx vs the derivative | PLAN.md §11.3 | fMRIPrep re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|---|
| run-1 MNI152NLin2009cAsym.res2 `resample` | 97×115×97×63 | bit-identical | bit-identical | pass: bit-identical | same | 22.6 s (8t) | 21.7 s | 5.19 s + 1.55 s (8t) |

**PIPE-03 on fMRIPrep's own field map** ([register](../findings/upstream-defects.md), [resample-series-jacobian.md](resample-series-jacobian.md)). The comparison above runs `upstream_defects="reproduce"`, fMRIPrep's Jacobian along the target axis that has the source's phase-encoding index, in target voxels. larmorx's default (`fix`) takes the derivative along the phase-encoding direction mapped into the target grid. Change of the default against fMRIPrep's output, per brain voxel (fMRIPrep's BOLD brain mask on that grid, e.g. `out/sub-002/func/sub-002_task-clip_run-1_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz`): the relative change of the voxel's temporal mean, which with the Jacobian is the ratio of the two factors. Where fMRIPrep ran without the Jacobian (its default for field maps other than PEPOLAR), larmorx's default leaves the output unchanged (checked), and both runs are repeated with the Jacobian on, as `--force fmap-jacobian` would run them. Where fMRIPrep's factor is close to 0 (a fold of the distortion), the relative change is huge and says little; the percentiles describe the bulk. Coordinates are world mm of the output grid (MNI for the standard space); clusters are 6-connected voxels changed by more than 1 %:

| Run | Jacobian in fMRIPrep's run | Shape | Brain voxels | Changed > 0.1 % | > 1 % | > 5 % | Median / 90th / 99th pct / max \|change\| | Signed range | Where it is largest |
|---|---|---|---|---|---|---|---|---|---|
| run-1 MNI152NLin2009cAsym.res2 `resample` | on | 97×115×97×63 | 254,305 | 95.6 % | 59.5 % | 5.6 % | 1.3 % / 3.8 % / 11.3 % / 55.6 % | -0.556 to +0.404 | max at (59.5, -18.5, -28.5); 149,930 voxels about (-1.6, -19.7, 10.5), up to 55.6 %; 47 voxels about (-57.4, -5.1, 17.2), up to 2.3 % |

The two factors themselves (a volume of ones through the run's transforms without head motion, with fMRIPrep's field map on the target grid), inside the brain mask:

| Run | fMRIPrep's factor | larmorx's factor | Differ by > 0.01 | > 0.05 | Median / max difference |
|---|---|---|---|---|---|
| run-1 MNI152NLin2009cAsym.res2 `resample` | 0.464 to 1.473 | 0.331 to 1.774 | 59.6 % | 5.6 % | 0.0128 / 0.417 |

## F08: boldref and BOLD mask to T1w (Lanczos, MultiLabel)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-1 `resample_mask` | sub-002_task-clip_run-1_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 3.81 s (1t) | 1.06 s | 0.62 s (1t) |
| run-1 `resample_ref` | sub-002_task-clip_run-1_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.91 s (1t) | 0.96 s | 0.20 s (1t) |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| run-1 `resample_mask` | sub-002_task-clip_run-1_desc-brain_mask_trans.nii.gz | out/sub-002/func/sub-002_task-clip_run-1_space-T1w_desc-brain_mask.nii.gz | bit-identical |
| run-1 `resample_ref` | sub-002_task-clip_run-1_desc-coreg_boldref_trans.nii.gz | out/sub-002/func/sub-002_task-clip_run-1_space-T1w_boldref.nii.gz | bit-identical |

## F09: boldref and BOLD mask to MNI (Lanczos, MultiLabel, `.h5`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-1 MNI152NLin2009cAsym.res2 `resample_mask` | sub-002_task-clip_run-1_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 6.54 s (1t) | 5.13 s | 4.97 s (1t) |
| run-1 MNI152NLin2009cAsym.res2 `resample_ref` | sub-002_task-clip_run-1_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 5.69 s (1t) | 4.13 s | 1.65 s (1t) |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| run-1 MNI152NLin2009cAsym.res2 `resample_mask` | sub-002_task-clip_run-1_desc-brain_mask_trans.nii.gz | out/sub-002/func/sub-002_task-clip_run-1_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| run-1 MNI152NLin2009cAsym.res2 `resample_ref` | sub-002_task-clip_run-1_desc-coreg_boldref_trans.nii.gz | out/sub-002/func/sub-002_task-clip_run-1_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz | bit-identical |

## F10: Confound masks to the boldref (Gaussian, MultiLabel, inverse)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-1 `acc_msk_tfm0` | sub-002_label-CSF_probseg_trans.nii.gz | 101 of 163,840 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | pass: max rel 2.2e-16 | same | 1.16 s (1t) | 0.85 s | 0.28 s (1t) |
| run-1 `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | 13 of 163,840 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | pass: max rel 2.2e-16 | same | 0.85 s (1t) | 0.79 s | 0.32 s (1t) |
| run-1 `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | 16 of 163,840 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | pass: max rel 2.2e-16 | same | 0.89 s (1t) | 0.78 s | 0.33 s (1t) |
| run-1 `t1w_mask_tfm` | sub-002_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 1.66 s (1t) | 1.47 s | 0.82 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| run-1 `acc_msk_tfm0` | sub-002_label-CSF_probseg_trans.nii.gz | same | 101 of 163,840 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | same | same | explained | 101 of 163,840 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 |
| run-1 `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | same | 13 of 163,840 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | same | same | explained | 13 of 163,840 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 |
| run-1 `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | same | 16 of 163,840 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | same | same | explained | 16 of 163,840 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 |

## F11: Carpet-plot segmentation to the boldref (MultiLabel, `.h5`, `-u int`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-1 `resample_parc` | tpl-MNI152NLin2009cAsym_res-01_desc-carpet_dseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 4.06 s (1t) | 2.13 s | 1.74 s (1t) |

## F12: Report: T1w and WM mask to the boldref (Lanczos, NearestNeighbor)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-1 `boldref_wm` | sub-002_dseg_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 1.66 s (1t) | 0.54 s | 0.10 s (1t) |
| run-1 `t1w_boldref` | sub-002_desc-preproc_T1w_trans.nii.gz | bit-identical | pass: bit-identical | same | 3.49 s (1t) | 1.20 s | 0.37 s (1t) |

## Reproduce

```bash
scripts/run_fmriprep_oracle.sh ~/.cache/larmorx-testdata/data/openneuro/ds005040 002 ds005040-sub-002-noreconall --fs-no-reconall   # the oracle run (about an hour)
scripts/build_ants_oracle.sh && scripts/build_crmath_preload.sh   # the references
scripts/build_afni_oracle.sh   # AFNI 25.2.09 (3dTshift)
(cd crates-gpl && cargo build --release -p larmorx-gpl-cli)   # the 3dTshift replica
python -m larmorx_validation pipeline-stages --oracle <workspace>/oracles/fmriprep-25.2.6/ds005040-sub-002-noreconall --out docs/validation --reuse --name fmriprep-stages-ds005040
```

larmorx's outputs and the reference runs are kept in `<workspace>/oracles/compare/fmriprep-25.2.6/ds005040-sub-002-noreconall/<node path>/<run>/` (`larmorx`, `larmorx-replica`, `larmorx-original`, `*-fix`, `container-<n>t`, `container-1t-cr`, `ants-2.6.5-1t`, `ants-2.6.5-1t-cr`, `ants-2.6.5-1t-cr-double`, `afni-25.2.09`, `fsl-6.0.7`, `confounds-*`), each with `run.json` (arguments, environment, exit status, time). `--reuse` reuses the one-thread reference runs of an earlier invocation; `--stage A01 F05` limits the comparison; `--render` writes this page again from the JSON record, without running anything.

