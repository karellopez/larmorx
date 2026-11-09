# Parity: niworkflows' helpers and nipype's command lines on the field maps' paths

**All cases agree.** 5 cases on the `standard` tier: 5 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `lx.niworkflows`, `larmorx._nipype`
- **Reference:** niworkflows 1.15.3 (the environments' release; of its Python files only interfaces/confounds.py and the version differ from 1.15.4's, the port's source) and nipype 1.12.0 in-process (sdcflows 2.17.1's calls), and fMRIPrep 25.2.6's own files and command lines of the oracle runs (numpy 2.5.4)
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `standard`
- **Generated:** 2026-11-09 on Linux x86_64, with `python -m larmorx_validation parity niworkflows-helpers --tier standard --python newest`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

50 comparison lines in 5 script runs.
The originals' own step (numpy 2.3.5 (OpenBLAS 0.3.30), SciPy 1.15.3 → numpy 2.5.4 (OpenBLAS 0.3.34), SciPy 1.18.1): 2 comparisons, 0 changed with a known reason, 0 varying in the shared releases, the rest identical.

## Thresholds

| Quantity | Requirement |
|---|---|
| Every array, file (decompressed), header, warning, message and command line | bit-identical |
| upstream_defects='reproduce' | the original's result bit for bit, its errors and warnings |
| A corrected defect (the default) | the correction the register names, checked against an independent construction; an EXPECTED line names it |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| check_merge | 2 | 2 | 0 | 0 | 0 |
| check_nipype | 1 | 1 | 0 | 0 | 0 |
| check_regrid | 1 | 1 | 0 | 0 | 0 |
| check_skimage | 1 | 1 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (e81cf26b7a80) |
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
| `merge` | pass | 14/14 | IntraModalMerge: seeded series, errors, RAS reorientation, grand mean scaling |
| `merge-hmc` | pass | 11/11 | IntraModalMerge's head-motion correction (FSL's mcflirt replica) and fMRIPrep's magnitude merges |
| `nipype-command-lines` | pass | 26/26 | nipype's command lines of every program on these paths against every oracle run's command.txt |
| `regrid` | pass | 10/10 | RegridToZooms (SciPy's inv, gaussian_filter, nibabel) on seeded images and fMRIPrep's calls |
| `intensity-clip` | pass | 7/7 | IntensityClip on fMRIPrep's calls |
