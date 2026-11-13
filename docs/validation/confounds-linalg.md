# Parity: numpy's, OpenBLAS's, LAPACK's and SciPy's arithmetic under fMRIPrep's confounds

**All cases agree.** 32 cases on the `standard` tier: 32 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx_transform::numpy_linalg`, `larmorx_transform::openblas`, `larmorx_core::numpy`, `larmorx_confounds::scipy_fft` (through `larmorx._core`)
- **Reference:** numpy's and SciPy's own OpenBLAS and LAPACK through their exported routines, numpy, SciPy and nitime in-process, in numpy 2.5.4 / OpenBLAS 0.3.34 / LAPACK 3.12.1 (and at this machine's GEMM_Q 512), numpy 2.3.5 / OpenBLAS 0.3.30 / LAPACK 3.12.0 and fMRIPrep 25.2.6's image
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `standard`
- **Generated:** 2026-11-13 on Linux x86_64, with `python -m larmorx_validation parity confounds-linalg --tier standard`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

846 comparison lines in 32 script runs (e1 14, e2 14, e3 4).

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
| check_blas | 2 | 2 | 0 | 0 | 0 |
| check_fft | 6 | 6 | 0 | 0 | 0 |
| check_gelsd | 4 | 4 | 0 | 0 | 0 |
| check_gesdd | 4 | 4 | 0 | 0 | 0 |
| check_gesvd | 4 | 4 | 0 | 0 | 0 |
| check_lapack | 2 | 2 | 0 | 0 | 0 |
| check_numpy_reductions | 6 | 6 | 0 | 0 | 0 |
| check_svd | 4 | 4 | 0 | 0 | 0 |

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
| `blas-quick-e2` | pass | 27/27 | OpenBLAS's dgemm, dsyrk, dgemv, dtrmm, dger, dtrmv, the level-1 routines and numpy's @ and dot against numpy's library … |
| `blas-quick-e1` | pass | 27/27 | OpenBLAS's dgemm, dsyrk, dgemv, dtrmm, dger, dtrmv, the level-1 routines and numpy's @ and dot against numpy's library … |
| `lapack-quick-e2` | pass | 48/48 | LAPACK's blocked QR and LQ (DGEQRF, DGELQF, DLARFT, DLARFB), DORGQR/DORGLQ/DORMQR/DORMLQ, DORMBR/DORGBR and the bidiago… |
| `lapack-quick-e1` | pass | 48/48 | LAPACK's blocked QR and LQ (DGEQRF, DGELQF, DLARFT, DLARFB), DORGQR/DORGLQ/DORMQR/DORMLQ, DORMBR/DORGBR and the bidiago… |
| `svd-quick-e2` | pass | 27/27 | the divide-and-conquer bidiagonal SVD (DBDSDC, DLASD0 to DLASD5, DLASDQ, DLAED6) against numpy's library (quick tier, n… |
| `svd-quick-e1` | pass | 27/27 | the divide-and-conquer bidiagonal SVD (DBDSDC, DLASD0 to DLASD5, DLASDQ, DLAED6) against numpy's library (quick tier, n… |
| `svd-control-quick-e2` | pass | 17/17 | the negative control of check_svd.py: larmorx told the other library release must differ (quick tier, numpy 2.3.5 / Ope… |
| `svd-control-quick-e1` | pass | 17/17 | the negative control of check_svd.py: larmorx told the other library release must differ (quick tier, numpy 2.5.4 / Ope… |
| `gesdd-quick-e2` | pass | 30/30 | DGESDD's four thin paths, numpy's svd(full_matrices=False) and pinv up to CompCor's largest real shapes (quick tier, nu… |
| `gesdd-quick-e1` | pass | 30/30 | DGESDD's four thin paths, numpy's svd(full_matrices=False) and pinv up to CompCor's largest real shapes (quick tier, nu… |
| `gesdd-control-quick-e2` | pass | 18/18 | the negative control of check_gesdd.py: larmorx told the other library release must differ (quick tier, numpy 2.3.5 / O… |
| `gesdd-control-quick-e1` | pass | 18/18 | the negative control of check_gesdd.py: larmorx told the other library release must differ (quick tier, numpy 2.5.4 / O… |
| `gelsd-quick-e2` | pass | 106/106 | DGELSD's three paths, DLALSD's two (DLASDA, DLALS0, DLALSA), DLASRT and numpy's lstsq, the cosine filter's real calls i… |
| `gelsd-quick-e1` | pass | 106/106 | DGELSD's three paths, DLALSD's two (DLASDA, DLALS0, DLALSA), DLASRT and numpy's lstsq, the cosine filter's real calls i… |
| `gelsd-control-quick-e2` | pass | 80/80 | the negative control of check_gelsd.py: larmorx told the other library release must differ (quick tier, numpy 2.3.5 / O… |
| `gelsd-control-quick-e1` | pass | 80/80 | the negative control of check_gelsd.py: larmorx told the other library release must differ (quick tier, numpy 2.5.4 / O… |
| `gesvd-quick-e2` | pass | 83/83 | SciPy's DGESVD fallback (DBDSQR, OpenBLAS 0.3.28's DLARFT) and nipype's fallback_svd and _full_rank against SciPy's own… |
| `gesvd-quick-e1` | pass | 83/83 | SciPy's DGESVD fallback (DBDSQR, OpenBLAS 0.3.28's DLARFT) and nipype's fallback_svd and _full_rank against SciPy's own… |
| `gesvd-control-quick-e2` | pass | 87/87 | the negative control of check_gesvd.py: larmorx told the other library release must differ (quick tier, numpy 2.3.5 / O… |
| `gesvd-control-quick-e1` | pass | 87/87 | the negative control of check_gesvd.py: larmorx told the other library release must differ (quick tier, numpy 2.5.4 / O… |
| `reductions-quick-e1` | pass | 18/18 | numpy's ordered sums, means, variances, cumsum, extremes, percentiles, searchsorted, nan_to_num, diff and the Legendre … |
| `reductions-quick-e2` | pass | 18/18 | numpy's ordered sums, means, variances, cumsum, extremes, percentiles, searchsorted, nan_to_num, diff and the Legendre … |
| `reductions-quick-e3` | pass | 18/18 | numpy's ordered sums, means, variances, cumsum, extremes, percentiles, searchsorted, nan_to_num, diff and the Legendre … |
| `reductions-control-quick-e1` | pass | 30/30 | the negative control of check_numpy_reductions.py: larmorx told the other library release must differ (quick tier, nump… |
| `reductions-control-quick-e2` | pass | 30/30 | the negative control of check_numpy_reductions.py: larmorx told the other library release must differ (quick tier, nump… |
| `reductions-control-quick-e3` | pass | 30/30 | the negative control of check_numpy_reductions.py: larmorx told the other library release must differ (quick tier, fMRI… |
| `fft-quick-e1` | pass | 81/81 | SciPy's float32 FFTs (1.15's pocketfft, 1.18.1's ducc0) and nitime's fftconvolve under nipype's DVARS (quick tier, nump… |
| `fft-quick-e2` | pass | 81/81 | SciPy's float32 FFTs (1.15's pocketfft, 1.18.1's ducc0) and nitime's fftconvolve under nipype's DVARS (quick tier, nump… |
| `fft-quick-e3` | pass | 81/81 | SciPy's float32 FFTs (1.15's pocketfft, 1.18.1's ducc0) and nitime's fftconvolve under nipype's DVARS (quick tier, fMRI… |
| `fft-control-quick-e1` | pass | 80/80 | the negative control of check_fft.py: larmorx told the other library release must differ (quick tier, numpy 2.5.4 / Ope… |
| `fft-control-quick-e2` | pass | 80/80 | the negative control of check_fft.py: larmorx told the other library release must differ (quick tier, numpy 2.3.5 / Ope… |
| `fft-control-quick-e3` | pass | 80/80 | the negative control of check_fft.py: larmorx told the other library release must differ (quick tier, fMRIPrep 25.2.6's… |
