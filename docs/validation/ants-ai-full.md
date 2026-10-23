# Parity: antsAI

**All cases agree.** 144 cases on the `full` tier: 87 pass, 53 rejected by both, 4 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsAI` / `lx.ants.ai` (crates larmorx-ants, larmorx-registration)
- **Reference:** antsAI from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread
- **Test data:** larmorx-testdata `00b862132e9c`, tier `full`
- **Generated:** 2026-10-23 on Linux x86_64, with `python -m larmorx_validation parity ants-ai --tier full`

87 cases compared: 86 identical to the unmodified ANTs (exit status, standard output, standard error, every file), 1 identical once ANTs uses CORE-MATH's correctly rounded functions (glibc's are not always correctly rounded).

Thread band, 6 cases: ANTs at 2, 4, 6 threads differs from its own one-thread result by up to 0.137 in a transform parameter (`search/rigid-grid` 3.89e-16, `search/affine-mi` 4.07e-20, `search/rigid-gc` 1.7e-14, `search/2d-affine-grid` 4.72e-16, `real/ds000005-sub-01/brain-extraction` 9.95e-14, `real/ds000005-sub-01/bold-mixedgamblestask-run-01` 0.137); larmorx gives the one-thread result at any thread count, within 0 of ANTs at one thread.

Thread band, blob alignment (2 cases: `blobs/shift-rigid`, `real/ds000005-sub-01/blobs-t1w-shift`): ANTs' `antsAI -b` crashes at 2, 4, 6 threads (ANTS-60: its blob detector indexes per-thread heaps past their end); larmorx gives ANTs' one-thread result at any thread count.

**Blob alignment against the known transforms** (`-b`; where the moving image is the fixed one moved by a known transform: the distance from where each program's transform sends test points to where they truly are; larmorx's default corrects ANTS-110, ANTS-111, ANTS-112, ANTS-114 and ITK-60, `reproduce` is ANTs' result):

| Case | ANTs 2.6.5 (one thread) | larmorx's default |
|---|---|---|
| `blobs/shift-rigid` | 0 mm mean, 0 mm largest | 0.497 mm mean, 1.38 mm largest |
| `blobs/shift-affine` | 1.11e-14 mm mean, 2.2e-14 mm largest | 0.483 mm mean, 1.92 mm largest |
| `blobs/shift-similarity` | 0 mm mean, 0 mm largest | 0.497 mm mean, 1.38 mm largest |
| `blobs/rigid-rigid` | 34 mm mean, 66.8 mm largest | no transform |
| `blobs/rigid-affine` | 654 mm mean, 1.52e+03 mm largest | no transform |
| `blobs/affine-rigid` | nan mm mean, nan mm largest | no transform |
| `blobs/2d-shift-rigid` | 0 mm mean, 0 mm largest | 0.33 mm mean, 0.422 mm largest |
| `blobs/2d-shift-affine` | 5.81e-14 mm mean, 2.07e-13 mm largest | 0.471 mm mean, 1.16 mm largest |
| `blobs/2d-rigid-rigid` | 6.2 mm mean, 12.3 mm largest | 1.62 mm mean, 3.54 mm largest |
| `blobs/2d-rigid-affine` | 45.4 mm mean, 125 mm largest | 2.73 mm mean, 11.2 mm largest |
| `blobs/2d-rigid-similarity` | 6.2 mm mean, 12.3 mm largest | 1.62 mm mean, 3.54 mm largest |
| `blobs/shift-rigid-match-10` | 0 mm mean, 0 mm largest | 0.577 mm mean, 1.61 mm largest |
| `blobs/shift-rigid-search` | 3.83 mm mean, 7.99 mm largest | 4.11 mm mean, 9.07 mm largest |
| `blobs/rigid-affine-search-h5` | 603 mm mean, 1.25e+03 mm largest | no transform |
| `blobs/principal-axes-ignored` | 0 mm mean, 0 mm largest | 0.497 mm mean, 1.38 mm largest |
| `real/ds000005-sub-01/blobs-t1w-shift` | 5.92e-16 mm mean, 1.78e-15 mm largest | 1.19 mm mean, 2.1 mm largest |
| `real/ds000005-sub-01/blobs-t1w-rotated` | nan mm mean, nan mm largest | 1.03 mm mean, 1.87 mm largest |

- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - ANTS-110: 14 case(s) named it, 14 with changed values (largest difference inf): `blobs/shift-rigid`, `blobs/shift-affine`, `blobs/shift-similarity`, `blobs/2d-shift-rigid`, `blobs/2d-shift-affine`, `blobs/2d-rigid-rigid`, `blobs/2d-rigid-affine`, `blobs/2d-rigid-similarity` …
  - ANTS-111: 16 case(s) named it, 11 with changed values (largest difference inf), 5 with other changes (a transform written by one only; exit N -> N; files ['t.hN']; files ['t.mat']): `blobs/shift-rigid`, `blobs/shift-affine`, `blobs/shift-similarity`, `blobs/rigid-rigid`, `blobs/rigid-affine`, `blobs/2d-shift-rigid`, `blobs/2d-shift-affine`, `blobs/2d-rigid-rigid` …
  - ANTS-112: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (a transform written by one only; exit N -> N; files ['t.mat']): `blobs/constant`
  - ANTS-114: 11 case(s) named it, 0 with changed values (largest difference 0), 11 with other changes (a transform written by one only; exit N -> N; files ['t.mat']): `blobs/rigid-rigid`, `blobs/affine-rigid`, `blobs/no-pairs-rigid`, `blobs/2d-no-pairs-rigid`, `blobs/constant`, `blobs/noise-rigid`, `real/ds000005-sub-01/blobs-mni-rigid`, `real/ds001600-sub-1/blobs-mni-rigid` …
  - ANTS-53: 4 case(s) named it, 0 with changed values (largest difference 0), 4 with other changes (a transform written by one only): `search/align-with-iterations`, `search/align-with-iterations-quiet`, `search/align-with-iterations-2d-gc`, `search/align-with-iterations-no-start`
  - ANTS-55: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (a transform written by one only; exit N -> N; files ['t.mat']): `search/grid-two-extents`, `search/grid-one-extent`
  - ITK-18: 3 case(s) named it, 2 with changed values (largest difference 0.0294): `search/affine-mi`, `search/2d-mi-dense`, `search/mi-empty-mask-dense`

fMRIPrep's own transforms (ANTs 2.6.2, 8 threads, fMRIPrep's seed): larmorx is within 1.56e-13 (`real/ds000005-sub-01/brain-extraction`), 1.28e-13 (`real/ds000005-sub-01/bold-mixedgamblestask-run-01`), 1.71e-13 (`real/ds000005-sub-01/bold-mixedgamblestask-run-02`), 8.53e-14 (`real/ds000005-sub-01/bold-mixedgamblestask-run-03`), 1.22e-12 (`real/ds001600-sub-1/brain-extraction`), 2.22e-16 (`real/ds005040-sub-002/brain-extraction`), 9.59e-14 (`real/ds005040-sub-002/bold-clip-run-1`), 5.68e-14 (`real/ds005454-sub-16/brain-extraction`), 1.46e-09 (`real/ds005454-sub-16/bold-rest`), 1.42e-13 (`real/ds005752-sub-ON01016/brain-extraction`) of them.

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | identical |
| Standard output | identical after masking addresses and output paths |
| Standard error | identical (larmorx's D13 notices aside) |
| Files | transforms byte for byte (HDF5: every dataset) |
| glibc's maths | where ANTs as built differs, ANTs with CORE-MATH preloaded must be identical |
| Aborts and crashes | everything ANTs printed before it stopped; larmorx exits 1 |
| Thread band | reported: ANTs at 2, 4, 6 threads against 1 |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| blobs | 25 | 20 | 5 | 0 | 0 |
| initial | 9 | 9 | 0 | 0 | 0 |
| messages | 51 | 3 | 46 | 2 | 0 |
| random | 2 | 0 | 0 | 2 | 0 |
| real | 19 | 17 | 2 | 0 | 0 |
| search | 38 | 38 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `messages/search-factor-negative` | ANTs never finishes (class (a)); larmorx refuses |
| `messages/translation-step-zero` | ANTs never finishes (class (a)); larmorx refuses |
| `random/random-sampling` | ANTs seeds from the clock (ANTS-58): two runs differ; larmorx is repeatable |
| `random/seed-zero` | ANTs seeds from the clock (ANTS-58): two runs differ; larmorx is repeatable |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `messages/no-arguments` | No arguments: the short menu, exit 1 | Print the help menu. |
| `messages/help-0` | --help 0: nothing to do (ANTS-45) | Image dimensionality not specified. See command line option --dimensionality |
| `messages/h-0` | -h 0 (ANTS-45) | Image dimensionality not specified. See command line option --dimensionality |
| `messages/no-dimension` | No -d | Image dimensionality not specified. See command line option --dimensionality |
| `messages/dimension-4` | -d 4 | Unrecognized dimensionality. Please see help menu. |
| `messages/dimension-1` | -d 1 | Unrecognized dimensionality. Please see help menu. |
| `messages/no-metric` | No -m | Input metric not specified. |
| `messages/no-metric-quiet` | No -m, without -v | Input metric not specified. |
| `messages/one-image` | -m with one image | Fixed and/or moving images not specified. |
| `messages/unknown-sampling` | An unknown sampling | Unrecognized sampling strategy. |
| `messages/unknown-transform` | An unknown transform | Unrecognized transform option. |
| `messages/unknown-transform-quiet` | An unknown transform, without -v (ANTS-45) | Unrecognized transform option. |
| `messages/no-transform` | No -t | Unrecognized transform option. |
| `messages/unknown-metric` | An unknown metric with iterations | ERROR: Unrecognized metric. |
| `messages/unknown-metric-align` | An unknown metric, AlignCentersOfMass with iterations | ERROR: Unrecognized metric. |
| `messages/two-blobs` | -b 2: too few blobs | Please specify a greater number of blobs (>=3). |
| `messages/two-blobs-bracket` | -b [2,5] | Please specify a greater number of blobs (>=3). |
| `messages/unknown-flag` | An unknown flag | ERROR: Invalid flag provided bogus |
| `messages/missing-fixed` | A fixed image that does not exist | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/missing-moving-geometry` | AlignGeometricCenters with a missing moving image | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/missing-fixed-mass` | AlignCentersOfMass with a missing fixed image | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/bad-iterations` | -c 1o: the parser's conversion fails | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/bad-bins` | 32 bins written 3x2 | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/unbalanced-bracket` | An unbalanced bracket | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/bad-seed` | --random-seed 1x | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/bad-env-seed` | ANTS_RANDOM_SEED=abc (std::stoi throws) | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/env-seed-range` | ANTS_RANDOM_SEED out of int's range | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/zero-moving-mass` | AlignCentersOfMass, a moving image of zeros | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/zero-principal-axes` | Principal axes of an image of zeros | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/zero-fixed-search` | A search from a fixed image of zeros | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/unknown-extension` | An output name no writer takes | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/no-directory` | An output in a directory that does not exist | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/align-iterations-unknown-extension` | ANTS-53 with an output name no writer takes | larmorx: upstream defect ANTS-53 (antsAI) reproduced: AlignGeometricCenters and AlignCentersOfMass with iterations write no transform, as in ANTs [LARMORX_UPST… |
| `messages/align-iterations-txt` | ANTS-53 into a text file: ITK's writer reads an empty list | larmorx: upstream defect ANTS-53 (antsAI) reproduced: AlignGeometricCenters and AlignCentersOfMass with iterations write no transform, as in ANTs [LARMORX_UPST… |
| `messages/align-iterations-h5` | ANTS-53 into an HDF5 file | larmorx: upstream defect ANTS-53 (antsAI) reproduced: AlignGeometricCenters and AlignCentersOfMass with iterations write no transform, as in ANTs [LARMORX_UPST… |
| `messages/empty-start-list` | A negative arc fraction: no start point, the best of an empty list | larmorx: antsAI: no start point (a negative arc fraction or translation grid): ANTs reads the best parameters from an empty list and crashes |
| `messages/empty-grid` | A negative grid extent: no start point | larmorx: antsAI: no start point (a negative arc fraction or translation grid): ANTs reads the best parameters from an empty list and crashes |
| `messages/empty-sample-mattes` | Regular sampling inside an empty mask: no sample point | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/empty-sample-mi` | The same with MI | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/empty-sample-gc` | The same with GC | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/empty-sample-align` | No sample point, AlignGeometricCenters with iterations | larmorx: upstream defect ANTS-53 (antsAI) reproduced: AlignGeometricCenters and AlignCentersOfMass with iterations write no transform, as in ANTs [LARMORX_UPST… |
| `messages/random-zero` | Random sampling of 0: no sample point | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/mattes-empty-mask-dense` | Mattes over every voxel with an empty fixed mask (ITK-20: the second start point crashes) | larmorx: antsAI: start point 0 threw (Negative spacing is not supported and may result in undefined behavior. Refusing to change spacing from [1, 1] to [-2.430… |
| `messages/mattes-empty-moving-mask` | Mattes with an empty moving mask (ITK-20) | larmorx: antsAI: start point 0 threw (Negative spacing is not supported and may result in undefined behavior. Refusing to change spacing from [1, 1] to [16.229… |
| `messages/mattes-one-start-point` | Mattes with an empty mask and one start point: the best value of an empty list | larmorx: antsAI: every start point threw: ANTs' observer reads the best metric value from an empty list and crashes |
| `messages/percentage-zero` | Regular sampling of 0 (ANTS-06), one sample point: one intensity (ITK-20) | larmorx: upstream defect ANTS-06 (ANTs) reproduced: the Regular sampling step: unsigned long of inf is undefined; 9223372036854775808, as x86-64 converts it [L… |
| `blobs/affine-affine` | Affine from 30 blobs of a phantom and its copy moved by a rotation of 5 degrees with scales 1.06 and 0.96 | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `blobs/no-pairs-affine` | -b [5,0] with Affine: too few landmarks, ITK's exception | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `blobs/thin` | An axis of 3 voxels: the Laplacian of Gaussian refuses it (ITK's exception) | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `blobs/missing-fixed` | A fixed image that does not exist: the blob detector's input is not set | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `blobs/missing-moving` | A moving image that does not exist (after the fixed image's blobs) | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `real/ds000005-sub-01/blobs-t1w-rotated-affine` | The same with Affine (ANTs: too few landmarks) | larmorx: upstream defect ANTS-110 (antsAI) reproduced: 5 of 7 blob pairs preserve their distances to at least 3 others; ANTs' test of the displacement lengths … |
| `real/ds005040-sub-002/blobs-oasis-affine` | OASIS30ANTs (1 mm) against ds005040's T1w, Affine from 100 blobs | larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (708bb99389fe) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

Inputs named `gen:<name>` are built by the suites (seeded phantoms); the real cases re-run fMRIPrep 25.2.6's own command lines from the oracle runs.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `initial/rigid` | pass | 6/6 | Rigid, no search: the centres of mass |
| `initial/rigid-principal-axes` | pass | 6/6 | Rigid from the principal axes (Wahba) |
| `initial/affine-principal-axes` | pass | 6/6 | Affine from the principal axes |
| `initial/similarity-principal-axes` | pass | 11/11 | Similarity from the principal axes (Similarity3D's matrix: cbrt) |
| `initial/geometric-centers` | pass | 6/6 | AlignGeometricCenters |
| `initial/centers-of-mass` | pass | 6/6 | AlignCentersOfMass |
| `initial/2d-rigid-principal-axes` | pass | 6/6 | 2D Rigid from the principal axes |
| `initial/2d-centers-of-mass-txt` | pass | 6/6 | 2D AlignCentersOfMass, a text file |
| `initial/h5` | pass | 6/6 | Affine from the principal axes into an HDF5 file |
| `search/rigid-grid` | pass | 7/7 | Rigid search with a translation grid |
| `search/similarity` | pass | 6/6 | Similarity search |
| `search/affine-mi` | pass | 7/7 | Affine search, joint-histogram MI |
| `search/rigid-gc` | pass | 7/7 | Rigid search from the principal axes, GC |
| `search/affine-oblique` | pass | 6/6 | Affine search on oblique grids (1.5 x 1.2 x 2 mm) |
| `search/masks` | pass | 6/6 | Affine search with fixed and moving masks |
| `search/fixed-mask` | pass | 6/6 | Rigid search with a fixed mask |
| `search/missing-mask` | pass | 6/6 | A mask that cannot be read is no mask |
| `search/dense` | pass | 6/6 | Rigid search over every voxel (no sampling) |
| `search/gc-dense` | pass | 6/6 | GC over every voxel |
| `search/2d-affine-grid` | pass | 7/7 | 2D Affine search with a translation grid |
| `search/2d-rigid-grid` | pass | 6/6 | 2D Rigid with a translation grid (ANTS-52: the grid is ignored) |
| `search/2d-rigid` | pass | 6/6 | 2D Rigid from the principal axes |
| `search/2d-similarity-gc` | pass | 6/6 | 2D Similarity, GC over every pixel |
| `search/2d-mi-dense` | pass | 6/6 | 2D Affine, MI over every pixel |
| `search/similarity-grid` | pass | 6/6 | 3D Similarity with a translation grid (ANTS-52: the grid is ignored) |
| `search/grid-two-extents` | pass | 6/6 | A grid of two extents in 3D (ANTS-55: the third is read past the end) |
| `search/grid-one-extent` | pass | 6/6 | A grid of one extent in 3D (ANTS-55) |
| `search/arc-zero` | pass | 6/6 | An arc fraction of 0: one angle per axis |
| `search/arc-wide` | pass | 6/6 | An arc fraction of 3 (beyond a full turn) |
| `search/seed` | pass | 6/6 | --random-seed 7 |
| `search/env-seed` | pass | 6/6 | ANTS_RANDOM_SEED=99 |
| `search/env-seed-prefix` | pass | 6/6 | ANTS_RANDOM_SEED=12abc (std::stoi reads 12) |
| `search/percentage-above-one` | pass | 6/6 | Regular sampling of 5 (every voxel) |
| `search/quiet` | pass | 6/6 | Without -v: nothing printed |
| `search/txt` | pass | 6/6 | A text transform file |
| `search/h5` | pass | 6/6 | An HDF5 transform file (no compression after a search) |
| `search/plain-values` | pass | 6/6 | -s 90 and -c 2 without brackets |
| `search/two-outputs` | pass | 6/6 | Two -o: the last is written |
| `search/verbose-last` | pass | 6/6 | -v 1 -v 0: the last one counts |
| `search/unknown-metric-initial` | pass | 6/6 | An unknown metric with 0 iterations: the metric is not used |
| `search/empty-start-list-no-output` | pass | 5/5 | A negative arc fraction without -o: no start point, nothing written |
| `search/align-with-iterations` | pass | 6/6 | AlignGeometricCenters with iterations (ANTS-53: an empty .mat) |
| `search/align-with-iterations-quiet` | pass | 6/6 | AlignGeometricCenters with iterations, without -v (ANTS-53) |
| `search/align-with-iterations-2d-gc` | pass | 6/6 | 2D AlignCentersOfMass with iterations, GC (ANTS-53) |
| `search/align-with-iterations-no-start` | pass | 6/6 | AlignGeometricCenters with iterations and a negative arc (ANTS-53) |
| `search/gc-empty-mask-dense` | pass | 6/6 | GC over every voxel with an empty fixed mask: no valid point, the maximum value |
| `search/mi-empty-mask-dense` | pass | 6/6 | MI over every voxel with an empty fixed mask (ITK-19: every start point keeps the first value) |
| `messages/no-arguments` | both-error | 2/2 | No arguments: the short menu, exit 1 |
| `messages/h` | pass | 5/5 | -h: the short menu |
| `messages/help` | pass | 5/5 | --help: the long menu |
| `messages/help-0` | both-error | 2/2 | --help 0: nothing to do (ANTS-45) |
| `messages/h-0` | both-error | 2/2 | -h 0 (ANTS-45) |
| `messages/no-dimension` | both-error | 2/2 | No -d |
| `messages/dimension-4` | both-error | 2/2 | -d 4 |
| `messages/dimension-1` | both-error | 2/2 | -d 1 |
| `messages/no-metric` | both-error | 2/2 | No -m |
| `messages/no-metric-quiet` | both-error | 1/2 | No -m, without -v |
| `messages/one-image` | both-error | 2/2 | -m with one image |
| `messages/unknown-sampling` | both-error | 2/2 | An unknown sampling |
| `messages/unknown-transform` | both-error | 2/2 | An unknown transform |
| `messages/unknown-transform-quiet` | both-error | 2/2 | An unknown transform, without -v (ANTS-45) |
| `messages/no-transform` | both-error | 2/2 | No -t |
| `messages/unknown-metric` | both-error | 2/2 | An unknown metric with iterations |
| `messages/unknown-metric-align` | both-error | 2/2 | An unknown metric, AlignCentersOfMass with iterations |
| `messages/two-blobs` | both-error | 2/2 | -b 2: too few blobs |
| `messages/two-blobs-bracket` | both-error | 2/2 | -b [2,5] |
| `messages/unknown-flag` | both-error | 2/2 | An unknown flag |
| `messages/missing-fixed` | both-error | 3/3 | A fixed image that does not exist |
| `messages/missing-moving-geometry` | both-error | 3/3 | AlignGeometricCenters with a missing moving image |
| `messages/missing-fixed-mass` | both-error | 3/3 | AlignCentersOfMass with a missing fixed image |
| `messages/bad-iterations` | both-error | 3/3 | -c 1o: the parser's conversion fails |
| `messages/bad-bins` | both-error | 3/3 | 32 bins written 3x2 |
| `messages/unbalanced-bracket` | both-error | 3/3 | An unbalanced bracket |
| `messages/bad-seed` | both-error | 3/3 | --random-seed 1x |
| `messages/bad-env-seed` | both-error | 3/3 | ANTS_RANDOM_SEED=abc (std::stoi throws) |
| `messages/env-seed-range` | both-error | 3/3 | ANTS_RANDOM_SEED out of int's range |
| `messages/zero-moving-mass` | both-error | 3/3 | AlignCentersOfMass, a moving image of zeros |
| `messages/zero-moving-geometry` | pass | 6/6 | AlignGeometricCenters, a moving image of zeros: no moments needed |
| `messages/zero-principal-axes` | both-error | 3/3 | Principal axes of an image of zeros |
| `messages/zero-fixed-search` | both-error | 3/3 | A search from a fixed image of zeros |
| `messages/unknown-extension` | both-error | 3/3 | An output name no writer takes |
| `messages/no-directory` | both-error | 3/3 | An output in a directory that does not exist |
| `messages/align-iterations-unknown-extension` | both-error | 3/3 | ANTS-53 with an output name no writer takes |
| `messages/align-iterations-txt` | both-error | 3/3 | ANTS-53 into a text file: ITK's writer reads an empty list |
| `messages/align-iterations-h5` | both-error | 3/3 | ANTS-53 into an HDF5 file |
| `messages/empty-start-list` | both-error | 3/3 | A negative arc fraction: no start point, the best of an empty list |
| `messages/empty-grid` | both-error | 3/3 | A negative grid extent: no start point |
| `messages/empty-sample-mattes` | both-error | 3/3 | Regular sampling inside an empty mask: no sample point |
| `messages/empty-sample-mi` | both-error | 3/3 | The same with MI |
| `messages/empty-sample-gc` | both-error | 3/3 | The same with GC |
| `messages/empty-sample-align` | both-error | 3/3 | No sample point, AlignGeometricCenters with iterations |
| `messages/random-zero` | both-error | 3/3 | Random sampling of 0: no sample point |
| `messages/mattes-empty-mask-dense` | both-error | 3/3 | Mattes over every voxel with an empty fixed mask (ITK-20: the second start point crashes) |
| `messages/mattes-empty-moving-mask` | both-error | 3/3 | Mattes with an empty moving mask (ITK-20) |
| `messages/mattes-one-start-point` | both-error | 3/3 | Mattes with an empty mask and one start point: the best value of an empty list |
| `messages/percentage-zero` | both-error | 3/3 | Regular sampling of 0 (ANTS-06), one sample point: one intensity (ITK-20) |
| `messages/search-factor-negative` | expected-divergence | 2/2 | A negative search factor: ANTs' loop never ends |
| `messages/translation-step-zero` | expected-divergence | 2/2 | A translation step of 0: ANTs' loop never ends |
| `random/random-sampling` | expected-divergence | 3/3 | Random sampling: ANTs draws the voxels from the clock (ANTS-58) |
| `random/seed-zero` | expected-divergence | 3/3 | --random-seed 0: ANTs seeds Regular sampling from the clock (ANTS-58) |
| `real/ds000005-sub-01/brain-extraction` | pass | 8/8 | fMRIPrep's init_aff, brain extraction, with fMRIPrep's seed |
| `real/ds000005-sub-01/bold-mixedgamblestask-run-01` | pass | 8/8 | fMRIPrep's init_aff, BOLD reference (mixedgamblestask_run_01), with fMRIPrep's seed |
| `real/ds000005-sub-01/bold-mixedgamblestask-run-02` | pass | 7/7 | fMRIPrep's init_aff, BOLD reference (mixedgamblestask_run_02), with fMRIPrep's seed |
| `real/ds000005-sub-01/bold-mixedgamblestask-run-03` | pass | 7/7 | fMRIPrep's init_aff, BOLD reference (mixedgamblestask_run_03), with fMRIPrep's seed |
| `real/ds001600-sub-1/brain-extraction` | pass | 7/7 | fMRIPrep's init_aff, brain extraction, with fMRIPrep's seed |
| `real/ds005040-sub-002/brain-extraction` | pass | 7/7 | fMRIPrep's init_aff, brain extraction, with fMRIPrep's seed |
| `real/ds005040-sub-002/bold-clip-run-1` | pass | 7/7 | fMRIPrep's init_aff, BOLD reference (clip_run_1), with fMRIPrep's seed |
| `real/ds005454-sub-16/brain-extraction` | pass | 7/7 | fMRIPrep's init_aff, brain extraction, with fMRIPrep's seed |
| `real/ds005454-sub-16/bold-rest` | pass | 7/7 | fMRIPrep's init_aff, BOLD reference (rest), with fMRIPrep's seed |
| `real/ds005752-sub-ON01016/brain-extraction` | pass | 7/7 | fMRIPrep's init_aff, brain extraction, with fMRIPrep's seed |
| `blobs/shift-rigid` | pass | 9/9 | Rigid from 30 blobs of a phantom and its copy moved by an exact translation (ANTs' own test keeps the true pairs) |
| `blobs/shift-affine` | pass | 8/8 | Affine from 30 blobs of a phantom and its copy moved by an exact translation (ANTs' own test keeps the true pairs) |
| `blobs/shift-similarity` | pass | 8/8 | Similarity from 30 blobs of a phantom and its copy moved by an exact translation (ANTs' own test keeps the true pairs) |
| `blobs/rigid-rigid` | pass | 8/8 | Rigid from 30 blobs of a phantom and its copy moved by a rotation of 7 and 4 degrees and a shift |
| `blobs/rigid-affine` | pass | 8/8 | Affine from 30 blobs of a phantom and its copy moved by a rotation of 7 and 4 degrees and a shift |
| `blobs/affine-rigid` | pass | 8/8 | Rigid from 30 blobs of a phantom and its copy moved by a rotation of 5 degrees with scales 1.06 and 0.96 |
| `blobs/affine-affine` | both-error | 3/3 | Affine from 30 blobs of a phantom and its copy moved by a rotation of 5 degrees with scales 1.06 and 0.96 |
| `blobs/2d-shift-rigid` | pass | 8/8 | Rigid from 30 blobs of a phantom and its copy moved by a 2D translation |
| `blobs/2d-shift-affine` | pass | 8/8 | Affine from 30 blobs of a phantom and its copy moved by a 2D translation |
| `blobs/2d-rigid-rigid` | pass | 8/8 | Rigid from 30 blobs of a phantom and its copy moved by a 2D rotation of 9 degrees and a shift |
| `blobs/2d-rigid-affine` | pass | 8/8 | Affine from 30 blobs of a phantom and its copy moved by a 2D rotation of 9 degrees and a shift |
| `blobs/2d-rigid-similarity` | pass | 8/8 | Similarity from 30 blobs of a phantom and its copy moved by a 2D rotation of 9 degrees and a shift |
| `blobs/shift-rigid-match-10` | pass | 8/8 | -b [30,10]: 30 blobs extracted, at most 10 pairs |
| `blobs/shift-rigid-search` | pass | 8/8 | The blob alignment, then a search over rotations about the image axes |
| `blobs/rigid-affine-search-h5` | pass | 8/8 | Affine from the blobs of the rotated phantom, a search, an HDF5 output |
| `blobs/identical-rigid` | pass | 6/6 | An image against itself: every displacement is 0, so ANTs' ratios are 0/0 (ANTS-110) |
| `blobs/no-pairs-rigid` | pass | 6/6 | -b [5,0]: no pair to match; the rigid fit of no landmark (ANTS-114) |
| `blobs/no-pairs-affine` | both-error | 3/3 | -b [5,0] with Affine: too few landmarks, ITK's exception |
| `blobs/2d-no-pairs-rigid` | pass | 6/6 | 2D, no pair: ITK's warning (fewer than 2 landmarks), then NaN (ANTS-114) |
| `blobs/constant` | pass | 6/6 | Two constant images: every voxel is a maximum of value 0 |
| `blobs/noise-rigid` | pass | 6/6 | Noise against itself, 48^3: patches barely fit |
| `blobs/thin` | both-error | 3/3 | An axis of 3 voxels: the Laplacian of Gaussian refuses it (ITK's exception) |
| `blobs/missing-fixed` | both-error | 3/3 | A fixed image that does not exist: the blob detector's input is not set |
| `blobs/missing-moving` | both-error | 3/3 | A moving image that does not exist (after the fixed image's blobs) |
| `blobs/principal-axes-ignored` | pass | 8/8 | -p 1 with -b: the blobs decide the initial transform |
| `real/ds000005-sub-01/blobs-t1w-shift` | pass | 9/9 | fMRIPrep's T1w of ds000005 against its copy with the origin moved (6, -4, 3) mm |
| `real/ds000005-sub-01/blobs-t1w-rotated` | pass | 8/8 | fMRIPrep's T1w of ds000005 against its copy rotated by 8 degrees and shifted |
| `real/ds000005-sub-01/blobs-t1w-rotated-affine` | both-error | 3/3 | The same with Affine (ANTs: too few landmarks) |
| `real/ds000005-sub-01/blobs-mni-rigid` | pass | 6/6 | MNI152NLin2009cAsym (2 mm) against ds000005-sub-01's T1w, Rigid from 30 blobs |
| `real/ds001600-sub-1/blobs-mni-rigid` | pass | 6/6 | MNI152NLin2009cAsym (2 mm) against ds001600-sub-1's T1w, Rigid from 30 blobs |
| `real/ds005040-sub-002/blobs-mni-rigid` | pass | 6/6 | MNI152NLin2009cAsym (2 mm) against ds005040-sub-002's T1w, Rigid from 30 blobs |
| `real/ds005454-sub-16/blobs-mni-rigid` | pass | 6/6 | MNI152NLin2009cAsym (2 mm) against ds005454-sub-16's T1w, Rigid from 30 blobs |
| `real/ds005752-sub-ON01016/blobs-mni-rigid` | pass | 6/6 | MNI152NLin2009cAsym (2 mm) against ds005752-sub-ON01016's T1w, Rigid from 30 blobs |
| `real/ds005040-sub-002/blobs-oasis-affine` | both-error | 3/3 | OASIS30ANTs (1 mm) against ds005040's T1w, Affine from 100 blobs |
