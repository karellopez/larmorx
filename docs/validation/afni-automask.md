# Parity: 3dAutomask

**All cases agree.** 105 cases on the `standard` tier: 85 pass, 19 rejected by both, 1 expected divergences, 0 failures.

- **Validated:** `larmorx afni 3dAutomask` / `lx.afni.automask` (crate larmorx-afni, the Apache-2.0 clean-room original written from `specs/3dAutomask.md`), with `implementation="original"`
- **Reference:** 3dAutomask from AFNI (AFNI_25.2.09, gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0), the binary built by `scripts/build_afni_oracle.sh`
- **Test data:** larmorx-testdata `b0999a9e0c31`, tier `standard`
- **Generated:** 2026-10-23 on Linux x86_64, with `python -m larmorx_validation parity afni-automask --tier standard`

**Bit-identical to AFNI: 80 of 81 compared cases that write outputs**: every output the same bytes (data and NIfTI header), with the same exit status and messages. The other 4 compared case(s) write no output in either program (an existing output, a name with a blank, a directory that does not exist) and agree on the exit status and messages; the default mode's corrections there are in the D13 lines below.
- Messages (standard output and error, byte for byte; AFNI's version banner and compile date, and larmorx's own lines, set aside): 98 of 98 identical, in every case that ran both programs.
- 1 case(s) whose input carries AFNI's own header extension, whose attributes AFNI reads in place of the NIfTI geometry (AFNI-10; the clean-room original ignores it, as 3dTshift's does): the data are bit-identical, and AFNI run on the input without its extension writes the original's header byte for byte: `fmriprep/ds005454-task_rest`.
- 6 case(s) where AFNI crashes (a missing option value, AFNI-40): larmorx prints AFNI's messages up to the crash, then refuses with one line and exit status 1.
- **fMRIPrep's own outputs** (the oracle runs' node outputs, as fMRIPrep 25.2.6 ran them in its image): 7 of 7 bit-identical to the original's.
- AFNI's own variability, AFNI at 1 thread(s): 10 of 10 runs give the oracle's output bit for bit.
- AFNI's own variability, AFNI at 6 thread(s): 7 of 7 runs give the oracle's output bit for bit.
- AFNI's own variability, fMRIPrep 25.2.6's AFNI: 10 of 10 runs give the oracle's output bit for bit.
- Python API (`lx.afni.automask(..., implementation="original")`): 20 of 20 cases give the command line's outputs bit for bit.
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - AFNI-01: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (messages): `defects/negative-slope-4d`
  - AFNI-02: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (masked.nii header, messages): `defects/head-f32-slope-inter`
  - AFNI-07: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (mask.nii header): `defects/head-metre`
  - AFNI-11: 1 case(s) named it, 0 with changed values (largest difference 0): `global/exists`
  - AFNI-12: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the outputs written where AFNI refuses (a b.nii, exit N)): `global/prefix-with-blank`
  - AFNI-16: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (mask.nii header): `defects/head-nearly-plumb`
  - AFNI-18: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the outputs written where AFNI refuses (mask.nii, exit N)): `global/input-dir-with-blank`
  - AFNI-23: 1 case(s) named it, 1 with changed values (largest difference 2.02e+03), 1 with other changes (masked.nii header, messages): `defects/nifti-scale-no`
  - AFNI-26: 1 case(s) named it, 0 with changed values (largest difference 0): `global/missing-directory`
  - AFNI-41: 1 case(s) named it, 0 with changed values (largest difference 0): `options/nbhrs-40`
  - AFNI-43: 1 case(s) named it, 1 with changed values (largest difference 1e+03), 1 with other changes (messages): `defects/peels-0-fmriprep`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (both succeed, or both reject the arguments); where AFNI crashes, larmorx exits 1 |
| Messages | standard output and error byte for byte (AFNI's banner and compile date, larmorx's own lines set aside) |
| Every output (mask, masked input, depth) | the same values, bit for bit, and the same NIfTI header bytes |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| datatypes | 13 | 13 | 0 | 0 | 0 |
| defects | 7 | 7 | 0 | 0 | 0 |
| dimensions | 9 | 9 | 0 | 0 | 0 |
| divergences | 1 | 0 | 0 | 1 | 0 |
| environment | 5 | 5 | 0 | 0 | 0 |
| errors | 19 | 0 | 19 | 0 | 0 |
| fmriprep | 7 | 7 | 0 | 0 | 0 |
| global | 8 | 8 | 0 | 0 | 0 |
| options | 33 | 33 | 0 | 0 | 0 |
| real | 3 | 3 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `divergences/default-prefix` | AFNI's default prefix 'automask' is its own dataset format, which larmorx-gpl does not write: it refuses (NIfTI only). |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/slice-small` | a 2D slice smaller than 16 x 16: no mask | AFNI exit 1: ** FATAL ERROR: Mask creation fails for unknown reasons!; larmorx exit 1: ** FATAL ERROR: Mask creation fails for unknown reasons! (the same messa… |
| `errors/row` | one row (64 x 1 x 1): no mask | AFNI exit 1: ** FATAL ERROR: Mask creation fails for unknown reasons!; larmorx exit 1: ** FATAL ERROR: Mask creation fails for unknown reasons! (the same messa… |
| `errors/missing-value-peels` | -peels as the last argument: AFNI crashes (AFNI-40) | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: -peels needs a value: AFNI crashes (Fatal Signal 11) when a… |
| `errors/missing-value-nbhrs` | -nbhrs as the last argument: AFNI crashes (AFNI-40) | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: -nbhrs needs a value: AFNI crashes (Fatal Signal 11) when a… |
| `errors/missing-value-clfrac` | -clfrac as the last argument: AFNI crashes (AFNI-40) | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: -clfrac needs a value: AFNI crashes (Fatal Signal 11) when … |
| `errors/missing-value-SI` | -SI as the last argument: AFNI crashes (AFNI-40) | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: -SI needs a value: AFNI crashes (Fatal Signal 11) when an o… |
| `errors/missing-value-dilate` | -dilate as the last argument: AFNI crashes (AFNI-40) | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: -dilate needs a value: AFNI crashes (Fatal Signal 11) when … |
| `errors/missing-value-erode` | -erode as the last argument: AFNI crashes (AFNI-40) | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: -erode needs a value: AFNI crashes (Fatal Signal 11) when a… |
| `errors/missing-name-prefix` | -prefix as the last argument: '(null) is illegal' | AFNI exit 1: ** FATAL ERROR: -prefix (null) is illegal!; larmorx exit 1: ** FATAL ERROR: -prefix (null) is illegal! (the same messages) |
| `errors/missing-name-depth` | -depth as the last argument: '(null) is illegal' | AFNI exit 1: ** FATAL ERROR: -depth (null) is illegal!; larmorx exit 1: ** FATAL ERROR: -depth (null) is illegal! (the same messages) |
| `errors/missing-name-apply_prefix` | -apply_prefix as the last argument: '(null) is illegal' | AFNI exit 1: ** FATAL ERROR: -apply_prefix (null) is illegal!; larmorx exit 1: ** FATAL ERROR: -apply_prefix (null) is illegal! (the same messages) |
| `errors/clfrac-high` | -clfrac 0.95 | AFNI exit 1: ** FATAL ERROR: -clfrac value 0.950000 is illegal!; larmorx exit 1: ** FATAL ERROR: -clfrac value 0.950000 is illegal! (the same messages) |
| `errors/clfrac-low` | -clfrac 0.05 | AFNI exit 1: ** FATAL ERROR: -clfrac value 0.050000 is illegal!; larmorx exit 1: ** FATAL ERROR: -clfrac value 0.050000 is illegal! (the same messages) |
| `errors/SI-negative` | -SI -3 | AFNI exit 1: ** FATAL ERROR: -SI value -3.000000 is illegal!; larmorx exit 1: ** FATAL ERROR: -SI value -3.000000 is illegal! (the same messages) |
| `errors/dilate-negative` | -dilate -1 | AFNI exit 1: ** FATAL ERROR: -dilate -1 is illegal!; larmorx exit 1: ** FATAL ERROR: -dilate -1 is illegal! (the same messages) |
| `errors/erode-negative` | -erode -2 | AFNI exit 1: ** FATAL ERROR: -erode -2 is illegal!; larmorx exit 1: ** FATAL ERROR: -erode -2 is illegal! (the same messages) |
| `errors/unknown-option` | an unknown option | AFNI exit 1: ** FATAL ERROR: ILLEGAL option: -bogus; larmorx exit 1: ** FATAL ERROR: ILLEGAL option: -bogus (the same messages) |
| `errors/no-dataset` | no dataset | AFNI exit 1: ** FATAL ERROR: Can't open dataset '(null)'; larmorx exit 1: ** FATAL ERROR: Can't open dataset '(null)' (the same messages) |
| `errors/missing-input` | a missing input | AFNI exit 1: ** FATAL ERROR: Can't open dataset '/nonexistent/x.nii'; larmorx exit 1: ** FATAL ERROR: Can't open dataset '/nonexistent/x.nii' (the same message… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (753e72eb525b) |
| larmorx-testdata | b0999a9e0c31 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs run in directories of their own on the same input files, in the same environment (no AFNI variables of this process, `HOME` an empty directory, `AFNI_DONT_LOGFILE=YES`, `AFNI_NIFTI_NOEXT=YES`, plus each case's variables); outputs are compared as stored (data bytes, header bytes decompressed). The original is compared in `reproduce` mode; the fix pass runs its default. The real cases also run AFNI at other thread counts and fMRIPrep 25.2.6's own AFNI (in its image) and report how far they are from the oracle (AFNI's own variability); the fMRIPrep cases compare the original with fMRIPrep's own output of the node as well.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `options/default` | pass | 7/7 | the defaults (gradual clip level 0.5, 1 peel, NN2) |
| `options/fmriprep-call` | pass | 9/9 | fMRIPrep's options: -apply_prefix, -dilate 1 |
| `options/clfrac-0.1` | pass | 7/7 | -clfrac 0.1 |
| `options/clfrac-0.3` | pass | 7/7 | -clfrac 0.3 |
| `options/clfrac-0.9` | pass | 7/7 | -clfrac 0.9 |
| `options/mfrac` | pass | 6/6 | -mfrac, the same as -clfrac |
| `options/nograd` | pass | 7/7 | -nograd: one clip level |
| `options/nogr-prefix` | pass | 6/6 | -nogr: AFNI compares 5 characters |
| `options/peels-2` | pass | 7/7 | -peels 2 |
| `options/peels-3` | pass | 7/7 | -peels 3 |
| `options/nbhrs-6` | pass | 6/6 | -nbhrs 6 (AFNI uses 17 outside 6..26, AFNI-41) |
| `options/nbhrs-15` | pass | 6/6 | -nbhrs 15 (AFNI uses 17 outside 6..26, AFNI-41) |
| `options/nbhrs-18` | pass | 6/6 | -nbhrs 18 (AFNI uses 17 outside 6..26, AFNI-41) |
| `options/nbhrs-26` | pass | 6/6 | -nbhrs 26 (AFNI uses 17 outside 6..26, AFNI-41) |
| `options/nbhrs-40` | pass | 6/6 | -nbhrs 40 (AFNI uses 17 outside 6..26, AFNI-41) |
| `options/NN1-dilate-erode-depth` | pass | 8/8 | -NN1 with -dilate 2, -erode 1 and -depth (and the peeling count it sets, AFNI-42) |
| `options/NN2-dilate-erode-depth` | pass | 8/8 | -NN2 with -dilate 2, -erode 1 and -depth (and the peeling count it sets, AFNI-42) |
| `options/NN3-dilate-erode-depth` | pass | 8/8 | -NN3 with -dilate 2, -erode 1 and -depth (and the peeling count it sets, AFNI-42) |
| `options/nbhrs-then-NN2` | pass | 6/6 | -nbhrs 12 then -NN2: -NN2 sets 18 (AFNI-42) |
| `options/eclip` | pass | 7/7 | -eclip |
| `options/eclip-nograd-clfrac` | pass | 6/6 | -eclip -nograd -clfrac 0.2 |
| `options/dilate-3` | pass | 6/6 | -dilate 3 |
| `options/erode-2` | pass | 7/7 | -erode 2 |
| `options/depth` | pass | 9/9 | -depth alone |
| `options/apply-only` | pass | 6/6 | -apply_prefix without -prefix: the only output |
| `options/quiet` | pass | 6/6 | -q |
| `options/SI-head-f32` | pass | 7/7 | -SI 60 along z, superior last |
| `options/SI-head-superior-first` | pass | 6/6 | -SI 60 along z, superior first |
| `options/SI-head-coronal` | pass | 6/6 | -SI 60 along y |
| `options/SI-head-las` | pass | 6/6 | -SI 60 along z, LAS storage |
| `options/SI-everything` | pass | 6/6 | -SI 1: clipping beyond the mask |
| `options/t2-contrast` | pass | 6/6 | the T2-like phantom |
| `options/big` | pass | 8/8 | a volume of 1.2 million voxels |
| `datatypes/head-i16` | pass | 10/10 | head-i16: the mask, the masked input and the depth |
| `datatypes/head-u8` | pass | 10/10 | head-u8: the mask, the masked input and the depth |
| `datatypes/head-i16-slope2` | pass | 10/10 | head-i16-slope2: the mask, the masked input and the depth |
| `datatypes/head-i16-slope1` | pass | 10/10 | head-i16-slope1: the mask, the masked input and the depth |
| `datatypes/head-f64` | pass | 10/10 | head-f64: the mask, the masked input and the depth |
| `datatypes/head-i8` | pass | 10/10 | head-i8: the mask, the masked input and the depth |
| `datatypes/head-u16` | pass | 10/10 | head-u16: the mask, the masked input and the depth |
| `datatypes/head-i32` | pass | 10/10 | head-i32: the mask, the masked input and the depth |
| `datatypes/head-nan` | pass | 10/10 | head-nan: the mask, the masked input and the depth |
| `datatypes/head-oblique` | pass | 10/10 | head-oblique: the mask, the masked input and the depth |
| `datatypes/head-qform-only` | pass | 10/10 | head-qform-only: the mask, the masked input and the depth |
| `datatypes/head-sform-only` | pass | 10/10 | head-sform-only: the mask, the masked input and the depth |
| `datatypes/head-las` | pass | 10/10 | head-las: the mask, the masked input and the depth |
| `defects/head-f32-slope-inter` | pass | 8/8 | float32 with scl_slope and scl_inter (AFNI-02) |
| `defects/head-metre` | pass | 6/6 | a grid in metres (AFNI-07) |
| `defects/head-nearly-plumb` | pass | 6/6 | a grid 0.009 degrees from plumb (AFNI-16) |
| `defects/nifti-scale-no` | pass | 8/8 | AFNI_NIFTI_SCALE=NO on int16 with an intercept (AFNI-23) |
| `defects/peels-0` | pass | 7/7 | -peels 0 (AFNI-43: AFNI peels once) |
| `defects/peels-0-fmriprep` | pass | 8/8 | -peels 0 added to fMRIPrep's call on its BOLD reference of ds005040 (AFNI-43) |
| `defects/negative-slope-4d` | pass | 8/8 | a 4D series with scl_slope -2 (AFNI-01) |
| `dimensions/series-i16` | pass | 8/8 | a 4D int16 series: the mean of \|values\| |
| `dimensions/series-f32` | pass | 8/8 | a 4D float32 series |
| `dimensions/series-slope` | pass | 8/8 | a 4D series with scl_slope 2 |
| `dimensions/series-one` | pass | 8/8 | a 4D file of one volume |
| `dimensions/slice` | pass | 8/8 | a 2D slice (64 x 64 x 1): AFNI's 2D path |
| `dimensions/two-slices` | pass | 8/8 | two slices (64 x 64 x 2) |
| `dimensions/tiny` | pass | 8/8 | 2 x 2 x 2 voxels: nothing to cluster |
| `dimensions/zeros` | pass | 8/8 | an image of zeros: the clip level is 0 |
| `dimensions/constant` | pass | 8/8 | a constant image |
| `errors/slice-small` | both-error | 3/3 | a 2D slice smaller than 16 x 16: no mask |
| `errors/row` | both-error | 3/3 | one row (64 x 1 x 1): no mask |
| `errors/missing-value-peels` | both-error | 2/2 | -peels as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-value-nbhrs` | both-error | 2/2 | -nbhrs as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-value-clfrac` | both-error | 2/2 | -clfrac as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-value-SI` | both-error | 2/2 | -SI as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-value-dilate` | both-error | 2/2 | -dilate as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-value-erode` | both-error | 2/2 | -erode as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-name-prefix` | both-error | 3/3 | -prefix as the last argument: '(null) is illegal' |
| `errors/missing-name-depth` | both-error | 3/3 | -depth as the last argument: '(null) is illegal' |
| `errors/missing-name-apply_prefix` | both-error | 3/3 | -apply_prefix as the last argument: '(null) is illegal' |
| `errors/clfrac-high` | both-error | 3/3 | -clfrac 0.95 |
| `errors/clfrac-low` | both-error | 3/3 | -clfrac 0.05 |
| `errors/SI-negative` | both-error | 3/3 | -SI -3 |
| `errors/dilate-negative` | both-error | 3/3 | -dilate -1 |
| `errors/erode-negative` | both-error | 3/3 | -erode -2 |
| `errors/unknown-option` | both-error | 3/3 | an unknown option |
| `errors/no-dataset` | both-error | 3/3 | no dataset |
| `errors/missing-input` | both-error | 3/3 | a missing input |
| `divergences/default-prefix` | expected-divergence | 1/1 | no -prefix: AFNI writes its own format (automask+orig) |
| `global/exists` | pass | 4/4 | an existing output (AFNI-11) |
| `global/overwrite` | pass | 6/6 | -overwrite |
| `global/deconflict-yes` | pass | 6/6 | AFNI_DECONFLICT=YES: renamed |
| `global/D-deconflict-overwrite` | pass | 6/6 | -DAFNI_DECONFLICT=OVERWRITE |
| `global/D-write-type-2` | pass | 6/6 | -DAFNI_NIFTI_WRITE_TYPE=2: NIfTI-2 |
| `environment/quiet-startup` | pass | 6/6 | AFNI_QUIET_STARTUP=YES: no author or output lines |
| `environment/message-prefix` | pass | 6/6 | AFNI_MESSAGE_PREFIX |
| `environment/echo-commandline` | pass | 6/6 | AFNI_ECHO_COMMANDLINE=YES |
| `environment/colorize-no` | pass | 6/6 | AFNI_MESSAGE_COLORIZE=NO with a warning |
| `environment/nifti-priority-q` | pass | 6/6 | AFNI_NIFTI_PRIORITY=Q on an oblique input |
| `global/prefix-with-blank` | pass | 4/4 | a -prefix with a blank (AFNI-12) |
| `global/input-dir-with-blank` | pass | 4/4 | an input in a directory with a blank (AFNI-18) |
| `global/missing-directory` | pass | 4/4 | an output in a missing directory (AFNI-26) |
| `fmriprep/ds000005-task_mixedgamblestask_run_01` | pass | 13/13 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds000005-task_mixedgamblestask_run_01) |
| `fmriprep/ds000005-task_mixedgamblestask_run_02` | pass | 13/13 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds000005-task_mixedgamblestask_run_02) |
| `fmriprep/ds000005-task_mixedgamblestask_run_03` | pass | 13/13 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds000005-task_mixedgamblestask_run_03) |
| `fmriprep/ds005040-task_clip_run_1` | pass | 13/13 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds005040-task_clip_run_1) |
| `fmriprep/ds005454-task_rest` | pass | 13/13 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds005454-task_rest) |
| `fmriprep/ds005752-ses_01_task_rest_dir_forward` | pass | 13/13 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds005752-ses_01_task_rest_dir_forward) |
| `fmriprep/ds005752-ses_01_task_rest_dir_reverse` | pass | 13/13 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds005752-ses_01_task_rest_dir_reverse) |
| `real/sub-01_task-mixedgamblestask_run-01` | pass | 8/8 | AFNI's default on a BOLD series (sub-01_task-mixedgamblestask_run-01) |
| `real/sub-1_task-rest_acq-v4` | pass | 8/8 | AFNI's default on a BOLD series (sub-1_task-rest_acq-v4) |
| `real/t1w-brain` | pass | 8/8 | the skull-stripped T1w of an fMRIPrep run (ds000005, sMRIPrep's desc-brain) |
