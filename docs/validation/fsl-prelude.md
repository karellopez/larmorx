# Parity: PRELUDE (clean-room original)

**All cases agree.** 166 cases on the `standard` tier: 138 pass, 28 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx fsl prelude` (larmorx 0.0.1, crate larmorx-fsl: the clean-room original written from specs/prelude.md, Apache-2.0), as a program (`python -m larmorx`, or `LARMORX_PRELUDE_BIN`)
- **Reference:** prelude from FSL 6.0.7.23 (fsl-fugue 2603.0, fsl-newimage 2601.0), the conda installation in `~/fsl-6.0.7.23`; the replica (`larmorx-nc fsl prelude`) by default as a black box
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-07 on Linux x86_64, with `python -m larmorx_validation parity fsl-prelude --tier standard`
- **Original programs:** 166 runs: 152 from the oracle cache, 14 run (4 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

- **The replica suite's cases and random testing's permanent cases (89), the original standing in for the replica's program:** 69 pass, 20 where both refuse, 0 expected, 0 failures (FSL's runs from the oracle cache under that suite's keys).
- **The original's own 77 cases:** 69 pass, 8 where both refuse, 0 expected, 0 failures, 0 skipped.
  - with `reproduce` against FSL: 282 of 282 headers and data bit-identical;
  - by default against the replica's default: 248 of 248 headers and data bit-identical, and 5 of 5 unwrapped phases where GLIBC-07 is named with the replica's integer offsets on at least 99.9 % of the mask's voxels.

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | FSL's (FSL's abort, signal 6: the original exits 1 after FSL's lines and its own) |
| Standard output and error | identical (the run directory and FSL's program replaced; larmorx's own lines left out) |
| Files written | the same names |
| `reproduce` against FSL | every file bit for bit, decompressed: headers whole (the extension bytes included) but `descrip`, data; values FSL takes from memory it never wrote not compared (FSL-263, FSL-265) |
| The default against the replica's | the same, and the same upstream defects named; where GLIBC-07 is named, the unwrapped phase with the replica's integer offsets on at least 99.9 % of the mask's voxels, the raw phase and the labels reported |
| D13 pass | a defect named where the default's status, output or files differ from `reproduce`'s |
| fMRIPrep's calls | both modes' unwrapped phase bit for bit with fMRIPrep's own output (header fields but `descrip`) |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| difftest | 3 | 3 | 0 | 0 | 0 |
| err | 22 | 4 | 18 | 0 | 0 |
| fmriprep | 7 | 7 | 0 | 0 | 0 |
| opt | 46 | 46 | 0 | 0 | 0 |
| orig-errors | 6 | 1 | 5 | 0 | 0 |
| orig-fmriprep | 7 | 7 | 0 | 0 | 0 |
| orig-images | 29 | 28 | 1 | 0 | 0 |
| orig-nan | 8 | 8 | 0 | 0 | 0 |
| orig-options | 19 | 19 | 0 | 0 | 0 |
| orig-types | 8 | 6 | 2 | 0 | 0 |
| out | 6 | 5 | 1 | 0 | 0 |
| panel | 5 | 4 | 1 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `out/invalid` | FSLOUTPUTTYPE=bogus | prelude exits 1; the same from the original. FTI, NIFTI_PAIR, NIFTI2, NIFTI2_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ, NIFTI2_GZ, NIFTI2_PAIR_GZ, NIFTI_ZST, NIFTI_PAIR_ZS… |
| `panel/ds000256/raw` | the data panel's ds000256 phase difference in the scanner's units (FSL's range error) | prelude exits 255; the same from the original. ERROR: input phase image exceeds allowable phase range. Allowable range is 6.283 radians. Image range is: 8188 r… |
| `err/help` | -h | prelude exits 1; the same from the original. the raw phase output -l,--labels filename for saving the area labels output --removeramps remove phase ramps durin… |
| `err/no-arguments` | no arguments | prelude exits 1; the same from the original. the raw phase output -l,--labels filename for saving the area labels output --removeramps remove phase ramps durin… |
| `err/no-output` | no -o | prelude exits 1; the same from the original. the raw phase output -l,--labels filename for saving the area labels output --removeramps remove phase ramps durin… |
| `err/complex-and-phase` | -c with -p | prelude exits 1; the same from the original. the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diagnostic messag… |
| `err/phase-without-abs` | -p without -a | prelude exits 1; the same from the original. saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diagnostic… |
| `err/abs-without-phase` | -a without -p | prelude exits 1; the same from the original. saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diagnostic… |
| `err/n1` | -n 1 | prelude exits 1; the same from the original. ing the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diagnostic me… |
| `err/unknown-option` | an unknown option | prelude exits 1; the same from the original. s filename for saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switc… |
| `err/bad-number` | -n abc | prelude exits 1; the same from the original. ame for saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on di… |
| `err/stray-word` | a word that is no option | prelude exits 1; the same from the original. filename for saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch … |
| `err/missing-file` | a phase image that does not exist | prelude exits -6; the same from the original. ead volume nothere Error : No image files match: nothere terminate called after throwing an instance of 'std::run… |
| `err/no-input` | neither -c nor -p and -a | prelude exits -6; the same from the original. e Exception : #22 :: Failed to read volume Error : No image files match: terminate called after throwing an insta… |
| `err/complex-empty-name` | --complex= (an empty name) | prelude exits 1; the same from the original. for saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diagno… |
| `err/range` | a phase range above 3pi (exit 255) | prelude exits 255; the same from the original. ERROR: input phase image exceeds allowable phase range. Allowable range is 6.283 radians. Image range is: 14.137… |
| `err/long-option-space` | --start 1 (a long option's value needs =) | prelude exits 1; the same from the original. e for saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diag… |
| `err/start-outside` | --start beyond the series | prelude exits -6; the same from the original. Image Exception : #61 :: Invalid t index in [] operator terminate called after throwing an instance of 'std::runt… |
| `err/start-negative` | --start=-1 | prelude exits -6; the same from the original. Image Exception : #61 :: Invalid t index in [] operator terminate called after throwing an instance of 'std::runt… |
| `err/end-outside` | --end beyond the series | prelude exits -6; the same from the original. Image Exception : #61 :: Invalid t index in [] operator terminate called after throwing an instance of 'std::runt… |
| `orig-images/series-short-mag` | `-p ph3 -a mag -o o -v --savemask=m -l lab` | both stop (exit -6): what(): Invalid t index in [] operator |
| `orig-errors/end-before-start-outside` | `-p ph3 -a mag3 -o o --start=4 --end=1` | both stop (exit -6): what(): Invalid t index in [] operator |
| `orig-errors/complex-as-phase` | `-p cplx -a mag -o o` | both stop (exit -6): what(): Fslread: DT 32 not supported |
| `orig-errors/complex-as-mask` | `-p ph -a mag -m cplx -o o` | both stop (exit -6): what(): Fslread: DT 32 not supported |
| `orig-errors/range` | `-p phrange -a mag -o o -v` | both stop (exit 255): Aborting. |
| `orig-errors/no-dir` | `-p ph -a mag -o nodir/o -v` | both stop (exit -6): what(): Error: cant open file nodir/o.nii.gz |
| `orig-types/unset` | `-p ph -a mag -o o -v --savemask=m` (FSLOUTPUTTYPE=None) | both stop (exit 1): e.g. tcsh or csh users add the line "source ${FSLDIR}/etc/fslconf/fsl.csh" |
| `orig-types/bogus` | `-p ph -a mag -o o -v --savemask=m` (FSLOUTPUTTYPE=bogus) | both stop (exit 1): Legal values are: NIFTI, NIFTI_PAIR, NIFTI2, NIFTI2_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ, NIFTI2_GZ, NIFTI2_PAIR_GZ, NIFTI_ZST, NIFTI_PAIR_ZST, NI… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (1ca117ed64eb) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The replica suite's cases are run with its own checks, as a black box: larmorx's original stands in for the replica's program, and FSL's runs are served from the oracle cache under that suite's keys. The original's own cases compare it with FSL in `reproduce` mode, and its default with the replica's default (the replica shares the corrected default and is bit-identical to FSL where no defect is named).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep/ds001600-sub-1-noreconall/wf_auto_00000` | pass | 8/8 | fMRIPrep's phase unwrapping call (sdcflows' phase-difference workflow) |
| `fmriprep/ds001600-sub-1-noreconall/wf_auto_00001` | pass | 8/8 | fMRIPrep's phase unwrapping call (sdcflows' phase-difference workflow) |
| `fmriprep/ds001600-sub-1-noreconall/wf_auto_00002` | pass | 8/8 | fMRIPrep's phase unwrapping call (sdcflows' phase-difference workflow) |
| `fmriprep/ds002422-sub-02-noreconall/wf_auto_00000` | pass | 8/8 | fMRIPrep's phase unwrapping call (sdcflows' phase-difference workflow) |
| `fmriprep/ds002422-sub-02-noreconall/wf_auto_00001` | pass | 8/8 | fMRIPrep's phase unwrapping call (sdcflows' phase-difference workflow) |
| `fmriprep/ds002603-sub-01-noreconall/wf_auto_00000` | pass | 8/8 | fMRIPrep's phase unwrapping call (sdcflows' phase-difference workflow) |
| `fmriprep/ds005454-sub-16-noreconall/wf_gre16` | pass | 8/8 | fMRIPrep's phase unwrapping call (sdcflows' phase-difference workflow) |
| `opt/default` | pass | 6/6 | phase and magnitude, the defaults (3D labels and unwrapping) |
| `opt/twopi-mask` | pass | 6/6 | a phase from 0 to 2pi (rewrapped) with a mask (fMRIPrep's form) |
| `opt/verbose` | pass | 6/6 | -v: every message |
| `opt/big-auto-2d-labels` | pass | 6/6 | an image wider than 64 voxels: labels slice by slice, 3D unwrapping |
| `opt/big-force3d` | pass | 6/6 | -f on a wide image: 3D labels |
| `opt/labelslices` | pass | 6/6 | --labelslices |
| `opt/slices` | pass | 6/6 | -s: masks, labels and unwrapping slice by slice |
| `opt/slices-force3d` | pass | 6/6 | -s -f: -f wins |
| `opt/n2` | pass | 6/6 | -n 2 |
| `opt/n3` | pass | 6/6 | -n 3 |
| `opt/n24` | pass | 6/6 | -n 24 |
| `opt/thresh` | pass | 6/6 | -t 300 |
| `opt/thresh-zero` | pass | 6/6 | -t 0: every voxel |
| `opt/mask` | pass | 6/6 | -m |
| `opt/mask-thresh` | pass | 6/6 | -m with -t 0.2 |
| `opt/series` | pass | 6/6 | three volumes |
| `opt/series-mask2` | pass | 6/6 | three volumes, a mask of two (the last mask for the third) |
| `opt/series-start-end` | pass | 6/6 | --start 1 --end 2 |
| `opt/series-start` | pass | 6/6 | --start 2 |
| `opt/series-end` | pass | 6/6 | --end 0 |
| `opt/series-saved` | pass | 6/6 | --savemask, -l and -r with three volumes (FSL-262) |
| `opt/saved` | pass | 6/6 | --savemask, -l and -r |
| `opt/saved-slices` | pass | 6/6 | --savemask, -l, -r with -s |
| `opt/removeramps` | pass | 6/6 | --removeramps (FSL-261) |
| `opt/removeramps-verbose` | pass | 6/6 | --removeramps -v (FSL-266: grot) |
| `opt/removeramps-slices` | pass | 6/6 | --removeramps -s |
| `opt/removeramps-one-slice` | pass | 6/6 | --removeramps on one slice (FSL-261) |
| `opt/one-slice` | pass | 6/6 | an image of one slice |
| `opt/neurological` | pass | 6/6 | a neurological image (swapped in memory) |
| `opt/int16-phase` | pass | 6/6 | an int16 phase with scl_slope |
| `opt/gap-slice` | pass | 6/6 | a mask with an empty slice between two parts, labels slice by slice (the largest component kept) |
| `opt/complex` | pass | 6/6 | -c: a complex64 image |
| `opt/complex-big-endian` | pass | 6/6 | -c: a big-endian complex64 image (FSL-264) |
| `opt/complex128` | pass | 6/6 | -c: a complex128 image (FSL-265) |
| `opt/complex-real` | pass | 6/6 | -c: a float32 image (FSL-263) |
| `opt/complex-int16` | pass | 6/6 | -c: an int16 image (FSL-263) |
| `opt/nan-magnitude` | pass | 6/6 | NaN magnitudes (FSL-268) |
| `opt/nan-first-magnitude` | pass | 6/6 | a NaN first magnitude voxel (FSL-268) |
| `opt/nan-phase` | pass | 6/6 | NaN phases inside the mask (FSL-269) |
| `opt/nan-first-twopi` | pass | 6/6 | a phase from 0 to 2pi whose first value is NaN: FSL does not rewrap it (FSL-272; the default rewraps it as without the … |
| `opt/nan-first-range` | pass | 6/6 | a phase of range 4pi whose first value is NaN: FSL does not refuse it (FSL-272; the default stops with FSL's range erro… |
| `opt/nan-phase-first` | pass | 6/6 | NaN phases at the mask's first voxels (FSL-269) |
| `opt/nan-phase-region` | pass | 6/6 | a NaN phase inside a region of the first phase bin (FSL-269) |
| `opt/nan-phase-slices` | pass | 6/6 | NaN phases with -s (FSL-269) |
| `out/nifti` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI |
| `out/nifti_pair` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI_PAIR |
| `out/nifti2_gz` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI2_GZ |
| `out/nifti_pair_gz` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI_PAIR_GZ |
| `out/nifti2` | pass | 6/6 | FSLOUTPUTTYPE=NIFTI2 |
| `out/invalid` | both-error | 5/5 | FSLOUTPUTTYPE=bogus |
| `opt/long-names` | pass | 6/6 | the long option names (--phase, --abs, --out, --numphasesplit, --thresh, --labels, --rawphase, --verbose, --slices, --f… |
| `opt/u-alias` | pass | 6/6 | -u (the output's other name) and grouped flags -fv |
| `panel/ds000256/rads` | pass | 6/6 | the data panel's ds000256 phase difference in radians (sdcflows' au2rads) and its first magnitude: prelude's own mask |
| `panel/ds000256/options` | pass | 6/6 | the data panel's ds000256 phase difference in radians: -s -v with the mask and labels |
| `panel/ds004958/rads` | pass | 6/6 | the data panel's ds004958 phase difference in radians (sdcflows' au2rads) and its first magnitude: prelude's own mask |
| `panel/ds004958/options` | pass | 6/6 | the data panel's ds004958 phase difference in radians: --removeramps -v with the raw phase |
| `panel/ds000256/raw` | both-error | 5/5 | the data panel's ds000256 phase difference in the scanner's units (FSL's range error) |
| `err/help` | both-error | 5/5 | -h |
| `err/no-arguments` | both-error | 5/5 | no arguments |
| `err/no-output` | both-error | 5/5 | no -o |
| `err/complex-and-phase` | both-error | 5/5 | -c with -p |
| `err/phase-without-abs` | both-error | 5/5 | -p without -a |
| `err/abs-without-phase` | both-error | 5/5 | -a without -p |
| `err/n1` | both-error | 5/5 | -n 1 |
| `err/unknown-option` | both-error | 5/5 | an unknown option |
| `err/bad-number` | both-error | 5/5 | -n abc |
| `err/stray-word` | both-error | 5/5 | a word that is no option |
| `err/missing-file` | both-error | 5/5 | a phase image that does not exist |
| `err/no-input` | both-error | 5/5 | neither -c nor -p and -a |
| `err/complex-empty-name` | both-error | 5/5 | --complex= (an empty name) |
| `err/range` | both-error | 5/5 | a phase range above 3pi (exit 255) |
| `err/long-option-space` | both-error | 5/5 | --start 1 (a long option's value needs =) |
| `err/start-outside` | both-error | 5/5 | --start beyond the series |
| `err/start-negative` | both-error | 5/5 | --start=-1 |
| `err/end-outside` | both-error | 5/5 | --end beyond the series |
| `err/empty-range` | pass | 6/6 | --end before --start (FSL-270) |
| `err/size-mismatch` | pass | 6/6 | a magnitude of another size (FSL-271) |
| `err/size-mismatch-larger` | pass | 6/6 | a magnitude larger than the phase (FSL-271) |
| `err/size-mismatch-slices` | pass | 6/6 | a mask of fewer slices, labels slice by slice (FSL-271) |
| `difftest/nan-phase-verbose-notice` | pass | 6/6 | A NaN phase in a one-voxel mask with -v: FSL-269's correction changes only -v's lines (the size of the starting matrice… |
| `difftest/ramps-notice-unchanged` | pass | 6/6 | --removeramps with mask voxels in the last planes where FSL's loops give the same result: FSL-261's notice now fires on… |
| `orig-options/basic` | pass | 13/13 | `-p ph -a mag -o o -v` |
| `orig-options/n2` | pass | 13/13 | `-p ph -a mag -o o -n 2 -v` |
| `orig-options/n3` | pass | 13/13 | `-p ph -a mag -o o -n 3` |
| `orig-options/n24` | pass | 13/13 | `-p ph -a mag -o o -n 24 -v` |
| `orig-options/t0` | pass | 13/13 | `-p ph -a mag -o o -t 0 -v` |
| `orig-options/t300` | pass | 17/17 | `-p ph -a mag -o o -t 300 --savemask=m` |
| `orig-options/mask` | pass | 17/17 | `-p ph -a mag -m mask -o o -v --savemask=m` |
| `orig-options/mask-t` | pass | 17/17 | `-p ph -a mag -m mask -t 0.2 -o o --savemask=m` |
| `orig-options/maskgap` | pass | 21/21 | `-p ph -a mag -m maskgap -o o -v --savemask=m -l lab` |
| `orig-options/slices` | pass | 25/25 | `-p ph -a mag -o o -s -v --savemask=m -l lab -r raw` |
| `orig-options/labelslices` | pass | 17/17 | `-p ph -a mag -o o --labelslices -v -l lab` |
| `orig-options/force3d-slices` | pass | 13/13 | `-p ph -a mag -o o -s -f -v` |
| `orig-options/ramps` | pass | 28/28 | `-p ph -a mag -o o --removeramps -v -r raw -l lab --savemask=m` |
| `orig-options/ramps-slices` | pass | 28/28 | `-p ph -a mag -o o -s --removeramps -v -r raw -l lab --savemask=m` |
| `orig-options/ramps-labelslices` | pass | 18/18 | `-p ph -a mag -o o --labelslices --removeramps -r raw` |
| `orig-options/ramps-noverbose` | pass | 18/18 | `-p ph -a mag -o o --removeramps -r raw` |
| `orig-options/grot` | pass | 16/16 | `-p ph -a mag -o o -v --removeramps` |
| `orig-options/long-names` | pass | 21/21 | `--phase=ph --abs=mag --unwrap=o --numphasesplit=5 --thresh=100 --rawphase=raw --labels=lab --verbose` |
| `orig-options/u-grouped` | pass | 13/13 | `-p ph -a mag -u o -fvn 4` |
| `orig-images/one-slice` | pass | 17/17 | `-p ph1 -a mag1 -o o -v -l lab` |
| `orig-images/one-slice-ramps` | pass | 20/20 | `-p ph1 -a mag1 -o o --removeramps -v -r raw` |
| `orig-images/one-slice-s` | pass | 13/13 | `-p ph1 -a mag1 -o o -s -v` |
| `orig-images/wide` | pass | 17/17 | `-p phwide -a magwide -o o -v -l lab` |
| `orig-images/wide-f` | pass | 17/17 | `-p phwide -a magwide -o o -f -v -l lab` |
| `orig-images/neuro` | pass | 25/25 | `-p phneuro -a magneuro -o o -v --savemask=m -l lab -r raw` |
| `orig-images/neuro-ramps` | pass | 18/18 | `-p phneuro -a magneuro -o o --removeramps -r raw` |
| `orig-images/int16-slope` | pass | 17/17 | `-p phint -a mag -o o -v -r raw` |
| `orig-images/rewrap` | pass | 17/17 | `-p ph2pi -a mag -o o -v -r raw` |
| `orig-images/series` | pass | 26/26 | `-p ph3 -a mag3 -o o -v --savemask=m -l lab -r raw` |
| `orig-images/series-start` | pass | 13/13 | `-p ph3 -a mag3 -o o --start=1 -v` |
| `orig-images/series-start-end` | pass | 17/17 | `-p ph3 -a mag3 -o o --start=1 --end=1 -r raw` |
| `orig-images/series-mask2` | pass | 18/18 | `-p ph3 -a mag3 -m mask2 -o o -v --savemask=m` |
| `orig-images/series-short-mag` | both-error | 15/16 | `-p ph3 -a mag -o o -v --savemask=m -l lab` |
| `orig-images/complex` | pass | 26/26 | `-c cplx -o o -v --savemask=m -l lab -r raw` |
| `orig-images/complex-be` | pass | 14/14 | `-c cplxbe -o o -v` |
| `orig-images/complex128` | pass | 11/11 | `-c cplx128 -o o -v` |
| `orig-images/complex-float32` | pass | 9/9 | `-c notcplx -o o -v` |
| `orig-images/complex-int16` | pass | 9/9 | `-c notcplx16 -o o` |
| `orig-images/size-big` | pass | 14/14 | `-p ph -a magbig -o o -v --savemask=m` |
| `orig-images/size-small` | pass | 14/14 | `-p ph -a magsmall -o o -v --savemask=m` |
| `orig-images/size-mask` | pass | 14/14 | `-p ph -a mag -m magsmall -o o --savemask=m` |
| `orig-images/time-step-zero` | pass | 26/26 | `-p ph3tr0 -a mag3 -o o -r raw -l lab --savemask=m` |
| `orig-images/time-step-negative` | pass | 18/18 | `-p ph3 -a mag3trneg -o o --savemask=m -m mag3trneg` |
| `orig-images/time-step-one-volume` | pass | 21/21 | `-p ph3tr0 -a mag3trneg -o o --start=1 --end=1 -r raw --savemask=m` |
| `orig-images/time-step-3d` | pass | 17/17 | `-p phtrneg -a mag -o o -r raw` |
| `orig-images/time-step-4d-one` | pass | 17/17 | `-p ph1vol -a mag1vol -o o --savemask=m` |
| `orig-images/time-step-complex` | pass | 22/22 | `-c cplx2trneg -o o -r raw --savemask=m` |
| `orig-images/time-step-complex-zero` | pass | 18/18 | `-c cplx2tr0 -o o --savemask=m` |
| `orig-nan/mag-first` | pass | 18/18 | `-p ph -a magnanfirst -o o -v --savemask=m` |
| `orig-nan/mag-inside` | pass | 17/17 | `-p ph -a magnaninside -o o -v --savemask=m` |
| `orig-nan/mag-first-s` | pass | 18/18 | `-p ph -a magnanfirst -o o -s -v --savemask=m` |
| `orig-nan/phase-first` | pass | 13/13 | `-p phnanfirst -a mag -o o -v` |
| `orig-nan/phase-first-2pi` | pass | 18/18 | `-p ph2pinanfirst -a mag -o o -v -r raw` |
| `orig-nan/phase-inside` | pass | 26/26 | `-p phnaninside -a mag -o o -v --savemask=m -l lab -r raw` |
| `orig-nan/phase-inside-s` | pass | 18/18 | `-p phnaninside -a mag -o o -s -v -l lab` |
| `orig-nan/phase-inside-ramps` | pass | 20/20 | `-p phnaninside -a mag -o o --removeramps -v -r raw` |
| `orig-errors/end-before-start` | pass | 12/12 | `-p ph3 -a mag3 -o o --start=2 --end=1 -r raw` |
| `orig-errors/end-before-start-outside` | both-error | 11/12 | `-p ph3 -a mag3 -o o --start=4 --end=1` |
| `orig-errors/complex-as-phase` | both-error | 11/12 | `-p cplx -a mag -o o` |
| `orig-errors/complex-as-mask` | both-error | 11/12 | `-p ph -a mag -m cplx -o o` |
| `orig-errors/range` | both-error | 9/9 | `-p phrange -a mag -o o -v` |
| `orig-errors/no-dir` | both-error | 11/12 | `-p ph -a mag -o nodir/o -v` |
| `orig-types/pair` | pass | 25/25 | `-p ph -a mag -o o -l lab --savemask=m -r raw` (FSLOUTPUTTYPE=NIFTI_PAIR) |
| `orig-types/pair-gz` | pass | 17/17 | `-p ph -a mag -o o -l lab` (FSLOUTPUTTYPE=NIFTI_PAIR_GZ) |
| `orig-types/pair-zst` | pass | 17/17 | `-p ph -a mag -o o --savemask=m` (FSLOUTPUTTYPE=NIFTI_PAIR_ZST) |
| `orig-types/nifti2-pair-bz2` | pass | 17/17 | `-p ph -a mag -o o -l lab` (FSLOUTPUTTYPE=NIFTI2_PAIR_BZ2) |
| `orig-types/nifti2-gz` | pass | 17/17 | `-p ph -a mag -o o --savemask=m` (FSLOUTPUTTYPE=NIFTI2_GZ) |
| `orig-types/nifti-zst` | pass | 17/17 | `-p ph -a mag -o o -r raw` (FSLOUTPUTTYPE=NIFTI_ZST) |
| `orig-types/unset` | both-error | 9/10 | `-p ph -a mag -o o -v --savemask=m` (FSLOUTPUTTYPE=None) |
| `orig-types/bogus` | both-error | 9/10 | `-p ph -a mag -o o -v --savemask=m` (FSLOUTPUTTYPE=bogus) |
| `orig-fmriprep/ds001600-sub-1-noreconall/wf_auto_00000` | pass | 15/15 | fMRIPrep 25.2.6's call on ds001600-sub-1-noreconall/wf_auto_00000: clipped.nii.gz clipped_mask.nii.gz sub-1_acq-v1_phas… |
| `orig-fmriprep/ds001600-sub-1-noreconall/wf_auto_00001` | pass | 15/15 | fMRIPrep 25.2.6's call on ds001600-sub-1-noreconall/wf_auto_00001: clipped.nii.gz clipped_mask.nii.gz sub-1_acq-v2_phas… |
| `orig-fmriprep/ds001600-sub-1-noreconall/wf_auto_00002` | pass | 15/15 | fMRIPrep 25.2.6's call on ds001600-sub-1-noreconall/wf_auto_00002: clipped.nii.gz clipped_mask.nii.gz sub-1_acq-v4_phas… |
| `orig-fmriprep/ds002422-sub-02-noreconall/wf_auto_00000` | pass | 15/15 | fMRIPrep 25.2.6's call on ds002422-sub-02-noreconall/wf_auto_00000: clipped.nii.gz clipped_mask.nii.gz sub-02_run-01_ph… |
| `orig-fmriprep/ds002422-sub-02-noreconall/wf_auto_00001` | pass | 15/15 | fMRIPrep 25.2.6's call on ds002422-sub-02-noreconall/wf_auto_00001: clipped.nii.gz clipped_mask.nii.gz sub-02_run-02_ph… |
| `orig-fmriprep/ds002603-sub-01-noreconall/wf_auto_00000` | pass | 15/15 | fMRIPrep 25.2.6's call on ds002603-sub-01-noreconall/wf_auto_00000: clipped.nii.gz clipped_mask.nii.gz sub-01_acq-Mm2_p… |
| `orig-fmriprep/ds005454-sub-16-noreconall/wf_gre16` | pass | 15/15 | fMRIPrep 25.2.6's call on ds005454-sub-16-noreconall/wf_gre16: clipped.nii.gz clipped_mask.nii.gz sub-16_phasediff_rads… |
| `difftest/time-step-zero-series` | pass | 6/6 | random testing: every file bit-identical |
