# Stage by stage: larmorx against fMRIPrep 25.2.6

larmorx's tools run on fMRIPrep's **own inputs and parameters** for each step it already implements, read from nipype's working directory (`command.txt`, `_report/report.rst`), and their outputs are compared with what fMRIPrep produced at that step. So every stage is judged on its own, with no error carried over from earlier stages. **The headline numbers are against fMRIPrep as it ran**; where they differ, the step is run again in fMRIPrep's own container and with the repository's oracle builds to find out why.

- **Oracle:** fMRIPrep 25.2.6, image `nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e` (Apptainer), run `ds005752-sub-ON01016-noreconall`: `/data /out participant --participant-label ON01016 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 8 --omp-nthreads 8 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall`. Input: ds005752 sub-ON01016: 1 T1w, 2 BOLD runs of 10, 200 volumes, slice timing (2 runs); field maps: 1 `fieldmap`, 1 `magnitude`, **not used by fMRIPrep** (no estimator built; its log says "No fieldmap correction").
- **Tools inside fMRIPrep's image:** ANTs 2.6.2.dev1-gff61a3d (conda-forge `ants` 2.6.2) on ITK 5.4.4; FSL `mcflirt` 2111.0 (`newimage` 2501.4); AFNI 25.2.09; nitransforms 25.1.0, SciPy 1.15.2, numpy 2.2.6 on OpenBLAS 0.3.30; glibc 2.35.
- **larmorx:** 0.0.1 (58230ae137bd), on Linux x86_64, Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs.
- **References for the causes:** fMRIPrep's own tools re-run in its image, at its thread count and at one thread, as built and with CORE-MATH's correctly rounded functions preloaded (`scripts/build_crmath_preload.sh`; the image has this machine's glibc 2.35); ANTs 2.6.5 on ITK 5.4.5 built by `scripts/build_ants_oracle.sh` with GCC 14.3 (one thread, as built and with CORE-MATH preloaded); FSL 6.0.7's `mcflirt` on this machine; AFNI 25.2.09 built by `scripts/build_afni_oracle.sh` (one thread, `HOME` an empty directory). 3dTshift runs in both larmorx implementations: the bit-exact replica (`larmorx-gpl afni 3dTshift`, GPL-3.0-or-later, the `crates-gpl` release build, which `larmorx afni 3dTshift` runs as a separate program) and the clean-room original (`LARMORX_IMPLEMENTATION=original`).
- **Generated:** 2026-10-12, with `python -m larmorx_validation pipeline-stages --oracle <workspace>/oracles/fmriprep-25.2.6/ds005752-sub-ON01016-noreconall --out docs/validation --reuse --name fmriprep-stages-ds005752`. Times are **on a shared machine** (load average 4.5 at the start; other jobs: 0), so they are indicative only; fMRIPrep's own times come from its run (8 jobs at once on a machine shared with a development agent, see `RUN_INFO.txt`), the re-runs of fMRIPrep's tools from this invocation, at fMRIPrep's thread counts.

## Results

**65 of 78 compared nodes are bit-identical to fMRIPrep's own output** (plus 2 head-motion corrections, which larmorx does with a clean-room algorithm and cannot match bit for bit). The other differences are explained by fMRIPrep's thread count, glibc's maths or the ANTs build (fMRIPrep's conda-forge 2.6.2 against the repository's build of 2.6.5; with those removed, larmorx is bit-identical). Slice timing (3dTshift) counts by the bit-exact replica, which `larmorx afni 3dTshift` runs when it is installed: 2 of 2 bit-identical, headers included; the clean-room original is bit-identical in 0 of 2 and within [afni-tshift.md](afni-tshift.md)'s thresholds in 2 of 2.

| Stage | Step | Nodes | larmorx | Against fMRIPrep as it ran | Why they differ | PLAN.md §11.3 | fMRIPrep (its run) | fMRIPrep's tool re-run | larmorx |
|---|---|---|---|---|---|---|---|---|---|
| A01 | Denoise the T1w (`DenoiseImage -n Rician`) | 1 | `lx.ants.denoise_image` | 0 of 1 outputs bit-identical; worst: 4,056,645 of 12,845,056 differ; max \|diff\| 285 (rel 0.13), NCC 1 - 3.8e-05 | not reproducible run to run, fMRIPrep's thread count, glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 40.7 s (8t) | 37.8 s (8t) | 3.61 s (8t) |
| A03 | Brain extraction: truncate intensities (`TruncateImageIntensity`) | 1 | `lx.ants.image_math` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 2.77 s (1t) | 2.05 s (1t) | 0.82 s (1t) |
| A04 | Brain extraction: N4, first pass | 1 | `lx.ants.n4_bias_field_correction` | 0 of 1 outputs bit-identical; worst: 12,845,056 of 12,845,056 differ; max \|diff\| 0.0178 (rel 9.1e-06), NCC 1 - 1.3e-12 | fMRIPrep's thread count, glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 34.7 s (8t) | 29.7 s (8t) | 7.21 s (8t) |
| A05 | Brain extraction: Laplacian of the T1w and the template | 2 | `lx.ants.image_math` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 15.1 s (1t) | 11.9 s (1t) | 3.56 s (1t) |
| A06 | Brain extraction: template priors to the T1w (Gaussian, inverse warp) | 2 | `lx.ants.apply_transforms` | 0 of 2 outputs bit-identical; worst: 2,053 of 12,845,056 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 | glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (max rel ≤ 0.0001) | 34.2 s (1t) | 27.5 s (1t) | 27.9 s (1t) |
| A07 | Brain extraction: threshold the brain prior (`ThresholdImage`) | 1 | `lx.ants.threshold_image` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 0.87 s (1t) | 0.94 s (1t) | 0.26 s (1t) |
| A08 | Brain extraction: mask and label morphology (`ImageMath` MD/ME/GetLargestComponent/FillHoles/PadImage/addtozero, `MultiplyImages`) | 26 | `lx.ants.image_math` | **bit-identical**: 26 of 26 outputs | – | pass (bit-identical) | 57.6 s (1t) | 51.2 s (1t) | 9.99 s (1t) |
| A09 | Brain extraction: final N4 (weighted by the WM posterior) | 1 | `lx.ants.n4_bias_field_correction` | 0 of 2 outputs bit-identical; worst: 4,721,646 of 12,845,056 differ; max \|diff\| 0.134 (rel 2.1e-05), NCC 1 - 6.2e-11 | fMRIPrep's thread count, glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 21.9 s (8t) | 13.2 s (8t) | 2.09 s (8t) |
| A10 | Normalisation: truncate the moving T1w | 1 | `lx.ants.image_math` | **bit-identical**: 1 of 1 outputs | – | pass (bit-identical) | 2.92 s (1t) | 2.10 s (1t) | 0.79 s (1t) |
| A11 | Anatomical outputs to MNI (`.h5` composite; Lanczos, MultiLabel, Gaussian) | 8 | `lx.ants.apply_transforms` | **bit-identical**: 8 of 8 outputs | – | pass (bit-identical) | 58.5 s (1t) | 37.1 s (1t) | 31.8 s (1t) |
| F01 | Head-motion estimation (mcflirt on the raw BOLD, to the HMC boldref) | 2 | `lx.mri.hmc` | not bit-exact (clean-room); RMS deviation 95th pct ≤ 0.024 mm, median Δparam ≤ 0.0027 mm / 0.0042° | clean-room algorithm; mcflirt reproduces itself exactly | pass (≤ 0.05 mm / 0.05°; FD r > 0.99 where max FD ≥ 0.5 mm, or mcflirt's own band on the run) | 38.8 s (1t) | 34.8 s (1t) | 33.8 s (1t) |
| F02 | Slice-timing correction (`3dTshift`) | 2 | `lx.afni.tshift` | replica: **2 of 2 bit-identical**; clean-room original: 0 of 2 bit-identical (int16: 86 of 39,321,600 stored values differ, max \|diff\| 1 unit; int16: 1 of 1,966,080 stored values differ, max \|diff\| 1 unit) | original: clean-room, within [afni-tshift.md](afni-tshift.md)'s thresholds (its Fourier shift computed in double precision, AFNI's in float32) | pass (float32 max rel ≤ 1e-05, integers ≤ 1 stored unit, the same type and scale factor) | 10.5 s (1t) | 9.77 s (1t) | 3.03 s (1t) |
| F03 | Coregistration reference: N4 of the HMC boldref | 2 | `lx.ants.n4_bias_field_correction` | 0 of 2 outputs bit-identical; worst: 193,141 of 196,608 differ; max \|diff\| 3.3 (rel 0.00069), NCC 1 - 5.4e-09 | glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (NCC > 0.995) | 2.58 s (1t) | 2.52 s (1t) | 0.43 s (1t) |
| F04 | Coregistration reference: MNI brain prior to the boldref (Linear) | 2 | `lx.ants.apply_transforms` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 0.82 s (1t) | 1.21 s (1t) | 0.19 s (1t) |
| F05 | One-shot resampling to the boldref (HMC) | 2 | `lx.transforms.resample_series` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 13.9 s (8t) | 14.4 s (8t) | 2.87 s (8t) |
| F06 | One-shot resampling to T1w (HMC + coregistration) | 2 | `lx.transforms.resample_series` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 6.60 s (8t) | 8.73 s (8t) | 1.85 s (8t) |
| F07 | One-shot resampling to MNI res-2 (HMC + coregistration + `.h5` warp) | 2 | `lx.transforms.resample_series` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 69.4 s (8t) | 69.2 s (8t) | 14.0 s (8t) |
| F08 | boldref and BOLD mask to T1w (Lanczos, MultiLabel) | 4 | `lx.ants.apply_transforms` | **bit-identical**: 4 of 4 outputs | – | pass (bit-identical) | 2.98 s (1t) | 3.47 s (1t) | 1.49 s (1t) |
| F09 | boldref and BOLD mask to MNI (Lanczos, MultiLabel, `.h5`) | 4 | `lx.ants.apply_transforms` | **bit-identical**: 4 of 4 outputs | – | pass (bit-identical) | 28.0 s (1t) | 23.5 s (1t) | 16.8 s (1t) |
| F10 | Confound masks to the boldref (Gaussian, MultiLabel, inverse) | 8 | `lx.ants.apply_transforms` | 2 of 8 outputs bit-identical; worst: 67 of 196,608 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | glibc's maths; larmorx = ANTs 2.6.5 with correctly rounded maths, bit for bit | pass (max rel ≤ 0.0001) | 6.18 s (1t) | 8.09 s (1t) | 4.34 s (1t) |
| F11 | Carpet-plot segmentation to the boldref (MultiLabel, `.h5`, `-u int`) | 2 | `lx.ants.apply_transforms` | **bit-identical**: 2 of 2 outputs | – | pass (bit-identical) | 4.66 s (1t) | 4.08 s (1t) | 3.10 s (1t) |
| F12 | Report: T1w and WM mask to the boldref (Lanczos, NearestNeighbor) | 4 | `lx.ants.apply_transforms` | **bit-identical**: 4 of 4 outputs | – | pass (bit-identical) | 2.38 s (1t) | 3.38 s (1t) | 0.88 s (1t) |

Times are sums over the stage's nodes, with the thread counts used. fMRIPrep's run times include nipype's own overhead for each node; the re-runs time the program alone (inside the image, without Apptainer's start-up), and larmorx's the call (the command line in-process for the ANTs programs and head-motion correction; computing and writing the output for the resampling).

**Not run in this oracle run:**

- A02 N4 before the anatomical template (ANTs defaults): runs only with more than one T1w; this subject has one.
- S01 Field map: N4 of the field-map reference (sdcflows' brain extraction): no field map estimated (no estimator assigned to a BOLD run).
- S02 Unwarp the HMC boldref for coregistration (`ResampleSeries` with the field map): no field map applied to a BOLD run.
- S03 Field-map report: the field map and its reference to the boldref (Lanczos, `--float 1`): no field map applied to a BOLD run.

## What larmorx cannot run yet

Every other node of the run that runs a neuroimaging program, or a Python step larmorx will need, by the tool it needs. The time is fMRIPrep's (its run, 8 jobs at once). These are the next tools to port (CLAUDE.md, "Next").

| Tool | fMRIPrep nodes | fMRIPrep time | What fMRIPrep uses it for | larmorx milestone |
|---|---|---|---|---|
| antsRegistration | 4 | 2367 s | rigid/affine (BOLD premask) and SyN (brain extraction, MNI normalisation) | A4 (linear), A5 (SyN) |
| antsAI | 3 | 92.8 s | affine initialisation by rotation search | A4 |
| Atropos | 1 | 22.6 s | K-means + MRF segmentation in brain extraction | A6 |
| 3dvolreg | 2 | 29.6 s | aligns the reference volumes before the median (niworkflows' RobustAverage, inside the HMC-reference node) | AFNI |
| 3dUnifize | 2 | 144 s | T2-contrast intensity unifying of the boldref | AFNI |
| 3dAutomask | 2 | 0.38 s | BOLD brain mask, second pass | AFNI |
| BET | 2 | 2.94 s | BOLD brain mask, first pass | FSL (clean-room + `larmorx-nc`) |
| FAST | 1 | 65.3 s | GM/WM/CSF segmentation and probability maps | FSL |
| FLIRT-BBR | 2 | 112 s | boundary-based BOLD-to-T1w registration | FSL |
| fslmaths | 2 | 0.24 s | mask product of the two BOLD masks | FSL (trivial) |
| mri_coreg | 2 | 55.2 s | BOLD-to-T1w initialisation | FreeSurfer |
| lta_convert | 6 | 0.59 s | transform format conversion (LTA <-> FSL) | FreeSurfer (trivial; nitransforms has it) |
| topup | – | – | PEPOLAR field-map estimation (sdcflows' PEPOLAR workflow) | FSL (clean-room + `larmorx-nc`) |
| prelude | – | – | phase unwrapping of the phase-difference field map | FSL (clean-room + `larmorx-nc`) |
| mri_robust_template | – | – | anatomical template of several T1w images | FreeSurfer |
| field-map B-splines | – | – | sdcflows' B-spline fit of the field (`BSplineApprox`, `TOPUPCoeffReorient`) and fMRIPrep's `ReconstructFieldmap` (the field from its coefficients on a target grid, the input of `ResampleSeries`) | larmorprepx (Rust: B-spline evaluation) |
| field-map preparation | – | – | sdcflows' Python steps: phase difference to Hz (`calc_phdiff`, `phmap2rads`, `compfmap`), PEPOLAR series sorted, padded, regridded and averaged, the field-map reference's brain mask, readout times and distortion parameters | larmorprepx (numpy) |
| confounds | 22 | 25.9 s | DVARS, CompCor (a/t/crown), global signals, expansion, spikes (fMRIPrep's Python); FD, RMSD and motion parameters are computed here from larmorx's HMC | larmorprepx (numpy) |

## A01: Denoise the T1w (`DenoiseImage -n Rician`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `denoise0` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected.nii.gz | 4,056,645 of 12,845,056 differ; max \|diff\| 285 (rel 0.13), NCC 1 - 3.8e-05 | pass: NCC 0.9999616 | 73,849 of 12,845,056 differ; max \|diff\| 8.7 (rel 0.004), NCC 1 - 4.4e-09 | 40.7 s (8t) | 37.8 s | 3.61 s (8t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `denoise0` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected.nii.gz | 4,056,327 of 12,845,056 differ; max \|diff\| 285 (rel 0.13), NCC 1 - 3.8e-05 | 4,286 of 12,845,056 differ; max \|diff\| 0.000397 (rel 1.8e-07), NCC 1 - 3.3e-16 | same | same | explained | 4,286 of 12,845,056 differ; max \|diff\| 0.000397 (rel 1.8e-07), NCC 1 - 3.3e-16 |

## A03: Brain extraction: truncate intensities (`TruncateImageIntensity`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `truncate_images0` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected_maths.nii.gz | bit-identical | pass: bit-identical | same | 2.77 s (1t) | 2.05 s | 0.82 s (1t) |

## A04: Brain extraction: N4, first pass

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `inu_n40` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected_maths_corrected.nii.gz | 12,845,056 of 12,845,056 differ; max \|diff\| 0.0178 (rel 9.1e-06), NCC 1 - 1.3e-12 | pass: NCC 1.0000000 | same | 34.7 s (8t) | 29.7 s | 7.21 s (8t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `inu_n40` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected_maths_corrected.nii.gz | 12,845,056 of 12,845,056 differ; max \|diff\| 0.00903 (rel 4.6e-06), NCC 1 - 4.6e-13 | 12,845,056 of 12,845,056 differ; max \|diff\| 0.00989 (rel 5.1e-06), NCC 1 - 6.7e-13 | same | same | explained | 12,845,056 of 12,845,056 differ; max \|diff\| 0.00989 (rel 5.1e-06), NCC 1 - 6.7e-13 |

## A05: Brain extraction: Laplacian of the T1w and the template

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `lap_target` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected_maths_corrected_maths.nii.gz | bit-identical | pass: bit-identical | same | 6.64 s (1t) | 5.44 s | 1.58 s (1t) |
| `lap_tmpl` | tpl-OASIS30ANTs_res-01_T1w_maths.nii.gz | bit-identical | pass: bit-identical | same | 8.46 s (1t) | 6.41 s | 1.98 s (1t) |

## A06: Brain extraction: template priors to the T1w (Gaussian, inverse warp)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `map_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans.nii.gz | 2,053 of 12,845,056 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 | pass: max rel 5.6e-16 | same | 17.2 s (1t) | 14.1 s | 14.1 s (1t) |
| `map_wmmask` | summap_trans.nii.gz | 3,070 of 12,845,056 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 | pass: max rel 5.6e-16 | same | 17.0 s (1t) | 13.4 s | 13.8 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `map_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans.nii.gz | same | 2,053 of 12,845,056 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 | same | same | explained | 2,053 of 12,845,056 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 |
| `map_wmmask` | summap_trans.nii.gz | same | 3,070 of 12,845,056 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 | same | same | explained | 3,070 of 12,845,056 differ; max \|diff\| 5.55e-16 (rel 5.6e-16), NCC 1 |

## A07: Brain extraction: threshold the brain prior (`ThresholdImage`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `thr_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled.nii.gz | bit-identical | pass: bit-identical | same | 0.87 s (1t) | 0.94 s | 0.26 s (1t) |

## A08: Brain extraction: mask and label morphology (`ImageMath` MD/ME/GetLargestComponent/FillHoles/PadImage/addtozero, `MultiplyImages`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `02_pad_segm` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected_maths_corrected_labeled_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.01 s (1t) | 1.02 s | 0.36 s (1t) |
| `03_pad_mask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.67 s (1t) | 1.03 s | 0.34 s (1t) |
| `05_get_wm` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected_maths_corrected_labeled_maths_class-03_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.59 s (1t) | 1.21 s | 0.26 s (1t) |
| `06_get_gm` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected_maths_corrected_labeled_maths_class-02_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.67 s (1t) | 1.30 s | 0.26 s (1t) |
| `07_fill_gm` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected_maths_corrected_labeled_maths_class-02_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 12.8 s (1t) | 9.47 s | 0.35 s (1t) |
| `08_mult_gm` | 08_mult_gm.nii.gz | bit-identical | pass: bit-identical | same | 1.10 s (1t) | 1.01 s | 0.33 s (1t) |
| `09_relabel_wm` | 09_relabel_wm.nii.gz | bit-identical | pass: bit-identical | same | 0.95 s (1t) | 0.84 s | 0.25 s (1t) |
| `10_me_csf` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected_maths_corrected_labeled_maths_class-01_maths.nii.gz | bit-identical | pass: bit-identical | same | 5.20 s (1t) | 3.32 s | 0.89 s (1t) |
| `11_add_gm` | 08_mult_gm_maths.nii.gz | bit-identical | pass: bit-identical | same | 2.02 s (1t) | 1.36 s | 0.34 s (1t) |
| `12_relabel_gm` | 12_relabel_gm.nii.gz | bit-identical | pass: bit-identical | same | 0.67 s (1t) | 0.90 s | 0.25 s (1t) |
| `13_add_gm_wm` | 09_relabel_wm_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.53 s (1t) | 1.58 s | 0.38 s (1t) |
| `15_add_7` | 09_relabel_wm_maths_class-03_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.15 s (1t) | 1.25 s | 0.27 s (1t) |
| `16_me_7` | 09_relabel_wm_maths_class-03_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 2.50 s (1t) | 2.45 s | 0.90 s (1t) |
| `17_comp_7` | 09_relabel_wm_maths_class-03_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.41 s (1t) | 1.39 s | 0.33 s (1t) |
| `18_md_7` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.37 s (1t) | 1.31 s | 0.40 s (1t) |
| `19_fill_7` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 9.81 s (1t) | 9.41 s | 0.35 s (1t) |
| `20_add_7_2` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.56 s (1t) | 1.33 s | 0.35 s (1t) |
| `21_md_7_2` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.54 s (1t) | 1.49 s | 0.43 s (1t) |
| `22_me_7_2` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 3.01 s (1t) | 2.58 s | 0.89 s (1t) |
| `23_depad_mask` | 09_relabel_wm_maths_class-03_maths_maths_maths_maths_maths_maths_maths_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.68 s (1t) | 0.95 s | 0.31 s (1t) |
| `24_depad_segm` | 09_relabel_wm_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.67 s (1t) | 0.97 s | 0.33 s (1t) |
| `25_depad_gm` | 12_relabel_gm_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.74 s (1t) | 0.95 s | 0.32 s (1t) |
| `26_depad_wm` | 09_relabel_wm_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.01 s (1t) | 0.94 s | 0.31 s (1t) |
| `27_depad_csf` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected_maths_corrected_labeled_maths_class-01_maths.nii.gz | bit-identical | pass: bit-identical | same | 0.77 s (1t) | 0.81 s | 0.29 s (1t) |
| `dil_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.14 s (1t) | 1.21 s | 0.30 s (1t) |
| `get_brainmask` | tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled_maths_maths.nii.gz | bit-identical | pass: bit-identical | same | 1.10 s (1t) | 1.15 s | 0.18 s (1t) |

## A09: Brain extraction: final N4 (weighted by the WM posterior)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| `inu_n4_final0` | corrected | 4,721,646 of 12,845,056 differ; max \|diff\| 0.134 (rel 2.1e-05), NCC 1 - 6.2e-11 | pass: NCC 1.0000000 | same | 21.9 s (8t) | 13.2 s | 2.09 s (8t) |
| `inu_n4_final0` | bias field | 12,844,555 of 12,845,056 differ; max \|diff\| 1.69e-05 (rel 1.8e-05), NCC 1 - 4.5e-11 | pass: NCC 1.0000000 | same | 21.9 s (8t) | 13.2 s | 2.09 s (8t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| `inu_n4_final0` | corrected | 4,722,043 of 12,845,056 differ; max \|diff\| 0.0605 (rel 9.6e-06), NCC 1 - 7.6e-13 | 4,316,144 of 12,845,056 differ; max \|diff\| 0.0957 (rel 1.5e-05), NCC 1 - 6.6e-11 | same | same | explained | 4,316,144 of 12,845,056 differ; max \|diff\| 0.0957 (rel 1.5e-05), NCC 1 - 6.6e-11 |
| `inu_n4_final0` | bias field | 12,845,056 of 12,845,056 differ; max \|diff\| 7.72e-06 (rel 8e-06), NCC 1 - 1.4e-11 | 11,033,687 of 12,845,056 differ; max \|diff\| 1.35e-05 (rel 1.4e-05), NCC 1 - 5.9e-11 | same | same | explained | 11,033,687 of 12,845,056 differ; max \|diff\| 1.35e-05 (rel 1.4e-05), NCC 1 - 5.9e-11 |

## A10: Normalisation: truncate the moving T1w

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| MNI152NLin2009cAsym `trunc_mov` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w_noise_corrected_corrected_xform_maths.nii.gz | bit-identical | pass: bit-identical | same | 2.92 s (1t) | 2.10 s | 0.79 s (1t) |

## A11: Anatomical outputs to MNI (`.h5` composite; Lanczos, MultiLabel, Gaussian)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| MNI152NLin2009cAsym.res2 `mask_std` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 10.2 s (1t) | 7.45 s | 6.84 s (1t) |
| MNI152NLin2009cAsym.res2 `t1w_std` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_desc-preproc_T1w_trans.nii.gz | bit-identical | pass: bit-identical | same | 5.60 s (1t) | 4.00 s | 2.14 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_dseg` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_dseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 12.8 s (1t) | 7.48 s | 7.77 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_mask` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 11.7 s (1t) | 6.97 s | 7.02 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_t1w` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_desc-preproc_T1w_masked_trans.nii.gz | bit-identical | pass: bit-identical | same | 7.51 s (1t) | 3.84 s | 2.02 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms0` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_label-GM_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 4.08 s (1t) | 2.43 s | 1.99 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms1` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_label-WM_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 3.40 s (1t) | 2.45 s | 2.00 s (1t) |
| MNI152NLin2009cAsym.res2 `anat2std_tpms2` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_label-CSF_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 3.22 s (1t) | 2.44 s | 2.02 s (1t) |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| MNI152NLin2009cAsym.res2 `anat2std_dseg` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_dseg_trans.nii.gz | out/sub-ON01016/ses-01/anat/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_space-MNI152NLin2009cAsym_res-2_dseg.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_mask` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_desc-brain_mask_trans.nii.gz | out/sub-ON01016/ses-01/anat/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_t1w` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_desc-preproc_T1w_masked_trans.nii.gz | out/sub-ON01016/ses-01/anat/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_tpms0` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_label-GM_probseg_trans.nii.gz | out/sub-ON01016/ses-01/anat/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_space-MNI152NLin2009cAsym_res-2_label-GM_probseg.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_tpms1` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_label-WM_probseg_trans.nii.gz | out/sub-ON01016/ses-01/anat/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_space-MNI152NLin2009cAsym_res-2_label-WM_probseg.nii.gz | bit-identical |
| MNI152NLin2009cAsym.res2 `anat2std_tpms2` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_label-CSF_probseg_trans.nii.gz | out/sub-ON01016/ses-01/anat/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_space-MNI152NLin2009cAsym_res-2_label-CSF_probseg.nii.gz | bit-identical |

## F01: Head-motion estimation (mcflirt on the raw BOLD, to the HMC boldref)

larmorx runs `larmorx mri hmc` with mcflirt's arguments from `command.txt` (the raw BOLD, fMRIPrep's HMC boldref as `-reffile`, `-mats`). Matrices against mcflirt's, per volume (RMS deviation over a sphere of 80 mm about the field of view's centre, Jenkinson 1999; parameter differences about the same centre); mcflirt again in fMRIPrep's image, and FSL 6.0.7's mcflirt on this machine, against fMRIPrep's run:

| Run | Volumes | RMS deviation median / 95th pct / max (mm) | Median Δparam (mm / °) | Max Δparam (mm / °) | mcflirt re-run: identical volumes | FSL 6.0.7 mcflirt: identical volumes |
|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `mcflirt` | 200 | 0.0128 / 0.0240 / 0.0321 | 0.0023 / 0.0042 | 0.0209 / 0.0316 | 200 of 200 | 200 of 200 |
| ses_01_task_rest_dir_reverse `mcflirt` | 10 | 0.0105 / 0.0196 / 0.0201 | 0.0027 / 0.0038 | 0.0132 / 0.0139 | 10 of 10 | 10 of 10 |

**What fMRIPrep derives from them**, computed by fMRIPrep's own code in its image (niworkflows' `MCFLIRT2ITK`, fMRIPrep's `FSLMotionParams`, `FramewiseDisplacement`, `FSLRMSDeviation`) from larmorx's matrices, against the confounds file fMRIPrep wrote (`desc-confounds_timeseries.tsv`) and its `desc-hmc_xfm.txt`. The same code on mcflirt's matrices reproduces fMRIPrep's file (last column). mcflirt's band is its own lowest FD correlation when its input is perturbed slightly: on this run, where PLAN.md §11.3's FD correlation applies and larmorx misses it (below), else the [mri-hmc](mri-hmc.md) band of the dataset's testdata case, where there is one (another series of the dataset, so for information only):

| Run | Median Δ motion params (mm / °) | Max Δ (mm / °) | FD r | Max FD (mm) | Mean FD fMRIPrep / larmorx (mm) | Max Δ RMSD (mm) | Max Δ ITK matrix entry (mm for translations) | Check: mcflirt's matrices |
|---|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `mcflirt` | 0.0021 / 0.0042 | 0.0217 / 0.0315 | 0.9738 (band on this run 0.9613) | 0.560 | 0.1016 / 0.1025 | 0.024 | 0.022 | same |
| ses_01_task_rest_dir_reverse `mcflirt` | 0.0031 / 0.0038 | 0.0158 / 0.0139 | 0.9954 | 0.484 | 0.1486 / 0.1493 | 0.019 | 0.018 | same |

**mcflirt's own band on these runs**: FSL 6.0.7's mcflirt (which reproduces fMRIPrep's matrices bit for bit) with fMRIPrep's arguments on the series perturbed as the [mri-hmc](mri-hmc.md) band perturbs it (uniform noise of ±0.5 of a stored unit, three realisations; the volumes in reverse order), each through fMRIPrep's own confound code, against fMRIPrep's confounds file, exactly as larmorx's matrices above. The FD threshold is widened to the lowest of these (porting guide §7):

| Run | Perturbation | Median Δ motion params (mm / °) | Max Δ (mm / °) | FD r | Max Δ FD (mm) |
|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `mcflirt` | noise | 0.0019 / 0.0040 | 0.0230 / 0.0251 | 0.9732 | 0.053 |
| ses_01_task_rest_dir_forward `mcflirt` | noise2 | 0.0017 / 0.0034 | 0.0238 / 0.0278 | 0.9760 | 0.048 |
| ses_01_task_rest_dir_forward `mcflirt` | noise3 | 0.0017 / 0.0038 | 0.0230 / 0.0334 | 0.9723 | 0.05 |
| ses_01_task_rest_dir_forward `mcflirt` | reverse | 0.0034 / 0.0058 | 0.0192 / 0.0371 | 0.9613 | 0.074 |

Times: fMRIPrep's node (its run), mcflirt re-run in fMRIPrep's image on this machine (one thread, as mcflirt runs), `larmorx mri hmc` with the same arguments (it also resamples and writes the series, as mcflirt does), and the Python API with `resample=False` (the matrices only, all fMRIPrep uses):

| Run | fMRIPrep | mcflirt re-run | larmorx CLI | larmorx API | larmorx API, all CPUs |
|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `mcflirt` | 36.8 s | 32.9 s | 32.2 s (1t) | 27.1 s (1t) | 5.82 s (12t) |
| ses_01_task_rest_dir_reverse `mcflirt` | 1.96 s | 1.89 s | 1.60 s (1t) | 1.36 s (1t) | 0.34 s (12t) |

## F02: Slice-timing correction (`3dTshift`)

`larmorx afni 3dTshift` with the node's arguments from `command.txt`, in both implementations, in `reproduce` mode (fMRIPrep as it ran). nipype's `TShift` writes the slice times into `slice_timing.1D` in the node directory and links the input there, and removes both after the run; the harness writes the file again exactly as nipype does (the times joined by tabs, as Python's `str` prints them, 48 slices) and gives the input's original path, from the node's hash file. The thresholds are [afni-tshift.md](afni-tshift.md)'s. Both larmorx implementations write no AFNI history extension (`docs/api/afni-tshift.md`), so `vox_offset` differs when AFNI wrote one.

| Run | Implementation | Against fMRIPrep's output | Thresholds | Header (as stored) | Against AFNI 25.2.09 (oracle build) | Time |
|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `slice_timing_correction` | replica | bit-identical | pass: bit-identical | identical but `vox_offset` (AFNI's history extension, which larmorx does not write) | same | 2.88 s (1t) |
| ses_01_task_rest_dir_forward `slice_timing_correction` | original | 86 of 39,321,600 differ; max \|diff\| 1 (rel 0.00012), NCC 1 - 1e-12 | pass: int16: 86 of 39,321,600 stored values differ, max \|diff\| 1 unit | identical but `vox_offset` (AFNI's history extension, which larmorx does not write) | 86 of 39,321,600 differ; max \|diff\| 1 (rel 0.00012), NCC 1 - 1e-12 | 2.54 s (1t) |
| ses_01_task_rest_dir_forward `slice_timing_correction` | fMRIPrep's AFNI, re-run in its image / AFNI 25.2.09 oracle build | same / same |  |  |  | 9.07 s (1t, its run); 8.57 s (re-run) |
| ses_01_task_rest_dir_reverse `slice_timing_correction` | replica | bit-identical | pass: bit-identical | identical but `vox_offset` (AFNI's history extension, which larmorx does not write) | same | 0.15 s (1t) |
| ses_01_task_rest_dir_reverse `slice_timing_correction` | original | 1 of 1,966,080 differ; max \|diff\| 1 (rel 0.00011), NCC 1 - 2.3e-13 | pass: int16: 1 of 1,966,080 stored values differ, max \|diff\| 1 unit | identical but `vox_offset` (AFNI's history extension, which larmorx does not write) | 1 of 1,966,080 differ; max \|diff\| 1 (rel 0.00011), NCC 1 - 2.3e-13 | 0.13 s (1t) |
| ses_01_task_rest_dir_reverse `slice_timing_correction` | fMRIPrep's AFNI, re-run in its image / AFNI 25.2.09 oracle build | same / same |  |  |  | 1.43 s (1t, its run); 1.19 s (re-run) |

larmorx's default (`upstream_defects="fix"`, AFNI's defects corrected, D13) against the `reproduce` output above, and the defects it named:

| Run | Implementation | Data | Header fields | Notices |
|---|---|---|---|---|
| ses_01_task_rest_dir_forward `slice_timing_correction` | replica | same | same | none |
| ses_01_task_rest_dir_forward `slice_timing_correction` | original | same | same | none |
| ses_01_task_rest_dir_reverse `slice_timing_correction` | replica | same | same | none |
| ses_01_task_rest_dir_reverse `slice_timing_correction` | original | same | same | none |

## F03: Coregistration reference: N4 of the HMC boldref

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `n4_correct` | sub-ON01016_ses-01_task-rest_dir-forward_desc-hmc_boldref_corrected.nii.gz | 193,141 of 196,608 differ; max \|diff\| 3.3 (rel 0.00069), NCC 1 - 5.4e-09 | pass: NCC 1.0000000 | same | 1.31 s (1t) | 1.32 s | 0.21 s (1t) |
| ses_01_task_rest_dir_reverse `n4_correct` | sub-ON01016_ses-01_task-rest_dir-reverse_desc-hmc_boldref_corrected.nii.gz | 192,977 of 196,608 differ; max \|diff\| 2.84 (rel 0.00051), NCC 1 - 3.5e-09 | pass: NCC 1.0000000 | same | 1.27 s (1t) | 1.20 s | 0.22 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `n4_correct` | sub-ON01016_ses-01_task-rest_dir-forward_desc-hmc_boldref_corrected.nii.gz | same | 193,141 of 196,608 differ; max \|diff\| 3.3 (rel 0.00069), NCC 1 - 5.4e-09 | same | same | explained | 193,141 of 196,608 differ; max \|diff\| 3.3 (rel 0.00069), NCC 1 - 5.4e-09 |
| ses_01_task_rest_dir_reverse `n4_correct` | sub-ON01016_ses-01_task-rest_dir-reverse_desc-hmc_boldref_corrected.nii.gz | same | 192,977 of 196,608 differ; max \|diff\| 2.84 (rel 0.00051), NCC 1 - 3.5e-09 | same | same | explained | 192,977 of 196,608 differ; max \|diff\| 2.84 (rel 0.00051), NCC 1 - 3.5e-09 |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| ses_01_task_rest_dir_forward `n4_correct` | sub-ON01016_ses-01_task-rest_dir-forward_desc-hmc_boldref_corrected.nii.gz | out/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-forward_desc-coreg_boldref.nii.gz | 193,141 of 196,608 differ; max \|diff\| 3.3 (rel 0.00069), NCC 1 - 5.4e-09 |
| ses_01_task_rest_dir_reverse `n4_correct` | sub-ON01016_ses-01_task-rest_dir-reverse_desc-hmc_boldref_corrected.nii.gz | out/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-reverse_desc-coreg_boldref.nii.gz | 192,977 of 196,608 differ; max \|diff\| 2.84 (rel 0.00051), NCC 1 - 3.5e-09 |

## F04: Coregistration reference: MNI brain prior to the boldref (Linear)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `map_brainmask` | tpl-MNI152NLin2009cAsym_res-01_label-brain_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.41 s (1t) | 0.60 s | 0.11 s (1t) |
| ses_01_task_rest_dir_reverse `map_brainmask` | tpl-MNI152NLin2009cAsym_res-01_label-brain_probseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.41 s (1t) | 0.61 s | 0.08 s (1t) |

## F05: One-shot resampling to the boldref (HMC)

`lx.transforms.resample_series` with the `ResampleSeries` node's inputs from its report: the node's `in_file` (fMRIPrep's own input: the BOLD series, after slice timing where fMRIPrep corrected it, or the boldref), the target grid, the transforms (`1` files), order 3, mode `grid-constant`, Jacobian False, float32 output. fMRIPrep's time includes writing the compressed output; larmorx's is shown as compute + write. fMRIPrep's re-run has the node's `num_threads` and `OMP_NUM_THREADS=1`, as fMRIPrep's image sets it: with more OpenBLAS threads, fMRIPrep's resampler corrupts some volumes ([fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx's result does not depend on any thread count.

| Run | Shape | larmorx vs fMRIPrep's node | larmorx vs the derivative | PLAN.md §11.3 | fMRIPrep re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `boldref_bold` | 64×64×48×200 | bit-identical | bit-identical | pass: bit-identical | same | 13.2 s (8t) | 12.7 s | 1.76 s + 0.92 s (8t) |
| ses_01_task_rest_dir_reverse `boldref_bold` | 64×64×48×10 | bit-identical | bit-identical | pass: bit-identical | same | 0.72 s (8t) | 1.65 s | 0.14 s + 0.05 s (8t) |

## F06: One-shot resampling to T1w (HMC + coregistration)

`lx.transforms.resample_series` with the `ResampleSeries` node's inputs from its report: the node's `in_file` (fMRIPrep's own input: the BOLD series, after slice timing where fMRIPrep corrected it, or the boldref), the target grid, the transforms (`2` files), order 3, mode `grid-constant`, Jacobian False, float32 output. fMRIPrep's time includes writing the compressed output; larmorx's is shown as compute + write. fMRIPrep's re-run has the node's `num_threads` and `OMP_NUM_THREADS=1`, as fMRIPrep's image sets it: with more OpenBLAS threads, fMRIPrep's resampler corrupts some volumes ([fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx's result does not depend on any thread count.

| Run | Shape | larmorx vs fMRIPrep's node | larmorx vs the derivative | PLAN.md §11.3 | fMRIPrep re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `resample` | 43×52×43×200 | bit-identical | bit-identical | pass: bit-identical | same | 6.21 s (8t) | 7.22 s | 1.17 s + 0.51 s (8t) |
| ses_01_task_rest_dir_reverse `resample` | 43×52×43×10 | bit-identical | bit-identical | pass: bit-identical | same | 0.38 s (8t) | 1.51 s | 0.14 s + 0.04 s (8t) |

## F07: One-shot resampling to MNI res-2 (HMC + coregistration + `.h5` warp)

`lx.transforms.resample_series` with the `ResampleSeries` node's inputs from its report: the node's `in_file` (fMRIPrep's own input: the BOLD series, after slice timing where fMRIPrep corrected it, or the boldref), the target grid, the transforms (`3` files), order 3, mode `grid-constant`, Jacobian False, float32 output. fMRIPrep's time includes writing the compressed output; larmorx's is shown as compute + write. fMRIPrep's re-run has the node's `num_threads` and `OMP_NUM_THREADS=1`, as fMRIPrep's image sets it: with more OpenBLAS threads, fMRIPrep's resampler corrupts some volumes ([fmriprep-oracle.md](../findings/fmriprep-oracle.md)); larmorx's result does not depend on any thread count.

| Run | Shape | larmorx vs fMRIPrep's node | larmorx vs the derivative | PLAN.md §11.3 | fMRIPrep re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward MNI152NLin2009cAsym.res2 `resample` | 97×115×97×200 | bit-identical | bit-identical | pass: bit-identical | same | 63.1 s (8t) | 61.9 s | 6.82 s + 5.00 s (8t) |
| ses_01_task_rest_dir_reverse MNI152NLin2009cAsym.res2 `resample` | 97×115×97×10 | bit-identical | bit-identical | pass: bit-identical | same | 6.26 s (8t) | 7.28 s | 1.96 s + 0.25 s (8t) |

## F08: boldref and BOLD mask to T1w (Lanczos, MultiLabel)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `resample_mask` | sub-ON01016_ses-01_task-rest_dir-forward_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.84 s (1t) | 1.00 s | 0.57 s (1t) |
| ses_01_task_rest_dir_forward `resample_ref` | sub-ON01016_ses-01_task-rest_dir-forward_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.63 s (1t) | 0.69 s | 0.17 s (1t) |
| ses_01_task_rest_dir_reverse `resample_mask` | sub-ON01016_ses-01_task-rest_dir-reverse_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.90 s (1t) | 1.03 s | 0.58 s (1t) |
| ses_01_task_rest_dir_reverse `resample_ref` | sub-ON01016_ses-01_task-rest_dir-reverse_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.61 s (1t) | 0.74 s | 0.18 s (1t) |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| ses_01_task_rest_dir_forward `resample_mask` | sub-ON01016_ses-01_task-rest_dir-forward_desc-brain_mask_trans.nii.gz | out/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-forward_space-T1w_desc-brain_mask.nii.gz | bit-identical |
| ses_01_task_rest_dir_forward `resample_ref` | sub-ON01016_ses-01_task-rest_dir-forward_desc-coreg_boldref_trans.nii.gz | out/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-forward_space-T1w_boldref.nii.gz | bit-identical |
| ses_01_task_rest_dir_reverse `resample_mask` | sub-ON01016_ses-01_task-rest_dir-reverse_desc-brain_mask_trans.nii.gz | out/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-reverse_space-T1w_desc-brain_mask.nii.gz | bit-identical |
| ses_01_task_rest_dir_reverse `resample_ref` | sub-ON01016_ses-01_task-rest_dir-reverse_desc-coreg_boldref_trans.nii.gz | out/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-reverse_space-T1w_boldref.nii.gz | bit-identical |

## F09: boldref and BOLD mask to MNI (Lanczos, MultiLabel, `.h5`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward MNI152NLin2009cAsym.res2 `resample_mask` | sub-ON01016_ses-01_task-rest_dir-forward_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 8.29 s (1t) | 7.19 s | 6.25 s (1t) |
| ses_01_task_rest_dir_forward MNI152NLin2009cAsym.res2 `resample_ref` | sub-ON01016_ses-01_task-rest_dir-forward_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 5.86 s (1t) | 4.62 s | 2.00 s (1t) |
| ses_01_task_rest_dir_reverse MNI152NLin2009cAsym.res2 `resample_mask` | sub-ON01016_ses-01_task-rest_dir-reverse_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 8.07 s (1t) | 7.16 s | 6.61 s (1t) |
| ses_01_task_rest_dir_reverse MNI152NLin2009cAsym.res2 `resample_ref` | sub-ON01016_ses-01_task-rest_dir-reverse_desc-coreg_boldref_trans.nii.gz | bit-identical | pass: bit-identical | same | 5.75 s (1t) | 4.52 s | 1.99 s (1t) |

Outputs that fMRIPrep also writes as derivatives, against the derivative:

| Node | Output | Derivative | larmorx vs derivative |
|---|---|---|---|
| ses_01_task_rest_dir_forward MNI152NLin2009cAsym.res2 `resample_mask` | sub-ON01016_ses-01_task-rest_dir-forward_desc-brain_mask_trans.nii.gz | out/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-forward_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| ses_01_task_rest_dir_forward MNI152NLin2009cAsym.res2 `resample_ref` | sub-ON01016_ses-01_task-rest_dir-forward_desc-coreg_boldref_trans.nii.gz | out/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-forward_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz | bit-identical |
| ses_01_task_rest_dir_reverse MNI152NLin2009cAsym.res2 `resample_mask` | sub-ON01016_ses-01_task-rest_dir-reverse_desc-brain_mask_trans.nii.gz | out/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-reverse_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz | bit-identical |
| ses_01_task_rest_dir_reverse MNI152NLin2009cAsym.res2 `resample_ref` | sub-ON01016_ses-01_task-rest_dir-reverse_desc-coreg_boldref_trans.nii.gz | out/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-reverse_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz | bit-identical |

## F10: Confound masks to the boldref (Gaussian, MultiLabel, inverse)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `acc_msk_tfm0` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_label-CSF_probseg_trans.nii.gz | 67 of 196,608 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | pass: max rel 2.2e-16 | same | 0.61 s (1t) | 0.86 s | 0.35 s (1t) |
| ses_01_task_rest_dir_forward `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | 6 of 196,608 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | pass: max rel 2.2e-16 | same | 0.56 s (1t) | 0.78 s | 0.35 s (1t) |
| ses_01_task_rest_dir_forward `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | 7 of 196,608 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 | pass: max rel 1.1e-16 | same | 0.57 s (1t) | 0.83 s | 0.32 s (1t) |
| ses_01_task_rest_dir_forward `t1w_mask_tfm` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 1.34 s (1t) | 1.57 s | 1.19 s (1t) |
| ses_01_task_rest_dir_reverse `acc_msk_tfm0` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_label-CSF_probseg_trans.nii.gz | 77 of 196,608 differ; max \|diff\| 3.33e-16 (rel 3.3e-16), NCC 1 | pass: max rel 3.3e-16 | same | 0.62 s (1t) | 0.90 s | 0.34 s (1t) |
| ses_01_task_rest_dir_reverse `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | 8 of 196,608 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 | pass: max rel 1.1e-16 | same | 0.56 s (1t) | 0.77 s | 0.32 s (1t) |
| ses_01_task_rest_dir_reverse `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | 8 of 196,608 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 | pass: max rel 1.1e-16 | same | 0.56 s (1t) | 0.81 s | 0.31 s (1t) |
| ses_01_task_rest_dir_reverse `t1w_mask_tfm` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_desc-brain_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 1.37 s (1t) | 1.55 s | 1.14 s (1t) |

Where larmorx and fMRIPrep differ, each step of the chain changes one thing: fMRIPrep's run → its program at one thread (threads) → the same with CORE-MATH's correctly rounded functions preloaded (glibc's maths) → ANTs 2.6.5 from the repository's oracle build, one thread, correctly rounded (version and build) → larmorx. The last column compares larmorx with ANTs 2.6.5 as built (glibc's maths), as the tool's own validation record does.

| Node | Output | fMRIPrep → 1 thread | → correctly rounded | → ANTs 2.6.5 (oracle build) | → larmorx |  | larmorx vs ANTs 2.6.5 as built |
|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `acc_msk_tfm0` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_label-CSF_probseg_trans.nii.gz | same | 67 of 196,608 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | same | same | explained | 67 of 196,608 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 |
| ses_01_task_rest_dir_forward `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | same | 6 of 196,608 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 | same | same | explained | 6 of 196,608 differ; max \|diff\| 2.22e-16 (rel 2.2e-16), NCC 1 |
| ses_01_task_rest_dir_forward `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | same | 7 of 196,608 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 | same | same | explained | 7 of 196,608 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 |
| ses_01_task_rest_dir_reverse `acc_msk_tfm0` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_label-CSF_probseg_trans.nii.gz | same | 77 of 196,608 differ; max \|diff\| 3.33e-16 (rel 3.3e-16), NCC 1 | same | same | explained | 77 of 196,608 differ; max \|diff\| 3.33e-16 (rel 3.3e-16), NCC 1 |
| ses_01_task_rest_dir_reverse `acc_msk_tfm1` | acompcor_wm_trans.nii.gz | same | 8 of 196,608 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 | same | same | explained | 8 of 196,608 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 |
| ses_01_task_rest_dir_reverse `acc_msk_tfm2` | acompcor_wmcsf_trans.nii.gz | same | 8 of 196,608 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 | same | same | explained | 8 of 196,608 differ; max \|diff\| 1.11e-16 (rel 1.1e-16), NCC 1 |

## F11: Carpet-plot segmentation to the boldref (MultiLabel, `.h5`, `-u int`)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `resample_parc` | tpl-MNI152NLin2009cAsym_res-01_desc-carpet_dseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 2.15 s (1t) | 2.06 s | 1.67 s (1t) |
| ses_01_task_rest_dir_reverse `resample_parc` | tpl-MNI152NLin2009cAsym_res-01_desc-carpet_dseg_trans.nii.gz | bit-identical | pass: bit-identical | same | 2.51 s (1t) | 2.02 s | 1.43 s (1t) |

## F12: Report: T1w and WM mask to the boldref (Lanczos, NearestNeighbor)

| Node | Output | larmorx vs fMRIPrep | PLAN.md §11.3 | fMRIPrep's tool re-run vs its run | fMRIPrep | Re-run | larmorx |
|---|---|---|---|---|---|---|---|
| ses_01_task_rest_dir_forward `boldref_wm` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_dseg_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.26 s (1t) | 0.57 s | 0.06 s (1t) |
| ses_01_task_rest_dir_forward `t1w_boldref` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_desc-preproc_T1w_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.92 s (1t) | 1.14 s | 0.39 s (1t) |
| ses_01_task_rest_dir_reverse `boldref_wm` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_dseg_mask_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.29 s (1t) | 0.54 s | 0.06 s (1t) |
| ses_01_task_rest_dir_reverse `t1w_boldref` | sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_desc-preproc_T1w_trans.nii.gz | bit-identical | pass: bit-identical | same | 0.91 s (1t) | 1.12 s | 0.38 s (1t) |

## Reproduce

```bash
scripts/run_fmriprep_oracle.sh ~/.cache/larmorx-testdata/data/openneuro/ds005752 ON01016 ds005752-sub-ON01016-noreconall --fs-no-reconall   # the oracle run (about an hour)
scripts/build_ants_oracle.sh && scripts/build_crmath_preload.sh   # the references
scripts/build_afni_oracle.sh   # AFNI 25.2.09 (3dTshift)
(cd crates-gpl && cargo build --release -p larmorx-gpl-cli)   # the 3dTshift replica
python -m larmorx_validation pipeline-stages --oracle <workspace>/oracles/fmriprep-25.2.6/ds005752-sub-ON01016-noreconall --out docs/validation --reuse --name fmriprep-stages-ds005752
```

larmorx's outputs and the reference runs are kept in `<workspace>/oracles/compare/fmriprep-25.2.6/ds005752-sub-ON01016-noreconall/<node path>/<run>/` (`larmorx`, `larmorx-replica`, `larmorx-original`, `*-fix`, `container-<n>t`, `container-1t-cr`, `ants-2.6.5-1t`, `ants-2.6.5-1t-cr`, `ants-2.6.5-1t-cr-double`, `afni-25.2.09`, `fsl-6.0.7`, `confounds-*`), each with `run.json` (arguments, environment, exit status, time). `--reuse` reuses the one-thread reference runs of an earlier invocation; `--stage A01 F05` limits the comparison; `--render` writes this page again from the JSON record, without running anything.

