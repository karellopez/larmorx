# Parity: SmoothImage

**All cases agree.** 38 cases on the `standard` tier: 31 pass, 7 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx ants SmoothImage` / `lx.ants.smooth_image` (crates larmorx-ants, larmorx-image)
- **Reference:** SmoothImage from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), g++ (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `28167dea9be5`, tier `standard`
- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation parity ants-smooth-image --tier standard`

**Bit-identical: 30 of 30 passing cases** produce exactly the values ANTs writes; 30 of 30 also write exactly its header bytes.

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| dimensions | 6 | 6 | 0 | 0 | 6 | 0 |
| errors | 8 | 1 | 7 | 0 | 0 | – |
| gaussian | 14 | 14 | 0 | 0 | 14 | 0 |
| median | 6 | 6 | 0 | 0 | 6 | 0 |
| real | 4 | 4 | 0 | 0 | 4 | 0 |

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both produce their outputs, or neither does |
| Header | every field of the 348-byte header identical |
| Data type and shape | identical |
| Affine (as nibabel reads it) | max \|diff\| ≤ 1e-06 mm |
| Values | bit-identical, unless the case declares a tolerance in ulps with its cause |
| stdout (where compared) | identical text |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| dimensions | 6 | 6 | 0 | 0 | 0 |
| errors | 8 | 1 | 7 | 0 | 0 |
| gaussian | 14 | 14 | 0 | 0 | 0 |
| median | 6 | 6 | 0 | 0 | 0 |
| real | 4 | 4 | 0 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/too-few-arguments` | two arguments: the usage, exit 1 | ANTs exit 1: A separate sigma can be specified for each dimension, e.g., 1.5x1x2; larmorx exit 1: A separate sigma can be specified for each dimension, e.g., 1… |
| `errors/dimension-5` | dimension 5: 'Unsupported dimension' | ANTs exit 1: Unsupported dimension; larmorx exit 1: Unsupported dimension |
| `errors/sigma-0` | sigma 0: 'Sigma must be greater than zero' (ITK throws, ANTs aborts) | ANTs killed by signal 6: ITK ERROR: RecursiveGaussianImageFilter(0x5e7f42d74510): Sigma must be greater than zero.; larmorx exit 1: larmorx: SmoothImage: Sigma… |
| `errors/thin` | Gaussian smoothing of an axis of 3 voxels (ITK throws, ANTs aborts) | ANTs killed by signal 6: ITK ERROR: SmoothingRecursiveGaussianImageFilter(0x57b04c35ddf0): The number of pixels along dimension 2 is less than 4. This filter r… |
| `errors/missing-input` | an input that does not exist (ANTs crashes) | ANTs killed by signal 11: file <tmp>/none.nii.gz does not exist .; larmorx exit 1: larmorx: SmoothImage: cannot read image '<tmp>/none.nii.gz' (ANTs crashes he… |
| `errors/no-output` | no output name: ANTs smooths, then std::string(nullptr) throws | ANTs killed by signal 6: what(): basic_string::_M_construct null not valid; larmorx exit 1: larmorx: SmoothImage: no output name (std::string(nullptr) throws) … |
| `errors/bad-flag` | a fifth argument that is not a number: std::stoi throws | ANTs killed by signal 6: what(): stoi; larmorx exit 1: larmorx: SmoothImage: 'yes' is not a number (ANTs aborts: std::stoi throws) |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (54d3282e3bf5-dirty) |
| larmorx-testdata | 28167dea9be5 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs get the same arguments and files; ANTs runs as its own binary and larmorx in-process through its Python console entry point, both with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4`. Outputs are read with nibabel (values as stored, before `scl_slope`) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs`. Real images come from `larmorx-testdata` (OpenNeuro ds000005 and its fMRIPrep derivatives).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `gaussian/voxels-1.5` | pass | 8/8 | sigma 1.5 voxels on the oblique anisotropic phantom |
| `gaussian/voxels-vector` | pass | 8/8 | one sigma per axis, in voxels (1x2x0.5) |
| `gaussian/physical-2` | pass | 8/8 | sigma 2 mm (fifth argument 1) |
| `gaussian/physical-vector` | pass | 8/8 | sigmas 1x2x3 mm |
| `gaussian/explicit-zeros` | pass | 8/8 | flags 0 0: Gaussian, sigma in voxels |
| `gaussian/large-sigma` | pass | 8/8 | sigma 8 voxels (wider than the image's z axis) |
| `gaussian/small-sigma` | pass | 8/8 | sigma 0.3 voxels |
| `gaussian/wrong-length` | pass | 8/8 | two sigmas in 3D: 'Incorrect sigma vector size', then the default sigma (1 mm) |
| `gaussian/int16-scaled` | pass | 8/8 | scaled int16 input |
| `gaussian/outliers` | pass | 8/8 | hot and negative voxels (NaN and -inf read as 0) |
| `gaussian/constant` | pass | 8/8 | a constant image |
| `gaussian/tiny` | pass | 8/8 | 4 x 4 x 4, the smallest image ITK accepts |
| `gaussian/negative-pixdim` | pass | 8/8 | a qform-only image with a negative pixdim |
| `gaussian/las-sform-only` | pass | 8/8 | an sform-only LAS image |
| `median/radius-1` | pass | 8/8 | median filter, radius 1 (27 voxels) |
| `median/radius-vector` | pass | 8/8 | median filter, radii 2x1x1 |
| `median/radius-0` | pass | 8/8 | median filter, radius 0: unchanged |
| `median/radius-truncated` | pass | 8/8 | radius 1.7 truncates to 1 |
| `median/thin` | pass | 8/8 | the median filter on an axis of 3 voxels |
| `median/outliers` | pass | 8/8 | median of hot and negative voxels |
| `dimensions/dim2` | pass | 8/8 | SmoothImage 2 on a 2D image |
| `dimensions/dim2-median` | pass | 8/8 | a 2D median filter |
| `dimensions/dim4` | pass | 8/8 | SmoothImage 4: smoothed along time too (TR 2 s) |
| `dimensions/dim4-physical` | pass | 8/8 | SmoothImage 4 with sigmas in mm and s (1x1x1x4) |
| `dimensions/dim4-median` | pass | 8/8 | a 4D median filter |
| `dimensions/dim3-of-4d` | pass | 8/8 | SmoothImage 3 on a 4D file: its first volume, with an identity direction |
| `real/T1w-raw` | pass | 8/8 | the raw T1w of ds000005, sigma 1 voxel |
| `real/boldref` | pass | 8/8 | the boldref, sigma 2 mm |
| `real/T1w-preproc-median` | pass | 8/8 | fMRIPrep's preprocessed T1w, median radius 1 |
| `real/bold-4d` | pass | 8/8 | fMRIPrep's preprocessed BOLD series, SmoothImage 4, sigma 1 voxel |
| `errors/help` | pass | 1/1 | --help: the usage, exit 0 |
| `errors/too-few-arguments` | both-error | – | two arguments: the usage, exit 1 |
| `errors/dimension-5` | both-error | – | dimension 5: 'Unsupported dimension' |
| `errors/sigma-0` | both-error | – | sigma 0: 'Sigma must be greater than zero' (ITK throws, ANTs aborts) |
| `errors/thin` | both-error | – | Gaussian smoothing of an axis of 3 voxels (ITK throws, ANTs aborts) |
| `errors/missing-input` | both-error | – | an input that does not exist (ANTs crashes) |
| `errors/no-output` | both-error | – | no output name: ANTs smooths, then std::string(nullptr) throws |
| `errors/bad-flag` | both-error | – | a fifth argument that is not a number: std::stoi throws |
