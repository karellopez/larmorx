# Parity: BET (replica)

**All cases agree.** 176 cases on the `standard` tier: 128 pass, 31 rejected by both, 17 expected divergences, 0 failures.

- **Validated:** `larmorx-nc fsl bet`, `bet2` and `betsurf` (larmorx-nc 0.0.1; crate larmorx-nc-bet, the replica translated from FSL 6.0.7.23's source under the FSL Licence), run by larmorx as a separate process: `larmorx fsl <tool>` and `lx.fsl.bet` / `lx.fsl.betsurf`
- **Reference:** bet, bet2 and betsurf from FSL 6.0.7.23 (fsl-bet2 2111.9, fsl-newimage 2601.0, fsl-meshclass 2111.0), the conda installation in `~/fsl-6.0.7.23`
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-10-29 on Linux x86_64, with `python -m larmorx_validation parity fsl-bet --tier standard --implementation replica`
- **Original programs:** 432 runs: 381 from the oracle cache, 51 run (34 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

- **128 of 176 cases bit-identical to FSL** (exit status, messages, every file); 31 where both refuse or FSL crashes and the replica refuses; 12 where FSL crashes and the replica's `reproduce` fills what FSL leaves undefined (D13's failure cases: deterministic, named; the default refuses or corrects); 4 pending (options that run FSL programs larmorx does not replicate yet).
- **315 of 315 files** written byte for byte as FSL writes them.
- fMRIPrep's own BET outputs: 58 of 58 with the same values as the replica's.
- FSL run again: 38 of 38 runs bit-identical (FSL is deterministic).
- fMRIPrep 25.2.6's FSL (fsl-newimage 2501.4, fsl-miscmaths 2412.4): 38 of 38 runs with the oracle's values.
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - FSL-40: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit status N became N; files ['ab.nii.gz', 'ab_mask.nii.gz'] became ['a.hdrb.nii.gz', 'a.hdrb_mask.nii.gz']; files [] became ['out.nii.gz']): `bet2/img-output`, `defects/hdr-in-name`
  - FSL-42: 3 case(s) named it, 3 with changed values (largest difference 0): `defects/nan-f`, `defects/negative-r`, `defects/zero-w`
  - FSL-43: 9 case(s) named it, 4 with changed values (largest difference 9.44e+03), 3 with other changes (exit status N became N; files ['out.nii.gz'] became []; out.img.gz changed; out_mesh.vtk changed): `bet2/constant`, `difftest/bet-fill-by-index`, `difftest/bet-robust-crash-filled`, `difftest/bet-script-failure-constant`, `difftest/bet2-fill-memory-beyond`, `difftest/bet2-filled-then-output-type`, `difftest/bet2-n-crash-nothing-asked`, `difftest/bet2-verbose-constant-filled` …
  - FSL-44: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (exit status N became N; files [] became ['my out.nii.gz', 'my out_mask.nii.gz']): `defects/blank-in-output`
  - FSL-45: 3 case(s) named it, 3 with changed values (largest difference inf): `bet/small-fov`, `bet/small-fov-verbose`, `bet/debug`
  - FSL-46: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the files present before the run: [] kept, ['out_tmp_notes.txt'] by the default): `defects/cleanup`
  - FSL-48: 1 case(s) named it, 1 with changed values (largest difference 0): `defects/t2-other-grid`
  - FSL-52: 3 case(s) named it, 3 with changed values (largest difference 0): `defects/mesh-unread`, `defects/mesh-missing-vertex`, `defects/mesh-truncated`
  - FSL-53: 1 case(s) named it, 1 with changed values (largest difference 0): `defects/short-matrix`
  - FSL-54: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit status N became N): `defects/too-few-t1only`, `defects/no-matrix-t1only`
  - FSL-56: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (files ['out.nii', 'out_mask.nii'] became ['out.nii', 'out_mask.nii']; out.nii changed; out_mask.nii changed): `defects/extensions`
  - FSL-62: 1 case(s) named it, 1 with changed values (largest difference 1e+03): `defects/binary-mask-ct`
  - FSL-63: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (files ['out.nii.gz'] became ['out.nii.gz', 'out_mesh.vtk']): `difftest/bet-robust-mesh-echo`
- **FSL's own step, FSL 6.0.7.17 → 6.0.7.23** (both installations on the same arguments; the previous one's runs from the oracle-output cache or its recording where an earlier record made them): 27 of 172 cases identical (exit status, messages, every output); 145 changed: 2 by FSL-41 (FSL 6.0.7.23's bet2 takes the robust maximum as the median when no voxel lies between the 2 and 98 % thresholds (a binary image); 6.0.7.17 read before an empty list): `bet2/binary-mask-verbose`, `difftest/bet2-verbose-constant-filled`; 1 by FSL-43-memory (the mask's fill starts outside the image and FSL reads memory beyond the volume (FSL-43): what it finds there, and whether it crashes, depends on the memory each build lays out): `difftest/bet2-fill-memory-beyond`; 12 by binary-mask (FSL 6.0.7.23's bet2 has the --binaryMask option (the mesh from a mask)): `bet2/help`, `bet2/usage`, `bet2/unknown-option`, `bet2/missing-value`, `bet2/bad-number`, `bet2/long-without-equals`, `bet2/unrecognised-token`, `bet2/binary-mask`, `bet2/binary-mask-verbose`, `defects/binary-mask-ct`, `bet2/binary-mask-missing`, `defects/blank-in-output`; 114 by build-string (newimage 2601.0 writes its own build string into an image's descrip ('2601.0 2025-12-08T15:20:57Z'; 2203.12 wrote '2203.12-dirty 2024-02-01T16:17:47+00:00')): `fmriprep/ds000005-task_mixedgamblestask_run_01-enhance_and_skullstrip_bold`, `fmriprep/ds000005-task_mixedgamblestask_run_02-enhance_and_skullstrip_bold`, `fmriprep/ds000005-task_mixedgamblestask_run_03-enhance_and_skullstrip_bold`, `fmriprep/ds000228-task_pixar-enhance_and_skullstrip_bold`, `fmriprep/ds000256-task_restbaseline-enhance_and_skullstrip_bold`, `fmriprep/ds000256-task_restfixationfixed-enhance_and_skullstrip_bold`, `fmriprep/ds000256-task_restfixationnofeedback-enhance_and_skullstrip_bold`, `fmriprep/ds002422-task_arithm_run_01-enhance_and_skullstrip_bold`, `fmriprep/ds002422-task_arithm_run_01-skullstrip_bold`, `fmriprep/ds002422-task_arithm_run_02-enhance_and_skullstrip_bold`, `fmriprep/ds002422-task_arithm_run_02-skullstrip_bold`, `fmriprep/ds002422-task_rest-enhance_and_skullstrip_bold`, `fmriprep/ds002422-task_rest-skullstrip_bold`, `fmriprep/ds002603-task_wm_acq_Mb4Mm3Tr550-enhance_and_skullstrip_bold`, `fmriprep/ds002603-task_wm_acq_Mb4Mm3Tr550-skullstrip_bold`, `fmriprep/ds002603-task_wm_acq_SeqMm3Tr2000-enhance_and_skullstrip_bold`, `fmriprep/ds002603-task_wm_acq_SeqMm3Tr2000-skullstrip_bold`, `fmriprep/ds003592-ses_1_task_rest_echo_1-enhance_and_skullstrip_bold`, `fmriprep/ds004884-ses_217_task_naming40_acq_epfid2_dir_AP_run_2-enhance_and_skullstrip_bold`, `fmriprep/ds004884-ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9-enhance_and_skullstrip_bold`, `fmriprep/ds004958-task_rest-enhance_and_skullstrip_bold`, `fmriprep/ds005003-ses_1_task_motor-enhance_and_skullstrip_bold`, `fmriprep/ds005003-ses_1_task_rest-enhance_and_skullstrip_bold`, `fmriprep/ds005040-task_clip_run_1-enhance_and_skullstrip_bold`, `fmriprep/ds005040-task_clip_run_1-skullstrip_bold`, `fmriprep/ds005454-task_rest-enhance_and_skullstrip_bold`, `fmriprep/ds005454-task_rest-skullstrip_bold`, `fmriprep/ds005752-ses_01_task_rest_dir_forward-enhance_and_skullstrip_bold`, `fmriprep/ds005752-ses_01_task_rest_dir_reverse-enhance_and_skullstrip_bold`, `bet2/default`, `bet2/all-outputs`, `bet2/f-0.1`, `bet2/f-0.3`, `bet2/f-0.7`, `bet2/f-0.9`, `bet2/g-0.4`, `bet2/g0.4`, `bet2/r`, `bet2/c`, `bet2/w`, `bet2/w-0.5`, `bet2/t`, `bet2/n`, `bet2/ct`, `bet2/d1d2`, `bet2/short-run`, `bet2/repeated`, `bet2/t2`, `defects/hdr-in-name`, `defects/negative-r`, `formats/head-i16`, `formats/head-u8`, `formats/head-i8`, `formats/head-u16`, `formats/head-i32`, `formats/head-f64`, `formats/head-i16-scaled`, `formats/head-neuro`, `formats/head-qform-only`, `formats/head-sform-only`, `formats/head-noxform`, `formats/head-oblique`, `formats/head-nifti2`, `formats/head-uncompressed`, `formats/pair-input`, `formats/analyze-input`, `formats/output-NIFTI`, `formats/output-NIFTI_PAIR`, `formats/output-NIFTI2`, `formats/output-NIFTI2_GZ`, `formats/output-NIFTI_PAIR_GZ`, `formats/4d-input`, `defects/2d`, `defects/extensions`, `formats/extensions-dropped`, `bet/default`, `bet/options`, `bet/verbose`, `bet/robust`, `bet/robust-n`, `bet/robust-verbose`, `bet/fmri`, `bet/fmri-verbose`, `bet/small-fov`, `bet/small-fov-verbose`, `bet/small-fov-large`, `bet/debug`, `defects/empty-argument`, `defects/cleanup`, `betsurf/t1only`, `betsurf/t1t2`, `betsurf/verbose`, `betsurf/precision`, `defects/short-matrix`, `defects/t2-other-grid`, `betsurf/off-mesh`, `betsurf/fs-mesh`, `real/child-3y`, `real/child-7y`, `real/adult-ds000005`, `real/older-75y`, `real/older-82y`, `real/7t`, `real/tumour`, `real/stroke`, `real/philips`, `real/bold-F`, `real/bold-first`, `difftest/bet-F-double-series`, `difftest/bet-fill-by-index`, `difftest/bet-robust-crash-filled`, `difftest/bet-robust-mesh-echo`, `difftest/bet2-fill-by-index`, `difftest/cog-warning-empty-brain`; 2 by compressed-input (FSL 6.0.7.23 reads zstd and bzip2 compressed images): `formats/head-zst`, `formats/head-bz2`; 9 by output-types (FSL 6.0.7.23 adds the zstd and bzip2 output types (FSLOUTPUTTYPE NIFTI_ZST, NIFTI_BZ2, ...) and lists every type in the message for an unknown one): `formats/output-ANALYZE`, `formats/output-ANALYZE_GZ`, `formats/output-NIFTI_ZST`, `formats/output-NIFTI2_PAIR_ZST`, `formats/output-NIFTI_BZ2`, `formats/output-NIFTI_PAIR_BZ2`, `formats/output-bad`, `bet/zst-output`, `difftest/betsurf-t1only-output-type`; 15 by usage-id (utils 2412.6 prints its build ID in every usage text ('Part of FSL (ID: 2412.6-dirty)'; 2203.5 printed an empty pair of quotes)): `bet2/help`, `bet2/usage`, `bet2/unknown-option`, `bet2/missing-value`, `bet2/bad-number`, `bet2/long-without-equals`, `bet2/unrecognised-token`, `bet2/binary-mask-verbose`, `defects/blank-in-output`, `betsurf/help`, `betsurf/too-few`, `defects/too-few-t1only`, `betsurf/dash-argument`, `betsurf/bad-option`, `betsurf/p-bad`.

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (both succeed, or both refuse); where FSL crashes, larmorx exits 1 |
| Messages | standard output and error byte for byte (larmorx's own lines set aside; the bet script's path read as `bet`) |
| Every file written | byte for byte (images decompressed): header and data, meshes as text |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bet | 23 | 13 | 6 | 4 | 0 |
| bet2 | 36 | 22 | 13 | 1 | 0 |
| betsurf | 14 | 8 | 6 | 0 | 0 |
| defects | 17 | 11 | 1 | 5 | 0 |
| difftest | 13 | 5 | 1 | 7 | 0 |
| fmriprep | 29 | 29 | 0 | 0 | 0 |
| formats | 33 | 29 | 4 | 0 | 0 |
| real | 11 | 11 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `bet2/constant` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-43 and writes its outputs; the default refuses |
| `defects/nan-f` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-42 and writes its outputs; the default refuses |
| `defects/zero-w` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-42 and writes its outputs; the default refuses |
| `bet/pending-S` | needs FSL programs larmorx does not replicate yet: bet: -S (eye and optic nerve cleanup) runs FSL's FLIRT (standard_space_roi and flirt), which larmorx does not replicate yet; this option is pending (larmorx) |
| `bet/pending-B` | needs FSL programs larmorx does not replicate yet: bet: -B (bias field and neck cleanup) runs FSL's FLIRT (standard_space_roi) and FAST (the bias field), which larmorx does not replicate yet; this option is pending (larmorx) |
| `bet/pending-A` | needs FSL programs larmorx does not replicate yet: bet: -A (skull and scalp surfaces) runs FSL's FLIRT (the registration to MNI152), which larmorx does not replicate yet; this option is pending (larmorx) |
| `bet/pending-A2` | needs FSL programs larmorx does not replicate yet: bet: -A2 (skull and scalp surfaces with a T2) runs FSL's FLIRT (the T2-to-T1 and MNI152 registrations), which larmorx does not replicate yet; this option is pending (larmorx) |
| `defects/mesh-unread` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-52 and writes its outputs; the default refuses |
| `defects/mesh-missing-vertex` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-52 and writes its outputs; the default refuses |
| `defects/mesh-truncated` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-52 and writes its outputs; the default refuses |
| `difftest/bet-robust-crash-filled` | FSL crashes (status 1); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-43 and writes its outputs; the default corrects it |
| `difftest/bet-script-failure-constant` | FSL crashes (status 1); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-43 and then stops where FSL never gets (bet2: stopped: the surface's centre lies outside the image (voxel -2147483648 -2147483648 -2147483648), where FSL's bet2 starts its … |
| `difftest/bet2-fill-memory-beyond` | FSL-43: the mask's fill starts outside the image and FSL reads memory beyond the volume, on which its outputs depend; with upstream_defects='reproduce' larmorx does FSL's fill inside the volume and reads that memory as not part of the mask (out.nii.gz: the header is the same; 96 of 96 values differ) |
| `difftest/bet2-filled-then-output-type` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-43 and then stops where FSL never gets (larmorx-nc: upstream defect FSL-43 (bet2, betsurf) reproduced: the fill of the brain mask starts at voxel -2147483648 -2147483648 … |
| `difftest/bet2-n-crash-nothing-asked` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-43 and writes its outputs; the default corrects it |
| `difftest/bet2-verbose-constant-filled` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-43 and writes its outputs; the default corrects it |
| `difftest/betsurf-empty-brain` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-43 and writes its outputs; the default corrects it |

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
| `bet2/binary-mask-missing` | --binaryMask naming no image (FSL aborts) | FSL crashes, larmorx refuses: bet2: stopped: FSL's bet2 aborts here with an uncaught std::runtime_error (Failed to read volume /nonexistent/mask / Error : No i… |
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
| `betsurf/too-few` | too few arguments | both refuse (exit 255) with FSL's usage |
| `betsurf/no-matrix` | a matrix that cannot be opened | both refuse (exit 255) with unable to open /nonexistent.mat |
| `betsurf/no-mesh` | a mesh that cannot be opened | both refuse (exit 255) with terminating. |
| `betsurf/dash-argument` | a word starting with - among the files | both refuse (exit 255) with FSL's usage |
| `betsurf/bad-option` | an unknown option | both refuse (exit 1) with FSL's usage |
| `betsurf/p-bad` | -p with a word | both refuse (exit 1) with FSL's usage |
| `difftest/betsurf-t1only-output-type` | betsurf --t1only with an FSLOUTPUTTYPE FSL does not know (ANALYZE): FSL's FslGetEnvOutputType prints its message and calls exit(1) itself, so t1only_main's dropped status (FSL-54) does not apply; the replica's reproduce had exited 0 (fixed 2026-10-26) | both refuse (exit 1) with Legal values are: NIFTI, NIFTI_PAIR, NIFTI2, NIFTI2_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ, NIFTI2_GZ, NIFTI2_PAIR_GZ, NIFTI_ZST, NIFTI_PAIR_Z… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (caefd642193a) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs run in directories of their own on the same input files, with `PATH=/usr/bin:/bin` (FSL never on it), `FSLDIR` set, an empty `HOME` and the case's `FSLOUTPUTTYPE`. The replica is compared in `reproduce` mode; the fix pass runs its default. The fMRIPrep cases also compare the replica with fMRIPrep's own outputs of the node and run FSL again and fMRIPrep 25.2.6's FSL (FSL's own variability). The `-S`, `-B`, `-A` and `-A2` variations of `bet` run FSL programs larmorx does not replicate yet (standard_space_roi, FLIRT, FAST): the replica refuses them, naming what is missing. FSL version: 6.0.7.23 (`--fsl-version`, `LARMORX_FSL_VERSION`; installations: 6.0.7.23 `~/fsl-6.0.7.23`, 6.0.7.17 `~/fsl`). Each case also ran FSL 6.0.7.17 on the same arguments (from the oracle-output cache or its recording where an earlier record ran it): the check 'FSL's own step: 6.0.7.17 → 6.0.7.23' says whether FSL's own exit status, messages and outputs changed, and why.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds000005-task_mixedgamblestask_run_01-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-mixedgamblestask_run-01_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000005-task_mixedgamblestask_run_02-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-mixedgamblestask_run-02_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000005-task_mixedgamblestask_run_03-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-mixedgamblestask_run-03_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000228-task_pixar-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-pixar008_task-pixar_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000256-task_restbaseline-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-CTS203_task-restbaseline_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000256-task_restfixationfixed-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-CTS203_task-restfixationfixed_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds000256-task_restfixationnofeedback-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-CTS203_task-restfixationnofeedback_desc-hmc_boldref_corrected.nii… |
| `fmriprep/ds002422-task_arithm_run_01-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-arithm_run-01_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds002422-task_arithm_run_01-skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-arithm_run-01_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds002422-task_arithm_run_02-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-arithm_run-02_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds002422-task_arithm_run_02-skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-arithm_run-02_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds002422-task_rest-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-rest_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds002422-task_rest-skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-02_task-rest_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds002603-task_wm_acq_Mb4Mm3Tr550-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-wm_acq-Mb4Mm3Tr550_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds002603-task_wm_acq_Mb4Mm3Tr550-skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-wm_acq-Mb4Mm3Tr550_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds002603-task_wm_acq_SeqMm3Tr2000-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-wm_acq-SeqMm3Tr2000_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds002603-task_wm_acq_SeqMm3Tr2000-skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-01_task-wm_acq-SeqMm3Tr2000_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds003592-ses_1_task_rest_echo_1-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-280_ses-1_task-rest_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds004884-ses_217_task_naming40_acq_epfid2_dir_AP_run_2-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-M2079_ses-217_task-naming40_acq-epfid2_dir-AP_run-2_desc-hmc_bold… |
| `fmriprep/ds004884-ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-M2079_ses-217_task-rest_acq-epfidp2p2_dir-AP_run-9_desc-hmc_boldr… |
| `fmriprep/ds004958-task_rest-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-fsm64ps_task-rest_sbref_corrected.nii.gz) |
| `fmriprep/ds005003-ses_1_task_motor-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-RS001_ses-1_task-motor_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds005003-ses_1_task_rest-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-RS001_ses-1_task-rest_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds005040-task_clip_run_1-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-002_task-clip_run-1_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds005040-task_clip_run_1-skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-002_task-clip_run-1_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds005454-task_rest-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-16_task-rest_desc-hmc_boldref_corrected.nii.gz) |
| `fmriprep/ds005454-task_rest-skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-16_task-rest_desc-coreg_boldref.nii.gz) |
| `fmriprep/ds005752-ses_01_task_rest_dir_forward-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-ON01016_ses-01_task-rest_dir-forward_desc-hmc_boldref_corrected.n… |
| `fmriprep/ds005752-ses_01_task_rest_dir_reverse-enhance_and_skullstrip_bold` | pass | 13/13 | fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` (sub-ON01016_ses-01_task-rest_dir-reverse_desc-hmc_boldref_corrected.n… |
| `bet2/default` | pass | 8/8 | bet2's defaults (-f 0.5) |
| `bet2/all-outputs` | pass | 16/16 | -o -m -s -e -v: every output, verbose |
| `bet2/f-0.1` | pass | 9/9 | -f 0.1 |
| `bet2/f-0.3` | pass | 9/9 | -f 0.3 |
| `bet2/f-0.7` | pass | 9/9 | -f 0.7 |
| `bet2/f-0.9` | pass | 9/9 | -f 0.9 |
| `bet2/g-0.4` | pass | 7/7 | -g -0.4 |
| `bet2/g0.4` | pass | 7/7 | -g 0.4 |
| `bet2/r` | pass | 7/7 | -r 60 (the radius) |
| `bet2/c` | pass | 7/7 | -c: the centre in voxels |
| `bet2/w` | pass | 7/7 | -w 2 and -w 0.5 (smoothness) |
| `bet2/w-0.5` | pass | 7/7 | -w 0.5 |
| `bet2/t` | pass | 7/7 | -t: thresholding |
| `bet2/n` | pass | 6/6 | -n: no brain image |
| `bet2/ct` | pass | 7/7 | --ct: CT intensities |
| `bet2/d1d2` | pass | 7/7 | the hidden --parameter_d1/d2 |
| `bet2/short-run` | pass | 7/7 | a run of one-letter options (-mno) |
| `bet2/repeated` | pass | 7/7 | an option given twice keeps the last |
| `bet2/t2` | pass | 7/7 | the T2-like head |
| `bet2/help` | pass | 5/5 | -h |
| `bet2/usage` | both-error | 5/5 | no arguments |
| `bet2/one-argument` | both-error | 5/5 | one argument |
| `bet2/dash-names` | both-error | 5/5 | an output name starting with - |
| `bet2/unknown-option` | both-error | 5/5 | an unknown option |
| `bet2/missing-value` | both-error | 5/5 | -f without its value |
| `bet2/bad-number` | both-error | 5/5 | -f with a word |
| `bet2/long-without-equals` | both-error | 5/5 | --parameter_d1 5: long options need = |
| `bet2/f-out-of-range` | both-error | 5/5 | -f 1.5 |
| `bet2/g-out-of-range` | both-error | 5/5 | -g -2 |
| `bet2/unrecognised-token` | both-error | 5/5 | a third word |
| `bet2/missing-input` | both-error | 5/5 | an input that does not exist (FSL aborts) |
| `bet2/img-output` | both-error | 5/5 | an output name with .img and no .hdr (FSL-40: FSL aborts) |
| `defects/hdr-in-name` | pass | 7/7 | an output name with .hdr inside (FSL-40) |
| `bet2/constant` | expected-divergence | 7/7 | a constant image: FSL 6.0.7.23 takes the robust maximum as the median (FSL-41, fixed upstream; 6.0.7.17 read an empty l… |
| `bet2/binary-mask` | pass | 9/9 | --binaryMask: the surface found on a brain mask, the brain, mask, outline, skull and thresholds from the input (FSL 6.0… |
| `bet2/binary-mask-verbose` | pass | 7/7 | --binaryMask -v: the parameters printed are the mask's |
| `defects/binary-mask-ct` | pass | 7/7 | --ct --binaryMask: FSL cuts the brain image from the mask (FSL-62) |
| `bet2/binary-mask-missing` | both-error | 5/5 | --binaryMask naming no image (FSL aborts) |
| `defects/nan-f` | expected-divergence | 7/7 | -f nan (FSL-42; FSL crashes in its fill, FSL-43) |
| `defects/negative-r` | pass | 7/7 | -r -50 (FSL-42) |
| `defects/zero-w` | expected-divergence | 7/7 | -w 0 (FSL-42; FSL crashes in its fill, FSL-43) |
| `formats/head-i16` | pass | 7/7 | head-i16: brain and mask |
| `formats/head-u8` | pass | 7/7 | head-u8: brain and mask |
| `formats/head-i8` | pass | 7/7 | head-i8: brain and mask |
| `formats/head-u16` | pass | 7/7 | head-u16: brain and mask |
| `formats/head-i32` | pass | 7/7 | head-i32: brain and mask |
| `formats/head-f64` | pass | 7/7 | head-f64: brain and mask |
| `formats/head-i16-scaled` | pass | 7/7 | head-i16-scaled: brain and mask |
| `formats/head-neuro` | pass | 7/7 | head-neuro: brain and mask |
| `formats/head-qform-only` | pass | 7/7 | head-qform-only: brain and mask |
| `formats/head-sform-only` | pass | 7/7 | head-sform-only: brain and mask |
| `formats/head-noxform` | pass | 7/7 | head-noxform: brain and mask |
| `formats/head-oblique` | pass | 7/7 | head-oblique: brain and mask |
| `formats/head-nifti2` | pass | 7/7 | head-nifti2: brain and mask |
| `formats/head-uncompressed` | pass | 7/7 | head-uncompressed: brain and mask |
| `formats/head-zst` | pass | 7/7 | head-zst: brain and mask |
| `formats/head-bz2` | pass | 7/7 | head-bz2: brain and mask |
| `formats/pair-input` | pass | 7/7 | a NIfTI pair input (.hdr/.img) |
| `formats/analyze-input` | pass | 7/7 | an ANALYZE 7.5 input |
| `formats/output-NIFTI` | pass | 8/8 | FSLOUTPUTTYPE=NIFTI |
| `formats/output-NIFTI_PAIR` | pass | 11/11 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `formats/output-NIFTI2` | pass | 8/8 | FSLOUTPUTTYPE=NIFTI2 |
| `formats/output-NIFTI2_GZ` | pass | 8/8 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `formats/output-NIFTI_PAIR_GZ` | pass | 11/11 | FSLOUTPUTTYPE=NIFTI_PAIR_GZ |
| `formats/output-ANALYZE` | both-error | 5/5 | FSLOUTPUTTYPE=ANALYZE |
| `formats/output-ANALYZE_GZ` | both-error | 5/5 | FSLOUTPUTTYPE=ANALYZE_GZ |
| `formats/output-NIFTI_ZST` | pass | 8/8 | FSLOUTPUTTYPE=NIFTI_ZST |
| `formats/output-NIFTI2_PAIR_ZST` | pass | 11/11 | FSLOUTPUTTYPE=NIFTI2_PAIR_ZST |
| `formats/output-NIFTI_BZ2` | pass | 8/8 | FSLOUTPUTTYPE=NIFTI_BZ2 |
| `formats/output-NIFTI_PAIR_BZ2` | pass | 11/11 | FSLOUTPUTTYPE=NIFTI_PAIR_BZ2 |
| `formats/output-unset` | both-error | 5/5 | FSLOUTPUTTYPE unset (FSL stops) |
| `formats/output-bad` | both-error | 5/5 | FSLOUTPUTTYPE=JPEG |
| `formats/4d-input` | pass | 7/7 | a 4D input: its first volume, with FSL's warning |
| `defects/2d` | pass | 7/7 | a single slice: the mask's fill starts below the image (FSL-43) |
| `defects/extensions` | pass | 7/7 | two extensions of different sizes kept (FSL_LOAD_NIFTI_EXTENSIONS=1, FSL-56) |
| `formats/extensions-dropped` | pass | 7/7 | extensions in the input, not kept by default |
| `bet/default` | pass | 6/6 | bet's defaults |
| `bet/zst-output` | pass | 7/7 | FSLOUTPUTTYPE=NIFTI_ZST through the script (fslval, fslmaths on zstd files; FSL 6.0.7.23) |
| `bet/options` | pass | 10/10 | every bet2 option bet passes on |
| `bet/verbose` | pass | 7/7 | -v: the script's variables, then bet2's messages |
| `bet/robust` | pass | 9/9 | -R: bet2 again from the brain's centre |
| `bet/robust-n` | pass | 7/7 | -R -n: the script removes -n |
| `bet/robust-verbose` | pass | 6/6 | -R -v |
| `bet/fmri` | pass | 9/9 | -F: the first volume's mask, dilated, on every volume |
| `bet/fmri-verbose` | pass | 7/7 | -F -v -f 0.4 |
| `bet/small-fov` | pass | 7/7 | -Z: a slab padded to 100 mm (FSL-45: the crop back needs imglob) |
| `bet/small-fov-verbose` | pass | 8/8 | -Z -v -s |
| `bet/small-fov-large` | pass | 7/7 | -Z on an image already 100 mm high: no padding |
| `bet/debug` | pass | 9/9 | -d: the temporary files kept |
| `bet/pending-S` | expected-divergence | 1/1 | -S: needs FSL programs larmorx does not replicate yet (eyes) |
| `bet/pending-B` | expected-divergence | 1/1 | -B: needs FSL programs larmorx does not replicate yet (bias) |
| `bet/pending-A` | expected-divergence | 1/1 | -A: needs FSL programs larmorx does not replicate yet (surfaces) |
| `bet/pending-A2` | expected-divergence | 1/1 | -A2 <T2>: pending (FLIRT) |
| `bet/usage` | both-error | 5/5 | bet without an output |
| `bet/no-input` | both-error | 5/5 | an input that does not exist |
| `bet/unknown-option` | both-error | 5/5 | an unknown option |
| `bet/missing-value` | both-error | 5/5 | -f without its value |
| `bet/c-short` | both-error | 5/5 | -c with two numbers |
| `bet/bet2-fails` | both-error | 5/5 | bet2 refuses (-f 2): the script's ERR trap |
| `defects/blank-in-output` | both-error | 5/5 | an output name with a blank (FSL-44) |
| `defects/empty-argument` | pass | 6/6 | an empty argument ends the options (FSL-44) |
| `defects/cleanup` | pass | 7/7 | the cleanup glob removes out_tmp* files it did not create (FSL-46) |
| `betsurf/t1only` | pass | 17/17 | --t1only -o -m -s: bet -A's call |
| `betsurf/t1t2` | pass | 15/15 | -o -m -s with the T2: bet -A2's call |
| `betsurf/verbose` | pass | 11/11 | -v -m |
| `betsurf/precision` | pass | 11/11 | -p 1: retessellated |
| `betsurf/surfaces-only` | pass | 8/8 | no option: the meshes only |
| `betsurf/help` | pass | 5/5 | -h |
| `betsurf/too-few` | both-error | 5/5 | too few arguments |
| `defects/too-few-t1only` | pass | 5/5 | --t1only with too few arguments exits 0 (FSL-54) |
| `betsurf/no-matrix` | both-error | 5/5 | a matrix that cannot be opened |
| `defects/no-matrix-t1only` | pass | 5/5 | --t1only with a missing matrix exits 0 (FSL-54) |
| `betsurf/no-mesh` | both-error | 5/5 | a mesh that cannot be opened |
| `betsurf/dash-argument` | both-error | 5/5 | a word starting with - among the files |
| `betsurf/bad-option` | both-error | 5/5 | an unknown option |
| `betsurf/p-bad` | both-error | 5/5 | -p with a word |
| `defects/short-matrix` | pass | 11/11 | a matrix of 6 numbers (FSL-53) |
| `defects/mesh-unread` | expected-divergence | 7/7 | a coordinate that does not read (FSL-52: FSL crashes; reproduce fills what FSL leaves undefined) |
| `defects/mesh-missing-vertex` | expected-divergence | 7/7 | a face naming a vertex the mesh does not have (FSL-52: FSL crashes; reproduce fills what FSL leaves undefined) |
| `defects/mesh-truncated` | expected-divergence | 7/7 | a mesh file that ends after two vertices (FSL-52: FSL crashes; reproduce fills what FSL leaves undefined) |
| `defects/t2-other-grid` | pass | 11/11 | a T2 on another grid (FSL-48) |
| `betsurf/off-mesh` | pass | 11/11 | an OFF mesh |
| `betsurf/fs-mesh` | pass | 11/11 | a FreeSurfer ASCII mesh |
| `real/child-3y` | pass | 9/9 | a child of 3.5 years (ds000228): bet's defaults and -m |
| `real/child-7y` | pass | 9/9 | a child of 7.5 years with motion (ds000256): bet's defaults and -m |
| `real/adult-ds000005` | pass | 9/9 | an adult, the fMRIPrep oracle run's T1w (ds000005): bet's defaults and -m |
| `real/older-75y` | pass | 9/9 | an adult of 75 years at 1.5 T (ds002422): bet's defaults and -m |
| `real/older-82y` | pass | 9/9 | an adult of 82 years (ds003592): bet's defaults and -m |
| `real/7t` | pass | 9/9 | 7 T, MP2RAGE (ds005454): bet's defaults and -m |
| `real/tumour` | pass | 9/9 | a tumour (ds005003): bet's defaults and -m |
| `real/stroke` | pass | 9/9 | a stroke (ds004884): bet's defaults and -m |
| `real/philips` | pass | 9/9 | a Philips scanner (ds002603): bet's defaults and -m |
| `real/bold-F` | pass | 7/7 | a BOLD series (ds000005): bet -F |
| `real/bold-first` | pass | 7/7 | a BOLD series (ds000005): bet2 on its first volume (FSL's warning) |
| `difftest/bet-F-double-series` | pass | 7/7 | bet -F on a series stored as double: fslmaths -mas computes in double and writes double for a double input; the replica… |
| `difftest/bet-fill-by-index` | pass | 8/8 | bet -f 0.7 -g -0.3 -m -o on a 22 x 18 x 21 image of 0.01 mm slices: the mask's fill starts 86 slices above the image an… |
| `difftest/bet-robust-crash-filled` | expected-divergence | 7/7 | bet -R -g 0.5 -e: FSL's second bet2 crashes in a fill that starts far above the image, and the script's ERR trap prints… |
| `difftest/bet-robust-mesh-echo` | pass | 6/6 | bet -R -e: the script passes bet2's options through bash's echo, which takes the leading -e as its own option, so FSL w… |
| `difftest/bet-script-failure-constant` | expected-divergence | 6/6 | bet on a constant one-voxel image: FSL's bet2 crashes (FSL-41) and the bet script's ERR trap prints 'bet failed during … |
| `difftest/bet2-fill-by-index` | pass | 6/6 | bet2 -r 60 on a 3 x 6 x 4 head: the mask's fill starts four rows before the image; FSL reaches each point by its linear… |
| `difftest/bet2-fill-memory-beyond` | expected-divergence | 6/6 | bet2 -c 22 19 8 on a 3 x 4 x 8 ramp: the fill starts outside the image and FSL reads memory beyond the volume (6.0.7.17… |
| `difftest/bet2-filled-then-output-type` | expected-divergence | 6/6 | bet2 on a constant image with an FSLOUTPUTTYPE FSL does not know: FSL crashes first (FSL-41); reproduce fills the media… |
| `difftest/bet2-n-crash-nothing-asked` | expected-divergence | 7/7 | bet2 -n alone (no output asked) where FSL crashes in the mask's fill (a constant image: the surface's centre is not a n… |
| `difftest/bet2-verbose-constant-filled` | expected-divergence | 7/7 | bet2 -v on a constant one-voxel image: FSL crashes (FSL-41) and its buffered verbose lines are lost; reproduce fills th… |
| `difftest/betsurf-empty-brain` | expected-divergence | 7/7 | betsurf --t1only on a constant one-voxel image: no voxel of the T1 lies inside the brain surface, and FSL reads the 98t… |
| `difftest/betsurf-t1only-output-type` | both-error | 6/6 | betsurf --t1only with an FSLOUTPUTTYPE FSL does not know (ANALYZE): FSL's FslGetEnvOutputType prints its message and ca… |
| `difftest/cog-warning-empty-brain` | pass | 6/6 | bet -R on a small head whose brain comes out empty: FSL's fslstats -C prints newimage's 'WARNING::in calculating COG, t… |
