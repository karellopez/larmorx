# Parity: 3dUnifize

**All cases agree.** 97 cases on the `standard` tier: 76 pass, 19 rejected by both, 2 expected divergences, 0 failures.

- **Validated:** `larmorx afni 3dUnifize` / `lx.afni.unifize` (crate larmorx-afni, the Apache-2.0 clean-room original written from `specs/3dUnifize.md`), with `implementation="original"`
- **Reference:** 3dUnifize from AFNI (AFNI_25.2.09, gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0, with OpenMP), the binary built by `scripts/build_afni_oracle.sh`
- **Test data:** larmorx-testdata `b0999a9e0c31`, tier `standard`
- **Generated:** 2026-10-24 on Linux x86_64, with `python -m larmorx_validation parity afni-unifize --tier standard`

**Bit-identical to AFNI: 32 of 72 compared cases that write outputs**: every output the same bytes (data and NIfTI header), with the same exit status and messages. The other 4 compared case(s) write no output in either program (an existing output, a name with a blank, a directory that does not exist) and agree on the exit status and messages; the default mode's corrections there are in the D13 lines below.
- Messages (standard output and error, byte for byte; AFNI's version banner and compile date, and larmorx's own lines, set aside): 93 of 93 identical, in every case that ran both programs.
- The other 40 compared case(s) write float32 values within the threshold (AFNI's squash calls glibc's `tanhf`, which is not correctly rounded on some arguments; the original's is): with CORE-MATH's correctly rounded functions preloaded into AFNI, 40 of them are bit-identical to the original.
- 3 case(s) where AFNI crashes (a missing option value, AFNI-40; an empty automask dereferenced, AFNI-46): larmorx prints AFNI's messages up to the crash, then refuses with one line and exit status 1.
- **fMRIPrep's own outputs** (the oracle runs' node outputs, as fMRIPrep 25.2.6 ran them in its image): 1 of 7 bit-identical to the original's; the other 6 within the threshold, and in 6 of them fMRIPrep's output is AFNI's oracle output bit for bit, which AFNI with correctly rounded maths turns into the original's (glibc's `tanhf`).
- AFNI's own variability, AFNI at 1 thread(s): 7 of 7 runs give the oracle's output bit for bit.
- AFNI's own variability, AFNI at 6 thread(s): 9 of 9 runs give the oracle's output bit for bit.
- AFNI's own variability, fMRIPrep 25.2.6's AFNI: 9 of 9 runs give the oracle's output bit for bit.
- Python API (`lx.afni.unifize(..., implementation="original")`): 20 of 20 cases give the command line's outputs bit for bit.
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - AFNI-01: 1 case(s) named it, 1 with changed values (largest difference 1.67e+03), 1 with other changes (messages): `defects/EPI-negative-slope`
  - AFNI-02: 1 case(s) named it, 1 with changed values (largest difference 0.00427): `defects/head-f32-slope-inter`
  - AFNI-07: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (uni.nii header): `defects/head-metre`
  - AFNI-11: 2 case(s) named it, 0 with changed values (largest difference 0): `global/exists`, `global/ssave-exists`
  - AFNI-12: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the outputs written where AFNI refuses (a b.nii, exit N)): `global/prefix-with-blank`
  - AFNI-16: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (uni.nii header): `defects/head-nearly-plumb`
  - AFNI-18: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the outputs written where AFNI refuses (uni.nii, exit N)): `global/input-dir-with-blank`
  - AFNI-23: 1 case(s) named it, 1 with changed values (largest difference 1.03e+03): `defects/nifti-scale-no`
  - AFNI-26: 1 case(s) named it, 0 with changed values (largest difference 0): `global/missing-directory`
  - AFNI-45: 2 case(s) named it, 2 with changed values (largest difference 1.46e+03), 2 with other changes (messages): `defects/T2up-100`, `defects/T2up-99.99`
  - AFNI-47: 1 case(s) named it, 1 with changed values (largest difference 1.5e+03), 1 with other changes (messages): `defects/slab`
  - AFNI-48: 1 case(s) named it, 0 with changed values (largest difference 0): `options/T2-T2-T2`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (both succeed, or both reject the arguments); where AFNI crashes, larmorx exits 1 |
| Messages | standard output and error byte for byte (AFNI's banner and compile date, larmorx's own lines and the elapsed time set aside) |
| Every output (the unifized image, -ssave, -amsave) | the same NIfTI header bytes; the values bit for bit, or float32 values within max \|diff\| ≤ 1e-5 × max \|AFNI output\| (PLAN.md §11.3) with bit-identity reported (AFNI's squash calls glibc's tanhf, which is not correctly rounded on some arguments) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| datatypes | 10 | 10 | 0 | 0 | 0 |
| defects | 8 | 7 | 0 | 1 | 0 |
| dimensions | 6 | 6 | 0 | 0 | 0 |
| divergences | 1 | 0 | 0 | 1 | 0 |
| environment | 3 | 3 | 0 | 0 | 0 |
| errors | 19 | 0 | 19 | 0 | 0 |
| fmriprep | 7 | 7 | 0 | 0 | 0 |
| global | 8 | 8 | 0 | 0 | 0 |
| options | 33 | 33 | 0 | 0 | 0 |
| real | 2 | 2 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `defects/slab` | AFNI reads memory it never wrote (AFNI-47): this run crashed (Fatal Signal 11) after the same messages; larmorx fills the read with 0 in reproduce mode and corrects it by default |
| `divergences/default-prefix` | AFNI's default prefix 'Unifized' is its own dataset format, which larmorx-gpl does not write: it refuses (NIfTI only). |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/slice-small` | a 2D slice smaller than 16 x 16: no automask | AFNI exit 1: ** FATAL ERROR: Can't compute Unifize-d dataset for some reason :-(; larmorx exit 1: ** FATAL ERROR: Can't compute Unifize-d dataset for some reas… |
| `errors/slice-small-T2` | -T2 on a 2D slice smaller than 16 x 16: AFNI crashes (AFNI-46) | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: the automask of a 2D image smaller than 16 x 16 is empty (N… |
| `errors/Urad-low` | -Urad 3 <input> | AFNI exit 1: ** FATAL ERROR: Illegal value 3.000000 after option -Urad; larmorx exit 1: ** FATAL ERROR: Illegal value 3.000000 after option -Urad (the same mes… |
| `errors/Urad-high` | -Urad 120 <input> | AFNI exit 1: ** FATAL ERROR: Illegal value 120.000000 after option -Urad; larmorx exit 1: ** FATAL ERROR: Illegal value 120.000000 after option -Urad (the same… |
| `errors/rbt-bottom` | -rbt 18 20 80 <input> | AFNI exit 1: ** FATAL ERROR: Illegal values (R pb pt) after '-rbt'; larmorx exit 1: ** FATAL ERROR: Illegal values (R pb pt) after '-rbt' (the same messages) |
| `errors/rbt-order` | -rbt 18 70 60 <input> | AFNI exit 1: ** FATAL ERROR: Illegal values (R pb pt) after '-rbt'; larmorx exit 1: ** FATAL ERROR: Illegal values (R pb pt) after '-rbt' (the same messages) |
| `errors/rbt-top` | -rbt 18 70 95 <input> | AFNI exit 1: ** FATAL ERROR: Illegal values (R pb pt) after '-rbt'; larmorx exit 1: ** FATAL ERROR: Illegal values (R pb pt) after '-rbt' (the same messages) |
| `errors/rbt-missing` | -rbt 18 60 | AFNI exit 1: ** FATAL ERROR: Need 3 arguments (R pb pt) after '-rbt'; larmorx exit 1: ** FATAL ERROR: Need 3 arguments (R pb pt) after '-rbt' (the same message… |
| `errors/clfrac-low` | -clfrac 0.05 <input> | AFNI exit 1: ** FATAL ERROR: -clfrac value 0.050000 is illegal!; larmorx exit 1: ** FATAL ERROR: -clfrac value 0.050000 is illegal! (the same messages) |
| `errors/clfrac-missing` | -clfrac | AFNI exit 1: ** FATAL ERROR: Need argument after '-clfrac'; larmorx exit 1: ** FATAL ERROR: Need argument after '-clfrac' (the same messages) |
| `errors/T2up-low` | -T2up 89 <input> | AFNI exit 1: ** FATAL ERROR: -T2up value is out of range 90..100 :-(; larmorx exit 1: ** FATAL ERROR: -T2up value is out of range 90..100 :-( (the same message… |
| `errors/T2up-high` | -T2up 101 <input> | AFNI exit 1: ** FATAL ERROR: -T2up value is out of range 90..100 :-(; larmorx exit 1: ** FATAL ERROR: -T2up value is out of range 90..100 :-( (the same message… |
| `errors/T2up-missing` | -T2up | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: -T2up needs a value: AFNI crashes (Fatal Signal 11) when an… |
| `errors/prefix-missing` | -prefix | AFNI exit 1: ** FATAL ERROR: Need argument after '-prefix'; larmorx exit 1: ** FATAL ERROR: Need argument after '-prefix' (the same messages) |
| `errors/ssave-missing` | -ssave | AFNI exit 1: ** FATAL ERROR: Need argument after '-ssave'; larmorx exit 1: ** FATAL ERROR: Need argument after '-ssave' (the same messages) |
| `errors/input-twice` | -input <input> -input <input> | AFNI exit 1: ** FATAL ERROR: Can't use '-input' twice; larmorx exit 1: ** FATAL ERROR: Can't use '-input' twice (the same messages) |
| `errors/unknown-option` | -bogus <input> | AFNI exit 1: ** FATAL ERROR: Unknown option: -bogus; larmorx exit 1: ** FATAL ERROR: Unknown option: -bogus (the same messages) |
| `errors/no-dataset` | -quiet | AFNI exit 1: ** FATAL ERROR: No dataset name on command line?; larmorx exit 1: ** FATAL ERROR: No dataset name on command line? (the same messages) |
| `errors/missing-input` | -prefix uni.nii /nonexistent/x.nii | AFNI exit 1: ** FATAL ERROR: Can't open dataset '/nonexistent/x.nii'; larmorx exit 1: ** FATAL ERROR: Can't open dataset '/nonexistent/x.nii' (the same message… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (2a009d1ba7d6) |
| larmorx-testdata | b0999a9e0c31 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

Both programs run in directories of their own on the same input files, in the same environment (no AFNI variables of this process, `HOME` an empty directory, `AFNI_DONT_LOGFILE=YES`, `AFNI_NIFTI_NOEXT=YES`, plus each case's variables); outputs are compared as stored (data bytes, header bytes decompressed). AFNI runs with OpenMP at this machine's default thread count unless a case sets `OMP_NUM_THREADS`; the replica with its own threads (results do not depend on them). The original is compared in `reproduce` mode; the fix pass runs its default. The real cases also run AFNI at other OpenMP thread counts and fMRIPrep 25.2.6's own AFNI (in its image) and report how far they are from the oracle; the fMRIPrep cases compare the original with fMRIPrep's own output of the node as well.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `options/default` | pass | 9/9 | the defaults on the T1-like phantom |
| `options/fmriprep-call` | pass | 7/7 | fMRIPrep's options on the T2-like phantom: -clfrac 0.2 -rbt 18.3 65 90 -T2 |
| `options/Urad-5` | pass | 9/9 | -Urad 5 |
| `options/Urad-12` | pass | 6/6 | -Urad 12 |
| `options/Urad-30` | pass | 6/6 | -Urad 30 |
| `options/rbt` | pass | 9/9 | -rbt 10 30 60 |
| `options/param` | pass | 8/8 | -param, the same as -rbt |
| `options/clfrac-0.1` | pass | 8/8 | -clfrac 0.1 |
| `options/clfrac-0.5` | pass | 6/6 | -clfrac 0.5 |
| `options/clfrac-0.9` | pass | 8/8 | -clfrac 0.9 |
| `options/mfrac` | pass | 8/8 | -mfrac, the same as -clfrac |
| `options/T2` | pass | 7/7 | -T2 on the T2-like phantom |
| `options/T2-T2` | pass | 7/7 | -T2 -T2: no second inversion, bright edges clipped |
| `options/T2-T2-T2` | pass | 6/6 | -T2 three times: neither (AFNI-48) |
| `options/T2up-95` | pass | 7/7 | -T2 -T2up 95 |
| `options/GM` | pass | 9/9 | -GM |
| `options/GM-T2` | pass | 6/6 | -GM -T2 |
| `options/nosquash` | pass | 7/7 | -nosquash |
| `options/oink` | pass | 6/6 | -oink, the same as -nosquash |
| `options/ssave-amsave` | pass | 13/13 | -ssave and -amsave |
| `options/quiet` | pass | 8/8 | -quiet |
| `options/verb` | pass | 8/8 | -verb: the automask's messages too |
| `options/verb-verb` | pass | 8/8 | -verb -verb |
| `options/useall` | pass | 8/8 | -useall: disabled, a warning |
| `options/inset` | pass | 8/8 | -inset |
| `options/case-insensitive` | pass | 6/6 | -t2 -gm -NOSQUASH: AFNI's strcasecmp options |
| `options/duplo` | pass | 8/8 | 1.2 million voxels: the half-size path |
| `options/noduplo` | pass | 9/9 | 1.2 million voxels with -noduplo |
| `options/duplo-T2` | pass | 8/8 | the half-size path with -T2 and -ssave |
| `options/EPI` | pass | 7/7 | -EPI on a 4D series |
| `options/EPI-GM` | pass | 6/6 | -EPI -GM: -GM turned off |
| `options/EPI-3D` | pass | 6/6 | -EPI on one volume: a warning, -T2 kept |
| `options/4D-without-EPI` | pass | 8/8 | a 4D series without -EPI: sub-brick #0 only |
| `datatypes/head-i16` | pass | 10/10 | head-i16, with -ssave |
| `datatypes/head-u8` | pass | 8/8 | head-u8, with -ssave |
| `datatypes/head-i16-slope2` | pass | 8/8 | head-i16-slope2, with -ssave |
| `datatypes/head-i16-slope1` | pass | 10/10 | head-i16-slope1, with -ssave |
| `datatypes/head-f64` | pass | 10/10 | head-f64, with -ssave |
| `datatypes/head-i8` | pass | 8/8 | head-i8, with -ssave |
| `datatypes/head-nan` | pass | 8/8 | head-nan, with -ssave |
| `datatypes/head-oblique` | pass | 10/10 | head-oblique, with -ssave |
| `datatypes/head-qform-only` | pass | 10/10 | head-qform-only, with -ssave |
| `datatypes/head-las` | pass | 10/10 | head-las, with -ssave |
| `defects/head-f32-slope-inter` | pass | 6/6 | float32 with scl_slope and scl_inter (AFNI-02) |
| `defects/head-metre` | pass | 8/8 | a grid in metres (AFNI-07) |
| `defects/head-nearly-plumb` | pass | 8/8 | a grid 0.009 degrees from plumb (AFNI-16) |
| `defects/nifti-scale-no` | pass | 6/6 | AFNI_NIFTI_SCALE=NO on int16 with an intercept (AFNI-23) |
| `defects/T2up-100` | pass | 7/7 | -T2 -T2up 100: one past the sorted values (AFNI-45) |
| `defects/T2up-99.99` | pass | 6/6 | -T2 -T2up 99.99 on a small mask: one past the values too (AFNI-45) |
| `defects/EPI-negative-slope` | pass | 6/6 | -EPI on a series with scl_slope -2 (AFNI-01) |
| `defects/slab` | expected-divergence | 3/3 | 1 million voxels with an axis of 2: the half-size grid keeps 1 (AFNI-47) |
| `dimensions/slice` | pass | 6/6 | a 2D slice (64 x 64 x 1) |
| `dimensions/two-slices` | pass | 6/6 | two slices |
| `dimensions/tiny` | pass | 6/6 | 2 x 2 x 2 voxels |
| `dimensions/zeros` | pass | 6/6 | an image of zeros |
| `dimensions/constant` | pass | 6/6 | a constant image |
| `dimensions/series-one` | pass | 8/8 | a 4D file of one volume |
| `errors/slice-small` | both-error | 3/3 | a 2D slice smaller than 16 x 16: no automask |
| `errors/slice-small-T2` | both-error | 2/2 | -T2 on a 2D slice smaller than 16 x 16: AFNI crashes (AFNI-46) |
| `errors/Urad-low` | both-error | 3/3 | -Urad 3 <input> |
| `errors/Urad-high` | both-error | 3/3 | -Urad 120 <input> |
| `errors/rbt-bottom` | both-error | 3/3 | -rbt 18 20 80 <input> |
| `errors/rbt-order` | both-error | 3/3 | -rbt 18 70 60 <input> |
| `errors/rbt-top` | both-error | 3/3 | -rbt 18 70 95 <input> |
| `errors/rbt-missing` | both-error | 3/3 | -rbt 18 60 |
| `errors/clfrac-low` | both-error | 3/3 | -clfrac 0.05 <input> |
| `errors/clfrac-missing` | both-error | 3/3 | -clfrac |
| `errors/T2up-low` | both-error | 3/3 | -T2up 89 <input> |
| `errors/T2up-high` | both-error | 3/3 | -T2up 101 <input> |
| `errors/T2up-missing` | both-error | 2/2 | -T2up |
| `errors/prefix-missing` | both-error | 3/3 | -prefix |
| `errors/ssave-missing` | both-error | 3/3 | -ssave |
| `errors/input-twice` | both-error | 3/3 | -input <input> -input <input> |
| `errors/unknown-option` | both-error | 3/3 | -bogus <input> |
| `errors/no-dataset` | both-error | 3/3 | -quiet |
| `errors/missing-input` | both-error | 3/3 | -prefix uni.nii /nonexistent/x.nii |
| `divergences/default-prefix` | expected-divergence | 1/1 | no -prefix: AFNI writes its own format (Unifized+orig) |
| `global/exists` | pass | 4/4 | an existing output (AFNI-11) |
| `global/overwrite` | pass | 8/8 | -overwrite |
| `global/deconflict-yes` | pass | 8/8 | AFNI_DECONFLICT=YES: renamed |
| `global/ssave-exists` | pass | 8/8 | an existing -ssave output (AFNI-11) |
| `global/D-write-type-2` | pass | 8/8 | -DAFNI_NIFTI_WRITE_TYPE=2 |
| `environment/quiet-startup` | pass | 8/8 | AFNI_QUIET_STARTUP=YES |
| `environment/message-prefix` | pass | 8/8 | AFNI_MESSAGE_PREFIX |
| `environment/echo-commandline` | pass | 8/8 | AFNI_ECHO_COMMANDLINE=YES |
| `global/prefix-with-blank` | pass | 4/4 | a -prefix with a blank (AFNI-12) |
| `global/input-dir-with-blank` | pass | 4/4 | an input in a directory with a blank (AFNI-18) |
| `global/missing-directory` | pass | 4/4 | an output in a missing directory (AFNI-26) |
| `fmriprep/ds000005-task_mixedgamblestask_run_01` | pass | 11/11 | fMRIPrep's call on its BOLD reference's input (ds000005-task_mixedgamblestask_run_01) |
| `fmriprep/ds000005-task_mixedgamblestask_run_02` | pass | 14/14 | fMRIPrep's call on its BOLD reference's input (ds000005-task_mixedgamblestask_run_02) |
| `fmriprep/ds000005-task_mixedgamblestask_run_03` | pass | 14/14 | fMRIPrep's call on its BOLD reference's input (ds000005-task_mixedgamblestask_run_03) |
| `fmriprep/ds005040-task_clip_run_1` | pass | 14/14 | fMRIPrep's call on its BOLD reference's input (ds005040-task_clip_run_1) |
| `fmriprep/ds005454-task_rest` | pass | 14/14 | fMRIPrep's call on its BOLD reference's input (ds005454-task_rest) |
| `fmriprep/ds005752-ses_01_task_rest_dir_forward` | pass | 14/14 | fMRIPrep's call on its BOLD reference's input (ds005752-ses_01_task_rest_dir_forward) |
| `fmriprep/ds005752-ses_01_task_rest_dir_reverse` | pass | 14/14 | fMRIPrep's call on its BOLD reference's input (ds005752-ses_01_task_rest_dir_reverse) |
| `real/T1w-ds000005` | pass | 12/12 | AFNI's default on the T1w of ds000005 (the half-size path) |
| `real/EPI-ds000005` | pass | 8/8 | -EPI on the BOLD series of ds000005 |
