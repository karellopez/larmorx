# Parity: 4×4 matrix products and inverses (numpy with OpenBLAS's Haswell kernels)

**All cases agree.** 28 cases on the `standard` tier: 27 pass, 0 rejected by both, 1 expected divergences, 0 failures.

- **Validated:** `larmorx_transform::openblas` (`matmul4`, `inv4`), the point products of `larmorx_transform::nitransforms`, `nitransforms::closest_orthogonal`, through `lx.transforms`
- **Reference:** numpy 2.3.5 with its bundled OpenBLAS, in-process, on the Haswell kernels; nitransforms, sdcflows and fMRIPrep for the mapped points, orientations and loaded transforms
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-11 on Linux x86_64, with `python -m larmorx_validation parity numpy-linalg --tier standard`

**18,143,058 of 18,143,058 compared results are bit-identical** to numpy's on the Haswell kernels (OpenBLAS core reported by threadpoolctl).

| Case | Comparison | Bit-identical |
|---|---|---|
| `products/general` | a @ b (general) | 1,000,000 of 1,000,000 |
| `products/affine` | a @ b (affine) | 1,000,000 of 1,000,000 |
| `products/exponents` | a @ b (exponents) | 1,000,000 of 1,000,000 |
| `products/sparse` | a @ b (sparse) | 1,000,000 of 1,000,000 |
| `products/nonfinite` | a @ b (nonfinite; NaN compared as NaN, not by its bits) | 1,000,000 of 1,000,000 |
| `products/itk-to-ras` | itk-to-ras | 1,000,000 of 1,000,000 |
| `products/head-motion` | head-motion | 1,000,000 of 1,000,000 |
| `inverses/general` | np.linalg.inv (general) | 1,000,000 of 1,000,000 |
| `inverses/affine` | np.linalg.inv (affine) | 1,000,000 of 1,000,000 |
| `inverses/rigid` | np.linalg.inv (rigid) | 1,000,000 of 1,000,000 |
| `inverses/exponents` | np.linalg.inv (exponents) | 1,000,000 of 1,000,000 |
| `inverses/near-singular` | np.linalg.inv (near-singular) | 1,000,000 of 1,000,000 |
| `inverses/permutation` | np.linalg.inv (permutation) | 1,000,000 of 1,000,000 |
| `inverses/ties` | np.linalg.inv (ties) | 1,000,000 of 1,000,000 |
| `inverses/triangular` | np.linalg.inv (triangular) | 1,000,000 of 1,000,000 |
| `inverses/subnormal` | np.linalg.inv (subnormal) | 1,000,000 of 1,000,000 |
| `inverses/singular` | np.linalg.inv (singular) | 1,000,000 of 1,000,000 |
| `inverses/nonfinite` | np.linalg.inv (nonfinite; NaN compared as NaN, not by its bits) | 1,000,000 of 1,000,000 |
| `points/affine-1` | Affine.map, 1 point(s) | 20,000 of 20,000 |
| `points/affine-2` | Affine.map, 2 point(s) | 20,000 of 20,000 |
| `points/affine-3` | Affine.map, 3 point(s) | 20,000 of 20,000 |
| `points/affine-5` | Affine.map, 5 point(s) | 20,000 of 20,000 |
| `points/affine-64` | Affine.map, 64 point(s) | 20,000 of 20,000 |
| `points/field` | DenseField.map (1, 2 and 9 points, on and off the grid) | 1,200 of 1,200 |
| `points/one-voxel` | resample_series onto a one-voxel grid (head motion, 3 volumes) | 200 of 200 |
| `orientation/oblique` | axis codes and reoriented data | 20,000 of 20,000 |
| `orientation/oblique` | new affine | 20,000 of 20,000 |
| `real/resample-series` | loaded matrices (affines, head-motion series, field grids and inverses) | 825 of 825 |
| `real/resample-series` | source affines after ensure_positive_cosines, and their inverses | 25 of 25 |
| `real/resample-series` | head motion in voxel space (ras2vox @ M @ vox2ras) | 808 of 808 |

## Thresholds

| Quantity | Requirement |
|---|---|
| Every matrix, point and affine | bit-identical; with NaN or infinite inputs, NaN is compared as NaN |
| Singular matrices | numpy raises `LinAlgError` exactly when larmorx does |
| Exactly 45° grids | axis decisions may differ (tied cosines; documented divergence) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| inverses | 11 | 11 | 0 | 0 | 0 |
| orientation | 2 | 1 | 0 | 1 | 0 |
| points | 7 | 7 | 0 | 0 | 0 |
| products | 7 | 7 | 0 | 0 | 0 |
| real | 1 | 1 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `orientation/45-degrees` | axis decisions agree for 4300 of 4800 grids rotated by exactly 45°: two cosines tie, and nibabel decides with numpy's LAPACK SVD, larmorx with vnl's |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (b4b2401c25ff) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| threadpoolctl | 3.7.0 |
| nitransforms | 25.1.0 |
| nibabel | 5.4.2 |
| sdcflows | 2.17.1 |
| fmriprep | 26.0.0.dev1+g21a490fb8 |

## Notes

Inputs are seeded and built with elementwise numpy and larmorx's own products, so they do not depend on the BLAS kernel. Other kernels: `python -m larmorx_validation.parity.numpy_linalg kernels` (docs/findings/numpy-blas.md).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `products/general` | pass | 1/1 | a @ b, 1,000,000 general 4×4 pairs |
| `products/affine` | pass | 1/1 | a @ b, 1,000,000 affine 4×4 pairs |
| `products/exponents` | pass | 1/1 | a @ b, 1,000,000 exponents 4×4 pairs |
| `products/sparse` | pass | 1/1 | a @ b, 1,000,000 sparse 4×4 pairs |
| `products/nonfinite` | pass | 1/1 | a @ b, 1,000,000 nonfinite 4×4 pairs |
| `products/itk-to-ras` | pass | 1/1 | nitransforms' ITK-to-RAS conversion LPS·C⁺·M·C⁻·LPS, 1,000,000 float32 transforms |
| `products/head-motion` | pass | 1/1 | fMRIPrep's inv(vox2ras) @ M @ vox2ras, 1,000,000 oblique grids and rigid motions |
| `inverses/general` | pass | 1/1 | np.linalg.inv, 1,000,000 general 4×4 matrices |
| `inverses/affine` | pass | 1/1 | np.linalg.inv, 1,000,000 affine 4×4 matrices |
| `inverses/rigid` | pass | 1/1 | np.linalg.inv, 1,000,000 rigid 4×4 matrices |
| `inverses/exponents` | pass | 1/1 | np.linalg.inv, 1,000,000 exponents 4×4 matrices |
| `inverses/near-singular` | pass | 2/2 | np.linalg.inv, 1,000,000 near-singular 4×4 matrices |
| `inverses/permutation` | pass | 1/1 | np.linalg.inv, 1,000,000 permutation 4×4 matrices |
| `inverses/ties` | pass | 2/2 | np.linalg.inv, 1,000,000 ties 4×4 matrices |
| `inverses/triangular` | pass | 1/1 | np.linalg.inv, 1,000,000 triangular 4×4 matrices |
| `inverses/subnormal` | pass | 1/1 | np.linalg.inv, 1,000,000 subnormal 4×4 matrices |
| `inverses/singular` | pass | 2/2 | np.linalg.inv, 1,000,000 singular 4×4 matrices |
| `inverses/nonfinite` | pass | 1/1 | np.linalg.inv, 1,000,000 nonfinite 4×4 matrices |
| `points/affine-1` | pass | 1/1 | Affine.map vs nitransforms, 20,000 affines, 1 point(s) |
| `points/affine-2` | pass | 1/1 | Affine.map vs nitransforms, 20,000 affines, 2 point(s) |
| `points/affine-3` | pass | 1/1 | Affine.map vs nitransforms, 20,000 affines, 3 point(s) |
| `points/affine-5` | pass | 1/1 | Affine.map vs nitransforms, 20,000 affines, 5 point(s) |
| `points/affine-64` | pass | 1/1 | Affine.map vs nitransforms, 20,000 affines, 64 point(s) |
| `points/field` | pass | 1/1 | DenseField.map vs nitransforms |
| `points/one-voxel` | pass | 1/1 | resample_series vs fMRIPrep's resample_image onto one voxel, 200 runs |
| `orientation/oblique` | pass | 2/2 | ensure_positive_cosines vs sdcflows, 20,000 grids: 48 axis orders and flips, rotated up to 40° |
| `orientation/45-degrees` | expected-divergence | – | ensure_positive_cosines vs sdcflows, 4,800 grids rotated by exactly 45° (tied cosines) |
| `real/resample-series` | pass | 3/3 | every transform file, source grid and head-motion series of the resample-series suite |
