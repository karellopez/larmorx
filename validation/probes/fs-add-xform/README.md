# fs-add-xform: `mri_add_xform_to_header` against FreeSurfer 8.2.0's official program

**Closed to FreeSurfer clean-room implementers**: the scripts run the replica's example and name
its defect handling (the behaviour is `specs/mri_add_xform_to_header.md`, whose §6 says how a
clean room checks itself).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes`
(`src/mri_add_xform_to_header/`) gives the official program's results bit for bit with
`--upstream-defects reproduce` (exit status, standard output, standard error, every file and folder
after the run), case by case, and differs by default only where a defect it corrects is noted.
Queue item 3.2-10 (track A, relay 50).

## Files

- `check.py --seed <s> --n <n>`: the port's example `fs_xform_probe` generates the cases (the
  fixed ones: the command line's paths, every input, transform file and output, the environment,
  a long transform name; then seeded random volumes and options); the official
  `mri_add_xform_to_header` (`$FREESURFER_HOME/bin`, with `FS_DISABLE_LANG=1`:
  `docs/findings/freesurfer-info.md`, the locale and `errno`) runs each in a folder of its own
  through the oracle-output cache at one thread; the port runs them in both modes; compared line
  by line with the folder's name as `<DIR>`. `SKIP`: FS-1154 and FS-1156 (the official build
  aborts), FS-58 (a crash), FS-167 and FS-441 (memory contents). `--golden`: the golden cases'
  official SHA-256 (`tests/mri_add_xform_to_header_golden.rs`). `--long`: the cases whose paths
  pass 1,024 bytes (not in the recorded test, macOS's paths stop there). No case sets `DIAG_NO`
  (the licence check then prints the licence file); the cache keeps the extension text of each
  argument for this program (`oracle_cache.policy.EXTENSION_FROM_WHOLE_PATH`:
  `docs/findings/freesurfer-add-xform.md`).
- `replay.sh <larmorx-freesurfer> <work>` (`MODE=reproduce|fix|official`): recon-all's two
  recorded steps on every recorded run.
- `record.py`: `docs/validation/freesurfer-mri-add-xform-to-header.{json,md}`.

## Results (relay 50)

Seed 1, 600 cases: 561 identical with `reproduce`, 39 `SKIP`, 0 different; `fix` differs on 351,
each with a notice (FS-01, FS-1155, FS-1157, FS-1158). Seed 3, 1,000 cases: 938 identical, 62
`SKIP`, 0 different. Long cases: 2, both `SKIP` (FS-1156; the official build aborts, measured).
Recorded: `tests/fs_xform_official.rs` (seed 1, 600 cases, 39 `SKIP`, `e5aa197c...`; debug
9.6 s). Golden: 5 cases, `92ff34ba...` with `reproduce` (the official value), `8c733367...` by
default (FS-1158). The recorded steps: 24 on 12 runs, all reproduced, both modes.

    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-add-xform/check.py --seed 1 --n 600
    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-add-xform/check.py --golden
    MODE=fix scripts/orchestration/boxed.sh 20 validation/probes/fs-add-xform/replay.sh <larmorx-freesurfer> <work>
