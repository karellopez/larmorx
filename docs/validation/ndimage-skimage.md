# Parity: SciPy's ndimage, SciPy's inv, scikit-image and numpy on the field maps' paths

**All cases agree.** 10 cases on the `standard` tier: 10 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `lx.ndimage` (`crates/larmorx-ndimage`), `lx.skimage`, `larmorx.transforms._linalg.scipy_inv`, `larmorx_core::numpy`
- **Reference:** SciPy, numpy (on OpenBLAS's Haswell kernels, one thread) and scikit-image 0.26.0 in-process, in the environment's releases (numpy 2.5.4)
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `standard`
- **Generated:** 2026-11-09 on Linux x86_64, with `python -m larmorx_validation parity ndimage-skimage --tier standard --python newest`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

333 comparison lines in 10 script runs.
The originals' own step (numpy 2.3.5 (OpenBLAS 0.3.30), SciPy 1.15.3 → numpy 2.5.4 (OpenBLAS 0.3.34), SciPy 1.18.1): 287 comparisons, 126 changed with a known reason, 0 varying in the shared releases, the rest identical.

## Thresholds

| Quantity | Requirement |
|---|---|
| Every array, file (decompressed), header, warning, message and command line | bit-identical |
| upstream_defects='reproduce' | the original's result bit for bit, its errors and warnings |
| A corrected defect (the default) | the correction the register names, checked against an independent construction; an EXPECTED line names it |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| check_epi_mask | 1 | 1 | 0 | 0 | 0 |
| check_match_histogram | 1 | 1 | 0 | 0 | 0 |
| check_ndimage | 2 | 2 | 0 | 0 | 0 |
| check_numpy | 1 | 1 | 0 | 0 | 0 |
| check_regrid | 1 | 1 | 0 | 0 | 0 |
| check_scipy_inv | 2 | 2 | 0 | 0 | 0 |
| check_skimage | 2 | 2 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (4c39679966e8) |
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
| `ndimage-quick` | pass | 181/181 | SciPy's ndimage filters, morphology, labels and sums, a quick draw of every section |
| `ndimage` | pass | 181/181 | SciPy's ndimage: every filter, binary and grey morphology, labels, sums, their arguments as SciPy's and SCI-09 to SCI-3… |
| `numpy` | pass | 15/15 | numpy's percentiles, medians, clips and float32 exp on seeded arrays |
| `skimage` | pass | 19/19 | scikit-image's ball and threshold_otsu, numpy's dot, norm and histogram |
| `skimage-random-walker` | pass | 14/14 | scikit-image's random_walker (both SciPy orders, copy=False, isolated labels) |
| `scipy-inv-quick` | pass | 88/88 | SciPy's inv on 2,000 seeded matrices per family and the edge cases |
| `scipy-inv` | pass | 90/90 | SciPy's inv on 20,000 seeded matrices per family, the edge cases and the oracle runs' image affines |
| `gaussian-filters` | pass | 47/47 | SciPy's correlate1d and Gaussian filters (derivatives, complex input, long-double sigma) |
| `watershed-com` | pass | 18/18 | SciPy's watershed_ift (SCI-W1 to SCI-W4 both ways) and center_of_mass |
| `match-histograms` | pass | 7/7 | scikit-image's match_histograms on arrays (channels, types, errors) |
