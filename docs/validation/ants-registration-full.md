# Parity: antsRegistration (linear stages)

**All cases agree.** 175 cases on the `full` tier: 135 pass, 31 rejected by both, 9 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsRegistration` / `lx.ants.registration` (crates larmorx-registration, larmorx-ants)
- **Reference:** antsRegistration from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread and ANTS_RANDOM_SEED=42
- **Test data:** larmorx-testdata `00b862132e9c`, tier `full`
- **Generated:** 2026-10-17 on Linux x86_64, with `python -m larmorx_validation parity ants-registration --tier full`

135 cases compared: 102 identical to the unmodified ANTs (exit status, standard output with every DIAGNOSTIC line, standard error, every file), 33 identical once ANTs uses CORE-MATH's correctly rounded `exp`, `log`, `sin` and `cos` (glibc's are not always correctly rounded).

Thread band, 14 cases: ANTs at 2, 4, 8 threads differs from its own one-thread result by up to 0.00488 in a transform parameter (`smoke/rigid` 6.85e-13, `smoke/affine-float` 0.00488, `metrics/mattes-mi-alias` 1.61e-13, `metrics/mi2` 3.01e-14, `metrics/mi2-dense` 9.71e-17, `metrics/meansquares` 8.67e-14, `metrics/gc` 1.61e-12, `metrics/gc-dense` 1.45e-13, `metrics/cc-sampled` 2.47e-13, `metrics/cc-dense` 5.47e-14, `metrics/mattes-gradient-filter` 4.44e-16, `metrics/cc-gradient-filter` 9.42e-14, `real/ds000005-sub-01/boldref-affine` 3.51e-09, `real/ds005040-sub-002/coregister` 6.09e-13); larmorx gives the one-thread result at any thread count, within 3.47e-18 of ANTs at one thread.

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
| errors | 37 | 2 | 28 | 7 | 0 |
| initial | 11 | 9 | 1 | 1 | 0 |
| metrics | 20 | 20 | 0 | 0 | 0 |
| outputs | 7 | 7 | 0 | 0 | 0 |
| real | 4 | 4 | 0 | 0 | 0 |
| real-linear | 10 | 10 | 0 | 0 | 0 |
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
| `errors/point-set-metric` | larmorx does not port ANTs' point-set metrics (ICP, PSE, JHCT, IGDM) yet and refuses them with a message |
| `errors/minc` | the oracle has no MINC transform IO (it prints ITK's error and exits 0); larmorx refuses MINC transform files with a message |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `initial/save-state` | --save-state after linear stages: ANTs has no state and exits 1 (ANTS-50) | 1 |
| `dimensions-2d/file-unregistered` | -r an ANTSCenteredAffine2DTransform file: ITK's factory does not know the class | "VersorTransform_float_3_3" |
| `dimensions-2d/file-3d` | -r a 3D transform in a 2D registration | 1 |
| `errors/no-arguments` | no arguments: the short menu | 1 |
| `errors/bad-number` | -c [1x,1e-6,5]: a number that does not convert | 1 |
| `errors/levels-mismatch` | two shrink factors for one level | 1 |
| `errors/missing-image` | a metric image that does not exist: ANTs uses the null image and crashes (ANTS-02) | file /tmp/lx-reg-7g58zkqp/none.nii.gz does not exist . |
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
| `errors/sampling-above-1` | a sampling percentage of 1.5 | Description: ITK ERROR: ImageRegistrationMethodv4(0x6416f6a7cbc0): sampling percentage outside expected (0,1] range |
| `errors/missing-initial` | -r a transform file that does not exist | Transform file does not exist: /tmp/lx-reg-xxo2rine/none.mat |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (36aa668df831) |
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
| `errors/point-set-metric` | expected-divergence | 1/1 | an ICP stage (a point-set metric) |
| `errors/minc` | expected-divergence | 1/1 | --minc 1: MINC transform files |
| `real/ds000005-sub-01/boldref-affine` | pass | 7/7 | fMRIPrep 25.2.6's enhance_and_skullstrip_bold_wf/norm of ds000005-sub-01-noreconall |
| `real/ds005040-sub-002/boldref-affine` | pass | 11/11 | fMRIPrep 25.2.6's enhance_and_skullstrip_bold_wf/norm of ds005040-sub-002-noreconall |
| `real/ds005040-sub-002/coregister` | pass | 7/7 | fMRIPrep 25.2.6's fmapreg_wf/coregister of ds005040-sub-002-noreconall |
| `real/ds005454-sub-16/coregister` | pass | 6/6 | fMRIPrep 25.2.6's fmapreg_wf/coregister of ds005454-sub-16-noreconall |
| `real-linear/ds000005-sub-01/brain-extraction` | pass | 13/13 | the linear stages of fMRIPrep 25.2.6's norm of ds000005-sub-01-noreconall (its SyN stage left out) |
| `real-linear/ds000005-sub-01/template` | pass | 15/15 | the linear stages of fMRIPrep 25.2.6's registration of ds000005-sub-01-noreconall (its SyN stage left out) |
| `real-linear/ds001600-sub-1/brain-extraction` | pass | 13/13 | the linear stages of fMRIPrep 25.2.6's norm of ds001600-sub-1-noreconall (its SyN stage left out) |
| `real-linear/ds001600-sub-1/template` | pass | 15/15 | the linear stages of fMRIPrep 25.2.6's registration of ds001600-sub-1-noreconall (its SyN stage left out) |
| `real-linear/ds005040-sub-002/brain-extraction` | pass | 13/13 | the linear stages of fMRIPrep 25.2.6's norm of ds005040-sub-002-noreconall (its SyN stage left out) |
| `real-linear/ds005040-sub-002/template` | pass | 15/15 | the linear stages of fMRIPrep 25.2.6's registration of ds005040-sub-002-noreconall (its SyN stage left out) |
| `real-linear/ds005454-sub-16/brain-extraction` | pass | 13/13 | the linear stages of fMRIPrep 25.2.6's norm of ds005454-sub-16-noreconall (its SyN stage left out) |
| `real-linear/ds005454-sub-16/template` | pass | 15/15 | the linear stages of fMRIPrep 25.2.6's registration of ds005454-sub-16-noreconall (its SyN stage left out) |
| `real-linear/ds005752-sub-ON01016/brain-extraction` | pass | 13/13 | the linear stages of fMRIPrep 25.2.6's norm of ds005752-sub-ON01016-noreconall (its SyN stage left out) |
| `real-linear/ds005752-sub-ON01016/template` | pass | 15/15 | the linear stages of fMRIPrep 25.2.6's registration of ds005752-sub-ON01016-noreconall (its SyN stage left out) |
