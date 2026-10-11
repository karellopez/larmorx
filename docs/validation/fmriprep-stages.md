# Stage by stage: larmorx against fMRIPrep 25.2.6

larmorx's tools run on fMRIPrep's **own inputs and parameters** for each step it already implements, read from nipype's working directory (`command.txt`, `_report/report.rst`), and their outputs are compared with what fMRIPrep produced at that step. So every stage is judged on its own, with no error carried over from earlier stages. **The headline numbers are against fMRIPrep as it ran**; where they differ, the step is run again in fMRIPrep's own container and with the repository's oracle builds to find out why.

- **Oracle:** fMRIPrep 25.2.6, image `nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e` (Apptainer), run `ds000005-sub-01-noreconall`: `/data /out participant --participant-label 01 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 8 --omp-nthreads 8 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall`. Input: ds000005 sub-01 (one T1w, three BOLD runs of 240 volumes, no field maps, no slice timing).
- **Tools inside fMRIPrep's image:** ANTs 2.6.2.dev1-gff61a3d (conda-forge `ants` 2.6.2) on ITK 5.4.4; FSL `mcflirt` 2111.0 (`newimage` 2501.4); AFNI 25.2.09; nitransforms 25.1.0, SciPy 1.15.2, numpy 2.2.6 on OpenBLAS 0.3.30; glibc 2.35.
- **larmorx:** 0.0.1 (45564252b1ee), on Linux x86_64, Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs.
- **References for the causes:** fMRIPrep's own tools re-run in its image, at its thread count and at one thread, as built and with CORE-MATH's correctly rounded functions preloaded (`scripts/build_crmath_preload.sh`; the image has this machine's glibc 2.35); ANTs 2.6.5 on ITK 5.4.5 built by `scripts/build_ants_oracle.sh` (one thread, as built and with CORE-MATH preloaded); FSL 6.0.7's `mcflirt` on this machine.
- **Generated:** 2026-10-11, with `python -m larmorx_validation pipeline-stages --oracle <workspace>/oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall --out docs/validation`. Times are **on a shared machine** (load average 1.8 at the start; other jobs: 0), so they are indicative only; fMRIPrep's own times come from its run (8 jobs at once on a machine shared with a development agent, see `RUN_INFO.txt`), the re-runs of fMRIPrep's tools from this invocation, at fMRIPrep's thread counts.

## Results

**61 of 92 compared nodes are bit-identical to fMRIPrep's own output** (plus 3 head-motion corrections, which larmorx does with a clean-room algorithm and cannot match bit for bit). The other differences are explained by fMRIPrep's thread count, glibc's maths or the ANTs build (fMRIPrep's conda-forge 2.6.2 against the repository's GCC 11 build of 2.6.5; with those removed, larmorx is bit-identical), except 14 node(s) where larmorx itself differs, for a known reason: `antsApplyTransforms --float 1`, which ANTs runs in single precision and larmorx in double (ANTs in double precision, rounded to float32, is larmorx's output bit for bit).

**3 of those outputs miss PLAN.md §11.3's interpolation threshold**, each in at most 6 of 1,082,035 voxels (`A11 sub-01_label-GM_probseg_trans.nii.gz`, `A11 sub-01_label-WM_probseg_trans.nii.gz`, `A11 sub-01_label-CSF_probseg_trans.nii.gz`): voxels where ANTs' single-precision Gaussian interpolation departs from its own double-precision result, which is larmorx's. Everywhere else these outputs stay within the threshold.

| Stage | Step | Nodes | larmorx | Against fMRIPrep as it ran | Why they differ | PLAN.md §11.3 | fMRIPrep (its run) | fMRIPrep's tool re-run | larmorx |
|---|---|---|---|---|---|---|---|---|---|
| A01 | Denoise the T1w (`DenoiseImage -n Rician`) | 1 | `lx.ants.denoise_image` | 0 of 1 outputs bit-identical; worst: 4,900,422 of 5,898,240 differ; max \|diff\| 271 (rel 0.19), NCC 1 - 0.00011 | not reproducible run to run, fMRIPrep's thread count, glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 53.9 s (8t) | 58.2 s (8t) | 3.24 s (8t) |
| A03 | Brain extraction: truncate intensities (`TruncateImageIntensity`) | 1 | `lx.ants.image_math` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 3.10 s (1t) | 1.70 s (1t) | 0.81 s (1t) |
| A04 | Brain extraction: N4, first pass | 1 | `lx.ants.n4_bias_field_correction` | 0 of 1 outputs bit-identical; worst: 5,786,955 of 5,898,240 differ; max \|diff\| 0.00183 (rel 3.4e-06), NCC 1 - 4.5e-13 | fMRIPrep's thread count, glibc's maths, the ANTs build (fMRIPrep's 2.6.2 vs the oracle's 2.6.5; for N4 the oracle's GCC 11 code generation, [fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 14.4 s (8t) | 13.1 s (8t) | 3.06 s (8t) |
| A05 | Brain extraction: Laplacian of the T1w and the template | 2 | `lx.ants.image_math` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 12.8 s (1t) | 9.98 s (1t) | 3.53 s (1t) |
| A06 | Brain extraction: template priors to the T1w (Gaussian, inverse warp) | 2 | `lx.ants.apply_transforms` | 0 of 2 outputs bit-identical; worst: 1,588 of 5,898,240 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 | glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (max rel ≤ 0.0001) | 20.7 s (1t) | 16.0 s (1t) | 20.5 s (1t) |
| A07 | Brain extraction: threshold the brain prior (`ThresholdImage`) | 1 | `lx.ants.threshold_image` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 0.65 s (1t) | 0.67 s (1t) | 0.13 s (1t) |
| A08 | Brain extraction: mask and label morphology (`ImageMath` MD/ME/GetLargestComponent/FillHoles/PadImage/addtozero, `MultiplyImages`) | 26 | `lx.ants.image_math` | **bit-identical**: 26 of 26 outputs | – | pass (bit-identical) | 27.6 s (1t) | 30.9 s (1t) | 4.69 s (1t) |
| A09 | Brain extraction: final N4 (weighted by the WM posterior) | 1 | `lx.ants.n4_bias_field_correction` | 0 of 2 outputs bit-identical; worst: 5,367,471 of 5,898,240 differ; max \|diff\| 0.0789 (rel 2.9e-05), NCC 1 - 1.6e-11 | fMRIPrep's thread count, glibc's maths, the ANTs build (fMRIPrep's 2.6.2 vs the oracle's 2.6.5; for N4 the oracle's GCC 11 code generation, [fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 9.52 s (8t) | 7.02 s (8t) | 1.13 s (8t) |
| A10 | Normalisation: truncate the moving T1w | 1 | `lx.ants.image_math` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 2.61 s (1t) | 1.72 s (1t) | 0.82 s (1t) |
| A11 | Anatomical outputs to MNI (`.h5` composite; Lanczos, MultiLabel, Gaussian) | 8 | `lx.ants.apply_transforms` | 3 of 8 outputs bit-identical; worst: 136,562 of 1,082,035 differ; max \|diff\| 0.146 (rel 0.15), NCC 1 - 2.5e-07 | **larmorx computes `--float 1` in double** (known; ANTs in double precision, rounded to float32, is larmorx bit for bit) | **fail** (max rel ≤ 0.0001) | 54.4 s (1t) | 33.7 s (1t) | 33.1 s (1t) |
| F01 | Head-motion estimation (mcflirt on the raw BOLD, to the HMC boldref) | 3 | `lx.mri.hmc` | not bit-exact (clean-room); RMS deviation 95th pct ≤ 0.023 mm, median Δparam ≤ 0.0025 mm / 0.0036° | clean-room algorithm; mcflirt reproduces itself exactly | pass (≤ 0.05 mm / 0.05°; FD r > 0.99 where max FD ≥ 0.5 mm) | 97.9 s (1t) | 85.8 s (1t) | 84.4 s (1t) |
| F03 | Coregistration reference: N4 of the HMC boldref | 3 | `lx.ants.n4_bias_field_correction` | 0 of 3 outputs bit-identical; worst: 138,062 of 139,264 differ; max \|diff\| 1.42 (rel 0.0023), NCC 1 - 6.8e-08 | glibc's maths, the ANTs build (fMRIPrep's 2.6.2 vs the oracle's 2.6.5; for N4 the oracle's GCC 11 code generation, [fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 3.16 s (1t) | 2.97 s (1t) | 0.45 s (1t) |
| F04 | Coregistration reference: MNI brain prior to the boldref (Linear) | 3 | `lx.ants.apply_transforms` | **bit-identical**: 3 of 3 outputs | – | pass (bit-identical) | 1.37 s (1t) | 1.92 s (1t) | 0.30 s (1t) |
| F05 | One-shot resampling to the boldref (HMC) | 3 | `lx.transforms.resample_series` | **bit-identical**: 3 of 3 outputs | – | pass (bit-identical) | 35.1 s (8t) | 31.7 s (8t) | 8.44 s (8t) |
| F06 | One-shot resampling to T1w (HMC + coregistration) | 3 | `lx.transforms.resample_series` | **bit-identical**: 3 of 3 outputs | – | pass (bit-identical) | 24.6 s (8t) | 25.6 s (8t) | 5.74 s (8t) |
| F07 | One-shot resampling to MNI res-2 (HMC + coregistration + `.h5` warp) | 3 | `lx.transforms.resample_series` | **bit-identical**: 3 of 3 outputs | – | pass (bit-identical) | 227 s (8t) | 222 s (8t) | 41.6 s (8t) |
| F08 | boldref and BOLD mask to T1w (Lanczos, MultiLabel) | 6 | `lx.ants.apply_transforms` | 3 of 6 outputs bit-identical; worst: 80,525 of 105,300 differ; max \|diff\| 0.00325 (rel 4.5e-06), NCC 1 - 5.8e-13 | **larmorx computes `--float 1` in double** (known; ANTs in double precision, rounded to float32, is larmorx bit for bit) | pass (max rel ≤ 0.0001) | 4.03 s (1t) | 5.03 s (1t) | 2.47 s (1t) |
| F09 | boldref and BOLD mask to MNI (Lanczos, MultiLabel, `.h5`) | 6 | `lx.ants.apply_transforms` | 3 of 6 outputs bit-identical; worst: 795,031 of 1,082,035 differ; max \|diff\| 0.0032 (rel 3.9e-06), NCC 1 - 3.3e-13 | **larmorx computes `--float 1` in double** (known; ANTs in double precision, rounded to float32, is larmorx bit for bit) | pass (max rel ≤ 0.0001) | 35.7 s (1t) | 30.5 s (1t) | 25.3 s (1t) |
| F10 | Confound masks to the boldref (Gaussian, MultiLabel, inverse) | 12 | `lx.ants.apply_transforms` | 3 of 12 outputs bit-identical; worst: 76 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (max rel ≤ 0.0001) | 6.46 s (1t) | 9.09 s (1t) | 5.38 s (1t) |
| F11 | Carpet-plot segmentation to the boldref (MultiLabel, `.h5`, `-u int`) | 3 | `lx.ants.apply_transforms` | **bit-identical**: 3 of 3 outputs | – | pass (bit-identical) | 6.52 s (1t) | 6.60 s (1t) | 5.02 s (1t) |
| F12 | Report: T1w and WM mask to the boldref (Lanczos, NearestNeighbor) | 6 | `lx.ants.apply_transforms` | 3 of 6 outputs bit-identical; worst: 108,206 of 139,264 differ; max \|diff\| 0.0245 (rel 9.5e-06), NCC 1 - 7.9e-12 | **larmorx computes `--float 1` in double** (known; ANTs in double precision, rounded to float32, is larmorx bit for bit) | pass (max rel ≤ 0.0001) | 2.89 s (1t) | 5.41 s (1t) | 1.33 s (1t) |

Times are sums over the stage's nodes, with the thread counts used. fMRIPrep's run times include nipype's own overhead for each node; the re-runs time the program alone (inside the image, without Apptainer's start-up), and larmorx's the call (the command line in-process for the ANTs programs and head-motion correction; computing and writing the output for the resampling).

**Not run in this oracle run:**

- A02 N4 before the anatomical template (ANTs defaults): runs only with more than one T1w; ds000005 sub-01 has one.
- F02 Slice-timing correction (`3dTshift`): no `SliceTiming` in ds000005's metadata, so fMRIPrep skips it.

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

## A01: Denoise the T1w (`DenoiseImage -n Rician`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `denoise0` | sub-01_T1w_noise_corrected.nii.gz | 4,900,422 of 5,898,240 differ; max \|diff\| 271 (rel 0.19), NCC 1 - 0.00011 | pass: NCC 0.9998888 | 163,367 of 5,898,240 differ; max \|diff\| 72.6 (rel 0.051), NCC 1 - 1.1e-06 | 53.9 s (8t) | 58.2 s | 3.24 s (8t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `denoise0` | sub-01_T1w_noise_corrected.nii.gz | 4,900,198 of 5,898,240 differ; max \|diff\| 271 (rel 0.19), NCC 1 - 0.00011 | 5,522 of 5,898,240 differ; max \|diff\| 0.000198 (rel 1.4e-07), NCC 1 - 3.3e-16 | same | same | explained | 5,522 of 5,898,240 differ; max \|diff\| 0.000198 (rel 1.4e-07), NCC 1 - 3.3e-16 |

## A03: Brain extraction: truncate intensities (`TruncateImageIntensity`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `truncate_images0` | sub-01_T1w_noise_corrected_maths.nii.gz | bit-identical | pass: bit-identical | same | 3.10 s (1t) | 1.70 s | 0.81 s (1t) |

## A04: Brain extraction: N4, first pass

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `inu_n40` | sub-01_T1w_noise_corrected_maths_corrected.nii.gz | 5,786,955 of 5,898,240 differ; max \|diff\| 0.00183 (rel 3.4e-06), NCC 1 - 4.5e-13 | pass: NCC 1.0000000 | same | 14.4 s (8t) | 13.1 s | 3.06 s (8t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `inu_n40` | sub-01_T1w_noise_corrected_maths_corrected.nii.gz | 5,896,800 of 5,898,240 differ; max \|diff\| 0.0155 (rel 2.9e-05), NCC 1 - 2.4e-11 | 5,815,826 of 5,898,240 differ; max \|diff\| 0.0094 (rel 1.7e-05), NCC 1 - 3.4e-12 | 5,857,587 of 5,898,240 differ; max \|diff\| 0.00925 (rel 1.7e-05), NCC 1 - 1.4e-11 | same | explained | 5,737,141 of 5,898,240 differ; max \|diff\| 0.00867 (rel 1.6e-05), NCC 1 - 3.4e-12 |

## A05: Brain extraction: Laplacian of the T1w and the template

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `lap_target` | sub-01_T1w_noise_corrected_maths_corrected_maths.nii.gz | bit-identical | pass: bit-identical | same | 4.22 s (1t) | 3.58 s | 0.97 s (1t) |
| `lap_tmpl` | tpl-OASIS30ANTs_res-01_T1w_maths.nii.gz | bit-identical | pass: bit-identical | same | 8.58 s (1t) | 6.40 s | 2.57 s (1t) |

## A06: Brain extraction: template priors to the T1w (Gaussian, inverse warp)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `map_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans.nii.gz | 1,588 of 5,898,240 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 | pass: max rel 6.7e-16 | same | 10.3 s (1t) | 8.23 s | 10.2 s (1t) |
| `map_wmmask` | summap_trans.nii.gz | 2,104 of 5,898,240 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 | pass: max rel 5.6e-16 | same | 10.4 s (1t) | 7.77 s | 10.3 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `map_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans.nii.gz | same | 1,588 of 5,898,240 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 | same | same | explained | 1,588 of 5,898,240 differ; max \|diff\| 6.66e-16 (rel 6.7e-16), NCC 1 |
| `map_wmmask` | summap_trans.nii.gz | same | 2,104 of 5,898,240 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 | same | same | explained | 2,104 of 5,898,240 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 |

## A07: Brain extraction: threshold the brain prior (`ThresholdImage`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `thr_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled.nii.gz | bit-identical | pass: bit-identical | same | 0.65 s (1t) | 0.67 s | 0.13 s (1t) |

## A08: Brain extraction: mask and label morphology (`ImageMath` MD/ME/GetLargestComponent/FillHoles/PadImage/addtozero, `MultiplyImages`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `02_pad_segm` | sub-01_T1w_noise_corrected_maths_corrected_labeled_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.49 s (1t) | 0.71 s | 0.19 s (1t) |
| `03_pad_mask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.46 s (1t) | 0.60 s | 0.13 s (1t) |
| `05_get_wm` | sub-01_T1w_noise_corrected_maths_corrected_labeled_maths_class-03_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.87 s (1t) | 0.83 s | 0.13 s (1t) |
| `06_get_gm` | sub-01_T1w_noise_corrected_maths_corrected_labeled_maths_class-02_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.86 s (1t) | 0.87 s | 0.14 s (1t) |
| `07_fill_gm` | sub-01_T1w_noise_corrected_maths_corrected_labeled_maths_class-02_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 5.06 s (1t) | 4.99 s | 0.17 s (1t) |
| `08_mult_gm` | 08_mult_gm.nii.gz | bit-identical | pass: bit-identical | same | 0.48 s (1t) | 0.65 s | 0.16 s (1t) |
| `09_relabel_wm` | 09_relabel_wm.nii.gz | bit-identical | pass: bit-identical | same | 0.45 s (1t) | 0.53 s | 0.10 s (1t) |
| `10_me_csf` | sub-01_T1w_noise_corrected_maths_corrected_labeled_maths_class-01_maths.nii.gz | bit-identical | pass: bit-identical | same | 2.08 s (1t) | 2.10 s | 0.42 s (1t) |
| `11_add_gm` | 08_mult_gm_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.78 s (1t) | 0.98 s | 0.14 s (1t) |
| `12_relabel_gm` | 12_relabel_gm.nii.gz | bit-identical | pass: bit-identical | same | 0.39 s (1t) | 0.57 s | 0.11 s (1t) |
| `13_add_gm_wm` | 09_relabel_wm_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.88 s (1t) | 0.90 s | 0.15 s (1t) |
| `15_add_7` | 09_relabel_wm_maths_class-03_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.65 s (1t) | 0.79 s | 0.14 s (1t) |
| `16_me_7` | 09_relabel_wm_maths_class-03_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.33 s (1t) | 1.46 s | 0.44 s (1t) |
| `17_comp_7` | 09_relabel_wm_maths_class-03_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.73 s (1t) | 0.86 s | 0.13 s (1t) |
| `18_md_7` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.79 s (1t) | 1.04 s | 0.19 s (1t) |
| `19_fill_7` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 4.54 s (1t) | 4.60 s | 0.18 s (1t) |
| `20_add_7_2` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.75 s (1t) | 0.95 s | 0.16 s (1t) |
| `21_md_7_2` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.84 s (1t) | 0.95 s | 0.20 s (1t) |
| `22_me_7_2` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.47 s (1t) | 1.53 s | 0.41 s (1t) |
| `23_depad_mask` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.37 s (1t) | 0.67 s | 0.15 s (1t) |
| `24_depad_segm` | 09_relabel_wm_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.41 s (1t) | 0.69 s | 0.15 s (1t) |
| `25_depad_gm` | 12_relabel_gm_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.50 s (1t) | 0.74 s | 0.16 s (1t) |
| `26_depad_wm` | 09_relabel_wm_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.48 s (1t) | 0.73 s | 0.17 s (1t) |
| `27_depad_csf` | sub-01_T1w_noise_corrected_maths_corrected_labeled_maths_class-01_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.40 s (1t) | 0.60 s | 0.16 s (1t) |
| `dil_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.78 s (1t) | 0.77 s | 0.15 s (1t) |
| `get_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.76 s (1t) | 0.75 s | 0.09 s (1t) |

## A09: Brain extraction: final N4 (weighted by the WM posterior)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `inu_n4_final0` | corrected | 5,367,471 of 5,898,240 differ; max \|diff\| 0.0789 (rel 2.9e-05), NCC 1 - 1.6e-11 | pass: NCC 1.0000000 | same | 9.52 s (8t) | 7.02 s | 1.13 s (8t) |
| `inu_n4_final0` | bias field | 5,885,813 of 5,898,240 differ; max \|diff\| 1.31e-05 (rel 1.4e-05), NCC 1 - 1.2e-11 | pass: NCC 1.0000000 | same | 9.52 s (8t) | 7.02 s | 1.13 s (8t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `inu_n4_final0` | corrected | 5,317,238 of 5,898,240 differ; max \|diff\| 0.0657 (rel 2.4e-05), NCC 1 - 5.3e-12 | 5,327,635 of 5,898,240 differ; max \|diff\| 0.0312 (rel 1.2e-05), NCC 1 - 3.3e-12 | 5,379,375 of 5,898,240 differ; max \|diff\| 0.0188 (rel 7e-06), NCC 1 - 4.7e-13 | same | explained | 3,371,210 of 5,898,240 differ; max \|diff\| 0.0173 (rel 6.4e-06), NCC 1 - 9.5e-13 |
| `inu_n4_final0` | bias field | 5,829,414 of 5,898,240 differ; max \|diff\| 1.07e-05 (rel 1.1e-05), NCC 1 - 4.3e-12 | 5,842,682 of 5,898,240 differ; max \|diff\| 5.78e-06 (rel 6.1e-06), NCC 1 - 4.8e-12 | 5,897,348 of 5,898,240 differ; max \|diff\| 3.1e-06 (rel 3.3e-06), NCC 1 - 1.1e-12 | same | explained | 3,859,631 of 5,898,240 differ; max \|diff\| 5.13e-06 (rel 5.4e-06), NCC 1 - 1.5e-12 |

## A10: Normalisation: truncate the moving T1w

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| MNI152NLin2009cAsym `trunc_mov` | sub-01_T1w_noise_corrected_corrected_xform_maths.nii.gz | bit-identical | pass: bit-identical | same | 2.61 s (1t) | 1.72 s | 0.82 s (1t) |

## A11: Anatomical outputs to MNI (`.h5` composite; Lanczos, MultiLabel, Gaussian)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| MNI152NLin2009cAsym.res2 `mask_std` | sub-01_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 8.97 s (1t) | 6.92 s | 7.61 s (1t) |
| MNI152NLin2009cAsym.res2 `t1w_std` | sub-01_desc-preproc_T1w_trans.nii.gz | 842,741 of 1,082,035 differ; max \|diff\| 0.0172 (rel 6.9e-06), NCC 1 - 1.4e-12 | pass: max rel 6.9e-06 | same | 8.27 s (1t) | 3.86 s | 1.73 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_dseg` | sub-01_dseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 11.7 s (1t) | 6.62 s | 8.03 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_mask` | sub-01_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 10.5 s (1t) | 6.07 s | 7.48 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_t1w` | sub-01_desc-preproc_T1w_masked_trans.nii.gz | 265,089 of 1,082,035 differ; max \|diff\| 0.0126 (rel 5.8e-06), NCC 1 - 4.5e-13 | pass: max rel 5.8e-06 | same | 7.31 s (1t) | 3.63 s | 1.65 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms0` | sub-01_label-GM_probseg_trans.nii.gz | 196,274 of 1,082,035 differ; max \|diff\| 0.109 (rel 0.11), NCC 1 - 2e-07 | **fail**: max rel 0.11 (6 of 1,082,035 voxels above 0.0001) | same | 2.72 s (1t) | 2.37 s | 2.18 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms1` | sub-01_label-WM_probseg_trans.nii.gz | 136,562 of 1,082,035 differ; max \|diff\| 0.146 (rel 0.15), NCC 1 - 2.5e-07 | **fail**: max rel 0.15 (4 of 1,082,035 voxels above 0.0001) | same | 2.55 s (1t) | 2.14 s | 2.18 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms2` | sub-01_label-CSF_probseg_trans.nii.gz | 148,382 of 1,082,035 differ; max \|diff\| 0.0391 (rel 0.039), NCC 1 - 6.6e-08 | **fail**: max rel 0.039 (5 of 1,082,035 voxels above 0.0001) | same | 2.34 s (1t) | 2.12 s | 2.25 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| MNI152NLin2009cAsym.res2 `t1w_std` | sub-01_desc-preproc_T1w_trans.nii.gz | same | same | same | 842,741 of 1,082,035 differ; max \|diff\| 0.0172 (rel 6.9e-06), NCC 1 - 1.4e-12 | known: float pipeline | 842,741 of 1,082,035 differ; max \|diff\| 0.0172 (rel 6.9e-06), NCC 1 - 1.4e-12 |
| MNI152NLin2009cAsym.res2 `anat2std_t1w` | sub-01_desc-preproc_T1w_masked_trans.nii.gz | same | same | same | 265,089 of 1,082,035 differ; max \|diff\| 0.0126 (rel 5.8e-06), NCC 1 - 4.5e-13 | known: float pipeline | 265,089 of 1,082,035 differ; max \|diff\| 0.0126 (rel 5.8e-06), NCC 1 - 4.5e-13 |
| MNI152NLin2009cAsym.res2 `anat2std_tpms0` | sub-01_label-GM_probseg_trans.nii.gz | same | same | same | 196,274 of 1,082,035 differ; max \|diff\| 0.109 (rel 0.11), NCC 1 - 2e-07 | known: float pipeline | 196,274 of 1,082,035 differ; max \|diff\| 0.109 (rel 0.11), NCC 1 - 2e-07 |
| MNI152NLin2009cAsym.res2 `anat2std_tpms1` | sub-01_label-WM_probseg_trans.nii.gz | same | same | same | 136,562 of 1,082,035 differ; max \|diff\| 0.146 (rel 0.15), NCC 1 - 2.5e-07 | known: float pipeline | 136,562 of 1,082,035 differ; max \|diff\| 0.146 (rel 0.15), NCC 1 - 2.5e-07 |
| MNI152NLin2009cAsym.res2 `anat2std_tpms2` | sub-01_label-CSF_probseg_trans.nii.gz | same | same | same | 148,382 of 1,082,035 differ; max \|diff\| 0.0391 (rel 0.039), NCC 1 - 6.6e-08 | known: float pipeline | 148,382 of 1,082,035 differ; max \|diff\| 0.0391 (rel 0.039), NCC 1 - 6.6e-08 |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| MNI152NLin2009cAsym.res2 `anat2std_dseg` | sub-01_dseg_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_dseg.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_mask` | sub-01_desc-brain_mask_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_t1w` | sub-01_desc-preproc_T1w_masked_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz | 265,089 of 1,082,035 differ; max \|diff\| 0.0126 (rel 5.8e-06), NCC 1 - 4.5e-13 |
| MNI152NLin2009cAsym.res2 `anat2std_tpms0` | sub-01_label-GM_probseg_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_label-GM_probseg.nii.gz | 196,274 of 1,082,035 differ; max \|diff\| 0.109 (rel 0.11), NCC 1 - 2e-07 |
| MNI152NLin2009cAsym.res2 `anat2std_tpms1` | sub-01_label-WM_probseg_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_label-WM_probseg.nii.gz | 136,562 of 1,082,035 differ; max \|diff\| 0.146 (rel 0.15), NCC 1 - 2.5e-07 |
| MNI152NLin2009cAsym.res2 `anat2std_tpms2` | sub-01_label-CSF_probseg_trans.nii.gz | out/sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_label-CSF_probseg.nii.gz | 148,382 of 1,082,035 differ; max \|diff\| 0.0391 (rel 0.039), NCC 1 - 6.6e-08 |

## F01: Head-motion estimation (mcflirt on the raw BOLD, to the HMC boldref)

larmorx runs `larmorx mri hmc` with mcflirt's arguments from `command.txt` (the raw BOLD, fMRIPrep's HMC boldref as `-reffile`, `-mats`). Matrices against mcflirt's, per volume (RMS deviation over a sphere of 80 mm about the field of view's centre, Jenkinson 1999; parameter differences about the same centre); mcflirt again in fMRIPrep's image, and FSL 6.0.7's mcflirt on this machine, against fMRIPrep's run:

| Run | Volumes | RMS deviation median / 95th pct / max (mm) | Median Δparam (mm / °) | Max Δparam (mm / °) | mcflirt re-run: identical volumes | FSL 6.0.7 mcflirt: identical volumes |
|---|---|---|---|---|---|---|
| run-01 `mcflirt` | 240 | 0.0101 / 0.0193 / 0.0283 | 0.0018 / 0.0012 | 0.0227 / 0.0293 | 240 of 240 | 240 of 240 |
| run-02 `mcflirt` | 240 | 0.0123 / 0.0230 / 0.0438 | 0.0017 / 0.0035 | 0.0390 / 0.0307 | 240 of 240 | 240 of 240 |
| run-03 `mcflirt` | 240 | 0.0136 / 0.0228 / 0.0263 | 0.0025 / 0.0036 | 0.0205 / 0.0274 | 240 of 240 | 240 of 240 |

**What fMRIPrep derives from them**, computed by fMRIPrep's own code in its image (niworkflows' `MCFLIRT2ITK`, fMRIPrep's `FSLMotionParams`, `FramewiseDisplacement`, `FSLRMSDeviation`) from larmorx's matrices, against the confounds file fMRIPrep wrote (`desc-confounds_timeseries.tsv`) and its `desc-hmc_xfm.txt`. The same code on mcflirt's matrices reproduces fMRIPrep's file (last column). mcflirt's band is its own lowest FD correlation when its input is perturbed slightly ([mri-hmc](mri-hmc.md), from the testdata's ds000005 HMC reference):

| Run | Median Δ motion params (mm / °) | Max Δ (mm / °) | FD r | Max FD (mm) | Mean FD fMRIPrep / larmorx (mm) | Max Δ RMSD (mm) | Max Δ ITK matrix entry (mm for translations) | Check: mcflirt's matrices |
|---|---|---|---|---|---|---|---|---|
| run-01 `mcflirt` | 0.0017 / 0.0012 | 0.0233 / 0.0292 | 0.8879 (band 0.7941) | 0.181 | 0.0728 / 0.0728 | 0.026 | 0.025 | same |
| run-02 `mcflirt` | 0.0012 / 0.0035 | 0.0390 / 0.0307 | 0.9262 (band 0.7941) | 0.252 | 0.0970 / 0.0975 | 0.027 | 0.039 | same |
| run-03 `mcflirt` | 0.0024 / 0.0036 | 0.0194 / 0.0274 | 0.9257 (band 0.7941) | 0.334 | 0.0984 / 0.0968 | 0.022 | 0.027 | same |

Times: fMRIPrep's node (its run), mcflirt re-run in fMRIPrep's image on this machine (one thread, as mcflirt runs), `larmorx mri hmc` with the same arguments (it also resamples and writes the series, as mcflirt does), and the Python API with `resample=False` (the matrices only, all fMRIPrep uses):

| Run | fMRIPrep | mcflirt re-run | larmorx CLI | larmorx API | larmorx API, all CPUs |
|---|---|---|---|---|---|
| run-01 `mcflirt` | 30.7 s | 26.5 s | 29.3 s (1t) | 26.1 s (1t) | 5.08 s (12t) |
| run-02 `mcflirt` | 33.4 s | 29.6 s | 26.5 s (1t) | 22.8 s (1t) | 4.93 s (12t) |
| run-03 `mcflirt` | 33.8 s | 29.7 s | 28.7 s (1t) | 25.4 s (1t) | 5.29 s (12t) |

## F03: Coregistration reference: N4 of the HMC boldref

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-01 `n4_correct` | sub-01_task-mixedgamblestask_run-01_desc-hmc_boldref_corrected.nii.gz | 71,885 of 139,264 differ; max \|diff\| 0.00287 (rel 3.3e-06), NCC 1 - 2.7e-13 | pass: NCC 1.0000000 | same | 0.97 s (1t) | 0.91 s | 0.10 s (1t) |
| run-02 `n4_correct` | sub-01_task-mixedgamblestask_run-02_desc-hmc_boldref_corrected.nii.gz | 138,050 of 139,264 differ; max \|diff\| 0.398 (rel 0.00049), NCC 1 - 4.6e-09 | pass: NCC 1.0000000 | same | 1.14 s (1t) | 0.96 s | 0.18 s (1t) |
| run-03 `n4_correct` | sub-01_task-mixedgamblestask_run-03_desc-hmc_boldref_corrected.nii.gz | 138,062 of 139,264 differ; max \|diff\| 1.42 (rel 0.0023), NCC 1 - 6.8e-08 | pass: NCC 0.9999999 | same | 1.05 s (1t) | 1.09 s | 0.17 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| run-01 `n4_correct` | sub-01_task-mixedgamblestask_run-01_desc-hmc_boldref_corrected.nii.gz | same | 76,229 of 139,264 differ; max \|diff\| 0.0114 (rel 1.3e-05), NCC 1 - 2.5e-12 | 76,320 of 139,264 differ; max \|diff\| 0.00873 (rel 1e-05), NCC 1 - 1.6e-12 | same | explained | 74,354 of 139,264 differ; max \|diff\| 0.00317 (rel 3.6e-06), NCC 1 - 2.5e-13 |
| run-02 `n4_correct` | sub-01_task-mixedgamblestask_run-02_desc-hmc_boldref_corrected.nii.gz | same | 137,886 of 139,264 differ; max \|diff\| 0.381 (rel 0.00047), NCC 1 - 1.9e-09 | 138,028 of 139,264 differ; max \|diff\| 0.307 (rel 0.00038), NCC 1 - 2.5e-09 | same | explained | 129,836 of 139,264 differ; max \|diff\| 0.00159 (rel 1.9e-06), NCC 1 - 4.1e-14 |
| run-03 `n4_correct` | sub-01_task-mixedgamblestask_run-03_desc-hmc_boldref_corrected.nii.gz | same | 138,068 of 139,264 differ; max \|diff\| 1.27 (rel 0.0021), NCC 1 - 5.6e-08 | 137,675 of 139,264 differ; max \|diff\| 0.152 (rel 0.00025), NCC 1 - 7.8e-10 | same | explained | 135,014 of 139,264 differ; max \|diff\| 0.0211 (rel 3.4e-05), NCC 1 - 5.1e-11 |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| run-01 `n4_correct` | sub-01_task-mixedgamblestask_run-01_desc-hmc_boldref_corrected.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-01_desc-coreg_boldref.nii.gz | 71,885 of 139,264 differ; max \|diff\| 0.00287 (rel 3.3e-06), NCC 1 - 2.7e-13 |
| run-02 `n4_correct` | sub-01_task-mixedgamblestask_run-02_desc-hmc_boldref_corrected.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-02_desc-coreg_boldref.nii.gz | 138,050 of 139,264 differ; max \|diff\| 0.398 (rel 0.00049), NCC 1 - 4.6e-09 |
| run-03 `n4_correct` | sub-01_task-mixedgamblestask_run-03_desc-hmc_boldref_corrected.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-03_desc-coreg_boldref.nii.gz | 138,062 of 139,264 differ; max \|diff\| 1.42 (rel 0.0023), NCC 1 - 6.8e-08 |

## F04: Coregistration reference: MNI brain prior to the boldref (Linear)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-01 `map_brainmask` | tpl-MNI152NLin2009cAsym_res-01_label-brain_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.44 s (1t) | 0.66 s | 0.10 s (1t) |
| run-02 `map_brainmask` | tpl-MNI152NLin2009cAsym_res-01_label-brain_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.47 s (1t) | 0.68 s | 0.11 s (1t) |
| run-03 `map_brainmask` | tpl-MNI152NLin2009cAsym_res-01_label-brain_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.46 s (1t) | 0.58 s | 0.09 s (1t) |

## F05: One-shot resampling to the boldref (HMC)

`lx.transforms.resample_series` with the `ResampleSeries` node's inputs from its report: the raw BOLD, the target grid, the transforms (`1` files), order 3, mode `grid-constant`, Jacobian False, float32 output. fMRIPrep's time includes writing the compressed output; larmorx's is shown as compute + write. fMRIPrep's re-run has the node's `num_threads` and `OMP_NUM_THREADS=1`, as fMRIPrep's image sets it: with more OpenBLAS threads, fMRIPrep's resampler corrupts some volumes ([fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx's result does not depend on any thread count.

| Run | Shape | larmorx vs fMRIPrep's node | larmorx vs the derivative | PLAN.md §11.3 | fMRIPrep re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|---|
| run-01 `boldref_bold` | 64×64×34×240 | bit-identical | bit-identical | pass: bit-identical | same | 12.5 s (8t) | 9.69 s | 1.18 s + 2.15 s (8t) |
| run-02 `boldref_bold` | 64×64×34×240 | bit-identical | bit-identical | pass: bit-identical | same | 11.2 s (8t) | 11.0 s | 1.30 s + 1.49 s (8t) |
| run-03 `boldref_bold` | 64×64×34×240 | bit-identical | bit-identical | pass: bit-identical | same | 11.4 s (8t) | 11.0 s | 1.29 s + 1.04 s (8t) |

## F06: One-shot resampling to T1w (HMC + coregistration)

`lx.transforms.resample_series` with the `ResampleSeries` node's inputs from its report: the raw BOLD, the target grid, the transforms (`2` files), order 3, mode `grid-constant`, Jacobian False, float32 output. fMRIPrep's time includes writing the compressed output; larmorx's is shown as compute + write. fMRIPrep's re-run has the node's `num_threads` and `OMP_NUM_THREADS=1`, as fMRIPrep's image sets it: with more OpenBLAS threads, fMRIPrep's resampler corrupts some volumes ([fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx's result does not depend on any thread count.

| Run | Shape | larmorx vs fMRIPrep's node | larmorx vs the derivative | PLAN.md §11.3 | fMRIPrep re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|---|
| run-01 `resample` | 50×54×39×240 | bit-identical | bit-identical | pass: bit-identical | same | 7.58 s (8t) | 8.12 s | 1.15 s + 0.64 s (8t) |
| run-02 `resample` | 50×54×39×240 | bit-identical | bit-identical | pass: bit-identical | same | 7.49 s (8t) | 8.71 s | 1.32 s + 0.76 s (8t) |
| run-03 `resample` | 50×54×39×240 | bit-identical | bit-identical | pass: bit-identical | same | 9.48 s (8t) | 8.75 s | 1.25 s + 0.63 s (8t) |

## F07: One-shot resampling to MNI res-2 (HMC + coregistration + `.h5` warp)

`lx.transforms.resample_series` with the `ResampleSeries` node's inputs from its report: the raw BOLD, the target grid, the transforms (`3` files), order 3, mode `grid-constant`, Jacobian False, float32 output. fMRIPrep's time includes writing the compressed output; larmorx's is shown as compute + write. fMRIPrep's re-run has the node's `num_threads` and `OMP_NUM_THREADS=1`, as fMRIPrep's image sets it: with more OpenBLAS threads, fMRIPrep's resampler corrupts some volumes ([fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx's result does not depend on any thread count.

| Run | Shape | larmorx vs fMRIPrep's node | larmorx vs the derivative | PLAN.md §11.3 | fMRIPrep re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|---|
| run-01 MNI152NLin2009cAsym.res2 `resample` | 97×115×97×240 | bit-identical | bit-identical | pass: bit-identical | same | 72.0 s (8t) | 69.6 s | 7.55 s + 6.09 s (8t) |
| run-02 MNI152NLin2009cAsym.res2 `resample` | 97×115×97×240 | bit-identical | bit-identical | pass: bit-identical | same | 74.7 s (8t) | 77.4 s | 7.95 s + 6.07 s (8t) |
| run-03 MNI152NLin2009cAsym.res2 `resample` | 97×115×97×240 | bit-identical | bit-identical | pass: bit-identical | same | 79.9 s (8t) | 75.4 s | 7.84 s + 6.08 s (8t) |

## F08: boldref and BOLD mask to T1w (Lanczos, MultiLabel)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-01 `resample_mask` | sub-01_task-mixedgamblestask_run-01_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.75 s (1t) | 0.89 s | 0.63 s (1t) |
| run-01 `resample_ref` | sub-01_task-mixedgamblestask_run-01_desc-coreg_boldref_trans.nii.gz | 78,926 of 105,300 differ; max \|diff\| 0.00319 (rel 3.5e-06), NCC 1 - 3.1e-13 | pass: max rel 3.5e-06 | same | 0.55 s (1t) | 0.75 s | 0.15 s (1t) |
| run-02 `resample_mask` | sub-01_task-mixedgamblestask_run-02_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.71 s (1t) | 0.93 s | 0.72 s (1t) |
| run-02 `resample_ref` | sub-01_task-mixedgamblestask_run-02_desc-coreg_boldref_trans.nii.gz | 80,525 of 105,300 differ; max \|diff\| 0.00325 (rel 4.5e-06), NCC 1 - 5.8e-13 | pass: max rel 4.5e-06 | same | 0.52 s (1t) | 0.80 s | 0.15 s (1t) |
| run-03 `resample_mask` | sub-01_task-mixedgamblestask_run-03_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.72 s (1t) | 0.95 s | 0.68 s (1t) |
| run-03 `resample_ref` | sub-01_task-mixedgamblestask_run-03_desc-coreg_boldref_trans.nii.gz | 79,688 of 105,300 differ; max \|diff\| 0.00211 (rel 3.2e-06), NCC 1 - 4e-13 | pass: max rel 3.2e-06 | same | 0.77 s (1t) | 0.71 s | 0.15 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| run-01 `resample_ref` | sub-01_task-mixedgamblestask_run-01_desc-coreg_boldref_trans.nii.gz | same | same | same | 78,926 of 105,300 differ; max \|diff\| 0.00319 (rel 3.5e-06), NCC 1 - 3.1e-13 | known: float pipeline | 78,926 of 105,300 differ; max \|diff\| 0.00319 (rel 3.5e-06), NCC 1 - 3.1e-13 |
| run-02 `resample_ref` | sub-01_task-mixedgamblestask_run-02_desc-coreg_boldref_trans.nii.gz | same | same | same | 80,525 of 105,300 differ; max \|diff\| 0.00325 (rel 4.5e-06), NCC 1 - 5.8e-13 | known: float pipeline | 80,525 of 105,300 differ; max \|diff\| 0.00325 (rel 4.5e-06), NCC 1 - 5.8e-13 |
| run-03 `resample_ref` | sub-01_task-mixedgamblestask_run-03_desc-coreg_boldref_trans.nii.gz | same | same | same | 79,688 of 105,300 differ; max \|diff\| 0.00211 (rel 3.2e-06), NCC 1 - 4e-13 | known: float pipeline | 79,688 of 105,300 differ; max \|diff\| 0.00211 (rel 3.2e-06), NCC 1 - 4e-13 |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| run-01 `resample_mask` | sub-01_task-mixedgamblestask_run-01_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-01_space-T1w_desc-brain_mask.nii.gz | bit-identical |
| run-01 `resample_ref` | sub-01_task-mixedgamblestask_run-01_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-01_space-T1w_boldref.nii.gz | 78,926 of 105,300 differ; max \|diff\| 0.00319 (rel 3.5e-06), NCC 1 - 3.1e-13 |
| run-02 `resample_mask` | sub-01_task-mixedgamblestask_run-02_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-02_space-T1w_desc-brain_mask.nii.gz | bit-identical |
| run-02 `resample_ref` | sub-01_task-mixedgamblestask_run-02_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-02_space-T1w_boldref.nii.gz | 80,525 of 105,300 differ; max \|diff\| 0.00325 (rel 4.5e-06), NCC 1 - 5.8e-13 |
| run-03 `resample_mask` | sub-01_task-mixedgamblestask_run-03_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-03_space-T1w_desc-brain_mask.nii.gz | bit-identical |
| run-03 `resample_ref` | sub-01_task-mixedgamblestask_run-03_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-03_space-T1w_boldref.nii.gz | 79,688 of 105,300 differ; max \|diff\| 0.00211 (rel 3.2e-06), NCC 1 - 4e-13 |

## F09: boldref and BOLD mask to MNI (Lanczos, MultiLabel, `.h5`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-01 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-01_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 6.49 s (1t) | 5.82 s | 6.85 s (1t) |
| run-01 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-01_desc-coreg_boldref_trans.nii.gz | 790,371 of 1,082,035 differ; max \|diff\| 0.00397 (rel 4.1e-06), NCC 1 - 2.9e-13 | pass: max rel 4.1e-06 | same | 5.03 s (1t) | 4.40 s | 1.46 s (1t) |
| run-02 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-02_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 7.21 s (1t) | 5.55 s | 6.76 s (1t) |
| run-02 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-02_desc-coreg_boldref_trans.nii.gz | 795,031 of 1,082,035 differ; max \|diff\| 0.0032 (rel 3.9e-06), NCC 1 - 3.3e-13 | pass: max rel 3.9e-06 | same | 5.79 s (1t) | 4.55 s | 1.58 s (1t) |
| run-03 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-03_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 6.20 s (1t) | 5.62 s | 7.10 s (1t) |
| run-03 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-03_desc-coreg_boldref_trans.nii.gz | 792,590 of 1,082,035 differ; max \|diff\| 0.00247 (rel 3.8e-06), NCC 1 - 2.6e-13 | pass: max rel 3.8e-06 | same | 5.02 s (1t) | 4.52 s | 1.51 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| run-01 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-01_desc-coreg_boldref_trans.nii.gz | same | same | same | 790,371 of 1,082,035 differ; max \|diff\| 0.00397 (rel 4.1e-06), NCC 1 - 2.9e-13 | known: float pipeline | 790,371 of 1,082,035 differ; max \|diff\| 0.00397 (rel 4.1e-06), NCC 1 - 2.9e-13 |
| run-02 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-02_desc-coreg_boldref_trans.nii.gz | same | same | same | 795,031 of 1,082,035 differ; max \|diff\| 0.0032 (rel 3.9e-06), NCC 1 - 3.3e-13 | known: float pipeline | 795,031 of 1,082,035 differ; max \|diff\| 0.0032 (rel 3.9e-06), NCC 1 - 3.3e-13 |
| run-03 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-03_desc-coreg_boldref_trans.nii.gz | same | same | same | 792,590 of 1,082,035 differ; max \|diff\| 0.00247 (rel 3.8e-06), NCC 1 - 2.6e-13 | known: float pipeline | 792,590 of 1,082,035 differ; max \|diff\| 0.00247 (rel 3.8e-06), NCC 1 - 2.6e-13 |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| run-01 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-01_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-01_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| run-01 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-01_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-01_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz | 790,371 of 1,082,035 differ; max \|diff\| 0.00397 (rel 4.1e-06), NCC 1 - 2.9e-13 |
| run-02 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-02_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-02_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| run-02 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-02_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-02_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz | 795,031 of 1,082,035 differ; max \|diff\| 0.0032 (rel 3.9e-06), NCC 1 - 3.3e-13 |
| run-03 MNI152NLin2009cAsym.res2 `resample_mask` | sub-01_task-mixedgamblestask_run-03_desc-brain_mask_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-03_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| run-03 MNI152NLin2009cAsym.res2 `resample_ref` | sub-01_task-mixedgamblestask_run-03_desc-coreg_boldref_trans.nii.gz | out/sub-01/func/sub-01_task-mixedgamblestask_run-03_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz | 792,590 of 1,082,035 differ; max \|diff\| 0.00247 (rel 3.8e-06), NCC 1 - 2.6e-13 |

## F10: Confound masks to the boldref (Gaussian, MultiLabel, inverse)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-01 `acc_msk_tfm0` | sub-01_label-CSF_probseg_trans.nii.gz | 76 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | pass: max rel 2.2e-16 | same | 0.45 s (1t) | 0.66 s | 0.26 s (1t) |
| run-01 `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | 10 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | pass: max rel 2.2e-16 | same | 0.43 s (1t) | 0.62 s | 0.24 s (1t) |
| run-01 `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | 13 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | pass: max rel 2.2e-16 | same | 0.39 s (1t) | 0.60 s | 0.28 s (1t) |
| run-01 `t1w_mask_tfm` | sub-01_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.96 s (1t) | 1.07 s | 0.94 s (1t) |
| run-02 `acc_msk_tfm0` | sub-01_label-CSF_probseg_trans.nii.gz | 87 of 139,264 differ; max \|diff\| 3.33e-16 (rel 3.3e-16), NCC 1 | pass: max rel 3.3e-16 | same | 0.47 s (1t) | 0.67 s | 0.28 s (1t) |
| run-02 `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | 9 of 139,264 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 | pass: max rel 1.1e-16 | same | 0.37 s (1t) | 0.67 s | 0.28 s (1t) |
| run-02 `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | 11 of 139,264 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 | pass: max rel 1.1e-16 | same | 0.41 s (1t) | 0.65 s | 0.26 s (1t) |
| run-02 `t1w_mask_tfm` | sub-01_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.88 s (1t) | 1.09 s | 1.01 s (1t) |
| run-03 `acc_msk_tfm0` | sub-01_label-CSF_probseg_trans.nii.gz | 89 of 139,264 differ; max \|diff\| 4.44e-16 (rel 4.4e-16), NCC 1 | pass: max rel 4.4e-16 | same | 0.43 s (1t) | 0.65 s | 0.26 s (1t) |
| run-03 `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | 5 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | pass: max rel 2.2e-16 | same | 0.43 s (1t) | 0.64 s | 0.28 s (1t) |
| run-03 `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | 6 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | pass: max rel 2.2e-16 | same | 0.43 s (1t) | 0.65 s | 0.25 s (1t) |
| run-03 `t1w_mask_tfm` | sub-01_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.79 s (1t) | 1.12 s | 1.04 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| run-01 `acc_msk_tfm0` | sub-01_label-CSF_probseg_trans.nii.gz | same | 76 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | same | same | explained | 76 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 |
| run-01 `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | same | 10 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | same | same | explained | 10 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 |
| run-01 `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | same | 13 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | same | same | explained | 13 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 |
| run-02 `acc_msk_tfm0` | sub-01_label-CSF_probseg_trans.nii.gz | same | 87 of 139,264 differ; max \|diff\| 3.33e-16 (rel 3.3e-16), NCC 1 | same | same | explained | 87 of 139,264 differ; max \|diff\| 3.33e-16 (rel 3.3e-16), NCC 1 |
| run-02 `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | same | 9 of 139,264 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 | same | same | explained | 9 of 139,264 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 |
| run-02 `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | same | 11 of 139,264 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 | same | same | explained | 11 of 139,264 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 |
| run-03 `acc_msk_tfm0` | sub-01_label-CSF_probseg_trans.nii.gz | same | 89 of 139,264 differ; max \|diff\| 4.44e-16 (rel 4.4e-16), NCC 1 | same | same | explained | 89 of 139,264 differ; max \|diff\| 4.44e-16 (rel 4.4e-16), NCC 1 |
| run-03 `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | same | 5 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | same | same | explained | 5 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 |
| run-03 `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | same | 6 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | same | same | explained | 6 of 139,264 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 |

## F11: Carpet-plot segmentation to the boldref (MultiLabel, `.h5`, `-u int`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-01 `resample_parc` | tpl-MNI152NLin2009cAsym_res-01_desc-carpet_dseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 1.92 s (1t) | 2.00 s | 1.58 s (1t) |
| run-02 `resample_parc` | tpl-MNI152NLin2009cAsym_res-01_desc-carpet_dseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 2.42 s (1t) | 2.01 s | 1.66 s (1t) |
| run-03 `resample_parc` | tpl-MNI152NLin2009cAsym_res-01_desc-carpet_dseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 2.17 s (1t) | 2.59 s | 1.78 s (1t) |

## F12: Report: T1w and WM mask to the boldref (Lanczos, NearestNeighbor)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| run-01 `boldref_wm` | sub-01_dseg_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.29 s (1t) | 0.65 s | 0.04 s (1t) |
| run-01 `t1w_boldref` | sub-01_desc-preproc_T1w_trans.nii.gz | 108,206 of 139,264 differ; max \|diff\| 0.0245 (rel 9.5e-06), NCC 1 - 7.9e-12 | pass: max rel 9.5e-06 | same | 0.82 s (1t) | 1.18 s | 0.32 s (1t) |
| run-02 `boldref_wm` | sub-01_dseg_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.24 s (1t) | 0.60 s | 0.12 s (1t) |
| run-02 `t1w_boldref` | sub-01_desc-preproc_T1w_trans.nii.gz | 107,746 of 139,264 differ; max \|diff\| 0.0187 (rel 7.2e-06), NCC 1 - 4.9e-12 | pass: max rel 7.2e-06 | same | 0.68 s (1t) | 1.08 s | 0.35 s (1t) |
| run-03 `boldref_wm` | sub-01_dseg_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.25 s (1t) | 0.68 s | 0.06 s (1t) |
| run-03 `t1w_boldref` | sub-01_desc-preproc_T1w_trans.nii.gz | 107,778 of 139,264 differ; max \|diff\| 0.0211 (rel 8.2e-06), NCC 1 - 5.2e-12 | pass: max rel 8.2e-06 | same | 0.63 s (1t) | 1.23 s | 0.44 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| run-01 `t1w_boldref` | sub-01_desc-preproc_T1w_trans.nii.gz | same | same | same | 108,206 of 139,264 differ; max \|diff\| 0.0245 (rel 9.5e-06), NCC 1 - 7.9e-12 | known: float pipeline | 108,206 of 139,264 differ; max \|diff\| 0.0245 (rel 9.5e-06), NCC 1 - 7.9e-12 |
| run-02 `t1w_boldref` | sub-01_desc-preproc_T1w_trans.nii.gz | same | same | same | 107,746 of 139,264 differ; max \|diff\| 0.0187 (rel 7.2e-06), NCC 1 - 4.9e-12 | known: float pipeline | 107,746 of 139,264 differ; max \|diff\| 0.0187 (rel 7.2e-06), NCC 1 - 4.9e-12 |
| run-03 `t1w_boldref` | sub-01_desc-preproc_T1w_trans.nii.gz | same | same | same | 107,778 of 139,264 differ; max \|diff\| 0.0211 (rel 8.2e-06), NCC 1 - 5.2e-12 | known: float pipeline | 107,778 of 139,264 differ; max \|diff\| 0.0211 (rel 8.2e-06), NCC 1 - 5.2e-12 |

## Reproduce

```bash
scripts/run_fmriprep_oracle.sh ~/.cache/larmorx-testdata/data/openneuro/ds000005 01 ds000005-sub-01-noreconall --fs-no-reconall   # the oracle run (about an hour)
scripts/build_ants_oracle.sh && scripts/build_crmath_preload.sh   # the references
python -m larmorx_validation pipeline-stages --oracle <workspace>/oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall --out docs/validation
```

larmorx's outputs and the reference runs are kept in `<workspace>/oracles/compare/fmriprep-25.2.6/ds000005-sub-01-noreconall/<node path>/<run>/` (`larmorx`, `container-<n>t`, `container-1t-cr`, `ants-2.6.5-1t`, `ants-2.6.5-1t-cr`, `ants-2.6.5-1t-cr-double`, `fsl-6.0.7`, `confounds-*`), each with `run.json` (arguments, environment, exit status, time). `--reuse` reuses the one-thread reference runs of an earlier invocation; `--stage A01 F05` limits the comparison; `--render` writes this page again from the JSON record, without running anything.

