# Parity: FAST replica (bit-exact, FSL Licence)

**All cases agree.** 87 cases on the `standard` tier: 59 pass, 27 rejected by both, 1 expected divergences, 0 failures.

- **Validated:** `larmorx-nc fsl fast` (crate larmorx-nc-fast, translated from FSL 6.0.7.23's source)
- **Reference:** fast from FSL 6.0.7.23 (fsl-fast4 2111.3, fsl-newimage 2601.0, fsl-miscmaths 2412.6, fsl-utils 2412.6; the conda build at `~/fsl-6.0.7.23`, as built: glibc 2.35's maths and random numbers)
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-03 on Linux x86_64, with `python -m larmorx_validation parity fsl-fast-replica --tier standard`
- **Original programs:** 87 runs: 65 from the oracle cache, 22 run (10 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same |
| Standard output and error | identical (the run directory and FSL's program path replaced; the replica's `larmorx-nc:` lines left out) |
| Files written | the same names |
| Every file | bit-identical (images decompressed; `descrip` left out) |
| D13 fix pass | a defect named exactly when the default's outputs differ |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| difftest | 3 | 0 | 3 | 0 | 0 |
| err | 29 | 9 | 19 | 1 | 0 |
| fmriprep | 3 | 3 | 0 | 0 | 0 |
| multi | 15 | 14 | 1 | 0 | 0 |
| opt | 23 | 21 | 2 | 0 | 0 |
| priors | 13 | 11 | 2 | 0 | 0 |
| real | 1 | 1 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `err/S2-sizes-larger-first` | undefined behaviour of FSL's (FSL-104); refused. FSL exits -11: |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `multi/s0-error` | -S 0: fewer than two channels (FAST's error) | fast exits 1; the same from the replica. At least 2 channels required for Multi Channel Segmentation |
| `opt/offset` | negative values, 2nd percentile below 0 (the minimum subtracted) | fast exits 255; the same from the replica. |
| `opt/nan-first` | the negative voxels' image with a NaN first voxel (FSL-203: FSL's extrema NaN, the negative voxels kept and the mask empty: the k-means exception, exit 255; the default segments) | fast exits 255; the same from the replica. |
| `err/no-arguments` | no arguments: the usage | fast exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) FAST Copyright(c) 2004-2012, University of Oxford Usage: fast [options] file(s) Optiona… |
| `err/help` | -h: the usage | fast exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) FAST Copyright(c) 2004-2012, University of Oxford Usage: fast [options] file(s) Optiona… |
| `err/unknown-option` | an unknown option | fast exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) FAST Copyright(c) 2004-2012, University of Oxford Usage: fast [options] file(s) Optiona… |
| `err/missing-value` | an option without its value | fast exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) FAST Copyright(c) 2004-2012, University of Oxford Usage: fast [options] file(s) Optiona… |
| `err/bad-number` | a value that is not a number | fast exits 1; the same from the replica. Part of FSL (ID: 2412.6-dirty) FAST Copyright(c) 2004-2012, University of Oxford Usage: fast [options] file(s) Optiona… |
| `err/H-negative` | -H below 0 | fast exits 1; the same from the replica. ERROR: Segmentation smoothness must be positive. Exiting |
| `err/n0` | -n 0 (FAST aborts; FSL-99) | fast exits -6; the same from the replica. terminate called after throwing an instance of 'std::invalid_argument' what(): Mixeltype exceeds allowed types |
| `err/n-negative` | -n -1 (FAST aborts; FSL-99) | fast exits -6; the same from the replica. terminate called after throwing an instance of 'std::length_error' what(): vector::_M_fill_insert |
| `err/n-negative-multi` | -S 2 -n -2 (FAST aborts; FSL-99) | fast exits -6; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): Mat::init(): requested size is too large |
| `err/n-minus2` | -n -2 (FAST aborts; FSL-99) | fast exits -6; the same from the replica. terminate called after throwing an instance of 'std::length_error' what(): vector::_M_default_append |
| `err/missing-input` | an input that does not exist | fast exits -6; the same from the replica. Image Exception : #63 :: No image files match: nothere Image Exception : #22 :: Failed to read volume nothere.nii.gz … |
| `err/option-after-input` | an option after the input (its value read as the image) | fast exits -6; the same from the replica. Image Exception : #63 :: No image files match: 2 Image Exception : #22 :: Failed to read volume 2 Error : No image fi… |
| `err/constant` | a constant image (k-means cannot start) | fast exits 255; the same from the replica. |
| `err/zeros` | an image of zeros (an empty mask) | fast exits 255; the same from the replica. |
| `err/fsldir-unset` | FSLDIR unset (no priors asked for: FAST aborts; FSL-100) | fast exits -6; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): basic_string: construction from null is not… |
| `err/fsldir-unset-a` | FSLDIR unset with -a (FAST aborts; refused by default) | fast exits -6; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): basic_string: construction from null is not… |
| `err/fsldir-unset-A-a` | FSLDIR unset with -A and -a (FLIRT's command starts with (null): the shell fails and FAST crashes; FSL-102, FSL-101) | fast exits -11; the same from the replica. sh: 1: Syntax error: word unexpected sh: 1: Syntax error: word unexpected sh: 1: Syntax error: word unexpected csf p… |
| `err/S2-sizes` | -S 2 with images of different sizes, channel 1 the smaller (FAST aborts; FSL-104) | fast exits -6; the same from the replica. Image Exception : #3 :: mask and vol have different sizes in calc_percentiles terminate called after throwing an inst… |
| `err/S4-two-images` | -S 4 with two images (an option word read as an image) | fast exits -6; the same from the replica. Image Exception : #63 :: No image files match: 4 Image Exception : #22 :: Failed to read volume 4 Error : No image fi… |
| `priors/bad-matrix` | -a naming a missing matrix (FLIRT fails; FAST crashes: FSL-101) | fast exits -11; the same from the replica. Could not open matrix file nothere.mat matrix multiplication: incompatible matrix dimensions: 4x4 and 0x0 Could not … |
| `priors/name-blank` | -o with a blank: FLIRT's command split by the shell (FSL-102) | fast exits -11; the same from the replica. Unrecognised option s Unrecognised option s Unrecognised option s csf prior image not transformed correctly! priors … |
| `difftest/fsl101-empty-mask-runs-on` | FSL-101 (FLIRT cannot write the registered priors: an output type FSL 6.0.7.23 refuses) on an image whose mask is empty: FAST runs on, as no voxel reads the priors it never allocated; reproduce now does the same | fast exits 1; the same from the replica. ERROR:: Unrecognised value (ANALYZE) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NIFTI2… |
| `difftest/fsl101-multi-exception` | FSL-101 with three channels whose first voxel is in the mask: FAST's bounds-checked access to the unallocated priors throws 'Out of Bounds (time index)' and aborts; reproduce now throws the same | fast exits -6; the same from the replica. ERROR:: Unrecognised value (ANALYZE) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NIFTI… |
| `difftest/fsl101-verbose-before-crash` | FSL-101 with -v: FAST prints 'Beginning prior-based initialisation' before it crashes on the priors it never allocated; reproduce now prints it before its refusal | fast exits -11; the same from the replica. ERROR:: Unrecognised value (ANALYZE) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NIFT… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (53796ce0a606) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The replica runs as a separate program, the FSL replica package's (`larmorx-nc`), compared with FSL in `reproduce` mode (D13); the default `fix` pass is checked against the reproduced run. fMRIPrep's own outputs come from its container's FSL and are reported only. The derived inputs are made once from ds000005's fMRIPrep run (`oracles/inputs/fsl-fast-replica/`).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds000005-sub-01-noreconall` | pass | 7/7 | fMRIPrep's segmentation call |
| `fmriprep/ds000228-sub-pixar008-noreconall` | pass | 7/7 | fMRIPrep's segmentation call |
| `fmriprep/ds005003-sub-RS001-noreconall` | pass | 7/7 | fMRIPrep's segmentation call |
| `real/fmriprep-call-half` | pass | 6/6 | fMRIPrep's call (-I 0 -N -p -g -S 1) on ds000005's T1w brain at half resolution |
| `multi/s2-outputs` | pass | 7/7 | -S 2 (T1w, then T2w: the T2w is channel 1) with every output (-B -b -g -p) |
| `multi/s2-type2-verbose` | pass | 6/6 | -S 2 -t 2 -v: the T2w as channel 1 with the classes in reversed order, every message |
| `multi/s3` | pass | 6/6 | -S 3 (BOLD reference, T2w, T1w) with the bias field's outputs (3 x 3 matrices) |
| `multi/s4-n2` | pass | 7/7 | -S 4 -n 2 (T1w/T2w ratio, BOLD reference, T2w, T1w: LAPACK's 4 x 4 paths), two classes |
| `multi/s2-n4` | pass | 6/6 | -S 2 -n 4: more than three classes (their own mixel types) |
| `multi/s2-nopve-default-name` | pass | 6/6 | -S 2 --nopve without -o (the outputs named after channel 1) |
| `multi/s2-nobias-outputs` | pass | 7/7 | -S 2 -N -B -b: no bias field, its outputs written |
| `multi/s2-manualseg` | pass | 6/6 | -S 2 -s: the initial class intensities from a file (three per channel) |
| `multi/s2-options` | pass | 6/6 | -S 2 with every numeric option (-I 2 -O 1 -W 5 -l 30 -f 0.05 -H 0.2 -R 0.5) |
| `multi/s2-quarter` | pass | 6/6 | -S 2 -B -b on the T1w and T2w at 4 mm |
| `multi/s5` | pass | 6/6 | -S 5 -B -b (T1w+T2w, T1w/T2w ratio, BOLD reference, T2w, T1w): 5 x 5 covariances (LAPACK's LU, inverse and solve beyond… |
| `multi/s8` | pass | 6/6 | -S 8 -n 2 -g: eight channels (the five, a square root, a product and a mix of the T1w and BOLD reference), 8 x 8 covari… |
| `multi/s0-error` | both-error | 5/5 | -S 0: fewer than two channels (FAST's error) |
| `opt/bias-outputs` | pass | 6/6 | the bias field (default) with -b -B -g |
| `opt/type2-verbose` | pass | 6/6 | -t 2 -v: reversed class order, every message |
| `opt/type3-n2` | pass | 6/6 | -t 3 -n 2 -g: PD-weighted, two classes |
| `opt/n4-probs` | pass | 7/7 | -n 4 -p: four classes, their mixel types |
| `opt/n1` | pass | 6/6 | -n 1 -v: one class (no mixel types) |
| `opt/nopve` | pass | 6/6 | --nopve -g -B |
| `opt/numeric` | pass | 6/6 | every numeric option (-I 2 -O 1 -W 5 -l 30 -f 0.05 -H 0.2 -R 0.5) with -b |
| `opt/H0-N` | pass | 6/6 | -H 0 -N: no MRF smoothness, no bias field |
| `opt/long-names` | pass | 6/6 | the long option names |
| `opt/manualseg` | pass | 6/6 | -s: the initial class intensities |
| `opt/manualseg-cycle` | pass | 6/6 | -s with the classes in a cyclic order (sorted at the end) |
| `opt/manualseg-two` | pass | 6/6 | -s with too few values (the percentiles instead) |
| `opt/manualseg-missing` | pass | 6/6 | -s naming no file (the percentiles instead) |
| `opt/2d` | pass | 6/6 | a 2D image (one slice) with -B -b |
| `opt/4d` | pass | 6/6 | a 4D image (its first volume, with newimage's warning) |
| `opt/offset` | both-error | 5/5 | negative values, 2nd percentile below 0 (the minimum subtracted) |
| `opt/negvox` | pass | 6/6 | a few negative voxels (set to 0) |
| `opt/nan-first` | both-error | 6/6 | the negative voxels' image with a NaN first voxel (FSL-203: FSL's extrema NaN, the negative voxels kept and the mask em… |
| `opt/nan` | pass | 7/7 | NaN voxels with -B -b -p |
| `opt/int16` | pass | 6/6 | a 16-bit integer image |
| `opt/big` | pass | 6/6 | values above 2^24 (the brightest voxels and the mask) |
| `opt/output-type` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `opt/default-name` | pass | 6/6 | no -o: the outputs named after the input |
| `err/no-arguments` | both-error | 5/5 | no arguments: the usage |
| `err/help` | both-error | 5/5 | -h: the usage |
| `err/unknown-option` | both-error | 5/5 | an unknown option |
| `err/missing-value` | both-error | 5/5 | an option without its value |
| `err/bad-number` | both-error | 5/5 | a value that is not a number |
| `err/H-negative` | both-error | 5/5 | -H below 0 |
| `err/H-nan` | pass | 6/6 | -H nan (passes the check for a negative value) |
| `err/H-inf` | pass | 6/6 | -H inf |
| `err/l-zero` | pass | 6/6 | -l 0 (no smoothing extent) |
| `err/f-nan` | pass | 6/6 | -f nan |
| `err/R-nan` | pass | 6/6 | -R nan |
| `err/n0` | both-error | 6/6 | -n 0 (FAST aborts; FSL-99) |
| `err/n-negative` | both-error | 6/6 | -n -1 (FAST aborts; FSL-99) |
| `err/n-negative-multi` | both-error | 6/6 | -S 2 -n -2 (FAST aborts; FSL-99) |
| `err/n0-multi` | pass | 7/7 | -S 2 -n 0 (FSL-99) |
| `err/n-minus2` | both-error | 6/6 | -n -2 (FAST aborts; FSL-99) |
| `err/l-negative` | pass | 6/6 | -l -5 (FSL-99) |
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
| `err/fsldir-unset-A` | pass | 6/6 | FSLDIR unset with -A (no FSLDIR needed) |
| `err/S1-two-images` | pass | 6/6 | -S 1 with two images (the last one segmented) |
| `priors/a-verbose` | pass | 6/6 | -a with FSL's standard priors, -v |
| `priors/a-P-probs` | pass | 7/7 | -a -P: the priors throughout, -p -g |
| `priors/a-n2` | pass | 6/6 | -a -n 2: two classes |
| `priors/a-P-n2` | pass | 6/6 | -a -P -n 2 |
| `priors/a-n4` | pass | 6/6 | -a -n 4 (priors serve 2 or 3 classes only) |
| `priors/A-a` | pass | 6/6 | -A (ds000005's own maps in the template's space) with -a (the identity) and -P |
| `priors/A-alone` | pass | 6/6 | -A without -a (no priors) |
| `priors/P-alone` | pass | 6/6 | -P without -a (no priors) |
| `priors/missing-prior` | pass | 6/6 | -A naming a missing prior with -a |
| `priors/bad-matrix` | both-error | 6/6 | -a naming a missing matrix (FLIRT fails; FAST crashes: FSL-101) |
| `priors/name-blank` | both-error | 6/6 | -o with a blank: FLIRT's command split by the shell (FSL-102) |
| `priors/multi-a-P` | pass | 6/6 | -S 2 -a -P: the multi-channel segmentation with the priors |
| `priors/multi-a-n2` | pass | 6/6 | -S 2 -a -n 2 |
| `multi/s2-opposite-contrasts` | pass | 6/6 | -S 2 -B -b on a phantom of opposite contrasts: classes paired across the channels start empty; every voxel ends in clas… |
| `multi/s2-manualseg-cycle` | pass | 6/6 | -S 2 -s (T1w/T2w ratio, T1w) with the classes in a cyclic order (FSL-98) |
| `difftest/fsl101-empty-mask-runs-on` | both-error | 5/5 | FSL-101 (FLIRT cannot write the registered priors: an output type FSL 6.0.7.23 refuses) on an image whose mask is empty… |
| `difftest/fsl101-multi-exception` | both-error | 5/5 | FSL-101 with three channels whose first voxel is in the mask: FAST's bounds-checked access to the unallocated priors th… |
| `difftest/fsl101-verbose-before-crash` | both-error | 5/5 | FSL-101 with -v: FAST prints 'Beginning prior-based initialisation' before it crashes on the priors it never allocated;… |
