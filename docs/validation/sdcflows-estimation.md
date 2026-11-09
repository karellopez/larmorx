# Parity: sdcflows' field-map estimation and correction, its BIDS app and fMRIPrep's own field-map steps

**All cases agree.** 19 cases on the `standard` tier: 19 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `lx.sdcflows` (`crates/larmorx-sdcflows`), `lx.bids`, larmorprepx's field-map stages
- **Reference:** sdcflows 2.17.1 (and 2.15's PEPOLAR wiring) in-process with nipype, fMRIPrep 25.2.6's own graphs, files and command lines of the oracle runs, and sdcflows 2.17.1's own runs; the programs underneath through larmorx's ports and the oracle cache (numpy 2.5.4)
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `standard`
- **Generated:** 2026-11-09 on Linux x86_64, with `python -m larmorx_validation parity sdcflows-estimation --tier standard --python newest`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

323 comparison lines in 19 script runs.
The originals' own step (numpy 2.3.5 (OpenBLAS 0.3.30), SciPy 1.15.3 → numpy 2.5.4 (OpenBLAS 0.3.34), SciPy 1.18.1): 6 comparisons, 0 changed with a known reason, 0 varying in the shared releases, the rest identical.

## Thresholds

| Quantity | Requirement |
|---|---|
| Every array, file (decompressed), header, warning, message and command line | bit-identical |
| upstream_defects='reproduce' | the original's result bit for bit, its errors and warnings |
| A corrected defect (the default) | the correction the register names, checked against an independent construction; an EXPECTED line names it |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| check_app | 2 | 2 | 0 | 0 | 0 |
| check_bold_fmap | 1 | 1 | 0 | 0 | 0 |
| check_bold_stage | 1 | 1 | 0 | 0 | 0 |
| check_bold_steps | 1 | 1 | 0 | 0 | 0 |
| check_derivatives | 1 | 1 | 0 | 0 | 0 |
| check_epi_mask | 1 | 1 | 0 | 0 | 0 |
| check_estimators | 1 | 1 | 0 | 0 | 0 |
| check_fmap_stage | 1 | 1 | 0 | 0 | 0 |
| check_masker_steps | 1 | 1 | 0 | 0 | 0 |
| check_match_histogram | 1 | 1 | 0 | 0 | 0 |
| check_pepolar | 1 | 1 | 0 | 0 | 0 |
| check_qwarp_steps | 1 | 1 | 0 | 0 | 0 |
| check_skimage | 1 | 1 | 0 | 0 | 0 |
| check_syn_preprocessing | 1 | 1 | 0 | 0 | 0 |
| check_syn_sdc | 1 | 1 | 0 | 0 | 0 |
| check_unwarp | 1 | 1 | 0 | 0 | 0 |
| check_workflows | 1 | 1 | 0 | 0 | 0 |
| relay22/audit_contract | 1 | 1 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (ab5ffe99467a) |
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
| `masker` | pass | 6/6 | sdcflows' brain_masker on fMRIPrep's calls |
| `masker-steps` | pass | 16/16 | sdcflows' brain_masker step by step on fMRIPrep's calls |
| `epi-mask` | pass | 26/26 | sdcflows' epi_mask on synthetic images and four oracle runs' BOLD references |
| `match-histogram` | pass | 37/37 | sdcflows' match_histogram on synthetic images and the SyN runs' anatomy |
| `workflows` | pass | 53/53 | the brain-extraction, magnitude and field-map workflows (direct field maps) |
| `pepolar-units` | pass | 27/27 | PEPOLAR's interfaces (readout times, SelectPEVolumes, SortPEBlips, UniformGrid, TOPUP's inputs, RobustAverage) |
| `contract-audit` | pass | 6/6 | the part's contract: every registered _core function has its stub, every public function of the part's families its API… |
| `qwarp-steps` | pass | 22/22 | 3dQwarp's Python steps (Flatten, _sorted_pe, qwarp2ants) and the workflow's refusal |
| `syn-preprocessing` | pass | 65/65 | init_syn_preprocessing_wf's nodes on fMRIPrep's --use-syn-sdc runs and its library steps |
| `syn-sdc` | pass | 29/29 | init_syn_sdc_wf's nodes on fMRIPrep's --use-syn-sdc estimators |
| `estimators` | pass | 9/9 | lx.bids' index, sdcflows' wrangler and estimators, fMRIPrep's estimator selection on every oracle run |
| `derivatives` | pass | 40/40 | DerivativesDataSink, build_path, the derivatives and fmap_preproc workflows |
| `fmap-stage` | pass | 46/46 | larmorprepx's field-map estimation stage against fMRIPrep 25.2.6's graph and nodes |
| `unwarp` | pass | 31/31 | unwarp_workflow (threads, cases, errors) |
| `app` | pass | 27/27 | sdcflows' BIDS app: its parser, TOML and settings |
| `app-dry-runs` | pass | 22/22 | sdcflows' BIDS app: dry runs and exit statuses |
| `bold-fmap` | pass | 34/34 | fMRIPrep's BOLD field-map nodes (coregistration, skull strip, coeff2epi) |
| `bold-steps` | pass | 30/30 | fMRIPrep's DistortionParameters, ConcatenateXFMs, reconstruct_fieldmap, unwarp |
| `bold-stage` | pass | 38/38 | larmorprepx's BOLD fit field-map stages against fMRIPrep's files (PIPE-20) |
