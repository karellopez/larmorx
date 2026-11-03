# Parity: FLIRT (clean-room original)

**8 case(s) fail.** 305 cases on the `standard` tier: 206 pass, 85 rejected by both, 6 expected divergences, 8 failures.

- **Validated:** `larmorx fsl flirt`, `convert_xfm` and FLIRT's other programs (larmorx 0.0.1, crate larmorx-fsl: the clean-room original written from specs/flirt.md on the shared registration engine of larmorx-mri, Apache-2.0)
- **Reference:** flirt, convert_xfm and FLIRT's other programs from FSL 6.0.7.23 (fsl-flirt 2111.4), the conda build at `~/fsl-6.0.7.23`
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-03 on Linux x86_64, with `python -m larmorx_validation parity fsl-flirt --tier standard`
- **Original programs:** 314 runs: 300 from the oracle cache, 14 run; cache `<workspace>/oracles/cache`, mode `use`

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
| bbr | 11 | 11 | 0 | 0 | 0 |
| convert_xfm | 16 | 7 | 9 | 0 | 0 |
| debug | 2 | 2 | 0 | 0 | 0 |
| edge | 35 | 9 | 19 | 6 | 1 |
| fmriprep | 3 | 3 | 0 | 0 | 0 |
| programs | 129 | 79 | 50 | 0 | 0 |
| real | 61 | 54 | 0 | 0 | 7 |
| schedule | 29 | 22 | 7 | 0 | 0 |
| verbose | 19 | 19 | 0 | 0 | 0 |

## Failures

### `real/cost-normmi`

-cost normmi (and -searchcost corratio)

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 1.5996, threshold 0.5 or FSL's own variability): maximum 1.9198 mm, RMS deviation 3.8287 mm; FSL's own p95 on a perturbed input 1.2445 mm

### `real/cost-leastsq`

-cost leastsq (and -searchcost corratio)

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 81.8069, threshold 0.5 or FSL's own variability): maximum 95.6303 mm, RMS deviation 93.6118 mm; FSL's own p95 on a perturbed input 46.7560 mm

### `real/weights-normcorr`

weights with -cost normcorr

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 0.5785, threshold 0.1 or FSL's own variability): maximum 0.7099 mm, RMS deviation 1.2748 mm; FSL's own p95 on a perturbed input 0.1879 mm

### `real/weights-normmi`

weights with -cost normmi

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 10.151, threshold 0.1 or FSL's own variability): maximum 12.1231 mm, RMS deviation 14.0621 mm; FSL's own p95 on a perturbed input 10.0584 mm

### `real/weights-woods`

weights with -cost woods

- **stderr: the same lines (numbers aside)** (identical = no, threshold the same text, numbers read as numbers): line 4004: FSL '<end>', original 'WARNING: Woods is not implemented with cost function weighting'

### `real/weights-empty-refweight`

an empty -refweight (every voxel 0): refused by default (FSL-91)

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 330.6537, threshold 0.5): maximum 371.6170 mm, RMS deviation 83.9047 mm; FSL's own p95 on a perturbed input 0.0001 mm

### `real/weights-empty-inweight`

an empty -inweight (every voxel 0): refused by default (FSL-91)

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 133.6991, threshold 0.5): maximum 172.9050 mm, RMS deviation 113.3139 mm; FSL's own p95 on a perturbed input 0.0000 mm

### `edge/one-voxel-input`

a 1 x 1 x 1 input registered to the T1w: one value, FLIRT's matrix an artefact of its search; refused by default (FSL-92)

- **out.mat: 95th-percentile displacement over the reference's brain** (p95 displacement (mm) = 131.933, threshold 0.5): maximum 159.0868 mm, RMS deviation 303.8605 mm; FSL's own p95 on a perturbed input 0.0000 mm

## Expected divergences

| Case | Reason |
|---|---|
| `edge/bbrstep-0` | FSL never finishes (FSL-71: undefined behaviour); the original refuses in both modes |
| `edge/read-not-numbers` | FSL never finishes (FSL-72: undefined behaviour); the original refuses in both modes |
| `edge/background-overflow` | FSL crashes (FSL-75: undefined behaviour); the original finishes naming it |
| `edge/background-empty` | FSL crashes (FSL-75: undefined behaviour); the original refuses in both modes |
| `edge/zero-voxel-size` | FSL never finishes (FSL-93: undefined behaviour); the original corrects it by default and refuses with reproduce |
| `edge/basescale-0` | FSL never finishes (FSL-93: undefined behaviour); the original refuses in both modes |

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
| `programs/applyxfm4D-usage` | applyxfm4D without arguments (its usage, exit 255) | applyxfm4D exits 255; the same from the original. Usage: ~/fsl-6.0.7.23/bin/applyxfm4D <input volume> <ref volume> <output volume> <transformation matrix file/… |
| `programs/applyxfm4D-bad-interp` | an unknown interpolation (its message, exit 1) | applyxfm4D exits 1; the same from the original. interpolation method: cubic is not a valid choice. Please choose from nearestneighbour, trilinear, sinc, or spl… |
| `programs/applyxfm4D-userprefix-last` | -userprefix as the last word (std::string of null) | applyxfm4D exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): basic_string: construction from null… |
| `programs/applyxfm4D-missing-matrix` | a directory without the matrices (FSL aborts) | applyxfm4D exits -6; the same from the original. Could not open matrix file none/MAT_00000 terminate called after throwing an instance of 'std::logic_error' wh… |
| `programs/applyxfm4D-missing-input` | an input that does not exist (FSL aborts) | applyxfm4D exits -6; the same from the original. Image Exception : #63 :: No image files match: none terminate called after throwing an instance of 'std::runti… |
| `programs/aff2rigid-usage` | aff2rigid without arguments (its usage, exit 1) | aff2rigid exits 1; the same from the original. |
| `programs/aff2rigid-default` | aff2rigid of an affine standard-space matrix (numpy's arithmetic) | aff2rigid exits 1; the same from the original. Traceback (most recent call last): File "~/fsl-6.0.7.23/bin/aff2rigid", line 105, in <module> v41=mat(zeros((4,1… |
| `programs/aff2rigid-fmriprep` | aff2rigid of fMRIPrep's initial matrix | aff2rigid exits 1; the same from the original. Traceback (most recent call last): File "~/fsl-6.0.7.23/bin/aff2rigid", line 105, in <module> v41=mat(zeros((4,1… |
| `programs/extracttxt-usage` | extracttxt with one argument (its usage, exit 255) | extracttxt exits 2; the same from the original. ~/fsl-6.0.7.23/bin/extracttxt: 68: exit: Illegal number: -1 |
| `programs/extracttxt-line0` | a relative start reaching line 0 (sed's error, exit 1) | extracttxt exits 1; the same from the original. sed: -e expression #1, char 4: invalid usage of line address 0 |
| `programs/extracttxt-dc-negative` | a start before line 1: dc reads its own negative result as a subtraction (stack empty), sed the range as an option | extracttxt exits 1; the same from the original. dc: stack empty sed: invalid option -- '2' Usage: sed [OPTION]... {script-only-if-no-other-script} [input-file]… |
| `programs/extracttxt-sed-option` | a start on line 0 with no trailing line: the range -1,-1 (sed's unknown option) | extracttxt exits 1; the same from the original. sed: invalid option -- '1' Usage: sed [OPTION]... {script-only-if-no-other-script} [input-file]... -n, --quiet,… |
| `programs/pairreg-usage` | pairreg with four arguments (its usage, exit 1) | pairreg exits 1; the same from the original. |
| `programs/midtrans-usage` | midtrans without transforms (its message and usage, exit 1) | midtrans exits 1; the same from the original. Must specify at least 2 transforms to find the mid-transform Part of FSL (ID: 2412.6-dirty) midtrans Copyright(c)… |
| `programs/midtrans-help` | midtrans -h (its usage, exit 1) | midtrans exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) midtrans Copyright(c) 2010, University of Oxford (Mark Jenkinson) Usage: midtrans … |
| `programs/midtrans-one` | one transform only (at least 2 needed, exit 1) | midtrans exits 1; the same from the original. Must specify at least 2 transforms to find the mid-transform Part of FSL (ID: 2412.6-dirty) midtrans Copyright(c)… |
| `programs/midtrans-singular` | a transform with a determinant below 1e-6 (its message, exit 1) | midtrans exits 1; the same from the original. Could not read matrix sing.mat |
| `programs/pointflirt-usage` | pointflirt without arguments (the compulsory options, exit 1) | pointflirt exits 1; the same from the original. *************************************************** The following COMPULSORY options have not been set: -i file… |
| `programs/pointflirt-vox-missing` | --vox without the volumes (its error, exit 2) | pointflirt exits 2; the same from the original. ERROR: need to specify --invol and --refvol to use --vox |
| `programs/pointflirt-count` | files with different numbers of points (its error, exit 255) | pointflirt exits 255; the same from the original. ERROR: Different number of coordinates in the two files |
| `programs/pointflirt-missing` | a coordinate file that does not exist (FSL aborts) | pointflirt exits 255; the same from the original. Could not open matrix file none.txt ERROR: Different number of coordinates in the two files |
| `programs/pointflirt-two` | two points (two rows of three: FSL reads past its rows, aborts) | pointflirt exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): copy into submatrix: incompatible ma… |
| `programs/pointflirt-four-columns` | points of four coordinates (FSL's block sizes clash, aborts) | pointflirt exits -6; the same from the original. terminate called after throwing an instance of 'std::logic_error' what(): copy into submatrix: incompatible ma… |
| `programs/avscale-usage` | avscale without arguments (its usage, exit 255) | avscale exits 255; the same from the original. Usage: ~/fsl-6.0.7.23/bin/avscale [--allparams/--inverteddies] matrixfile [non-reference-volume] |
| `programs/avscale-singular` | a singular matrix: Armadillo's solve throws, avscale aborts | avscale exits -6; the same from the original. matrix multiplication: problem with matrix inverse; suggest to use solve() instead terminate called after throwin… |
| `programs/avscale-missing-matrix` | a matrix file that does not exist (exit 254) | avscale exits 254; the same from the original. Could not open matrix file none.mat |
| `programs/avscale-missing-volume` | a volume that does not exist: newimage's exception, abort | avscale exits -6; the same from the original. Image Exception : #63 :: No image files match: none Image Exception : #22 :: Failed to read volume none.nii.gz Er… |
| `programs/img2imgcoord-usage` | img2imgcoord without arguments (its usage, exit 1) | img2imgcoord exits 1; the same from the original. |
| `programs/img2imgcoord-missing-coords` | img2imgcoord: a coordinate file that does not exist | img2imgcoord exits 255; the same from the original. Could not open matrix file none.txt |
| `programs/img2imgcoord-unknown` | img2imgcoord: an unknown option (exit 255) | img2imgcoord exits 255; the same from the original. Unrecognised option -frob |
| `programs/img2imgcoord-missing-image` | img2imgcoord: an image that does not exist (abort) | img2imgcoord exits -6; the same from the original. Image Exception : #63 :: No image files match: none Image Exception : #22 :: Failed to read volume none.nii … |
| `programs/std2imgcoord-usage` | std2imgcoord without arguments (its usage, exit 1) | std2imgcoord exits 1; the same from the original. |
| `programs/std2imgcoord-missing-coords` | std2imgcoord: a coordinate file that does not exist | std2imgcoord exits 255; the same from the original. Could not open matrix file none.txt |
| `programs/std2imgcoord-unknown` | std2imgcoord: an unknown option (exit 255) | std2imgcoord exits 255; the same from the original. Unrecognised option -frob |
| `programs/std2imgcoord-missing-image` | std2imgcoord: an image that does not exist (abort) | std2imgcoord exits -6; the same from the original. Image Exception : #63 :: No image files match: none Image Exception : #22 :: Failed to read volume none.nii … |
| `programs/img2stdcoord-usage` | img2stdcoord without arguments (its usage, exit 1) | img2stdcoord exits 1; the same from the original. |
| `programs/img2stdcoord-missing-coords` | img2stdcoord: a coordinate file that does not exist | img2stdcoord exits 255; the same from the original. Could not open matrix file none.txt |
| `programs/img2stdcoord-unknown` | img2stdcoord: an unknown option (exit 255) | img2stdcoord exits 255; the same from the original. Unrecognised option -frob |
| `programs/img2stdcoord-missing-image` | img2stdcoord: an image that does not exist (abort) | img2stdcoord exits -6; the same from the original. Image Exception : #63 :: No image files match: none Image Exception : #22 :: Failed to read volume none.nii … |
| `programs/std2imgcoord-singular` | std2imgcoord with a singular matrix: its inverse throws | std2imgcoord exits -6; the same from the original. terminate called after throwing an instance of 'std::runtime_error' what(): inv(): matrix is singular |
| `programs/img2stdcoord-warp-missing` | img2stdcoord -warp of a file that does not exist | img2stdcoord exits 1; the same from the original. Image Exception : #63 :: No image files match: none Image Exception : #22 :: Failed to read volume none.nii.g… |
| `programs/rmsdiff-usage` | rmsdiff without arguments (its usage, exit 255) | rmsdiff exits 255; the same from the original. Usage: ~/fsl-6.0.7.23/bin/rmsdiff matrixfile1 matrixfile2 refvol [mask] Outputs rms deviation between matrices (… |
| `programs/rmsdiff-singular` | a singular second matrix: rms_deviation's message, exit 251 | rmsdiff exits 251; the same from the original. WARNING:: matrix 2 has low determinant 1.000000 0.000000 0.000000 0.000000 0.000000 0.000000 0.000000 0.000000 0… |
| `programs/rmsdiff-missing-matrix` | a matrix file that does not exist | rmsdiff exits 254; the same from the original. Could not open matrix file none.mat Could not read matrix none.mat |
| `programs/makerot-usage` | makerot without arguments (the compulsory -t, its usage, exit 1) | makerot exits 1; the same from the original. *************************************************** The following COMPULSORY options have not been set: -t,--theta… |
| `programs/makerot-help` | makerot -h (its usage, exit 1) | makerot exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) makerot Copyright(c) 2003, University of Oxford (Mark Jenkinson) Usage: makerot [op… |
| `programs/makerot-cov-missing` | --cov of a file that is not an image (FSL aborts) | makerot exits -6; the same from the original. Image Exception : #63 :: No image files match: none Image Exception : #22 :: Failed to read volume none.nii Error… |
| `programs/makerot-zero-axis` | a zero axis (its error, exit 1) | makerot exits 1; the same from the original. ERROR:: Axis cannot be zero |
| `programs/makerot-bad-theta` | a theta that is not a number (Couldn't set_value, exit 1) | makerot exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) makerot Copyright(c) 2003, University of Oxford (Mark Jenkinson) Usage: makerot [op… |
| `programs/makerot-token` | a word that is not an option (unrecognised token, exit 1) | makerot exits 1; the same from the original. Part of FSL (ID: 2412.6-dirty) makerot Copyright(c) 2003, University of Oxford (Mark Jenkinson) Usage: makerot [op… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (f0dd88b671b6) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| scipy | 1.15.3 |

## Notes

The cases are the replica suite's (`fsl-flirt-replica`) for `flirt`, `convert_xfm` and FLIRT's other programs; FSL's runs are served from the oracle cache under that suite's keys. The original is compared in `reproduce` mode (D13) and its default `fix` pass against the reproduced run. The original's own cases (`verbose/` and `debug/` beyond the replica's: part 2) cover the verbose levels' and -debug's code paths; at a verbose level of 5 or more the streams are compared with each run of cost evaluation lines as one (the evaluations follow the optimisation's path). -debug's volumes before the registration (the pyramid, its weights, the input at 8 mm) are held to FSL's header and values within 1e-6 of their range, -debugsave's search grids to FSL's header and a correlation of 0.999.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds000228-sub-pixar008-noreconall/bold_task_pixar_wf` | pass | 8/8 | fMRIPrep's BBR call |
| `fmriprep/ds003592-sub-280-noreconall/bold_ses_1_task_rest_echo_1_wf` | pass | 8/8 | fMRIPrep's BBR call |
| `fmriprep/ds005040-sub-002-noreconall/bold_task_clip_run_1_wf` | pass | 8/8 | fMRIPrep's BBR call |
| `real/anglerep-quaternion` | pass | 5/5 | -anglerep quaternion: the rotations as a quaternion's last three components (12 dof) |
| `real/anglerep-quaternion-dof6` | pass | 6/6 | -anglerep quaternion at 6 dof, -out, verbose (the parameters printed) |
| `real/default` | pass | 7/7 | the default (12 dof, corratio) with -out and -omat |
| `real/default-full` | pass | 6/6 | the full-size T1w as reference |
| `real/dof6` | pass | 6/6 | -dof 6 |
| `real/dof7` | pass | 6/6 | -dof 7 |
| `real/dof9` | pass | 6/6 | -dof 9 |
| `real/cost-mutualinfo` | pass | 6/6 | -cost mutualinfo (and -searchcost corratio) |
| `real/cost-normmi` | fail | 5/6 | -cost normmi (and -searchcost corratio) |
| `real/cost-normcorr` | pass | 6/6 | -cost normcorr (and -searchcost corratio) |
| `real/cost-leastsq` | fail | 5/6 | -cost leastsq (and -searchcost corratio) |
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
| `real/forcescaling` | pass | 5/5 | -forcescaling |
| `real/weights` | pass | 6/6 | -refweight and -inweight |
| `real/weights-normcorr` | fail | 5/6 | weights with -cost normcorr |
| `real/weights-leastsq` | pass | 6/6 | weights with -cost leastsq |
| `real/weights-normmi` | fail | 5/6 | weights with -cost normmi |
| `real/weights-mutualinfo` | pass | 6/6 | weights with -cost mutualinfo |
| `real/weights-woods` | fail | 5/6 | weights with -cost woods |
| `real/weights-empty-refweight` | fail | 5/6 | an empty -refweight (every voxel 0): refused by default (FSL-91) |
| `real/weights-empty-inweight` | fail | 5/6 | an empty -inweight (every voxel 0): refused by default (FSL-91) |
| `real/applyxfm-nan-border` | pass | 6/6 | -applyxfm of a BOLD reference with NaN voxels in its border and an infinite one: the padding's background value (FSL-22… |
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
| `real/applyisoxfm-4d-blur-setbackground` | pass | 6/6 | -applyisoxfm 6 with -setbackground (the blur's outside a constant) |
| `real/applyxfm-usesqform` | pass | 6/6 | -applyxfm -usesqform |
| `real/out-only` | pass | 6/6 | a registration with -out and no -omat (the matrix printed) |
| `real/basescale-small` | pass | 6/6 | 0.6 mm voxels (the base scale from the voxel size) |
| `real/basescale-small-init` | pass | 6/6 | 0.6 mm voxels with -init (the initial matrix in the base scale's millimetres) |
| `real/basescale-big` | pass | 6/6 | 14 mm voxels (the base scale from the voxel size) |
| `real/basescale-forced` | pass | 6/6 | -basescale 2 |
| `bbr/default` | pass | 7/7 | BBR (FSL's bbr.sch) with -cost bbr |
| `bbr/fmriprep-form` | pass | 6/6 | fMRIPrep's form: -searchcost bbr -basescale 1 |
| `bbr/type-global_abs` | pass | 6/6 | -bbrtype global_abs |
| `bbr/type-local_abs` | pass | 6/6 | -bbrtype local_abs |
| `bbr/debug` | pass | 13/13 | -debug with -cost bbr (BBR's debug volume `debug`, part 3) |
| `bbr/slope` | pass | 6/6 | -bbrslope -0.25 |
| `bbr/coords` | pass | 6/6 | -wmcoords and -wmnorms |
| `bbr/fieldmap` | pass | 7/7 | -fieldmap -pedir 2 -echospacing (the BBR resampling for -out) |
| `bbr/fieldmap-mask` | pass | 6/6 | -fieldmapmask -pedir -2 |
| `bbr/weights` | pass | 6/6 | BBR with -refweight |
| `bbr/bad-type` | pass | 5/5 | -bbrtype unknown (FSL's exception) |
| `schedule/simple3D.sch` | pass | 6/6 | FSL's simple3D.sch |
| `schedule/xyztrans.sch` | pass | 5/5 | FSL's xyztrans.sch |
| `schedule/ytransonly.sch` | pass | 5/5 | FSL's ytransonly.sch |
| `schedule/ztransonly.sch` | pass | 5/5 | FSL's ztransonly.sch |
| `schedule/sch3Dtrans_3dof` | pass | 5/5 | FSL's sch3Dtrans_3dof |
| `schedule/measurecost1.sch` | pass | 5/5 | FSL's measurecost1.sch |
| `schedule/pairreg1.sch` | pass | 6/6 | FSL's pairreg1.sch |
| `schedule/pairreg2.sch` | pass | 6/6 | FSL's pairreg2.sch |
| `schedule/pairreg3.sch` | pass | 6/6 | FSL's pairreg3.sch |
| `schedule/sch2D_3dof` | pass | 5/5 | FSL's sch2D_3dof on one slice |
| `schedule/sch2D_4dof` | pass | 6/6 | FSL's sch2D_4dof on one slice |
| `schedule/sch2D_5dof` | pass | 5/5 | FSL's sch2D_5dof on one slice |
| `schedule/sch2D_6dof` | pass | 6/6 | FSL's sch2D_6dof on one slice |
| `real/2d` | pass | 6/6 | -2D's default schedule on one slice, with -out |
| `verbose/v` | pass | 6/6 | -v |
| `verbose/2` | pass | 6/6 | -verbose 2 |
| `verbose/3` | pass | 6/6 | -verbose 3 |
| `verbose/5` | pass | 6/6 | -verbose 5 |
| `verbose/printinit` | pass | 5/5 | -printinit |
| `verbose/debugsave` | pass | 6/6 | -debugsave -verbose 4 (the search's volumes saved) |
| `verbose/debug` | pass | 12/12 | -debug: the pyramid's volumes and weights saved (no weights: the one-voxel weight) |
| `verbose/debug-weights` | pass | 16/16 | -debug with -refweight and -inweight (the weights' levels saved) |
| `schedule/every-command` | both-error | 5/5 | a schedule using every command and option |
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
| `schedule/paramsubset-general` | pass | 5/5 | a parameter subset whose column has two ones in different rows of another (rz; tx+ty) |
| `schedule/paramsubset-weighted` | pass | 5/5 | a parameter subset with weights other than 0 and 1 sharing rows (the SVD's QR path) |
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
| `convert_xfm/fixscaleskew` | pass | 5/5 | -fixscaleskew |
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
| `edge/background-overflow` | expected-divergence | 2/2 | a 2 x 1 x 1 input and an 8 x 1 x 1 reference: FLIRT's pyramid computes the background of a 3-voxel-wide level, writing … |
| `edge/background-empty` | expected-divergence | 2/2 | -applyxfm of a 1-voxel input: its background value reads an empty buffer (FSL-75) |
| `edge/one-voxel-input` | fail | 5/6 | a 1 x 1 x 1 input registered to the T1w: one value, FLIRT's matrix an artefact of its search; refused by default (FSL-9… |
| `edge/zero-voxel-size` | expected-divergence | 2/2 | a header voxel size of 0 along z: FLIRT's base scale 0, every voxel size infinite, it never finishes; counted as 1 by d… |
| `edge/basescale-0` | expected-divergence | 2/2 | -basescale 0: every voxel size infinite, FLIRT never finishes (FSL-93) |
| `convert_xfm/missing-concat` | both-error | 4/4 | -concat with a file that does not exist |
| `programs/applyxfm4D-usage` | both-error | 4/4 | applyxfm4D without arguments (its usage, exit 255) |
| `programs/applyxfm4D-default` | pass | 7/7 | a 4D int16 series, a matrix per volume (MAT_0000n), sinc |
| `programs/applyxfm4D-single-trilinear` | pass | 6/6 | -singlematrix with trilinear interpolation, verbose |
| `programs/applyxfm4D-fourdigit-nn` | pass | 6/6 | --fourdigit (MAT_000n) with nearest neighbour (nn) |
| `programs/applyxfm4D-userprefix-spline` | pass | 7/7 | -userprefix with spline on an integer series (FSL-79) |
| `programs/applyxfm4D-float-sinc` | pass | 7/7 | a float input (one volume), -singlematrix, sinc |
| `programs/applyxfm4D-3d` | pass | 7/7 | -3D (the rectangular sinc of width 9) on a float volume |
| `programs/applyxfm4D-3d-spline-float` | pass | 7/7 | -3D with spline on a float volume |
| `programs/applyxfm4D-int8` | pass | 6/6 | an int8 input (FSL-09: written as uint8), trilinear |
| `programs/applyxfm4D-double-sinc` | pass | 7/7 | a double series (sinc sums in double precision; the background value kept in double precision where the mask sets it) |
| `programs/applyxfm4D-double-3d-spline` | pass | 7/7 | -3D spline on a double volume (its coefficients in double precision) |
| `programs/applyxfm4D-double-nn` | pass | 6/6 | a double volume, nearest neighbour (values through float) |
| `programs/applyxfm4D-3d-spline-int` | pass | 7/7 | -3D spline on an int16 volume (FSL-79: coefficients and values in the type) |
| `programs/applyxfm4D-nan-border` | pass | 6/6 | NaN voxels in the input's border and an infinite one: the background value's NaN order (FSL-22), spline coefficients th… |
| `programs/applyxfm4D-nan-border-3d` | pass | 7/7 | the same with -3D and sinc |
| `programs/applyxfm4D-metres` | pass | 6/6 | an input whose header gives metres (a nonsense header): read as FSL reads it |
| `programs/applyxfm4D-bad-interp` | both-error | 4/4 | an unknown interpolation (its message, exit 1) |
| `programs/applyxfm4D-userprefix-last` | both-error | 4/4 | -userprefix as the last word (std::string of null) |
| `programs/applyxfm4D-missing-matrix` | both-error | 4/4 | a directory without the matrices (FSL aborts) |
| `programs/applyxfm4D-missing-input` | both-error | 4/4 | an input that does not exist (FSL aborts) |
| `programs/aff2rigid-usage` | both-error | 4/4 | aff2rigid without arguments (its usage, exit 1) |
| `programs/aff2rigid-default` | both-error | 5/5 | aff2rigid of an affine standard-space matrix (numpy's arithmetic) |
| `programs/aff2rigid-fmriprep` | both-error | 5/5 | aff2rigid of fMRIPrep's initial matrix |
| `programs/extracttxt-usage` | both-error | 5/5 | extracttxt with one argument (its usage, exit 255) |
| `programs/extracttxt-rest` | pass | 6/6 | the lines from the first match to the end |
| `programs/extracttxt-count` | pass | 6/6 | three lines from the first match |
| `programs/extracttxt-regex-start` | pass | 6/6 | a regular expression, two lines, starting 2 lines after |
| `programs/extracttxt-before` | pass | 6/6 | a negative relative start (lines before the match) |
| `programs/extracttxt-zero-count` | pass | 6/6 | zero trailing lines (sed's range ends before it starts) |
| `programs/extracttxt-line0` | both-error | 5/5 | a relative start reaching line 0 (sed's error, exit 1) |
| `programs/extracttxt-dc-negative` | both-error | 5/5 | a start before line 1: dc reads its own negative result as a subtraction (stack empty), sed the range as an option |
| `programs/extracttxt-sed-option` | both-error | 5/5 | a start on line 0 with no trailing line: the range -1,-1 (sed's unknown option) |
| `programs/extracttxt-signed` | pass | 6/6 | signed arguments (+2, -0): dc reads each sign as a command |
| `programs/extracttxt-nomatch` | pass | 6/6 | a word that never matches (nothing printed, exit 0) |
| `programs/extracttxt-missing` | pass | 6/6 | a file that does not exist (grep's message, exit 0) |
| `programs/pairreg-usage` | both-error | 4/4 | pairreg with four arguments (its usage, exit 1) |
| `programs/pairreg-default` | pass | 6/6 | pairreg of the T1w and its moved copy (brains and skulls the same) |
| `programs/pairreg-metres` | pass | 6/6 | pairreg of images whose headers give metres (nonsense headers) |
| `programs/midtrans-usage` | both-error | 4/4 | midtrans without transforms (its message and usage, exit 1) |
| `programs/midtrans-help` | both-error | 4/4 | midtrans -h (its usage, exit 1) |
| `programs/midtrans-one` | both-error | 4/4 | one transform only (at least 2 needed, exit 1) |
| `programs/midtrans-default` | pass | 5/5 | three transforms, printed (the MNI origin assumed) |
| `programs/midtrans-out-verbose` | pass | 6/6 | -o with -v (every stage printed, the matrix written) |
| `programs/midtrans-template-separate` | pass | 5/5 | --template (its centre of gravity) and --separate |
| `programs/midtrans-template-50mm` | pass | 5/5 | --template with voxels of 50 mm (a nonsense header: its centre of gravity far out) |
| `programs/midtrans-debug` | pass | 5/5 | --debug (the origin at 0 and the debug matrices) |
| `programs/midtrans-singular` | both-error | 4/4 | a transform with a determinant below 1e-6 (its message, exit 1) |
| `programs/midtrans-token` | pass | 5/5 | an option after the transforms (read as a file: an empty matrix, the size error caught) |
| `programs/midtrans-not-square` | pass | 5/5 | a 3 x 4 matrix (the determinant's error, caught) |
| `programs/midtrans-3x3` | pass | 5/5 | a 3 x 3 matrix (the product's size error, caught) |
| `programs/pointflirt-usage` | both-error | 4/4 | pointflirt without arguments (the compulsory options, exit 1) |
| `programs/pointflirt-default` | pass | 5/5 | eight points, the matrix printed |
| `programs/pointflirt-out-verbose` | pass | 5/5 | -v and -o (the coordinates printed, the matrix written) |
| `programs/pointflirt-vox` | pass | 5/5 | --vox with --invol and --refvol (voxel coordinates scaled) |
| `programs/pointflirt-vox-odd` | pass | 5/5 | --vox with voxels of 0.01 mm and of 50 mm (nonsense headers) |
| `programs/pointflirt-vox-missing` | both-error | 4/4 | --vox without the volumes (its error, exit 2) |
| `programs/pointflirt-nullspace` | pass | 5/5 | four points (a nullspace: the warning, a non-unique solution) |
| `programs/pointflirt-count` | both-error | 4/4 | files with different numbers of points (its error, exit 255) |
| `programs/pointflirt-many` | pass | 5/5 | twenty points (x·xᵀ by BLAS dsyrk) |
| `programs/pointflirt-missing` | both-error | 4/4 | a coordinate file that does not exist (FSL aborts) |
| `programs/pointflirt-three` | pass | 5/5 | three points: a square file is read with points as columns |
| `programs/pointflirt-two` | both-error | 4/4 | two points (two rows of three: FSL reads past its rows, aborts) |
| `programs/pointflirt-four-columns` | both-error | 4/4 | points of four coordinates (FSL's block sizes clash, aborts) |
| `programs/avscale-usage` | both-error | 5/5 | avscale without arguments (its usage, exit 255) |
| `programs/avscale-default` | pass | 6/6 | avscale of fMRIPrep's initial matrix |
| `programs/avscale-allparams` | pass | 6/6 | --allparams, the translations about a volume's centre |
| `programs/avscale-inverteddies` | pass | 6/6 | --inverteddies (the check and the inverted eddy matrix) |
| `programs/avscale-flip` | pass | 6/6 | a matrix that swaps left and right (the half transforms of the swap) |
| `programs/avscale-singular` | both-error | 5/5 | a singular matrix: Armadillo's solve throws, avscale aborts |
| `programs/avscale-missing-matrix` | both-error | 5/5 | a matrix file that does not exist (exit 254) |
| `programs/avscale-empty-volume` | pass | 6/6 | an empty volume name: the centre of an empty volume |
| `programs/avscale-missing-volume` | both-error | 5/5 | a volume that does not exist: newimage's exception, abort |
| `programs/img2imgcoord-usage` | both-error | 5/5 | img2imgcoord without arguments (its usage, exit 1) |
| `programs/img2imgcoord-xfm` | pass | 6/6 | img2imgcoord through fMRIPrep's initial matrix |
| `programs/img2imgcoord-vox` | pass | 6/6 | img2imgcoord -vox |
| `programs/img2imgcoord-mm` | pass | 6/6 | img2imgcoord -mm |
| `programs/img2imgcoord-verbose` | pass | 6/6 | img2imgcoord verbose (sizes, matrices; fixed notation after them) |
| `programs/img2imgcoord-missing-coords` | both-error | 5/5 | img2imgcoord: a coordinate file that does not exist |
| `programs/img2imgcoord-not-numbers` | pass | 6/6 | img2imgcoord: coordinates that stop at a word |
| `programs/img2imgcoord-lacking` | pass | 6/6 | img2imgcoord: an option lacking its argument (a message, the run goes on) |
| `programs/img2imgcoord-unknown` | both-error | 5/5 | img2imgcoord: an unknown option (exit 255) |
| `programs/img2imgcoord-missing-image` | both-error | 5/5 | img2imgcoord: an image that does not exist (abort) |
| `programs/std2imgcoord-usage` | both-error | 5/5 | std2imgcoord without arguments (its usage, exit 1) |
| `programs/std2imgcoord-xfm` | pass | 6/6 | std2imgcoord through fMRIPrep's initial matrix |
| `programs/std2imgcoord-vox` | pass | 6/6 | std2imgcoord -vox |
| `programs/std2imgcoord-mm` | pass | 6/6 | std2imgcoord -mm |
| `programs/std2imgcoord-verbose` | pass | 6/6 | std2imgcoord verbose (sizes, matrices; fixed notation after them) |
| `programs/std2imgcoord-missing-coords` | both-error | 5/5 | std2imgcoord: a coordinate file that does not exist |
| `programs/std2imgcoord-not-numbers` | pass | 6/6 | std2imgcoord: coordinates that stop at a word |
| `programs/std2imgcoord-lacking` | pass | 6/6 | std2imgcoord: an option lacking its argument (a message, the run goes on) |
| `programs/std2imgcoord-unknown` | both-error | 5/5 | std2imgcoord: an unknown option (exit 255) |
| `programs/std2imgcoord-missing-image` | both-error | 5/5 | std2imgcoord: an image that does not exist (abort) |
| `programs/img2stdcoord-usage` | both-error | 5/5 | img2stdcoord without arguments (its usage, exit 1) |
| `programs/img2stdcoord-xfm` | pass | 6/6 | img2stdcoord through fMRIPrep's initial matrix |
| `programs/img2stdcoord-vox` | pass | 6/6 | img2stdcoord -vox |
| `programs/img2stdcoord-mm` | pass | 6/6 | img2stdcoord -mm |
| `programs/img2stdcoord-verbose` | pass | 6/6 | img2stdcoord verbose (sizes, matrices; fixed notation after them) |
| `programs/img2stdcoord-missing-coords` | both-error | 5/5 | img2stdcoord: a coordinate file that does not exist |
| `programs/img2stdcoord-not-numbers` | pass | 6/6 | img2stdcoord: coordinates that stop at a word |
| `programs/img2stdcoord-lacking` | pass | 6/6 | img2stdcoord: an option lacking its argument (a message, the run goes on) |
| `programs/img2stdcoord-unknown` | both-error | 5/5 | img2stdcoord: an unknown option (exit 255) |
| `programs/img2stdcoord-missing-image` | both-error | 5/5 | img2stdcoord: an image that does not exist (abort) |
| `programs/img2imgcoord-flirt-premat` | pass | 6/6 | img2imgcoord -flirt and -premat |
| `programs/std2imgcoord-standard` | pass | 6/6 | std2imgcoord from FSL's MNI152 2 mm |
| `programs/std2imgcoord-no-std` | pass | 6/6 | std2imgcoord without -std (its warning; 1 mm voxels) |
| `programs/std2imgcoord-singular` | both-error | 5/5 | std2imgcoord with a singular matrix: its inverse throws |
| `programs/img2stdcoord-noform` | pass | 6/6 | img2stdcoord to a standard image without sform or qform |
| `programs/img2stdcoord-warp-missing` | both-error | 5/5 | img2stdcoord -warp of a file that does not exist |
| `programs/rmsdiff-usage` | both-error | 5/5 | rmsdiff without arguments (its usage, exit 255) |
| `programs/rmsdiff-rms` | pass | 6/6 | the RMS deviation over 80 mm about a volume's centre |
| `programs/rmsdiff-mask` | pass | 6/6 | the largest and the RMS displacement over a mask |
| `programs/rmsdiff-low-determinant` | pass | 6/6 | a matrix of low determinant (its warning, the matrix on standard error) |
| `programs/rmsdiff-singular` | both-error | 5/5 | a singular second matrix: rms_deviation's message, exit 251 |
| `programs/rmsdiff-missing-matrix` | both-error | 5/5 | a matrix file that does not exist |
| `programs/rmsdiff-4d` | pass | 6/6 | a 4D reference (its first volume, the warning) |
| `programs/makerot-usage` | both-error | 5/5 | makerot without arguments (the compulsory -t, its usage, exit 1) |
| `programs/makerot-help` | both-error | 5/5 | makerot -h (its usage, exit 1) |
| `programs/makerot-default` | pass | 6/6 | 30 degrees about z through the origin |
| `programs/makerot-axis-centre` | pass | 6/6 | an axis that is not of unit length (FSL-76) through a centre |
| `programs/makerot-long` | pass | 6/6 | the long options with their values after = |
| `programs/makerot-cov` | pass | 6/6 | the centre of an image's volume (--cov) |
| `programs/makerot-cov-missing` | both-error | 5/5 | --cov of a file that is not an image (FSL aborts) |
| `programs/makerot-zero-axis` | both-error | 5/5 | a zero axis (its error, exit 1) |
| `programs/makerot-short-axis` | pass | 6/6 | an axis of one value (FSL-77: the buffer's zeros) |
| `programs/makerot-stale-axis` | pass | 6/6 | the axis given twice, the second of one value (FSL-77: the first's values) |
| `programs/makerot-bad-theta` | both-error | 5/5 | a theta that is not a number (Couldn't set_value, exit 1) |
| `programs/makerot-out` | pass | 6/6 | the matrix written to a file (-o) |
| `programs/makerot-out-unwritable` | pass | 6/6 | an output file that cannot be written (the message, exit 0) |
| `programs/makerot-token` | both-error | 5/5 | a word that is not an option (unrecognised token, exit 1) |
| `verbose/6-search-debugsave` | pass | 10/10 | -verbose 6 -debugsave on the search alone: its inner level 4 and the grids it saves |
| `verbose/7` | pass | 6/6 | -verbose 7 on the default schedule: the search's evaluations, starts and tolerances |
| `verbose/5-optimisers` | pass | 5/5 | -verbose 5: gridmeasurecost, Brent, Powell's correction, a parameter subset |
| `verbose/7-subset` | pass | 5/5 | -verbose 7: Brent's and a subset's starts, tolerances and doubled evaluations |
| `verbose/3-scales-options` | pass | 5/5 | -verbose 3: forced and input-only scales (the evaluation counts), every setoption echoed, a short tolerance list's inde… |
| `verbose/2-noclamp` | pass | 5/5 | -verbose 2 -noclamp: the images' ranges unclamped |
| `verbose/2-noresample` | pass | 5/5 | -verbose 2 -noresample: no subsampling lines |
| `verbose/2-applyxfm` | pass | 6/6 | -verbose 2 -applyxfm: the images read and the resampled volume |
| `verbose/2-out` | pass | 6/6 | -verbose 2 -out without -omat: the transformed input, then the final result |
| `verbose/3-2d` | pass | 5/5 | -2D -verbose 3: the tripled slice, no sform messages |
| `verbose/3-debug` | pass | 11/11 | -verbose 3 -debug: each setscale's debug lines before the input's |
| `debug/scales` | pass | 11/11 | -debug over forced and input-only scales: the pair's input at each setscale |
| `debug/bbr-setoption` | pass | 11/11 | -debug -wmseg: the boundary-based cost set up by setoption and by a new pair |
