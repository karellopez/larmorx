# Parity: BET (clean-room original)

**2 case(s) fail.** 252 cases on the `standard` tier: 198 pass, 46 rejected by both, 6 expected divergences, 2 failures.

- **Validated:** `larmorx fsl bet2`, `bet`, `betsurf`, `standard_space_roi` and `fslroi` (larmorx 0.0.1, crate larmorx-fsl: the clean-room originals written from specs/bet.md and specs/standard_space_roi.md, Apache-2.0), in process, and `lx.fsl.bet` / `lx.fsl.betsurf` / `lx.fsl.standard_space_roi` / `lx.fsl.fslroi(..., implementation='original')`
- **Reference:** bet, bet2, betsurf, standard_space_roi and fslroi from FSL 6.0.7.23 (fsl-bet2 2111.9, fsl-newimage 2601.0, fsl-meshclass 2111.0), the conda installation in `~/fsl-6.0.7.23`
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-05 on Linux x86_64, with `python -m larmorx_validation parity fsl-bet --tier standard`
- **Original programs:** 353 runs: 339 from the oracle cache, 14 run (6 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

- **198 of 252 cases within the thresholds** (exit status and messages identical, every file within its threshold); 46 where both refuse or FSL crashes and larmorx refuses; 6 where FSL crashes and `reproduce` fills what FSL leaves undefined (D13's failure cases); 0 pending (options that run FSL programs larmorx does not have yet).
- **378 of 380 brains, masks and outlines with FSL's values, bit for bit**, 0 more within the thresholds; 11 of 11 meshes with FSL's vertices to the file's 6 decimals; 28 of 28 skull images bit-identical, 0 more within the masks' thresholds.
- **betsurf: 34 of 116 masks and outlines with FSL's values, bit for bit**, 80 more within its thresholds (Dice >= 0.99, boundary p95 <= 1.0 voxel); its 63 meshes reported: vertex distance p95 0.0000 to 2.8577 mm, largest 16.86 mm.
- fMRIPrep's own BET outputs: 58 of 58 with the same values as the original's.
- FSL run again: 38 of 38 runs bit-identical (FSL is deterministic).
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - FSL-09: 4 case(s) named it, 4 with changed values (largest difference 630), 1 with other changes (out_tmp_to_std.mat changed; out_tmp_to_std_inv.mat changed): `real/adult-ds000005-S`, `real/older-82y-B`, `ssroi/adult-default`, `ssroi/older-maskfov`
  - FSL-182: 8 case(s) named it, 8 with changed values (largest difference 2.28e+06), 2 with other changes (out.img changed; out_mask.img changed; out_tmp_premask_tmp_to_std.mat changed; out_tmp_premask_tmp_to_std_inv.mat changed): `bet/eyes`, `bet/eyes-debug`, `bet/eyes-verbose`, `bet/eyes-outline-mesh`, `bet/eyes-pair`, `defects/eyes-threshold-exponent`, `defects/eyes-negative-mean`, `real/tumour-S`
  - FSL-202: 7 case(s) named it, 7 with changed values (largest difference 880), 2 with other changes (files ['out.nii.gz', 'out_mask.nii.gz', 'out_skull.nii.gz'] became ['out.nii.gz', 'out_mask.nii.gz', 'out_mesh.vtk', 'out_skull.nii.gz']; out_tmp_premask_tmp_to_std.mat changed; out_tmp_premask_tmp_to_std_inv.mat changed; out_tmp_unbiased_premask_tmp_to_std.mat changed; out_tmp_unbiased_premask_tmp_to_std_inv.mat changed): `bet/bias`, `bet/bias-debug`, `bet/bias-skull`, `bet/bias-n`, `defects/bias-skull-echo`, `real/child-3y-B`, `real/older-82y-B`
  - FSL-211: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit status N became N; files [] became ['out put.nii.gz']): `defects/output-blank`, `defects/option-blank`
  - FSL-212: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the files present before the run: [] kept, ['out_tmp_notes.txt'] by the default): `defects/cleanup-glob`
  - FSL-213: 1 case(s) named it, 1 with changed values (largest difference 31): `defects/maskthr-env`
  - FSL-214: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit status N became N): `defects/flirt-refuses`, `defects/option-blank`
  - FSL-215: 4 case(s) named it, 0 with changed values (largest difference 0), 4 with other changes (exit status N became N): `fslroi/invalid-roi`, `fslroi/time-out-of-bounds`, `fslroi/missing-input`, `fslroi/words`
  - FSL-216: 2 case(s) named it, 2 with changed values (largest difference 100): `defects/eyes-negative-mean`, `defects/eyes-nan`
  - FSL-217: 6 case(s) named it, 6 with changed values (largest difference inf): `bet/fmri-int8`, `bet/small-fov-int8`, `bet/small-fov-int8-debug`, `fslroi/type-head-i8`, `fslroi/int8-negative`, `fslroi/int8-positive`
  - FSL-40: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit status N became N; files ['ab.nii.gz', 'ab_mask.nii.gz'] became ['a.hdrb.nii.gz', 'a.hdrb_mask.nii.gz']; files [] became ['out.nii.gz']): `bet2/img-output`, `defects/hdr-in-name`
  - FSL-42: 3 case(s) named it, 3 with changed values (largest difference 0): `defects/nan-f`, `defects/negative-r`, `defects/zero-w`
  - FSL-43: 2 case(s) named it, 1 with changed values (largest difference 0): `bet2/constant`, `defects/2d`
  - FSL-44: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit status N became N; files ['out.nii.gz'] became ['out.nii.gz', 'out_mask.nii.gz']; files [] became ['my out.nii.gz', 'my out_mask.nii.gz']): `defects/blank-in-output`, `defects/empty-argument`
  - FSL-45: 5 case(s) named it, 5 with changed values (largest difference inf): `bet/small-fov`, `bet/small-fov-verbose`, `bet/debug`, `bet/small-fov-int8`, `bet/small-fov-int8-debug`
  - FSL-46: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the files present before the run: [] kept, ['out_tmp_notes.txt'] by the default): `defects/cleanup`
  - FSL-48: 1 case(s) named it, 1 with changed values (largest difference 0): `defects/t2-other-grid`
  - FSL-51: 4 case(s) named it, 4 with changed values (largest difference 1), 4 with other changes (out_inskull_mesh.vtk changed; out_outskin_mesh.vtk changed; out_outskull_mesh.vtk changed): `real/child-7y-A2`, `real/child-7y-surf-t2`, `real/tumour-surf-t2`, `real/philips-surf-t2`
  - FSL-52: 3 case(s) named it, 3 with changed values (largest difference 0): `defects/mesh-unread`, `defects/mesh-missing-vertex`, `defects/mesh-truncated`
  - FSL-53: 1 case(s) named it, 1 with changed values (largest difference 0): `defects/short-matrix`
  - FSL-54: 2 case(s) named it, 0 with changed values (largest difference 0), 2 with other changes (exit status N became N): `defects/too-few-t1only`, `defects/no-matrix-t1only`
  - FSL-56: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (files ['out.nii', 'out_mask.nii'] became ['out.nii', 'out_mask.nii']; out.nii changed; out_mask.nii changed): `defects/extensions`
  - FSL-62: 1 case(s) named it, 1 with changed values (largest difference 1e+03): `defects/binary-mask-ct`
  - FSL-63: 1 case(s) named it, 1 with changed values (largest difference 719), 1 with other changes (files ['out.nii.gz', 'out_mask.nii.gz', 'out_skull.nii.gz'] became ['out.nii.gz', 'out_mask.nii.gz', 'out_mesh.vtk', 'out_skull.nii.gz']): `defects/bias-skull-echo`
  - FSL-68: 1 case(s) named it, 0 with changed values (largest difference 0): `bet/surfaces-t2-missing`
  - GLIBC-07 (not named): 10 case(s) named it, 10 with changed values (largest difference 1.42e+05), 7 with other changes (out_inskull_mesh.vtk changed; out_outskin_mesh.vtk changed; out_outskull_mesh.vtk changed; out_tmp_TN_to_std.mat changed; out_tmp_to_std.mat changed; out_tmp_to_std_inv.mat changed): `bet/surfaces`, `bet/surfaces-debug`, `bet/surfaces-t2`, `bet/surfaces-t2-debug`, `bet/last-variation`, `real/older-75y-A`, `ssroi/phantom-flirt-options`, `ssroi/child-b` …

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
| Skull | as the masks: bit-identical, or Dice and boundary distance within the masks' thresholds (specs/bet.md §4.7 gives the skull search exactly) |
| betsurf's masks and outlines | bit-identical, or Dice >= 0.99 and the boundary distance's 95th percentile <= 1.0 voxel (specs/bet.md §11; the outlines: the drawn surfaces' distance, likewise); its three meshes reported (vertex distance) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bet | 40 | 31 | 9 | 0 | 0 |
| bet2 | 36 | 22 | 13 | 1 | 0 |
| betsurf | 14 | 8 | 6 | 0 | 0 |
| defects | 27 | 18 | 4 | 5 | 0 |
| fmriprep | 29 | 29 | 0 | 0 | 0 |
| formats | 33 | 29 | 4 | 0 | 0 |
| fslroi | 26 | 24 | 2 | 0 | 0 |
| real | 22 | 20 | 0 | 0 | 2 |
| ssroi | 25 | 17 | 8 | 0 | 0 |

## Failures

### `real/child-3y-B`

a 3.5-year-old (ds000228): bet -B -m (the bias field, the neck)

- **stage by stage (-d): the first file that differs** (file = none, threshold FLIRT's matrices, FAST's bias field or FLIRT's resampled images (their programs' own thresholds); else the step that made it): every temporary file FSL's
- **out.nii.gz** (Dice; boundary distance p95 / max (voxels) = 0.998917; 0.00 / 2.24, threshold >= 0.995; <= 0.5 / 2.0; values identical inside both): 2439 of 6488064 values differ, 0 inside both
- **out_mask.nii.gz** (Dice; boundary distance p95 / max (voxels) = 0.998305; 1.00 / 2.24, threshold >= 0.995; <= 0.5 / 2.0): 3890 of 6488064 values differ

### `real/philips-surf-t2`

a Philips scanner (ds002603): betsurf -o -m -s with the T2 FLIRT registered

- **out_outskin_mask.nii.gz** (Dice; boundary distance p95 / max (voxels) = 0.993521; 2.00 / 14.25, threshold >= 0.99; p95 <= 1.0): 49022 of 16384000 values differ
- **out_outskin_mesh.nii.gz** (drawn surface distance p95 / max (voxels) = 1.73 / 14.25, threshold p95 <= 1.0): 56411 of 16384000 values differ

## Expected divergences

| Case | Reason |
|---|---|
| `bet2/constant` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-43 and writes its outputs; the default refuses |
| `defects/nan-f` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-42 and writes its outputs; the default refuses |
| `defects/zero-w` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-42 and writes its outputs; the default refuses |
| `defects/mesh-unread` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-52 and writes its outputs; the default refuses |
| `defects/mesh-missing-vertex` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-52 and writes its outputs; the default refuses |
| `defects/mesh-truncated` | FSL crashes (status -11); with upstream_defects='reproduce' larmorx fills what FSL leaves undefined deterministically, names FSL-52 and writes its outputs; the default refuses |

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
| `bet/eyes-no-brain` | -S -n: the raw brain is not written and fslmaths aborts (the ERR trap) | FSL crashes, larmorx refuses: larmorx fsl fslmaths: FSL aborts here (signal 6); exit status 1 (larmorx) |
| `bet/surfaces-t2-missing` | -A2 of a T2 that does not exist: FLIRT fails (the ERR trap) | FSL crashes, larmorx refuses: larmorx fsl betsurf: out_tmp_T2_to_T1 cannot be read; FSL aborts here (larmorx) |
| `bet/surfaces-t2-no-value` | -A2 as the last argument: the usage | both refuse (exit 1) with FSL's usage |
| `defects/eyes-nan` | -S with NaN voxels in the brain: FLIRT inside standard_space_roi fails (FSL-14; convert_xfm aborts, the script goes on), bet2 then aborts (FSL-216's threshold is not reached) | FSL crashes, larmorx refuses: larmorx fsl fslmaths: FSL aborts here (signal 6); exit status 1 (larmorx) |
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
| `fslroi/usage` | three arguments: the usage (exit 1) | both refuse (exit 1) with FSL's usage |
| `fslroi/no-arguments` | no arguments: the usage | both refuse (exit 1) with FSL's usage |
| `ssroi/usage` | one argument: the usage (exit 1; FSLDIR in its text) | both refuse (exit 1) with FSL's usage |
| `ssroi/invalid-input` | an input that does not exist | both refuse (exit 1) with Input image /nonexistent/in not valid |
| `ssroi/empty-input` | an empty input: remove_ext's usage becomes its name (FSL-211) | both refuse (exit 1) with FSL's usage |
| `ssroi/invalid-ssref` | -ssref of an image that does not exist | both refuse (exit 1) with standard space reference image not valid |
| `ssroi/ssref-without-value` | -ssref as the last argument: the usage | both refuse (exit 1) with FSL's usage |
| `ssroi/invalid-altinput` | -altinput of an image that does not exist | both refuse (exit 1) with Input image /nonexistent/x not valid |
| `ssroi/invalid-roimask` | -roiMASK of an image that does not exist | both refuse (exit 1) with Image /nonexistent/x not valid |
| `ssroi/invalid-maskmask` | -maskMASK of an image that does not exist | both refuse (exit 1) with Image /nonexistent/x not valid |
| `defects/output-blank` | an output name with a blank: split into words (FSL-211) | both refuse (exit 1) with FSL's usage |
| `defects/input-blank` | an input name with a blank: not valid (FSL-211) | both refuse (exit 1) with Input image <generated>/phantoms/head-t2 x not valid |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (2b3db9eda759) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

Both programs run in directories of their own on the same input files, with `PATH=/usr/bin:/bin` (FSL never on it), `FSLDIR` set, an empty `HOME` and the case's `FSLOUTPUTTYPE`. The original is compared in `reproduce` mode; the fix pass runs its default. The fMRIPrep cases also run FSL again and fMRIPrep 25.2.6's FSL (FSL's own variability) and report the original's agreement with fMRIPrep's own outputs. betsurf's real images (the full tier's for the replica) run in this standard tier, on bet2's meshes and FLIRT's matrices made by FSL. The `-S`, `-B`, `-A` and `-A2` variations of `bet` have no clean-room original yet (the replica runs them; the original's catch-up, from `specs/bet.md` §5.3): the original refuses them, naming what is missing. FSL 6.0.7.23 (the `--fsl-version` default).

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
| `bet/fmri-int8` | pass | 8/8 | -F of an int8 head with negative values: fslroi writes uint8 (FSL-217), bet2 runs on 128 to 255 |
| `bet/fmri-int8-positive` | pass | 8/8 | -F of an int8 head without negative values: the same values either way (FSL-217 not named) |
| `bet/small-fov-int8` | pass | 8/8 | -Z of an int8 slab with negative values: fslroi's slices and fslmerge (FSL-217) |
| `bet/small-fov-int8-debug` | pass | 12/12 | -Z -d of an int8 slab: the int8 temporary files kept (FSL-217) |
| `bet/eyes` | pass | 13/13 | -S: the eyes and optic nerves removed (standard_space_roi -b, FLIRT, fslmaths, fslstats, dc) |
| `bet/eyes-debug` | pass | 38/38 | -S -d: every temporary file kept (standard_space_roi's too) |
| `bet/eyes-verbose` | pass | 10/10 | -S -v -t -f 0.4: the options passed to both bet2 runs (the later -f wins) |
| `bet/eyes-outline-mesh` | pass | 10/10 | -S -o -e: the outline and mesh only in the temporary bet2 run |
| `bet/eyes-no-brain` | both-error | 26/26 | -S -n: the raw brain is not written and fslmaths aborts (the ERR trap) |
| `bet/eyes-pair` | pass | 10/10 | -S with FSLOUTPUTTYPE=NIFTI_PAIR (every program's output a pair) |
| `bet/bias` | pass | 10/10 | -B: FAST's bias field removed, the neck cut (standard_space_roi -b twice), bet -R |
| `bet/bias-debug` | pass | 48/48 | -B -d: FAST's and standard_space_roi's files kept |
| `bet/bias-skull` | pass | 10/10 | -B -s -v: the skull from standard_space_roi -maskFOV -roiNONE and bet2, immv |
| `bet/bias-n` | pass | 10/10 | -B -n -o: the robust run removes -n (the brain written) |
| `bet/surfaces` | pass | 27/27 | -A: FLIRT to MNI152, bet2 -e, betsurf --t1only -o -m -s |
| `bet/surfaces-debug` | pass | 27/27 | -A -d -m: FLIRT's matrix kept |
| `bet/surfaces-t2` | pass | 26/26 | -A2 <T2>: FLIRT of the T2 (sinc), FLIRT to MNI152, bet2 -e, betsurf -o -m -s |
| `bet/surfaces-t2-debug` | pass | 28/28 | -A2 <T2> -d -v |
| `bet/surfaces-t2-missing` | both-error | 8/8 | -A2 of a T2 that does not exist: FLIRT fails (the ERR trap) |
| `bet/surfaces-t2-no-value` | both-error | 4/4 | -A2 as the last argument: the usage |
| `bet/last-variation` | pass | 24/24 | -B -R -S -A: the last variation given (-A) |
| `defects/eyes-threshold-exponent` | pass | 10/10 | -S on intensities above 1e6: fslstats prints 2.1e+06 and dc reads the exponent as commands (FSL-216) |
| `defects/eyes-negative-mean` | pass | 10/10 | -S on a head whose brain has a negative mean: dc reads fslstats' minus sign as its subtraction, with too few numbers on… |
| `defects/eyes-nan` | both-error | 13/13 | -S with NaN voxels in the brain: FLIRT inside standard_space_roi fails (FSL-14; convert_xfm aborts, the script goes on)… |
| `defects/bias-skull-echo` | pass | 10/10 | -B -e -s: the skull's sed passes bet2's options through echo, which takes -e (FSL-63) |
| `bet/usage` | both-error | 4/4 | bet without an output |
| `bet/no-input` | both-error | 4/4 | an input that does not exist |
| `bet/unknown-option` | both-error | 4/4 | an unknown option |
| `bet/missing-value` | both-error | 4/4 | -f without its value |
| `bet/c-short` | both-error | 4/4 | -c with two numbers |
| `bet/bet2-fails` | both-error | 4/4 | bet2 refuses (-f 2): the script's ERR trap |
| `defects/blank-in-output` | both-error | 4/4 | an output name with a blank (FSL-44) |
| `defects/empty-argument` | pass | 6/6 | an empty argument ends the options (FSL-44) |
| `defects/cleanup` | pass | 8/8 | the cleanup glob removes out_tmp* files it did not create (FSL-46) |
| `betsurf/t1only` | pass | 23/23 | --t1only -o -m -s: bet -A's call |
| `betsurf/t1t2` | pass | 21/21 | -o -m -s with the T2: bet -A2's call |
| `betsurf/verbose` | pass | 13/13 | -v -m |
| `betsurf/precision` | pass | 13/13 | -p 1: retessellated |
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
| `defects/short-matrix` | pass | 13/13 | a matrix of 6 numbers (FSL-53) |
| `defects/mesh-unread` | expected-divergence | 6/6 | a coordinate that does not read (FSL-52: FSL crashes; reproduce fills what FSL leaves undefined) |
| `defects/mesh-missing-vertex` | expected-divergence | 6/6 | a face naming a vertex the mesh does not have (FSL-52: FSL crashes; reproduce fills what FSL leaves undefined) |
| `defects/mesh-truncated` | expected-divergence | 6/6 | a mesh file that ends after two vertices (FSL-52: FSL crashes; reproduce fills what FSL leaves undefined) |
| `defects/t2-other-grid` | pass | 13/13 | a T2 on another grid (FSL-48) |
| `betsurf/off-mesh` | pass | 13/13 | an OFF mesh |
| `betsurf/fs-mesh` | pass | 13/13 | a FreeSurfer ASCII mesh |
| `real/child-3y` | pass | 10/10 | a child of 3.5 years (ds000228): bet's defaults and -m |
| `real/child-7y` | pass | 10/10 | a child of 7.5 years with motion (ds000256): bet's defaults and -m |
| `real/adult-ds000005` | pass | 10/10 | an adult, the fMRIPrep oracle run's T1w (ds000005): bet's defaults and -m |
| `real/older-75y` | pass | 10/10 | an adult of 75 years at 1.5 T (ds002422): bet's defaults and -m |
| `real/older-82y` | pass | 10/10 | an adult of 82 years (ds003592): bet's defaults and -m |
| `real/7t` | pass | 10/10 | 7 T, MP2RAGE (ds005454): bet's defaults and -m |
| `real/tumour` | pass | 10/10 | a tumour (ds005003): bet's defaults and -m |
| `real/stroke` | pass | 10/10 | a stroke (ds004884): bet's defaults and -m |
| `real/philips` | pass | 10/10 | a Philips scanner (ds002603): bet's defaults and -m |
| `real/bold-F` | pass | 8/8 | a BOLD series (ds000005): bet -F |
| `real/adult-ds000005-S` | pass | 10/10 | an adult (ds000005): bet -S -m (the eyes removed) |
| `real/tumour-S` | pass | 10/10 | a tumour (ds005003): bet -S |
| `real/child-3y-B` | fail | 6/9 | a 3.5-year-old (ds000228): bet -B -m (the bias field, the neck) |
| `real/older-82y-B` | pass | 10/10 | an adult of 82 years with the neck (ds003592): bet -B -s |
| `real/older-75y-A` | pass | 24/24 | an adult of 75 years at 1.5 T (ds002422): bet -A |
| `real/child-7y-A2` | pass | 26/26 | a child of 7.5 years with motion (ds000256): bet -A2 with its T2w |
| `real/bold-first` | pass | 8/8 | a BOLD series (ds000005): bet2 on its first volume (FSL's warning) |
| `fslroi/t` | pass | 6/6 | one volume (tmin tsize) of a 3D image |
| `fslroi/box` | pass | 6/6 | a box with a negative start (zeros outside) and a size of -1 |
| `fslroi/box-4d` | pass | 6/6 | x, y, z and t of a BOLD series |
| `fslroi/t-4d` | pass | 6/6 | volumes 3 to the end (-1) of a BOLD series |
| `fslroi/type-head-i16` | pass | 6/6 | a box of head-i16 (its type, orientation, transforms and format kept) |
| `fslroi/type-head-u8` | pass | 6/6 | a box of head-u8 (its type, orientation, transforms and format kept) |
| `fslroi/type-head-i8` | pass | 6/6 | a box of head-i8 (its type, orientation, transforms and format kept) |
| `fslroi/type-head-u16` | pass | 6/6 | a box of head-u16 (its type, orientation, transforms and format kept) |
| `fslroi/type-head-i32` | pass | 6/6 | a box of head-i32 (its type, orientation, transforms and format kept) |
| `fslroi/type-head-f64` | pass | 6/6 | a box of head-f64 (its type, orientation, transforms and format kept) |
| `fslroi/type-head-i16-scaled` | pass | 6/6 | a box of head-i16-scaled (its type, orientation, transforms and format kept) |
| `fslroi/type-head-neuro` | pass | 6/6 | a box of head-neuro (its type, orientation, transforms and format kept) |
| `fslroi/type-head-oblique` | pass | 6/6 | a box of head-oblique (its type, orientation, transforms and format kept) |
| `fslroi/type-head-noxform` | pass | 6/6 | a box of head-noxform (its type, orientation, transforms and format kept) |
| `fslroi/type-head-pair` | pass | 6/6 | a box of head-pair (its type, orientation, transforms and format kept) |
| `fslroi/type-head-nifti2` | pass | 6/6 | a box of head-nifti2 (its type, orientation, transforms and format kept) |
| `fslroi/type-head-analyze` | pass | 6/6 | a box of head-analyze (its type, orientation, transforms and format kept) |
| `fslroi/output-pair` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `fslroi/usage` | both-error | 4/4 | three arguments: the usage (exit 1) |
| `fslroi/no-arguments` | both-error | 4/4 | no arguments: the usage |
| `fslroi/invalid-roi` | pass | 4/4 | a size of -3: Invalid ROI dimensions, exit 0 (FSL-215) |
| `fslroi/time-out-of-bounds` | pass | 4/4 | a time index past the series: Out of Bounds, exit 0 (FSL-215) |
| `fslroi/missing-input` | pass | 4/4 | an input that does not exist: exit 0 (FSL-215) |
| `fslroi/words` | pass | 4/4 | numbers that are words (atoi: 0) |
| `fslroi/int8-negative` | pass | 6/6 | an int8 image with negative values: written as uint8 with the same bytes (FSL-217) |
| `fslroi/int8-positive` | pass | 6/6 | an int8 image without negative values: uint8 (FSL-217, the type only) |
| `ssroi/phantom-default` | pass | 6/6 | the T2-like head to the T1-like one (-ssref): the FOV mask and cut |
| `ssroi/phantom-debug` | pass | 20/20 | -d: the temporary files kept |
| `ssroi/phantom-masknone` | pass | 6/6 | -maskNONE: the cut only |
| `ssroi/phantom-roinone` | pass | 16/16 | -roiNONE -d: the mask only (imcp of the masked image) |
| `ssroi/phantom-none` | pass | 6/6 | -maskNONE -roiNONE: imcp of the input, as stored (.nii.gz) |
| `ssroi/phantom-none-convert` | pass | 6/6 | -maskNONE -roiNONE with FSLOUTPUTTYPE=NIFTI: fslpy converts the .nii.gz input with nibabel |
| `ssroi/phantom-masks` | pass | 20/20 | -maskMASK and -roiMASK (the head's mask as a standard-space mask) -d |
| `ssroi/phantom-altinput` | pass | 6/6 | -altinput: another image masked and cut |
| `ssroi/phantom-flirt-options` | pass | 6/6 | FLIRT's options after the script's (-dof 6 -cost mutualinfo) |
| `ssroi/phantom-pair` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI_PAIR (every file a pair; imcp copies the pair) |
| `ssroi/phantom-last-wins` | pass | 6/6 | -maskNONE -maskFOV -roiNONE -roiFOV: the last of each kind |
| `ssroi/usage` | both-error | 4/4 | one argument: the usage (exit 1; FSLDIR in its text) |
| `ssroi/invalid-input` | both-error | 4/4 | an input that does not exist |
| `ssroi/empty-input` | both-error | 4/4 | an empty input: remove_ext's usage becomes its name (FSL-211) |
| `ssroi/invalid-ssref` | both-error | 4/4 | -ssref of an image that does not exist |
| `ssroi/ssref-without-value` | both-error | 4/4 | -ssref as the last argument: the usage |
| `ssroi/invalid-altinput` | both-error | 4/4 | -altinput of an image that does not exist |
| `ssroi/invalid-roimask` | both-error | 4/4 | -roiMASK of an image that does not exist |
| `ssroi/invalid-maskmask` | both-error | 4/4 | -maskMASK of an image that does not exist |
| `defects/flirt-refuses` | pass | 4/4 | an option FLIRT refuses: the script goes on and exits 0 without an output (FSL-214) |
| `defects/output-blank` | both-error | 4/4 | an output name with a blank: split into words (FSL-211) |
| `defects/input-blank` | both-error | 4/4 | an input name with a blank: not valid (FSL-211) |
| `defects/option-blank` | pass | 4/4 | an option with a blank ends the options with the shell's message (FSL-211) |
| `defects/cleanup-glob` | pass | 6/6 | a file of the user named <out>_tmp...: the cleanup removes it (FSL-212) |
| `defects/maskthr-env` | pass | 16/16 | maskthr in the environment applied to the mask (FSL-213) |
| `ssroi/adult-default` | pass | 6/6 | an adult's T1w (ds000005) to MNI152 2 mm: the defaults |
| `ssroi/child-b` | pass | 16/16 | a 3.5-year-old's T1w: -b -d (bet -S and -B's call) |
| `ssroi/older-maskfov` | pass | 16/16 | an 82-year-old's T1w: -maskFOV -roiNONE -d (bet -B's skull call) |
| `ssroi/tumour-roimask` | pass | 6/6 | a tumour's T1w: -maskNONE -roiMASK MNI152's brain mask |
| `ssroi/philips-b-masks` | pass | 6/6 | a Philips T1w: -b then -maskMASK (the last mask, -b's threshold kept) |
| `ssroi/t2-default` | pass | 6/6 | a T2w (ds000256) to the T1 template: the defaults |
| `real/adult-ds000005-surf` | pass | 21/21 | an adult (ds000005): betsurf --t1only -o -m -s on FLIRT's matrix |
| `real/older-82y-surf` | pass | 21/21 | an adult of 82 years: betsurf --t1only -o -m -s on FLIRT's matrix |
| `real/child-7y-surf-t2` | pass | 21/21 | a child of 7.5 years (ds000256): betsurf -o -m -s with the T2 FLIRT registered |
| `real/tumour-surf-t2` | pass | 21/21 | a tumour (ds005003): betsurf -o -m -s with the T2 FLIRT registered |
| `real/philips-surf-t2` | fail | 19/21 | a Philips scanner (ds002603): betsurf -o -m -s with the T2 FLIRT registered |
