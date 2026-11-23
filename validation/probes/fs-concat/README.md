# fs-concat: `mri_concat` against FreeSurfer 8.2.0's official program

**Closed to FreeSurfer clean-room implementers**: the scripts run the replica's example and name
its defect handling (the behaviour is `specs/mri_concat.md`, whose §7 says how a clean room checks
itself).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes` (`src/mri_concat/`)
gives the official program's results bit for bit with `--upstream-defects reproduce` (exit status,
standard output, standard error, the files written), case by case, and differs by default only
where a defect it corrects is noted. Queue item 3.2-39, track A relay 55: the whole program.

## Files

- `check.py --seed <s> --n <n>`: the port's example `fs_concat_probe` generates the cases (the
  fixed ones: the command line's oddities, every refused combination, every option and reduction
  on a four-frame volume, a volume with zeros and three inputs, the edge values of each value
  option, the matrices, weights, masks, `--in-frame`, `--no-check`, `--zconcat` and `--f` lists;
  then seeded command lines) with their files: 4 x 5 x 3 volumes of four voxel types and one to six
  frames in MGZ, NIfTI and gzipped NIfTI, segmentations with and without a colour table, volumes
  with zeros, NaN and infinite voxels, surface overlays (`pctsurfcon`'s form), a volume of another
  grid, masks of both grids, an MGH file cut after its header, colour tables, list files, matrices
  and weights; the official `mri_concat` (`$FREESURFER_HOME/bin`, by name, `FS_DISABLE_LANG=1`,
  the licence from `FS_LICENSE`) runs each through the oracle-output cache at one thread, every
  case in the same folder (`<inputs>/fs-concat/case`, the layout keyed as an input), with a 30 s
  limit; a case with `--debug` or `--rusage` (the run's resource usage) is never cached, and those
  lines and files are masked. The port runs the cases in both modes; compared line by line with
  the folder as `<DIR>`. `SKIP`: `CRASH` (the port's status is one FreeSurfer's build reaches by a
  signal, or FS-564's and FS-13's ends, where FreeSurfer crashes: the official run's unflushed
  standard output is lost) and `MEMORY` (FreeSurfer's result comes from memory contents or is
  undefined: FS-53, FS-249, FS-1196, FS-1243, FS-1247). `--golden`: the golden cases' official
  SHA-256 (`tests/mri_concat_golden.rs`). `--dump <dir>`: every line of the three sides. No case
  sets `DIAG_NO` or any diagnostic level (FreeSurfer's licence check prints the licence's lines
  when the diagnostic number is above 0: the script refuses such a case).
- `real.py --port <larmorx-freesurfer>`: `pctsurfcon`'s two calls per run on the six classic and
  six v8 recorded runs (the white and grey matter samples made again by the official
  `mri_vol2surf` with the run's own commands: the official percent contrast is then the run's
  recorded `surf/<h>.w-g.pct.mgh`), and `mri_mcadura_seg`'s sum of two segmentations with
  `mca-dura.ctab` on the v8 runs (the head's `aseg.mgz` taken onto each hemisphere's crop and back
  by the official programs, in `<inputs>/fs-concat-real/`).
- `long_names.sh <empty folder>`: the official program with `--f` list names of 1,998 to 2,013
  bytes (nested folders) and lists of 400,000 to 401,000 words (FS-1197, FS-1198; paths too long
  for macOS, so not in the recorded test).
- `record.py`: `docs/validation/freesurfer-mri-concat.{json,md}`.

## Results (relay 55; the record `docs/validation/freesurfer-mri-concat.md`)

Seed 1, 600 cases: 569 identical with `reproduce`, 31 `SKIP`, 0 different; `fix` differs on 64,
each with a notice. Seed 2, 1,500 cases: 1,435 identical, 65 `SKIP`, 0 different; `fix` differs on
201, each with a notice. The recorded calls: 30 on 12 runs, all identical in both modes, the 24
official percent contrasts equal to the runs' recorded files. Recorded test:
`tests/fs_concat_official.rs` (seed 1, 600 cases, 31 `SKIP`, `87569463...`). Golden: 15 cases,
`58329896...` (official and `reproduce`), `e4af9f7a...` (`fix`). `long_names.sh`: names of 2,000
to 2,008 bytes run normally, 2,009 to 2,012 abort (`*** stack smashing detected ***`, status 134);
400,000 list words read the first input, 400,001 and more crash (status 139).

    scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-concat/check.py --seed 1 --n 600
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-concat/check.py --golden
    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-concat/real.py --port <larmorx-freesurfer>
    scripts/orchestration/boxed.sh 4 validation/probes/fs-concat/long_names.sh <empty folder>
