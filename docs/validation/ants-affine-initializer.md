# Parity: antsAffineInitializer

**All cases agree.** 38 cases on the `standard` tier: 23 pass, 13 rejected by both, 2 expected divergences, 0 failures.

- **Validated:** `larmorx ants antsAffineInitializer` / `lx.ants.affine_initializer` (crates larmorx-ants, larmorx-registration)
- **Reference:** antsAffineInitializer from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0, the binary built by `scripts/build_ants_oracle.sh`, run with one thread
- **Test data:** larmorx-testdata `00b862132e9c`, tier `standard`
- **Generated:** 2026-10-17 on Linux x86_64, with `python -m larmorx_validation parity ants-affine-initializer --tier standard`

23 cases compared: 23 identical to the unmodified ANTs (exit status, standard output, standard error, every file), 0 identical once ANTs uses CORE-MATH's correctly rounded functions (glibc's are not always correctly rounded).

Thread band, 4 cases: ANTs at 2, 4, 8 threads differs from its own one-thread result by up to 9.76e-07 in a transform parameter (`search/defaults` 0, `search/principal-axes` 9.99e-16, `search/2d` 5.55e-17, `real/mni-2mm` 9.76e-07); larmorx gives the one-thread result at any thread count, within 0 of ANTs at one thread.

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | identical |
| Standard output and error | identical (larmorx's D13 notices aside) |
| Files | the transform byte for byte (HDF5: every dataset) |
| glibc's maths | where ANTs as built differs, ANTs with CORE-MATH preloaded must be identical |
| Aborts and crashes | everything ANTs printed before it stopped; larmorx exits 1 |
| Thread band | reported: ANTs at 2, 4, 8 threads against 1 |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| messages | 19 | 4 | 13 | 2 | 0 |
| real | 1 | 1 | 0 | 0 | 0 |
| search | 18 | 18 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `messages/search-factor-zero` | ANTs never finishes (class (a)); larmorx refuses |
| `messages/search-factor-text` | ANTs never finishes (class (a)); larmorx refuses |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `messages/two-arguments` | Two arguments: std::string of a null pointer (ANTS-57) | larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/three-arguments` | Three arguments (ANTS-57) | larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/dimension-text` | A dimension std::stoi cannot read | larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/dimension-range` | A dimension beyond int's range | larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/principal-axes-text` | A principal-axes flag std::stoi cannot read | larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/missing-fixed` | A fixed image that does not exist | larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/missing-moving` | A moving image that does not exist | larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/zero-fixed` | A fixed image of zeros | larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/zero-moving` | A moving image of zeros | larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/unknown-extension` | An output name no writer takes | larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/no-directory` | An output in a directory that does not exist | larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1 |
| `messages/search-factor-nan` | A search factor that is not a number: no start point | larmorx: antsAffineInitializer: no start point (a search factor or radian fraction that is not a number): ANTs reads the best parameters from an empty list and… |
| `messages/radian-fraction-nan` | A radian fraction that is not a number: no start point | larmorx: antsAffineInitializer: no start point (a search factor or radian fraction that is not a number): ANTs reads the best parameters from an empty list and… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (81ac401e129b) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

Inputs named `gen:<name>` are built by the suites (seeded phantoms).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `messages/no-arguments` | pass | 5/5 | No arguments: the usage, exit 0 |
| `messages/dimension-only` | pass | 5/5 | One argument: the usage, exit 0 |
| `messages/two-arguments` | both-error | 3/3 | Two arguments: std::string of a null pointer (ANTS-57) |
| `messages/three-arguments` | both-error | 3/3 | Three arguments (ANTS-57) |
| `messages/dimension-text` | both-error | 3/3 | A dimension std::stoi cannot read |
| `messages/dimension-range` | both-error | 3/3 | A dimension beyond int's range |
| `messages/dimension-5` | pass | 6/6 | Dimension 5 runs as 2D (ANTS-56) |
| `messages/dimension-1` | pass | 6/6 | Dimension 1 runs as 2D (ANTS-56) |
| `messages/principal-axes-text` | both-error | 3/3 | A principal-axes flag std::stoi cannot read |
| `messages/missing-fixed` | both-error | 3/3 | A fixed image that does not exist |
| `messages/missing-moving` | both-error | 3/3 | A moving image that does not exist |
| `messages/zero-fixed` | both-error | 3/3 | A fixed image of zeros |
| `messages/zero-moving` | both-error | 3/3 | A moving image of zeros |
| `messages/unknown-extension` | both-error | 3/3 | An output name no writer takes |
| `messages/no-directory` | both-error | 3/3 | An output in a directory that does not exist |
| `messages/search-factor-zero` | expected-divergence | 2/2 | A search factor of 0: ANTs' loop never ends |
| `messages/search-factor-text` | expected-divergence | 2/2 | A search factor atof reads as 0: ANTs' loop never ends |
| `messages/search-factor-nan` | both-error | 4/4 | A search factor that is not a number: no start point |
| `messages/radian-fraction-nan` | both-error | 4/4 | A radian fraction that is not a number: no start point |
| `search/defaults` | pass | 7/7 | 3D, every default (10 degrees, pi/4) |
| `search/niworkflows` | pass | 6/6 | niworkflows' arguments (15 0.1 0 10) |
| `search/principal-axes` | pass | 7/7 | From the principal axes |
| `search/principal-axes-mask` | pass | 6/6 | From the principal axes, a fixed mask |
| `search/missing-mask` | pass | 6/6 | A mask that cannot be read is no mask |
| `search/fraction-above-one` | pass | 6/6 | A radian fraction of 5 (clamped to 1) |
| `search/fraction-below-minimum` | pass | 6/6 | A radian fraction of 0.001 (clamped to 0.01) |
| `search/local-search-text` | pass | 6/6 | A local-search argument that is not read (ANTS-54) |
| `search/principal-axes-2` | pass | 6/6 | A principal-axes flag of 2 (true) |
| `search/dimension-text-prefix` | pass | 6/6 | A dimension of 3x (std::stoi reads 3) |
| `search/2d` | pass | 7/7 | 2D from the principal axes |
| `search/2d-defaults` | pass | 6/6 | 2D, every default |
| `search/4d` | pass | 6/6 | 4D: the initial alignment only |
| `search/4d-principal-axes` | pass | 6/6 | 4D from the principal axes |
| `search/oblique` | pass | 6/6 | Oblique grids, from the principal axes |
| `search/txt` | pass | 6/6 | A text transform file |
| `search/h5` | pass | 6/6 | An HDF5 transform file |
| `search/extra-arguments` | pass | 6/6 | Arguments after the mask are ignored |
| `real/mni-2mm` | pass | 7/7 | niworkflows' arguments on the 2 mm MNI template and fMRIPrep's preprocessed T1w (ds000005) |
