# Parity: SciPy's ndimage, SciPy's inv, scikit-image and numpy on the field maps' paths

**All cases agree.** 1 cases on the `full` tier: 1 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `lx.ndimage` (`crates/larmorx-ndimage`), `lx.skimage`, `larmorx.transforms._linalg.scipy_inv`, `larmorx_core::numpy`
- **Reference:** SciPy, numpy (on OpenBLAS's Haswell kernels, one thread) and scikit-image 0.26.0 in-process, in the environment's releases (numpy 2.5.4)
- **Test data:** larmorx-testdata `52e371aab0ac`, tier `full`
- **Generated:** 2026-11-09 on Linux x86_64, with `python -m larmorx_validation parity ndimage-skimage --tier full --case '^numpy-expf$' --python newest`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

8 comparison lines in 1 script runs.
The originals' own step (numpy 2.3.5 (OpenBLAS 0.3.30), SciPy 1.15.3 → numpy 2.5.4 (OpenBLAS 0.3.34), SciPy 1.18.1): 8 comparisons, 1 changed with a known reason, 0 varying in the shared releases, the rest identical.

## Thresholds

| Quantity | Requirement |
|---|---|
| Every array, file (decompressed), header, warning, message and command line | bit-identical |
| upstream_defects='reproduce' | the original's result bit for bit, its errors and warnings |
| A corrected defect (the default) | the correction the register names, checked against an independent construction; an EXPECTED line names it |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| check_numpy | 1 | 1 | 0 | 0 | 0 |

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
| `numpy-expf` | pass | 19/19 | numpy's float32 exp on every float32 value (with numpy's AVX-512 kernels and without), its working files in the case's … |
