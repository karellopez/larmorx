# Parity: BET (clean-room original)

**3 case(s) fail.** 142 cases on the `standard` tier: 108 pass, 24 rejected by both, 7 expected divergences, 3 failures.

- **Validated:** `larmorx fsl bet2` and `bet` (larmorx 0.0.1, crate larmorx-fsl: the clean-room original written from specs/bet.md, Apache-2.0), in process, and `lx.fsl.bet(..., implementation='original')`; betsurf's original is still to be written
- **Reference:** bet and bet2 from FSL 6.0.7.23 (fsl-bet2 2111.9, fsl-newimage 2601.0, fsl-meshclass 2111.0), the conda installation in `~/fsl-6.0.7.23`
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-10-30 on Linux x86_64, with `python -m larmorx_validation parity fsl-bet --tier standard`
- **Original programs:** 230 runs: 222 from the oracle cache, 8 run (3 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

- **108 of 142 cases within the thresholds** (exit status and messages identical, every file within its threshold); 24 where both refuse or FSL crashes and larmorx refuses; 3 where FSL crashes and `reproduce` fills what FSL leaves undefined (D13's failure cases); 4 pending (options that run FSL programs larmorx does not have yet).
- **202 of 217 brains, masks and outlines with FSL's values, bit for bit**, 9 more within the thresholds; 3 of 3 meshes with FSL's vertices to the file's 6 decimals; 25 skull images not bit-identical, reported (Dice) with the skull search's open details.
- fMRIPrep's own BET outputs: 58 of 58 with the same values as the original's.
- FSL run again: 38 of 38 runs bit-identical (FSL is deterministic).
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - FSL-40: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit status N became N; files ['ab.nii.gz', 'ab_mask.nii.gz'] became ['a.hdrb.nii.gz', 'a.hdrb_mask.nii.gz']; files [] became ['out.nii.gz']): `bet2/img-output`, `defects/hdr-in-name`
  - FSL-42: 3 case(s) named it, 3 with changed values (largest difference 0): `defects/nan-f`, `defects/negative-r`, `defects/zero-w`
  - FSL-43: 2 case(s) named it, 1 with changed values (largest difference 0): `bet2/constant`, `defects/2d`
  - FSL-44: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit status N became N; files ['out.nii.gz'] became ['out.nii.gz', 'out_mask.nii.gz']; files [] became ['my out.nii.gz', 'my out_mask.nii.gz']): `defects/blank-in-output`, `defects/empty-argument`
  - FSL-45: 3 case(s) named it, 3 with changed values (largest difference inf): `bet/small-fov`, `bet/small-fov-verbose`, `bet/debug`
  - FSL-46: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the files present before the run: [] kept, ['out_tmp_notes.txt'] by the default): `defects/cleanup`
  - FSL-56: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (files ['out.nii', 'out_mask.nii'] became ['out.nii', 'out_mask.nii']; out.nii changed; out_mask.nii changed): `defects/extensions`
  - FSL-62: 1 case(s) named it, 1 with changed values (largest difference 1e+03): `defects/binary-mask-ct`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (both succeed, or both refuse); where FSL crashes, larmorx exits 1 |
| Messages | standard output and error byte for byte (larmorx's own lines set aside; the bet script's path read as `bet`) |
| Files written | the same names |
| Image headers | byte for byte but for `descrip` (the original writes `larmorx fsl bet2`, FSL its build string) |
| Masks and brains | bit-identical, or Dice >= 0.995 with the boundary distance's 95th percentile <= 0.5 and maximum <= 2.0 voxels and the brain's values identical inside both (specs/bet.md §11) |
| Outlines | bit-identical, or the drawn surfaces within 0.5 / 2.0 voxels (95th percentile / maximum) and the input identical elsewhere |
| Meshes | the same triangles; per-vertex distance 95th percentile <= 0.1 mm and maximum <= 1.0 mm (specs/bet.md §11) |
| Skull | reported (Dice): specs/bet.md §4.7 leaves details of the skull search open (a question for the spec writer) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bet | 23 | 13 | 6 | 4 | 0 |
| bet2 | 36 | 22 | 13 | 1 | 0 |
| defects | 10 | 7 | 1 | 2 | 0 |
| fmriprep | 29 | 29 | 0 | 0 | 0 |
| formats | 33 | 29 | 4 | 0 | 0 |
| real | 11 | 8 | 0 | 0 | 3 |

## Failures

### `real/older-75y`

an adult of 75 years at 1.5 T (ds002422): bet's defaults and -m

- **out.nii.gz** (Dice; boundary distance p95 / max (voxels) = 0.994915; 1.00 / 6.71, threshold >= 0.995; <= 0.5 / 2.0; values identical inside both): 16737 of 11534336 values differ, 0 inside both
- **out_mask.nii.gz** (Dice; boundary distance p95 / max (voxels) = 0.994914; 1.00 / 6.71, threshold >= 0.995; <= 0.5 / 2.0): 16738 of 11534336 values differ

### `real/7t`

7 T, MP2RAGE (ds005454): bet's defaults and -m

- **out.nii.gz** (Dice; boundary distance p95 / max (voxels) = 0.980396; 2.24 / 5.92, threshold >= 0.995; <= 0.5 / 2.0; values identical inside both): 64531 of 17547264 values differ, 0 inside both
- **out_mask.nii.gz** (Dice; boundary distance p95 / max (voxels) = 0.980343; 2.24 / 5.92, threshold >= 0.995; <= 0.5 / 2.0): 64889 of 17547264 values differ

### `real/tumour`

a tumour (ds005003): bet's defaults and -m

- **out.nii.gz** (Dice; boundary distance p95 / max (voxels) = 0.990864; 1.41 / 5.00, threshold >= 0.995; <= 0.5 / 2.0; values identical inside both): 28263 of 9437184 values differ, 0 inside both
- **out_mask.nii.gz** (Dice; boundary distance p95 / max (voxels) = 0.990864; 1.41 / 5.00, threshold >= 0.995; <= 0.5 / 2.0): 28264 of 9437184 values differ

## Expected divergences

| Case | Reason |
|---|---|
| `bet2/constant` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-43 and writes its outputs; the default refuses |
| `defects/nan-f` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-42 and writes its outputs; the default refuses |
| `defects/zero-w` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-42 and writes its outputs; the default refuses |
| `bet/pending-S` | needs FSL programs larmorx does not replicate yet: larmorx fsl bet: -S is pending: it runs FSL's standard_space_roi, FLIRT and fslmaths' morphology, which larmorx does not have yet (larmorx) |
| `bet/pending-B` | needs FSL programs larmorx does not replicate yet: larmorx fsl bet: -B is pending: it runs FSL's standard_space_roi and FAST, which larmorx does not have yet (larmorx) |
| `bet/pending-A` | needs FSL programs larmorx does not replicate yet: larmorx fsl bet: -A is pending: it runs FSL's FLIRT (then betsurf), which larmorx does not have yet (larmorx) |
| `bet/pending-A2` | needs FSL programs larmorx does not replicate yet: larmorx fsl bet: -A2 is pending: it runs FSL's FLIRT (then betsurf), which larmorx does not have yet (larmorx) |

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
| `bet2/missing-input` | an input that does not exist (FSL aborts) | FSL crashes, larmorx refuses: larmorx fsl bet2: /nonexistent/in cannot be read; FSL aborts here (larmorx) |
| `bet2/img-output` | an output name with .img and no .hdr (FSL-40: FSL aborts) | FSL crashes, larmorx refuses: larmorx fsl bet2: a name holds '.img' and no '.hdr', on which FSL aborts (FSL-40) (larmorx) |
| `bet2/binary-mask-missing` | --binaryMask naming no image (FSL aborts) | FSL crashes, larmorx refuses: larmorx fsl bet2: /nonexistent/mask cannot be read; FSL aborts here (larmorx) |
| `formats/output-ANALYZE` | FSLOUTPUTTYPE=ANALYZE | both refuse (exit 1) with Legal values are: NIFTI, NIFTI_PAIR, NIFTI2, NIFTI2_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ, NIFTI2_GZ, NIFTI2_PAIR_GZ, NIFTI_ZST, NIFTI_PAIR_Z… |
| `formats/output-ANALYZE_GZ` | FSLOUTPUTTYPE=ANALYZE_GZ | both refuse (exit 1) with Legal values are: NIFTI, NIFTI_PAIR, NIFTI2, NIFTI2_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ, NIFTI2_GZ, NIFTI2_PAIR_GZ, NIFTI_ZST, NIFTI_PAIR_Z… |
| `formats/output-unset` | FSLOUTPUTTYPE unset (FSL stops) | both refuse (exit 1) with e.g. tcsh or csh users add the line "source ${FSLDIR}/etc/fslconf/fsl.csh" |
| `formats/output-bad` | FSLOUTPUTTYPE=JPEG | both refuse (exit 1) with Legal values are: NIFTI, NIFTI_PAIR, NIFTI2, NIFTI2_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ, NIFTI2_GZ, NIFTI2_PAIR_GZ, NIFTI_ZST, NIFTI_PAIR_Z… |
| `bet/usage` | bet without an output | both refuse (exit 1) with FSL's usage |
| `bet/no-input` | an input that does not exist | both refuse (exit 1) with Error: input image /nonexistent/in not valid |
| `bet/unknown-option` | an unknown option | both refuse (exit 1) with FSL's usage |
| `bet/missing-value` | -f without its value | both refuse (exit 1) with FSL's usage |
| `bet/c-short` | -c with two numbers | both refuse (exit 1) with FSL's usage |
| `bet/bet2-fails` | bet2 refuses (-f 2): the script's ERR trap | both refuse (exit 1) with bet failed during command:<generated>/phantoms/head.nii.gz out -f 2 |
| `defects/blank-in-output` | an output name with a blank (FSL-44) | both refuse (exit 1) with FSL's usage |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (2c5f6692c6c9) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

Both programs run in directories of their own on the same input files, with `PATH=/usr/bin:/bin` (FSL never on it), `FSLDIR` set, an empty `HOME` and the case's `FSLOUTPUTTYPE`. The original is compared in `reproduce` mode; the fix pass runs its default. The fMRIPrep cases also run FSL again and fMRIPrep 25.2.6's FSL (FSL's own variability) and report the original's agreement with fMRIPrep's own outputs. betsurf's cases wait for its original. The `-S`, `-B`, `-A` and `-A2` variations of `bet` run FSL programs larmorx does not have yet (standard_space_roi, FLIRT, FAST): the original refuses them, naming what is missing. FSL 6.0.7.23 (the `--fsl-version` default).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds000005-task_mixedgamblestask_run_01-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-mixedgamblestask_run-01_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000005-task_mixedgamblestask_run_02-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-mixedgamblestask_run-02_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000005-task_mixedgamblestask_run_03-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-mixedgamblestask_run-03_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000228-task_pixar-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-pixar008_task-pixar_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000256-task_restbaseline-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-CTS203_task-restbaseline_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000256-task_restfixationfixed-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-CTS203_task-restfixationfixed_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000256-task_restfixationnofeedback-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-CTS203_task-restfixationnofeedback_desc-hmc_boldref_corrected.nii… |
| `fmriprep/ds002422-task_arithm_run_01-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-arithm_run-01_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds002422-task_arithm_run_01-skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-arithm_run-01_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds002422-task_arithm_run_02-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-arithm_run-02_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds002422-task_arithm_run_02-skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-arithm_run-02_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds002422-task_rest-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-rest_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds002422-task_rest-skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-rest_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds002603-task_wm_acq_Mb4Mm3Tr550-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-wm_acq-Mb4Mm3Tr550_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds002603-task_wm_acq_Mb4Mm3Tr550-skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-wm_acq-Mb4Mm3Tr550_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds002603-task_wm_acq_SeqMm3Tr2000-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-wm_acq-SeqMm3Tr2000_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds002603-task_wm_acq_SeqMm3Tr2000-skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-wm_acq-SeqMm3Tr2000_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds003592-ses_1_task_rest_echo_1-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-280_ses-1_task-rest_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds004884-ses_217_task_naming40_acq_epfid2_dir_AP_run_2-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-M2079_ses-217_task-naming40_acq-epfid2_dir-AP_run-2_desc-hmc_bold… |
| `fmriprep/ds004884-ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-M2079_ses-217_task-rest_acq-epfidp2p2_dir-AP_run-9_desc-hmc_boldr… |
| `fmriprep/ds004958-task_rest-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-fsm64ps_task-rest_sbref_corrected.nii.gz) |
| `fmriprep/ds005003-ses_1_task_motor-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-RS001_ses-1_task-motor_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds005003-ses_1_task_rest-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-RS001_ses-1_task-rest_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds005040-task_clip_run_1-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-002_task-clip_run-1_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds005040-task_clip_run_1-skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-002_task-clip_run-1_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds005454-task_rest-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-16_task-rest_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds005454-task_rest-skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-16_task-rest_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds005752-ses_01_task_rest_dir_forward-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-ON01016_ses-01_task-rest_dir-forward_desc-hmc_boldref_corrected.n… |
| `fmriprep/ds005752-ses_01_task_rest_dir_reverse-enhance_and_skullstrip_bold` | pass | 14/14 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-ON01016_ses-01_task-rest_dir-reverse_desc-hmc_boldref_corrected.n… |
| `bet2/default` | pass | 8/8 | bet2's defaults (-f 0.5) |
| `bet2/all-outputs` | pass | 19/19 | -o -m -s -e -v: every output, verbose |
| `bet2/f-0.1` | pass | 10/10 | -f 0.1 |
| `bet2/f-0.3` | pass | 10/10 | -f 0.3 |
| `bet2/f-0.7` | pass | 10/10 | -f 0.7 |
| `bet2/f-0.9` | pass | 10/10 | -f 0.9 |
| `bet2/g-0.4` | pass | 8/8 | -g -0.4 |
| `bet2/g0.4` | pass | 8/8 | -g 0.4 |
| `bet2/r` | pass | 8/8 | -r 60 (the radius) |
| `bet2/c` | pass | 8/8 | -c: the centre in voxels |
| `bet2/w` | pass | 8/8 | -w 2 and -w 0.5 (smoothness) |
| `bet2/w-0.5` | pass | 8/8 | -w 0.5 |
| `bet2/t` | pass | 8/8 | -t: thresholding |
| `bet2/n` | pass | 6/6 | -n: no brain image |
| `bet2/ct` | pass | 8/8 | --ct: CT intensities |
| `bet2/d1d2` | pass | 8/8 | the hidden --parameter_d1/d2 |
| `bet2/short-run` | pass | 8/8 | a run of one-letter options (-mno) |
| `bet2/repeated` | pass | 8/8 | an option given twice keeps the last |
| `bet2/t2` | pass | 8/8 | the T2-like head |
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
| `defects/hdr-in-name` | pass | 8/8 | an output name with .hdr inside (FSL-40) |
| `bet2/constant` | expected-divergence | 6/6 | a constant image: FSL 6.0.7.23 takes the robust maximum as the median (FSL-41, fixed upstream; 6.0.7.17 read an empty l… |
| `bet2/binary-mask` | pass | 12/12 | --binaryMask: the surface found on a brain mask, the brain, mask, outline, skull and thresholds from the input (FSL 6.0… |
| `bet2/binary-mask-verbose` | pass | 7/7 | --binaryMask -v: the parameters printed are the mask's |
| `defects/binary-mask-ct` | pass | 8/8 | --ct --binaryMask: FSL cuts the brain image from the mask (FSL-62) |
| `bet2/binary-mask-missing` | both-error | 4/4 | --binaryMask naming no image (FSL aborts) |
| `defects/nan-f` | expected-divergence | 6/6 | -f nan (FSL-42; FSL crashes in its fill, FSL-43) |
| `defects/negative-r` | pass | 8/8 | -r -50 (FSL-42) |
| `defects/zero-w` | expected-divergence | 6/6 | -w 0 (FSL-42; FSL crashes in its fill, FSL-43) |
| `formats/head-i16` | pass | 8/8 | head-i16: brain and mask |
| `formats/head-u8` | pass | 8/8 | head-u8: brain and mask |
| `formats/head-i8` | pass | 8/8 | head-i8: brain and mask |
| `formats/head-u16` | pass | 8/8 | head-u16: brain and mask |
| `formats/head-i32` | pass | 8/8 | head-i32: brain and mask |
| `formats/head-f64` | pass | 8/8 | head-f64: brain and mask |
| `formats/head-i16-scaled` | pass | 8/8 | head-i16-scaled: brain and mask |
| `formats/head-neuro` | pass | 8/8 | head-neuro: brain and mask |
| `formats/head-qform-only` | pass | 8/8 | head-qform-only: brain and mask |
| `formats/head-sform-only` | pass | 8/8 | head-sform-only: brain and mask |
| `formats/head-noxform` | pass | 8/8 | head-noxform: brain and mask |
| `formats/head-oblique` | pass | 8/8 | head-oblique: brain and mask |
| `formats/head-nifti2` | pass | 8/8 | head-nifti2: brain and mask |
| `formats/head-uncompressed` | pass | 8/8 | head-uncompressed: brain and mask |
| `formats/head-zst` | pass | 8/8 | head-zst: brain and mask |
| `formats/head-bz2` | pass | 8/8 | head-bz2: brain and mask |
| `formats/pair-input` | pass | 8/8 | a NIfTI pair input (.hdr/.img) |
| `formats/analyze-input` | pass | 8/8 | an ANALYZE 7.5 input |
| `formats/output-NIFTI` | pass | 10/10 | FSLOUTPUTTYPE=NIFTI |
| `formats/output-NIFTI_PAIR` | pass | 10/10 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `formats/output-NIFTI2` | pass | 10/10 | FSLOUTPUTTYPE=NIFTI2 |
| `formats/output-NIFTI2_GZ` | pass | 10/10 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `formats/output-NIFTI_PAIR_GZ` | pass | 10/10 | FSLOUTPUTTYPE=NIFTI_PAIR_GZ |
| `formats/output-ANALYZE` | both-error | 4/4 | FSLOUTPUTTYPE=ANALYZE |
| `formats/output-ANALYZE_GZ` | both-error | 4/4 | FSLOUTPUTTYPE=ANALYZE_GZ |
| `formats/output-NIFTI_ZST` | pass | 10/10 | FSLOUTPUTTYPE=NIFTI_ZST |
| `formats/output-NIFTI2_PAIR_ZST` | pass | 10/10 | FSLOUTPUTTYPE=NIFTI2_PAIR_ZST |
| `formats/output-NIFTI_BZ2` | pass | 10/10 | FSLOUTPUTTYPE=NIFTI_BZ2 |
| `formats/output-NIFTI_PAIR_BZ2` | pass | 10/10 | FSLOUTPUTTYPE=NIFTI_PAIR_BZ2 |
| `formats/output-unset` | both-error | 4/4 | FSLOUTPUTTYPE unset (FSL stops) |
| `formats/output-bad` | both-error | 4/4 | FSLOUTPUTTYPE=JPEG |
| `formats/4d-input` | pass | 8/8 | a 4D input: its first volume, with FSL's warning |
| `defects/2d` | pass | 8/8 | a single slice: the mask's fill starts below the image (FSL-43) |
| `defects/extensions` | pass | 6/6 | two extensions of different sizes kept (FSL_LOAD_NIFTI_EXTENSIONS=1, FSL-56) |
| `formats/extensions-dropped` | pass | 8/8 | extensions in the input, not kept by default |
| `bet/default` | pass | 6/6 | bet's defaults |
| `bet/zst-output` | pass | 8/8 | FSLOUTPUTTYPE=NIFTI_ZST through the script (fslval, fslmaths on zstd files; FSL 6.0.7.23) |
| `bet/options` | pass | 13/13 | every bet2 option bet passes on |
| `bet/verbose` | pass | 8/8 | -v: the script's variables, then bet2's messages |
| `bet/robust` | pass | 10/10 | -R: bet2 again from the brain's centre |
| `bet/robust-n` | pass | 8/8 | -R -n: the script removes -n |
| `bet/robust-verbose` | pass | 6/6 | -R -v |
| `bet/fmri` | pass | 10/10 | -F: the first volume's mask, dilated, on every volume |
| `bet/fmri-verbose` | pass | 8/8 | -F -v -f 0.4 |
| `bet/small-fov` | pass | 8/8 | -Z: a slab padded to 100 mm (FSL-45: the crop back needs imglob) |
| `bet/small-fov-verbose` | pass | 10/10 | -Z -v -s |
| `bet/small-fov-large` | pass | 8/8 | -Z on an image already 100 mm high: no padding |
| `bet/debug` | pass | 12/12 | -d: the temporary files kept |
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
| `defects/empty-argument` | pass | 6/6 | an empty argument ends the options (FSL-44) |
| `defects/cleanup` | pass | 8/8 | the cleanup glob removes out_tmp* files it did not create (FSL-46) |
| `real/child-3y` | pass | 10/10 | a child of 3.5 years (ds000228): bet's defaults and -m |
| `real/child-7y` | pass | 10/10 | a child of 7.5 years with motion (ds000256): bet's defaults and -m |
| `real/adult-ds000005` | pass | 10/10 | an adult, the fMRIPrep oracle run's T1w (ds000005): bet's defaults and -m |
| `real/older-75y` | fail | 8/10 | an adult of 75 years at 1.5 T (ds002422): bet's defaults and -m |
| `real/older-82y` | pass | 10/10 | an adult of 82 years (ds003592): bet's defaults and -m |
| `real/7t` | fail | 8/10 | 7 T, MP2RAGE (ds005454): bet's defaults and -m |
| `real/tumour` | fail | 8/10 | a tumour (ds005003): bet's defaults and -m |
| `real/stroke` | pass | 10/10 | a stroke (ds004884): bet's defaults and -m |
| `real/philips` | pass | 10/10 | a Philips scanner (ds002603): bet's defaults and -m |
| `real/bold-F` | pass | 8/8 | a BOLD series (ds000005): bet -F |
| `real/bold-first` | pass | 8/8 | a BOLD series (ds000005): bet2 on its first volume (FSL's warning) |
