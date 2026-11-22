# fs-temp-file: `fs_temp_file` against FreeSurfer 8.2.0's official program

**Closed to FreeSurfer clean-room implementers**: the scripts run the replica's example and name
its defect handling (the behaviour is `specs/fs_temp_file.md`, whose §6 says how a clean room checks
itself).

What it shows: the replica `crates-freesurfer/larmorx-freesurfer-volumes` (`src/fs_temp_file/`)
gives the official program's results with `--upstream-defects reproduce` (exit status, standard
output, standard error, the files made and their modes), case by case, and differs by default only
where FS-1171 is noted. Queue item 3.2-11 (track A, relay 52).

## Files

- `check.py [--golden] [--json out.json]`: the port's example `fs_tempfile_probe` gives the cases
  (`tests/fs_tempfile/cases.rs`); the official `fs_temp_file` (`$FREESURFER_HOME/bin`, by name,
  `FS_DISABLE_LANG=1`, no `DIAG_NO`) runs each through the oracle-output cache, never stored (six
  random letters), in a folder of its own with only the case's variables; the port runs them in
  both modes; compared line by line with the folder as `<DIR>`, the letters as `XXXXXX`,
  `--all-info`'s time, user and machine as `*`. Prints the two hashes of the recorded test (Unix's;
  without modes and the Unix-only cases). Needs a machine without `/scratch`.
- `record.py`: `docs/validation/freesurfer-fs-temp-file.{json,md}` (with recon-all's recorded
  call, read from the reference runs' `recon-all.out`).

## Results (relay 52)

73 cases: 73 identical with `reproduce`, 0 different; `fix` differs on 6, each with FS-1171's
notice. Golden: 5 cases, `86cabb94...` (without modes `82aae397...`), both modes. Recorded:
`tests/fs_temp_file_official.rs`. recon-all's call: `/tmp/tmp.XXXXXX` in all 18 reference runs.

    scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-temp-file/check.py --json all.json
    scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-temp-file/check.py --golden --json golden.json
