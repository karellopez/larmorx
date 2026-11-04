# Parity: fslmaths replica (bit-exact, FSL Licence)

**All cases agree.** 421 cases on the `standard` tier: 359 pass, 61 rejected by both, 1 expected divergences, 0 failures.

- **Validated:** `larmorx-nc fsl fslmaths` (crate larmorx-nc-avwutils, translated from FSL 6.0.7.23's source)
- **Reference:** fslmaths from FSL 6.0.7.23 (fsl-avwutils 2209.6, fsl-newimage 2601.0, fsl-miscmaths 2412.6, fsl-cprob 2111.0; the conda build at `~/fsl-6.0.7.23`, as built: glibc 2.35's maths)
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-04 on Linux x86_64, with `python -m larmorx_validation parity fsl-maths-replica --tier standard`
- **Original programs:** 416 runs: 377 from the oracle cache, 39 run; cache `<workspace>/oracles/cache`, mode `use`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (a crash: the replica exits 1) |
| Standard output and error | identical (the run directory and FSL's program path replaced; the replica's `larmorx-nc:` lines left out) |
| Files written | the same names |
| Every file | bit-identical (images decompressed; `descrip` left out) |
| D13 fix pass | a defect named exactly when the default's outputs differ |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bptf | 12 | 11 | 1 | 0 | 0 |
| cli | 24 | 14 | 10 | 0 | 0 |
| conv | 29 | 27 | 2 | 0 | 0 |
| d13 | 82 | 56 | 26 | 0 | 0 |
| difftest | 4 | 3 | 1 | 0 | 0 |
| dilall | 4 | 4 | 0 | 0 | 0 |
| dim | 38 | 33 | 5 | 0 | 0 |
| dog | 5 | 5 | 0 | 0 | 0 |
| dt | 20 | 20 | 0 | 0 | 0 |
| edge | 6 | 6 | 0 | 0 | 0 |
| fillh | 5 | 5 | 0 | 0 | 0 |
| fmriprep | 18 | 18 | 0 | 0 | 0 |
| fn | 37 | 36 | 0 | 1 | 0 |
| io | 5 | 2 | 3 | 0 | 0 |
| morph | 24 | 23 | 1 | 0 | 0 |
| norm | 6 | 6 | 0 | 0 | 0 |
| op | 25 | 23 | 2 | 0 | 0 |
| roc | 8 | 7 | 1 | 0 | 0 |
| roi | 7 | 5 | 2 | 0 | 0 |
| stat | 16 | 16 | 0 | 0 | 0 |
| sub | 7 | 7 | 0 | 0 | 0 |
| tensor | 4 | 3 | 1 | 0 | 0 |
| tfce | 15 | 12 | 3 | 0 | 0 |
| tfceS | 8 | 7 | 1 | 0 | 0 |
| thr | 12 | 10 | 2 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `fn/double-tan` | GLIBC-07: 3 of 210 double voxels 1 unit(s) in the last place from FSL's, where glibc 2.35's function (IBM's code, LGPL) is not correctly rounded and the replica is (measured on image intensities: tan 0.21 %, asin 0.16 %, acos 0.071 %, atan 0.083 %) |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `cli/usage` | no arguments: the usage (exit 1) | fslmaths exits 1; the same from the replica. |
| `cli/missing-input` | an input that does not exist | fslmaths exits -6; the same from the replica. Image Exception : #63 :: No image files match: nothere terminate called after throwing an instance of 'std::runti… |
| `cli/no-output` | no output name | fslmaths exits 1; the same from the replica. Error: no output filename specified! |
| `cli/unknown-op` | an unknown operation: the usage | fslmaths exits 1; the same from the replica. |
| `cli/odt-unknown` | -odt with an unknown type | fslmaths exits 1; the same from the replica. |
| `cli/dt-unknown` | -dt with an unknown type | fslmaths exits 1; the same from the replica. |
| `cli/dt-no-input` | -dt float without an input (FSL-158) | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): basic_string: construction from null is… |
| `cli/add-output-image` | -add reading the output's name as an image | fslmaths exits -6; the same from the replica. Image Exception : #63 :: No image files match: out Image Exception : #22 :: Failed to read volume out Error : No … |
| `cli/analyze` | FSLOUTPUTTYPE=ANALYZE | fslmaths exits 1; the same from the replica. ERROR:: Unrecognised value (ANALYZE) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NI… |
| `cli/outtype-bad` | FSLOUTPUTTYPE unknown | fslmaths exits 1; the same from the replica. ERROR:: Unrecognised value (BOGUS) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NIFT… |
| `io/truncated-gz` | a gzip stream cut inside the data (FSL's short read) | fslmaths exits -6; the same from the replica. Image Exception : #22 :: Failed to read volume trunc.nii.gz Error : Error: short read, file may be truncated term… |
| `io/truncated-raw` | an uncompressed file cut inside the data | fslmaths exits -6; the same from the replica. Image Exception : #22 :: Failed to read volume truncraw.nii Error : Error: short read, file may be truncated term… |
| `io/huge-header` | a header claiming 1000 x 1000 x 1000 voxels, its file 64 bytes of data | fslmaths exits -6; the same from the replica. Image Exception : #22 :: Failed to read volume hugehdr.nii Error : Error: short read, file may be truncated termi… |
| `op/4d-by-4d-other-t` | 4D images of 3 and 5 volumes: refused | fslmaths exits -6; the same from the replica. Image Exception : #3 :: Attempted to add images of different sizes terminate called after throwing an instance of… |
| `op/size-mismatch` | -add an image of another size: refused | fslmaths exits -6; the same from the replica. WARNING:: Inconsistent orientations for individual images in pipeline! Will use voxel-based orientation which is … |
| `thr/thr-word` | -thr without a number | fslmaths exits 1; the same from the replica. Error: -thr requires a number |
| `thr/uthrP-word` | -uthrP without a number | fslmaths exits 1; the same from the replica. Error: -uthrP requires a number |
| `d13/rem-zero` | FSL-156: -rem 0 (FSL crashes) | fslmaths exits -8; the same from the replica. |
| `d13/rem-half` | FSL-156: -rem 0.5 (FSL crashes) | fslmaths exits -8; the same from the replica. |
| `d13/rem-image-half` | FSL-156: -rem by an image with a voxel 0.5 | fslmaths exits -8; the same from the replica. |
| `d13/div-int-half` | FSL-156: -dt int -div 0.5 (FSL crashes) | fslmaths exits -8; the same from the replica. |
| `d13/grid-zero` | FSL-156: -grid with spacing 0 | fslmaths exits -8; the same from the replica. |
| `d13/grid-null` | FSL-158: -grid without its spacing (reads the null pointer) | fslmaths exits -11; the same from the replica. |
| `conv/kernel-no-output` | -kernel box 3 without an output name | fslmaths exits 1; the same from the replica. Error: no output filename specified! |
| `conv/kernel-missing-size` | -kernel box with the output name as its size: no output name left | fslmaths exits 1; the same from the replica. Error: no output filename specified! |
| `morph/file-inf2` | -kernel file with an infinite value and none at 0.5: more values than the vector holds | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::out_of_range' what(): Mat::operator(): index out of bounds |
| `d13/tensor-nan` | FSL-166: -tensor_decomp with a NaN tensor | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::out_of_range' what(): Mat::operator(): index out of bounds |
| `tensor/not-a-tensor` | -tensor_decomp of a 3-volume image | fslmaths exits 1; the same from the replica. |
| `d13/kernel-boxv3-null` | FSL-158: -kernel boxv3 without its sizes (reads the null pointer) | fslmaths exits -11; the same from the replica. |
| `d13/subsamp2-4d` | FSL-163: -subsamp2 of a 4D image | fslmaths exits -6; the same from the replica. Image Exception : #75 :: 3D only method called by higher-dimensional volume. terminate called after throwing an i… |
| `d13/subsamp2offc-4d-odd` | FSL-163: -subsamp2offc of a 4D image of odd sizes | fslmaths exits -6; the same from the replica. Image Exception : #75 :: 3D only method called by higher-dimensional volume. terminate called after throwing an i… |
| `d13/fillh-2d` | FSL-165: -fillh of a 2D image | fslmaths exits -8; the same from the replica. |
| `d13/fmedian-nan` | FSL-162: -fmedian with NaN voxels | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `d13/dilD-nan` | FSL-162: -dilD with a NaN next to a zero voxel | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `d13/fmean-int-trap` | FSL-156: -kernel sphere 5 -fmean under -dt int (an integer division by zero) | fslmaths exits -8; the same from the replica. |
| `dim/unknown` | -Tfoo: an unknown reduction | fslmaths exits 1; the same from the replica. Error unknown Dimensionality operation: -Tfoo |
| `dim/empty` | -T alone | fslmaths exits 1; the same from the replica. Error unknown Dimensionality operation: -T |
| `dim/perc-range` | -Tperc 150 (newimage's exception) | fslmaths exits -6; the same from the replica. Image Exception : #4 :: Percentiles must be in the range [0.0,1.0] terminate called after throwing an instance of… |
| `dim/perc-negative` | -Tperc -5 | fslmaths exits -6; the same from the replica. Image Exception : #4 :: Percentiles must be in the range [0.0,1.0] terminate called after throwing an instance of… |
| `dim/perc-no-argument` | -Tperc with the output name as its argument | fslmaths exits 1; the same from the replica. Error: no output filename specified! |
| `roi/into-output` | -roi with the output name as its last argument | fslmaths exits 1; the same from the replica. Error: no output filename specified! |
| `roi/null` | FSL-158: -roi without its last sizes (reads the null pointer) | fslmaths exits -11; the same from the replica. |
| `bptf/into-output` | -bptf with the output name as its second sigma | fslmaths exits 1; the same from the replica. Error: no output filename specified! |
| `d13/rank-nan` | FSL-162: -rank of series holding NaN | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `d13/ranknorm-nan` | FSL-162: -ranknorm of series holding NaN | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `d13/bptf-lp-huge` | FSL-171: -bptf -1 2e8 (FSL crashes) | fslmaths exits -11; the same from the replica. |
| `d13/bptf-hp-length` | FSL-171: -bptf 4e8 -1 (std::bad_array_new_length) | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::bad_array_new_length' what(): std::bad_array_new_length |
| `d13/bptf-lp-length` | FSL-171: -bptf -1 6e7 (std::bad_array_new_length) | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::bad_array_new_length' what(): std::bad_array_new_length |
| `tfce/small` | -tfce of an image 2 voxels wide (tfce's error) | fslmaths exits 1; the same from the replica. ERROR: TFCE failed, please check your file for valid sizes and voxel values.Error: tfce currently requires an inpu… |
| `tfce/negative-delta` | -tfce_delta -1 (tfce's error) | fslmaths exits 1; the same from the replica. ERROR: TFCE failed, please check your file for valid sizes and voxel values.Error: tfce requires a positive deltaT… |
| `tfce/into-output` | -tfce with the output name as its connectivity | fslmaths exits 1; the same from the replica. Error: no output filename specified! |
| `tfceS/inf` | -tfceS at an infinite voxel (in no cluster: std::out_of_range) | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::out_of_range' what(): Mat::operator(): index out of bounds |
| `d13/tfce-zero` | FSL-175: -tfce of an image of zeros | fslmaths exits 1; the same from the replica. ERROR: TFCE failed, please check your file for valid sizes and voxel values.Error: tfce requires a positive deltaT… |
| `d13/tfce-negative` | FSL-175: -tfce of an image without positive values | fslmaths exits 1; the same from the replica. ERROR: TFCE failed, please check your file for valid sizes and voxel values.Error: tfce requires a positive deltaT… |
| `d13/tfceS-stall` | FSL-174: -tfceS with -tfce_minT -1e9 (std::out_of_range) | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::out_of_range' what(): Mat::operator(): index out of bounds |
| `d13/tfceS-negative-delta` | FSL-174: -tfceS with -tfce_delta -0.5 | fslmaths exits -8; the same from the replica. |
| `d13/tfceS-minT` | FSL-177: -tfceS with -tfce_minT -2 (more thresholds than its arrays) | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::out_of_range' what(): Mat::operator(): index out of bounds |
| `roc/truth-size` | -roc with a truth image of another size (newimage's exception) | fslmaths exits -6; the same from the replica. Image Exception : #3 :: Attempted to multiply images of different sizes terminate called after throwing an instan… |
| `d13/roc-no-true` | FSL-180: -roc with a truth image without true voxels | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `d13/roc-no-noise` | FSL-180: -roc -0.05 with a truth image without zero voxels | fslmaths exits -6; the same from the replica. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `difftest/subsamp2-three-wide` | random testing (2026-11-03): -subsamp2 of a 3 x 1 x 2 image (a 6-voxel axis halved twice): the background value's buffer is too small (FSL-75), fslmaths writes past it and aborts (double free or corruption); the replica names FSL-75 (the value is never used) | fslmaths exits -6; the same from the replica. double free or corruption (out) |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (f840a802d06f) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The replica runs as a separate program, the FSL replica package's (`larmorx-nc`), compared with FSL in `reproduce` mode (D13); the default `fix` pass is checked against the reproduced run. The synthetic inputs are made once (`oracles/inputs/fsl-maths-replica/`); fMRIPrep's masks come from the oracle runs' working directories.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds000005-sub-01-noreconall/sub_01_wf/enhance_and_skullstrip_bold_wf` | pass | 7/7 | fMRIPrep's combine_masks call (fslmaths <mask> -mul <mask> <out>) |
| `fmriprep/ds003592-sub-280-noreconall/sub_280_ses_1_wf/enhance_and_skullstrip_bold_wf` | pass | 7/7 | fMRIPrep's combine_masks call (fslmaths <mask> -mul <mask> <out>) |
| `fmriprep/ds005040-sub-002-noreconall/sub_002_wf/enhance_and_skullstrip_bold_wf` | pass | 7/7 | fMRIPrep's combine_masks call (fslmaths <mask> -mul <mask> <out>) |
| `fmriprep/ds005040-sub-002-noreconall/sub_002_wf/skullstrip_bold_wf` | pass | 7/7 | fMRIPrep's combine_masks call (fslmaths <mask> -mul <mask> <out>) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/stdev_volume` | pass | 6/6 | fMRIPrep's goodvoxels mask, node stdev_volume (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/mean_volume` | pass | 6/6 | fMRIPrep's goodvoxels mask, node mean_volume (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_volume` | pass | 6/6 | fMRIPrep's goodvoxels mask, node cov_volume (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_ribbon` | pass | 6/6 | fMRIPrep's goodvoxels mask, node cov_ribbon (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_ribbon_norm` | pass | 6/6 | fMRIPrep's goodvoxels mask, node cov_ribbon_norm (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/smooth_norm` | pass | 6/6 | fMRIPrep's goodvoxels mask, node smooth_norm (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_ribbon_norm_smooth` | pass | 6/6 | fMRIPrep's goodvoxels mask, node cov_ribbon_norm_smooth (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_norm` | pass | 6/6 | fMRIPrep's goodvoxels mask, node cov_norm (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_norm_modulate` | pass | 6/6 | fMRIPrep's goodvoxels mask, node cov_norm_modulate (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_norm_modulate_ribbon` | pass | 6/6 | fMRIPrep's goodvoxels mask, node cov_norm_modulate_ribbon (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/bin_mean_volume` | pass | 6/6 | fMRIPrep's goodvoxels mask, node bin_mean_volume (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/goodvoxels_thr` | pass | 6/6 | fMRIPrep's goodvoxels mask, node goodvoxels_thr (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/goodvoxels_mask` | pass | 6/6 | fMRIPrep's goodvoxels mask, node goodvoxels_mask (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/goodvoxels_ribbon_mask` | pass | 6/6 | fMRIPrep's goodvoxels mask, node goodvoxels_ribbon_mask (--project-goodvoxels) |
| `cli/usage` | both-error | 5/5 | no arguments: the usage (exit 1) |
| `cli/help` | pass | 6/6 | -h: the usage (exit 0) |
| `cli/help-long` | pass | 6/6 | --help |
| `cli/missing-input` | both-error | 5/5 | an input that does not exist |
| `cli/no-output` | both-error | 5/5 | no output name |
| `cli/copy` | pass | 6/6 | no operation: a copy (the display range kept) |
| `cli/unknown-op` | both-error | 5/5 | an unknown operation: the usage |
| `cli/odt-unknown` | both-error | 5/5 | -odt with an unknown type |
| `cli/dt-unknown` | both-error | 5/5 | -dt with an unknown type |
| `cli/dt-no-input` | both-error | 5/5 | -dt float without an input (FSL-158) |
| `cli/op-as-output` | pass | 6/6 | an operation in the output's place |
| `cli/add-output-image` | both-error | 5/5 | -add reading the output's name as an image |
| `cli/chain` | pass | 6/6 | a chain of operations |
| `cli/datatype-long` | pass | 6/6 | -datatype and -output_datatype |
| `cli/save-restart` | pass | 6/6 | -save mid-chain and -restart |
| `cli/range` | pass | 6/6 | -range: the display range of the result |
| `cli/nifti` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI |
| `cli/pair` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI_PAIR, a pair as input |
| `cli/nifti2` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `cli/analyze` | both-error | 5/5 | FSLOUTPUTTYPE=ANALYZE |
| `cli/outtype-bad` | both-error | 5/5 | FSLOUTPUTTYPE unknown |
| `cli/nii-input` | pass | 6/6 | an uncompressed input |
| `io/truncated-gz` | both-error | 5/5 | a gzip stream cut inside the data (FSL's short read) |
| `io/truncated-raw` | both-error | 5/5 | an uncompressed file cut inside the data |
| `io/huge-header` | both-error | 5/5 | a header claiming 1000 x 1000 x 1000 voxels, its file 64 bytes of data |
| `io/gap-raw` | pass | 6/6 | the data after a gap (vox_offset 1024), uncompressed |
| `io/gap-gz` | pass | 6/6 | the data after a gap (vox_offset 1024), gzip-compressed |
| `cli/neuro` | pass | 6/6 | a neurological input (swapped in memory, back on output) |
| `cli/noform` | pass | 6/6 | an input without sform and qform |
| `dt/char` | pass | 6/6 | -dt char on a uint8 image |
| `dt/short` | pass | 6/6 | -dt short (wrap-around) |
| `dt/int` | pass | 6/6 | -dt int |
| `dt/double` | pass | 6/6 | -dt double |
| `dt/float` | pass | 6/6 | -dt float |
| `dt/input-short` | pass | 6/6 | -dt input on int16 |
| `dt/input-char` | pass | 6/6 | -dt input on uint8 |
| `dt/input-uint16` | pass | 6/6 | -dt input on uint16 (FSL-155) |
| `dt/input-int8` | pass | 6/6 | -dt input on int8 (FSL-155) |
| `dt/double-default` | pass | 6/6 | a double input: computed and written in double |
| `dt/scaled` | pass | 6/6 | a scaled int16 input (read as float) |
| `dt/odt-char` | pass | 6/6 | -odt char (rounded; values out of range: FSL-151) |
| `dt/odt-short` | pass | 6/6 | -odt short |
| `dt/odt-int` | pass | 6/6 | -odt int |
| `dt/odt-double` | pass | 6/6 | -odt double |
| `dt/odt-input` | pass | 6/6 | -odt input on int16 |
| `dt/odt-input-uint16` | pass | 6/6 | -odt input on uint16 |
| `d13/char-uint8-thr` | pass | 6/6 | FSL-154: -dt char on a uint8 image, -thr 100 (values 128-255 read as negative) |
| `d13/input-uint8-div` | pass | 6/6 | FSL-154: -dt input on a uint8 image, -div 2, written as float |
| `d13/odt-input-int8` | pass | 6/6 | FSL-157: -odt input on an int8 image (truncated, written as uint8) |
| `d13/odt-input-uint16-round` | pass | 6/6 | FSL-157: -odt input on a uint16 image, -mul 0.5 (truncated, not rounded) |
| `dt/odt-nan` | pass | 6/6 | -odt short with NaN and infinite voxels (FSL-151) |
| `dt/int-overflow` | pass | 6/6 | -dt int with a number beyond int (FSL-151) |
| `dt/int-exp` | pass | 6/6 | -dt short -exp (results beyond short: FSL-151) |
| `op/add-number` | pass | 6/6 | -add a number |
| `op/sub-number` | pass | 6/6 | -sub a number |
| `op/mul-number` | pass | 6/6 | -mul a number |
| `op/div-number` | pass | 6/6 | -div a number |
| `op/div-zero` | pass | 6/6 | -div 0: nothing done |
| `op/add-image` | pass | 6/6 | -add an image |
| `op/sub-image` | pass | 6/6 | -sub an image |
| `op/mul-image` | pass | 6/6 | -mul an image |
| `op/div-image` | pass | 6/6 | -div an image with zeros (0 there) |
| `op/3d-by-4d` | pass | 6/6 | a 3D image times a 4D one: cloned over time |
| `op/4d-by-3d` | pass | 6/6 | a 4D image times a 3D one |
| `op/4d-by-4d-other-t` | both-error | 5/5 | 4D images of 3 and 5 volumes: refused |
| `op/size-mismatch` | both-error | 5/5 | -add an image of another size: refused |
| `op/row` | pass | 6/6 | -add a 7x1x1 image (newimage compares the first dimension only) |
| `op/orientation` | pass | 6/6 | -add an image of another orientation: the warning |
| `op/mas` | pass | 6/6 | -mas a uint8 mask |
| `op/mas-4d` | pass | 6/6 | -mas a 4D mask on a 3D image |
| `op/mas-inf` | pass | 6/6 | -mas: infinite voxels outside the mask |
| `op/rem-number` | pass | 6/6 | -rem a number |
| `op/rem-image` | pass | 6/6 | -rem an image (0 voxels skipped) |
| `op/max-number` | pass | 6/6 | -max a number |
| `op/min-number` | pass | 6/6 | -min a number |
| `op/max-image` | pass | 6/6 | -max an image |
| `op/min-image-4d` | pass | 6/6 | -min a 4D image |
| `op/nan-number` | pass | 6/6 | -add nan |
| `thr/thr` | pass | 6/6 | -thr |
| `thr/uthr` | pass | 6/6 | -uthr |
| `thr/thrp` | pass | 6/6 | -thrp: the robust range |
| `thr/thrP` | pass | 6/6 | -thrP: the positive voxels' robust range |
| `thr/uthrp` | pass | 6/6 | -uthrp |
| `thr/uthrP` | pass | 6/6 | -uthrP |
| `thr/thrP-empty` | pass | 6/6 | -thrP of an image without positive voxels: the warnings |
| `d13/thrp-far` | pass | 6/6 | FSL-187: -thrp 50 of 1000 values on [0, 1) and one at 1e12 (the robust range) |
| `d13/uthrP-far` | pass | 6/6 | FSL-187: -uthrP 98 of the same image (the positive voxels' robust range) |
| `thr/thr-word` | both-error | 5/5 | -thr without a number |
| `thr/uthrP-word` | both-error | 5/5 | -uthrP without a number |
| `thr/thr-short` | pass | 6/6 | -thr under -dt short |
| `thr/thr-4d` | pass | 6/6 | -thrp of a 4D image |
| `thr/thr-inf` | pass | 6/6 | -thr with infinite voxels |
| `fn/exp` | pass | 6/6 | -exp |
| `fn/log` | pass | 6/6 | -log |
| `fn/sin` | pass | 6/6 | -sin |
| `fn/cos` | pass | 6/6 | -cos |
| `fn/tan` | pass | 6/6 | -tan |
| `fn/asin` | pass | 6/6 | -asin (NaN outside [-1, 1]) |
| `fn/acos` | pass | 6/6 | -acos |
| `fn/atan` | pass | 6/6 | -atan |
| `fn/sqr` | pass | 6/6 | -sqr |
| `fn/sqrt` | pass | 6/6 | -sqrt (0 for non-positive values) |
| `fn/recip` | pass | 6/6 | -recip |
| `fn/abs` | pass | 6/6 | -abs |
| `fn/sin-inf` | pass | 6/6 | -sin of infinite and NaN voxels |
| `fn/tan-inf` | pass | 6/6 | -tan of infinite and NaN voxels |
| `fn/exp-inf` | pass | 6/6 | -exp of infinite and NaN voxels |
| `fn/log-inf` | pass | 6/6 | -log of infinite voxels |
| `fn/double-sin` | pass | 6/6 | -sin in double |
| `fn/double-cos` | pass | 6/6 | -cos in double |
| `fn/double-tan` | expected-divergence | 4/5 | -tan in double |
| `fn/double-atan` | pass | 6/6 | -atan in double |
| `fn/double-asin` | pass | 6/6 | -asin and -acos in double |
| `fn/double-exp-log` | pass | 6/6 | -exp, -log and -pow in double |
| `fn/bin` | pass | 6/6 | -bin |
| `fn/binv` | pass | 6/6 | -binv |
| `fn/nan` | pass | 6/6 | -nan |
| `fn/nanm` | pass | 6/6 | -nanm |
| `fn/index` | pass | 6/6 | -index |
| `fn/grid` | pass | 6/6 | -grid |
| `fn/rand` | pass | 6/6 | -rand (fixed initial state) |
| `fn/randn` | pass | 6/6 | -randn |
| `fn/seed-rand` | pass | 6/6 | -seed then -rand |
| `fn/seed-randn` | pass | 6/6 | -seed then -randn |
| `fn/pow` | pass | 6/6 | -pow (undocumented) |
| `fn/invbin` | pass | 6/6 | -invbin (undocumented) |
| `fn/int-recip` | pass | 6/6 | -recip under -dt int |
| `fn/int-sqrt` | pass | 6/6 | -sqrt under -dt short |
| `fn/char-index` | pass | 6/6 | -index under -dt char (wraps) |
| `d13/bin-large` | pass | 6/6 | FSL-152: -bin of values from 2^24 |
| `d13/bin-char` | pass | 6/6 | FSL-152: -bin under -dt char (max 127 + 1 wraps) |
| `d13/thr-inf` | pass | 6/6 | FSL-152: -thr when the maximum is infinite |
| `d13/uthr-char` | pass | 6/6 | FSL-152: -uthr under -dt char (min - 1 wraps) |
| `d13/nan-thr` | pass | 6/6 | FSL-153: -thr with a NaN first voxel |
| `d13/nan-bin` | pass | 6/6 | FSL-153: -bin with a NaN first voxel |
| `d13/nan-mas` | pass | 6/6 | FSL-153: -mas by a mask with a NaN first voxel |
| `d13/nan-range` | pass | 6/6 | FSL-153: -range with a NaN first voxel |
| `d13/nan-thrp` | pass | 6/6 | FSL-153: -thrp with NaN voxels |
| `d13/rem-zero` | both-error | 5/5 | FSL-156: -rem 0 (FSL crashes) |
| `d13/rem-half` | both-error | 5/5 | FSL-156: -rem 0.5 (FSL crashes) |
| `d13/rem-image-half` | both-error | 5/5 | FSL-156: -rem by an image with a voxel 0.5 |
| `d13/div-int-half` | both-error | 5/5 | FSL-156: -dt int -div 0.5 (FSL crashes) |
| `d13/grid-zero` | both-error | 5/5 | FSL-156: -grid with spacing 0 |
| `d13/grid-null` | both-error | 5/5 | FSL-158: -grid without its spacing (reads the null pointer) |
| `d13/div-other` | pass | 6/6 | FSL-159: -div by an image of another size |
| `d13/rem-other` | pass | 6/6 | FSL-159: -rem by a smaller image |
| `d13/max-other` | pass | 6/6 | FSL-159: -max with a larger image |
| `d13/min-small` | pass | 2/2 | FSL-159: -min with a smaller image (FSL reads past its end) |
| `morph/dilF` | pass | 6/6 | -dilF (maximum filter, default kernel) |
| `morph/dilF-nan` | pass | 6/6 | -dilF with NaN voxels (left out) |
| `morph/dilM` | pass | 6/6 | -dilM (mean dilation) |
| `morph/dilM-nan` | pass | 6/6 | -dilM with NaN neighbours |
| `morph/dilD` | pass | 6/6 | -dilD (modal dilation) |
| `morph/dilD-short` | pass | 6/6 | -dilD of labels under -dt short |
| `morph/ero` | pass | 6/6 | -ero (zeroing next to zeros) |
| `morph/eroF` | pass | 6/6 | -eroF (minimum filter) |
| `morph/eroF-nan` | pass | 6/6 | -eroF with NaN voxels |
| `morph/fmedian` | pass | 6/6 | -fmedian |
| `morph/fmedian-2D` | pass | 6/6 | -kernel 2D -fmedian |
| `morph/fmedian-char` | pass | 6/6 | -fmedian under -dt char |
| `morph/eroF-double` | pass | 6/6 | -eroF of a double image |
| `morph/sphere-4d` | pass | 6/6 | -kernel sphere 4 -dilF on each volume of a 4D image |
| `morph/boxv-even` | pass | 6/6 | -kernel boxv 2: the off-centre warning |
| `morph/boxv3` | pass | 6/6 | -kernel boxv3 3 1 5 -fmedian |
| `morph/box-mm` | pass | 6/6 | -kernel box 4 (mm, anisotropic voxels) -eroF |
| `morph/file` | pass | 6/6 | -kernel file (values around 0.5) -dilF |
| `morph/file-even` | pass | 6/6 | -kernel file of even size -dilM |
| `morph/file-4d` | pass | 6/6 | -kernel file of a 4D image (the first volume, a warning) |
| `morph/file-inf` | pass | 6/6 | -kernel file with an infinite value (Armadillo's bounds check) |
| `morph/2d-ero` | pass | 6/6 | -kernel 2D -ero of a 2D image |
| `morph/2d-dilD` | pass | 6/6 | -dilD of a 2D image |
| `dilall/3d` | pass | 6/6 | -dilall |
| `dilall/4d` | pass | 6/6 | -thr 10 -dilall of a 4D image |
| `dilall/int` | pass | 6/6 | -dilall under -dt int |
| `dilall/nan` | pass | 6/6 | -dilall with NaN voxels |
| `conv/s` | pass | 6/6 | -s 2 (separable Gaussian) |
| `conv/s-odd` | pass | 6/6 | -s 1.5 of an odd, anisotropic image |
| `conv/s-4d` | pass | 6/6 | -s 1 of a 4D image |
| `conv/s-2d` | pass | 6/6 | -s 2 of a 2D image |
| `conv/s-nan` | pass | 6/6 | -s 1 with NaN voxels |
| `conv/fmean-default` | pass | 6/6 | -fmean with the default kernel (not separable) |
| `conv/fmean-box` | pass | 6/6 | -kernel box 5 -fmean (separable) |
| `conv/fmean-boxv-even` | pass | 6/6 | -kernel boxv 4 -fmean (separable, even: warnings) |
| `conv/fmean-sphere` | pass | 6/6 | -kernel sphere 3 -fmean (direct sum) |
| `conv/fmean-sphere-nan` | pass | 6/6 | -kernel sphere 4 -fmean with NaN voxels |
| `conv/fmean-double` | pass | 6/6 | -kernel sphere 4 -fmean of a double image |
| `conv/fmean-file` | pass | 6/6 | -kernel file -fmean (not separable) |
| `conv/fmean-file-even` | pass | 6/6 | -kernel file of even size -fmean (warnings) |
| `conv/fmean-2D` | pass | 6/6 | -kernel 2D -fmean |
| `conv/separable-kept` | pass | 6/6 | -kernel gauss 1 -kernel 3D -fmean (separable kept from gauss) |
| `conv/fmeanu` | pass | 6/6 | -kernel sphere 5 -fmeanu |
| `conv/fmeanu-gauss` | pass | 6/6 | -kernel gauss 2 -fmeanu (not separable) |
| `conv/fmeanu-char` | pass | 6/6 | -kernel sphere 3 -fmeanu under -dt char |
| `conv/fmean-int` | pass | 6/6 | -kernel boxv 3 -fmean under -dt int (separable) |
| `conv/fft-sphere` | pass | 6/6 | -kernel sphere 12 -fmean (the FFT path: radices 2, 3, 11, 13) |
| `conv/fft-box` | pass | 6/6 | -kernel box 24 -fmeanu (the FFT path) |
| `conv/fft-even` | pass | 6/6 | -kernel boxv 12 -fmeanu (the FFT path, an even kernel: radix 4) |
| `conv/fft-odd` | pass | 6/6 | -kernel sphere 10 -fmean of an odd, anisotropic image (the FFT path: radix 5) |
| `conv/fft-double` | pass | 6/6 | -kernel sphere 12 -fmean of a double image (the FFT path) |
| `conv/fft-4d` | pass | 6/6 | -kernel sphere 12 -fmeanu of a 4D image (the FFT path) |
| `conv/fft-int` | pass | 6/6 | -kernel sphere 12 -fmeanu under -dt int (the FFT path) |
| `conv/fft-nan` | pass | 6/6 | -kernel sphere 12 -fmeanu with NaN voxels (the FFT path) |
| `conv/kernel-no-output` | both-error | 5/5 | -kernel box 3 without an output name |
| `conv/kernel-missing-size` | both-error | 5/5 | -kernel box with the output name as its size: no output name left |
| `morph/file-inf2` | both-error | 5/5 | -kernel file with an infinite value and none at 0.5: more values than the vector holds |
| `sub/subsamp2` | pass | 6/6 | -subsamp2 |
| `sub/subsamp2offc` | pass | 6/6 | -subsamp2offc |
| `sub/subsamp2-short` | pass | 6/6 | -subsamp2 under -dt short |
| `sub/subsamp2offc-int` | pass | 6/6 | -subsamp2offc under -dt int |
| `sub/subsamp2offc-double` | pass | 6/6 | -subsamp2offc of a double image |
| `sub/subsamp2-noform` | pass | 6/6 | -subsamp2 without sform or qform |
| `sub/subsamp2-nan` | pass | 6/6 | -subsamp2 with NaN voxels |
| `edge/3d` | pass | 6/6 | -edge |
| `edge/4d` | pass | 6/6 | -edge of a 4D image |
| `edge/2d` | pass | 6/6 | -edge of a 2D image |
| `edge/short` | pass | 6/6 | -edge under -dt short |
| `edge/nan` | pass | 6/6 | -edge with NaN voxels |
| `edge/aniso` | pass | 6/6 | -edge with anisotropic voxels |
| `dog/3d` | pass | 6/6 | -dog_edge 1 1.6 |
| `dog/aniso` | pass | 6/6 | -dog_edge 0.5 2 of an odd image |
| `dog/double` | pass | 6/6 | -dog_edge of a double image |
| `dog/int` | pass | 6/6 | -dog_edge under -dt int |
| `dog/radius0` | pass | 6/6 | -dog_edge 0.1 0.2 (radius 0) |
| `fillh/6` | pass | 6/6 | -fillh |
| `fillh/26` | pass | 6/6 | -fillh26 |
| `fillh/blob` | pass | 6/6 | -fillh of a blob at the edge |
| `fillh/short` | pass | 6/6 | -fillh under -dt short |
| `fillh/4d` | pass | 6/6 | -fillh26 of a 4D mask |
| `norm/inm` | pass | 6/6 | -inm 100 of a 4D image |
| `norm/ing` | pass | 6/6 | -ing 100 of a 4D image |
| `norm/inm-sparse` | pass | 6/6 | -inm 100 of a sparse image |
| `norm/inm-empty` | pass | 6/6 | -inm of an image of zeros (an empty mask: NaN) |
| `norm/ing-int` | pass | 6/6 | -ing under -dt int |
| `norm/inm-nan` | pass | 6/6 | -inm with NaN voxels |
| `tensor/decomp` | pass | 6/6 | -tensor_decomp |
| `tensor/chain` | pass | 6/6 | -tensor_decomp, then the FA image multiplied by 2 |
| `tensor/double` | pass | 6/6 | -tensor_decomp under -dt double, an output name with its extension |
| `d13/tensor-nan` | both-error | 5/5 | FSL-166: -tensor_decomp with a NaN tensor |
| `tensor/not-a-tensor` | both-error | 5/5 | -tensor_decomp of a 3-volume image |
| `d13/kernel-boxv3-null` | both-error | 5/5 | FSL-158: -kernel boxv3 without its sizes (reads the null pointer) |
| `d13/subsamp2-4d` | both-error | 5/5 | FSL-163: -subsamp2 of a 4D image |
| `d13/subsamp2offc-4d-odd` | both-error | 5/5 | FSL-163: -subsamp2offc of a 4D image of odd sizes |
| `d13/subsamp2offc-4d-even` | pass | 6/6 | FSL-163: -subsamp2offc of a 4D image of even sizes (the first volume) |
| `d13/dog-4d` | pass | 6/6 | FSL-164: -dog_edge of a 4D image |
| `d13/dog-negative` | pass | 6/6 | FSL-161: -dog_edge -1 -2 (a kernel without voxels) |
| `d13/fillh-2d` | both-error | 5/5 | FSL-165: -fillh of a 2D image |
| `d13/s-zero` | pass | 6/6 | FSL-160: -s 0 |
| `d13/gauss-zero` | pass | 6/6 | FSL-160: -kernel gauss 0 -fmean |
| `d13/gauss-zero-dilF` | pass | 6/6 | FSL-160: -kernel gauss 0 -dilF (no change: no notice) |
| `d13/kernel-boxv0` | pass | 6/6 | FSL-161: -kernel boxv 0 -dilF |
| `d13/kernel-negative` | pass | 6/6 | FSL-161: -kernel boxv -3 -dilF |
| `d13/kernel-unknown` | pass | 6/6 | FSL-161: -kernel foo 3 -fmean |
| `d13/kernel-unknown-dilM` | pass | 6/6 | FSL-161: -kernel foo 3 -dilM |
| `d13/gauss-negative` | pass | 6/6 | FSL-161: -kernel gauss -1 -fmean |
| `d13/s-negative` | pass | 6/6 | FSL-161: -s -1 |
| `d13/fmedian-nan` | both-error | 5/5 | FSL-162: -fmedian with NaN voxels |
| `d13/dilD-nan` | both-error | 5/5 | FSL-162: -dilD with a NaN next to a zero voxel |
| `d13/fmean-int-trap` | both-error | 5/5 | FSL-156: -kernel sphere 5 -fmean under -dt int (an integer division by zero) |
| `dim/Tmean` | pass | 6/6 | -Tmean |
| `dim/Tstd` | pass | 6/6 | -Tstd |
| `dim/Tmax` | pass | 6/6 | -Tmax |
| `dim/Tmin` | pass | 6/6 | -Tmin |
| `dim/Tmaxn` | pass | 6/6 | -Tmaxn (ties: the first index) |
| `dim/Tmedian` | pass | 6/6 | -Tmedian (ties, signed zeros) |
| `dim/Tperc` | pass | 6/6 | -Tperc 30 |
| `dim/Tperc-0` | pass | 6/6 | -Tperc 0 |
| `dim/Tperc-100` | pass | 6/6 | -Tperc 100 |
| `dim/Tar1` | pass | 6/6 | -Tar1 |
| `dim/Tmean-nan` | pass | 6/6 | -Tmean of series holding NaN |
| `dim/Tar1-nan` | pass | 6/6 | -Tar1 of series holding NaN |
| `dim/Xmean` | pass | 6/6 | -Xmean |
| `dim/Ymax` | pass | 6/6 | -Ymax |
| `dim/Zstd` | pass | 6/6 | -Zstd |
| `dim/Xmaxn` | pass | 6/6 | -Xmaxn |
| `dim/Ymedian` | pass | 6/6 | -Ymedian |
| `dim/Zperc` | pass | 6/6 | -Zperc 75 |
| `dim/Xar1` | pass | 6/6 | -Xar1 |
| `dim/Ymin` | pass | 6/6 | -Ymin |
| `dim/Smean` | pass | 6/6 | -Smean (a capital that reduces nothing) |
| `dim/chain` | pass | 6/6 | -Tmean then -Xmax then -add |
| `dim/unknown` | both-error | 5/5 | -Tfoo: an unknown reduction |
| `dim/empty` | both-error | 5/5 | -T alone |
| `dim/perc-range` | both-error | 5/5 | -Tperc 150 (newimage's exception) |
| `dim/perc-negative` | both-error | 5/5 | -Tperc -5 |
| `dim/perc-nan` | pass | 6/6 | -Tperc nan |
| `dim/perc-no-argument` | both-error | 5/5 | -Tperc with the output name as its argument |
| `dim/3d-mean` | pass | 6/6 | -Tmean of a 3D image |
| `dim/Tmean-short` | pass | 6/6 | -Tmean under -dt short |
| `dim/Tstd-int` | pass | 6/6 | -Tstd under -dt int |
| `dim/Tar1-short` | pass | 6/6 | -Tar1 under -dt short |
| `dim/Tar1-int-large` | pass | 6/6 | -Tar1 under -dt int with products beyond int's range |
| `dim/Tmedian-int` | pass | 6/6 | -Tmedian under -dt int |
| `dim/Tmaxn-short` | pass | 6/6 | -Tmaxn under -dt short |
| `dim/Tstd-double` | pass | 6/6 | -Tstd of a double image |
| `dim/Tmedian-double` | pass | 6/6 | -Tmedian of a double image |
| `dim/Tmean-long` | pass | 6/6 | -Tmean of 48 time points |
| `stat/pval` | pass | 6/6 | -pval |
| `stat/pval0` | pass | 6/6 | -pval0 (zeros left out) |
| `stat/cpval` | pass | 6/6 | -cpval |
| `stat/cpval-double` | pass | 6/6 | -cpval of a double image |
| `stat/cpval-int` | pass | 6/6 | -cpval under -dt int |
| `stat/pval-short` | pass | 6/6 | -pval under -dt short |
| `stat/pval-3d` | pass | 6/6 | -pval of a 3D image |
| `stat/ztop` | pass | 6/6 | -ztop (with +-40 and NaN) |
| `stat/ztop-double` | pass | 6/6 | -ztop in double |
| `stat/ptoz` | pass | 6/6 | -ptoz (with 0, 1, outside [0, 1] and NaN) |
| `stat/ztop-ptoz` | pass | 6/6 | -ztop then -ptoz |
| `stat/rank` | pass | 6/6 | -rank (ties, signed zeros) |
| `stat/ranknorm` | pass | 6/6 | -ranknorm |
| `stat/rank-short` | pass | 6/6 | -rank under -dt short |
| `stat/rank-3d` | pass | 6/6 | -rank of a 3D image |
| `stat/ranknorm-double` | pass | 6/6 | -ranknorm of a double image |
| `roi/box` | pass | 6/6 | -roi of a box |
| `roi/full` | pass | 6/6 | -roi with every size -1 |
| `roi/neuro` | pass | 6/6 | -roi of a neurological image (x mapped to newimage's order) |
| `roi/outside` | pass | 6/6 | -roi of a box outside the image |
| `roi/negative-size` | pass | 6/6 | -roi with a size of -3 |
| `roi/into-output` | both-error | 5/5 | -roi with the output name as its last argument |
| `roi/null` | both-error | 5/5 | FSL-158: -roi without its last sizes (reads the null pointer) |
| `bptf/hp` | pass | 6/6 | -bptf 2 -1 (high-pass) |
| `bptf/lp` | pass | 6/6 | -bptf -1 1.5 (low-pass) |
| `bptf/both` | pass | 6/6 | -bptf 3 0.8 |
| `bptf/none` | pass | 6/6 | -bptf 0 0 |
| `bptf/nan` | pass | 6/6 | -bptf nan nan (both skipped) |
| `bptf/wide` | pass | 6/6 | -bptf 100 50 (wider than the series) |
| `bptf/3d` | pass | 6/6 | -bptf of a 3D image |
| `bptf/long` | pass | 6/6 | -bptf 5 2 of 48 time points |
| `bptf/short` | pass | 6/6 | -bptf under -dt short |
| `bptf/double` | pass | 6/6 | -bptf of a double image |
| `bptf/nan-values` | pass | 6/6 | -bptf of series holding NaN |
| `bptf/into-output` | both-error | 5/5 | -bptf with the output name as its second sigma |
| `d13/Tmax-nan` | pass | 6/6 | FSL-153: -Tmax with a NaN first value |
| `d13/Tmin-nan` | pass | 6/6 | FSL-153: -Tmin with a NaN first value |
| `d13/Tmaxn-nan` | pass | 6/6 | FSL-153: -Tmaxn with a NaN first value |
| `d13/Tmedian-nan` | pass | 6/6 | FSL-168: -Tmedian of values holding NaN |
| `d13/Tperc-nan` | pass | 6/6 | FSL-168: -Tperc 80 of values holding NaN |
| `d13/Tstd-3d` | pass | 6/6 | FSL-167: -Tstd of a 3D image |
| `d13/Tstd-negative` | pass | 6/6 | FSL-167: -Tstd of series whose variance rounds below 0 |
| `d13/ranknorm-negative` | pass | 6/6 | FSL-167: -ranknorm of series whose variance rounds below 0 |
| `d13/ranknorm-3d` | pass | 6/6 | FSL-167: -ranknorm of a 3D image |
| `d13/rank-nan` | both-error | 5/5 | FSL-162: -rank of series holding NaN |
| `d13/ranknorm-nan` | both-error | 5/5 | FSL-162: -ranknorm of series holding NaN |
| `d13/pval-nan` | pass | 6/6 | FSL-169: -pval with NaN statistics |
| `d13/pval0-nan` | pass | 6/6 | FSL-169: -pval0 with NaN statistics |
| `d13/cpval-nan` | pass | 6/6 | FSL-169: -cpval with NaN statistics |
| `d13/ztop-inf` | pass | 6/6 | FSL-170: -ztop of infinite z |
| `d13/ptoz-tiny` | pass | 6/6 | FSL-172: -ptoz of p-values whose 1 - p rounds to 1 |
| `d13/bptf-lp-huge` | both-error | 5/5 | FSL-171: -bptf -1 2e8 (FSL crashes) |
| `d13/bptf-hp-length` | both-error | 5/5 | FSL-171: -bptf 4e8 -1 (std::bad_array_new_length) |
| `d13/bptf-lp-length` | both-error | 5/5 | FSL-171: -bptf -1 6e7 (std::bad_array_new_length) |
| `d13/bptf-hp-inf` | pass | 2/2 | FSL-171: -bptf inf -1 (FSL writes outside its array) |
| `tfce/basic` | pass | 6/6 | -tfce 2 0.5 6 |
| `tfce/conn18` | pass | 6/6 | -tfce 2 0.5 18 |
| `tfce/conn26` | pass | 6/6 | -tfce 2 0.5 26 |
| `tfce/minT` | pass | 6/6 | -tfce_minT 1 -tfce 2 0.5 6 |
| `tfce/delta` | pass | 6/6 | -tfce_delta 0.3 -tfce 1.5 0.66 26 |
| `tfce/negative-minT` | pass | 6/6 | -tfce_minT -1 -tfce 0.5 0.5 6 (powf of negative heights: NaN) |
| `tfce/many-steps` | pass | 6/6 | -tfce_delta 0.0004: the warning of many integral steps |
| `tfce/4d` | pass | 6/6 | -tfce of each volume of a 4D image |
| `tfce/double` | pass | 6/6 | -tfce of a double image |
| `tfce/short` | pass | 6/6 | -tfce under -dt short |
| `tfce/inf` | pass | 6/6 | -tfce with an infinite maximum |
| `tfce/small` | both-error | 5/5 | -tfce of an image 2 voxels wide (tfce's error) |
| `tfce/negative-delta` | both-error | 5/5 | -tfce_delta -1 (tfce's error) |
| `tfce/into-output` | both-error | 5/5 | -tfce with the output name as its connectivity |
| `tfce/chain` | pass | 6/6 | -tfce then -mul (the options kept between operations) |
| `tfceS/basic` | pass | 6/6 | -tfceS 2 0.5 6 3 3 3 30 |
| `tfceS/conn26` | pass | 6/6 | -tfceS 2 0.5 26 7 6 5 20 |
| `tfceS/not-reached` | pass | 6/6 | -tfceS with a threshold its sum never reaches (the message) |
| `tfceS/delta` | pass | 6/6 | -tfce_delta 0.35 -tfceS (the arrays sized from the step) |
| `tfceS/4d` | pass | 6/6 | -tfceS of each volume of a 4D image |
| `tfceS/double` | pass | 6/6 | -tfceS of a double image |
| `tfceS/low-voxel` | pass | 6/6 | -tfceS at a voxel without positive value |
| `tfceS/inf` | both-error | 5/5 | -tfceS at an infinite voxel (in no cluster: std::out_of_range) |
| `d13/tfce-zero` | both-error | 5/5 | FSL-175: -tfce of an image of zeros |
| `d13/tfce-negative` | both-error | 5/5 | FSL-175: -tfce of an image without positive values |
| `d13/tfce-nan` | pass | 6/6 | FSL-153: -tfce with a NaN first voxel |
| `d13/tfce-conn7` | pass | 6/6 | FSL-176: -tfce 2 0.5 7 |
| `d13/tfceS-conn7` | pass | 6/6 | FSL-176: -tfceS with connectivity 7 |
| `d13/tfce-delta-nan` | pass | 6/6 | FSL-174: -tfce_delta nan (FSL: zeros) |
| `d13/tfce-stall` | pass | 2/2 | FSL-174: -tfce_minT -1e9 (the thresholds never advance: FSL never ends) |
| `d13/tfceS-stall` | both-error | 5/5 | FSL-174: -tfceS with -tfce_minT -1e9 (std::out_of_range) |
| `d13/tfceS-negative-delta` | both-error | 5/5 | FSL-174: -tfceS with -tfce_delta -0.5 |
| `d13/tfceS-minT` | both-error | 5/5 | FSL-177: -tfceS with -tfce_minT -2 (more thresholds than its arrays) |
| `d13/tfceS-outside` | pass | 6/6 | FSL-178: -tfceS at x = 10 of 10 (FSL reads the next row's first voxel) |
| `d13/tfceS-far` | pass | 2/2 | FSL-178: -tfceS at a voxel far outside (FSL reads outside the image) |
| `roc/noise` | pass | 6/6 | -roc 0.05 with a noise image (the FWE rate) |
| `roc/truth` | pass | 6/6 | -roc -0.05: the FP rate from the truth's zero voxels (negative ones left out) |
| `roc/never-reached` | pass | 6/6 | -roc 2 (the FP rate never reaches it) |
| `roc/3d` | pass | 6/6 | -roc of a 3D image |
| `roc/double` | pass | 6/6 | -roc of a double image (log in double) |
| `roc/short` | pass | 6/6 | -roc under -dt short |
| `roc/zero` | pass | 6/6 | -roc 0 (0 / 0 printed) |
| `roc/truth-size` | both-error | 5/5 | -roc with a truth image of another size (newimage's exception) |
| `d13/roc-nan` | pass | 6/6 | FSL-153: -roc of an image whose first voxel is NaN |
| `d13/roc-noise-larger` | pass | 6/6 | FSL-179: -roc with a noise image of 4 volumes for 3 |
| `d13/roc-noise-smaller` | pass | 2/2 | FSL-179: -roc with a noise image of 2 volumes for 3 (FSL writes outside it) |
| `d13/roc-no-true` | both-error | 5/5 | FSL-180: -roc with a truth image without true voxels |
| `d13/roc-no-noise` | both-error | 5/5 | FSL-180: -roc -0.05 with a truth image without zero voxels |
| `difftest/dt-input-uint8-dog-edge` | pass | 6/6 | random testing (2026-11-03): -dt input on a uint8 image, -mul and -dog_edge: the default's unsigned template (FSL-154) … |
| `difftest/edge-double-square` | pass | 6/6 | random testing (2026-11-03): -edge of a double series: FSL's build squares with x * x (GCC folds pow(x, 2.0)), not glib… |
| `difftest/fft-twiddles-glibc-sincos` | pass | 6/6 | random testing (2026-11-03): -recip, -kernel gauss 2.25 and -fmeanu of a 30 x 1 x 3 float image (an FFT convolution: a … |
| `difftest/subsamp2-three-wide` | both-error | 2/2 | random testing (2026-11-03): -subsamp2 of a 3 x 1 x 2 image (a 6-voxel axis halved twice): the background value's buffe… |
