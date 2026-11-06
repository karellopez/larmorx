# Parity: PRELUDE replica (bit-exact, FSL Licence)

**All cases agree.** 86 cases on the `standard` tier: 66 pass, 20 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx-nc fsl prelude` (crate larmorx-nc-fugue, translated from FSL 6.0.7.23's source)
- **Reference:** prelude from FSL 6.0.7.23 (fsl-fugue 2603.0, fsl-newimage 2601.0, fsl-miscmaths 2412.6, fsl-newnifti 5.0.0, fsl-utils 2412.6; the conda build at `~/fsl-6.0.7.23`, as built)
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-06 on Linux x86_64, with `python -m larmorx_validation parity fsl-prelude-replica --tier standard`
- **Original programs:** 86 runs: 80 from the oracle cache, 6 run (1 never cached: the original varies); cache `<workspace>/oracles/cache`, mode `use`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same |
| Standard output and error | identical (the run directory and FSL's program path replaced; the replica's `larmorx-nc:` lines left out) |
| Files written | the same names |
| Every file | bit-identical (images decompressed; `descrip` left out) |
| D13 fix pass | a defect named exactly when the default's outputs differ |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| difftest | 2 | 2 | 0 | 0 | 0 |
| err | 22 | 4 | 18 | 0 | 0 |
| fmriprep | 7 | 7 | 0 | 0 | 0 |
| opt | 44 | 44 | 0 | 0 | 0 |
| out | 6 | 5 | 1 | 0 | 0 |
| panel | 5 | 4 | 1 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `out/invalid` | FSLOUTPUTTYPE=bogus | prelude exits 1; the same from the replica. FTI, NIFTI_PAIR, NIFTI2, NIFTI2_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ, NIFTI2_GZ, NIFTI2_PAIR_GZ, NIFTI_ZST, NIFTI_PAIR_ZST… |
| `panel/ds000256/raw` | the data panel's ds000256 phase difference in the scanner's units (FSL's range error) | prelude exits 255; the same from the replica. ERROR: input phase image exceeds allowable phase range. Allowable range is 6.283 radians. Image range is: 8188 ra… |
| `err/help` | -h | prelude exits 1; the same from the replica. the raw phase output -l,--labels filename for saving the area labels output --removeramps remove phase ramps during… |
| `err/no-arguments` | no arguments | prelude exits 1; the same from the replica. the raw phase output -l,--labels filename for saving the area labels output --removeramps remove phase ramps during… |
| `err/no-output` | no -o | prelude exits 1; the same from the replica. the raw phase output -l,--labels filename for saving the area labels output --removeramps remove phase ramps during… |
| `err/complex-and-phase` | -c with -p | prelude exits 1; the same from the replica. the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diagnostic message… |
| `err/phase-without-abs` | -p without -a | prelude exits 1; the same from the replica. saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diagnostic … |
| `err/abs-without-phase` | -a without -p | prelude exits 1; the same from the replica. saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diagnostic … |
| `err/n1` | -n 1 | prelude exits 1; the same from the replica. ing the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diagnostic mes… |
| `err/unknown-option` | an unknown option | prelude exits 1; the same from the replica. s filename for saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch… |
| `err/bad-number` | -n abc | prelude exits 1; the same from the replica. ame for saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on dia… |
| `err/stray-word` | a word that is no option | prelude exits 1; the same from the replica. filename for saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch o… |
| `err/missing-file` | a phase image that does not exist | prelude exits -6; the same from the replica. ead volume nothere Error : No image files match: nothere terminate called after throwing an instance of 'std::runt… |
| `err/no-input` | neither -c nor -p and -a | prelude exits -6; the same from the replica. e Exception : #22 :: Failed to read volume Error : No image files match: terminate called after throwing an instan… |
| `err/complex-empty-name` | --complex= (an empty name) | prelude exits 1; the same from the replica. for saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diagnos… |
| `err/range` | a phase range above 3pi (exit 255) | prelude exits 255; the same from the replica. ERROR: input phase image exceeds allowable phase range. Allowable range is 6.283 radians. Image range is: 14.1372… |
| `err/long-option-space` | --start 1 (a long option's value needs =) | prelude exits 1; the same from the replica. e for saving the area labels output --removeramps remove phase ramps during unwrapping -v,--verbose switch on diagn… |
| `err/start-outside` | --start beyond the series | prelude exits -6; the same from the replica. Image Exception : #61 :: Invalid t index in [] operator terminate called after throwing an instance of 'std::runti… |
| `err/start-negative` | --start=-1 | prelude exits -6; the same from the replica. Image Exception : #61 :: Invalid t index in [] operator terminate called after throwing an instance of 'std::runti… |
| `err/end-outside` | --end beyond the series | prelude exits -6; the same from the replica. Image Exception : #61 :: Invalid t index in [] operator terminate called after throwing an instance of 'std::runti… |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (1770116246bd) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The replica runs as a separate program, the FSL replica package's (`larmorx-nc`), compared with FSL in `reproduce` mode (D13); the default `fix` pass is checked against the reproduced run. fMRIPrep's own outputs come from its container's FSL and are reported only.

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
