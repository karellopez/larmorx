# Parity: antsRegistration (every stage: linear, SyN, B-spline, displacement-field and velocity-field)

**All cases agree.** 124 cases on the `full` tier: 103 pass, 16 rejected by both, 5 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsRegistration` / `lx.ants.registration` (crates larmorx-registration, larmorx-ants)
- **Reference:** antsRegistration from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread and ANTS_RANDOM_SEED=42
- **Test data:** larmorx-testdata `00b862132e9c`, tier `full`
- **Generated:** 2026-10-23 on Linux x86_64, with `python -m larmorx_validation parity ants-registration --tier full --case '^((exponential|bspline-exponential|time-varying|time-varying-bspline|corrections)/|real-(exponential|bspline-exponential|time-varying|time-varying-bspline)(-small)?/)'`

103 cases compared: 77 identical to the unmodified ANTs (exit status, standard output with every DIAGNOSTIC line, standard error, every file), 26 identical once ANTs uses CORE-MATH's correctly rounded `exp`, `log`, `sin` and `cos` (glibc's are not always correctly rounded).

Thread band, 6 cases: ANTs at 2, 4, 8 threads differs from its own one-thread result by up to 0.487 in a transform parameter (`exponential/meansquares` 0, `bspline-exponential/meansquares` 3.21e-08, `time-varying/meansquares` 0, `time-varying-bspline/meansquares` 1.91e-06, `real-exponential-small/ds000005-sub-01/template` 0.219, `real-time-varying-small/ds000005-sub-01/template` 0.487); larmorx gives the one-thread result at any thread count, within 0.265 of ANTs at one thread.

- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-65: 2 case(s) named it, 2 with changed values (largest difference 0.999): `time-varying-bspline/igdm`, `corrections/igdm-same-sets`
  - ANTS-67: 4 case(s) named it, 4 with changed values (largest difference 2.13): `time-varying-bspline/igdm`, `corrections/igdm-k-neighbourhood`, `corrections/igdm-true-gradient`, `corrections/igdm-same-sets`
  - ANTS-68: 4 case(s) named it, 4 with changed values (largest difference 2.13): `time-varying-bspline/igdm`, `corrections/igdm-k-neighbourhood`, `corrections/igdm-true-gradient`, `corrections/igdm-same-sets`
  - ANTS-70: 2 case(s) named it, 2 with changed values (largest difference 0.999): `time-varying-bspline/igdm`, `corrections/igdm-same-sets`
  - ANTS-76: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (files ['rNGenericAffine.mat']): `corrections/bspline-then-affine`, `corrections/initial-bspline-then-rigid`
  - ANTS-78: 16 case(s) named it, 16 with changed values (largest difference 2.16): `bspline-exponential/meansquares`, `bspline-exponential/velocity-mesh`, `bspline-exponential/knot-spacing`, `bspline-exponential/cc`, `bspline-exponential/float`, `bspline-exponential/2d`, `bspline-exponential/4d`, `bspline-exponential/masks` …
  - ANTS-79: 2 case(s) named it, 2 with changed values (largest difference 0.000366): `time-varying/two-stages`, `time-varying/then-syn`
  - ANTS-80: 34 case(s) named it, 34 with changed values (largest difference 0.42): `exponential/meansquares`, `exponential/no-collapse`, `exponential/short-name`, `exponential/collapse`, `exponential/composite`, `exponential/integration-steps`, `exponential/cc`, `exponential/mattes` …
  - ANTS-81: 1 case(s) named it, 1 with changed values (largest difference 0.00372): `exponential/short-name`
  - ITK-18: 1 case(s) named it, 1 with changed values (largest difference 0.00512): `exponential/mi2`
  - ITK-24: 3 case(s) named it, 3 with changed values (largest difference 0.0178): `time-varying-bspline/jhct`, `corrections/jhct-isotropic-sigma-2`, `corrections/jhct-anisotropic`
  - ITK-26: 1 case(s) named it, 1 with changed values (largest difference 0.0178): `corrections/jhct-anisotropic`
  - ITK-29: 1 case(s) named it, 1 with changed values (largest difference 0.0178): `corrections/jhct-anisotropic`
  - ITK-30: 1 case(s) named it, 1 with changed values (largest difference 0.000527): `exponential/gc`
  - ITK-32: 5 case(s) named it, 5 with changed values (largest difference 0.00913): `time-varying-bspline/icp`, `time-varying-bspline/pse`, `time-varying-bspline/jhct`, `time-varying-bspline/cc-and-icp`, `time-varying-bspline/meansquares-icp-cc`
  - ITK-35: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (files ['rNGenericAffine.mat']): `corrections/bspline-then-affine`, `corrections/initial-bspline-then-rigid`
  - ITK-36: 2 case(s) named it, 2 with changed values (largest difference 2.17e-05): `time-varying-bspline/cc-and-icp`, `time-varying-bspline/meansquares-icp-cc`
  - ITK-37: 6 case(s) named it, 6 with changed values (largest difference 0.0965): `time-varying-bspline/icp`, `time-varying-bspline/pse`, `time-varying-bspline/jhct`, `time-varying-bspline/igdm`, `time-varying-bspline/cc-and-icp`, `time-varying-bspline/meansquares-icp-cc`
  - ITK-39: 1 case(s) named it, 1 with changed values (largest difference 0.213): `time-varying/fixed-initial`
  - ITK-41: 2 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (files ['rNInverseWarp.nii.gz']): `exponential/zero-iterations-prefix`, `time-varying/zero-iterations-prefix`
  - ITK-42: 1 case(s) named it, 1 with changed values (largest difference 2.16): `corrections/bspline-field-coarse`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | identical |
| Standard output | identical after masking addresses, times and output paths |
| Standard error | identical (larmorx's D13 notices aside) |
| Files | the same files; transforms byte for byte (HDF5: every dataset), images bit for bit |
| glibc's maths | where ANTs as built differs, ANTs with CORE-MATH preloaded must be identical |
| Thread band | reported: ANTs at 2, 4, 8 threads against 1 |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bspline-exponential | 11 | 9 | 1 | 1 | 0 |
| corrections | 9 | 8 | 1 | 0 | 0 |
| exponential | 31 | 23 | 6 | 2 | 0 |
| real-bspline-exponential | 1 | 1 | 0 | 0 | 0 |
| real-bspline-exponential-small | 5 | 5 | 0 | 0 | 0 |
| real-exponential | 1 | 1 | 0 | 0 | 0 |
| real-exponential-small | 5 | 5 | 0 | 0 | 0 |
| real-time-varying-bspline-small | 5 | 5 | 0 | 0 | 0 |
| real-time-varying-small | 5 | 5 | 0 | 0 | 0 |
| time-varying | 25 | 19 | 5 | 1 | 0 |
| time-varying-bspline | 26 | 22 | 3 | 1 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `exponential/then-affine` | the affine stage after a displacement field samples its scales' 1000 voxels from the clock (ANTS-72): two ANTs runs differ |
| `exponential/regular-sampling` | the stage never sets its metric's sampling seed (ANTS-82): ANTs samples from the clock, two runs differ; larmorx samples with the registration's seed |
| `bspline-exponential/random-sampling` | the stage never sets its metric's sampling seed (ANTS-82): ANTs samples from the clock, two runs differ; larmorx samples with the registration's seed |
| `time-varying/regular-sampling` | the stage never sets its metric's sampling seed (ANTS-82): ANTs samples from the clock, two runs differ; larmorx samples with the registration's seed |
| `time-varying-bspline/mesh-0` | ANTs computes control-point spacings of infinity, prints vnl's SVD warnings and fails with an exception on a parametric domain of no span (ANTS-77); larmorx stops with an error after the same messages |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `exponential/print-similarity` | --print-similarity-measure-interval: the observer's copy of the field fails (ANTS-75) | 1 |
| `exponential/interval-volumes` | --write-interval-volumes: the observer's copy of the field fails (ANTS-75) | 1 |
| `exponential/save-state` | --save-state: no state (ANTS-50) | because the the output transform will contain the this processes initializer |
| `exponential/zero-iterations` | no iteration and one level: the transform never integrates, its inverse has no field (ITK-41) | Description: ITK ERROR: ConstantVelocityFieldTransform(0x61cf07d5b680): No displacement field is specified. |
| `exponential/icp-first` | ICP first: ANTs allocates the velocity field on a null image | larmorx: antsRegistration: an Exponential stage whose first metric is a point-set metric: ANTs allocates the velocity field on the null image of its first metr… |
| `exponential/missing-parameter` | Exponential[0.5,3]: the velocity variance is required | ERROR: Unable to convert '' to type 'f' as float |
| `bspline-exponential/mesh-dimension` | a 2D mesh in 3D: ERROR | 1 |
| `time-varying/icp-second` | CC and ICP | 1 |
| `time-varying/zero-iterations` | no iteration and warped images: ANTs maps points through a field without an interpolator input (ITK-41) | larmorx: upstream defect ITK-41 (ITK) reproduced: stage 0: the time-varying velocity field transform never integrated its velocity field: ITK crashes at the ne… |
| `time-varying/zero-time-indices` | 0 time indices: ANTs divides by 0 | larmorx: antsRegistration: a TimeVaryingVelocityField stage with 0 time indices: ANTs divides the velocity field's voxels by 0; ANTs crashes here, larmorx stops |
| `time-varying/icp-first` | ICP first: ANTs allocates the velocity field on a null image | larmorx: antsRegistration: a TimeVaryingVelocityField stage whose first metric is a point-set metric: ANTs allocates the velocity field on the null image of it… |
| `time-varying/missing-parameter` | TimeVaryingVelocityField[0.5,2,3]: the other variances are required | ERROR: Unable to convert '' to type 'f' as float |
| `time-varying-bspline/icp-ras` | ICP alone on RAS-stored phantoms: ANTs fits the points outside the parametric domain and throws (ITK-38) | 1 |
| `time-varying-bspline/icp-then-cc` | ICP first, then CC: ANTs cannot convert the multi-metric queue | 1 |
| `time-varying-bspline/mesh-dimension` | a mesh without its time size: the message, then failure | 1 |
| `corrections/bspline-then-bspline` | two BSpline stages (ITK-35; ANTs fails) | 1 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (a9401a69f759) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

Inputs named `gen:<name>` are built by this suite (seeded phantoms); the real cases re-run fMRIPrep 25.2.6's own command lines from the oracle runs.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `exponential/meansquares` | pass | 10/10 | Exponential (update variance 3, velocity variance 0.5), MeanSquares, 2 levels, warped images |
| `exponential/no-collapse` | pass | 11/11 | -z 0 after an affine stage: 0Affine.mat, 1Warp, 1InverseWarp and 1VelocityField |
| `exponential/short-name` | pass | 9/9 | -t Exp[...] with -z 0: no InverseWarp file (ANTS-81) |
| `exponential/collapse` | pass | 10/10 | an affine stage, then the velocity field: collapsed |
| `exponential/composite` | pass | 9/9 | -a 1 |
| `exponential/integration-steps` | pass | 9/9 | 4 integration steps, no velocity smoothing |
| `exponential/sigmas-in-mm` | pass | 7/7 | smoothing sigmas given in mm (as ANTs takes voxels, ANTS-80) |
| `exponential/cc` | pass | 9/9 | CC (radius 2) |
| `exponential/mattes` | pass | 17/17 | Mattes (24 bins) |
| `exponential/mi2` | pass | 17/17 | MI2 |
| `exponential/gc` | pass | 9/9 | GC (ITK-30) |
| `exponential/demons` | pass | 9/9 | Demons |
| `exponential/multi` | pass | 9/9 | CC (0.7) and Mattes (0.3) |
| `exponential/masks` | pass | 9/9 | -x [mask,mask] |
| `exponential/float` | pass | 9/9 | --float 1 |
| `exponential/2d` | pass | 9/9 | 2D |
| `exponential/4d` | pass | 8/8 | 4D, MeanSquares |
| `exponential/oblique` | pass | 9/9 | oblique grids |
| `exponential/restrict` | pass | 9/9 | -g 1x1x0 |
| `exponential/initial-field` | pass | 9/9 | -r a displacement field |
| `exponential/per-stage` | pass | 7/7 | -i 1 -a 1 -z 0 after an affine stage |
| `exponential/then-affine` | expected-divergence | 1/1 | an affine stage after the velocity field |
| `exponential/regular-sampling` | expected-divergence | 1/1 | MeanSquares, regular sampling 0.5 |
| `exponential/icp-second` | pass | 7/7 | MeanSquares and ICP |
| `exponential/print-similarity` | both-error | 2/2 | --print-similarity-measure-interval: the observer's copy of the field fails (ANTS-75) |
| `exponential/interval-volumes` | both-error | 2/2 | --write-interval-volumes: the observer's copy of the field fails (ANTS-75) |
| `exponential/save-state` | both-error | 2/2 | --save-state: no state (ANTS-50) |
| `exponential/zero-iterations` | both-error | 2/2 | no iteration and one level: the transform never integrates, its inverse has no field (ITK-41) |
| `exponential/zero-iterations-prefix` | pass | 6/6 | no iteration, no warped images: written as ANTs writes it (ITK-41) |
| `exponential/icp-first` | both-error | 4/4 | ICP first: ANTs allocates the velocity field on a null image |
| `exponential/missing-parameter` | both-error | 2/2 | Exponential[0.5,3]: the velocity variance is required |
| `bspline-exponential/meansquares` | pass | 10/10 | BSplineExponential (update mesh 4x4x4), MeanSquares, 2 levels, warped images |
| `bspline-exponential/velocity-mesh` | pass | 9/9 | a velocity-field mesh (2x2x2) and 5 integration steps |
| `bspline-exponential/knot-spacing` | pass | 9/9 | a knot spacing (30 mm), -z 0 |
| `bspline-exponential/cc` | pass | 9/9 | CC |
| `bspline-exponential/float` | pass | 7/7 | --float 1 |
| `bspline-exponential/2d` | pass | 9/9 | 2D |
| `bspline-exponential/4d` | pass | 7/7 | 4D |
| `bspline-exponential/masks` | pass | 7/7 | -x [mask,mask] |
| `bspline-exponential/order-2` | pass | 7/7 | order 2 |
| `bspline-exponential/mesh-dimension` | both-error | 2/2 | a 2D mesh in 3D: ERROR |
| `bspline-exponential/random-sampling` | expected-divergence | 1/1 | Mattes, random sampling 0.5 |
| `time-varying/meansquares` | pass | 11/11 | TimeVaryingVelocityField (3 time indices), MeanSquares, 2 levels, -z 0: Warp, InverseWarp, VelocityField |
| `time-varying/short-name` | pass | 10/10 | tvf[...] with time variances (0.5, 0.25) |
| `time-varying/collapse` | pass | 10/10 | an affine stage, then the velocity field: collapsed |
| `time-varying/composite` | pass | 9/9 | -a 1 after an affine stage |
| `time-varying/cc` | pass | 9/9 | CC |
| `time-varying/mattes` | pass | 7/7 | Mattes (24 bins) |
| `time-varying/multi` | pass | 9/9 | CC (0.7) and Mattes (0.3) |
| `time-varying/masks` | pass | 7/7 | -x [mask,mask]: the masks given to the metric unchanged (ITK-40) |
| `time-varying/float` | pass | 9/9 | --float 1 |
| `time-varying/2d` | pass | 10/10 | 2D, 4 time indices |
| `time-varying/4d` | pass | 7/7 | 4D: the velocity field (5D) cannot be written as NIfTI |
| `time-varying/one-time-index` | pass | 9/9 | one time index |
| `time-varying/restrict` | pass | 9/9 | -g 1x0x1 |
| `time-varying/two-stages` | pass | 11/11 | two TimeVaryingVelocityField stages: the number of time indices from the last -t (ANTS-79) |
| `time-varying/then-syn` | pass | 9/9 | a SyN stage after the velocity field |
| `time-varying/per-stage` | pass | 7/7 | -i 1 -a 1 -z 0 after an affine stage |
| `time-varying/fixed-initial` | pass | 9/9 | -q an affine transform: ignored by the method (ITK-39) |
| `time-varying/print-similarity` | pass | 7/7 | --print-similarity-measure-interval: no full-scale similarity in this observer |
| `time-varying/icp-second` | both-error | 2/2 | CC and ICP |
| `time-varying/regular-sampling` | expected-divergence | 1/1 | MeanSquares, regular sampling 0.5 |
| `time-varying/zero-iterations-prefix` | pass | 7/7 | no iteration: the transform never integrates; no warped images (ITK-41) |
| `time-varying/zero-iterations` | both-error | 4/4 | no iteration and warped images: ANTs maps points through a field without an interpolator input (ITK-41) |
| `time-varying/zero-time-indices` | both-error | 4/4 | 0 time indices: ANTs divides by 0 |
| `time-varying/icp-first` | both-error | 4/4 | ICP first: ANTs allocates the velocity field on a null image |
| `time-varying/missing-parameter` | both-error | 2/2 | TimeVaryingVelocityField[0.5,2,3]: the other variances are required |
| `time-varying-bspline/meansquares` | pass | 10/10 | TimeVaryingBSplineVelocityField (mesh 4x4x4x2, 4 time samples), MeanSquares, 2 levels, warped images |
| `time-varying-bspline/float` | pass | 7/7 | --float 1 |
| `time-varying-bspline/2d` | pass | 9/9 | 2D |
| `time-varying-bspline/4d` | pass | 7/7 | 4D |
| `time-varying-bspline/masks` | pass | 7/7 | -x [mask,mask] |
| `time-varying-bspline/fixed-mask-oblique` | pass | 7/7 | a fixed mask on an oblique grid |
| `time-varying-bspline/cc` | pass | 7/7 | CC |
| `time-varying-bspline/multi` | pass | 13/13 | CC (0.7) and Mattes (0.3) |
| `time-varying-bspline/order-2` | pass | 7/7 | order 2 |
| `time-varying-bspline/samples-2` | pass | 7/7 | 2 time samples, mesh 3x3x3x1 |
| `time-varying-bspline/samples-1` | pass | 7/7 | 1 time sample |
| `time-varying-bspline/oblique` | pass | 7/7 | oblique grids |
| `time-varying-bspline/restrict` | pass | 7/7 | -g 1x1x0 |
| `time-varying-bspline/interval-volumes` | pass | 7/7 | --write-interval-volumes: not written by this observer |
| `time-varying-bspline/per-stage` | pass | 7/7 | -i 1 -a 1 -z 0 after an affine stage |
| `time-varying-bspline/zero-iterations` | pass | 9/9 | no iteration on two levels |
| `time-varying-bspline/icp` | pass | 7/7 | ICP alone (identity-direction phantoms): the points fitted directly |
| `time-varying-bspline/pse` | pass | 7/7 | PSE alone |
| `time-varying-bspline/jhct` | pass | 7/7 | JHCT alone |
| `time-varying-bspline/igdm` | pass | 7/7 | IGDM alone |
| `time-varying-bspline/cc-and-icp` | pass | 7/7 | CC and ICP: the image metric warped twice (ITK-36) |
| `time-varying-bspline/meansquares-icp-cc` | pass | 7/7 | MeanSquares, ICP and CC (ITK-36, ITK-37) |
| `time-varying-bspline/icp-ras` | both-error | 2/2 | ICP alone on RAS-stored phantoms: ANTs fits the points outside the parametric domain and throws (ITK-38) |
| `time-varying-bspline/icp-then-cc` | both-error | 2/2 | ICP first, then CC: ANTs cannot convert the multi-metric queue |
| `time-varying-bspline/mesh-dimension` | both-error | 2/2 | a mesh without its time size: the message, then failure |
| `time-varying-bspline/mesh-0` | expected-divergence | 1/1 | a mesh size of 0: ANTs divides by zero and fails on a parametric domain of no span (ANTS-77) |
| `corrections/igdm-k-neighbourhood` | pass | 6/6 | IGDM with kNeighborhood 5 and gradientSigma 1.5, given sigmas (ANTS-67, ANTS-68) |
| `corrections/igdm-true-gradient` | pass | 6/6 | IGDM, given sigmas, one neighbour, central differences: the derivative alone (ANTS-68) |
| `corrections/igdm-same-sets` | pass | 6/6 | IGDM of an image with itself moved by half a voxel: every nearest distance 1 mm, the distance sigma 0, NaN parameters, … |
| `corrections/jhct-isotropic-sigma-2` | pass | 11/11 | JHCT, isotropic, sigma 2: the total number of points alone (ITK-24) |
| `corrections/jhct-anisotropic` | pass | 6/6 | JHCT, anisotropic: the Parzen kernel sigma (ITK-29, ITK-24, ITK-26) |
| `corrections/bspline-then-affine` | pass | 7/7 | an affine stage after a BSpline stage, with a mask (ITK-35; ANTs drops it, ANTS-49) |
| `corrections/bspline-then-bspline` | both-error | 2/2 | two BSpline stages (ITK-35; ANTs fails) |
| `corrections/initial-bspline-then-rigid` | pass | 7/7 | -r a B-spline transform, then Rigid, with a mask (ITK-35) |
| `corrections/bspline-field-coarse` | pass | 7/7 | BSplineDisplacementField 3x3x3 over 2 levels (ANTS-78; corrected, the coarse level needs ITK-42) |
| `real-exponential-small/ds000005-sub-01/template` | pass | 16/16 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall with Exponential[0.25,3,0.5] for its SyN stage, on one vox… |
| `real-exponential/ds000005-sub-01/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall with Exponential[0.25,3,0.5] for its SyN stage, full size |
| `real-bspline-exponential-small/ds000005-sub-01/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall with BSplineExponential[0.25,26,0,0,3] for its SyN stage, … |
| `real-bspline-exponential/ds000005-sub-01/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall with BSplineExponential[0.25,26,0,0,3] for its SyN stage, … |
| `real-time-varying-small/ds000005-sub-01/template` | pass | 16/16 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall with TimeVaryingVelocityField[0.5,4,3,0,0,0] for its SyN s… |
| `real-time-varying-bspline-small/ds000005-sub-01/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall with TimeVaryingBSplineVelocityField[0.5,4x4x4x2,4,3] for … |
| `real-exponential-small/ds001600-sub-1/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds001600-sub-1-noreconall with Exponential[0.25,3,0.5] for its SyN stage, on one voxe… |
| `real-bspline-exponential-small/ds001600-sub-1/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds001600-sub-1-noreconall with BSplineExponential[0.25,26,0,0,3] for its SyN stage, o… |
| `real-time-varying-small/ds001600-sub-1/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds001600-sub-1-noreconall with TimeVaryingVelocityField[0.5,4,3,0,0,0] for its SyN st… |
| `real-time-varying-bspline-small/ds001600-sub-1/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds001600-sub-1-noreconall with TimeVaryingBSplineVelocityField[0.5,4x4x4x2,4,3] for i… |
| `real-exponential-small/ds005040-sub-002/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005040-sub-002-noreconall with Exponential[0.25,3,0.5] for its SyN stage, on one vo… |
| `real-bspline-exponential-small/ds005040-sub-002/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005040-sub-002-noreconall with BSplineExponential[0.25,26,0,0,3] for its SyN stage,… |
| `real-time-varying-small/ds005040-sub-002/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005040-sub-002-noreconall with TimeVaryingVelocityField[0.5,4,3,0,0,0] for its SyN … |
| `real-time-varying-bspline-small/ds005040-sub-002/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005040-sub-002-noreconall with TimeVaryingBSplineVelocityField[0.5,4x4x4x2,4,3] for… |
| `real-exponential-small/ds005454-sub-16/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005454-sub-16-noreconall with Exponential[0.25,3,0.5] for its SyN stage, on one vox… |
| `real-bspline-exponential-small/ds005454-sub-16/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005454-sub-16-noreconall with BSplineExponential[0.25,26,0,0,3] for its SyN stage, … |
| `real-time-varying-small/ds005454-sub-16/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005454-sub-16-noreconall with TimeVaryingVelocityField[0.5,4,3,0,0,0] for its SyN s… |
| `real-time-varying-bspline-small/ds005454-sub-16/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005454-sub-16-noreconall with TimeVaryingBSplineVelocityField[0.5,4x4x4x2,4,3] for … |
| `real-exponential-small/ds005752-sub-ON01016/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005752-sub-ON01016-noreconall with Exponential[0.25,3,0.5] for its SyN stage, on on… |
| `real-bspline-exponential-small/ds005752-sub-ON01016/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005752-sub-ON01016-noreconall with BSplineExponential[0.25,26,0,0,3] for its SyN st… |
| `real-time-varying-small/ds005752-sub-ON01016/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005752-sub-ON01016-noreconall with TimeVaryingVelocityField[0.5,4,3,0,0,0] for its … |
| `real-time-varying-bspline-small/ds005752-sub-ON01016/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds005752-sub-ON01016-noreconall with TimeVaryingBSplineVelocityField[0.5,4x4x4x2,4,3]… |
