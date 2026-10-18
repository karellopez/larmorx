# Parity: antsRegistration (linear stages)

**All cases agree.** 224 cases on the `standard` tier: 169 pass, 44 rejected by both, 11 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsRegistration` / `lx.ants.registration` (crates larmorx-registration, larmorx-ants)
- **Reference:** antsRegistration from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread and ANTS_RANDOM_SEED=42
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-18 on Linux x86_64, with `python -m larmorx_validation parity ants-registration --tier standard`

169 cases compared: 145 identical to the unmodified ANTs (exit status, standard output with every DIAGNOSTIC line, standard error, every file), 24 identical once ANTs uses CORE-MATH's correctly rounded `exp`, `log`, `sin` and `cos` (glibc's are not always correctly rounded).

Thread band, 18 cases: ANTs at 2, 4, 8 threads differs from its own one-thread result by up to 0.00488 in a transform parameter (`smoke/rigid` 7.04e-13, `smoke/affine-float` 0.00488, `metrics/mattes-mi-alias` 6.79e-13, `metrics/mi2` 3.01e-14, `metrics/mi2-dense` 9.71e-17, `metrics/meansquares` 8.67e-14, `metrics/gc` 1.61e-12, `metrics/gc-dense` 1.45e-13, `metrics/cc-sampled` 2.47e-13, `metrics/cc-dense` 5.47e-14, `metrics/mattes-gradient-filter` 1.06e-15, `metrics/cc-gradient-filter` 9.42e-14, `point-sets/icp` 0, `point-sets/pse` 1.11e-16, `point-sets/jhct` 1.11e-16, `point-sets/igdm` 2.12e-22, `real/ds000005-sub-01/boldref-affine` 3.9e-10, `real/ds005040-sub-002/coregister` 4.68e-13); larmorx gives the one-thread result at any thread count, within 3.47e-18 of ANTs at one thread.

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
| dimensions-2d | 24 | 22 | 2 | 0 | 0 |
| dimensions-4d | 10 | 10 | 0 | 0 | 0 |
| errors | 36 | 2 | 28 | 6 | 0 |
| initial | 11 | 9 | 1 | 1 | 0 |
| metrics | 22 | 22 | 0 | 0 | 0 |
| outputs | 7 | 7 | 0 | 0 | 0 |
| point-sets | 57 | 41 | 13 | 3 | 0 |
| real | 4 | 4 | 0 | 0 | 0 |
| real-linear | 1 | 1 | 0 | 0 | 0 |
| sampling | 11 | 10 | 0 | 1 | 0 |
| smoke | 3 | 3 | 0 | 0 | 0 |
| transforms | 38 | 38 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `sampling/no-seed` | without a seed ANTs seeds its sampling from the clock and its result changes from run to run; larmorx uses a fixed seed |
| `initial/names-restored` | ANTs reads past the end of its list of file names and crashes (ANTS-51); larmorx writes the same files up to there and stops with an error |
| `errors/syn` | deformable registration is milestone A5; larmorx refuses with a message |
| `errors/version` | larmorx prints its own version and the ANTs version it ports |
| `errors/version-0` | ANTs exits 1 without a word (the reason is printed only with --verbose, which this command line does not set); larmorx prints the reason (ANTS-45) |
| `errors/no-dimension-quiet` | ANTs exits 1 without a word (the reason is printed only with --verbose, which this command line does not set); larmorx prints the reason (ANTS-45) |
| `errors/unbalanced-bracket` | ANTs exits 1 without a word (the reason is printed only with --verbose, which this command line does not set); larmorx prints the reason (ANTS-45) |
| `errors/minc` | the oracle has no MINC transform IO (it prints ITK's error and exits 0); larmorx refuses MINC transform files with a message |
| `point-sets/icp-sampled` | ANTs draws the points of a sampling percentage below 1 with a generator seeded from the clock (ANTS-64): two runs differ; larmorx seeds it from the registration's seed |
| `point-sets/igdm-neighbourhood` | ANTs' line search computes IGDM's centre index from a negative number and reads memory before the intensity arrays (ANTS-66, undefined); larmorx takes the two intensities as equal |
| `point-sets/igdm-2d` | ANTs reads memory before IGDM's intensity arrays for neighbourhoods of more than one voxel (ANTS-66) |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `initial/save-state` | --save-state after linear stages: ANTs has no state and exits 1 (ANTS-50) | 1 |
| `dimensions-2d/file-unregistered` | -r an ANTSCenteredAffine2DTransform file: ITK's factory does not know the class | "VersorTransform_float_3_3" |
| `dimensions-2d/file-3d` | -r a 3D transform in a 2D registration | 1 |
| `errors/no-arguments` | no arguments: the short menu | 1 |
| `errors/bad-number` | -c [1x,1e-6,5]: a number that does not convert | 1 |
| `errors/levels-mismatch` | two shrink factors for one level | 1 |
| `errors/missing-image` | a metric image that does not exist: ANTs uses the null image and crashes (ANTS-02) | file /tmp/lx-reg-8tiyuzyh/none.nii.gz does not exist . |
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
| `errors/sampling-above-1` | a sampling percentage of 1.5 | Description: ITK ERROR: ImageRegistrationMethodv4(0x5a16a0812bc0): sampling percentage outside expected (0,1] range |
| `errors/missing-initial` | -r a transform file that does not exist | Transform file does not exist: /tmp/lx-reg-ubp11596/none.mat |
| `point-sets/igdm-few-parameters` | IGDM with three parameters | The expected number of parameters aren't specified. Please see help menu. |
| `point-sets/igdm-radius-dimension` | IGDM with a 2D radius in 3D | The neighborhood size must equal the dimension. |
| `point-sets/icp-percentage-above-1` | A sampling percentage of 1.5 | Description: ITK ERROR: ImageRegistrationMethodv4(0x62c7eb93de60): sampling percentage outside expected (0,1] range |
| `point-sets/icp-no-mask` | Point-set metrics without a fixed mask: no virtual domain | larmorx: antsRegistration: a stage of point-set metrics needs a fixed mask (-x) to define its virtual domain: ANTs reads the spacing of a null image; ANTs cras… |
| `point-sets/icp-missing` | A point set that does not exist | larmorx: antsRegistration: a labelled point set could not be read: ANTs goes on with a null point set and crashes |
| `point-sets/icp-short-name` | A point set named with fewer than 3 characters | larmorx: antsRegistration: a labelled point set could not be read: ANTs goes on with a null point set and crashes |
| `point-sets/icp-unreadable` | A point set no reader takes (ITK's image reader throws) | larmorx: upstream defect ANTS-62 (antsRegistration) reproduced: '/tmp/lx-reg-d16qg81c/moving.txt' ends with a newline: an extra point, a copy of the last one, … |
| `point-sets/icp-vtk-no-points` | A VTK file without POINTS: libstdc++'s out_of_range | larmorx: antsRegistration: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `point-sets/igdm-missing-mask` | An IGDM mask that does not exist | larmorx: antsRegistration: an intensity point set could not be read: ANTs goes on with a null point set and crashes |
| `point-sets/icp-warped` | Warped images with a point-set metric first | larmorx: antsRegistration: warped images with a point-set metric first: ANTs resamples null images and crashes, larmorx stops |
| `point-sets/icp-print-similarity` | --print-similarity-measure-interval with a point-set metric first | larmorx: antsRegistration: --print-similarity-measure-interval with a point-set metric first: ANTs' observer computes the full-scale similarity of null images;… |
| `point-sets/icp-interval-volumes` | --write-interval-volumes with a point-set metric first | larmorx: antsRegistration: --write-interval-volumes with a point-set metric first: ANTs' observer resamples a null image; ANTs crashes here, larmorx stops |
| `point-sets/mattes-regular-then-icp` | Regular sampling with a point-set metric in the stage | larmorx: antsRegistration: a stage that samples its metrics (Regular or Random) with a point-set metric: ANTs gives the sample points to a null image metric an… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (8bac0bf019bc) |
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
| `errors/syn` | expected-divergence | 1/1 | a SyN stage: larmorx ports deformable stages in A5 |
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
| `real/ds000005-sub-01/boldref-affine` | pass | 7/7 | fMRIPrep 25.2.6's enhance_and_skullstrip_bold_wf/norm of ds000005-sub-01-noreconall |
| `real/ds005040-sub-002/boldref-affine` | pass | 11/11 | fMRIPrep 25.2.6's enhance_and_skullstrip_bold_wf/norm of ds005040-sub-002-noreconall |
| `real/ds005040-sub-002/coregister` | pass | 7/7 | fMRIPrep 25.2.6's fmapreg_wf/coregister of ds005040-sub-002-noreconall |
| `real/ds005454-sub-16/coregister` | pass | 6/6 | fMRIPrep 25.2.6's fmapreg_wf/coregister of ds005454-sub-16-noreconall |
| `real-linear/ds000005-sub-01/template` | pass | 15/15 | the linear stages of fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall (its SyN stage left out) |
