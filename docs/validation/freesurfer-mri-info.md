# Validation: FreeSurfer's `mri_info` (replica)

Status `experimental`. Generated 2026-11-22 on Linux x86_64 from commit `a23787a8af95` with `validation/probes/fs-info/record.py` (queue item 3.2-08).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_info/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_info ...`).
- **Reference:** FreeSurfer 8.2.0's recon-all runs (oracles/freesurfer-8.2.0/runs/: their logs) and its official program ($FREESURFER_HOME/bin/mri_info).

## The recorded calls

`mri_info` writes no file, so it is no step of the recordings: its calls are those recon-all and seg2cc make, on each of the 18 recorded runs (6 heads; the classic stream at four threads and at one, the v8 stream), run by `validation/probes/fs-info/replay.py` as recon-all runs them, against the official program now (exit status, both streams, the file written) and against the logs:

- `mri_info mri/rawavg.mgz` (recon-all, its output in `recon-all.log`): 18 calls; `reproduce` 18 identical to the official program, `fix` 18; 18 of 18 identical to the log.
- `mri_info mri/orig.mgz` (recon-all, its output in `recon-all.log`, also read for the field of view): 18 calls; `reproduce` 18 identical to the official program, `fix` 18; 18 of 18 identical to the log.
- `mri_info --nframes mri/orig/001.mgz` (recon-all's check of each input): 18 calls; `reproduce` 18 identical to the official program, `fix` 18 (not logged).
- `mri_info --o <tmp> --conformed-to-min mri/aseg.auto_noCCseg.mgz` (seg2cc, its answer in `seg2cc.log`): 18 calls; `reproduce` 18 identical to the official program, `fix` 18; 18 of 18 identical to the log.

## The whole program against the official one

`validation/probes/fs-info/check.py`: every option once on two volumes (with every header tag, and without), every format (MGZ, MGH, NIfTI, NIfTI compressed, Analyze, a COR folder), morphs and warp fields, names without an extension, missing files, folders, `--o` and `--otags`, a conformed volume, a voxel size of 0; then seeded volumes (1 to 5 voxels a side, 1 to 3 frames, four voxel types, every kind of axes) with zero to three options and one to three inputs. Exit status, both streams and the files written, case by case; counted apart: the cases where FreeSurfer crashes or its result depends on its build (`check.py`'s `SKIP`):

- seed 1: 600 cases; `reproduce` 587 identical, 0 different, 13 counted; `fix` 104 different with a notice, 0 without.
- seed 3: 1000 cases; `reproduce` 980 identical, 0 different, 20 counted; `fix` 196 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_info_golden.rs`: 32 cases on two 32^3 inputs made with integer arithmetic (a float volume with every header tag, a segmentation with a colour table: the report, `--dump`, the four whole-volume options, every matrix, `--otags` plain and compressed, `--ctab`, `--o`), on every platform, both modes: `e6f79174ec2f4ad0f925e0d1a700ce584e49ab27997ba83ca4f2f15ea0409d7c`, the official program's value on the same inputs (`check.py --golden`).

## Memory

`tests/parity/test_memory_peak.py::test_mri_info_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- report (reproduce): 5.0 MiB, FreeSurfer 6.2 MiB.
- report (fix): 5.0 MiB, FreeSurfer 6.2 MiB.
- stats (reproduce): 20.8 MiB, FreeSurfer 22.8 MiB.
- stats (fix): 21.0 MiB, FreeSurfer 22.8 MiB.
- voxvolsum (reproduce): 16.0 MiB, FreeSurfer 17.8 MiB.
- voxvolsum (fix): 15.8 MiB, FreeSurfer 17.8 MiB.
- entropy (reproduce): 21.2 MiB, FreeSurfer 22.8 MiB.
- entropy (fix): 21.5 MiB, FreeSurfer 22.8 MiB.
