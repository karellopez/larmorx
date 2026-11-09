# Parity: sdcflows' field-map estimation and correction, its BIDS app and fMRIPrep's own field-map steps

**All cases agree.** 14 cases on the `full` tier: 14 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `lx.sdcflows` (`crates/larmorx-sdcflows`), `lx.bids`, larmorprepx's field-map stages
- **Reference:** sdcflows 2.17.1 (and 2.15's PEPOLAR wiring) in-process with nipype, fMRIPrep 25.2.6's own graphs, files and command lines of the oracle runs, and sdcflows 2.17.1's own runs; the programs underneath through larmorx's ports and the oracle cache (numpy 2.5.4)
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `full`
- **Generated:** 2026-11-09 on Linux x86_64, with `python -m larmorx_validation parity sdcflows-estimation --tier full --case '^(pepolar-workflow|pepolar-image|syn-preprocessing-workflow|syn-sdc-workflow|estimators-image|derivatives-preproc|app-run-.*)$' --python newest`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

151 comparison lines in 14 script runs.

## Thresholds

| Quantity | Requirement |
|---|---|
| Every array, file (decompressed), header, warning, message and command line | bit-identical |
| upstream_defects='reproduce' | the original's result bit for bit, its errors and warnings |
| A corrected defect (the default) | the correction the register names, checked against an independent construction; an EXPECTED line names it |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| check_app | 8 | 8 | 0 | 0 | 0 |
| check_derivatives | 1 | 1 | 0 | 0 | 0 |
| check_estimators | 1 | 1 | 0 | 0 | 0 |
| check_pepolar | 2 | 2 | 0 | 0 | 0 |
| check_syn_preprocessing | 1 | 1 | 0 | 0 | 0 |
| check_syn_sdc | 1 | 1 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (241b3334bb67) |
| larmorx-testdata | 52e371aab0ac |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.5.4 |
| scipy | 1.18.1 |
| scikit-image | 0.26.0 |
| nibabel | 5.4.2 |
| nipype | 1.12.0 |
| niworkflows | 1.15.3 |
| sdcflows | 2.17.1 |
| nilearn | 0.14.1 |

## Notes

Each case is one of the investigation's check scripts (some of its sections), run as a separate process of the environment's interpreter; one check per summary line it prints, its final line and its exit status. The library scripts carry the originals' own step (python_versions).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `pepolar-workflow` | pass | 29/29 | topup_workflow, both wirings (sdcflows 2.15 and 2.17.1), against sdcflows' own graphs |
| `pepolar-image` | pass | 27/27 | the PEPOLAR steps with fMRIPrep's libraries against fMRIPrep's own node files |
| `syn-preprocessing-workflow` | pass | 11/11 | sdcflows' own SyN preprocessing graph run by nipype |
| `syn-sdc-workflow` | pass | 11/11 | the SyN registration against ANTs 2.6.5 and sdcflows' own graph run by nipype |
| `estimators-image` | pass | 4/4 | the estimators against fMRIPrep's own image (Apptainer) |
| `derivatives-preproc` | pass | 11/11 | init_fmap_preproc_wf run whole against sdcflows' own run |
| `app-run-ds005752` | pass | 19/19 | sdcflows' BIDS app run whole on ds005752-sub-ON01016-direct (a direct field map), against sdcflows' own |
| `app-run-ds001600` | pass | 19/19 | sdcflows' BIDS app run whole on ds001600-sub-1 (three phase-difference field maps, two of them from two phases, and a P… |
| `app-run-ds002422` | pass | 19/19 | sdcflows' BIDS app run whole on ds002422-sub-02 (two phase differences at 1.5 T), against sdcflows' own |
| `app-run-ds002603` | pass | 19/19 | sdcflows' BIDS app run whole on ds002603-sub-01 (Philips: a phase difference with one magnitude and two PEPOLAR estimat… |
| `app-run-ds004958` | pass | 19/19 | sdcflows' BIDS app run whole on ds004958-sub-fsm64ps-pepolar (a phase difference and a PEPOLAR pair, multiband-8), agai… |
| `app-run-ds005040` | pass | 19/19 | sdcflows' BIDS app run whole on ds005040-sub-002 (a PEPOLAR pair), against sdcflows' own |
| `app-run-ds005454` | pass | 19/19 | sdcflows' BIDS app run whole on ds005454-sub-16 (a phase difference named by its B0FieldIdentifier, multiband), against… |
| `app-run-ds006736` | pass | 19/19 | sdcflows' BIDS app run whole on ds006736-sub-004 (a PEPOLAR pair), against sdcflows' own |
