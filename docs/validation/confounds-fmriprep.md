# Parity: fMRIPrep's and niworkflows' confound steps and fMRIPrep's confounds workflow

**All cases agree.** 16 cases on the `standard` tier: 16 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `lx.confounds`, `lx.niworkflows`, `larmorx.pipelines.larmorprepx.confounds`
- **Reference:** fMRIPrep 25.2.7 (its confounds workflow is 25.2.6's file), niworkflows and pandas in-process in the newest and the shared releases and in fMRIPrep 25.2.6's image, and fMRIPrep 25.2.6's own node files and confounds files of 21 oracle runs
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `standard`
- **Generated:** 2026-11-13 on Linux x86_64, with `python -m larmorx_validation parity confounds-fmriprep --tier standard`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

735 comparison lines in 16 script runs (e1 5, e2 5, e3 4, filling 1, retained 1).

## Thresholds

| Quantity | Requirement |
|---|---|
| Every array, file (decompressed), header, table cell, warning, message and exception text | bit-identical (a NaN's payload apart: x86-64's default NaN) |
| upstream_defects='reproduce' | the original's result bit for bit, its errors and warnings |
| A corrected defect (the default) | the correction the register names, checked against an independent construction or against reproduce where the defect does not apply |
| A negative control | the other library release differs somewhere |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| check_masks | 3 | 3 | 0 | 0 | 0 |
| check_model | 3 | 3 | 0 | 0 | 0 |
| check_motion | 3 | 3 | 0 | 0 | 0 |
| check_tables | 3 | 3 | 0 | 0 | 0 |
| check_workflow | 2 | 2 | 0 | 0 | 0 |
| difftest | 2 | 2 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.1.0rc1 (9a106cf16763) |
| larmorx-testdata | 52e371aab0ac |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| scipy | 1.15.3 |
| pandas | 3.0.6 |
| nibabel | 5.4.2 |
| nipype | 1.12.0 |
| niworkflows | 1.15.3 |
| nitime | 0.12.1 |

## Notes

Each case is one of the investigation's check scripts (some of its sections, a tier) run as a separate process in the environment its id names (e1: numpy 2.5.4, e1-zen: the same at this machine's GEMM_Q 512, e2: numpy 2.3.5, e3: fMRIPrep 25.2.6's image); one check per summary line it prints, its final line and its exit status.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `tables-quick-e1` | pass | 79/79 | pandas' text round trips (precise_xstrtod of pandas 3.0.6 and 2.2.3, ujson), fMRIPrep's FramewiseDisplacement, FilterDr… |
| `tables-quick-e2` | pass | 79/79 | pandas' text round trips (precise_xstrtod of pandas 3.0.6 and 2.2.3, ujson), fMRIPrep's FramewiseDisplacement, FilterDr… |
| `tables-quick-e3` | pass | 83/83 | pandas' text round trips (precise_xstrtod of pandas 3.0.6 and 2.2.3, ujson), fMRIPrep's FramewiseDisplacement, FilterDr… |
| `model-quick-e1` | pass | 52/52 | niworkflows' ExpandModel, parse_formula, SpikeRegressors, NormalizeMotionParams, fMRIPrep's real model_expansion and sp… |
| `model-quick-e2` | pass | 52/52 | niworkflows' ExpandModel, parse_formula, SpikeRegressors, NormalizeMotionParams, fMRIPrep's real model_expansion and sp… |
| `model-quick-e3` | pass | 52/52 | niworkflows' ExpandModel, parse_formula, SpikeRegressors, NormalizeMotionParams, fMRIPrep's real model_expansion and sp… |
| `motion-quick-e1` | pass | 122/122 | SciPy's Rotation, nitransforms' ITK and FSL linear transforms, fMRIPrep's FSLMotionParams (25.2.7 and 25.2.6) and FSLRM… |
| `motion-quick-e2` | pass | 122/122 | SciPy's Rotation, nitransforms' ITK and FSL linear transforms, fMRIPrep's FSLMotionParams (25.2.7 and 25.2.6) and FSLRM… |
| `motion-quick-e3` | pass | 123/123 | SciPy's Rotation, nitransforms' ITK and FSL linear transforms, fMRIPrep's FSLMotionParams (25.2.7 and 25.2.6) and FSLRM… |
| `masks-quick-e1` | pass | 42/42 | fMRIPrep's _binary_union, aCompCorMasks, niworkflows' BinaryDilation, BinarySubtraction and SignalExtraction, fMRIPrep'… |
| `masks-quick-e2` | pass | 42/42 | fMRIPrep's _binary_union, aCompCorMasks, niworkflows' BinaryDilation, BinarySubtraction and SignalExtraction, fMRIPrep'… |
| `masks-quick-e3` | pass | 42/42 | fMRIPrep's _binary_union, aCompCorMasks, niworkflows' BinaryDilation, BinarySubtraction and SignalExtraction, fMRIPrep'… |
| `workflow-quick-e2` | pass | 48/48 | init_bold_confs_wf built by nipype for eight option sets against larmorx's steps, and the 36 confounds workflows end to… |
| `workflow-quick-e1` | pass | 48/48 | init_bold_confs_wf built by nipype for eight option sets against larmorx's steps, and the 36 confounds workflows end to… |
| `difftest/rename-acompcor-text-retained` | pass | 2/2 | RenameACompCor with a 'retained' column of text: pandas' KeyError prints the values' Index with line breaks at width 80… |
| `difftest/spikes-pipe67-after-filling` | pass | 2/2 | SpikeRegressors with lags [1, 2] and minimum_contiguous 5 (the trial's notice verdicts): the filling of short runs made… |
