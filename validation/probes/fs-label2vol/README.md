# fs-label2vol: `mri_label2vol` against FreeSurfer 8.2.0's official program

**Closed to FreeSurfer clean-room implementers**: the scripts run the replica's example and name
its defect handling (the behaviour is `specs/mri_label2vol.md`, whose §9 says how a clean room
checks itself).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes` (`src/mri_label2vol/`,
with fs.surf's `defects2seg.rs`) gives the official program's results bit for bit with
`--upstream-defects reproduce` (exit status, standard output, standard error, the files written),
case by case, and differs by default only where a defect it corrects is noted. Queue item 3.2-40,
track A relays 56 (part 1) and 57 (part 2: `--new-aseg2vol` and `--fill-ribbon`): every option
and path.

## Files

- `check.py --seed <s> --n <n>`: the port's example `fs_label2vol_probe` generates the cases (the
  fixed ones: the command line's oddities, every value-taking option without its value, every
  check and refused combination, the template and registration paths, labels, projections,
  annotations, segmentations, `--defects`, the environment, more than 100 labels, then part 2's:
  `--new-aseg2vol` and `--fill-ribbon` on every path; then seeded command lines of the six kinds)
  with their files: templates of four voxel types and five grids
  (one with a NaN voxel size), labels in tkregister and scanner coordinates (NaN, infinite and
  huge coordinates, an empty and a broken label, surface labels with points beyond the surface), a
  closed surface of 66 vertices (an octahedron divided twice) with and without its volume
  geometry, its thickness, defect numbers (NaN and infinities from their bits, negative, near the
  `int` range, a short overlay), an annotation with its colour table, segmentations with and
  without one, `register.dat` and `.lta` files, a subject's folder; part 2's (a generator of their
  own, so part 1's inputs stay as they were): the ribbons `subj/mri/ribbon.mgz` (3, 42 and other
  values), `sd2/subj/` (a second subjects folder), `subjfar/` (a ribbon of 8 mm voxels far from the
  surface), `y.lta` (the ribbon at the destination) and `z.lta` (at neither end), segmentations
  with labels below 0, a label of 10^6, labels without a tissue type (`fsh2/`'s colour table) or
  not in the colour table, one of 0.4 mm voxels and one of 0 only, a dense annotation and a
  surface label whose stats are NaN and infinite (`snan.label`); the official `mri_label2vol`
  (`$FREESURFER_HOME/bin`, by name, `FS_DISABLE_LANG=1`, the licence from `FS_LICENSE`) runs each
  through the oracle-output cache at one thread, every case in the same folder
  (`<inputs>/fs-label2vol/case`, the layout keyed as an input), with a 30 s limit. Every case runs
  with `SUBJECTS_DIR` the case's folder and `FREESURFER_SEED=1234` (the defects' colour table takes
  random colours); the `--ttype` and `--fill-ribbon` cases with `FREESURFER_HOME` a folder of the
  case holding a small `FreeSurferColorLUT.txt` (both sides read the same table; a case without
  `FREESURFER_HOME` is not compared: the official program then stops in its licence check, which
  the replica does not have). The port runs the cases in both modes;
  compared line by line with the folder as `<DIR>`, written volumes decompressed and their
  recorded command lines (`getAllInfo`'s: time, user, machine) masked as `<CMD>`. `SKIP`: `CRASH`
  (the port's status is one FreeSurfer's build reaches by a signal: the official run's unflushed
  standard output is lost), `MEMORY` (FreeSurfer's result comes from memory contents: FS-53,
  FS-249, FS-441, FS-1252, FS-1256, FS-1258, FS-1259, FS-1263, FS-1265) and `HANG` (FS-1257: the
  official run times out). `MRIseg2SegPVF(): done t = <seconds>` is masked (and volatile in the
  oracle-output cache).
  `--golden`: the golden cases' official SHA-256 (`tests/mri_label2vol_golden.rs`). `--dump <dir>`:
  every line of the three sides. No case sets `DIAG_NO` or any diagnostic level (FreeSurfer's
  licence check prints the licence's lines when the diagnostic number is above 0: the script
  refuses such a case).
- `real.py --port <larmorx-freesurfer>`: `defect2seg`'s two calls per run on the six classic and
  six v8 recorded runs (the run's own surfaces, defect numbers, cortex labels and `orig.mgz`; the
  right hemisphere's call on the left's official output), the official volume's header and voxels
  against the run's recorded `mri/surface.defects.mgz`, and on the v8 runs `seg2cc`'s form (`--seg aseg.mgz
  --temp rawavg.mgz --regheader`, then that output back onto `norm.mgz`) and part 2's
  (`--new-aseg2vol --ttype --resmm 0.5` onto `rawavg.mgz`; `--fill-ribbon` with `lh.cortex.label`
  and `--label-stat`, and with `rh.aparc.annot`, on which FreeSurfer crashes: FS-1268, counted as
  matched when `reproduce` ends with the same signal's status), in `<inputs>/fs-label2vol-real/`.
- `record.py`: `docs/validation/freesurfer-mri-label2vol.{json,md}`.

## Results (relay 57; the record `docs/validation/freesurfer-mri-label2vol.md`)

Seed 1, 600 cases: 551 identical with `reproduce`, 49 `SKIP` (FreeSurfer's crashes, memory
contents, a hang), 0 different; `fix` differs on 116, each with a notice. Seed 2, 1,500 cases:
1,381 identical, 119 `SKIP`, 0 different; `fix` differs on 264, each with a notice. The fixed cases
alone (240): 214 identical, 26 `SKIP`, 0 different. The recorded heads: 54 calls on 12 runs
(`defect2seg`'s 24, `seg2cc`'s form 12, part 2's 18 on the v8 runs), all 54 identical with
`reproduce` (the 6 `--fill-ribbon --annot rh.aparc.annot` calls: the official program crashes,
FS-1268, and `reproduce` ends with status 139), the default identical on 48 and different with
FS-1268's notice on 6; the 12 official defect volumes equal to the recordings. Recorded test:
`tests/fs_label2vol_official.rs` (seed 1, 600 cases, 49 `SKIP`, `bf8ecdd7...`). Golden: 15 cases,
`f50d82be...` (official and `reproduce`), `17475818...` (`fix`). Memory: the seven calls of
`test_memory_peak.py` within the rule (`--new-aseg2vol --ttype --resmm 0.5` 135 MiB against
FreeSurfer's 206, `--fill-ribbon` 248 against 267). Speed (one thread, ds000005 v8): `--new-aseg2vol`
3.7 s against FreeSurfer's 2.8 s, `--fill-ribbon` 14.2 s against 5.1 s (a speed item). The Windows
cross-check (TMP and TEMP behind a junction): the recorded test, the golden test and the crate's
tests pass.

Relay 56 (part 1): seed 1 589 identical, 11 `SKIP`; seed 2 1,489 identical, 11 `SKIP`; 0 different;
the recorded calls 36 of 36; the Windows cross-check needed a NIfTI output's voxel offset and
extension size masked with its command lines, and fs.volume's rounding without the C library's
`roundf` (MinGW's rounds 0.49999997 up).

    scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-label2vol/check.py --seed 1 --n 600
    scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-label2vol/check.py --golden
    scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-label2vol/real.py --port <larmorx-freesurfer>
