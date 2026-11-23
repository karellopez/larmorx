# Validation: FreeSurfer's `mri_concat` (replica)

Status `experimental`. Generated 2026-11-23 on Linux x86_64 from commit `f1dd4d2c60d6` with `validation/probes/fs-concat/record.py` (queue item 3.2-39, track A relay 55: every option, message, exit status and failure case).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_concat/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_concat ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_concat) and its recon-all runs (oracles/freesurfer-8.2.0/runs/: pctsurfcon's and mri_mcadura_seg's calls).

## The recorded calls, on the recorded heads

`validation/probes/fs-concat/real.py`: `pctsurfcon`'s percent contrast per hemisphere (`<wm> <gm> --paired-diff-norm --mul 100`, its samples made again by the official `mri_vol2surf` with the run's own commands) on the six classic and six v8 runs, and `mri_mcadura_seg`'s `--sum --ctab mca-dura.ctab` of two segmentations (the head's `aseg.mgz` taken onto each hemisphere's crop and back by the official programs) on the v8 runs (ds000005-sub-01-classic, ds000005-sub-01-v8, ds000228-sub-pixar008-classic, ds000228-sub-pixar008-v8, ds002422-sub-02-classic, ds002422-sub-02-v8, ds004173-sub-603309-classic, ds004173-sub-603309-v8, ds005003-sub-RS001-classic, ds005003-sub-RS001-v8, ds005454-sub-16-classic, ds005454-sub-16-v8; by kind: mcadura-sum 6, pctsurfcon 24): 30 calls, `reproduce` 30 identical to the official program (exit status, both streams, every file byte for byte), the default 30; the official percent contrast equal to the run's recorded file in 24 of 24 (the remade inputs are the run's).

## The whole program against the official one

`validation/probes/fs-concat/check.py`: the command line (no argument, the version and help options, unknown words, every value-taking option without its value, the case rules, `--debug`), every refused combination, inputs of four voxel types, one to six frames, formats MGZ, MGH, NIfTI and gzipped NIfTI, segmentations with and without a colour table, zeros, NaN and infinite voxels, surface overlays, another grid, a cut file; `--no-check` with fitting and unfitting inputs; every reduction and option (`--combine`, `--prune`, the normalisations, the matrices `--mtx`, `--gmean`, `--asl`, the pairs, the means, median, sums, `--fnorm`, `--tar1`, `--std`, `--var`, `--max`, `--max-index` with `-prune` and `-add`, `--conjunct`, `--min`, `--sort`, `--vote`, `--first-non-zero`, `--mul`, `--div`, `--add`, `--cumsum`, `--scm`, `--pca`, `--rep`, `--rms`, `--keep-datatype`, `--ctab`, `--rusage`) with edge values, weights, masks of both sizes, `--in-frame`, `--zconcat` and `--f` lists; then seeded command lines. Each case in one folder; exit status, both streams and every file written, case by case; counted apart: FreeSurfer's crashes (CRASH: statuses 134, 136, 139, and FS-564's and FS-13's, where the port ends with an error in both modes) and results it takes from memory contents or leaves undefined (MEMORY: FS-53, FS-249, FS-1196, FS-1243, FS-1247):

- seed 1: 600 cases; `reproduce` 569 identical, 0 different, 31 counted (CRASH 10, MEMORY 21); `fix` 64 different with a notice, 0 without.
- seed 2: 1500 cases; `reproduce` 1435 identical, 0 different, 65 counted (CRASH 29, MEMORY 39); `fix` 201 different with a notice, 0 without.

`validation/probes/fs-concat/long_names.sh` (paths too long for macOS's limit, run on Linux by hand): `--f` list names of 2,000 to 2,008 bytes run normally, 2,009 bytes and more abort when the command line has been read (FS-1197); lists of more than 400,000 words crash (FS-1198).

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_concat_golden.rs`: 15 cases on volumes up to 32^3 (`pctsurfcon`'s and `mri_mcadura_seg`'s calls, a concatenation into NIfTI, the frame mean and standard deviation, weighted frames' maximum index, a matrix of frames, a masked vote, the first frame not zero, NaN made by `--mul +nan` and by NaN weights, an input with NaN and infinite voxels, the principal components with their matrix files, `--zconcat`, a list file's median), on every platform: `reproduce` `583298963d943406802b3991ffe4dd07565426f02b873d184939a8764e37e65c` (the official program's value on the same inputs, `check.py --golden`), `fix` `e4af9f7a836f154b3800f3f117565048c6c9ba6b9d774c4df05efae4f5628f99`.

## Memory

`tests/parity/test_memory_peak.py::test_mri_concat_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's v8 volumes read in place, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the output identical byte for byte):

- frames (reproduce): 277.7 MiB, FreeSurfer 280.2 MiB.
- frames (fix): 278.0 MiB, FreeSurfer 280.2 MiB.
- mean (reproduce): 277.9 MiB, FreeSurfer 279.5 MiB.
- mean (fix): 277.0 MiB, FreeSurfer 279.5 MiB.
- std (reproduce): 341.8 MiB, FreeSurfer 343.7 MiB.
- std (fix): 341.9 MiB, FreeSurfer 343.7 MiB.
- mul (reproduce): 86.0 MiB, FreeSurfer 86.5 MiB.
- mul (fix): 85.8 MiB, FreeSurfer 86.5 MiB.
- mcadura (reproduce): 198.3 MiB, FreeSurfer 202.8 MiB.
- mcadura (fix): 198.0 MiB, FreeSurfer 202.8 MiB.
- pctsurfcon (reproduce): 7.8 MiB, FreeSurfer 8.2 MiB.
- pctsurfcon (fix): 8.0 MiB, FreeSurfer 8.2 MiB.
