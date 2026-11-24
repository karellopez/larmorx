# fs-label2vol: `mri_label2vol` against FreeSurfer 8.2.0's official program

**Closed to FreeSurfer clean-room implementers**: the scripts run the replica's example and name
its defect handling (the behaviour is `specs/mri_label2vol.md`, whose §9 says how a clean room
checks itself).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes` (`src/mri_label2vol/`,
with fs.surf's `defects2seg.rs`) gives the official program's results bit for bit with
`--upstream-defects reproduce` (exit status, standard output, standard error, the files written),
case by case, and differs by default only where a defect it corrects is noted. Queue item 3.2-40
part 1, track A relay 56: every option and path but `--new-aseg2vol` and `--fill-ribbon` (part 2).

## Files

- `check.py --seed <s> --n <n>`: the port's example `fs_label2vol_probe` generates the cases (the
  fixed ones: the command line's oddities, every value-taking option without its value, every
  check and refused combination, the template and registration paths, labels, projections,
  annotations, segmentations, `--defects`, the environment, more than 100 labels; then seeded
  command lines of the four kinds) with their files: templates of four voxel types and five grids
  (one with a NaN voxel size), labels in tkregister and scanner coordinates (NaN, infinite and
  huge coordinates, an empty and a broken label, surface labels with points beyond the surface), a
  closed surface of 66 vertices (an octahedron divided twice) with and without its volume
  geometry, its thickness, defect numbers (NaN and infinities from their bits, negative, near the
  `int` range, a short overlay), an annotation with its colour table, segmentations with and
  without one, `register.dat` and `.lta` files, a subject's folder; the official `mri_label2vol`
  (`$FREESURFER_HOME/bin`, by name, `FS_DISABLE_LANG=1`, the licence from `FS_LICENSE`) runs each
  through the oracle-output cache at one thread, every case in the same folder
  (`<inputs>/fs-label2vol/case`, the layout keyed as an input), with a 30 s limit. Every case runs
  with `SUBJECTS_DIR` the case's folder and `FREESURFER_SEED=1234` (the defects' colour table takes
  random colours); the `--ttype` cases with `FREESURFER_HOME` a folder of the case holding a small
  `FreeSurferColorLUT.txt` (both sides read the same table). The port runs the cases in both modes;
  compared line by line with the folder as `<DIR>`, written volumes decompressed and their
  recorded command lines (`getAllInfo`'s: time, user, machine) masked as `<CMD>`. `SKIP`: `CRASH`
  (the port's status is one FreeSurfer's build reaches by a signal: the official run's unflushed
  standard output is lost), `MEMORY` (FreeSurfer's result comes from memory contents: FS-53,
  FS-249, FS-1252, FS-1256, FS-1258, FS-1259) and `HANG` (FS-1257: the official run times out).
  `--golden`: the golden cases' official SHA-256 (`tests/mri_label2vol_golden.rs`). `--dump <dir>`:
  every line of the three sides. No case sets `DIAG_NO` or any diagnostic level (FreeSurfer's
  licence check prints the licence's lines when the diagnostic number is above 0: the script
  refuses such a case).
- `real.py --port <larmorx-freesurfer>`: `defect2seg`'s two calls per run on the six classic and
  six v8 recorded runs (the run's own surfaces, defect numbers, cortex labels and `orig.mgz`; the
  right hemisphere's call on the left's official output), the official volume's header and voxels
  against the run's recorded `mri/surface.defects.mgz`, and on the v8 runs `seg2cc`'s form (`--seg aseg.mgz
  --temp rawavg.mgz --regheader`, then that output back onto `norm.mgz`), in
  `<inputs>/fs-label2vol-real/`.
- `record.py`: `docs/validation/freesurfer-mri-label2vol.{json,md}`.

## Results (relay 56; the record `docs/validation/freesurfer-mri-label2vol.md`, from 880c6687)

Seed 1, 600 cases: 589 identical with `reproduce`, 11 `SKIP` (4 CRASH, 6 MEMORY, 1 HANG), 0
different; `fix` differs on 119, each with a notice. Seed 2, 1,500 cases: 1,489 identical, 11
`SKIP`, 0 different; `fix` differs on 292, each with a notice. The recorded calls: 36 on 12 runs
(`defect2seg`'s 24, `seg2cc`'s form 12 on the v8 runs), all identical in both modes, the 12
official defect volumes' header and voxels equal to the runs' recorded `surface.defects.mgz`.
Recorded test: `tests/fs_label2vol_official.rs` (seed 1, 600 cases, 11 `SKIP`, `47e3dc16...`).
Golden: 9 cases, `aeabe265...` (official and `reproduce`), `9cb289bf...` (`fix`). Memory: the five
calls of `test_memory_peak.py` within the rule (`defect2seg`'s left call 242.4 MiB, FreeSurfer's
226.1). The Windows cross-check (`validation/investigations/windows-cross-check/`): the 600 lines of
seed 1 identical to Linux's once a NIfTI output's voxel offset and extension size are masked with
its command lines, and fs.volume rounds without the C library's `roundf` (MinGW's rounds 0.49999997
up).

    scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-label2vol/check.py --seed 1 --n 600
    scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-label2vol/check.py --golden
    scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-label2vol/real.py --port <larmorx-freesurfer>
