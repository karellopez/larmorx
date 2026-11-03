# Parity: FAST (clean-room original)

**1 case(s) fail.** 87 cases on the `standard` tier: 60 pass, 25 rejected by both, 1 expected divergences, 1 failures.

- **Validated:** `larmorx fsl fast` (larmorx 0.0.1, crate larmorx-fsl: the clean-room original written from specs/fast.md, Apache-2.0)
- **Reference:** fast from FSL 6.0.7.23 (fsl-fast4 2111.3), the conda build at `~/fsl-6.0.7.23`
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-03 on Linux x86_64, with `python -m larmorx_validation parity fsl-fast --tier standard`
- **Original programs:** 100 runs: 77 from the oracle cache, 23 run (11 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (where FSL aborts, the original exits 1) |
| Files written | the same names |
| Standard output and error | the same lines, numbers to 4 significant digits |
| Segmentations | per-class Dice >= 0.999, >= 99.9% of mask voxels identical |
| Partial volumes | mean \|d\| <= 0.001, p99.9 <= 0.02, totals within 0.1 % |
| Probabilities | mean \|d\| <= 0.001 |
| Bias field, restored image | relative difference <= 0.0001 at p99.9 |
| D13 fix pass | a defect named exactly when the default's outputs differ |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| difftest | 4 | 2 | 2 | 0 | 0 |
| err | 29 | 9 | 19 | 1 | 0 |
| fmriprep | 3 | 3 | 0 | 0 | 0 |
| multi | 15 | 13 | 1 | 0 | 1 |
| opt | 22 | 21 | 1 | 0 | 0 |
| priors | 13 | 11 | 2 | 0 | 0 |
| real | 1 | 1 | 0 | 0 | 0 |

## Failures

### `multi/s5`

-S 5 -B -b (T1w+T2w, T1w/T2w ratio, BOLD reference, T2w, T1w): 5 x 5 covariances (LAPACK's LU, inverse and solve beyond 4 x 4)

- **Segmentations (_seg, _seg_n, _pveseg, _mixeltype): Dice and identical voxels** (per file = seg_mixeltype.nii.gz: identical 0.95852, Dice 0.90695, threshold Dice >= 0.999, identical >= 99.9%, outside identical): seg_mixeltype.nii.gz: identical 0.95852, Dice 0.90695; seg_pveseg.nii.gz: identical 0.97892, Dice 0.97265; seg_seg.nii.gz: identical 0.98555, Dice 0.98201
- **Partial volumes (_pve_n): mean and p99.9 differences, totals** (per file = seg_pve_0.nii.gz: mean 0.0064, p99.9 0.81, total 0.00058, threshold mean <= 0.001, p99.9 <= 0.02, total within 0.1%): seg_pve_0.nii.gz: mean 0.0064, p99.9 0.81, total 0.00058; seg_pve_1.nii.gz: mean 0.021, p99.9 1, total 0.0023; seg_pve_2.nii.gz: mean 0.017, p99.9 0.91, total 0.0025
- **Bias field and restored image: relative difference (p99.9)** (per file = seg_bias_1.nii.gz: rel p99.9 0.041, threshold <= 0.0001): seg_bias_1.nii.gz: rel p99.9 0.041; seg_bias_2.nii.gz: rel p99.9 0.043; seg_bias_3.nii.gz: rel p99.9 0.11; seg_bias_4.nii.gz: rel p99.9 0.034; seg_bias_5.nii.gz: rel p99.9 0.014; seg_restore_1.nii.gz: rel p99.9 0.042

## Expected divergences

| Case | Reason |
|---|---|
| `err/S2-sizes-larger-first` | undefined behaviour of FSL's (FSL-104); refused |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `multi/s0-error` | -S 0: fewer than two channels (FAST's error) | fast exits 1; the same from the original. At least 2 channels required for Multi Channel Segmentation |
| `opt/offset` | negative values, 2nd percentile below 0 (the minimum subtracted) | fast exits 255; the same from the original. |
| `err/no-arguments` | no arguments: the usage | fast exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) FAST Copyright(c) 2004-2012, University of Oxford Usage: fast [options] file(s) Option… |
| `err/help` | -h: the usage | fast exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) FAST Copyright(c) 2004-2012, University of Oxford Usage: fast [options] file(s) Option… |
| `err/unknown-option` | an unknown option | fast exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) FAST Copyright(c) 2004-2012, University of Oxford Usage: fast [options] file(s) Option… |
| `err/missing-value` | an option without its value | fast exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) FAST Copyright(c) 2004-2012, University of Oxford Usage: fast [options] file(s) Option… |
| `err/bad-number` | a value that is not a number | fast exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) FAST Copyright(c) 2004-2012, University of Oxford Usage: fast [options] file(s) Option… |
| `err/H-negative` | -H below 0 | fast exits 1; the same from the original. ERROR: Segmentation smoothness must be positive. Exiting |
| `err/n0` | -n 0 (FAST aborts; FSL-99) | fast exits -6; the same from the original. terminate called after throwing an instance of 'std::invalid_argument' what(): Mixeltype exceeds allowed types |
| `err/n-negative` | -n -1 (FAST aborts; FSL-99) | fast exits -6; the same from the original. terminate called after throwing an instance of 'std::length_error' what(): vector::_M_fill_insert |
| `err/n-negative-multi` | -S 2 -n -2 (FAST aborts; FSL-99) | fast exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): Mat::init(): requested size is too large |
| `err/n-minus2` | -n -2 (FAST aborts; FSL-99) | fast exits -6; the same from the original. terminate called after throwing an instance of 'std::length_error' what(): vector::_M_default_append |
| `err/missing-input` | an input that does not exist | fast exits -6; the same from the original. Image Exception : #63 :: No image files match: nothere Image Exception : #22 :: Failed to read volume nothere.nii.gz… |
| `err/option-after-input` | an option after the input (its value read as the image) | fast exits -6; the same from the original. Image Exception : #63 :: No image files match: 2 Image Exception : #22 :: Failed to read volume 2 Error : No image f… |
| `err/constant` | a constant image (k-means cannot start) | fast exits 255; the same from the original. |
| `err/zeros` | an image of zeros (an empty mask) | fast exits 255; the same from the original. |
| `err/fsldir-unset` | FSLDIR unset (no priors asked for: FAST aborts; FSL-100) | fast exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): basic_string: construction from null is no… |
| `err/fsldir-unset-a` | FSLDIR unset with -a (FAST aborts; refused by default) | fast exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): basic_string: construction from null is no… |
| `err/fsldir-unset-A-a` | FSLDIR unset with -A and -a (FLIRT's command starts with (null): the shell fails and FAST crashes; FSL-102, FSL-101) | fast exits -11; the same from the original. sh: 1: Syntax error: word unexpected sh: 1: Syntax error: word unexpected sh: 1: Syntax error: word unexpected csf … |
| `err/S2-sizes` | -S 2 with images of different sizes, channel 1 the smaller (FAST aborts; FSL-104) | fast exits -6; the same from the original. Image Exception : #3 :: mask and vol have different sizes in calc_percentiles terminate called after throwing an ins… |
| `err/S4-two-images` | -S 4 with two images (an option word read as an image) | fast exits -6; the same from the original. Image Exception : #63 :: No image files match: 4 Image Exception : #22 :: Failed to read volume 4 Error : No image f… |
| `priors/bad-matrix` | -a naming a missing matrix (FLIRT fails; FAST crashes: FSL-101) | fast exits -11; the same from the original. Could not open matrix file nothere.mat matrix multiplication: incompatible matrix dimensions: 4x4 and 0x0 Could not… |
| `priors/name-blank` | -o with a blank: FLIRT's command split by the shell (FSL-102) | fast exits -11; the same from the original. Unrecognised option s Unrecognised option s Unrecognised option s csf prior image not transformed correctly! priors… |
| `difftest/fsl101-verbose-line` | FSL-101 reproduced with -v: 'Beginning prior-based initialisation' before FAST crashes | fast exits -11; the same from the original. ERROR:: Unrecognised value (ANALYZE) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NIF… |
| `difftest/singular-kmeans-line` | -v with several channels and a singular covariance: FSL aborts without the iteration's 'KMeans Iteration' line | fast exits -6; the same from the original. terminate called after throwing an instance of 'std::runtime_error' what(): inv(): matrix is singular |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (f0dd88b671b6) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The cases are the replica suite's (`fsl-fast-replica`); FSL's runs are served from the oracle cache under that suite's keys. The original is compared in `reproduce` mode (D13) and its default `fix` pass against the reproduced run.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds000005-sub-01-noreconall` | pass | 9/9 | fMRIPrep's segmentation call |
| `fmriprep/ds000228-sub-pixar008-noreconall` | pass | 9/9 | fMRIPrep's segmentation call |
| `fmriprep/ds005003-sub-RS001-noreconall` | pass | 9/9 | fMRIPrep's segmentation call |
| `real/fmriprep-call-half` | pass | 9/9 | fMRIPrep's call (-I 0 -N -p -g -S 1) on ds000005's T1w brain at half resolution |
| `multi/s2-outputs` | pass | 11/11 | -S 2 (T1w, then T2w: the T2w is channel 1) with every output (-B -b -g -p) |
| `multi/s2-type2-verbose` | pass | 8/8 | -S 2 -t 2 -v: the T2w as channel 1 with the classes in reversed order, every message |
| `multi/s3` | pass | 9/9 | -S 3 (BOLD reference, T2w, T1w) with the bias field's outputs (3 x 3 matrices) |
| `multi/s4-n2` | pass | 10/10 | -S 4 -n 2 (T1w/T2w ratio, BOLD reference, T2w, T1w: LAPACK's 4 x 4 paths), two classes |
| `multi/s2-n4` | pass | 8/8 | -S 2 -n 4: more than three classes (their own mixel types) |
| `multi/s2-nopve-default-name` | pass | 7/7 | -S 2 --nopve without -o (the outputs named after channel 1) |
| `multi/s2-nobias-outputs` | pass | 10/10 | -S 2 -N -B -b: no bias field, its outputs written |
| `multi/s2-manualseg` | pass | 8/8 | -S 2 -s: the initial class intensities from a file (three per channel) |
| `multi/s2-options` | pass | 8/8 | -S 2 with every numeric option (-I 2 -O 1 -W 5 -l 30 -f 0.05 -H 0.2 -R 0.5) |
| `multi/s2-quarter` | pass | 9/9 | -S 2 -B -b on the T1w and T2w at 4 mm |
| `multi/s5` | fail | 6/9 | -S 5 -B -b (T1w+T2w, T1w/T2w ratio, BOLD reference, T2w, T1w): 5 x 5 covariances (LAPACK's LU, inverse and solve beyond… |
| `multi/s8` | pass | 8/8 | -S 8 -n 2 -g: eight channels (the five, a square root, a product and a mix of the T1w and BOLD reference), 8 x 8 covari… |
| `multi/s0-error` | both-error | 5/5 | -S 0: fewer than two channels (FAST's error) |
| `opt/bias-outputs` | pass | 9/9 | the bias field (default) with -b -B -g |
| `opt/type2-verbose` | pass | 8/8 | -t 2 -v: reversed class order, every message |
| `opt/type3-n2` | pass | 8/8 | -t 3 -n 2 -g: PD-weighted, two classes |
| `opt/n4-probs` | pass | 10/10 | -n 4 -p: four classes, their mixel types |
| `opt/n1` | pass | 8/8 | -n 1 -v: one class (no mixel types) |
| `opt/nopve` | pass | 8/8 | --nopve -g -B |
| `opt/numeric` | pass | 9/9 | every numeric option (-I 2 -O 1 -W 5 -l 30 -f 0.05 -H 0.2 -R 0.5) with -b |
| `opt/H0-N` | pass | 8/8 | -H 0 -N: no MRF smoothness, no bias field |
| `opt/long-names` | pass | 8/8 | the long option names |
| `opt/manualseg` | pass | 8/8 | -s: the initial class intensities |
| `opt/manualseg-cycle` | pass | 8/8 | -s with the classes in a cyclic order (sorted at the end) |
| `opt/manualseg-two` | pass | 8/8 | -s with too few values (the percentiles instead) |
| `opt/manualseg-missing` | pass | 8/8 | -s naming no file (the percentiles instead) |
| `opt/2d` | pass | 9/9 | a 2D image (one slice) with -B -b |
| `opt/4d` | pass | 8/8 | a 4D image (its first volume, with newimage's warning) |
| `opt/offset` | both-error | 5/5 | negative values, 2nd percentile below 0 (the minimum subtracted) |
| `opt/negvox` | pass | 9/9 | a few negative voxels (set to 0) |
| `opt/nan` | pass | 11/11 | NaN voxels with -B -b -p |
| `opt/int16` | pass | 8/8 | a 16-bit integer image |
| `opt/big` | pass | 8/8 | values above 2^24 (the brightest voxels and the mask) |
| `opt/output-type` | pass | 8/8 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `opt/default-name` | pass | 7/7 | no -o: the outputs named after the input |
| `err/no-arguments` | both-error | 5/5 | no arguments: the usage |
| `err/help` | both-error | 5/5 | -h: the usage |
| `err/unknown-option` | both-error | 5/5 | an unknown option |
| `err/missing-value` | both-error | 5/5 | an option without its value |
| `err/bad-number` | both-error | 5/5 | a value that is not a number |
| `err/H-negative` | both-error | 5/5 | -H below 0 |
| `err/H-nan` | pass | 8/8 | -H nan (passes the check for a negative value) |
| `err/H-inf` | pass | 8/8 | -H inf |
| `err/l-zero` | pass | 8/8 | -l 0 (no smoothing extent) |
| `err/f-nan` | pass | 8/8 | -f nan |
| `err/R-nan` | pass | 8/8 | -R nan |
| `err/n0` | both-error | 6/6 | -n 0 (FAST aborts; FSL-99) |
| `err/n-negative` | both-error | 6/6 | -n -1 (FAST aborts; FSL-99) |
| `err/n-negative-multi` | both-error | 6/6 | -S 2 -n -2 (FAST aborts; FSL-99) |
| `err/n0-multi` | pass | 8/8 | -S 2 -n 0 (FSL-99) |
| `err/n-minus2` | both-error | 6/6 | -n -2 (FAST aborts; FSL-99) |
| `err/l-negative` | pass | 8/8 | -l -5 (FSL-99) |
| `err/missing-input` | both-error | 5/5 | an input that does not exist |
| `err/option-after-input` | both-error | 5/5 | an option after the input (its value read as the image) |
| `err/constant` | both-error | 5/5 | a constant image (k-means cannot start) |
| `err/zeros` | both-error | 5/5 | an image of zeros (an empty mask) |
| `err/fsldir-unset` | both-error | 6/6 | FSLDIR unset (no priors asked for: FAST aborts; FSL-100) |
| `err/fsldir-unset-a` | both-error | 5/5 | FSLDIR unset with -a (FAST aborts; refused by default) |
| `err/fsldir-unset-A-a` | both-error | 6/6 | FSLDIR unset with -A and -a (FLIRT's command starts with (null): the shell fails and FAST crashes; FSL-102, FSL-101) |
| `err/S2-sizes` | both-error | 6/6 | -S 2 with images of different sizes, channel 1 the smaller (FAST aborts; FSL-104) |
| `err/S2-sizes-larger-first` | expected-divergence | 2/2 | -S 2 with images of different sizes, channel 1 the larger (refused; FSL-104) |
| `err/S4-two-images` | both-error | 5/5 | -S 4 with two images (an option word read as an image) |
| `err/fsldir-unset-A` | pass | 8/8 | FSLDIR unset with -A (no FSLDIR needed) |
| `err/S1-two-images` | pass | 8/8 | -S 1 with two images (the last one segmented) |
| `priors/a-verbose` | pass | 8/8 | -a with FSL's standard priors, -v |
| `priors/a-P-probs` | pass | 10/10 | -a -P: the priors throughout, -p -g |
| `priors/a-n2` | pass | 8/8 | -a -n 2: two classes |
| `priors/a-P-n2` | pass | 8/8 | -a -P -n 2 |
| `priors/a-n4` | pass | 8/8 | -a -n 4 (priors serve 2 or 3 classes only) |
| `priors/A-a` | pass | 8/8 | -A (ds000005's own maps in the template's space) with -a (the identity) and -P |
| `priors/A-alone` | pass | 8/8 | -A without -a (no priors) |
| `priors/P-alone` | pass | 8/8 | -P without -a (no priors) |
| `priors/missing-prior` | pass | 8/8 | -A naming a missing prior with -a |
| `priors/bad-matrix` | both-error | 6/6 | -a naming a missing matrix (FLIRT fails; FAST crashes: FSL-101) |
| `priors/name-blank` | both-error | 6/6 | -o with a blank: FLIRT's command split by the shell (FSL-102) |
| `priors/multi-a-P` | pass | 8/8 | -S 2 -a -P: the multi-channel segmentation with the priors |
| `priors/multi-a-n2` | pass | 8/8 | -S 2 -a -n 2 |
| `multi/s2-opposite-contrasts` | pass | 9/9 | -S 2 -B -b on a phantom of opposite contrasts: classes paired across the channels start empty; every voxel ends in clas… |
| `multi/s2-manualseg-cycle` | pass | 8/8 | -S 2 -s (T1w/T2w ratio, T1w) with the classes in a cyclic order (FSL-98) |
| `difftest/bias-nan-outside-mask` | pass | 9/9 | a class of NaN variance makes the bias field's mean NaN: FSL's field, _restore and _bias are NaN outside the mask too (… |
| `difftest/fsl101-verbose-line` | both-error | 5/5 | FSL-101 reproduced with -v: 'Beginning prior-based initialisation' before FAST crashes |
| `difftest/singular-kmeans-line` | both-error | 5/5 | -v with several channels and a singular covariance: FSL aborts without the iteration's 'KMeans Iteration' line |
| `difftest/zstd-outputs` | pass | 8/8 | FSLOUTPUTTYPE=NIFTI_ZST: the suite reads FSL's and the original's zstd images (it raised before) |
