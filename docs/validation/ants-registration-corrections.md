# Parity: antsRegistration: what the corrections decided on 2026-10-21 change

**All cases agree.** 19 cases on the `standard` tier: 19 pass, 0 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsRegistration` / `lx.ants.registration`, default mode against `reproduce` (ANTs' result, which the ants-registration record compares with ANTs)
- **Reference:** larmorx in `reproduce` mode; for ITK-35, antsRegistration and antsApplyTransforms from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0 (one thread, ANTS_RANDOM_SEED=42)
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-22 on Linux x86_64, with `python -m larmorx_validation parity ants-registration-corrections --tier standard`

## Thresholds

| Quantity | Requirement |
|---|---|
| Corrections named | every correction the case exercises is named in a notice |
| Effects | reported: translation errors, field differences, fits, distances |
| ANTS-78 | the corrected stage with twice the mesh is ANTs' stage byte for byte |
| ITK-35 | `reproduce` fails where ANTs fails; the default runs |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bspline-chain | 5 | 5 | 0 | 0 | 0 |
| igdm | 6 | 6 | 0 | 0 | 0 |
| jhct | 4 | 4 | 0 | 0 | 0 |
| mesh | 4 | 4 | 0 | 0 | 0 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (393b190e4ce6) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The phantoms are the ants-registration suite's (`gen:reg-*`, `gen:ps-*`).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `igdm/one-neighbour` | pass | 6/6 | IGDM, given sigmas (50, 5), one neighbour, central differences: the true derivative alone (ANTS-68) |
| `igdm/one-neighbour-gradient-sigma` | pass | 6/6 | as above with gradientSigma 1 mm: Gaussian gradients (ANTS-67, ANTS-68) |
| `igdm/five-neighbours` | pass | 6/6 | kNeighborhood 5, central differences (ANTS-67, ANTS-68) |
| `igdm/five-neighbours-gradient-sigma` | pass | 6/6 | kNeighborhood 5, gradientSigma 1.5 mm (ANTS-67, ANTS-68) |
| `igdm/estimated-sigmas` | pass | 6/6 | estimated sigmas: the distance sigma's floor applies (ANTS-70, with ANTS-65, ANTS-67, ANTS-68) |
| `igdm/same-sets-half-voxel` | pass | 6/6 | an image with itself, moved by half a voxel: every nearest distance 1 mm, ANTs' sigma 0 and NaN parameters (ANTS-70) |
| `jhct/isotropic-sigma-2` | pass | 6/6 | JHCT, isotropic, sigma 2, alpha 1.1: the total number of points alone (ITK-24) |
| `jhct/isotropic-sigma-1` | pass | 6/6 | JHCT, isotropic, sigma 1: ITK-24 and ITK-23 |
| `jhct/anisotropic-sigma-1` | pass | 6/6 | JHCT, anisotropic, sigma 1, alpha 1.1: the Parzen kernel sigma (ITK-29, with ITK-24 and ITK-26) |
| `jhct/anisotropic-sigma-2` | pass | 6/6 | JHCT, anisotropic, sigma 2, alpha 1.5 (ITK-29, ITK-24, ITK-26) |
| `mesh/bspline-field-3` | pass | 8/8 | BSplineDisplacementField 3x3x3 over two levels: ANTs' stage is the corrected one with 6x6x6; the coarse corrected level… |
| `mesh/bspline-field-5-total-4` | pass | 8/8 | BSplineDisplacementField 5x5x5 with a total mesh of 4 |
| `mesh/bspline-field-knots` | pass | 7/7 | BSplineDisplacementField with a knot spacing of 12 mm |
| `mesh/bspline-exponential-4` | pass | 8/8 | BSplineExponential 4x4x4 (the sigmas in mm: ANTS-80 aside) |
| `chain/bspline-then-affine` | pass | 12/12 | a BSpline stage (mesh 2x2x2) then Affine (ANTs drops the affine stage, ANTS-49) |
| `chain/bspline-then-rigid` | pass | 12/12 | a BSpline stage then Rigid |
| `chain/initial-bspline-then-rigid` | pass | 12/12 | -r a B-spline transform (smooth coefficients up to 2 mm), then Rigid |
| `chain/initial-bspline-then-affine` | pass | 12/12 | -r a B-spline transform, then Affine |
| `chain/bspline-then-bspline` | pass | 12/12 | two BSpline stages (ANTs fails at the second) |
