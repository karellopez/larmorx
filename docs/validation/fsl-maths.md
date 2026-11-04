# Parity: fslmaths (clean-room original)

**All cases agree.** 442 cases on the `standard` tier: 374 pass, 63 rejected by both, 5 expected divergences, 0 failures.

- **Validated:** `larmorx fsl fslmaths` (larmorx 0.0.1, crate larmorx-fsl: the clean-room original written from specs/fslmaths.md, Apache-2.0)
- **Reference:** fslmaths from FSL 6.0.7.23 (fsl-avwutils 2209.6), the conda build at `~/fsl-6.0.7.23`, as built: glibc 2.35's maths
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-04 on Linux x86_64, with `python -m larmorx_validation parity fsl-maths --tier standard`
- **Original programs:** 437 runs: 396 from the oracle cache, 41 run; cache `<workspace>/oracles/cache`, mode `use`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (FSL's crash or abort: the original exits 1) |
| Standard output and error | identical (the run directory replaced; the original's `larmorx` lines left out) |
| Files written | the same names |
| Headers | byte for byte (`descrip` left out) |
| Values | bit for bit; double-precision functions within one unit in the last place per function of the chain (GLIBC-07); the sums (-s, -fmean, -fmeanu, -dilM, -dilall, -edge, -subsamp2, -subsamp2offc, -inm, -ing, means, standard deviations, ar1, -ranknorm, -bptf, -ztop, -ptoz) in floating types: the largest difference at most 1e-5 of FSL's largest magnitude and 99 % of the voxels within 4 units in the last place, the NaN and infinite voxels identical (§21); -tensor_decomp's eigenvalues, FA and MD within 1e-5 relative, its mode 1e-4 absolute, its eigenvectors up to their sign (\|cos\| >= 0.9999) where the eigenvalues are distinct by more than 1e-4 relative; -roc's printed area and quartiles within 1e-5 absolute, its curve's numbers to 5 significant digits |
| TFCE | -tfce's values within 1e-5 of FSL's largest enhancement, the NaN and infinite voxels identical (§21); -tfceS bit for bit |
| FSL-75's heap corruption | where FSL ended by a signal (-subsamp2's unused background value corrupts its heap for some sizes) and the original finished naming FSL-75: the outputs compared, the exit status and glibc's message not |
| D13 fix pass | a defect named exactly when the default's outputs differ |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bptf | 12 | 11 | 1 | 0 | 0 |
| cli | 24 | 14 | 10 | 0 | 0 |
| conv | 29 | 25 | 2 | 2 | 0 |
| d13 | 82 | 56 | 26 | 0 | 0 |
| difftest | 17 | 14 | 3 | 0 | 0 |
| dilall | 4 | 4 | 0 | 0 | 0 |
| dim | 38 | 33 | 5 | 0 | 0 |
| dog | 5 | 5 | 0 | 0 | 0 |
| dt | 20 | 20 | 0 | 0 | 0 |
| edge | 6 | 6 | 0 | 0 | 0 |
| fillh | 5 | 5 | 0 | 0 | 0 |
| fmriprep | 18 | 18 | 0 | 0 | 0 |
| fn | 37 | 37 | 0 | 0 | 0 |
| io | 5 | 2 | 3 | 0 | 0 |
| morph | 24 | 23 | 1 | 0 | 0 |
| norm | 6 | 6 | 0 | 0 | 0 |
| op | 25 | 23 | 2 | 0 | 0 |
| orig | 8 | 7 | 0 | 1 | 0 |
| roc | 8 | 7 | 1 | 0 | 0 |
| roi | 7 | 5 | 2 | 0 | 0 |
| stat | 16 | 16 | 0 | 0 | 0 |
| sub | 7 | 5 | 0 | 2 | 0 |
| tensor | 4 | 3 | 1 | 0 | 0 |
| tfce | 15 | 12 | 3 | 0 | 0 |
| tfceS | 8 | 7 | 1 | 0 | 0 |
| thr | 12 | 10 | 2 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `conv/fft-double` | FSL's single-precision FFT convolution against the original's direct sum (where FSL takes its FFT, its products added in double precision and rounded once): the FFT's rounding is relative to the image's largest values, not to each voxel's own last place; against the exact result (float64) FSL's FFT… |
| `conv/fft-4d` | FSL's single-precision FFT convolution against the original's direct sum (where FSL takes its FFT, its products added in double precision and rounded once): the FFT's rounding is relative to the image's largest values, not to each voxel's own last place; against the exact result (float64) FSL's FFT… |
| `sub/subsamp2offc` | FSL's arithmetic of -subsamp2offc's 2 x 2 x 2 means, which no order of a single or double sum of the eight values gives (findings 13 and 29; spec question 13), against the spec's mean: the voxels beyond 4 ulp are those whose value cancels to near 0. Inside the bound: max \|diff\| 3.95e-08 of max \|FSL… |
| `sub/subsamp2offc-double` | FSL's arithmetic of -subsamp2offc's 2 x 2 x 2 means, which no order of a single or double sum of the eight values gives (findings 13 and 29; spec question 13), against the spec's mean: the voxels beyond 4 ulp are those whose value cancels to near 0. Inside the bound: max \|diff\| 5.52e-17 of max \|FSL… |
| `orig/fft-asym` | FSL's single-precision FFT convolution against the original's direct sum (where FSL takes its FFT, its products added in double precision and rounded once): the FFT's rounding is relative to the image's largest values, not to each voxel's own last place; against the exact result (float64) FSL's FFT… |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `cli/usage` | no arguments: the usage (exit 1) | fslmaths exits 1; the same from the original. |
| `cli/missing-input` | an input that does not exist | fslmaths exits -6; the same from the original. Image Exception : #63 :: No image files match: nothere terminate called after throwing an instance of 'std::runt… |
| `cli/no-output` | no output name | fslmaths exits 1; the same from the original. Error: no output filename specified! |
| `cli/unknown-op` | an unknown operation: the usage | fslmaths exits 1; the same from the original. |
| `cli/odt-unknown` | -odt with an unknown type | fslmaths exits 1; the same from the original. |
| `cli/dt-unknown` | -dt with an unknown type | fslmaths exits 1; the same from the original. |
| `cli/dt-no-input` | -dt float without an input (FSL-158) | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): basic_string: construction from null i… |
| `cli/add-output-image` | -add reading the output's name as an image | fslmaths exits -6; the same from the original. Image Exception : #63 :: No image files match: out Image Exception : #22 :: Failed to read volume out Error : No… |
| `cli/analyze` | FSLOUTPUTTYPE=ANALYZE | fslmaths exits 1; the same from the original. ERROR:: Unrecognised value (ANALYZE) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, N… |
| `cli/outtype-bad` | FSLOUTPUTTYPE unknown | fslmaths exits 1; the same from the original. ERROR:: Unrecognised value (BOGUS) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NIF… |
| `io/truncated-gz` | a gzip stream cut inside the data (FSL's short read) | fslmaths exits -6; the same from the original. Image Exception : #22 :: Failed to read volume trunc.nii.gz Error : Error: short read, file may be truncated ter… |
| `io/truncated-raw` | an uncompressed file cut inside the data | fslmaths exits -6; the same from the original. Image Exception : #22 :: Failed to read volume truncraw.nii Error : Error: short read, file may be truncated ter… |
| `io/huge-header` | a header claiming 1000 x 1000 x 1000 voxels, its file 64 bytes of data | fslmaths exits -6; the same from the original. Image Exception : #22 :: Failed to read volume hugehdr.nii Error : Error: short read, file may be truncated term… |
| `op/4d-by-4d-other-t` | 4D images of 3 and 5 volumes: refused | fslmaths exits -6; the same from the original. Image Exception : #3 :: Attempted to add images of different sizes terminate called after throwing an instance o… |
| `op/size-mismatch` | -add an image of another size: refused | fslmaths exits -6; the same from the original. WARNING:: Inconsistent orientations for individual images in pipeline! Will use voxel-based orientation which is… |
| `thr/thr-word` | -thr without a number | fslmaths exits 1; the same from the original. Error: -thr requires a number |
| `thr/uthrP-word` | -uthrP without a number | fslmaths exits 1; the same from the original. Error: -uthrP requires a number |
| `d13/rem-zero` | FSL-156: -rem 0 (FSL crashes) | fslmaths exits -8; the same from the original. |
| `d13/rem-half` | FSL-156: -rem 0.5 (FSL crashes) | fslmaths exits -8; the same from the original. |
| `d13/rem-image-half` | FSL-156: -rem by an image with a voxel 0.5 | fslmaths exits -8; the same from the original. |
| `d13/div-int-half` | FSL-156: -dt int -div 0.5 (FSL crashes) | fslmaths exits -8; the same from the original. |
| `d13/grid-zero` | FSL-156: -grid with spacing 0 | fslmaths exits -8; the same from the original. |
| `d13/grid-null` | FSL-158: -grid without its spacing (reads the null pointer) | fslmaths exits -11; the same from the original. |
| `conv/kernel-no-output` | -kernel box 3 without an output name | fslmaths exits 1; the same from the original. Error: no output filename specified! |
| `conv/kernel-missing-size` | -kernel box with the output name as its size: no output name left | fslmaths exits 1; the same from the original. Error: no output filename specified! |
| `morph/file-inf2` | -kernel file with an infinite value and none at 0.5: more values than the vector holds | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::out_of_range' what(): Mat::operator(): index out of bounds |
| `d13/tensor-nan` | FSL-166: -tensor_decomp with a NaN tensor | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::out_of_range' what(): Mat::operator(): index out of bounds |
| `tensor/not-a-tensor` | -tensor_decomp of a 3-volume image | fslmaths exits 1; the same from the original. |
| `d13/kernel-boxv3-null` | FSL-158: -kernel boxv3 without its sizes (reads the null pointer) | fslmaths exits -11; the same from the original. |
| `d13/subsamp2-4d` | FSL-163: -subsamp2 of a 4D image | fslmaths exits -6; the same from the original. Image Exception : #75 :: 3D only method called by higher-dimensional volume. terminate called after throwing an … |
| `d13/subsamp2offc-4d-odd` | FSL-163: -subsamp2offc of a 4D image of odd sizes | fslmaths exits -6; the same from the original. Image Exception : #75 :: 3D only method called by higher-dimensional volume. terminate called after throwing an … |
| `d13/fillh-2d` | FSL-165: -fillh of a 2D image | fslmaths exits -8; the same from the original. |
| `d13/fmedian-nan` | FSL-162: -fmedian with NaN voxels | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `d13/dilD-nan` | FSL-162: -dilD with a NaN next to a zero voxel | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `d13/fmean-int-trap` | FSL-156: -kernel sphere 5 -fmean under -dt int (an integer division by zero) | fslmaths exits -8; the same from the original. |
| `dim/unknown` | -Tfoo: an unknown reduction | fslmaths exits 1; the same from the original. Error unknown Dimensionality operation: -Tfoo |
| `dim/empty` | -T alone | fslmaths exits 1; the same from the original. Error unknown Dimensionality operation: -T |
| `dim/perc-range` | -Tperc 150 (newimage's exception) | fslmaths exits -6; the same from the original. Image Exception : #4 :: Percentiles must be in the range [0.0,1.0] terminate called after throwing an instance o… |
| `dim/perc-negative` | -Tperc -5 | fslmaths exits -6; the same from the original. Image Exception : #4 :: Percentiles must be in the range [0.0,1.0] terminate called after throwing an instance o… |
| `dim/perc-no-argument` | -Tperc with the output name as its argument | fslmaths exits 1; the same from the original. Error: no output filename specified! |
| `roi/into-output` | -roi with the output name as its last argument | fslmaths exits 1; the same from the original. Error: no output filename specified! |
| `roi/null` | FSL-158: -roi without its last sizes (reads the null pointer) | fslmaths exits -11; the same from the original. |
| `bptf/into-output` | -bptf with the output name as its second sigma | fslmaths exits 1; the same from the original. Error: no output filename specified! |
| `d13/rank-nan` | FSL-162: -rank of series holding NaN | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `d13/ranknorm-nan` | FSL-162: -ranknorm of series holding NaN | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `d13/bptf-lp-huge` | FSL-171: -bptf -1 2e8 (FSL crashes) | fslmaths exits -11; the same from the original. |
| `d13/bptf-hp-length` | FSL-171: -bptf 4e8 -1 (std::bad_array_new_length) | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::bad_array_new_length' what(): std::bad_array_new_length |
| `d13/bptf-lp-length` | FSL-171: -bptf -1 6e7 (std::bad_array_new_length) | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::bad_array_new_length' what(): std::bad_array_new_length |
| `tfce/small` | -tfce of an image 2 voxels wide (tfce's error) | fslmaths exits 1; the same from the original. ERROR: TFCE failed, please check your file for valid sizes and voxel values.Error: tfce currently requires an inp… |
| `tfce/negative-delta` | -tfce_delta -1 (tfce's error) | fslmaths exits 1; the same from the original. ERROR: TFCE failed, please check your file for valid sizes and voxel values.Error: tfce requires a positive delta… |
| `tfce/into-output` | -tfce with the output name as its connectivity | fslmaths exits 1; the same from the original. Error: no output filename specified! |
| `tfceS/inf` | -tfceS at an infinite voxel (in no cluster: std::out_of_range) | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::out_of_range' what(): Mat::operator(): index out of bounds |
| `d13/tfce-zero` | FSL-175: -tfce of an image of zeros | fslmaths exits 1; the same from the original. ERROR: TFCE failed, please check your file for valid sizes and voxel values.Error: tfce requires a positive delta… |
| `d13/tfce-negative` | FSL-175: -tfce of an image without positive values | fslmaths exits 1; the same from the original. ERROR: TFCE failed, please check your file for valid sizes and voxel values.Error: tfce requires a positive delta… |
| `d13/tfceS-stall` | FSL-174: -tfceS with -tfce_minT -1e9 (std::out_of_range) | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::out_of_range' what(): Mat::operator(): index out of bounds |
| `d13/tfceS-negative-delta` | FSL-174: -tfceS with -tfce_delta -0.5 | fslmaths exits -8; the same from the original. |
| `d13/tfceS-minT` | FSL-177: -tfceS with -tfce_minT -2 (more thresholds than its arrays) | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::out_of_range' what(): Mat::operator(): index out of bounds |
| `roc/truth-size` | -roc with a truth image of another size (newimage's exception) | fslmaths exits -6; the same from the original. Image Exception : #3 :: Attempted to multiply images of different sizes terminate called after throwing an insta… |
| `d13/roc-no-true` | FSL-180: -roc with a truth image without true voxels | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `d13/roc-no-noise` | FSL-180: -roc -0.05 with a truth image without zero voxels | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `difftest/fsl165-no-holes` | -fillh26 of a single slice without holes: FSL-165 (findings 27) | fslmaths exits -8; the same from the original. |
| `difftest/roc-truth-zeros` | -roc with a truth of zeros: no candidate, FSL-180 (findings 29) | fslmaths exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): sort(): detected NaN |
| `difftest/tfce-step-before-size` | -tfce of a thin image without positive values: the step's message first (findings 26) | fslmaths exits 1; the same from the original. ERROR: TFCE failed, please check your file for valid sizes and voxel values.Error: tfce requires a positive delta… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (4182e89066bc) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The cases are the replica suite's (`fsl-maths-replica`); FSL's runs are served from the oracle cache under that suite's keys. The original is compared in `reproduce` mode (D13) and its default `fix` pass against the reproduced run. The goodvoxels chain's ribbon is a stand-in: the segmentation's grey matter (the no-reconall runs have no FreeSurfer ribbon).

Random testing (one trial, 2026-11-03, `<workspace>/oracles/difftest/2026-11-03-fsl-maths-original`: 20 minutes on 13 workers, realistic cases only, FSL through the oracle cache): 150,000 chains of fslmaths operations, 143,191 identical, 804 differing only by a named correction, 6,005 differing. Each of these re-run with the final build and triaged by category (`validation/investigations/cleanroom-maths/trial_triage.json`, one reproducer per category): 4,623 larmorx mistakes fixed (findings 24 to 30 of `docs/findings/fsl-maths-original.md`; their 17 permanent cases in `validation/difftest/cases/fsl-maths-original/`) and 58 chains those fixes reach; 96 corrections of the suite's checks; 1,178 explained (-tensor_decomp of degenerate tensors, the header's offsets after a grid change within 6 units in the last place, -subsamp2's last bits under the floating types, the last bits of Φ and Φ⁻¹ in -ztop, -ptoz and double -randn, -dog_edge's crossings at exact ties, sums cancelling to near 0, signed zeros); 33 expected (-subsamp2offc's means, as this suite's two cases); 7 registered (FSL-75: FSL's heap corruption ends it before it writes; the original names it); **10 open**: 8 display ranges (`cal_min`, `cal_max`) after -range that differ in their last bits (4 under `-dt double`, 1 under `-dt input`, 2 after a sum; e.g. `fsl-maths-trial-41014-original`) or where FSL writes -2147483648 and the original NaN (`fsl-maths-trial-100080-original`), and 2 of -roc's curve numbers 5e-7 and 2.4e-7 from FSL's, beyond half a unit of the 5th digit (`fsl-maths-trial-54988-original`, `fsl-maths-trial-76339-original`); the findings' files are in the trial's `findings/fsl-maths-original/`.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds000005-sub-01-noreconall/sub_01_wf/enhance_and_skullstrip_bold_wf` | pass | 7/7 | fMRIPrep's combine_masks call (fslmaths <mask> -mul <mask> <out>) |
| `fmriprep/ds003592-sub-280-noreconall/sub_280_ses_1_wf/enhance_and_skullstrip_bold_wf` | pass | 7/7 | fMRIPrep's combine_masks call (fslmaths <mask> -mul <mask> <out>) |
| `fmriprep/ds005040-sub-002-noreconall/sub_002_wf/enhance_and_skullstrip_bold_wf` | pass | 7/7 | fMRIPrep's combine_masks call (fslmaths <mask> -mul <mask> <out>) |
| `fmriprep/ds005040-sub-002-noreconall/sub_002_wf/skullstrip_bold_wf` | pass | 7/7 | fMRIPrep's combine_masks call (fslmaths <mask> -mul <mask> <out>) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/stdev_volume` | pass | 7/7 | fMRIPrep's goodvoxels mask, node stdev_volume (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/mean_volume` | pass | 7/7 | fMRIPrep's goodvoxels mask, node mean_volume (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_volume` | pass | 7/7 | fMRIPrep's goodvoxels mask, node cov_volume (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_ribbon` | pass | 7/7 | fMRIPrep's goodvoxels mask, node cov_ribbon (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_ribbon_norm` | pass | 7/7 | fMRIPrep's goodvoxels mask, node cov_ribbon_norm (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/smooth_norm` | pass | 7/7 | fMRIPrep's goodvoxels mask, node smooth_norm (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_ribbon_norm_smooth` | pass | 7/7 | fMRIPrep's goodvoxels mask, node cov_ribbon_norm_smooth (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_norm` | pass | 7/7 | fMRIPrep's goodvoxels mask, node cov_norm (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_norm_modulate` | pass | 7/7 | fMRIPrep's goodvoxels mask, node cov_norm_modulate (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_norm_modulate_ribbon` | pass | 7/7 | fMRIPrep's goodvoxels mask, node cov_norm_modulate_ribbon (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/bin_mean_volume` | pass | 7/7 | fMRIPrep's goodvoxels mask, node bin_mean_volume (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/goodvoxels_thr` | pass | 7/7 | fMRIPrep's goodvoxels mask, node goodvoxels_thr (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/goodvoxels_mask` | pass | 7/7 | fMRIPrep's goodvoxels mask, node goodvoxels_mask (--project-goodvoxels) |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/goodvoxels_ribbon_mask` | pass | 7/7 | fMRIPrep's goodvoxels mask, node goodvoxels_ribbon_mask (--project-goodvoxels) |
| `cli/usage` | both-error | 4/4 | no arguments: the usage (exit 1) |
| `cli/help` | pass | 5/5 | -h: the usage (exit 0) |
| `cli/help-long` | pass | 5/5 | --help |
| `cli/missing-input` | both-error | 4/4 | an input that does not exist |
| `cli/no-output` | both-error | 4/4 | no output name |
| `cli/copy` | pass | 7/7 | no operation: a copy (the display range kept) |
| `cli/unknown-op` | both-error | 4/4 | an unknown operation: the usage |
| `cli/odt-unknown` | both-error | 4/4 | -odt with an unknown type |
| `cli/dt-unknown` | both-error | 4/4 | -dt with an unknown type |
| `cli/dt-no-input` | both-error | 4/4 | -dt float without an input (FSL-158) |
| `cli/op-as-output` | pass | 7/7 | an operation in the output's place |
| `cli/add-output-image` | both-error | 4/4 | -add reading the output's name as an image |
| `cli/chain` | pass | 7/7 | a chain of operations |
| `cli/datatype-long` | pass | 7/7 | -datatype and -output_datatype |
| `cli/save-restart` | pass | 9/9 | -save mid-chain and -restart |
| `cli/range` | pass | 7/7 | -range: the display range of the result |
| `cli/nifti` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI |
| `cli/pair` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI_PAIR, a pair as input |
| `cli/nifti2` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `cli/analyze` | both-error | 4/4 | FSLOUTPUTTYPE=ANALYZE |
| `cli/outtype-bad` | both-error | 4/4 | FSLOUTPUTTYPE unknown |
| `cli/nii-input` | pass | 7/7 | an uncompressed input |
| `io/truncated-gz` | both-error | 4/4 | a gzip stream cut inside the data (FSL's short read) |
| `io/truncated-raw` | both-error | 4/4 | an uncompressed file cut inside the data |
| `io/huge-header` | both-error | 4/4 | a header claiming 1000 x 1000 x 1000 voxels, its file 64 bytes of data |
| `io/gap-raw` | pass | 7/7 | the data after a gap (vox_offset 1024), uncompressed |
| `io/gap-gz` | pass | 7/7 | the data after a gap (vox_offset 1024), gzip-compressed |
| `cli/neuro` | pass | 7/7 | a neurological input (swapped in memory, back on output) |
| `cli/noform` | pass | 7/7 | an input without sform and qform |
| `dt/char` | pass | 7/7 | -dt char on a uint8 image |
| `dt/short` | pass | 7/7 | -dt short (wrap-around) |
| `dt/int` | pass | 7/7 | -dt int |
| `dt/double` | pass | 7/7 | -dt double |
| `dt/float` | pass | 7/7 | -dt float |
| `dt/input-short` | pass | 7/7 | -dt input on int16 |
| `dt/input-char` | pass | 7/7 | -dt input on uint8 |
| `dt/input-uint16` | pass | 5/5 | -dt input on uint16 (FSL-155) |
| `dt/input-int8` | pass | 5/5 | -dt input on int8 (FSL-155) |
| `dt/double-default` | pass | 7/7 | a double input: computed and written in double |
| `dt/scaled` | pass | 7/7 | a scaled int16 input (read as float) |
| `dt/odt-char` | pass | 7/7 | -odt char (rounded; values out of range: FSL-151) |
| `dt/odt-short` | pass | 7/7 | -odt short |
| `dt/odt-int` | pass | 7/7 | -odt int |
| `dt/odt-double` | pass | 7/7 | -odt double |
| `dt/odt-input` | pass | 7/7 | -odt input on int16 |
| `dt/odt-input-uint16` | pass | 7/7 | -odt input on uint16 |
| `d13/char-uint8-thr` | pass | 7/7 | FSL-154: -dt char on a uint8 image, -thr 100 (values 128-255 read as negative) |
| `d13/input-uint8-div` | pass | 7/7 | FSL-154: -dt input on a uint8 image, -div 2, written as float |
| `d13/odt-input-int8` | pass | 7/7 | FSL-157: -odt input on an int8 image (truncated, written as uint8) |
| `d13/odt-input-uint16-round` | pass | 7/7 | FSL-157: -odt input on a uint16 image, -mul 0.5 (truncated, not rounded) |
| `dt/odt-nan` | pass | 7/7 | -odt short with NaN and infinite voxels (FSL-151) |
| `dt/int-overflow` | pass | 7/7 | -dt int with a number beyond int (FSL-151) |
| `dt/int-exp` | pass | 7/7 | -dt short -exp (results beyond short: FSL-151) |
| `op/add-number` | pass | 7/7 | -add a number |
| `op/sub-number` | pass | 7/7 | -sub a number |
| `op/mul-number` | pass | 7/7 | -mul a number |
| `op/div-number` | pass | 7/7 | -div a number |
| `op/div-zero` | pass | 7/7 | -div 0: nothing done |
| `op/add-image` | pass | 7/7 | -add an image |
| `op/sub-image` | pass | 7/7 | -sub an image |
| `op/mul-image` | pass | 7/7 | -mul an image |
| `op/div-image` | pass | 7/7 | -div an image with zeros (0 there) |
| `op/3d-by-4d` | pass | 7/7 | a 3D image times a 4D one: cloned over time |
| `op/4d-by-3d` | pass | 7/7 | a 4D image times a 3D one |
| `op/4d-by-4d-other-t` | both-error | 4/4 | 4D images of 3 and 5 volumes: refused |
| `op/size-mismatch` | both-error | 4/4 | -add an image of another size: refused |
| `op/row` | pass | 7/7 | -add a 7x1x1 image (newimage compares the first dimension only) |
| `op/orientation` | pass | 7/7 | -add an image of another orientation: the warning |
| `op/mas` | pass | 7/7 | -mas a uint8 mask |
| `op/mas-4d` | pass | 7/7 | -mas a 4D mask on a 3D image |
| `op/mas-inf` | pass | 7/7 | -mas: infinite voxels outside the mask |
| `op/rem-number` | pass | 7/7 | -rem a number |
| `op/rem-image` | pass | 7/7 | -rem an image (0 voxels skipped) |
| `op/max-number` | pass | 7/7 | -max a number |
| `op/min-number` | pass | 7/7 | -min a number |
| `op/max-image` | pass | 7/7 | -max an image |
| `op/min-image-4d` | pass | 7/7 | -min a 4D image |
| `op/nan-number` | pass | 7/7 | -add nan |
| `thr/thr` | pass | 7/7 | -thr |
| `thr/uthr` | pass | 7/7 | -uthr |
| `thr/thrp` | pass | 7/7 | -thrp: the robust range |
| `thr/thrP` | pass | 7/7 | -thrP: the positive voxels' robust range |
| `thr/uthrp` | pass | 7/7 | -uthrp |
| `thr/uthrP` | pass | 7/7 | -uthrP |
| `thr/thrP-empty` | pass | 7/7 | -thrP of an image without positive voxels: the warnings |
| `d13/thrp-far` | pass | 7/7 | FSL-187: -thrp 50 of 1000 values on [0, 1) and one at 1e12 (the robust range) |
| `d13/uthrP-far` | pass | 7/7 | FSL-187: -uthrP 98 of the same image (the positive voxels' robust range) |
| `thr/thr-word` | both-error | 4/4 | -thr without a number |
| `thr/uthrP-word` | both-error | 4/4 | -uthrP without a number |
| `thr/thr-short` | pass | 7/7 | -thr under -dt short |
| `thr/thr-4d` | pass | 7/7 | -thrp of a 4D image |
| `thr/thr-inf` | pass | 7/7 | -thr with infinite voxels |
| `fn/exp` | pass | 7/7 | -exp |
| `fn/log` | pass | 7/7 | -log |
| `fn/sin` | pass | 7/7 | -sin |
| `fn/cos` | pass | 7/7 | -cos |
| `fn/tan` | pass | 7/7 | -tan |
| `fn/asin` | pass | 7/7 | -asin (NaN outside [-1, 1]) |
| `fn/acos` | pass | 7/7 | -acos |
| `fn/atan` | pass | 7/7 | -atan |
| `fn/sqr` | pass | 7/7 | -sqr |
| `fn/sqrt` | pass | 7/7 | -sqrt (0 for non-positive values) |
| `fn/recip` | pass | 7/7 | -recip |
| `fn/abs` | pass | 7/7 | -abs |
| `fn/sin-inf` | pass | 7/7 | -sin of infinite and NaN voxels |
| `fn/tan-inf` | pass | 7/7 | -tan of infinite and NaN voxels |
| `fn/exp-inf` | pass | 7/7 | -exp of infinite and NaN voxels |
| `fn/log-inf` | pass | 7/7 | -log of infinite voxels |
| `fn/double-sin` | pass | 7/7 | -sin in double |
| `fn/double-cos` | pass | 7/7 | -cos in double |
| `fn/double-tan` | pass | 7/7 | -tan in double |
| `fn/double-atan` | pass | 7/7 | -atan in double |
| `fn/double-asin` | pass | 7/7 | -asin and -acos in double |
| `fn/double-exp-log` | pass | 7/7 | -exp, -log and -pow in double |
| `fn/bin` | pass | 7/7 | -bin |
| `fn/binv` | pass | 7/7 | -binv |
| `fn/nan` | pass | 7/7 | -nan |
| `fn/nanm` | pass | 7/7 | -nanm |
| `fn/index` | pass | 7/7 | -index |
| `fn/grid` | pass | 7/7 | -grid |
| `fn/rand` | pass | 7/7 | -rand (fixed initial state) |
| `fn/randn` | pass | 7/7 | -randn |
| `fn/seed-rand` | pass | 7/7 | -seed then -rand |
| `fn/seed-randn` | pass | 7/7 | -seed then -randn |
| `fn/pow` | pass | 7/7 | -pow (undocumented) |
| `fn/invbin` | pass | 7/7 | -invbin (undocumented) |
| `fn/int-recip` | pass | 7/7 | -recip under -dt int |
| `fn/int-sqrt` | pass | 7/7 | -sqrt under -dt short |
| `fn/char-index` | pass | 7/7 | -index under -dt char (wraps) |
| `d13/bin-large` | pass | 7/7 | FSL-152: -bin of values from 2^24 |
| `d13/bin-char` | pass | 7/7 | FSL-152: -bin under -dt char (max 127 + 1 wraps) |
| `d13/thr-inf` | pass | 7/7 | FSL-152: -thr when the maximum is infinite |
| `d13/uthr-char` | pass | 7/7 | FSL-152: -uthr under -dt char (min - 1 wraps) |
| `d13/nan-thr` | pass | 7/7 | FSL-153: -thr with a NaN first voxel |
| `d13/nan-bin` | pass | 7/7 | FSL-153: -bin with a NaN first voxel |
| `d13/nan-mas` | pass | 7/7 | FSL-153: -mas by a mask with a NaN first voxel |
| `d13/nan-range` | pass | 7/7 | FSL-153: -range with a NaN first voxel |
| `d13/nan-thrp` | pass | 7/7 | FSL-153: -thrp with NaN voxels |
| `d13/rem-zero` | both-error | 4/4 | FSL-156: -rem 0 (FSL crashes) |
| `d13/rem-half` | both-error | 4/4 | FSL-156: -rem 0.5 (FSL crashes) |
| `d13/rem-image-half` | both-error | 4/4 | FSL-156: -rem by an image with a voxel 0.5 |
| `d13/div-int-half` | both-error | 4/4 | FSL-156: -dt int -div 0.5 (FSL crashes) |
| `d13/grid-zero` | both-error | 4/4 | FSL-156: -grid with spacing 0 |
| `d13/grid-null` | both-error | 4/4 | FSL-158: -grid without its spacing (reads the null pointer) |
| `d13/div-other` | pass | 7/7 | FSL-159: -div by an image of another size |
| `d13/rem-other` | pass | 7/7 | FSL-159: -rem by a smaller image |
| `d13/max-other` | pass | 7/7 | FSL-159: -max with a larger image |
| `d13/min-small` | pass | 2/2 | FSL-159: -min with a smaller image (FSL reads past its end) |
| `morph/dilF` | pass | 7/7 | -dilF (maximum filter, default kernel) |
| `morph/dilF-nan` | pass | 7/7 | -dilF with NaN voxels (left out) |
| `morph/dilM` | pass | 7/7 | -dilM (mean dilation) |
| `morph/dilM-nan` | pass | 7/7 | -dilM with NaN neighbours |
| `morph/dilD` | pass | 7/7 | -dilD (modal dilation) |
| `morph/dilD-short` | pass | 7/7 | -dilD of labels under -dt short |
| `morph/ero` | pass | 7/7 | -ero (zeroing next to zeros) |
| `morph/eroF` | pass | 7/7 | -eroF (minimum filter) |
| `morph/eroF-nan` | pass | 7/7 | -eroF with NaN voxels |
| `morph/fmedian` | pass | 7/7 | -fmedian |
| `morph/fmedian-2D` | pass | 7/7 | -kernel 2D -fmedian |
| `morph/fmedian-char` | pass | 7/7 | -fmedian under -dt char |
| `morph/eroF-double` | pass | 7/7 | -eroF of a double image |
| `morph/sphere-4d` | pass | 7/7 | -kernel sphere 4 -dilF on each volume of a 4D image |
| `morph/boxv-even` | pass | 7/7 | -kernel boxv 2: the off-centre warning |
| `morph/boxv3` | pass | 7/7 | -kernel boxv3 3 1 5 -fmedian |
| `morph/box-mm` | pass | 7/7 | -kernel box 4 (mm, anisotropic voxels) -eroF |
| `morph/file` | pass | 7/7 | -kernel file (values around 0.5) -dilF |
| `morph/file-even` | pass | 7/7 | -kernel file of even size -dilM |
| `morph/file-4d` | pass | 7/7 | -kernel file of a 4D image (the first volume, a warning) |
| `morph/file-inf` | pass | 7/7 | -kernel file with an infinite value (Armadillo's bounds check) |
| `morph/2d-ero` | pass | 7/7 | -kernel 2D -ero of a 2D image |
| `morph/2d-dilD` | pass | 7/7 | -dilD of a 2D image |
| `dilall/3d` | pass | 7/7 | -dilall |
| `dilall/4d` | pass | 7/7 | -thr 10 -dilall of a 4D image |
| `dilall/int` | pass | 7/7 | -dilall under -dt int |
| `dilall/nan` | pass | 7/7 | -dilall with NaN voxels |
| `conv/s` | pass | 7/7 | -s 2 (separable Gaussian) |
| `conv/s-odd` | pass | 7/7 | -s 1.5 of an odd, anisotropic image |
| `conv/s-4d` | pass | 7/7 | -s 1 of a 4D image |
| `conv/s-2d` | pass | 7/7 | -s 2 of a 2D image |
| `conv/s-nan` | pass | 7/7 | -s 1 with NaN voxels |
| `conv/fmean-default` | pass | 7/7 | -fmean with the default kernel (not separable) |
| `conv/fmean-box` | pass | 7/7 | -kernel box 5 -fmean (separable) |
| `conv/fmean-boxv-even` | pass | 7/7 | -kernel boxv 4 -fmean (separable, even: warnings) |
| `conv/fmean-sphere` | pass | 7/7 | -kernel sphere 3 -fmean (direct sum) |
| `conv/fmean-sphere-nan` | pass | 7/7 | -kernel sphere 4 -fmean with NaN voxels |
| `conv/fmean-double` | pass | 7/7 | -kernel sphere 4 -fmean of a double image |
| `conv/fmean-file` | pass | 7/7 | -kernel file -fmean (not separable) |
| `conv/fmean-file-even` | pass | 7/7 | -kernel file of even size -fmean (warnings) |
| `conv/fmean-2D` | pass | 7/7 | -kernel 2D -fmean |
| `conv/separable-kept` | pass | 7/7 | -kernel gauss 1 -kernel 3D -fmean (separable kept from gauss) |
| `conv/fmeanu` | pass | 7/7 | -kernel sphere 5 -fmeanu |
| `conv/fmeanu-gauss` | pass | 7/7 | -kernel gauss 2 -fmeanu (not separable) |
| `conv/fmeanu-char` | pass | 7/7 | -kernel sphere 3 -fmeanu under -dt char |
| `conv/fmean-int` | pass | 7/7 | -kernel boxv 3 -fmean under -dt int (separable) |
| `conv/fft-sphere` | pass | 8/8 | -kernel sphere 12 -fmean (the FFT path: radices 2, 3, 11, 13) |
| `conv/fft-box` | pass | 8/8 | -kernel box 24 -fmeanu (the FFT path) |
| `conv/fft-even` | pass | 8/8 | -kernel boxv 12 -fmeanu (the FFT path, an even kernel: radix 4) |
| `conv/fft-odd` | pass | 8/8 | -kernel sphere 10 -fmean of an odd, anisotropic image (the FFT path: radix 5) |
| `conv/fft-double` | expected-divergence | 7/8 | -kernel sphere 12 -fmean of a double image (the FFT path) |
| `conv/fft-4d` | expected-divergence | 7/8 | -kernel sphere 12 -fmeanu of a 4D image (the FFT path) |
| `conv/fft-int` | pass | 7/7 | -kernel sphere 12 -fmeanu under -dt int (the FFT path) |
| `conv/fft-nan` | pass | 7/7 | -kernel sphere 12 -fmeanu with NaN voxels (the FFT path) |
| `conv/kernel-no-output` | both-error | 4/4 | -kernel box 3 without an output name |
| `conv/kernel-missing-size` | both-error | 4/4 | -kernel box with the output name as its size: no output name left |
| `morph/file-inf2` | both-error | 4/4 | -kernel file with an infinite value and none at 0.5: more values than the vector holds |
| `sub/subsamp2` | pass | 8/8 | -subsamp2 |
| `sub/subsamp2offc` | expected-divergence | 7/8 | -subsamp2offc |
| `sub/subsamp2-short` | pass | 7/7 | -subsamp2 under -dt short |
| `sub/subsamp2offc-int` | pass | 7/7 | -subsamp2offc under -dt int |
| `sub/subsamp2offc-double` | expected-divergence | 7/8 | -subsamp2offc of a double image |
| `sub/subsamp2-noform` | pass | 8/8 | -subsamp2 without sform or qform |
| `sub/subsamp2-nan` | pass | 8/8 | -subsamp2 with NaN voxels |
| `edge/3d` | pass | 7/7 | -edge |
| `edge/4d` | pass | 7/7 | -edge of a 4D image |
| `edge/2d` | pass | 7/7 | -edge of a 2D image |
| `edge/short` | pass | 7/7 | -edge under -dt short |
| `edge/nan` | pass | 7/7 | -edge with NaN voxels |
| `edge/aniso` | pass | 7/7 | -edge with anisotropic voxels |
| `dog/3d` | pass | 7/7 | -dog_edge 1 1.6 |
| `dog/aniso` | pass | 7/7 | -dog_edge 0.5 2 of an odd image |
| `dog/double` | pass | 7/7 | -dog_edge of a double image |
| `dog/int` | pass | 7/7 | -dog_edge under -dt int |
| `dog/radius0` | pass | 7/7 | -dog_edge 0.1 0.2 (radius 0) |
| `fillh/6` | pass | 7/7 | -fillh |
| `fillh/26` | pass | 7/7 | -fillh26 |
| `fillh/blob` | pass | 7/7 | -fillh of a blob at the edge |
| `fillh/short` | pass | 7/7 | -fillh under -dt short |
| `fillh/4d` | pass | 7/7 | -fillh26 of a 4D mask |
| `norm/inm` | pass | 7/7 | -inm 100 of a 4D image |
| `norm/ing` | pass | 7/7 | -ing 100 of a 4D image |
| `norm/inm-sparse` | pass | 7/7 | -inm 100 of a sparse image |
| `norm/inm-empty` | pass | 7/7 | -inm of an image of zeros (an empty mask: NaN) |
| `norm/ing-int` | pass | 7/7 | -ing under -dt int |
| `norm/inm-nan` | pass | 7/7 | -inm with NaN voxels |
| `tensor/decomp` | pass | 25/25 | -tensor_decomp |
| `tensor/chain` | pass | 25/25 | -tensor_decomp, then the FA image multiplied by 2 |
| `tensor/double` | pass | 25/25 | -tensor_decomp under -dt double, an output name with its extension |
| `d13/tensor-nan` | both-error | 4/4 | FSL-166: -tensor_decomp with a NaN tensor |
| `tensor/not-a-tensor` | both-error | 4/4 | -tensor_decomp of a 3-volume image |
| `d13/kernel-boxv3-null` | both-error | 4/4 | FSL-158: -kernel boxv3 without its sizes (reads the null pointer) |
| `d13/subsamp2-4d` | both-error | 4/4 | FSL-163: -subsamp2 of a 4D image |
| `d13/subsamp2offc-4d-odd` | both-error | 4/4 | FSL-163: -subsamp2offc of a 4D image of odd sizes |
| `d13/subsamp2offc-4d-even` | pass | 8/8 | FSL-163: -subsamp2offc of a 4D image of even sizes (the first volume) |
| `d13/dog-4d` | pass | 7/7 | FSL-164: -dog_edge of a 4D image |
| `d13/dog-negative` | pass | 7/7 | FSL-161: -dog_edge -1 -2 (a kernel without voxels) |
| `d13/fillh-2d` | both-error | 4/4 | FSL-165: -fillh of a 2D image |
| `d13/s-zero` | pass | 7/7 | FSL-160: -s 0 |
| `d13/gauss-zero` | pass | 7/7 | FSL-160: -kernel gauss 0 -fmean |
| `d13/gauss-zero-dilF` | pass | 7/7 | FSL-160: -kernel gauss 0 -dilF (no change: no notice) |
| `d13/kernel-boxv0` | pass | 7/7 | FSL-161: -kernel boxv 0 -dilF |
| `d13/kernel-negative` | pass | 7/7 | FSL-161: -kernel boxv -3 -dilF |
| `d13/kernel-unknown` | pass | 7/7 | FSL-161: -kernel foo 3 -fmean |
| `d13/kernel-unknown-dilM` | pass | 7/7 | FSL-161: -kernel foo 3 -dilM |
| `d13/gauss-negative` | pass | 7/7 | FSL-161: -kernel gauss -1 -fmean |
| `d13/s-negative` | pass | 7/7 | FSL-161: -s -1 |
| `d13/fmedian-nan` | both-error | 4/4 | FSL-162: -fmedian with NaN voxels |
| `d13/dilD-nan` | both-error | 4/4 | FSL-162: -dilD with a NaN next to a zero voxel |
| `d13/fmean-int-trap` | both-error | 4/4 | FSL-156: -kernel sphere 5 -fmean under -dt int (an integer division by zero) |
| `dim/Tmean` | pass | 7/7 | -Tmean |
| `dim/Tstd` | pass | 7/7 | -Tstd |
| `dim/Tmax` | pass | 7/7 | -Tmax |
| `dim/Tmin` | pass | 7/7 | -Tmin |
| `dim/Tmaxn` | pass | 7/7 | -Tmaxn (ties: the first index) |
| `dim/Tmedian` | pass | 7/7 | -Tmedian (ties, signed zeros) |
| `dim/Tperc` | pass | 7/7 | -Tperc 30 |
| `dim/Tperc-0` | pass | 7/7 | -Tperc 0 |
| `dim/Tperc-100` | pass | 7/7 | -Tperc 100 |
| `dim/Tar1` | pass | 7/7 | -Tar1 |
| `dim/Tmean-nan` | pass | 7/7 | -Tmean of series holding NaN |
| `dim/Tar1-nan` | pass | 7/7 | -Tar1 of series holding NaN |
| `dim/Xmean` | pass | 7/7 | -Xmean |
| `dim/Ymax` | pass | 7/7 | -Ymax |
| `dim/Zstd` | pass | 7/7 | -Zstd |
| `dim/Xmaxn` | pass | 7/7 | -Xmaxn |
| `dim/Ymedian` | pass | 7/7 | -Ymedian |
| `dim/Zperc` | pass | 7/7 | -Zperc 75 |
| `dim/Xar1` | pass | 7/7 | -Xar1 |
| `dim/Ymin` | pass | 7/7 | -Ymin |
| `dim/Smean` | pass | 7/7 | -Smean (a capital that reduces nothing) |
| `dim/chain` | pass | 7/7 | -Tmean then -Xmax then -add |
| `dim/unknown` | both-error | 4/4 | -Tfoo: an unknown reduction |
| `dim/empty` | both-error | 4/4 | -T alone |
| `dim/perc-range` | both-error | 4/4 | -Tperc 150 (newimage's exception) |
| `dim/perc-negative` | both-error | 4/4 | -Tperc -5 |
| `dim/perc-nan` | pass | 7/7 | -Tperc nan |
| `dim/perc-no-argument` | both-error | 4/4 | -Tperc with the output name as its argument |
| `dim/3d-mean` | pass | 7/7 | -Tmean of a 3D image |
| `dim/Tmean-short` | pass | 7/7 | -Tmean under -dt short |
| `dim/Tstd-int` | pass | 7/7 | -Tstd under -dt int |
| `dim/Tar1-short` | pass | 7/7 | -Tar1 under -dt short |
| `dim/Tar1-int-large` | pass | 7/7 | -Tar1 under -dt int with products beyond int's range |
| `dim/Tmedian-int` | pass | 7/7 | -Tmedian under -dt int |
| `dim/Tmaxn-short` | pass | 7/7 | -Tmaxn under -dt short |
| `dim/Tstd-double` | pass | 7/7 | -Tstd of a double image |
| `dim/Tmedian-double` | pass | 7/7 | -Tmedian of a double image |
| `dim/Tmean-long` | pass | 7/7 | -Tmean of 48 time points |
| `stat/pval` | pass | 7/7 | -pval |
| `stat/pval0` | pass | 7/7 | -pval0 (zeros left out) |
| `stat/cpval` | pass | 7/7 | -cpval |
| `stat/cpval-double` | pass | 7/7 | -cpval of a double image |
| `stat/cpval-int` | pass | 7/7 | -cpval under -dt int |
| `stat/pval-short` | pass | 7/7 | -pval under -dt short |
| `stat/pval-3d` | pass | 7/7 | -pval of a 3D image |
| `stat/ztop` | pass | 7/7 | -ztop (with +-40 and NaN) |
| `stat/ztop-double` | pass | 8/8 | -ztop in double |
| `stat/ptoz` | pass | 7/7 | -ptoz (with 0, 1, outside [0, 1] and NaN) |
| `stat/ztop-ptoz` | pass | 7/7 | -ztop then -ptoz |
| `stat/rank` | pass | 7/7 | -rank (ties, signed zeros) |
| `stat/ranknorm` | pass | 7/7 | -ranknorm |
| `stat/rank-short` | pass | 7/7 | -rank under -dt short |
| `stat/rank-3d` | pass | 7/7 | -rank of a 3D image |
| `stat/ranknorm-double` | pass | 8/8 | -ranknorm of a double image |
| `roi/box` | pass | 7/7 | -roi of a box |
| `roi/full` | pass | 7/7 | -roi with every size -1 |
| `roi/neuro` | pass | 7/7 | -roi of a neurological image (x mapped to newimage's order) |
| `roi/outside` | pass | 7/7 | -roi of a box outside the image |
| `roi/negative-size` | pass | 7/7 | -roi with a size of -3 |
| `roi/into-output` | both-error | 4/4 | -roi with the output name as its last argument |
| `roi/null` | both-error | 4/4 | FSL-158: -roi without its last sizes (reads the null pointer) |
| `bptf/hp` | pass | 7/7 | -bptf 2 -1 (high-pass) |
| `bptf/lp` | pass | 7/7 | -bptf -1 1.5 (low-pass) |
| `bptf/both` | pass | 7/7 | -bptf 3 0.8 |
| `bptf/none` | pass | 7/7 | -bptf 0 0 |
| `bptf/nan` | pass | 7/7 | -bptf nan nan (both skipped) |
| `bptf/wide` | pass | 7/7 | -bptf 100 50 (wider than the series) |
| `bptf/3d` | pass | 7/7 | -bptf of a 3D image |
| `bptf/long` | pass | 7/7 | -bptf 5 2 of 48 time points |
| `bptf/short` | pass | 7/7 | -bptf under -dt short |
| `bptf/double` | pass | 7/7 | -bptf of a double image |
| `bptf/nan-values` | pass | 7/7 | -bptf of series holding NaN |
| `bptf/into-output` | both-error | 4/4 | -bptf with the output name as its second sigma |
| `d13/Tmax-nan` | pass | 7/7 | FSL-153: -Tmax with a NaN first value |
| `d13/Tmin-nan` | pass | 7/7 | FSL-153: -Tmin with a NaN first value |
| `d13/Tmaxn-nan` | pass | 7/7 | FSL-153: -Tmaxn with a NaN first value |
| `d13/Tmedian-nan` | pass | 7/7 | FSL-168: -Tmedian of values holding NaN |
| `d13/Tperc-nan` | pass | 7/7 | FSL-168: -Tperc 80 of values holding NaN |
| `d13/Tstd-3d` | pass | 7/7 | FSL-167: -Tstd of a 3D image |
| `d13/Tstd-negative` | pass | 7/7 | FSL-167: -Tstd of series whose variance rounds below 0 |
| `d13/ranknorm-negative` | pass | 7/7 | FSL-167: -ranknorm of series whose variance rounds below 0 |
| `d13/ranknorm-3d` | pass | 7/7 | FSL-167: -ranknorm of a 3D image |
| `d13/rank-nan` | both-error | 4/4 | FSL-162: -rank of series holding NaN |
| `d13/ranknorm-nan` | both-error | 4/4 | FSL-162: -ranknorm of series holding NaN |
| `d13/pval-nan` | pass | 7/7 | FSL-169: -pval with NaN statistics |
| `d13/pval0-nan` | pass | 7/7 | FSL-169: -pval0 with NaN statistics |
| `d13/cpval-nan` | pass | 7/7 | FSL-169: -cpval with NaN statistics |
| `d13/ztop-inf` | pass | 7/7 | FSL-170: -ztop of infinite z |
| `d13/ptoz-tiny` | pass | 7/7 | FSL-172: -ptoz of p-values whose 1 - p rounds to 1 |
| `d13/bptf-lp-huge` | both-error | 4/4 | FSL-171: -bptf -1 2e8 (FSL crashes) |
| `d13/bptf-hp-length` | both-error | 4/4 | FSL-171: -bptf 4e8 -1 (std::bad_array_new_length) |
| `d13/bptf-lp-length` | both-error | 4/4 | FSL-171: -bptf -1 6e7 (std::bad_array_new_length) |
| `d13/bptf-hp-inf` | pass | 2/2 | FSL-171: -bptf inf -1 (FSL writes outside its array) |
| `tfce/basic` | pass | 7/7 | -tfce 2 0.5 6 |
| `tfce/conn18` | pass | 7/7 | -tfce 2 0.5 18 |
| `tfce/conn26` | pass | 7/7 | -tfce 2 0.5 26 |
| `tfce/minT` | pass | 7/7 | -tfce_minT 1 -tfce 2 0.5 6 |
| `tfce/delta` | pass | 7/7 | -tfce_delta 0.3 -tfce 1.5 0.66 26 |
| `tfce/negative-minT` | pass | 7/7 | -tfce_minT -1 -tfce 0.5 0.5 6 (powf of negative heights: NaN) |
| `tfce/many-steps` | pass | 7/7 | -tfce_delta 0.0004: the warning of many integral steps |
| `tfce/4d` | pass | 7/7 | -tfce of each volume of a 4D image |
| `tfce/double` | pass | 7/7 | -tfce of a double image |
| `tfce/short` | pass | 7/7 | -tfce under -dt short |
| `tfce/inf` | pass | 7/7 | -tfce with an infinite maximum |
| `tfce/small` | both-error | 4/4 | -tfce of an image 2 voxels wide (tfce's error) |
| `tfce/negative-delta` | both-error | 4/4 | -tfce_delta -1 (tfce's error) |
| `tfce/into-output` | both-error | 4/4 | -tfce with the output name as its connectivity |
| `tfce/chain` | pass | 7/7 | -tfce then -mul (the options kept between operations) |
| `tfceS/basic` | pass | 7/7 | -tfceS 2 0.5 6 3 3 3 30 |
| `tfceS/conn26` | pass | 7/7 | -tfceS 2 0.5 26 7 6 5 20 |
| `tfceS/not-reached` | pass | 7/7 | -tfceS with a threshold its sum never reaches (the message) |
| `tfceS/delta` | pass | 7/7 | -tfce_delta 0.35 -tfceS (the arrays sized from the step) |
| `tfceS/4d` | pass | 7/7 | -tfceS of each volume of a 4D image |
| `tfceS/double` | pass | 7/7 | -tfceS of a double image |
| `tfceS/low-voxel` | pass | 7/7 | -tfceS at a voxel without positive value |
| `tfceS/inf` | both-error | 4/4 | -tfceS at an infinite voxel (in no cluster: std::out_of_range) |
| `d13/tfce-zero` | both-error | 4/4 | FSL-175: -tfce of an image of zeros |
| `d13/tfce-negative` | both-error | 4/4 | FSL-175: -tfce of an image without positive values |
| `d13/tfce-nan` | pass | 7/7 | FSL-153: -tfce with a NaN first voxel |
| `d13/tfce-conn7` | pass | 7/7 | FSL-176: -tfce 2 0.5 7 |
| `d13/tfceS-conn7` | pass | 7/7 | FSL-176: -tfceS with connectivity 7 |
| `d13/tfce-delta-nan` | pass | 7/7 | FSL-174: -tfce_delta nan (FSL: zeros) |
| `d13/tfce-stall` | pass | 2/2 | FSL-174: -tfce_minT -1e9 (the thresholds never advance: FSL never ends) |
| `d13/tfceS-stall` | both-error | 4/4 | FSL-174: -tfceS with -tfce_minT -1e9 (std::out_of_range) |
| `d13/tfceS-negative-delta` | both-error | 4/4 | FSL-174: -tfceS with -tfce_delta -0.5 |
| `d13/tfceS-minT` | both-error | 4/4 | FSL-177: -tfceS with -tfce_minT -2 (more thresholds than its arrays) |
| `d13/tfceS-outside` | pass | 7/7 | FSL-178: -tfceS at x = 10 of 10 (FSL reads the next row's first voxel) |
| `d13/tfceS-far` | pass | 2/2 | FSL-178: -tfceS at a voxel far outside (FSL reads outside the image) |
| `roc/noise` | pass | 8/8 | -roc 0.05 with a noise image (the FWE rate) |
| `roc/truth` | pass | 8/8 | -roc -0.05: the FP rate from the truth's zero voxels (negative ones left out) |
| `roc/never-reached` | pass | 8/8 | -roc 2 (the FP rate never reaches it) |
| `roc/3d` | pass | 8/8 | -roc of a 3D image |
| `roc/double` | pass | 8/8 | -roc of a double image (log in double) |
| `roc/short` | pass | 8/8 | -roc under -dt short |
| `roc/zero` | pass | 8/8 | -roc 0 (0 / 0 printed) |
| `roc/truth-size` | both-error | 5/5 | -roc with a truth image of another size (newimage's exception) |
| `d13/roc-nan` | pass | 8/8 | FSL-153: -roc of an image whose first voxel is NaN |
| `d13/roc-noise-larger` | pass | 8/8 | FSL-179: -roc with a noise image of 4 volumes for 3 |
| `d13/roc-noise-smaller` | pass | 2/2 | FSL-179: -roc with a noise image of 2 volumes for 3 (FSL writes outside it) |
| `d13/roc-no-true` | both-error | 5/5 | FSL-180: -roc with a truth image without true voxels |
| `d13/roc-no-noise` | both-error | 5/5 | FSL-180: -roc -0.05 with a truth image without zero voxels |
| `orig/fft-asym` | expected-divergence | 7/8 | FSL-192: -kernel file (asymmetric, 13 x 11 x 9) -fmeanu of a 16 x 13 x 11 image (FSL's FFT path: a true convolution, th… |
| `orig/direct-asym` | pass | 7/7 | the same kernel -fmeanu of a 25 x 25 x 25 image (FSL's direct sum: a correlation, the kernel as stored; FSL-192's refer… |
| `orig/fft-nan-local` | pass | 8/8 | FSL-191: -kernel boxv 11 -fmeanu (FSL's FFT path) of an image with one NaN voxel beyond the kernel's reach of most voxe… |
| `orig/roc-ramp` | pass | 8/8 | -roc -2 without a noise image: the rate never reaches 2 and the last true-positive rate is below 1 (the image's minimum… |
| `orig/roc-ramp-noise` | pass | 8/8 | -roc 2 with the same image as its noise: the FWE rate never reaches 2, the last true-positive rate below 1 |
| `orig/roc-below-zero` | pass | 8/8 | -roc -0.5 with truth voxels of -0.0005 (inside the mask, below 0: false-positive candidates) |
| `orig/roc-truth4d` | pass | 8/8 | -roc with a 4D truth image (its first volume, with the image library's warning) |
| `orig/roc-nodir` | pass | 7/7 | FSL-193: -roc with a curve file in a folder that does not exist (FSL writes no curve and says nothing; the default stop… |
| `difftest/edge-char-int-differences` | pass | 7/7 | -edge under -dt char: the differences as C's int (findings 29) |
| `difftest/fsl154-unsigned-wrap` | pass | 7/7 | -dt char on a uint8 image of zeros, -sub 1: FSL-154 named (the unsigned type wraps at 0) |
| `difftest/fsl157-int8-odt-input` | pass | 7/7 | -odt input on an int8 image: FSL-157 named for every int8 image (findings 29) |
| `difftest/fsl165-no-holes` | both-error | 4/4 | -fillh26 of a single slice without holes: FSL-165 (findings 27) |
| `difftest/fsl75-subsamp2offc-heap` | pass | 7/7 | -subsamp2offc of a 3 x 6 x 1 image: FSL-75's unused background value overruns FSL's buffer (FSL aborts after writing); … |
| `difftest/geometry-subsamp2offc-mirrored` | pass | 7/7 | -subsamp2offc of an image FSL holds mirrored: the output's matrices through the held frame (findings 24) |
| `difftest/geometry-xmax-mirrored` | pass | 7/7 | -Xmax of an image FSL holds mirrored: the output's offset moved to the held frame's first voxel (findings 24) |
| `difftest/kernel-file-below-half` | pass | 7/7 | -kernel file with values below 0.5: no empty-mask message (findings 25) |
| `difftest/kernel-file-odd-sizes` | pass | 7/7 | -kernel file of odd sizes: no off-centre warning (findings 25) |
| `difftest/ptoz-nan-sign` | pass | 7/7 | -ptoz of the default NaN: the positive NaN (findings 30) |
| `difftest/roc-candidates-below-bound` | pass | 8/8 | -roc without a noise image: the false-positive candidates are the voxels below the true voxels' bound (findings 29) |
| `difftest/roc-truth-zeros` | both-error | 5/5 | -roc with a truth of zeros: no candidate, FSL-180 (findings 29) |
| `difftest/subsamp2-char-sums` | pass | 7/7 | -subsamp2 under the char type: the sums not wrapped (findings 29) |
| `difftest/subsamp2-qform-quaternion` | pass | 8/8 | -subsamp2 of an image with qfac -1: the qform's quaternion from FSL's product of matrices (findings 24) |
| `difftest/tfce-double-image` | pass | 7/7 | -tfce of a float64 image: FSL's single-precision result (findings 26) |
| `difftest/tfce-step-before-size` | both-error | 4/4 | -tfce of a thin image without positive values: the step's message first (findings 26) |
| `difftest/ztop-nan-sign` | pass | 7/7 | -ztop of the default NaN: the positive NaN (findings 30) |
