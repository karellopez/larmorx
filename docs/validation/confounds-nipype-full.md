# Parity: nipype's confound algorithms (DVARS, CompCor, TSNR, the non-steady-state detector, framewise displacement)

**All cases agree.** 3 cases on the `full` tier: 3 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `lx.confounds` (`crates/larmorx-confounds`), niworkflows' `robust_acompcor` and `robust_tcompcor`
- **Reference:** nipype 1.12.0 and nitime in-process in the newest and the shared library releases and in fMRIPrep 25.2.6's image, and fMRIPrep 25.2.6's own node files of 21 oracle runs
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `full`
- **Generated:** 2026-11-13 on Linux x86_64, with `python -m larmorx_validation parity confounds-nipype --tier full --case '^compcor-full-'`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

75 comparison lines in 3 script runs (e1 1, e2 1, e3 1).

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
| check_compcor | 3 | 3 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.1.0rc1 (b7420b224553) |
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
| `compcor-full-e1` | pass | 30/30 | nipype's cosine and Legendre filters, compute_noise_components, CompCor, ACompCor and TCompCor over their options, fMRI… |
| `compcor-full-e2` | pass | 30/30 | nipype's cosine and Legendre filters, compute_noise_components, CompCor, ACompCor and TCompCor over their options, fMRI… |
| `compcor-full-e3` | pass | 30/30 | nipype's cosine and Legendre filters, compute_noise_components, CompCor, ACompCor and TCompCor over their options, fMRI… |
