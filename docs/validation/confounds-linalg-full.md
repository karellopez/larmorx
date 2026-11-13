# Parity: numpy's, OpenBLAS's, LAPACK's and SciPy's arithmetic under fMRIPrep's confounds

**All cases agree.** 12 cases on the `full` tier: 12 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx_transform::numpy_linalg`, `larmorx_transform::openblas`, `larmorx_core::numpy`, `larmorx_confounds::scipy_fft` (through `larmorx._core`)
- **Reference:** numpy's and SciPy's own OpenBLAS and LAPACK through their exported routines, numpy, SciPy and nitime in-process, in numpy 2.5.4 / OpenBLAS 0.3.34 / LAPACK 3.12.1 (and at this machine's GEMM_Q 512), numpy 2.3.5 / OpenBLAS 0.3.30 / LAPACK 3.12.0 and fMRIPrep 25.2.6's image
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `full`
- **Generated:** 2026-11-13 on Linux x86_64, with `python -m larmorx_validation parity confounds-linalg --tier full --case '^(svd|gesdd|gelsd|gesvd)-full-'`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

734 comparison lines in 12 script runs (e1 4, e1-zen 4, e2 4).

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
| check_gelsd | 3 | 3 | 0 | 0 | 0 |
| check_gesdd | 3 | 3 | 0 | 0 | 0 |
| check_gesvd | 3 | 3 | 0 | 0 | 0 |
| check_svd | 3 | 3 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.1.0rc1 (e54c3fb52532) |
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
| `svd-full-e2` | pass | 27/27 | the divide-and-conquer bidiagonal SVD (DBDSDC, DLASD0 to DLASD5, DLASDQ, DLAED6) against numpy's library (full tier, nu… |
| `svd-full-e1` | pass | 27/27 | the divide-and-conquer bidiagonal SVD (DBDSDC, DLASD0 to DLASD5, DLASDQ, DLAED6) against numpy's library (full tier, nu… |
| `svd-full-e1-zen` | pass | 32/32 | the divide-and-conquer bidiagonal SVD (DBDSDC, DLASD0 to DLASD5, DLASDQ, DLAED6) against numpy's library (full tier, nu… |
| `gesdd-full-e2` | pass | 30/30 | DGESDD's four thin paths, numpy's svd(full_matrices=False) and pinv up to CompCor's largest real shapes (full tier, num… |
| `gesdd-full-e1` | pass | 30/30 | DGESDD's four thin paths, numpy's svd(full_matrices=False) and pinv up to CompCor's largest real shapes (full tier, num… |
| `gesdd-full-e1-zen` | pass | 45/45 | DGESDD's four thin paths, numpy's svd(full_matrices=False) and pinv up to CompCor's largest real shapes (full tier, num… |
| `gelsd-full-e2` | pass | 126/126 | DGELSD's three paths, DLALSD's two (DLASDA, DLALS0, DLALSA), DLASRT and numpy's lstsq, the cosine filter's real calls i… |
| `gelsd-full-e1` | pass | 126/126 | DGELSD's three paths, DLALSD's two (DLASDA, DLALS0, DLALSA), DLASRT and numpy's lstsq, the cosine filter's real calls i… |
| `gelsd-full-e1-zen` | pass | 136/136 | DGELSD's three paths, DLALSD's two (DLASDA, DLALS0, DLALSA), DLASRT and numpy's lstsq, the cosine filter's real calls i… |
| `gesvd-full-e2` | pass | 83/83 | SciPy's DGESVD fallback (DBDSQR, OpenBLAS 0.3.28's DLARFT) and nipype's fallback_svd and _full_rank against SciPy's own… |
| `gesvd-full-e1` | pass | 83/83 | SciPy's DGESVD fallback (DBDSQR, OpenBLAS 0.3.28's DLARFT) and nipype's fallback_svd and _full_rank against SciPy's own… |
| `gesvd-full-e1-zen` | pass | 95/95 | SciPy's DGESVD fallback (DBDSQR, OpenBLAS 0.3.28's DLARFT) and nipype's fallback_svd and _full_rank against SciPy's own… |
