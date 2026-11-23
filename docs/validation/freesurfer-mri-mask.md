# Validation: FreeSurfer's `mri_mask` (replica)

Status `experimental`. Generated 2026-11-23 on Linux x86_64 from commit `6370322a2642` with `validation/probes/fs-mask/record.py` (queue item 3.2-26; the whole program).

- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_mask/), the larmorx-freesurfer program (`larmorx-freesurfer [--upstream-defects reproduce|fix] freesurfer mri_mask ...`).
- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_mask) and its recon-all runs' recorded steps (oracles/freesurfer-8.2.0/runs/).

## The recorded steps

recon-all's `mri_mask -T 5 brain.mgz brainmask.mgz brain.finalsurfs.mgz` (both streams) and the v8 stream's two `-oval 1 -invert` calls (its MCA-dura and venous-sinus masks) on the reference runs, replayed by `python -m larmorx_validation freesurfer-replay run <run> mri_mask mri_mask:2 mri_mask:3` with the port (every file compared, FreeSurfer's provenance aside):

- `reproduce`: 24 steps of 12 runs, **24 reproduced**, 0 not; files: 24 provenance; 27 s at [1] thread(s) (recorded 22 s).
- `fix`: 24 steps of 12 runs, **24 reproduced**, 0 not; files: 24 provenance; 31 s at [1] thread(s) (recorded 22 s).

## Calls on the recorded heads beyond the steps

`validation/probes/fs-mask/real.py`: in the classic runs `-xform` with the run's GCA morph (`talairach.m3z`: the run's `brainmask.mgz` onto its `norm.mgz` moved into the atlas by the official `mri_convert -at`; with and without `-T 5 -dilate 1`); in the v8 runs the two calls of recon-all's scripts whose inputs the runs keep (`T1.mgz` masked by `synthstrip.mgz`; `-bb 3` on `orig.mgz` into a NIfTI file); the official program against the port, every file compared: 12 calls on 6 runs, `reproduce` **12 identical**, 0 different; `fix` 12 identical, 0 different with a notice, 0 without.

## The whole program against the official binary

`validation/probes/fs-mask/prog.py` (the open probe's command lines, `validation/investigations/freesurfer-mri-mask-probe/`): every option, given last, its value forms; the inputs' types, NaNs, masks of every kind, geometries that differ, segmentations with and without a colour table, every way of `-BBEQ`, the crops, the transforms of every kind and their failures, the singular transforms of FS-1135 with each later option, the outputs' formats; exit status, standard output and error, and every file written compared:

- seed 1: 195 cases; `reproduce` 194 identical, 0 different, 1 counted (a transform file FreeSurfer crashes on where both modes end with an error: FS-307, FS-309, FS-310); `fix` 113 identical, 81 different with a notice, 0 without.
- seed 2: 195 cases; `reproduce` 194 identical, 0 different, 1 counted (a transform file FreeSurfer crashes on where both modes end with an error: FS-307, FS-309, FS-310); `fix` 113 identical, 81 different with a notice, 0 without.

## Golden cases

`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_mask_golden.rs`: fourteen cases on 32^3 inputs made with integer arithmetic and a 16 x 16 x 32 pair (recon-all's `-T 5`, `-invert`, a transform with a threshold and a dilation, `-transfer` with the deletion edits kept, `-BB`, `-BBEQ`, a crop to a field of view, a mask with NaNs and infinities with `-oval nan`, a NaN threshold, `-lh -no_cerebellum`, `-samseg`, an erosion with background noise, `-BBEQ` without a box, recon-all's spelling `-bb 0`), on every platform: `reproduce` `e3ec07a6ffc9e79e099e459680a210d9f7e96905b4250beb97f4b0d9116c3444`, the official program's value on the same inputs (`prog.py --golden`); the default `20be6598c53f16ef1c90f6d63f502590d7c7cad20fa3d2d7814e249d0624542b` (FS-01, FS-1133, FS-269).

## Memory

`tests/parity/test_memory_peak.py::test_mri_mask_replica_needs_no_more_memory_than_freesurfer` (ds000005 sub-01's files, one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB):

- T5 (reproduce): 70 MiB, FreeSurfer 72 MiB.
- T5 (fix): 70 MiB, FreeSurfer 72 MiB.
- invert (reproduce): 166 MiB, FreeSurfer 168 MiB.
- invert (fix): 166 MiB, FreeSurfer 168 MiB.
