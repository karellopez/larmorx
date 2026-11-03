# Parity: fslstats replica (bit-exact, FSL Licence)

**All cases agree.** 208 cases on the `standard` tier: 172 pass, 36 rejected by both, 0 expected divergences, 0 failures.

- **Validated:** `larmorx-nc fsl fslstats` (crate larmorx-nc-core's fslstats, translated from FSL 6.0.7.23's source)
- **Reference:** fslstats from FSL 6.0.7.23 (fsl-avwutils 2209.6, fsl-newimage 2601.0, fsl-armawrap 0.7.0, nlohmann/json 3.12.0; the conda build at `~/fsl-6.0.7.23`, as built: glibc 2.35's maths)
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-11-03 on Linux x86_64, with `python -m larmorx_validation parity fsl-stats-replica --tier standard`
- **Original programs:** 208 runs: 203 from the oracle cache, 5 run; cache `<workspace>/oracles/cache`, mode `use`

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | the same (a crash: the replica exits 1) |
| Standard output and error | identical (the run directory and FSL's program path replaced; the replica's `larmorx-nc:` lines left out) |
| FSL's own tests | the expected outputs of `feedsRun` and its JSON files |
| D13 fix pass | a defect named exactly when the default's output differs |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| bet | 1 | 1 | 0 | 0 | 0 |
| cli | 32 | 4 | 28 | 0 | 0 |
| d13 | 36 | 29 | 7 | 0 | 0 |
| difftest | 3 | 2 | 1 | 0 | 0 |
| fmriprep | 4 | 4 | 0 | 0 | 0 |
| fsltest | 56 | 56 | 0 | 0 | 0 |
| opt | 49 | 49 | 0 | 0 | 0 |
| order | 13 | 13 | 0 | 0 | 0 |
| ts | 9 | 9 | 0 | 0 | 0 |
| types | 5 | 5 | 0 | 0 | 0 |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `cli/usage` | no arguments: the usage (status 1) | fslstats exits 1; the same from the replica. |
| `cli/usage-one` | only an input: the usage | fslstats exits 1; the same from the replica. |
| `cli/usage-K` | -K and its mask only: the usage | fslstats exits 1; the same from the replica. |
| `cli/usage-t` | -t and an input only: the usage | fslstats exits 1; the same from the replica. |
| `cli/missing` | an input that does not exist (status 255) | fslstats exits 255; the same from the replica. Image Exception : #63 :: No image files match: missing Image Exception : #22 :: Failed to read volume missing.ni… |
| `cli/unknown` | an unknown option (status 3) | fslstats exits 3; the same from the replica. Unrecognised option: -q |
| `cli/no-arg-l` | -l without its argument (status 2) | fslstats exits 2; the same from the replica. Must specify an argument to -l |
| `cli/no-arg-u` | -u without its argument (status 2) | fslstats exits 2; the same from the replica. Must specify an argument to -u |
| `cli/no-arg-k` | -k without its argument (status 2) | fslstats exits 2; the same from the replica. Must specify an argument to -k |
| `cli/no-arg-d` | -d without its argument (status 2) | fslstats exits 2; the same from the replica. Must specify an argument to -d |
| `cli/no-arg-p` | -p without its argument (status 2) | fslstats exits 2; the same from the replica. Must specify an argument to -p |
| `cli/no-arg-P` | -P without its argument (status 2) | fslstats exits 2; the same from the replica. Must specify an argument to -P |
| `cli/no-arg-h` | -h without its argument (status 2) | fslstats exits 2; the same from the replica. Must specify the number of bins |
| `cli/no-arg-H` | -H without its argument (status 2) | fslstats exits 2; the same from the replica. Must specify the number of bins |
| `cli/H-no-min` | -H without its minimum | fslstats exits 2; the same from the replica. Must specify the histogram minimum intensity |
| `cli/H-no-max` | -H without its maximum | fslstats exits 2; the same from the replica. Must specify the histogram maximum intensity |
| `cli/p-above` | -p 101 (status 1) | fslstats exits 1; the same from the replica. Percentile must be between 0 and 100 |
| `cli/P-below` | -P -1 (status 1) | fslstats exits 1; the same from the replica. Percentile must be between 0 and 100 |
| `cli/h-zero` | -h 0 (status 1) | fslstats exits 1; the same from the replica. Must specify at least 1 bin |
| `cli/h-fraction` | -h 0.5: 0 bins after truncation | fslstats exits 1; the same from the replica. Must specify at least 1 bin |
| `cli/h-text` | -h abc (atof: 0) | fslstats exits 1; the same from the replica. Must specify at least 1 bin |
| `cli/K-4d` | -K with a 4D index mask (status 3) | fslstats exits 3; the same from the replica. Index mask must be 3D |
| `cli/K-size` | -K with an index mask of another size (an exception) | fslstats exits 255; the same from the replica. Image Exception : #3 :: Attempted to multiply images of different sizes Attempted to multiply images of differen… |
| `cli/k-size` | -k with a mask of another size (status 3) | fslstats exits 3; the same from the replica. Mask and image must be the same size |
| `cli/k-tsize` | -k with a mask of more volumes (status 3) | fslstats exits 3; the same from the replica. Mask and image must be the same size |
| `cli/k-missing` | -k naming a missing file | fslstats exits 255; the same from the replica. Image Exception : #63 :: No image files match: nomask Image Exception : #22 :: Failed to read volume nomask.nii.… |
| `cli/d-size` | -d with an image of another size (status 3) | fslstats exits 3; the same from the replica. Image used with -d must be the same size as the base image |
| `cli/d-tsize` | -d with more volumes (status 3) | fslstats exits 3; the same from the replica. Image must be the same size as the base image |
| `d13/t-k4` | FSL-183: -t with a 4D mask | fslstats exits 3; the same from the replica. Mask and image must be the same size |
| `d13/t-d3` | FSL-183: -t with a 3D -d image | fslstats exits 255; the same from the replica. Image Exception : #61 :: Invalid t index in [] operator Invalid t index in [] operator |
| `d13/t-d4` | FSL-183: -t with a 4D -d image | fslstats exits 3; the same from the replica. Image must be the same size as the base image |
| `d13/x-empty` | FSL-184: -x of an empty mask | fslstats exits -11; the same from the replica. calculateExtrema: mask is empty |
| `d13/X-empty` | FSL-184: -X of an empty mask | fslstats exits -11; the same from the replica. calculateExtrema: mask is empty |
| `d13/e-const` | FSL-185: -e of a constant image | fslstats exits -8; the same from the replica. |
| `d13/E-const` | FSL-185: -m -E -m of a constant image | fslstats exits -8; the same from the replica. |
| `difftest/json-notices-before-refusal` | random testing (2026-11-03): -json -E -k <mask> -X of a 1 x 1 x 1 x 2 image: -E's correction (FSL-185) was named only when the JSON was printed, and -X's refusal (FSL-184) ended the run first; the notices of the options before a refusal are now printed | fslstats exits -8; the same from the replica. findHistogram: mask is empty |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (6342d2742e0e) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |

## Notes

The replica runs as a separate program, the FSL replica package's (`larmorx-nc`), compared with FSL in `reproduce` mode (D13); the default `fix` pass is checked against the reproduced run. The goodvoxels calls run on FSL's own chain of fslmaths outputs (the ribbon a stand-in: the segmentation's grey matter, as the no-reconall oracle runs have no FreeSurfer ribbon).

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_ribbon_mean` | pass | 6/6 | fMRIPrep's goodvoxels mask, node cov_ribbon_mean (--project-goodvoxels; the ribbon a stand-in: the segmentation's grey … |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/cov_ribbon_std` | pass | 6/6 | fMRIPrep's goodvoxels mask, node cov_ribbon_std (--project-goodvoxels; the ribbon a stand-in: the segmentation's grey m… |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/mod_ribbon_mean` | pass | 6/6 | fMRIPrep's goodvoxels mask, node mod_ribbon_mean (--project-goodvoxels; the ribbon a stand-in: the segmentation's grey … |
| `fmriprep-goodvoxels/ds005040-sub-002-noreconall/mod_ribbon_std` | pass | 6/6 | fMRIPrep's goodvoxels mask, node mod_ribbon_std (--project-goodvoxels; the ribbon a stand-in: the segmentation's grey m… |
| `bet/ds005040-sub-002-noreconall/T1w` | pass | 5/5 | BET's call: fslstats <T1w> -l 0.001 -C (fMRIPrep's preprocessed T1w) |
| `cli/usage` | both-error | 6/6 | no arguments: the usage (status 1) |
| `cli/usage-one` | both-error | 6/6 | only an input: the usage |
| `cli/usage-K` | both-error | 6/6 | -K and its mask only: the usage |
| `cli/usage-t` | both-error | 6/6 | -t and an input only: the usage |
| `cli/missing` | both-error | 6/6 | an input that does not exist (status 255) |
| `cli/unknown` | both-error | 6/6 | an unknown option (status 3) |
| `cli/no-arg-l` | both-error | 6/6 | -l without its argument (status 2) |
| `cli/no-arg-u` | both-error | 6/6 | -u without its argument (status 2) |
| `cli/no-arg-k` | both-error | 6/6 | -k without its argument (status 2) |
| `cli/no-arg-d` | both-error | 6/6 | -d without its argument (status 2) |
| `cli/no-arg-p` | both-error | 6/6 | -p without its argument (status 2) |
| `cli/no-arg-P` | both-error | 6/6 | -P without its argument (status 2) |
| `cli/no-arg-h` | both-error | 6/6 | -h without its argument (status 2) |
| `cli/no-arg-H` | both-error | 6/6 | -H without its argument (status 2) |
| `cli/H-no-min` | both-error | 6/6 | -H without its minimum |
| `cli/H-no-max` | both-error | 6/6 | -H without its maximum |
| `cli/p-above` | both-error | 6/6 | -p 101 (status 1) |
| `cli/P-below` | both-error | 6/6 | -P -1 (status 1) |
| `cli/h-zero` | both-error | 6/6 | -h 0 (status 1) |
| `cli/h-fraction` | both-error | 6/6 | -h 0.5: 0 bins after truncation |
| `cli/h-text` | both-error | 6/6 | -h abc (atof: 0) |
| `cli/K-4d` | both-error | 6/6 | -K with a 4D index mask (status 3) |
| `cli/K-size` | both-error | 6/6 | -K with an index mask of another size (an exception) |
| `cli/K-none` | pass | 6/6 | -K with no positive label: nothing printed |
| `cli/k-size` | both-error | 6/6 | -k with a mask of another size (status 3) |
| `cli/k-tsize` | both-error | 6/6 | -k with a mask of more volumes (status 3) |
| `cli/k-missing` | both-error | 6/6 | -k naming a missing file |
| `cli/d-size` | both-error | 6/6 | -d with an image of another size (status 3) |
| `cli/d-tsize` | both-error | 6/6 | -d with more volumes (status 3) |
| `cli/json-twice` | pass | 6/6 | -json -json |
| `cli/json-empty` | pass | 6/6 | -json with only an option that prints nothing |
| `cli/json-repeat` | pass | 6/6 | -json with an option twice (the last kept) |
| `opt/-r` | pass | 6/6 | -r on a 3D image |
| `opt/json-r` | pass | 6/6 | -json -r |
| `opt/-R` | pass | 6/6 | -R on a 3D image |
| `opt/json-R` | pass | 6/6 | -json -R |
| `opt/-e` | pass | 6/6 | -e on a 3D image |
| `opt/json-e` | pass | 6/6 | -json -e |
| `opt/-E` | pass | 6/6 | -E on a 3D image |
| `opt/json-E` | pass | 6/6 | -json -E |
| `opt/-v` | pass | 6/6 | -v on a 3D image |
| `opt/json-v` | pass | 6/6 | -json -v |
| `opt/-V` | pass | 6/6 | -V on a 3D image |
| `opt/json-V` | pass | 6/6 | -json -V |
| `opt/-m` | pass | 6/6 | -m on a 3D image |
| `opt/json-m` | pass | 6/6 | -json -m |
| `opt/-M` | pass | 6/6 | -M on a 3D image |
| `opt/json-M` | pass | 6/6 | -json -M |
| `opt/-s` | pass | 6/6 | -s on a 3D image |
| `opt/json-s` | pass | 6/6 | -json -s |
| `opt/-S` | pass | 6/6 | -S on a 3D image |
| `opt/json-S` | pass | 6/6 | -json -S |
| `opt/-w` | pass | 6/6 | -w on a 3D image |
| `opt/json-w` | pass | 6/6 | -json -w |
| `opt/-x` | pass | 6/6 | -x on a 3D image |
| `opt/json-x` | pass | 6/6 | -json -x |
| `opt/-X` | pass | 6/6 | -X on a 3D image |
| `opt/json-X` | pass | 6/6 | -json -X |
| `opt/-c` | pass | 6/6 | -c on a 3D image |
| `opt/json-c` | pass | 6/6 | -json -c |
| `opt/-C` | pass | 6/6 | -C on a 3D image |
| `opt/json-C` | pass | 6/6 | -json -C |
| `opt/-D` | pass | 6/6 | -D on a 3D image |
| `opt/json-D` | pass | 6/6 | -json -D |
| `opt/neuro-w` | pass | 6/6 | -w on a neurological image (read mirrored) |
| `opt/neuro-x` | pass | 6/6 | -x on a neurological image (read mirrored) |
| `opt/neuro-X` | pass | 6/6 | -X on a neurological image (read mirrored) |
| `opt/neuro-C` | pass | 6/6 | -C on a neurological image (read mirrored) |
| `opt/neuro-c` | pass | 6/6 | -c on a neurological image (read mirrored) |
| `opt/p` | pass | 6/6 | -p 50 |
| `opt/p0` | pass | 6/6 | -p 0 |
| `opt/p100` | pass | 6/6 | -p 100 |
| `opt/P` | pass | 6/6 | -P 30 |
| `opt/h` | pass | 6/6 | -h 7 |
| `opt/H` | pass | 6/6 | -H 7 0 5 (values outside the limits) |
| `opt/json-h` | pass | 6/6 | -json -h 4 |
| `opt/a` | pass | 6/6 | -a then -m -R |
| `opt/n` | pass | 6/6 | -n then -m -R (NaN and infinities) |
| `opt/inf` | pass | 6/6 | infinities and NaN without -n |
| `opt/4d` | pass | 6/6 | every output on a 4D image |
| `order/M-l-M` | pass | 6/6 | the usage's example: -M -l 4 -M |
| `order/thresholds` | pass | 6/6 | -l 2 -u 8 then the outputs |
| `order/mask` | pass | 6/6 | -k then the outputs |
| `order/mask-then-l` | pass | 6/6 | -k then -l (the threshold replaces the mask) |
| `order/l-then-mask` | pass | 6/6 | -l then -k (the mask replaces the threshold) |
| `order/u-only` | pass | 6/6 | -u only |
| `order/d` | pass | 6/6 | -d then -m, -a, -m |
| `order/k3-4d` | pass | 6/6 | a 3D mask over a 4D image |
| `order/k4-4d` | pass | 6/6 | a 4D mask |
| `order/k4s-4d` | pass | 6/6 | a 4D mask of fewer volumes (its last repeated) |
| `order/d3-4d` | pass | 6/6 | -d of a 3D image on a 4D one |
| `order/d4s-4d` | pass | 6/6 | -d of fewer volumes (the last repeated) |
| `order/empty-mask` | pass | 6/6 | thresholds that keep nothing |
| `ts/t` | pass | 6/6 | -t: a line per volume |
| `ts/t-k3` | pass | 6/6 | -t with a 3D mask |
| `ts/t-K` | pass | 6/6 | -t -K: a line per volume and label |
| `ts/K` | pass | 6/6 | -K: a line per label (label 3 missing) |
| `ts/K-4d` | pass | 6/6 | -K over a 4D image |
| `ts/K-u` | pass | 6/6 | -K then a threshold |
| `ts/json-t-K` | pass | 6/6 | -json -t -K |
| `ts/json-K` | pass | 6/6 | -json -K with a missing label |
| `ts/K-json` | pass | 6/6 | -K before -json |
| `types/u8` | pass | 6/6 | a u8 image (read as float) |
| `types/i16` | pass | 6/6 | a i16 image (read as float) |
| `types/i16s` | pass | 6/6 | a i16s image (read as float) |
| `types/f64` | pass | 6/6 | a f64 image (read as float) |
| `types/noform` | pass | 6/6 | an image without sform or qform: -c, -C, -x, -w |
| `opt/json-nan` | pass | 6/6 | -json of NaN results (null) |
| `d13/nan-R` | pass | 6/6 | FSL-181: a NaN first value, -R |
| `d13/nan-x` | pass | 6/6 | FSL-181: a NaN first value, -x |
| `d13/nan-X` | pass | 6/6 | FSL-181: a NaN first value, -X |
| `d13/nan-r` | pass | 6/6 | FSL-181: a NaN first value, -r |
| `d13/nan-e` | pass | 6/6 | FSL-181: a NaN first value, -e |
| `d13/nan-E` | pass | 6/6 | FSL-181: a NaN first value, -E |
| `d13/nan-m` | pass | 6/6 | FSL-181: a NaN first value, -m |
| `d13/nan-M` | pass | 6/6 | FSL-181: a NaN first value, -M |
| `d13/nan-p` | pass | 6/6 | FSL-181: -p 50 with NaN values |
| `d13/nan-P` | pass | 6/6 | FSL-181: -P 30 with NaN values |
| `d13/nan-h` | pass | 6/6 | FSL-181: -h 5 with NaN values |
| `d13/nan-H` | pass | 6/6 | FSL-181: -H 5 0 10 with NaN values |
| `d13/nan2-p` | pass | 6/6 | FSL-181: -p 50 sorting NaN values |
| `d13/nan2-R` | pass | 6/6 | FSL-181: NaN values after the first |
| `d13/nan-n` | pass | 6/6 | FSL-181: -n first (no defect) |
| `d13/nan-k` | pass | 6/6 | FSL-181: NaN under a mask |
| `d13/const01-s` | pass | 6/6 | FSL-182: -s of a constant image |
| `d13/one-s` | pass | 6/6 | FSL-182: -s of one voxel |
| `d13/S` | pass | 6/6 | FSL-182: -S without a mask |
| `d13/t-k4` | both-error | 6/6 | FSL-183: -t with a 4D mask |
| `d13/t-d3` | both-error | 6/6 | FSL-183: -t with a 3D -d image |
| `d13/t-d4` | both-error | 6/6 | FSL-183: -t with a 4D -d image |
| `d13/x-empty` | both-error | 6/6 | FSL-184: -x of an empty mask |
| `d13/X-empty` | both-error | 6/6 | FSL-184: -X of an empty mask |
| `d13/e-const` | both-error | 6/6 | FSL-185: -e of a constant image |
| `d13/E-const` | both-error | 6/6 | FSL-185: -m -E -m of a constant image |
| `d13/h-const` | pass | 6/6 | -h of a constant image (no range: nothing printed) |
| `d13/H-equal` | pass | 6/6 | FSL-186: -H 5 3 3 |
| `d13/H-reversed` | pass | 6/6 | FSL-186: -H 5 4 1 |
| `d13/H-far` | pass | 6/6 | FSL-187: -H 10 0 1 with a value of 1e12 |
| `d13/e-far` | pass | 6/6 | FSL-187: -e with a value of 1e12 |
| `d13/w-zeros4` | pass | 6/6 | FSL-188: -w of an all-zero 4D image |
| `d13/w-zeros3` | pass | 6/6 | -w of an all-zero 3D image |
| `d13/json-v-big` | pass | 6/6 | FSL-189: -json -v beyond 2^24 voxels |
| `d13/json-v-replaced` | pass | 6/6 | FSL-189 not met: the -v beyond 2^24 voxels replaced by a later -v |
| `d13/v-big` | pass | 6/6 | -v -V beyond 2^24 voxels (text) |
| `fsltest/robust/-r` | pass | 7/7 | FSL's test robust: fslstats test.nii.gz -r |
| `fsltest/json/robust/-r` | pass | 7/7 | FSL's JSON test robust: fslstats -json test_4d.nii.gz -r |
| `fsltest/notRobust/-R` | pass | 7/7 | FSL's test notRobust: fslstats test.nii.gz -R |
| `fsltest/json/notRobust/-R` | pass | 7/7 | FSL's JSON test notRobust: fslstats -json test_4d.nii.gz -R |
| `fsltest/entropy/-e` | pass | 7/7 | FSL's test entropy: fslstats test.nii.gz -e |
| `fsltest/json/entropy/-e` | pass | 7/7 | FSL's JSON test entropy: fslstats -json test_4d.nii.gz -e |
| `fsltest/entropyNonzero/-E` | pass | 7/7 | FSL's test entropyNonzero: fslstats test.nii.gz -E |
| `fsltest/json/entropyNonzero/-E` | pass | 7/7 | FSL's JSON test entropyNonzero: fslstats -json test_4d.nii.gz -E |
| `fsltest/volume/-v` | pass | 7/7 | FSL's test volume: fslstats test.nii.gz -v |
| `fsltest/json/volume/-v` | pass | 7/7 | FSL's JSON test volume: fslstats -json test_4d.nii.gz -v |
| `fsltest/volumeNonzero/-V` | pass | 7/7 | FSL's test volumeNonzero: fslstats test.nii.gz -V |
| `fsltest/json/volumeNonzero/-V` | pass | 7/7 | FSL's JSON test volumeNonzero: fslstats -json test_4d.nii.gz -V |
| `fsltest/mean/-m` | pass | 7/7 | FSL's test mean: fslstats test.nii.gz -m |
| `fsltest/json/mean/-m` | pass | 7/7 | FSL's JSON test mean: fslstats -json test_4d.nii.gz -m |
| `fsltest/mean/-m_-a` | pass | 7/7 | FSL's test mean: fslstats test.nii.gz -m -a |
| `fsltest/json/mean/-m_-a` | pass | 7/7 | FSL's JSON test mean: fslstats -json test_4d.nii.gz -m -a |
| `fsltest/mean/-m_-a_-n` | pass | 7/7 | FSL's test mean: fslstats test.nii.gz -m -a -n |
| `fsltest/json/mean/-m_-a_-n` | pass | 7/7 | FSL's JSON test mean: fslstats -json test_4d.nii.gz -m -a -n |
| `fsltest/meanNonzero/-M` | pass | 7/7 | FSL's test meanNonzero: fslstats test.nii.gz -M |
| `fsltest/json/meanNonzero/-M` | pass | 7/7 | FSL's JSON test meanNonzero: fslstats -json test_4d.nii.gz -M |
| `fsltest/stdev/-s` | pass | 7/7 | FSL's test stdev: fslstats test.nii.gz -s |
| `fsltest/json/stdev/-s` | pass | 7/7 | FSL's JSON test stdev: fslstats -json test_4d.nii.gz -s |
| `fsltest/stdevNonzero/-S` | pass | 7/7 | FSL's test stdevNonzero: fslstats test.nii.gz -S |
| `fsltest/json/stdevNonzero/-S` | pass | 7/7 | FSL's JSON test stdevNonzero: fslstats -json test_4d.nii.gz -S |
| `fsltest/roi/-w` | pass | 7/7 | FSL's test roi: fslstats test.nii.gz -w |
| `fsltest/json/roi/-w` | pass | 7/7 | FSL's JSON test roi: fslstats -json test_4d.nii.gz -w |
| `fsltest/maxCoord/-x` | pass | 7/7 | FSL's test maxCoord: fslstats test.nii.gz -x |
| `fsltest/json/maxCoord/-x` | pass | 7/7 | FSL's JSON test maxCoord: fslstats -json test_4d.nii.gz -x |
| `fsltest/minCoord/-X` | pass | 7/7 | FSL's test minCoord: fslstats test.nii.gz -X |
| `fsltest/json/minCoord/-X` | pass | 7/7 | FSL's JSON test minCoord: fslstats -json test_4d.nii.gz -X |
| `fsltest/cogMM/-c` | pass | 7/7 | FSL's test cogMM: fslstats test.nii.gz -c |
| `fsltest/json/cogMM/-c` | pass | 7/7 | FSL's JSON test cogMM: fslstats -json test_4d.nii.gz -c |
| `fsltest/cogVox/-C` | pass | 7/7 | FSL's test cogVox: fslstats test.nii.gz -C |
| `fsltest/json/cogVox/-C` | pass | 7/7 | FSL's JSON test cogVox: fslstats -json test_4d.nii.gz -C |
| `fsltest/percentile/-p_50` | pass | 7/7 | FSL's test percentile: fslstats test.nii.gz -p 50 |
| `fsltest/json/percentile/-p_50` | pass | 7/7 | FSL's JSON test percentile: fslstats -json test_4d.nii.gz -p 50 |
| `fsltest/percentileNonzero/-P_50` | pass | 7/7 | FSL's test percentileNonzero: fslstats test.nii.gz -P 50 |
| `fsltest/json/percentileNonzero/-P_50` | pass | 7/7 | FSL's JSON test percentileNonzero: fslstats -json test_4d.nii.gz -P 50 |
| `fsltest/meanHistCog/-m_-h_10_-c` | pass | 7/7 | FSL's test meanHistCog: fslstats test.nii.gz -m -h 10 -c |
| `fsltest/json/meanHistCog/-m_-h_10_-c` | pass | 7/7 | FSL's JSON test meanHistCog: fslstats -json test_4d.nii.gz -m -h 10 -c |
| `fsltest/meanHist/-m_-h_10` | pass | 7/7 | FSL's test meanHist: fslstats test.nii.gz -m -h 10 |
| `fsltest/json/meanHist/-m_-h_10` | pass | 7/7 | FSL's JSON test meanHist: fslstats -json test_4d.nii.gz -m -h 10 |
| `fsltest/hist/-h_10` | pass | 7/7 | FSL's test hist: fslstats test.nii.gz -h 10 |
| `fsltest/json/hist/-h_10` | pass | 7/7 | FSL's JSON test hist: fslstats -json test_4d.nii.gz -h 10 |
| `fsltest/histLimits/-H_10_0_1` | pass | 7/7 | FSL's test histLimits: fslstats test.nii.gz -H 10 0 1 |
| `fsltest/json/histLimits/-H_10_0_1` | pass | 7/7 | FSL's JSON test histLimits: fslstats -json test_4d.nii.gz -H 10 0 1 |
| `fsltest/threshold/-l_0.25_-u_0.75_-m` | pass | 7/7 | FSL's test threshold: fslstats test.nii.gz -l 0.25 -u 0.75 -m |
| `fsltest/json/threshold/-l_0.25_-u_0.75_-m` | pass | 7/7 | FSL's JSON test threshold: fslstats -json test_4d.nii.gz -l 0.25 -u 0.75 -m |
| `fsltest/diff/-d_imageForDiff.nii.gz_-m` | pass | 7/7 | FSL's test diff: fslstats test.nii.gz -d imageForDiff.nii.gz -m |
| `fsltest/json/diff/-d_imageForDiff.nii.gz_-m` | pass | 7/7 | FSL's JSON test diff: fslstats -json test_4d.nii.gz -d imageForDiff.nii.gz -m |
| `fsltest/preopIndexMaskMean/-K_-m` | pass | 7/7 | FSL's test preopIndexMaskMean: fslstats -K mask.nii.gz test_4d.nii.gz -m |
| `fsltest/json/preopIndexMaskMean/-K_-m` | pass | 7/7 | FSL's JSON test preopIndexMaskMean |
| `fsltest/preopIndexMaskMeanTimeSeries/-t_-K_-m` | pass | 7/7 | FSL's test preopIndexMaskMeanTimeSeries: fslstats -t -K mask.nii.gz test_4d.nii.gz -m |
| `fsltest/json/preopIndexMaskMeanTimeSeries/-t_-K_-m` | pass | 7/7 | FSL's JSON test preopIndexMaskMeanTimeSeries |
| `fsltest/preopMeanTimeSeries/-t_-m` | pass | 7/7 | FSL's test preopMeanTimeSeries: fslstats -t test_4d.nii.gz -m |
| `fsltest/json/preopMeanTimeSeries/-t_-m` | pass | 7/7 | FSL's JSON test preopMeanTimeSeries |
| `difftest/cog-nan-sign-mm` | pass | 6/6 | random testing (2026-11-03): -E -c of an image holding NaN and infinities: the centre of gravity's NaN signs in millime… |
| `difftest/cog-nan-sign-voxels` | pass | 6/6 | random testing (2026-11-03): -l 0.001 -C (BET's form) of an image holding NaN and infinities: the centre of gravity's N… |
| `difftest/json-notices-before-refusal` | both-error | 6/6 | random testing (2026-11-03): -json -E -k <mask> -X of a 1 x 1 x 1 x 2 image: -E's correction (FSL-185) was named only w… |
