# Parity: FLIRT (clean-room original)

**22 case(s) fail.** 157 cases on the `standard` tier: 89 pass, 35 rejected by both, 4 expected divergences, 22 failures, 7 skipped.

- **Validated:** `larmorx fsl flirt` and `convert_xfm` (larmorx 0.0.1, crate larmorx-fsl: the clean-room original written from specs/flirt.md on the shared registration engine of larmorx-mri, Apache-2.0)
- **Reference:** flirt and convert_xfm from FSL 6.0.7.23 (fsl-flirt 2111.4), the conda build at `~/fsl-6.0.7.23`
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-02 on Linux x86_64, with `python -m larmorx_validation parity fsl-flirt --tier standard`
- **Original programs:** 169 runs: 167 from the oracle cache, 2 run; cache `<workspace>/oracles/cache`, mode `use`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (where FSL aborts, the original exits 1) |
| Standard output and error | the same lines, the numbers in them read as numbers |
| Files written | the same names |
| Matrices | identical text, or the 95th-percentile displacement over the reference's brain within 0.1 mm (boundary-based, 6 dof) or 0.5 mm (other registrations); `-applyxfm` and `convert_xfm` identical |
| Images | bit-identical from the same matrix; from the original's own matrix, NCC with FSL's image at least 0.999 inside the reference's mask |
| D13 fix pass | a defect named exactly when the default's outputs differ |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bbr | 11 | 5 | 0 | 0 | 5 |
| convert_xfm | 16 | 7 | 9 | 0 | 0 |
| edge | 32 | 9 | 19 | 4 | 0 |
| fmriprep | 3 | 3 | 0 | 0 | 0 |
| real | 58 | 42 | 0 | 0 | 16 |
| schedule | 29 | 22 | 7 | 0 | 0 |
| verbose | 8 | 1 | 0 | 0 | 1 |

## Failures

### `real/anglerep-quaternion`

-anglerep quaternion: the rotations as a quaternion's last three components (12 dof)

- **stderr: the same lines (numbers aside)** (identical = no, threshold the same text, numbers read as numbers): line 5: FSL '<end>', original 'Parameters do not form a valid axis - greater than unity'

### `real/anglerep-quaternion-dof6`

-anglerep quaternion at 6 dof, -out, verbose (the parameters printed)

- **stderr: the same lines (numbers aside)** (identical = no, threshold the same text, numbers read as numbers): line 10: FSL '<end>', original 'Parameters do not form a valid axis - greater than unity'

### `real/dof6`

-dof 6

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 0.1789, threshold 0.1 or FSL's own variability): maximum 0.2151 mm, RMS deviation 0.4625 mm; FSL's own p95 on a perturbed input 0.1553 mm

### `real/cost-normmi`

-cost normmi (and -searchcost corratio)

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 1.4169, threshold 0.5 or FSL's own variability): maximum 1.7684 mm, RMS deviation 3.2780 mm; FSL's own p95 on a perturbed input 1.2445 mm

### `real/cost-woods`

-cost woods (and -searchcost corratio)

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 50.4909, threshold 0.5 or FSL's own variability): maximum 54.9439 mm, RMS deviation 278.8845 mm; FSL's own p95 on a perturbed input 44.1365 mm

### `real/cost-labeldiff`

-cost labeldiff (and -searchcost corratio)

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 66.0454, threshold 0.5 or FSL's own variability): maximum 75.8375 mm, RMS deviation 96.9277 mm; FSL's own p95 on a perturbed input 54.5916 mm

### `real/coarse-fine`

-coarsesearch 30 -finesearch 10

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 0.1618, threshold 0.1 or FSL's own variability): maximum 0.1971 mm, RMS deviation 0.4197 mm; FSL's own p95 on a perturbed input 0.1553 mm

### `real/weights`

-refweight and -inweight

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 295.9329, threshold 0.5 or FSL's own variability): maximum 329.5561 mm, RMS deviation 281.4465 mm; FSL's own p95 on a perturbed input 17.6579 mm

### `real/weights-leastsq`

weights with -cost leastsq

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 0.2406, threshold 0.1 or FSL's own variability): maximum 0.2756 mm, RMS deviation 0.2089 mm; FSL's own p95 on a perturbed input 0.2067 mm

### `real/weights-normmi`

weights with -cost normmi

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 131.5883, threshold 0.1 or FSL's own variability): maximum 154.6531 mm, RMS deviation 101.4895 mm; FSL's own p95 on a perturbed input 10.0584 mm

### `real/weights-mutualinfo`

weights with -cost mutualinfo

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 108.893, threshold 0.1 or FSL's own variability): maximum 131.1597 mm, RMS deviation 183.6401 mm; FSL's own p95 on a perturbed input 30.8362 mm

### `real/weights-woods`

weights with -cost woods

- **stderr: the same lines (numbers aside)** (identical = no, threshold the same text, numbers read as numbers): line 3938: FSL 'WARNING: Woods is not implemented with cost function weighting', original '<end>'
- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 59.7626, threshold 0.1 or FSL's own variability): maximum 65.6105 mm, RMS deviation 82.2202 mm; FSL's own p95 on a perturbed input 43.5171 mm

### `real/applyisoxfm-4d-blur-setbackground`

-applyisoxfm 6 with -setbackground (the blur's outside a constant)

- **out.nii.gz: the same values within 1e-5 of the image's range** (max difference / range = 0.408, threshold 1e-05): 53831 of 172800 values differ (max 4.516e+04)

### `real/out-only`

a registration with -out and no -omat (the matrix printed)

- **out.nii.gz: correlation with FSL's image inside the reference's mask** (NCC = 0.998965, threshold 0.999): 

### `real/basescale-big`

14 mm voxels (the base scale from the voxel size)

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 0.2144, threshold 0.1): maximum 0.2374 mm, RMS deviation 0.3144 mm; FSL's own p95 on a perturbed input 0.0022 mm

### `real/basescale-forced`

-basescale 2

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 0.1321, threshold 0.1): maximum 0.1539 mm, RMS deviation 0.1733 mm; FSL's own p95 on a perturbed input 0.0477 mm

### `bbr/default`

BBR (FSL's bbr.sch) with -cost bbr

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 0.1074, threshold 0.1): maximum 0.1319 mm, RMS deviation 0.1612 mm; FSL's own p95 on a perturbed input 0.0148 mm

### `bbr/fmriprep-form`

fMRIPrep's form: -searchcost bbr -basescale 1

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 0.1074, threshold 0.1): maximum 0.1319 mm, RMS deviation 0.1612 mm; FSL's own p95 on a perturbed input 0.0148 mm

### `bbr/type-global_abs`

-bbrtype global_abs

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 0.1074, threshold 0.1): maximum 0.1319 mm, RMS deviation 0.1612 mm; FSL's own p95 on a perturbed input 0.0148 mm

### `bbr/type-local_abs`

-bbrtype local_abs

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 1.1777, threshold 0.1 or FSL's own variability): maximum 1.4270 mm, RMS deviation 2.5016 mm; FSL's own p95 on a perturbed input 0.9038 mm

### `bbr/fieldmap-mask`

-fieldmapmask -pedir -2

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 0.1179, threshold 0.1): maximum 0.1367 mm, RMS deviation 0.2011 mm; FSL's own p95 on a perturbed input 0.0063 mm

### `verbose/v`

-v

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 0.1789, threshold 0.1 or FSL's own variability): maximum 0.2151 mm, RMS deviation 0.4625 mm; FSL's own p95 on a perturbed input 0.1553 mm

## Expected divergences

| Case | Reason |
|---|---|
| `edge/bbrstep-0` | FSL never finishes (FSL-71: undefined behaviour); the original refuses in both modes |
| `edge/read-not-numbers` | FSL never finishes (FSL-72: undefined behaviour); the original refuses in both modes |
| `edge/background-overflow` | FSL crashes (FSL-75: undefined behaviour); the original finishes naming it |
| `edge/background-empty` | FSL crashes (FSL-75: undefined behaviour); the original refuses in both modes |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `schedule/every-command` | a schedule using every command and option | flirt exits 255; the same from the original. Wrong number of args to SETOPTION |
| `schedule/args-copy` | a schedule: copy U | flirt exits 255; the same from the original. Wrong number of args to COPY |
| `schedule/args-optimise` | a schedule: optimise 6 U | flirt exits 255; the same from the original. Wrong number of args to OPTIMISE |
| `schedule/bad-if` | a schedule: if 1 =< 2 | flirt exits 255; the same from the original. Cannot recognise operator =< in IF statement |
| `schedule/bad-costfunction` | a schedule: setoption costfunction nope | flirt exits 255; the same from the original. Unrecognised cost function: nope |
| `schedule/bad-matname` | a schedule: clear Uq | flirt exits -11; the same from the original. Can only accept UA to UZ, not Uq |
| `schedule/alignpaxes-args` | alignpaxes without its matrix | flirt exits 255; the same from the original. Wrong number of args to ALIGNPAXES |
| `edge/no-args` | no arguments (the usage, exit 1) | flirt exits 1; the same from the original. |
| `edge/no-in` | no -in | flirt exits 2; the same from the original. ERROR:: Input volume filename not found |
| `edge/no-ref` | no -ref | flirt exits 2; the same from the original. ERROR:: Reference volume filename not found |
| `edge/not-option` | a word that is not an option | flirt exits 255; the same from the original. Unrecognised option e |
| `edge/unknown-1` | an unknown option with one word after it | flirt exits 255; the same from the original. Lacking argument to option -frob |
| `edge/unknown-2` | an unknown option with two words after it | flirt exits 255; the same from the original. Unrecognised option -frob |
| `edge/lacking` | an option lacking its argument | flirt exits 255; the same from the original. Lacking argument to option -ref |
| `edge/lacking-2` | a two-argument option lacking one | flirt exits 255; the same from the original. Lacking argument to option -searchrx |
| `edge/bad-cost` | -cost nope | flirt exits 255; the same from the original. Unrecognised cost function type: nope |
| `edge/bad-searchcost` | -searchcost nope | flirt exits 255; the same from the original. Unrecognised cost function type: nope |
| `edge/bad-interp` | -interp cubic | flirt exits 255; the same from the original. Unrecognised interpolation method: cubic |
| `edge/bad-datatype` | -datatype long | flirt exits 255; the same from the original. Unrecognised data type: long |
| `edge/bad-sincwindow` | -sincwindow gauss | flirt exits 255; the same from the original. Unrecognised sinc window: gauss |
| `edge/bad-anglerep` | -anglerep rodrigues | flirt exits 255; the same from the original. Unrecognised angle representation: rodrigues |
| `edge/bad-pedir` | -pedir 0 | flirt exits 255; the same from the original. Unrecognised argument to pedir (0) - it should be a number between -3 and 3 (not 0) |
| `edge/missing-schedule` | a -schedule file that does not exist | flirt exits 255; the same from the original. Could not open filenone.sch |
| `edge/coords-mismatch` | -wmcoords and -wmnorms of different lengths | flirt exits 1; the same from the original. Coordinate and Normal matrices wmcoords.txt and n.txt are either zero size or different sizes |
| `edge/outtype-unset` | FSLOUTPUTTYPE unset | flirt exits 1; the same from the original. ERROR:: Environment variable FSLOUTPUTTYPE is not set! Please make sure that the appropriate configuration file is s… |
| `edge/outtype-ANALYZE` | FSLOUTPUTTYPE=ANALYZE | flirt exits 1; the same from the original. ERROR:: Unrecognised value (ANALYZE) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NIFT… |
| `convert_xfm/no-args` | no arguments | convert_xfm exits 1; the same from the original. |
| `convert_xfm/missing` | an input matrix that does not exist | convert_xfm exits 254; the same from the original. Could not open matrix file none.mat Cannot read input-matrix |
| `convert_xfm/short` | a 3-row input matrix | convert_xfm exits 254; the same from the original. Cannot read input-matrix |
| `convert_xfm/no-input` | no input matrix | convert_xfm exits 2; the same from the original. Input matrix filename not found |
| `convert_xfm/two-inputs` | two input matrices | convert_xfm exits 2; the same from the original. Unknown option x.mat |
| `convert_xfm/unknown` | an unknown option | convert_xfm exits 255; the same from the original. Unrecognised option -frob |
| `convert_xfm/lacking` | -omat lacking its argument | convert_xfm exits 2; the same from the original. Lacking argument to option -omat |
| `convert_xfm/singular` | -inverse of a singular matrix | convert_xfm exits -6; the same from the original. terminate called after throwing an instance of 'std::runtime_error' what(): inv(): matrix is singular |
| `convert_xfm/missing-concat` | -concat with a file that does not exist | convert_xfm exits 253; the same from the original. Could not open matrix file none.mat Cannot read concat-matrix |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (b244d758acba) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

The cases are the replica suite's (`fsl-flirt-replica`) for `flirt` and `convert_xfm`; FSL's runs are served from the oracle cache under that suite's keys. The original is compared in `reproduce` mode (D13) and its default `fix` pass against the reproduced run. Cases of options the original has not yet (`-debug`, `-debugsave`, the verbose levels above 1: part 2) are skipped.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds000228-sub-pixar008-noreconall/bold_task_pixar_wf` | pass | 8/8 | fMRIPrep's BBR call |
| `fmriprep/ds003592-sub-280-noreconall/bold_ses_1_task_rest_echo_1_wf` | pass | 8/8 | fMRIPrep's BBR call |
| `fmriprep/ds005040-sub-002-noreconall/bold_task_clip_run_1_wf` | pass | 8/8 | fMRIPrep's BBR call |
| `real/anglerep-quaternion` | fail | 5/6 | -anglerep quaternion: the rotations as a quaternion's last three components (12 dof) |
| `real/anglerep-quaternion-dof6` | fail | 6/7 | -anglerep quaternion at 6 dof, -out, verbose (the parameters printed) |
| `real/default` | pass | 7/7 | the default (12 dof, corratio) with -out and -omat |
| `real/default-full` | pass | 6/6 | the full-size T1w as reference |
| `real/dof6` | fail | 5/6 | -dof 6 |
| `real/dof7` | pass | 6/6 | -dof 7 |
| `real/dof9` | pass | 6/6 | -dof 9 |
| `real/cost-mutualinfo` | pass | 6/6 | -cost mutualinfo (and -searchcost corratio) |
| `real/cost-normmi` | fail | 5/6 | -cost normmi (and -searchcost corratio) |
| `real/cost-normcorr` | pass | 6/6 | -cost normcorr (and -searchcost corratio) |
| `real/cost-leastsq` | pass | 6/6 | -cost leastsq (and -searchcost corratio) |
| `real/cost-woods` | fail | 5/6 | -cost woods (and -searchcost corratio) |
| `real/cost-labeldiff` | fail | 5/6 | -cost labeldiff (and -searchcost corratio) |
| `real/searchcost-normmi` | pass | 6/6 | -searchcost normmi |
| `real/nosearch` | pass | 6/6 | -nosearch |
| `real/searchr` | pass | 6/6 | -searchrx/ry/rz with narrow ranges |
| `real/coarse-fine` | fail | 5/6 | -coarsesearch 30 -finesearch 10 |
| `real/init` | pass | 6/6 | -init (fMRIPrep's initial matrix) |
| `real/usesqform` | pass | 6/6 | -usesqform |
| `real/bins64` | pass | 6/6 | -bins 64 |
| `real/noresample` | pass | 6/6 | -noresample |
| `real/noclamp` | pass | 6/6 | -noclamp |
| `real/minsampling` | pass | 6/6 | -minsampling 4 |
| `real/forcescaling` | pass | 6/6 | -forcescaling |
| `real/weights` | fail | 5/6 | -refweight and -inweight |
| `real/weights-normcorr` | pass | 6/6 | weights with -cost normcorr |
| `real/weights-leastsq` | fail | 5/6 | weights with -cost leastsq |
| `real/weights-normmi` | fail | 5/6 | weights with -cost normmi |
| `real/weights-mutualinfo` | fail | 5/6 | weights with -cost mutualinfo |
| `real/weights-woods` | fail | 4/6 | weights with -cost woods |
| `real/labeldiff-dseg` | pass | 7/7 | -cost labeldiff on a label image |
| `real/known-motion` | pass | 6/6 | a known rigid motion of the T1w (-dof 6) |
| `real/interp-nearestneighbour` | pass | 6/6 | -interp nearestneighbour for -out |
| `real/interp-sinc` | pass | 6/6 | -interp sinc for -out |
| `real/interp-spline` | pass | 6/6 | -interp spline for -out |
| `real/interp-trilinear` | pass | 6/6 | -interp trilinear for -out |
| `real/sinc-window` | pass | 6/6 | -sincwidth 5 -sincwindow blackman |
| `real/sinc-rect` | pass | 6/6 | -sincwindow rectangular |
| `real/datatype-char` | pass | 6/6 | -datatype char |
| `real/datatype-short` | pass | 6/6 | -datatype short |
| `real/datatype-int` | pass | 6/6 | -datatype int |
| `real/datatype-float` | pass | 6/6 | -datatype float |
| `real/datatype-double` | pass | 6/6 | -datatype double |
| `real/int16-input` | pass | 6/6 | an int16 input (its type kept) |
| `real/paddingsize` | pass | 6/6 | -paddingsize 2 |
| `real/setbackground` | pass | 6/6 | -setbackground 100 |
| `real/noresampblur` | pass | 6/6 | -noresampblur |
| `real/applyisoxfm` | pass | 6/6 | -applyisoxfm 3 |
| `real/applyxfm-4d` | pass | 6/6 | -applyxfm on four volumes of the raw series, sinc |
| `real/applyisoxfm-4d-blur` | pass | 6/6 | -applyisoxfm 6 on four volumes of the raw series (blurred first) |
| `real/applyisoxfm-4d-blur-setbackground` | fail | 5/6 | -applyisoxfm 6 with -setbackground (the blur's outside a constant) |
| `real/applyxfm-usesqform` | pass | 7/7 | -applyxfm -usesqform |
| `real/out-only` | fail | 5/6 | a registration with -out and no -omat (the matrix printed) |
| `real/basescale-small` | pass | 6/6 | 0.6 mm voxels (the base scale from the voxel size) |
| `real/basescale-small-init` | pass | 6/6 | 0.6 mm voxels with -init (the initial matrix in the base scale's millimetres) |
| `real/basescale-big` | fail | 5/6 | 14 mm voxels (the base scale from the voxel size) |
| `real/basescale-forced` | fail | 5/6 | -basescale 2 |
| `bbr/default` | fail | 6/7 | BBR (FSL's bbr.sch) with -cost bbr |
| `bbr/fmriprep-form` | fail | 5/6 | fMRIPrep's form: -searchcost bbr -basescale 1 |
| `bbr/type-global_abs` | fail | 5/6 | -bbrtype global_abs |
| `bbr/type-local_abs` | fail | 5/6 | -bbrtype local_abs |
| `bbr/debug` | skipped | – | -debug with -cost bbr (BBR's debug volume `debug`, part 3) |
| `bbr/slope` | pass | 6/6 | -bbrslope -0.25 |
| `bbr/coords` | pass | 6/6 | -wmcoords and -wmnorms |
| `bbr/fieldmap` | pass | 7/7 | -fieldmap -pedir 2 -echospacing (the BBR resampling for -out) |
| `bbr/fieldmap-mask` | fail | 5/6 | -fieldmapmask -pedir -2 |
| `bbr/weights` | pass | 6/6 | BBR with -refweight |
| `bbr/bad-type` | pass | 5/5 | -bbrtype unknown (FSL's exception) |
| `schedule/simple3D.sch` | pass | 6/6 | FSL's simple3D.sch |
| `schedule/xyztrans.sch` | pass | 6/6 | FSL's xyztrans.sch |
| `schedule/ytransonly.sch` | pass | 6/6 | FSL's ytransonly.sch |
| `schedule/ztransonly.sch` | pass | 6/6 | FSL's ztransonly.sch |
| `schedule/sch3Dtrans_3dof` | pass | 6/6 | FSL's sch3Dtrans_3dof |
| `schedule/measurecost1.sch` | pass | 5/5 | FSL's measurecost1.sch |
| `schedule/pairreg1.sch` | pass | 6/6 | FSL's pairreg1.sch |
| `schedule/pairreg2.sch` | pass | 6/6 | FSL's pairreg2.sch |
| `schedule/pairreg3.sch` | pass | 6/6 | FSL's pairreg3.sch |
| `schedule/sch2D_3dof` | pass | 6/6 | FSL's sch2D_3dof on one slice |
| `schedule/sch2D_4dof` | pass | 6/6 | FSL's sch2D_4dof on one slice |
| `schedule/sch2D_5dof` | pass | 6/6 | FSL's sch2D_5dof on one slice |
| `schedule/sch2D_6dof` | pass | 6/6 | FSL's sch2D_6dof on one slice |
| `real/2d` | pass | 7/7 | -2D's default schedule on one slice, with -out |
| `verbose/v` | fail | 5/6 | -v |
| `verbose/2` | skipped | – | -verbose 2 |
| `verbose/3` | skipped | – | -verbose 3 |
| `verbose/5` | skipped | – | -verbose 5 |
| `verbose/printinit` | pass | 5/5 | -printinit |
| `verbose/debugsave` | skipped | – | -debugsave -verbose 4 (the search's volumes saved) |
| `verbose/debug` | skipped | – | -debug: the pyramid's volumes and weights saved (no weights: the one-voxel weight) |
| `verbose/debug-weights` | skipped | – | -debug with -refweight and -inweight (the weights' levels saved) |
| `schedule/every-command` | both-error | 6/6 | a schedule using every command and option |
| `schedule/args-copy` | both-error | 4/4 | a schedule: copy U |
| `schedule/args-optimise` | both-error | 4/4 | a schedule: optimise 6 U |
| `schedule/bad-if` | both-error | 4/4 | a schedule: if 1 =< 2 |
| `schedule/exit` | pass | 5/5 | a schedule: setrow U 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 |
| `schedule/no-matrix` | pass | 5/5 | a schedule: clear U |
| `schedule/bad-costfunction` | both-error | 4/4 | a schedule: setoption costfunction nope |
| `schedule/bad-matname` | both-error | 4/4 | a schedule: clear Uq |
| `schedule/alignpaxes` | pass | 5/5 | alignpaxes on real images |
| `schedule/alignpaxes-constant` | pass | 5/5 | alignpaxes on constant images (zero moments, total 0) |
| `schedule/alignpaxes-nan` | pass | 5/5 | alignpaxes on an input with a NaN voxel (eig_sym fails: the exception) |
| `schedule/alignpaxes-2d` | pass | 5/5 | alignpaxes in 2D mode (one slice: a zero third moment) |
| `schedule/alignpaxes-args` | both-error | 4/4 | alignpaxes without its matrix |
| `schedule/paramsubset-general` | pass | 6/6 | a parameter subset whose column has two ones in different rows of another (rz; tx+ty) |
| `schedule/paramsubset-weighted` | pass | 6/6 | a parameter subset with weights other than 0 and 1 sharing rows (the SVD's QR path) |
| `schedule/paramsubset-singular` | pass | 5/5 | a parameter subset reaching a singular matrix in a cost evaluation (the exception caught by FLIRT's main: FSL-68) |
| `edge/no-args` | both-error | 4/4 | no arguments (the usage, exit 1) |
| `edge/help` | pass | 5/5 | -help |
| `edge/version` | pass | 5/5 | -version |
| `edge/no-in` | both-error | 4/4 | no -in |
| `edge/no-ref` | both-error | 4/4 | no -ref |
| `edge/not-option` | both-error | 4/4 | a word that is not an option |
| `edge/unknown-1` | both-error | 4/4 | an unknown option with one word after it |
| `edge/unknown-2` | both-error | 4/4 | an unknown option with two words after it |
| `edge/lacking` | both-error | 4/4 | an option lacking its argument |
| `edge/lacking-2` | both-error | 4/4 | a two-argument option lacking one |
| `edge/bad-cost` | both-error | 4/4 | -cost nope |
| `edge/bad-searchcost` | both-error | 4/4 | -searchcost nope |
| `edge/bad-interp` | both-error | 4/4 | -interp cubic |
| `edge/bad-datatype` | both-error | 4/4 | -datatype long |
| `edge/bad-sincwindow` | both-error | 4/4 | -sincwindow gauss |
| `edge/bad-anglerep` | both-error | 4/4 | -anglerep rodrigues |
| `edge/bad-pedir` | both-error | 4/4 | -pedir 0 |
| `edge/missing-in` | pass | 5/5 | an input that does not exist |
| `edge/missing-ref` | pass | 5/5 | a reference that does not exist |
| `edge/missing-init` | pass | 5/5 | an -init file that does not exist |
| `edge/missing-schedule` | both-error | 4/4 | a -schedule file that does not exist |
| `edge/bad-init` | pass | 5/5 | an -init file with too few numbers |
| `edge/coords-mismatch` | both-error | 4/4 | -wmcoords and -wmnorms of different lengths |
| `edge/outtype-unset` | both-error | 4/4 | FSLOUTPUTTYPE unset |
| `edge/outtype-NIFTI` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI |
| `edge/outtype-NIFTI_PAIR` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `edge/outtype-NIFTI2_GZ` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `edge/outtype-ANALYZE` | both-error | 4/4 | FSLOUTPUTTYPE=ANALYZE |
| `convert_xfm/no-args` | both-error | 4/4 | no arguments |
| `convert_xfm/help` | pass | 5/5 | -help |
| `convert_xfm/inverse` | pass | 5/5 | -omat -inverse |
| `convert_xfm/concat` | pass | 5/5 | -omat -concat |
| `convert_xfm/fixscaleskew` | pass | 6/6 | -fixscaleskew |
| `convert_xfm/verbose` | pass | 5/5 | -v with -concat and -inverse (the matrices printed) |
| `convert_xfm/verbose-1` | pass | 5/5 | -verbose 1 (the result printed) |
| `convert_xfm/copy` | pass | 5/5 | -omat only (the matrix written again) |
| `convert_xfm/missing` | both-error | 4/4 | an input matrix that does not exist |
| `convert_xfm/short` | both-error | 4/4 | a 3-row input matrix |
| `convert_xfm/no-input` | both-error | 4/4 | no input matrix |
| `convert_xfm/two-inputs` | both-error | 4/4 | two input matrices |
| `convert_xfm/unknown` | both-error | 4/4 | an unknown option |
| `convert_xfm/lacking` | both-error | 4/4 | -omat lacking its argument |
| `convert_xfm/singular` | both-error | 4/4 | -inverse of a singular matrix |
| `edge/bbrstep-0` | expected-divergence | 2/2 | setoption bbrstep 0 with BBR: FLIRT's loop over the boundary points never ends (FSL-71) |
| `edge/read-not-numbers` | expected-divergence | 2/2 | a schedule's read of a file holding a word that is not a number: FLIRT reads for ever, its memory growing (FSL-72) |
| `edge/background-overflow` | expected-divergence | 2/2 | a 1-voxel input and an 8 x 1 x 1 reference: FLIRT's pyramid computes the background of a 3-voxel-wide level, writing pa… |
| `edge/background-empty` | expected-divergence | 2/2 | -applyxfm of a 1-voxel input: its background value reads an empty buffer (FSL-75) |
| `convert_xfm/missing-concat` | both-error | 4/4 | -concat with a file that does not exist |
