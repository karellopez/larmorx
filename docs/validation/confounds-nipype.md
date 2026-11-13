# Parity: nipype's confound algorithms (DVARS, CompCor, TSNR, the non-steady-state detector, framewise displacement) and nipype's interface runner (larmorx nipype run)

**All cases agree.** 25 cases on the `standard` tier: 25 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `lx.confounds` (`crates/larmorx-confounds`), niworkflows' `robust_acompcor` and `robust_tcompcor`, `lx.nipype`
- **Reference:** nipype 1.12.0 and nitime in-process in the newest and the shared library releases and in fMRIPrep 25.2.6's image, fMRIPrep 25.2.6's own node files of 21 oracle runs, and nipype 1.12.0's own nipypecli run with niworkflows 1.15.4 and fMRIPrep 25.2.7
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `standard`
- **Generated:** 2026-11-13 on Linux x86_64, with `python -m larmorx_validation parity confounds-nipype --tier standard`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

289 comparison lines in 25 script runs (e1 7, e2 4, e3 6, exact 1, left 1, nothing 1, notices 1, rows 1, shape 1, text 1, tolerance 1).

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
| check_compcor | 5 | 5 | 0 | 0 | 0 |
| check_dvars | 6 | 6 | 0 | 0 | 0 |
| check_nipype_cli | 1 | 1 | 0 | 0 | 0 |
| check_nipype_misc | 5 | 5 | 0 | 0 | 0 |
| difftest | 8 | 8 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.1.0rc1 (34272ae120de) |
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
| `dvars-quick-e1` | pass | 20/20 | numpy's complex64 product, nitime's crosscov and AR(1), SciPy's 1 x 1 solve, nipype's compute_dvars and ComputeDVARS, f… |
| `dvars-quick-e2` | pass | 20/20 | numpy's complex64 product, nitime's crosscov and AR(1), SciPy's 1 x 1 solve, nipype's compute_dvars and ComputeDVARS, f… |
| `dvars-quick-e3` | pass | 20/20 | numpy's complex64 product, nitime's crosscov and AR(1), SciPy's 1 x 1 solve, nipype's compute_dvars and ComputeDVARS, f… |
| `dvars-control-quick-e1` | pass | 16/16 | the negative control of check_dvars.py: larmorx told the other library release must differ (quick tier, numpy 2.5.4 / O… |
| `dvars-control-quick-e2` | pass | 16/16 | the negative control of check_dvars.py: larmorx told the other library release must differ (quick tier, numpy 2.3.5 / O… |
| `dvars-control-quick-e3` | pass | 16/16 | the negative control of check_dvars.py: larmorx told the other library release must differ (quick tier, fMRIPrep 25.2.6… |
| `compcor-quick-e1` | pass | 30/30 | nipype's cosine and Legendre filters, compute_noise_components, CompCor, ACompCor and TCompCor over their options, fMRI… |
| `compcor-quick-e2` | pass | 30/30 | nipype's cosine and Legendre filters, compute_noise_components, CompCor, ACompCor and TCompCor over their options, fMRI… |
| `compcor-quick-e3` | pass | 30/30 | nipype's cosine and Legendre filters, compute_noise_components, CompCor, ACompCor and TCompCor over their options, fMRI… |
| `compcor-control-quick-e1` | pass | 29/29 | the negative control of check_compcor.py: larmorx told the other library release must differ (quick tier, numpy 2.5.4 /… |
| `compcor-control-quick-e3` | pass | 29/29 | the negative control of check_compcor.py: larmorx told the other library release must differ (quick tier, fMRIPrep 25.2… |
| `misc-quick-e1` | pass | 68/68 | nipype's TSNR, NonSteadyStateDetector, FramewiseDisplacement (every source, NIPY included) and niworkflows' RobustAComp… |
| `misc-quick-e2` | pass | 69/69 | nipype's TSNR, NonSteadyStateDetector, FramewiseDisplacement (every source, NIPY included) and niworkflows' RobustAComp… |
| `misc-quick-e3` | pass | 69/69 | nipype's TSNR, NonSteadyStateDetector, FramewiseDisplacement (every source, NIPY included) and niworkflows' RobustAComp… |
| `misc-control-quick-e1` | pass | 46/46 | the negative control of check_nipype_misc.py: larmorx told the other library release must differ (quick tier, numpy 2.5… |
| `misc-control-quick-e3` | pass | 46/46 | the negative control of check_nipype_misc.py: larmorx told the other library release must differ (quick tier, fMRIPrep … |
| `cli-quick-e1` | pass | 18/18 | larmorx nipype run against nipype's own nipypecli run: the click layer, the 61 helps, argparse's and traits' errors, th… |
| `difftest/compcor-no-volume-left` | pass | 2/2 | random testing: DN fix pass |
| `difftest/compcor-pipe44-later-masks-add-nothing` | pass | 2/2 | ACompCor whose second mask gives no component and whose later masks give none either (the trial's notice verdicts): the… |
| `difftest/compcor-pipe44-reaches-other-shape` | pass | 2/2 | random testing: DN fix pass |
| `difftest/cosine-filter-glibc07-exact` | pass | 2/2 | cosine_filter whose glibc and correctly rounded cosines differ only before the product with sqrt(2 / T) rounds them to … |
| `difftest/cosine-filter-wide-basis-dbdsqr-tolerance` | pass | 2/2 | cosine_filter with a cut-off of 5 s at a TR of 3 s on 370 volumes: a cosine basis of 444 columns, wider than the series… |
| `difftest/dvars-constant-series-notices` | pass | 2/2 | compute_dvars of an all-zero series with remove_zerovariance=False (the trial's notice verdicts): PIPE-50's and PIPE-46… |
| `difftest/regress-poly-fortran-rows` | pass | 2/2 | regress_poly of Fortran-ordered rows (387 x 69 float32, degree 0): numpy's pinv(X).dot(data.T) takes gemv on a C-ordere… |
| `difftest/tcompcor-refused-degree-text` | pass | 2/2 | TCompCor with regress_poly_degree=0: traits' TraitError text ('The 'regress_poly_degree' trait of a TCompCorInputSpec i… |
