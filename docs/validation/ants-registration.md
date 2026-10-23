# Parity: antsRegistration (every stage: linear, SyN, B-spline, displacement-field and velocity-field)

**All cases agree.** 505 cases on the `standard` tier: 391 pass, 93 rejected by both, 21 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsRegistration` / `lx.ants.registration` (crates larmorx-registration, larmorx-ants)
- **Reference:** antsRegistration from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread and ANTS_RANDOM_SEED=42
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-23 on Linux x86_64, with `python -m larmorx_validation parity ants-registration --tier standard`

391 cases compared: 347 identical to the unmodified ANTs (exit status, standard output with every DIAGNOSTIC line, standard error, every file), 44 identical once ANTs uses CORE-MATH's correctly rounded `exp`, `log`, `sin` and `cos` (glibc's are not always correctly rounded).

Thread band, 36 cases: ANTs at 2, 4, 8 threads differs from its own one-thread result by up to 1.5 in a transform parameter (`smoke/rigid` 8.11e-13, `smoke/affine-float` 0.00488, `metrics/mattes-mi-alias` 8.11e-13, `metrics/mi2` 3.01e-14, `metrics/mi2-dense` 9.71e-17, `metrics/meansquares` 8.67e-14, `metrics/gc` 1.61e-12, `metrics/gc-dense` 1.45e-13, `metrics/cc-sampled` 2.47e-13, `metrics/cc-dense` 5.47e-14, `metrics/mattes-gradient-filter` 8.88e-16, `metrics/cc-gradient-filter` 9.42e-14, `point-sets/icp` 0, `point-sets/pse` 1.11e-16, `point-sets/jhct` 1.11e-16, `point-sets/igdm` 2.12e-22, `syn/cc` 0, `syn/meansquares-total` 0, `syn/mattes` 3.55e-14, `syn-point-sets/icp` 0.0829, `bspline-syn/cc` 2.33e-06, `bspline-syn/knot-spacing` 6.65e-07, `displacement-field/gaussian-cc` 0, `displacement-field/bspline-meansquares` 8.87e-07, `bspline/meansquares` 8.88e-16, `exponential/meansquares` 0, `bspline-exponential/meansquares` 3.21e-08, `time-varying/meansquares` 0, `time-varying-bspline/meansquares` 1.91e-06, `real/ds000005-sub-01/boldref-affine` 5.39e-10, `real/ds005040-sub-002/coregister` 1.46e-12, `real-syn-small/ds000005-sub-01/brain-extraction` 1.5, `real-syn-small/ds000005-sub-01/template` 1.35, `real-bsplinesyn-small/ds000005-sub-01/template` 0.833, `real-exponential-small/ds000005-sub-01/template` 0.276, `real-time-varying-small/ds000005-sub-01/template` 0.959); larmorx gives the one-thread result at any thread count, within 1.63 of ANTs at one thread.

- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-49: 5 case(s) named it, 0 with changed values (largest difference 0), 5 with other changes (exit N -> N; files ['rNGenericAffine.mat']): `metrics/demons`, `metrics/no-overlap`, `point-sets/icp-vtk-no-scalars`, `point-sets/icp-empty`, `point-sets/pse-sigma-zero`
  - ANTS-61: 2 case(s) named it, 2 with changed values (largest difference 0.431): `point-sets/icp-then-igdm`, `point-sets/mattes-then-igdm`
  - ANTS-62: 4 case(s) named it, 3 with changed values (largest difference 0.736): `point-sets/icp-text`, `point-sets/icp-text-no-final-newline`, `point-sets/pse-text-2d`, `point-sets/jhct-text`
  - ANTS-63: 1 case(s) named it, 1 with changed values (largest difference 1.66): `point-sets/icp-vtk-binary`
  - ANTS-65: 9 case(s) named it, 9 with changed values (largest difference 3.23): `point-sets/igdm`, `point-sets/igdm-fixed-initial`, `point-sets/igdm-2d-radius-0`, `point-sets/igdm-then-mattes`, `point-sets/icp-then-igdm`, `point-sets/mattes-then-igdm`, `bspline-syn/igdm`, `time-varying-bspline/igdm` …
  - ANTS-67: 12 case(s) named it, 12 with changed values (largest difference 3.23): `point-sets/igdm`, `point-sets/igdm-sigmas`, `point-sets/igdm-fixed-initial`, `point-sets/igdm-2d-radius-0`, `point-sets/igdm-then-mattes`, `point-sets/icp-then-igdm`, `point-sets/mattes-then-igdm`, `bspline-syn/igdm` …
  - ANTS-68: 12 case(s) named it, 12 with changed values (largest difference 3.23): `point-sets/igdm`, `point-sets/igdm-sigmas`, `point-sets/igdm-fixed-initial`, `point-sets/igdm-2d-radius-0`, `point-sets/igdm-then-mattes`, `point-sets/icp-then-igdm`, `point-sets/mattes-then-igdm`, `bspline-syn/igdm` …
  - ANTS-70: 9 case(s) named it, 9 with changed values (largest difference 3.23): `point-sets/igdm`, `point-sets/igdm-fixed-initial`, `point-sets/igdm-2d-radius-0`, `point-sets/igdm-then-mattes`, `point-sets/icp-then-igdm`, `point-sets/mattes-then-igdm`, `bspline-syn/igdm`, `time-varying-bspline/igdm` …
  - ANTS-73: 3 case(s) named it, 0 with changed values (largest difference 0), 3 with other changes (exit N -> N; files ['rNWarp.nii.gz', 'rNInverseWarp.nii.gz', 'rNWarp.nii.gz']): `syn/initial-scalar`, `syn/initial-scalar-2d`, `syn/initial-field-2d-in-3d`
  - ANTS-76: 7 case(s) named it, 0 with changed values (largest difference 0), 7 with other changes (files ['rNGenericAffine.mat']; files ['rNInverseWarp.nii.gz', 'rNWarp.nii.gz']): `bspline-syn/initial-bspline`, `bspline/then-affine`, `bspline/initial-bspline-syn`, `bspline/initial-bspline-affine`, `bspline/restore-bspline`, `corrections/bspline-then-affine`, `corrections/initial-bspline-then-rigid`
  - ANTS-78: 20 case(s) named it, 20 with changed values (largest difference 3.34): `displacement-field/bspline-meansquares`, `displacement-field/bspline-cc`, `displacement-field/bspline-total-mesh`, `displacement-field/bspline-order-2`, `displacement-field/bspline-knot`, `displacement-field/bspline-mattes`, `displacement-field/bspline-masks`, `displacement-field/bspline-float` …
  - ANTS-79: 2 case(s) named it, 2 with changed values (largest difference 0.000366): `time-varying/two-stages`, `time-varying/then-syn`
  - ANTS-80: 25 case(s) named it, 25 with changed values (largest difference 0.42): `syn/exponential`, `exponential/meansquares`, `exponential/no-collapse`, `exponential/short-name`, `exponential/collapse`, `exponential/composite`, `exponential/integration-steps`, `exponential/cc` …
  - ANTS-81: 1 case(s) named it, 1 with changed values (largest difference 0.00372): `exponential/short-name`
  - ITK-18: 10 case(s) named it, 10 with changed values (largest difference 2.8): `metrics/mi2`, `metrics/mi2-dense`, `metrics/mi2-2d`, `metrics/mi2-4d-gradient-filter`, `metrics/mi2-float`, `syn/mi2`, `bspline-syn/mi2`, `displacement-field/gaussian-mi2` …
  - ITK-22: 2 case(s) named it, 1 with changed values (largest difference 2.19e-06), 1 with other changes (exit N -> N; files ['rNGenericAffine.mat']): `point-sets/icp-vtk-no-scalars`, `point-sets/icp-not-bijective`
  - ITK-23: 3 case(s) named it, 3 with changed values (largest difference 1.35e-14): `point-sets/jhct-isotropic`, `point-sets/jhct-alpha-1`, `point-sets/jhct-alpha-2`
  - ITK-24: 12 case(s) named it, 12 with changed values (largest difference 2.89): `point-sets/jhct`, `point-sets/jhct-isotropic`, `point-sets/jhct-alpha-1`, `point-sets/jhct-alpha-2`, `point-sets/jhct-sigma-2`, `point-sets/jhct-anisotropic-sigma-2`, `point-sets/jhct-text`, `point-sets/jhct-2d` …
  - ITK-26: 6 case(s) named it, 6 with changed values (largest difference 2.89): `point-sets/jhct`, `point-sets/jhct-anisotropic-sigma-2`, `point-sets/jhct-text`, `point-sets/jhct-2d`, `syn-point-sets/jhct`, `corrections/jhct-anisotropic`
  - ITK-29: 6 case(s) named it, 6 with changed values (largest difference 2.89): `point-sets/jhct`, `point-sets/jhct-anisotropic-sigma-2`, `point-sets/jhct-text`, `point-sets/jhct-2d`, `syn-point-sets/jhct`, `corrections/jhct-anisotropic`
  - ITK-30: 4 case(s) named it, 4 with changed values (largest difference 3.2): `syn/gc`, `bspline-syn/gc`, `displacement-field/gaussian-gc`, `exponential/gc`
  - ITK-31: 8 case(s) named it, 8 with changed values (largest difference 2.24): `syn/fixed-mask-only`, `syn/moving-mask-only`, `syn/float-mattes-mask`, `syn-point-sets/cc-and-icp`, `syn-point-sets/icp-and-cc`, `bspline-syn/fixed-mask-only`, `bspline-syn/cc-and-icp`, `real-syn-small/ds000005-sub-01/brain-extraction`
  - ITK-32: 16 case(s) named it, 16 with changed values (largest difference 3.19): `syn-point-sets/icp`, `syn-point-sets/pse`, `syn-point-sets/jhct`, `syn-point-sets/icp-float`, `syn-point-sets/icp-moving-initial`, `syn-point-sets/icp-fixed-initial`, `syn-point-sets/cc-and-icp`, `syn-point-sets/icp-and-cc` …
  - ITK-33: 3 case(s) named it, 3 with changed values (largest difference 2.24): `syn-point-sets/cc-and-icp`, `syn-point-sets/icp-and-cc`, `bspline-syn/cc-and-icp`
  - ITK-35: 4 case(s) named it, 0 with changed values (largest difference 0), 4 with other changes (files ['rNGenericAffine.mat']): `bspline/then-affine`, `bspline/initial-bspline-affine`, `corrections/bspline-then-affine`, `corrections/initial-bspline-then-rigid`
  - ITK-36: 2 case(s) named it, 2 with changed values (largest difference 2.17e-05): `time-varying-bspline/cc-and-icp`, `time-varying-bspline/meansquares-icp-cc`
  - ITK-37: 6 case(s) named it, 6 with changed values (largest difference 0.0965): `time-varying-bspline/icp`, `time-varying-bspline/pse`, `time-varying-bspline/jhct`, `time-varying-bspline/igdm`, `time-varying-bspline/cc-and-icp`, `time-varying-bspline/meansquares-icp-cc`
  - ITK-39: 1 case(s) named it, 1 with changed values (largest difference 0.213): `time-varying/fixed-initial`
  - ITK-41: 2 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (files ['rNInverseWarp.nii.gz']): `exponential/zero-iterations-prefix`, `time-varying/zero-iterations-prefix`
  - ITK-42: 7 case(s) named it, 7 with changed values (largest difference 2.68): `displacement-field/bspline-meansquares`, `displacement-field/bspline-cc`, `displacement-field/bspline-mattes`, `displacement-field/bspline-masks`, `displacement-field/bspline-2d`, `displacement-field/gaussian-then-bspline`, `corrections/bspline-field-coarse`

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
| bspline | 36 | 27 | 7 | 2 | 0 |
| bspline-exponential | 10 | 8 | 1 | 1 | 0 |
| bspline-syn | 31 | 27 | 4 | 0 | 0 |
| corrections | 9 | 8 | 1 | 0 | 0 |
| dimensions-2d | 24 | 22 | 2 | 0 | 0 |
| dimensions-4d | 10 | 10 | 0 | 0 | 0 |
| displacement-field | 38 | 29 | 9 | 0 | 0 |
| errors | 35 | 2 | 28 | 5 | 0 |
| exponential | 30 | 22 | 6 | 2 | 0 |
| initial | 11 | 9 | 1 | 1 | 0 |
| metrics | 22 | 22 | 0 | 0 | 0 |
| outputs | 7 | 7 | 0 | 0 | 0 |
| point-sets | 57 | 41 | 13 | 3 | 0 |
| real | 4 | 4 | 0 | 0 | 0 |
| real-bspline-exponential-small | 1 | 1 | 0 | 0 | 0 |
| real-bsplinesyn-small | 1 | 1 | 0 | 0 | 0 |
| real-exponential-small | 1 | 1 | 0 | 0 | 0 |
| real-linear | 1 | 1 | 0 | 0 | 0 |
| real-syn-small | 2 | 2 | 0 | 0 | 0 |
| real-time-varying-bspline-small | 1 | 1 | 0 | 0 | 0 |
| real-time-varying-small | 1 | 1 | 0 | 0 | 0 |
| sampling | 11 | 10 | 0 | 1 | 0 |
| smoke | 3 | 3 | 0 | 0 | 0 |
| syn | 56 | 46 | 6 | 4 | 0 |
| syn-point-sets | 16 | 9 | 7 | 0 | 0 |
| time-varying | 24 | 18 | 5 | 1 | 0 |
| time-varying-bspline | 25 | 21 | 3 | 1 | 0 |
| transforms | 38 | 38 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `sampling/no-seed` | without a seed ANTs seeds its sampling from the clock and its result changes from run to run; larmorx uses a fixed seed |
| `initial/names-restored` | ANTs reads past the end of its list of file names and crashes (ANTS-51); larmorx writes the same files up to there and stops with an error |
| `errors/version` | larmorx prints its own version and the ANTs version it ports |
| `errors/version-0` | ANTs exits 1 without a word (the reason is printed only with --verbose, which this command line does not set); larmorx prints the reason (ANTS-45) |
| `errors/no-dimension-quiet` | ANTs exits 1 without a word (the reason is printed only with --verbose, which this command line does not set); larmorx prints the reason (ANTS-45) |
| `errors/unbalanced-bracket` | ANTs exits 1 without a word (the reason is printed only with --verbose, which this command line does not set); larmorx prints the reason (ANTS-45) |
| `errors/minc` | the oracle has no MINC transform IO (it prints ITK's error and exits 0); larmorx refuses MINC transform files with a message |
| `point-sets/icp-sampled` | ANTs draws the points of a sampling percentage below 1 with a generator seeded from the clock (ANTS-64): two runs differ; larmorx seeds it from the registration's seed |
| `point-sets/igdm-neighbourhood` | ANTs' line search computes IGDM's centre index from a negative number and reads memory before the intensity arrays (ANTS-66, undefined); larmorx takes the two intensities as equal |
| `point-sets/igdm-2d` | ANTs reads memory before IGDM's intensity arrays for neighbourhoods of more than one voxel (ANTS-66) |
| `syn/save-state-then-translation` | a linear stage after a displacement field: its learning rate comes from voxels ANTs draws with a generator seeded from the clock (ANTS-72); the saved state, the last SyN stage's, is identical to ANTs' (checked by hand, 2026-10-19) |
| `syn/restore-state-then-translation` | a linear stage after a displacement field: its learning rate comes from voxels ANTs draws with a generator seeded from the clock (ANTS-72); the saved state, the last SyN stage's, is identical to ANTs' (checked by hand, 2026-10-19) |
| `syn/then-linear` | a linear stage after a displacement field estimates its parameter scales from voxels drawn with a generator ANTs seeds from the clock (ANTS-72): two runs of ANTs differ; larmorx seeds it from the registration's seed |
| `syn/initial-field-linear` | a linear stage after a displacement field estimates its parameter scales from voxels drawn with a generator ANTs seeds from the clock (ANTS-72) |
| `bspline/mesh-0` | ANTs computes a grid of NaN and infinite geometry, prints vnl's SVD warnings and fails with an exception on garbage indices (ANTS-77); larmorx stops with an error after the same messages |
| `bspline/mesh-short` | ANTs reads the missing mesh size past the end of its vector (here a 0) and fails as with a mesh size of 0 (ANTS-77); larmorx stops with an error |
| `exponential/then-affine` | the affine stage after a displacement field samples its scales' 1000 voxels from the clock (ANTS-72): two ANTs runs differ |
| `exponential/regular-sampling` | the stage never sets its metric's sampling seed (ANTS-82): ANTs samples from the clock, two runs differ; larmorx samples with the registration's seed |
| `bspline-exponential/random-sampling` | the stage never sets its metric's sampling seed (ANTS-82): ANTs samples from the clock, two runs differ; larmorx samples with the registration's seed |
| `time-varying/regular-sampling` | the stage never sets its metric's sampling seed (ANTS-82): ANTs samples from the clock, two runs differ; larmorx samples with the registration's seed |
| `time-varying-bspline/mesh-0` | ANTs computes control-point spacings of infinity, prints vnl's SVD warnings and fails with an exception on a parametric domain of no span (ANTS-77); larmorx stops with an error after the same messages |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `initial/save-state` | --save-state after linear stages: ANTs has no state and exits 1 (ANTS-50) | 1 |
| `dimensions-2d/file-unregistered` | -r an ANTSCenteredAffine2DTransform file: ITK's factory does not know the class | "VersorTransform_float_3_3" |
| `dimensions-2d/file-3d` | -r a 3D transform in a 2D registration | 1 |
| `errors/no-arguments` | no arguments: the short menu | 1 |
| `errors/bad-number` | -c [1x,1e-6,5]: a number that does not convert | 1 |
| `errors/levels-mismatch` | two shrink factors for one level | 1 |
| `errors/missing-image` | a metric image that does not exist: ANTs uses the null image and crashes (ANTS-02) | file /tmp/lx-reg-5aiwzoxb/none.nii.gz does not exist . |
| `errors/unknown-transform` | an unknown transform name | Unknown registration method "foo" |
| `errors/save-state-without-composite` | --save-state without -a 1 | because the the output transform will contain the this processes initializer |
| `errors/per-stage-with-collapse` | -i 1 -a 1 with collapsing | ERROR: initialize-transforms-per-stage & collapse-output-transforms options are mutually exclusive. |
| `errors/restore-with-r` | --restore-state with -r | restore-state option is mutually exclusive with initial-moving-transform & initial-fixed-transform options. |
| `errors/no-dimension` | no -d, verbose: the dimension from the first fixed image is unknown | Image dimensionality not specified. See command line option --dimensionality |
| `errors/bad-dimension` | -d 5 | bad image dimension 5 |
| `errors/invalid-flag` | an option ANTs does not know | ERROR: Invalid command line flags found! Aborting execution. |
| `errors/misplaced-bracket` | -t Rig]id[0.1] | ERROR: the metric option ('-m') must be specified. See help menu. |
| `errors/no-transform` | no -t | ERROR: the transform option ('-t') must be specified. See help menu. |
| `errors/no-metric` | no -m | ERROR: the metric option ('-m') must be specified. See help menu. |
| `errors/no-convergence` | no -c | ERROR: the convergence option ('-c') must be specified. See help menu. |
| `errors/no-shrink-factors` | no -f | ERROR: the shrink factors option ('-f') must be specified. See help menu. |
| `errors/no-smoothing-sigmas` | no -s | ERROR: the smoothing sigmas option ('-s') must be specified. See help menu. |
| `errors/no-output` | no -o | ERROR: the output option ('-o') must be specified. See help menu. |
| `errors/per-stage-without-composite` | -i 1 without -a 1 | because the initializizing transform is collapsed into each stage for optimization |
| `errors/convergence-count` | two -c for one stage | ERROR: smoothing sigmas, shrink factors, and convergence options must be set once for each transform. |
| `errors/window-1` | -c [10,1e-6,1]: a convergence window of 1 | Convergence Window Size must be greater than or equal to 2 |
| `errors/bad-iterations` | -c [1ox,1e-6,5] | ERROR: Unable to convert '1o' to type 'j' as unsigned int |
| `errors/unknown-metric` | an unknown metric | 1 |
| `errors/bad-sampling` | an unknown sampling strategy | ERROR: invalid sampling strategy specified: sparse |
| `errors/sigmas-mismatch` | three sigmas for two levels | 1 |
| `errors/stage-without-metric` | a second -t without its -m | The number of transforms is 2 and the last stage ID as determined by the metrics is 0. |
| `errors/sampling-above-1` | a sampling percentage of 1.5 | Description: ITK ERROR: ImageRegistrationMethodv4(0x5f5b5ffb7bc0): sampling percentage outside expected (0,1] range |
| `errors/missing-initial` | -r a transform file that does not exist | Transform file does not exist: /tmp/lx-reg-u0oktzj3/none.mat |
| `point-sets/igdm-few-parameters` | IGDM with three parameters | The expected number of parameters aren't specified. Please see help menu. |
| `point-sets/igdm-radius-dimension` | IGDM with a 2D radius in 3D | The neighborhood size must equal the dimension. |
| `point-sets/icp-percentage-above-1` | A sampling percentage of 1.5 | Description: ITK ERROR: ImageRegistrationMethodv4(0x62c6d0552e60): sampling percentage outside expected (0,1] range |
| `point-sets/icp-no-mask` | Point-set metrics without a fixed mask: no virtual domain | larmorx: antsRegistration: a stage of point-set metrics needs a fixed mask (-x) to define its virtual domain: ANTs reads the spacing of a null image; ANTs cras… |
| `point-sets/icp-missing` | A point set that does not exist | larmorx: antsRegistration: a labelled point set could not be read: ANTs goes on with a null point set and crashes |
| `point-sets/icp-short-name` | A point set named with fewer than 3 characters | larmorx: antsRegistration: a labelled point set could not be read: ANTs goes on with a null point set and crashes |
| `point-sets/icp-unreadable` | A point set no reader takes (ITK's image reader throws) | larmorx: upstream defect ANTS-62 (antsRegistration) reproduced: '/tmp/lx-reg-nqsl8td1/moving.txt' ends with a newline: an extra point, a copy of the last one, … |
| `point-sets/icp-vtk-no-points` | A VTK file without POINTS: libstdc++'s out_of_range | larmorx: antsRegistration: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `point-sets/igdm-missing-mask` | An IGDM mask that does not exist | larmorx: upstream defect ANTS-68 (antsRegistration) reproduced: IGDM's derivative lacks the 1/sigma^2 factors of its two terms, as in ANTs [LARMORX_UPSTREAM_DE… |
| `point-sets/icp-warped` | Warped images with a point-set metric first | larmorx: antsRegistration: warped images with a point-set metric first: ANTs resamples null images and crashes, larmorx stops |
| `point-sets/icp-print-similarity` | --print-similarity-measure-interval with a point-set metric first | larmorx: antsRegistration: --print-similarity-measure-interval with a point-set metric first: ANTs' observer computes the full-scale similarity of null images;… |
| `point-sets/icp-interval-volumes` | --write-interval-volumes with a point-set metric first | larmorx: antsRegistration: --write-interval-volumes with a point-set metric first: ANTs' observer resamples a null image; ANTs crashes here, larmorx stops |
| `point-sets/mattes-regular-then-icp` | Regular sampling with a point-set metric in the stage | larmorx: antsRegistration: a stage that samples its metrics (Regular or Random) with a point-set metric: ANTs gives the sample points to a null image metric an… |
| `syn/print-similarity-crash` | --print-similarity-measure-interval without -q: a null transform (ANTS-71) | larmorx: upstream defect ANTS-71 (antsRegistration) reproduced: stage 0: ANTs crashes computing the full-scale similarity; larmorx stops there [LARMORX_UPSTREA… |
| `syn/print-similarity-multi-crash` | --print-similarity-measure-interval with two metrics: a null metric (ANTS-71) | larmorx: upstream defect ANTS-71 (antsRegistration) reproduced: stage 0: ANTs crashes computing the full-scale similarity; larmorx stops there [LARMORX_UPSTREA… |
| `syn/initial-field-inverse` | -r [field,1]: a field without an inverse | 1 |
| `syn/initial-composite-inverse` | -r [composite,1]: its field has no inverse | 1 |
| `syn/igdm` | IGDM first in a SyN stage: not implemented in ANTs | 1 |
| `syn/bad-variance` | SyN[0.2,3x]: a variance that does not convert | ERROR: Unable to convert '3x' to type 'f' as float |
| `syn-point-sets/icp-not-bijective` | a fixed label the moving set lacks: ITK throws at the first such point (ITK-22) | LabeledPointSetToPointSetMetricv4 (0x563d094079d0): The label sets are not bijective. |
| `syn-point-sets/icp-moving-extra-label` | a moving label the fixed set lacks: ITK throws in the moving-to-middle update (ITK-22) | LabeledPointSetToPointSetMetricv4 (0x55b5783403b0): The label sets are not bijective. |
| `syn-point-sets/cc-and-igdm` | CC and IGDM: SyN holds labelled point sets only, IGDM gets no input (ANTS-61) | 1 |
| `syn-point-sets/cc-igdm-icp` | CC, IGDM and ICP: IGDM without point sets before a metric with them (ANTS-61) | larmorx: upstream defect ANTS-68 (antsRegistration) reproduced: IGDM's derivative lacks the 1/sigma^2 factors of its two terms, as in ANTs [LARMORX_UPSTREAM_DE… |
| `syn-point-sets/cc-regular-and-icp` | CC sampled (Regular) and ICP: ANTs gives ICP's sample points to a null image metric | larmorx: antsRegistration: a stage that samples its metrics (Regular or Random) with a point-set metric: ANTs gives the sample points to a null image metric an… |
| `syn-point-sets/icp-field-initial` | ICP after -r a displacement field without an inverse: ITK maps the moving points with a null transform | larmorx: antsRegistration: the moving transform of a point-set metric in a SyN stage cannot be inverted (a linear transform with a singular matrix, or a displa… |
| `syn-point-sets/icp-no-mask` | ICP alone without a fixed mask: no virtual domain | larmorx: antsRegistration: a stage of point-set metrics needs a fixed mask (-x) to define its virtual domain: ANTs reads the spacing of a null image; ANTs cras… |
| `bspline-syn/save-state` | --save-state: BSplineSyN does not set the registration state (ANTS-50) | because the the output transform will contain the this processes initializer |
| `bspline-syn/mesh-dimension` | a 2D mesh in 3D: ERROR, the mesh sizes do not match the dimension | 1 |
| `bspline-syn/order-0` | spline order 0: the B-spline filter throws | 1 |
| `bspline-syn/bad-mesh` | a mesh that does not convert | ERROR: Unable to convert 'a' to type 'f' as float |
| `displacement-field/gaussian-print-similarity` | --print-similarity-measure-interval: the observer's copy of the field fails (ANTS-75) | 1 |
| `displacement-field/gaussian-interval-volumes` | --write-interval-volumes: the observer's copy of the field fails (ANTS-75) | 1 |
| `displacement-field/gaussian-save-state` | --save-state: no state (ANTS-50) | because the the output transform will contain the this processes initializer |
| `displacement-field/gaussian-igdm` | IGDM first: not implemented in ANTs | 1 |
| `displacement-field/gaussian-icp-first` | ICP first: ANTs allocates the field on a null image | larmorx: antsRegistration: a displacement field stage whose first metric is a point-set metric: ANTs allocates the field on the null image of its first metric;… |
| `displacement-field/gaussian-variance-missing` | GaussianDisplacementField[0.25,3]: the total variance is required | ERROR: Unable to convert '' to type 'f' as float |
| `displacement-field/bspline-coarse-total` | a total mesh of 1: the stationary boundary dominates the fit, the scales fall below epsilon and the stage fails | 1 |
| `displacement-field/bspline-mesh-dimension` | a 2D mesh in 3D: ERROR | 1 |
| `displacement-field/bspline-order-0` | order 0: the B-spline filter throws | 1 |
| `bspline/demons` | Demons: the metric needs a displacement field (ITK's exception) | 1 |
| `bspline/then-bspline` | two BSpline stages: the second fails (no Jacobian with respect to position) | 1 |
| `bspline/print-similarity` | --print-similarity-measure-interval: the observer's copy of the transform fails (ANTS-75) | 1 |
| `bspline/restrict` | -g 1x1x0: weights for the dimension, not the parameters (ITK's exception) | 1 |
| `bspline/save-state` | --save-state: no state (ANTS-50) | because the the output transform will contain the this processes initializer |
| `bspline/initial-bspline-inverse` | -r [bspline,1]: a B-spline transform has no inverse | 1 |
| `bspline/icp-first` | ICP first: ANTs reads the null image of the first metric | larmorx: antsRegistration: a BSpline stage whose first metric is a point-set metric: ANTs reads the spacing and size of the null image of its first metric; ANT… |
| `exponential/print-similarity` | --print-similarity-measure-interval: the observer's copy of the field fails (ANTS-75) | 1 |
| `exponential/interval-volumes` | --write-interval-volumes: the observer's copy of the field fails (ANTS-75) | 1 |
| `exponential/save-state` | --save-state: no state (ANTS-50) | because the the output transform will contain the this processes initializer |
| `exponential/zero-iterations` | no iteration and one level: the transform never integrates, its inverse has no field (ITK-41) | Description: ITK ERROR: ConstantVelocityFieldTransform(0x63a4590e1680): No displacement field is specified. |
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
| `smoke/rigid` | pass | 9/9 | Rigid, Mattes, regular sampling, 2 levels, warped images |
| `smoke/affine-float` | pass | 9/9 | Affine, MeanSquares, random sampling, --float 1 |
| `smoke/composite` | pass | 7/7 | -a 1: the composite and its inverse (HDF5) |
| `transforms/Translation-3d` | pass | 15/15 | Translation in 3D |
| `transforms/Translation-3d-z0` | pass | 7/7 | Translation in 3D, -z 0, -r [fixed,moving,1] |
| `transforms/Translation-3d-float` | pass | 6/6 | Translation in 3D, --float 1 |
| `transforms/Translation-2d` | pass | 6/6 | Translation in 2D |
| `transforms/Translation-2d-z0` | pass | 7/7 | Translation in 2D, -z 0, -r [fixed,moving,1] |
| `transforms/Translation-2d-float` | pass | 6/6 | Translation in 2D, --float 1 |
| `transforms/Translation-4d` | pass | 6/6 | Translation in 4D (an affine transform in 4D) |
| `transforms/Rigid-3d` | pass | 8/8 | Rigid in 3D |
| `transforms/Rigid-3d-z0` | pass | 7/7 | Rigid in 3D, -z 0, -r [fixed,moving,1] |
| `transforms/Rigid-3d-float` | pass | 6/6 | Rigid in 3D, --float 1 |
| `transforms/Rigid-2d` | pass | 6/6 | Rigid in 2D |
| `transforms/Rigid-2d-z0` | pass | 7/7 | Rigid in 2D, -z 0, -r [fixed,moving,1] |
| `transforms/Rigid-2d-float` | pass | 6/6 | Rigid in 2D, --float 1 |
| `transforms/Rigid-4d` | pass | 11/11 | Rigid in 4D (an affine transform in 4D) |
| `transforms/Similarity-3d` | pass | 8/8 | Similarity in 3D |
| `transforms/Similarity-3d-z0` | pass | 7/7 | Similarity in 3D, -z 0, -r [fixed,moving,1] |
| `transforms/Similarity-3d-float` | pass | 6/6 | Similarity in 3D, --float 1 |
| `transforms/Similarity-2d` | pass | 6/6 | Similarity in 2D |
| `transforms/Similarity-2d-z0` | pass | 7/7 | Similarity in 2D, -z 0, -r [fixed,moving,1] |
| `transforms/Similarity-2d-float` | pass | 6/6 | Similarity in 2D, --float 1 |
| `transforms/Similarity-4d` | pass | 11/11 | Similarity in 4D (an affine transform in 4D) |
| `transforms/Affine-3d` | pass | 8/8 | Affine in 3D |
| `transforms/Affine-3d-z0` | pass | 13/13 | Affine in 3D, -z 0, -r [fixed,moving,1] |
| `transforms/Affine-3d-float` | pass | 6/6 | Affine in 3D, --float 1 |
| `transforms/Affine-2d` | pass | 6/6 | Affine in 2D |
| `transforms/Affine-2d-z0` | pass | 7/7 | Affine in 2D, -z 0, -r [fixed,moving,1] |
| `transforms/Affine-2d-float` | pass | 6/6 | Affine in 2D, --float 1 |
| `transforms/Affine-4d` | pass | 11/11 | Affine in 4D (an affine transform in 4D) |
| `transforms/CompositeAffine-3d` | pass | 15/15 | CompositeAffine in 3D |
| `transforms/CompositeAffine-3d-z0` | pass | 7/7 | CompositeAffine in 3D, -z 0, -r [fixed,moving,1] |
| `transforms/CompositeAffine-3d-float` | pass | 6/6 | CompositeAffine in 3D, --float 1 |
| `transforms/CompositeAffine-2d` | pass | 6/6 | CompositeAffine in 2D |
| `transforms/CompositeAffine-2d-z0` | pass | 7/7 | CompositeAffine in 2D, -z 0, -r [fixed,moving,1] |
| `transforms/CompositeAffine-2d-float` | pass | 6/6 | CompositeAffine in 2D, --float 1 |
| `transforms/CompositeAffine-4d` | pass | 11/11 | CompositeAffine in 4D (an affine transform in 4D) |
| `transforms/two-stages` | pass | 8/8 | Rigid then Affine |
| `transforms/two-stages-z0` | pass | 13/13 | Rigid then Affine, not collapsed |
| `transforms/oblique` | pass | 8/8 | Affine between oblique grids |
| `metrics/mattes-mi-alias` | pass | 7/7 | Rigid with MI (mattes-mi-alias) |
| `metrics/mi2` | pass | 12/12 | Rigid with MI2 (mi2) |
| `metrics/mi2-dense` | pass | 7/7 | Rigid with MI2 (mi2-dense) |
| `metrics/meansquares` | pass | 7/7 | Rigid with MeanSquares (meansquares) |
| `metrics/gc` | pass | 7/7 | Rigid with GC (gc) |
| `metrics/gc-dense` | pass | 7/7 | Rigid with GC (gc-dense) |
| `metrics/cc-sampled` | pass | 7/7 | Rigid with CC (cc-sampled) |
| `metrics/cc-dense` | pass | 7/7 | Rigid with CC (cc-dense) |
| `metrics/mattes-gradient-filter` | pass | 7/7 | Rigid with Mattes (mattes-gradient-filter) |
| `metrics/cc-gradient-filter` | pass | 7/7 | Rigid with CC (cc-gradient-filter) |
| `metrics/mattes-gradient-filter-oblique` | pass | 6/6 | Rigid with Mattes and the gradient filter, oblique grids |
| `metrics/cc-gradient-filter-oblique` | pass | 6/6 | Rigid with CC and the gradient filter, oblique grids |
| `metrics/mi2-2d` | pass | 6/6 | MI2 in 2D |
| `metrics/cc-2d` | pass | 6/6 | CC in 2D |
| `metrics/gc-2d` | pass | 6/6 | GC in 2D |
| `metrics/mi2-4d-gradient-filter` | pass | 6/6 | MI2 in 4D |
| `metrics/mi2-float` | pass | 6/6 | MI2, --float 1 |
| `metrics/cc-float` | pass | 6/6 | CC, --float 1 |
| `metrics/gc-float` | pass | 6/6 | GC, --float 1 |
| `metrics/multi` | pass | 6/6 | two metrics in one stage (weights 1 and 0.5) |
| `metrics/demons` | pass | 6/6 | Demons in a linear stage: ITK throws and ANTs drops the stage (ANTS-49) |
| `metrics/no-overlap` | pass | 6/6 | no sample maps inside the moving image (Mattes throws, ANTS-49) |
| `sampling/random` | pass | 6/6 | Random 0.4 |
| `sampling/regular-1` | pass | 6/6 | Regular 1 (every voxel, perturbed) |
| `sampling/sigmas-mm` | pass | 6/6 | smoothing sigmas in mm |
| `sampling/shrink-4` | pass | 11/11 | three levels, shrink 4x2x1 |
| `sampling/winsorize-histogram` | pass | 6/6 | -w [0.005,0.995] -u 1 |
| `sampling/masks` | pass | 6/6 | -x [mask,mask] |
| `sampling/fixed-mask-only` | pass | 6/6 | -x mask (fixed only), dense |
| `sampling/moving-mask-null` | pass | 11/11 | -x [NULL,mask] |
| `sampling/restrict-deformation` | pass | 11/11 | -g 1x1x0x1x1x1: a rotation frozen |
| `sampling/zero-iterations` | pass | 6/6 | a first level of 0 iterations runs none |
| `sampling/no-seed` | expected-divergence | 1/1 | no seed: ANTs seeds its sampling from the clock |
| `initial/geometry` | pass | 8/8 | -r [fixed,moving,0] |
| `initial/moments` | pass | 8/8 | -r [fixed,moving,1] |
| `initial/origins` | pass | 8/8 | -r [fixed,moving,2] |
| `initial/fixed-initial` | pass | 6/6 | -q [fixed,moving,1] |
| `initial/file` | pass | 6/6 | -r a transform file |
| `initial/file-inverted` | pass | 6/6 | -r [file,1] |
| `initial/per-stage` | pass | 7/7 | --initialize-transforms-per-stage 1: each stage starts from the previous |
| `initial/per-stage-from-r` | pass | 8/8 | -i 1 with -r [fixed,moving,1] |
| `initial/restore-state` | pass | 6/6 | --restore-state from a transform file |
| `initial/save-state` | both-error | 2/2 | --save-state after linear stages: ANTs has no state and exits 1 (ANTS-50) |
| `initial/names-restored` | expected-divergence | 1/1 | -z 0 with a restored state: a file named after the wrong stage, then a read past the end of the names (ANTS-51) |
| `dimensions-2d/warped` | pass | 8/8 | the warped and inverse-warped images (linear) |
| `dimensions-2d/warped-nearest` | pass | 8/8 | -n NearestNeighbor |
| `dimensions-2d/warped-float` | pass | 8/8 | Similarity, --float 1 |
| `dimensions-2d/fixed-initial` | pass | 8/8 | -q [fixed,moving,1] |
| `dimensions-2d/file-affine` | pass | 8/8 | -r an AffineTransform file |
| `dimensions-2d/file-affine-inverted` | pass | 8/8 | -r [AffineTransform file,1] |
| `dimensions-2d/file-euler` | pass | 8/8 | -r a text Euler2DTransform file, -z 0 |
| `dimensions-2d/file-euler-inverted` | pass | 8/8 | -r [Euler2DTransform,1]: its own inverse class |
| `dimensions-2d/file-rigid2d` | pass | 7/7 | -r a Rigid2DTransform file, -z 0 -a 1 |
| `dimensions-2d/file-similarity-inverted` | pass | 8/8 | -r [Similarity2DTransform,1], --float 1 |
| `dimensions-2d/file-translation` | pass | 8/8 | -r a TranslationTransform file |
| `dimensions-2d/file-unregistered` | both-error | 2/2 | -r an ANTSCenteredAffine2DTransform file: ITK's factory does not know the class |
| `dimensions-2d/file-3d` | both-error | 2/2 | -r a 3D transform in a 2D registration |
| `dimensions-2d/composite` | pass | 9/9 | -a 1 with -r [fixed,moving,1] |
| `dimensions-2d/interval-volumes` | pass | 9/9 | --write-interval-volumes 2 |
| `dimensions-2d/interpolation-BSpline` | pass | 8/8 | -n BSpline[3] in 2D |
| `dimensions-2d/interpolation-BSpline5-float` | pass | 8/8 | -n BSpline[5] in 2D |
| `dimensions-2d/interpolation-Gaussian` | pass | 15/15 | -n Gaussian in 2D |
| `dimensions-2d/interpolation-Gaussian-sigmas` | pass | 15/15 | -n Gaussian[1.5x0.8,1.2] in 2D |
| `dimensions-2d/interpolation-LanczosWindowedSinc` | pass | 15/15 | -n LanczosWindowedSinc in 2D |
| `dimensions-2d/interpolation-BlackmanWindowedSinc-float` | pass | 8/8 | -n BlackmanWindowedSinc in 2D |
| `dimensions-2d/interpolation-WelchWindowedSinc` | pass | 15/15 | -n WelchWindowedSinc in 2D |
| `dimensions-2d/interpolation-GenericLabel` | pass | 8/8 | -n GenericLabel in 2D |
| `dimensions-2d/interpolation-MultiLabel` | pass | 8/8 | -n MultiLabel[1.2] in 2D |
| `dimensions-4d/warped` | pass | 15/15 | the warped and inverse-warped images (linear) |
| `dimensions-4d/warped-nearest-float` | pass | 8/8 | -n NearestNeighbor, --float 1 |
| `dimensions-4d/file-affine-inverted` | pass | 8/8 | -r [AffineTransform file,1] |
| `dimensions-4d/interpolation-BSpline` | pass | 8/8 | -n BSpline[2] in 4D |
| `dimensions-4d/interpolation-Gaussian-float` | pass | 15/15 | -n Gaussian[1x1x1x1,1.5] in 4D |
| `dimensions-4d/interpolation-HammingWindowedSinc` | pass | 15/15 | -n HammingWindowedSinc in 4D |
| `dimensions-4d/interpolation-GenericLabel` | pass | 8/8 | -n GenericLabel in 4D |
| `dimensions-4d/interpolation-MultiLabel-float` | pass | 15/15 | -n MultiLabel in 4D |
| `dimensions-4d/composite-z0` | pass | 10/10 | -a 1 -z 0 -r [fixed,moving,0] |
| `dimensions-4d/interval-volumes` | pass | 9/9 | --write-interval-volumes 1 |
| `outputs/interpolation-NearestNeighbor` | pass | 8/8 | warped images with -n NearestNeighbor |
| `outputs/interpolation-BSpline` | pass | 8/8 | warped images with -n BSpline[3] |
| `outputs/interpolation-LanczosWindowedSinc` | pass | 15/15 | warped images with -n LanczosWindowedSinc |
| `outputs/interpolation-MultiLabel` | pass | 8/8 | warped images with -n MultiLabel |
| `outputs/interpolation-Gaussian` | pass | 15/15 | warped images with -n Gaussian |
| `outputs/print-similarity` | pass | 6/6 | --print-similarity-measure-interval 2 (full-scale CC) |
| `outputs/write-interval-volumes` | pass | 12/12 | --write-interval-volumes 3 |
| `errors/help` | pass | 5/5 | --help: the menu, exit 0 |
| `errors/no-arguments` | both-error | 2/2 | no arguments: the short menu |
| `errors/bad-number` | both-error | 2/2 | -c [1x,1e-6,5]: a number that does not convert |
| `errors/levels-mismatch` | both-error | 2/2 | two shrink factors for one level |
| `errors/missing-image` | both-error | 0/2 | a metric image that does not exist: ANTs uses the null image and crashes (ANTS-02) |
| `errors/unknown-transform` | both-error | 2/2 | an unknown transform name |
| `errors/save-state-without-composite` | both-error | 2/2 | --save-state without -a 1 |
| `errors/per-stage-with-collapse` | both-error | 2/2 | -i 1 -a 1 with collapsing |
| `errors/restore-with-r` | both-error | 2/2 | --restore-state with -r |
| `errors/short-help` | pass | 5/5 | -h: the short menu, exit 0 |
| `errors/version` | expected-divergence | 1/1 | --version |
| `errors/version-0` | expected-divergence | 1/1 | --version 0: no version, then no dimension |
| `errors/no-dimension` | both-error | 2/2 | no -d, verbose: the dimension from the first fixed image is unknown |
| `errors/no-dimension-quiet` | expected-divergence | 1/1 | no -d, quiet |
| `errors/bad-dimension` | both-error | 2/2 | -d 5 |
| `errors/invalid-flag` | both-error | 2/2 | an option ANTs does not know |
| `errors/unbalanced-bracket` | expected-divergence | 1/1 | -t Rigid[0.1 (no closing bracket): the rest of the line joins the transform |
| `errors/misplaced-bracket` | both-error | 2/2 | -t Rig]id[0.1] |
| `errors/no-transform` | both-error | 2/2 | no -t |
| `errors/no-metric` | both-error | 2/2 | no -m |
| `errors/no-convergence` | both-error | 2/2 | no -c |
| `errors/no-shrink-factors` | both-error | 2/2 | no -f |
| `errors/no-smoothing-sigmas` | both-error | 2/2 | no -s |
| `errors/no-output` | both-error | 2/2 | no -o |
| `errors/per-stage-without-composite` | both-error | 2/2 | -i 1 without -a 1 |
| `errors/convergence-count` | both-error | 2/2 | two -c for one stage |
| `errors/window-1` | both-error | 2/2 | -c [10,1e-6,1]: a convergence window of 1 |
| `errors/bad-iterations` | both-error | 2/2 | -c [1ox,1e-6,5] |
| `errors/unknown-metric` | both-error | 2/2 | an unknown metric |
| `errors/bad-sampling` | both-error | 2/2 | an unknown sampling strategy |
| `errors/sigmas-mismatch` | both-error | 2/2 | three sigmas for two levels |
| `errors/stage-without-metric` | both-error | 2/2 | a second -t without its -m |
| `errors/sampling-above-1` | both-error | 2/2 | a sampling percentage of 1.5 |
| `errors/missing-initial` | both-error | 2/2 | -r a transform file that does not exist |
| `errors/minc` | expected-divergence | 1/1 | --minc 1: MINC transform files |
| `point-sets/icp` | pass | 7/7 | ICP between two label images, the fixed mask as the virtual domain |
| `point-sets/icp-affine-two-levels` | pass | 6/6 | ICP, Affine, two levels (shrink 2x1, smoothing ignored) |
| `point-sets/icp-float` | pass | 6/6 | ICP with --float 1 |
| `point-sets/icp-text` | pass | 6/6 | ICP between text point sets (a final newline: ANTs reads one more record, ANTS-62) |
| `point-sets/icp-text-no-final-newline` | pass | 6/6 | A text point set without a final newline |
| `point-sets/icp-vtk` | pass | 6/6 | ICP between ASCII VTK point sets |
| `point-sets/icp-vtk-binary` | pass | 6/6 | A binary VTK point set: its labels read past one value (ANTS-63) |
| `point-sets/icp-vtk-no-scalars` | pass | 6/6 | A VTK point set without labels: label 0 for every point |
| `point-sets/icp-boundary` | pass | 6/6 | ICP on the labels' contours (boundaryPointsOnly) |
| `point-sets/icp-not-bijective` | pass | 6/6 | A label only the fixed set has: ITK throws, the stage is dropped (ITK-22) |
| `point-sets/icp-empty` | pass | 6/6 | An empty fixed point set: the scales estimator throws |
| `point-sets/icp-sampled` | expected-divergence | 1/1 | ICP on half of each point set (a sampling percentage of 0.5) |
| `point-sets/icp-2d` | pass | 6/6 | ICP in 2D |
| `point-sets/icp-4d` | pass | 6/6 | ICP in 4D, Affine |
| `point-sets/icp-fixed-initial` | pass | 6/6 | ICP with an initial fixed transform (-q) |
| `point-sets/icp-moving-initial` | pass | 6/6 | ICP with an initial moving transform (-r) |
| `point-sets/icp-images-initial` | pass | 6/6 | ICP with -r [fixed,moving,1] from intensity images |
| `point-sets/icp-same-sets` | pass | 6/6 | ICP of a point set with itself |
| `point-sets/pse` | pass | 7/7 | PSE (sigma 1, 50 neighbours) |
| `point-sets/pse-boundary` | pass | 6/6 | PSE on the contours, 20 neighbours |
| `point-sets/pse-sigma-zero` | pass | 6/6 | PSE with a point-set sigma of 0: ITK throws |
| `point-sets/pse-text-2d` | pass | 6/6 | PSE between 2D text point sets |
| `point-sets/jhct` | pass | 7/7 | JHCT with anisotropic covariances (sigma 1, alpha 1.1) |
| `point-sets/jhct-isotropic` | pass | 6/6 | JHCT with isotropic covariances (sigma 1: ITK's univariate prefactor, ITK-23) |
| `point-sets/jhct-alpha-1` | pass | 6/6 | JHCT with alpha 1 (the logarithm) |
| `point-sets/jhct-alpha-2` | pass | 6/6 | JHCT with alpha 2 |
| `point-sets/jhct-sigma-2` | pass | 6/6 | JHCT with a point-set sigma of 2 |
| `point-sets/jhct-anisotropic-sigma-2` | pass | 6/6 | JHCT, anisotropic, sigma 2, alpha 1.5 |
| `point-sets/jhct-text` | pass | 6/6 | JHCT between text point sets (fewer points than neighbours: ITK's warnings) |
| `point-sets/jhct-2d` | pass | 6/6 | JHCT in 2D |
| `point-sets/igdm` | pass | 7/7 | IGDM between masked intensity images (sigmas estimated) |
| `point-sets/igdm-sigmas` | pass | 6/6 | IGDM with given sigmas |
| `point-sets/igdm-fixed-initial` | pass | 6/6 | IGDM with an initial fixed transform |
| `point-sets/igdm-neighbourhood` | expected-divergence | 1/1 | IGDM with a neighbourhood of radius 1 |
| `point-sets/igdm-2d` | expected-divergence | 1/1 | IGDM in 2D, radius 1 |
| `point-sets/igdm-2d-radius-0` | pass | 6/6 | IGDM in 2D |
| `point-sets/mattes-then-icp` | pass | 6/6 | Mattes and ICP in one stage (the image metric's virtual domain) |
| `point-sets/icp-then-mattes` | pass | 6/6 | ICP and Mattes in one stage, weights 0.5 and 2 |
| `point-sets/icp-then-mattes-regular` | pass | 6/6 | ICP first: the stage takes ICP's sampling (none) |
| `point-sets/igdm-then-mattes` | pass | 6/6 | IGDM and Mattes in one stage |
| `point-sets/icp-then-igdm` | pass | 6/6 | ICP and IGDM in one stage: IGDM gets no input, ITK throws (ANTS-61) |
| `point-sets/mattes-then-igdm` | pass | 6/6 | Mattes and IGDM in one stage: IGDM gets no input (ANTS-61) |
| `point-sets/icp-then-pse-stages` | pass | 6/6 | Two stages: ICP (Rigid), then PSE (Affine) |
| `point-sets/mattes-and-icp-warped` | pass | 8/8 | Mattes first, then ICP: the warped images of the image metric |
| `point-sets/igdm-few-parameters` | both-error | 2/2 | IGDM with three parameters |
| `point-sets/igdm-radius-dimension` | both-error | 2/2 | IGDM with a 2D radius in 3D |
| `point-sets/icp-percentage-above-1` | both-error | 2/2 | A sampling percentage of 1.5 |
| `point-sets/icp-no-mask` | both-error | 4/4 | Point-set metrics without a fixed mask: no virtual domain |
| `point-sets/icp-missing` | both-error | 4/4 | A point set that does not exist |
| `point-sets/icp-short-name` | both-error | 4/4 | A point set named with fewer than 3 characters |
| `point-sets/icp-unreadable` | both-error | 4/4 | A point set no reader takes (ITK's image reader throws) |
| `point-sets/icp-vtk-no-points` | both-error | 4/4 | A VTK file without POINTS: libstdc++'s out_of_range |
| `point-sets/igdm-missing-mask` | both-error | 4/4 | An IGDM mask that does not exist |
| `point-sets/icp-warped` | both-error | 4/4 | Warped images with a point-set metric first |
| `point-sets/icp-print-similarity` | both-error | 4/4 | --print-similarity-measure-interval with a point-set metric first |
| `point-sets/icp-interval-volumes` | both-error | 4/4 | --write-interval-volumes with a point-set metric first |
| `point-sets/mattes-regular-then-icp` | both-error | 4/4 | Regular sampling with a point-set metric in the stage |
| `syn/cc` | pass | 10/10 | SyN with CC (radius 2), 2 levels, warped images |
| `syn/meansquares-total` | pass | 8/8 | SyN with MeanSquares, a total field variance of 0.5 |
| `syn/mattes` | pass | 14/14 | SyN with Mattes (24 bins), every voxel |
| `syn/mattes-regular` | pass | 13/13 | SyN with Mattes, regular sampling 0.5 |
| `syn/mattes-random` | pass | 7/7 | SyN with Mattes, random sampling 0.6 |
| `syn/mi2` | pass | 13/13 | SyN with the joint-histogram MI (MI2, ITK-18 reproduced) |
| `syn/demons` | pass | 7/7 | SyN with Demons (the fixed image's gradient) |
| `syn/gc` | pass | 7/7 | SyN with GC: ITK adds every point's derivative to the first voxel (ITK-30) |
| `syn/multi` | pass | 7/7 | two metrics (CC 0.7, Mattes 0.3): the weights normalised at each evaluation |
| `syn/masks` | pass | 7/7 | -x [mask,mask]: both masks resampled through the fields |
| `syn/fixed-mask-only` | pass | 7/7 | a fixed mask on another grid, no moving mask (ITK-31) |
| `syn/moving-mask-only` | pass | 7/7 | -x [NULL,mask] with the mask on another grid (ITK-31) |
| `syn/rigid-affine-syn` | pass | 10/10 | Rigid, Affine then SyN, collapsed: 0GenericAffine.mat, 1Warp, 1InverseWarp |
| `syn/no-collapse` | pass | 10/10 | Affine then SyN, -z 0 |
| `syn/composite` | pass | 7/7 | Affine then SyN, -a 1: the composite and its inverse hold the fields |
| `syn/composite-no-collapse` | pass | 7/7 | Affine then SyN, -a 1 -z 0 |
| `syn/two-stages` | pass | 9/9 | two SyN stages, collapsed into one field |
| `syn/two-stages-no-collapse` | pass | 9/9 | two SyN stages, -z 0 |
| `syn/save-state` | pass | 8/8 | --save-state after SyN: the fixed-to-middle and moving-to-middle fields and inverses |
| `syn/save-state-then-translation` | expected-divergence | 1/1 | --save-state after SyN then Translation: ANTs saves the SyN state without the translation (ANTS-74) |
| `syn/restore-state-then-translation` | expected-divergence | 1/1 | --restore-state of a SyN state, then Translation, --save-state (ANTS-74) |
| `syn/restore-state` | pass | 7/7 | --restore-state of a SyN state: the composite ends with SyN's transform |
| `syn/restore-state-per-stage` | pass | 7/7 | --restore-state with -i 1: SyN starts from the restored fields |
| `syn/per-stage` | pass | 7/7 | -i 1: the second SyN stage starts from the first one's fields |
| `syn/interval-volumes` | pass | 13/13 | --write-interval-volumes 2 |
| `syn/print-similarity` | pass | 8/8 | --print-similarity-measure-interval 2 with initial transforms on both sides |
| `syn/print-similarity-crash` | both-error | 4/4 | --print-similarity-measure-interval without -q: a null transform (ANTS-71) |
| `syn/print-similarity-multi-crash` | both-error | 4/4 | --print-similarity-measure-interval with two metrics: a null metric (ANTS-71) |
| `syn/restrict-deformation` | pass | 7/7 | -g 1x0.5x0: the update weighted per axis |
| `syn/2d` | pass | 9/9 | SyN in 2D, three levels |
| `syn/2d-float` | pass | 7/7 | SyN in 2D, --float 1 |
| `syn/4d` | pass | 8/8 | SyN in 4D |
| `syn/float` | pass | 9/9 | SyN, --float 1 |
| `syn/float-mattes-mask` | pass | 13/13 | SyN with Mattes and a fixed mask, --float 1 |
| `syn/gradient-filter` | pass | 7/7 | CC and Demons with the gradient filter |
| `syn/mm-winsorize-histogram` | pass | 7/7 | sigmas in mm, -w and -u, an update variance of 0 |
| `syn/sdcflows-syn-sdc` | pass | 13/13 | sdcflows' fieldmap-less SyN (sd_syn.json): two SyN stages of two Mattes metrics (random sampling, then dense), -g along… |
| `syn/fixed-initial` | pass | 9/9 | -q [fixed,moving,1]: the fixed images resampled through it |
| `syn/convergence` | pass | 7/7 | a level stopped by the convergence test, a level of 0 iterations |
| `syn/oblique` | pass | 9/9 | SyN between oblique grids |
| `syn/default-variances` | pass | 7/7 | SyN[0.2]: the variances default to 3 and 0 |
| `syn/quiet` | pass | 10/10 | Affine then SyN without -v |
| `syn/then-linear` | expected-divergence | 1/1 | SyN then Affine: the linear stage's scales from random voxels (ANTS-72) |
| `syn/initial-field` | pass | 9/9 | -r a displacement field image |
| `syn/initial-field-linear` | expected-divergence | 1/1 | -r a displacement field image, then Affine (ANTS-72) |
| `syn/initial-field-inverse` | both-error | 2/2 | -r [field,1]: a field without an inverse |
| `syn/initial-composite` | pass | 10/10 | -r a composite holding an affine transform and a field |
| `syn/initial-composite-inverse` | both-error | 2/2 | -r [composite,1]: its field has no inverse |
| `syn/fixed-field` | pass | 9/9 | -q a displacement field image |
| `syn/initial-field-2d` | pass | 9/9 | -r a 2D displacement field image |
| `syn/initial-scalar` | pass | 8/8 | -r a scalar image: ITK reads it as a field of equal components (ANTS-73) |
| `syn/initial-scalar-2d` | pass | 8/8 | -r a 2D scalar image: ITK reads it as complex pixels, both parts equal (ANTS-73) |
| `syn/initial-field-2d-in-3d` | pass | 8/8 | -r a 2D field in 3D: one slice, each vector the product of its components (ANTS-73) |
| `syn/igdm` | both-error | 2/2 | IGDM first in a SyN stage: not implemented in ANTs |
| `syn/bad-variance` | both-error | 2/2 | SyN[0.2,3x]: a variance that does not convert |
| `syn/exponential` | pass | 7/7 | Exponential[0.1,3,0] with CC in SyN's settings (the velocity-field stages have their own categories) |
| `syn-point-sets/icp` | pass | 8/8 | ICP alone: the fixed mask's grid as the virtual domain |
| `syn-point-sets/pse` | pass | 7/7 | PSE (sigma 1, 20 neighbours) |
| `syn-point-sets/jhct` | pass | 13/13 | JHCT, anisotropic (sigma 1, alpha 1.1) |
| `syn-point-sets/icp-float` | pass | 7/7 | ICP, --float 1 |
| `syn-point-sets/icp-moving-initial` | pass | 8/8 | ICP after -r (a matrix) |
| `syn-point-sets/icp-fixed-initial` | pass | 7/7 | ICP after -q (a matrix) |
| `syn-point-sets/icp-not-bijective` | both-error | 2/2 | a fixed label the moving set lacks: ITK throws at the first such point (ITK-22) |
| `syn-point-sets/icp-moving-extra-label` | both-error | 2/2 | a moving label the fixed set lacks: ITK throws in the moving-to-middle update (ITK-22) |
| `syn-point-sets/cc-and-icp` | pass | 7/7 | CC and ICP: ITK gives ICP identity transforms (ITK-33) |
| `syn-point-sets/icp-and-cc` | pass | 7/7 | ICP first, then CC: ICP's virtual domain (ITK-33) |
| `syn-point-sets/cc-and-igdm` | both-error | 2/2 | CC and IGDM: SyN holds labelled point sets only, IGDM gets no input (ANTS-61) |
| `syn-point-sets/cc-igdm-icp` | both-error | 4/4 | CC, IGDM and ICP: IGDM without point sets before a metric with them (ANTS-61) |
| `syn-point-sets/icp-2d` | pass | 7/7 | ICP in 2D |
| `syn-point-sets/cc-regular-and-icp` | both-error | 4/4 | CC sampled (Regular) and ICP: ANTs gives ICP's sample points to a null image metric |
| `syn-point-sets/icp-field-initial` | both-error | 4/4 | ICP after -r a displacement field without an inverse: ITK maps the moving points with a null transform |
| `syn-point-sets/icp-no-mask` | both-error | 4/4 | ICP alone without a fixed mask: no virtual domain |
| `bspline-syn/cc` | pass | 10/10 | BSplineSyN (mesh 2x2x2) with CC (radius 2), 2 levels, warped images |
| `bspline-syn/knot-spacing` | pass | 8/8 | BSplineSyN[0.1,26,0,3] (antsRegistrationSyN.sh's): a knot spacing of 26 mm |
| `bspline-syn/total-mesh` | pass | 7/7 | a total-field mesh (3x3x3, update 4x4x4) |
| `bspline-syn/order-2` | pass | 7/7 | B-splines of order 2 |
| `bspline-syn/order-5` | pass | 7/7 | B-splines of order 5 |
| `bspline-syn/mesh-0` | pass | 7/7 | an update mesh of 0: the update field is not smoothed |
| `bspline-syn/mattes` | pass | 13/13 | Mattes (24 bins) |
| `bspline-syn/mi2` | pass | 7/7 | the joint-histogram MI (MI2) |
| `bspline-syn/gc` | pass | 7/7 | GC: ITK adds every point's derivative to the first voxel (ITK-30) |
| `bspline-syn/demons` | pass | 7/7 | Demons |
| `bspline-syn/multi` | pass | 7/7 | CC (0.7) and Mattes (0.3) |
| `bspline-syn/masks` | pass | 7/7 | -x [mask,mask]: the B-spline fits weighted by the masks resampled through the fields |
| `bspline-syn/fixed-mask-only` | pass | 7/7 | a fixed mask on another grid, no moving mask |
| `bspline-syn/regular` | pass | 7/7 | MeanSquares, regular sampling 0.5 |
| `bspline-syn/random` | pass | 7/7 | Mattes, random sampling 0.6 |
| `bspline-syn/float` | pass | 7/7 | --float 1 |
| `bspline-syn/2d` | pass | 7/7 | 2D, CC |
| `bspline-syn/affine-then` | pass | 10/10 | Affine then BSplineSyN, collapsed: 0GenericAffine.mat, 1Warp, 1InverseWarp |
| `bspline-syn/composite` | pass | 7/7 | Affine then BSplineSyN, -a 1 |
| `bspline-syn/no-collapse` | pass | 8/8 | Affine then BSplineSyN, -z 0 |
| `bspline-syn/per-stage` | pass | 6/6 | -i 1: the listing of the composite (a BSplineSmoothingOnUpdateDisplacementFieldTransform) for the next stage |
| `bspline-syn/restrict-deformation` | pass | 7/7 | -g 1x1x0 |
| `bspline-syn/save-state` | both-error | 2/2 | --save-state: BSplineSyN does not set the registration state (ANTS-50) |
| `bspline-syn/initial-field` | pass | 8/8 | -r a displacement field |
| `bspline-syn/initial-bspline` | pass | 6/6 | -r a B-spline transform: collapsed, ANTs loses the stage's fields (ANTS-76) |
| `bspline-syn/icp` | pass | 7/7 | ICP alone: each fixed point's tangent-space derivative fitted by a B-spline |
| `bspline-syn/cc-and-icp` | pass | 7/7 | CC and ICP (ITK-33) |
| `bspline-syn/igdm` | pass | 7/7 | IGDM alone |
| `bspline-syn/mesh-dimension` | both-error | 2/2 | a 2D mesh in 3D: ERROR, the mesh sizes do not match the dimension |
| `bspline-syn/order-0` | both-error | 2/2 | spline order 0: the B-spline filter throws |
| `bspline-syn/bad-mesh` | both-error | 2/2 | a mesh that does not convert |
| `displacement-field/gaussian-cc` | pass | 8/8 | GaussianDisplacementField with CC (radius 2), 2 levels, warped images |
| `displacement-field/gaussian-meansquares` | pass | 6/6 | MeanSquares, no total-field smoothing |
| `displacement-field/gaussian-mattes` | pass | 6/6 | Mattes (24 bins) |
| `displacement-field/gaussian-mi2` | pass | 11/11 | MI2 |
| `displacement-field/gaussian-gc` | pass | 6/6 | GC (ITK-30) |
| `displacement-field/gaussian-demons` | pass | 6/6 | Demons |
| `displacement-field/gaussian-multi` | pass | 6/6 | CC and Mattes |
| `displacement-field/gaussian-masks` | pass | 6/6 | -x [mask,mask] |
| `displacement-field/gaussian-regular` | pass | 6/6 | regular sampling 0.5 |
| `displacement-field/gaussian-random` | pass | 6/6 | random sampling 0.6 |
| `displacement-field/gaussian-float` | pass | 6/6 | --float 1 |
| `displacement-field/gaussian-2d` | pass | 6/6 | 2D |
| `displacement-field/gaussian-oblique` | pass | 6/6 | oblique grids |
| `displacement-field/gaussian-affine-then` | pass | 8/8 | Affine then the field, collapsed: 0GenericAffine.mat, 1Warp (no inverse) |
| `displacement-field/gaussian-composite` | pass | 6/6 | -a 1: the composite holds the field; no inverse composite |
| `displacement-field/gaussian-no-collapse` | pass | 7/7 | -z 0 |
| `displacement-field/gaussian-restrict` | pass | 6/6 | -g 1x1x0 |
| `displacement-field/gaussian-icp` | pass | 6/6 | MeanSquares and ICP: each point's derivative at its virtual voxel |
| `displacement-field/gaussian-initial-field` | pass | 6/6 | -r a displacement field |
| `displacement-field/gaussian-print-similarity` | both-error | 2/2 | --print-similarity-measure-interval: the observer's copy of the field fails (ANTS-75) |
| `displacement-field/gaussian-interval-volumes` | both-error | 2/2 | --write-interval-volumes: the observer's copy of the field fails (ANTS-75) |
| `displacement-field/gaussian-save-state` | both-error | 2/2 | --save-state: no state (ANTS-50) |
| `displacement-field/gaussian-igdm` | both-error | 2/2 | IGDM first: not implemented in ANTs |
| `displacement-field/gaussian-icp-first` | both-error | 4/4 | ICP first: ANTs allocates the field on a null image |
| `displacement-field/gaussian-variance-missing` | both-error | 2/2 | GaussianDisplacementField[0.25,3]: the total variance is required |
| `displacement-field/bspline-meansquares` | pass | 8/8 | BSplineDisplacementField (mesh 3x3x3, doubled at the first level), MeanSquares, warped |
| `displacement-field/bspline-cc` | pass | 6/6 | CC |
| `displacement-field/bspline-total-mesh` | pass | 6/6 | a total-field mesh (4x4x4, update 5x5x5) |
| `displacement-field/bspline-coarse-total` | both-error | 2/2 | a total mesh of 1: the stationary boundary dominates the fit, the scales fall below epsilon and the stage fails |
| `displacement-field/bspline-order-2` | pass | 6/6 | order 2 |
| `displacement-field/bspline-knot` | pass | 6/6 | a knot spacing (12 mm) |
| `displacement-field/bspline-mattes` | pass | 6/6 | Mattes |
| `displacement-field/bspline-masks` | pass | 6/6 | -x [mask,mask] |
| `displacement-field/bspline-float` | pass | 6/6 | --float 1 |
| `displacement-field/bspline-2d` | pass | 6/6 | 2D |
| `displacement-field/bspline-mesh-dimension` | both-error | 2/2 | a 2D mesh in 3D: ERROR |
| `displacement-field/bspline-order-0` | both-error | 2/2 | order 0: the B-spline filter throws |
| `displacement-field/gaussian-then-bspline` | pass | 7/7 | the two field stages, collapsed into one field |
| `bspline/meansquares` | pass | 8/8 | BSpline (mesh 2x2x2, doubled at each level), MeanSquares, 2 levels, warped |
| `bspline/cc` | pass | 6/6 | CC (radius 2) |
| `bspline/mattes` | pass | 6/6 | Mattes (24 bins) |
| `bspline/mi2` | pass | 11/11 | MI2 |
| `bspline/gc` | pass | 6/6 | GC |
| `bspline/demons` | both-error | 2/2 | Demons: the metric needs a displacement field (ITK's exception) |
| `bspline/multi` | pass | 6/6 | MeanSquares and CC |
| `bspline/masks` | pass | 6/6 | -x [mask,mask] |
| `bspline/regular` | pass | 6/6 | regular sampling 0.5 |
| `bspline/random` | pass | 6/6 | random sampling 0.5 |
| `bspline/float` | pass | 6/6 | --float 1, 2 levels |
| `bspline/2d` | pass | 6/6 | 2D |
| `bspline/oblique` | pass | 6/6 | oblique grids |
| `bspline/knot-spacing` | pass | 6/6 | a knot spacing (30 mm) |
| `bspline/mesh-1` | pass | 6/6 | mesh 1x1x1 |
| `bspline/icp` | pass | 6/6 | MeanSquares and ICP |
| `bspline/affine-then` | pass | 8/8 | Affine then BSpline, collapsed: 0GenericAffine.mat, 1BSpline.txt |
| `bspline/composite` | pass | 6/6 | -a 1: the composite holds the B-spline transform; no inverse |
| `bspline/no-collapse` | pass | 7/7 | -z 0 |
| `bspline/per-stage` | pass | 6/6 | -i 1 -a 1 -z 0: ANTs cannot initialise a B-spline transform from the affine one |
| `bspline/then-affine` | pass | 7/7 | BSpline then Affine: no Jacobian with respect to position; the Affine stage fails and is dropped (ANTS-49) |
| `bspline/then-bspline` | both-error | 2/2 | two BSpline stages: the second fails (no Jacobian with respect to position) |
| `bspline/print-similarity` | both-error | 2/2 | --print-similarity-measure-interval: the observer's copy of the transform fails (ANTS-75) |
| `bspline/print-similarity-mesh-1` | pass | 6/6 | --print-similarity-measure-interval with mesh 1: the copy works |
| `bspline/restrict` | both-error | 2/2 | -g 1x1x0: weights for the dimension, not the parameters (ITK's exception) |
| `bspline/save-state` | both-error | 2/2 | --save-state: no state (ANTS-50) |
| `bspline/initial-bspline-syn` | pass | 7/7 | -r a B-spline transform, then SyN: collapsed, ANTs loses the SyN fields (ANTS-76) |
| `bspline/initial-bspline-syn-composite` | pass | 6/6 | -r an affine and B-spline composite, then SyN, -a 1 |
| `bspline/initial-bspline-affine` | pass | 7/7 | -r a B-spline transform, then Affine: the stage fails and is dropped (ANTS-49) |
| `bspline/fixed-bspline` | pass | 7/7 | -q a B-spline transform |
| `bspline/initial-bspline-inverse` | both-error | 2/2 | -r [bspline,1]: a B-spline transform has no inverse |
| `bspline/restore-bspline` | pass | 7/7 | --restore-state a B-spline transform, then SyN (ANTS-76) |
| `bspline/interval-volumes-mesh-1` | pass | 9/9 | --write-interval-volumes with mesh 1: the copies work, the volumes through the B-spline transform |
| `bspline/mesh-0` | expected-divergence | 1/1 | a mesh size of 0: ANTs divides by zero and fails on garbage indices (ANTS-77) |
| `bspline/mesh-short` | expected-divergence | 1/1 | a 2D mesh in 3D: ANTs reads past the end of the list (ANTS-77) |
| `bspline/icp-first` | both-error | 4/4 | ICP first: ANTs reads the null image of the first metric |
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
| `real/ds000005-sub-01/boldref-affine` | pass | 7/7 | fMRIPrep 25.2.6's enhance_and_skullstrip_bold_wf/norm of ds000005-sub-01-noreconall |
| `real/ds005040-sub-002/boldref-affine` | pass | 11/11 | fMRIPrep 25.2.6's enhance_and_skullstrip_bold_wf/norm of ds005040-sub-002-noreconall |
| `real/ds005040-sub-002/coregister` | pass | 7/7 | fMRIPrep 25.2.6's fmapreg_wf/coregister of ds005040-sub-002-noreconall |
| `real/ds005454-sub-16/coregister` | pass | 6/6 | fMRIPrep 25.2.6's fmapreg_wf/coregister of ds005454-sub-16-noreconall |
| `real-syn-small/ds000005-sub-01/brain-extraction` | pass | 18/18 | fMRIPrep 25.2.6's norm of ds000005-sub-01-noreconall, whole (Rigid, Affine, SyN), on one voxel in 3 along each axis of … |
| `real-linear/ds000005-sub-01/template` | pass | 15/15 | the linear stages of fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall (its SyN stage left out) |
| `real-syn-small/ds000005-sub-01/template` | pass | 16/16 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall, whole (Rigid, Affine, SyN), on one voxel in 3 along each … |
| `real-bsplinesyn-small/ds000005-sub-01/template` | pass | 16/16 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall with BSplineSyN[0.1,26,0,3] (antsRegistrationSyN.sh's) for… |
| `real-exponential-small/ds000005-sub-01/template` | pass | 16/16 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall with Exponential[0.25,3,0.5] for its SyN stage, on one vox… |
| `real-bspline-exponential-small/ds000005-sub-01/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall with BSplineExponential[0.25,26,0,0,3] for its SyN stage, … |
| `real-time-varying-small/ds000005-sub-01/template` | pass | 16/16 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall with TimeVaryingVelocityField[0.5,4,3,0,0,0] for its SyN s… |
| `real-time-varying-bspline-small/ds000005-sub-01/template` | pass | 15/15 | fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall with TimeVaryingBSplineVelocityField[0.5,4x4x4x2,4,3] for … |
