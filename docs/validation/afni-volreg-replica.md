# Parity: 3dvolreg (replica)

**All cases agree.** 190 cases on the `standard` tier: 158 pass, 30 rejected by both, 2 expected divergences, 0 failures.

- **Validated:** the bit-exact replica (larmorx 0.0.1 (crate larmorx-afni-replica, built in): the translation of AFNI 26.3.05's source, Apache-2.0 with CC BY 4.0's attribution), in-process: `larmorx afni 3dvolreg` with `LARMORX_IMPLEMENTATION=replica` and `lx.afni.volreg(..., implementation="replica")`
- **Reference:** 3dvolreg from AFNI (AFNI_26.3.05, gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0; no OpenMP loop), the binary built by `scripts/build_afni_oracle.sh`
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-07 on Linux x86_64, with `python -m larmorx_validation parity afni-volreg --tier standard --implementation replica`
- **Original programs:** 386 runs: 386 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

**Bit-identical to AFNI: 137 of 137 compared cases that write outputs**: every output the same bytes (data and NIfTI header), with the same exit status and messages. The other 21 compared case(s) write no output in either program (an existing output, a name with a blank, a directory that does not exist) and agree on the exit status and messages; the default mode's corrections there are in the D13 lines below.
- Messages (standard output and error, byte for byte; AFNI's version banner and compile date, and larmorx's own lines, set aside): 180 of 180 identical, in every case that ran both programs.
- 7 case(s) where AFNI crashes (an option given last without its value, AFNI-88; a base whose least squares cannot be set up, AFNI-89; a name of 256 bytes or more, AFNI-90; a Fourier rotation that clears a row through a NULL pointer, AFNI-97): larmorx prints AFNI's messages up to the crash, then refuses with one line and exit status 1.
- **fMRIPrep's own outputs** (the oracle runs' node outputs, as fMRIPrep 25.2.6 ran them in its image): 8 of 8 bit-identical to the replica's.
- AFNI's own variability, fMRIPrep 25.2.6's AFNI: 8 of 8 runs give the oracle's output bit for bit.
- Python API (`lx.afni.volreg(..., implementation="replica")`): 21 of 21 cases give the command line's outputs bit for bit.
- **AFNI's own step, AFNI 25.2.09 → 26.3.05** (both builds on the same arguments; the previous one's runs from the oracle-output cache where an earlier record made them): 189 of 189 cases identical (exit status, messages, every output).
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - AFNI-01: 1 case(s) named it, 1 with changed values (largest difference 7.15e+03): `tshift/tshift-negative-slope`
  - AFNI-11: 1 case(s) named it, 0 with changed values (largest difference 0): `global/exists`
  - AFNI-12: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (a b.nii written in one mode only, messages): `global/prefix-with-blank`
  - AFNI-18: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the outputs written where AFNI refuses (o.nii, exit N)): `global/input-dir-with-blank`
  - AFNI-26: 1 case(s) named it, 0 with changed values (largest difference 0): `global/missing-directory`
  - AFNI-86: 4 case(s) named it, 4 with changed values (largest difference 84): `twopass/nocoarserot`, `edge cases/large_z-nocoarserot`, `edge cases/large_x-nocoarserot`, `edge cases/four-slices-nocoarserot`
  - AFNI-87: 2 case(s) named it, 2 with changed values (largest difference 20): `messages/verbose-byte`, `datatypes/byte`
  - AFNI-90: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the outputs written where AFNI refuses (DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD/LLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLLL.nii, exit N)): `defects/prefix-256`
  - AFNI-92: 2 case(s) named it, 1 with changed values (largest difference 59.1), 1 with other changes (messages): `twopass/nomaxdisp`, `difftest/notice-after-open-line`
  - AFNI-93: 4 case(s) named it, 3 with changed values (largest difference 59.1), 2 with other changes (messages; the outputs written where AFNI refuses (o.nii, exit N)): `edge cases/four-slices-twopass`, `edge cases/four-slices-nocoarserot`, `difftest/fourier-lone-row-null`, `difftest/notice-after-open-line`
  - AFNI-95: 1 case(s) named it, 0 with changed values (largest difference 0): `errors/base-twice`
  - AFNI-97: 1 case(s) named it, 0 with changed values (largest difference 0), 1 with other changes (the outputs written where AFNI refuses (o.nii, p.ND, d.txt, exit N)): `difftest/fourier-lone-row-default`
  - AFNI-99: 17 case(s) named it, 11 with changed values (largest difference 12.3), 16 with other changes (messages; the outputs written where AFNI refuses (o.nii, p.ND, d.txt, exit N)): `outputs/float`, `outputs/savedisp-oblique`, `rotparent/rotparent-oblique`, `rotparent/rotparent-int16`, `datatypes/s2_short`, `datatypes/s2_scaled`, `datatypes/s3_oblique`, `datatypes/s4_sagittal` …
  - GLIBC-07 (not named): 1 case(s) named it, 1 with changed values (largest difference 0.000122): `twopass/coarse-0-0`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (both succeed, or both reject the arguments); where AFNI crashes, larmorx exits 1 |
| Messages | standard output and error byte for byte (AFNI's banner and compile date, larmorx's own lines and the elapsed time set aside) |
| Every output (the registered series, -1Dfile, -dfile, -1Dmatrix_save, -maxdisp1D, -savedisp, -SAVEDISP) | the same bytes: NIfTI values bit for bit and header bytes; text files byte for byte; every file written compared |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| base | 7 | 7 | 0 | 0 | 0 |
| datatypes | 10 | 10 | 0 | 0 | 0 |
| defects | 3 | 2 | 1 | 0 | 0 |
| difftest | 7 | 3 | 4 | 0 | 0 |
| divergences | 2 | 0 | 0 | 2 | 0 |
| edge cases | 23 | 22 | 1 | 0 | 0 |
| environment | 6 | 6 | 0 | 0 | 0 |
| errors | 24 | 1 | 23 | 0 | 0 |
| fmriprep | 8 | 8 | 0 | 0 | 0 |
| global | 8 | 8 | 0 | 0 | 0 |
| interpolation | 21 | 21 | 0 | 0 | 0 |
| messages | 10 | 10 | 0 | 0 | 0 |
| outputs | 10 | 10 | 0 | 0 | 0 |
| rotparent | 20 | 19 | 1 | 0 | 0 |
| tshift | 8 | 8 | 0 | 0 | 0 |
| twodup | 4 | 4 | 0 | 0 | 0 |
| twopass | 11 | 11 | 0 | 0 | 0 |
| weights | 8 | 8 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `divergences/default-prefix` | AFNI's default prefix 'volreg' is its own dataset format, which larmorx does not write: it refuses (NIfTI only). |
| `divergences/savedisp-afni-format` | -savedisp without .nii/.nii.gz writes AFNI's own dataset format, which larmorx does not write: it refuses (NIfTI only). |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `rotparent/rotparent-without-attributes` | -rotparent of a dataset without VOLREG attributes: AFNI's error | AFNI exit 1: ** -rotparent dataset doesn't have VOLREG attributes!?; larmorx exit 1: ** -rotparent dataset doesn't have VOLREG attributes!? (the same messages) |
| `edge cases/constant-twopass` | a constant series, -twopass (AFNI-89) | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: the base's least squares could not be set up (the messages … |
| `errors/unknown-option` | -bogus <input> | AFNI exit 1: ** Unknown option: -bogus; larmorx exit 1: ** Unknown option: -bogus (the same messages) |
| `errors/final-bogus` | -final bogus <input> | AFNI exit 1: ** Illegal mode after -final; larmorx exit 1: ** Illegal mode after -final (the same messages) |
| `errors/twoblur-low` | -twoblur 1 <input> | AFNI exit 1: ** ERROR: value after -twoblur is < 2.0; larmorx exit 1: ** ERROR: value after -twoblur is < 2.0 (the same messages) |
| `errors/zpad-negative` | -zpad -2 <input> | AFNI exit 1: ** ERROR: Can't use -zpad -2; larmorx exit 1: ** ERROR: Can't use -zpad -2 (the same messages) |
| `errors/base-beyond` | -base 99 <input> | AFNI exit 1: ** Dataset ~/VScode_projects/super_fmriprep/oracles/inputs/afni-volreg/s1.nii.gz doesn't have base brick index = 99; larmorx exit 1: ** Dataset ~/… |
| `errors/base-negative` | -base -1 <input> | AFNI exit 1: ** FATAL ERROR: Illegal negative number after -base; larmorx exit 1: ** FATAL ERROR: Illegal negative number after -base (the same messages) |
| `errors/edging-negative` | -edging -1 <input> | AFNI exit 1: ** Illegal value after -edging; larmorx exit 1: ** Illegal value after -edging (the same messages) |
| `errors/edging-30pc` | -edging 30% <input> | AFNI exit 1: ** Illegal percentage after -edging; larmorx exit 1: ** Illegal percentage after -edging (the same messages) |
| `errors/weight-no-selector` | -weight <weight> <input> | AFNI exit 1: ** Illegal weight dataset after -weight; larmorx exit 1: ** Illegal weight dataset after -weight (the same messages) |
| `errors/weight-missing` | -weight nothere.nii.gz[0] -prefix o.nii <input> | AFNI exit 1: ** Can't open weight dataset nothere.nii.gz; larmorx exit 1: ** Can't open weight dataset nothere.nii.gz (the same messages) |
| `errors/base-missing` | -base nothere.nii.gz -prefix o.nii <input> | AFNI exit 1: ** FATAL ERROR: Couldn't open -base dataset nothere.nii.gz; larmorx exit 1: ** FATAL ERROR: Couldn't open -base dataset nothere.nii.gz (the same m… |
| `errors/tshift-negative` | -tshift -1 <input> | AFNI exit 1: ** -tshift parameter is negative!; larmorx exit 1: ** -tshift parameter is negative! (the same messages) |
| `errors/no-dataset` | -prefix o.nii | AFNI exit 1: ** FATAL ERROR: Too few arguments!? Last=o.nii; larmorx exit 1: ** FATAL ERROR: Too few arguments!? Last=o.nii (the same messages) |
| `errors/missing-input` | -prefix o.nii /nonexistent/x.nii | AFNI exit 1: ** Can't open dataset /nonexistent/x.nii; larmorx exit 1: ** Can't open dataset /nonexistent/x.nii (the same messages) |
| `errors/prefix-after-dataset` | -prefix o.nii <input> -prefix | AFNI exit 1: ** FATAL ERROR: Too many arguments?! Dataset=~/VScode_projects/super_fmriprep/oracles/inputs/afni-volreg/s1.nii.gz?; larmorx exit 1: ** FATAL ERRO… |
| `errors/maxite-last` | -maxite | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: need argument after -maxite (AFNI-88: AFNI crashes here)), … |
| `errors/input-last` | -input | AFNI exit 1: ** FATAL ERROR: Can't open input dataset '(null)'; larmorx exit 1: ** FATAL ERROR: Can't open input dataset '(null)' (the same messages) |
| `errors/rotparent-last` | -rotparent | AFNI exit 1: ; larmorx exit 1: (the same messages) |
| `errors/gridparent-last` | -gridparent | AFNI exit 1: ; larmorx exit 1: (the same messages) |
| `errors/matrix-save-last` | -1Dmatrix_save | AFNI exit 1: ** FATAL ERROR: no argument after '-1Dmatrix_save'!; larmorx exit 1: ** FATAL ERROR: no argument after '-1Dmatrix_save'! (the same messages) |
| `errors/one-volume-no-base` | -base 1 -prefix o.nii <one volume> | AFNI exit 1: ** Dataset ~/VScode_projects/super_fmriprep/oracles/inputs/afni-volreg/e_one.nii.gz doesn't have base brick index = 1; larmorx exit 1: ** Dataset … |
| `errors/base-beyond-input` | -base 99 -prefix o.nii -input <input> | AFNI exit 1: ** Dataset (null) doesn't have base brick index = 99; larmorx exit 1: ** Dataset (null) doesn't have base brick index = 99 (the same messages) |
| `errors/base-beyond-input-stray` | -base 99 -prefix o.nii -input <input> stray | AFNI exit 1: ** Dataset stray doesn't have base brick index = 99; larmorx exit 1: ** Dataset stray doesn't have base brick index = 99 (the same messages) |
| `defects/prefix-256` | a -prefix of 256 bytes: AFNI aborts (AFNI-90) | AFNI crashes (exit 1, Fatal Signal 6 (SIGABRT) received); larmorx refuses (exit 1: ** FATAL ERROR: the -prefix name has 256 bytes; AFNI's buffer holds 255 and … |
| `difftest/constant-twopass-lsqfit` | -twopass on a constant 4 x 34 x 4 series: the least squares cannot be set up and AFNI crashes at the first volume (AFNI-89); larmorx refuses after AFNI's messages | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: the base's least squares could not be set up (the messages … |
| `difftest/crash-after-progress-line` | -heptic -final Fourier -twopass -verbose: AFNI crashes (AFNI-97) after its open progress line 'Starting final pass ... 0..1'; larmorx ends the line before its error | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: a Fourier rotation shifted the last of an odd count of rows… |
| `difftest/fourier-lone-row-default` | -Fourier -final NN -twopass -twoblur 4.5 -wtinp on a uint16 series 21 degrees oblique: the fit runs to rotations of 160 degrees, and AFNI crashes in the final pass (AFNI-97); the default clears the lone row alone, names AFNI-97 and writes every output | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: a Fourier rotation shifted the last of an odd count of rows… |
| `difftest/fourier-lone-row-null` | -Fourier -twopass on 37 x 4 x 16 voxels: AFNI's coarse search, on a volume too thin for its region, starts the fit from its first candidate (AFNI-93), the final pass's rotation runs far beyond a head's motion and shifts the lone last row (odd x) and the previous pair's second row beyond the row length: AFNI clears the missing row through a NULL pointer and crashes (AFNI-97); the default, AFNI-93 corrected, does not run away and writes the output | AFNI crashes (exit 1, Fatal Signal 11 (SIGSEGV) received); larmorx refuses (exit 1: ** FATAL ERROR: a Fourier rotation shifted the last of an odd count of rows… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (1a9e7f20a8af) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

Both programs run in directories of their own on the same input files (made once in `oracles/inputs/afni-volreg/`), in the same environment (no AFNI variables of this process, `HOME` an empty directory, `AFNI_DONT_LOGFILE=YES`, `AFNI_NIFTI_NOEXT=YES`, `OMP_NUM_THREADS=1`: 3dvolreg has no OpenMP loop, plus each case's variables); outputs are compared as stored (data bytes, header bytes decompressed, text byte for byte). The replica is compared in `reproduce` mode; the fix pass runs its default (correctly rounded maths, which an iterative fit does not name: GLIBC-07). fMRIPrep's calls run on RobustAverage's input re-made from each oracle run's node, and the median of the registered volumes is compared with the node's own output; they also run fMRIPrep 25.2.6's own AFNI (its image). AFNI version: 26.3.05 (`--afni-version`, `LARMORX_AFNI_VERSION`; builds: 26.3.05, 25.2.09). Each case also ran AFNI 25.2.09 on the same arguments (from the oracle-output cache where an earlier record ran it): the check 'AFNI's own step: 25.2.09 → 26.3.05' says whether AFNI's own exit status, messages and outputs changed, and why.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `interpolation/default` | pass | 19/19 | the defaults (heptic), every text output |
| `interpolation/Fourier` | pass | 13/13 | -Fourier |
| `interpolation/cubic` | pass | 13/13 | -cubic |
| `interpolation/quintic` | pass | 12/12 | -quintic |
| `interpolation/linear` | pass | 12/12 | -linear |
| `interpolation/heptic` | pass | 12/12 | -heptic |
| `interpolation/fourier-lowercase` | pass | 10/10 | -fourier: AFNI compares four letters |
| `interpolation/final-NN` | pass | 11/11 | -final NN |
| `interpolation/final-Fourier` | pass | 10/10 | -final Fourier |
| `interpolation/final-cubic` | pass | 10/10 | -final cubic |
| `interpolation/final-linear` | pass | 10/10 | -final linear |
| `interpolation/final-quintic` | pass | 10/10 | -final quintic |
| `interpolation/final-heptic` | pass | 10/10 | -final heptic |
| `interpolation/final-cubic-noclip` | pass | 9/9 | -final cubic -noclip |
| `interpolation/Fourier-noclip` | pass | 8/8 | -Fourier -noclip |
| `interpolation/clipit` | pass | 8/8 | -clipit (the default, a notice), -wtrim, -coarserot |
| `interpolation/zpad-0` | pass | 11/11 | -zpad 0 |
| `interpolation/zpad-2` | pass | 10/10 | -zpad 2 |
| `interpolation/zpad-8` | pass | 10/10 | -zpad 8 |
| `interpolation/zpad-twice` | pass | 8/8 | -zpad 3 -zpad 2: the last one |
| `interpolation/Fourier-zpad-0` | pass | 10/10 | -Fourier -zpad 0 |
| `base/index` | pass | 11/11 | -base 3 |
| `base/last` | pass | 10/10 | -base 5 (the last volume) |
| `base/dataset` | pass | 12/12 | -base <3D dataset> |
| `base/dataset-twopass` | pass | 10/10 | -twopass -base <3D dataset> |
| `base/dataset-4d` | pass | 5/5 | -base <4D dataset>: its first volume |
| `base/3d-input` | pass | 8/8 | a 3D input with an external base |
| `base/vol3d-input` | pass | 10/10 | a 3D input, the base a moving series |
| `twopass/twopass` | pass | 11/11 | -twopass |
| `twopass/twoblur-3` | pass | 11/11 | -twopass -twoblur 3 |
| `twopass/coarse-1-1` | pass | 10/10 | -twopass -coarse 1 1 |
| `twopass/coarse-0-0` | pass | 10/10 | -twopass -coarse 0 0 |
| `twopass/sinit` | pass | 11/11 | -twopass -sinit |
| `twopass/nomaxdisp` | pass | 10/10 | -twopass -nomaxdisp: AFNI-92's clip fraction |
| `twopass/nocoarserot` | pass | 10/10 | -nocoarserot -twopass: AFNI reads -noclip (AFNI-86) |
| `twopass/twopass-weight` | pass | 10/10 | -twopass -weight |
| `twopass/twopass-wtinp` | pass | 10/10 | -twopass -wtinp |
| `twopass/twopass-maxite-2` | pass | 10/10 | -twopass -maxite 2: raised to 66 |
| `twopass/fmriprep-call` | pass | 17/17 | fMRIPrep's call on the synthetic series |
| `weights/wtinp` | pass | 11/11 | -wtinp |
| `weights/weight` | pass | 11/11 | -weight 'wset[1]' |
| `weights/edging-3` | pass | 11/11 | -edging 3 |
| `weights/edging-10pc` | pass | 10/10 | -edging 10% |
| `weights/edging-0` | pass | 10/10 | -edging 0 |
| `weights/controls` | pass | 11/11 | -maxite 2 -x_thresh 0.1 -rot_thresh 0.1 -delta 1 |
| `weights/params` | pass | 10/10 | -params 3 0.05 0.05 0.5 |
| `weights/controls-nonpositive` | pass | 10/10 | -maxite 0 -delta -1: ignored |
| `environment/edging-env` | pass | 10/10 | AFNI_VOLREG_EDGING=10 |
| `environment/edging-env-pc` | pass | 10/10 | AFNI_VOLREG_EDGING=12% |
| `environment/blur-firfac` | pass | 10/10 | -twopass with AFNI_BLUR_FIRFAC=1.5 |
| `environment/blur-firold` | pass | 10/10 | -twopass with AFNI_BLUR_FIROLD=YES |
| `environment/rota-zpad` | pass | 10/10 | AFNI_ROTA_ZPAD=7: not read |
| `outputs/rotcom` | pass | 8/8 | -rotcom -prefix NULL |
| `outputs/prefix-null` | pass | 14/14 | -prefix NULL with every text output |
| `outputs/matrix-no-suffix` | pass | 10/10 | -1Dmatrix_save m: .aff12.1D added |
| `outputs/maxdisp1D-stdout` | pass | 10/10 | -maxdisp1D -: standard output, and the file -_delt |
| `outputs/float` | pass | 9/9 | -float on int16 |
| `outputs/nii-gz` | pass | 8/8 | a compressed output |
| `outputs/savedisp` | pass | 15/15 | -savedisp |
| `outputs/SAVEDISP` | pass | 46/46 | -SAVEDISP -verbose: 19 datasets |
| `outputs/savedisp-oblique` | pass | 12/12 | -savedisp -prefix NULL on an oblique series |
| `outputs/savedisp-bare` | pass | 5/5 | -savedisp .nii |
| `tshift/tshift-0` | pass | 11/11 | -tshift 0 on sequential slices |
| `tshift/tshift-1` | pass | 10/10 | -tshift 1 |
| `tshift/tshift-verbose` | pass | 10/10 | -verbose -tshift 2 -base 1 |
| `tshift/tshift-twopass` | pass | 10/10 | -tshift 0 -twopass -Fourier |
| `tshift/tshift-negative-slope` | pass | 10/10 | -tshift 0 on int16 with a negative slope (AFNI-01) and alternating slices |
| `tshift/tshift-no-timing` | pass | 8/8 | -tshift 0 without slice times |
| `tshift/tshift-ignore-large` | pass | 8/8 | -tshift 0 on 4 volumes: ignore too large |
| `tshift/tshift-savedisp` | pass | 14/14 | -tshift 0 -savedisp |
| `rotparent/rotparent` | pass | 13/13 | -rotparent |
| `rotparent/rotparent-gridparent` | pass | 8/8 | -verbose -rotparent -gridparent (27 slices) |
| `rotparent/gridparent-24` | pass | 8/8 | -rotparent -gridparent (24 slices) -Fourier -final cubic |
| `rotparent/gridparent-moved` | pass | 8/8 | -rotparent -gridparent (moved origin) -noclip -final linear |
| `rotparent/rotparent-slice-times` | pass | 8/8 | -verbose -rotparent -gridparent on slice-timed data |
| `rotparent/rotparent-tshift` | pass | 8/8 | -tshift 0 -rotparent |
| `rotparent/gridparent-gp22` | pass | 5/5 | -rotparent -gridparent with too few slices |
| `rotparent/gridparent-gpx` | pass | 5/5 | -rotparent -gridparent with another x size |
| `rotparent/gridparent-gpsp` | pass | 5/5 | -rotparent -gridparent with another spacing |
| `rotparent/gridparent-alone` | pass | 10/10 | -gridparent without -rotparent |
| `rotparent/rotparent-twice` | pass | 5/5 | -rotparent twice |
| `rotparent/gridparent-twice` | pass | 5/5 | -gridparent twice |
| `rotparent/rotparent-missing` | pass | 5/5 | -rotparent of a missing file |
| `rotparent/rotparent-twopass` | pass | 10/10 | -rotparent -twopass |
| `rotparent/rotparent-oblique` | pass | 10/10 | -rotparent on an oblique series |
| `rotparent/rotparent-twodup-grid` | pass | 8/8 | -rotparent -twopass -twodup -base <shifted> -gridparent |
| `rotparent/rotparent-twodup` | pass | 8/8 | -rotparent -twopass -twodup -base <shifted> |
| `rotparent/rotparent-savedisp` | pass | 14/14 | -rotparent -savedisp -gridparent |
| `rotparent/rotparent-int16` | pass | 8/8 | -rotparent on int16 (LPI) |
| `rotparent/rotparent-without-attributes` | both-error | 5/5 | -rotparent of a dataset without VOLREG attributes: AFNI's error |
| `twodup/twodup` | pass | 11/11 | -twopass -twodup -base <shifted> |
| `twodup/twodup-onepass` | pass | 8/8 | -twodup without -twopass |
| `twodup/twodup-internal` | pass | 10/10 | -twopass -twodup, an internal base |
| `twodup/twodup-savedisp` | pass | 14/14 | -twopass -twodup -base <shifted> -savedisp on slice-timed data |
| `messages/verbose` | pass | 8/8 | -verbose |
| `messages/verbose-verbose` | pass | 8/8 | -verbose -verbose -twopass -Fourier |
| `messages/base-before-verbose` | pass | 8/8 | -base <dset> -verbose: messages in the scan's order |
| `messages/verbose-base-mismatch` | pass | 5/5 | -verbose -base <another grid> |
| `messages/verbose-weight` | pass | 8/8 | -verbose -weight -base 2 |
| `messages/verbose-names` | pass | 8/8 | -verbose with a base and a weight: their names |
| `messages/verbose-byte` | pass | 10/10 | -verbose on bytes |
| `messages/verbose-tshift-base` | pass | 8/8 | -verbose -tshift 0 -base <a 4D weight> |
| `messages/verbose-3d-input` | pass | 8/8 | -verbose, a 3D input and a 4D base |
| `messages/weight-index-beyond` | pass | 5/5 | -weight 'wset[5]': beyond its volumes |
| `datatypes/s2_short` | pass | 12/12 | int16, LPI |
| `datatypes/s2_scaled` | pass | 12/12 | int16 with a slope: written as float |
| `datatypes/s3_oblique` | pass | 18/18 | oblique, odd sizes |
| `datatypes/s4_sagittal` | pass | 18/18 | sagittal |
| `datatypes/e_nifti2` | pass | 12/12 | NIfTI-2 |
| `datatypes/e_plain` | pass | 12/12 | uncompressed NIfTI-1 |
| `datatypes/e_qform` | pass | 12/12 | a qform only |
| `datatypes/e_bigendian` | pass | 12/12 | int16, big-endian |
| `datatypes/byte` | pass | 10/10 | bytes (AFNI-87) |
| `datatypes/input-option` | pass | 8/8 | -input, and a dataset name left over |
| `edge cases/one-volume` | pass | 5/5 | a 4D file of one volume |
| `edge cases/one-volume-twopass` | pass | 5/5 | one volume, -twopass |
| `edge cases/no-motion` | pass | 14/14 | four identical volumes |
| `edge cases/no-motion-fmriprep` | pass | 12/12 | four identical volumes, fMRIPrep's call |
| `edge cases/large_z` | pass | 12/12 | rotations of 12 degrees per volume about z, -twopass -Fourier |
| `edge cases/large_z-onepass` | pass | 10/10 | the same, one pass |
| `edge cases/large_z-nocoarserot` | pass | 10/10 | the same, -twopass -nocoarserot (AFNI-86) |
| `edge cases/large_x` | pass | 12/12 | rotations of 9 degrees per volume about x, -twopass -Fourier |
| `edge cases/large_x-onepass` | pass | 10/10 | the same, one pass |
| `edge cases/large_x-nocoarserot` | pass | 10/10 | the same, -twopass -nocoarserot (AFNI-86) |
| `edge cases/nan` | pass | 12/12 | NaN in volume 2 |
| `edge cases/nan-twopass` | pass | 10/10 | NaN in volume 2, fMRIPrep's call |
| `edge cases/nan-base` | pass | 10/10 | NaN in the base (-base 2) |
| `edge cases/two-slices` | pass | 10/10 | two slices |
| `edge cases/one-slice` | pass | 5/5 | one slice |
| `edge cases/one-slice-twopass` | pass | 5/5 | one slice, -twopass |
| `edge cases/subsampled` | pass | 12/12 | 120 x 120 x 120 voxels, -twopass: the coarse search blurred and subsampled |
| `edge cases/subsampled-fmriprep` | pass | 10/10 | 120 x 120 x 120 voxels, fMRIPrep's call |
| `edge cases/subsampled-coarse-2` | pass | 10/10 | 120 x 120 x 120 voxels, -twopass -coarse 2 1: the subsampled step 1 |
| `edge cases/four-slices-twopass` | pass | 10/10 | four slices, -twopass: an empty coarse region (AFNI-93) |
| `edge cases/four-slices-nocoarserot` | pass | 10/10 | four slices, -twopass -nocoarserot |
| `edge cases/constant` | pass | 8/8 | a constant series: one pass registers it |
| `edge cases/constant-twopass` | both-error | 4/4 | a constant series, -twopass (AFNI-89) |
| `errors/unknown-option` | both-error | 5/5 | -bogus <input> |
| `errors/final-bogus` | both-error | 5/5 | -final bogus <input> |
| `errors/twoblur-low` | both-error | 5/5 | -twoblur 1 <input> |
| `errors/zpad-negative` | both-error | 5/5 | -zpad -2 <input> |
| `errors/base-beyond` | both-error | 5/5 | -base 99 <input> |
| `errors/base-negative` | both-error | 5/5 | -base -1 <input> |
| `errors/edging-negative` | both-error | 5/5 | -edging -1 <input> |
| `errors/edging-30pc` | both-error | 5/5 | -edging 30% <input> |
| `errors/weight-no-selector` | both-error | 5/5 | -weight <weight> <input> |
| `errors/weight-missing` | both-error | 5/5 | -weight nothere.nii.gz[0] -prefix o.nii <input> |
| `errors/base-missing` | both-error | 5/5 | -base nothere.nii.gz -prefix o.nii <input> |
| `errors/tshift-negative` | both-error | 5/5 | -tshift -1 <input> |
| `errors/no-dataset` | both-error | 5/5 | -prefix o.nii |
| `errors/missing-input` | both-error | 5/5 | -prefix o.nii /nonexistent/x.nii |
| `errors/base-twice` | pass | 8/8 | -base 0 -base 3 -prefix o.nii <input> |
| `errors/prefix-after-dataset` | both-error | 5/5 | -prefix o.nii <input> -prefix |
| `errors/maxite-last` | both-error | 4/4 | -maxite |
| `errors/input-last` | both-error | 5/5 | -input |
| `errors/rotparent-last` | both-error | 5/5 | -rotparent |
| `errors/gridparent-last` | both-error | 5/5 | -gridparent |
| `errors/matrix-save-last` | both-error | 5/5 | -1Dmatrix_save |
| `errors/one-volume-no-base` | both-error | 5/5 | -base 1 -prefix o.nii <one volume> |
| `errors/base-beyond-input` | both-error | 5/5 | -base 99 -prefix o.nii -input <input> |
| `errors/base-beyond-input-stray` | both-error | 5/5 | -base 99 -prefix o.nii -input <input> stray |
| `divergences/default-prefix` | expected-divergence | 2/3 | no -prefix: AFNI writes its own format (volreg+orig) |
| `divergences/savedisp-afni-format` | expected-divergence | 2/3 | -savedisp sd: AFNI's own format |
| `global/exists` | pass | 6/6 | an existing output (AFNI-11) |
| `global/overwrite` | pass | 8/8 | -overwrite |
| `global/deconflict-yes` | pass | 8/8 | AFNI_DECONFLICT=YES: renamed |
| `global/D-write-type-2` | pass | 8/8 | -DAFNI_NIFTI_WRITE_TYPE=2 |
| `environment/quiet-startup` | pass | 8/8 | AFNI_QUIET_STARTUP=YES |
| `global/prefix-with-blank` | pass | 6/6 | a -prefix with a blank: AFNI writes nothing, with exit status 0 (AFNI-12) |
| `global/savedisp-with-blank` | pass | 5/5 | -savedisp with a blank (AFNI-12) |
| `global/input-dir-with-blank` | pass | 6/6 | an input in a directory with a blank (AFNI-18) |
| `global/missing-directory` | pass | 6/6 | an output in a missing directory (AFNI-26) |
| `defects/prefix-255` | pass | 8/8 | a -prefix of 255 bytes |
| `defects/prefix-256` | both-error | 5/5 | a -prefix of 256 bytes: AFNI aborts (AFNI-90) |
| `fmriprep/ds000005-run-01` | pass | 21/21 | fMRIPrep's RobustAverage call (single-band, int16, 64 x 64 x 34, 20 volumes) |
| `fmriprep/ds000005-run-03` | pass | 20/20 | fMRIPrep's RobustAverage call (two volumes) |
| `fmriprep/ds005454` | pass | 20/20 | fMRIPrep's RobustAverage call (multiband, 160 x 160 x 96 (the subsampled coarse search), oblique) |
| `fmriprep/ds003592-echo-1` | pass | 20/20 | fMRIPrep's RobustAverage call (multi-echo (echo 1), oblique, 64 x 64 x 43) |
| `fmriprep/ds004958` | pass | 20/20 | fMRIPrep's RobustAverage call (multiband 8, uint16 stored, 104 x 104 x 72, 3 volumes, 24 degrees oblique) |
| `fmriprep/ds002603-mb4` | pass | 20/20 | fMRIPrep's RobustAverage call (Philips multiband 4, float32 stored) |
| `fmriprep/pepolar-ds005040` | pass | 20/20 | fMRIPrep's RobustAverage call (sdcflows' PEPOLAR reference average: 2 topup-corrected volumes, float32) |
| `fmriprep/pepolar-ds001600` | pass | 20/20 | fMRIPrep's RobustAverage call (sdcflows' PEPOLAR reference average: 2 volumes, 80 x 80 x 60) |
| `defects/coarse-1-large` | pass | 4/4 | -twopass -coarse 1 1 on 120 x 120 x 120 voxels: AFNI never ends (AFNI-94); larmorx refuses in both modes (AFNI is not r… |
| `difftest/afni99-notice-unchanged` | pass | 12/12 | AFNI-99 named only where a maximum displacement the run prints or writes changes: -cubic -final Fourier -zpad 0 on an i… |
| `difftest/constant-twopass-lsqfit` | both-error | 4/4 | -twopass on a constant 4 x 34 x 4 series: the least squares cannot be set up and AFNI crashes at the first volume (AFNI… |
| `difftest/crash-after-progress-line` | both-error | 4/4 | -heptic -final Fourier -twopass -verbose: AFNI crashes (AFNI-97) after its open progress line 'Starting final pass ... … |
| `difftest/fourier-lone-row-default` | both-error | 5/5 | -Fourier -final NN -twopass -twoblur 4.5 -wtinp on a uint16 series 21 degrees oblique: the fit runs to rotations of 160… |
| `difftest/fourier-lone-row-null` | both-error | 5/5 | -Fourier -twopass on 37 x 4 x 16 voxels: AFNI's coarse search, on a volume too thin for its region, starts the fit from… |
| `difftest/notice-after-open-line` | pass | 10/10 | -twopass on blobs with a large scale difference: AFNI's 'might not converge.' leaves its line open, and larmorx's notic… |
| `difftest/uint16-base-and-input` | pass | 5/5 | a uint16 base and input: AFNI announces the conversion to float once per run (THD_open_nifti's n_xform_warn); the repli… |
