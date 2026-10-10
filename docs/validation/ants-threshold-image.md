# Parity: ThresholdImage

**All cases agree.** 41 cases on the `standard` tier: 35 pass, 5 rejected by both, 1 expected divergences, 0 failures.

- **Validated:** `larmorx ants ThresholdImage` / `lx.ants.threshold_image`, `lx.ants.otsu_threshold` (crates larmorx-ants, larmorx-image)
- **Reference:** ThresholdImage from ANTs v2.6.5 (fdce4d2f84) on ITK v5.4.5 (f51594ad88), g++ (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0, the binary built by `scripts/build_ants_oracle.sh`
- **Test data:** larmorx-testdata `5434909ddf09`, tier `standard`
- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation parity ants-threshold-image --tier standard`

**Bit-identical: 35 of 35 passing cases** produce exactly the values ANTs writes; 35 of 35 also write exactly its header bytes.

| Category | Cases | Pass | Both error | Expected divergence | Bit-identical | Most values differing |
|---|---|---|---|---|---|---|
| divergence | 1 | 0 | 0 | 1 | 0 | – |
| errors | 5 | 0 | 5 | 0 | 0 | – |
| otsu | 18 | 18 | 0 | 0 | 18 | 0 |
| range | 17 | 17 | 0 | 0 | 17 | 0 |

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
| divergence | 1 | 0 | 0 | 1 | 0 |
| errors | 5 | 0 | 5 | 0 | 0 |
| otsu | 18 | 18 | 0 | 0 | 0 |
| range | 17 | 17 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `divergence/kmeans` | ThresholdImage Kmeans is not ported yet (fMRIPrep does not use it). |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/lower-above-upper` | lower > upper: ITK throws (ANTs aborts) | ANTs killed by signal 6: ITK ERROR: BinaryThresholdImageFilter(0x5ff15c17c120): Lower threshold cannot be greater than upper threshold.; larmorx exit 1: larmor… |
| `errors/missing-input` | an input that does not exist | ANTs killed by signal 6: ITK ERROR: BinaryThresholdImageFilter(0x59a266a9e900): Input Primary is required but not set.; larmorx exit 1: larmorx: ThresholdImage… |
| `errors/otsu-count-text` | Otsu 'two': std::stoi throws | ANTs killed by signal 6: what(): stoi; larmorx exit 1: larmorx: ThresholdImage: 'two' is not a number (std::stoi throws) (ANTs crashes here) |
| `errors/dimension-5` | dimension 5: 'Unsupported dimension' | ANTs exit 1: Unsupported dimension; larmorx exit 1: Unsupported dimension |
| `errors/too-few-arguments` | one argument: the usage, exit 1 | ANTs exit 1: Inclusive thresholds; larmorx exit 1: Inclusive thresholds |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (17a3d2c14f30-dirty) |
| larmorx-testdata | 5434909ddf09 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs get the same arguments and files; ANTs runs as its own binary and larmorx in-process through its Python console entry point, both with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4`. Outputs are read with nibabel (values as stored, before `scl_slope`) and the headers from their raw bytes. Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs`.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `range/probseg-0.5-1` | pass | 8/8 | 0.5 1 on a probability map with exact 0.5 and 1 |
| `range/head-50-120` | pass | 8/8 | 50 120 on the phantom |
| `range/inside-outside` | pass | 8/8 | inside 7, outside -2 |
| `range/sixth-argument-as-mask` | pass | 8/8 | inside 255: read first as a mask image ('file 255 does not exist') |
| `range/equal-bounds` | pass | 8/8 | lower = upper = 0.5 |
| `range/nan-lower` | pass | 8/8 | lower 'nan' (atof gives NaN): every voxel outside |
| `range/inf-upper` | pass | 8/8 | upper 'inf' |
| `range/hex` | pass | 8/8 | hexadecimal bounds (atof reads 0x40 as 64) |
| `range/not-a-number` | pass | 8/8 | lower 'abc' (atof gives 0) |
| `range/lowercase-otsu` | pass | 8/8 | 'otsu' is not 'Otsu': a range threshold from atof('otsu') = 0 |
| `range/float32-bounds` | pass | 8/8 | bounds rounded to float (0.1 is not exact) |
| `range/int16-scaled` | pass | 8/8 | scaled int16 input |
| `range/outliers` | pass | 8/8 | NaN and -inf voxels are outside |
| `range/dim2` | pass | 8/8 | a 2D image |
| `range/dim4` | pass | 8/8 | a 4D series |
| `range/real-GM-probseg` | pass | 8/8 | fMRIPrep's binarisation (0.5 1) of its GM probability map |
| `range/real-MNI-brain-probseg` | pass | 8/8 | the MNI brain probability map at 0.5 1 (fMRIPrep's thr_brainmask) |
| `otsu/head-1` | pass | 8/8 | Otsu with 1 threshold(s) on the phantom |
| `otsu/head-2` | pass | 8/8 | Otsu with 2 threshold(s) on the phantom |
| `otsu/head-3` | pass | 8/8 | Otsu with 3 threshold(s) on the phantom |
| `otsu/head-4` | pass | 8/8 | Otsu with 4 threshold(s) on the phantom |
| `otsu/head-1-mask` | pass | 8/8 | Otsu 1 inside a brain mask |
| `otsu/head-3-mask` | pass | 8/8 | Otsu 3 inside a brain mask |
| `otsu/fractional-mask` | pass | 8/8 | a float mask read as int: only values in [1, 2) are label 1 |
| `otsu/tissue-labels-mask` | pass | 8/8 | a three-label mask: only label 1 is the region |
| `otsu/missing-mask` | pass | 8/8 | a mask that does not exist: message, then Otsu without a mask |
| `otsu/int16-scaled` | pass | 8/8 | scaled int16 |
| `otsu/outliers` | pass | 8/8 | NaN and -inf voxels |
| `otsu/constant` | pass | 8/8 | a constant image |
| `otsu/count-with-text` | pass | 8/8 | number of thresholds '2abc' (std::stoi reads 2) |
| `otsu/dim2` | pass | 8/8 | a 2D image |
| `otsu/dim4` | pass | 8/8 | a 4D series |
| `otsu/real-boldref` | pass | 8/8 | Otsu 2 on a boldref |
| `otsu/real-boldref-mask` | pass | 8/8 | Otsu 1 on a boldref inside its mask |
| `otsu/real-T1w-3` | pass | 8/8 | Otsu 3 on fMRIPrep's T1w inside its brain mask |
| `errors/lower-above-upper` | both-error | – | lower > upper: ITK throws (ANTs aborts) |
| `errors/missing-input` | both-error | – | an input that does not exist |
| `errors/otsu-count-text` | both-error | – | Otsu 'two': std::stoi throws |
| `errors/dimension-5` | both-error | – | dimension 5: 'Unsupported dimension' |
| `errors/too-few-arguments` | both-error | – | one argument: the usage, exit 1 |
| `divergence/kmeans` | expected-divergence | 1/1 | Kmeans: ITK's k-d tree k-means (not supported by larmorx yet) |
