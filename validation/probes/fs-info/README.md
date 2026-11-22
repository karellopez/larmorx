# fs-info: `mri_info` against FreeSurfer 8.2.0's official program

**Closed to FreeSurfer clean-room implementers**: the scripts run the replica's example and name
its defect handling (the behaviour is `specs/mri_info.md`, whose §6 says how a clean room checks
itself).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes` (`src/mri_info/`) gives
the official program's results bit for bit with `--upstream-defects reproduce` (exit status,
standard output, standard error, the files written), case by case, and differs by default only
where a defect it corrects is noted. Queue item 3.2-08 (track A, relay 49).

## Files

- `check.py --seed <s> --n <n>`: the port's example `fs_info_probe` generates the cases (271
  fixed ones: every option on two volumes, every format, morphs and warp fields, failures; then
  seeded); the official `mri_info` (`$FREESURFER_HOME/bin`, with `FS_DISABLE_LANG=1`:
  `docs/findings/freesurfer-info.md`, the locale and `errno`) runs each in a folder of its own
  through the oracle-output cache at one thread; the port runs them in both modes; compared line
  by line with the folder's name as `<DIR>`. `SKIP`: FS-1148, FS-1149, FS-1150, FS-1152 (crashes,
  a build-dependent result) and FS-53 (a voxel outside the volume). `--golden`: the golden cases'
  official SHA-256 (`tests/mri_info_golden.rs`). It refuses a case that would read a volume with
  `DIAG_NO` set (the licence check then prints the licence file).
- `replay.py --port <larmorx-freesurfer>`: recon-all's and seg2cc's calls of `mri_info` on every
  recorded run, against the logs and the official program.
- `record.py`: `docs/validation/freesurfer-mri-info.{json,md}`.

## Results (relay 49)

Seed 1, 600 cases: 587 identical with `reproduce`, 13 `SKIP`, 0 different; `fix` differs on 104,
each with a notice (FS-01, FS-1146, FS-1147, FS-1148, FS-1151, FS-788). Seed 3, 1,000 cases: 980
identical, 20 `SKIP`, 0 different. Recorded: `tests/fs_info_official.rs` (seed 1, 600 cases, 13
`SKIP`, `57631f5f...`; debug 4.5 s). Golden: 32 cases, `e6f79174...`, both modes. The recorded
calls: 72 on 18 runs, all identical to the official program and the logs, both modes.

    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-info/check.py --seed 1 --n 600
    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-info/check.py --golden
    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-info/replay.py --port <larmorx-freesurfer>
