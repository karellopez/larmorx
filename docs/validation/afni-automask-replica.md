# Parity: 3dAutomask (replica)

**All cases agree.** 125 cases on the `standard` tier: 105 pass, 19 rejected by both, 1 expected divergences, 0 failures.

- **Validated:** the bit-exact replica (larmorx 0.0.1 (crate larmorx-afni-replica, built in): the translation of AFNI 26.3.05's source, Apache-2.0 with CC BY 4.0's attribution, part of larmorx since 2026-10-29), in-process: `larmorx afni 3dAutomask` with `LARMORX_IMPLEMENTATION=replica` and `lx.afni.automask(..., implementation="replica")`
- **Reference:** 3dAutomask from AFNI (AFNI_26.3.05, gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0), the binary built by `scripts/build_afni_oracle.sh`
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-10-30 on Linux x86_64, with `python -m larmorx_validation parity afni-automask --tier standard --implementation replica`
- **Original programs:** 322 runs: 318 from the oracle cache, 4 run; cache `<workspace>/oracles/cache`, mode `use`

**Bit-identical to AFNI: 101 of 101 compared cases that write outputs**: every output the same bytes (data and NIfTI header), with the same exit status and messages. The other 4 compared case(s) write no output in either program (an existing output, a name with a blank, a directory that does not exist) and agree on the exit status and messages; the default mode's corrections there are in the D13 lines below.
- Messages (standard output and error, byte for byte; AFNI's version banner and compile date, and larmorx's own lines, set aside): 118 of 118 identical, in every case that ran both programs.
- 6 case(s) where AFNI crashes (a missing option value, AFNI-40): larmorx prints AFNI's messages up to the crash, then refuses with one line and exit status 1.
- **fMRIPrep's own outputs** (the oracle runs' node outputs, as fMRIPrep 25.2.6 ran them in its image): 22 of 22 bit-identical to the replica's.
- AFNI's own variability, AFNI at 1 thread(s): 25 of 25 runs give the oracle's output bit for bit.
- AFNI's own variability, AFNI at 6 thread(s): 22 of 22 runs give the oracle's output bit for bit.
- AFNI's own variability, fMRIPrep 25.2.6's AFNI: 25 of 25 runs give the oracle's output bit for bit.
- Python API (`lx.afni.automask(..., implementation="replica")`): 35 of 35 cases give the command line's outputs bit for bit.
- **AFNI's own step, AFNI 25.2.09 → 26.3.05** (both builds on the same arguments; the previous one's runs from the oracle-output cache where an earlier record made them): 124 of 125 cases identical (exit status, messages, every output); 1 changed: 1 by AFNI-03 (AFNI 26.3.05 reads header slice code 2 (sequential decreasing) as NIfTI defines it, from slice_end down to slice_start; 25.2.09 gave every slice time 0): `difftest/slice-code-2-masked-afni26`.
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
  - AFNI-41: 5 case(s) named it, 0 with changed values (largest difference 0): `options/nbhrs-40`, `options/nbhrs-5`, `options/nbhrs-27`, `options/nbhrs-0`, `options/nbhrs-40-then-NN1`
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
| difftest | 1 | 1 | 0 | 0 | 0 |
| dimensions | 9 | 9 | 0 | 0 | 0 |
| divergences | 1 | 0 | 0 | 1 | 0 |
| environment | 5 | 5 | 0 | 0 | 0 |
| errors | 19 | 0 | 19 | 0 | 0 |
| fmriprep | 22 | 22 | 0 | 0 | 0 |
| global | 8 | 8 | 0 | 0 | 0 |
| options | 37 | 37 | 0 | 0 | 0 |
| real | 3 | 3 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `divergences/default-prefix` | AFNI's default prefix 'automask' is its own dataset format, which larmorx does not write: it refuses (NIfTI only). |

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
| larmorx | 0.0.1 (b6c7066c2dc2) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs run in directories of their own on the same input files, in the same environment (no AFNI variables of this process, `HOME` an empty directory, `AFNI_DONT_LOGFILE=YES`, `AFNI_NIFTI_NOEXT=YES`, plus each case's variables); outputs are compared as stored (data bytes, header bytes decompressed). The replica is compared in `reproduce` mode; the fix pass runs its default. The real cases also run AFNI at other thread counts and fMRIPrep 25.2.6's own AFNI (in its image) and report how far they are from the oracle (AFNI's own variability); the fMRIPrep cases compare the replica with fMRIPrep's own output of the node as well. AFNI version: 26.3.05 (`--afni-version`, `LARMORX_AFNI_VERSION`; builds: 26.3.05, 25.2.09). Each case also ran AFNI 25.2.09 on the same arguments (from the oracle-output cache where an earlier record ran it): the check 'AFNI's own step: 25.2.09 → 26.3.05' says whether AFNI's own exit status, messages and outputs changed, and why.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `options/default` | pass | 8/8 | the defaults (gradual clip level 0.5, 1 peel, NN2) |
| `options/fmriprep-call` | pass | 10/10 | fMRIPrep's options: -apply_prefix, -dilate 1 |
| `options/clfrac-0.1` | pass | 8/8 | -clfrac 0.1 |
| `options/clfrac-0.3` | pass | 8/8 | -clfrac 0.3 |
| `options/clfrac-0.9` | pass | 8/8 | -clfrac 0.9 |
| `options/mfrac` | pass | 7/7 | -mfrac, the same as -clfrac |
| `options/nograd` | pass | 8/8 | -nograd: one clip level |
| `options/nogr-prefix` | pass | 7/7 | -nogr: AFNI compares 5 characters |
| `options/peels-2` | pass | 8/8 | -peels 2 |
| `options/peels-3` | pass | 8/8 | -peels 3 |
| `options/nbhrs-6` | pass | 7/7 | -nbhrs 6 (AFNI uses 17 outside 6..26, AFNI-41) |
| `options/nbhrs-15` | pass | 7/7 | -nbhrs 15 (AFNI uses 17 outside 6..26, AFNI-41) |
| `options/nbhrs-18` | pass | 7/7 | -nbhrs 18 (AFNI uses 17 outside 6..26, AFNI-41) |
| `options/nbhrs-26` | pass | 7/7 | -nbhrs 26 (AFNI uses 17 outside 6..26, AFNI-41) |
| `options/nbhrs-40` | pass | 7/7 | -nbhrs 40 (AFNI uses 17 outside 6..26, AFNI-41) |
| `options/nbhrs-5` | pass | 7/7 | -nbhrs 5: outside 6..26, refused by default (AFNI-41) |
| `options/nbhrs-27` | pass | 7/7 | -nbhrs 27: outside 6..26, refused by default (AFNI-41) |
| `options/nbhrs-0` | pass | 7/7 | -nbhrs 0: outside 6..26, refused by default (AFNI-41) |
| `options/nbhrs-40-then-NN1` | pass | 7/7 | -nbhrs 40 then -NN1: AFNI peels as -NN1 says; refused by default (AFNI-41) |
| `options/NN1-dilate-erode-depth` | pass | 9/9 | -NN1 with -dilate 2, -erode 1 and -depth (and the peeling count it sets, AFNI-42) |
| `options/NN2-dilate-erode-depth` | pass | 9/9 | -NN2 with -dilate 2, -erode 1 and -depth (and the peeling count it sets, AFNI-42) |
| `options/NN3-dilate-erode-depth` | pass | 9/9 | -NN3 with -dilate 2, -erode 1 and -depth (and the peeling count it sets, AFNI-42) |
| `options/nbhrs-then-NN2` | pass | 7/7 | -nbhrs 12 then -NN2: -NN2 sets 18 (AFNI-42) |
| `options/eclip` | pass | 8/8 | -eclip |
| `options/eclip-nograd-clfrac` | pass | 7/7 | -eclip -nograd -clfrac 0.2 |
| `options/dilate-3` | pass | 7/7 | -dilate 3 |
| `options/erode-2` | pass | 8/8 | -erode 2 |
| `options/depth` | pass | 10/10 | -depth alone |
| `options/apply-only` | pass | 7/7 | -apply_prefix without -prefix: the only output |
| `options/quiet` | pass | 7/7 | -q |
| `options/SI-head-f32` | pass | 8/8 | -SI 60 along z, superior last |
| `options/SI-head-superior-first` | pass | 7/7 | -SI 60 along z, superior first |
| `options/SI-head-coronal` | pass | 7/7 | -SI 60 along y |
| `options/SI-head-las` | pass | 7/7 | -SI 60 along z, LAS storage |
| `options/SI-everything` | pass | 7/7 | -SI 1: clipping beyond the mask |
| `options/t2-contrast` | pass | 7/7 | the T2-like phantom |
| `options/big` | pass | 9/9 | a volume of 1.2 million voxels |
| `datatypes/head-i16` | pass | 11/11 | head-i16: the mask, the masked input and the depth |
| `datatypes/head-u8` | pass | 11/11 | head-u8: the mask, the masked input and the depth |
| `datatypes/head-i16-slope2` | pass | 11/11 | head-i16-slope2: the mask, the masked input and the depth |
| `datatypes/head-i16-slope1` | pass | 11/11 | head-i16-slope1: the mask, the masked input and the depth |
| `datatypes/head-f64` | pass | 11/11 | head-f64: the mask, the masked input and the depth |
| `datatypes/head-i8` | pass | 11/11 | head-i8: the mask, the masked input and the depth |
| `datatypes/head-u16` | pass | 11/11 | head-u16: the mask, the masked input and the depth |
| `datatypes/head-i32` | pass | 11/11 | head-i32: the mask, the masked input and the depth |
| `datatypes/head-nan` | pass | 11/11 | head-nan: the mask, the masked input and the depth |
| `datatypes/head-oblique` | pass | 11/11 | head-oblique: the mask, the masked input and the depth |
| `datatypes/head-qform-only` | pass | 11/11 | head-qform-only: the mask, the masked input and the depth |
| `datatypes/head-sform-only` | pass | 11/11 | head-sform-only: the mask, the masked input and the depth |
| `datatypes/head-las` | pass | 11/11 | head-las: the mask, the masked input and the depth |
| `defects/head-f32-slope-inter` | pass | 9/9 | float32 with scl_slope and scl_inter (AFNI-02) |
| `defects/head-metre` | pass | 7/7 | a grid in metres (AFNI-07) |
| `defects/head-nearly-plumb` | pass | 7/7 | a grid 0.009 degrees from plumb (AFNI-16) |
| `defects/nifti-scale-no` | pass | 9/9 | AFNI_NIFTI_SCALE=NO on int16 with an intercept (AFNI-23) |
| `defects/peels-0` | pass | 8/8 | -peels 0 (AFNI-43: AFNI peels once) |
| `defects/peels-0-fmriprep` | pass | 9/9 | -peels 0 added to fMRIPrep's call on its BOLD reference of ds005040 (AFNI-43) |
| `defects/negative-slope-4d` | pass | 9/9 | a 4D series with scl_slope -2 (AFNI-01) |
| `dimensions/series-i16` | pass | 9/9 | a 4D int16 series: the mean of \|values\| |
| `dimensions/series-f32` | pass | 9/9 | a 4D float32 series |
| `dimensions/series-slope` | pass | 9/9 | a 4D series with scl_slope 2 |
| `dimensions/series-one` | pass | 9/9 | a 4D file of one volume |
| `dimensions/slice` | pass | 9/9 | a 2D slice (64 x 64 x 1): AFNI's 2D path |
| `dimensions/two-slices` | pass | 9/9 | two slices (64 x 64 x 2) |
| `dimensions/tiny` | pass | 9/9 | 2 x 2 x 2 voxels: nothing to cluster |
| `dimensions/zeros` | pass | 9/9 | an image of zeros: the clip level is 0 |
| `dimensions/constant` | pass | 9/9 | a constant image |
| `errors/slice-small` | both-error | 4/4 | a 2D slice smaller than 16 x 16: no mask |
| `errors/row` | both-error | 4/4 | one row (64 x 1 x 1): no mask |
| `errors/missing-value-peels` | both-error | 3/3 | -peels as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-value-nbhrs` | both-error | 3/3 | -nbhrs as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-value-clfrac` | both-error | 3/3 | -clfrac as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-value-SI` | both-error | 3/3 | -SI as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-value-dilate` | both-error | 3/3 | -dilate as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-value-erode` | both-error | 3/3 | -erode as the last argument: AFNI crashes (AFNI-40) |
| `errors/missing-name-prefix` | both-error | 4/4 | -prefix as the last argument: '(null) is illegal' |
| `errors/missing-name-depth` | both-error | 4/4 | -depth as the last argument: '(null) is illegal' |
| `errors/missing-name-apply_prefix` | both-error | 4/4 | -apply_prefix as the last argument: '(null) is illegal' |
| `errors/clfrac-high` | both-error | 4/4 | -clfrac 0.95 |
| `errors/clfrac-low` | both-error | 4/4 | -clfrac 0.05 |
| `errors/SI-negative` | both-error | 4/4 | -SI -3 |
| `errors/dilate-negative` | both-error | 4/4 | -dilate -1 |
| `errors/erode-negative` | both-error | 4/4 | -erode -2 |
| `errors/unknown-option` | both-error | 4/4 | an unknown option |
| `errors/no-dataset` | both-error | 4/4 | no dataset |
| `errors/missing-input` | both-error | 4/4 | a missing input |
| `divergences/default-prefix` | expected-divergence | 2/2 | no -prefix: AFNI writes its own format (automask+orig) |
| `global/exists` | pass | 5/5 | an existing output (AFNI-11) |
| `global/overwrite` | pass | 7/7 | -overwrite |
| `global/deconflict-yes` | pass | 7/7 | AFNI_DECONFLICT=YES: renamed |
| `global/D-deconflict-overwrite` | pass | 7/7 | -DAFNI_DECONFLICT=OVERWRITE |
| `global/D-write-type-2` | pass | 7/7 | -DAFNI_NIFTI_WRITE_TYPE=2: NIfTI-2 |
| `environment/quiet-startup` | pass | 7/7 | AFNI_QUIET_STARTUP=YES: no author or output lines |
| `environment/message-prefix` | pass | 7/7 | AFNI_MESSAGE_PREFIX |
| `environment/echo-commandline` | pass | 7/7 | AFNI_ECHO_COMMANDLINE=YES |
| `environment/colorize-no` | pass | 7/7 | AFNI_MESSAGE_COLORIZE=NO with a warning |
| `environment/nifti-priority-q` | pass | 7/7 | AFNI_NIFTI_PRIORITY=Q on an oblique input |
| `global/prefix-with-blank` | pass | 5/5 | a -prefix with a blank (AFNI-12) |
| `global/input-dir-with-blank` | pass | 5/5 | an input in a directory with a blank (AFNI-18) |
| `global/missing-directory` | pass | 5/5 | an output in a missing directory (AFNI-26) |
| `fmriprep/ds000005-task_mixedgamblestask_run_01` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds000005-task_mixedgamblestask_run_01) |
| `fmriprep/ds000005-task_mixedgamblestask_run_02` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds000005-task_mixedgamblestask_run_02) |
| `fmriprep/ds000005-task_mixedgamblestask_run_03` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds000005-task_mixedgamblestask_run_03) |
| `fmriprep/ds000228-task_pixar` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds000228-task_pixar) |
| `fmriprep/ds000256-task_restbaseline` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds000256-task_restbaseline) |
| `fmriprep/ds000256-task_restfixationfixed` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds000256-task_restfixationfixed) |
| `fmriprep/ds000256-task_restfixationnofeedback` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds000256-task_restfixationnofeedback) |
| `fmriprep/ds002422-task_arithm_run_01` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds002422-task_arithm_run_01) |
| `fmriprep/ds002422-task_arithm_run_02` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds002422-task_arithm_run_02) |
| `fmriprep/ds002422-task_rest` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds002422-task_rest) |
| `fmriprep/ds002603-task_wm_acq_Mb4Mm3Tr550` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds002603-task_wm_acq_Mb4Mm3Tr550) |
| `fmriprep/ds002603-task_wm_acq_SeqMm3Tr2000` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds002603-task_wm_acq_SeqMm3Tr2000) |
| `fmriprep/ds003592-ses_1_task_rest_echo_1` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds003592-ses_1_task_rest_echo_1) |
| `fmriprep/ds004884-ses_217_task_naming40_acq_epfid2_dir_AP_run_2` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds004884-ses_217_task_naming40_acq_epfid2_dir_… |
| `fmriprep/ds004884-ses_217_task_rest_acq_epfidp2p2_dir_AP_run_9` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds004884-ses_217_task_rest_acq_epfidp2p2_dir_A… |
| `fmriprep/ds004958-task_rest` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds004958-task_rest) |
| `fmriprep/ds005003-ses_1_task_motor` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds005003-ses_1_task_motor) |
| `fmriprep/ds005003-ses_1_task_rest` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds005003-ses_1_task_rest) |
| `fmriprep/ds005040-task_clip_run_1` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds005040-task_clip_run_1) |
| `fmriprep/ds005454-task_rest` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds005454-task_rest) |
| `fmriprep/ds005752-ses_01_task_rest_dir_forward` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds005752-ses_01_task_rest_dir_forward) |
| `fmriprep/ds005752-ses_01_task_rest_dir_reverse` | pass | 14/14 | fMRIPrep's call on the input of its BOLD reference's second skull strip (ds005752-ses_01_task_rest_dir_reverse) |
| `real/sub-01_task-mixedgamblestask_run-01` | pass | 9/9 | AFNI's default on a BOLD series (sub-01_task-mixedgamblestask_run-01) |
| `real/sub-1_task-rest_acq-v4` | pass | 9/9 | AFNI's default on a BOLD series (sub-1_task-rest_acq-v4) |
| `real/t1w-brain` | pass | 9/9 | the skull-stripped T1w of an fMRIPrep run (ds000005, sMRIPrep's desc-brain) |
| `difftest/slice-code-2-masked-afni26` | pass | 9/9 | Header slice code 2 with -apply_prefix, against the replica and AFNI 26.3.05: AFNI 26.3.05 reads the code as NIfTI defi… |
