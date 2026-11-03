# Parity: fslstats (clean-room original)

**All cases agree.** 162 cases on the `standard` tier: 126 pass, 36 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx fsl fslstats` (larmorx 0.0.1, crate larmorx-fsl: the clean-room original written from specs/fslstats.md, Apache-2.0)
- **Reference:** fslstats from FSL 6.0.7.23 (fsl-avwutils 2209.6), the conda build at `~/fsl-6.0.7.23`, as built: glibc 2.35's maths
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-03 on Linux x86_64, with `python -m larmorx_validation parity fsl-stats --tier standard`
- **Original programs:** 162 runs: 157 from the oracle cache, 5 run; cache `<workspace>/oracles/cache`, mode `use`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (FSL's crash: the original exits 1) |
| Standard error | identical (the run directory replaced; the original's `larmorx` lines left out) |
| Standard output | identical text; the numbers of -M, -S, -m, -s without a mask, -D, -e, -E, -c and -C within 1e-6 relative (absolute below 1); -json: the text without its numbers identical, the numbers by the same rule (§10) |
| D13 fix pass | a defect named exactly when the default's output differs |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bet | 1 | 1 | 0 | 0 | 0 |
| cli | 32 | 4 | 28 | 0 | 0 |
| d13 | 36 | 29 | 7 | 0 | 0 |
| difftest | 4 | 4 | 0 | 0 | 0 |
| fmriprep | 4 | 4 | 0 | 0 | 0 |
| opt | 49 | 49 | 0 | 0 | 0 |
| order | 13 | 13 | 0 | 0 | 0 |
| orig | 9 | 8 | 1 | 0 | 0 |
| ts | 9 | 9 | 0 | 0 | 0 |
| types | 5 | 5 | 0 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `cli/usage` | no arguments: the usage (status 1) | fslstats exits 1; the same from the original. |
| `cli/usage-one` | only an input: the usage | fslstats exits 1; the same from the original. |
| `cli/usage-K` | -K and its mask only: the usage | fslstats exits 1; the same from the original. |
| `cli/usage-t` | -t and an input only: the usage | fslstats exits 1; the same from the original. |
| `cli/missing` | an input that does not exist (status 255) | fslstats exits 255; the same from the original. Image Exception : #63 :: No image files match: missing Image Exception : #22 :: Failed to read volume missing.n… |
| `cli/unknown` | an unknown option (status 3) | fslstats exits 3; the same from the original. Unrecognised option: -q |
| `cli/no-arg-l` | -l without its argument (status 2) | fslstats exits 2; the same from the original. Must specify an argument to -l |
| `cli/no-arg-u` | -u without its argument (status 2) | fslstats exits 2; the same from the original. Must specify an argument to -u |
| `cli/no-arg-k` | -k without its argument (status 2) | fslstats exits 2; the same from the original. Must specify an argument to -k |
| `cli/no-arg-d` | -d without its argument (status 2) | fslstats exits 2; the same from the original. Must specify an argument to -d |
| `cli/no-arg-p` | -p without its argument (status 2) | fslstats exits 2; the same from the original. Must specify an argument to -p |
| `cli/no-arg-P` | -P without its argument (status 2) | fslstats exits 2; the same from the original. Must specify an argument to -P |
| `cli/no-arg-h` | -h without its argument (status 2) | fslstats exits 2; the same from the original. Must specify the number of bins |
| `cli/no-arg-H` | -H without its argument (status 2) | fslstats exits 2; the same from the original. Must specify the number of bins |
| `cli/H-no-min` | -H without its minimum | fslstats exits 2; the same from the original. Must specify the histogram minimum intensity |
| `cli/H-no-max` | -H without its maximum | fslstats exits 2; the same from the original. Must specify the histogram maximum intensity |
| `cli/p-above` | -p 101 (status 1) | fslstats exits 1; the same from the original. Percentile must be between 0 and 100 |
| `cli/P-below` | -P -1 (status 1) | fslstats exits 1; the same from the original. Percentile must be between 0 and 100 |
| `cli/h-zero` | -h 0 (status 1) | fslstats exits 1; the same from the original. Must specify at least 1 bin |
| `cli/h-fraction` | -h 0.5: 0 bins after truncation | fslstats exits 1; the same from the original. Must specify at least 1 bin |
| `cli/h-text` | -h abc (atof: 0) | fslstats exits 1; the same from the original. Must specify at least 1 bin |
| `cli/K-4d` | -K with a 4D index mask (status 3) | fslstats exits 3; the same from the original. Index mask must be 3D |
| `cli/K-size` | -K with an index mask of another size (an exception) | fslstats exits 255; the same from the original. Image Exception : #3 :: Attempted to multiply images of different sizes Attempted to multiply images of differe… |
| `cli/k-size` | -k with a mask of another size (status 3) | fslstats exits 3; the same from the original. Mask and image must be the same size |
| `cli/k-tsize` | -k with a mask of more volumes (status 3) | fslstats exits 3; the same from the original. Mask and image must be the same size |
| `cli/k-missing` | -k naming a missing file | fslstats exits 255; the same from the original. Image Exception : #63 :: No image files match: nomask Image Exception : #22 :: Failed to read volume nomask.nii… |
| `cli/d-size` | -d with an image of another size (status 3) | fslstats exits 3; the same from the original. Image used with -d must be the same size as the base image |
| `cli/d-tsize` | -d with more volumes (status 3) | fslstats exits 3; the same from the original. Image must be the same size as the base image |
| `d13/t-k4` | FSL-183: -t with a 4D mask | fslstats exits 3; the same from the original. Mask and image must be the same size |
| `d13/t-d3` | FSL-183: -t with a 3D -d image | fslstats exits 255; the same from the original. Image Exception : #61 :: Invalid t index in [] operator Invalid t index in [] operator |
| `d13/t-d4` | FSL-183: -t with a 4D -d image | fslstats exits 3; the same from the original. Image must be the same size as the base image |
| `d13/x-empty` | FSL-184: -x of an empty mask | fslstats exits -11; the same from the original. calculateExtrema: mask is empty |
| `d13/X-empty` | FSL-184: -X of an empty mask | fslstats exits -11; the same from the original. calculateExtrema: mask is empty |
| `d13/e-const` | FSL-185: -e of a constant image | fslstats exits -8; the same from the original. |
| `d13/E-const` | FSL-185: -m -E -m of a constant image | fslstats exits -8; the same from the original. |
| `orig/e-empty-mask` | -l 100 -e -E: the histogram's message, then FSL-185 (FSL crashes) | fslstats exits -8; the same from the original. findHistogram: mask is empty |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (8ce1c6adf070) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The cases are the replica suite's (`fsl-stats-replica`) but its `fsltest/` ones (FSL's own tests, in FSL's source tree, which the clean-room implementer does not open); FSL's runs are served from the oracle cache under that suite's keys. The original is compared in `reproduce` mode (D13) and its default `fix` pass against the reproduced run. The goodvoxels calls run on FSL's own chain of fslmaths outputs (the ribbon a stand-in: the segmentation's grey matter, as the no-reconall oracle runs have no FreeSurfer ribbon).

Random testing (the trial of fslmaths' original, 2026-11-03, `<workspace>/oracles/difftest/2026-11-03-fsl-maths-original`: realistic cases only, FSL through the oracle cache): 150,000 fslstats option lists, 149,214 identical, 7 differing only by a named correction, 779 differing. Each of these re-run with the final build and triaged by category (`validation/investigations/cleanroom-maths/trial_triage.json`): 545 larmorx mistakes fixed (findings 28 of `docs/findings/fsl-maths-original.md`: the histograms without a histogram; 4 permanent cases in `validation/difftest/cases/fsl-stats-original/`); 74 corrections of the suite's checks (§10's bound over a histogram's lines and an output cut short); 160 explained (129: -c and -C of images holding NaN and infinities, the NaN's sign; 31: the last printed digit at §10's bound); none open.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_ribbon_mean` | pass | 4/4 | fMRIPrep's goodvoxels mask, node cov_ribbon_mean (--project-goodvoxels; the ribbon a stand-in: the segmentation's grey … |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_ribbon_std` | pass | 4/4 | fMRIPrep's goodvoxels mask, node cov_ribbon_std (--project-goodvoxels; the ribbon a stand-in: the segmentation's grey m… |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/mod_ribbon_mean` | pass | 4/4 | fMRIPrep's goodvoxels mask, node mod_ribbon_mean (--project-goodvoxels; the ribbon a stand-in: the segmentation's grey … |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/mod_ribbon_std` | pass | 4/4 | fMRIPrep's goodvoxels mask, node mod_ribbon_std (--project-goodvoxels; the ribbon a stand-in: the segmentation's grey m… |
| `bet/ds005040-sub-002-noreconall/T1w` | pass | 4/4 | BET's call: fslstats <T1w> -l 0.001 -C (fMRIPrep's preprocessed T1w) |
| `cli/usage` | both-error | 4/4 | no arguments: the usage (status 1) |
| `cli/usage-one` | both-error | 4/4 | only an input: the usage |
| `cli/usage-K` | both-error | 4/4 | -K and its mask only: the usage |
| `cli/usage-t` | both-error | 4/4 | -t and an input only: the usage |
| `cli/missing` | both-error | 4/4 | an input that does not exist (status 255) |
| `cli/unknown` | both-error | 4/4 | an unknown option (status 3) |
| `cli/no-arg-l` | both-error | 4/4 | -l without its argument (status 2) |
| `cli/no-arg-u` | both-error | 4/4 | -u without its argument (status 2) |
| `cli/no-arg-k` | both-error | 4/4 | -k without its argument (status 2) |
| `cli/no-arg-d` | both-error | 4/4 | -d without its argument (status 2) |
| `cli/no-arg-p` | both-error | 4/4 | -p without its argument (status 2) |
| `cli/no-arg-P` | both-error | 4/4 | -P without its argument (status 2) |
| `cli/no-arg-h` | both-error | 4/4 | -h without its argument (status 2) |
| `cli/no-arg-H` | both-error | 4/4 | -H without its argument (status 2) |
| `cli/H-no-min` | both-error | 4/4 | -H without its minimum |
| `cli/H-no-max` | both-error | 4/4 | -H without its maximum |
| `cli/p-above` | both-error | 4/4 | -p 101 (status 1) |
| `cli/P-below` | both-error | 4/4 | -P -1 (status 1) |
| `cli/h-zero` | both-error | 4/4 | -h 0 (status 1) |
| `cli/h-fraction` | both-error | 4/4 | -h 0.5: 0 bins after truncation |
| `cli/h-text` | both-error | 4/4 | -h abc (atof: 0) |
| `cli/K-4d` | both-error | 4/4 | -K with a 4D index mask (status 3) |
| `cli/K-size` | both-error | 4/4 | -K with an index mask of another size (an exception) |
| `cli/K-none` | pass | 4/4 | -K with no positive label: nothing printed |
| `cli/k-size` | both-error | 4/4 | -k with a mask of another size (status 3) |
| `cli/k-tsize` | both-error | 4/4 | -k with a mask of more volumes (status 3) |
| `cli/k-missing` | both-error | 4/4 | -k naming a missing file |
| `cli/d-size` | both-error | 4/4 | -d with an image of another size (status 3) |
| `cli/d-tsize` | both-error | 4/4 | -d with more volumes (status 3) |
| `cli/json-twice` | pass | 4/4 | -json -json |
| `cli/json-empty` | pass | 4/4 | -json with only an option that prints nothing |
| `cli/json-repeat` | pass | 4/4 | -json with an option twice (the last kept) |
| `opt/-r` | pass | 4/4 | -r on a 3D image |
| `opt/json-r` | pass | 4/4 | -json -r |
| `opt/-R` | pass | 4/4 | -R on a 3D image |
| `opt/json-R` | pass | 4/4 | -json -R |
| `opt/-e` | pass | 4/4 | -e on a 3D image |
| `opt/json-e` | pass | 4/4 | -json -e |
| `opt/-E` | pass | 4/4 | -E on a 3D image |
| `opt/json-E` | pass | 4/4 | -json -E |
| `opt/-v` | pass | 4/4 | -v on a 3D image |
| `opt/json-v` | pass | 4/4 | -json -v |
| `opt/-V` | pass | 4/4 | -V on a 3D image |
| `opt/json-V` | pass | 4/4 | -json -V |
| `opt/-m` | pass | 4/4 | -m on a 3D image |
| `opt/json-m` | pass | 4/4 | -json -m |
| `opt/-M` | pass | 4/4 | -M on a 3D image |
| `opt/json-M` | pass | 4/4 | -json -M |
| `opt/-s` | pass | 4/4 | -s on a 3D image |
| `opt/json-s` | pass | 4/4 | -json -s |
| `opt/-S` | pass | 4/4 | -S on a 3D image |
| `opt/json-S` | pass | 4/4 | -json -S |
| `opt/-w` | pass | 4/4 | -w on a 3D image |
| `opt/json-w` | pass | 4/4 | -json -w |
| `opt/-x` | pass | 4/4 | -x on a 3D image |
| `opt/json-x` | pass | 4/4 | -json -x |
| `opt/-X` | pass | 4/4 | -X on a 3D image |
| `opt/json-X` | pass | 4/4 | -json -X |
| `opt/-c` | pass | 4/4 | -c on a 3D image |
| `opt/json-c` | pass | 4/4 | -json -c |
| `opt/-C` | pass | 4/4 | -C on a 3D image |
| `opt/json-C` | pass | 4/4 | -json -C |
| `opt/-D` | pass | 4/4 | -D on a 3D image |
| `opt/json-D` | pass | 4/4 | -json -D |
| `opt/neuro-w` | pass | 4/4 | -w on a neurological image (read mirrored) |
| `opt/neuro-x` | pass | 4/4 | -x on a neurological image (read mirrored) |
| `opt/neuro-X` | pass | 4/4 | -X on a neurological image (read mirrored) |
| `opt/neuro-C` | pass | 4/4 | -C on a neurological image (read mirrored) |
| `opt/neuro-c` | pass | 4/4 | -c on a neurological image (read mirrored) |
| `opt/p` | pass | 4/4 | -p 50 |
| `opt/p0` | pass | 4/4 | -p 0 |
| `opt/p100` | pass | 4/4 | -p 100 |
| `opt/P` | pass | 4/4 | -P 30 |
| `opt/h` | pass | 4/4 | -h 7 |
| `opt/H` | pass | 4/4 | -H 7 0 5 (values outside the limits) |
| `opt/json-h` | pass | 4/4 | -json -h 4 |
| `opt/a` | pass | 4/4 | -a then -m -R |
| `opt/n` | pass | 4/4 | -n then -m -R (NaN and infinities) |
| `opt/inf` | pass | 4/4 | infinities and NaN without -n |
| `opt/4d` | pass | 4/4 | every output on a 4D image |
| `order/M-l-M` | pass | 4/4 | the usage's example: -M -l 4 -M |
| `order/thresholds` | pass | 4/4 | -l 2 -u 8 then the outputs |
| `order/mask` | pass | 4/4 | -k then the outputs |
| `order/mask-then-l` | pass | 4/4 | -k then -l (the threshold replaces the mask) |
| `order/l-then-mask` | pass | 4/4 | -l then -k (the mask replaces the threshold) |
| `order/u-only` | pass | 4/4 | -u only |
| `order/d` | pass | 4/4 | -d then -m, -a, -m |
| `order/k3-4d` | pass | 4/4 | a 3D mask over a 4D image |
| `order/k4-4d` | pass | 4/4 | a 4D mask |
| `order/k4s-4d` | pass | 4/4 | a 4D mask of fewer volumes (its last repeated) |
| `order/d3-4d` | pass | 4/4 | -d of a 3D image on a 4D one |
| `order/d4s-4d` | pass | 4/4 | -d of fewer volumes (the last repeated) |
| `order/empty-mask` | pass | 4/4 | thresholds that keep nothing |
| `ts/t` | pass | 4/4 | -t: a line per volume |
| `ts/t-k3` | pass | 4/4 | -t with a 3D mask |
| `ts/t-K` | pass | 4/4 | -t -K: a line per volume and label |
| `ts/K` | pass | 4/4 | -K: a line per label (label 3 missing) |
| `ts/K-4d` | pass | 4/4 | -K over a 4D image |
| `ts/K-u` | pass | 4/4 | -K then a threshold |
| `ts/json-t-K` | pass | 4/4 | -json -t -K |
| `ts/json-K` | pass | 4/4 | -json -K with a missing label |
| `ts/K-json` | pass | 4/4 | -K before -json |
| `types/u8` | pass | 4/4 | a u8 image (read as float) |
| `types/i16` | pass | 4/4 | a i16 image (read as float) |
| `types/i16s` | pass | 4/4 | a i16s image (read as float) |
| `types/f64` | pass | 4/4 | a f64 image (read as float) |
| `types/noform` | pass | 4/4 | an image without sform or qform: -c, -C, -x, -w |
| `opt/json-nan` | pass | 4/4 | -json of NaN results (null) |
| `d13/nan-R` | pass | 4/4 | FSL-181: a NaN first value, -R |
| `d13/nan-x` | pass | 4/4 | FSL-181: a NaN first value, -x |
| `d13/nan-X` | pass | 4/4 | FSL-181: a NaN first value, -X |
| `d13/nan-r` | pass | 4/4 | FSL-181: a NaN first value, -r |
| `d13/nan-e` | pass | 4/4 | FSL-181: a NaN first value, -e |
| `d13/nan-E` | pass | 4/4 | FSL-181: a NaN first value, -E |
| `d13/nan-m` | pass | 4/4 | FSL-181: a NaN first value, -m |
| `d13/nan-M` | pass | 4/4 | FSL-181: a NaN first value, -M |
| `d13/nan-p` | pass | 4/4 | FSL-181: -p 50 with NaN values |
| `d13/nan-P` | pass | 4/4 | FSL-181: -P 30 with NaN values |
| `d13/nan-h` | pass | 4/4 | FSL-181: -h 5 with NaN values |
| `d13/nan-H` | pass | 4/4 | FSL-181: -H 5 0 10 with NaN values |
| `d13/nan2-p` | pass | 4/4 | FSL-181: -p 50 sorting NaN values |
| `d13/nan2-R` | pass | 4/4 | FSL-181: NaN values after the first |
| `d13/nan-n` | pass | 4/4 | FSL-181: -n first (no defect) |
| `d13/nan-k` | pass | 4/4 | FSL-181: NaN under a mask |
| `d13/const01-s` | pass | 4/4 | FSL-182: -s of a constant image |
| `d13/one-s` | pass | 4/4 | FSL-182: -s of one voxel |
| `d13/S` | pass | 4/4 | FSL-182: -S without a mask |
| `d13/t-k4` | both-error | 4/4 | FSL-183: -t with a 4D mask |
| `d13/t-d3` | both-error | 4/4 | FSL-183: -t with a 3D -d image |
| `d13/t-d4` | both-error | 4/4 | FSL-183: -t with a 4D -d image |
| `d13/x-empty` | both-error | 4/4 | FSL-184: -x of an empty mask |
| `d13/X-empty` | both-error | 4/4 | FSL-184: -X of an empty mask |
| `d13/e-const` | both-error | 4/4 | FSL-185: -e of a constant image |
| `d13/E-const` | both-error | 4/4 | FSL-185: -m -E -m of a constant image |
| `d13/h-const` | pass | 4/4 | -h of a constant image (no range: nothing printed) |
| `d13/H-equal` | pass | 4/4 | FSL-186: -H 5 3 3 |
| `d13/H-reversed` | pass | 4/4 | FSL-186: -H 5 4 1 |
| `d13/H-far` | pass | 4/4 | FSL-187: -H 10 0 1 with a value of 1e12 |
| `d13/e-far` | pass | 4/4 | FSL-187: -e with a value of 1e12 |
| `d13/w-zeros4` | pass | 4/4 | FSL-188: -w of an all-zero 4D image |
| `d13/w-zeros3` | pass | 4/4 | -w of an all-zero 3D image |
| `d13/json-v-big` | pass | 4/4 | FSL-189: -json -v beyond 2^24 voxels |
| `d13/json-v-replaced` | pass | 4/4 | FSL-189 not met: the -v beyond 2^24 voxels replaced by a later -v |
| `d13/v-big` | pass | 4/4 | -v -V beyond 2^24 voxels (text) |
| `orig/P-makes-masks` | pass | 4/4 | -P makes the masks from the thresholds and multiplies the image by them (+inf becomes NaN for the later -D) |
| `orig/w-makes-masks` | pass | 4/4 | -w makes the masks likewise |
| `orig/e-makes-masks` | pass | 4/4 | -e makes the masks likewise |
| `orig/zeros-M` | pass | 4/4 | -M without nonzero values (0, no message) |
| `orig/zeros-sS` | pass | 4/4 | -m, -s, -S, -V without nonzero values |
| `orig/json-K-none` | pass | 4/4 | -json -K without a positive label: an empty list of indices |
| `orig/e-empty-mask` | both-error | 4/4 | -l 100 -e -E: the histogram's message, then FSL-185 (FSL crashes) |
| `orig/p-nan` | pass | 4/4 | -p nan -P nan: the smallest value |
| `orig/t-3d` | pass | 4/4 | -t on a 3D image: one pass |
| `difftest/histogram-empty-mask-and-range` | pass | 4/4 | -h with an empty mask and an empty range: no histogram (findings 28) |
| `difftest/histogram-empty-mask` | pass | 4/4 | -H with an empty mask: the message, then the histogram of zeros (findings 28) |
| `difftest/json-histogram-empty-mask` | pass | 4/4 | -json -H with an empty mask: the zeros (findings 28) |
| `difftest/json-no-histogram` | pass | 4/4 | -json -h of a constant image: an empty list (findings 28) |
