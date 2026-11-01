# Parity: FLIRT replica (bit-exact, FSL Licence)

**All cases agree.** 137 cases on the `standard` tier: 103 pass, 34 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx-nc fsl flirt` and `larmorx-nc fsl convert_xfm` (crate larmorx-nc-flirt, translated from FSL 6.0.7.23's source)
- **Reference:** flirt and convert_xfm from FSL 6.0.7.23 (fsl-flirt 2111.4, fsl-newimage 2601.0, fsl-miscmaths 2412.6; the conda build at `~/fsl-6.0.7.23`, as built: glibc 2.35's maths, OpenBLAS on its Haswell kernels)
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-01 on Linux x86_64, with `python -m larmorx_validation parity fsl-flirt-replica --tier standard`
- **Original programs:** 137 runs: 135 from the oracle cache, 2 run; cache `<workspace>/oracles/cache`, mode `use`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (an abort: the replica exits 1 after the same text) |
| Standard output and error | identical (the run directory and FSL's program path replaced; the replica's `larmorx-nc:` lines left out) |
| Files written | the same names |
| Every file | bit-identical (images decompressed; `descrip` left out) |
| D13 fix pass | a defect named exactly when the default's outputs differ |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bbr | 10 | 10 | 0 | 0 | 0 |
| convert_xfm | 16 | 7 | 9 | 0 | 0 |
| edge | 28 | 9 | 19 | 0 | 0 |
| fmriprep | 3 | 3 | 0 | 0 | 0 |
| real | 53 | 53 | 0 | 0 | 0 |
| schedule | 21 | 15 | 6 | 0 | 0 |
| verbose | 6 | 6 | 0 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `schedule/every-command` | a schedule using every command and option | flirt exits 255; the same from the replica. Wrong number of args to SETOPTION |
| `schedule/args-copy` | a schedule: copy U | flirt exits 255; the same from the replica. Wrong number of args to COPY |
| `schedule/args-optimise` | a schedule: optimise 6 U | flirt exits 255; the same from the replica. Wrong number of args to OPTIMISE |
| `schedule/bad-if` | a schedule: if 1 =< 2 | flirt exits 255; the same from the replica. Cannot recognise operator =< in IF statement |
| `schedule/bad-costfunction` | a schedule: setoption costfunction nope | flirt exits 255; the same from the replica. Unrecognised cost function: nope |
| `schedule/bad-matname` | a schedule: clear Uq | flirt exits -11; the same from the replica. Can only accept UA to UZ, not Uq |
| `edge/no-args` | no arguments (the usage, exit 1) | flirt exits 1; the same from the replica. |
| `edge/no-in` | no -in | flirt exits 2; the same from the replica. ERROR:: Input volume filename not found |
| `edge/no-ref` | no -ref | flirt exits 2; the same from the replica. ERROR:: Reference volume filename not found |
| `edge/not-option` | a word that is not an option | flirt exits 255; the same from the replica. Unrecognised option e |
| `edge/unknown-1` | an unknown option with one word after it | flirt exits 255; the same from the replica. Lacking argument to option -frob |
| `edge/unknown-2` | an unknown option with two words after it | flirt exits 255; the same from the replica. Unrecognised option -frob |
| `edge/lacking` | an option lacking its argument | flirt exits 255; the same from the replica. Lacking argument to option -ref |
| `edge/lacking-2` | a two-argument option lacking one | flirt exits 255; the same from the replica. Lacking argument to option -searchrx |
| `edge/bad-cost` | -cost nope | flirt exits 255; the same from the replica. Unrecognised cost function type: nope |
| `edge/bad-searchcost` | -searchcost nope | flirt exits 255; the same from the replica. Unrecognised cost function type: nope |
| `edge/bad-interp` | -interp cubic | flirt exits 255; the same from the replica. Unrecognised interpolation method: cubic |
| `edge/bad-datatype` | -datatype long | flirt exits 255; the same from the replica. Unrecognised data type: long |
| `edge/bad-sincwindow` | -sincwindow gauss | flirt exits 255; the same from the replica. Unrecognised sinc window: gauss |
| `edge/bad-anglerep` | -anglerep rodrigues | flirt exits 255; the same from the replica. Unrecognised angle representation: rodrigues |
| `edge/bad-pedir` | -pedir 0 | flirt exits 255; the same from the replica. Unrecognised argument to pedir (0) - it should be a number between -3 and 3 (not 0) |
| `edge/missing-schedule` | a -schedule file that does not exist | flirt exits 255; the same from the replica. Could not open filenone.sch |
| `edge/coords-mismatch` | -wmcoords and -wmnorms of different lengths | flirt exits 1; the same from the replica. Coordinate and Normal matrices wmcoords.txt and n.txt are either zero size or different sizes |
| `edge/outtype-unset` | FSLOUTPUTTYPE unset | flirt exits 1; the same from the replica. ERROR:: Environment variable FSLOUTPUTTYPE is not set! Please make sure that the appropriate configuration file is so… |
| `edge/outtype-ANALYZE` | FSLOUTPUTTYPE=ANALYZE | flirt exits 1; the same from the replica. ERROR:: Unrecognised value (ANALYZE) of environment variable FSLOUTPUTTYPE Legal values are: NIFTI, NIFTI_PAIR, NIFTI… |
| `convert_xfm/no-args` | no arguments | convert_xfm exits 1; the same from the replica. |
| `convert_xfm/missing` | an input matrix that does not exist | convert_xfm exits 254; the same from the replica. Could not open matrix file none.mat Cannot read input-matrix |
| `convert_xfm/short` | a 3-row input matrix | convert_xfm exits 254; the same from the replica. Cannot read input-matrix |
| `convert_xfm/no-input` | no input matrix | convert_xfm exits 2; the same from the replica. Input matrix filename not found |
| `convert_xfm/two-inputs` | two input matrices | convert_xfm exits 2; the same from the replica. Unknown option x.mat |
| `convert_xfm/unknown` | an unknown option | convert_xfm exits 255; the same from the replica. Unrecognised option -frob |
| `convert_xfm/lacking` | -omat lacking its argument | convert_xfm exits 2; the same from the replica. Lacking argument to option -omat |
| `convert_xfm/singular` | -inverse of a singular matrix | convert_xfm exits -6; the same from the replica. terminate called after throwing an instance of 'std::runtime_error' what(): inv(): matrix is singular |
| `convert_xfm/missing-concat` | -concat with a file that does not exist | convert_xfm exits 253; the same from the replica. Could not open matrix file none.mat Cannot read concat-matrix |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (435d03f394c7) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

The replica runs as a separate program, the FSL replica package's (`larmorx-nc`), compared with FSL in `reproduce` mode (D13); the default `fix` pass is checked against the reproduced run. fMRIPrep's own matrices come from its container's FSL and are reported only. The derived inputs are made once from ds005040's fMRIPrep run (`oracles/inputs/fsl-flirt-replica/`).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds000228-sub-pixar008-noreconall/bold_task_pixar_wf` | pass | 7/7 | fMRIPrep's BBR call |
| `fmriprep/ds003592-sub-280-noreconall/bold_ses_1_task_rest_echo_1_wf` | pass | 7/7 | fMRIPrep's BBR call |
| `fmriprep/ds005040-sub-002-noreconall/bold_task_clip_run_1_wf` | pass | 7/7 | fMRIPrep's BBR call |
| `real/default` | pass | 6/6 | the default (12 dof, corratio) with -out and -omat |
| `real/default-full` | pass | 6/6 | the full-size T1w as reference |
| `real/dof6` | pass | 6/6 | -dof 6 |
| `real/dof7` | pass | 6/6 | -dof 7 |
| `real/dof9` | pass | 6/6 | -dof 9 |
| `real/cost-mutualinfo` | pass | 6/6 | -cost mutualinfo (and -searchcost corratio) |
| `real/cost-normmi` | pass | 6/6 | -cost normmi (and -searchcost corratio) |
| `real/cost-normcorr` | pass | 6/6 | -cost normcorr (and -searchcost corratio) |
| `real/cost-leastsq` | pass | 6/6 | -cost leastsq (and -searchcost corratio) |
| `real/cost-woods` | pass | 6/6 | -cost woods (and -searchcost corratio) |
| `real/cost-labeldiff` | pass | 6/6 | -cost labeldiff (and -searchcost corratio) |
| `real/searchcost-normmi` | pass | 6/6 | -searchcost normmi |
| `real/nosearch` | pass | 6/6 | -nosearch |
| `real/searchr` | pass | 6/6 | -searchrx/ry/rz with narrow ranges |
| `real/coarse-fine` | pass | 6/6 | -coarsesearch 30 -finesearch 10 |
| `real/init` | pass | 6/6 | -init (fMRIPrep's initial matrix) |
| `real/usesqform` | pass | 6/6 | -usesqform |
| `real/bins64` | pass | 6/6 | -bins 64 |
| `real/noresample` | pass | 6/6 | -noresample |
| `real/noclamp` | pass | 6/6 | -noclamp |
| `real/minsampling` | pass | 6/6 | -minsampling 4 |
| `real/forcescaling` | pass | 6/6 | -forcescaling |
| `real/weights` | pass | 6/6 | -refweight and -inweight |
| `real/weights-normcorr` | pass | 6/6 | weights with -cost normcorr |
| `real/weights-leastsq` | pass | 6/6 | weights with -cost leastsq |
| `real/weights-normmi` | pass | 6/6 | weights with -cost normmi |
| `real/weights-mutualinfo` | pass | 6/6 | weights with -cost mutualinfo |
| `real/weights-woods` | pass | 6/6 | weights with -cost woods |
| `real/labeldiff-dseg` | pass | 6/6 | -cost labeldiff on a label image |
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
| `real/applyxfm-usesqform` | pass | 6/6 | -applyxfm -usesqform |
| `real/out-only` | pass | 6/6 | a registration with -out and no -omat (the matrix printed) |
| `real/basescale-small` | pass | 6/6 | 0.6 mm voxels (the base scale from the voxel size) |
| `real/basescale-big` | pass | 6/6 | 14 mm voxels (the base scale from the voxel size) |
| `real/basescale-forced` | pass | 6/6 | -basescale 2 |
| `bbr/default` | pass | 6/6 | BBR (FSL's bbr.sch) with -cost bbr |
| `bbr/fmriprep-form` | pass | 6/6 | fMRIPrep's form: -searchcost bbr -basescale 1 |
| `bbr/type-global_abs` | pass | 6/6 | -bbrtype global_abs |
| `bbr/type-local_abs` | pass | 6/6 | -bbrtype local_abs |
| `bbr/slope` | pass | 6/6 | -bbrslope -0.25 |
| `bbr/coords` | pass | 6/6 | -wmcoords and -wmnorms |
| `bbr/fieldmap` | pass | 6/6 | -fieldmap -pedir 2 -echospacing (the BBR resampling for -out) |
| `bbr/fieldmap-mask` | pass | 6/6 | -fieldmapmask -pedir -2 |
| `bbr/weights` | pass | 6/6 | BBR with -refweight |
| `bbr/bad-type` | pass | 6/6 | -bbrtype unknown (FSL's exception) |
| `schedule/simple3D.sch` | pass | 6/6 | FSL's simple3D.sch |
| `schedule/xyztrans.sch` | pass | 6/6 | FSL's xyztrans.sch |
| `schedule/ytransonly.sch` | pass | 6/6 | FSL's ytransonly.sch |
| `schedule/ztransonly.sch` | pass | 6/6 | FSL's ztransonly.sch |
| `schedule/sch3Dtrans_3dof` | pass | 6/6 | FSL's sch3Dtrans_3dof |
| `schedule/measurecost1.sch` | pass | 6/6 | FSL's measurecost1.sch |
| `schedule/pairreg1.sch` | pass | 6/6 | FSL's pairreg1.sch |
| `schedule/pairreg2.sch` | pass | 6/6 | FSL's pairreg2.sch |
| `schedule/pairreg3.sch` | pass | 6/6 | FSL's pairreg3.sch |
| `schedule/sch2D_3dof` | pass | 6/6 | FSL's sch2D_3dof on one slice |
| `schedule/sch2D_4dof` | pass | 6/6 | FSL's sch2D_4dof on one slice |
| `schedule/sch2D_5dof` | pass | 6/6 | FSL's sch2D_5dof on one slice |
| `schedule/sch2D_6dof` | pass | 6/6 | FSL's sch2D_6dof on one slice |
| `real/2d` | pass | 6/6 | -2D's default schedule on one slice, with -out |
| `verbose/v` | pass | 6/6 | -v |
| `verbose/2` | pass | 6/6 | -verbose 2 |
| `verbose/3` | pass | 6/6 | -verbose 3 |
| `verbose/5` | pass | 6/6 | -verbose 5 |
| `verbose/printinit` | pass | 6/6 | -printinit |
| `verbose/debugsave` | pass | 6/6 | -debugsave -verbose 4 (the search's volumes saved) |
| `schedule/every-command` | both-error | 5/5 | a schedule using every command and option |
| `schedule/args-copy` | both-error | 5/5 | a schedule: copy U |
| `schedule/args-optimise` | both-error | 5/5 | a schedule: optimise 6 U |
| `schedule/bad-if` | both-error | 5/5 | a schedule: if 1 =< 2 |
| `schedule/exit` | pass | 6/6 | a schedule: setrow U 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 |
| `schedule/no-matrix` | pass | 6/6 | a schedule: clear U |
| `schedule/bad-costfunction` | both-error | 5/5 | a schedule: setoption costfunction nope |
| `schedule/bad-matname` | both-error | 5/5 | a schedule: clear Uq |
| `edge/no-args` | both-error | 5/5 | no arguments (the usage, exit 1) |
| `edge/help` | pass | 6/6 | -help |
| `edge/version` | pass | 6/6 | -version |
| `edge/no-in` | both-error | 5/5 | no -in |
| `edge/no-ref` | both-error | 5/5 | no -ref |
| `edge/not-option` | both-error | 5/5 | a word that is not an option |
| `edge/unknown-1` | both-error | 5/5 | an unknown option with one word after it |
| `edge/unknown-2` | both-error | 5/5 | an unknown option with two words after it |
| `edge/lacking` | both-error | 5/5 | an option lacking its argument |
| `edge/lacking-2` | both-error | 5/5 | a two-argument option lacking one |
| `edge/bad-cost` | both-error | 5/5 | -cost nope |
| `edge/bad-searchcost` | both-error | 5/5 | -searchcost nope |
| `edge/bad-interp` | both-error | 5/5 | -interp cubic |
| `edge/bad-datatype` | both-error | 5/5 | -datatype long |
| `edge/bad-sincwindow` | both-error | 5/5 | -sincwindow gauss |
| `edge/bad-anglerep` | both-error | 5/5 | -anglerep rodrigues |
| `edge/bad-pedir` | both-error | 5/5 | -pedir 0 |
| `edge/missing-in` | pass | 6/6 | an input that does not exist |
| `edge/missing-ref` | pass | 6/6 | a reference that does not exist |
| `edge/missing-init` | pass | 6/6 | an -init file that does not exist |
| `edge/missing-schedule` | both-error | 5/5 | a -schedule file that does not exist |
| `edge/bad-init` | pass | 6/6 | an -init file with too few numbers |
| `edge/coords-mismatch` | both-error | 5/5 | -wmcoords and -wmnorms of different lengths |
| `edge/outtype-unset` | both-error | 5/5 | FSLOUTPUTTYPE unset |
| `edge/outtype-NIFTI` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI |
| `edge/outtype-NIFTI_PAIR` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `edge/outtype-NIFTI2_GZ` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `edge/outtype-ANALYZE` | both-error | 5/5 | FSLOUTPUTTYPE=ANALYZE |
| `convert_xfm/no-args` | both-error | 5/5 | no arguments |
| `convert_xfm/help` | pass | 6/6 | -help |
| `convert_xfm/inverse` | pass | 6/6 | -omat -inverse |
| `convert_xfm/concat` | pass | 6/6 | -omat -concat |
| `convert_xfm/fixscaleskew` | pass | 6/6 | -fixscaleskew |
| `convert_xfm/verbose` | pass | 6/6 | -v with -concat and -inverse (the matrices printed) |
| `convert_xfm/verbose-1` | pass | 6/6 | -verbose 1 (the result printed) |
| `convert_xfm/copy` | pass | 6/6 | -omat only (the matrix written again) |
| `convert_xfm/missing` | both-error | 5/5 | an input matrix that does not exist |
| `convert_xfm/short` | both-error | 5/5 | a 3-row input matrix |
| `convert_xfm/no-input` | both-error | 5/5 | no input matrix |
| `convert_xfm/two-inputs` | both-error | 5/5 | two input matrices |
| `convert_xfm/unknown` | both-error | 5/5 | an unknown option |
| `convert_xfm/lacking` | both-error | 5/5 | -omat lacking its argument |
| `convert_xfm/singular` | both-error | 5/5 | -inverse of a singular matrix |
| `convert_xfm/missing-concat` | both-error | 5/5 | -concat with a file that does not exist |
