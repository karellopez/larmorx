# Parity: BET (replica)

**All cases agree.** 143 cases on the `full` tier: 107 pass, 32 rejected by both, 4 expected divergences, 0 failures.

- **Validated:** `larmorx-nc fsl bet`, `bet2` and `betsurf` (larmorx-nc 0.0.1; crate larmorx-nc-bet, the replica translated from FSL 6.0.7.17's source under the FSL Licence), run by larmorx as a separate process: `larmorx fsl <tool>` and `lx.fsl.bet` / `lx.fsl.betsurf`
- **Reference:** bet, bet2 and betsurf from FSL 6.0.7.17 (fsl-bet2 2111.8, fsl-newimage 2203.12, fsl-meshclass 2111.0), the conda installation in `~/fsl`
- **Test data:** larmorx-testdata `b0999a9e0c31`, tier `full`
- **Generated:** 2026-10-24 on Linux x86_64, with `python -m larmorx_validation parity fsl-bet --tier full --implementation replica`

- **107 of 143 cases bit-identical to FSL** (exit status, messages, every file); 32 where both refuse or FSL crashes and the replica refuses; 4 pending (options that run FSL programs larmorx does not replicate yet).
- **320 of 320 files** written byte for byte as FSL writes them.
- fMRIPrep's own BET outputs: 18 of 18 with the same values as the replica's.
- FSL run again: 18 of 18 runs bit-identical (FSL is deterministic).
- fMRIPrep 25.2.6's FSL (fsl-newimage 2501.4, fsl-miscmaths 2412.4): 18 of 18 runs with the oracle's values.
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - FSL-40: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit status N became N; files ['ab.nii.gz', 'ab_mask.nii.gz'] became ['a.hdrb.nii.gz', 'a.hdrb_mask.nii.gz']; files [] became ['out.nii.gz']): `bet2/img-output`, `defects/hdr-in-name`
  - FSL-42: 3 case(s) named it, 3 with changed values (largest difference 0): `defects/nan-f`, `defects/negative-r`, `defects/zero-w`
  - FSL-43: 1 case(s) named it, 0 with changed values (largest difference 0): `defects/2d`
  - FSL-44: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit status N became N; files [] became ['my out.nii.gz', 'my out_mask.nii.gz']): `defects/blank-in-output`
  - FSL-45: 3 case(s) named it, 3 with changed values (largest difference inf): `bet/small-fov`, `bet/small-fov-verbose`, `bet/debug`
  - FSL-46: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the files present before the run: [] kept, ['out_tmp_notes.txt'] by the default): `defects/cleanup`
  - FSL-48: 1 case(s) named it, 1 with changed values (largest difference 0): `defects/t2-other-grid`
  - FSL-53: 1 case(s) named it, 1 with changed values (largest difference 0): `defects/short-matrix`
  - FSL-54: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit status N became N): `defects/too-few-t1only`, `defects/no-matrix-t1only`
  - FSL-56: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (files ['out.nii', 'out_mask.nii'] became ['out.nii', 'out_mask.nii']; out.nii changed; out_mask.nii changed): `defects/extensions`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (both succeed, or both refuse); where FSL crashes, larmorx exits 1 |
| Messages | standard output and error byte for byte (larmorx's own lines set aside; the bet script's path read as `bet`) |
| Every file written | byte for byte (images decompressed): header and data, meshes as text |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bet | 22 | 12 | 6 | 4 | 0 |
| bet2 | 33 | 20 | 13 | 0 | 0 |
| betsurf | 14 | 8 | 6 | 0 | 0 |
| defects | 13 | 10 | 3 | 0 | 0 |
| fmriprep | 9 | 9 | 0 | 0 | 0 |
| formats | 27 | 23 | 4 | 0 | 0 |
| real | 25 | 25 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `bet/pending-S` | needs FSL programs larmorx does not replicate yet: bet: -S (eye and optic nerve cleanup) runs FSL's FLIRT (standard_space_roi and flirt), which larmorx does not replicate yet; this option is pending (larmorx) |
| `bet/pending-B` | needs FSL programs larmorx does not replicate yet: bet: -B (bias field and neck cleanup) runs FSL's FLIRT (standard_space_roi) and FAST (the bias field), which larmorx does not replicate yet; this option is pending (larmorx) |
| `bet/pending-A` | needs FSL programs larmorx does not replicate yet: bet: -A (skull and scalp surfaces) runs FSL's FLIRT (the registration to MNI152), which larmorx does not replicate yet; this option is pending (larmorx) |
| `bet/pending-A2` | needs FSL programs larmorx does not replicate yet: bet: -A2 (skull and scalp surfaces with a T2) runs FSL's FLIRT (the T2-to-T1 and MNI152 registrations), which larmorx does not replicate yet; this option is pending (larmorx) |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `bet2/usage` | no arguments | both refuse (exit 1) with FSL's usage |
| `bet2/one-argument` | one argument | both refuse (exit 255) with error: not enough arguments, use bet -h for help |
| `bet2/dash-names` | an output name starting with - | both refuse (exit 255) with error : two first arguments should be input and output names, see bet -h for help |
| `bet2/unknown-option` | an unknown option | both refuse (exit 1) with FSL's usage |
| `bet2/missing-value` | -f without its value | both refuse (exit 1) with FSL's usage |
| `bet2/bad-number` | -f with a word | both refuse (exit 1) with FSL's usage |
| `bet2/long-without-equals` | --parameter_d1 5: long options need = | both refuse (exit 1) with FSL's usage |
| `bet2/f-out-of-range` | -f 1.5 | both refuse (exit 1) with Fractional intensity threshold must be between 0 and 1! |
| `bet2/g-out-of-range` | -g -2 | both refuse (exit 1) with Vertical gradient must be between -1 and 1! |
| `bet2/unrecognised-token` | a third word | both refuse (exit 1) with FSL's usage |
| `bet2/missing-input` | an input that does not exist (FSL aborts) | FSL crashes, larmorx refuses: bet2: stopped: FSL's bet2 aborts here with an uncaught std::runtime_error (Failed to read volume /nonexistent/in / Error : No ima… |
| `bet2/img-output` | an output name with .img and no .hdr (FSL-40: FSL aborts) | FSL crashes, larmorx refuses: bet2: stopped: FSL's bet2 aborts here with an uncaught std::out_of_range (basic_string::erase: a name with .img but no .hdr, FSL-… |
| `bet2/constant` | a constant image (FSL-41: FSL reads an empty list and crashes) | FSL crashes, larmorx refuses: bet2: stopped: no voxel inside the initial sphere has an intensity strictly between the 2nd and 98th percentiles, so the median b… |
| `defects/nan-f` | -f nan (FSL-42) | FSL crashes, larmorx refuses: bet2: stopped: the surface's centre lies outside the image (voxel -2147483648 -2147483648 -2147483648), where FSL's bet2 starts i… |
| `defects/zero-w` | -w 0 (FSL-42) | FSL crashes, larmorx refuses: bet2: stopped: the surface's centre lies outside the image (voxel -2147483648 -2147483648 -2147483648), where FSL's bet2 starts i… |
| `formats/output-ANALYZE` | FSLOUTPUTTYPE=ANALYZE | both refuse (exit 1) with Legal values are: NIFTI, NIFTI_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ |
| `formats/output-ANALYZE_GZ` | FSLOUTPUTTYPE=ANALYZE_GZ | both refuse (exit 1) with Legal values are: NIFTI, NIFTI_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ |
| `formats/output-unset` | FSLOUTPUTTYPE unset (FSL stops) | both refuse (exit 1) with e.g. tcsh or csh users add the line "source ${FSLDIR}/etc/fslconf/fsl.csh" |
| `formats/output-bad` | FSLOUTPUTTYPE=JPEG | both refuse (exit 1) with Legal values are: NIFTI, NIFTI_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ |
| `bet/usage` | bet without an output | both refuse (exit 1) with FSL's usage |
| `bet/no-input` | an input that does not exist | both refuse (exit 1) with Error: input image /nonexistent/in not valid |
| `bet/unknown-option` | an unknown option | both refuse (exit 1) with FSL's usage |
| `bet/missing-value` | -f without its value | both refuse (exit 1) with FSL's usage |
| `bet/c-short` | -c with two numbers | both refuse (exit 1) with FSL's usage |
| `bet/bet2-fails` | bet2 refuses (-f 2): the script's ERR trap | both refuse (exit 1) with bet failed during command:<generated>/phantoms/head.nii.gz out -f 2 |
| `defects/blank-in-output` | an output name with a blank (FSL-44) | both refuse (exit 1) with FSL's usage |
| `betsurf/too-few` | too few arguments | both refuse (exit 255) with FSL's usage |
| `betsurf/no-matrix` | a matrix that cannot be opened | both refuse (exit 255) with unable to open /nonexistent.mat |
| `betsurf/no-mesh` | a mesh that cannot be opened | both refuse (exit 255) with terminating. |
| `betsurf/dash-argument` | a word starting with - among the files | both refuse (exit 255) with FSL's usage |
| `betsurf/bad-option` | an unknown option | both refuse (exit 1) with FSL's usage |
| `betsurf/p-bad` | -p with a word | both refuse (exit 1) with FSL's usage |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (fe1815060cc3) |
| larmorx-testdata | b0999a9e0c31 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs run in directories of their own on the same input files, with `PATH=/usr/bin:/bin` (FSL never on it), `FSLDIR` set, an empty `HOME` and the case's `FSLOUTPUTTYPE`. The replica is compared in `reproduce` mode; the fix pass runs its default. The fMRIPrep cases also compare the replica with fMRIPrep's own outputs of the node and run FSL again and fMRIPrep 25.2.6's FSL (FSL's own variability). The `-S`, `-B`, `-A` and `-A2` variations of `bet` run FSL programs larmorx does not replicate yet (standard_space_roi, FLIRT, FAST): the replica refuses them, naming what is missing.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds000005-task_mixedgamblestask_run_01-enhance_and_skullstrip_bold` | pass | 12/12 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-mixedgamblestask_run-01_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000005-task_mixedgamblestask_run_02-enhance_and_skullstrip_bold` | pass | 12/12 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-mixedgamblestask_run-02_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000005-task_mixedgamblestask_run_03-enhance_and_skullstrip_bold` | pass | 12/12 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-mixedgamblestask_run-03_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds005040-task_clip_run_1-enhance_and_skullstrip_bold` | pass | 12/12 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-002_task-clip_run-1_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds005040-task_clip_run_1-skullstrip_bold` | pass | 12/12 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-002_task-clip_run-1_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds005454-task_rest-enhance_and_skullstrip_bold` | pass | 12/12 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-16_task-rest_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds005454-task_rest-skullstrip_bold` | pass | 12/12 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-16_task-rest_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds005752-ses_01_task_rest_dir_forward-enhance_and_skullstrip_bold` | pass | 12/12 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-ON01016_ses-01_task-rest_dir-forward_desc-hmc_boldref_corrected.n… |
| `fmriprep/ds005752-ses_01_task_rest_dir_reverse-enhance_and_skullstrip_bold` | pass | 12/12 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-ON01016_ses-01_task-rest_dir-reverse_desc-hmc_boldref_corrected.n… |
| `bet2/default` | pass | 7/7 | bet2's defaults (-f 0.5) |
| `bet2/all-outputs` | pass | 15/15 | -o -m -s -e -v: every output, verbose |
| `bet2/f-0.1` | pass | 8/8 | -f 0.1 |
| `bet2/f-0.3` | pass | 8/8 | -f 0.3 |
| `bet2/f-0.7` | pass | 8/8 | -f 0.7 |
| `bet2/f-0.9` | pass | 8/8 | -f 0.9 |
| `bet2/g-0.4` | pass | 6/6 | -g -0.4 |
| `bet2/g0.4` | pass | 6/6 | -g 0.4 |
| `bet2/r` | pass | 6/6 | -r 60 (the radius) |
| `bet2/c` | pass | 6/6 | -c: the centre in voxels |
| `bet2/w` | pass | 6/6 | -w 2 and -w 0.5 (smoothness) |
| `bet2/w-0.5` | pass | 6/6 | -w 0.5 |
| `bet2/t` | pass | 6/6 | -t: thresholding |
| `bet2/n` | pass | 5/5 | -n: no brain image |
| `bet2/ct` | pass | 6/6 | --ct: CT intensities |
| `bet2/d1d2` | pass | 6/6 | the hidden --parameter_d1/d2 |
| `bet2/short-run` | pass | 6/6 | a run of one-letter options (-mno) |
| `bet2/repeated` | pass | 6/6 | an option given twice keeps the last |
| `bet2/t2` | pass | 6/6 | the T2-like head |
| `bet2/help` | pass | 4/4 | -h |
| `bet2/usage` | both-error | 4/4 | no arguments |
| `bet2/one-argument` | both-error | 4/4 | one argument |
| `bet2/dash-names` | both-error | 4/4 | an output name starting with - |
| `bet2/unknown-option` | both-error | 4/4 | an unknown option |
| `bet2/missing-value` | both-error | 4/4 | -f without its value |
| `bet2/bad-number` | both-error | 4/4 | -f with a word |
| `bet2/long-without-equals` | both-error | 4/4 | --parameter_d1 5: long options need = |
| `bet2/f-out-of-range` | both-error | 4/4 | -f 1.5 |
| `bet2/g-out-of-range` | both-error | 4/4 | -g -2 |
| `bet2/unrecognised-token` | both-error | 4/4 | a third word |
| `bet2/missing-input` | both-error | 4/4 | an input that does not exist (FSL aborts) |
| `bet2/img-output` | both-error | 4/4 | an output name with .img and no .hdr (FSL-40: FSL aborts) |
| `defects/hdr-in-name` | pass | 6/6 | an output name with .hdr inside (FSL-40) |
| `bet2/constant` | both-error | 4/4 | a constant image (FSL-41: FSL reads an empty list and crashes) |
| `defects/nan-f` | both-error | 4/4 | -f nan (FSL-42) |
| `defects/negative-r` | pass | 6/6 | -r -50 (FSL-42) |
| `defects/zero-w` | both-error | 4/4 | -w 0 (FSL-42) |
| `formats/head-i16` | pass | 6/6 | head-i16: brain and mask |
| `formats/head-u8` | pass | 6/6 | head-u8: brain and mask |
| `formats/head-i8` | pass | 6/6 | head-i8: brain and mask |
| `formats/head-u16` | pass | 6/6 | head-u16: brain and mask |
| `formats/head-i32` | pass | 6/6 | head-i32: brain and mask |
| `formats/head-f64` | pass | 6/6 | head-f64: brain and mask |
| `formats/head-i16-scaled` | pass | 6/6 | head-i16-scaled: brain and mask |
| `formats/head-neuro` | pass | 6/6 | head-neuro: brain and mask |
| `formats/head-qform-only` | pass | 6/6 | head-qform-only: brain and mask |
| `formats/head-sform-only` | pass | 6/6 | head-sform-only: brain and mask |
| `formats/head-noxform` | pass | 6/6 | head-noxform: brain and mask |
| `formats/head-oblique` | pass | 6/6 | head-oblique: brain and mask |
| `formats/head-nifti2` | pass | 6/6 | head-nifti2: brain and mask |
| `formats/head-uncompressed` | pass | 6/6 | head-uncompressed: brain and mask |
| `formats/pair-input` | pass | 6/6 | a NIfTI pair input (.hdr/.img) |
| `formats/analyze-input` | pass | 6/6 | an ANALYZE 7.5 input |
| `formats/output-NIFTI` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI |
| `formats/output-NIFTI_PAIR` | pass | 10/10 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `formats/output-NIFTI2` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI2 |
| `formats/output-NIFTI2_GZ` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `formats/output-NIFTI_PAIR_GZ` | pass | 10/10 | FSLOUTPUTTYPE=NIFTI_PAIR_GZ |
| `formats/output-ANALYZE` | both-error | 4/4 | FSLOUTPUTTYPE=ANALYZE |
| `formats/output-ANALYZE_GZ` | both-error | 4/4 | FSLOUTPUTTYPE=ANALYZE_GZ |
| `formats/output-unset` | both-error | 4/4 | FSLOUTPUTTYPE unset (FSL stops) |
| `formats/output-bad` | both-error | 4/4 | FSLOUTPUTTYPE=JPEG |
| `formats/4d-input` | pass | 6/6 | a 4D input: its first volume, with FSL's warning |
| `defects/2d` | pass | 6/6 | a single slice: the mask's fill starts below the image (FSL-43) |
| `defects/extensions` | pass | 6/6 | two extensions of different sizes kept (FSL_LOAD_NIFTI_EXTENSIONS=1, FSL-56) |
| `formats/extensions-dropped` | pass | 6/6 | extensions in the input, not kept by default |
| `bet/default` | pass | 5/5 | bet's defaults |
| `bet/options` | pass | 9/9 | every bet2 option bet passes on |
| `bet/verbose` | pass | 6/6 | -v: the script's variables, then bet2's messages |
| `bet/robust` | pass | 8/8 | -R: bet2 again from the brain's centre |
| `bet/robust-n` | pass | 6/6 | -R -n: the script removes -n |
| `bet/robust-verbose` | pass | 5/5 | -R -v |
| `bet/fmri` | pass | 8/8 | -F: the first volume's mask, dilated, on every volume |
| `bet/fmri-verbose` | pass | 6/6 | -F -v -f 0.4 |
| `bet/small-fov` | pass | 6/6 | -Z: a slab padded to 100 mm (FSL-45: the crop back needs imglob) |
| `bet/small-fov-verbose` | pass | 7/7 | -Z -v -s |
| `bet/small-fov-large` | pass | 6/6 | -Z on an image already 100 mm high: no padding |
| `bet/debug` | pass | 8/8 | -d: the temporary files kept |
| `bet/pending-S` | expected-divergence | 1/1 | -S: needs FSL programs larmorx does not replicate yet (eyes) |
| `bet/pending-B` | expected-divergence | 1/1 | -B: needs FSL programs larmorx does not replicate yet (bias) |
| `bet/pending-A` | expected-divergence | 1/1 | -A: needs FSL programs larmorx does not replicate yet (surfaces) |
| `bet/pending-A2` | expected-divergence | 1/1 | -A2 <T2>: pending (FLIRT) |
| `bet/usage` | both-error | 4/4 | bet without an output |
| `bet/no-input` | both-error | 4/4 | an input that does not exist |
| `bet/unknown-option` | both-error | 4/4 | an unknown option |
| `bet/missing-value` | both-error | 4/4 | -f without its value |
| `bet/c-short` | both-error | 4/4 | -c with two numbers |
| `bet/bet2-fails` | both-error | 4/4 | bet2 refuses (-f 2): the script's ERR trap |
| `defects/blank-in-output` | both-error | 4/4 | an output name with a blank (FSL-44) |
| `defects/empty-argument` | pass | 5/5 | an empty argument ends the options (FSL-44) |
| `defects/cleanup` | pass | 6/6 | the cleanup glob removes out_tmp* files it did not create (FSL-46) |
| `betsurf/t1only` | pass | 16/16 | --t1only -o -m -s: bet -A's call |
| `betsurf/t1t2` | pass | 14/14 | -o -m -s with the T2: bet -A2's call |
| `betsurf/verbose` | pass | 10/10 | -v -m |
| `betsurf/precision` | pass | 10/10 | -p 1: retessellated |
| `betsurf/surfaces-only` | pass | 7/7 | no option: the meshes only |
| `betsurf/help` | pass | 4/4 | -h |
| `betsurf/too-few` | both-error | 4/4 | too few arguments |
| `defects/too-few-t1only` | pass | 4/4 | --t1only with too few arguments exits 0 (FSL-54) |
| `betsurf/no-matrix` | both-error | 4/4 | a matrix that cannot be opened |
| `defects/no-matrix-t1only` | pass | 4/4 | --t1only with a missing matrix exits 0 (FSL-54) |
| `betsurf/no-mesh` | both-error | 4/4 | a mesh that cannot be opened |
| `betsurf/dash-argument` | both-error | 4/4 | a word starting with - among the files |
| `betsurf/bad-option` | both-error | 4/4 | an unknown option |
| `betsurf/p-bad` | both-error | 4/4 | -p with a word |
| `defects/short-matrix` | pass | 10/10 | a matrix of 6 numbers (FSL-53) |
| `defects/t2-other-grid` | pass | 10/10 | a T2 on another grid (FSL-48) |
| `betsurf/off-mesh` | pass | 10/10 | an OFF mesh |
| `betsurf/fs-mesh` | pass | 10/10 | a FreeSurfer ASCII mesh |
| `real/child-3y` | pass | 8/8 | a child of 3.5 years (ds000228): bet's defaults and -m |
| `real/child-3y-R` | pass | 8/8 | a child of 3.5 years (ds000228): -R -m -s -e |
| `real/child-7y` | pass | 8/8 | a child of 7.5 years with motion (ds000256): bet's defaults and -m |
| `real/child-7y-R` | pass | 8/8 | a child of 7.5 years with motion (ds000256): -R -m -s -e |
| `real/adult-ds000005` | pass | 8/8 | an adult, the fMRIPrep oracle run's T1w (ds000005): bet's defaults and -m |
| `real/adult-ds000005-R` | pass | 8/8 | an adult, the fMRIPrep oracle run's T1w (ds000005): -R -m -s -e |
| `real/older-75y` | pass | 8/8 | an adult of 75 years at 1.5 T (ds002422): bet's defaults and -m |
| `real/older-75y-R` | pass | 8/8 | an adult of 75 years at 1.5 T (ds002422): -R -m -s -e |
| `real/older-82y` | pass | 8/8 | an adult of 82 years (ds003592): bet's defaults and -m |
| `real/older-82y-R` | pass | 8/8 | an adult of 82 years (ds003592): -R -m -s -e |
| `real/7t` | pass | 8/8 | 7 T, MP2RAGE (ds005454): bet's defaults and -m |
| `real/7t-R` | pass | 8/8 | 7 T, MP2RAGE (ds005454): -R -m -s -e |
| `real/tumour` | pass | 8/8 | a tumour (ds005003): bet's defaults and -m |
| `real/tumour-R` | pass | 8/8 | a tumour (ds005003): -R -m -s -e |
| `real/stroke` | pass | 8/8 | a stroke (ds004884): bet's defaults and -m |
| `real/stroke-R` | pass | 8/8 | a stroke (ds004884): -R -m -s -e |
| `real/philips` | pass | 8/8 | a Philips scanner (ds002603): bet's defaults and -m |
| `real/philips-R` | pass | 8/8 | a Philips scanner (ds002603): -R -m -s -e |
| `real/adult-ds000005-surf` | pass | 14/14 | an adult (ds000005): betsurf --t1only -o -m -s on FLIRT's matrix |
| `real/older-82y-surf` | pass | 14/14 | an adult of 82 years: betsurf --t1only -o -m -s on FLIRT's matrix |
| `real/child-7y-surf-t2` | pass | 14/14 | a child of 7.5 years (ds000256): betsurf -o -m -s with the T2 FLIRT registered |
| `real/tumour-surf-t2` | pass | 14/14 | a tumour (ds005003): betsurf -o -m -s with the T2 FLIRT registered |
| `real/philips-surf-t2` | pass | 14/14 | a Philips scanner (ds002603): betsurf -o -m -s with the T2 FLIRT registered |
| `real/bold-F` | pass | 6/6 | a BOLD series (ds000005): bet -F |
| `real/bold-first` | pass | 6/6 | a BOLD series (ds000005): bet2 on its first volume (FSL's warning) |
